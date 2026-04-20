use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    pub authority: Pubkey,

    pub minimum_deposit: u64,

    pub fee_bps: u64,

    pub usdc_mint: Pubkey,

    pub is_initialized: bool,

    /// Platform fee vault token account.
    pub platform_vault: Pubkey,
    /// Bump of the platform vault authority PDA.
    pub platform_vault_bump: u8,

    pub bump: u8,
}
