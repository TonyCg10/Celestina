#![forbid(unsafe_code)]

pub mod atomic_file;
pub mod desktop_entry;
pub mod image;
pub mod pathkey;
pub mod percent;
pub mod xdg;

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Identifies one request in a monotonically increasing sequence.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Generation(u64);

impl Generation {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Issues unique generations without silently wrapping at `u64::MAX`.
#[derive(Debug, Default)]
pub struct GenerationClock {
    current: Generation,
}

impl GenerationClock {
    #[must_use]
    pub const fn current(&self) -> Generation {
        self.current
    }

    pub fn issue(&mut self) -> Result<Generation, GenerationExhausted> {
        let next = self.current.checked_next().ok_or(GenerationExhausted)?;
        self.current = next;
        Ok(next)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationExhausted;

impl fmt::Display for GenerationExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generation counter exhausted")
    }
}

impl Error for GenerationExhausted {}

/// A cheap, cloneable cancellation signal for cooperative background work.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        // A cancelled operation must not stay parked: releasing the pause is
        // what lets the worker reach its next check and stop there.
        self.paused.store(false, Ordering::Release);
    }

    /// Holds the work where it is, without giving it up.
    ///
    /// Pausing rides on this token rather than on a second one because the
    /// places a long operation asks "should I stop?" are exactly the places it
    /// is safe to wait: between two entries, and between two chunks of one
    /// file. A separate token would have to be threaded through every signature
    /// in two crates to reach those same points.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Whether the work should stop — and, while it is paused, where it waits.
    ///
    /// **This blocks while paused.** That is the point: every caller already
    /// asks this at each safe point, so the answer "not yet, and hold here" is
    /// delivered by simply not returning. It must therefore never be called on
    /// the UI thread, which is already true of every caller in this workspace:
    /// a token is asked by the worker that owns the operation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        while self.paused.load(Ordering::Acquire) && !self.cancelled.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        self.cancelled.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, Generation, GenerationClock};

    #[test]
    fn clock_issues_monotonic_generations() {
        let mut clock = GenerationClock::default();

        assert_eq!(clock.current(), Generation::INITIAL);
        assert_eq!(clock.issue().expect("first generation").value(), 1);
        assert_eq!(clock.issue().expect("second generation").value(), 2);
    }

    #[test]
    fn a_paused_token_holds_its_worker_until_it_resumes() {
        let token = CancellationToken::new();
        token.pause();
        assert!(token.is_paused());

        let worker = token.clone();
        let held = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = held.clone();
        let handle = std::thread::spawn(move || {
            // Blocks here until the pause lifts, then answers.
            let cancelled = worker.is_cancelled();
            flag.store(true, std::sync::atomic::Ordering::Release);
            cancelled
        });

        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(
            !held.load(std::sync::atomic::Ordering::Acquire),
            "the worker went past a paused token"
        );

        token.resume();
        assert!(
            !handle.join().expect("worker"),
            "resuming is not cancelling"
        );
    }

    #[test]
    fn cancelling_releases_a_paused_worker() {
        let token = CancellationToken::new();
        token.pause();
        let worker = token.clone();
        let handle = std::thread::spawn(move || worker.is_cancelled());
        std::thread::sleep(std::time::Duration::from_millis(80));
        token.cancel();
        assert!(handle.join().expect("worker"), "a cancelled pause must end");
    }

    #[test]
    fn cancellation_is_shared_between_clones() {
        let token = CancellationToken::new();
        let worker_token = token.clone();

        token.cancel();

        assert!(worker_token.is_cancelled());
    }
}
