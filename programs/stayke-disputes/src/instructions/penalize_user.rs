// ---------------------------------------------------------------------------
// DEPRECATED — STK-168 refactor: P2P dispute flow with admin escalation.
// The penalty flow is being reworked alongside the dispute refactor. This
// module is commented out to keep the crate compiling during the refactor.
// ---------------------------------------------------------------------------
/*
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use stayke_config::{GlobalConfig, CPI_AUTHORITY_SEED, GLOBAL_CONFIG_SEED};
use stayke_core::{
    cpi::{
        accounts::{UpdateReputationProfile, UpdateUserProfile},
        add_infraction, update_deposit,
    },
    program::StaykeCore,
    state::{ReputationProfile, UserProfile},
    PenaltySeverity, REPUTATION_PROFILE_SEED, USER_PROFILE_SEED,
};
use stayke_treasury::{
    cpi::{accounts::PenalizeTransferCpi, cpi_penalize_transfer},
    program::StaykeTreasury,
    TreasuryConfig, TREASURY_CONFIG_SEED,
};

use crate::{
    constants::DISPUTE_CONFIG_PDA_SEED, error::DisputeError, events::UserPenalized,
    state::DisputeConfig,
};

#[derive(Accounts)]
pub struct PenalizeUser<'info> {
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
        seeds = [USER_PROFILE_SEED.as_bytes(), penalized_user_profile.authority.key().as_ref()],
        seeds::program = stayke_core::ID,
        bump = penalized_user_profile.bump,
        constraint = !penalized_user_profile.banned @ DisputeError::UserBanned,
    )]
    pub penalized_user_profile: Box<Account<'info, UserProfile>>,

    #[account(
        mut,
        seeds = [
            REPUTATION_PROFILE_SEED.as_bytes(),
            penalized_user_profile.authority.key().as_ref(),
        ],
        seeds::program = stayke_core::ID,
        bump = penalized_reputation_profile.bump,
        constraint = penalized_reputation_profile.authority == penalized_user_profile.authority
            @ DisputeError::InvalidReputationProfile,
    )]
    pub penalized_reputation_profile: Box<Account<'info, ReputationProfile>>,

    #[account(
        mut,
        constraint = affected_token_account.mint == usdc_mint.key() @ DisputeError::InvalidTokenMint,
        constraint = affected_token_account.owner == affected_wallet.key()
            @ DisputeError::InvalidAffectedTokenAccount,
    )]
    pub affected_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Wallet of the compensation recipient; token account must be owned by this key.
    pub affected_wallet: UncheckedAccount<'info>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_treasury_program.key(),
        bump = treasury_config.bump,
    )]
    pub treasury_config: Box<Account<'info, TreasuryConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        seeds::program = stayke_config::ID,
        bump = global_config.bump,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(mut)]
    pub treasury_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Treasury PDA to sign the transfer (managed inside stayke-treasury).
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = usdc_mint.key() == global_config.usdc_mint @ DisputeError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Disputes CPI authority PDA — signs privileged core mutators.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
    pub stayke_treasury_program: Program<'info, StaykeTreasury>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_penalize_user(ctx: Context<PenalizeUser>, severity: PenaltySeverity) -> Result<()> {
    let config = &ctx.accounts.config;
    let penalized = &ctx.accounts.penalized_user_profile;

    let bps = match severity {
        PenaltySeverity::Low => config.retribution_bps_low,
        PenaltySeverity::Medium => config.retribution_bps_medium,
        PenaltySeverity::High => config.retribution_bps_high,
    };

    let retribution = (penalized.deposited as u128)
        .saturating_mul(bps as u128)
        .saturating_div(10_000) as u64;

    let actual_retribution = retribution.min(penalized.deposited);

    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

    if actual_retribution > 0 {
        let cpi_accounts = PenalizeTransferCpi {
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            config: ctx.accounts.treasury_config.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            treasury_vault: ctx.accounts.treasury_vault.to_account_info(),
            treasury_pda: ctx.accounts.treasury_pda.to_account_info(),
            destination_token_account: ctx.accounts.affected_token_account.to_account_info(),
            usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.stayke_treasury_program.key(),
            cpi_accounts,
            signer_seeds,
        );
        cpi_penalize_transfer(cpi_ctx, actual_retribution)?;

        let update_deposit_cpi_accounts = UpdateUserProfile {
            user_profile: ctx.accounts.penalized_user_profile.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        };
        let update_deposit_ctx = CpiContext::new_with_signer(
            ctx.accounts.stayke_core_program.key(),
            update_deposit_cpi_accounts,
            signer_seeds,
        );
        update_deposit(update_deposit_ctx, actual_retribution, false)?;
    }

    let add_inf_cpi_accounts = UpdateReputationProfile {
        reputation_profile: ctx.accounts.penalized_reputation_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    let add_inf_ctx = CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        add_inf_cpi_accounts,
        signer_seeds,
    );
    add_infraction(add_inf_ctx, severity)?;

    emit!(UserPenalized {
        penalized_user: ctx.accounts.penalized_user_profile.authority,
        affected_wallet: ctx.accounts.affected_wallet.key(),
        penalty_amount: actual_retribution,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
*/
