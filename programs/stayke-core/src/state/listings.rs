use anchor_lang::prelude::*;

#[account]
pub struct Listing {
    pub authority: Pubkey,

    pub total_reviews: u64,

    pub rating: u64,

    pub price: u64,
    pub is_occupied: Option<Pubkey>,

    // This hash will be a representation of the data of the listing, such as the name, description, location, and other relevant information.
    // This way we can ensure that the data of the listing is not tampered.
    pub data_hash: [u8; 32],

    pub bump: u8,
}
