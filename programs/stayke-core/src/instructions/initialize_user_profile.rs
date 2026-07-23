use anchor_lang::prelude::*;

use crate::{ReputationProfile, UserProfile, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED};

#[derive(Accounts)]
pub struct InitializeUserProfile<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = payer, space = 8 + UserProfile::INIT_SPACE, seeds = [USER_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub user_profile: Account<'info, UserProfile>,
    #[account(init, payer = payer, space = 8 + ReputationProfile::INIT_SPACE, seeds = [REPUTATION_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub reputation_profile: Account<'info, ReputationProfile>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_user_profile(ctx: Context<InitializeUserProfile>) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.authority = ctx.accounts.authority.key();
    user_profile.bump = ctx.bumps.user_profile;

    let reputation_profile = &mut ctx.accounts.reputation_profile;
    reputation_profile.authority = ctx.accounts.authority.key();
    reputation_profile.bump = ctx.bumps.reputation_profile;

    Ok(())
}
