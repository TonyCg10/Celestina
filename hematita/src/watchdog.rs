//! A worker's bounded wait for a call that cannot bound its own wait.
//!
//! A unit action on the system manager does not return until polkit has
//! asked the person and the person has answered, and
//! `zbus::blocking::Proxy::call_with_flags` — the only call that carries
//! `AllowInteractiveAuth` — never consults the connection's
//! `method_timeout` (only `Connection::call_method` does). [`answer_within`]
//! therefore runs such a call on a thread of its own and waits for its answer
//! at most a given time; the caller decides what an expired wait means and
//! how to release whatever the call is still holding.

use std::io;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

/// How a bounded wait ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Waited<T> {
    /// The call answered in time.
    Answered(T),
    /// The limit passed first; the call's thread is still waiting and its
    /// answer, if one ever comes, is dropped.
    Expired,
    /// The call's thread ended without an answer.
    Lost,
}

/// Runs `call` on a thread named `name` and waits at most `limit` for what
/// it returns.
///
/// # Errors
///
/// The OS refused to create the thread; nothing was called.
pub fn answer_within<T: Send + 'static>(
    name: &str,
    limit: Duration,
    call: impl FnOnce() -> T + Send + 'static,
) -> io::Result<Waited<T>> {
    // One slot: the answer is sent once and never blocks its sender, even
    // when nobody is waiting for it any more.
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let _ = sender.send(call());
        })?;
    Ok(match receiver.recv_timeout(limit) {
        Ok(answer) => Waited::Answered(answer),
        Err(RecvTimeoutError::Timeout) => Waited::Expired,
        Err(RecvTimeoutError::Disconnected) => Waited::Lost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "hematita-watchdog-test";

    #[test]
    fn a_call_that_answers_in_time_is_answered() {
        let waited = answer_within(NAME, Duration::from_secs(5), || 7).expect("a thread");
        assert_eq!(waited, Waited::Answered(7));
    }

    #[test]
    fn a_call_that_never_answers_expires_at_the_limit() {
        let (release, parked) = mpsc::channel::<()>();
        let started = std::time::Instant::now();
        let waited = answer_within(NAME, Duration::from_millis(50), move || {
            // Stands for a polkit prompt nobody answers.
            let _ = parked.recv();
            "late"
        })
        .expect("a thread");
        assert_eq!(waited, Waited::Expired);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the wait is bounded"
        );
        // The parked call ends and its answer is dropped without blocking.
        let _ = release.send(());
    }

    #[test]
    fn a_call_whose_thread_ends_without_an_answer_is_lost() {
        let waited = answer_within(NAME, Duration::from_secs(5), || -> u8 {
            panic!("the call's thread ends without an answer")
        })
        .expect("a thread");
        assert_eq!(waited, Waited::Lost);
    }
}
