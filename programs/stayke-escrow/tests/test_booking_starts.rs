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

/// Common account setup shared by every test. Holds the PDAs the instruction
/// needs: guest/host profiles, global config, and the escrow CPI authority PDA.
struct BookingStartsEnv {
    guest_profile: Pubkey,
    host_profile: Pubkey,
    global_config: Pubkey,
    cpi_authority: Pubkey,
    property: Pubkey,
}

fn setup_env(svm: &mut litesvm::LiteSVM) -> BookingStartsEnv {
    let guest = solana_keypair::Keypair::new();
    let host = solana_keypair::Keypair::new();

    let guest_profile = setup_user_profile(svm, guest.pubkey());
    let host_profile = setup_user_profile(svm, host.pubkey());
    // Registers `stayke_escrow` as the escrow program in the global config so
    // `assert_cpi_authority(..., [AllowedCaller::Escrow])` resolves our PDA.
    let global_config = setup_global_config(svm, stayke_escrow::id());

    // The escrow program signs core CPIs via its own `cpi_authority` PDA.
    let cpi_authority = Pubkey::find_program_address(
        &[stayke_escrow::constants::CPI_AUTHORITY_SEED.as_bytes()],
        &stayke_escrow::id(),
    )
    .0;

    BookingStartsEnv {
        guest_profile,
        host_profile,
        global_config,
        cpi_authority,
        property: Pubkey::new_unique(),
    }
}

fn booking_starts_ix(
    payer: &solana_keypair::Keypair,
    booking: Pubkey,
    env: &BookingStartsEnv,
) -> Instruction {
    Instruction::new_with_bytes(
        stayke_escrow::id(),
        &stayke_escrow::instruction::BookingStarts {}.data(),
        stayke_escrow::accounts::BookingStarts {
            payer: payer.pubkey(),
            booking,
            guest: env.guest_profile,
            host_profile: env.host_profile,
            cpi_authority: env.cpi_authority,
            global_config: env.global_config,
            stayke_core: stayke_core::id(),
        }
        .to_account_metas(None),
    )
}

fn send_booking_starts(
    svm: &mut litesvm::LiteSVM,
    payer: &solana_keypair::Keypair,
    booking: Pubkey,
    env: &BookingStartsEnv,
) -> Result<(), TransactionError> {
    let instruction = booking_starts_ix(payer, booking, env);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

fn setup_booking(
    svm: &mut litesvm::LiteSVM,
    env: &BookingStartsEnv,
    status: BookingStatus,
) -> Pubkey {
    setup_booking_at_pda(
        svm,
        env.guest_profile,
        env.host_profile,
        env.property,
        CHECK_IN,
        CHECK_OUT,
        status,
    )
}

// ===========================================================================
// Happy path: HostAccepted -> Active when now >= check_in
// ===========================================================================

#[test]
fn booking_starts_transitions_host_accepted_to_active() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let env = setup_env(&mut svm);
    let booking = setup_booking(&mut svm, &env, BookingStatus::HostAccepted);

    // Clock exactly at check-in — the boundary must succeed.
    set_clock(&mut svm, CHECK_IN);

    send_booking_starts(&mut svm, &payer, booking, &env).expect("booking_starts should succeed");

    let data: stayke_escrow::state::Booking =
        AnchorDeserialize::deserialize(&mut &svm.get_account(&booking).unwrap().data[8..]).unwrap();
    assert_eq!(data.status, BookingStatus::Active);
    assert_eq!(data.updated_at, CHECK_IN);

    // The CPI must have linked the booking into the guest's active-booking slot.
    let guest: stayke_core::state::UserProfile = AnchorDeserialize::deserialize(
        &mut &svm.get_account(&env.guest_profile).unwrap().data[8..],
    )
    .unwrap();
    assert_eq!(guest.active_booking, Some(booking));
}

// ===========================================================================
// Error: wrong booking status
// ===========================================================================

#[test]
fn booking_starts_wrong_status_fails() {
    let (mut svm, payer) = build_svm_with_escrow_programs();
    let env = setup_env(&mut svm);
    let booking = setup_booking(&mut svm, &env, BookingStatus::Pending);

    set_clock(&mut svm, CHECK_IN);

    let err = send_booking_starts(&mut svm, &payer, booking, &env).unwrap_err();
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
    let env = setup_env(&mut svm);
    let booking = setup_booking(&mut svm, &env, BookingStatus::HostAccepted);

    // One second before check-in.
    set_clock(&mut svm, CHECK_IN - 1);

    let err = send_booking_starts(&mut svm, &payer, booking, &env).unwrap_err();
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
