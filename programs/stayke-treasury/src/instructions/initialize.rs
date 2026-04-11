use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{TreasuryConfig, error::TreasuryError};

// ---------------------------------------------------------------------------
// Initialize treasury
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeTreasury<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + TreasuryConfig::INIT_SPACE,
        seeds = [b"treasury_config"],
        bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    /// CHECK: PDA that owns the treasury vault. No data stored here.
    #[account(
        seeds = [b"treasury"],
        bump,
    )]
    pub treasury_pda: UncheckedAccount<'info>,

    /// USDC token account controlled by treasury_pda.
    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = treasury_pda,
        seeds = [b"treasury_vault"],
        bump,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_treasury(
    ctx: Context<InitializeTreasury>,
    minimum_deposit: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    require!(!config.is_initialized, TreasuryError::AlreadyInitialized);

    config.authority = ctx.accounts.authority.key();
    config.treasury_vault = ctx.accounts.treasury_vault.key();
    config.treasury_bump = ctx.bumps.treasury_pda;
    config.usdc_mint = ctx.accounts.usdc_mint.key();
    config.minimum_deposit = minimum_deposit;
    config.is_initialized = true;
    config.bump = ctx.bumps.config;

    Ok(())
}
