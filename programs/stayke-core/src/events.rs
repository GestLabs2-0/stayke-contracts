use anchor_lang::prelude::*;

/// Emitted when a guest review is accumulated into a listing's rating.
#[event]
pub struct ListingReviewSubmitted {
    pub listing: Pubkey,
    pub reviewer: Pubkey,
    pub score: u8,
    pub total_reviews: u64,
}

#[event]
pub struct UserBanned {
    pub identity: Option<Pubkey>,
    pub reputation_profile: Pubkey,
    pub user_profile: Pubkey,
}
