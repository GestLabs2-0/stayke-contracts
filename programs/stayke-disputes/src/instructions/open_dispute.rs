use anchor_lang::prelude::*;
use stayke_core::{state::UserProfile, USER_PROFILE_SEED};

use stayke_escrow::{
    cpi::{accounts::UpdateBookingStatusCpi, cpi_update_booking_status},
    program::StaykeEscrow,
    state::Booking,
    BookingStatus,
};

use crate::{
    constants::DISPUTE_PDA_SEED,
    error::DisputeError,
    events::DisputeOpened,
    state::{Dispute, DisputeReason, DisputeStatus},
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

    #[account(mut)]
    pub booking: Box<Account<'info, Booking>>,

    #[account(
        init,
        payer = payer,
        space = 8 + Dispute::INIT_SPACE,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    pub stayke_escrow_program: Program<'info, StaykeEscrow>,
    pub system_program: Program<'info, System>,
}

pub fn handler_open_dispute(ctx: Context<OpenDispute>, reason: DisputeReason) -> Result<()> {
    let initiator_profile_key = ctx.accounts.initiator_profile.key();
    require!(
        ctx.accounts.booking.guest == initiator_profile_key
            || ctx.accounts.booking.host == initiator_profile_key,
        DisputeError::UnauthorizedDisputeInitiator
    );
    require!(
        ctx.accounts.booking.status == BookingStatus::Active,
        DisputeError::BookingNotActive
    );

    let booking = &ctx.accounts.booking;

    // Guilty at open = accused counterparty (non-initiator).
    let guilty = guilty_counterparty(initiator_profile_key, booking.guest, booking.host)?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.booking = ctx.accounts.booking.key();
    dispute.property = ctx.accounts.booking.property;
    dispute.initiator = initiator_profile_key;
    dispute.guilty = guilty;
    dispute.reason = reason.clone();
    dispute.status = DisputeStatus::Open;
    dispute.created_at = Clock::get()?.unix_timestamp;
    dispute.resolved_at = None;
    dispute.bump = ctx.bumps.dispute;

    let cpi_accounts = UpdateBookingStatusCpi {
        booking: ctx.accounts.booking.to_account_info(),
        authority: ctx.accounts.initiator.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.stayke_escrow_program.key(), cpi_accounts);
    cpi_update_booking_status(cpi_ctx, BookingStatus::Disputed)?;

    emit!(DisputeOpened {
        dispute: dispute.key(),
        booking: dispute.booking,
        property: dispute.property,
        initiator: dispute.initiator,
        reason,
        timestamp: dispute.created_at,
    });

    Ok(())
}

/// Accused counterparty PDA at open: guest opens → host; host opens → guest.
pub(crate) fn guilty_counterparty(
    initiator_profile: Pubkey,
    guest: Pubkey,
    host: Pubkey,
) -> Result<Pubkey> {
    require!(
        initiator_profile == guest || initiator_profile == host,
        DisputeError::UnauthorizedDisputeInitiator
    );
    Ok(if initiator_profile == guest {
        host
    } else {
        guest
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_open_sets_guilty_to_host() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        assert_eq!(guilty_counterparty(guest, guest, host).unwrap(), host);
    }

    #[test]
    fn host_open_sets_guilty_to_guest() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        assert_eq!(guilty_counterparty(host, guest, host).unwrap(), guest);
    }

    #[test]
    fn non_party_open_fails() {
        let guest = Pubkey::new_unique();
        let host = Pubkey::new_unique();
        let stranger = Pubkey::new_unique();
        assert!(guilty_counterparty(stranger, guest, host).is_err());
    }
}
