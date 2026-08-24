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
    #[msg("Single year booking invalid")]
    SingleYearBookingInvalid,
    #[msg("Cross year booking invalid")]
    CrossYearBookingInvalid,
    #[msg("Single year unbooking invalid")]
    SingleYearUnbookingInvalid,
    #[msg("Cross year unbooking invalid")]
    CrossYearUnbookingInvalid,

    // Booking state machine
    #[msg("Invalid booking status for this action")]
    InvalidBookingStatus,
    #[msg("Booking must be in Active status to complete the stay")]
    BookingNotActive,
    #[msg("Booking must be in ReviewCompleted status to complete the stay")]
    BookingNotReviewCompleted,
    #[msg("Too early to start booking — check-in time not reached")]
    TooEarlyToActivate,
    #[msg("Exceeded time to accept booking")]
    ExceededAcceptTime,

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
    #[msg("Client already has an active booking")]
    ActiveBookingExists,

    // Scores
    #[msg("Invalid score — must be between 1 and 5")]
    InvalidScore,

    // Token
    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,
    #[msg("Insufficient funds to cover the booking")]
    InsufficientFunds,
    #[msg("Price calculation overflow")]
    PriceOverflow,
    #[msg("The treasury/vault account does not match the configured one")]
    InvalidVaultAccount,
    #[msg("Wrong guest pubkey passed")]
    WrongGuestPassed,
    #[msg("Payout token account is not owned by the booking party")]
    InvalidPayoutTokenAccount,

    // Expire booking
    #[msg("Pending booking must be over 24h")]
    NotOver24Hours,

    // Config
    #[msg("Unauthorized admin action")]
    UnauthorizedAdmin,

    // Booking lifecycle (permissionless transitions) — appended at the end to
    // preserve the numeric error codes of all previously shipped variants.
    #[msg("Booking must be in HostAccepted status to start")]
    BookingNotAccepted,
    #[msg("Too early to complete booking — check-out time not reached")]
    TooEarlyToComplete,

    // Release funds
    #[msg("Booking must be in Completed status to release funds")]
    BookingNotCompleted,
    #[msg("Release window (24h) has not elapsed")]
    ReleaseWindowNotElapsed,
    #[msg("Booking must be in Active or Completed status to open a dispute")]
    BookingNotDisputable,

    // Reviews — appended at the end to preserve the numeric error codes of all
    // previously shipped variants.
    #[msg("Review has already been submitted for this booking")]
    ReviewAlreadySubmitted,

    // Cancellation — appended at the end to preserve the numeric error codes of
    // all previously shipped variants.
    #[msg("Booking can only be cancelled before check-in")]
    CheckInPassed,
    #[msg("Only the guest or host of the booking can cancel it")]
    UnauthorizedCancellation,
    #[msg("Invalid cancellation window")]
    InvalidCancellationWindow,
    #[msg("Invalid cancellation percentage")]
    InvalidCancellationPercentage,

    #[msg("Profile does not match booking")]
    ProfileUnmatchBooking,
    #[msg("Victim and guilty can not be the same")]
    NotAllowedSameProfile,

    #[msg("Invalid listing")]
    InvalidListing,
}
