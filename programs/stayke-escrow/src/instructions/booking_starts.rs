use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{
    cpi::{accounts::UpdateUserProfile, set_active_booking},
    program::StaykeCore,
    UserProfile, USER_PROFILE_SEED,
};

use crate::{
    constants::{BOOKING_SEED, CPI_AUTHORITY_SEED},
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

// TODO: define what will happen if guest is banned

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

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), guest.authority.as_ref()],
        bump = guest.bump,
        seeds::program = stayke_core::ID,
        constraint = booking.guest == guest.key() @ EscrowError::WrongGuestPassed
    )]
    pub guest: Account<'info, UserProfile>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators.
    #[account(seeds=[CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub stayke_core: Program<'info, StaykeCore>,
}

pub fn handler_booking_starts(ctx: Context<BookingStarts>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let booking = &mut ctx.accounts.booking;

    require!(now >= booking.check_in, EscrowError::TooEarlyToActivate);

    booking.status = BookingStatus::Active;
    booking.updated_at = now;

    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]]];
    let update_ctx = UpdateUserProfile {
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        user_profile: ctx.accounts.guest.to_account_info(),
    };

    set_active_booking(
        CpiContext::new_with_signer(ctx.accounts.stayke_core.key(), update_ctx, signer_seeds),
        booking.key(),
    )?;

    emit!(BookingStatusUpdated {
        status: BookingStatus::Active,
        booking: booking.key()
    });

    Ok(())
}
