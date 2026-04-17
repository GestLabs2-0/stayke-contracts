use anchor_lang::prelude::*;

use crate::{PenaltySeverity, ReputationProfile, UserProfile};

// TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand
// I think the best way to handle this all is by creating a global contract

#[derive(Accounts)]
pub struct UpdateUserProfile<'info> {
     #[account(
        mut, 
        seeds = [b"user_profile", user_profile.owner.key().as_ref()], 
        bump = user_profile.bump,
        // NOTE: the `authority` here can be either the user's wallet (direct call)
        // or a trusted PDA from another Stayke program (CPI call).
        // Callers are responsible for checking ownership before invoking.
    )]
    pub user_profile: Account<'info, UserProfile>,
    
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateReputationProfile<'info> {
     #[account(
        mut, 
        seeds = [b"reputation_profile", reputation_profile.owner.key().as_ref()], 
        bump = reputation_profile.bump,
    )]
    pub reputation_profile: Account<'info, ReputationProfile>,
    
    pub authority: Signer<'info>, // Often will be a PDA of an escrow/dispute contract rather than the user
}

pub fn handler_set_host_status(ctx: Context<UpdateUserProfile>, status: bool) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.is_host = status;
    Ok(())
}

// These functions abstract the logic that would normally be called by CPI from a Treasury/Escrow program.
pub fn handler_update_deposit(ctx: Context<UpdateUserProfile>, amount: u64, is_deposit: bool) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    
    if is_deposit {
        user_profile.deposited = user_profile.deposited.saturating_add(amount);
        user_profile.deposit_timestamp = Clock::get()?.unix_timestamp;
    } else {
        user_profile.deposited = user_profile.deposited.saturating_sub(amount);
    }

    Ok(())
}

pub fn handler_clear_active_booking(ctx: Context<UpdateUserProfile>) -> Result<()> {
    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.active_booking = None;
    Ok(())
}

pub fn handler_add_infraction(ctx: Context<UpdateReputationProfile>, severity: PenaltySeverity) -> Result<()> {
    let reputation_profile = &mut ctx.accounts.reputation_profile;

    match severity {
        PenaltySeverity::Low => {
            reputation_profile.low_infractions = reputation_profile.low_infractions.saturating_add(1);
        }
        PenaltySeverity::Medium => {
            reputation_profile.medium_infractions = reputation_profile.medium_infractions.saturating_add(1);
        }
        PenaltySeverity::High => {
            reputation_profile.high_infractions = reputation_profile.high_infractions.saturating_add(1);
        }
    }

    Ok(())
}
