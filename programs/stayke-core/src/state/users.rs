use anchor_lang::prelude::*;

#[account]
pub struct UserProfile {
    pub owner: Pubkey,
    pub identity: Pubkey,

    pub banned: bool,

    pub active_booking: bool,
    pub active_stay: bool,

    pub bump: u8,
}

// Should I handle the reputation of the user in the same account or should I create a separate account for that? I think it would be better to create a separate account for the reputation.
// This way we have control only over the reputation
#[account]
pub struct ReputationProfile {
    pub owner: Pubkey,

    pub bump: u8,
}

// I decided to create an Identity account so whenever an user tries to suplant another identity, it must pass first through verification
// provided by the KYC. In case the user is not verified, the account remains on the blockchain and if other person is the real user
// they can claim the account by providing the correct information to the KYC. This way we can avoid identity theft and also we can have a better control of the users.

#[account]
pub struct Identity {
    pub owner: Pubkey,
    pub country_code: [u8; 2],
    pub id: [u8; 32],
    pub verified_at: i64,         // unix timestamp
    pub verifier: Option<Pubkey>, // who verified (oracle, admin, or the very program)
    pub is_frozen: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum DocType {
    Passport,
    DriversLicense,
    IdCard,
}
