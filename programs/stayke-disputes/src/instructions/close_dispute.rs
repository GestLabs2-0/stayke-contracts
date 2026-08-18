use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    cpi::{
        accounts::{ClearListingBooking, UpdateUserProfile},
        clear_active_booking, clear_listing_booking,
    },
    program::StaykeCore,
    state::{Listing, UserProfile},
    LISTING_SEED, USER_PROFILE_SEED,
};

use stayke_escrow::{constants::BOOKING_SEED, state::Booking};

use crate::{
    constants::{DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    state::{Dispute, DisputeConfig, DisputeStatus},
};

#[derive(Accounts)]
pub struct CloseDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Box<Account<'info, DisputeConfig>>,

    #[account(
        mut,
        close = admin,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.status != DisputeStatus::Open @ DisputeError::DisputeNotOpen,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    /// Dispute PDA is seeded from this booking key.
    #[account(
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        seeds::program = stayke_escrow::ID,
        bump = booking.bump
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), guest_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = guest_profile.bump,
        constraint = guest_profile.key() == booking.guest @ DisputeError::UnboundBookingAccount,
    )]
    pub guest_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), host_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = host_profile.bump,
        constraint = host_profile.key() == booking.host @ DisputeError::UnboundBookingAccount,
    )]
    pub host_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [
            LISTING_SEED.as_bytes(),
            host_profile.key().as_ref(),
            listing.listing_id.to_le_bytes().as_ref(),
        ],
        seeds::program = stayke_core::ID,
        bump = listing.bump,
        constraint = listing.key() == booking.property @ DisputeError::UnboundBookingAccount,
    )]
    pub listing: Box<Account<'info, Listing>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let guest_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.guest_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    clear_active_booking(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        guest_cpi_accounts,
        signer_seeds,
    ))?;

    let host_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    clear_active_booking(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        host_cpi_accounts,
        signer_seeds,
    ))?;

    let clear_listing_accounts = ClearListingBooking {
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        listing: ctx.accounts.listing.to_account_info(),
        user_profile: ctx.accounts.host_profile.to_account_info(),
    };

    clear_listing_booking(CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        clear_listing_accounts,
        signer_seeds,
    ))?;

    Ok(())
}
