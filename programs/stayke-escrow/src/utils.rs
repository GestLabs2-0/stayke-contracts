use anchor_lang::prelude::*;

use crate::{BookingDays, EscrowError};

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct DateComponents {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub year_month: u32,
}

/// Converts a Unix timestamp (seconds since epoch) to calendar components
/// using Howard Hinnant's civil date algorithm.
pub fn derive_date(unix_timestamp: i64) -> DateComponents {
    let days_since_epoch = (unix_timestamp / 86400) as u32;

    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    DateComponents {
        year,
        month,
        day,
        year_month: year * 100 + month,
    }
}

pub trait TimestampExt {
    fn to_date(&self) -> DateComponents;
    fn year_month(&self) -> u32;
}

impl TimestampExt for i64 {
    fn to_date(&self) -> DateComponents {
        derive_date(*self)
    }

    fn year_month(&self) -> u32 {
        derive_date(*self).year_month
    }
}

// ---------------------------------------------------------------------------
// Helpers: bitmap operations for day-occupancy tracking
// ---------------------------------------------------------------------------

pub fn months_days(month: u32) -> Result<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => Ok(28), // Not accounting for leap years for simplicity
        _ => err!(EscrowError::InvalidMonth),
    }
}

pub fn bitmap_days(start_day: u32, end_day: u32) -> u32 {
    let mut mask: u32 = 0;
    for day in start_day..=end_day {
        mask |= 1 << (day - 1) as usize;
    }
    mask
}

// ---------------------------------------------------------------------------
// Reserve / Release day-bitmap helpers
// ---------------------------------------------------------------------------

pub fn reserve_days<'a>(
    remaining_accounts: &'a [AccountInfo<'a>],
    property: Pubkey,
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    if booking_days.initialized {
        require!(
            booking_days.property == property,
            EscrowError::InvalidBookingDaysAccount
        );
    } else {
        booking_days.property = property;
        booking_days.month = check_in.month;
        booking_days.year = check_in.year;
        booking_days.occupied_days = 0;
        booking_days.initialized = true;
    }

    require!(
        booking_days.month == check_in.month,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        booking_days.year == check_in.year,
        EscrowError::InvalidBookingDaysAccount
    );

    if booking_days.month == check_out.month && booking_days.year == check_out.year {
        let mask = bitmap_days(check_in.day, check_out.day);
        require!(
            booking_days.occupied_days & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days |= mask;
    } else {
        let end_day = months_days(booking_days.month)?;
        let mask = bitmap_days(check_in.day, end_day);
        require!(
            booking_days.occupied_days & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days |= mask;

        let years_to_reserve = if check_out.year > check_in.year {
            check_out.year.saturating_sub(check_in.year)
        } else {
            0
        };

        for (account_info, month) in remaining_accounts
            .iter()
            .zip(check_in.month + 1..=check_out.month + 12 * years_to_reserve)
        {
            let actual_month = month % 12;
            let mut bd = Account::<BookingDays>::try_from(account_info)?;
            if !bd.initialized {
                bd.property = property;
                bd.month = actual_month;
                let actual_year = check_in.year + month.div_euclid(12);
                bd.year = actual_year;
                bd.occupied_days = 0;
                bd.initialized = true;
            }
            require!(
                bd.property == property,
                EscrowError::InvalidBookingDaysAccount
            );
            require!(
                bd.month == actual_month,
                EscrowError::InvalidBookingDaysAccount
            );

            let end = if actual_month == check_out.month {
                check_out.day
            } else {
                months_days(actual_month)?
            };
            let mask = bitmap_days(1, end);
            require!(
                bd.occupied_days & mask == 0,
                EscrowError::DatesAlreadyBooked
            );
            bd.occupied_days |= mask;
            bd.exit(&crate::ID)?;
        }
    }

    Ok(())
}

pub fn release_days<'a>(
    remaining_accounts: &'a [AccountInfo<'a>],
    booking_property: Pubkey,
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    require!(
        booking_days.initialized,
        EscrowError::UninitializedBookingDays
    );
    require!(
        booking_days.month == check_in.month,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        booking_days.year == check_in.year,
        EscrowError::InvalidBookingDaysAccount
    );

    if booking_days.month == check_out.month && booking_days.year == check_out.year {
        let mask = bitmap_days(check_in.day, check_out.day);
        require!(
            booking_days.occupied_days & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days &= !mask;
    } else {
        let end_day = months_days(booking_days.month)?;
        let mask = bitmap_days(check_in.day, end_day);
        require!(
            booking_days.occupied_days & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days &= !mask;

        let years_to_reserve = if check_out.year > check_in.year {
            check_out.year.saturating_sub(check_in.year)
        } else {
            0
        };

        for (account_info, month) in remaining_accounts
            .iter()
            .zip(check_in.month + 1..=check_out.month + 12 * years_to_reserve)
        {
            let mut bd = Account::<BookingDays>::try_from(account_info)?;
            require!(bd.initialized, EscrowError::UninitializedBookingDays);
            require!(
                bd.property == booking_property,
                EscrowError::InvalidBookingDaysAccount
            );
            let actual_month = month % 12;

            require!(
                bd.month == actual_month,
                EscrowError::InvalidBookingDaysAccount
            );

            let end = if actual_month == check_out.month {
                check_out.day
            } else {
                months_days(actual_month)?
            };
            let mask = bitmap_days(1, end);
            require!(bd.occupied_days & mask == mask, EscrowError::DatesUnbooked);
            bd.occupied_days &= !mask;
            bd.exit(&crate::ID)?;
        }
    }

    Ok(())
}
