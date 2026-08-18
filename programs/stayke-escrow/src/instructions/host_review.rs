use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::{REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    program::StaykeCore,
    ReputationProfile, UserProfile,
};

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::ReviewSubmitted,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Host: rate the guest after the booking is settled.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostReview<'info> {
    pub host: Signer<'info>,

    /// The host's UserProfile — validates the reviewer is the booking's host.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host.key() == host_profile.authority @ EscrowError::UnauthorizedHost,
        constraint = host_profile.key() == booking.host @ EscrowError::InvalidHostBooking,
    )]
    pub host_profile: Account<'info, UserProfile>,

    /// The guest's UserProfile — used to derive and validate the guest's
    /// ReputationProfile for the review CPI.
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ EscrowError::UnauthorizedBooking,
    )]
    pub guest_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_reputation.bump,
        constraint = guest_reputation.authority == guest_profile.authority @ EscrowError::UnauthorizedBooking,
    )]
    pub guest_reputation: Account<'info, ReputationProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host_review == 0 @ EscrowError::ReviewAlreadySubmitted,
        constraint = booking.status == BookingStatus::Completed
            || booking.status == BookingStatus::Released
            || booking.status == BookingStatus::DisputeResolved
            || booking.status == BookingStatus::DisputeRejected @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_host_review(ctx: Context<HostReview>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), EscrowError::InvalidScore);

    let booking = &mut ctx.accounts.booking;
    booking.host_review = score;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let review_accounts = stayke_core::cpi::accounts::UpdateClientReview {
        client_profile: ctx.accounts.guest_profile.to_account_info(),
        client_reputation: ctx.accounts.guest_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::update_client_review(
        CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            review_accounts,
            signer_seeds,
        ),
        score,
    )?;

    emit!(ReviewSubmitted {
        booking: booking.key(),
        reviewer: ctx.accounts.host.key(),
        rating: score,
        is_host_review: true,
    });

    Ok(())
}
