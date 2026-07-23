use anchor_lang::prelude::*;

use crate::{constants::DISPUTE_CONFIG_PDA_SEED, state::DisputeConfig};

// ---------------------------------------------------------------------------
// Add / Remove admin / Init config — (Standard admin logic)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + DisputeConfig::INIT_SPACE,
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        bump,
    )]
    pub config: Account<'info, DisputeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admins.push(ctx.accounts.authority.key());
    config.retribution_bps_low = 1000;
    config.retribution_bps_medium = 3000;
    config.retribution_bps_high = 10000;
    config.is_initialized = true;
    config.bump = ctx.bumps.config;
    Ok(())
}
