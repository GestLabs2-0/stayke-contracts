#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod cpi_authority;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use cpi_authority::*;
pub use instructions::*;
pub use state::*;

declare_id!("9ESE5Ztpr8zWbLyXCyiB5QqcjxHghotT8zqJxD2S3zaT");

#[program]
pub mod stayke_config {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        minimum_deposit: u64,
        fee_bps: u64,
        free_ops: u8,
        allowed_programs: AllowedPrograms,
    ) -> Result<()> {
        handler_initialize_config(ctx, minimum_deposit, fee_bps, free_ops, allowed_programs)
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
        handler_withdraw_fees(ctx, amount)
    }
}
