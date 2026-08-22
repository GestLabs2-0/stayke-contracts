use anchor_lang::prelude::*;
use stayke_core::PenaltySeverity;
use stayke_escrow::BookingStatus;

// #[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
// pub enum InfractionSeverity {
//     Low,
//     Medium,
//     High,
//     Max,
// }

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DisputeParty {
    Guest,
    Host,
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DisputeState {
    OpenP2P,
    Escalated,
    ResolvedByP2P,
    ResolvedByAdmin,
    Closed,
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum DisputeOutcome {
    GuestFavored { severity: PenaltySeverity },
    HostFavored { severity: PenaltySeverity },
    NoFaultFound,
    MaliciousClaim { severity: PenaltySeverity },
}

#[account]
#[derive(InitSpace)]
pub struct DisputeAccount {
    pub booking: Pubkey,
    pub opened_by: DisputeParty,
    pub state: DisputeState,
    pub opened_at: i64,
    pub guest_evidence: Option<[u8; 32]>,
    pub host_evidence: Option<[u8; 32]>,
    pub outcome: Option<DisputeOutcome>,
    pub original_booking_status: BookingStatus,
    pub bump: u8,
}
