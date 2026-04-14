use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_core::{
    cpi::{
        accounts::{UpdateReputationProfile, UpdateUserProfile},
        add_infraction, update_deposit,
    },
    state::{ReputationProfile, UserProfile},
    PenaltySeverity,
};
use stayke_treasury::cpi::{accounts::PenalizeTransferCpi, cpi_penalize_transfer};

use crate::{error::DisputeError, events::UserPenalized, state::DisputeConfig};

// ---------------------------------------------------------------------------
// Add / Remove admin / Init config — (Standard admin logic)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + DisputeConfig::INIT_SPACE,
        seeds = [b"dispute_config"],
        bump,
    )]
    pub config: Account<'info, DisputeConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admins.push(ctx.accounts.authority.key());
    config.retribution_bps_low = 1000;
    config.retribution_bps_medium = 3000;
    config.retribution_bps_high = 10000;
    config.is_initialized = true;
    config.bump = ctx.bumps.config;
    Ok(())
}

// ---------------------------------------------------------------------------
// Penalize user
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct PenalizeUser<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"dispute_config"],
        constraint = config.admins.contains(&admin.key()) @ DisputeError::UnauthorizedAdmin,
        bump = config.bump,
    )]
    pub config: Account<'info, DisputeConfig>,

    /// The offending user's UserProfile (requires stayke-core CPI to mutate deposit).
    #[account(
        mut,
        constraint = !penalized_user_profile.banned @ DisputeError::UserBanned,
    )]
    pub penalized_user_profile: Account<'info, UserProfile>,

    /// The offending user's ReputationProfile (requires stayke-core CPI to mutate infractions).
    #[account(mut)]
    pub penalized_reputation_profile: Account<'info, ReputationProfile>,

    /// The affected user's USDC account to receive the retribution.
    #[account(mut)]
    pub affected_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Wallet of the affected user, used for logging/events.
    pub affected_wallet: UncheckedAccount<'info>,

    /// The treasury program config.
    /// CHECK: Used strictly for CPI validation on treasury side.
    pub treasury_config: UncheckedAccount<'info>,

    /// The global treasury vault from stayke-treasury.
    #[account(mut)]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Treasury PDA to sign the transfer (managed inside stayke-treasury).
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(mut)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub stayke_core_program: Program<'info, stayke_core::program::StaykeCore>,
    pub stayke_treasury_program: Program<'info, stayke_treasury::program::StaykeTreasury>,
    pub token_program: Interface<'info, TokenInterface>,
}

// WE MUST MOVE USER PENALIZATION TO DISPUTES FLOW BEFORE ENDING THE DISPUTE OR AT LEAST APPLY IT ONLY WHEN THE DISPUTE IS OPEN
pub fn handler_penalize_user(ctx: Context<PenalizeUser>, severity: PenaltySeverity) -> Result<()> {
    let config = &ctx.accounts.config;
    let penalized = &ctx.accounts.penalized_user_profile;

    // 1. Determine retribution amount from severity
    let bps = match severity {
        PenaltySeverity::Low => config.retribution_bps_low,
        PenaltySeverity::Medium => config.retribution_bps_medium,
        PenaltySeverity::High => config.retribution_bps_high,
    };

    let retribution = (penalized.deposited as u128)
        .saturating_mul(bps as u128)
        .saturating_div(10_000) as u64;

    let actual_retribution = retribution.min(penalized.deposited);

    // 2. Perform transfer from treasury to affected via stayke-treasury CPI
    if actual_retribution > 0 {
        let cpi_accounts = PenalizeTransferCpi {
            authority: ctx.accounts.admin.to_account_info(),
            config: ctx.accounts.treasury_config.to_account_info(),
            treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
            treasury_pda: ctx.accounts.treasury_pda.to_account_info(),
            destination_token_account: ctx.accounts.affected_token_account.to_account_info(),
            usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.stayke_treasury_program.key(), cpi_accounts);
        cpi_penalize_transfer(cpi_ctx, actual_retribution)?;

        // 3. Subtract deposit via stayke-core CPI
        let update_deposit_cpi_accounts = UpdateUserProfile {
            user_profile: ctx.accounts.penalized_user_profile.to_account_info(),
            authority: ctx.accounts.admin.to_account_info(),
        };
        let update_deposit_ctx = CpiContext::new(
            ctx.accounts.stayke_core_program.key(),
            update_deposit_cpi_accounts,
        );
        update_deposit(update_deposit_ctx, actual_retribution, false)?;
    }

    // 4. Update infraction counters via stayke-core CPI
    let add_inf_cpi_accounts = UpdateReputationProfile {
        reputation_profile: ctx.accounts.penalized_reputation_profile.to_account_info(),
        authority: ctx.accounts.admin.to_account_info(),
    };
    let add_inf_ctx = CpiContext::new(ctx.accounts.stayke_core_program.key(), add_inf_cpi_accounts);
    add_infraction(add_inf_ctx, severity)?;

    emit!(UserPenalized {
        penalized_user: ctx.accounts.penalized_user_profile.owner,
        affected_wallet: ctx.accounts.affected_wallet.key(),
        penalty_amount: actual_retribution,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
