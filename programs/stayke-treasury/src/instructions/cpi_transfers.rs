use crate::{error::TreasuryError, TreasuryConfig};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::{transfer_checked, TransferChecked},
    token_interface::{Mint, TokenAccount, TokenInterface},
};

// ---------------------------------------------------------------------------
// CPI Endpoint: Penalize Transfer (Used by stayke-disputes)
// ---------------------------------------------------------------------------
// Allows stayke-disputes to extract a penalized amount of USDC from the
// global treasury vault and route it directly to the affected party.

#[derive(Accounts)]
pub struct PenalizeTransferCpi<'info> {
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"treasury_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    #[account(
        mut,
        constraint = treasury_vault.key() == config.treasury_vault @ TreasuryError::InvalidTreasuryVault,
    )]
    pub treasury_vault: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Treasury PDA — signs the CPI transfer out of the vault.
    #[account(seeds = [b"treasury"], bump = config.treasury_bump)]
    pub treasury_pda: UncheckedAccount<'info>,

    /// Destination: the destination token account owned by the affected party.
    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = usdc_mint.key() == config.usdc_mint @ TreasuryError::InvalidTokenMint)]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cpi_penalize_transfer(ctx: Context<PenalizeTransferCpi>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;

    require!(amount > 0, TreasuryError::ZeroWithdrawal);

    let treasury_seeds: &[&[&[u8]]] = &[&[b"treasury", &[config.treasury_bump]]];
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.treasury_vault.to_account_info(),
        to: ctx.accounts.destination_token_account.to_account_info(),
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
