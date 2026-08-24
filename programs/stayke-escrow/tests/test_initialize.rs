mod common;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, AnchorDeserialize, InstructionData,
        ToAccountMetas,
    },
    common::*,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_config as config, stayke_escrow as escrow,
};

fn escrow_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[escrow::constants::ESCROW_CONFIG_SEED.as_bytes()],
        &escrow::id(),
    )
}

fn setup_global_config_with_custom_authority(
    svm: &mut litesvm::LiteSVM,
    authority: Pubkey,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &config::id(),
    );

    let gc = config::state::GlobalConfig {
        authority,
        free_ops: 4,
        minimum_deposit: 0,
        fee_bps: 200,
        usdc_mint: Pubkey::new_unique(),
        is_initialized: true,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: bump,
        core_program: stayke_core::id(),
        escrow_program: escrow::id(),
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

fn initialize_escrow_ix(authority: Pubkey, escrow_config: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        escrow::id(),
        &escrow::instruction::InitializeEscrow {}.data(),
        escrow::accounts::InitializeConfigEscrow {
            authority,
            escrow_config,
            global_config: global_config_pda(),
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    )
}

#[test]
fn test_initialize_escrow_success() {
    let (mut svm, _payer) = build_svm_with_escrow_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    setup_global_config_with_custom_authority(&mut svm, admin.pubkey());

    let (config_pda, expected_bump) = escrow_config_pda();

    let ix = initialize_escrow_ix(admin.pubkey(), config_pda);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&admin.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&admin]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transaction failed: {:?}", res.err());

    let acc = svm
        .get_account(&config_pda)
        .expect("EscrowConfig should exist");
    let escrow_config: escrow::state::EscrowConfig =
        AnchorDeserialize::deserialize(&mut &acc.data[8..]).unwrap();

    assert_eq!(escrow_config.authority, admin.pubkey());
    assert!(escrow_config.is_initialized);
    assert_eq!(escrow_config.bump, expected_bump);
}

#[test]
fn test_initialize_escrow_unauthorized_fails() {
    let (mut svm, _payer) = build_svm_with_escrow_programs();
    let admin = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    setup_global_config_with_custom_authority(&mut svm, admin.pubkey());

    let (config_pda, _) = escrow_config_pda();

    let ix = initialize_escrow_ix(attacker.pubkey(), config_pda);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&attacker.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::UnauthorizedAdmin
            ))
        )
    );
}
