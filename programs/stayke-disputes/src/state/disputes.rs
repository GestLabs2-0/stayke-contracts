use anchor_lang::prelude::*;

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum DisputeReason {
    PropertyNotAsDescribed,
    HostUnreachable,
    GuestDamagedProperty,
    GuestBrokeRules,
    Other,
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum DisputeStatus {
    Open,
    Resolved,
    Rejected,
}

#[account]
#[derive(InitSpace)]
pub struct Dispute {
    pub booking: Pubkey,
    pub property: Pubkey,
    pub initiator: Pubkey,
    pub guilty: Pubkey,
    pub reason: DisputeReason,
    pub status: DisputeStatus,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
    pub bump: u8,
}
