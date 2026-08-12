use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{constants::ESCROW_CONFIG_SEED, state::EscrowConfig};

// ---------------------------------------------------------------------------
// Initialize escrow config
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeConfigEscrow<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + EscrowConfig::INIT_SPACE,
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub escrow_config: Account<'info, EscrowConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config_escrow(ctx: Context<InitializeConfigEscrow>) -> Result<()> {
    let config = &mut ctx.accounts.escrow_config;
    config.authority = ctx.accounts.authority.key();
    config.is_initialized = true;
    config.bump = ctx.bumps.escrow_config;

    Ok(())
}
