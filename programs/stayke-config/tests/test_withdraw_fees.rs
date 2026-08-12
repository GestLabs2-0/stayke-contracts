//! LiteSVM tests for stayke-config withdraw_fees instruction.

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
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
const MAX_OPERATIONS: u8 = 4;
const TOKEN_ACCOUNT_LEN: usize = 165;

/// Pack a classic SPL Token Mint account (no spl-token crate — avoids entrypoint clash).
fn pack_mint(mint_authority: &Pubkey, decimals: u8) -> Vec<u8> {
    let mut data = vec![0u8; MINT_LEN];
    // COption::Some = 1u32 LE
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(mint_authority.as_ref());
    // supply u64 = 0 already zeroed
    data[44] = decimals;
    data[45] = 1; // is_initialized
    data
}

/// Pack an initialized SPL Token account with the given mint, owner, and amount.
fn pack_token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; TOKEN_ACCOUNT_LEN];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    // delegate: COption::None = 0u32 LE (already zeroed at offset 72)
    data[108] = 1; // state: Initialized
    data
}

/// Create an SPL Token account manually via set_account.
fn create_token_account(svm: &mut LiteSVM, mint: &Pubkey, owner: &Pubkey, amount: u64) -> Pubkey {
    let kp = Keypair::new();
    let data = pack_token_account(mint, owner, amount);
    let lamports = svm.minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_LEN);
    svm.set_account(
        kp.pubkey(),
        Account {
            lamports,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[test]
fn test_withdraw_fees_success() {
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

    // --- Setup: mint ---
    let mint_kp = Keypair::new();
    let mint_data = pack_mint(&payer.pubkey(), 6); // USDC-like: 6 decimals
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
    let usdc_mint = mint_kp.pubkey();

    // --- Setup: PDAs ---
    let (global_config, _) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &program_id,
    );
    let (platform_vault_pda, _vault_pda_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::PLATFORM_VAULT_SEED.as_bytes()],
        &program_id,
    );
    let (platform_vault, _) = Pubkey::find_program_address(
        &[stayke_config::constants::PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
        &program_id,
    );

    // --- Step 1: initialize_config ---
    let ix_init = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::InitializeConfig {
            minimum_deposit: MINIMUM_DEPOSIT,
            fee_bps: FEE_BPS,
            max_operations: MAX_OPERATIONS,
        }
        .data(),
        stayke_config::accounts::InitializeConfig {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix_init], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx)
        .expect("initialize_config must succeed");

    // --- Step 2: fund the platform vault with 500 USDC (6 decimals) ---
    let vault_amount: u64 = 500_000_000; // 500.000000 USDC
    {
        let mut vault_account = svm.get_account(&platform_vault).expect("vault exists");
        vault_account.data[64..72].copy_from_slice(&vault_amount.to_le_bytes());
        svm.set_account(platform_vault, vault_account).unwrap();
    }

    // --- Step 3: create the authority's destination token account ---
    let destination = create_token_account(&mut svm, &usdc_mint, &payer.pubkey(), 0);

    // --- Step 4: withdraw_fees (200 USDC) ---
    let withdraw_amount: u64 = 200_000_000;
    let ix_withdraw = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::WithdrawFees {
            amount: withdraw_amount,
        }
        .data(),
        stayke_config::accounts::WithdrawFees {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            destination_token_account: destination,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    // We must derive the vault_pda properly so LiteSVM recognizes the signer.
    // Anchor stores the bump on-chain; LiteSVM needs the correct bump to find the PDA.
    let msg = Message::new_with_blockhash(&[ix_withdraw], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    // Pre-register the vault bump so the PDA's signer seeds resolve correctly.
    svm.set_account(
        platform_vault_pda,
        svm.get_account(&platform_vault_pda)
            .unwrap_or_else(|| Account {
                lamports: svm.minimum_balance_for_rent_exemption(0),
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            }),
    )
    .unwrap();

    let result = svm.send_transaction(tx);
    if let Err(ref e) = result {
        panic!("withdraw_fees failed: {e:?}");
    }
    result.unwrap();

    // --- Verify ---
    let vault_after = svm
        .get_account(&platform_vault)
        .expect("vault still exists");
    let vault_balance = u64::from_le_bytes(vault_after.data[64..72].try_into().unwrap());
    assert_eq!(
        vault_balance, 300_000_000,
        "Vault should have 300 USDC remaining"
    );

    let dest_after = svm
        .get_account(&destination)
        .expect("destination still exists");
    let dest_balance = u64::from_le_bytes(dest_after.data[64..72].try_into().unwrap());
    assert_eq!(
        dest_balance, 200_000_000,
        "Destination should have received 200 USDC"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

/// Attempting to withdraw 0 should fail with ZeroAmount.
#[test]
fn test_withdraw_fees_zero_amount_fails() {
    let program_id = stayke_config::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_config.so");
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
    let usdc_mint = mint_kp.pubkey();

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

    let blockhash = svm.latest_blockhash();

    // Initialize config
    let ix_init = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::InitializeConfig {
            minimum_deposit: MINIMUM_DEPOSIT,
            fee_bps: FEE_BPS,
            max_operations: MAX_OPERATIONS,
        }
        .data(),
        stayke_config::accounts::InitializeConfig {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    let msg = Message::new_with_blockhash(&[ix_init], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let destination = create_token_account(&mut svm, &usdc_mint, &payer.pubkey(), 0);

    // Ensure the vault PDA exists so seeds resolve
    svm.set_account(
        platform_vault_pda,
        svm.get_account(&platform_vault_pda)
            .unwrap_or_else(|| Account {
                lamports: svm.minimum_balance_for_rent_exemption(0),
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            }),
    )
    .unwrap();

    let ix_withdraw = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::WithdrawFees { amount: 0 }.data(),
        stayke_config::accounts::WithdrawFees {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            destination_token_account: destination,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let msg = Message::new_with_blockhash(&[ix_withdraw], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let result = svm.send_transaction(tx);

    assert!(result.is_err(), "Zero-amount withdrawal must fail");
}

/// Unauthorized signer must be rejected.
#[test]
fn test_withdraw_fees_unauthorized_fails() {
    let program_id = stayke_config::id();
    let payer = Keypair::new();
    let attacker = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_config.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

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
    let usdc_mint = mint_kp.pubkey();

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

    let blockhash = svm.latest_blockhash();

    // Initialize config with PAYER as authority
    let ix_init = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::InitializeConfig {
            minimum_deposit: MINIMUM_DEPOSIT,
            fee_bps: FEE_BPS,
            max_operations: MAX_OPERATIONS,
        }
        .data(),
        stayke_config::accounts::InitializeConfig {
            global_config,
            authority: payer.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    let msg = Message::new_with_blockhash(&[ix_init], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    // Fund vault
    let vault_amount: u64 = 500_000_000;
    {
        let mut vault_account = svm.get_account(&platform_vault).expect("vault exists");
        vault_account.data[64..72].copy_from_slice(&vault_amount.to_le_bytes());
        svm.set_account(platform_vault, vault_account).unwrap();
    }

    let destination = create_token_account(&mut svm, &usdc_mint, &attacker.pubkey(), 0);

    // Ensure the vault PDA exists so seeds resolve
    svm.set_account(
        platform_vault_pda,
        svm.get_account(&platform_vault_pda)
            .unwrap_or_else(|| Account {
                lamports: svm.minimum_balance_for_rent_exemption(0),
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            }),
    )
    .unwrap();

    // ATTACKER tries to withdraw
    let ix_withdraw = Instruction::new_with_bytes(
        program_id,
        &stayke_config::instruction::WithdrawFees {
            amount: 100_000_000,
        }
        .data(),
        stayke_config::accounts::WithdrawFees {
            global_config,
            authority: attacker.pubkey(), // unauthorized
            platform_vault_pda,
            platform_vault,
            destination_token_account: destination,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );

    let msg = Message::new_with_blockhash(&[ix_withdraw], Some(&attacker.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();
    let result = svm.send_transaction(tx);

    assert!(result.is_err(), "Unauthorized withdrawal must fail");
}
