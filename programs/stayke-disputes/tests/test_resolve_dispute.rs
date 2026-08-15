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

#[test]
fn resolve_dispute_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let valid_admin = Keypair::new();
    let rogue_admin = Keypair::new();
    let guest = Pubkey::new_unique();
    let host = Pubkey::new_unique();
    let property = Pubkey::new_unique();
    svm.airdrop(&rogue_admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, valid_admin.pubkey());
    let booking_key = Pubkey::new_unique();

    // Booking must exist so Anchor can deserialize it before constraint evaluation.
    setup_booking(
        &mut svm,
        booking_key,
        guest,
        host,
        property,
        BookingStatus::Active,
    );

    let usdc_mint = Pubkey::new_unique();
    let platform_vault = Pubkey::new_unique();

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

    let dispute_key = setup_dispute(
        &mut svm,
        booking_key,
        property,
        guest,
        host,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Open,
    );

    // Create all required SPL token accounts and mint so InterfaceAccount
    // deserialization succeeds before Anchor evaluates constraints.
    let escrow_ata = Pubkey::new_unique();
    let host_ata = Pubkey::new_unique();
    let guest_ata = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    make_token_account(&mut svm, escrow_ata, usdc_mint, Pubkey::new_unique());
    make_token_account(&mut svm, host_ata, usdc_mint, host);
    make_token_account(&mut svm, guest_ata, usdc_mint, guest);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique());

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::ResolveDispute {
            host_share_bps: 5000,
            rejected: false,
        }
        .data(),
        stayke_disputes::accounts::ResolveDispute {
            admin: rogue_admin.pubkey(),
            config: config_pda,
            dispute: dispute_key,
            booking: booking_key,
            global_config: global_cfg_pda,
            cpi_authority: cpi_auth,
            escrow_token_account: escrow_ata,
            host_token_account: host_ata,
            guest_token_account: guest_ata,
            platform_vault_token_account: platform_vault,
            usdc_mint,
            stayke_escrow_program: stayke_escrow::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
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
    let guest = Pubkey::new_unique();
    let host = Pubkey::new_unique();
    let property = Pubkey::new_unique();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();

    setup_dispute_config(&mut svm, admin.pubkey());
    let booking_key = Pubkey::new_unique();

    setup_booking(
        &mut svm,
        booking_key,
        guest,
        host,
        property,
        BookingStatus::Active,
    );

    let usdc_mint = Pubkey::new_unique();
    let platform_vault = Pubkey::new_unique();

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gcfg = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        minimum_deposit: 100_000,
        free_ops: 4,
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

    let dispute_key = setup_dispute(
        &mut svm,
        booking_key,
        property,
        guest,
        host,
        DisputeReason::PropertyNotAsDescribed,
        DisputeStatus::Resolved,
    );

    let escrow_ata = Pubkey::new_unique();
    let host_ata = Pubkey::new_unique();
    let guest_ata = Pubkey::new_unique();
    make_mint(&mut svm, usdc_mint);
    make_token_account(&mut svm, escrow_ata, usdc_mint, Pubkey::new_unique());
    make_token_account(&mut svm, host_ata, usdc_mint, host);
    make_token_account(&mut svm, guest_ata, usdc_mint, guest);
    make_token_account(&mut svm, platform_vault, usdc_mint, Pubkey::new_unique());

    let cpi_auth = cpi_authority_pda(&stayke_disputes::id());
    let config_pda = Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0;

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::ResolveDispute {
            host_share_bps: 5000,
            rejected: false,
        }
        .data(),
        stayke_disputes::accounts::ResolveDispute {
            admin: admin.pubkey(),
            config: config_pda,
            dispute: dispute_key,
            booking: booking_key,
            global_config: global_cfg_pda,
            cpi_authority: cpi_auth,
            escrow_token_account: escrow_ata,
            host_token_account: host_ata,
            guest_token_account: guest_ata,
            platform_vault_token_account: platform_vault,
            usdc_mint,
            stayke_escrow_program: stayke_escrow::id(),
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
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotOpen
            ))
        )
    );
}
