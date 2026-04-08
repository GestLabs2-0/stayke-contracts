use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct ConfigAcc {
    pub authority: Pubkey,

    pub bump: u8,
}
