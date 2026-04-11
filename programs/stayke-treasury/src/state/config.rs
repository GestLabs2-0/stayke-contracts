use anchor_lang::prelude::*;

/// Config account for the Treasury program.
/// Stores the authority allowed to manage treasury operations
/// and the references to the on-chain token vaults.
#[account]
#[derive(InitSpace)]
pub struct TreasuryConfig {
    /// The admin/authority pubkey that can manage the treasury.
    pub authority: Pubkey,

    /// Pubkey of the treasury token account (controlled by treasury_pda).
    /// Holds user USDC guarantee deposits.
    pub treasury_vault: Pubkey,

    /// Bump of the treasury PDA ([b"treasury"]) — used to sign CPI calls.
    pub treasury_bump: u8,

    /// The USDC mint accepted by this treasury.
    pub usdc_mint: Pubkey,

    /// Minimum deposit amount (in USDC lamports) required from each user.
    pub minimum_deposit: u64,

    /// Guard flag to prevent re-initialization.
    pub is_initialized: bool,

    pub bump: u8,
}
