//! LiteSVM: initialize_listing persists the PDA bump.

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stayke_core::{Listing, UserProfile, LISTING_SEED, USER_PROFILE_SEED},
};

fn serialize_account<T: anchor_lang::AccountSerialize>(data: &T) -> Vec<u8> {
    let mut out = Vec::new();
    data.try_serialize(&mut out).unwrap();
    out
}

#[test]
fn initialize_listing_writes_bump() {
    let core_id = stayke_core::id();
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
        listings: 0,
        completed_stays: 0,
        hosted_stays: 0,
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

    let listing_id: u16 = 0;
    let (listing_pda, listing_bump) = Pubkey::find_program_address(
        &[
            LISTING_SEED.as_bytes(),
            profile_pda.as_ref(),
            listing_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );

    let ix = Instruction::new_with_bytes(
        core_id,
        &stayke_core::instruction::InitializeListing {
            price: 1_000_000,
            listing_id,
            state_hash: [7u8; 32],
            content_ref: [3u8; 32],
        }
        .data(),
        stayke_core::accounts::InitializeListing {
            payer: payer.pubkey(),
            authority: payer.pubkey(),
            listing: listing_pda,
            user_profile: profile_pda,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx)
        .expect("initialize_listing must succeed");

    let acc = svm.get_account(&listing_pda).expect("listing exists");
    let listing = Listing::try_deserialize(&mut acc.data.as_slice()).expect("deserialize listing");
    assert_eq!(listing.bump, listing_bump);
    assert_eq!(listing.listing_id, listing_id);
    assert_eq!(listing.price, 1_000_000);
    assert!(listing.is_active);

    let profile_acc = svm.get_account(&profile_pda).unwrap();
    let after = UserProfile::try_deserialize(&mut profile_acc.data.as_slice()).unwrap();
    assert_eq!(after.listings, 1);
}
