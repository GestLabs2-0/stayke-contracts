//! LiteSVM tests for free-tier deposit enforcement (STK-250).

mod common;

use {
    anchor_lang::{
        prelude::system_program,
        solana_program::instruction::{AccountMeta, Instruction},
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
};

// ---------------------------------------------------------------------------
// Timestamp constants
// ---------------------------------------------------------------------------

/// 2026-06-15 00:00 UTC (future date, well past "now" in LiteSVM)
const CHECK_IN: i64 = 1781251200;
/// 2026-06-20 00:00 UTC
const CHECK_OUT: i64 = 1781683200;
/// Year derived from CHECK_IN (June 2026).
const YEAR: u32 = 2026;
/// Guest USDC balance — covers `total_price` (listing price 100_000 × 5 nights).
const GUEST_USDC_BALANCE: u64 = 1_000_000;

/// Build the full `CreateBooking` account metas, including the Token2022
/// escrow-funding accounts (escrow token account, guest ATA, mint, token program).
#[allow(clippy::too_many_arguments)]
fn create_booking_accounts(
    payer: Pubkey,
    guest: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    listing: Pubkey,
    usdc_mint: Pubkey,
    guest_ata: Pubkey,
) -> Vec<AccountMeta> {
    stayke_escrow::accounts::CreateBooking {
        payer,
        client: guest,
        client_profile: guest_profile,
        host_profile,
        booking: booking_pda(listing, guest_profile, CHECK_IN),
        property: listing,
        global_config: global_config_pda(),
        system_program: system_program::id(),
        booking_days: booking_days_pda(listing, YEAR),
        escrow_token_account: escrow_token_pda(booking_pda(listing, guest_profile, CHECK_IN)),
        client_token_account: guest_ata,
        mint: usdc_mint,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
}

// ===========================================================================
// Free tier: user with no deposit but low counters succeeds
// ===========================================================================

#[test]
fn create_booking_free_tier_bypass_succeeds() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let guest_ata = Pubkey::new_unique();
    make_token_account(
        &mut svm,
        guest_ata,
        usdc_mint,
        guest.pubkey(),
        GUEST_USDC_BALANCE,
    );

    // Guest: 0 deposit, 1 completed_stays, 0 hosted_stays → sum = 1 < free_ops (4)
    let guest_profile = setup_user_profile_custom(&mut svm, guest.pubkey(), 0, 1, 0);
    // Host: normal profile (deposit exists, but host check also applies free tier)
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), 0, 1, 0);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 500_000, 4, usdc_mint);

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CreateBooking {
            check_in: CHECK_IN,
            check_out: CHECK_OUT,
        }
        .data(),
        create_booking_accounts(
            payer.pubkey(),
            guest.pubkey(),
            guest_profile,
            host_profile,
            listing,
            usdc_mint,
            guest_ata,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "create_booking should succeed when in free tier (counters < free_ops), got: {:?}",
        res.err()
    );
}

// ===========================================================================
// Deposit required: user with no deposit and high counters fails
// ===========================================================================

#[test]
fn create_booking_no_deposit_high_counters_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let guest_ata = Pubkey::new_unique();
    make_token_account(
        &mut svm,
        guest_ata,
        usdc_mint,
        guest.pubkey(),
        GUEST_USDC_BALANCE,
    );

    // Guest: 0 deposit, 3 completed + 2 hosted = 5 >= free_ops (4) → deposit required
    let guest_profile = setup_user_profile_custom(&mut svm, guest.pubkey(), 0, 3, 2);
    // Host also needs to pass: host has counters < free_ops, so host check would pass.
    // The test verifies the GUEST constraint fires first.
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), 1_000_000, 0, 0);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 500_000, 4, usdc_mint);

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CreateBooking {
            check_in: CHECK_IN,
            check_out: CHECK_OUT,
        }
        .data(),
        create_booking_accounts(
            payer.pubkey(),
            guest.pubkey(),
            guest_profile,
            host_profile,
            listing,
            usdc_mint,
            guest_ata,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Expected InsufficientDeposit error");
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InsufficientDeposit
            ))
        )
    );
}

// ===========================================================================
// Deposit present: user with deposit succeeds even with high counters
// ===========================================================================

#[test]
fn create_booking_with_deposit_succeeds_despite_high_counters() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let guest_ata = Pubkey::new_unique();
    make_token_account(
        &mut svm,
        guest_ata,
        usdc_mint,
        guest.pubkey(),
        GUEST_USDC_BALANCE,
    );

    // Guest: 1M deposit, counters = 10 (way above free_ops=4) → deposit check passes
    let guest_profile = setup_user_profile_custom(&mut svm, guest.pubkey(), 1_000_000, 5, 5);
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), 1_000_000, 0, 0);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 500_000, 4, usdc_mint);

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CreateBooking {
            check_in: CHECK_IN,
            check_out: CHECK_OUT,
        }
        .data(),
        create_booking_accounts(
            payer.pubkey(),
            guest.pubkey(),
            guest_profile,
            host_profile,
            listing,
            usdc_mint,
            guest_ata,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "create_booking should succeed when user has sufficient deposit, got: {:?}",
        res.err()
    );
}

// ===========================================================================
// Host: free tier exhausted, no deposit → host_accept_booking fails
// ===========================================================================

#[test]
fn host_accept_booking_no_deposit_high_counters_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    // Guest: normal deposit to pass guest check
    let guest_profile = setup_user_profile_custom(&mut svm, guest.pubkey(), 1_000_000, 0, 0);
    // Host: 0 deposit, high counters → host_accept_booking should fail
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), 0, 3, 2);
    setup_global_config_custom(
        &mut svm,
        stayke_escrow::id(),
        500_000,
        4,
        Pubkey::new_unique(),
    );

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let mut days = [0u32; 12];
    days[5] = stayke_escrow::utils::bitmap_days(15, 20);
    setup_booking_days(&mut svm, listing, YEAR, days);

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        listing,
        CHECK_IN,
        CHECK_OUT,
        stayke_escrow::state::BookingStatus::Pending,
    );

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostAcceptBooking {}.data(),
        stayke_escrow::accounts::HostAcceptBooking {
            payer: payer.pubkey(),
            host: host.pubkey(),
            host_profile,
            booking,
            global_config: global_config_pda(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &host]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Expected InsufficientDeposit error for host");
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InsufficientDeposit
            ))
        )
    );
}

// ===========================================================================
// Free tier disabled: free_ops = 0 → deposit always required
// ===========================================================================

#[test]
fn create_booking_fails_when_free_ops_is_zero_and_no_deposit() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let guest_ata = Pubkey::new_unique();
    make_token_account(
        &mut svm,
        guest_ata,
        usdc_mint,
        guest.pubkey(),
        GUEST_USDC_BALANCE,
    );

    // Guest: 0 deposit, 0 counters. free_ops=0 means no free tier.
    let guest_profile = setup_user_profile_custom(&mut svm, guest.pubkey(), 0, 0, 0);
    let host_profile = setup_user_profile_custom(&mut svm, host.pubkey(), 1_000_000, 0, 0);
    setup_global_config_custom(&mut svm, stayke_escrow::id(), 500_000, 0, usdc_mint);

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CreateBooking {
            check_in: CHECK_IN,
            check_out: CHECK_OUT,
        }
        .data(),
        create_booking_accounts(
            payer.pubkey(),
            guest.pubkey(),
            guest_profile,
            host_profile,
            listing,
            usdc_mint,
            guest_ata,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_err(),
        "create_booking should fail when free_ops=0 and no deposit"
    );
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InsufficientDeposit
            ))
        )
    );
}
