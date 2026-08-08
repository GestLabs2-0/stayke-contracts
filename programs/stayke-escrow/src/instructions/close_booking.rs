use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{
    constants::{REPUTATION_PROFILE_SEED, USER_PROFILE_SEED, CPI_AUTHORITY_SEED},
    cpi::accounts::UpdateHostReview,
    program::StaykeCore,
    ReputationProfile, UserProfile,
};

use crate::{
    constants::BOOKING_SEED,
    error::EscrowError,
    events::BookingStatusUpdated,
    state::{Booking, BookingStatus},
};

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

    /// Host UserProfile (correct seed family).
    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
    )]
    pub host_profile: Account<'info, UserProfile>,

    /// Host ReputationProfile (correct seed family).
    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_reputation.bump,
    )]
    pub host_reputation: Account<'info, ReputationProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        constraint = booking.guest == client_profile.key() @ EscrowError::UnauthorizedBooking,
        constraint = booking.host == host_profile.key() @ EscrowError::InvalidHostBooking,
        constraint = booking.status == BookingStatus::Active @ EscrowError::BookingNotActive,
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Escrow CPI authority PDA — signs privileged core mutators (A2).
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_review_completed(ctx: Context<CloseBooking>, score: u8) -> Result<()> {
    require!((1..=5).contains(&score), EscrowError::InvalidScore);

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let review_accounts = UpdateHostReview {
        host_profile: ctx.accounts.host_profile.to_account_info(),
        host_reputation: ctx.accounts.host_reputation.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::update_host_review(
        CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            review_accounts,
            signer_seeds,
        ),
        score,
    )?;

    let booking = &mut ctx.accounts.booking;
    booking.status = BookingStatus::ReviewCompleted;

    emit!(BookingStatusUpdated {
        status: BookingStatus::ReviewCompleted,
        booking: booking.key(),
    });

    Ok(())
}
