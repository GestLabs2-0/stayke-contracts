use anchor_lang::prelude::*;

#[error_code]
pub enum StaykeConfigError {
    #[msg("Invalid fee basis points. Must be less than 10000.")]
    InvalidFeeBps,

    #[msg("Invalid global config")]
    InvalidGlobalConfig,
}
