
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



// TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand
// I think the best way to handle this all is by creating a global contract
#[derive(Accounts)]
pub struct ClearListingBooking<'info> {
    // CHECK: if the user_profile is the seed for the listing, then we can be sure that the listing is being modified either by:
    // - Our contracts
    // - Authority, but maybe this property of the listing should only be modified by the contracts and not the user
    // because we don't hosts clearing properties all the time. I know the account BookingDays make sure that we don't 
    // book in occupied days, but still I don't want this to be modified in any aside of the logic flow.
    pub user_profile: Box<UncheckedAccount<'info, UserProfile>>,

        #[account(
        mut, 
        seeds = [b"listing", user_profile.key().as_ref(), listing.listing_id.to_be_bytes().as_ref()], bump = listing.bump, 
        constraint = listing.owner == user_profile.key() @ StaykeError::Unauthorized,
    )]
    pub listing: Account<'info, Listing>,
    
    pub authority: Signer<'info>,
}

pub fn handle_clear_listing_bookig(ctx: Context<ClearActiveBooking>) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.is_occupied = None;

    Ok(())
}