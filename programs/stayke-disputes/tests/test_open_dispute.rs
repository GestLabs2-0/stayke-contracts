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
    stayke_disputes::state::{DisputeAccount, DisputeParty, DisputeState},
    stayke_escrow::state::BookingStatus,
};

const CHECK_IN: i64 = 1_735_689_600;

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

fn send_and_read_dispute(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    initiator: &Keypair,
    booking: Pubkey,
) -> (Result<(), TransactionError>, Option<DisputeAccount>) {
    let instruction = open_dispute_ix(payer.pubkey(), initiator.pubkey(), booking);

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, initiator]).unwrap();
    let res = svm.send_transaction(tx);

    let dispute = dispute_pda(booking);
    let account = svm.get_account(&dispute);
    let disp = account.map(|a| {
        AnchorDeserialize::deserialize(&mut &a.data[8..]).expect("deserialize DisputeAccount")
    });

    (res.map(|_| ()).map_err(|e| e.err), disp)
}

#[test]
fn open_dispute_guest_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN;
    svm.set_sysvar::<Clock>(&clock);

    let (res, disp) = send_and_read_dispute(&mut svm, &payer, &guest, booking_key);
    assert!(
        res.is_ok(),
        "Expected successful open_dispute, got: {:?}",
        res.err()
    );

    let disp = disp.expect("dispute account must exist");
    assert_eq!(disp.booking, booking_key);
    assert_eq!(disp.opened_by, DisputeParty::Guest);
    assert_eq!(disp.state, DisputeState::OpenP2P);
    assert_eq!(disp.opened_at, CHECK_IN);
    assert_eq!(disp.guest_evidence, None);
    assert_eq!(disp.host_evidence, None);
    assert_eq!(disp.outcome, None);

    let booking: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking_key).unwrap().data[8..])
            .unwrap();
    assert_eq!(booking.status, BookingStatus::Disputed);
}

#[test]
fn open_dispute_host_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, host.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let (res, disp) = send_and_read_dispute(&mut svm, &payer, &host, booking_key);
    assert!(
        res.is_ok(),
        "Expected successful open_dispute by host, got: {:?}",
        res.err()
    );

    let disp = disp.expect("dispute account must exist");
    assert_eq!(disp.booking, booking_key);
    assert_eq!(disp.opened_by, DisputeParty::Host);
    assert_eq!(disp.state, DisputeState::OpenP2P);
    assert_eq!(disp.guest_evidence, None);
    assert_eq!(disp.host_evidence, None);
    assert_eq!(disp.outcome, None);
}

#[test]
fn open_dispute_booking_not_active_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Pending,
    );

    let (res, _) = send_and_read_dispute(&mut svm, &payer, &guest, booking_key);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::BookingNotDisputable
            ))
        )
    );
}

#[test]
fn open_dispute_stranger_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let stranger = Keypair::new();
    let property = Pubkey::new_unique();
    svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, stranger.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let (res, _) = send_and_read_dispute(&mut svm, &payer, &stranger, booking_key);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnauthorizedDisputeInitiator
            ))
        )
    );
}

#[test]
fn open_dispute_banned_user_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_banned_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let (res, _) = send_and_read_dispute(&mut svm, &payer, &guest, booking_key);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_disputes::error::DisputeError::UserBanned))
        )
    );
}

#[test]
fn open_dispute_unverified_user_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_unverified_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let (res, _) = send_and_read_dispute(&mut svm, &payer, &guest, booking_key);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UserNotVerified
            ))
        )
    );
}
