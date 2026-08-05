//! Which day falls where in a month.
//!
//! The calendar is the one thing in this shell that needs no provider, no
//! service and no permission: a month's shape follows from arithmetic that has
//! not changed since 1582. So it is computed rather than fetched, and the only
//! thing the surface has to supply is which day it is — which it already knows.
//!
//! Weeks start on Monday, as they do where this session is used. That is a
//! product decision, stated once here rather than re-derived by whoever draws
//! the grid.

/// The days in each month of a common year, January first.
const COMMON_LENGTHS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Whether `year` has a 29th of February, by the Gregorian rule.
#[must_use]
pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// How many days `month` has. `month` is 1-12; anything else has none, which is
/// how a caller's bad input stops here instead of becoming a strange grid.
#[must_use]
pub fn days_in_month(year: i32, month: u8) -> u8 {
    if !(1..=12).contains(&month) {
        return 0;
    }
    if month == 2 && is_leap(year) {
        return 29;
    }
    COMMON_LENGTHS[usize::from(month - 1)]
}

/// Which weekday the given date falls on, as 0 = Monday through 6 = Sunday.
///
/// Zeller's congruence, with January and February counted as months 13 and 14
/// of the previous year — which is what makes the leap day fall at the end of
/// the year being counted, where the rule needs it.
#[must_use]
pub fn weekday(year: i32, month: u8, day: u8) -> u8 {
    let (year, month) = if month <= 2 {
        (year - 1, i32::from(month) + 12)
    } else {
        (year, i32::from(month))
    };
    let century = year.div_euclid(100);
    let within = year.rem_euclid(100);
    let zeller =
        (i32::from(day) + (13 * (month + 1)) / 5 + within + within / 4 + century / 4 + 5 * century)
            .rem_euclid(7);
    // Zeller counts 0 = Saturday. This shell counts weeks from Monday.
    u8::try_from((zeller + 5).rem_euclid(7)).unwrap_or(0)
}

/// One month laid out as the surface draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Month {
    pub year: i32,
    pub month: u8,
    /// How many empty cells come before the 1st, so the first row lines up
    /// under the right weekday.
    pub leading_blanks: u8,
    pub days: u8,
}

impl Month {
    /// Builds the layout for a month, or nothing when that is not a month.
    #[must_use]
    pub fn new(year: i32, month: u8) -> Option<Self> {
        let days = days_in_month(year, month);
        if days == 0 {
            return None;
        }
        Some(Self {
            year,
            month,
            leading_blanks: weekday(year, month, 1),
            days,
        })
    }

    /// How many week rows the grid needs. Five for most months; six when a long
    /// month starts late enough to spill, four only for a non-leap February
    /// that starts on a Monday.
    #[must_use]
    pub fn rows(&self) -> u8 {
        let cells = u16::from(self.leading_blanks) + u16::from(self.days);
        u8::try_from(cells.div_ceil(7)).unwrap_or(6)
    }

    /// The month before this one, crossing the year when it must.
    #[must_use]
    pub fn previous(&self) -> Self {
        let (year, month) = if self.month == 1 {
            (self.year - 1, 12)
        } else {
            (self.year, self.month - 1)
        };
        Self::new(year, month).unwrap_or_else(|| self.clone())
    }

    /// The month after this one.
    #[must_use]
    pub fn next(&self) -> Self {
        let (year, month) = if self.month == 12 {
            (self.year + 1, 1)
        } else {
            (self.year, self.month + 1)
        };
        Self::new(year, month).unwrap_or_else(|| self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gregorian_leap_rule_is_the_whole_rule() {
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        // The two exceptions people forget.
        assert!(!is_leap(1900));
        assert!(is_leap(2000));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
    }

    #[test]
    fn a_month_that_is_not_a_month_has_no_days() {
        assert_eq!(days_in_month(2026, 0), 0);
        assert_eq!(days_in_month(2026, 13), 0);
        assert_eq!(Month::new(2026, 13), None);
    }

    #[test]
    fn weekdays_are_counted_from_monday() {
        // Known dates, checked against the calendar rather than against this
        // implementation: 2026-08-04 is a Tuesday, 2000-01-01 a Saturday.
        assert_eq!(weekday(2026, 8, 4), 1);
        assert_eq!(weekday(2000, 1, 1), 5);
        assert_eq!(weekday(2024, 2, 29), 3);
        assert_eq!(weekday(1970, 1, 1), 3);
    }

    #[test]
    fn a_month_starts_where_its_first_day_falls() {
        let august = Month::new(2026, 8).expect("a month");
        assert_eq!(august.days, 31);
        // The 1st of August 2026 is a Saturday: five blanks before it.
        assert_eq!(august.leading_blanks, 5);
        assert_eq!(august.rows(), 6);
    }

    #[test]
    fn a_february_that_starts_on_a_monday_needs_four_rows() {
        let february = Month::new(2027, 2).expect("a month");
        assert_eq!(february.leading_blanks, 0);
        assert_eq!(february.days, 28);
        assert_eq!(february.rows(), 4);
    }

    #[test]
    fn stepping_a_month_crosses_the_year() {
        let january = Month::new(2026, 1).expect("a month");
        assert_eq!(january.previous(), Month::new(2025, 12).expect("december"));

        let december = Month::new(2026, 12).expect("a month");
        assert_eq!(december.next(), Month::new(2027, 1).expect("january"));
    }
}
