use anchor_lang::prelude::*;

use crate::state::DisputeParty;

#[event]
pub struct EvidenceLinked {
    pub evidence: [u8; 32],
    pub dispute: Pubkey,
    pub is_guest: bool,
}

#[event]
pub struct DisputeOpened {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub opened_by: DisputeParty,
    pub opened_at: i64,
}

#[event]
pub struct DisputeEscalated {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub opened_at: i64,
    pub escalated_at: i64,
}

#[event]
pub struct DisputeSolved {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub solved_by: DisputeParty,
    pub solved_at: i64,
}

#[event]
pub struct DisputeResolvedByAdmin {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub resolved_by: Pubkey,
    pub resolved_at: i64,
}

#[event]
pub struct DisputeClosed {
    pub dispute: Pubkey,
    pub booking: Pubkey,
    pub closed_by: DisputeParty,
    pub closed_at: i64,
}

// ---------------------------------------------------------------------------
// DEPRECATED — STK-168 refactor: P2P dispute flow with admin escalation.
// These events belong to the admin-mediated dispute flow being replaced.
// ---------------------------------------------------------------------------
/*
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
    pub penalty_amount: u64,
    pub affected_wallet: Pubkey,
    pub timestamp: i64,
}
*/
