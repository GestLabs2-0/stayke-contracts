use {
    anchor_lang::{
        error::ErrorCode,
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
};

#[test]
fn test_initialize_err_global_acc() {
    let program_id = stayke_escrow::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stayke_escrow.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &stayke_escrow::instruction::InitializeEscrow {}.data(),
        stayke_escrow::accounts::InitializeConfigEscrow {
            authority: payer.pubkey(),
            escrow_config: Pubkey::find_program_address(
                &[stayke_escrow::constants::ESCROW_CONFIG_SEED.as_bytes()],
                &program_id,
            )
            .0,
            global_config: Pubkey::find_program_address(
                &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
                &stayke_config::id(),
            )
            .0,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(0, Custom(ErrorCode::AccountNotInitialized.into()))
    );
}
