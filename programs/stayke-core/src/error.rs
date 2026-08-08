use anchor_lang::prelude::*;

#[error_code]
pub enum StaykeError {
    #[msg("Identity is frozen and already linked")]
    IdentityFrozen,
    #[msg("Identity is banned and cannot be used to create a user profile")]
    IdentityBanned,
    #[msg("User profile is not verified, cannot perform this action")]
    UserProfileNotVerified,
    #[msg("User profile already linked")]
    UserProfileAlreadyLinked,

    // Admin Errors
    #[msg("Unauthorized: Only the authority can perform this action")]
    Unauthorized,

    #[msg("Invalid listing ID: The provided listing ID does not match the last existing listing")]
    InvalidListingId,

    #[msg("Max listings reached")]
    MaxListingsReached,

    #[msg("Invalid review score: must be between 1 and 5")]
    InvalidScore,
}
