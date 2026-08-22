mod common;

// ---------------------------------------------------------------------------
// resolve_dispute test suite.
//
// BLOCKED: every case currently fails with "Access violation in stack frame 5"
// inside the stayke-disputes program during account loading (reproducible in
// LiteSVM; the full 19-account struct crashes even with a stubbed handler, while
// trimmed structs load cleanly). The cases below document the intended behavior
// and are marked #[ignore] until the program-side crash is fixed.
// ---------------------------------------------------------------------------
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
    stayke_core::{state::ReputationProfile, PenaltySeverity},
    stayke_disputes::state::{DisputeAccount, DisputeOutcome, DisputeState},
    stayke_escrow::state::{Booking, BookingStatus},
};

const CHECK_IN: i64 = 1_735_689_600;
const ESCROW_TOTAL: u64 = 1_000_000_000;
const DEPOSIT: u64 = 1_000_000;
const TREASURY_FUNDING: u64 = 5_000_000_000;
const FEE_BPS: u64 = 200;

// With fee_bps = 200: fee = 20_000_000, distributable = 980_000_000.
const FEE: u64 = 20_000_000;
const DISTRIBUTABLE: u64 = 980_000_000;
// Medium deposit slash (DEPOSIT_SLASH_MEDIUM_BPS = 3_000): 30% of the deposit.
const DEPOSIT_SLASH_MEDIUM: u64 = 300_000;
const RETRIBUTION_MEDIUM: u64 = 700_000;

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

fn global_config_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    )
    .0
}

fn dispute_config_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[stayke_disputes::constants::DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        &stayke_disputes::id(),
    )
    .0
}

fn treasury_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[stayke_treasury::TREASURY_SEED.as_bytes()],
        &stayke_treasury::id(),
    )
    .0
}

fn escrow_token_pda(booking: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            stayke_escrow::constants::ESCROW_PDA_SEED.as_bytes(),
            booking.as_ref(),
        ],
        &stayke_escrow::id(),
    )
    .0
}

fn setup_global_config_full(
    svm: &mut LiteSVM,
    usdc_mint: Pubkey,
    platform_vault: Pubkey,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[stayke_config::constants::GLOBAL_CONFIG_SEED.as_bytes()],
        &stayke_config::id(),
    );
    let gc = stayke_config::state::GlobalConfig {
        authority: Pubkey::new_unique(),
        free_ops: 4,
        minimum_deposit: 0,
        fee_bps: FEE_BPS,
        usdc_mint,
        is_initialized: true,
        platform_vault,
        platform_vault_bump: bump,
        core_program: stayke_core::id(),
        escrow_program: stayke_escrow::id(),
        disputes_program: stayke_disputes::id(),
        treasury_program: stayke_treasury::id(),
        bump,
    };
    svm.set_account(
        pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("GlobalConfig", &gc),
            owner: stayke_config::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    pda
}

fn setup_user_profile_deposit(svm: &mut LiteSVM, authority: Pubkey, deposited: u64) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            stayke_core::constants::USER_PROFILE_SEED.as_bytes(),
            authority.as_ref(),
        ],
        &stayke_core::id(),
    );
    let profile = stayke_core::state::UserProfile {
        completed_stays: 0,
        hosted_stays: 0,
        authority,
        identity: Some(Pubkey::new_unique()),
        active_booking: None,
        deposited,
        lending: 0,
        staked: 0,
        banned: false,
        listings: 0,
        bump,
    };
    svm.set_account(
        pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("UserProfile", &profile),
            owner: stayke_core::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    pda
}

fn setup_booking_with_price(
    svm: &mut LiteSVM,
    guest: Pubkey,
    host: Pubkey,
    property: Pubkey,
    status: BookingStatus,
    total_price: u64,
) -> Pubkey {
    let (booking_key, bump) = Pubkey::find_program_address(
        &[
            stayke_escrow::constants::BOOKING_SEED.as_bytes(),
            property.as_ref(),
            guest.as_ref(),
            CHECK_IN.to_le_bytes().as_ref(),
        ],
        &stayke_escrow::id(),
    );
    let (_escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", booking_key.as_ref()], &stayke_escrow::id());
    let booking = Booking {
        guest,
        host,
        guest_review: 0,
        host_review: 0,
        updated_at: CHECK_IN,
        property,
        check_in: CHECK_IN,
        check_out: CHECK_IN + 86_400,
        total_price,
        status,
        escrow_bump,
        bump,
    };
    svm.set_account(
        booking_key,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("Booking", &booking),
            owner: stayke_escrow::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    booking_key
}

/// SPL token account (165 bytes) with the given balance (owned by `owner`).
fn make_token_account(svm: &mut LiteSVM, key: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // AccountState::Initialized
    svm.set_account(
        key,
        solana_account::Account {
            lamports: 1_000_000_000,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
}

fn setup_treasury(svm: &mut LiteSVM, usdc_mint: Pubkey, amount: u64) -> (Pubkey, Pubkey) {
    let (config_pda, config_bump) = Pubkey::find_program_address(
        &[stayke_treasury::TREASURY_CONFIG_SEED.as_bytes()],
        &stayke_treasury::id(),
    );
    let (_treasury_pda, treasury_bump) = Pubkey::find_program_address(
        &[stayke_treasury::TREASURY_SEED.as_bytes()],
        &stayke_treasury::id(),
    );
    let vault = Pubkey::new_unique();
    let config = stayke_treasury::state::TreasuryConfig {
        authority: Pubkey::new_unique(),
        treasury_vault: vault,
        treasury_bump,
        global_config: global_config_pda(),
        is_initialized: true,
        bump: config_bump,
    };
    svm.set_account(
        config_pda,
        solana_account::Account {
            lamports: 1_000_000_000,
            data: to_account_data("TreasuryConfig", &config),
            owner: stayke_treasury::id(),
            executable: false,
            rent_epoch: u64::MAX,
        },
    )
    .unwrap();
    make_token_account(svm, vault, usdc_mint, treasury_pda(), amount);
    (config_pda, vault)
}

fn token_balance(svm: &LiteSVM, key: Pubkey) -> u64 {
    match svm.get_account(&key) {
        Some(a) if a.data.len() >= 72 => u64::from_le_bytes(a.data[64..72].try_into().unwrap()),
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Environment + instruction builders
// ---------------------------------------------------------------------------

struct ResolveEnv {
    booking: Pubkey,
    dispute: Pubkey,
    dispute_config: Pubkey,
    global_config: Pubkey,
    escrow_token: Pubkey,
    host_token: Pubkey,
    guest_token: Pubkey,
    platform_vault: Pubkey,
    treasury_config: Pubkey,
    treasury_vault: Pubkey,
    guest_profile: Pubkey,
    host_profile: Pubkey,
    guest_reputation: Pubkey,
    host_reputation: Pubkey,
    usdc_mint: Pubkey,
}

/// Full setup: a booking (Active) with funded escrow, profiles with the given
/// deposits, reputation profiles for both parties, treasury, and a dispute
/// opened by `guest`. When `escalate` is true the dispute is escalated past
/// the P2P window so it is ready for admin resolution.
#[allow(clippy::too_many_arguments)]
fn setup_resolvable_dispute(
    svm: &mut LiteSVM,
    payer: &Keypair,
    guest: &Keypair,
    guest_deposit: u64,
    host_deposit: u64,
    escalate: bool,
) -> ResolveEnv {
    let usdc_mint = Pubkey::new_unique();
    make_mint(svm, usdc_mint);

    let platform_vault = Pubkey::new_unique();
    make_token_account(svm, platform_vault, usdc_mint, Pubkey::new_unique(), 0);
    let global_config = setup_global_config_full(svm, usdc_mint, platform_vault);

    let guest_wallet = guest.pubkey();
    let host_wallet = Pubkey::new_unique();
    let guest_profile = setup_user_profile_deposit(svm, guest_wallet, guest_deposit);
    let host_profile = setup_user_profile_deposit(svm, host_wallet, host_deposit);
    let guest_reputation = setup_reputation_profile(svm, guest_wallet);
    let host_reputation = setup_reputation_profile(svm, host_wallet);

    let property = Pubkey::new_unique();
    let booking = setup_booking_with_price(
        svm,
        guest_profile,
        host_profile,
        property,
        BookingStatus::Active,
        ESCROW_TOTAL,
    );

    let escrow_token = escrow_token_pda(booking);
    make_token_account(svm, escrow_token, usdc_mint, booking, ESCROW_TOTAL);
    let host_token = Pubkey::new_unique();
    let guest_token = Pubkey::new_unique();
    make_token_account(svm, host_token, usdc_mint, host_wallet, 0);
    make_token_account(svm, guest_token, usdc_mint, guest_wallet, 0);

    let (treasury_config, treasury_vault) = setup_treasury(svm, usdc_mint, TREASURY_FUNDING);

    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = CHECK_IN;
    svm.set_sysvar::<Clock>(&clock);

    let dispute = dispute_pda(booking);
    let open_ix = Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::OpenDispute {}.data(),
        stayke_disputes::accounts::OpenDispute {
            payer: payer.pubkey(),
            initiator: guest.pubkey(),
            initiator_profile: guest_profile,
            booking,
            dispute,
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            global_config,
            stayke_escrow_program: stayke_escrow::id(),
            system_program: solana_system_program::id(),
        }
        .to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[open_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, guest]).unwrap();
    svm.send_transaction(tx).expect("open dispute must succeed");

    if escalate {
        clock.unix_timestamp = CHECK_IN + 86_401;
        svm.set_sysvar::<Clock>(&clock);

        let escalate_ix = Instruction::new_with_bytes(
            stayke_disputes::id(),
            &stayke_disputes::instruction::EscalateDispute {}.data(),
            stayke_disputes::accounts::EscalateDispute { dispute }.to_account_metas(None),
        );
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[escalate_ix], Some(&payer.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
        svm.send_transaction(tx)
            .expect("escalate dispute must succeed");
    }

    ResolveEnv {
        booking,
        dispute,
        dispute_config: dispute_config_pda(),
        global_config,
        escrow_token,
        host_token,
        guest_token,
        platform_vault,
        treasury_config,
        treasury_vault,
        guest_profile,
        host_profile,
        guest_reputation,
        host_reputation,
        usdc_mint,
    }
}

fn resolve_ix(
    env: &ResolveEnv,
    admin: &Keypair,
    guilty_reputation: Pubkey,
    outcome: DisputeOutcome,
) -> Instruction {
    Instruction::new_with_bytes(
        stayke_disputes::id(),
        &stayke_disputes::instruction::ResolveDispute { outcome }.data(),
        stayke_disputes::accounts::ResolveDispute {
            admin: admin.pubkey(),
            config: env.dispute_config,
            dispute: env.dispute,
            booking: env.booking,
            host_profile: env.host_profile,
            guest_profile: env.guest_profile,
            guilty_reputation,
            global_config: env.global_config,
            cpi_authority: cpi_authority_pda(&stayke_disputes::id()),
            escrow_token_account: env.escrow_token,
            host_token_account: env.host_token,
            guest_token_account: env.guest_token,
            platform_vault_token_account: env.platform_vault,
            usdc_mint: env.usdc_mint,
            treasury_vault: env.treasury_vault,
            treasury_pda: treasury_pda(),
            config_treasury: env.treasury_config,
            stayke_core_program: stayke_core::id(),
            stayke_treasury_program: stayke_treasury::id(),
            stayke_escrow_program: stayke_escrow::id(),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    )
}

fn send_resolve(
    svm: &mut LiteSVM,
    payer: &Keypair,
    admin: &Keypair,
    env: &ResolveEnv,
    guilty_reputation: Pubkey,
    outcome: DisputeOutcome,
) -> Result<(), TransactionError> {
    let instruction = resolve_ix(env, admin, guilty_reputation, outcome);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let signers: &[&Keypair] = if admin.pubkey() == payer.pubkey() {
        &[payer]
    } else {
        &[payer, admin]
    };
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    match svm.send_transaction(tx) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("RESOLVE FAIL: {:?}", e);
            Err(e.err)
        }
    }
}

fn read_dispute(svm: &LiteSVM, dispute: Pubkey) -> DisputeAccount {
    let account = svm
        .get_account(&dispute)
        .expect("dispute account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize DisputeAccount")
}

fn read_booking(svm: &LiteSVM, booking: Pubkey) -> Booking {
    let account = svm
        .get_account(&booking)
        .expect("booking account must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize Booking")
}

fn read_reputation(svm: &LiteSVM, reputation: Pubkey) -> ReputationProfile {
    let account = svm.get_account(&reputation).expect("reputation must exist");
    AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize ReputationProfile")
}

fn read_deposit(svm: &LiteSVM, profile: Pubkey) -> u64 {
    let account = svm.get_account(&profile).expect("profile must exist");
    let profile: stayke_core::state::UserProfile =
        AnchorDeserialize::deserialize(&mut &account.data[8..]).expect("deserialize UserProfile");
    profile.deposited
}

// ---------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------

/// GuestFavored Medium (guest opened): guilty = host. Host's escrow share and
/// deposit are slashed; guest receives the escrow majority plus the treasury
/// retribution; the platform keeps its fee; the host earns a medium infraction.
#[test]
fn resolve_guest_favored_medium_distributes_and_penalizes() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, 0, DEPOSIT, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.host_reputation,
        DisputeOutcome::GuestFavored {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_ok(), "resolve should succeed, got: {:?}", res.err());

    assert_eq!(
        read_dispute(&svm, env.dispute).state,
        DisputeState::ResolvedByAdmin
    );
    assert_eq!(
        read_booking(&svm, env.booking).status,
        BookingStatus::DisputeResolved
    );

    // Escrow: guest (victim) 490M + treasury retribution 700K; host (guilty)
    // 490M; platform fee 20M. Escrow account closed.
    assert_eq!(
        token_balance(&svm, env.guest_token),
        DISTRIBUTABLE / 2 + RETRIBUTION_MEDIUM
    );
    assert_eq!(token_balance(&svm, env.host_token), DISTRIBUTABLE / 2);
    assert_eq!(token_balance(&svm, env.platform_vault), FEE);
    assert_eq!(token_balance(&svm, env.escrow_token), 0);

    // Treasury: 30% of the host's deposit moved to the guest; deposit reduced.
    assert_eq!(
        token_balance(&svm, env.treasury_vault),
        TREASURY_FUNDING - RETRIBUTION_MEDIUM
    );
    assert_eq!(
        read_deposit(&svm, env.host_profile),
        DEPOSIT - DEPOSIT_SLASH_MEDIUM
    );
    assert_eq!(
        read_reputation(&svm, env.host_reputation).medium_infractions,
        1
    );
}

/// HostFavored Medium: guilty = guest. Host receives the whole escrow (minus
/// the platform fee) plus the treasury retribution; guest's deposit is slashed.
#[test]
fn resolve_host_favored_medium_distributes_and_penalizes() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, DEPOSIT, 0, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.guest_reputation,
        DisputeOutcome::HostFavored {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_ok(), "resolve should succeed, got: {:?}", res.err());

    assert_eq!(
        read_dispute(&svm, env.dispute).state,
        DisputeState::ResolvedByAdmin
    );
    assert_eq!(
        read_booking(&svm, env.booking).status,
        BookingStatus::DisputeResolved
    );

    // Escrow: host (victim) receives everything minus the fee, plus treasury.
    assert_eq!(
        token_balance(&svm, env.host_token),
        DISTRIBUTABLE + RETRIBUTION_MEDIUM
    );
    assert_eq!(token_balance(&svm, env.guest_token), 0);
    assert_eq!(token_balance(&svm, env.platform_vault), FEE);
    assert_eq!(token_balance(&svm, env.escrow_token), 0);

    assert_eq!(
        token_balance(&svm, env.treasury_vault),
        TREASURY_FUNDING - RETRIBUTION_MEDIUM
    );
    assert_eq!(
        read_deposit(&svm, env.guest_profile),
        DEPOSIT - DEPOSIT_SLASH_MEDIUM
    );
    assert_eq!(
        read_reputation(&svm, env.guest_reputation).medium_infractions,
        1
    );
}

/// MaliciousClaim opened by the guest: the opener is the guilty party.
#[test]
fn resolve_malicious_claim_penalizes_opener() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, DEPOSIT, 0, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.guest_reputation,
        DisputeOutcome::MaliciousClaim {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_ok(), "resolve should succeed, got: {:?}", res.err());

    assert_eq!(
        read_dispute(&svm, env.dispute).state,
        DisputeState::ResolvedByAdmin
    );
    // The opener (guest) is guilty: host receives the escrow plus treasury.
    assert_eq!(
        token_balance(&svm, env.host_token),
        DISTRIBUTABLE + RETRIBUTION_MEDIUM
    );
    assert_eq!(token_balance(&svm, env.guest_token), 0);
    assert_eq!(token_balance(&svm, env.platform_vault), FEE);
    assert_eq!(token_balance(&svm, env.escrow_token), 0);

    assert_eq!(
        token_balance(&svm, env.treasury_vault),
        TREASURY_FUNDING - RETRIBUTION_MEDIUM
    );
    assert_eq!(
        read_deposit(&svm, env.guest_profile),
        DEPOSIT - DEPOSIT_SLASH_MEDIUM
    );
    assert_eq!(
        read_reputation(&svm, env.guest_reputation).medium_infractions,
        1
    );
}

/// HostFavored Low: no deposit slash (Low -> 0 bps). The full deposit is paid
/// out from the treasury and the deposit counter is untouched.
#[test]
fn resolve_host_favored_low_pays_full_deposit_without_slash() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, DEPOSIT, 0, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.guest_reputation,
        DisputeOutcome::HostFavored {
            severity: PenaltySeverity::Low,
        },
    );
    assert!(res.is_ok(), "resolve should succeed, got: {:?}", res.err());

    assert_eq!(
        token_balance(&svm, env.treasury_vault),
        TREASURY_FUNDING - DEPOSIT
    );
    assert_eq!(read_deposit(&svm, env.guest_profile), DEPOSIT);
    assert_eq!(token_balance(&svm, env.host_token), DISTRIBUTABLE + DEPOSIT);
    assert_eq!(token_balance(&svm, env.guest_token), 0);
    assert_eq!(token_balance(&svm, env.escrow_token), 0);
    assert_eq!(
        read_reputation(&svm, env.guest_reputation).low_infractions,
        1
    );
}

/// NoFaultFound: no judgement means no CPIs at all — escrow untouched, deposits
/// and reputation unchanged; only the dispute is marked ResolvedByAdmin. The
/// booking stays frozen in Disputed (the close_dispute cleanup restores it).
#[test]
fn resolve_no_fault_found_leaves_escrow_untouched() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, 0, DEPOSIT, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.host_reputation,
        DisputeOutcome::NoFaultFound,
    );
    assert!(res.is_ok(), "resolve should succeed, got: {:?}", res.err());

    assert_eq!(
        read_dispute(&svm, env.dispute).state,
        DisputeState::ResolvedByAdmin
    );
    assert_eq!(
        read_booking(&svm, env.booking).status,
        BookingStatus::Disputed
    );
    assert_eq!(token_balance(&svm, env.escrow_token), ESCROW_TOTAL);
    assert_eq!(token_balance(&svm, env.guest_token), 0);
    assert_eq!(token_balance(&svm, env.host_token), 0);
    assert_eq!(token_balance(&svm, env.platform_vault), 0);
    assert_eq!(token_balance(&svm, env.treasury_vault), TREASURY_FUNDING);
    assert_eq!(read_deposit(&svm, env.host_profile), DEPOSIT);
    assert_eq!(
        read_reputation(&svm, env.host_reputation).medium_infractions,
        0
    );
}

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

#[test]
fn resolve_not_escalated_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    // Setup WITHOUT escalation: the dispute stays in OpenP2P.
    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, 0, DEPOSIT, false);

    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.host_reputation,
        DisputeOutcome::GuestFavored {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::DisputeNotEscalated
            ))
        )
    );
}

#[test]
fn resolve_unauthorized_admin_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    let rogue = Keypair::new();
    svm.airdrop(&rogue.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey()); // only `admin` is allowed
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, 0, DEPOSIT, true);

    let res = send_resolve(
        &mut svm,
        &payer,
        &rogue,
        &env,
        env.host_reputation,
        DisputeOutcome::GuestFavored {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::UnauthorizedAdmin
            ))
        )
    );
}

#[test]
fn resolve_wrong_guilty_reputation_fails() {
    let (mut svm, payer) = build_svm_with_programs();
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    setup_dispute_config(&mut svm, admin.pubkey());
    let guest = Keypair::new();
    svm.airdrop(&guest.pubkey(), 1_000_000_000).unwrap();

    let env = setup_resolvable_dispute(&mut svm, &payer, &guest, 0, DEPOSIT, true);

    // GuestFavored -> guilty = host, but we pass the guest's reputation.
    let res = send_resolve(
        &mut svm,
        &payer,
        &admin,
        &env,
        env.guest_reputation,
        DisputeOutcome::GuestFavored {
            severity: PenaltySeverity::Medium,
        },
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        TransactionError::InstructionError(
            0,
            Custom(u32::from(
                stayke_disputes::error::DisputeError::InvalidReputationProfile
            ))
        )
    );
}

// TODO: missing test more edge cases
