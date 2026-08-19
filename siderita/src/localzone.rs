//! The machine's time zone, answered for the archive domain.
//!
//! A zip's dates carry no zone, so `siderita-archive` asks its caller which one
//! to write and read them in (`siderita_archive::Zone`). That answer is not a
//! domain rule: it lives in `TZ`, in the zone database and in the C library, so
//! it is the application that gives it — here, and once.
//!
//! The offset is asked per instant, and answered per instant: `localtime_r`
//! resolves summer time for the date being converted, so a file stamped in
//! January is not dated with July's offset.

use std::time::{SystemTime, UNIX_EPOCH};

/// The zone the desktop session is running in.
pub(crate) struct LocalZone;

impl siderita_archive::Zone for LocalZone {
    fn offset_at(&self, time: SystemTime) -> i32 {
        let seconds = match time.duration_since(UNIX_EPOCH) {
            Ok(elapsed) => elapsed.as_secs() as i64,
            // Before the epoch: still a valid `time_t`, and a zip cannot hold
            // such a date anyway, so the offset it gets does not matter much.
            Err(before) => -(before.duration().as_secs() as i64),
        };
        // SAFETY: localtime_r writes into a fully-owned, zeroed `tm`; the value
        // is a valid `time_t` and the call has no other effects. Same contract
        // as the properties panel's own conversion.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let stamp = seconds as libc::time_t;
        if unsafe { libc::localtime_r(&stamp, &mut tm) }.is_null() {
            // No zone information available: UTC, which is what the archive
            // domain falls back to on its own.
            return 0;
        }
        i32::try_from(tm.tm_gmtoff).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::LocalZone;
    use siderita_archive::Zone;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn the_offset_is_a_whole_number_of_minutes_within_a_days_range() {
        // Every real zone offset is between -12 h and +14 h and lands on a
        // minute; this holds whatever zone the test machine is in.
        for day in [0u64, 15_000, 20_600] {
            let instant = UNIX_EPOCH + Duration::from_secs(day * 86_400);
            let offset = LocalZone.offset_at(instant);
            assert!((-12 * 3600..=14 * 3600).contains(&offset), "{offset}");
            assert_eq!(offset % 60, 0, "{offset}");
        }
    }
}
