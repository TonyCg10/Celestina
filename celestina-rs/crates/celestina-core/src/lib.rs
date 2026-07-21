#![forbid(unsafe_code)]

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
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
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
    fn cancellation_is_shared_between_clones() {
        let token = CancellationToken::new();
        let worker_token = token.clone();

        token.cancel();

        assert!(worker_token.is_cancelled());
    }
}
