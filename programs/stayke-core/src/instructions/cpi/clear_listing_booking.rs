use crate::{
    constants::LISTING_SEED,
    error::StaykeError,
    state::{Listing, UserProfile},
};
use anchor_lang::prelude::*;

// TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand
// I think the best way to handle this all is by creating a global contract
#[derive(Accounts)]
pub struct ClearListingBooking<'info> {
    // TODO: if the user_profile is the seed for the listing, then we can be sure that the listing is being modified either by:
    // - Our contracts
    // - Authority, but maybe this property of the listing should only be modified by the contracts and not the user
    // because we don't hosts clearing properties all the time. I know the account BookingDays make sure that we don't
    // book in occupied days, but still I don't want this to be modified in any aside of the logic flow.
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing.listing_id.to_le_bytes().as_ref()],
        bump = listing.bump,
        constraint = listing.owner == user_profile.key() @ StaykeError::Unauthorized,
    )]
    pub listing: Account<'info, Listing>,

    pub authority: Signer<'info>,
}

pub fn handle_clear_listing_booking(ctx: Context<ClearListingBooking>) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    listing.is_occupied = false;

    Ok(())
}
