//! The lock's clock and date (design §4.8, H-10, M1).
//!
//! `CLOCK_MONOTONIC` does not advance during suspend, so a monotonic minute
//! timer would show the old time for up to a minute after resume. The lock
//! arms a `timerfd` on `CLOCK_REALTIME` instead, absolute at the next minute
//! boundary, with `TFD_TIMER_CANCEL_ON_SET`:
//!
//! * a machine that resumes has a realtime clock that jumped past the
//!   deadline, so the timer is expired and fires at once;
//! * a clock step (NTP, `date -s`) cancels the timer, and the read reports
//!   `ECANCELED`: the clock re-arms for the next boundary of the new time.
//!
//! Either way the loop is told to draw again; the frame itself formats the
//! time from [`SystemTime`] and the bundle's `utc_offset_s`. This module
//! spawns no `date` and reads no time-zone file: the offset the shell read for
//! this spawn is the only zone knowledge the lock has, and a daylight-saving
//! change during one lock still shows the offset of its spawn (accepted).
//! There is no timer at all between minutes: a lock at rest draws one frame a
//! minute, plus one a second only while the machine counts down a delay.
#![forbid(unsafe_code)]

use std::io;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};
use rustix::time::{
    timerfd_create, timerfd_settime, Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags,
    Timespec,
};

use crate::copy;

/// A wall-clock instant broken into the fields the lock shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
    /// 0 is Sunday.
    pub weekday: u8,
    pub day: u8,
    /// 1 is January.
    pub month: u8,
    pub year: i64,
}

impl LocalTime {
    /// `HH:MM`.
    #[must_use]
    pub fn clock(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// The date line, in the product copy (`viernes 23 de octubre`).
    #[must_use]
    pub fn date(&self) -> String {
        copy::lock_date(self.weekday, self.day, self.month)
    }
}

/// Seconds since the Unix epoch, negative before it.
fn unix_seconds(at: SystemTime) -> i64 {
    match at.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
    }
}

/// The civil date of a day count since 1970-01-01 (proleptic Gregorian;
/// Howard Hinnant's `civil_from_days`): `(year, month 1..=12, day 1..=31)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era.saturating_add(era.saturating_mul(400)) + i64::from(month <= 2);
    (year, month, day)
}

/// The local time at `at` for a zone `utc_offset_s` seconds east of UTC. No
/// time-zone file, no process: arithmetic on the offset the bundle carried.
#[must_use]
pub fn local_time(at: SystemTime, utc_offset_s: i32) -> LocalTime {
    let local = unix_seconds(at).saturating_add(i64::from(utc_offset_s));
    let days = local.div_euclid(86_400);
    let seconds = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let small = |value: i64| u8::try_from(value).unwrap_or(0);
    LocalTime {
        hour: small(seconds / 3_600),
        minute: small(seconds / 60 % 60),
        // 1970-01-01 was a Thursday, and Sunday is 0.
        weekday: small((days + 4).rem_euclid(7)),
        day: small(day),
        month: small(month),
        year,
    }
}

/// The first whole minute strictly after `now`.
#[must_use]
pub fn next_minute_boundary(now: SystemTime) -> SystemTime {
    let seconds = unix_seconds(now);
    let next = (seconds.div_euclid(60) + 1) * 60;
    match u64::try_from(next) {
        Ok(after) => UNIX_EPOCH + Duration::from_secs(after),
        // A clock before 1970: fire as soon as possible.
        Err(_) => UNIX_EPOCH,
    }
}

/// The two things the clock needs of its descriptor, so the logic around
/// them is tested without stepping the machine's real clock.
pub trait Alarm {
    /// Arms the alarm for the absolute realtime instant `at`, replacing any
    /// earlier setting. An instant that already passed fires at once.
    ///
    /// # Errors
    ///
    /// When the kernel refuses the setting.
    fn set(&self, at: SystemTime) -> io::Result<()>;

    /// Consumes a firing. `ECANCELED` means the realtime clock was set.
    ///
    /// # Errors
    ///
    /// `ECANCELED` after a clock step, `WouldBlock` when nothing fired.
    fn read(&self) -> io::Result<()>;
}

/// What the loop has to do after the alarm's descriptor became readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// A minute boundary passed, or the machine resumed: draw again.
    Minute,
    /// The realtime clock was set: draw again, the time may be anything.
    Stepped,
    /// Nothing fired: do nothing.
    Idle,
}

impl Wake {
    /// Whether the screen has to be drawn again.
    #[must_use]
    pub fn redraw(self) -> bool {
        self != Self::Idle
    }
}

/// The alarm and the rule for re-arming it: always for the next boundary of
/// the *current* time, so a resume or a step never leaves it behind.
#[derive(Debug)]
pub struct ClockWatch<A> {
    alarm: A,
}

impl<A: Alarm> ClockWatch<A> {
    #[must_use]
    pub fn new(alarm: A) -> Self {
        Self { alarm }
    }

    /// Arms the alarm for the next minute boundary after `now`.
    ///
    /// # Errors
    ///
    /// When the kernel refuses the setting.
    pub fn arm(&self, now: SystemTime) -> io::Result<()> {
        self.alarm.set(next_minute_boundary(now))
    }

    /// Reads the alarm after its descriptor became readable, re-arms it and
    /// says whether to draw. `now` is the time after the read.
    pub fn service(&self, now: SystemTime) -> Wake {
        let wake = match self.alarm.read() {
            Ok(()) => Wake::Minute,
            Err(err) if err.raw_os_error() == Some(rustix::io::Errno::CANCELED.raw_os_error()) => {
                Wake::Stepped
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Wake::Idle,
            Err(err) => {
                // Unknown: draw (a wrong minute is worse than an extra frame)
                // and try to arm again.
                tracing::warn!(%err, "the clock timer could not be read");
                Wake::Minute
            }
        };
        if let Err(err) = self.arm(now) {
            tracing::error!(%err, "cannot re-arm the clock timer: the clock may go stale");
        }
        wake
    }
}

/// The realtime `timerfd`.
#[derive(Clone, Debug)]
pub struct RealtimeTimer {
    fd: Arc<OwnedFd>,
}

impl RealtimeTimer {
    /// A disarmed timer on `CLOCK_REALTIME`.
    ///
    /// # Errors
    ///
    /// When the kernel refuses the timerfd.
    pub fn new() -> io::Result<Self> {
        let fd = timerfd_create(
            TimerfdClockId::Realtime,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        Ok(Self { fd: Arc::new(fd) })
    }

    /// Arms it for the next minute boundary after the current time.
    ///
    /// # Errors
    ///
    /// When the kernel refuses the setting.
    pub fn arm_next_minute(&self) -> io::Result<()> {
        ClockWatch::new(self.clone()).arm(SystemTime::now())
    }

    /// Registers the timer on the loop: when it fires or is cancelled by a
    /// clock step it is read, re-armed, and `on_tick` runs (it posts the
    /// message that makes the shell draw again).
    ///
    /// # Errors
    ///
    /// When the loop refuses the source.
    pub fn install<D: 'static>(
        &self,
        handle: &LoopHandle<'static, D>,
        mut on_tick: impl FnMut() + 'static,
    ) -> Result<(), calloop::InsertError<Generic<Arc<OwnedFd>>>> {
        let watch = ClockWatch::new(self.clone());
        handle
            .insert_source(
                Generic::new(Arc::clone(&self.fd), Interest::READ, Mode::Level),
                move |_, _, _| {
                    if watch.service(SystemTime::now()).redraw() {
                        on_tick();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .map(|_| ())
    }
}

impl Alarm for RealtimeTimer {
    fn set(&self, at: SystemTime) -> io::Result<()> {
        // An all-zero `it_value` would disarm: an instant at or before the
        // epoch is one nanosecond after it, which has long passed.
        let since = at.duration_since(UNIX_EPOCH).unwrap_or_default();
        let value = Timespec {
            tv_sec: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(since.subsec_nanos().max(u32::from(since.as_secs() == 0))),
        };
        timerfd_settime(
            &*self.fd,
            TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET,
            &Itimerspec {
                it_interval: Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                it_value: value,
            },
        )?;
        Ok(())
    }

    fn read(&self) -> io::Result<()> {
        let mut count = [0u8; 8];
        rustix::io::read(&*self.fd, &mut count)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::event::{poll, PollFd, PollFlags};
    use std::cell::{Cell, RefCell};

    fn at(secs: u64, nanos: u32) -> SystemTime {
        UNIX_EPOCH + Duration::new(secs, nanos)
    }

    fn ready(timer: &RealtimeTimer, timeout_ms: i64) -> bool {
        let mut fds = [PollFd::new(&*timer.fd, PollFlags::IN)];
        let spec = Timespec {
            tv_sec: timeout_ms / 1_000,
            tv_nsec: timeout_ms % 1_000 * 1_000_000,
        };
        poll(&mut fds, Some(&spec)).map(|n| n > 0).unwrap_or(false)
    }

    #[test]
    fn the_timer_fires_at_the_next_minute_boundary() {
        // The boundary is the first whole minute strictly after now.
        assert_eq!(next_minute_boundary(at(120, 0)), at(180, 0));
        assert_eq!(next_minute_boundary(at(121, 0)), at(180, 0));
        assert_eq!(next_minute_boundary(at(179, 999_999_999)), at(180, 0));
        assert_eq!(next_minute_boundary(at(0, 0)), at(60, 0));
        // And the real descriptor honours an absolute realtime instant: not
        // before it, and once it has come.
        let timer = RealtimeTimer::new().expect("timerfd");
        assert!(!ready(&timer, 20), "a new timer is disarmed");
        let now = SystemTime::now();
        timer.set(now + Duration::from_secs(3_600)).expect("arm");
        assert!(!ready(&timer, 50), "an hour away is not now");
        timer.set(now + Duration::from_millis(30)).expect("arm");
        assert!(ready(&timer, 2_000), "it fires at its instant");
        timer.read().expect("one firing");
        assert!(!ready(&timer, 20), "read, it is quiet until re-armed");
        // The production arming is for a boundary within one minute.
        timer.arm_next_minute().expect("arm");
        let left = timer.fd.as_ref();
        let armed = rustix::time::timerfd_gettime(left).expect("gettime");
        assert!(
            armed.it_value.tv_sec <= 60 && armed.it_value.tv_sec >= 0,
            "{:?}",
            armed.it_value
        );
    }

    /// An alarm whose answers the test scripts: what `read` returns next and
    /// every instant it was armed for.
    struct Scripted {
        reads: RefCell<Vec<io::Result<()>>>,
        armed: RefCell<Vec<SystemTime>>,
        refuse_arm: Cell<bool>,
    }

    impl Scripted {
        fn new(reads: Vec<io::Result<()>>) -> Self {
            Self {
                reads: RefCell::new(reads),
                armed: RefCell::new(Vec::new()),
                refuse_arm: Cell::new(false),
            }
        }
    }

    impl Alarm for &Scripted {
        fn set(&self, at: SystemTime) -> io::Result<()> {
            if self.refuse_arm.get() {
                return Err(io::Error::other("refused"));
            }
            self.armed.borrow_mut().push(at);
            Ok(())
        }
        fn read(&self) -> io::Result<()> {
            self.reads
                .borrow_mut()
                .pop()
                .unwrap_or_else(|| Err(io::ErrorKind::WouldBlock.into()))
        }
    }

    fn canceled() -> io::Error {
        io::Error::from_raw_os_error(rustix::io::Errno::CANCELED.raw_os_error())
    }

    #[test]
    fn a_clock_step_reports_ecanceled_and_rearms() {
        // The real descriptor reports ECANCELED as errno 125; the watch
        // treats it as a step, re-arms for the boundary after the *new* time
        // and asks for a redraw.
        assert_eq!(canceled().raw_os_error(), Some(125));
        let script = Scripted::new(vec![Err(canceled())]);
        let watch = ClockWatch::new(&script);
        let after_step = at(1_000_000 * 60 + 17, 500);
        assert_eq!(watch.service(after_step), Wake::Stepped);
        assert!(Wake::Stepped.redraw());
        assert_eq!(
            *script.armed.borrow(),
            [at((1_000_000 + 1) * 60, 0)],
            "re-armed for the next boundary of the new time"
        );
        // A read with nothing to read is not a tick and arms nothing.
        assert_eq!(watch.service(after_step), Wake::Idle);
        assert!(!Wake::Idle.redraw());
        assert_eq!(script.armed.borrow().len(), 1);
        // A failed re-arm is logged, not fatal: the frame is still drawn.
        script.refuse_arm.set(true);
        script.reads.borrow_mut().push(Ok(()));
        assert_eq!(watch.service(after_step), Wake::Minute);
    }

    #[test]
    fn resume_from_suspend_redraws() {
        // The mechanism, with the real descriptor: a deadline the realtime
        // clock already passed is what a resumed machine looks like (its
        // clock jumped over the boundary while the timer slept), and it
        // fires at once instead of at the next minute.
        let timer = RealtimeTimer::new().expect("timerfd");
        let slept_through = SystemTime::now() - Duration::from_secs(3 * 3_600);
        timer.set(slept_through).expect("arm");
        assert!(ready(&timer, 2_000), "a passed deadline fires at once");
        let watch = ClockWatch::new(timer.clone());
        let woke = SystemTime::now();
        assert_eq!(watch.service(woke), Wake::Minute);
        assert!(Wake::Minute.redraw());
        // And it re-armed for the following boundary, within a minute.
        let armed = rustix::time::timerfd_gettime(timer.fd.as_ref()).expect("gettime");
        assert!(
            (0..=60).contains(&armed.it_value.tv_sec)
                && armed.it_value
                    != Timespec {
                        tv_sec: 0,
                        tv_nsec: 0
                    },
            "{:?}",
            armed.it_value
        );
        assert!(!ready(&timer, 20), "and is quiet until then");
        // The epoch itself is an instant long past, never a disarm.
        timer.set(UNIX_EPOCH).expect("arm");
        assert!(ready(&timer, 2_000));
    }

    #[test]
    fn format_uses_the_bundle_offset_and_no_time_zone_file() {
        // 2026-10-23 21:30:15 UTC, a Friday.
        let instant = at(1_792_791_015, 0);
        let utc = local_time(instant, 0);
        assert_eq!((utc.hour, utc.minute), (21, 30));
        assert_eq!(
            (utc.year, utc.month, utc.day, utc.weekday),
            (2026, 10, 23, 5)
        );
        assert_eq!(utc.clock(), "21:30");
        // The offset moves the hour and, across midnight, the date.
        let madrid = local_time(instant, 7_200);
        assert_eq!(madrid.clock(), "23:30");
        assert_eq!((madrid.month, madrid.day, madrid.weekday), (10, 23, 5));
        let tokyo = local_time(instant, 9 * 3_600);
        assert_eq!(tokyo.clock(), "06:30");
        assert_eq!((tokyo.month, tokyo.day, tokyo.weekday), (10, 24, 6));
        let newfoundland = local_time(instant, -(3 * 3_600 + 1_800));
        assert_eq!(newfoundland.clock(), "18:00");
        assert_eq!(newfoundland.day, 23);
        // The calendar: a leap day, a year end, the epoch, a time before it.
        assert_eq!(local_time(at(951_782_400, 0), 0).day, 29);
        let leap = local_time(at(951_782_400, 0), 0);
        assert_eq!(
            (leap.year, leap.month, leap.day, leap.weekday),
            (2000, 2, 29, 2)
        );
        let new_year = local_time(at(1_767_225_599, 0), 0);
        assert_eq!(
            (new_year.year, new_year.month, new_year.day),
            (2025, 12, 31)
        );
        let epoch = local_time(UNIX_EPOCH, 0);
        assert_eq!(
            (epoch.year, epoch.month, epoch.day, epoch.weekday),
            (1970, 1, 1, 4)
        );
        let before = local_time(UNIX_EPOCH - Duration::from_secs(1), 0);
        assert_eq!(
            (before.year, before.month, before.day, before.clock()),
            (1969, 12, 31, "23:59".to_owned())
        );
        // The far ends of the clock never panic.
        let _ = local_time(UNIX_EPOCH + Duration::from_secs(u64::MAX / 4), i32::MAX);
        let _ = local_time(UNIX_EPOCH - Duration::from_secs(u64::MAX / 4), i32::MIN);
        // The date line is the product copy's.
        assert_eq!(utc.date(), copy::lock_date(5, 23, 10));
        // Nothing here spawns a process or reads a zone file (a scan of this
        // file's production code).
        let source = include_str!("clock.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        for forbidden in [
            ["Command", "::new"].concat(),
            ["std::", "process"].concat(),
            ["/usr/share/", "zoneinfo"].concat(),
            ["/etc/", "localtime"].concat(),
            ["std::", "fs"].concat(),
            ["env::", "var"].concat(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "the lock clock uses {forbidden}"
            );
        }
    }
}
