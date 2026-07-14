use anchor_lang::prelude::*;

use crate::{error::StaykeError, Listing, UserProfile, LISTING_SEED, USER_PROFILE_SEED};

#[derive(Accounts)]
pub struct UpdateListing<'info> {
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_profile.bump,
        constraint = user_profile.identity.is_some() @StaykeError::UserProfileNotVerified,
        constraint = !user_profile.banned @ StaykeError::IdentityBanned,
        constraint = user_profile.authority == authority.key() @ StaykeError::Unauthorized
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing.listing_id.to_le_bytes().as_ref()],
        bump = listing.bump,
        constraint = listing.owner == user_profile.key() @ StaykeError::Unauthorized,
    )]
    pub listing: Account<'info, Listing>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

pub fn handler_update_listing_price(ctx: Context<UpdateListing>, price: u64) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.price = price;
    Ok(())
}

pub fn handler_update_listing_state(ctx: Context<UpdateListing>, state: [u8; 32]) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.state_hash = state;
    Ok(())
}
