#![allow(dead_code)]
//! Shared helpers for stayke-disputes LiteSVM tests.

use anchor_lang::AnchorSerialize;
use litesvm::LiteSVM;
use solana_account::Account;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use stayke_config as config;
use stayke_core as core;
use stayke_disputes as disputes;
use stayke_escrow as escrow;
use stayke_treasury as treasury;

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

// ---------------------------------------------------------------------------
// GlobalConfig (stayke_config)
// ---------------------------------------------------------------------------

pub fn setup_global_config(
    svm: &mut LiteSVM,
    disputes_program: Pubkey,
    escrow_program: Pubkey,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    );

    let config = config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        max_operations: 4,
        minimum_deposit: 100_000,
        fee_bps: 200,
        usdc_mint: Pubkey::new_unique(),
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: bump,
        core_program: Pubkey::new_unique(),
        escrow_program,
        disputes_program,
        treasury_program: Pubkey::new_unique(),
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &config),
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
        deposited: 0,
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
            data: to_account_data("UserProfile", &profile),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

pub fn setup_banned_user_profile(svm: &mut LiteSVM, authority: Pubkey) -> Pubkey {
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
        deposited: 0,
        lending: 0,
        staked: 0,
        banned: true,
        listings: 0,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("UserProfile", &profile),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

pub fn setup_unverified_user_profile(svm: &mut LiteSVM, authority: Pubkey) -> Pubkey {
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
        identity: None,
        active_booking: None,
        deposited: 0,
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
            data: to_account_data("UserProfile", &profile),
            owner: core::id(),
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

pub fn setup_booking(
    svm: &mut LiteSVM,
    booking_key: Pubkey,
    guest: Pubkey,
    host: Pubkey,
    property: Pubkey,
    status: escrow::state::BookingStatus,
) -> Pubkey {
    let booking = escrow::state::Booking {
        guest,
        host,
        property,
        deposit: 0,
        check_in: 0,
        check_out: 0,
        days: 0,
        check_in_date: escrow::utils::DateComponents {
            day: 1,
            month: 1,
            year: 2026,
            year_month: 202601,
        },
        check_out_date: escrow::utils::DateComponents {
            day: 2,
            month: 1,
            year: 2026,
            year_month: 202601,
        },
        total_price: 0,
        review: 0,
        status,
        escrow_bump: 255,
        bump: 255,
    };

    svm.set_account(
        booking_key,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &booking),
            owner: escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    booking_key
}

// ---------------------------------------------------------------------------
// DisputeConfig (stayke-disputes)
// ---------------------------------------------------------------------------

pub fn setup_dispute_config(svm: &mut LiteSVM, admin: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &disputes::id(),
    );

    let config = disputes::state::DisputeConfig {
        admins: vec![admin],
        retribution_bps_low: 1000,
        retribution_bps_medium: 3000,
        retribution_bps_high: 10000,
        is_initialized: true,
        bump,
    };

    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("DisputeConfig", &config),
            owner: disputes::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    pda
}

// ---------------------------------------------------------------------------
// LiteSVM core setup
// ---------------------------------------------------------------------------

pub fn build_svm_with_programs() -> (LiteSVM, solana_keypair::Keypair) {
    let payer = solana_keypair::Keypair::new();
    let mut svm = LiteSVM::new();

    svm.add_program(
        disputes::id(),
        include_bytes!("../../../../target/deploy/stayke_disputes.so"),
    )
    .unwrap();
    svm.add_program(
        escrow::id(),
        include_bytes!("../../../../target/deploy/stayke_escrow.so"),
    )
    .unwrap();
    svm.add_program(
        config::id(),
        include_bytes!("../../../../target/deploy/stayke_config.so"),
    )
    .unwrap();
    svm.add_program(
        core::id(),
        include_bytes!("../../../../target/deploy/stayke_core.so"),
    )
    .unwrap();
    svm.add_program(
        treasury::id(),
        include_bytes!("../../../../target/deploy/stayke_treasury.so"),
    )
    .unwrap();

    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer)
}

// ---------------------------------------------------------------------------
// PDA helpers
// ---------------------------------------------------------------------------

pub fn dispute_pda(booking: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            disputes::constants::DISPUTE_PDA_SEED.as_bytes(),
            booking.as_ref(),
        ],
        &disputes::id(),
    )
    .0
}

pub fn cpi_authority_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[config::constants::CPI_AUTHORITY_SEED.as_bytes()],
        program_id,
    )
    .0
}

// ---------------------------------------------------------------------------
// Listing (stayke_core)
// ---------------------------------------------------------------------------

pub fn setup_listing(svm: &mut LiteSVM, owner: Pubkey, host_profile: Pubkey) -> Pubkey {
    let listing_id: u16 = 1;
    let (listing_key, bump) = Pubkey::find_program_address(
        &[
            core::constants::LISTING_SEED.as_bytes(),
            host_profile.as_ref(),
            &listing_id.to_le_bytes(),
        ],
        &core::id(),
    );

    let listing = core::state::Listing {
        is_active: true,
        listing_id,
        total_reviews: 0,
        rating: 0,
        content_ref: [0; 32],
        price: 100_000,
        is_occupied: true,
        state_hash: [0u8; 32],
        bump,
    };

    svm.set_account(
        listing_key,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Listing", &listing),
            owner: core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    listing_key
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
        host_cancellations_within_48h: 0,
        client_cancellations_within_48h: 0,
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
// SPL Token helpers (manual byte serialization for SDK 3.x compatibility)
// ---------------------------------------------------------------------------

/// Serialize a minimal SPL token account with zero balance.
pub fn make_token_account(svm: &mut LiteSVM, key: Pubkey, mint: Pubkey, owner: Pubkey) {
    // SPL Token Account layout (165 bytes):
    //   mint:             Pubkey        0..32
    //   owner:            Pubkey        32..64
    //   amount:           u64           64..72
    //   delegate:         COption<Pub>  72..108   (4-byte tag + optional 32-byte Pubkey)
    //   state:            AccountState  108       (1 byte, 1 = Initialized)
    //   is_native:        COption<u64>  109..121   (4-byte tag + optional 8-byte u64)
    //   delegated_amount: u64           121..129
    //   close_authority:  COption<Pub>  129..165   (4-byte tag + optional 32-byte Pubkey)
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
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

/// Serialize a minimal SPL mint with zero supply.
pub fn make_mint(svm: &mut LiteSVM, key: Pubkey) {
    // SPL Mint layout (82 bytes):
    //   mint_authority:   COption<Pub>  0..36
    //   supply:           u64            36..44
    //   decimals:         u8             44
    //   is_initialized:   bool           45
    //   freeze_authority: COption<Pub>   46..82
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

// ---------------------------------------------------------------------------
// Dispute account
// ---------------------------------------------------------------------------

pub fn setup_dispute(
    svm: &mut LiteSVM,
    booking: Pubkey,
    property: Pubkey,
    initiator: Pubkey,
    guilty: Pubkey,
    reason: disputes::state::DisputeReason,
    status: disputes::state::DisputeStatus,
) -> Pubkey {
    let (dispute_key, bump) = Pubkey::find_program_address(
        &[
            disputes::constants::DISPUTE_PDA_SEED.as_bytes(),
            booking.as_ref(),
        ],
        &disputes::id(),
    );

    let dispute = disputes::state::Dispute {
        booking,
        property,
        initiator,
        guilty,
        reason,
        status: status.clone(),
        created_at: 0,
        resolved_at: if status != disputes::state::DisputeStatus::Open {
            Some(1000)
        } else {
            None
        },
        bump,
    };

    svm.set_account(
        dispute_key,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Dispute", &dispute),
            owner: disputes::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    dispute_key
}
