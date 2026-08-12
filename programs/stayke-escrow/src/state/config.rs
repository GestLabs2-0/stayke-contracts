use anchor_lang::prelude::*;

/// Config account for the Escrow program.
/// Stores fee parameters, the platform vault address, and the USDC mint.
#[account]
#[derive(InitSpace)]
pub struct EscrowConfig {
    pub authority: Pubkey,

    pub is_initialized: bool,

    pub bump: u8,
}
