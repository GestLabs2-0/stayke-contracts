//! LiteSVM integration tests for `host_cancel_booking` /
//! `host_cancel_booking_cross_year`.
//!
//! The host cancels a booking before check-in: full refund to the guest plus a
//! deposit slash paid to the guest. The escrow is settled, the reserved days
//! are released, and `host_cancellations` is incremented via CPI.

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
/// Host cancellation penalty: 10% of the available deposit.
const HOST_PENALTY: u8 = 10;
/// Cancellation window: 72 h in seconds.
const WINDOW_SECONDS: i64 = 72 * 3600;

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct Fixture {
    host: Keypair,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    host_reputation: Pubkey,
    property: Pubkey,
    usdc_mint: Pubkey,
    booking: Pubkey,
    escrow: Pubkey,
    guest_ata: Pubkey,
    treasury_config: Pubkey,
    treasury_vault: Pubkey,
}

/// Builds the full account set for a host cancellation scenario.
fn setup(
    status: BookingStatus,
    total_price: u64,
    host_deposit: u64,
    treasury_vault_amount: u64,
    cross_year: bool,
) -> (litesvm::LiteSVM, Keypair, Fixture) {
    let (mut svm, payer) = build_svm_with_all_programs();
    let guest = Keypair::new();
    let host = Keypair::new();

    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&host.pubkey(), 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), host_deposit, 0, 0);
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    // Host cancellation never touches the platform vault, so no token account
    // is created for it — the config only needs a distinct pubkey.
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

    let (treasury_config, treasury_vault) =
        setup_treasury(&mut svm, usdc_mint, treasury_vault_amount);

    (
        svm,
        payer,
        Fixture {
            host,
            guest_profile,
            host_profile,
            host_reputation,
            property: listing,
            usdc_mint,
            booking,
            escrow,
            guest_ata,
            treasury_config,
            treasury_vault,
        },
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn host_cancel_accounts(caller: Pubkey, f: &Fixture) -> Vec<AccountMeta> {
    stayke_escrow::accounts::HostCancelBooking {
        caller,
        guest_profile: f.guest_profile,
        host_profile: f.host_profile,
        host_reputation: f.host_reputation,
        booking: f.booking,
        booking_days: booking_days_pda(f.property, 2025),
        global_config: global_config_pda(),
        escrow_token_account: f.escrow,
        guest_token_account: f.guest_ata,
        mint: f.usdc_mint,
        cpi_authority: cpi_authority_pda(),
        treasury_config: f.treasury_config,
        treasury_vault: f.treasury_vault,
        treasury_pda: treasury_pda(),
        stayke_core_program: stayke_core::id(),
        stayke_treasury_program: stayke_treasury::id(),
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

fn host_cancel_cross_year_accounts(caller: Pubkey, f: &Fixture) -> Vec<AccountMeta> {
    stayke_escrow::accounts::HostCancelBookingCrossYear {
        caller,
        guest_profile: f.guest_profile,
        host_profile: f.host_profile,
        host_reputation: f.host_reputation,
        booking: f.booking,
        booking_days: booking_days_pda(f.property, 2025),
        booking_days_next: booking_days_pda(f.property, 2026),
        global_config: global_config_pda(),
        escrow_token_account: f.escrow,
        guest_token_account: f.guest_ata,
        mint: f.usdc_mint,
        cpi_authority: cpi_authority_pda(),
        treasury_config: f.treasury_config,
        treasury_vault: f.treasury_vault,
        treasury_pda: treasury_pda(),
        stayke_core_program: stayke_core::id(),
        stayke_treasury_program: stayke_treasury::id(),
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

fn read_user_profile(svm: &litesvm::LiteSVM, account: &Pubkey) -> stayke_core::state::UserProfile {
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
// Host cancellation — full refund + deposit slash
// ===========================================================================

#[test]
fn host_cancels_full_refund_and_slash() {
    let host_deposit = 1_000_000;
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        host_deposit,
        host_deposit,
        false,
    );
    set_clock(&mut svm, CHECK_IN - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("host cancellation should succeed");

    let slash = host_deposit * HOST_PENALTY as u64 / 100;

    // Guest receives the full price plus the slash (paid from the treasury).
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE + slash);

    // Treasury vault is debited by the slash.
    assert_eq!(token_balance(&svm, &f.treasury_vault), host_deposit - slash);

    // Host's tracked deposit is decremented by the slash.
    assert_eq!(
        read_user_profile(&svm, &f.host_profile).deposited,
        host_deposit - slash
    );

    // Host reputation incremented.
    assert_eq!(
        read_reputation(&svm, &f.host_reputation).host_cancellations,
        1
    );

    assert!(svm.get_account(&f.escrow).is_none());
    assert_eq!(
        read_booking(&svm, &f.booking).status,
        BookingStatus::Cancelled
    );
}

// ===========================================================================
// Host cancellation — no deposit slashes nothing
// ===========================================================================

#[test]
fn host_cancels_with_no_deposit_slashes_nothing() {
    let (mut svm, payer, f) = setup(BookingStatus::HostAccepted, TOTAL_PRICE, 0, 0, false);
    set_clock(&mut svm, CHECK_IN - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("host cancellation should succeed");

    // No deposit to slash: the guest only gets the full refund.
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE);
    assert_eq!(token_balance(&svm, &f.treasury_vault), 0);
    assert_eq!(read_user_profile(&svm, &f.host_profile).deposited, 0);

    // Cancellation is still recorded against the host.
    assert_eq!(
        read_reputation(&svm, &f.host_reputation).host_cancellations,
        1
    );
}

// ===========================================================================
// Host cancellation — tiny deposit rounds slash down to zero
// ===========================================================================

#[test]
fn host_cancels_tiny_deposit_slash_rounds_to_zero() {
    // 5 * 10% = 0.5 -> 0, so no funds move from the treasury.
    let (mut svm, payer, f) = setup(BookingStatus::HostAccepted, TOTAL_PRICE, 5, 5, false);
    set_clock(&mut svm, CHECK_IN - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("host cancellation should succeed");

    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE);
    assert_eq!(token_balance(&svm, &f.treasury_vault), 5);
    assert_eq!(read_user_profile(&svm, &f.host_profile).deposited, 5);
    assert_eq!(
        read_reputation(&svm, &f.host_reputation).host_cancellations,
        1
    );
}

// ===========================================================================
// Host cancellation — outside the window (no slash)
// ===========================================================================

#[test]
fn host_cancels_outside_window_no_slash() {
    let host_deposit = 1_000_000;
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        host_deposit,
        host_deposit,
        false,
    );
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("host cancellation should succeed");

    // Outside the window: full refund only, no deposit slash.
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE);
    assert_eq!(token_balance(&svm, &f.treasury_vault), host_deposit);
    assert_eq!(
        read_user_profile(&svm, &f.host_profile).deposited,
        host_deposit
    );

    // Cancellation is still recorded against the host.
    assert_eq!(
        read_reputation(&svm, &f.host_reputation).host_cancellations,
        1
    );
}

// ===========================================================================
// Host cancellation — exactly at the window boundary (inside window, slashes)
// ===========================================================================

#[test]
fn host_cancels_exactly_at_window_boundary_slashes() {
    let host_deposit = 1_000_000;
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        host_deposit,
        host_deposit,
        false,
    );
    set_clock(&mut svm, CHECK_IN - WINDOW_SECONDS);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("host cancellation should succeed");

    let slash = host_deposit * HOST_PENALTY as u64 / 100;

    // Exactly 72 h counts as "inside the window": the slash applies.
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE + slash);
    assert_eq!(token_balance(&svm, &f.treasury_vault), host_deposit - slash);
    assert_eq!(
        read_user_profile(&svm, &f.host_profile).deposited,
        host_deposit - slash
    );
}

// ===========================================================================
// Host cancellation — cross-year (full refund + slash + both years released)
// ===========================================================================

#[test]
fn host_cancels_cross_year_releases_both_years() {
    let host_deposit = 1_000_000;
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        host_deposit,
        host_deposit,
        true,
    );
    set_clock(&mut svm, CHECK_IN_CROSS_YEAR - 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBookingCrossYear {}.data(),
        host_cancel_cross_year_accounts(f.host.pubkey(), &f),
    );
    run(&mut svm, &payer, &f.host, ix).expect("cross-year host cancellation should succeed");

    let slash = host_deposit * HOST_PENALTY as u64 / 100;
    assert_eq!(token_balance(&svm, &f.guest_ata), TOTAL_PRICE + slash);

    assert_eq!(
        read_booking_days(&svm, &booking_days_pda(f.property, 2025)).occupied_days[11],
        0
    );
    assert_eq!(
        read_booking_days(&svm, &booking_days_pda(f.property, 2026)).occupied_days[0],
        0
    );

    assert_eq!(
        read_booking(&svm, &f.booking).status,
        BookingStatus::Cancelled
    );
}

// ===========================================================================
// Host cancellation — error cases
// ===========================================================================

#[test]
fn host_unauthorized_caller_fails() {
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        1_000_000,
        1_000_000,
        false,
    );
    set_clock(&mut svm, CHECK_IN - 3600);

    let rogue = Keypair::new();
    svm.airdrop(&rogue.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(rogue.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &rogue, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::UnauthorizedHost
            ))
        )
    );
}

#[test]
fn host_cancellation_after_check_in_fails() {
    let (mut svm, payer, f) = setup(
        BookingStatus::HostAccepted,
        TOTAL_PRICE,
        1_000_000,
        1_000_000,
        false,
    );
    set_clock(&mut svm, CHECK_IN + 3600);

    let ix = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostCancelBooking {}.data(),
        host_cancel_accounts(f.host.pubkey(), &f),
    );
    let err = run(&mut svm, &payer, &f.host, ix).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(stayke_escrow::error::EscrowError::CheckInPassed))
        )
    );
}
