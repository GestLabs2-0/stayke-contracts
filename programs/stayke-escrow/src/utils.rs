use anchor_lang::prelude::*;

use crate::{BookingDays, EscrowError};

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct DateComponents {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub leap_year: bool,
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
        leap_year: year % 4 == 0,
    }
}

pub trait TimestampExt {
    fn to_date(&self) -> DateComponents;
    fn year(&self) -> u32;
}

impl TimestampExt for i64 {
    fn to_date(&self) -> DateComponents {
        derive_date(*self)
    }

    fn year(&self) -> u32 {
        derive_date(*self).year
    }
}

// ---------------------------------------------------------------------------
// Helpers: bitmap operations for day-occupancy tracking
// ---------------------------------------------------------------------------

pub fn months_days(month: u32, leap_year: bool) -> Result<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => {
            if leap_year {
                return Ok(29);
            }
            Ok(28)
        } // Not accounting for leap years for simplicity
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

pub fn reserve_days_single_year(
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    // Checks that bookings days already have a year which means that was initialized
    if booking_days.year == 0 {
        booking_days.year = check_in.year;
        booking_days.occupied_days = [0u32; 12];
    }
    require!(
        check_out.year == check_in.year,
        EscrowError::SingleYearBookingInvalid
    );
    require!(
        check_in.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        check_out.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=check_out.month {
        let idx = (month - 1) as usize;
        let in_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };

        let end_day = if month == check_out.month {
            check_out.month
        } else {
            months_days(month, check_in.leap_year)?
        };

        let mask = bitmap_days(in_day, end_day);
        require!(
            booking_days.occupied_days[idx] & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days[idx] |= mask;
    }

    Ok(())
}

pub fn reserve_days_cross_years(
    booking_days: &mut Account<'_, BookingDays>,
    booking_days_next: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    // Checks that bookings days already have a year which means that was initialized
    if booking_days.year == 0 {
        booking_days.year = check_in.year;
        booking_days.occupied_days = [0u32; 12];
    }
    if booking_days_next.year == 0 {
        booking_days_next.year = check_in.year;
        booking_days_next.occupied_days = [0u32; 12];
    }

    require!(
        check_in.year + 1 == check_out.year,
        EscrowError::CrossYearBookingInvalid
    );
    require!(
        check_in.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        check_out.year == booking_days_next.year,
        EscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=12 {
        let idx = (month - 1) as usize;
        let in_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };
        let end_day = months_days(month, check_in.leap_year)?;

        let mask = bitmap_days(in_day, end_day);
        require!(
            booking_days.occupied_days[idx] & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days[idx] |= mask;
    }

    for month in 1..=check_out.month {
        let idx = (month - 1) as usize;

        let end_day = if month == check_out.month {
            check_out.month
        } else {
            months_days(month, check_out.leap_year)?
        };

        let mask = bitmap_days(1, end_day);

        require!(
            booking_days_next.occupied_days[idx] & mask == 0,
            EscrowError::DatesAlreadyBooked
        );
        booking_days_next.occupied_days[idx] |= mask;
    }

    Ok(())
}

pub fn release_days_single_year(
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    require!(booking_days.year > 0, EscrowError::UninitializedBookingDays);
    require!(
        check_out.year == check_in.year,
        EscrowError::SingleYearUnbookingInvalid
    );
    require!(
        check_in.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        check_out.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=check_out.month {
        let idx = (month - 1) as usize;
        let in_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };

        let end_day = if month == check_out.month {
            check_out.month
        } else {
            months_days(month, check_in.leap_year)?
        };

        let mask = bitmap_days(in_day, end_day);
        require!(
            booking_days.occupied_days[idx] & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days[idx] &= !mask;
    }

    Ok(())
}

pub fn release_days_cross_years(
    booking_days: &mut Account<'_, BookingDays>,
    booking_days_next: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    // Checks that bookings days already have a year which means that was initialized
    require!(booking_days.year > 0, EscrowError::UninitializedBookingDays);
    require!(
        booking_days_next.year > 0,
        EscrowError::UninitializedBookingDays
    );

    require!(
        check_in.year + 1 == check_out.year,
        EscrowError::CrossYearBookingInvalid
    );
    require!(
        check_in.year == booking_days.year,
        EscrowError::InvalidBookingDaysAccount
    );
    require!(
        check_out.year == booking_days_next.year,
        EscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=12 {
        let idx = (month - 1) as usize;
        let in_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };
        let end_day = months_days(month, check_in.leap_year)?;

        let mask = bitmap_days(in_day, end_day);
        require!(
            booking_days.occupied_days[idx] & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days[idx] &= !mask;
    }

    for month in 1..=check_out.month {
        let idx = (month - 1) as usize;

        let end_day = if month == check_out.month {
            check_out.month
        } else {
            months_days(month, check_out.leap_year)?
        };

        let mask = bitmap_days(1, end_day);

        require!(
            booking_days.occupied_days[idx] & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days.occupied_days[idx] &= !mask;
    }

    Ok(())
}
