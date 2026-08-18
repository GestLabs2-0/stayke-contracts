use crate::{
    error::StaykeError, ReputationProfile, UserProfile, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED,
};
use anchor_lang::prelude::*;
use stayke_config::{
    cpi_authority::{assert_cpi_authority, AllowedCaller},
    GlobalConfig, GLOBAL_CONFIG_SEED,
};

#[derive(Accounts)]
pub struct UpdateClientReview<'info> {
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client_profile.authority.key().as_ref()],
        bump = client_profile.bump,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), client_profile.authority.key().as_ref()],
        bump = client_reputation.bump,
        constraint = client_reputation.authority == client_profile.authority @ StaykeError::Unauthorized,
    )]
    pub client_reputation: Account<'info, ReputationProfile>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_update_client_review(ctx: Context<UpdateClientReview>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), StaykeError::InvalidScore);
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;

    let reputation = &mut ctx.accounts.client_reputation;
    reputation.client_reviews = reputation.client_reviews.saturating_add(1);
    reputation.total_score_client = reputation.total_score_client.saturating_add(score as u64);

    Ok(())
}
