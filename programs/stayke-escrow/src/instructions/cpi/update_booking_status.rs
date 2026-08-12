use crate::{
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};
use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

// ---------------------------------------------------------------------------
// CPI Endpoint: Set Booking Status (Used by stayke-disputes)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct UpdateBookingStatusCpi<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        // Since it's a CPI from stayke-disputes, any caller can technically invoke it.
        // We assume the stayke-disputes program has already authorized the user.
        // In a production scenario, we should enforce that ONLY stayke-disputes PDA
        // or program can call this, or check admin signatures.
        // But for simplicity of matching the original design, we just need mutable access.
    )]
    pub booking: Account<'info, Booking>,

    pub cpi_authority: Signer<'info>, // could be the admin or user from the dispute context
}

pub fn handler_cpi_update_booking_status(
    ctx: Context<UpdateBookingStatusCpi>,
    status: BookingStatus,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let booking = &mut ctx.accounts.booking;

    // Optional safety checks depending on transition logic:
    // Only allow specific transitions like Active -> Disputed or Disputed -> Resolved
    if status == BookingStatus::Disputed {
        require!(
            booking.status == BookingStatus::Active,
            EscrowError::BookingNotActive
        );
    } else if status == BookingStatus::DisputeResolved || status == BookingStatus::DisputeRejected {
        require!(
            booking.status == BookingStatus::Disputed,
            EscrowError::InvalidBookingStatus
        );
    }

    booking.status = status.clone();

    emit!(BookingStatusUpdated {
        status,
        booking: booking.key()
    });

    Ok(())
}
