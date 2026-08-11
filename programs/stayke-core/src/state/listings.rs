use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Listing {
    pub owner: Pubkey,

    pub listing_id: u16,

    pub total_reviews: u64,

    pub rating: u64,

    pub price: u64,

    pub is_occupied: bool,
    // This hash will be a representation of the data of the listing, such as the name, description, location, and other relevant information.
    // This way we can ensure that the data of the listing is not tampered.
    pub state_hash: [u8; 32],

    /// Arweave transaction ID (32 bytes, Base64url-encoded off-chain).
    /// Backend URL
    /// IPFS CID v1
    /// Resolves to: https://arweave.net/{base64url(arweave_tx_id)}
    pub content_ref: [u8; 32],

    pub bump: u8,
}
