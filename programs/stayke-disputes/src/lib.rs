#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use error::*;
pub use instructions::*;
pub use state::*;

// Must match Anchor.toml [programs.devnet].
declare_id!("8vgDvWkdqhpGBPAczpmZ3DJahVgNN36soRnyw6MbfMCJ");

#[program]
pub mod stayke_disputes {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        handler_initialize_config(ctx)
    }

    // ---------------------------------------------------------------------------
    // Disputes
    // ---------------------------------------------------------------------------

    pub fn open_dispute(ctx: Context<OpenDispute>) -> Result<()> {
        handler_open_dispute(ctx)
    }

    pub fn escalate_dispute(ctx: Context<EscalateDispute>) -> Result<()> {
        handler_escalate_dispute(ctx)
    }

    pub fn solve_dispute_before_admin(ctx: Context<SolveDisputeBeforeAdmin>) -> Result<()> {
        handler_solve_dispute_before_admin(ctx)
    }
}
