#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod cpi_authority;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use cpi_authority::*;
pub use instructions::{cpi::*, *};
pub use state::*;

declare_id!("8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP");

#[program]
pub mod stayke_core {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        handler_initialize_config(ctx)
    }

    pub fn initialize_user_profile(ctx: Context<InitializeUserProfile>) -> Result<()> {
        handler_initialize_user_profile(ctx)
    }

    pub fn initialize_listing(
        ctx: Context<InitializeListing>,
        price: u64,
        listing_id: u16,
    ) -> Result<()> {
        handler_initialize_listing(ctx, price, listing_id)
    }
    // ---------------------------------------------------------------------------
    // Privileged CPI mutators — require A2 CPI authority PDA + GlobalConfig allowlist
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

    pub fn set_listing_occupied(ctx: Context<SetListingOccupied>, occupied: bool) -> Result<()> {
        handler_set_listing_occupied(ctx, occupied)
    }

    pub fn update_host_review(ctx: Context<UpdateHostReview>, score: u8) -> Result<()> {
        handler_update_host_review(ctx, score)
    }

    // --------------------------------------------------------------------------
    // Identity Verification
    // ------------------------------------------------------------------------

    pub fn link_identity(ctx: Context<LinkIdentity>, _id: [u8; 32]) -> Result<()> {
        handler_link_identity(ctx)
    }

    pub fn init_identity(ctx: Context<InitIdentity>, _id: [u8; 32]) -> Result<()> {
        handler_init_identity(ctx)
    }
}
