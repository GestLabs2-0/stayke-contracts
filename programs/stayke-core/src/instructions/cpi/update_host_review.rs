use crate::{
    error::StaykeError, ReputationProfile, UserProfile, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED,
};
use anchor_lang::prelude::*;
use stayke_config::{
    cpi_authority::{assert_cpi_authority, AllowedCaller},
    GlobalConfig, GLOBAL_CONFIG_SEED,
};

#[derive(Accounts)]
pub struct UpdateHostReview<'info> {
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        bump = host_profile.bump,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        bump = host_reputation.bump,
        constraint = host_reputation.authority == host_profile.authority @ StaykeError::Unauthorized,
    )]
    pub host_reputation: Account<'info, ReputationProfile>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_update_host_review(ctx: Context<UpdateHostReview>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), StaykeError::InvalidScore);
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;

    let reputation = &mut ctx.accounts.host_reputation;
    reputation.host_reviews = reputation.host_reviews.saturating_add(1);
    reputation.total_score_host = reputation.total_score_host.saturating_add(score as u64);
    reputation.hosted_stays = reputation.hosted_stays.saturating_add(1);
    Ok(())
}
