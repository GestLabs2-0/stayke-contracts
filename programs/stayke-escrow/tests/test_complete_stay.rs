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
    stayke_escrow::state::BookingStatus,
};

const CHECK_IN: i64 = 1_700_000_000;

fn setup_complete_stay_world(
    svm: &mut litesvm::LiteSVM,
    guest: &Keypair,
    host: &Keypair,
) -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
    let guest_profile = setup_user_profile(svm, guest.pubkey());
    let host_profile = setup_user_profile(svm, host.pubkey());
    let listing = setup_listing(svm, host_profile, 0, 100_000, true);

    let usdc_mint = Pubkey::new_unique();
    let platform_vault = Pubkey::new_unique();
    make_mint(svm, usdc_mint);
    make_token_account(svm, platform_vault, usdc_mint, Pubkey::new_unique());

    let (global_cfg_pda, global_bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gc = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops: 4,
        minimum_deposit: 0,
        fee_bps: 200,
        usdc_mint,
        is_initialized: true,
        platform_vault,
        platform_vault_bump: global_bump,
        core_program: stayke_core::id(),
        escrow_program: stayke_escrow::id(),
        disputes_program: Pubkey::new_unique(),
        treasury_program: Pubkey::new_unique(),
        bump: global_bump,
    };
    svm.set_account(
        global_cfg_pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gc),
            owner: stayke_config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    setup_escrow_config(svm, guest.pubkey());

    let booking = setup_booking_at_pda(
        svm,
        guest_profile,
        host_profile,
        listing,
        CHECK_IN,
        CHECK_IN + 86_400,
        BookingStatus::ReviewCompleted,
    );
    let (escrow_ata, escrow_bump) = escrow_token_pda(booking);
    make_token_account(svm, escrow_ata, usdc_mint, booking);

    let (_, booking_bump) = Pubkey::find_program_address(
        &[
            stayke_escrow::constants::BOOKING_SEED.as_bytes(),
            listing.as_ref(),
            guest_profile.as_ref(),
            CHECK_IN.to_le_bytes().as_ref(),
        ],
        &stayke_escrow::id(),
    );
    let booking_acc = stayke_escrow::state::Booking {
        guest: guest_profile,
        host: host_profile,
        property: listing,
        is_deposit: true,
        check_in: CHECK_IN,
        check_out: CHECK_IN + 86_400,
        total_price: 100_000,
        status: BookingStatus::ReviewCompleted,
        escrow_bump,
        bump: booking_bump,
    };
    svm.set_account(
        booking,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &booking_acc),
            owner: stayke_escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    (
        guest_profile,
        host_profile,
        booking,
        global_cfg_pda,
        escrow_ata,
        usdc_mint,
        platform_vault,
    )
}

#[test]
fn complete_stay_foreign_host_token_account_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let guest = Keypair::new();
    let host = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let (guest_profile, host_profile, booking, global_cfg, escrow_ata, usdc_mint, platform_vault) =
        setup_complete_stay_world(&mut svm, &guest, &host);

    let foreign_ata = Pubkey::new_unique();
    make_token_account(&mut svm, foreign_ata, usdc_mint, Pubkey::new_unique());

    let instruction = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CompleteStay {}.data(),
        stayke_escrow::accounts::CompleteStay {
            payer: payer.pubkey(),
            client: guest.pubkey(),
            client_profile: guest_profile,
            host_profile,
            booking,
            global_config: global_cfg,
            escrow_config: Pubkey::find_program_address(
                &[stayke_escrow::constants::ESCROW_CONFIG_SEED.as_bytes()],
                &stayke_escrow::id(),
            )
            .0,
            escrow_token_account: escrow_ata,
            host_token_account: foreign_ata,
            platform_vault,
            mint: usdc_mint,
            cpi_authority: cpi_authority_pda(&stayke_escrow::id()),
            stayke_core_program: stayke_core::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidPayoutTokenAccount
            ))
        )
    );
}
