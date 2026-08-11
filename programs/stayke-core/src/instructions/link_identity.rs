use anchor_lang::prelude::*;

use crate::{
    constants::{CORE_CONFIG_SEED, IDENTITY_SEED},
    error::StaykeError,
    ConfigAcc, Identity, UserProfile, USER_PROFILE_SEED,
};

#[derive(Accounts)]
#[instruction(id: [u8; 32])]
pub struct LinkIdentity<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut, constraint = config.authority == authority.key() @ StaykeError::Unauthorized )]
    pub authority: Signer<'info>,

    #[account(mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.authority.key().as_ref()],
        bump = user_profile.bump,
        constraint = user_profile.identity.is_none() @ StaykeError::UserProfileAlreadyLinked,
        constraint = !user_profile.banned @ StaykeError::IdentityBanned
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(mut,
        seeds = [id.as_ref(), IDENTITY_SEED.as_bytes()],
        bump = identity.bump,
        constraint = !identity.linked @ StaykeError::IdentityFrozen
    )]
    pub identity: Account<'info, Identity>,

    #[account(seeds = [CORE_CONFIG_SEED.as_bytes()], bump = config.bump )]
    pub config: Account<'info, ConfigAcc>,
}

pub fn handler_link_identity(ctx: Context<LinkIdentity>) -> Result<()> {
    let identity = &mut ctx.accounts.identity;
    let user_profile = &mut ctx.accounts.user_profile;

    user_profile.identity = Some(identity.key());
    identity.linked = true;

    Ok(())
}
