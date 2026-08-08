use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    cpi_authority::{assert_cpi_authority, AllowedCaller},
    PenaltySeverity, ReputationProfile, UserProfile, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED,
};

#[derive(Accounts)]
pub struct UpdateUserProfile<'info> {
    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), user_profile.authority.key().as_ref()],
        bump = user_profile.bump,
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CPI authority PDA of an allowlisted Stayke program.
    pub cpi_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateReputationProfile<'info> {
    #[account(
        mut,
        seeds = [REPUTATION_PROFILE_SEED.as_bytes(), reputation_profile.authority.key().as_ref()],
        bump = reputation_profile.bump,
    )]
    pub reputation_profile: Account<'info, ReputationProfile>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
        constraint = global_config.is_initialized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_update_deposit(
    ctx: Context<UpdateUserProfile>,
    amount: u64,
    is_deposit: bool,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Treasury, AllowedCaller::Disputes],
    )?;

    let user_profile = &mut ctx.accounts.user_profile;

    if is_deposit {
        user_profile.deposited = user_profile.deposited.saturating_add(amount);
    } else {
        user_profile.deposited = user_profile.deposited.saturating_sub(amount);
    }

    Ok(())
}

pub fn handler_clear_active_booking(ctx: Context<UpdateUserProfile>) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let user_profile = &mut ctx.accounts.user_profile;
    user_profile.active_booking = None;
    Ok(())
}

pub fn handler_add_infraction(
    ctx: Context<UpdateReputationProfile>,
    severity: PenaltySeverity,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let reputation_profile = &mut ctx.accounts.reputation_profile;

    match severity {
        PenaltySeverity::Low => {
            reputation_profile.low_infractions =
                reputation_profile.low_infractions.saturating_add(1);
        }
        PenaltySeverity::Medium => {
            reputation_profile.medium_infractions =
                reputation_profile.medium_infractions.saturating_add(1);
        }
        PenaltySeverity::High => {
            reputation_profile.high_infractions =
                reputation_profile.high_infractions.saturating_add(1);
        }
    }

    Ok(())
}
