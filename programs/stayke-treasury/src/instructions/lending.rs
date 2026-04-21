use anchor_lang::prelude::*;

use crate::error::TreasuryError;

// ---------------------------------------------------------------------------
// Placeholder: Lend
// ---------------------------------------------------------------------------
// TODO: In the future this instruction will allow users to lend their USDC
// into yield-bearing protocols (e.g., MarginFi, Kamino) via CPI, so the
// deposited guarantee earns passive yield while it's not used for a booking.

#[derive(Accounts)]
pub struct Lend<'info> {
    pub signer: Signer<'info>,
    // TODO: Add lending protocol accounts
}

pub fn handler_lend(_ctx: Context<Lend>, _amount: u64) -> Result<()> {
    err!(TreasuryError::LendingNotEnabled)
}

// ---------------------------------------------------------------------------
// Placeholder: Withdraw from Lending
// ---------------------------------------------------------------------------
// TODO: In the future this instruction will allow users to recall their USDC
// from the lending protocol back into the treasury vault.

#[derive(Accounts)]
pub struct WithdrawFromLending<'info> {
    pub signer: Signer<'info>,
    // TODO: Add lending protocol accounts
}

pub fn handler_withdraw_from_lending(_ctx: Context<WithdrawFromLending>, _amount: u64) -> Result<()> {
    err!(TreasuryError::LendingNotEnabled)
}

// ---------------------------------------------------------------------------
// Placeholder: Stake
// ---------------------------------------------------------------------------
// TODO: Liquid staking placeholder (e.g., stake USDC or SOL into a protocol
// like Marinade / Jito and receive an stToken in exchange).

#[derive(Accounts)]
pub struct Stake<'info> {
    pub signer: Signer<'info>,
    // TODO: Add staking protocol accounts
}

pub fn handler_stake(_ctx: Context<Stake>, _amount: u64) -> Result<()> {
    err!(TreasuryError::LendingNotEnabled)
}
