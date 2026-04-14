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

    pub fn handler_verify_identity(ctx: Context<VerifyIdentity>) -> Result<()> {
        verify_identity::handler_verify_identity(ctx)
    }

    // ---------------------------------------------------------------------------
    // User profile mutations — callable directly or via CPI
    // ---------------------------------------------------------------------------

    pub fn update_deposit(
        ctx: Context<UpdateUserProfile>,
        amount: u64,
        is_deposit: bool,
    ) -> Result<()> {
        user_profile_mutators::handler_update_deposit(ctx, amount, is_deposit)
    }

    pub fn set_host_status(ctx: Context<UpdateUserProfile>, status: bool) -> Result<()> {
        user_profile_mutators::handler_set_host_status(ctx, status)
    }
    pub fn clear_active_booking(ctx: Context<UpdateUserProfile>) -> Result<()> {
        user_profile_mutators::handler_clear_active_booking(ctx)
    }

    pub fn add_infraction(
        ctx: Context<UpdateReputationProfile>,
        severity: crate::PenaltySeverity,
    ) -> Result<()> {
        user_profile_mutators::handler_add_infraction(ctx, severity)
    }

    pub fn clear_listing_booking(ctx: Context<ClearListingBooking>) -> Result<()> {
        listing_mutator::handle_clear_listing_bookig(ctx)
    }
}
