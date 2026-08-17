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
    stayke_disputes::state::DisputeReason,
    stayke_escrow::state::BookingStatus,
};

/// Compute the UserProfile PDA for a given authority pubkey.
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

#[test]
// #[ignore = "Requires aligned program binary IDs between test crate and .so for stayke-escrow CPI"]
fn open_dispute_guest_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Active,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = 1735689600;
    svm.set_sysvar::<Clock>(&clock);

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::PropertyNotAsDescribed,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful open_dispute, got: {:?}",
        res.err()
    );

    let account = svm.get_account(&dispute).unwrap();
    assert_eq!(account.owner, stayke_disputes::id());
    let disp: stayke_disputes::state::Dispute =
        AnchorDeserialize::deserialize(&mut &account.data[8..]).unwrap();

    println!("Time: {:?}", disp.created_at);
    assert_eq!(disp.booking, booking_key);
    assert_eq!(disp.property, property);
    assert_eq!(disp.initiator, guest_profile_pda);
    assert_eq!(disp.guilty, host_profile_pda);
    assert!(matches!(disp.reason, DisputeReason::PropertyNotAsDescribed));
    assert!(matches!(
        disp.status,
        stayke_disputes::state::DisputeStatus::Open
    ));
    assert_eq!(disp.created_at, 1735689600);
    assert_eq!(disp.resolved_at, None);
}

#[test]
// #[ignore = "Requires aligned program binary IDs between test crate and .so for stayke-escrow CPI"]
fn open_dispute_host_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, host.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Active,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::HostUnreachable,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: host.pubkey(),
            initiator_profile: host_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful open_dispute by host, got: {:?}",
        res.err()
    );

    let account = svm.get_account(&dispute).unwrap();
    let disp: stayke_disputes::state::Dispute =
        AnchorDeserialize::deserialize(&mut &account.data[8..]).unwrap();
    assert_eq!(disp.initiator, host_profile_pda);
    assert_eq!(disp.guilty, guest_profile_pda);
    assert!(matches!(disp.reason, DisputeReason::HostUnreachable));
}

#[test]
fn open_dispute_booking_not_active_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Pending,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::PropertyNotAsDescribed,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
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

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(&mut svm, stranger.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Active,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;
    let stranger_profile_pda = user_profile_pda(stranger.pubkey());

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::PropertyNotAsDescribed,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: stranger.pubkey(),
            initiator_profile: stranger_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &stranger]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
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

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_banned_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Active,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::PropertyNotAsDescribed,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
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

    let booking_key = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());

    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());
    setup_unverified_user_profile(&mut svm, guest.pubkey());
    setup_booking(
        &mut svm,
        booking_key,
        guest_profile_pda,
        host_profile_pda,
        property,
        BookingStatus::Active,
    );

    let dispute = dispute_pda(booking_key);
    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let global_cfg = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {
            reason: DisputeReason::PropertyNotAsDescribed,
        }
        .data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile_pda,
            booking: booking_key,
            dispute,
            cpi_authority: cpi_auth,
            global_config: global_cfg,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UserNotVerified
            ))
        )
    );
}
