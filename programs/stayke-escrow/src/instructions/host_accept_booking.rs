use anchor_lang::prelude::*;
use stayke_core::{constants::USER_PROFILE_SEED, UserProfile};

use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// ---------------------------------------------------------------------------
// Host: accept pending booking
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HostAcceptBooking<'info> {
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
        constraint = host_profile.deposited >= global_config.minimum_deposit @ EscrowError::InsufficientDeposit,
    )]
    pub host_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidBookingProperty,
        constraint = booking.status == BookingStatus::Pending @ EscrowError::InvalidBookingStatus,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,
}

pub fn handler_host_accept_booking(ctx: Context<HostAcceptBooking>) -> Result<()> {
    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::HostAccepted;
    emit!(BookingStatusUpdated {
        status: BookingStatus::HostAccepted,
        booking: booking.key()
    });
    Ok(())
}
