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
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_escrow::state::BookingStatus,
};

// ---------------------------------------------------------------------------
// Timestamp constants
// ---------------------------------------------------------------------------

const CHECK_IN: i64 = 1735689600; // 2025-01-01 00:00:00 UTC
const CHECK_OUT: i64 = 1736035200; // 2025-01-05 00:00:00 UTC

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds the permissionless `booking_starts` instruction. It reads only the
/// booking account, so guest/host/property are plain seed material and the
/// payer can be any signer.
fn booking_starts_ix(payer: &solana_keypair::Keypair, booking: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::BookingStarts {}.data(),
        stayke_escrow::accounts::BookingStarts {
            payer: payer.pubkey(),
            booking,
        }
        .to_account_metas(None),
    )
}

fn send_booking_starts(
    svm: &mut litesvm::LiteSVM,
    payer: &solana_keypair::Keypair,
    booking: Pubkey,
) -> Result<(), TransactionError> {
    let instruction = booking_starts_ix(payer, booking);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

// ===========================================================================
// Happy path: HostAccepted -> Active when now >= check_in
// ===========================================================================

#[test]
fn booking_starts_transitions_host_accepted_to_active() {
    let (mut svm, payer) = build_svm_with_escrow_programs();

    let booking = setup_booking_at_pda(
        &mut svm,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::HostAccepted,
    );

    // Clock exactly at check-in — the boundary must succeed.
    set_clock(&mut svm, CHECK_IN);

    send_booking_starts(&mut svm, &payer, booking).expect("booking_starts should succeed");

    let data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    assert_eq!(data.status, BookingStatus::Active);
    assert_eq!(data.updated_at, CHECK_IN);
}

// ===========================================================================
// Error: wrong booking status
// ===========================================================================

#[test]
fn booking_starts_wrong_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();

    // Pending (not HostAccepted) must be rejected.
    let booking = setup_booking_at_pda(
        &mut svm,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::Pending,
    );

    set_clock(&mut svm, CHECK_IN);

    let err = send_booking_starts(&mut svm, &payer, booking).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::BookingNotAccepted
            ))
        )
    );
}

// ===========================================================================
// Error: check-in time not reached
// ===========================================================================

#[test]
fn booking_starts_too_early_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();

    let booking = setup_booking_at_pda(
        &mut svm,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        CHECK_IN,
        CHECK_OUT,
        BookingStatus::HostAccepted,
    );

    // One second before check-in.
    set_clock(&mut svm, CHECK_IN - 1);

    let err = send_booking_starts(&mut svm, &payer, booking).unwrap_err();
    assert_eq!(
        err,
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_escrow::error::EscrowError::TooEarlyToActivate
            ))
        )
    );
}
