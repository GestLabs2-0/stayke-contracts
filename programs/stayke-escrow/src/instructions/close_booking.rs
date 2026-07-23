use anchor_lang::prelude::*;
use stayke_core::{
    constants::{REPUTATION_PROFILE_SEED, USER_PROFILE_SEED},
    UserProfile,
};

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Close booking — leaves review, updates profile counters (via CPI to stayke-core)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseBooking<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_reputation.bump,
    )]
    pub host_reputation: Account<'info, stayke_core::ReputationProfile>,

    /// Host's ReputationProfile from stayke-core (receives score update).
    #[account(
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidHostBooking,
        constraint = booking.status == BookingStatus::Active @ EscrowError::BookingNotActive,
    )]
    pub booking: Account<'info, Booking>,
}

pub fn handler_review_completed(ctx: Context<CloseBooking>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), EscrowError::InvalidScore);

    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::ReviewCompleted;

    // Update reputation counters directly (no CPI needed — we hold the account).
    let reputation = &mut ctx.accounts.host_reputation;
    reputation.host_reviews += 1;
    reputation.total_score_host += score as u64;
    reputation.hosted_stays += 1;

    emit!(BookingStatusUpdated {
        status: BookingStatus::ReviewCompleted,
        booking: booking.key(),
    });

    Ok(())
}
