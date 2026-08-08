use anchor_lang::prelude::*;
use anchor_spl::{
    token::{transfer_checked, TransferChecked},
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use stayke_config::{error::StaykeConfigError, GLOBAL_CONFIG_SEED};
use stayke_core::program::StaykeCore;
use stayke_core::UserProfile;
use stayke_core::{cpi::accounts::UpdateUserProfile, CPI_AUTHORITY_SEED, USER_PROFILE_SEED};

use crate::{error::TreasuryError, TreasuryConfig, TREASURY_CONFIG_SEED};

// ---------------------------------------------------------------------------
// Deposit guarantee
// ---------------------------------------------------------------------------
// 1. Transfers USDC from the user's wallet into the treasury vault.
// 2. CPIs into stayke-core to increment `UserProfile.deposited` with A2 PDA signer.

#[derive(Accounts)]
pub struct DepositGuarantee<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump = config.bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
        constraint = global_config.key() == config.global_config @ StaykeConfigError::InvalidGlobalConfig,
    )]
    pub global_config: Box<Account<'info, stayke_config::GlobalConfig>>,

    #[account(mut)]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_vault.key() == config.treasury_vault @ TreasuryError::InvalidTreasuryVault,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = usdc_mint.key() == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,

    #[account(
        mut,
        seeds = [USER_PROFILE_SEED.as_bytes(), signer.key().as_ref()],
        seeds::program = stayke_core_program.key(),
        bump = user_profile.bump,
        constraint = user_profile.authority == signer.key() @ TreasuryError::Unauthorized,
    )]
    pub user_profile: Account<'info, UserProfile>,

    /// CHECK: Treasury CPI authority PDA — signs privileged core mutators (A2).
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_deposit_guarantee(ctx: Context<DepositGuarantee>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.global_config;

    require!(
        amount >= config.minimum_deposit,
        TreasuryError::DepositTooLow
    );

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.sender_token_account.to_account_info(),
        to: ctx.accounts.treasury_vault.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
    transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    let cpi_program = ctx.accounts.stayke_core_program.key();
    let bump = ctx.bumps.cpi_authority;
    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
    let cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.user_profile.to_account_info(),
        global_config: ctx.accounts.global_config.to_account_info(),
        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
    };
    stayke_core::cpi::update_deposit(
        CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
        amount,
        true,
    )?;

    Ok(())
}
