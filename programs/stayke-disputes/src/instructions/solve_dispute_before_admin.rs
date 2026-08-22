use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{state::UserProfile, USER_PROFILE_SEED};

use stayke_escrow::{
    constants::BOOKING_SEED,
    cpi::{accounts::UpdateBookingStatusCpi, cpi_update_booking_status},
    program::StaykeEscrow,
    state::Booking,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_P2P_WINDOW_SECONDS, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeSolved,
    state::{DisputeAccount, DisputeParty, DisputeState},
};

/// Withdraws an open P2P dispute before the 24-hour window expires.
///
/// Only the user who opened the dispute (guest or host) can invoke it.
/// Restores the booking to its pre-dispute status so the normal escrow
/// flow (e.g. release_funds) can proceed as if the dispute never existed.
#[derive(Accounts)]
pub struct SolveDisputeBeforeAdmin<'info> {
    #[account(mut)]
    pub initiator: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), initiator.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = initiator_profile.bump,
    )]
    pub initiator_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), dispute.booking.as_ref()],
        bump = dispute.bump,
    )]
    pub dispute: Box<Account<'info, DisputeAccount>>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        seeds::program = stayke_escrow::ID,
    )]
    pub booking: Box<Account<'info, Booking>>,

    /// CHECK: CPI authority PDA of an allowlisted Stayke program.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
}

pub fn handler_solve_dispute_before_admin(ctx: Context<SolveDisputeBeforeAdmin>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;

    require!(
        dispute.state == DisputeState::OpenP2P,
        DisputeError::DisputeNotOpenP2P
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        now <= dispute.opened_at.saturating_add(DISPUTE_P2P_WINDOW_SECONDS),
        DisputeError::P2PWindowElapsed
    );

    let initiator_profile_key = ctx.accounts.initiator_profile.key();
    let opener_profile = if dispute.opened_by == DisputeParty::Guest {
        ctx.accounts.booking.guest
    } else {
        ctx.accounts.booking.host
    };
    require!(
        initiator_profile_key == opener_profile,
        DisputeError::UnauthorizedDisputeSolver
    );

    dispute.state = DisputeState::ResolvedByP2P;

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    let cpi_accounts = UpdateBookingStatusCpi {
        booking: ctx.accounts.booking.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.stayke_escrow_program.key(),
        cpi_accounts,
        signer_seeds,
    );
    cpi_update_booking_status(cpi_ctx, dispute.original_booking_status.clone())?;

    emit!(DisputeSolved {
        dispute: dispute.key(),
        booking: dispute.booking,
        solved_by: dispute.opened_by.clone(),
        solved_at: now,
    });

    Ok(())
}
