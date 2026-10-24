// Copied from `celestina-qt/src/provider_adapter/worker.rs` at
// c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413 (the commit that moved it there
// unchanged from `celestina/src/provider_adapter/worker.rs`, last changed in
// 5924ad457b55d56dd52a7c7d5c69bc255117c5c4 and identical at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9), adapted: the panic is reported
// through `tracing` instead of the helper's stderr line, the prose names the
// shell's in-process workers, tests are added, and `join_within` bounds the
// join for the shell's exit. `Reaper` (the off-loop stop of a worker whose
// duty was turned off) moved here from `celestina/src/energy/mod.rs` when
// CEL-2-K's glance workers became its second user.

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
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
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

/// A short-lived named thread running the stop of a worker whose duty was
/// turned off, so the event loop never waits for the join itself.
///
/// The stop is expected to be bounded ([`Worker::join_within`]). Whoever may
/// start the same worker again calls [`Reaper::wait`] first, so an old and a
/// new instance never overlap; the exit path waits for every reaper too.
///
/// A reaper is a handle on **one stop finishing**, so it can be cloned and
/// waited for by several parties: a chain of replacements, each one's
/// teardown waiting for the teardown it replaced, stays transitive (CEL-2-I,
/// I-1). Waiting does not join the thread; it waits for the stop to return
/// (or to panic), after which the thread has nothing left to do.
#[derive(Clone, Debug, Default)]
pub struct Reaper(Option<Arc<Finished>>);

/// Whether a reaper's stop has returned, and the way to wait for it.
#[derive(Debug, Default)]
struct Finished {
    done: Mutex<bool>,
    ended: Condvar,
}

impl Finished {
    fn lock(&self) -> MutexGuard<'_, bool> {
        // The flag is a plain bool: a poisoned lock holds no broken invariant.
        match self.done.lock() {
            Ok(done) => done,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// Marks a reaper's stop as over when the thread leaves it, by any path.
struct Signal(Arc<Finished>);

impl Drop for Signal {
    fn drop(&mut self) {
        if thread::panicking() {
            tracing::error!("a worker reaper panicked");
        }
        *self.0.lock() = true;
        self.0.ended.notify_all();
    }
}

impl Reaper {
    /// Runs `stop` on a thread named `reap-{what}`. Without a thread, `stop`
    /// is dropped unrun, which drops what it owns: a [`Worker`] in it is then
    /// joined by its `Drop`, on this thread.
    pub fn spawn(what: &'static str, stop: impl FnOnce() + Send + 'static) -> Reaper {
        let finished = Arc::new(Finished::default());
        let signal = Signal(Arc::clone(&finished));
        match thread::Builder::new()
            .name(format!("reap-{what}"))
            .spawn(move || {
                let _signal = signal;
                stop();
            }) {
            Ok(_) => Reaper(Some(finished)),
            Err(err) => {
                tracing::warn!(%err, "no thread to reap the {what} worker; it was joined here");
                Reaper(None)
            }
        }
    }

    /// Waits for the stop to return, if there is one. Idempotent: waiting
    /// again, or on a clone, returns at once once it is over.
    pub fn wait(&self) {
        let Some(finished) = &self.0 else {
            return;
        };
        let mut done = finished.lock();
        while !*done {
            done = match finished.ended.wait(done) {
                Ok(done) => done,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_reaper_runs_the_stop_off_the_caller_and_wait_waits_for_it() {
        let done = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let reaper = {
            let done = Arc::clone(&done);
            let release = Arc::clone(&release);
            Reaper::spawn("test", move || {
                while !release.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(2));
                }
                done.store(true, Ordering::Release);
            })
        };
        // `spawn` returned while the stop is still held.
        assert!(!done.load(Ordering::Acquire));
        release.store(true, Ordering::Release);
        reaper.wait();
        assert!(done.load(Ordering::Acquire));
        // Waiting again, or on a default reaper, is nothing.
        reaper.wait();
        Reaper::default().wait();
    }

    #[test]
    fn a_cloned_reaper_is_waited_for_by_every_holder_and_a_panicking_stop_still_ends() {
        let release = Arc::new(AtomicBool::new(false));
        let reaper = {
            let release = Arc::clone(&release);
            Reaper::spawn("test-clone", move || {
                while !release.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(2));
                }
            })
        };
        let waiters: Vec<_> = (0..3)
            .map(|_| {
                let reaper = reaper.clone();
                thread::spawn(move || reaper.wait())
            })
            .collect();
        thread::sleep(Duration::from_millis(30));
        assert!(
            waiters.iter().all(|waiter| !waiter.is_finished()),
            "nobody is released while the stop runs"
        );
        release.store(true, Ordering::Release);
        for waiter in waiters {
            assert!(waiter.join().is_ok());
        }
        // A holder that comes late finds it over.
        reaper.wait();

        let panicked = Reaper::spawn("test-panic", || panic!("on purpose"));
        panicked.wait();
    }

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
