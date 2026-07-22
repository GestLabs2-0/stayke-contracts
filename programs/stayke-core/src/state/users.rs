use anchor_lang::prelude::*;

/// User profile account to store central data
#[account]
#[derive(InitSpace)]
pub struct UserProfile {
    pub authority: Pubkey,
    /// Identity pubkey linked to IdentityAccount
    pub identity: Option<Pubkey>,
    /// Refers to the active booking paid by the user
    pub active_booking: Option<Pubkey>,
    /// This field represents the total amount of tokens that the user has deposited in the platform, excluding the ones that are currently being used for lending and staking.
    pub deposited: u64,
    ///This field represents the total amount of tokens that the user has lent
    pub lending: u64,
    /// This field represents the amount of liquid staked tokens that the user has.
    pub staked: u64,
    /// This might be deleted in the future, but for now I think it's better to have it here so we don't call identity account every time we need to check if the user is banned or not.
    pub banned: bool,
    /// Counter for the amount of listings that the user has created, this is used to generate the listing_id for each listing created by the user.
    pub listings: u16,
    pub bump: u8,
}

// Should I handle the reputation of the user in the same account or should I create a separate account for that? I think it would be better to create a separate account for the reputation.
// This way we have control only over the reputation
#[account]
#[derive(InitSpace)]
pub struct ReputationProfile {
    pub authority: Pubkey,
    /// Number of reviews received as host
    pub host_reviews: u32,
    /// Total score from reviews (e.g., sum of ratings)
    pub total_score_host: u64,
    /// Number of reviews received as client
    pub client_reviews: u32,
    /// Total score from reviews (e.g., sum of ratings)
    pub total_score_client: u64,
    /// Number of stays hosted
    pub hosted_stays: u32,
    /// Number of stays completed as a guest
    pub completed_stays: u32,
    /// Number of cancellations as host
    pub host_cancellations: u32,
    /// Number of cancellations as client
    pub client_cancellations: u32,
    /// Number of cancellations as host within 24 hours of the stay
    pub host_cancellations_within_48h: u32,
    /// Number of cancellations as host within 24 hours of the stay
    pub client_cancellations_within_48h: u32,
    /// Infraction counter
    pub low_infractions: u8,
    /// Infraction counter
    pub medium_infractions: u8,
    /// Infraction counter
    pub high_infractions: u8,
    pub bump: u8,
}

/// Identity account derived using a hash in the backend from the ID provided by Didit
#[account]
#[derive(InitSpace)]
pub struct Identity {
    pub verified_at: i64,
    pub linked: bool,
    pub bump: u8,
}
