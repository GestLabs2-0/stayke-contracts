use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    pub authority: Pubkey,

    pub minimum_deposit: u64,

    pub fee_bps: u64,

    pub usdc_mint: Pubkey,

    pub is_initialized: bool,

    /// Free operations before deposit is required
    pub free_ops: u8,

    /// Platform fee vault token account.
    pub platform_vault: Pubkey,
    /// Bump of the platform vault authority PDA.
    pub platform_vault_bump: u8,

    /// Stayke core program ID (CPI allowlist SoT).
    pub core_program: Pubkey,
    /// Stayke escrow program ID (CPI allowlist SoT).
    pub escrow_program: Pubkey,
    /// Stayke disputes program ID (CPI allowlist SoT).
    pub disputes_program: Pubkey,
    /// Stayke treasury program ID (CPI allowlist SoT).
    pub treasury_program: Pubkey,

    pub bump: u8,
}
