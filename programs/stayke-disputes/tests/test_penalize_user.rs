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
    stayke_core::state::PenaltySeverity,
};

#[test]
fn penalize_user_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let valid_admin = Keypair::new();
    let rogue_admin = Keypair::new();
    let penalized = Pubkey::new_unique();
    svm.airdrop(&rogue_admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, valid_admin.pubkey());

    let usdc_mint = Pubkey::new_unique();

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        minimum_deposit: 100_000,
        fee_bps: 200,
        usdc_mint,
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

    setup_user_profile(&mut svm, penalized);
    let reputation_pda = setup_reputation_profile(&mut svm, penalized);

    let (treasury_config_pda, tc_bump) = Pubkey::find_program_address(
        &[stayke_treasury::TREASURY_CONFIG_SEED.as_bytes()],
        &stayke_treasury::id(),
    );
    svm.set_account(
        treasury_config_pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data(
                "TreasuryConfig",
                &stayke_treasury::state::TreasuryConfig {
                    authority: valid_admin.pubkey(),
                    treasury_vault: Pubkey::new_unique(),
                    treasury_bump: tc_bump,
                    global_config: global_cfg_pda,
                    is_initialized: true,
                    bump: tc_bump,
                },
            ),
            owner: stayke_treasury::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    let vault_ata = Pubkey::new_unique();
    let affected_ata = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    make_token_account(&mut svm, vault_ata, usdc_mint, Pubkey::new_unique());
    make_token_account(&mut svm, affected_ata, usdc_mint, Pubkey::new_unique());

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;
    let penalized_pda = Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            penalized.as_ref(),
        ],
        &stayke_core::id(),
    )
    .0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::PenalizeUser {
            severity: PenaltySeverity::Low,
        }
        .data(),
        stayke_disputes::accounts::PenalizeUser {
            admin: rogue_admin.pubkey(),
            config: config_pda,
            penalized_user_profile: penalized_pda,
            penalized_reputation_profile: reputation_pda,
            affected_token_account: affected_ata,
            affected_wallet: Pubkey::new_unique(),
            treasury_config: treasury_config_pda,
            global_config: global_cfg_pda,
            treasury_vault: vault_ata,
            treasury_pda: Pubkey::new_unique(),
            usdc_mint,
            cpi_authority: cpi_auth,
            stayke_core_program: stayke_core::id(),
            stayke_treasury_program: stayke_treasury::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &rogue_admin])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(0,
            Custom(u32::from(stayke_disputes::error::DisputeError::UnauthorizedAdmin)))
    );
}

#[test]
fn penalize_user_banned_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    let penalized = Pubkey::new_unique();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());

    let usdc_mint = Pubkey::new_unique();

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        minimum_deposit: 100_000,
        fee_bps: 200,
        usdc_mint,
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

    setup_banned_user_profile(&mut svm, penalized);
    let reputation_pda = setup_reputation_profile(&mut svm, penalized);

    let (treasury_config_pda, tc_bump) = Pubkey::find_program_address(
        &[stayke_treasury::TREASURY_CONFIG_SEED.as_bytes()],
        &stayke_treasury::id(),
    );
    svm.set_account(
        treasury_config_pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data(
                "TreasuryConfig",
                &stayke_treasury::state::TreasuryConfig {
                    authority: admin.pubkey(),
                    treasury_vault: Pubkey::new_unique(),
                    treasury_bump: tc_bump,
                    global_config: global_cfg_pda,
                    is_initialized: true,
                    bump: tc_bump,
                },
            ),
            owner: stayke_treasury::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    let vault_ata = Pubkey::new_unique();
    let affected_ata = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    make_token_account(&mut svm, vault_ata, usdc_mint, Pubkey::new_unique());
    make_token_account(&mut svm, affected_ata, usdc_mint, Pubkey::new_unique());

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;
    let penalized_pda = Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            penalized.as_ref(),
        ],
        &stayke_core::id(),
    )
    .0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::PenalizeUser {
            severity: PenaltySeverity::Medium,
        }
        .data(),
        stayke_disputes::accounts::PenalizeUser {
            admin: admin.pubkey(),
            config: config_pda,
            penalized_user_profile: penalized_pda,
            penalized_reputation_profile: reputation_pda,
            affected_token_account: affected_ata,
            affected_wallet: Pubkey::new_unique(),
            treasury_config: treasury_config_pda,
            global_config: global_cfg_pda,
            treasury_vault: vault_ata,
            treasury_pda: Pubkey::new_unique(),
            usdc_mint,
            cpi_authority: cpi_auth,
            stayke_core_program: stayke_core::id(),
            stayke_treasury_program: stayke_treasury::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &admin]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(0,
            Custom(u32::from(stayke_disputes::error::DisputeError::UserBanned)))
    );
}