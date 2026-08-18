use anchor_lang::prelude::*;

/// Emitted when a guest review is accumulated into a listing's rating.
#[event]
pub struct ListingReviewSubmitted {
    pub listing: Pubkey,
    pub reviewer: Pubkey,
    pub score: u8,
    pub total_reviews: u64,
}
