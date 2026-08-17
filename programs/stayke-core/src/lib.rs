#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::{cpi::*, *};
pub use state::*;

declare_id!("2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm");

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

        state_hash: [u8; 32],
        content_ref: [u8; 32],
    ) -> Result<()> {
        handler_initialize_listing(ctx, price, listing_id, state_hash, content_ref)
    }
    // ---------------------------------------------------------------------------
    // Privileged CPI mutators — require `cpi_authority` PDA signer + GlobalConfig allowlist
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

    pub fn set_active_booking(ctx: Context<UpdateUserProfile>, booking: Pubkey) -> Result<()> {
        handler_set_active_booking(ctx, booking)
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

    pub fn increment_completed_stays(ctx: Context<UpdateUserProfile>) -> Result<()> {
        handler_increment_completed_stays(ctx)
    }

    pub fn increment_hosted_stays(ctx: Context<UpdateUserProfile>) -> Result<()> {
        handler_increment_hosted_stays(ctx)
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

    //-----------------------------------------------------------
    // Listing mutators
    //----------------------------------------------------------

    pub fn update_listing_state(
        ctx: Context<UpdateListing>,
        state: [u8; 32],
        content_ref: [u8; 32],
    ) -> Result<()> {
        handler_update_listing_state(ctx, state, content_ref)
    }

    pub fn update_listing_price(ctx: Context<UpdateListing>, price: u64) -> Result<()> {
        handler_update_listing_price(ctx, price)
    }

    pub fn update_listing_publish(ctx: Context<UpdateListing>, active: bool) -> Result<()> {
        handler_update_listing_publish(ctx, active)
    }
}
