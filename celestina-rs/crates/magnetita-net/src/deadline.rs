//! Absolute deadlines for blocking socket work.
//!
//! A socket timeout is per syscall, so a peer that dribbles one byte per tick
//! keeps a per-syscall-bounded read loop alive forever. Every bounded phase in
//! this crate therefore fixes one [`Instant`] up front and re-checks it on each
//! iteration; the socket timeout only decides how often that check happens.
//!
//! This is the one place that recipe lives: the payload transfer and the link
//! handshake share it rather than each keeping its own clock arithmetic.

use std::io;
use std::time::{Duration, Instant};

/// How long is left before `deadline`, or a `TimedOut` error carrying `message`
/// once the deadline has passed.
pub(crate) fn remaining_before(deadline: Instant, message: &'static str) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, message))
}

/// Whether an error only means "nothing arrived in this syscall's window", in
/// which case the caller retries until its own absolute deadline decides.
pub(crate) fn is_retryable_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

#[cfg(test)]
mod tests {
    use super::{is_retryable_timeout, remaining_before};
    use std::io;
    use std::time::{Duration, Instant};

    #[test]
    fn a_future_deadline_reports_what_is_left() {
        let remaining = remaining_before(Instant::now() + Duration::from_secs(5), "late").unwrap();
        assert!(remaining <= Duration::from_secs(5));
        assert!(!remaining.is_zero());
    }

    #[test]
    fn a_passed_deadline_is_a_timeout_carrying_its_reason() {
        let error =
            remaining_before(Instant::now() - Duration::from_millis(1), "late").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("late"));
    }

    #[test]
    fn only_window_expiry_and_interruption_are_retryable() {
        for kind in [
            io::ErrorKind::TimedOut,
            io::ErrorKind::WouldBlock,
            io::ErrorKind::Interrupted,
        ] {
            assert!(is_retryable_timeout(&io::Error::from(kind)));
        }
        for kind in [
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::UnexpectedEof,
        ] {
            assert!(!is_retryable_timeout(&io::Error::from(kind)));
        }
    }
}
