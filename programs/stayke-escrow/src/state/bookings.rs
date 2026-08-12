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

// impl BookingDays {
//     pub fn year_month(&self) -> u32 {
//         self.year * 100 + self.month
//     }
// }

#[derive(InitSpace, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum BookingStatus {
    Pending,
    HostAccepted,
    ClientAccepted,
    Active,
    ReviewCompleted,
    Completed,
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
    /// Was money already deposited?
    pub is_deposit: bool,
    /// Check in as unix timestamp
    pub check_in: i64,
    /// Check out as unix timestamp
    pub check_out: i64,
    /// Total price by all nights
    pub total_price: u64,
    /// Booking Status
    pub status: BookingStatus,
    /// Bump of the escrow token account PDA — needed to sign CPIs in complete_stay.
    pub escrow_bump: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Review {
    /// Pending = 0. Review goes from 1 to 5
    host_review: u8,
    /// Pending = 0. Review goes from 1 to 5
    guest_review: u8,
    bump: u8,
}
