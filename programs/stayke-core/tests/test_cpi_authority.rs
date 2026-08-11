//! LiteSVM: wallet cannot call privileged core mutators.

use {
    anchor_lang::{
        prelude::{AccountDeserialize, AccountSerialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED},
    stayke_core::{Listing, UserProfile, LISTING_SEED, USER_PROFILE_SEED},
};

fn program_pubkey(s: &str) -> Pubkey {
    s.parse().unwrap()
}

fn sample_global_config(bump: u8) -> GlobalConfig {
    GlobalConfig {
        authority: Pubkey::new_unique(),
        minimum_deposit: 100_000,
        fee_bps: 500,
        usdc_mint: Pubkey::new_unique(),
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: 255,
        core_program: program_pubkey("8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP"),
        escrow_program: program_pubkey("FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ"),
        disputes_program: program_pubkey("7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB"),
        treasury_program: program_pubkey("59buEPHFBK4h8LyLE2KtnV1kpaQTyjb82NWt5F9jSuHu"),
        bump,
    }
}

fn serialize_account<T: AccountSerialize>(data: &T) -> Vec<u8> {
    let mut out = Vec::new();
    data.try_serialize(&mut out).unwrap();
    out
}

#[test]
fn wallet_update_deposit_unauthorized_leaves_deposited_unchanged() {
    let core_id = stayke_core::id();
    let config_id = stayke_config::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_core.so");
    assert!(
        bytes.len() > 10_000,
        "stayke_core.so looks stub-sized ({})",
        bytes.len()
    );
    svm.add_program(core_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &config_id);
    let gc = sample_global_config(gc_bump);
    svm.set_account(
        global_config_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&gc),
            owner: config_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (profile_pda, profile_bump) = Pubkey::find_program_address(
        &[USER_PROFILE_SEED.as_bytes(), payer.pubkey().as_ref()],
        &core_id,
    );
    let profile = UserProfile {
        authority: payer.pubkey(),
        identity: Some(Pubkey::new_unique()),
        active_booking: None,
        deposited: 42,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 0,
        bump: profile_bump,
    };
    svm.set_account(
        profile_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&profile),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        core_id,
        &stayke_core::instruction::UpdateDeposit {
            amount: 100,
            is_deposit: true,
        }
        .data(),
        stayke_core::accounts::UpdateUserProfile {
            user_profile: profile_pda,
            global_config: global_config_pda,
            cpi_authority: payer.pubkey(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(
        svm.send_transaction(tx).is_err(),
        "wallet direct update_deposit must fail"
    );

    let acc = svm.get_account(&profile_pda).unwrap();
    let after = UserProfile::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(after.deposited, 42);
}

#[test]
fn wallet_clear_listing_booking_unauthorized_leaves_occupied_true() {
    let core_id = stayke_core::id();
    let config_id = stayke_config::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_core.so");
    assert!(
        bytes.len() > 10_000,
        "stayke_core.so looks stub-sized ({})",
        bytes.len()
    );
    svm.add_program(core_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &config_id);
    svm.set_account(
        global_config_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&sample_global_config(gc_bump)),
            owner: config_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (profile_pda, profile_bump) = Pubkey::find_program_address(
        &[USER_PROFILE_SEED.as_bytes(), payer.pubkey().as_ref()],
        &core_id,
    );
    let profile = UserProfile {
        authority: payer.pubkey(),
        identity: Some(Pubkey::new_unique()),
        active_booking: None,
        deposited: 0,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 1,
        bump: profile_bump,
    };
    svm.set_account(
        profile_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&profile),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let listing_id: u16 = 1;
    let (listing_pda, listing_bump) = Pubkey::find_program_address(
        &[
            LISTING_SEED.as_bytes(),
            profile_pda.as_ref(),
            listing_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let listing = Listing {
        owner: profile_pda,
        listing_id,
        total_reviews: 0,
        rating: 0,
        price: 1_000_000,
        is_occupied: true,
        state_hash: [0u8; 32],
        bump: listing_bump,
    };
    svm.set_account(
        listing_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&listing),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        core_id,
        &stayke_core::instruction::ClearListingBooking {}.data(),
        stayke_core::accounts::ClearListingBooking {
            user_profile: profile_pda,
            listing: listing_pda,
            global_config: global_config_pda,
            cpi_authority: payer.pubkey(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(
        svm.send_transaction(tx).is_err(),
        "wallet direct clear_listing_booking must fail"
    );

    let acc = svm.get_account(&listing_pda).unwrap();
    let after = Listing::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(
        after.is_occupied,
        "occupied must remain true after unauthorized call"
    );
}
