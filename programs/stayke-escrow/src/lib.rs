#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use error::*;
pub use instructions::{cpi::*, *};
pub use state::*;

declare_id!("68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a");

#[program]
pub mod stayke_escrow {
    use super::*;

    // ---------------------------------------------------------------------------
    // Admin
    // ---------------------------------------------------------------------------

    pub fn initialize_escrow(ctx: Context<InitializeConfigEscrow>) -> Result<()> {
        handler_initialize_config_escrow(ctx)
    }

    // ---------------------------------------------------------------------------
    // Booking lifecycle
    // ---------------------------------------------------------------------------

    pub fn create_booking(
        ctx: Context<CreateBooking>,
        check_in: i64,
        check_out: i64,
    ) -> Result<()> {
        handler_create_booking(ctx, check_in, check_out)
    }

    pub fn create_booking_cross_year(
        ctx: Context<CreateBookingCrossYear>,
        check_in: i64,
        check_out: i64,
    ) -> Result<()> {
        handler_create_booking_cross_year(ctx, check_in, check_out)
    }

    pub fn host_accept_booking(ctx: Context<HostAcceptBooking>) -> Result<()> {
        handler_host_accept_booking(ctx)
    }

    pub fn booking_starts(ctx: Context<BookingStarts>) -> Result<()> {
        handler_booking_starts(ctx)
    }

    pub fn booking_completes(ctx: Context<BookingCompletes>) -> Result<()> {
        handler_booking_completes(ctx)
    }

    pub fn host_reject_booking(ctx: Context<HostRejectBooking>) -> Result<()> {
        handler_host_reject_booking(ctx)
    }

    pub fn expire_booking(ctx: Context<ExpireBooking>) -> Result<()> {
        handler_expire_booking(ctx)
    }

    pub fn expire_booking_crossday(ctx: Context<ExpireBookingCrossDays>) -> Result<()> {
        handler_expire_booking_crossday(ctx)
    }

    pub fn host_reject_booking_cross_year(ctx: Context<HostRejectBookingCrossYear>) -> Result<()> {
        handler_host_reject_booking_cross_year(ctx)
    }

    pub fn client_reject_reserve(ctx: Context<ClientRejectReserve>, check_in: i64) -> Result<()> {
        handler_client_reject_reserve(ctx, check_in)
    }

    pub fn client_reject_reserve_cross_year(
        ctx: Context<ClientRejectReserveCrossYear>,
        check_in: i64,
    ) -> Result<()> {
        handler_client_reject_reserve_cross_year(ctx, check_in)
    }

    pub fn review_completed(ctx: Context<CloseBooking>, score: u8) -> Result<()> {
        handler_review_completed(ctx, score)
    }

    pub fn release_funds(ctx: Context<ReleaseFunds>) -> Result<()> {
        handler_release_funds(ctx)
    }

    pub fn host_review(ctx: Context<HostReview>, score: u8) -> Result<()> {
        handler_host_review(ctx, score)
    }

    pub fn guest_review(ctx: Context<GuestReview>, score: u8) -> Result<()> {
        handler_guest_review(ctx, score)
    }

    // ---------------------------------------------------------------------------
    // CPI endpoints for stayke-disputes
    // ---------------------------------------------------------------------------

    pub fn cpi_update_booking_status(
        ctx: Context<UpdateBookingStatusCpi>,
        status: BookingStatus,
    ) -> Result<()> {
        handler_cpi_update_booking_status(ctx, status)
    }

    pub fn cpi_resolve_dispute_transfer(
        ctx: Context<ResolveDisputeTransferCpi>,
        host_share_bps: u16,
        rejected: bool,
    ) -> Result<()> {
        handler_cpi_resolve_dispute_transfer(ctx, host_share_bps, rejected)
    }
}
