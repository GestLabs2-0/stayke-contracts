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

/// 2025-01-01 00:00 UTC
const CHECK_IN_JAN_1_2025: i64 = 1735689600;
/// 2025-01-05 00:00 UTC
const CHECK_OUT_JAN_5_2025: i64 = 1736035200;
/// 2025-12-28 00:00 UTC
const CHECK_IN_DEC_28_2025: i64 = 1766880000;
/// 2026-01-03 00:00 UTC
const CHECK_OUT_JAN_3_2026: i64 = 1767398400;

// ===========================================================================
// Single-year — happy path
// ===========================================================================

#[test]
fn host_reject_booking_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    // Day 1 of January is always the check-in month — but compute full range
    // to match the occupancy that reserve_days would produce.
    let mut days = [0u32; 12];
    days[0] = stayke_escrow::utils::bitmap_days(1, 5);
    setup_booking_days(&mut svm, property, 2025, days);

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBooking {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            guest: guest_profile,
            booking,
            booking_days: bd,
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
        "Expected successful host_reject_booking, got: {:?}",
        res.err()
    );

    // Booking account should be closed.
    assert!(
        svm.get_account(&booking).is_none(),
        "Booking account should be closed after rejection"
    );

    // BookingDays should have released day 1.
    let bd_account = svm.get_account(&bd).expect("BookingDays should still exist");
    let bd_data: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &bd_account.data[8..]).unwrap();
    assert_eq!(
        bd_data.occupied_days[0], 0,
        "Day 1 should be released"
    );
}

// ===========================================================================
// Single-year — error: wrong booking status
// ===========================================================================

#[test]
fn host_reject_booking_wrong_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days = [0u32; 12];
    days[0] = 1;
    setup_booking_days(&mut svm, property, 2025, days);

    // Booking is Active, not Pending.
    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Active,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBooking {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            guest: guest_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
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
// Single-year — error: wrong host (unauthorized)
// ===========================================================================

#[test]
fn host_reject_booking_wrong_host_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let wrong_host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let wrong_host_profile = setup_user_profile(&mut svm, wrong_host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days = [0u32; 12];
    days[0] = 1;
    setup_booking_days(&mut svm, property, 2025, days);

    // Booking owned by `host`, not `wrong_host`.
    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBooking {
            payer: payer.pubkey(),
            host: wrong_host.pubkey(),
            host_profile: wrong_host_profile,
            guest: guest_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer, &wrong_host],
    )
    .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingProperty
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: wrong guest passed for rent return
// ===========================================================================

#[test]
fn host_reject_booking_wrong_guest_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let wrong_guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let wrong_guest_profile = setup_user_profile(&mut svm, wrong_guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let mut days = [0u32; 12];
    days[0] = 1;
    setup_booking_days(&mut svm, property, 2025, days);

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    // Pass wrong_guest_profile as guest — should fail constraint
    // `guest.key() == booking.guest`.
    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {
            check_in: CHECK_IN_JAN_1_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBooking {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            guest: wrong_guest_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_escrow::error::EscrowError::WrongGuestPassed))
        )
    );
}

// ===========================================================================
// Cross-year — happy path
// ===========================================================================

#[test]
fn host_reject_booking_cross_year_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    // Dec 28–31 occupied in 2025 — compute mask with the actual bitmap_days.
    let mut days_2025 = [0u32; 12];
    days_2025[11] = stayke_escrow::utils::bitmap_days(28, 31);
    setup_booking_days(&mut svm, property, 2025, days_2025);

    // Jan 1–3 occupied in 2026.
    let mut days_2026 = [0u32; 12];
    days_2026[0] = stayke_escrow::utils::bitmap_days(1, 3);
    setup_booking_days(&mut svm, property, 2026, days_2026);

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
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
        &stayke_escrow::instruction::HostRejectBookingCrossYear {
            check_in: CHECK_IN_DEC_28_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBookingCrossYear {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            guest: guest_profile,
            booking,
            booking_days: bd,
            booking_days_next: bd_next,
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
        "Expected successful host_reject_booking_cross_year, got: {:?}",
        res.err()
    );

    // Booking closed.
    assert!(svm.get_account(&booking).is_none());

    // 2025 BookingDays — December days released.
    let bd_2025: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_2025.occupied_days[11], 0,
        "December days should be released"
    );

    // 2026 BookingDays — January days released.
    let bd_2026: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd_next).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_2026.occupied_days[0], 0,
        "January days should be released"
    );
}

// ===========================================================================
// Cross-year — error: single-year instruction with cross-year booking
// ===========================================================================

#[test]
fn host_reject_booking_single_year_fails_cross_year_booking() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    // Cross-year booking, but we call the *single-year* instruction.
    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
    );

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {
            check_in: CHECK_IN_DEC_28_2025,
        }
        .data(),
        stayke_escrow::accounts::HostRejectBooking {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            guest: guest_profile,
            booking,
            booking_days: bd,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
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
