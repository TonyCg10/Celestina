//! The machine's one deadline, as a monotonic `timerfd` in the loop
//! (design §4.10: "the delay and fade timers").
//!
//! The fd is armed to the machine's next wake and disarmed when it has none,
//! so a lock at rest wakes for nothing. When it fires, a loop source reads it
//! and posts a message to the shell; the shell turns that into
//! [`crate::machine::Input::Tick`] and re-arms. No thread, no polling.
#![forbid(unsafe_code)]

use std::io;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};
use rustix::time::{
    timerfd_create, timerfd_settime, Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags,
    Timespec,
};

/// The shortest arming: a deadline already past fires at once, not never (an
/// all-zero `it_value` would disarm).
const MIN_ARM: Duration = Duration::from_millis(1);

/// A re-armable monotonic timer.
#[derive(Clone, Debug)]
pub struct Wake {
    fd: Arc<OwnedFd>,
}

fn timespec(duration: Duration) -> Timespec {
    Timespec {
        tv_sec: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(duration.subsec_nanos()),
    }
}

impl Wake {
    /// A disarmed timer.
    ///
    /// # Errors
    ///
    /// When the kernel refuses the timerfd.
    pub fn new() -> io::Result<Self> {
        let fd = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        Ok(Self { fd: Arc::new(fd) })
    }

    /// Arms the timer for `at` (`None` disarms it).
    ///
    /// # Errors
    ///
    /// When the kernel refuses the new setting.
    pub fn arm(&self, at: Option<Instant>, now: Instant) -> io::Result<()> {
        let value = at.map_or(Duration::ZERO, |at| {
            at.saturating_duration_since(now).max(MIN_ARM)
        });
        timerfd_settime(
            &*self.fd,
            TimerfdTimerFlags::empty(),
            &Itimerspec {
                it_interval: timespec(Duration::ZERO),
                it_value: timespec(value),
            },
        )?;
        Ok(())
    }

    /// Reads the expiration count, clearing readiness. A timer that is not
    /// ready returns at once.
    pub fn drain(&self) {
        let mut count = [0u8; 8];
        let _ = rustix::io::read(&*self.fd, &mut count);
    }

    /// Registers the timer on the loop: when it fires it is drained and
    /// `on_fire` runs (it posts the message that wakes the shell).
    ///
    /// # Errors
    ///
    /// When the loop refuses the source.
    pub fn install<D: 'static>(
        &self,
        handle: &LoopHandle<'static, D>,
        mut on_fire: impl FnMut() + 'static,
    ) -> Result<(), calloop::InsertError<Generic<Arc<OwnedFd>>>> {
        let wake = self.clone();
        handle
            .insert_source(
                Generic::new(Arc::clone(&self.fd), Interest::READ, Mode::Level),
                move |_, _, _| {
                    wake.drain();
                    on_fire();
                    Ok(PostAction::Continue)
                },
            )
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::event::{poll, PollFd, PollFlags};

    fn ready(wake: &Wake, timeout_ms: i64) -> bool {
        let mut fds = [PollFd::new(&*wake.fd, PollFlags::IN)];
        let spec = Timespec {
            tv_sec: 0,
            tv_nsec: timeout_ms * 1_000_000,
        };
        poll(&mut fds, Some(&spec)).map(|n| n > 0).unwrap_or(false)
    }

    #[test]
    fn an_armed_timer_fires_and_a_disarmed_one_does_not() {
        let wake = Wake::new().expect("timerfd");
        let now = Instant::now();
        assert!(!ready(&wake, 20), "a new timer is disarmed");
        wake.arm(Some(now + Duration::from_millis(10)), now)
            .expect("arm");
        assert!(ready(&wake, 500), "it fires");
        wake.drain();
        assert!(!ready(&wake, 20), "drained, it is quiet until re-armed");
        wake.arm(Some(now + Duration::from_secs(60)), now)
            .expect("arm");
        wake.arm(None, now).expect("disarm");
        assert!(!ready(&wake, 30), "disarmed");
    }

    #[test]
    fn a_deadline_in_the_past_fires_at_once() {
        let wake = Wake::new().expect("timerfd");
        let now = Instant::now();
        wake.arm(Some(now), now + Duration::from_secs(5))
            .expect("arm");
        assert!(ready(&wake, 500));
    }
}
