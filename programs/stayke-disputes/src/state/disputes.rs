use anchor_lang::prelude::*;

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum InfractionSeverity {
    Low,
    Medium,
    High,
    Max,
}

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
    GuestFavored { severity: InfractionSeverity },
    HostFavored { severity: InfractionSeverity },
    NoFaultFound,
    MaliciousClaim { severity: InfractionSeverity },
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
    pub bump: u8,
}
