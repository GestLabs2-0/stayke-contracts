use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use stayke_core::{state::UserProfile, USER_PROFILE_SEED};

use stayke_escrow::{
    constants::BOOKING_SEED,
    cpi::{accounts::UpdateBookingStatusCpi, cpi_update_booking_status},
    program::StaykeEscrow,
    state::Booking,
    BookingStatus,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeOpened,
    state::{DisputeAccount, DisputeParty, DisputeState},
};

#[derive(Accounts)]
pub struct OpenDispute<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub initiator: Signer<'info>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), initiator.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = initiator_profile.bump,
        constraint = !initiator_profile.banned @ DisputeError::UserBanned,
        constraint = initiator_profile.identity.is_some() @ DisputeError::UserNotVerified,
    )]
    pub initiator_profile: Account<'info, UserProfile>,

    #[account(
        mut,
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        seeds::program = stayke_escrow::ID,
        constraint = booking.status == BookingStatus::Active || booking.status == BookingStatus::Completed @ DisputeError::BookingNotDisputable
    )]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        init,
        payer = payer,
        space = 8 + DisputeAccount::INIT_SPACE,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
    )]
    pub dispute: Box<Account<'info, DisputeAccount>>,

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
    pub system_program: Program<'info, System>,
}

pub fn handler_open_dispute(ctx: Context<OpenDispute>) -> Result<()> {
    let initiator_profile_key = ctx.accounts.initiator_profile.key();
    require!(
        ctx.accounts.booking.guest == initiator_profile_key
            || ctx.accounts.booking.host == initiator_profile_key,
        DisputeError::UnauthorizedDisputeInitiator
    );

    let opened_by = if ctx.accounts.booking.guest == initiator_profile_key {
        DisputeParty::Guest
    } else {
        DisputeParty::Host
    };
    let opened_at = Clock::get()?.unix_timestamp;

    let dispute = &mut ctx.accounts.dispute;
    dispute.booking = ctx.accounts.booking.key();
    dispute.opened_by = opened_by.clone();
    dispute.state = DisputeState::OpenP2P;
    dispute.opened_at = opened_at;
    dispute.guest_evidence = None;
    dispute.host_evidence = None;
    dispute.outcome = None;
    dispute.bump = ctx.bumps.dispute;

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
    cpi_update_booking_status(cpi_ctx, BookingStatus::Disputed)?;

    emit!(DisputeOpened {
        dispute: dispute.key(),
        booking: dispute.booking,
        opened_by,
        opened_at,
    });

    Ok(())
}
