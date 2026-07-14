use anchor_lang::prelude::*;

use crate::{config::ConfigAcc, error::StaykeError, Identity, CORE_CONFIG_SEED, IDENTITY_SEED};
#[derive(Accounts)]
#[instruction(id: [u8; 32])]
pub struct InitIdentity<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(constraint = authority.key() == config.authority @ StaykeError::Unauthorized)]
    pub authority: Signer<'info>,

    #[account(init,
        payer = payer,
        space = 8 + Identity::INIT_SPACE,
        seeds = [id.as_ref(), IDENTITY_SEED.as_bytes()],
        bump
    )]
    pub identity: Account<'info, Identity>,

    #[account(seeds = [CORE_CONFIG_SEED.as_bytes()], bump = config.bump)]
    pub config: Account<'info, ConfigAcc>,

    pub system_program: Program<'info, System>,
}

/// Initialize an identity account by an authorized signer
pub fn handler_init_identity(ctx: Context<InitIdentity>) -> Result<()> {
    let identity = &mut ctx.accounts.identity;
    let bump = ctx.bumps.identity;

    identity.verified_at = Clock::get()?.unix_timestamp;
    identity.bump = bump;

    Ok(())
}
