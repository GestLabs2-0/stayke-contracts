//! LiteSVM integration tests for `host_review`.
//!
//! The host rates the guest once the booking reaches `Completed`, `Released`,
//! `DisputeResolved` or `DisputeRejected`. It writes `booking.host_review` and
//! CPIs into `stayke-core::update_client_review` to bump the guest's
//! `client_reviews` / `total_score_client`.

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
fn host_review_accounts(
    host: Pubkey,
    host_profile: Pubkey,
    guest_profile: Pubkey,
    guest_reputation: Pubkey,
    booking: Pubkey,
) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
    stayke_escrow::accounts::HostReview {
        host,
        host_profile,
        guest_profile,
        guest_reputation,
        booking,
        global_config: global_config_pda(),
        cpi_authority: cpi_authority_pda(),
        stayke_core_program: stayke_core::id(),
    }
    .to_account_metas(None)
}

#[allow(clippy::too_many_arguments)]
fn send_host_review(
    svm: &mut litesvm::LiteSVM,
    payer: &Keypair,
    host: &Keypair,
    host_profile: Pubkey,
    guest_profile: Pubkey,
    guest_reputation: Pubkey,
    booking: Pubkey,
    score: u8,
) -> Result<(), TransactionError> {
    let instruction = Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::HostReview { score }.data(),
        host_review_accounts(
            host.pubkey(),
            host_profile,
            guest_profile,
            guest_reputation,
            booking,
        ),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, host]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

fn set_host_review(svm: &mut litesvm::LiteSVM, booking: Pubkey, value: u8) {
    let mut data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    data.host_review = value;
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
fn host_review_sets_booking_and_updates_guest_reputation() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

    send_host_review(
        &mut svm,
        &payer,
        &host,
        host_profile,
        guest_profile,
        guest_reputation,
        booking,
        SCORE,
    )
    .expect("host_review should succeed");

    let data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    assert_eq!(data.host_review, SCORE);

    let rep: stayke_core::state::ReputationProfile =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&guest_reputation).unwrap().data[8..])
            .unwrap();
    assert_eq!(rep.client_reviews, 1);
    assert_eq!(rep.total_score_client, SCORE as u64);
}

// ===========================================================================
// Allowed post-settlement statuses
// ===========================================================================

#[test]
fn host_review_accepts_released_and_dispute_resolution_statuses() {
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
        let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

        send_host_review(
            &mut svm,
            &payer,
            &host,
            host_profile,
            guest_profile,
            guest_reputation,
            booking,
            SCORE,
        )
        .expect("host_review should succeed");

        let data: stayke_escrow::state::Booking =
            AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..])
                .unwrap();
        assert_eq!(data.host_review, SCORE);
    }
}

// ===========================================================================
// Error: wrong caller
// ===========================================================================

#[test]
fn host_review_wrong_caller_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

    // The guest signs as `host`; its own profile is passed as host_profile,
    // so host_profile.key() != booking.host.
    let err = send_host_review(
        &mut svm,
        &payer,
        &guest,
        guest_profile,
        guest_profile,
        guest_reputation,
        booking,
        SCORE,
    )
    .unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::InvalidHostBooking
            ))
        )
    );
}

// ===========================================================================
// Error: invalid booking status
// ===========================================================================

#[test]
fn host_review_invalid_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

    let err = send_host_review(
        &mut svm,
        &payer,
        &host,
        host_profile,
        guest_profile,
        guest_reputation,
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
fn host_review_disputed_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

    let err = send_host_review(
        &mut svm,
        &payer,
        &host,
        host_profile,
        guest_profile,
        guest_reputation,
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
fn host_review_invalid_score_fails() {
    for score in [0u8, 6u8] {
        let (mut svm, payer) = build_svm_with_escrow_programs();
        let host = Keypair::new();
        let guest = Keypair::new();
        let property = Pubkey::new_unique();

        let host_profile = setup_user_profile(&mut svm, host.pubkey());
        let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
        let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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

        let err = send_host_review(
            &mut svm,
            &payer,
            &host,
            host_profile,
            guest_profile,
            guest_reputation,
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
fn host_review_already_reviewed_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let host = Keypair::new();
    let guest = Keypair::new();
    let property = Pubkey::new_unique();

    let host_profile = setup_user_profile(&mut svm, host.pubkey());
    let guest_profile = setup_user_profile(&mut svm, guest.pubkey());
    let guest_reputation = setup_reputation_profile(&mut svm, guest.pubkey());
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
    set_host_review(&mut svm, booking, 5);

    let err = send_host_review(
        &mut svm,
        &payer,
        &host,
        host_profile,
        guest_profile,
        guest_reputation,
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
