use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserProfile {
    pub owner: Pubkey,
    pub identity: Pubkey,

    // Refers to the active booking paid by the user
    pub active_booking: Option<Pubkey>,

    // This field represents the total amount of tokens that the user has deposited in the platform, excluding the ones that are currently being used for lending and staking.
    pub deposited: u64,
    pub deposit_timestamp: i64,

    // This field represents the total amount of tokens that the user has lent
    pub lending: u64,
    // This field represents the amount of liquid staked tokens that the user has.
    pub staked: u64,

    pub is_verified: bool,

    pub banned: bool, // This might be deleted in the future, but for now I think it's better to have it here so we don't call identity account every time we need to check if the user is banned or not.

    // Counter for the amount of listings that the user has created, this is used to generate the listing_id for each listing created by the user.
    pub listings: u16,

    pub bump: u8,
}

// Should I handle the reputation of the user in the same account or should I create a separate account for that? I think it would be better to create a separate account for the reputation.
// This way we have control only over the reputation
#[account]
#[derive(InitSpace)]
pub struct ReputationProfile {
    pub owner: Pubkey,

    pub host_reviews: u32,     // Number of reviews received as host
    pub total_score_host: u64, // Total score from reviews (e.g., sum of ratings)

    pub client_reviews: u32,     // Number of reviews received as client
    pub total_score_client: u64, // Total score from reviews (e.g., sum of ratings)

    pub hosted_stays: u32,    // Number of stays hosted
    pub completed_stays: u32, // Number of stays completed as a guest

    pub host_cancellations: u32,   // Number of cancellations as host
    pub client_cancellations: u32, // Number of cancellations as client

    pub host_cancellations_within_48h: u32, // Number of cancellations as host within 24 hours of the stay
    pub client_cancellations_within_48h: u32, // Number of cancellations as host within 24 hours of the stay

    pub low_infractions: u8,
    pub medium_infractions: u8,
    pub high_infractions: u8,

    pub last_updated: i64, // unix timestamp of the last update to the reputation profile

    pub bump: u8,
}

// I decided to create an Identity account so whenever an user tries to suplant another identity, it must pass first through verification
// provided by the KYC. In case the user is not verified, the account remains on the blockchain and if other person is the real user
// they can claim the account by providing the correct information to the KYC. This way we can avoid identity theft and also we can have a better control of the users.

#[account]
#[derive(InitSpace)]
pub struct Identity {
    pub owner: Pubkey,
    pub country_code: [u8; 2],
    pub id: [u8; 32],
    pub verified_at: i64,         // unix timestamp
    pub verifier: Option<Pubkey>, // who verified (oracle, admin, or the very program)
    pub doc_type: DocType,
    pub is_frozen: bool,
    pub is_banned: bool,
    pub banned_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum DocType {
    Passport,
    DriversLicense,
    IdCard,
}
