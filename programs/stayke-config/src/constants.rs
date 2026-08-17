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

/// Compiled Stayke program IDs persisted on `GlobalConfig` at init.
/// Must match each crate's `declare_id!` and `Anchor.toml` `[programs.devnet]`.
pub const CORE_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm");
pub const ESCROW_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a");
pub const DISPUTES_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("89yo4qWuvaQcAPtAcutNB6vht3JwvEwMMLbSwpMM2Czt");
pub const TREASURY_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("3JE5y7vtjkZkA6s3eRAKorT1eQmgoJQmnVqpy15uUjq8");
