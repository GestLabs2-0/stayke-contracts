use anchor_lang::prelude::*;

use crate::{error::StaykeError, Listing, UserProfile, LISTING_SEED, USER_PROFILE_SEED};

#[derive(Accounts)]
#[instruction(price: u64, listing_id: u16)]
pub struct InitializeListing<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub authority: Signer<'info>,

    #[account(init,
            payer = payer,
            space = 8 + Listing::INIT_SPACE,
            seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing_id.to_le_bytes().as_ref()],
            bump
        )]
    pub listing: Account<'info, Listing>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.authority.key().as_ref()],
        bump = user_profile.bump,
        constraint = user_profile.identity.is_some() @StaykeError::UserProfileNotVerified,
        constraint = !user_profile.banned @ StaykeError::IdentityBanned,
        constraint = user_profile.authority == authority.key() @ StaykeError::Unauthorized,
        constraint = user_profile.listings == listing_id @ StaykeError::InvalidListingId,
    )]
    pub user_profile: Account<'info, UserProfile>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_listing(
    ctx: Context<InitializeListing>,
    price: u64,
    listing_id: u16,
) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    let user_profile = &mut ctx.accounts.user_profile;

    listing.owner = user_profile.key();
    listing.listing_id = listing_id;
    listing.price = price;

    user_profile.listings += 1;

    Ok(())
}
