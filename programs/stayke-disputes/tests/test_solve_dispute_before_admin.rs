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

fn solve_dispute_ix(initiator: Pubkey, dispute: Pubkey, booking: Pubkey) -> Instruction {
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::SolveDisputeBeforeAdmin {}.data(),
        stayke_disputes::accounts::SolveDisputeBeforeAdmin {
            initiator,
            initiator_profile: user_profile_pda(initiator),
            dispute,
            booking,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
        }
        .to_account_metas(None),
    )
}

fn setup_open_dispute(svm: &mut litesvm::LiteSVM, payer: &Keypair, guest: &Keypair) -> Pubkey {
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(svm, guest.pubkey());
    setup_booking(
        svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN;
    svm.set_sysvar::<Clock>(&clock);

    let instruction = open_dispute_ix(payer.pubkey(), guest.pubkey(), booking_key);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, guest]).unwrap();
    svm.send_transaction(tx).unwrap();

    booking_key
}

fn send_solve(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    initiator: &Keypair,
    booking: Pubkey,
) -> Result<(), TransactionError> {
    let dispute = dispute_pda(booking);
    let instruction = solve_dispute_ix(initiator.pubkey(), dispute, booking);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, initiator]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

fn read_dispute(svm: &litesvm::LiteSVM, booking: Pubkey) -> DisputeAccount {
    let dispute = dispute_pda(booking);
    let account = svm
        .get_account(&dispute)
        .expect("dispute account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize DisputeAccount")
}

fn read_booking(svm: &litesvm::LiteSVM, booking: Pubkey) -> stayke_escrow::state::Booking {
    let account = svm
        .get_account(&booking)
        .expect("booking account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize Booking")
}

#[test]
fn solve_by_guest_before_window_succeeds() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    let res = send_solve(&mut svm, &payer, &guest, booking);
    assert!(
        res.is_ok(),
        "guest solve before window should succeed, got: {:?}",
        res.err()
    );

    let disp = read_dispute(&svm, booking);
    assert_eq!(disp.state, DisputeState::ResolvedByP2P);

    let bkg = read_booking(&svm, booking);
    assert_eq!(bkg.status, BookingStatus::Active);
}

#[test]
fn solve_by_host_before_window_succeeds() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let property = Pubkey::new_unique();
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

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN;
    svm.set_sysvar::<Clock>(&clock);

    let instruction = open_dispute_ix(payer.pubkey(), host.pubkey(), booking_key);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
    svm.send_transaction(tx).unwrap();

    let res = send_solve(&mut svm, &payer, &host, booking_key);
    assert!(
        res.is_ok(),
        "host solve before window should succeed, got: {:?}",
        res.err()
    );

    let disp = read_dispute(&svm, booking_key);
    assert_eq!(disp.state, DisputeState::ResolvedByP2P);

    let bkg = read_booking(&svm, booking_key);
    assert_eq!(bkg.status, BookingStatus::Active);
}

#[test]
fn solve_by_non_opener_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    // Create profiles for both parties so the initiator_profile constraint
    // resolves even for the non-opener.
    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, guest.pubkey());
    setup_user_profile(&mut svm, host.pubkey());

    let booking = {
        let property = Pubkey::new_unique();
        let guest_profile_pda = user_profile_pda(guest.pubkey());
        let host_profile_pda = user_profile_pda(host.pubkey());
        let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);
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

        let instruction = open_dispute_ix(payer.pubkey(), guest.pubkey(), booking_key);
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest])
            .unwrap();
        svm.send_transaction(tx).unwrap();

        booking_key
    };

    // Host tries to solve a dispute opened by guest.
    let res = send_solve(&mut svm, &payer, &host, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnauthorizedDisputeSolver
            ))
        )
    );
}

#[test]
fn solve_after_window_elapsed_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN + stayke_disputes::constants::DISPUTE_P2P_WINDOW_SECONDS + 1;
    svm.set_sysvar::<Clock>(&clock);

    let res = send_solve(&mut svm, &payer, &guest, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::P2PWindowElapsed
            ))
        )
    );
}

#[test]
fn solve_not_open_p2p_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking = setup_open_dispute(&mut svm, &payer, &guest);

    // First solve succeeds.
    let res = send_solve(&mut svm, &payer, &guest, booking);
    assert!(res.is_ok(), "first solve failed: {:?}", res.err());

    // Second solve must fail because state is ResolvedByP2P.
    let other_payer = Keypair::new();
    svm.airdrop(&other_payer.pubkey(), 1_000_000_000).unwrap();
    let res = send_solve(&mut svm, &other_payer, &guest, booking);
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotOpenP2P
            ))
        )
    );

    assert_eq!(
        read_dispute(&svm, booking).state,
        DisputeState::ResolvedByP2P
    );
}
