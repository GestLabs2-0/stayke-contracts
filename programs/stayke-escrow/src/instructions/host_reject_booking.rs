use anchor_lang::prelude::*;

use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::release_days,
};

// TODO: change reputation if host rejects inside a 48 hours frame

// ---------------------------------------------------------------------------
// Host: reject pending booking (releases days, closes booking account)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostRejectBooking<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub host: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host.key() == host_profile.authority @ EscrowError::UnauthorizedHost,
        constraint = !host_profile.banned @ EscrowError::UserBanned,
        constraint = host_profile.identity.is_some() @ EscrowError::UserNotVerified,
    )]
    pub host_profile: Account<'info, UserProfile>,

    // TODO: check if lamports for closing booking goes to payer
    /// CHECK: The guest wallet — receives the rent lamports from the closed booking account.
    #[account(mut, constraint = guest.key() == booking.guest @ EscrowError::WrongGuestPassed)]
    pub guest: UncheckedAccount<'info>,

    #[account(
        mut,
        close = guest,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking_days.property.as_ref(), booking_days.year_month().to_le_bytes().as_ref()],
        bump,
        constraint = booking_days.property == booking.property @ EscrowError::InvalidBookingDaysAccount,
    )]
    pub booking_days: Account<'info, BookingDays>,
}

pub fn handler_host_reject_booking(ctx: Context<HostRejectBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;
    release_days(
        ctx.remaining_accounts,
        booking.property,
        &mut ctx.accounts.booking_days,
        &booking.check_in_date.clone(),
        &booking.check_out_date.clone(),
    )?;
    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });
    Ok(())
}
