//! What the properties panel says about one item.
//!
//! Pure and testable: it turns a catalogue record into the handful of strings
//! the panel shows. Nothing here opens the file — every value is either already
//! on the record or comes from a single `stat`, because looking at an item's
//! properties must not cost what decoding it costs.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fluorita_core::{MediaRecord, MediaSource, SourceSet};

use super::copy;

/// One item, as the panel shows it. Every field is already a display string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ItemDetail {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) size: String,
    pub(super) modified: String,
    /// Empty unless the catalogue learned one; this never starts a probe to
    /// find out.
    pub(super) duration: String,
    /// The folder the item was catalogued under, so the panel says where it
    /// came from rather than leaving the user to read it out of the path.
    pub(super) folder: String,
    /// Empty while the file is where the catalogue last saw it.
    pub(super) notice: String,
}

pub(super) fn describe(record: &MediaRecord, configured: &SourceSet) -> ItemDetail {
    let identity = record.identity();
    ItemDetail {
        name: record.display_name(),
        // Lossy for display only, like every other label: the menu acts on the
        // byte-exact path the row carries, never on this.
        path: record.path().to_string_lossy().into_owned(),
        kind: copy::kind_noun(record.kind()).to_owned(),
        size: bytes(identity.size),
        modified: timestamp(identity.modified),
        duration: record.metadata().duration.map(clock).unwrap_or_default(),
        folder: configured
            .get(record.source())
            .map(MediaSource::display_name)
            .unwrap_or_default(),
        notice: if record.is_available() {
            String::new()
        } else {
            copy::FILE_MISSING.to_owned()
        },
    }
}

/// Binary units, because that is what a file manager and a disk report. One
/// decimal above a kibibyte is enough to tell two files apart without pretending
/// to a precision the number does not have.
fn bytes(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if size < 1024 {
        return format!("{size} B");
    }
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// `HH:MM:SS`, or `MM:SS` under an hour. Truncated rather than rounded, so a
/// track never reads as one second longer than it plays.
fn clock(duration: Duration) -> String {
    let total = duration.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// A calendar date in ISO order, in UTC.
///
/// Deliberately not localised and deliberately not "3 days ago": this crate has
/// no timezone database and no clock it can trust for relative wording, and a
/// date that is wrong by a day is worse than one that is unambiguous.
fn timestamp(moment: SystemTime) -> String {
    let Ok(elapsed) = moment.duration_since(UNIX_EPOCH) else {
        return String::new();
    };
    let seconds = elapsed.as_secs();
    let days = seconds / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let (hour, minute) = ((seconds % 86_400) / 3600, (seconds % 3600) / 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

/// Howard Hinnant's civil-from-days, the standard branch-free conversion.
/// Written out rather than pulled in as a dependency: one calendar function
/// does not justify a date crate in a media player.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::{bytes, clock, timestamp};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn sizes_read_in_the_units_a_disk_reports() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1023), "1023 B");
        assert_eq!(bytes(1024), "1.0 KiB");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(3 * 1024 * 1024), "3.0 MiB");
        // Past the largest unit the number keeps growing rather than wrapping
        // into a unit that does not exist.
        assert_eq!(bytes(u64::MAX), "16777216.0 TiB");
    }

    #[test]
    fn a_duration_never_reads_longer_than_it_plays() {
        assert_eq!(clock(Duration::from_secs(0)), "0:00");
        assert_eq!(clock(Duration::from_secs(61)), "1:01");
        // 3599.9 seconds is still under the hour and must not become 1:00:00.
        assert_eq!(clock(Duration::from_millis(3_599_900)), "59:59");
        assert_eq!(clock(Duration::from_secs(3600)), "1:00:00");
        assert_eq!(clock(Duration::from_secs(3661)), "1:01:01");
    }

    #[test]
    fn a_date_is_unambiguous_rather_than_localised() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01 00:00 UTC");
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::from_secs(1_770_000_000)),
            "2026-02-02 02:40 UTC"
        );
        // A leap day, the case an off-by-one calendar gets wrong.
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29 00:00 UTC"
        );
    }

    #[test]
    fn a_moment_before_the_epoch_says_nothing_rather_than_guessing() {
        let before = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(timestamp(before), "");
    }
}
