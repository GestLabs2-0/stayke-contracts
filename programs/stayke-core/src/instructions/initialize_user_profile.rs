use anchor_lang::prelude::*;

use crate::{
    error::StaykeError, DocType, Identity, ReputationProfile, UserProfile, IDENTITY_SEED,
    REPUTATION_PROFILE_SEED, USER_PROFILE_SEED,
};

// TODO: REFACTOR IDENTITY CREATION BY USING A UNIQUE INSTRUCTION TO CREATE IT

#[derive(Accounts)]
#[instruction(id: [u8; 32])]
pub struct InitializeUserProfile<'info> {
    #[account(init, payer = authority, space = 8 + UserProfile::INIT_SPACE, seeds = [USER_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub user_profile: Account<'info, UserProfile>,
    #[account(init, payer = authority, space = 8 + ReputationProfile::INIT_SPACE, seeds = [REPUTATION_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub reputation_profile: Account<'info, ReputationProfile>,
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + Identity::INIT_SPACE,
        seeds = [IDENTITY_SEED.as_bytes(), id.as_ref()],
        bump,
        constraint = !identity.is_banned @ StaykeError::IdentityBanned
    )]
    pub identity: Account<'info, Identity>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_user_profile(
    ctx: Context<InitializeUserProfile>,
    id: [u8; 32],
    country_code: [u8; 2],
    doctype: DocType,
) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.owner = ctx.accounts.authority.key();
    user_profile.identity = ctx.accounts.identity.key();
    user_profile.bump = ctx.bumps.user_profile;

    let reputation_profile = &mut ctx.accounts.reputation_profile;
    reputation_profile.owner = ctx.accounts.authority.key();
    reputation_profile.bump = ctx.bumps.reputation_profile;

    // This is just for initialization, we will update the identity data later when the user updates their profile.
    let identity = &mut ctx.accounts.identity;
    identity.owner = ctx.accounts.authority.key();
    identity.id = id;
    identity.doc_type = doctype;
    identity.country_code = country_code;
    identity.bump = ctx.bumps.identity;

    Ok(())
}
