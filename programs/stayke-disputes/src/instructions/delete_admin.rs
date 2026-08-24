use anchor_lang::prelude::*;

use crate::{constants::DISPUTE_CONFIG_PDA_SEED, error::DisputeError, DisputeConfig};

#[derive(Accounts)]
pub struct DeleteAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
    )]
    pub dispute_config: Account<'info, DisputeConfig>,
}

pub fn handler_delete_admin(ctx: Context<DeleteAdmin>, admin: Pubkey) -> Result<()> {
    let dispute_config = &mut ctx.accounts.dispute_config;

    dispute_config.admins.retain(|old| old != &admin);

    Ok(())
}
