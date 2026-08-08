use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_config::{error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_escrow::{
    cpi::{accounts::ResolveDisputeTransferCpi, cpi_resolve_dispute_transfer},
    program::StaykeEscrow,
    state::{Booking, EscrowConfig},
    constants::ESCROW_CONFIG_SEED,
};

use crate::{
    constants::{DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED},
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
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_escrow_program.key(),
        bump = escrow_config.bump,
        constraint = escrow_config.global_config == global_config.key() @ StaykeConfigError::InvalidGlobalConfig,
    )]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut)]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
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
        authority: ctx.accounts.admin.to_account_info(),
        booking: ctx.accounts.booking.to_account_info(),
        escrow_config: ctx.accounts.escrow_config.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        escrow_token_account: ctx.accounts.escrow_token_account.to_account_info(),
        host_token_account: ctx.accounts.host_token_account.to_account_info(),
        guest_token_account: ctx.accounts.guest_token_account.to_account_info(),
        platform_vault_token_account: ctx.accounts.platform_vault_token_account.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.stayke_escrow_program.key(), cpi_accounts);
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
