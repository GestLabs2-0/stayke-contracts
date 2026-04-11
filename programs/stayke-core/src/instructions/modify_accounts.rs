
use anchor_lang::prelude::*;

use crate::{Listing, UserProfile, error::StaykeError};

#[derive(Accounts)]
pub struct UpdateListing<'info> {
     #[account( 
        seeds = [b"user_profile", user_profile.owner.key().as_ref()], 
        bump = user_profile.bump, 
        constraint = user_profile.is_verified == true @ StaykeError::UserProfileNotVerified,
        constraint = user_profile.banned == false @ StaykeError::IdentityBanned,
        constraint = user_profile.owner == authority.key() @ StaykeError::Unauthorized
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut, 
        seeds = [b"listing", user_profile.key().as_ref(), listing.listing_id.to_be_bytes().as_ref()], bump = listing.bump, 
        constraint = listing.owner == user_profile.key() @ StaykeError::Unauthorized,
    )]
    pub listing: Account<'info, Listing>,
    
    pub authority: Signer<'info>,
}

pub fn handler_update_listing_price(ctx: Context<UpdateListing>, price: u64) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.price = price;
    Ok(())
}

pub fn handler_update_listing_state(ctx: Context<UpdateListing>,  state: [u8; 32]) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.state_hash = state;
    Ok(())
}


