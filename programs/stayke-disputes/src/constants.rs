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
