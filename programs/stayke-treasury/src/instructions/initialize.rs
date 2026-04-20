use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_config::{error::StaykeConfigError, GLOBAL_CONFIG_SEED};

use crate::{
    error::TreasuryError, TreasuryConfig, TREASURY_CONFIG_SEED, TREASURY_SEED, TREASURY_VAULT_SEED,
};

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
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    /// CHECK: PDA that owns the treasury vault. No data stored here.
    #[account(
        seeds = [TREASURY_SEED.as_bytes()],
        bump,
    )]
    pub treasury_pda: UncheckedAccount<'info>,

    /// USDC token account controlled by treasury_pda.
    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = treasury_pda,
        seeds = [TREASURY_VAULT_SEED.as_bytes()],
        bump,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = usdc_mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, stayke_config::GlobalConfig>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_treasury(ctx: Context<InitializeTreasury>) -> Result<()> {
    let config = &mut ctx.accounts.config;

    require!(!config.is_initialized, TreasuryError::AlreadyInitialized);

    config.authority = ctx.accounts.authority.key();
    config.treasury_vault = ctx.accounts.treasury_vault.key();
    config.treasury_bump = ctx.bumps.treasury_pda;
    config.global_config = ctx.accounts.global_config.key();
    config.is_initialized = true;
    config.bump = ctx.bumps.config;

    Ok(())
}
