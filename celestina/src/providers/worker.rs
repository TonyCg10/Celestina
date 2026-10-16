// Copied from `celestina-qt/src/provider_adapter/worker.rs` at
// c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413 (the commit that moved it there
// unchanged from `celestina/src/provider_adapter/worker.rs`, last changed in
// 5924ad457b55d56dd52a7c7d5c69bc255117c5c4 and identical at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9), adapted: the panic is reported
// through `tracing` instead of the helper's stderr line, the prose names the
// shell's in-process workers, tests are added, and `join_within` bounds the
// join for the shell's exit.

//! One owned, cancellable provider thread.
//!
//! A provider that keeps a thread has to be able to stop it. Dropping a bare
//! `JoinHandle` detaches the thread instead, which then outlives the shell's
//! loop. Every provider thread uses this one shape so none can be left running
//! by an early return: the guard requests shutdown and waits, whether it is
//! joined deliberately or simply dropped.
//!
//! The flag is the shell's own shutdown flag, not a private one. A worker that
//! goes away is telling the rest of the process the same thing the signal
//! handler would. A body must therefore look at the flag at a bounded interval
//! (every blocking read or receive carries a timeout), which is what makes the
//! join itself bounded.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// A named provider thread that is stopped and joined when this is joined or
/// dropped.
pub struct Worker {
    what: &'static str,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Worker")
            .field("what", &self.what)
            .field("running", &self.handle.is_some())
            .finish()
    }
}

impl Worker {
    /// Starts `body` on a thread named `what`, which it will be held
    /// responsible for.
    ///
    /// # Errors
    ///
    /// Returns the failure to start the thread at all.
    pub fn spawn(
        what: &'static str,
        shutdown: &Arc<AtomicBool>,
        body: impl FnOnce() + Send + 'static,
    ) -> io::Result<Worker> {
        let handle = thread::Builder::new().name(what.to_owned()).spawn(body)?;
        Ok(Self {
            what,
            shutdown: Arc::clone(shutdown),
            handle: Some(handle),
        })
    }

    /// Asks the thread to stop and waits for it, reporting a panic by name.
    pub fn join(mut self) {
        if let Some(handle) = self.take_and_stop() {
            if handle.join().is_err() {
                tracing::error!("the {} worker panicked", self.what);
            }
        }
    }

    /// [`Worker::join`], waiting at most `wait`. The join runs on a helper
    /// thread; a worker that has not stopped by then (a body blocked in a
    /// read without a timeout) is reported and left behind, so the shell's
    /// exit stays bounded. Returns whether it stopped in time.
    pub fn join_within(self, wait: Duration) -> bool {
        let what = self.what;
        self.shutdown.store(true, Ordering::Release);
        let (done, finished) = mpsc::channel();
        let helper = thread::Builder::new()
            .name(format!("join-{what}"))
            .spawn(move || {
                self.join();
                let _ = done.send(());
            });
        match helper {
            Ok(_) => {
                let stopped = finished.recv_timeout(wait).is_ok();
                if !stopped {
                    tracing::warn!(
                        "the {what} worker did not stop within {} ms; it is left behind",
                        wait.as_millis()
                    );
                }
                stopped
            }
            // The closure, and the worker in it, was dropped: `Drop` has
            // joined it without a bound.
            Err(err) => {
                tracing::warn!(%err, "no helper thread to bound the join of the {what} worker");
                true
            }
        }
    }

    fn take_and_stop(&mut self) -> Option<JoinHandle<()>> {
        self.shutdown.store(true, Ordering::Release);
        self.handle.take()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(handle) = self.take_and_stop() {
            if handle.join().is_err() {
                tracing::error!("the {} worker panicked", self.what);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn until_shutdown(shutdown: &Arc<AtomicBool>) -> impl FnOnce() + Send + 'static {
        let shutdown = Arc::clone(shutdown);
        move || {
            while !shutdown.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    #[test]
    fn joining_raises_the_shared_flag_and_waits() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Worker::spawn("test-join", &shutdown, until_shutdown(&shutdown))
            .expect("the thread starts");

        worker.join();

        assert!(shutdown.load(Ordering::Acquire));
    }

    #[test]
    fn dropping_stops_the_thread_too() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let body = {
            let finished = Arc::clone(&finished);
            let wait = until_shutdown(&shutdown);
            move || {
                wait();
                finished.store(true, Ordering::Release);
            }
        };

        drop(Worker::spawn("test-drop", &shutdown, body).expect("the thread starts"));

        assert!(finished.load(Ordering::Acquire));
    }

    #[test]
    fn a_bounded_join_waits_for_a_worker_that_stops_and_leaves_one_that_does_not() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Worker::spawn("test-bounded", &shutdown, until_shutdown(&shutdown))
            .expect("the thread starts");
        assert!(worker.join_within(Duration::from_secs(2)));

        let stuck = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let body = {
            let release = Arc::clone(&release);
            move || {
                // Ignores the shutdown flag, as a read without a timeout would.
                while !release.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        };
        let worker = Worker::spawn("test-stuck", &stuck, body).expect("the thread starts");
        let started = std::time::Instant::now();
        assert!(!worker.join_within(Duration::from_millis(50)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(stuck.load(Ordering::Acquire), "the flag was raised");
        // Let the left-behind thread end with the test.
        release.store(true, Ordering::Release);
    }

    #[test]
    fn a_panicking_body_is_reported_not_propagated() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = Worker::spawn("test-panic", &shutdown, || panic!("on purpose"))
            .expect("the thread starts");

        // Returns instead of unwinding into the caller.
        worker.join();
    }
}
