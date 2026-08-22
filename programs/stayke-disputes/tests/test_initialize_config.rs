mod common;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, AnchorDeserialize, InstructionData,
        ToAccountMetas,
    },
    common::*,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stayke_disputes::{constants::DISPUTE_CONFIG_PDA_SEED, state::DisputeConfig},
};

fn initialize_config_ix(payer: Pubkey, config: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::InitializeConfig {}.data(),
        stayke_disputes::accounts::InitializeConfig {
            authority: payer,
            config,
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    )
}

fn config_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0
}

#[test]
fn initialize_config_creates_pda_with_authority_as_admin() {
    let (mut svm, payer) = build_svm_with_programs();
    let config = config_pda();

    let instruction = initialize_config_ix(payer.pubkey(), config);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Expected successful initialization, got: {:?}",
        res.err()
    );

    let account = svm.get_account(&config).unwrap();
    assert_eq!(account.owner, stayke_disputes::id());
    assert!(account.lamports > 0);

    let disc = discriminator("DisputeConfig");
    assert_eq!(&account.data[..8], &disc, "Wrong discriminator");

    let config: DisputeConfig = AnchorDeserialize::deserialize(&mut &account.data[8..]).unwrap();
    assert!(config.is_initialized);
    assert_eq!(config.admins.len(), 1);
    assert_eq!(config.admins[0], payer.pubkey());
    assert_eq!(config.retribution_bps_low, 1000);
    assert_eq!(config.retribution_bps_medium, 3000);
    assert_eq!(config.retribution_bps_high, 10000);
}

#[test]
fn initialize_config_twice_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let config = config_pda();

    let instruction = initialize_config_ix(payer.pubkey(), config);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "First initialization should succeed, got: {:?}",
        res.err()
    );

    let instruction = initialize_config_ix(payer.pubkey(), config);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Second initialization should fail");
}
