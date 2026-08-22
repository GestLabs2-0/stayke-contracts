use anchor_lang::prelude::*;

use stayke_escrow::{constants::BOOKING_SEED, state::Booking};

use stayke_core::{constants::USER_PROFILE_SEED, state::UserProfile};

use crate::{
    constants::DISPUTE_PDA_SEED,
    error::DisputeError,
    events::EvidenceLinked,
    state::{DisputeAccount, DisputeParty, DisputeState},
};

#[derive(Accounts)]
pub struct LinkEvidence<'info> {
    // Initiator or guilty
    pub signer: Signer<'info>,

    #[account(
        seeds = [DISPUTE_PDA_SEED.as_bytes(), booking.key().as_ref()],
        bump = dispute.bump,
        constraint = dispute.state == DisputeState::Escalated @ DisputeError::DisputeNotEscalated
    )]
    pub dispute: Account<'info, DisputeAccount>,

    #[account(
        seeds = [BOOKING_SEED.as_bytes(), booking.property.as_ref(), booking.guest.as_ref(), booking.check_in.to_le_bytes().as_ref()],
        bump = booking.bump,
        seeds::program = stayke_escrow::ID
    )]
    pub booking: Account<'info, Booking>,

    #[account(
        seeds = [USER_PROFILE_SEED.as_bytes(), signer.key().as_ref()],
        bump = user_profile.bump,
        seeds::program = stayke_core::ID,
        constraint = !user_profile.banned @ DisputeError::UserBanned,
        constraint = user_profile.identity.is_some() @ DisputeError::UserNotVerified,
    )]
    pub user_profile: Account<'info, UserProfile>,
}

pub fn handler_link_evidence(ctx: Context<LinkEvidence>, evidence: [u8; 32]) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let booking = &ctx.accounts.booking;
    let user_profile_key = ctx.accounts.user_profile.key();

    let is_guest = if dispute.opened_by == DisputeParty::Guest {
        require!(
            user_profile_key == booking.guest,
            DisputeError::UnauthorizedUser
        );
        dispute.guest_evidence = Some(evidence);
        true
    } else {
        require!(
            user_profile_key == booking.host,
            DisputeError::UnauthorizedUser
        );
        dispute.host_evidence = Some(evidence);
        false
    };

    emit!(EvidenceLinked {
        is_guest,
        dispute: dispute.key(),
        evidence
    });

    Ok(())
}
