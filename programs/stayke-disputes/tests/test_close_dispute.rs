// ---------------------------------------------------------------------------
// DEPRECATED - STK-168 refactor: P2P dispute flow with admin escalation.
// This test targets the old admin-mediated dispute model and is commented
// out to keep the test suite compiling during the refactor.
// ---------------------------------------------------------------------------
/*
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

fn set_active_booking(svm: &mut LiteSVM, authority: Pubkey, booking: Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &stayke_core::id(),
    );
    let p = stayke_core::state::UserProfile {
        completed_stays: 0,
        hosted_stays: 0,
        authority,
        identity: Some(Pubkey::new_unique()),
        active_booking: Some(booking),
        deposited: 0,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 1,
        bump,
    };
    svm.set_account(
        pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("UserProfile", &p),
            owner: stayke_core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    pda
}

fn global_config(svm: &mut LiteSVM) -> Pubkey {
    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        minimum_deposit: 100_000,
        fee_bps: 200,
        free_ops: 4,
        usdc_mint: Pubkey::new_unique(),
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: global_bump,
        core_program: stayke_core::id(),
        escrow_program: stayke_escrow::id(),
        disputes_program: stayke_disputes::id(),
        treasury_program: stayke_treasury::id(),
        bump: global_bump,
    };
    svm.set_account(
        global_cfg_pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gcfg),
            owner: stayke_config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    global_cfg_pda
}

fn close_ix(
    admin: Pubkey,
    bound: &BoundBooking,
    dispute_key: Pubkey,
    global_cfg_pda: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    listing: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::CloseDispute {}.data(),
        stayke_disputes::accounts::CloseDispute {
            admin,
            config: config_pda,
            dispute: dispute_key,
            booking: bound.booking,
            guest_profile,
            host_profile,
            listing,
            global_config: global_cfg_pda,
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            stayke_core_program: stayke_core::id(),
        }
        .to_account_metas(None),
    )
}

#[test]
fn close_dispute_resolved_success() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let global_cfg_pda = global_config(&mut svm);

    let bound = setup_bound_booking(
        &mut svm,
        stayke_escrow::state::BookingStatus::DisputeResolved,
    );
    set_active_booking(&mut svm, bound.guest_wallet, bound.booking);
    set_active_booking(&mut svm, bound.host_wallet, bound.booking);

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Resolved,
    );

    let instruction = close_ix(
        admin.pubkey(),
        &bound,
        dispute_key,
        global_cfg_pda,
        bound.guest_profile,
        bound.host_profile,
        bound.listing,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful close_dispute, got: {:?}",
        res.err()
    );
    assert!(
        svm.get_account(&dispute_key).is_none(),
        "Dispute should be closed"
    );
}

#[test]
fn close_dispute_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let valid_admin = Keypair::new();
    let rogue_admin = Keypair::new();
    svm.airdrop(&rogue_admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, valid_admin.pubkey());
    let global_cfg_pda = global_config(&mut svm);
    let bound = setup_bound_booking(
        &mut svm,
        stayke_escrow::state::BookingStatus::DisputeResolved,
    );

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Resolved,
    );

    let instruction = close_ix(
        rogue_admin.pubkey(),
        &bound,
        dispute_key,
        global_cfg_pda,
        bound.guest_profile,
        bound.host_profile,
        bound.listing,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &rogue_admin])
        .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnauthorizedAdmin
            ))
        )
    );
}

#[test]
fn close_dispute_still_open_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let global_cfg_pda = global_config(&mut svm);
    let bound = setup_bound_booking(
        &mut svm,
        stayke_escrow::state::BookingStatus::DisputeResolved,
    );

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Open,
    );

    let instruction = close_ix(
        admin.pubkey(),
        &bound,
        dispute_key,
        global_cfg_pda,
        bound.guest_profile,
        bound.host_profile,
        bound.listing,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotOpen
            ))
        )
    );
}

#[test]
fn close_dispute_unrelated_guest_profile_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let global_cfg_pda = global_config(&mut svm);
    let bound = setup_bound_booking(
        &mut svm,
        stayke_escrow::state::BookingStatus::DisputeResolved,
    );
    let stranger = setup_user_profile(&mut svm, Pubkey::new_unique());

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Resolved,
    );

    let instruction = close_ix(
        admin.pubkey(),
        &bound,
        dispute_key,
        global_cfg_pda,
        stranger,
        bound.host_profile,
        bound.listing,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnboundBookingAccount
            ))
        )
    );
}
*/
