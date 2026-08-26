use crate::{
    constants::{CORE_CONFIG_SEED, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    error::StaykeError,
    state::{ConfigAcc, ReputationProfile, UserProfile},
    UserBanned, MAX_HIGH_INFRACTIONS, MAX_LOW_INFRACTIONS, MAX_MID_INFRACTIONS,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct BanUser<'info> {
    #[account(constraint = config.authority == authority.key() @ StaykeError::Unauthorized)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.authority.as_ref()],
        bump = user_profile.bump,
        constraint = !user_profile.banned @ StaykeError::UserAlreadyBanned
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), user_profile.authority.as_ref()],
        bump = reputation_profile.bump,
        constraint = user_profile.authority == reputation_profile.authority @ StaykeError::Unauthorized
    )]
    pub reputation_profile: Account<'info, ReputationProfile>,

    #[account(
        seeds = [CORE_CONFIG_SEED.as_bytes()],
        bump = config.bump
    )]
    pub config: Account<'info, ConfigAcc>,
}

// TODO: what would happen if by any reason the user still has deposited money?
// Should we return the money depending on the type of ban?
// Maybe if user surpassed HIGH_INFRACTIONS stayke should absorbe all deposit or store in somekind of treasury
// Maybe if user surpassed MAX_MID_INFRACTIONS or MAX_LOW_INFRACTIONS stayke should return an slash of the deposit
pub fn handle_ban_user(ctx: Context<BanUser>) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    let reputation_profile = &ctx.accounts.reputation_profile;

    if reputation_profile.high_infractions >= MAX_HIGH_INFRACTIONS
        || reputation_profile.medium_infractions >= MAX_MID_INFRACTIONS
        || reputation_profile.low_infractions >= MAX_LOW_INFRACTIONS
    {
        user_profile.banned = true;
        emit!(UserBanned {
            identity: user_profile.identity,
            reputation_profile: reputation_profile.key(),
            user_profile: user_profile.key()
        })
    }
    Ok(())
}
