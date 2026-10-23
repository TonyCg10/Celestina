//! What the app keeps for the overview besides its surface (CEL-2-L): the
//! state machine, the panel, and the three short-lived workers that serve one
//! opening.
//!
//! Nothing here runs at rest. The capture worker is started by the request of
//! an opening and let go when its reply arrives or the opening ends (it owns a
//! Wayland connection only while a request is in flight); the timer that
//! bounds the wait and the bindings reader live for one wait or one read.
//! Each is a [`Worker`] with a flag of its own. None is ever joined on the
//! loop's thread: a worker that is done or given up is handed to [`reap`],
//! which drops (stops and joins) it on a [`Reaper`] thread.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::thumbnail::Thumb;
use celestina_wayland::OutputId;

use super::{flow::Flow, Overview};
use crate::capture::Spawner;
use crate::capture::{Busy, CaptureHandle, CaptureRequest, CAPTURE_TIMEOUT};
use crate::providers::keybinds::Keybinds;
use crate::providers::{Reaper, Worker};

/// How long past [`CAPTURE_TIMEOUT`] the app waits for the reply before it
/// opens the panel without it: the worker's own deadline fires first, this
/// only covers a worker that died or hung.
pub const TIMEOUT_MARGIN: Duration = Duration::from_millis(150);

/// How often the timer looks at its flag.
const TIMER_POLL: Duration = Duration::from_millis(20);

/// Why a capture was not asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAsked {
    /// Nothing is on screen to take a picture of.
    NothingToCapture,
    /// An earlier request is still in flight.
    Busy,
    /// The worker could not be started or has stopped.
    Unavailable,
}

/// Drops `worker` (raises its flag and joins it) on a [`Reaper`] thread, so
/// the loop's thread never waits on a join. Without a thread to spare the
/// reaper drops the worker here after all: every worker's join is bounded, so
/// that wait is too. The reaper is not kept: nothing starts an overview worker
/// again that this must be ordered before.
pub fn reap(worker: Worker) {
    drop(Reaper::spawn("overview", move || drop(worker)));
}

struct CaptureWorker {
    handle: CaptureHandle,
    /// The worker's own flag, raised at once when its capture is given up.
    shutdown: Arc<AtomicBool>,
    worker: Worker,
    /// The opening whose request it serves.
    generation: u64,
}

impl CaptureWorker {
    /// Signals the worker now and lets it go off the loop's thread.
    fn give_up(self) {
        self.shutdown.store(true, Ordering::Release);
        reap(self.worker);
    }
}

/// The overview's parts, owned by the app.
#[derive(Default)]
pub struct Slot {
    /// Where the overview is in its life.
    pub flow: Flow,
    /// The open panel; destroyed through the app, never dropped silently
    /// (its textures need the painter).
    pub panel: Option<Overview>,
    /// The output the current opening's panel opens on, chosen before its
    /// capture so that the capture names that output only.
    pub target: Option<OutputId>,
    /// Workspace previews that arrived and wait for the panel's next step.
    pub previews: Vec<(u64, Thumb)>,
    /// Bindings that arrived and wait for the panel's next step.
    pub keybinds: Option<Keybinds>,
    /// Closed panels whose textures could not be freed yet (the painter was
    /// busy); the app frees them at its next frame or close.
    pub dying: Vec<Overview>,
    capture: Option<CaptureWorker>,
    timer: Option<Worker>,
    reader: Option<(u64, Worker)>,
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("flow", &self.flow)
            .field("panel", &self.panel.is_some())
            .field("previews", &self.previews.len())
            .field("dying", &self.dying.len())
            .field("capturing", &self.capture.is_some())
            .finish_non_exhaustive()
    }
}

impl Slot {
    /// Asks for one capture of `request` on a worker started for it.
    ///
    /// # Errors
    ///
    /// Why the request was not made; the caller then opens the panel with
    /// geometry only.
    pub fn request_capture(
        &mut self,
        spawner: &dyn Spawner,
        request: CaptureRequest,
    ) -> Result<(), NotAsked> {
        if request.outputs.is_empty() {
            return Err(NotAsked::NothingToCapture);
        }
        let fresh = self.capture.is_none();
        if fresh {
            let shutdown = Arc::new(AtomicBool::new(false));
            match spawner.spawn(&shutdown) {
                Ok((handle, worker)) => {
                    self.capture = Some(CaptureWorker {
                        handle,
                        shutdown,
                        worker,
                        generation: request.generation,
                    });
                }
                Err(err) => {
                    tracing::warn!(%err, "overview: the capture worker did not start");
                    return Err(NotAsked::Unavailable);
                }
            }
        }
        let Some(capture) = self.capture.as_mut() else {
            return Err(NotAsked::Unavailable);
        };
        let generation = request.generation;
        match capture.handle.request(request) {
            Ok(()) => {
                capture.generation = generation;
                Ok(())
            }
            // The one in flight keeps its own number: its reply is dropped
            // as stale and lets the worker go. A worker started for this
            // request is let go at once: the capture in flight belongs to
            // another one (given up and still draining), and an idle worker
            // must not wait for the next opening.
            Err(Busy::InFlight) => {
                if fresh {
                    if let Some(idle) = self.capture.take() {
                        idle.give_up();
                    }
                }
                Err(NotAsked::Busy)
            }
            Err(Busy::Gone) => {
                // The next opening starts a fresh one.
                if let Some(gone) = self.capture.take() {
                    gone.give_up();
                }
                Err(NotAsked::Unavailable)
            }
        }
    }

    /// A capture reply of opening `generation` arrived: the worker that
    /// served it is done and is let go. A reply from a worker already given
    /// up leaves the current one alone.
    pub fn capture_replied(&mut self, generation: u64) {
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| capture.generation == generation)
        {
            if let Some(done) = self.capture.take() {
                done.give_up();
            }
        }
    }

    /// Gives up the capture in flight, if any: its worker is signalled now
    /// and let go, so a re-open asks anew and its reply, if one still comes,
    /// carries a number nobody waits for.
    pub fn cancel_capture(&mut self) {
        if let Some(capture) = self.capture.take() {
            capture.give_up();
        }
    }

    /// Starts the timer that calls `fire` once after the capture's deadline
    /// plus [`TIMEOUT_MARGIN`], unless [`Slot::stop_timer`] comes first.
    pub fn start_timer(&mut self, fire: impl FnOnce() + Send + 'static) {
        self.stop_timer();
        match spawn_timer(CAPTURE_TIMEOUT + TIMEOUT_MARGIN, fire) {
            Ok(worker) => self.timer = Some(worker),
            Err(err) => {
                // The worker's own deadline still bounds the wait.
                tracing::warn!(%err, "overview: no timer; the capture's own deadline bounds the wait");
            }
        }
    }

    /// Stops the wait's timer, if any, off the loop's thread.
    pub fn stop_timer(&mut self) {
        if let Some(timer) = self.timer.take() {
            reap(timer);
        }
    }

    /// Keeps the bindings reader of opening `generation` until its reply.
    pub fn hold_reader(&mut self, generation: u64, worker: Worker) {
        if let Some((_, old)) = self.reader.replace((generation, worker)) {
            reap(old);
        }
    }

    /// The bindings of opening `generation` arrived: that reader is done.
    pub fn reader_replied(&mut self, generation: u64) {
        if self
            .reader
            .as_ref()
            .is_some_and(|(held, _)| *held == generation)
        {
            if let Some((_, done)) = self.reader.take() {
                reap(done);
            }
        }
    }

    /// `output` is gone. An opening still waiting for its capture on it is
    /// given up whole (flow closed, capture cancelled, previews dropped)
    /// rather than opened on another output; returns whether one was.
    pub fn output_removed(&mut self, output: OutputId) -> bool {
        if self.flow.is_waiting() && self.target == Some(output) {
            self.flow.closed();
            self.clear_pending();
            return true;
        }
        false
    }

    /// Forgets what a closed panel was waiting for: the pictures and bindings
    /// not yet handed over, the timer, the capture in flight (signalled and
    /// let go) and the reader. Nothing is joined here.
    pub fn clear_pending(&mut self) {
        self.previews.clear();
        self.keybinds = None;
        self.target = None;
        self.stop_timer();
        self.cancel_capture();
        if let Some((_, reader)) = self.reader.take() {
            reap(reader);
        }
    }
}

/// Calls `fire` once after `wait`, from a thread that ends early when its
/// worker is dropped.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_timer(wait: Duration, fire: impl FnOnce() + Send + 'static) -> io::Result<Worker> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&shutdown);
    Worker::spawn("overview-timeout", &shutdown, move || {
        let due = Instant::now() + wait;
        while Instant::now() < due {
            if flag.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(TIMER_POLL.min(due.saturating_duration_since(Instant::now())));
        }
        if !flag.load(Ordering::Acquire) {
            fire();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureReply, CaptureResult, OutputRequest};
    use std::sync::atomic::AtomicU32;
    use std::sync::mpsc;

    fn request(generation: u64) -> CaptureRequest {
        CaptureRequest {
            generation,
            outputs: vec![OutputRequest {
                name: "DP-1".to_owned(),
                logical_width: 100,
                logical_height: 100,
                scale: 1.0,
                workspace_id: 1,
            }],
        }
    }

    /// Never starts a worker, counting the attempts.
    struct Failing(AtomicU32);

    impl Spawner for Failing {
        fn spawn(&self, _: &Arc<AtomicBool>) -> io::Result<(CaptureHandle, Worker)> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(io::Error::other("no thread"))
        }
    }

    #[test]
    fn an_empty_request_is_not_asked_and_starts_no_worker() {
        let spawner = Failing(AtomicU32::new(0));
        let mut slot = Slot::default();
        assert_eq!(
            slot.request_capture(&spawner, CaptureRequest::default()),
            Err(NotAsked::NothingToCapture)
        );
        assert_eq!(spawner.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_worker_that_cannot_start_is_unavailable_and_tried_again_next_time() {
        let spawner = Failing(AtomicU32::new(0));
        let mut slot = Slot::default();
        assert_eq!(
            slot.request_capture(&spawner, request(1)),
            Err(NotAsked::Unavailable)
        );
        assert_eq!(
            slot.request_capture(&spawner, request(2)),
            Err(NotAsked::Unavailable)
        );
        assert_eq!(spawner.0.load(Ordering::Relaxed), 2);
    }

    /// A capturer that answers when told to, or when it is stopped, and
    /// says on `ended` that its capture is over (`true`: it saw the stop).
    struct Held {
        release: mpsc::Receiver<()>,
        started: mpsc::Sender<()>,
        ended: mpsc::Sender<bool>,
        /// How long it keeps going after the stop, as a capture draining its
        /// last step would.
        drain: Duration,
    }

    impl crate::capture::worker::Capturer for Held {
        fn capture(
            &mut self,
            _: &CaptureRequest,
            _: Instant,
            shutdown: &AtomicBool,
        ) -> CaptureResult {
            let _ = self.started.send(());
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until && !shutdown.load(Ordering::Acquire) {
                if self.release.recv_timeout(Duration::from_millis(5)).is_ok() {
                    break;
                }
            }
            let stopped = shutdown.load(Ordering::Acquire);
            if stopped {
                thread::sleep(self.drain);
            }
            let _ = self.ended.send(stopped);
            CaptureResult::Unavailable("test".to_owned())
        }
    }

    /// Starts `Held` workers whose replies go to `replies`; each spawn takes
    /// the next release channel.
    struct HeldSpawner {
        releases: std::sync::Mutex<Vec<mpsc::Receiver<()>>>,
        replies: mpsc::Sender<CaptureReply>,
        started: mpsc::Sender<()>,
        ended: mpsc::Sender<bool>,
        drain: Duration,
        /// One mark for every worker it starts, as the live ones share
        /// `IN_FLIGHT`; a test's own, so parallel tests never meet.
        mark: &'static AtomicBool,
    }

    impl Spawner for HeldSpawner {
        fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(CaptureHandle, Worker)> {
            let release = self
                .releases
                .lock()
                .ok()
                .and_then(|mut list| list.pop())
                .ok_or_else(|| io::Error::other("spent"))?;
            let replies = self.replies.clone();
            crate::capture::worker::spawn(
                Held {
                    release,
                    started: self.started.clone(),
                    ended: self.ended.clone(),
                    drain: self.drain,
                },
                move |reply| replies.send(reply).is_ok(),
                shutdown,
                self.mark,
            )
        }
    }

    struct HeldRig {
        spawner: HeldSpawner,
        release: Vec<mpsc::Sender<()>>,
        replies: mpsc::Receiver<CaptureReply>,
        started: mpsc::Receiver<()>,
        ended: mpsc::Receiver<bool>,
    }

    fn held_draining(n: usize, drain: Duration) -> HeldRig {
        let (replies, inbox) = mpsc::channel();
        let (ended_tx, ended) = mpsc::channel();
        let (started_tx, started) = mpsc::channel();
        let (senders, mut receivers): (Vec<_>, Vec<_>) = (0..n).map(|_| mpsc::channel()).unzip();
        // Popped from the end: the first worker takes the first channel.
        receivers.reverse();
        HeldRig {
            spawner: HeldSpawner {
                releases: std::sync::Mutex::new(receivers),
                replies,
                started: started_tx,
                ended: ended_tx,
                drain,
                mark: Box::leak(Box::new(AtomicBool::new(false))),
            },
            release: senders,
            replies: inbox,
            started,
            ended,
        }
    }

    fn held(
        n: usize,
    ) -> (
        HeldSpawner,
        Vec<mpsc::Sender<()>>,
        mpsc::Receiver<CaptureReply>,
    ) {
        let rig = held_draining(n, Duration::ZERO);
        (rig.spawner, rig.release, rig.replies)
    }

    #[test]
    fn a_second_request_while_one_is_in_flight_is_busy_not_queued() {
        let (spawner, release, replies) = held(1);
        let mut slot = Slot::default();
        assert_eq!(slot.request_capture(&spawner, request(1)), Ok(()));
        assert_eq!(
            slot.request_capture(&spawner, request(2)),
            Err(NotAsked::Busy)
        );
        let _ = release[0].send(());
        let reply = replies.recv_timeout(Duration::from_secs(2));
        assert_eq!(reply.map(|r| r.generation), Ok(1), "the first one's number");
        // The stale-numbered reply still lets its own worker go.
        slot.capture_replied(1);
        assert!(slot.capture.is_none());
    }

    #[test]
    fn a_cancelled_capture_publishes_nothing_and_a_reopen_asks_anew() {
        // Spawns to spare: a request made in the instant between the
        // capture's end and its mark being cleared is refused as busy, and
        // the fresh worker it started is let go, spent.
        let rig = held_draining(8, Duration::ZERO);
        let mut slot = Slot::default();
        assert_eq!(slot.request_capture(&rig.spawner, request(1)), Ok(()));
        assert_eq!(rig.started.recv_timeout(Duration::from_secs(2)), Ok(()));
        let started = Instant::now();
        slot.cancel_capture();
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "cancelling never waits for the worker"
        );
        assert!(slot.capture.is_none());
        // The given-up capture ends at its next look at the stop; the test
        // waits for that end instead of racing it.
        assert_eq!(
            rig.ended.recv_timeout(Duration::from_secs(2)),
            Ok(true),
            "the capture saw the stop and ended"
        );
        // Its mark clears right after it says so; until then the next
        // opening is refused as busy.
        let deadline = Instant::now() + Duration::from_secs(2);
        let asked = loop {
            match slot.request_capture(&rig.spawner, request(2)) {
                Err(NotAsked::Busy) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                other => break other,
            }
        };
        assert_eq!(
            asked,
            Ok(()),
            "a fresh worker takes the next opening's request"
        );
        // The given-up one was stopped before it could publish.
        assert!(rig
            .replies
            .recv_timeout(Duration::from_millis(100))
            .is_err());
        slot.cancel_capture();
    }

    #[test]
    fn clearing_the_pending_work_cancels_the_capture() {
        let rig = held_draining(1, Duration::ZERO);
        let mut slot = Slot::default();
        assert_eq!(slot.request_capture(&rig.spawner, request(1)), Ok(()));
        assert_eq!(rig.started.recv_timeout(Duration::from_secs(2)), Ok(()));
        slot.previews.push((
            1,
            Thumb {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
        ));
        slot.clear_pending();
        assert!(slot.capture.is_none() && slot.previews.is_empty());
        assert_eq!(
            rig.ended.recv_timeout(Duration::from_secs(2)),
            Ok(true),
            "the capture saw the stop and ended"
        );
        assert!(rig.replies.recv_timeout(Duration::from_millis(50)).is_err());
    }

    #[test]
    fn a_draining_capture_keeps_the_next_opening_busy_until_it_ends() {
        let rig = held_draining(3, Duration::from_millis(300));
        let mut slot = Slot::default();
        assert_eq!(slot.request_capture(&rig.spawner, request(1)), Ok(()));
        assert_eq!(rig.started.recv_timeout(Duration::from_secs(2)), Ok(()));
        slot.cancel_capture();
        // The given-up worker is still draining: a fresh one is refused, and
        // is not kept idle.
        assert_eq!(
            slot.request_capture(&rig.spawner, request(2)),
            Err(NotAsked::Busy)
        );
        assert!(slot.capture.is_none(), "the idle fresh worker is let go");
        assert_eq!(rig.ended.recv_timeout(Duration::from_secs(2)), Ok(true));
        // Once it has ended (its ticket dropped right after), the next
        // opening is taken.
        let deadline = Instant::now() + Duration::from_secs(2);
        let asked = loop {
            match slot.request_capture(&rig.spawner, request(3)) {
                Err(NotAsked::Busy) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                other => break other,
            }
        };
        assert_eq!(asked, Ok(()));
        slot.cancel_capture();
    }

    #[test]
    fn an_output_removed_while_waiting_gives_the_opening_up() {
        use super::super::Mode;
        use crate::command::Presence;
        let (spawner, _release, replies) = held(1);
        let mut slot = Slot::default();
        slot.flow.verb(Mode::Desktop, Presence::Open);
        slot.target = Some(OutputId(4));
        assert_eq!(
            slot.request_capture(&spawner, request(slot.flow.generation())),
            Ok(())
        );
        assert!(!slot.output_removed(OutputId(5)), "another output");
        assert!(slot.flow.is_waiting());
        slot.previews.push((
            1,
            Thumb {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
        ));
        assert!(slot.output_removed(OutputId(4)));
        assert!(!slot.flow.is_active());
        assert!(slot.capture.is_none() && slot.previews.is_empty() && slot.target.is_none());
        // A late reply of that opening finds nothing waiting for it.
        assert_eq!(slot.flow.captured(slot.flow.generation()), Vec::new());
        assert!(replies.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(!slot.output_removed(OutputId(4)), "only once");
    }

    #[test]
    fn a_stale_reply_leaves_the_current_capture_alone() {
        let (spawner, _release, _replies) = held(1);
        let mut slot = Slot::default();
        assert_eq!(slot.request_capture(&spawner, request(5)), Ok(()));
        slot.capture_replied(4);
        assert!(slot.capture.is_some(), "a reply of opening 4 is not ours");
        slot.capture_replied(5);
        assert!(slot.capture.is_none());
    }

    #[test]
    fn letting_workers_go_never_waits_for_them() {
        let slow = |what| {
            let shutdown = Arc::new(AtomicBool::new(false));
            // Ignores its flag for a while, as a read in progress would.
            Worker::spawn(what, &shutdown, || {
                thread::sleep(Duration::from_millis(300))
            })
            .expect("the thread starts")
        };
        let mut slot = Slot::default();
        slot.hold_reader(3, slow("test-reader"));
        slot.timer = Some(slow("test-timer"));
        let started = Instant::now();
        slot.reader_replied(2);
        assert!(
            slot.reader.is_some(),
            "a reply of opening 2 is not this reader's"
        );
        slot.reader_replied(3);
        slot.stop_timer();
        slot.hold_reader(4, slow("test-reader-2"));
        slot.clear_pending();
        assert!(slot.reader.is_none() && slot.timer.is_none());
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn the_timer_fires_once_and_a_dropped_timer_never_does() {
        let (tx, rx) = mpsc::channel();
        let sender = tx.clone();
        let fired = spawn_timer(Duration::from_millis(30), move || {
            let _ = sender.send(1);
        })
        .expect("the thread starts");
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Ok(1));
        drop(fired);

        let sender = tx.clone();
        let stopped = spawn_timer(Duration::from_millis(200), move || {
            let _ = sender.send(2);
        })
        .expect("the thread starts");
        drop(stopped);
        drop(tx);
        assert!(rx.recv_timeout(Duration::from_millis(400)).is_err());
    }
}
