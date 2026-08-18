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
            check_out.day
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
        booking_days_next.year = check_out.year;
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
            check_out.day
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
            check_out.day
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
            check_out.day
        } else {
            months_days(month, check_out.leap_year)?
        };

        let mask = bitmap_days(1, end_day);

        require!(
            booking_days_next.occupied_days[idx] & mask == mask,
            EscrowError::DatesUnbooked
        );
        booking_days_next.occupied_days[idx] &= !mask;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests — pure functions
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // derive_date
    // -----------------------------------------------------------------------

    #[test]
    fn derive_date_epoch() {
        let d = derive_date(0);
        assert_eq!(d.year, 1970);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn derive_date_epoch_plus_one_day() {
        let d = derive_date(86400);
        assert_eq!(d.year, 1970);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 2);
    }

    #[test]
    fn derive_date_jan_1_2025() {
        // 2025-01-01 — verified against existing dispute test timestamp.
        let d = derive_date(1735689600);
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn derive_date_dec_31_2024() {
        // Day before Jan 1, 2025.
        let d = derive_date(1735689600 - 86400);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 12);
        assert_eq!(d.day, 31);
    }

    #[test]
    fn derive_date_leap_day_2024() {
        // 2024-02-29 00:00:00 UTC
        let d = derive_date(1709164800);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 2);
        assert_eq!(d.day, 29);
        assert!(d.leap_year);
    }

    #[test]
    fn derive_date_non_leap_feb_2025() {
        // 2025-02-28 — 2025 is not a leap year.
        let d = derive_date(1740700800);
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 2);
        assert_eq!(d.day, 28);
        assert!(!d.leap_year);
    }

    #[test]
    fn derive_date_roundtrip_2026() {
        // 2026-06-15
        let d = derive_date(1781481600);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 15);
        assert!(!d.leap_year);
    }

    #[test]
    fn derive_date_roundtrips_year_boundary() {
        // Dec 31 → Jan 1 transition.
        let dec31 = derive_date(1735603200);
        let jan1 = derive_date(1735689600);
        assert_eq!(dec31.year + 1, jan1.year);
        assert_eq!(dec31.month, 12);
        assert_eq!(dec31.day, 31);
        assert_eq!(jan1.month, 1);
        assert_eq!(jan1.day, 1);
    }

    // -----------------------------------------------------------------------
    // TimestampExt
    // -----------------------------------------------------------------------

    #[test]
    fn timestamp_ext_to_date() {
        let d = 1735689600_i64.to_date();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn timestamp_ext_year() {
        assert_eq!(1735689600_i64.year(), 2025);
        assert_eq!(1704067200_i64.year(), 2024);
        assert_eq!(0_i64.year(), 1970);
    }

    #[test]
    fn timestamp_ext_year_leap() {
        assert_eq!(1709251200_i64.year(), 2024);
    }

    // -----------------------------------------------------------------------
    // months_days
    // -----------------------------------------------------------------------

    #[test]
    fn months_days_all_months_non_leap() {
        let expected = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (i, exp) in expected.iter().enumerate() {
            let m = (i + 1) as u32;
            assert_eq!(months_days(m, false).unwrap(), *exp, "month {m}");
        }
    }

    #[test]
    fn months_days_february_leap_year() {
        assert_eq!(months_days(2, true).unwrap(), 29);
    }

    #[test]
    fn months_days_february_non_leap() {
        assert_eq!(months_days(2, false).unwrap(), 28);
    }

    #[test]
    fn months_days_invalid_zero() {
        assert!(months_days(0, false).is_err());
    }

    #[test]
    fn months_days_invalid_thirteen() {
        assert!(months_days(13, false).is_err());
    }

    // -----------------------------------------------------------------------
    // bitmap_days
    // -----------------------------------------------------------------------

    #[test]
    fn bitmap_single_day() {
        assert_eq!(bitmap_days(1, 1), 1);
        assert_eq!(bitmap_days(15, 15), 1 << 14);
        assert_eq!(bitmap_days(31, 31), 1 << 30);
    }

    #[test]
    fn bitmap_range() {
        // Days 1–5 → bits 0–4 → binary 0b11111 = 31
        assert_eq!(bitmap_days(1, 5), 0b11111);
    }

    #[test]
    fn bitmap_full_month() {
        // All 31 days → 0x7FFF_FFFF
        assert_eq!(bitmap_days(1, 31), 0x7FFF_FFFF);
    }

    #[test]
    fn bitmap_mid_month() {
        let mask = bitmap_days(10, 20);
        for day in 1..=31 {
            if (10..=20).contains(&day) {
                assert!(mask & (1 << (day - 1)) != 0, "day {day} should be set");
            } else {
                assert!(mask & (1 << (day - 1)) == 0, "day {day} should be clear");
            }
        }
    }

    #[test]
    fn bitmap_last_five_days() {
        let mask = bitmap_days(27, 31);
        assert_eq!(mask, 0b11111 << 26);
    }

    // -----------------------------------------------------------------------
    // bitmap_days + months_days interaction (regression fence)
    // -----------------------------------------------------------------------

    #[test]
    fn bitmap_with_month_days_produces_full_mask() {
        for m in 1..=12 {
            let days = months_days(m, false).unwrap();
            let mask = bitmap_days(1, days);
            assert_eq!(mask.count_ones(), days, "month {m}");
        }
    }

    #[test]
    fn bitmap_no_overlap_between_consecutive_ranges() {
        let a = bitmap_days(1, 10);
        let b = bitmap_days(11, 20);
        assert_eq!(a & b, 0);
    }
}
