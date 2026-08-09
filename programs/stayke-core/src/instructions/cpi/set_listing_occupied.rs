use crate::{
    constants::LISTING_SEED,
    error::StaykeError,
    state::{Listing, UserProfile},
};
use anchor_lang::prelude::*;
use stayke_config::{
    cpi_authority::{assert_cpi_authority, AllowedCaller},
    GlobalConfig, GLOBAL_CONFIG_SEED,
};

#[derive(Accounts)]
pub struct SetListingOccupied<'info> {
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing.listing_id.to_le_bytes().as_ref()],
        bump = listing.bump,
        constraint = listing.owner == user_profile.key() @ StaykeError::Unauthorized,
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

pub fn handler_set_listing_occupied(
    ctx: Context<SetListingOccupied>,
    occupied: bool,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;

    ctx.accounts.listing.is_occupied = occupied;
    Ok(())
}
