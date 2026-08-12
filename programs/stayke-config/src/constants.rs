use anchor_lang::prelude::*;

#[constant]
pub const GLOBAL_CONFIG_SEED: &str = "global_config";

#[constant]
pub const PLATFORM_VAULT_SEED: &str = "platform_vault";

#[constant]
pub const PLATFORM_VAULT_CONFIG_SEED: &str = "platform_vault_token";

/// Seed used by treasury/escrow/disputes to derive their CPI authority PDA.
#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";
