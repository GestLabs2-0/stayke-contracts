use anchor_lang::prelude::*;

use crate::{ConfigAcc, CORE_CONFIG_SEED};
#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(init, payer = authority, space = 8 + ConfigAcc::INIT_SPACE, seeds = [CORE_CONFIG_SEED.as_bytes()], bump)]
    pub config: Account<'info, ConfigAcc>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.bump = ctx.bumps.config;
    Ok(())
}
