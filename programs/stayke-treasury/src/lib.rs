#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

// Must match Anchor.toml [programs.devnet].
declare_id!("3JE5y7vtjkZkA6s3eRAKorT1eQmgoJQmnVqpy15uUjq8");

#[program]
pub mod stayke_treasury {
    use super::*;

    // ---------------------------------------------------------------------------
    // Admin
    // ---------------------------------------------------------------------------

    pub fn initialize_treasury(ctx: Context<InitializeTreasury>) -> Result<()> {
        initialize::handler_initialize_treasury(ctx)
    }

    // ---------------------------------------------------------------------------
    // User — Guarantee deposits
    // ---------------------------------------------------------------------------

    pub fn deposit_guarantee(ctx: Context<DepositGuarantee>, amount: u64) -> Result<()> {
        deposit_guarantee::handler_deposit_guarantee(ctx, amount)
    }

    pub fn withdraw_guarantee(ctx: Context<WithdrawGuarantee>, amount: u64) -> Result<()> {
        withdraw_guarantee::handler_withdraw_guarantee(ctx, amount)
    }

    // ---------------------------------------------------------------------------
    // Lending & Staking (placeholders — not yet enabled)
    // ---------------------------------------------------------------------------

    // pub fn lend(ctx: Context<Lend>, amount: u64) -> Result<()> {
    //     lending::handler_lend(ctx, amount)
    // }

    // pub fn withdraw_from_lending(ctx: Context<WithdrawFromLending>, amount: u64) -> Result<()> {
    //     lending::handler_withdraw_from_lending(ctx, amount)
    // }

    // pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    //     lending::handler_stake(ctx, amount)
    // }

    // ---------------------------------------------------------------------------
    // CPI endpoints for stayke-disputes
    // ---------------------------------------------------------------------------

    pub fn cpi_penalize_transfer(ctx: Context<PenalizeTransferCpi>, amount: u64) -> Result<()> {
        penalize_transfer_cpi::handler_cpi_penalize_transfer(ctx, amount)
    }
}
