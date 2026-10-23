//! The progressive delay after refusals (design §2 decision 5, H-6).
//!
//! The lock keeps one [`Backoff`] for its life: it starts from the failures
//! the shell carried over a relaunch, grows with every counted refusal and is
//! gone with the process at an authenticated unlock.

use std::time::Duration;

use crate::Verdict;

/// The longest wait after a refusal.
pub const WAIT_CAP: Duration = Duration::from_secs(30);
/// The wait after an attempt that proved nothing (a hung or missing stack).
pub const UNAVAILABLE_WAIT: Duration = Duration::from_secs(2);

/// The failures counted so far and the wait the last verdict imposed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Backoff {
    failures: u8,
    wait: Duration,
}

impl Backoff {
    /// A backoff with no failures and no wait.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            failures: 0,
            wait: Duration::ZERO,
        }
    }

    /// A backoff that continues a previous lock's count (the bundle's
    /// `prior_failures`). It imposes no wait by itself: nobody failed just
    /// now.
    #[must_use]
    pub const fn with_prior_failures(failures: u8) -> Self {
        Self {
            failures,
            wait: Duration::ZERO,
        }
    }

    /// The failures counted since the last unlock.
    #[must_use]
    pub const fn failures(&self) -> u8 {
        self.failures
    }

    /// The wait the last verdict imposed, from the moment it arrived.
    #[must_use]
    pub const fn wait(&self) -> Duration {
        self.wait
    }

    /// Takes one verdict in and returns the wait it imposes.
    ///
    /// A refusal or account refusal counts and waits `min(2^failures, 30)`
    /// seconds, so 2, 4, 8, 16, 30, 30. Any other verdict except
    /// `Authenticated` counts for nothing and waits [`UNAVAILABLE_WAIT`].
    /// `Authenticated` clears the count and waits nothing.
    pub fn record(&mut self, verdict: Verdict) -> Duration {
        self.wait = match verdict {
            Verdict::Authenticated => {
                self.failures = 0;
                Duration::ZERO
            }
            Verdict::Refused | Verdict::AccountRefused => {
                self.failures = self.failures.saturating_add(1);
                refusal_wait(self.failures)
            }
            Verdict::Unavailable
            | Verdict::Unsupported
            | Verdict::ServiceMissing
            | Verdict::BadRequest => UNAVAILABLE_WAIT,
        };
        self.wait
    }
}

/// `min(2^failures, 30)` seconds, without a shift that could overflow.
fn refusal_wait(failures: u8) -> Duration {
    let seconds = 1u64.checked_shl(u32::from(failures)).unwrap_or(u64::MAX);
    Duration::from_secs(seconds).min(WAIT_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn backoff_schedule_is_2_4_8_16_30_30() {
        let mut backoff = Backoff::new();
        assert_eq!(backoff.failures(), 0);
        assert_eq!(backoff.wait(), Duration::ZERO);
        let waits: Vec<Duration> = (0..8).map(|_| backoff.record(Verdict::Refused)).collect();
        assert_eq!(
            waits,
            [
                secs(2),
                secs(4),
                secs(8),
                secs(16),
                secs(30),
                secs(30),
                secs(30),
                secs(30)
            ]
        );
        assert_eq!(backoff.wait(), secs(30));
        assert_eq!(backoff.failures(), 8);
    }

    #[test]
    fn account_refusals_count_like_wrong_passwords() {
        let mut backoff = Backoff::new();
        assert_eq!(backoff.record(Verdict::AccountRefused), secs(2));
        assert_eq!(backoff.record(Verdict::Refused), secs(4));
        assert_eq!(backoff.record(Verdict::AccountRefused), secs(8));
        assert_eq!(backoff.failures(), 3);
    }

    #[test]
    fn unavailable_does_not_count_and_waits_2_s() {
        for verdict in [
            Verdict::Unavailable,
            Verdict::Unsupported,
            Verdict::ServiceMissing,
            Verdict::BadRequest,
        ] {
            let mut backoff = Backoff::with_prior_failures(3);
            assert_eq!(backoff.record(verdict), secs(2), "{verdict:?}");
            assert_eq!(backoff.wait(), secs(2), "{verdict:?}");
            assert_eq!(backoff.failures(), 3, "{verdict:?}");
            // The schedule continues where the refusals left it.
            assert_eq!(backoff.record(Verdict::Refused), secs(16), "{verdict:?}");
        }
    }

    #[test]
    fn prior_failures_continue_the_schedule() {
        let mut backoff = Backoff::with_prior_failures(2);
        assert_eq!(backoff.failures(), 2);
        // Seeding imposes no wait by itself: nobody has failed just now.
        assert_eq!(backoff.wait(), Duration::ZERO);
        assert_eq!(backoff.record(Verdict::Refused), secs(8));
        assert_eq!(backoff.failures(), 3);
    }

    #[test]
    fn a_very_large_count_neither_overflows_nor_exceeds_the_cap() {
        let mut backoff = Backoff::with_prior_failures(u8::MAX);
        assert_eq!(backoff.record(Verdict::Refused), secs(30));
        assert_eq!(backoff.failures(), u8::MAX);
        assert_eq!(backoff.record(Verdict::AccountRefused), secs(30));
    }

    #[test]
    fn authenticated_resets_the_count_and_waits_nothing() {
        let mut backoff = Backoff::with_prior_failures(4);
        assert_eq!(backoff.record(Verdict::Authenticated), Duration::ZERO);
        assert_eq!(backoff.failures(), 0);
        assert_eq!(backoff.wait(), Duration::ZERO);
    }

    #[test]
    fn the_attempt_deadline_is_fifteen_seconds() {
        assert_eq!(crate::ATTEMPT_DEADLINE, secs(15));
    }
}
