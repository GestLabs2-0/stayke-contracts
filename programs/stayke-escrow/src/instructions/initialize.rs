use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{error::EscrowError, state::EscrowConfig};

// ---------------------------------------------------------------------------
// Initialize escrow config
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + EscrowConfig::INIT_SPACE,
        seeds = [b"escrow_config"],
        bump,
    )]
    pub escrow_config: Account<'info, EscrowConfig>,

    /// CHECK: PDA that will sign as the platform-vault authority.
    #[account(seeds = [b"platform_vault"], bump)]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = platform_vault_pda,
        seeds = [b"platform_vault_token"],
        bump,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_escrow(ctx: Context<InitializeEscrow>, fee_bps: u16) -> Result<()> {
    require!(fee_bps < 10_000, EscrowError::InvalidBps);

    let config = &mut ctx.accounts.escrow_config;
    config.authority = ctx.accounts.authority.key();
    config.platform_vault = ctx.accounts.platform_vault.key();
    config.platform_vault_bump = ctx.bumps.platform_vault_pda;
    config.usdc_mint = ctx.accounts.usdc_mint.key();
    config.fee_bps = fee_bps;
    config.is_initialized = true;
    config.bump = ctx.bumps.escrow_config;

    Ok(())
}
