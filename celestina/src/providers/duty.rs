//! The on/off life of one duty's worker (CEL-2-J): a duty that is `on` has a
//! worker, a duty that is `off` has none. It is the shape
//! [`crate::energy::EnergyHandle`] keeps for its three workers, kept once for
//! the phone and bubbles duties.
//!
//! Every start gets a **run number**. The worker stamps the news it sends with
//! it, and the loop drops news whose number is not the current run's: a worker
//! that was just switched off may still have one value in flight, and it must
//! not show up as the state of a duty that is off (or of its successor).
//! Neither stopping nor starting blocks the loop: the old worker is joined on
//! a short-lived thread, and a duty switched on again starts its new worker at
//! once without waiting for that join. For a moment the old worker (shutting
//! down, bounded by [`JOIN_WAIT`]) and the new one may both exist; both are
//! only clients of the Melibea socket or the Magnetita bus, and the run number
//! keeps the old one's last news out of the new state. Only [`DutyWorker::stop`]
//! (the shell leaving) waits for the reapers, each bounded by [`JOIN_WAIT`].

use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::Worker;

/// How long stopping one worker waits for it.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);

/// A running worker and the way into it.
struct Slot<C: ?Sized> {
    control: Box<C>,
    worker: Worker,
}

impl<C: ?Sized> Slot<C> {
    fn stop(self) {
        let Slot { control, worker } = self;
        // Dropping the control disconnects the queue, so the worker also ends
        // by itself; the flag is raised by the join.
        drop(control);
        worker.join_within(JOIN_WAIT);
    }
}

/// One duty's worker, if the duty is on.
pub struct DutyWorker<C: ?Sized + Send + 'static> {
    what: &'static str,
    slot: Option<Slot<C>>,
    /// The threads still joining workers that were switched off.
    reapers: Vec<JoinHandle<()>>,
    run: u64,
}

impl<C: ?Sized + Send + 'static> DutyWorker<C> {
    /// A duty with no worker; `what` names it in the journal.
    #[must_use]
    pub fn new(what: &'static str) -> Self {
        Self {
            what,
            slot: None,
            reapers: Vec::new(),
            run: 0,
        }
    }

    /// Whether the worker exists.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.slot.is_some()
    }

    /// The way into the worker, while it runs.
    #[must_use]
    pub fn control(&self) -> Option<&C> {
        self.slot.as_ref().map(|slot| &*slot.control)
    }

    /// Whether news stamped `run` is the running worker's.
    #[must_use]
    pub fn accepts(&self, run: u64) -> bool {
        self.slot.is_some() && run == self.run
    }

    /// Brings the duty to `on`; returns whether a worker started or stopped.
    ///
    /// `start` receives the run number to stamp news with and the worker's
    /// own shutdown flag, and returns the control and the worker. A failure to
    /// start is logged and leaves the duty without a worker; the next call
    /// tries again.
    pub fn set(
        &mut self,
        on: bool,
        start: impl FnOnce(u64, &Arc<AtomicBool>) -> io::Result<(Box<C>, Worker)>,
    ) -> bool {
        match (on, self.slot.is_some()) {
            (true, false) => {
                self.reapers.retain(|reaper| !reaper.is_finished());
                // Before `start`, so news from a worker that fails halfway is
                // already not the previous run's.
                let run = self.run.wrapping_add(1);
                self.run = run;
                let flag = Arc::new(AtomicBool::new(false));
                match start(run, &flag) {
                    Ok((control, worker)) => {
                        self.slot = Some(Slot { control, worker });
                        tracing::info!("{} duty on", self.what);
                        true
                    }
                    Err(err) => {
                        tracing::warn!("{}: the worker did not start: {err}", self.what);
                        false
                    }
                }
            }
            (false, true) => {
                if let Some(slot) = self.slot.take() {
                    let spawned = std::thread::Builder::new()
                        .name(format!("reap-{}", self.what))
                        .spawn(move || slot.stop());
                    match spawned {
                        Ok(handle) => self.reapers.push(handle),
                        // The closure, and the slot in it, was dropped: the
                        // worker's `Drop` joined it on this thread.
                        Err(err) => tracing::warn!(
                            %err,
                            "no thread to reap the {} worker; it was joined here",
                            self.what
                        ),
                    }
                    tracing::info!("{} duty off", self.what);
                }
                true
            }
            _ => false,
        }
    }

    /// Stops the worker and waits for it (and for the reapers still joining
    /// earlier ones, each bounded by [`JOIN_WAIT`]). Also what dropping the
    /// duty does.
    pub fn stop(&mut self) {
        if let Some(slot) = self.slot.take() {
            slot.stop();
        }
        for handle in self.reapers.drain(..) {
            if handle.join().is_err() {
                tracing::error!("the {} reaper panicked", self.what);
            }
        }
    }
}

impl<C: ?Sized + Send + 'static> Drop for DutyWorker<C> {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Instant;

    use super::*;

    /// A worker that ignores its shutdown flag until `release` is raised.
    fn stubborn(
        release: &Arc<AtomicBool>,
    ) -> impl FnOnce(u64, &Arc<AtomicBool>) -> io::Result<(Box<()>, Worker)> {
        let release = Arc::clone(release);
        move |_, flag| {
            let worker = Worker::spawn("test-duty", flag, move || {
                while !release.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(5));
                }
            })?;
            Ok((Box::new(()), worker))
        }
    }

    #[test]
    fn switching_on_again_does_not_wait_for_the_old_worker() {
        let release = Arc::new(AtomicBool::new(false));
        let mut duty: DutyWorker<()> = DutyWorker::new("test");
        assert!(duty.set(true, stubborn(&release)));
        assert!(duty.accepts(1));
        assert!(duty.set(false, stubborn(&release)), "a worker stopped");
        assert!(!duty.is_on());

        // The old worker is still blocked: an off -> on must not wait for it.
        let begun = Instant::now();
        assert!(duty.set(true, stubborn(&release)));
        assert!(
            begun.elapsed() < Duration::from_millis(500),
            "no unbounded join on the loop: {:?}",
            begun.elapsed()
        );
        assert!(duty.is_on());
        assert!(duty.accepts(2), "the new run is current");
        assert!(!duty.accepts(1), "the old run's news is dropped");

        release.store(true, Ordering::Release);
        duty.stop();
        assert!(!duty.is_on());
    }

    #[test]
    fn a_failed_start_leaves_no_worker_and_a_new_run_number() {
        let mut duty: DutyWorker<()> = DutyWorker::new("test");
        let seen = std::cell::Cell::new(0);
        let failed = duty.set(true, |run, _| {
            seen.set(run);
            Err(io::Error::other("no thread"))
        });
        assert!(!failed);
        assert_eq!(seen.get(), 1);
        assert!(!duty.is_on());
        assert!(
            !duty.accepts(1),
            "news of a duty without a worker is dropped"
        );
        let release = Arc::new(AtomicBool::new(true));
        assert!(duty.set(true, stubborn(&release)));
        assert!(duty.accepts(2));
        duty.stop();
    }

    #[test]
    fn rapid_toggling_keeps_only_the_last_run_current() {
        let release = Arc::new(AtomicBool::new(true));
        let mut duty: DutyWorker<()> = DutyWorker::new("test");
        let begun = Instant::now();
        for _ in 0..20 {
            assert!(duty.set(true, stubborn(&release)));
            assert!(duty.set(false, stubborn(&release)));
        }
        assert!(duty.set(true, stubborn(&release)));
        assert!(begun.elapsed() < Duration::from_secs(2));
        assert!(duty.accepts(21));
        assert!((1..21).all(|run| !duty.accepts(run)));
        duty.stop();
    }
}
