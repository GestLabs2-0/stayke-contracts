use anchor_lang::prelude::error_code;

#[error_code]
pub enum TreasuryError {
    // Config
    #[msg("Treasury is already initialized")]
    AlreadyInitialized,

    // Deposit
    #[msg("Deposit amount is below the minimum required")]
    DepositTooLow,
    #[msg("The treasury vault account does not match the configured one")]
    InvalidTreasuryVault,
    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,

    // Withdraw
    #[msg("Insufficient guarantee balance to withdraw the requested amount")]
    InsufficientBalance,
    #[msg("Withdrawal amount must be greater than zero")]
    ZeroWithdrawal,
    #[msg("User has an active booking and cannot withdraw their guarantee")]
    ActiveBookingExists,
    #[msg("User is banned and cannot perform this action")]
    UserBanned,

    // Auth
    #[msg("Only the authority can perform this action")]
    Unauthorized,

    // Lending (reserved for future use)
    #[msg("Lending is not yet enabled")]
    LendingNotEnabled,
}
