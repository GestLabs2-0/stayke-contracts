use anchor_lang::prelude::*;

#[constant]
pub const LISTING_SEED: &str = "listing";

#[constant]
pub const USER_PROFILE_SEED: &str = "user_profile";

#[constant]
pub const REPUTATION_PROFILE_SEED: &str = "reputation_profile";

#[constant]
pub const IDENTITY_SEED: &str = "identity";

#[constant]
pub const CORE_CONFIG_SEED: &str = "config";

/// Seed used by treasury/escrow/disputes to derive their CPI authority PDA.
#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";
