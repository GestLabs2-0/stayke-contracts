//! LiteSVM integration tests for `guest_cancel_booking` /
//! `guest_cancel_booking_cross_year`.
//!
//! The guest cancels a booking before check-in, applying a policy that depends
//! on the time remaining until check-in. The escrow is settled, the reserved
//! days are released, and `client_cancellations` is incremented via CPI.

mod common;

use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        AnchorDeserialize, InstructionData, ToAccountMetas,
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
// Constants
// ---------------------------------------------------------------------------

/// 2025-01-01 00:00:00 UTC
const CHECK_IN: i64 = 1735689600;
/// 2025-01-05 00:00:00 UTC
const CHECK_OUT: i64 = 1736035200;
/// 2025-12-28 00:00:00 UTC
const CHECK_IN_CROSS_YEAR: i64 = 1766880000;
/// 2026-01-03 00:00:00 UTC
const CHECK_OUT_CROSS_YEAR: i64 = 1767398400;
/// Escrow total for the booking — matches `total_price` in the booking account.
const TOTAL_PRICE: u64 = 100_000;
/// Cancellation window: 72 h in seconds.
const WINDOW_SECONDS: i64 = 72 * 3600;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct Fixture {
    guest: Keypair,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    guest_reputation: Pubkey,
    property: Pubkey,
    usdc_mint: Pubkey,
    platform_vault: Pubkey,
    booking: Pubkey,
    escrow: Pubkey,
    guest_ata: Pubkey,
    host_ata: Pubkey,
}

/// Builds the full account set for a guest cancellation scenario.
fn setup(
    status: BookingStatus,
    total_price: u64,
    cross_year: bool,
) -> (litesvm::LiteSVM, Keypair, Fixture) {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();

    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let platform_vault = Pubkey::new_unique();
    setup_global_config_with_vault(&mut svm, stayke_escrow::id(), usdc_mint, platform_vault);

    let listing = setup_listing(&mut svm, host_profile, 1, 100_000, true);

    let (check_in, check_out) = if cross_year {
        (CHECK_IN_CROSS_YEAR, CHECK_OUT_CROSS_YEAR)
    } else {
        (CHECK_IN, CHECK_OUT)
    };

    if cross_year {
        let mut days_2025 = [0u32; 12];
        days_2025[11] = stayke_escrow::utils::bitmap_days(28, 31);
        setup_booking_days(&mut svm, listing, 2025, days_2025);

        let mut days_2026 = [0u32; 12];
        days_2026[0] = stayke_escrow::utils::bitmap_days(1, 3);
        setup_booking_days(&mut svm, listing, 2026, days_2026);
    } else {
        let mut days = [0u32; 12];
        days[0] = stayke_escrow::utils::bitmap_days(1, 5);
        setup_booking_days(&mut svm, listing, 2025, days);
    }

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        listing,
        check_in,
        check_out,
        status,
        total_price,
        0,
    );

    let escrow = escrow_token_pda(booking);
    make_token_account(&mut svm, escrow, usdc_mint, booking, total_price);

    let guest_ata = Pubkey::new_unique();
    make_token_account(&mut svm, guest_ata, usdc_mint, guest.pubkey(), 0);
    let host_ata = Pubkey::new_unique();
    make_token_account(&mut svm, host_ata, usdc_mint, host.pubkey(), 0);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);

    (
        svm,
        payer,
        Fixture {
            guest,
            guest_profile,
            host_profile,
            guest_reputation,
            property: listing,
            usdc_mint,
            platform_vault,
            booking,
            escrow,
            guest_ata,
            host_ata,
        },
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn guest_cancel_accounts(caller: Pubkey, f: &Fixture) -> Vec<AccountMeta> {
    stayke_escrow::accounts::GuestCancelBooking {
        caller,
        guest_profile: f.guest_profile,
        host_profile: f.host_profile,
        guest_reputation: f.guest_reputation,
        booking: f.booking,
        booking_days: booking_days_pda(f.property, 2025),
        global_config: global_config_pda(),
        escrow_token_account: f.escrow,
        guest_token_account: f.guest_ata,
        host_token_account: f.host_ata,
        platform_vault: f.platform_vault,
        mint: f.usdc_mint,
        cpi_authority: cpi_authority_pda(),
        stayke_core_program: stayke_core::id(),
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

fn guest_cancel_cross_year_accounts(caller: Pubkey, f: &Fixture) -> Vec<AccountMeta> {
    stayke_escrow::accounts::GuestCancelBookingCrossYear {
        caller,
        guest_profile: f.guest_profile,
        host_profile: f.host_profile,
        guest_reputation: f.guest_reputation,
        booking: f.booking,
        booking_days: booking_days_pda(f.property, 2025),
        booking_days_next: booking_days_pda(f.property, 2026),
        global_config: global_config_pda(),
        escrow_token_account: f.escrow,
        guest_token_account: f.guest_ata,
        host_token_account: f.host_ata,
        platform_vault: f.platform_vault,
        mint: f.usdc_mint,
        cpi_authority: cpi_authority_pda(),
        stayke_core_program: stayke_core::id(),
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

fn run(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    caller: &Keypair,
    ix: Instruction,
) -> Result<(), TransactionError> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, caller]).unwrap();
    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.err),
    }
}

fn token_balance(svm: &litesvm::LiteSVM, account: &Pubkey) -> u64 {
    let data = svm
        .get_account(account)
        .expect("token account should exist")
        .data;
    let mut amount = [0u8; 8];
    amount.copy_from_slice(&data[64..72]);
    u64::from_le_bytes(amount)
}

fn read_reputation(
    svm: &litesvm::LiteSVM,
    account: &Pubkey,
) -> stayke_core::state::ReputationProfile {
    let data = &svm.get_account(account).unwrap().data;
    AnchorDeserialize::deserialize(&mut &data[8..]).unwrap()
}

fn read_booking(svm: &litesvm::LiteSVM, account: &Pubkey) -> stayke_escrow::state::Booking {
    let data = &svm.get_account(account).unwrap().data;
    AnchorDeserialize::deserialize(&mut &data[8..]).unwrap()
}

fn read_booking_days(
    svm: &litesvm::LiteSVM,
    account: &Pubkey,
) -> stayke_escrow::state::BookingDays {
    let data = &svm.get_account(account).unwrap().data;
    AnchorDeserialize::deserialize(&mut &data[8..]).unwrap()
}

// ===========================================================================
// Guest cancellation — outside the window (full refund)
// ===========================================================================

#[test]
fn guest_cancels_outside_window_full_refund() {
    let (mut svm, payer, f) = setup(BookingStatus::Pending, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.guest, ix).expect("guest cancellation should succeed");

    // Guest receives the full price back; host and platform receive nothing.
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE);
    assert_eq!(token_balance(&svm, &f.host_ata), 0);
    assert_eq!(token_balance(&svm, &f.platform_vault), 0);

    // Escrow token account is closed.
    assert!(svm.get_account(&f.escrow).is_none());

    // Booking is marked cancelled.
    assert_eq!(
        read_booking(&svm, &f.booking).status,
        BookingStatus::Cancelled
    );

    // Reserved days are released.
    assert_eq!(
        read_booking_days(&svm, &booking_days_pda(f.property, 2025)).occupied_days[0],
        0
    );

    // Guest reputation incremented.
    assert_eq!(
        read_reputation(&svm, &f.guest_reputation).client_cancellations,
        1
    );
}

// ===========================================================================
// Guest cancellation — inside the window (60 / 30 / 10 split)
// ===========================================================================

#[test]
fn guest_cancels_inside_window_split() {
    let (mut svm, payer, f) = setup(BookingStatus::HostAccepted, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS + 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.guest, ix).expect("guest cancellation should succeed");

    // 60% -> guest, 30% -> host, 10% -> Stayke.
    assert_eq!(token_balance(&svm, &f.guest_ata), 60_000);
    assert_eq!(token_balance(&svm, &f.host_ata), 30_000);
    assert_eq!(token_balance(&svm, &f.platform_vault), 10_000);

    assert!(svm.get_account(&f.escrow).is_none());
    assert_eq!(
        read_booking(&svm, &f.booking).status,
        BookingStatus::Cancelled
    );
    assert_eq!(
        read_reputation(&svm, &f.guest_reputation).client_cancellations,
        1
    );
}

// ===========================================================================
// Guest cancellation — exactly at the window boundary (inside window)
// ===========================================================================

#[test]
fn guest_cancels_exactly_at_window_boundary() {
    let (mut svm, payer, f) = setup(BookingStatus::Pending, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.guest, ix).expect("cancellation at boundary should succeed");

    // Exactly 72 h counts as "inside the window": the split applies.
    assert_eq!(token_balance(&svm, &f.guest_ata), 60_000);
    assert_eq!(token_balance(&svm, &f.host_ata), 30_000);
    assert_eq!(token_balance(&svm, &f.platform_vault), 10_000);
}

// ===========================================================================
// Guest cancellation — cross-year (inside window split + both years released)
// ===========================================================================

#[test]
fn guest_cancels_cross_year_releases_both_years() {
    let (mut svm, payer, f) = setup(BookingStatus::HostAccepted, TOTAL_PRICE, true);
    set_clock(&mut svm, CHECK_IN_CROSS_YEAR - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBookingCrossYear {}.data(),
        guest_cancel_cross_year_accounts(f.guest.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.guest, ix).expect("cross-year guest cancellation should succeed");

    assert_eq!(token_balance(&svm, &f.guest_ata), 60_000);
    assert_eq!(token_balance(&svm, &f.host_ata), 30_000);
    assert_eq!(token_balance(&svm, &f.platform_vault), 10_000);

    assert_eq!(
        read_booking_days(&svm, &booking_days_pda(f.property, 2025)).occupied_days[11],
        0
    );
    assert_eq!(
        read_booking_days(&svm, &booking_days_pda(f.property, 2026)).occupied_days[0],
        0
    );
}

// ===========================================================================
// Guest cancellation — rounding does not lose funds
// ===========================================================================

#[test]
fn rounding_does_not_lose_funds() {
    // total_price = 101 -> guest 60, host 30, stayke 11 (sums to 101).
    let total = 101;
    let (mut svm, payer, f) = setup(BookingStatus::Pending, total, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS + 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.guest, ix).expect("guest cancellation should succeed");

    assert_eq!(token_balance(&svm, &f.guest_ata), 60);
    assert_eq!(token_balance(&svm, &f.host_ata), 30);
    assert_eq!(token_balance(&svm, &f.platform_vault), 11);
}

// ===========================================================================
// Guest cancellation — error cases
// ===========================================================================

#[test]
fn guest_cancellation_after_check_in_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::HostAccepted, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN + 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &f.guest, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_escrow::error::EscrowError::CheckInPassed))
        )
    );
}

#[test]
fn guest_unauthorized_caller_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::Pending, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    let rogue = Keypair::new();
    svm.airdrop(&rogue.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(rogue.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &rogue, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::UnauthorizedCancellation
            ))
        )
    );
}

#[test]
fn guest_already_cancelled_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::Cancelled, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &f.guest, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingStatus
            ))
        )
    );
}

#[test]
fn guest_completed_booking_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::Completed, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &f.guest, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingStatus
            ))
        )
    );
}

#[test]
fn guest_wrong_guest_token_account_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::Pending, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    // A token account owned by someone other than the guest.
    let rogue_ata = Pubkey::new_unique();
    make_token_account(&mut svm, rogue_ata, f.usdc_mint, Pubkey::new_unique(), 0);

    let mut metas = guest_cancel_accounts(f.guest.pubkey(), &f);
    for meta in metas.iter_mut() {
        if meta.pubkey == f.guest_ata {
            meta.pubkey = rogue_ata;
        }
    }
    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        metas,
    );
    assert!(run(&mut svm, &payer, &f.guest, ix).is_err());
}

#[test]
fn guest_escrow_insufficient_funds_fails() {
    let (mut svm, payer, f) = setup(BookingStatus::Pending, TOTAL_PRICE, false);
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    // Drain the escrow below total_price.
    make_token_account(&mut svm, f.escrow, f.usdc_mint, f.booking, TOTAL_PRICE - 1);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestCancelBooking {}.data(),
        guest_cancel_accounts(f.guest.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &f.guest, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InsufficientFunds
            ))
        )
    );
}
