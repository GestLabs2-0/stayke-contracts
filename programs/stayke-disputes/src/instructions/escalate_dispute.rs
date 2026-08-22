use anchor_lang::prelude::*;

use crate::{
    constants::{DISPUTE_P2P_WINDOW_SECONDS, DISPUTE_PDA_SEED},
    error::DisputeError,
    events::DisputeEscalated,
    state::{DisputeAccount, DisputeState},
};

/// Escalates an OpenP2P dispute to admin review once the 24-hour P2P window
/// (measured from `opened_at`) has elapsed.
///
/// This instruction is permissionless: anyone can invoke it. It only advances
/// the dispute state machine, so no authorization is required.
#[derive(Accounts)]
pub struct EscalateDispute<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_PDA_SEED.as_bytes(), dispute.booking.as_ref()],
        bump = dispute.bump,
    )]
    pub dispute: Box<Account<'info, DisputeAccount>>,
}

pub fn handler_escalate_dispute(ctx: Context<EscalateDispute>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;

    require!(
        dispute.state == DisputeState::OpenP2P,
        DisputeError::DisputeNotOpenP2P
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        now > dispute.opened_at.saturating_add(DISPUTE_P2P_WINDOW_SECONDS),
        DisputeError::EscalationWindowNotElapsed
    );

    dispute.state = DisputeState::Escalated;

    emit!(DisputeEscalated {
        dispute: dispute.key(),
        booking: dispute.booking,
        opened_at: dispute.opened_at,
        escalated_at: now,
    });

    Ok(())
}
