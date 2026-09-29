//! The worker a tab's directory scans run on.
//!
//! One scan runs at a time and at most one request waits behind it; a newer
//! request replaces the waiting one. Two things this has to survive, because a
//! file manager meets them daily:
//!
//! - **A scan that never returns.** `read_dir` or `stat` on a dead network
//!   share, a sleeping phone or a pulled disk blocks in the kernel and cannot
//!   be cancelled from here. When a request is waiting behind a scan that has
//!   run longer than [`STUCK_AFTER`], that worker is abandoned: a fresh one
//!   takes the request, and whatever the stuck one eventually finds is thrown
//!   away. At most [`MAX_ABANDONED`] workers are ever abandoned, so a mount
//!   that swallows every scan cannot pile threads up; past the cap the request
//!   waits, as it always did.
//! - **No thread to spare.** Creating the worker can fail when the process is
//!   out of threads or memory, and that is an error the caller reports, not a
//!   panic that takes the whole file manager down.
//!
//! Dropping the executor cancels everything, then waits a bounded time for its
//! workers to finish: a tab closing on the Qt thread must never wait on a dead
//! mount.

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;

use crate::scan::{scan_directory, DirectorySnapshot, ScanError, ScanRequest};

pub type ScanResult = Result<DirectorySnapshot, ScanError>;

/// How long a scan may run with a request waiting behind it before its worker
/// is considered stuck and replaced. A healthy local folder of tens of
/// thousands of entries scans well inside this.
pub const STUCK_AFTER: Duration = Duration::from_secs(2);

/// How many stuck workers may be left behind before a waiting request simply
/// waits for one of them.
pub const MAX_ABANDONED: usize = 4;

/// How long dropping the executor waits for its workers before leaving the
/// stuck ones to finish on their own.
const DROP_WAIT: Duration = Duration::from_millis(500);

/// What reads one directory. The real one is [`scan_directory`]; tests put a
/// scanner that hangs in its place.
pub(crate) type Scanner = Arc<dyn Fn(&ScanRequest) -> ScanResult + Send + Sync>;
type Publisher = Mutex<Box<dyn Fn(ScanResult) + Send>>;

/// The bounds above, as values a test can shorten.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tuning {
    pub(crate) stuck_after: Duration,
    pub(crate) max_abandoned: usize,
}

/// The scan a worker is running, and since when.
#[derive(Debug)]
struct Running {
    cancellation: CancellationToken,
    since: Instant,
}

#[derive(Debug)]
struct ExecutorState {
    pending: Option<ScanRequest>,
    running: Option<Running>,
    shutting_down: bool,
    /// The worker whose answers are published. Any other has been abandoned.
    current: u64,
    next_worker: u64,
    /// Workers abandoned behind a stuck scan that have not returned yet.
    abandoned: usize,
    /// A standby thread is already watching the running scan.
    standby: bool,
    /// Threads started and not yet returned, for the bounded wait on drop.
    alive: usize,
}

struct SharedState {
    state: Mutex<ExecutorState>,
    wake: Condvar,
    scanner: Scanner,
    publish: Publisher,
    tuning: Tuning,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

/// Owns the scan workers and at most one pending scan request.
///
/// Replacing a pending request cancels it. Dropping the executor cancels both
/// pending and running work, then waits a bounded time for its workers.
pub struct ScanExecutor {
    shared: Arc<SharedState>,
}

impl fmt::Debug for ScanExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScanExecutor")
            .finish_non_exhaustive()
    }
}

/// The scan worker thread could not be created.
#[derive(Debug)]
pub struct WorkerUnavailable {
    source: io::Error,
}

impl fmt::Display for WorkerUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not start the scan worker: {}",
            self.source
        )
    }
}

impl Error for WorkerUnavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

impl ScanExecutor {
    /// Starts the worker that runs [`scan_directory`] and hands every current
    /// answer to `publish`.
    ///
    /// # Errors
    ///
    /// [`WorkerUnavailable`] when the operating system refuses a thread.
    pub fn new(publish: impl Fn(ScanResult) + Send + 'static) -> Result<Self, WorkerUnavailable> {
        Self::with_scanner(
            Arc::new(scan_directory),
            publish,
            Tuning {
                stuck_after: STUCK_AFTER,
                max_abandoned: MAX_ABANDONED,
            },
        )
    }

    pub(crate) fn with_scanner(
        scanner: Scanner,
        publish: impl Fn(ScanResult) + Send + 'static,
        tuning: Tuning,
    ) -> Result<Self, WorkerUnavailable> {
        let shared = Arc::new(SharedState {
            state: Mutex::new(ExecutorState {
                pending: None,
                running: None,
                shutting_down: false,
                current: 1,
                next_worker: 2,
                abandoned: 0,
                standby: false,
                alive: 1,
            }),
            wake: Condvar::new(),
            scanner,
            publish: Mutex::new(Box::new(publish)),
            tuning,
            handles: Mutex::new(Vec::new()),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("siderita-directory-scan".to_owned())
            .spawn(move || worker_loop(&worker_shared, 1))
            .map_err(|source| WorkerUnavailable { source })?;
        lock(&shared.handles).push(worker);
        Ok(Self { shared })
    }

    pub fn submit(&self, request: ScanRequest) -> Result<(), ExecutorStopped> {
        let mut state = lock(&self.shared.state);
        if state.shutting_down {
            return Err(ExecutorStopped);
        }

        if let Some(replaced) = state.pending.replace(request) {
            replaced.cancellation().cancel();
        }
        self.shared.wake.notify_all();

        // A scan is running and this request waits behind it. Someone has to
        // notice if that scan never comes back.
        let watch = state.running.is_some()
            && !state.standby
            && state.abandoned < self.shared.tuning.max_abandoned;
        if watch {
            let id = state.next_worker;
            state.next_worker += 1;
            state.standby = true;
            state.alive += 1;
            drop(state);
            let shared = Arc::clone(&self.shared);
            let started = thread::Builder::new()
                .name("siderita-directory-scan".to_owned())
                .spawn(move || standby(&shared, id));
            match started {
                Ok(handle) => lock(&self.shared.handles).push(handle),
                // No thread to spare: the request waits for the running scan,
                // which is what it did before there was a standby at all.
                Err(_) => {
                    let mut state = lock(&self.shared.state);
                    state.standby = false;
                    state.alive -= 1;
                }
            }
        }
        Ok(())
    }

    /// How many workers are currently abandoned behind stuck scans.
    #[cfg(test)]
    pub(crate) fn abandoned(&self) -> usize {
        lock(&self.shared.state).abandoned
    }
}

impl Drop for ScanExecutor {
    fn drop(&mut self) {
        let deadline = Instant::now() + DROP_WAIT;
        {
            let mut state = lock(&self.shared.state);
            state.shutting_down = true;
            if let Some(pending) = state.pending.take() {
                pending.cancellation().cancel();
            }
            if let Some(running) = state.running.take() {
                running.cancellation.cancel();
            }
            self.shared.wake.notify_all();
            while state.alive > 0 {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                state = self
                    .shared
                    .wake
                    .wait_timeout(state, deadline - now)
                    .map(|(state, _)| state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner().0);
            }
        }
        // Join what has returned; a worker still blocked in the kernel is left
        // to finish on its own, and publishes nothing when it does.
        for handle in std::mem::take(&mut *lock(&self.shared.handles)) {
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutorStopped;

impl fmt::Display for ExecutorStopped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scan executor is shutting down")
    }
}

impl Error for ExecutorStopped {}

/// Runs scans as worker `me` for as long as it is the current worker.
fn worker_loop(shared: &SharedState, me: u64) {
    loop {
        let request = {
            let mut state = lock(&shared.state);
            while state.pending.is_none() && !state.shutting_down {
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if state.shutting_down {
                retire(shared, &mut state);
                return;
            }
            let Some(request) = state.pending.take() else {
                continue;
            };
            state.running = Some(Running {
                cancellation: request.cancellation().clone(),
                since: Instant::now(),
            });
            request
        };

        let result = (shared.scanner)(&request);

        let publish = {
            let mut state = lock(&shared.state);
            if state.current != me {
                // Abandoned while this scan hung: a newer worker owns the
                // executor now, and this answer is nobody's.
                state.abandoned = state.abandoned.saturating_sub(1);
                retire(shared, &mut state);
                return;
            }
            state.running = None;
            shared.wake.notify_all();
            !state.shutting_down
        };

        if publish {
            (lock(&shared.publish))(result);
        }
    }
}

/// Watches the running scan on behalf of the request waiting behind it, and
/// takes over as worker `me` if that scan outlives the bound.
fn standby(shared: &SharedState, me: u64) {
    let mut state = lock(&shared.state);
    loop {
        let (Some(running), true, false) = (
            state.running.as_ref(),
            state.pending.is_some(),
            state.shutting_down,
        ) else {
            // The scan came back, or nothing is waiting any more.
            state.standby = false;
            retire(shared, &mut state);
            return;
        };
        let stuck_at = running.since + shared.tuning.stuck_after;
        let now = Instant::now();
        if now >= stuck_at {
            break;
        }
        state = shared
            .wake
            .wait_timeout(state, stuck_at - now)
            .map(|(state, _)| state)
            .unwrap_or_else(|poisoned| poisoned.into_inner().0);
    }

    // Still stuck, and a request still waits: abandon that worker and serve it.
    if let Some(running) = state.running.take() {
        running.cancellation.cancel();
    }
    state.abandoned += 1;
    state.current = me;
    state.standby = false;
    drop(state);
    worker_loop(shared, me);
}

/// Records that a worker or standby thread is returning.
fn retire(shared: &SharedState, state: &mut ExecutorState) {
    state.alive = state.alive.saturating_sub(1);
    shared.wake.notify_all();
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::{ScanCoordinator, ScanExecutor};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "celestina-scan-executor-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn publishes_a_completed_scan() {
        let fixture = TestDirectory::new();
        fs::write(fixture.0.join("entry"), b"content").expect("write fixture");
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin(&fixture.0).expect("scan request");
        let (sender, receiver) = mpsc::channel();
        let executor = ScanExecutor::new(move |result| {
            let _ = sender.send(result);
        })
        .expect("a scan worker");

        executor.submit(request).expect("submit scan");
        let snapshot = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("scan result")
            .expect("successful scan");

        assert_eq!(snapshot.entries().len(), 1);
    }

    #[test]
    fn dropping_idle_executor_joins_worker() {
        let executor = ScanExecutor::new(|_| {}).expect("a scan worker");
        drop(executor);
    }

    /// A scanner whose first call blocks until the test releases it, the way a
    /// `read_dir` on a dead mount does; every later call scans for real.
    struct Hang {
        started: mpsc::Receiver<()>,
        release: mpsc::Sender<()>,
        scanner: super::Scanner,
    }

    fn hanging_scanner() -> Hang {
        let (started_tx, started) = mpsc::channel::<()>();
        let (release, release_rx) = mpsc::channel::<()>();
        let first = std::sync::Mutex::new(Some((started_tx, release_rx)));
        let scanner: super::Scanner = std::sync::Arc::new(move |request| {
            let hold = first.lock().ok().and_then(|mut slot| slot.take());
            if let Some((started, release)) = hold {
                let _ = started.send(());
                let _ = release.recv();
            }
            crate::scan_directory(request)
        });
        Hang {
            started,
            release,
            scanner,
        }
    }

    fn quick() -> super::Tuning {
        super::Tuning {
            stuck_after: Duration::from_millis(100),
            max_abandoned: 2,
        }
    }

    /// SID-25: a scan blocked on a dead mount used to hold every later
    /// navigation of that tab behind it for as long as the mount took to give
    /// up. A request that supersedes a scan stuck past the bound is served by a
    /// fresh worker, and the stuck scan's late answer is never published.
    #[test]
    fn a_request_behind_a_stuck_scan_is_served_by_a_fresh_worker() {
        let stuck = TestDirectory::new();
        let wanted = TestDirectory::new();
        fs::write(wanted.0.join("entry"), b"x").expect("write fixture");
        let hang = hanging_scanner();
        let (sender, receiver) = mpsc::channel();
        let executor = ScanExecutor::with_scanner(
            hang.scanner,
            move |result| {
                let _ = sender.send(result);
            },
            quick(),
        )
        .expect("a scan worker");
        let mut coordinator = ScanCoordinator::new();

        executor
            .submit(coordinator.begin(&stuck.0).expect("request"))
            .expect("submit the scan that hangs");
        hang.started
            .recv_timeout(Duration::from_secs(2))
            .expect("the first scan started");
        executor
            .submit(coordinator.begin(&wanted.0).expect("request"))
            .expect("submit the navigation behind it");

        let snapshot = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the navigation is answered while the first scan still hangs")
            .expect("a successful scan");
        assert_eq!(snapshot.location(), wanted.0.as_path());

        // Letting the stuck scan finish publishes nothing: its worker was
        // abandoned, and an abandoned worker's answer is not current.
        let _ = hang.release.send(());
        assert!(receiver.recv_timeout(Duration::from_millis(300)).is_err());
    }

    /// Dropping the executor while a scan hangs returns within a bound instead
    /// of joining a thread that may never come back: that drop runs on the Qt
    /// thread when a tab closes.
    #[test]
    fn dropping_an_executor_with_a_stuck_scan_does_not_hang() {
        let stuck = TestDirectory::new();
        let hang = hanging_scanner();
        let executor =
            ScanExecutor::with_scanner(hang.scanner, |_| {}, quick()).expect("a scan worker");
        let mut coordinator = ScanCoordinator::new();
        executor
            .submit(coordinator.begin(&stuck.0).expect("request"))
            .expect("submit");
        hang.started
            .recv_timeout(Duration::from_secs(2))
            .expect("the scan started");

        let started = std::time::Instant::now();
        drop(executor);
        assert!(started.elapsed() < Duration::from_secs(3));
        let _ = hang.release.send(());
    }

    /// Abandoning is capped: past `max_abandoned` stuck workers no new one is
    /// started, so a mount that swallows every scan cannot pile threads up.
    #[test]
    fn abandoned_workers_are_capped() {
        let stuck = TestDirectory::new();
        let (entered_tx, entered) = mpsc::channel::<()>();
        let entered_tx = std::sync::Mutex::new(entered_tx);
        let (_never, parked) = mpsc::channel::<()>();
        let parked = std::sync::Mutex::new(parked);
        // Every scan hangs for good.
        let scanner: super::Scanner = std::sync::Arc::new(move |request| {
            if let Ok(sender) = entered_tx.lock() {
                let _ = sender.send(());
            }
            if let Ok(parked) = parked.lock() {
                let _ = parked.recv_timeout(Duration::from_secs(5));
            }
            crate::scan_directory(request)
        });
        let executor = ScanExecutor::with_scanner(scanner, |_| {}, quick()).expect("a scan worker");
        let mut coordinator = ScanCoordinator::new();

        let mut workers_entered = 0;
        for _ in 0..6 {
            executor
                .submit(coordinator.begin(&stuck.0).expect("request"))
                .expect("submit");
            if entered.recv_timeout(Duration::from_millis(400)).is_ok() {
                workers_entered += 1;
            }
        }
        // The first worker, plus at most `max_abandoned` replacements.
        assert!(
            workers_entered <= 1 + quick().max_abandoned,
            "{workers_entered}"
        );
        assert!(
            workers_entered >= 2,
            "a replacement did start: {workers_entered}"
        );
        assert_eq!(executor.abandoned(), quick().max_abandoned);
    }
}
