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
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_disputes::state::{DisputeReason, DisputeStatus},
    stayke_escrow::state::BookingStatus,
};

fn global_config(svm: &mut litesvm::LiteSVM, usdc_mint: Pubkey, platform_vault: Pubkey) -> Pubkey {
    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops: 4,
        minimum_deposit: 100_000,
        fee_bps: 200,
        usdc_mint,
        is_initialized: true,
        platform_vault,
        platform_vault_bump: global_bump,
        core_program: Pubkey::new_unique(),
        escrow_program: stayke_escrow::id(),
        disputes_program: stayke_disputes::id(),
        treasury_program: Pubkey::new_unique(),
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

struct ResolveAccounts {
    host_ata: Pubkey,
    guest_ata: Pubkey,
    escrow_ata: Pubkey,
    usdc_mint: Pubkey,
    platform_vault: Pubkey,
    global_cfg: Pubkey,
}

fn setup_resolve_tokens(svm: &mut litesvm::LiteSVM, bound: &BoundBooking) -> ResolveAccounts {
    let usdc_mint = Pubkey::new_unique();
    let platform_vault = Pubkey::new_unique();
    let escrow_ata = Pubkey::new_unique();
    let host_ata = Pubkey::new_unique();
    let guest_ata = Pubkey::new_unique();
    make_mint(svm, usdc_mint);
    make_token_account(svm, escrow_ata, usdc_mint, Pubkey::new_unique());
    make_token_account(svm, host_ata, usdc_mint, bound.host_wallet);
    make_token_account(svm, guest_ata, usdc_mint, bound.guest_wallet);
    make_token_account(svm, platform_vault, usdc_mint, Pubkey::new_unique());
    let global_cfg = global_config(svm, usdc_mint, platform_vault);
    ResolveAccounts {
        host_ata,
        guest_ata,
        escrow_ata,
        usdc_mint,
        platform_vault,
        global_cfg,
    }
}

fn resolve_ix(
    admin: Pubkey,
    bound: &BoundBooking,
    dispute_key: Pubkey,
    tokens: &ResolveAccounts,
    host_ata: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;
    anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::ResolveDispute {
            host_share_bps: 5000,
            rejected: false,
        }
        .data(),
        stayke_disputes::accounts::ResolveDispute {
            admin,
            config: config_pda,
            dispute: dispute_key,
            booking: bound.booking,
            host_profile: bound.host_profile,
            guest_profile: bound.guest_profile,
            global_config: tokens.global_cfg,
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            escrow_token_account: tokens.escrow_ata,
            host_token_account: host_ata,
            guest_token_account: tokens.guest_ata,
            platform_vault_token_account: tokens.platform_vault,
            usdc_mint: tokens.usdc_mint,
            stayke_escrow_program: stayke_escrow::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    )
}

#[test]
fn resolve_dispute_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let valid_admin = Keypair::new();
    let rogue_admin = Keypair::new();
    svm.airdrop(&rogue_admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, valid_admin.pubkey());
    let bound = setup_bound_booking(&mut svm, BookingStatus::Active);
    let tokens = setup_resolve_tokens(&mut svm, &bound);

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Open,
    );

    let instruction = resolve_ix(
        rogue_admin.pubkey(),
        &bound,
        dispute_key,
        &tokens,
        tokens.host_ata,
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
fn resolve_dispute_already_resolved_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let bound = setup_bound_booking(&mut svm, BookingStatus::Active);
    let tokens = setup_resolve_tokens(&mut svm, &bound);

    let dispute_key = setup_dispute(
        &mut svm,
        bound.booking,
        bound.listing,
        bound.guest_profile,
        bound.host_profile,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Resolved,
    );

    let instruction = resolve_ix(
        admin.pubkey(),
        &bound,
        dispute_key,
        &tokens,
        tokens.host_ata,
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
fn resolve_dispute_foreign_host_token_account_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let bound = setup_bound_booking(&mut svm, BookingStatus::Disputed);
    let tokens = setup_resolve_tokens(&mut svm, &bound);

    let foreign_ata = Pubkey::new_unique();
    make_token_account(
        &mut svm,
        foreign_ata,
        tokens.usdc_mint,
        Pubkey::new_unique(),
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

    let instruction = resolve_ix(admin.pubkey(), &bound, dispute_key, &tokens, foreign_ata);

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
                stayke_disputes::error::DisputeError::InvalidPayoutTokenAccount
            ))
        )
    );
}
*/
