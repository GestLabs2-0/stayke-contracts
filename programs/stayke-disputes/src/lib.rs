pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use error::*;
pub use instructions::*;
pub use state::*;
use stayke_core::state::PenaltySeverity;

declare_id!("7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB");

#[program]
pub mod stayke_disputes {
    use super::*;

    // ---------------------------------------------------------------------------
    // Admin config
    // ---------------------------------------------------------------------------

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        admin::handler_initialize_config(ctx)
    }

    pub fn penalize_user(ctx: Context<PenalizeUser>, severity: PenaltySeverity) -> Result<()> {
        admin::handler_penalize_user(ctx, severity)
    }

    // ---------------------------------------------------------------------------
    // Disputes
    // ---------------------------------------------------------------------------

    pub fn open_dispute(ctx: Context<OpenDispute>, reason: DisputeReason) -> Result<()> {
        manage_disputes::handler_open_dispute(ctx, reason)
    }

    pub fn resolve_dispute(
        ctx: Context<ResolveDispute>,
        host_share_bps: u16,
        rejected: bool,
    ) -> Result<()> {
        manage_disputes::handler_resolve_dispute(ctx, host_share_bps, rejected)
    }

    pub fn close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
        manage_disputes::handler_close_dispute(ctx)
    }
}
