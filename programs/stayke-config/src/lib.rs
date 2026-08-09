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

declare_id!("2GM2yLmDtz2Hyb8T5VBftERmiyJ5whKUmv6V4hBjNXMW");

#[program]
pub mod stayke_config {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        minimum_deposit: u64,
        fee_bps: u64,
    ) -> Result<()> {
        handler_initialize_config(ctx, minimum_deposit, fee_bps)
    }

    // TODO: create instruction to withdraw fees from vault
}
