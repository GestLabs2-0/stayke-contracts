use anchor_lang::prelude::*;

#[account]
pub struct ConfigAcc {
    pub authority: Pubkey,

    pub bump: u8,
}
