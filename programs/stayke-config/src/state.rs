use anchor_lang::prelude::*;

// TODO: add stayke contracts to Global Config
// The purpose is to have a single source of truth for all the stayke contracts, so
// whenever we make any CPI call to any of the stayke contracts we can be sure that the signer is correct and we don't have to hardcode any addresses in the code.

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
