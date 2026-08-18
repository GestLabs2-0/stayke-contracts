//! LiteSVM integration tests for `release_funds`.
//!
//! `release_funds` is a permissionless settlement step: once a booking is
//! `Completed` and the 24h dispute window has elapsed, anyone can trigger the
//! payout to the host (minus the platform fee), close the escrow token
//! account, transition the booking to `Released`, and increment the guest's
//! `completed_stays` and the host's `hosted_stays` counters via CPI.

mod common;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, AnchorDeserialize, InstructionData,
        ToAccountMetas,
    },
    common::*,
    solana_account::Account,
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
// Timestamp / amount constants
// ---------------------------------------------------------------------------

/// 2025-01-01 00:00:00 UTC
const CHECK_IN: i64 = 1735689600;
/// 2025-01-05 00:00:00 UTC
const CHECK_OUT: i64 = 1736035200;
/// Escrow total for the booking — matches `total_price` in the booking account.
const TOTAL_PRICE: u64 = 100_000;
/// `fee_bps` configured in `setup_global_config_with_vault`.
const FEE_BPS: u64 = 200;
/// Platform fee = 100_000 * 200 / 10_000.
const FEE: u64 = TOTAL_PRICE * FEE_BPS / 10_000;
/// Host payout = total minus fee.
const HOST_AMOUNT: u64 = TOTAL_PRICE - FEE;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn release_funds_accounts(
    payer: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    booking: Pubkey,
    escrow_token_account: Pubkey,
    host_token_account: Pubkey,
    platform_vault: Pubkey,
    usdc_mint: Pubkey,
    cpi_authority: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::ReleaseFunds {
        payer,
        guest_profile,
        host_profile,
        booking,
        global_config: global_config_pda(),
        escrow_token_account,
        host_token_account,
        platform_vault,
        mint: usdc_mint,
        cpi_authority,
        stayke_core_program: stayke_core::id(),
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
// Happy path
// ===========================================================================

#[test]
fn release_funds_after_window_pays_host_and_increments_counters() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let platform_vault = Pubkey::new_unique();
    setup_global_config_with_vault(&mut svm, stayke_escrow::id(), usdc_mint, platform_vault);

    // updated_at = 0 → the 24h window is satisfied once the clock is >= 86400.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
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

    {
        let guest_acc = &mut &svm.get_account(&guest_profile).unwrap();
        let mut guest_data: stayke_core::state::UserProfile =
            AnchorDeserialize::deserialize(&mut &guest_acc.data[8..]).unwrap();
        // Is this a pointer?
        guest_data.active_booking = Some(booking);

        svm.set_account(
            guest_profile,
            Account {
                data: to_account_data_spaced("UserProfile", &guest_data),
                lamports: 1_000_000_000,
                owner: stayke_core::id(),
                executable: false,
                rent_epoch: u64::MAX,
            },
        )
        .unwrap();
    }

    let host_ata = Pubkey::new_unique();
    make_token_account(&mut svm, host_ata, usdc_mint, host.pubkey(), 0);

    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);

    let cpi_authority = cpi_authority_pda();
    set_clock(&mut svm, 86_400);

    let guest_data: stayke_core::state::UserProfile =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&guest_profile).unwrap().data[8..])
            .unwrap();

    assert_eq!(guest_data.active_booking, Some(booking));

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ReleaseFunds {}.data(),
        release_funds_accounts(
            payer.pubkey(),
            guest_profile,
            host_profile,
            booking,
            escrow_token_account,
            host_ata,
            platform_vault,
            usdc_mint,
            cpi_authority,
        ),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "release_funds should succeed, got: {:?}",
        res.err()
    );

    // Booking is Released (not closed — reviews still reference it).
    let booking_data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    assert_eq!(booking_data.status, BookingStatus::Released);

    // Escrow token account is closed.
    assert!(svm.get_account(&escrow_token_account).is_none());

    // Funds: host gets total - fee, platform vault gets the fee.
    assert_eq!(token_balance(&svm, &host_ata), HOST_AMOUNT);
    assert_eq!(token_balance(&svm, &platform_vault), FEE);

    // Counters were incremented via CPI.
    let guest_data: stayke_core::state::UserProfile =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&guest_profile).unwrap().data[8..])
            .unwrap();
    assert_eq!(guest_data.completed_stays, 1);
    assert_eq!(guest_data.active_booking, None);

    let host_data: stayke_core::state::UserProfile =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&host_profile).unwrap().data[8..])
            .unwrap();
    assert_eq!(host_data.hosted_stays, 1);
}

// ===========================================================================
// Error: release window not elapsed
// ===========================================================================

#[test]
fn release_funds_before_window_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let platform_vault = Pubkey::new_unique();
    setup_global_config_with_vault(&mut svm, stayke_escrow::id(), usdc_mint, platform_vault);

    // updated_at == now → the 24h window has NOT elapsed.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
        TOTAL_PRICE,
        CHECK_IN,
    );

    let escrow_token_account = escrow_token_pda(booking);
    make_token_account(
        &mut svm,
        escrow_token_account,
        usdc_mint,
        booking,
        TOTAL_PRICE,
    );

    let host_ata = Pubkey::new_unique();
    make_token_account(&mut svm, host_ata, usdc_mint, host.pubkey(), 0);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);

    let cpi_authority = cpi_authority_pda();
    set_clock(&mut svm, CHECK_IN);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ReleaseFunds {}.data(),
        release_funds_accounts(
            payer.pubkey(),
            guest_profile,
            host_profile,
            booking,
            escrow_token_account,
            host_ata,
            platform_vault,
            usdc_mint,
            cpi_authority,
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
                stayke_escrow::error::EscrowError::ReleaseWindowNotElapsed
            ))
        )
    );
}

// ===========================================================================
// Error: wrong booking status
// ===========================================================================

#[test]
fn release_funds_wrong_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let platform_vault = Pubkey::new_unique();
    setup_global_config_with_vault(&mut svm, stayke_escrow::id(), usdc_mint, platform_vault);

    // Booking is Active, not Completed.
    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
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

    let host_ata = Pubkey::new_unique();
    make_token_account(&mut svm, host_ata, usdc_mint, host.pubkey(), 0);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);

    let cpi_authority = cpi_authority_pda();
    set_clock(&mut svm, 86_400);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ReleaseFunds {}.data(),
        release_funds_accounts(
            payer.pubkey(),
            guest_profile,
            host_profile,
            booking,
            escrow_token_account,
            host_ata,
            platform_vault,
            usdc_mint,
            cpi_authority,
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
                stayke_escrow::error::EscrowError::BookingNotCompleted
            ))
        )
    );
}

// ===========================================================================
// Error: invalid token mint
// ===========================================================================

#[test]
fn release_funds_wrong_mint_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    let property = Pubkey::new_unique();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());

    let usdc_mint = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    let platform_vault = Pubkey::new_unique();
    setup_global_config_with_vault(&mut svm, stayke_escrow::id(), usdc_mint, platform_vault);

    // A different, valid mint that does NOT match global_config.usdc_mint.
    let wrong_mint = Pubkey::new_unique();
    make_mint(&mut svm, wrong_mint);

    let booking = setup_booking_with_escrow(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
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

    let host_ata = Pubkey::new_unique();
    make_token_account(&mut svm, host_ata, wrong_mint, host.pubkey(), 0);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);

    let cpi_authority = cpi_authority_pda();
    set_clock(&mut svm, 86_400);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::ReleaseFunds {}.data(),
        release_funds_accounts(
            payer.pubkey(),
            guest_profile,
            host_profile,
            booking,
            escrow_token_account,
            host_ata,
            platform_vault,
            wrong_mint,
            cpi_authority,
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
