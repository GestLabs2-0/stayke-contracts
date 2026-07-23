use anchor_lang::prelude::*;
use stayke_core::{
    cpi::{
        accounts::{ClearListingBooking, UpdateUserProfile},
        clear_active_booking, clear_listing_booking,
    },
    program::StaykeCore,
    state::{Listing, UserProfile},
};

use stayke_escrow::state::Booking;

use crate::{
    constants::{DISPUTE_CONFIG_PDA_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    state::{Dispute, DisputeConfig, DisputeStatus},
};

// ---------------------------------------------------------------------------
// Close Dispute (Closes dispute account and clears active_booking flags)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseDispute<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [DISPUTE_CONFIG_PDA_SEED.as_bytes()],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Account<'info, DisputeConfig>,

    #[account(
        mut,
        close = admin,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.status != DisputeStatus::Open @ DisputeError::DisputeNotOpen,
    )]
    pub dispute: Account<'info, Dispute>,

    /// CHECK: Read-only access here, trust it's the right booking if the dispute seed matches.
    pub booking: Account<'info, Booking>,

    // Note: The caller MUST pass the exact UserProfiles corresponding to the guest and host
    // of this booking, as well as the listing. We cannot enforce seeds entirely off-chain
    // unless we bring stayke-core profiles directly into scope but the instructions exist to do it dynamically.
    // So we just take mutable UserProfile accounts and let the caller invoke `clear_active_booking`.

    // Instead of forcing all 3 in the main struct if they are not always needed, we could use them directly.
    #[account(mut)]
    pub guest_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub host_profile: Account<'info, UserProfile>,

    #[account(mut)]
    pub listing: Account<'info, Listing>,

    // Currently we mapped Listing to have active_booking in the monolith, wait: In our new core design, Listing doesn't have active_booking, it has `is_occupied: Option<Pubkey>`.
    // We didn't create a mutator for `is_occupied`. Let's just clear the users since user_profile has `active_booking` and `active_stay`!
    pub stayke_core_program: Program<'info, StaykeCore>,
}

// TODO: instead of the admin users, we must only use the account PDA as the signer, but for simplicity we can just use the admin signer for now.
// We just need to make sure that only the admin can call this instruction, which is already enforced by the account constraint.
pub fn handler_close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
    // 1. Clear Guest's active booking
    let guest_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.guest_profile.to_account_info(),
        authority: ctx.accounts.admin.to_account_info(),
    };
    clear_active_booking(CpiContext::new(
        ctx.accounts.stayke_core_program.key(),
        guest_cpi_accounts,
    ))?;

    // 2. Clear Host's active stay
    let host_cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.host_profile.to_account_info(),
        authority: ctx.accounts.admin.to_account_info(),
    };
    clear_active_booking(CpiContext::new(
        ctx.accounts.stayke_core_program.key(),
        host_cpi_accounts,
    ))?;

    let clear_listing_accounts = ClearListingBooking {
        authority: ctx.accounts.admin.to_account_info(),
        listing: ctx.accounts.listing.to_account_info(),
        user_profile: ctx.accounts.host_profile.to_account_info(), // We need the host profile to check if the host has an active stay that matches the listing before clearing the listing's active booking
    };

    clear_listing_booking(CpiContext::new(
        ctx.accounts.stayke_core_program.key(),
        clear_listing_accounts,
    ))?;

    Ok(())
}
