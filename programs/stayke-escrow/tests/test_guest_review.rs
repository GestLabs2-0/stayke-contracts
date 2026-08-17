//! LiteSVM integration tests for `guest_review`.
//!
//! The guest rates the host once the booking reaches `Completed`, `Released`,
//! `DisputeResolved` or `DisputeRejected`. It writes `booking.guest_review` and
//! CPIs into `stayke-core::update_host_review` to bump the host's
//! `host_reviews` / `total_score_host`.

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
    stayke_escrow::state::BookingStatus,
};

const CHECK_IN: i64 = 1735689600;
const CHECK_OUT: i64 = 1736035200;
const SCORE: u8 = 4;

#[allow(clippy::too_many_arguments)]
fn guest_review_accounts(
    guest: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    host_reputation: Pubkey,
    booking: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::GuestReview {
        guest,
        guest_profile,
        host_profile,
        host_reputation,
        booking,
        global_config: global_config_pda(),
        cpi_authority: cpi_authority_pda(),
        stayke_core_program: stayke_core::id(),
    }
    .to_account_metas(None)
}

#[allow(clippy::too_many_arguments)]
fn send_guest_review(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    guest: &Keypair,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    host_reputation: Pubkey,
    booking: Pubkey,
    score: u8,
) -> Result<(), TransactionError> {
    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::GuestReview { score }.data(),
        guest_review_accounts(
            guest.pubkey(),
            guest_profile,
            host_profile,
            host_reputation,
            booking,
        ),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, guest]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

fn set_guest_review(svm: &mut litesvm::LiteSVM, booking: Pubkey, value: u8) {
    let mut data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    data.guest_review = value;
    svm.set_account(
        booking,
        Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &data),
            owner: stayke_escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
}

// ===========================================================================
// Happy path
// ===========================================================================

#[test]
fn guest_review_sets_booking_and_updates_host_reputation() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
    );

    send_guest_review(
        &mut svm,
        &payer,
        &guest,
        guest_profile,
        host_profile,
        host_reputation,
        booking,
        SCORE,
    )
    .expect("guest_review should succeed");

    let data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    assert_eq!(data.guest_review, SCORE);

    let rep: stayke_core::state::ReputationProfile =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&host_reputation).unwrap().data[8..])
            .unwrap();
    assert_eq!(rep.host_reviews, 1);
    assert_eq!(rep.total_score_host, SCORE as u64);
}

// ===========================================================================
// Allowed post-settlement statuses
// ===========================================================================

#[test]
fn guest_review_accepts_released_and_dispute_resolution_statuses() {
    for status in [
        BookingStatus::Released,
        BookingStatus::DisputeResolved,
        BookingStatus::DisputeRejected,
    ] {
        let (mut svm, payer) = build_svm_with_escrow_programs();
        let host = Keypair::new();
        let guest = Keypair::new();
        let property = Pubkey::new_unique();

        let host_profile = setup_user_profile(&mut svm, host.pubkey());
        let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
        let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
        setup_global_config(&mut svm, stayke_escrow::id());

        let booking = setup_booking_at_pda(
            &mut svm,
            guest_profile,
            host_profile,
            property,
            CHECK_IN,
            CHECK_OUT,
            status,
        );

        send_guest_review(
            &mut svm,
            &payer,
            &guest,
            guest_profile,
            host_profile,
            host_reputation,
            booking,
            SCORE,
        )
        .expect("guest_review should succeed");

        let data: stayke_escrow::state::Booking =
            AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..])
                .unwrap();
        assert_eq!(data.guest_review, SCORE);
    }
}

// ===========================================================================
// Error: wrong caller
// ===========================================================================

#[test]
fn guest_review_wrong_caller_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
    );

    // The host signs as `guest`; its own profile is passed as guest_profile,
    // so guest_profile.key() != booking.guest.
    let err = send_guest_review(
        &mut svm,
        &payer,
        &host,
        host_profile,
        host_profile,
        host_reputation,
        booking,
        SCORE,
    )
    .unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::UnauthorizedBooking
            ))
        )
    );
}

// ===========================================================================
// Error: invalid booking status
// ===========================================================================

#[test]
fn guest_review_invalid_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Active,
    );

    let err = send_guest_review(
        &mut svm,
        &payer,
        &guest,
        guest_profile,
        host_profile,
        host_reputation,
        booking,
        SCORE,
    )
    .unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingStatus
            ))
        )
    );
}

#[test]
fn guest_review_disputed_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Disputed,
    );

    let err = send_guest_review(
        &mut svm,
        &payer,
        &guest,
        guest_profile,
        host_profile,
        host_reputation,
        booking,
        SCORE,
    )
    .unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidBookingStatus
            ))
        )
    );
}

// ===========================================================================
// Error: invalid score
// ===========================================================================

#[test]
fn guest_review_invalid_score_fails() {
    for score in [0u8, 6u8] {
        let (mut svm, payer) = build_svm_with_escrow_programs();
        let host = Keypair::new();
        let guest = Keypair::new();
        let property = Pubkey::new_unique();

        let host_profile = setup_user_profile(&mut svm, host.pubkey());
        let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
        let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
        setup_global_config(&mut svm, stayke_escrow::id());

        let booking = setup_booking_at_pda(
            &mut svm,
            guest_profile,
            host_profile,
            property,
            CHECK_IN,
            CHECK_OUT,
            BookingStatus::Completed,
        );

        let err = send_guest_review(
            &mut svm,
            &payer,
            &guest,
            guest_profile,
            host_profile,
            host_reputation,
            booking,
            score,
        )
        .unwrap_err();
        assert_eq!(
            err,
            TransactionError::InstructionError(
                0,
                Custom(u32::from(stayke_escrow::error::EscrowError::InvalidScore))
            )
        );
    }
}

// ===========================================================================
// Error: review already submitted
// ===========================================================================

#[test]
fn guest_review_already_reviewed_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let host_reputation = setup_reputation_profile(&mut svm, host.pubkey());
    setup_global_config(&mut svm, stayke_escrow::id());

    let booking = setup_booking_at_pda(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Completed,
    );
    set_guest_review(&mut svm, booking, 5);

    let err = send_guest_review(
        &mut svm,
        &payer,
        &guest,
        guest_profile,
        host_profile,
        host_reputation,
        booking,
        SCORE,
    )
    .unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::ReviewAlreadySubmitted
            ))
        )
    );
}
