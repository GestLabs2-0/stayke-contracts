use anchor_lang::prelude::*;

use crate::state::BookingStatus;

#[event]
pub struct NewBookingEvent {
    pub guest: Pubkey,
    pub booking: Pubkey,
    pub property: Pubkey,
    pub host: Pubkey,
    pub status: BookingStatus,
    pub check_in: i64,
    pub check_out: i64,
}

#[event]
pub struct BookingStatusUpdated {
    pub booking: Pubkey,
    pub status: BookingStatus,
}

#[event]
pub struct BookingExpired {
    pub booking: Pubkey,
    pub guest: Pubkey,
    pub host: Pubkey,
}

#[event]
pub struct ReviewSubmitted {
    pub booking: Pubkey,
    pub reviewer: Pubkey,
    pub rating: u8,
    pub is_host_review: bool,
}
