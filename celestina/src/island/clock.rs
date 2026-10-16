//! The capsule's clock: `HH:MM` formatted in process from the wall clock and
//! the local UTC offset, read once per process through `date +%z` (no
//! timezone dependency, and no `unsafe` for `localtime_r`), and a monotonic
//! minute tick. Ported from the spike's `src/island/mod.rs`.

use std::{
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The tick fires a little after the boundary so the frame it wakes never
/// still reads the old minute.
pub const TICK_MARGIN: Duration = Duration::from_millis(50);

/// Re-arm decision for the minute timer. The timer is monotonic, so the
/// deadline is too: any frame at or after it re-arms from the wall clock,
/// whatever minute that reads (an NTP step cannot freeze the clock).
#[derive(Clone, Copy, Debug, Default)]
pub struct ClockTick {
    deadline: Option<Instant>,
}

impl ClockTick {
    #[must_use]
    pub fn should_arm(&self, now: Instant) -> bool {
        self.deadline.is_none_or(|d| now >= d)
    }

    pub fn arm(&mut self, now: Instant, wait: Duration) {
        self.deadline = Some(now + wait);
    }
}

/// Seconds and nanoseconds since the Unix epoch (0 before it).
#[must_use]
pub fn unix_now() -> (u64, u32) {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (d.as_secs(), d.subsec_nanos())
}

/// Time left until the next minute boundary.
#[must_use]
pub fn until_next_minute(secs: u64, subsec_nanos: u32) -> Duration {
    Duration::from_secs(60 - secs % 60)
        .saturating_sub(Duration::from_nanos(u64::from(subsec_nanos)))
}

/// `HH:MM` of Unix time `secs` at `offset` seconds from UTC.
#[must_use]
pub fn format_local(secs: u64, offset: i64) -> String {
    let secs = i64::try_from(secs).unwrap_or(i64::MAX);
    let day = secs.saturating_add(offset).rem_euclid(86_400);
    format!("{:02}:{:02}", day / 3600, day / 60 % 60)
}

/// `date +%z` output (`±HHMM`) to seconds east of UTC.
#[must_use]
pub fn parse_utc_offset(s: &str) -> Option<i64> {
    let s = s.trim();
    let (sign, digits) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let minutes: i64 = digits[2..].parse().ok()?;
    (hours < 24 && minutes < 60).then_some(sign * (hours * 3600 + minutes * 60))
}

/// The local UTC offset, read once per process through `date +%z`. A DST
/// change while running is ignored until restart. Falls back to UTC, with
/// one warning, when `date` fails.
pub fn utc_offset() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| {
        let offset = Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| parse_utc_offset(&s));
        offset.unwrap_or_else(|| {
            tracing::warn!("`date +%z` unavailable, the clock shows UTC");
            0
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_boundary_is_computed_from_the_wall_clock() {
        assert_eq!(until_next_minute(120, 0), Duration::from_secs(60));
        assert_eq!(until_next_minute(121, 0), Duration::from_secs(59));
        assert_eq!(
            until_next_minute(179, 250_000_000),
            Duration::from_millis(750)
        );
    }

    #[test]
    fn clock_tick_rearms_at_or_after_its_deadline_whatever_the_wall_clock_says() {
        let t0 = Instant::now();
        let mut tick = ClockTick::default();
        assert!(tick.should_arm(t0));
        tick.arm(t0, Duration::from_secs(30));
        assert!(!tick.should_arm(t0 + Duration::from_secs(10)));
        assert!(tick.should_arm(t0 + Duration::from_secs(30)));
        assert!(tick.should_arm(t0 + Duration::from_secs(45)));
    }

    #[test]
    fn clock_formats_hours_and_minutes_at_an_offset() {
        // UTC (also the fallback).
        assert_eq!(format_local(0, 0), "00:00");
        assert_eq!(format_local(13 * 3600 + 5 * 60 + 59, 0), "13:05");
        assert_eq!(format_local(24 * 3600 + 61, 0), "00:01");
        // East and west of UTC, wrapping across midnight both ways.
        assert_eq!(format_local(13 * 3600 + 5 * 60, 2 * 3600), "15:05");
        assert_eq!(format_local(23 * 3600, 2 * 3600), "01:00");
        assert_eq!(format_local(3600, -4 * 3600), "21:00");
        assert_eq!(format_local(0, -(4 * 3600 + 30 * 60)), "19:30");
        assert_eq!(format_local(0, 5 * 3600 + 45 * 60), "05:45");
        // Never panics at the far end of the clock.
        assert_eq!(format_local(u64::MAX, 3600).len(), 5);
    }

    #[test]
    fn utc_offset_parses_date_z_output() {
        assert_eq!(parse_utc_offset("+0000\n"), Some(0));
        assert_eq!(parse_utc_offset("+0200"), Some(7200));
        assert_eq!(parse_utc_offset("-0400"), Some(-14_400));
        assert_eq!(parse_utc_offset("-0430"), Some(-16_200));
        assert_eq!(parse_utc_offset("+0545"), Some(20_700));
        for bad in [
            "", "0200", "+02", "+02:00", "+2a00", "+0260", "+2400", "UTC",
        ] {
            assert_eq!(parse_utc_offset(bad), None, "{bad:?}");
        }
    }
}
