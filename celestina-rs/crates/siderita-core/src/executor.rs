use std::error::Error;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use celestina_core::CancellationToken;

use crate::scan::{scan_directory, DirectorySnapshot, ScanError, ScanRequest};

pub type ScanResult = Result<DirectorySnapshot, ScanError>;

#[derive(Debug)]
struct ExecutorState {
    pending: Option<ScanRequest>,
    running: Option<CancellationToken>,
    shutting_down: bool,
}

impl ExecutorState {
    const fn new() -> Self {
        Self {
            pending: None,
            running: None,
            shutting_down: false,
        }
    }
}

#[derive(Debug)]
struct SharedState {
    state: Mutex<ExecutorState>,
    wake: Condvar,
}

impl SharedState {
    const fn new() -> Self {
        Self {
            state: Mutex::new(ExecutorState::new()),
            wake: Condvar::new(),
        }
    }
}

/// Owns one worker and at most one pending scan request.
///
/// Replacing a pending request cancels it. Dropping the executor cancels both
/// pending and running work, then joins the worker before returning.
pub struct ScanExecutor {
    shared: Arc<SharedState>,
    worker: Option<JoinHandle<()>>,
}

impl fmt::Debug for ScanExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScanExecutor")
            .finish_non_exhaustive()
    }
}

impl ScanExecutor {
    pub fn new(publish: impl Fn(ScanResult) + Send + 'static) -> Self {
        let shared = Arc::new(SharedState::new());
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("siderita-directory-scan".to_owned())
            .spawn(move || worker_loop(&worker_shared, &publish))
            .expect("failed to create Siderita scan worker");

        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub fn submit(&self, request: ScanRequest) -> Result<(), ExecutorStopped> {
        let mut state = lock_state(&self.shared.state);
        if state.shutting_down {
            return Err(ExecutorStopped);
        }

        if let Some(replaced) = state.pending.replace(request) {
            replaced.cancellation().cancel();
        }
        self.shared.wake.notify_one();
        Ok(())
    }
}

impl Drop for ScanExecutor {
    fn drop(&mut self) {
        {
            let mut state = lock_state(&self.shared.state);
            state.shutting_down = true;
            if let Some(pending) = state.pending.take() {
                pending.cancellation().cancel();
            }
            if let Some(running) = state.running.take() {
                running.cancel();
            }
            self.shared.wake.notify_one();
        }

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
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

fn worker_loop(shared: &SharedState, publish: &impl Fn(ScanResult)) {
    loop {
        let request = {
            let mut state = lock_state(&shared.state);
            while state.pending.is_none() && !state.shutting_down {
                state = wait_for_work(&shared.wake, state);
            }

            if state.shutting_down {
                return;
            }

            let Some(request) = state.pending.take() else {
                continue;
            };
            state.running = Some(request.cancellation().clone());
            request
        };

        let result = scan_directory(&request);
        let should_publish = {
            let mut state = lock_state(&shared.state);
            state.running = None;
            !state.shutting_down
        };

        if should_publish {
            publish(result);
        }
    }
}

fn lock_state(mutex: &Mutex<ExecutorState>) -> MutexGuard<'_, ExecutorState> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wait_for_work<'a>(
    wake: &Condvar,
    state: MutexGuard<'a, ExecutorState>,
) -> MutexGuard<'a, ExecutorState> {
    wake.wait(state).unwrap_or_else(PoisonError::into_inner)
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
        });

        executor.submit(request).expect("submit scan");
        let snapshot = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("scan result")
            .expect("successful scan");

        assert_eq!(snapshot.entries().len(), 1);
    }

    #[test]
    fn dropping_idle_executor_joins_worker() {
        let executor = ScanExecutor::new(|_| {});
        drop(executor);
    }
}
