pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ");

#[program]
pub mod stayke_escrow {
    use super::*;

    // ---------------------------------------------------------------------------
    // Admin
    // ---------------------------------------------------------------------------

    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        fee_bps: u16,
        minimum_deposit: u64,
        price_per_night: u64,
    ) -> Result<()> {
        initialize::handler_initialize_escrow(ctx, fee_bps, minimum_deposit, price_per_night)
    }

    // ---------------------------------------------------------------------------
    // Booking lifecycle
    // ---------------------------------------------------------------------------

    pub fn create_booking(
        ctx: Context<CreateBooking>,
        check_in: i64,
        check_out: i64,
    ) -> Result<()> {
        booking::handler_create_booking(ctx, check_in, check_out)
    }

    pub fn host_accept_booking(ctx: Context<HostAcceptBooking>) -> Result<()> {
        booking::handler_host_accept_booking(ctx)
    }

    pub fn host_reject_booking(ctx: Context<HostRejectBooking>) -> Result<()> {
        booking::handler_host_reject_booking(ctx)
    }

    pub fn client_accept_reserve(ctx: Context<ClientAcceptReserve>) -> Result<()> {
        booking::handler_client_accept_reserve(ctx)
    }

    pub fn client_reject_reserve(ctx: Context<ClientRejectReserve>) -> Result<()> {
        booking::handler_client_reject_reserve(ctx)
    }

    pub fn complete_stay(ctx: Context<CompleteStay>) -> Result<()> {
        booking::handler_complete_stay(ctx)
    }

    pub fn close_booking(ctx: Context<CloseBooking>, score: u8) -> Result<()> {
        booking::handler_close_booking(ctx, score)
    }

    // ---------------------------------------------------------------------------
    // CPI endpoints for stayke-disputes
    // ---------------------------------------------------------------------------

    pub fn cpi_update_booking_status(
        ctx: Context<UpdateBookingStatusCpi>,
        status: BookingStatus,
    ) -> Result<()> {
        dispute_cpi::handler_cpi_update_booking_status(ctx, status)
    }

    pub fn cpi_resolve_dispute_transfer(
        ctx: Context<ResolveDisputeTransferCpi>,
        host_share_bps: u16,
        rejected: bool,
    ) -> Result<()> {
        dispute_cpi::handler_cpi_resolve_dispute_transfer(ctx, host_share_bps, rejected)
    }
}
