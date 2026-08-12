use anchor_lang::prelude::*;

use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
};

// TODO: change reputation if client rejects inside a 48 hours frame

// ---------------------------------------------------------------------------
// Client: reject reservation (closes booking, releases days)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(check_in: i64)]
pub struct ClientRejectReserve<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.identity.is_some() @ EscrowError::UserNotVerified,
    )]
    pub client_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        close = client,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = (booking.status == BookingStatus::HostAccepted || booking.status == BookingStatus::Pending) @ EscrowError::InvalidBookingStatus,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.check_in == check_in @ EscrowError::DatesUnbooked
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), check_in.year().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,
}

pub fn handler_client_reject_reserve(
    ctx: Context<ClientRejectReserve>,
    check_in: i64,
) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;

    let start_date = derive_date(check_in);
    let end_date = derive_date(booking.check_out);

    release_days_single_year(&mut ctx.accounts.booking_days, &start_date, &end_date)?;
    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(check_in: i64)]
pub struct ClientRejectReserveCrossYear<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub client: Signer<'info>,
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), client.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = client_profile.bump,
        constraint = client.key() == client_profile.authority @ EscrowError::UnauthorizedBooking,
        constraint = !client_profile.banned @ EscrowError::UserBanned,
        constraint = client_profile.identity.is_some() @ EscrowError::UserNotVerified,
    )]
    pub client_profile: Account<'info, UserProfile>,
    #[account(
        mut,
        close = client,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = (booking.status == BookingStatus::HostAccepted || booking.status == BookingStatus::Pending) @ EscrowError::InvalidBookingStatus,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.check_in == check_in @ EscrowError::DatesUnbooked
    )]
    pub booking: Account<'info, Booking>,
    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), check_in.year().to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days: Account<'info, BookingDays>,
    #[account(
        mut,
        seeds = [BOOKING_DAYS_SEED.as_bytes(), booking.property.as_ref(), (check_in.year() + 1).to_le_bytes().as_ref()],
        bump,
    )]
    pub booking_days_next: Account<'info, BookingDays>,
}

pub fn handler_client_reject_reserve_cross_year(
    ctx: Context<ClientRejectReserveCrossYear>,
    check_in: i64,
) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::Cancelled;

    let start_date = derive_date(check_in);
    let end_date = derive_date(booking.check_out);

    release_days_cross_years(
        &mut ctx.accounts.booking_days,
        &mut ctx.accounts.booking_days_next,
        &start_date,
        &end_date,
    )?;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Cancelled,
        booking: booking.key()
    });

    Ok(())
}
