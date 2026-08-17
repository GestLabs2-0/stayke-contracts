//! LiteSVM integration tests for `expire_booking` / `expire_booking_crossday`.
//!
//! The expire instructions let the guest close a *Pending* booking after more
//! than 24 hours have elapsed without the host accepting it, returning the
//! escrowed USDC to the guest and releasing the occupied calendar days.

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

/// Escrow total for the booking — matches `total_price` in the booking account.
const TOTAL_PRICE: u64 = 100_000;

// ---------------------------------------------------------------------------
// Helpers — account metas
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn expire_booking_accounts(
    payer: Pubkey,
    guest_profile: Pubkey,
    booking: Pubkey,
    escrow_token_account: Pubkey,
    guest_token_account: Pubkey,
    booking_days: Pubkey,
    usdc_mint: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::ExpireBooking {
        payer,
        guest: guest_profile,
        booking,
        escrow_token_account,
        guest_token_account,
        booking_days,
        global_config: global_config_pda(),
        mint: usdc_mint,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

#[allow(clippy::too_many_arguments)]
fn expire_booking_crossday_accounts(
    payer: Pubkey,
    guest_profile: Pubkey,
    booking: Pubkey,
    escrow_token_account: Pubkey,
    guest_token_account: Pubkey,
    booking_days: Pubkey,
    booking_days_next: Pubkey,
    usdc_mint: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::ExpireBookingCrossDays {
        payer,
        guest: guest_profile,
        booking,
        escrow_token_account,
        guest_token_account,
        booking_days,
        booking_days_next,
        global_config: global_config_pda(),
        mint: usdc_mint,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

/// Reads the SPL token account balance (u64 LE at offset 64..72).
fn token_balance(svm: &litesvm::LiteSVM, account: &Pubkey) -> u64 {
    let data = svm
        .get_account(account)
        .expect("token account should exist")
        .data;
    let mut amount = [0u8; 8];
    amount.copy_from_slice(&data[64..72]);
    u64::from_le_bytes(amount)
}

// ===========================================================================
// Single-year — happy path
// ===========================================================================

#[test]
fn expire_booking_releases_days_transfers_funds_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    let mut days = [0u32; 12];
    days[0] = stayke_escrow::utils::bitmap_days(1, 5);
    setup_booking_days(&mut svm, property, 2025, days);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    // Ensure the 24 h timer is satisfied: updated_at (0) + 86400 < clock.
    set_clock(&mut svm, CHECK_IN_JAN_1_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful expire_booking, got: {:?}",
        res.err()
    );

    // Booking account should be closed.
    assert!(svm.get_account(&booking).is_none());

    // BookingDays should have released days 1..5 of January.
    let bd_data: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_data.occupied_days[0], 0,
        "January days should be released"
    );

    // Escrow funds should have been returned to the guest.
    assert_eq!(token_balance(&svm, &client_ata), TOTAL_PRICE);
}

// ===========================================================================
// Single-year — error: not over 24 h
// ===========================================================================

#[test]
fn expire_booking_not_over_24_hours_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    // updated_at == now, so the 24 h timer is NOT satisfied.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
        TOTAL_PRICE,
        CHECK_IN_JAN_1_2025,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    set_clock(&mut svm, CHECK_IN_JAN_1_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_escrow::error::EscrowError::NotOver24Hours))
        )
    );
}

// ===========================================================================
// Single-year — error: wrong booking status
// ===========================================================================

#[test]
fn expire_booking_wrong_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    // Booking is Active, not Pending.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Active,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    set_clock(&mut svm, CHECK_IN_JAN_1_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
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
// Single-year — error: wrong guest
// ===========================================================================

#[test]
fn expire_booking_wrong_guest_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let wrong_guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let wrong_guest_profile = setup_user_profile(&mut svm, wrong_guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, wrong_guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    set_clock(&mut svm, CHECK_IN_JAN_1_2025);

    // Pass wrong_guest_profile as client_profile — should fail
    // `booking.guest == client_profile.key()`.
    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            wrong_guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::WrongGuestPassed
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: invalid token mint
// ===========================================================================

#[test]
fn expire_booking_wrong_mint_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    // A different, valid mint that does NOT match global_config.usdc_mint. The
    // escrow/client token accounts are minted against it so their own
    // `token::mint` constraints pass and only the mint's `usdc_mint` check fires.
    let wrong_mint = Pubkey::new_unique();
    make_mint(&mut svm, wrong_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_JAN_1_2025,
        CHECK_OUT_JAN_5_2025,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        wrong_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, wrong_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    set_clock(&mut svm, CHECK_IN_JAN_1_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            wrong_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidTokenMint
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: cross-year booking with single-year instruction
// ===========================================================================

#[test]
fn expire_booking_single_year_fails_cross_year_booking() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);

    // Cross-year booking, but we call the single-year instruction.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    set_clock(&mut svm, CHECK_IN_DEC_28_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBooking {}.data(),
        expire_booking_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
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

// ===========================================================================
// Cross-year — happy path
// ===========================================================================

#[test]
fn expire_booking_cross_year_releases_days_transfers_funds_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    // Dec 28–31 occupied in 2025.
    let mut days_2025 = [0u32; 12];
    days_2025[11] = stayke_escrow::utils::bitmap_days(28, 31);
    setup_booking_days(&mut svm, property, 2025, days_2025);

    // Jan 1–3 occupied in 2026.
    let mut days_2026 = [0u32; 12];
    days_2026[0] = stayke_escrow::utils::bitmap_days(1, 3);
    setup_booking_days(&mut svm, property, 2026, days_2026);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);
    set_clock(&mut svm, CHECK_IN_DEC_28_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBookingCrossday {}.data(),
        expire_booking_crossday_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            bd_next,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful expire_booking_crossday, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&booking).is_none());

    let bd_2025: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_2025.occupied_days[11], 0,
        "December days should be released"
    );

    let bd_2026: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd_next).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_2026.occupied_days[0], 0,
        "January days should be released"
    );

    assert_eq!(token_balance(&svm, &client_ata), TOTAL_PRICE);
}

// ===========================================================================
// Cross-year — error: insufficient escrow funds
// ===========================================================================

#[test]
fn expire_booking_cross_year_insufficient_funds_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    let mut days_2025 = [0u32; 12];
    days_2025[11] = stayke_escrow::utils::bitmap_days(28, 31);
    setup_booking_days(&mut svm, property, 2025, days_2025);

    let mut days_2026 = [0u32; 12];
    days_2026[0] = stayke_escrow::utils::bitmap_days(1, 3);
    setup_booking_days(&mut svm, property, 2026, days_2026);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
        TOTAL_PRICE,
        0,
    );

    // Escrow holds less than total_price.
    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE - 1,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);
    set_clock(&mut svm, CHECK_IN_DEC_28_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBookingCrossday {}.data(),
        expire_booking_crossday_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            bd_next,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InsufficientFunds
            ))
        )
    );
}

// ===========================================================================
// Cross-year — error: not over 24 h
// ===========================================================================

#[test]
fn expire_booking_cross_year_not_over_24_hours_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    setup_booking_days(&mut svm, property, 2025, [0u32; 12]);
    setup_booking_days(&mut svm, property, 2026, [0u32; 12]);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN_DEC_28_2025,
        CHECK_OUT_JAN_3_2026,
        BookingStatus::Pending,
        TOTAL_PRICE,
        CHECK_IN_DEC_28_2025,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let client_ata = Pubkey::new_unique();
    make_token_account(&mut svm, client_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);
    set_clock(&mut svm, CHECK_IN_DEC_28_2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ExpireBookingCrossday {}.data(),
        expire_booking_crossday_accounts(
            payer.pubkey(),
            guest_profile,
            booking,
            escrow_token_account,
            client_ata,
            bd,
            bd_next,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_escrow::error::EscrowError::NotOver24Hours))
        )
    );
}
