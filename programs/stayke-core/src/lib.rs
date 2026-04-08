pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4fyRhe1g8fJjHRxLAS9vT1RLjS44W3FutzF9USXAdNtB");

#[program]
pub mod stayke_contracts {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        initialize::handler_initialize_config(ctx)
    }

    pub fn handler_initialize_user_profile(
        ctx: Context<InitializeUserProfile>,
        id: [u8; 32],
        country_code: [u8; 2],
        doctype: DocType,
    ) -> Result<()> {
        initialize::handler_initialize_user_profile(ctx, id, country_code, doctype)
    }
}
