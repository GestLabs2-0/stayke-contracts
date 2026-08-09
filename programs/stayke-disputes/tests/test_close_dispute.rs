mod common;

use {
    anchor_lang::{InstructionData, ToAccountMetas},
    common::*,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_disputes::state::{DisputeReason, DisputeStatus},
};

/// Helper: create a minimal UserProfile at its PDA owned by stayke_core.
fn make_user_profile(svm: &mut LiteSVM, authority: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[stayke_core::constants::USER_PROFILE_SEED.as_bytes(), authority.as_ref()],
        &stayke_core::id(),
    );
    let p = stayke_core::state::UserProfile {
        authority, identity: Some(Pubkey::new_unique()), active_booking: None,
        deposited: 0, lending: 0, staked: 0, banned: false, listings: 0, bump,
    };
    svm.set_account(pda, solana_account::Account {
        lamports: 1_000_000_000, data: to_account_data("UserProfile", &p),
        owner: stayke_core::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();
    pda
}

/// Helper: create a Listing at its PDA for a given host profile.
fn make_listing(svm: &mut LiteSVM, host_profile_pda: Pubkey) -> Pubkey {
    let listing_id: u16 = 1;
    let (pda, bump) = Pubkey::find_program_address(
        &[stayke_core::constants::LISTING_SEED.as_bytes(), host_profile_pda.as_ref(), &listing_id.to_le_bytes()],
        &stayke_core::id(),
    );
    let l = stayke_core::state::Listing {
        owner: host_profile_pda, listing_id, total_reviews: 0, rating: 0, price: 100_000,
        is_occupied: true, state_hash: [0u8; 32], bump,
    };
    svm.set_account(pda, solana_account::Account {
        lamports: 1_000_000_000, data: to_account_data("Listing", &l),
        owner: stayke_core::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();
    pda
}

#[test]
fn close_dispute_resolved_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    let guest = Pubkey::new_unique(); let host = Pubkey::new_unique();
    let property = Pubkey::new_unique();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()], &stayke_config::id());
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(), minimum_deposit: 100_000, fee_bps: 200,
        usdc_mint: Pubkey::new_unique(), is_initialized: true,
        platform_vault: Pubkey::new_unique(), platform_vault_bump: global_bump,
        core_program: stayke_core::id(), escrow_program: stayke_escrow::id(),
        disputes_program: stayke_disputes::id(), treasury_program: stayke_treasury::id(),
        bump: global_bump,
    };
    svm.set_account(global_cfg_pda, solana_account::Account {
        lamports: 1_000_000_000, data: to_account_data("GlobalConfig", &gcfg),
        owner: stayke_config::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();

    let booking_key = Pubkey::new_unique();
    setup_booking(&mut svm, booking_key, guest, host, property,
        stayke_escrow::state::BookingStatus::DisputeResolved);

    let dispute_key = setup_dispute(&mut svm, booking_key, property, guest, host,
        DisputeReason::PropertyNotAsDescribed, DisputeStatus::Resolved);

    let guest_pda = make_user_profile(&mut svm, guest);
    let host_pda = make_user_profile(&mut svm, host);

    // Give guest an active_booking
    let (_, g_bump) = Pubkey::find_program_address(
        &[stayke_core::constants::USER_PROFILE_SEED.as_bytes(), guest.as_ref()],
        &stayke_core::id(),
    );
    let gp = stayke_core::state::UserProfile {
        authority: guest, identity: Some(Pubkey::new_unique()),
        active_booking: Some(booking_key), deposited: 0, lending: 0, staked: 0,
        banned: false, listings: 0, bump: g_bump,
    };
    svm.set_account(guest_pda, solana_account::Account {
        lamports: 1_000_000_000, data: to_account_data("UserProfile", &gp),
        owner: stayke_core::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();

    // Host profile with active_booking
    let (_, h_bump) = Pubkey::find_program_address(
        &[stayke_core::constants::USER_PROFILE_SEED.as_bytes(), host.as_ref()],
        &stayke_core::id(),
    );
    let hp = stayke_core::state::UserProfile {
        authority: host, identity: Some(Pubkey::new_unique()),
        active_booking: Some(booking_key), deposited: 0, lending: 0, staked: 0,
        banned: false, listings: 1, bump: h_bump,
    };
    svm.set_account(host_pda, solana_account::Account {
        lamports: 1_000_000_000, data: to_account_data("UserProfile", &hp),
        owner: stayke_core::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();

    let listing_key = make_listing(&mut svm, host_pda);

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    ).0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(), &stayke_disputes::instruction::CloseDispute {}.data(),
        stayke_disputes::accounts::CloseDispute {
            admin: admin.pubkey(), config: config_pda, dispute: dispute_key,
            booking: booking_key, guest_profile: guest_pda, host_profile: host_pda,
            listing: listing_key, global_config: global_cfg_pda, cpi_authority: cpi_auth,
            stayke_core_program: stayke_core::id(),
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Expected successful close_dispute, got: {:?}", res.err());
    assert!(svm.get_account(&dispute_key).is_none(), "Dispute should be closed");
}

#[test]
fn close_dispute_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let valid_admin = Keypair::new();
    let rogue_admin = Keypair::new();
    let guest = Pubkey::new_unique(); let host = Pubkey::new_unique();
    let property = Pubkey::new_unique();
    svm.airdrop(&rogue_admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, valid_admin.pubkey());
    let booking_key = Pubkey::new_unique();

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()], &stayke_config::id());
    svm.set_account(global_cfg_pda, solana_account::Account {
        lamports: 1_000_000_000,
        data: to_account_data("GlobalConfig", &stayke_config::state::GlobalConfig {
            authority: Pubkey::new_unique(), minimum_deposit: 100_000, fee_bps: 200,
            usdc_mint: Pubkey::new_unique(), is_initialized: true,
            platform_vault: Pubkey::new_unique(), platform_vault_bump: global_bump,
            core_program: stayke_core::id(), escrow_program: stayke_escrow::id(),
            disputes_program: stayke_disputes::id(), treasury_program: Pubkey::new_unique(),
            bump: global_bump,
        }),
        owner: stayke_config::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();

    let dispute_key = setup_dispute(&mut svm, booking_key, property, guest, host,
        DisputeReason::PropertyNotAsDescribed, DisputeStatus::Resolved);

    setup_booking(&mut svm, booking_key, guest, host, property,
        stayke_escrow::state::BookingStatus::DisputeResolved);

    let guest_pda = make_user_profile(&mut svm, guest);
    let host_pda = make_user_profile(&mut svm, host);
    let listing_key = make_listing(&mut svm, host_pda);

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    ).0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(), &stayke_disputes::instruction::CloseDispute {}.data(),
        stayke_disputes::accounts::CloseDispute {
            admin: rogue_admin.pubkey(), config: config_pda, dispute: dispute_key,
            booking: booking_key, guest_profile: guest_pda, host_profile: host_pda,
            listing: listing_key, global_config: global_cfg_pda, cpi_authority: cpi_auth,
            stayke_core_program: stayke_core::id(),
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg), &[&payer, &rogue_admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().err,
        TransactionError::InstructionError(0,
            Custom(u32::from(stayke_disputes::error::DisputeError::UnauthorizedAdmin))));
}

#[test]
fn close_dispute_still_open_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    let guest = Pubkey::new_unique(); let host = Pubkey::new_unique();
    let property = Pubkey::new_unique();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let booking_key = Pubkey::new_unique();

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()], &stayke_config::id());
    svm.set_account(global_cfg_pda, solana_account::Account {
        lamports: 1_000_000_000,
        data: to_account_data("GlobalConfig", &stayke_config::state::GlobalConfig {
            authority: Pubkey::new_unique(), minimum_deposit: 100_000, fee_bps: 200,
            usdc_mint: Pubkey::new_unique(), is_initialized: true,
            platform_vault: Pubkey::new_unique(), platform_vault_bump: global_bump,
            core_program: stayke_core::id(), escrow_program: stayke_escrow::id(),
            disputes_program: stayke_disputes::id(), treasury_program: Pubkey::new_unique(),
            bump: global_bump,
        }),
        owner: stayke_config::id(), executable: false, rent_epoch: u64::MAX,
    }).unwrap();

    let dispute_key = setup_dispute(&mut svm, booking_key, property, guest, host,
        DisputeReason::PropertyNotAsDescribed, DisputeStatus::Open);

    setup_booking(&mut svm, booking_key, guest, host, property,
        stayke_escrow::state::BookingStatus::DisputeResolved);

    let guest_pda = make_user_profile(&mut svm, guest);
    let host_pda = make_user_profile(&mut svm, host);
    let listing_key = make_listing(&mut svm, host_pda);

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    ).0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(), &stayke_disputes::instruction::CloseDispute {}.data(),
        stayke_disputes::accounts::CloseDispute {
            admin: admin.pubkey(), config: config_pda, dispute: dispute_key,
            booking: booking_key, guest_profile: guest_pda, host_profile: host_pda,
            listing: listing_key, global_config: global_cfg_pda, cpi_authority: cpi_auth,
            stayke_core_program: stayke_core::id(),
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().err,
        TransactionError::InstructionError(0,
            Custom(u32::from(stayke_disputes::error::DisputeError::DisputeNotOpen))));
}
