use anchor_lang::prelude::error_code;

#[error_code]
pub enum EscrowError {
    // Booking dates
    #[msg("Invalid booking dates: check-in must be before check-out and in the future")]
    InvalidBookingDates,
    #[msg("Dates already booked for this property")]
    DatesAlreadyBooked,
    #[msg("Dates are not booked for this property")]
    DatesUnbooked,
    #[msg("Invalid BookingDays account for the given dates")]
    InvalidBookingDaysAccount,
    #[msg("Invalid month")]
    InvalidMonth,
    #[msg("Uninitialized BookingDays account")]
    UninitializedBookingDays,

    // Booking state machine
    #[msg("Invalid booking status for this action")]
    InvalidBookingStatus,
    #[msg("Booking must be in Active status to complete the stay")]
    BookingNotActive,
    #[msg("Booking must be in ReviewCompleted status to complete the stay")]
    BookingNotReviewCompleted,
    #[msg("Too early to activate booking — check-in must be within 24 h")]
    TooEarlyToActivate,

    // Auth
    #[msg("Only the client can perform this action on their booking")]
    UnauthorizedBooking,
    #[msg("Only the host can perform this action")]
    UnauthorizedHost,
    #[msg("Host cannot book their own property")]
    HostCannotBookOwnProperty,
    #[msg("Invalid host for this property")]
    InvalidHost,
    #[msg("Invalid booking property")]
    InvalidBookingProperty,
    #[msg("Invalid host for this booking")]
    InvalidHostBooking,

    // User state
    #[msg("User is banned")]
    UserBanned,
    #[msg("User is not verified")]
    UserNotVerified,
    #[msg("Host not verified")]
    HostNotVerified,
    #[msg("User does not have enough deposit to perform this action")]
    InsufficientDeposit,
    #[msg("User is not registered as a host")]
    UserNotHost,

    // Scores
    #[msg("Invalid score — must be between 1 and 5")]
    InvalidScore,

    // Token
    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,
    #[msg("The treasury/vault account does not match the configured one")]
    InvalidVaultAccount,
    #[msg("Wrong guest pubkey passed")]
    WrongGuestPassed,

    // Config
    #[msg("Unauthorized admin action")]
    UnauthorizedAdmin,
}
