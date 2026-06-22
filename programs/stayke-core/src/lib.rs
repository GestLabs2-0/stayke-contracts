#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::{cpi::*, *};
pub use state::*;

declare_id!("8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP");

#[program]
pub mod stayke_core {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        initialize_config::handler_initialize_config(ctx)
    }

    pub fn initialize_user_profile(
        ctx: Context<InitializeUserProfile>,
        id: [u8; 32],
        country_code: [u8; 2],
        doctype: DocType,
    ) -> Result<()> {
        initialize_user_profile::handler_initialize_user_profile(ctx, id, country_code, doctype)
    }

    pub fn verify_identity(ctx: Context<VerifyIdentity>) -> Result<()> {
        verify_identity::handler_verify_identity(ctx)
    }

    pub fn initialize_listing(
        ctx: Context<InitializeListing>,
        price: u64,
        listing_id: u16,
    ) -> Result<()> {
        initialize_listing::handler_initialize_listing(ctx, price, listing_id)
    }
    // ---------------------------------------------------------------------------
    // User profile mutations — callable directly or via CPI
    // ---------------------------------------------------------------------------

    pub fn update_deposit(
        ctx: Context<UpdateUserProfile>,
        amount: u64,
        is_deposit: bool,
    ) -> Result<()> {
        handler_update_deposit(ctx, amount, is_deposit)
    }

    pub fn clear_active_booking(ctx: Context<UpdateUserProfile>) -> Result<()> {
        handler_clear_active_booking(ctx)
    }

    pub fn add_infraction(
        ctx: Context<UpdateReputationProfile>,
        severity: PenaltySeverity,
    ) -> Result<()> {
        handler_add_infraction(ctx, severity)
    }

    pub fn clear_listing_booking(ctx: Context<ClearListingBooking>) -> Result<()> {
        handle_clear_listing_booking(ctx)
    }
}
