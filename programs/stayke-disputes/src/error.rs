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

    // User errors
    #[msg("User is banned")]
    UserBanned,
    #[msg("User is not verified")]
    UserNotVerified,
}
