//! LiteSVM: create_booking_cross_year is on the ABI and inits the next-year PDA.

mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, system_program},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::*,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stayke_escrow::state::BookingDays,
};

/// 2026-12-28 00:00 UTC
const CHECK_IN: i64 = 1_798_416_000;
/// 2027-01-05 00:00 UTC
const CHECK_OUT: i64 = 1_799_107_200;

#[test]
fn create_booking_cross_year_inits_next_year_from_checkout() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();

    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let listing = setup_listing(&mut svm, host_profile, 0, 100_000, true);

    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::CreateBookingCrossYear {
            check_in: CHECK_IN,
            check_out: CHECK_OUT,
        }
        .data(),
        stayke_escrow::accounts::CreateBookingCrossYear {
            payer: payer.pubkey(),
            client: guest.pubkey(),
            client_profile: guest_profile,
            host_profile,
            booking: booking_pda(listing, guest_profile, CHECK_IN),
            property: listing,
            global_config: global_config_pda(),
            system_program: system_program::id(),
            booking_days: booking_days_pda(listing, 2026),
            booking_days_next: booking_days_pda(listing, 2027),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &guest]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "create_booking_cross_year should succeed, got: {:?}",
        res.err()
    );

    let next_acc = svm
        .get_account(&booking_days_pda(listing, 2027))
        .expect("booking_days_next exists");
    let next = BookingDays::try_deserialize(&mut next_acc.data.as_slice())
        .expect("deserialize booking_days_next");
    assert_eq!(next.year, 2027);
}
