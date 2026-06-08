use anchor_lang::prelude::*;

use crate::{
    config::ConfigAcc, error::StaykeError, Identity, UserProfile, CORE_CONFIG_SEED, IDENTITY_SEED,
    USER_PROFILE_SEED,
};
#[derive(Accounts)]
pub struct VerifyIdentity<'info> {
    #[account(mut, constraint = authority.key() == config.authority @ StaykeError::Unauthorized)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.owner.key().as_ref()], bump = user_profile.bump)]
    pub user_profile: Account<'info, UserProfile>,

    #[account(mut, seeds = [IDENTITY_SEED.as_bytes(), identity.id.as_ref()], bump = identity.bump, constraint = !identity.is_frozen @ StaykeError::IdentityFrozen)]
    pub identity: Account<'info, Identity>,

    #[account(seeds = [CORE_CONFIG_SEED.as_bytes()], bump = config.bump)]
    pub config: Account<'info, ConfigAcc>,
}

/// Used to handle identity verification
pub fn handler_verify_identity(ctx: Context<VerifyIdentity>) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    let identity = &mut ctx.accounts.identity;

    // We will just set the is_verified field to true, in a real implementation we would have some off-chain verification process and then update the identity data accordingly.

    identity.verifier = Some(ctx.accounts.authority.key());
    identity.is_frozen = true;
    identity.owner = user_profile.key();
    identity.verified_at = Clock::get()?.unix_timestamp;

    user_profile.is_verified = true;

    Ok(())
}
