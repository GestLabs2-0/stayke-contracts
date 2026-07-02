use anchor_lang::prelude::*;

use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::release_days,
};

// TODO: change reputation if client rejects inside a 48 hours frame

// ---------------------------------------------------------------------------
// Client: reject reservation (closes booking, releases days)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ClientRejectReserve<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.owner @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.is_verified @ EscrowError::UserNotVerified,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        close = client,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = (booking.status == BookingStatus::HostAccepted || booking.status == BookingStatus::Pending) @ EscrowError::InvalidBookingStatus,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), booking_days.year_month().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,
}

pub fn handler_client_reject_reserve(ctx: Context<ClientRejectReserve>) -> Result<()> {
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
