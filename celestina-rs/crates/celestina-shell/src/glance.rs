//! What the bar's readings need to decide before they are drawn: how loaded a
//! meter is, and which day it is.
//!
//! Both are pure. The load thresholds are policy restated here (the system
//! reading itself is not this crate's); the date is arithmetic over a unix
//! time and a UTC offset the caller supplies, so no clock or time zone
//! database is read. Names of months and weekdays are product copy and live in
//! the product, not here.

use crate::calendar;

/// A meter at or above this percentage is elevated.
pub const ELEVATED_PERCENT: u8 = 80;
/// A meter at or above this percentage is critical.
pub const CRITICAL_PERCENT: u8 = 90;

/// How worried a meter's colour should be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Normal,
    Elevated,
    Critical,
}

impl Load {
    /// The level of a percentage; anything above 100 is as critical as 100.
    #[must_use]
    pub fn of(percent: u8) -> Self {
        if percent >= CRITICAL_PERCENT {
            Self::Critical
        } else if percent >= ELEVATED_PERCENT {
            Self::Elevated
        } else {
            Self::Normal
        }
    }

    /// A stable token for diagnostics and tests.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Elevated => "elevated",
            Self::Critical => "critical",
        }
    }
}

/// The first year a date is given for.
pub const MIN_YEAR: i32 = 1;
/// The last year a date is given for: four digits, as a calendar prints it.
pub const MAX_YEAR: i32 = 9999;

/// A calendar date and its weekday, as the short date and the month page need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Today {
    pub year: i32,
    /// 1-12.
    pub month: u8,
    /// 1-31.
    pub day: u8,
    /// 0 = Monday through 6 = Sunday, as [`calendar::weekday`] counts.
    pub weekday: u8,
}

impl Today {
    /// The local date for a unix time and the local UTC offset, both in
    /// seconds. Returns `None` for a time outside years [`MIN_YEAR`] to
    /// [`MAX_YEAR`], which no clock reports and the calendar does not draw.
    #[must_use]
    pub fn from_unix(unix_seconds: i64, utc_offset_seconds: i32) -> Option<Self> {
        let local = unix_seconds.checked_add(i64::from(utc_offset_seconds))?;
        let (year, month, day) = civil_from_days(local.div_euclid(86_400))?;
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return None;
        }
        Some(Self {
            year,
            month,
            day,
            weekday: calendar::weekday(year, month, day),
        })
    }
}

/// The civil (year, month, day) of a count of days since 1970-01-01.
///
/// Howard Hinnant's `civil_from_days`: eras of 400 years, so leap years and
/// century rules fall out of the arithmetic. `None` when the year overflows.
#[must_use]
pub fn civil_from_days(days: i64) -> Option<(i32, u8, u8)> {
    let shifted = days.checked_add(719_468)?;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = u8::try_from(day_of_year - (153 * shifted_month + 2) / 5 + 1).ok()?;
    let month = u8::try_from(if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    })
    .ok()?;
    let year = i32::try_from(year_of_era + era * 400 + i64::from(month <= 2)).ok()?;
    Some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_changes_at_eighty_and_ninety() {
        assert_eq!(Load::of(0), Load::Normal);
        assert_eq!(Load::of(79), Load::Normal);
        assert_eq!(Load::of(80), Load::Elevated);
        assert_eq!(Load::of(89), Load::Elevated);
        assert_eq!(Load::of(90), Load::Critical);
        assert_eq!(Load::of(255), Load::Critical);
        assert_eq!(Load::Normal.as_str(), "normal");
        assert_eq!(Load::Elevated.as_str(), "elevated");
        assert_eq!(Load::Critical.as_str(), "critical");
    }

    #[test]
    fn the_epoch_is_a_thursday() {
        assert_eq!(civil_from_days(0), Some((1970, 1, 1)));
        let today = Today::from_unix(0, 0).expect("a date");
        assert_eq!((today.year, today.month, today.day), (1970, 1, 1));
        assert_eq!(today.weekday, 3);
    }

    #[test]
    fn leap_days_are_found_and_century_rules_hold() {
        // 2024-02-29 00:00:00 UTC, a Thursday.
        let leap = Today::from_unix(1_709_164_800, 0).expect("a date");
        assert_eq!(
            (leap.year, leap.month, leap.day, leap.weekday),
            (2024, 2, 29, 3)
        );
        // 2000 was a leap year; 1900 and 2100 were not.
        assert_eq!(civil_from_days(11_016), Some((2000, 2, 29)));
        assert_eq!(civil_from_days(11_017), Some((2000, 3, 1)));
        assert_eq!(civil_from_days(-25_508), Some((1900, 3, 1)));
        assert_eq!(civil_from_days(47_541), Some((2100, 3, 1)));
    }

    #[test]
    fn a_year_boundary_follows_the_offset() {
        // 2025-12-31 23:30:00 UTC.
        let utc = 1_767_223_800;
        let before = Today::from_unix(utc, 0).expect("a date");
        assert_eq!((before.year, before.month, before.day), (2025, 12, 31));
        // One hour east it is already the new year.
        let after = Today::from_unix(utc, 3_600).expect("a date");
        assert_eq!((after.year, after.month, after.day), (2026, 1, 1));
        assert_eq!(after.weekday, 3);
    }

    #[test]
    fn a_negative_offset_goes_back_a_day() {
        // 2026-01-01 03:00:00 UTC is still 2025-12-31 in UTC-5.
        let utc = 1_767_236_400;
        let west = Today::from_unix(utc, -5 * 3_600).expect("a date");
        assert_eq!((west.year, west.month, west.day), (2025, 12, 31));
        assert_eq!(west.weekday, 2);
    }

    #[test]
    fn times_before_the_epoch_have_dates_too() {
        // One second before the epoch, and a leap-year start.
        let last = Today::from_unix(-1, 0).expect("a date");
        assert_eq!((last.year, last.month, last.day), (1969, 12, 31));
        assert_eq!(last.weekday, 2);
        assert_eq!(civil_from_days(-719_468), Some((0, 3, 1)));
    }

    #[test]
    fn years_outside_the_window_are_no_date() {
        // 0001-01-01 and 9999-12-31 are in; the days around them are not.
        let first = -62_135_596_800;
        let last = 253_402_300_799;
        assert_eq!(
            Today::from_unix(first, 0).map(|d| (d.year, d.month, d.day)),
            Some((1, 1, 1))
        );
        assert_eq!(
            Today::from_unix(last, 0).map(|d| (d.year, d.month, d.day)),
            Some((9999, 12, 31))
        );
        assert_eq!(Today::from_unix(first - 1, 0), None);
        assert_eq!(Today::from_unix(last + 1, 0), None);
        assert_eq!(Today::from_unix(i64::MIN, 0), None);
        assert_eq!(Today::from_unix(i64::MIN, -86_400), None);
    }

    #[test]
    fn an_absurd_time_is_no_date() {
        assert_eq!(Today::from_unix(i64::MAX, 3_600), None);
        assert_eq!(Today::from_unix(i64::MAX, 0), None);
    }
}
