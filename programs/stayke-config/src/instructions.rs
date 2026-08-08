use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    error::StaykeConfigError, GlobalConfig, GLOBAL_CONFIG_SEED, PLATFORM_VAULT_CONFIG_SEED,
    PLATFORM_VAULT_SEED,
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(init, payer = authority, space = 8 + GlobalConfig::INIT_SPACE, seeds = [GLOBAL_CONFIG_SEED.as_bytes()], bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: PDA that will sign as the platform-vault authority.
    #[account(seeds = [PLATFORM_VAULT_SEED.as_bytes()], bump)]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = platform_vault_pda,
        seeds = [PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(
    ctx: Context<InitializeConfig>,
    minimum_deposit: u64,
    fee_bps: u64,
) -> Result<()> {
    require!(fee_bps < 10_000, StaykeConfigError::InvalidFeeBps);

    let global_config = &mut ctx.accounts.global_config;
    global_config.authority = ctx.accounts.authority.key();
    global_config.bump = ctx.bumps.global_config;
    global_config.minimum_deposit = minimum_deposit;
    global_config.fee_bps = fee_bps;
    global_config.usdc_mint = ctx.accounts.usdc_mint.key();
    global_config.platform_vault = ctx.accounts.platform_vault.key();
    global_config.platform_vault_bump = ctx.bumps.platform_vault_pda;
    // Embedded declare_id! values — used as the canonical registry for CPI allowlisting.
    global_config.core_program = pubkey!("8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP");
    global_config.escrow_program = pubkey!("FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ");
    global_config.disputes_program = pubkey!("7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB");
    global_config.treasury_program = pubkey!("59buEPHFBK4h8LyLE2KtnV1kpaQTyjb82NWt5F9jSuHu");
    global_config.is_initialized = true;

    Ok(())
}
