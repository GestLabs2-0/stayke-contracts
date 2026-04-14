use anchor_lang::prelude::*;

/// Config account for the Escrow program.
/// Stores fee parameters, the platform vault address, and the USDC mint.
#[account]
#[derive(InitSpace)]
pub struct EscrowConfig {
    pub authority: Pubkey,

    /// Platform fee vault token account.
    pub platform_vault: Pubkey,
    /// Bump of the platform vault authority PDA.
    pub platform_vault_bump: u8,

    /// USDC mint accepted by this escrow program.
    pub usdc_mint: Pubkey,

    /// Platform fee in basis points (e.g. 500 = 5%).
    pub fee_bps: u16,

    pub is_initialized: bool,
    pub bump: u8,
}
