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
    #[msg("Booking must be in Active status to open a dispute")]
    BookingNotActive,
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

    #[msg("Profile or listing is not bound to this booking")]
    UnboundBookingAccount,
    #[msg("Reputation profile does not belong to the penalized user")]
    InvalidReputationProfile,
    #[msg("Payout token account is not owned by the booking party")]
    InvalidPayoutTokenAccount,
    #[msg("Token account is not owned by the affected wallet")]
    InvalidAffectedTokenAccount,
}
