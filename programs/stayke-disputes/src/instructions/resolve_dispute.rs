// ---------------------------------------------------------------------------
// DEPRECATED — STK-168 refactor: P2P dispute flow with admin escalation.
// The admin-mediated resolve_dispute flow is being replaced. This module is
// commented out to keep the crate compiling during the refactor.
// ---------------------------------------------------------------------------
/*
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_config::{error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};
use stayke_escrow::{
    cpi::{accounts::ResolveDisputeTransferCpi, cpi_resolve_dispute_transfer},
    program::StaykeEscrow,
    state::Booking,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeResolved,
    state::{Dispute, DisputeConfig, DisputeStatus},
};

#[derive(Accounts)]
pub struct ResolveDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Box<Account<'info, DisputeConfig>>,

    #[account(
        mut,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::Open @ DisputeError::DisputeNotOpen,
    )]
    pub dispute: Account<'info, Dispute>,

    #[account(mut)]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = booking.host == host_profile.key() @ DisputeError::UnboundBookingAccount,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = booking.guest == guest_profile.key() @ DisputeError::UnboundBookingAccount,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: CPI authority PDA of an allowlisted Stayke program.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = host_token_account.mint == usdc_mint.key() @ DisputeError::InvalidTokenMint,
        constraint = host_token_account.owner == host_profile.authority @ DisputeError::InvalidPayoutTokenAccount,
    )]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = guest_token_account.mint == usdc_mint.key() @ DisputeError::InvalidTokenMint,
        constraint = guest_token_account.owner == guest_profile.authority @ DisputeError::InvalidPayoutTokenAccount,
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
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_resolve_dispute(
    ctx: Context<ResolveDispute>,
    host_share_bps: u16,
    rejected: bool,
) -> Result<()> {
    let cpi_accounts = ResolveDisputeTransferCpi {
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        booking: ctx.accounts.booking.to_account_info(),
        host_profile: ctx.accounts.host_profile.to_account_info(),
        guest_profile: ctx.accounts.guest_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        escrow_token_account: ctx.accounts.escrow_token_account.to_account_info(),
        host_token_account: ctx.accounts.host_token_account.to_account_info(),
        guest_token_account: ctx.accounts.guest_token_account.to_account_info(),
        platform_vault_token_account: ctx.accounts.platform_vault_token_account.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.stayke_escrow_program.key(),
        cpi_accounts,
        signer_seeds,
    );
    cpi_resolve_dispute_transfer(cpi_ctx, host_share_bps, rejected)?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = if rejected {
        DisputeStatus::Rejected
    } else {
        DisputeStatus::Resolved
    };
    dispute.resolved_at = Some(Clock::get()?.unix_timestamp);

    emit!(DisputeResolved {
        dispute: dispute.key(),
        booking: dispute.booking,
        host_share_bps,
        timestamp: dispute.resolved_at.unwrap(),
    });

    Ok(())
}
*/
