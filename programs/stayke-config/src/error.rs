use anchor_lang::prelude::*;

#[error_code]
pub enum StaykeConfigError {
    #[msg("Invalid fee basis points. Must be less than 10000.")]
    InvalidFeeBps,

    #[msg("Invalid global config")]
    InvalidGlobalConfig,

    // Config/Treasury errors
    #[msg("The treasury/vault account does not match the configured one")]
    InvalidVaultAccount,
    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,
}
