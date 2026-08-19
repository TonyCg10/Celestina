//! Modification times across the archive boundary.
//!
//! A file manager that compresses a folder and extracts it again must give back
//! the dates it was handed; a tree that comes out all stamped "today" has lost
//! something real, and the folder's own date column is where that loss shows.
//!
//! `tar` carries a Unix timestamp and needs nothing from here. A zip carries an
//! MS-DOS date **with no zone at all**, and every other tool reads that field as
//! local time — so writing UTC into it makes an archive show up hours off in
//! another manager, in `unzip -l`, and in this one after a round trip through
//! either.
//!
//! A pure domain cannot know the machine's zone: that answer lives in `TZ`, in
//! the zone database and in the C library. So it is asked for, through [`Zone`],
//! and asked *per instant* rather than once — the offset in force in July is not
//! the one in force in January, and a single fixed offset misdates half the
//! year. The calendar arithmetic stays here; only the offset comes from outside.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The time zone an archive's zone-less dates are written in and read as.
///
/// One question, asked per instant: how far east of UTC was local time at that
/// moment. The application answers it — on unix from `localtime_r`'s
/// `tm_gmtoff` — and the domain only does the arithmetic.
pub trait Zone {
    /// Seconds east of UTC in force at `time` (negative west of it).
    fn offset_at(&self, time: SystemTime) -> i32;
}

/// The zone that always answers zero: for a caller that has no zone information,
/// and for tests that need a fixed, machine-independent answer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Utc;

impl Zone for Utc {
    fn offset_at(&self, _time: SystemTime) -> i32 {
        0
    }
}

/// The calendar parts of `time` **as `zone` spells them** — what goes into a
/// zip's date field, since that is what every reader will take it for.
pub(crate) fn local_parts(zone: &dyn Zone, time: SystemTime) -> Option<(u16, u8, u8, u8, u8, u8)> {
    utc_parts(shift(time, zone.offset_at(time))?)
}

/// The instant `zone` spells with these calendar parts: the inverse, for reading
/// a zip's date back.
///
/// Two passes, because the offset depends on the very instant being computed:
/// the parts are first read as if they were UTC, that rough instant names an
/// offset, and the offset is then re-asked at the corrected instant. A clock
/// reading inside a daylight-saving jump is ambiguous by nature; this settles it
/// at the offset in force after the correction, the same choice `mktime` makes.
pub(crate) fn instant_from_local(
    zone: &dyn Zone,
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
) -> Option<SystemTime> {
    let naive = utc_instant(year, month, day, hour, minute, second)?;
    let rough = shift(naive, -zone.offset_at(naive))?;
    shift(naive, -zone.offset_at(rough))
}

/// `time` moved by `offset` seconds, or `None` when that leaves the range a
/// `SystemTime` can hold.
fn shift(time: SystemTime, offset: i32) -> Option<SystemTime> {
    if offset >= 0 {
        time.checked_add(Duration::from_secs(offset as u64))
    } else {
        time.checked_sub(Duration::from_secs(offset.unsigned_abs() as u64))
    }
}

/// The UTC calendar parts (year, month, day, hour, minute, second) of `time`,
/// or `None` outside the range MS-DOS can hold.
fn utc_parts(time: SystemTime) -> Option<(u16, u8, u8, u8, u8, u8)> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let year = u16::try_from(year).ok()?;
    Some((
        year,
        month,
        day,
        (rest / 3600) as u8,
        ((rest % 3600) / 60) as u8,
        (rest % 60) as u8,
    ))
}

/// The instant those same UTC parts name.
fn utc_instant(
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
) -> Option<SystemTime> {
    let days = days_from_civil(i64::from(year), month, day);
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second))?;
    from_epoch_seconds(seconds)
}

/// An instant as a Unix timestamp, for the zip's extended-timestamp field, or
/// `None` outside the 32 bits that field holds.
pub(crate) fn epoch_seconds(time: SystemTime) -> Option<u32> {
    u32::try_from(time.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()
}

/// A Unix timestamp as an instant, for the tar header's own `mtime`.
pub(crate) fn from_epoch_seconds(seconds: i64) -> Option<SystemTime> {
    if seconds >= 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
    }
}

/// Days since 1970-01-01 as a proleptic-Gregorian date, and its inverse below.
/// Howard Hinnant's `civil_from_days` / `days_from_civil`: exact integer
/// arithmetic with no table, no leap-year special case at the call site and no
/// dependency — which is why this domain can date a zip while staying pure.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u8;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u8;
    (year + i64::from(month <= 2), month, day)
}

fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::{instant_from_local, local_parts, utc_instant, utc_parts, Utc, Zone};
    use std::time::SystemTime;

    /// A zone with summer time: +2 h from April to October, +1 h otherwise —
    /// enough to prove the offset is asked per instant and not once.
    struct Madrid;

    impl Zone for Madrid {
        fn offset_at(&self, time: SystemTime) -> i32 {
            let (_, month, ..) = utc_parts(time).expect("in range");
            if (4..=10).contains(&month) {
                2 * 3600
            } else {
                3600
            }
        }
    }

    #[test]
    fn a_date_survives_the_round_trip_through_the_zip_calendar() {
        // A leap day, an end of century and an ordinary afternoon.
        for parts in [
            (2024u16, 2u8, 29u8, 23u8, 59u8, 58u8),
            (2000, 3, 1, 0, 0, 0),
            (2026, 8, 18, 14, 31, 6),
            (1980, 1, 1, 0, 0, 0),
        ] {
            let instant =
                utc_instant(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5).expect("instant");
            assert_eq!(utc_parts(instant), Some(parts));
        }
    }

    #[test]
    fn a_date_is_written_and_read_back_in_the_same_zone() {
        for parts in [
            (2026u16, 1u8, 15u8, 8u8, 30u8, 0u8),
            (2026, 7, 15, 8, 30, 0),
        ] {
            let instant =
                utc_instant(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5).expect("instant");
            let written = local_parts(&Madrid, instant).expect("write");
            let read = instant_from_local(
                &Madrid, written.0, written.1, written.2, written.3, written.4, written.5,
            )
            .expect("read");
            assert_eq!(read, instant, "{parts:?} did not come back the same");
        }
    }

    #[test]
    fn the_offset_asked_is_the_one_in_force_at_that_instant() {
        // Noon UTC in January is 13:00 local; the same instant in July is 14:00.
        // One fixed offset could not produce both.
        let winter = utc_instant(2026, 1, 15, 12, 0, 0).expect("winter");
        let summer = utc_instant(2026, 7, 15, 12, 0, 0).expect("summer");
        assert_eq!(local_parts(&Madrid, winter).expect("winter").3, 13);
        assert_eq!(local_parts(&Madrid, summer).expect("summer").3, 14);
        // A caller with no zone information still gets UTC, unshifted.
        assert_eq!(local_parts(&Utc, summer).expect("utc").3, 12);
    }
}
