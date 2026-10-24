//! The capsule's clock: `HH:MM` formatted in process from the wall clock and
//! the local UTC offset, read once per process through `date +%z` (no
//! timezone dependency, and no `unsafe` for `localtime_r`), and a monotonic
//! minute tick. Ported from the spike's `src/island/mod.rs`. The session lock
//! reads the offset afresh per spawn through [`read_utc_offset`] instead
//! (CEL-2-H-b).

use rustix::{
    event::{poll, PollFd, PollFlags, Timespec},
    io::Errno,
    process::{kill_process_group, Pid, Signal},
};
use std::{
    ffi::OsStr,
    io::Read,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The tick fires a little after the boundary so the frame it wakes never
/// still reads the old minute.
pub const TICK_MARGIN: Duration = Duration::from_millis(50);

/// The minute timer's deadline. The timer is monotonic, so the deadline is
/// too: any frame at or after it re-arms from the wall clock, whatever
/// minute that reads (an NTP step cannot freeze the clock).
///
/// A surface has one redraw timer and an earlier deadline replaces the armed
/// one (the page's [`SecondTick`] does), so every frame asks for the minute
/// deadline again: once the earlier timer stops, the minute one is still
/// there. Asking again for an armed deadline costs nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClockTick {
    deadline: Option<Instant>,
}

impl ClockTick {
    /// The wait until the minute deadline, at `now` and wall-clock time
    /// `secs`.`subsec_nanos`: the deadline in force, or the next boundary
    /// plus the margin once it has passed.
    pub fn wait(&mut self, now: Instant, secs: u64, subsec_nanos: u32) -> Duration {
        match self.deadline {
            Some(d) if now < d => d - now,
            _ => {
                let wait = until_next_minute(secs, subsec_nanos) + TICK_MARGIN;
                self.deadline = Some(now + wait);
                wait
            }
        }
    }
}

/// Re-arm decision for the page's one-second timer. It exists only while a
/// page is open: a closed Island forgets it, so rest keeps drawing one frame
/// a minute. Monotonic like [`ClockTick`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SecondTick {
    deadline: Option<Instant>,
}

impl SecondTick {
    /// The wait to arm for the next second boundary, or `None` when no
    /// timer is due: a closed page arms nothing and drops the deadline, an
    /// open one re-arms at or after its deadline.
    pub fn wait(&mut self, open: bool, now: Instant, subsec_nanos: u32) -> Option<Duration> {
        if !open {
            self.deadline = None;
            return None;
        }
        if self.deadline.is_some_and(|d| now < d) {
            return None;
        }
        let wait = until_next_second(subsec_nanos) + TICK_MARGIN;
        self.deadline = Some(now + wait);
        Some(wait)
    }

    /// Whether a timer is armed (tests).
    #[cfg(test)]
    fn is_armed(&self) -> bool {
        self.deadline.is_some()
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

/// Time left until the next second boundary.
#[must_use]
pub fn until_next_second(subsec_nanos: u32) -> Duration {
    Duration::from_secs(1).saturating_sub(Duration::from_nanos(u64::from(subsec_nanos)))
}

/// `HH:MM:SS` of Unix time `secs` at `offset` seconds from UTC.
#[must_use]
pub fn format_local_seconds(secs: u64, offset: i64) -> String {
    let secs = i64::try_from(secs).unwrap_or(i64::MAX);
    let day = secs.saturating_add(offset).rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", day / 3600, day / 60 % 60, day % 60)
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

/// How long `date +%z` may take: it prints four characters, so a run past
/// this is hung and the offset is unknown.
const DATE_DEADLINE: Duration = Duration::from_millis(500);

/// How often a running `date` is looked at while it is waited for.
const DATE_POLL: Duration = Duration::from_millis(5);

/// The most a `date +%z` run may print; more reads as unknown.
const DATE_MAX_OUTPUT: u64 = 64;

/// Where `date` lives: an absolute path, never a search through `PATH`
/// (design §4.2).
const DATE_PROGRAM: &str = "/usr/bin/date";

/// What `program args` printed when it succeeded within `limit`, with `env`
/// added to its environment. The whole run, the read of its output included,
/// is inside `limit`: the output is read only when `poll` says it is
/// readable, with the time that is left, so a descendant that inherits the
/// pipe and outlives `program` cannot hold the caller. The child leads its
/// own process group; a run that overstays has the group killed through the
/// child's pid (never by name) and the child reaped, and the answer is
/// `None`.
fn bounded_output(
    program: &OsStr,
    args: &[&str],
    env: &[(&str, &str)],
    limit: Duration,
) -> Option<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let end = Instant::now() + limit;
    let out = read_until(&mut child, end);
    let status = out.as_ref().and_then(|_| wait_until(&mut child, end));
    if status.is_none() {
        // Not reaped yet, so its pid (the group id) cannot have been reused.
        if let Some(pid) = i32::try_from(child.id()).ok().and_then(Pid::from_raw) {
            let _ = kill_process_group(pid, Signal::KILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    if !status?.success() {
        return None;
    }
    out
}

/// The output of `child` up to end of file (at most [`DATE_MAX_OUTPUT`]
/// bytes), or `None` when `end` comes first.
fn read_until(child: &mut Child, end: Instant) -> Option<Vec<u8>> {
    let stdout = child.stdout.as_mut()?;
    let mut out = Vec::new();
    let mut chunk = [0_u8; 64];
    loop {
        let left = end.checked_duration_since(Instant::now())?;
        let timeout = Timespec::try_from(left).ok()?;
        let mut fds = [PollFd::new(&*stdout, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(_) => return None,
        }
        // Readable (or hung up): this read returns at once.
        match stdout.read(&mut chunk) {
            Ok(0) => return Some(out),
            Ok(n) => {
                out.extend_from_slice(&chunk[..n]);
                if out.len() as u64 > DATE_MAX_OUTPUT {
                    return None;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

/// The exit status of `child` once it exits before `end`.
fn wait_until(child: &mut Child, end: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < end => std::thread::sleep(DATE_POLL),
            Ok(None) | Err(_) => return None,
        }
    }
}

/// What `date +%z` printed, when it ran and succeeded in time.
fn date_z_output() -> Option<Vec<u8>> {
    bounded_output(OsStr::new(DATE_PROGRAM), &["+%z"], &[], DATE_DEADLINE)
}

/// The offset in the bytes a `date +%z` run printed.
fn offset_of(output: Option<Vec<u8>>) -> Option<i64> {
    String::from_utf8(output?)
        .ok()
        .and_then(|s| parse_utc_offset(&s))
}

/// The local UTC offset, read afresh through `date +%z` on every call: no
/// cell, so a daylight-saving change is seen by the next call. `None` when
/// `date` fails or prints something else; the caller decides what that means
/// (the session lock hides its clock rather than guess, design §4.8, M1). It
/// waits for a child process for at most [`DATE_DEADLINE`] (a hung `date`
/// reads as unknown), so it never runs on a loop thread.
#[must_use]
pub fn read_utc_offset() -> Option<i64> {
    offset_of(date_z_output())
}

/// The local UTC offset, read once per process through `date +%z`. A DST
/// change while running is ignored until restart (a separate bug unit, not
/// this one). Falls back to UTC, with one warning, when `date` fails.
pub fn utc_offset() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| {
        read_utc_offset().unwrap_or_else(|| {
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
        let secs = Duration::from_secs;
        let mut tick = ClockTick::default();
        // Wall clock 30 s into a minute: the boundary plus the margin.
        assert_eq!(tick.wait(t0, 90, 0), secs(30) + TICK_MARGIN);
        // Before the deadline: what is left of it, whatever the wall clock reads.
        assert_eq!(tick.wait(t0 + secs(10), 0, 0), secs(20) + TICK_MARGIN);
        // At and after it: re-armed from the wall clock.
        assert_eq!(
            tick.wait(t0 + secs(30) + TICK_MARGIN, 120, 0),
            secs(60) + TICK_MARGIN
        );
        assert_eq!(
            tick.wait(t0 + secs(200), 150, 0),
            secs(30) + TICK_MARGIN,
            "an NTP step back re-arms from the clock"
        );
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

    #[test]
    fn seconds_format_across_midnight_and_offsets() {
        assert_eq!(format_local_seconds(0, 0), "00:00:00");
        assert_eq!(format_local_seconds(13 * 3600 + 5 * 60 + 59, 0), "13:05:59");
        assert_eq!(format_local_seconds(86_399, 0), "23:59:59");
        assert_eq!(format_local_seconds(86_400, 0), "00:00:00");
        assert_eq!(
            format_local_seconds(23 * 3600 + 59 * 60 + 59, 2 * 3600),
            "01:59:59"
        );
        assert_eq!(format_local_seconds(0, -1), "23:59:59");
        assert_eq!(format_local_seconds(0, 5 * 3600 + 45 * 60 + 7), "05:45:07");
        assert_eq!(format_local_seconds(u64::MAX, 3600).len(), 8);
    }

    #[test]
    fn the_second_boundary_is_computed_from_the_subsecond() {
        assert_eq!(until_next_second(0), Duration::from_secs(1));
        assert_eq!(until_next_second(250_000_000), Duration::from_millis(750));
        assert_eq!(until_next_second(999_999_999), Duration::from_nanos(1));
        assert_eq!(until_next_second(u32::MAX), Duration::ZERO);
    }

    #[test]
    fn the_second_timer_exists_only_while_a_page_is_open() {
        let t0 = Instant::now();
        let mut tick = SecondTick::default();
        // Closed: nothing is armed, however often it is asked.
        assert_eq!(tick.wait(false, t0, 0), None);
        assert!(!tick.is_armed());
        // Open: armed once for the next boundary plus the margin.
        assert_eq!(
            tick.wait(true, t0, 250_000_000),
            Some(Duration::from_millis(750) + TICK_MARGIN)
        );
        // Not due again before the deadline, due at it.
        assert_eq!(tick.wait(true, t0 + Duration::from_millis(500), 0), None);
        assert!(tick
            .wait(true, t0 + Duration::from_millis(800), 0)
            .is_some());
        // Closing drops the deadline; reopening arms afresh.
        assert_eq!(tick.wait(false, t0 + Duration::from_secs(1), 0), None);
        assert!(!tick.is_armed());
        assert!(tick.wait(true, t0 + Duration::from_secs(1), 0).is_some());
    }

    /// `date +%z` run with `TZ` in its own environment only: a POSIX string
    /// such as `XXX-5` (five hours east) needs no time-zone database, and the
    /// process environment, shared by every parallel test, is never changed.
    fn offset_in(tz: &str) -> Option<i64> {
        offset_of(bounded_output(
            OsStr::new(DATE_PROGRAM),
            &["+%z"],
            &[("TZ", tz)],
            Duration::from_secs(10),
        ))
    }

    #[test]
    fn read_utc_offset_is_uncached_and_fails_to_none() {
        // Every run asks `date` again: three zones, three answers.
        assert_eq!(offset_in("XXX-5"), Some(5 * 3600));
        assert_eq!(offset_in("YYY3"), Some(-3 * 3600));
        assert_eq!(offset_in("ZZZ-5:30"), Some(5 * 3600 + 1800));
        // A failed run or output that is not an offset is `None`, not UTC.
        assert_eq!(offset_of(None), None);
        assert_eq!(offset_of(Some(b"".to_vec())), None);
        assert_eq!(offset_of(Some(b"UTC\n".to_vec())), None);
        assert_eq!(offset_of(Some(vec![0xff, 0xfe])), None);
        assert_eq!(offset_of(Some(b"+0100\n".to_vec())), Some(3600));
    }

    #[test]
    fn a_date_that_hangs_is_killed_at_the_deadline_and_reads_as_unknown() {
        // `sleep 30` stands for a `date` that never answers.
        let started = Instant::now();
        let out = bounded_output(
            OsStr::new("sleep"),
            &["30"],
            &[],
            Duration::from_millis(150),
        );
        let took = started.elapsed();
        assert_eq!(out, None);
        assert!(took >= Duration::from_millis(150), "{took:?}");
        assert!(took < Duration::from_secs(5), "{took:?}");
    }

    /// Whether `pid` is gone (or only a zombie waiting for its reaper).
    fn is_dead(pid: &str) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(_) => true,
            Ok(stat) => stat.rsplit(") ").next().is_some_and(|r| r.starts_with('Z')),
        }
    }

    #[test]
    fn a_descendant_holding_the_pipe_cannot_hold_the_caller() {
        let dir = std::env::temp_dir().join(format!("clock-b1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let pidfile = dir.join("sleeper");
        // `sh` prints the offset and exits at once; its `sleep` keeps stdout.
        let script = format!("sleep 3 & echo $! > {}; echo +0100", pidfile.display());
        let started = Instant::now();
        let out = bounded_output(
            OsStr::new("sh"),
            &["-c", &script],
            &[],
            Duration::from_millis(150),
        );
        let took = started.elapsed();
        assert_eq!(out, None);
        assert!(took < Duration::from_millis(500), "{took:?}");
        let pid = std::fs::read_to_string(&pidfile).expect("pid file");
        let pid = pid.trim();
        let gone = (0..100).any(|_| {
            std::thread::sleep(Duration::from_millis(20));
            is_dead(pid)
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert!(gone, "the sleep of the group is still alive");
    }

    #[test]
    fn an_exec_that_never_answers_is_killed_with_its_group() {
        let started = Instant::now();
        let out = bounded_output(
            OsStr::new("sh"),
            &["-c", "exec sleep 30"],
            &[],
            Duration::from_millis(150),
        );
        assert_eq!(out, None);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn a_date_that_fails_or_cannot_start_reads_as_unknown() {
        let limit = Duration::from_secs(10);
        assert_eq!(bounded_output(OsStr::new("false"), &[], &[], limit), None);
        assert_eq!(
            bounded_output(OsStr::new("/nonexistent/date"), &[], &[], limit),
            None
        );
        assert_eq!(
            bounded_output(OsStr::new("echo"), &["+0200"], &[], limit),
            Some(b"+0200\n".to_vec())
        );
    }

    #[test]
    fn the_island_keeps_its_cached_offset() {
        // The Island's call is filled once per process: every call answers
        // what the first did (its staleness across a zone change is noted for
        // a separate bug unit, not changed here).
        let first = utc_offset();
        assert_eq!(utc_offset(), first);
    }
}
