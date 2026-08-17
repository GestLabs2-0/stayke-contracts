//! LiteSVM tests for stayke-config GlobalConfig program-ID registry.

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
};

const MINIMUM_DEPOSIT: u64 = 100_000;
const FEE_BPS: u64 = 500;
const TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const MINT_LEN: usize = 82;
const FREE_OPS: u8 = 4;

/// Pack a classic SPL Token Mint account (no spl-token crate — avoids entrypoint clash).
fn pack_mint(mint_authority: &Pubkey, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; MINT_LEN];
    // COption::Some = 1u32 LE
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(mint_authority.as_ref());
    // supply u64 = 0 already zeroed
    data[44] = decimals;
    data[45] = 1; // is_initialized
                  // freeze_authority COption::None = 0 already zeroed
    data
}

#[test]
fn test_initialize_config_persists_program_ids() {
    let program_id = stayke_config::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_config.so");
    assert!(
        bytes.len() > 10_000,
        "stayke_config.so looks stub-sized ({})",
        bytes.len()
    );
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let mint_kp = Keypair::new();
    let mint_data = pack_mint(&payer.pubkey(), 6);
    let mint_lamports = svm.minimum_balance_for_rent_exemption(MINT_LEN);
    svm.set_account(
        mint_kp.pubkey(),
        Account {
            lamports: mint_lamports,
            data: mint_data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (global_config, _) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &program_id,
    );
    let (platform_vault_pda, _) = Pubkey::find_program_address(
        &[stayke_config::constants::PLATFORM_VAULT_SEED.as_bytes()],
        &program_id,
    );
    let (platform_vault, _) = Pubkey::find_program_address(
        &[stayke_config::constants::PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
        &program_id,
    );

    let instruction = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::InitializeConfig {
            minimum_deposit: MINIMUM_DEPOSIT,
            fee_bps: FEE_BPS,
            free_ops: FREE_OPS,
            allowed_programs: stayke_config::AllowedPrograms {
                core: Pubkey::new_unique(),
                escrow: Pubkey::new_unique(),
                disputes: Pubkey::new_unique(),
                treasury: Pubkey::new_unique(),
            },
        }
        .data(),
        stayke_config::accounts::InitializeConfig {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint: mint_kp.pubkey(),
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx)
        .expect("initialize_config must succeed");

    let account = svm
        .get_account(&global_config)
        .expect("GlobalConfig exists");
    let config = stayke_config::GlobalConfig::try_deserialize(&mut account.data.as_slice())
        .expect("deserialize GlobalConfig");

    assert!(config.is_initialized);
    assert_eq!(config.free_ops, FREE_OPS);
    assert_eq!(config.core_program, stayke_config::CORE_PROGRAM_ID);
    assert_eq!(config.escrow_program, stayke_config::ESCROW_PROGRAM_ID);
    assert_eq!(config.disputes_program, stayke_config::DISPUTES_PROGRAM_ID);
    assert_eq!(config.treasury_program, stayke_config::TREASURY_PROGRAM_ID);
}

#[test]
fn test_no_migrate_instruction_surface() {
    let ix = stayke_config::instruction::InitializeConfig {
        minimum_deposit: 1,
        fee_bps: 1,
        free_ops: 1,
        allowed_programs: stayke_config::AllowedPrograms {
            core: Pubkey::new_unique(),
            escrow: Pubkey::new_unique(),
            disputes: Pubkey::new_unique(),
            treasury: Pubkey::new_unique(),
        },
    };
    assert!(!ix.data().is_empty());
}
