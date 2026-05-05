use anchor_lang::prelude::*;

use crate::{
    config::ConfigAcc, error::StaykeError, DocType, Identity, Listing, ReputationProfile,
    UserProfile, CORE_CONFIG_SEED, IDENTITY_SEED, LISTING_SEED, REPUTATION_PROFILE_SEED,
    USER_PROFILE_SEED,
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(init, payer = authority, space = 8 + ConfigAcc::INIT_SPACE, seeds = [CORE_CONFIG_SEED.as_bytes()], bump)]
    pub config: Account<'info, ConfigAcc>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.bump = ctx.bumps.config;
    Ok(())
}

#[derive(Accounts)]
#[instruction(id: [u8; 32])]
pub struct InitializeUserProfile<'info> {
    #[account(init, payer = authority, space = 8 + UserProfile::INIT_SPACE, seeds = [USER_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub user_profile: Account<'info, UserProfile>,
    #[account(init, payer = authority, space = 8 + ReputationProfile::INIT_SPACE, seeds = [REPUTATION_PROFILE_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub reputation_profile: Account<'info, ReputationProfile>,
    #[account(
        init_if_needed, 
        payer = authority, 
        space = 8 + Identity::INIT_SPACE, 
        seeds = [IDENTITY_SEED.as_bytes(), id.as_ref()], 
        bump,
        constraint = identity.is_banned == false @ StaykeError::IdentityBanned
    )]
    pub identity: Account<'info, Identity>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_user_profile(
    ctx: Context<InitializeUserProfile>,
    id: [u8; 32],
    country_code: [u8; 2],
    doctype: DocType,
) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.owner = ctx.accounts.authority.key();
    user_profile.identity = ctx.accounts.identity.key();
    user_profile.bump = ctx.bumps.user_profile;

    let reputation_profile = &mut ctx.accounts.reputation_profile;
    reputation_profile.owner = ctx.accounts.authority.key();
    reputation_profile.bump = ctx.bumps.reputation_profile;

    // This is just for initialization, we will update the identity data later when the user updates their profile.
    let identity = &mut ctx.accounts.identity;
    identity.owner = ctx.accounts.authority.key();
    identity.id = id;
    identity.doc_type = doctype;
    identity.country_code = country_code;
    identity.bump = ctx.bumps.identity;

    Ok(())
}

#[derive(Accounts)]
#[instruction(price: u64, listing_id: u16)]
pub struct InitializeListing<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(init, 
            payer = authority, 
            space = 8 + Listing::INIT_SPACE, 
            seeds = [LISTING_SEED.as_bytes(), user_profile.key().as_ref(), listing_id.to_le_bytes().as_ref()], 
            bump
        )]
    pub listing: Account<'info, Listing>,

    #[account( 
        seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.owner.key().as_ref()], 
        bump = user_profile.bump, 
        constraint = user_profile.is_verified == true @ StaykeError::UserProfileNotVerified,
        constraint = user_profile.banned == false @ StaykeError::IdentityBanned,
        constraint = user_profile.owner == authority.key() @ StaykeError::Unauthorized,
        constraint = user_profile.listings == listing_id @ StaykeError::InvalidListingId,
    )]
    pub user_profile: Account<'info, UserProfile>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_listing(ctx: Context<InitializeListing>, price: u64, listing_id: u16) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    let user_profile = &mut ctx.accounts.user_profile;

    listing.owner = user_profile.key();
    listing.listing_id = listing_id;
    listing.price = price;

    user_profile.listings += 1;

    Ok(())
}
