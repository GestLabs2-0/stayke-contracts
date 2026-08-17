use anchor_lang::prelude::error_code;

#[error_code]
pub enum DisputeError {
    #[msg("Unauthorized admin action")]
    UnauthorizedAdmin,
    #[msg("Max admins reached")]
    MaxAdminsReached,
    #[msg("At least one admin is required")]
    AtLeastOneAdminRequired,
    #[msg("Cannot remove yourself as admin")]
    CannotRemoveSelf,
    #[msg("Admin not found")]
    AdminNotFound,

    #[msg("Only the guest or host can open a dispute")]
    UnauthorizedDisputeInitiator,
    #[msg("Booking must be in Active or Completed status to open a dispute")]
    BookingNotDisputable,
    #[msg("Dispute is already resolved or rejected")]
    DisputeNotOpen,

    #[msg("Invalid configuration")]
    InvalidFeeBps,

    #[msg("Treasury config is not linked to the provided global config")]
    UnlinkedTreasuryConfig,

    #[msg("Token mint does not match GlobalConfig.usdc_mint")]
    InvalidTokenMint,

    // User errors
    #[msg("User is banned")]
    UserBanned,
    #[msg("User is not verified")]
    UserNotVerified,
}
