use anchor_lang::prelude::*;

#[constant]
pub const ESCROW_PDA_SEED: &str = "escrow";

#[constant]
pub const ESCROW_CONFIG_SEED: &str = "escrow_config";

#[constant]
pub const BOOKING_DAYS_SEED: &str = "booking_days";

#[constant]
pub const BOOKING_SEED: &str = "booking";

#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";

// ---------------------------------------------------------------------------
// Cancellation policy.
//
// Hardcoded for the MVP. In a future iteration these values will be sourced
// from the `Listing` or the `UserProfile` instead of being compile-time
// constants.
// ---------------------------------------------------------------------------

/// Number of hours before `check_in` inside which a guest cancellation is
/// subject to the refund split instead of a full refund.
pub const CANCELLATION_WINDOW_HOURS: u32 = 72;

/// Percentage of `total_price` refunded to the guest when cancelling inside
/// the cancellation window.
pub const CANCELLATION_REFUND_PERCENTAGE: u8 = 60;

/// Share (percentage) of the post-refund remainder paid to the host when a
/// guest cancels inside the window. Stayke keeps the rest — derived as the
/// remainder so the three amounts always sum exactly to `total_price`.
pub const CANCELLATION_HOST_SHARE_PERCENTAGE: u8 = 75;

/// Percentage of the host's available deposit slashed when the host cancels
/// before check-in.
pub const HOST_CANCELLATION_PENALTY_PERCENTAGE: u8 = 10;
