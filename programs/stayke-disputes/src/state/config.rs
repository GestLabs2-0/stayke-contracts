use anchor_lang::prelude::*;

pub const MAX_ADMINS: usize = 5;

#[account]
#[derive(InitSpace)]
pub struct DisputeConfig {
    #[max_len(MAX_ADMINS)]
    pub admins: Vec<Pubkey>,

    /// The basis points taken from the penalty and given to the affected party.
    /// Default values for penalties.
    pub retribution_bps_low: u16,
    pub retribution_bps_medium: u16,
    pub retribution_bps_high: u16,

    pub is_initialized: bool,
    pub bump: u8,
}
