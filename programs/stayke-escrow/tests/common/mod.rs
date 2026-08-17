#![allow(dead_code)]
//! Shared helpers for stayke-escrow LiteSVM tests.

use anchor_lang::solana_program::clock::Clock;
use anchor_lang::{AnchorSerialize, Space};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use stayke_config as config;
use stayke_core as core;
use stayke_escrow as escrow;

// ---------------------------------------------------------------------------
// Discriminator
// ---------------------------------------------------------------------------

pub fn discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("account:{name}");
    let hash = solana_program::hash::hash(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash.to_bytes()[..8]);
    disc
}

pub fn to_account_data<T: AnchorSerialize>(name: &str, value: &T) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator(name));
    value.serialize(&mut data).unwrap();
    data
}

/// Serializes into a properly sized account buffer: an 8-byte discriminator
/// plus `T::INIT_SPACE`, zero-padding the tail. Required for accounts with
/// `Option` fields that can grow when written (e.g. `UserProfile::active_booking`
/// going from `None` to `Some`), which the compact `to_account_data` under-sizes.
pub fn to_account_data_spaced<T: AnchorSerialize + Space>(name: &str, value: &T) -> Vec<u8> {
    let mut data = vec![0u8; 8 + T::INIT_SPACE];
    data[..8].copy_from_slice(&discriminator(name));

    let mut encoded = Vec::new();
    value.serialize(&mut encoded).unwrap();
    assert!(
        encoded.len() <= T::INIT_SPACE,
        "serialized account ({} bytes) exceeds INIT_SPACE ({} bytes)",
        encoded.len(),
        T::INIT_SPACE
    );
    data[8..8 + encoded.len()].copy_from_slice(&encoded);

    data
}

// ---------------------------------------------------------------------------
// GlobalConfig (stayke_config)
// ---------------------------------------------------------------------------

pub fn setup_global_config(svm: &mut LiteSVM, escrow_program: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    );

    let gc = config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops: 4,
        minimum_deposit: 0, // zero so minimum deposit doesn't block
        fee_bps: 200,
        usdc_mint: Pubkey::new_unique(),
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: bump,
        core_program: core::id(),
        escrow_program,
        disputes_program: Pubkey::new_unique(),
        treasury_program: Pubkey::new_unique(),
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gc),
            owner: config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// UserProfile (stayke_core)
// ---------------------------------------------------------------------------

pub fn setup_user_profile(svm: &mut LiteSVM, authority: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &core::id(),
    );

    let profile = core::state::UserProfile {
        completed_stays: 0,
        hosted_stays: 0,
        authority,
        identity: Some(Pubkey::new_unique()),
        active_booking: None,
        deposited: 1_000_000,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 0,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data_spaced("UserProfile", &profile),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// ReputationProfile (stayke_core)
// ---------------------------------------------------------------------------

pub fn setup_reputation_profile(svm: &mut LiteSVM, authority: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            core::constants::REPUTATION_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &core::id(),
    );

    let profile = core::state::ReputationProfile {
        authority,
        host_reviews: 0,
        total_score_host: 0,
        client_reviews: 0,
        total_score_client: 0,
        host_cancellations: 0,
        client_cancellations: 0,
        host_reviews_skipped: 0,
        guest_reviews_skipped: 0,
        low_infractions: 0,
        medium_infractions: 0,
        high_infractions: 0,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("ReputationProfile", &profile),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// BookingDays (stayke_escrow)
// ---------------------------------------------------------------------------

pub fn setup_booking_days(
    svm: &mut LiteSVM,
    property: Pubkey,
    year: u32,
    occupied_days: [u32; 12],
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            escrow::constants::BOOKING_DAYS_SEED.as_bytes(),
            property.as_ref(),
            year.to_le_bytes().as_ref(),
        ],
        &escrow::id(),
    );

    let bd = escrow::state::BookingDays {
        year,
        occupied_days,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("BookingDays", &bd),
            owner: escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// Booking (stayke_escrow)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn setup_booking_at_pda(
    svm: &mut LiteSVM,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    property: Pubkey,
    check_in: i64,
    check_out: i64,
    status: escrow::state::BookingStatus,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            escrow::constants::BOOKING_SEED.as_bytes(),
            property.as_ref(),
            guest_profile.as_ref(),
            check_in.to_le_bytes().as_ref(),
        ],
        &escrow::id(),
    );

    let booking = escrow::state::Booking {
        guest: guest_profile,
        host: host_profile,
        property,
        check_in,
        check_out,
        total_price: 100_000,
        host_review: 0,
        guest_review: 0,
        status,
        escrow_bump: 255,
        updated_at: 0,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &booking),
            owner: escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// Booking with a correctly derived escrow bump (for instructions that transfer
// out of the escrow, whose authority is the booking PDA).
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn setup_booking_with_escrow(
    svm: &mut LiteSVM,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    property: Pubkey,
    check_in: i64,
    check_out: i64,
    status: escrow::state::BookingStatus,
    total_price: u64,
    updated_at: i64,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            escrow::constants::BOOKING_SEED.as_bytes(),
            property.as_ref(),
            guest_profile.as_ref(),
            check_in.to_le_bytes().as_ref(),
        ],
        &escrow::id(),
    );

    let (_, escrow_bump) = Pubkey::find_program_address(
        &[escrow::constants::ESCROW_PDA_SEED.as_bytes(), pda.as_ref()],
        &escrow::id(),
    );

    let booking = escrow::state::Booking {
        guest: guest_profile,
        host: host_profile,
        property,
        check_in,
        check_out,
        total_price,
        host_review: 0,
        guest_review: 0,
        status,
        escrow_bump,
        updated_at,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &booking),
            owner: escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// Clock sysvar manipulation (expire_booking's 24 h timer reads the clock).
// ---------------------------------------------------------------------------

pub fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar::<Clock>(&clock);
}

// ---------------------------------------------------------------------------
// LiteSVM core setup
// ---------------------------------------------------------------------------

pub fn build_svm_with_escrow_programs() -> (LiteSVM, solana_keypair::Keypair) {
    let payer = solana_keypair::Keypair::new();
    let mut svm = LiteSVM::new();

    svm.add_program(
        escrow::id(),
        include_bytes!("../../../../target/deploy/stayke_escrow.so"),
    )
    .unwrap();
    svm.add_program(
        core::id(),
        include_bytes!("../../../../target/deploy/stayke_core.so"),
    )
    .unwrap();
    svm.add_program(
        config::id(),
        include_bytes!("../../../../target/deploy/stayke_config.so"),
    )
    .unwrap();

    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer)
}

// ---------------------------------------------------------------------------
// PDA helpers
// ---------------------------------------------------------------------------

pub fn booking_pda(property: Pubkey, guest_profile: Pubkey, check_in: i64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            escrow::constants::BOOKING_SEED.as_bytes(),
            property.as_ref(),
            guest_profile.as_ref(),
            check_in.to_le_bytes().as_ref(),
        ],
        &escrow::id(),
    )
    .0
}

pub fn booking_days_pda(property: Pubkey, year: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[
            escrow::constants::BOOKING_DAYS_SEED.as_bytes(),
            property.as_ref(),
            year.to_le_bytes().as_ref(),
        ],
        &escrow::id(),
    )
    .0
}

pub fn escrow_token_pda(booking: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            escrow::constants::ESCROW_PDA_SEED.as_bytes(),
            booking.as_ref(),
        ],
        &escrow::id(),
    )
    .0
}

pub fn global_config_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    )
    .0
}

pub fn user_profile_pda(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &core::id(),
    )
    .0
}

pub fn reputation_profile_pda(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            core::constants::REPUTATION_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &core::id(),
    )
    .0
}

// ---------------------------------------------------------------------------
// Listing (stayke_core)
// ---------------------------------------------------------------------------

pub fn setup_listing(
    svm: &mut LiteSVM,
    user_profile: Pubkey,
    listing_id: u16,
    price: u64,
    is_active: bool,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            core::constants::LISTING_SEED.as_bytes(),
            user_profile.as_ref(),
            listing_id.to_le_bytes().as_ref(),
        ],
        &core::id(),
    );

    let listing = core::state::Listing {
        listing_id,
        total_reviews: 0,
        rating: 0,
        price,
        is_active,
        is_occupied: false,
        state_hash: [0u8; 32],
        content_ref: [0u8; 32],
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Listing", &listing),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// SPL Token helpers (classic program, mirroring the disputes test suite).
// The program uses `anchor_spl::token_interface` (Token2022-compatible); the
// tests exercise it through the classic SPL Token program, which LiteSVM has
// built-in.
// ---------------------------------------------------------------------------

/// Serialize a minimal SPL mint (82 bytes) with 6 decimals.
pub fn make_mint(svm: &mut LiteSVM, key: Pubkey) {
    let mut data = vec![0u8; 82];
    data[44] = 6; // decimals
    data[45] = 1; // is_initialized = true

    svm.set_account(
        key,
        Account {
            lamports: 1_000_000_000,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
}

/// Serialize a minimal SPL token account (165 bytes) with the given balance.
pub fn make_token_account(
    svm: &mut LiteSVM,
    key: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
) {
    // SPL Token Account layout (165 bytes):
    //   mint (0..32), owner (32..64), amount u64 (64..72),
    //   delegate COption<Pubkey> (72..108), state (108), is_native (109..121),
    //   delegated_amount (121..129), close_authority (129..165)
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // AccountState::Initialized

    svm.set_account(
        key,
        Account {
            lamports: 1_000_000_000,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// UserProfile with custom deposit / counters
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn setup_user_profile_custom(
    svm: &mut LiteSVM,
    authority: Pubkey,
    deposited: u64,
    completed_stays: u32,
    hosted_stays: u32,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &core::id(),
    );

    let profile = core::state::UserProfile {
        completed_stays,
        hosted_stays,
        authority,
        identity: Some(Pubkey::new_unique()),
        active_booking: None,
        deposited,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 0,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data_spaced("UserProfile", &profile),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// GlobalConfig with custom minimum_deposit and free_ops
// ---------------------------------------------------------------------------

pub fn setup_global_config_custom(
    svm: &mut LiteSVM,
    escrow_program: Pubkey,
    minimum_deposit: u64,
    free_ops: u8,
    usdc_mint: Pubkey,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    );

    let gc = config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops,
        minimum_deposit,
        fee_bps: 200,
        usdc_mint,
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: bump,
        core_program: core::id(),
        escrow_program,
        disputes_program: Pubkey::new_unique(),
        treasury_program: Pubkey::new_unique(),
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gc),
            owner: config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// GlobalConfig with an explicit platform vault (for fund-settlement tests).
// ---------------------------------------------------------------------------

pub fn setup_global_config_with_vault(
    svm: &mut LiteSVM,
    escrow_program: Pubkey,
    usdc_mint: Pubkey,
    platform_vault: Pubkey,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    );

    let gc = config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops: 4,
        minimum_deposit: 0,
        fee_bps: 200,
        usdc_mint,
        is_initialized: true,
        platform_vault,
        platform_vault_bump: bump,
        core_program: core::id(),
        escrow_program,
        disputes_program: Pubkey::new_unique(),
        treasury_program: Pubkey::new_unique(),
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gc),
            owner: config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// Escrow CPI authority PDA.
// ---------------------------------------------------------------------------

pub fn cpi_authority_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[config::constants::CPI_AUTHORITY_SEED.as_bytes()],
        &escrow::id(),
    )
    .0
}
