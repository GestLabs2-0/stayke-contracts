use anchor_lang::prelude::*;
use anchor_spl::{
    token::{transfer_checked, TransferChecked},
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use stayke_core::cpi::accounts::UpdateUserProfile;
use stayke_core::program::StaykeCore;
use stayke_core::UserProfile;

use crate::{error::TreasuryError, TreasuryConfig};

// ---------------------------------------------------------------------------
// Deposit guarantee
// ---------------------------------------------------------------------------
// 1. Transfers USDC from the user's wallet into the treasury vault.
// 2. CPIs into stayke-core to increment `UserProfile.deposited`.

#[derive(Accounts)]
pub struct DepositGuarantee<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    // ---- Treasury config ----
    #[account(
        seeds = [b"treasury_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    // ---- Token accounts ----
    /// Source: the user's own USDC token account.
    #[account(mut)]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Destination: the treasury vault (must match config).
    #[account(
        mut,
        constraint = treasury_vault.key() == config.treasury_vault @ TreasuryError::InvalidTreasuryVault,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = usdc_mint.key() == config.usdc_mint @ TreasuryError::InvalidTokenMint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,

    // ---- stayke-core CPI ----
    /// The user's UserProfile PDA in stayke-core — will be mutated via CPI.
    #[account(
        mut,
        seeds = [b"user_profile", signer.key().as_ref()],
        seeds::program = stayke_core_program.key(),
        bump = user_profile.bump,
        constraint = user_profile.owner == signer.key() @ TreasuryError::Unauthorized,
    )]
    pub user_profile: Account<'info, UserProfile>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_deposit_guarantee(ctx: Context<DepositGuarantee>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;

    require!(
        amount >= config.minimum_deposit,
        TreasuryError::DepositTooLow
    );

    // 1. Transfer USDC into the treasury vault.
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.sender_token_account.to_account_info(),
        to: ctx.accounts.treasury_vault.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
    transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    // 2. CPI → stayke-core: increment deposited balance.
    let cpi_program = ctx.accounts.stayke_core_program.key();
    let cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.user_profile.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
    };
    stayke_core::cpi::update_deposit(
        CpiContext::new(cpi_program, cpi_accounts),
        amount,
        true, // is_deposit = true → add to deposited
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Withdraw guarantee
// ---------------------------------------------------------------------------
// 1. Validates that the on-chain deposited balance covers the requested amount.
// 2. CPIs into stayke-core to decrement `UserProfile.deposited`.
// 3. Transfers USDC from the treasury vault back to the user.

#[derive(Accounts)]
pub struct WithdrawGuarantee<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    // ---- Treasury config ----
    #[account(
        seeds = [b"treasury_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    // ---- Token accounts ----
    /// Treasury vault — source of the withdrawal.
    #[account(
        mut,
        constraint = treasury_vault.key() == config.treasury_vault @ TreasuryError::InvalidTreasuryVault,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Treasury PDA — signs the CPI transfer out of the vault.
    #[account(seeds = [b"treasury"], bump = config.treasury_bump)]
    pub treasury_pda: UncheckedAccount<'info>,

    /// Destination: the user's own USDC token account.
    #[account(mut)]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = usdc_mint.key() == config.usdc_mint @ TreasuryError::InvalidTokenMint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,

    // ---- stayke-core CPI ----
    #[account(
        mut,
        seeds = [b"user_profile", signer.key().as_ref()],
        seeds::program = stayke_core_program.key(),
        bump = user_profile.bump,
        constraint = user_profile.owner == signer.key() @ TreasuryError::Unauthorized,
        constraint = !user_profile.banned @ TreasuryError::UserBanned,
        constraint = user_profile.active_booking.is_none() @ TreasuryError::ActiveBookingExists,
    )]
    pub user_profile: Account<'info, UserProfile>,

    pub stayke_core_program: Program<'info, StaykeCore>,
}

pub fn handler_withdraw_guarantee(ctx: Context<WithdrawGuarantee>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;

    require!(amount > 0, TreasuryError::ZeroWithdrawal);
    require!(
        ctx.accounts.user_profile.deposited >= amount,
        TreasuryError::InsufficientBalance
    );

    // 1. CPI → stayke-core: decrement deposited balance first (checks-effects-interactions).
    let cpi_program = ctx.accounts.stayke_core_program.key();
    let cpi_accounts = UpdateUserProfile {
        user_profile: ctx.accounts.user_profile.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
    };
    stayke_core::cpi::update_deposit(
        CpiContext::new(cpi_program, cpi_accounts),
        amount,
        false, // is_deposit = false → subtract from deposited
    )?;

    // 2. Transfer USDC from the treasury vault to the user.
    let treasury_seeds: &[&[&[u8]]] = &[&[b"treasury", &[config.treasury_bump]]];
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.treasury_vault.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.treasury_pda.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_accounts,
        treasury_seeds,
    );
    transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    Ok(())
}
