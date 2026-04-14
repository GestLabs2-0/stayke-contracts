pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("HnPTYDpC6MV2AHcRoNfGVCuiPHfVdAZLUoPAFCBfzXRx");

#[program]
pub mod stayke_treasury {
    use super::*;

    // ---------------------------------------------------------------------------
    // Admin
    // ---------------------------------------------------------------------------

    pub fn initialize_treasury(
        ctx: Context<InitializeTreasury>,
        minimum_deposit: u64,
    ) -> Result<()> {
        initialize::handler_initialize_treasury(ctx, minimum_deposit)
    }

    // ---------------------------------------------------------------------------
    // User — Guarantee deposits
    // ---------------------------------------------------------------------------

    pub fn deposit_guarantee(ctx: Context<DepositGuarantee>, amount: u64) -> Result<()> {
        guarantee::handler_deposit_guarantee(ctx, amount)
    }

    pub fn withdraw_guarantee(ctx: Context<WithdrawGuarantee>, amount: u64) -> Result<()> {
        guarantee::handler_withdraw_guarantee(ctx, amount)
    }

    // ---------------------------------------------------------------------------
    // Lending & Staking (placeholders — not yet enabled)
    // ---------------------------------------------------------------------------

    pub fn lend(ctx: Context<Lend>, amount: u64) -> Result<()> {
        lending::handler_lend(ctx, amount)
    }

    pub fn withdraw_from_lending(ctx: Context<WithdrawFromLending>, amount: u64) -> Result<()> {
        lending::handler_withdraw_from_lending(ctx, amount)
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        lending::handler_stake(ctx, amount)
    }

    // ---------------------------------------------------------------------------
    // CPI endpoints for stayke-disputes
    // ---------------------------------------------------------------------------

    pub fn cpi_penalize_transfer(ctx: Context<PenalizeTransferCpi>, amount: u64) -> Result<()> {
        cpi_transfers::handler_cpi_penalize_transfer(ctx, amount)
    }
}
