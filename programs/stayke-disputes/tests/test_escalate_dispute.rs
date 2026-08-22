mod common;

use {
    anchor_lang::{
        prelude::Clock, solana_program::instruction::Instruction, AnchorDeserialize,
        InstructionData, ToAccountMetas,
    },
    common::*,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_disputes::state::{DisputeAccount, DisputeState},
    stayke_escrow::state::BookingStatus,
};

/// Fixed baseline timestamp used to drive the P2P window deterministically.
const T0: i64 = 1_735_689_600;

fn user_profile_pda(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &stayke_core::id(),
    )
    .0
}

fn open_dispute_ix(payer: Pubkey, initiator: Pubkey, booking: Pubkey) -> Instruction {
    let dispute = dispute_pda(booking);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {}.data(),
        stayke_disputes::accounts::OpenDispute {
            payer,
            initiator,
            initiator_profile: user_profile_pda(initiator),
            booking,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    )
}

fn escalate_dispute_ix(dispute: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::EscalateDispute {}.data(),
        stayke_disputes::accounts::EscalateDispute { dispute }.to_account_metas(None),
    )
}

/// Bootstraps an OpenP2P dispute: sets up config/profile/booking, opens the
/// dispute at clock `T0`, and returns the booking key.
fn setup_open_dispute(svm: &mut litesvm::LiteSVM, payer: &Keypair, guest: &Keypair) -> Pubkey {
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, T0);

    setup_global_config(svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(svm, guest.pubkey());
    setup_booking(
        svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        T0,
        BookingStatus::Active,
    );

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = T0;
    svm.set_sysvar::<Clock>(&clock);

    let instruction = open_dispute_ix(payer.pubkey(), guest.pubkey(), booking_key);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, guest]).unwrap();
    svm.send_transaction(tx).unwrap();

    booking_key
}

fn set_clock(svm: &mut litesvm::LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar::<Clock>(&clock);
}

fn send_escalate(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    booking: Pubkey,
) -> Result<(), TransactionError> {
    let instruction = escalate_dispute_ix(dispute_pda(booking));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

fn read_dispute(svm: &litesvm::LiteSVM, booking: Pubkey) -> DisputeAccount {
    let dispute = dispute_pda(booking);
    let account = svm
        .get_account(&dispute)
        .expect("dispute account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize DisputeAccount")
}

#[test]
fn escalate_after_window_succeeds_permissionless() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    // Arbitrary caller with no relationship to the dispute or its booking.
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    let now = T0 + stayke_disputes::constants::DISPUTE_P2P_WINDOW_SECONDS + 1;
    set_clock(&mut svm, now);

    let res = send_escalate(&mut svm, &stranger, booking);
    assert!(
        res.is_ok(),
        "permissionless escalate after window should succeed, got: {:?}",
        res.err()
    );

    let disp = read_dispute(&svm, booking);
    assert_eq!(disp.state, DisputeState::Escalated);
}

#[test]
fn escalate_before_window_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    // Clock still at T0, far inside the 24h window.
    let res = send_escalate(&mut svm, &payer, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::EscalationWindowNotElapsed
            ))
        )
    );

    assert_eq!(read_dispute(&svm, booking).state, DisputeState::OpenP2P);
}

#[test]
fn escalate_exactly_at_window_boundary_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    // now == opened_at + window: escalation requires strictly greater.
    let boundary = T0 + stayke_disputes::constants::DISPUTE_P2P_WINDOW_SECONDS;
    set_clock(&mut svm, boundary);

    let res = send_escalate(&mut svm, &payer, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::EscalationWindowNotElapsed
            ))
        )
    );
}

#[test]
fn escalate_not_open_p2p_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    // First escalation succeeds, moving the dispute to Escalated.
    set_clock(
        &mut svm,
        T0 + stayke_disputes::constants::DISPUTE_P2P_WINDOW_SECONDS + 1,
    );
    let res = send_escalate(&mut svm, &payer, booking);
    assert!(res.is_ok(), "first escalation failed: {:?}", res.err());

    // Use a different payer so the transaction signature differs.
    let other_payer = Keypair::new();
    svm.airdrop(&other_payer.pubkey(), 1_000_000_000).unwrap();

    // Escalating again must fail because state is no longer OpenP2P.
    let res = send_escalate(&mut svm, &other_payer, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotOpenP2P
            ))
        )
    );

    assert_eq!(read_dispute(&svm, booking).state, DisputeState::Escalated);
}
