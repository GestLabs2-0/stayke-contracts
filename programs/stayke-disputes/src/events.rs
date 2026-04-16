use anchor_lang::prelude::*;

use crate::state::DisputeReason;

#[event]
pub struct DisputeOpened {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub property: Pubkey,
    pub initiator: Pubkey,
    pub reason: DisputeReason,
    pub timestamp: i64,
}

#[event]
pub struct DisputeResolved {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub host_share_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct UserPenalized {
    pub penalized_user: Pubkey,
    pub affected_wallet: Pubkey,
    pub penalty_amount: u64,
    pub timestamp: i64,
}
