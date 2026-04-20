pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP");

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
}
