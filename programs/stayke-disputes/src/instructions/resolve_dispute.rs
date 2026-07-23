use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use stayke_escrow::{
    cpi::{accounts::ResolveDisputeTransferCpi, cpi_resolve_dispute_transfer},
    program::StaykeEscrow,
    state::Booking,
};

use crate::{
    constants::{DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeResolved,
    state::{Dispute, DisputeConfig, DisputeStatus},
};

// ---------------------------------------------------------------------------
// Resolve Dispute (Admin determines blame and routes funds)
// ---------------------------------------------------------------------------

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

    // Escrow Accounts needed for the CPI:
    /// CHECK: Escrow config validated by stayke-escrow program during CPI.
    pub escrow_config: UncheckedAccount<'info>,

    /// CHECK: Global config validated by stayke-escrow program during CPI.
    pub global_config: UncheckedAccount<'info>,

    // TODO: add validations for token accounts. Platform and usdc_mint need to be equal to the other config files
    #[account(mut)]
    pub escrow_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub host_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub guest_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub platform_vault_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_resolve_dispute(
    ctx: Context<ResolveDispute>,
    host_share_bps: u16,
    rejected: bool,
) -> Result<()> {
    // Escrow transfer CPI
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
