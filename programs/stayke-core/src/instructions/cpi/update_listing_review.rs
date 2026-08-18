use crate::{
    constants::LISTING_SEED,
    error::StaykeError,
    events::ListingReviewSubmitted,
    state::{Listing, UserProfile},
};
use anchor_lang::prelude::*;
use stayke_config::{
    cpi_authority::{assert_cpi_authority, AllowedCaller},
    GlobalConfig, GLOBAL_CONFIG_SEED,
};

#[derive(Accounts)]
pub struct UpdateListingReview<'info> {
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing.listing_id.to_le_bytes().as_ref()],
        bump = listing.bump,
    )]
    pub listing: Account<'info, Listing>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

/// CPI-gated instruction: accumulates a guest review into the listing's
/// `total_reviews` and `rating` (a running sum — the average is derived
/// off-chain as `rating / total_reviews`). Only the Escrow program may call
/// this, when a guest reviews a listing after their stay is settled.
pub fn handler_update_listing_review(
    ctx: Context<UpdateListingReview>,
    score: u8,
    reviewer: Pubkey,
) -> Result<()> {
    require!((1..=5).contains(&score), StaykeError::InvalidScore);
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;

    let listing = &mut ctx.accounts.listing;
    listing.total_reviews = listing.total_reviews.saturating_add(1);
    listing.rating = listing.rating.saturating_add(score as u64);

    emit!(ListingReviewSubmitted {
        listing: listing.key(),
        reviewer,
        score,
        total_reviews: listing.total_reviews,
    });

    Ok(())
}
