use anchor_lang::prelude::*;

use crate::{constants::DISPUTE_CONFIG_PDA_SEED, error::DisputeError, DisputeConfig, MAX_ADMINS};

#[derive(Accounts)]
pub struct AddAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
    )]
    pub dispute_config: Account<'info, DisputeConfig>,
}

pub fn handler_add_admin(ctx: Context<AddAdmin>, admin: Pubkey) -> Result<()> {
    let dispute_config = &mut ctx.accounts.dispute_config;
    if dispute_config.admins.len() >= MAX_ADMINS {
        return err!(DisputeError::UnableToAddAdmins);
    }
    dispute_config.admins.push(admin);

    Ok(())
}
