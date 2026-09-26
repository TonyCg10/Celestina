//! The suite's shared, interface-neutral rules: one owner for each invariant
//! several products need, so none of them keeps a private copy.
//!
//! - [`Generation`] and [`GenerationClock`] — which answer is current.
//! - [`CancellationToken`] — cooperative stop and pause for background work.
//! - [`atomic_file`] — small files written whole (suite state, private state,
//!   user media landed beside its source) and small files read bounded.
//! - [`xdg`] — the base directories, the runtime directory with no `/tmp`
//!   fallback, and the private (`0700`) directories the suite keeps under them.
//! - [`file_uri`] — `file://` URIs to local paths and back, strictly.
//! - [`percent`] — the byte-exact `%XX` codec behind path keys and URIs.
//! - [`pathkey`] — the opaque identity a path has across the Qt seam (ADR 0008).
//! - [`desktop_entry`] — `.desktop` files: parsing, `Exec` expansion, a bounded
//!   reader and the one application scan with one shadowing rule.
//! - [`image`] — whether untrusted cover art is bounded and plausible.
//!
//! Nothing here knows Qt, QML or an application. Every function that touches
//! the filesystem is blocking and belongs on a worker thread.
//!
//! # Dormant owners
//!
//! The 2026-09-26 monorepo audit found several of these rules copied across
//! the suite with different answers. Unit `RS-H1-A` added their owners without
//! switching any caller (ruling R-A3): [`file_uri`], [`xdg::runtime_dir`],
//! [`xdg::ensure_private_dir`], [`atomic_file::replace_private`],
//! [`atomic_file::land_media`] with [`atomic_file::stage_media`],
//! [`atomic_file::read_bounded`], [`atomic_file::Published`],
//! [`desktop_entry::read`], [`desktop_entry::scan`], [`desktop_entry::find`]
//! and [`CancellationToken::is_cancel_requested`]. Each module's "Adoption"
//! section names the product unit that moves its copies here; until then the
//! old copies keep their behaviour.

#![forbid(unsafe_code)]

pub mod atomic_file;
pub mod desktop_entry;
pub mod file_uri;
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

    /// Whether cancellation has been requested, answered at once.
    ///
    /// Unlike [`CancellationToken::is_cancelled`] this never waits, whether or
    /// not the token is paused, so any thread may ask it: a UI deciding what to
    /// show, a coordinator holding a lock, a callback that must not stall. A
    /// worker's own safe points keep using `is_cancelled`, which is also where
    /// a pause holds it.
    #[must_use]
    pub fn is_cancel_requested(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Whether the work should stop — and, while it is paused, where it waits.
    ///
    /// **This blocks while paused.** That is the point: every caller already
    /// asks this at each safe point, so the answer "not yet, and hold here" is
    /// delivered by simply not returning. It must therefore never be called on
    /// the UI thread, which is already true of every caller in this workspace:
    /// a token is asked by the worker that owns the operation. A caller that
    /// only needs to know uses [`CancellationToken::is_cancel_requested`].
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        while self.paused.load(Ordering::Acquire) && !self.cancelled.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Per-test scratch directories under the system temporary directory, removed
/// when the guard drops.
#[cfg(test)]
pub(crate) mod scratch {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    pub(crate) struct Scratch(PathBuf);

    impl Scratch {
        pub(crate) fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "celestina-core-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, Generation, GenerationClock};
    use std::sync::mpsc;
    use std::time::Duration;

    /// Runs `question` on another thread and fails if it has not answered
    /// within a second: the question must not wait on the token.
    fn answers_at_once(question: impl FnOnce() -> bool + Send + 'static) -> bool {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(question());
        });
        receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("the question waited on the token")
    }

    #[test]
    fn asking_for_a_cancel_request_never_waits_on_a_pause() {
        let token = CancellationToken::new();
        token.pause();

        let asked = token.clone();
        assert!(!answers_at_once(move || asked.is_cancel_requested()));

        token.cancel();
        let asked = token.clone();
        assert!(answers_at_once(move || asked.is_cancel_requested()));
    }

    #[test]
    fn a_pause_after_cancel_does_not_hold_a_worker() {
        let token = CancellationToken::new();
        token.cancel();
        token.pause();

        assert!(token.is_cancel_requested());
        let worker = token.clone();
        assert!(answers_at_once(move || worker.is_cancelled()));
    }

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
