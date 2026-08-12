use anchor_lang::prelude::*;

use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use crate::{
    constants::{BOOKING_DAYS_SEED, BOOKING_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingDays, BookingStatus},
    utils::{derive_date, release_days_cross_years, release_days_single_year, TimestampExt},
};

// TODO: change reputation if host rejects inside a 48 hours frame

// ---------------------------------------------------------------------------
// Host: reject pending booking (releases days, closes booking account)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(check_in: i64)]
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

pub fn handler_host_reject_booking(ctx: Context<HostRejectBooking>, check_in: i64) -> Result<()> {
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
pub struct HostRejectBookingCrossYear<'info> {
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

pub fn handler_host_reject_booking_cross_year(
    ctx: Context<HostRejectBookingCrossYear>,
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
