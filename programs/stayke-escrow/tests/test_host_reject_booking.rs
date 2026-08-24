//! LiteSVM integration tests for `host_reject_booking` /
//! `host_reject_booking_cross_year`.
//!
//! The host rejects a *Pending* booking, releasing the occupied calendar days
//! and returning the escrowed USDC to the guest before closing the booking.

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
fn host_reject_accounts(
    payer: Pubkey,
    host: Pubkey,
    host_profile: Pubkey,
    guest_profile: Pubkey,
    booking: Pubkey,
    booking_days: Pubkey,
    escrow_token_account: Pubkey,
    guest_token_account: Pubkey,
    usdc_mint: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::HostRejectBooking {
        payer,
        host,
        host_profile,
        guest: guest_profile,
        booking,
        booking_days,
        global_config: global_config_pda(),
        escrow_token_account,
        guest_token_account,
        mint: usdc_mint,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

#[allow(clippy::too_many_arguments)]
fn host_reject_cross_year_accounts(
    payer: Pubkey,
    host: Pubkey,
    host_profile: Pubkey,
    guest_profile: Pubkey,
    booking: Pubkey,
    booking_days: Pubkey,
    booking_days_next: Pubkey,
    escrow_token_account: Pubkey,
    guest_token_account: Pubkey,
    usdc_mint: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::HostRejectBookingCrossYear {
        payer,
        host,
        host_profile,
        guest: guest_profile,
        booking,
        booking_days,
        booking_days_next,
        global_config: global_config_pda(),
        escrow_token_account,
        guest_token_account,
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
fn host_reject_booking_returns_funds_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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

    assert!(svm.get_account(&booking).is_none());
    assert!(svm.get_account(&escrow_token_account).is_none());

    let bd_data: stayke_escrow::state::BookingDays =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&bd).unwrap().data[8..]).unwrap();
    assert_eq!(
        bd_data.occupied_days[0], 0,
        "January days should be released"
    );

    assert_eq!(token_balance(&svm, &guest_ata), TOTAL_PRICE);
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

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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
// Single-year — error: wrong host
// ===========================================================================

#[test]
fn host_reject_booking_wrong_host_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let wrong_host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let wrong_host_profile = setup_user_profile(&mut svm, wrong_host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            wrong_host.pubkey(),
            wrong_host_profile,
            guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &wrong_host])
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
// Single-year — error: wrong guest
// ===========================================================================

#[test]
fn host_reject_booking_wrong_guest_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let wrong_guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let wrong_guest_profile = setup_user_profile(&mut svm, wrong_guest.pubkey());

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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, wrong_guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            wrong_guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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
                stayke_escrow::error::EscrowError::WrongGuestPassed
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: insufficient escrow funds
// ===========================================================================

#[test]
fn host_reject_booking_insufficient_funds_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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

    // Escrow holds less than total_price.
    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE - 1,
    );
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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
                stayke_escrow::error::EscrowError::InsufficientFunds
            ))
        )
    );
}

// ===========================================================================
// Single-year — error: invalid token mint
// ===========================================================================

#[test]
fn host_reject_booking_wrong_mint_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 0, 4, usdc_mint);

    // A different, valid mint that does NOT match global_config.usdc_mint.
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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, wrong_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBooking {}.data(),
        host_reject_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            escrow_token_account,
            guest_ata,
            wrong_mint,
        ),
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
                stayke_escrow::error::EscrowError::InvalidTokenMint
            ))
        )
    );
}

// ===========================================================================
// Cross-year — happy path
// ===========================================================================

#[test]
fn host_reject_booking_cross_year_returns_funds_releases_days_and_closes_account() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBookingCrossYear {}.data(),
        host_reject_cross_year_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            bd_next,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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

    assert!(svm.get_account(&booking).is_none());
    assert!(svm.get_account(&escrow_token_account).is_none());

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

    assert_eq!(token_balance(&svm, &guest_ata), TOTAL_PRICE);
}

// ===========================================================================
// Cross-year — error: insufficient escrow funds
// ===========================================================================

#[test]
fn host_reject_booking_cross_year_insufficient_funds_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());

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
    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);

    let bd = booking_days_pda(property, 2025);
    let bd_next = booking_days_pda(property, 2026);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostRejectBookingCrossYear {}.data(),
        host_reject_cross_year_accounts(
            payer.pubkey(),
            host.pubkey(),
            host_profile,
            guest_profile,
            booking,
            bd,
            bd_next,
            escrow_token_account,
            guest_ata,
            usdc_mint,
        ),
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
                stayke_escrow::error::EscrowError::InsufficientFunds
            ))
        )
    );
}
