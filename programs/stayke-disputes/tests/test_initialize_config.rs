mod common;

use {
    anchor_lang::{solana_program::instruction::Instruction, AnchorDeserialize, InstructionData, ToAccountMetas},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stayke_disputes::{constants::DISPUTE_CONFIG_PDA_SEED, state::DisputeConfig},
};

#[test]
fn initialize_config_creates_pda_with_authority_as_admin() {
    let program_id = stayke_disputes::id();
    let payer = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_disputes.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let config_pda = solana_pubkey::Pubkey::find_program_address(
        &[DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &program_id,
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer.pubkey(),
            config: config_pda,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful initialization, got: {:?}",
        res.err()
    );

    let account = svm.get_account(&config_pda).unwrap();
    assert_eq!(account.owner, program_id);
    assert!(account.lamports > 0);

    // Verify discriminator and fields.
    let disc = common::discriminator("DisputeConfig");
    assert_eq!(&account.data[..8], &disc, "Wrong discriminator");
    let config: DisputeConfig =
        AnchorDeserialize::deserialize(&mut &account.data[8..]).unwrap();

    assert!(config.is_initialized);
    assert_eq!(config.admins.len(), 1);
    assert_eq!(config.admins[0], payer.pubkey());
    assert_eq!(config.retribution_bps_low, 1000);
    assert_eq!(config.retribution_bps_medium, 3000);
    assert_eq!(config.retribution_bps_high, 10000);
}

#[test]
fn initialize_config_twice_fails() {
    let program_id = stayke_disputes::id();
    let payer = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_disputes.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let config_pda = solana_pubkey::Pubkey::find_program_address(
        &[DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &program_id,
    )
    .0;

    // First init — succeeds.
    let ix1 = Instruction::new_with_bytes(
        program_id,
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer.pubkey(),
            config: config_pda,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok());

    // Second init should fail — PDA already exists.
    let ix2 = Instruction::new_with_bytes(
        program_id,
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer.pubkey(),
            config: config_pda,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    assert!(svm.send_transaction(tx2).is_err());
}

#[test]
fn initialize_config_different_payer_reuses_pda_fails() {
    let program_id = stayke_disputes::id();
    let payer1 = Keypair::new();
    let payer2 = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_disputes.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer1.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&payer2.pubkey(), 1_000_000_000).unwrap();

    let config_pda = solana_pubkey::Pubkey::find_program_address(
        &[DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &program_id,
    )
    .0;

    // Payer1 initializes.
    let ix1 = Instruction::new_with_bytes(
        program_id,
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer1.pubkey(),
            config: config_pda,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer1.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer1]).unwrap();
    assert!(svm.send_transaction(tx).is_ok());

    // Payer2 tries to init the same PDA — should fail.
    let ix2 = Instruction::new_with_bytes(
        program_id,
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer2.pubkey(),
            config: config_pda,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer2.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer2]).unwrap();
    assert!(svm.send_transaction(tx2).is_err());
}
