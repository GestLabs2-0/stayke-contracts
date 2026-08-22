use anchor_lang::prelude::*;

#[constant]
pub const DISPUTE_PDA_SEED: &str = "dispute";

#[constant]
pub const DISPUTE_CONFIG_PDA_SEED: &str = "dispute_config";

#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";

/// Duration (seconds) of the P2P resolution window. Once elapsed, any party can
/// escalate the dispute to admin review.
#[constant]
pub const DISPUTE_P2P_WINDOW_SECONDS: i64 = 86_400;

pub const ESCROW_SLASH_MEDIUM_BPS: u16 = 5_000;
pub const ESCROW_SLASH_HIGH_BPS: u16 = 10_000;
pub const DEPOSIT_SLASH_MEDIUM_BPS: u16 = 3_000;
pub const DEPOSIT_SLASH_HIGH_BPS: u16 = 10_000;
pub const STAYKE_FEE_BPS: u16 = 1_000;
pub const BPS_DIVISOR: u16 = 10_000;
