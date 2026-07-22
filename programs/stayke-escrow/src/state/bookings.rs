use anchor_lang::prelude::*;

use crate::utils::DateComponents;
// TODO: refactor bookings days to store all year instead of multiple accounts

/// Tracks which calendar days are occupied for a given property in a given month.
/// Uses bitwise operations on a u32 (max 31 days).
#[account]
#[derive(InitSpace)]
pub struct BookingDays {
    pub property: Pubkey,
    pub occupied_days: u32,
    pub month: u32,
    pub year: u32,
    /// Prevents double-counting when re-using an existing account from a prior booking.
    pub initialized: bool,
    pub bump: u8,
}

impl BookingDays {
    pub fn year_month(&self) -> u32 {
        self.year * 100 + self.month
    }
}

#[derive(InitSpace, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum BookingStatus {
    Pending,
    HostAccepted,
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
    pub guest: Pubkey,
    pub host: Pubkey,
    pub property: Pubkey,
    pub deposit: u64,
    pub check_in: i64,
    pub check_out: i64,
    pub days: u64,
    pub check_in_date: DateComponents,
    pub check_out_date: DateComponents,
    pub total_price: u64,
    /// 0 = no review yet; 1–5 = star rating
    pub review: u8,
    pub status: BookingStatus,
    /// Bump of the escrow token account PDA — needed to sign CPIs in complete_stay.
    pub escrow_bump: u8,
    pub bump: u8,
}
