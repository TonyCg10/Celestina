//! What every worker the app spawns answers to: one owner per QObject that
//! knows when the object is closing, hands each worker a guard to check
//! before it delivers, and joins every worker on drop. Pure and tested; the
//! models only decide what each worker does.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// The owner of a QObject's workers. Dropping it closes: the guard turns
/// false, every worker is joined, and nothing spawned afterwards runs.
#[derive(Default)]
pub struct Owned {
    closing: Arc<AtomicBool>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

/// What a worker asks before delivering to its owner: `open()` is false the
/// moment the owner begins shutdown, so a late result is dropped, never
/// applied to an object that is going away.
#[derive(Clone, Default)]
pub struct Guard(Arc<AtomicBool>);

impl Guard {
    pub fn open(&self) -> bool {
        !self.0.load(Ordering::Acquire)
    }

    /// Sleeps in slices, waking early when the owner closes; true when the
    /// whole time passed with the owner still open.
    #[cfg(test)]
    pub fn wait(&self, total: std::time::Duration) -> bool {
        let slice = std::time::Duration::from_millis(50);
        let mut left = total;
        while !left.is_zero() {
            if !self.open() {
                return false;
            }
            let step = left.min(slice);
            std::thread::sleep(step);
            left -= step;
        }
        self.open()
    }
}

impl Owned {
    pub fn guard(&self) -> Guard {
        Guard(Arc::clone(&self.closing))
    }

    /// Runs `work` on a thread of its own, joined on drop. A spawn after the
    /// owner began closing does nothing.
    pub fn spawn<F: FnOnce(Guard) + Send + 'static>(&self, work: F) {
        if self.closing.load(Ordering::Acquire) {
            return;
        }
        let guard = self.guard();
        let handle = std::thread::spawn(move || work(guard));
        let mut workers = self.workers.lock().unwrap_or_else(|e| e.into_inner());
        // Finished workers leave the list here, so a long-lived owner does
        // not hold a handle for every refresh it ever ran.
        workers.retain(|w| !w.is_finished());
        workers.push(handle);
    }

    /// Begins shutdown and joins every worker.
    pub fn close(&self) {
        self.closing.store(true, Ordering::Release);
        let workers: Vec<_> = self
            .workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for worker in workers {
            let _ = worker.join();
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.close();
    }
}

/// One kind of read, coalesced: while a read is in flight, further requests
/// fold into one follow-up that starts when the read finishes, so a burst
/// of signals costs at most two round trips and the newest snapshot wins.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reload {
    in_flight: bool,
    pending: bool,
}

impl Reload {
    /// A request: true when the caller must start a read now.
    pub fn request(&mut self) -> bool {
        if self.in_flight {
            self.pending = true;
            return false;
        }
        self.in_flight = true;
        true
    }

    /// The read finished: true when a follow-up read must start now.
    pub fn finish(&mut self) -> bool {
        let again = self.pending;
        self.in_flight = false;
        self.pending = false;
        if again {
            self.in_flight = true;
        }
        again
    }

    #[cfg(test)]
    pub fn in_flight(&self) -> bool {
        self.in_flight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[test]
    fn a_burst_coalesces_into_one_follow_up() {
        let mut reload = Reload::default();
        assert!(reload.request());
        assert!(!reload.request());
        assert!(!reload.request());
        assert!(!reload.request());
        assert!(reload.finish(), "one follow-up for the whole burst");
        assert!(reload.in_flight());
        assert!(!reload.finish(), "and nothing after it");
        assert!(!reload.in_flight());
        assert!(reload.request());
    }

    #[test]
    fn closing_joins_every_worker_and_no_late_result_is_delivered() {
        for _ in 0..20 {
            let delivered = Arc::new(AtomicUsize::new(0));
            let rejected = Arc::new(AtomicUsize::new(0));
            let owner = Owned::default();
            for i in 0..16 {
                let delivered = Arc::clone(&delivered);
                let rejected = Arc::clone(&rejected);
                owner.spawn(move |guard| {
                    // Half answer at once, half after the owner is gone.
                    if i % 2 == 1 {
                        guard.wait(Duration::from_millis(400));
                    }
                    if guard.open() {
                        delivered.fetch_add(1, Ordering::SeqCst);
                    } else {
                        rejected.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
            std::thread::sleep(Duration::from_millis(30));
            owner.close();
            // Every worker has returned: the counters are final.
            assert_eq!(
                delivered.load(Ordering::SeqCst) + rejected.load(Ordering::SeqCst),
                16
            );
            assert!(
                rejected.load(Ordering::SeqCst) >= 8,
                "the slow half was refused"
            );
            // A spawn after closing never runs.
            let late = Arc::clone(&delivered);
            owner.spawn(move |_| {
                late.fetch_add(100, Ordering::SeqCst);
            });
            assert!(delivered.load(Ordering::SeqCst) < 100);
        }
    }

    #[test]
    fn a_guard_wakes_early_when_the_owner_closes() {
        let owner = Owned::default();
        let guard = owner.guard();
        let started = std::time::Instant::now();
        owner.spawn(|guard| {
            assert!(!guard.wait(Duration::from_secs(10)));
        });
        owner.close();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!guard.open());
    }
}
