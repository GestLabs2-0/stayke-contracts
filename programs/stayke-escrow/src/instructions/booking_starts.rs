use anchor_lang::prelude::*;

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Permissionless: start an accepted booking once check-in time is reached
// ---------------------------------------------------------------------------

/// Transitions a booking from `HostAccepted` to `Active` when the check-in
/// timestamp is reached.
///
/// Anyone can call this — the program validates the conditions and executes
/// when they hold. It moves no funds and only refreshes `updated_at`. From
/// this point on the booking can no longer be cancelled.
#[derive(Accounts)]
pub struct BookingStarts<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.status == BookingStatus::HostAccepted @ EscrowError::BookingNotAccepted,
    )]
    pub booking: Account<'info, Booking>,
}

pub fn handler_booking_starts(ctx: Context<BookingStarts>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    require!(now >= booking.check_in, EscrowError::TooEarlyToActivate);

    booking.status = BookingStatus::Active;
    booking.updated_at = now;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Active,
        booking: booking.key()
    });

    Ok(())
}
