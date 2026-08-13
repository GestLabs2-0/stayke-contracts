use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::{
    error::StaykeConfigError, AllowedPrograms, GlobalConfig, GLOBAL_CONFIG_SEED,
    PLATFORM_VAULT_CONFIG_SEED, PLATFORM_VAULT_SEED,
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(init, payer = authority, space = 8 + GlobalConfig::INIT_SPACE, seeds = [GLOBAL_CONFIG_SEED.as_bytes()], bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: PDA that will sign as the platform-vault authority.
    #[account(seeds = [PLATFORM_VAULT_SEED.as_bytes()], bump)]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = platform_vault_pda,
        seeds = [PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(
    ctx: Context<InitializeConfig>,
    minimum_deposit: u64,
    fee_bps: u64,
    free_ops: u8,
    allowed_programs: AllowedPrograms,
) -> Result<()> {
    require!(fee_bps < 10_000, StaykeConfigError::InvalidFeeBps);

    let global_config = &mut ctx.accounts.global_config;
    global_config.authority = ctx.accounts.authority.key();
    global_config.bump = ctx.bumps.global_config;
    global_config.minimum_deposit = minimum_deposit;
    global_config.fee_bps = fee_bps;
    global_config.usdc_mint = ctx.accounts.usdc_mint.key();
    global_config.platform_vault = ctx.accounts.platform_vault.key();
    global_config.platform_vault_bump = ctx.bumps.platform_vault_pda;

    global_config.core_program = allowed_programs.core;
    global_config.escrow_program = allowed_programs.escrow;
    global_config.disputes_program = allowed_programs.disputes;
    global_config.treasury_program = allowed_programs.treasury;
    global_config.is_initialized = true;
    global_config.free_ops = free_ops;
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// Only the configured authority can withdraw fees.
    #[account(
        constraint = authority.key() == global_config.authority @ StaykeConfigError::Unauthorized,
    )]
    pub authority: Signer<'info>,

    /// CHECK: PDA that signs the SPL Token transfer. Validated by seeds + stored bump.
    #[account(
        seeds = [PLATFORM_VAULT_SEED.as_bytes()],
        bump = global_config.platform_vault_bump,
    )]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ StaykeConfigError::InvalidVaultAccount,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    /// Destination token account for withdrawn fees.
    #[account(
        mut,
        constraint = destination_token_account.mint == global_config.usdc_mint @ StaykeConfigError::InvalidTokenMint,
    )]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// USDC mint, used for transfer_checked decimals validation.
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
    let global_config = &ctx.accounts.global_config;

    require!(
        global_config.is_initialized,
        StaykeConfigError::InvalidGlobalConfig
    );
    require!(amount > 0, StaykeConfigError::ZeroAmount);
    let bump = global_config.platform_vault_bump;
    let vault_seed = PLATFORM_VAULT_SEED.as_bytes();
    let signer_seeds: &[&[&[u8]]] = &[&[vault_seed, &[bump]]];

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.platform_vault.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.destination_token_account.to_account_info(),
        authority: ctx.accounts.platform_vault_pda.to_account_info(),
    };

    let cpi_ctx =
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), cpi_accounts, signer_seeds);

    transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    Ok(())
}
