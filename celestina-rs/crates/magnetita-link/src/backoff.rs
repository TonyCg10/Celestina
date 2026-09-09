//! The reconnection schedule, as a pure state machine the daemon steps.
//!
//! A phone comes and goes constantly — Doze, Wi-Fi, a walk to the kitchen —
//! so the link is reconnected forever, but never hammered: the delay doubles
//! from a quarter second to a minute and resets the moment a session was
//! actually established.

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backoff {
    attempt: u32,
}

impl Backoff {
    pub const FIRST: Duration = Duration::from_millis(250);
    pub const LONGEST: Duration = Duration::from_secs(60);

    pub fn new() -> Self {
        Self { attempt: 0 }
    }

    /// How long to wait before the next attempt, then counts it.
    pub fn next_delay(&mut self) -> Duration {
        let delay = Self::FIRST
            .saturating_mul(1u32 << self.attempt.min(8))
            .min(Self::LONGEST);
        self.attempt = self.attempt.saturating_add(1);
        delay
    }

    /// A session was established: the next failure starts over.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_from_a_quarter_second_and_caps_at_a_minute() {
        let mut b = Backoff::new();
        let delays: Vec<u64> = (0..12).map(|_| b.next_delay().as_millis() as u64).collect();
        assert_eq!(&delays[..5], &[250, 500, 1000, 2000, 4000]);
        assert_eq!(delays[8], 60_000);
        assert_eq!(delays[11], 60_000);
        b.reset();
        assert_eq!(b.next_delay(), Backoff::FIRST);
    }
}
