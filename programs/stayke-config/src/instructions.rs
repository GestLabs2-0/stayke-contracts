use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    error::StaykeConfigError, AllowedPrograms, GlobalConfig, GLOBAL_CONFIG_SEED,
    PLATFORM_VAULT_CONFIG_SEED, PLATFORM_VAULT_SEED,
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
    max_operations: u8,
    allowed_programs: AllowedPrograms,
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

    global_config.core_program = allowed_programs.core;
    global_config.escrow_program = allowed_programs.escrow;
    global_config.disputes_program = allowed_programs.disputes;
    global_config.treasury_program = allowed_programs.treasury;
    global_config.is_initialized = true;
    global_config.max_operations = max_operations;
    Ok(())
}
