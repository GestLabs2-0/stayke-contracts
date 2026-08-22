mod common;

use {
    anchor_lang::{
        prelude::Clock, solana_program::instruction::Instruction, AnchorDeserialize,
        InstructionData, ToAccountMetas,
    },
    common::*,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::{
        versioned::VersionedTransaction, InstructionError::Custom, TransactionError,
    },
    stayke_disputes::state::{DisputeAccount, DisputeParty, DisputeState},
    stayke_escrow::state::BookingStatus,
};

const CHECK_IN: i64 = 1_735_689_600;

fn user_profile_pda(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &stayke_core::id(),
    )
    .0
}

fn global_config_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0
}

fn close_dispute_ix(
    dispute: Pubkey,
    booking: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    opener_wallet: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::CloseDispute {}.data(),
        stayke_disputes::accounts::CloseDispute {
            dispute,
            booking,
            guest_profile,
            host_profile,
            opener_wallet,
            global_config: global_config_pda(),
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            stayke_core_program: stayke_core::id(),
            stayke_escrow_program: stayke_escrow::id(),
        }
        .to_account_metas(None),
    )
}

fn send_close(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instruction: Instruction,
) -> Result<(), TransactionError> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).map(|_| ()).map_err(|e| e.err)
}

/// Writes a `DisputeAccount` directly (used to reach the admin-resolved states
/// that require `resolve_dispute`, which is not wired yet).
fn set_dispute_account(
    svm: &mut LiteSVM,
    booking: Pubkey,
    opened_by: DisputeParty,
    state: DisputeState,
    original_booking_status: BookingStatus,
) -> Pubkey {
    let (dispute_key, bump) = Pubkey::find_program_address(
        &[
            stayke_disputes::constants::DISPUTE_PDA_SEED.as_bytes(),
            booking.as_ref(),
        ],
        &stayke_disputes::id(),
    );

    let dispute = DisputeAccount {
        booking,
        opened_by,
        state,
        opened_at: CHECK_IN,
        guest_evidence: None,
        host_evidence: None,
        outcome: None,
        original_booking_status,
        bump,
    };

    svm.set_account(
        dispute_key,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("DisputeAccount", &dispute),
            owner: stayke_disputes::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();

    dispute_key
}

fn read_booking(svm: &LiteSVM, booking: Pubkey) -> stayke_escrow::state::Booking {
    let account = svm
        .get_account(&booking)
        .expect("booking account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize Booking")
}

fn read_profile(svm: &LiteSVM, profile: Pubkey) -> stayke_core::state::UserProfile {
    let account = svm
        .get_account(&profile)
        .expect("profile account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize UserProfile")
}

/// Opens a dispute with the guest as initiator on an `Active` booking.
/// Returns (booking, guest_profile, host_profile).
fn setup_open_dispute(
    svm: &mut LiteSVM,
    payer: &Keypair,
    guest: &Keypair,
) -> (Pubkey, Pubkey, Pubkey) {
    let host = Keypair::new();
    let property = Pubkey::new_unique();
    let guest_profile_pda = user_profile_pda(guest.pubkey());
    let host_profile_pda = user_profile_pda(host.pubkey());
    let booking_key = booking_pda(property, guest_profile_pda, CHECK_IN);

    setup_global_config(svm, stayke_disputes::id(), stayke_escrow::id());
    setup_user_profile(svm, guest.pubkey());
    setup_user_profile(svm, host.pubkey());
    setup_booking(
        svm,
        guest_profile_pda,
        host_profile_pda,
        property,
        CHECK_IN,
        BookingStatus::Active,
    );

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN;
    svm.set_sysvar::<Clock>(&clock);

    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {}.data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile_pda,
            booking: booking_key,
            dispute: dispute_pda(booking_key),
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            global_config: global_config_pda(),
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, guest]).unwrap();
    svm.send_transaction(tx).expect("open dispute must succeed");

    (booking_key, guest_profile_pda, host_profile_pda)
}

fn send_solve(svm: &mut LiteSVM, payer: &Keypair, initiator: &Keypair, booking: Pubkey) {
    let instruction = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::SolveDisputeBeforeAdmin {}.data(),
        stayke_disputes::accounts::SolveDisputeBeforeAdmin {
            initiator: initiator.pubkey(),
            initiator_profile: user_profile_pda(initiator.pubkey()),
            dispute: dispute_pda(booking),
            booking,
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            global_config: global_config_pda(),
            stayke_escrow_program: stayke_escrow::id(),
        }
        .to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, initiator]).unwrap();
    svm.send_transaction(tx)
        .expect("solve dispute must succeed");
}

// ---------------------------------------------------------------------------
// P2P route
// ---------------------------------------------------------------------------

/// A P2P-resolved dispute closes without touching the booking or stay counters:
/// `solve_dispute_before_admin` restored the booking, and the normal escrow flow
/// will count the stays.
#[test]
fn close_p2p_resolved_succeeds_and_preserves_booking() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let (booking, guest_profile, host_profile) = setup_open_dispute(&mut svm, &payer, &guest);
    send_solve(&mut svm, &payer, &guest, booking);

    // state is ResolvedByP2P with the booking restored to Active.
    let dispute = dispute_pda(booking);
    let balance_before = svm
        .get_account(&guest.pubkey())
        .map(|a| a.lamports)
        .unwrap();

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(
            dispute,
            booking,
            guest_profile,
            host_profile,
            guest.pubkey(),
        ),
    );
    assert!(
        res.is_ok(),
        "P2P close should succeed, got: {:?}",
        res.err()
    );

    assert!(
        svm.get_account(&dispute).is_none(),
        "Dispute account should be closed"
    );
    assert_eq!(read_booking(&svm, booking).status, BookingStatus::Active);
    assert_eq!(read_profile(&svm, guest_profile).completed_stays, 0);
    let balance_after = svm
        .get_account(&guest.pubkey())
        .expect("opener wallet must exist")
        .lamports;
    assert!(
        balance_after > balance_before,
        "opener wallet should receive the account rent"
    );
}

// ---------------------------------------------------------------------------
// Admin route
// ---------------------------------------------------------------------------

/// Admin-resolved with escrow distributed (booking already DisputeResolved):
/// `release_funds` never runs, so close_dispute settles the stay counters and
/// leaves the booking resolved.
#[test]
fn close_admin_resolved_counts_stays() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest_wallet = Pubkey::new_unique();
    let host_wallet = Pubkey::new_unique();
    svm.airdrop(&guest_wallet, 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest_wallet);
    let host_profile = setup_user_profile(&mut svm, host_wallet);
    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());

    let property = Pubkey::new_unique();
    let booking = setup_booking(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        BookingStatus::DisputeResolved,
    );
    let dispute = set_dispute_account(
        &mut svm,
        booking,
        DisputeParty::Guest,
        DisputeState::ResolvedByAdmin,
        BookingStatus::Active,
    );

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(dispute, booking, guest_profile, host_profile, guest_wallet),
    );
    assert!(
        res.is_ok(),
        "admin close should succeed, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&dispute).is_none());
    assert_eq!(
        read_booking(&svm, booking).status,
        BookingStatus::DisputeResolved
    );
    assert_eq!(read_profile(&svm, guest_profile).completed_stays, 1);
    assert_eq!(read_profile(&svm, host_profile).hosted_stays, 1);
}

/// Admin-resolved with NoFaultFound: the escrow was not distributed, so the
/// booking is restored to its original status for the normal flow to release
/// funds, and the stays are not double-counted.
#[test]
fn close_admin_no_fault_restores_booking() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest_wallet = Pubkey::new_unique();
    let host_wallet = Pubkey::new_unique();
    svm.airdrop(&guest_wallet, 1_000_000_000).unwrap();
    svm.airdrop(&host_wallet, 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest_wallet);
    let host_profile = setup_user_profile(&mut svm, host_wallet);
    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());

    let property = Pubkey::new_unique();
    let booking = setup_booking(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        BookingStatus::Disputed,
    );
    let dispute = set_dispute_account(
        &mut svm,
        booking,
        DisputeParty::Host,
        DisputeState::ResolvedByAdmin,
        BookingStatus::Active,
    );

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(dispute, booking, guest_profile, host_profile, host_wallet),
    );
    assert!(
        res.is_ok(),
        "admin close should succeed, got: {:?}",
        res.err()
    );

    assert!(svm.get_account(&dispute).is_none());
    assert_eq!(read_booking(&svm, booking).status, BookingStatus::Active);
    assert_eq!(read_profile(&svm, guest_profile).completed_stays, 0);
    assert_eq!(read_profile(&svm, host_profile).hosted_stays, 0);
}

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

#[test]
fn close_unresolved_dispute_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let (booking, guest_profile, _host_profile) = setup_open_dispute(&mut svm, &payer, &guest);
    let dispute = dispute_pda(booking);

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(
            dispute,
            booking,
            guest_profile,
            _host_profile,
            guest.pubkey(),
        ),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotResolved
            ))
        )
    );
}

#[test]
fn close_with_wrong_opener_wallet_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest_wallet = Pubkey::new_unique();
    let host_wallet = Pubkey::new_unique();
    let stranger = Pubkey::new_unique();
    svm.airdrop(&stranger, 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest_wallet);
    let host_profile = setup_user_profile(&mut svm, host_wallet);
    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());

    let property = Pubkey::new_unique();
    let booking = setup_booking(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        BookingStatus::DisputeResolved,
    );
    let dispute = set_dispute_account(
        &mut svm,
        booking,
        DisputeParty::Guest,
        DisputeState::ResolvedByAdmin,
        BookingStatus::Active,
    );

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(dispute, booking, guest_profile, host_profile, stranger),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::InvalidOpenerWallet
            ))
        )
    );
}

#[test]
fn close_with_unrelated_guest_profile_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let guest_wallet = Pubkey::new_unique();
    let host_wallet = Pubkey::new_unique();
    let stranger_wallet = Pubkey::new_unique();
    svm.airdrop(&guest_wallet, 1_000_000_000).unwrap();

    let guest_profile = setup_user_profile(&mut svm, guest_wallet);
    let host_profile = setup_user_profile(&mut svm, host_wallet);
    let stranger_profile = setup_user_profile(&mut svm, stranger_wallet);
    setup_global_config(&mut svm, stayke_disputes::id(), stayke_escrow::id());

    let property = Pubkey::new_unique();
    let booking = setup_booking(
        &mut svm,
        guest_profile,
        host_profile,
        property,
        CHECK_IN,
        BookingStatus::DisputeResolved,
    );
    let dispute = set_dispute_account(
        &mut svm,
        booking,
        DisputeParty::Guest,
        DisputeState::ResolvedByAdmin,
        BookingStatus::Active,
    );

    let res = send_close(
        &mut svm,
        &payer,
        close_dispute_ix(
            dispute,
            booking,
            stranger_profile,
            host_profile,
            guest_wallet,
        ),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnboundBookingAccount
            ))
        )
    );
}
