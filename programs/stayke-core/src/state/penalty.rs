use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace, Debug, Copy)]
pub enum PenaltySeverity {
    Low,
    Medium,
    High,
}
