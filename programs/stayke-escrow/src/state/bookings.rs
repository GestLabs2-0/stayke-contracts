use anchor_lang::prelude::*;

/// Tracks which calendar days are occupied for a given property in a given year.
/// Uses bitwise operations on a u32 (max 31 days).
#[account]
#[derive(InitSpace)]
pub struct BookingDays {
    pub occupied_days: [u32; 12],
    pub year: u32,
    pub bump: u8,
}

#[derive(InitSpace, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum BookingStatus {
    Pending,
    HostAccepted,
    Active,
    Completed,
    Released,
    Cancelled,
    Disputed,
    DisputeResolved,
    DisputeRejected,
}

#[account]
#[derive(InitSpace)]
pub struct Booking {
    /// Guest Pubkey
    pub guest: Pubkey,
    /// Host Pubkey
    pub host: Pubkey,
    /// Property pubkey
    pub property: Pubkey,
    /// Check in as unix timestamp
    pub check_in: i64,
    /// Check out as unix timestamp
    pub check_out: i64,
    /// Total price by all nights
    pub total_price: u64,
    /// Host review - if review == 0 then review wasn't set. Range from 1 - 5
    pub host_review: u8,
    /// Guest review - if review == 0 then review wasn't set. Range from 1 - 5
    pub guest_review: u8,
    /// Booking Status
    pub status: BookingStatus,
    /// Bump of the escrow token account PDA — needed to sign CPIs in complete_stay.
    pub escrow_bump: u8,
    /// Works as a timer for finishing a booking, accepting or cancelling one
    pub updated_at: i64,
    pub bump: u8,
}
