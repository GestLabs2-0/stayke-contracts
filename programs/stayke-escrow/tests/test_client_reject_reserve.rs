mod common;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, AnchorDeserialize, InstructionData,
        ToAccountMetas,
    },
    common::*,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_escrow::state::BookingStatus,
};

// ---------------------------------------------------------------------------
// Helpers — timestamp constants
// ---------------------------------------------------------------------------

const CHECK_IN_JAN_1_2025: i64 = 1735689600;
const CHECK_OUT_JAN_5_2025: i64 = 1736035200;
const CHECK_IN_DEC_28_2025: i64 = 1766880000;
const CHECK_OUT_JAN_3_2026: i64 = 1767398400;

// ===========================================================================
// Single-year — happy path (status = Pending)
// ===========================================================================

#[test]
fn client_reject_reserve_pending_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days = [0u32; 12];
    days[0] = stayke_escrow::utils::bitmap_days(1, 5);
    setup_booking_days(&mut svm, property, 2025, days);

    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserve {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserve {
            payer: payer.pubkey(),
            client: client.pubkey(),
            client_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &client]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful client_reject_reserve, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&booking).is_none());

    let bd_data: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(bd_data.occupied_days[0], 0);
}

// ===========================================================================
// Single-year — happy path (status = HostAccepted)
// ===========================================================================

#[test]
fn client_reject_reserve_host_accepted_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days = [0u32; 12];
    days[0] = stayke_escrow::utils::bitmap_days(1, 5);
    setup_booking_days(&mut svm, property, 2025, days);

    // HostAccepted is also a valid rejection status for the client.
    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::HostAccepted,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserve {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserve {
            payer: payer.pubkey(),
            client: client.pubkey(),
            client_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &client]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected client_reject_reserve for HostAccepted to succeed, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&booking).is_none());
}

// ===========================================================================
// Single-year — error: wrong booking status
// ===========================================================================

#[test]
fn client_reject_reserve_active_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Active,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserve {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserve {
            payer: payer.pubkey(),
            client: client.pubkey(),
            client_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &client]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingStatus
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: wrong client (unauthorized)
// ===========================================================================

#[test]
fn client_reject_reserve_wrong_client_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let wrong_client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let wrong_client_profile = setup_user_profile(&mut svm, wrong_client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    // Booking owned by `client`, not `wrong_client`.
    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserve {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserve {
            payer: payer.pubkey(),
            client: wrong_client.pubkey(),
            client_profile: wrong_client_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer, &wrong_client],
    )
    .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::UnauthorizedBooking
            ))
        )
    );
}

// ===========================================================================
// Cross-year — happy path
// ===========================================================================

#[test]
fn client_reject_reserve_cross_year_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days_2025 = [0u32; 12];
    days_2025[11] = stayke_escrow::utils::bitmap_days(28, 31);
    setup_booking_days(&mut svm, property, 2025, days_2025);

    let mut days_2026 = [0u32; 12];
    days_2026[0] = stayke_escrow::utils::bitmap_days(1, 3);
    setup_booking_days(&mut svm, property, 2026, days_2026);

    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserveCrossYear {
            check_in: CHECK_IN_DEC_28_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserveCrossYear {
            payer: payer.pubkey(),
            client: client.pubkey(),
            client_profile,
            booking,
            booking_days: bd,
            booking_days_next: bd_next,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &client]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected client_reject_reserve_cross_year to succeed, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&booking).is_none());

    let bd_2025: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(bd_2025.occupied_days[11], 0);

    let bd_2026: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd_next).unwrap().data[8..]).unwrap();
    assert_eq!(bd_2026.occupied_days[0], 0);
}

// ===========================================================================
// Cross-year — error: single-year instruction with cross-year dates
// ===========================================================================

#[test]
fn client_reject_reserve_single_year_fails_cross_year_booking() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let client = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let client_profile = setup_user_profile(&mut svm, client.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    let booking = setup_booking_at_pda(
        &mut svm,
        client_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ClientRejectReserve {
            check_in: CHECK_IN_DEC_28_2025,
        }
        .data(),
        stayke_escrow::accounts::ClientRejectReserve {
            payer: payer.pubkey(),
            client: client.pubkey(),
            client_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &client]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::SingleYearUnbookingInvalid
            ))
        )
    );
}
