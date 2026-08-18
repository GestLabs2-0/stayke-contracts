use anchor_lang::prelude::*;

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Permissionless: complete an active booking once check-out time is reached
// ---------------------------------------------------------------------------

/// Transitions a booking from `Active` to `Completed` when the check-out
/// timestamp is reached.
///
/// Anyone can call this. It moves no funds. It records `updated_at` as the
/// start of the two parallel post-stay windows: 24 h for `release_funds` and
/// 72 h for reviews.
#[derive(Accounts)]
pub struct BookingCompletes<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::Active @ EscrowError::BookingNotActive,
    )]
    pub booking: Account<'info, Booking>,
}

pub fn handler_booking_completes(ctx: Context<BookingCompletes>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    require!(now >= booking.check_out, EscrowError::TooEarlyToComplete);

    booking.status = BookingStatus::Completed;
    booking.updated_at = now;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Completed,
        booking: booking.key()
    });

    Ok(())
}
