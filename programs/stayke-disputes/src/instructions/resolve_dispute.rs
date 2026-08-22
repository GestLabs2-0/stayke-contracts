use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_config::{error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::USER_PROFILE_SEED,
    cpi::{
        accounts::{UpdateReputationProfile, UpdateUserProfile},
        add_infraction, update_deposit,
    },
    program::StaykeCore,
    PenaltySeverity, ReputationProfile, UserProfile, REPUTATION_PROFILE_SEED,
};
use stayke_escrow::{
    constants::BOOKING_SEED,
    cpi::{accounts::ResolveDisputeTransferCpi, cpi_resolve_dispute_transfer},
    program::StaykeEscrow,
    state::Booking,
};
use stayke_treasury::{
    cpi::{accounts::PenalizeTransferCpi, cpi_penalize_transfer},
    program::StaykeTreasury,
    TreasuryConfig, TreasuryError, TREASURY_CONFIG_SEED, TREASURY_SEED,
};

use crate::{
    constants::{
        CPI_AUTHORITY_SEED, DEPOSIT_SLASH_HIGH_BPS, DEPOSIT_SLASH_MEDIUM_BPS,
        DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED, ESCROW_SLASH_HIGH_BPS, ESCROW_SLASH_MEDIUM_BPS,
    },
    error::DisputeError,
    events::DisputeResolvedByAdmin,
    state::{DisputeAccount, DisputeConfig, DisputeState},
    DisputeOutcome, DisputeParty,
};

#[derive(Accounts)]
pub struct ResolveDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        bump = config.bump,
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
    )]
    pub config: Box<Account<'info, DisputeConfig>>,

    #[account(
        mut,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.state == DisputeState::Escalated @ DisputeError::DisputeNotEscalated,
    )]
    pub dispute: Box<Account<'info, DisputeAccount>>,

    #[account(mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref() ],
        bump = booking.bump,
        seeds::program = stayke_escrow::ID,
        constraint = dispute.booking == booking.key() @ DisputeError::UnboundBooking
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = booking.host == host_profile.key() @ DisputeError::UnboundBookingAccount,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = booking.guest == guest_profile.key() @ DisputeError::UnboundBookingAccount,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), guilty_reputation.authority.as_ref()],
        seeds::program = stayke_core::ID,
        bump = guilty_reputation.bump
    )]
    pub guilty_reputation: Box<Account<'info, ReputationProfile>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: CPI authority PDA of an allowlisted Stayke program.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = host_profile.authority,
        token::token_program = token_program,
        // constraint = host_token_account.mint == .key() @ DisputeError::InvalidTokenMint,
        // constraint = host_token_account.owner ==  @ DisputeError::InvalidPayoutTokenAccount,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = guest_profile.authority,
        token::token_program = token_program,
        // constraint = guest_token_account.mint == usdc_mint.key() @ DisputeError::InvalidTokenMint,
        // constraint = guest_token_account.owner == guest_profile.authority @ DisputeError::InvalidPayoutTokenAccount,
    )]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = platform_vault_token_account.key() == global_config.platform_vault @ StaykeConfigError::InvalidVaultAccount,
    )]
    pub platform_vault_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = usdc_mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        constraint = treasury_vault.key() == config_treasury.treasury_vault @ TreasuryError::InvalidTreasuryVault,
    )]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Treasury PDA — signs the CPI transfer out of the vault.
    #[account(seeds = [TREASURY_SEED.as_bytes()], bump = config_treasury.treasury_bump, seeds::program = stayke_treasury::ID)]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump = config_treasury.bump,
        seeds::program = stayke_treasury::ID
    )]
    pub config_treasury: Box<Account<'info, TreasuryConfig>>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub stayke_treasury_program: Program<'info, StaykeTreasury>,
    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_resolve_dispute(
    ctx: Context<ResolveDispute>,
    outcome: DisputeOutcome,
) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let global_config = &ctx.accounts.global_config;
    let token_program = &ctx.accounts.token_program;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let judgement = find_judgement(
        dispute.opened_by.clone(),
        outcome,
        &ctx.accounts.host_profile,
        &ctx.accounts.guest_profile,
        &mut ctx.accounts.host_token_account,
        &mut ctx.accounts.guest_token_account,
    );
    let guilty_reputation = &mut ctx.accounts.guilty_reputation;

    if let Some(Judgement {
        severity,
        guilty,
        victim,
        guilty_token,
        victim_token,
        escrow_slash,
        treasury_slash,
    }) = judgement
    {
        require!(
            guilty.authority == guilty_reputation.authority,
            DisputeError::InvalidReputationProfile
        );

        let cpi_infraction = UpdateReputationProfile {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            global_config: global_config.to_account_info(),
            reputation_profile: guilty_reputation.to_account_info(),
        };

        let cpi_ctx_infract =
            CpiContext::new_with_signer(stayke_core::ID, cpi_infraction, signer_seeds);
        add_infraction(cpi_ctx_infract, severity)?;

        let cpi_accounts = ResolveDisputeTransferCpi {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            booking: ctx.accounts.booking.to_account_info(),
            guilty_profile: guilty.to_account_info(),
            victim_profile: victim.to_account_info(),
            global_config: global_config.to_account_info(),
            escrow_token_account: ctx.accounts.escrow_token_account.to_account_info(),
            guilty_token_account: guilty_token.to_account_info(),
            victim_token_account: victim_token.to_account_info(),
            platform_vault_token_account: ctx
                .accounts
                .platform_vault_token_account
                .to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let cpi_ctx_resolve =
            CpiContext::new_with_signer(stayke_escrow::ID, cpi_accounts, signer_seeds);
        cpi_resolve_dispute_transfer(cpi_ctx_resolve, escrow_slash)?;

        let penalize_transfer = PenalizeTransferCpi {
            config: ctx.accounts.config_treasury.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            destination_token_account: victim_token.to_account_info(),
            global_config: global_config.to_account_info(),
            token_program: token_program.to_account_info(),
            treasury_pda: ctx.accounts.treasury_pda.to_account_info(),
            treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
            usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
        };

        let amount_slash = (guilty.deposited as u128)
            .saturating_mul(treasury_slash as u128)
            .saturating_div(10_000) as u64;
        let victim_amount = guilty.deposited.saturating_sub(amount_slash);

        let cpi_ctx_penalize =
            CpiContext::new_with_signer(stayke_treasury::ID, penalize_transfer, signer_seeds);

        cpi_penalize_transfer(cpi_ctx_penalize, victim_amount)?;

        let update_deposit_cpi = UpdateUserProfile {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            global_config: global_config.to_account_info(),
            user_profile: guilty.to_account_info(),
        };

        let cpi_ctx_update_deposit =
            CpiContext::new_with_signer(stayke_core::ID, update_deposit_cpi, signer_seeds);

        update_deposit(cpi_ctx_update_deposit, amount_slash, false)?;
    }

    dispute.state = DisputeState::ResolvedByAdmin;

    emit!(DisputeResolvedByAdmin {
        dispute: dispute.key(),
        booking: dispute.booking,
        resolved_by: ctx.accounts.admin.key(),
        resolved_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub struct Judgement<'info, 'a>
where
    'info: 'a,
{
    severity: PenaltySeverity,
    guilty: &'a Account<'info, UserProfile>,
    victim: &'a Account<'info, UserProfile>,
    guilty_token: &'a mut InterfaceAccount<'info, TokenAccount>,
    victim_token: &'a mut InterfaceAccount<'info, TokenAccount>,
    escrow_slash: u16,
    treasury_slash: u16,
}

fn find_judgement<'info, 'a>(
    opened_by: DisputeParty,
    outcome: DisputeOutcome,
    host_profile: &'a Account<'info, UserProfile>,
    guest_profile: &'a Account<'info, UserProfile>,
    host_token_account: &'a mut InterfaceAccount<'info, TokenAccount>,
    guest_token_account: &'a mut InterfaceAccount<'info, TokenAccount>,
) -> Option<Judgement<'info, 'a>>
where
    'info: 'a,
{
    match outcome {
        DisputeOutcome::GuestFavored { severity } => {
            let (escrow_slash, treasury_slash) = host_favored_slash(&severity, false);

            Some(Judgement {
                severity,
                guilty: host_profile,
                victim: guest_profile,
                guilty_token: host_token_account,
                victim_token: guest_token_account,
                escrow_slash,
                treasury_slash,
            })
        }

        DisputeOutcome::NoFaultFound => None,
        DisputeOutcome::HostFavored { severity } => {
            // because host is the victim, he should have all the money of the escrow always
            let (escrow_slash, treasury_slash) = host_favored_slash(&severity, true);
            Some(Judgement {
                severity,
                guilty: guest_profile,
                victim: host_profile,
                guilty_token: guest_token_account,
                victim_token: host_token_account,
                escrow_slash,
                treasury_slash,
            })
        }
        DisputeOutcome::MaliciousClaim { severity } => {
            let (guilty, victim, guilty_token, victim_token, escrow_slash, treasury_slash) =
                if opened_by == DisputeParty::Guest {
                    let (escrow_slash, treasury_slash) = host_favored_slash(&severity, true);

                    (
                        guest_profile,
                        host_profile,
                        guest_token_account,
                        host_token_account,
                        escrow_slash,
                        treasury_slash,
                    )
                } else {
                    let (escrow_slash, treasury_slash) = host_favored_slash(&severity, false);

                    (
                        host_profile,
                        guest_profile,
                        host_token_account,
                        guest_token_account,
                        escrow_slash,
                        treasury_slash,
                    )
                };
            Some(Judgement {
                severity,
                guilty,
                victim,
                guilty_token,
                victim_token,
                escrow_slash,
                treasury_slash,
            })
        }
    }
}

fn host_favored_slash(penalty: &PenaltySeverity, is_host_favored: bool) -> (u16, u16) {
    match (penalty, is_host_favored) {
        (PenaltySeverity::High, true) => (10_000, DEPOSIT_SLASH_HIGH_BPS),
        (PenaltySeverity::Medium, true) => (10_000, DEPOSIT_SLASH_MEDIUM_BPS),
        (PenaltySeverity::Low, true) => (10_000, 0),
        (PenaltySeverity::High, false) => (ESCROW_SLASH_HIGH_BPS, DEPOSIT_SLASH_HIGH_BPS),
        (PenaltySeverity::Medium, false) => (ESCROW_SLASH_MEDIUM_BPS, DEPOSIT_SLASH_MEDIUM_BPS),
        (PenaltySeverity::Low, false) => (0, 0),
    }
}
