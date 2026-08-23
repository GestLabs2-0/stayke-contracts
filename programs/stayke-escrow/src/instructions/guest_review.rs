use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::{LISTING_SEED, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    program::StaykeCore,
    Listing, ReputationProfile, UserProfile,
};

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::ReviewSubmitted,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Guest: rate the host after the booking is settled.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct GuestReview<'info> {
    pub guest: Signer<'info>,

    /// The guest's UserProfile — validates the reviewer is the booking's guest.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest.key() == guest_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = guest_profile.key() == booking.guest @ EscrowError::UnauthorizedBooking,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    /// The host's UserProfile — used to derive and validate the host's
    /// ReputationProfile for the review CPI.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_reputation.bump,
        constraint = host_reputation.authority == host_profile.authority @ EscrowError::InvalidHostBooking,
    )]
    pub host_reputation: Box<Account<'info, ReputationProfile>>,

    /// The listing being reviewed — mutable to accumulate `total_reviews` and
    /// `rating` via the stayke-core CPI.
    #[account(
        mut,
        constraint = listing.key() == booking.property @ EscrowError::InvalidBookingProperty,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.guest_review == 0 @ EscrowError::ReviewAlreadySubmitted,
        constraint = booking.status == BookingStatus::Completed
            || booking.status == BookingStatus::Released
            || booking.status == BookingStatus::DisputeResolved
            || booking.status == BookingStatus::DisputeRejected @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_guest_review(ctx: Context<GuestReview>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), EscrowError::InvalidScore);

    let (expected_listing, _) = Pubkey::find_program_address(
        &[
            LISTING_SEED.as_bytes(),
            ctx.accounts.host_profile.key().as_ref(),
            &ctx.accounts.listing.listing_id.to_le_bytes(),
        ],
        &stayke_core::ID,
    );

    require!(
        ctx.accounts.listing.key() == expected_listing,
        EscrowError::InvalidListing
    );

    let booking = &mut ctx.accounts.booking;
    booking.guest_review = score;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let review_accounts = stayke_core::cpi::accounts::UpdateHostReview {
        host_profile: ctx.accounts.host_profile.to_account_info(),
        host_reputation: ctx.accounts.host_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::update_host_review(
        CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            review_accounts,
            signer_seeds,
        ),
        score,
    )?;

    let listing_accounts = stayke_core::cpi::accounts::UpdateListingReview {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        listing: ctx.accounts.listing.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::update_listing_review(
        CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            listing_accounts,
            signer_seeds,
        ),
        score,
        ctx.accounts.guest.key(),
    )?;

    emit!(ReviewSubmitted {
        booking: booking.key(),
        reviewer: ctx.accounts.guest.key(),
        rating: score,
        is_host_review: false,
    });

    Ok(())
}
