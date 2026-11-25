//! The on-screen display duty (CEL-2-C): volume, microphone and brightness
//! changes shown on a `celestina-osd` layer surface, behind `duties.osd`,
//! on since the cutover (CEL-2-M) unless `shell.json` says off.
//!
//! This module is the duty's wiring. [`OsdRuntime`] holds what exists only
//! while the loop runs: the audio and brightness workers the duty asks for
//! (none while it is off, started and stopped live as `shell.json` changes,
//! each run with a shutdown flag of its own) and the calloop timer that
//! carries the display's timeouts back into the loop as [`OsdTick`]s.
//! [`surface::Osds`] is the app's side: the pure `celestina_shell::osd`
//! machine, the per-monitor brightness baselines and the surface, which is
//! created when something shows and destroyed after its exit, so a display
//! at rest costs no surface and no frame. [`view`] is its look.
//!
//! Starting a worker is behind [`Spawner`] and arming a timer behind
//! [`TimerLoop`], so the rules are tested without a compositor, a bus or a
//! child process.

pub mod audio_rows;
pub mod control;
pub mod monitor_rows;
pub mod popup;
pub mod readings;
pub mod surface;
pub mod verbs;
pub mod view;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};
use celestina_shell::osd::Kind;
use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::audio::{self, AudioCommand, AudioProvider};
use crate::providers::brightness::{self, BrightnessCommand, BrightnessHandle, BrightnessWorkers};
use crate::providers::Worker;

/// The layer-shell namespace of the display's surface.
pub const NAMESPACE: &str = "celestina-osd";

/// How long stopping the duty's workers waits for them in all.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);

/// A timer of the display came due; each carries the generation it was armed
/// for, so a timer outlived by a later change is recognised and ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsdTick {
    /// The display's timeout passed (`celestina_shell::osd::Timer`).
    Expire(u64),
    /// The exit fade of the display that went away is over: its surface can
    /// go.
    Retire(u64),
    /// The workers of a duty turned off have stopped: a duty turned on again
    /// meanwhile can start its own now ([`OsdRuntime::resume`]).
    Reaped,
}

/// A slider popup opening or closing, as the display hears it (posted by the
/// loop to itself, because an input call controls only its own surface).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suppression {
    /// A popup driving these kinds opened.
    Open(&'static [Kind]),
    /// It closed; `asked` is the last level it asked of each kind (`None`
    /// for a mute toggle), so their read-backs are not announced. `written`
    /// is how many distinct monitors of a brightness list it asked a level
    /// of: the DDC worker writes and reads them one at a time, so the quiet
    /// after the close lasts one settle for each.
    Closed {
        kinds: &'static [Kind],
        asked: Vec<(Kind, Option<u8>)>,
        written: usize,
    },
}

/// Arms the display's one timer.
pub trait TimerLoop {
    /// Delivers `tick` to the loop at `at`. A later call replaces an earlier
    /// timer that has not fired.
    fn arm(&mut self, at: Instant, tick: OsdTick);
}

/// The display's timer on the shell's calloop loop: the timer's callback only
/// posts the tick through the loop's own sender, because the decisions need
/// the app and its `RuntimeCtl`, which a calloop callback does not have.
pub struct LoopTimer<D: 'static> {
    handle: LoopHandle<'static, D>,
    sender: Sender<Message>,
    armed: Option<RegistrationToken>,
}

impl<D: 'static> LoopTimer<D> {
    #[must_use]
    pub fn new(handle: LoopHandle<'static, D>, sender: Sender<Message>) -> Self {
        Self {
            handle,
            sender,
            armed: None,
        }
    }
}

impl<D: 'static> TimerLoop for LoopTimer<D> {
    fn arm(&mut self, at: Instant, tick: OsdTick) {
        // A token of a timer that already fired is stale; calloop ignores it.
        if let Some(old) = self.armed.take() {
            self.handle.remove(old);
        }
        let sender = self.sender.clone();
        let inserted = self
            .handle
            .insert_source(Timer::from_deadline(at), move |_, _, _| {
                if sender.send(Message::Osd(tick)).is_err() {
                    tracing::debug!(?tick, "osd: the loop is gone; the timer is dropped");
                }
                TimeoutAction::Drop
            });
        match inserted {
            Ok(token) => self.armed = Some(token),
            Err(err) => {
                tracing::warn!(?tick, error = %err.error, "osd: no timer; the display stays until the next change")
            }
        }
    }
}

/// A worker thread of the duty that can be stopped within a bound.
pub trait Stoppable: Send {
    /// Raises its shutdown flag and waits at most `wait`; whether it stopped.
    fn stop_within(self: Box<Self>, wait: Duration) -> bool;
}

impl Stoppable for Worker {
    fn stop_within(self: Box<Self>, wait: Duration) -> bool {
        self.join_within(wait)
    }
}

impl Stoppable for BrightnessWorkers {
    fn stop_within(self: Box<Self>, wait: Duration) -> bool {
        self.join_within(wait)
    }
}

/// What starting the duty produced: the queues of the workers that started
/// and their threads.
#[derive(Default)]
pub struct Started {
    pub audio: Option<AudioProvider>,
    pub brightness: Option<BrightnessHandle>,
    pub threads: Vec<Box<dyn Stoppable>>,
}

/// Starts the duty's workers with the duty's own `shutdown` flag.
pub trait Spawner {
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> Started;

    /// What the reaper thread runs once the workers of a duty turned off
    /// have stopped: the live one posts [`OsdTick::Reaped`] to the loop.
    fn on_reaped(&self) -> Box<dyn FnOnce() + Send> {
        Box::new(|| {})
    }
}

/// The real workers, publishing into the loop.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> Started {
        let mut started = Started::default();
        match audio::spawn(self.sender.clone(), shutdown) {
            Ok((provider, worker)) => {
                started.audio = Some(provider);
                started.threads.push(Box::new(worker));
            }
            Err(err) => tracing::warn!(%err, "osd: no audio worker; volume changes are not shown"),
        }
        match brightness::spawn(self.sender.clone(), shutdown) {
            Ok((handle, workers)) => {
                started.brightness = Some(handle);
                started.threads.push(Box::new(workers));
            }
            Err(err) => {
                tracing::warn!(%err, "osd: no brightness workers; brightness changes are not shown")
            }
        }
        started
    }

    fn on_reaped(&self) -> Box<dyn FnOnce() + Send> {
        let sender = self.sender.clone();
        Box::new(move || {
            if sender.send(Message::Osd(OsdTick::Reaped)).is_err() {
                tracing::debug!("osd: the loop is gone; the reaper's news is dropped");
            }
        })
    }
}

/// The running workers of the duty.
struct Running {
    audio: Option<AudioProvider>,
    brightness: Option<BrightnessHandle>,
    threads: Vec<Box<dyn Stoppable>>,
}

impl Running {
    /// Stops every thread within [`JOIN_WAIT`] in all; the queues are
    /// dropped first so the workers also see their senders gone.
    fn stop(self) {
        let Running {
            audio,
            brightness,
            threads,
        } = self;
        drop(audio);
        drop(brightness);
        let started = Instant::now();
        for thread in threads {
            thread.stop_within(JOIN_WAIT.saturating_sub(started.elapsed()));
        }
    }
}

/// The thread that is stopping the workers of a duty turned off, so the loop
/// never waits on them. A duty turned on again while it runs does not start
/// (two `pactl subscribe` children, or two DDC owners, must never run at
/// once) and does not wait for it either: the reaper posts
/// [`OsdTick::Reaped`] when it is done and the start happens then.
#[derive(Default)]
struct Reaper {
    handle: Option<std::thread::JoinHandle<()>>,
    /// Raised by the thread once the workers have stopped.
    done: Arc<AtomicBool>,
}

impl Reaper {
    fn reap(running: Running, then: Box<dyn FnOnce() + Send>) -> Reaper {
        let done = Arc::new(AtomicBool::new(false));
        let raised = Arc::clone(&done);
        match std::thread::Builder::new()
            .name("reap-osd".to_owned())
            .spawn(move || {
                running.stop();
                raised.store(true, Ordering::Release);
                then();
            }) {
            Ok(handle) => Reaper {
                handle: Some(handle),
                done,
            },
            Err(err) => {
                // The closure, and the workers in it, was dropped: their
                // guards have joined them on this thread.
                tracing::warn!(%err, "osd: no thread to stop the workers; they were joined here");
                Reaper::default()
            }
        }
    }

    /// Whether the workers it stops may still run. Never blocks: a thread
    /// past its stop has only its news left to post and is let go.
    fn busy(&mut self) -> bool {
        if self.handle.is_none() {
            return false;
        }
        if !self.done.load(Ordering::Acquire) {
            return true;
        }
        if let Some(handle) = self.handle.take() {
            if handle.is_finished() && handle.join().is_err() {
                tracing::error!("osd: the worker reaper panicked");
            }
        }
        false
    }

    /// Waits for it: the exit path only, never the loop's own work.
    fn wait(&mut self) {
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                tracing::error!("osd: the worker reaper panicked");
            }
        }
    }
}

struct Inner {
    spawner: Box<dyn Spawner>,
    running: Option<Running>,
    reaper: Reaper,
    /// The settings in force ask for the duty (its start may be waiting for
    /// the reaper).
    wanted: bool,
    /// The settings in force give the DDC worker a consumer.
    consumer: bool,
    /// The running brightness workers were told so (one `Show` each).
    consumer_told: bool,
    /// The settings in force give the audio graph a consumer.
    graph_consumer: bool,
    /// The running audio worker was told so (one `Graph(true)`).
    graph_told: bool,
}

impl Inner {
    fn start(&mut self) {
        // The duty's own flag: stopping its workers must not stop the
        // shell, whose flag the other workers share.
        let flag = Arc::new(AtomicBool::new(false));
        let started = self.spawner.spawn(&flag);
        tracing::info!(
            audio = started.audio.is_some(),
            brightness = started.brightness.is_some(),
            "osd duty on: workers started"
        );
        self.running = Some(Running {
            audio: started.audio,
            brightness: started.brightness,
            threads: started.threads,
        });
        self.consumer_told = false;
        self.tell_consumer();
        self.graph_told = false;
        self.tell_graph_consumer();
    }

    /// Tells the audio worker the graph has a consumer, once per consumer
    /// and per worker.
    fn tell_graph_consumer(&mut self) {
        if !self.graph_consumer || self.graph_told {
            return;
        }
        let Some(handle) = self.running.as_ref().and_then(|r| r.audio.as_ref()) else {
            return;
        };
        // A refusal is logged by the handle; the next settings change, or the
        // next volume popup, asks again.
        self.graph_told = handle.request(AudioCommand::Graph(true)).is_ok();
    }

    /// Takes whether the settings in force give the graph a consumer: one
    /// that appears is told to the running worker, one that left is taken
    /// back from it.
    fn set_graph_consumer(&mut self, consumer: bool) {
        self.graph_consumer = consumer;
        if consumer {
            self.tell_graph_consumer();
            return;
        }
        if std::mem::take(&mut self.graph_told) {
            if let Some(handle) = self.running.as_ref().and_then(|r| r.audio.as_ref()) {
                // A refusal is logged by the handle; the worker then keeps
                // reading a graph nobody lists, until it stops.
                let _ = handle.request(AudioCommand::Graph(false));
            }
        }
    }

    /// Tells the brightness workers a consumer exists, once per consumer
    /// and per set of workers.
    fn tell_consumer(&mut self) {
        if !self.consumer || self.consumer_told {
            return;
        }
        let Some(handle) = self.running.as_ref().and_then(|r| r.brightness.as_ref()) else {
            return;
        };
        // A refusal (a full queue, a worker gone) is logged by the handle;
        // the next settings change asks again.
        self.consumer_told = handle.request(BrightnessCommand::Show).is_ok();
    }

    /// Takes whether the settings in force give the DDC worker a consumer:
    /// one that appears is told to the running workers, one that left is
    /// taken back from them.
    fn set_consumer(&mut self, consumer: bool) {
        self.consumer = consumer;
        if consumer {
            self.tell_consumer();
            return;
        }
        if std::mem::take(&mut self.consumer_told) {
            if let Some(handle) = self.running.as_ref().and_then(|r| r.brightness.as_ref()) {
                // A refusal is logged by the handle. The worker then keeps
                // reading new monitors for a piece that left, until its
                // workers stop: a read too many, never a write.
                let _ = handle.request(BrightnessCommand::Hide);
            }
        }
    }
}

/// The duty's runtime parts: its workers and its timer.
pub struct OsdRuntime {
    inner: RefCell<Inner>,
    timer: RefCell<Box<dyn TimerLoop>>,
}

impl std::fmt::Debug for OsdRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsdRuntime")
            .field("running", &self.running())
            .finish_non_exhaustive()
    }
}

impl OsdRuntime {
    /// The runtime over the real system and the shell's loop. Nothing runs
    /// until [`OsdRuntime::apply`] finds the duty on.
    #[must_use]
    pub fn new<D: 'static>(sender: Sender<Message>, handle: LoopHandle<'static, D>) -> Self {
        let timer = LoopTimer::new(handle, sender.clone());
        Self::with(Box::new(Live { sender }), Box::new(timer))
    }

    /// The runtime over `spawner` and `timer`, for tests.
    #[must_use]
    pub fn with(spawner: Box<dyn Spawner>, timer: Box<dyn TimerLoop>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                running: None,
                reaper: Reaper::default(),
                wanted: false,
                consumer: false,
                consumer_told: false,
                graph_consumer: false,
                graph_told: false,
            }),
            timer: RefCell::new(timer),
        }
    }

    /// Whether the duty's workers exist.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.borrow().running.is_some()
    }

    /// Brings the workers to what `settings` ask for: the duty on has them,
    /// off has none (stopped by a reaper thread, never on the loop's). A
    /// duty turned on while the last workers still stop starts when the
    /// reaper is done ([`OsdRuntime::resume`]). The brightness workers are
    /// also told whether a layout holds the brightness piece
    /// ([`control::brightness_consumer`]): `Show` once when it does, `Hide`
    /// once when it no longer does; and the audio worker whether one holds
    /// the volume piece ([`control::volume_consumer`]): `Graph(true)` once
    /// when it does, `Graph(false)` once when it no longer does. Idempotent,
    /// so it is called at start and after every settings change.
    pub fn apply(&self, settings: &Settings) {
        let on = settings.duties.osd == Duty::On;
        let mut inner = self.inner.borrow_mut();
        inner.wanted = on;
        // Before the workers start, so a start tells them; and here rather
        // than on an output's arrival, because at the shell's start the
        // outputs are announced before this runtime exists (a placed piece
        // then never asked for its detection).
        inner.set_consumer(control::brightness_consumer(settings));
        inner.set_graph_consumer(control::volume_consumer(settings));
        match (on, inner.running.is_some()) {
            (true, false) => {
                if inner.reaper.busy() {
                    tracing::info!(
                        "osd duty on: the last workers are still stopping; it starts when they have"
                    );
                    return;
                }
                inner.start();
            }
            (false, true) => {
                if let Some(running) = inner.running.take() {
                    tracing::info!("osd duty off: workers stopping");
                    let then = inner.spawner.on_reaped();
                    inner.reaper = Reaper::reap(running, then);
                }
            }
            _ => {}
        }
    }

    /// Asks the audio worker again for the graph when the settings want it
    /// and an earlier ask was refused (a full queue); nothing when it was
    /// told or the volume piece is absent. Called when a volume popup opens.
    pub fn retry_graph_gate(&self) {
        self.inner.borrow_mut().tell_graph_consumer();
    }

    /// The reaper is done ([`OsdTick::Reaped`]): a duty wanted on meanwhile
    /// starts now.
    pub fn resume(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.wanted && inner.running.is_none() && !inner.reaper.busy() {
            inner.start();
        }
    }

    /// The audio worker's queue while the duty runs.
    #[must_use]
    pub fn audio(&self) -> Option<AudioProvider> {
        self.inner
            .borrow()
            .running
            .as_ref()
            .and_then(|r| r.audio.clone())
    }

    /// The brightness workers' queue while the duty runs.
    #[must_use]
    pub fn brightness(&self) -> Option<BrightnessHandle> {
        self.inner
            .borrow()
            .running
            .as_ref()
            .and_then(|r| r.brightness.clone())
    }

    /// An output appeared or went: the DDC worker re-detects once, and only
    /// if it detected before (plan decision 5). Nothing while the duty is
    /// off.
    pub fn outputs_changed(&self) {
        if let Some(handle) = self.brightness() {
            // A refusal is logged by the handle; the next change asks again.
            let _ = handle.request(BrightnessCommand::OutputsChanged);
        }
    }

    /// Arms the display's timer.
    pub fn arm(&self, at: Instant, tick: OsdTick) {
        match self.timer.try_borrow_mut() {
            Ok(mut timer) => timer.arm(at, tick),
            Err(_) => tracing::warn!(?tick, "osd: the timer is in use; not armed"),
        }
    }

    /// Stops the workers and waits for them (and for a reaper still stopping
    /// earlier ones): the exit path. Also what dropping the runtime does.
    pub fn stop(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(running) = inner.running.take() {
            running.stop();
        }
        inner.reaper.wait();
    }
}

impl Drop for OsdRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    use celestina_shell::settings::Duties;

    use super::*;
    use crate::test_support::quiet_settings;

    /// Counts the starts and stops of worker threads that run until their
    /// flag is raised, as the real ones do, taking `stop_delay` to stop.
    #[derive(Clone, Default)]
    struct Counting {
        started: Arc<AtomicUsize>,
        stopped: Arc<AtomicUsize>,
        reaped: Arc<AtomicUsize>,
        stop_delay: Duration,
        brightness: Option<BrightnessHandle>,
        audio: Option<AudioProvider>,
    }

    impl Spawner for Counting {
        fn spawn(&self, shutdown: &Arc<AtomicBool>) -> Started {
            self.started.fetch_add(1, Ordering::SeqCst);
            let flag = Arc::clone(shutdown);
            let stopped = Arc::clone(&self.stopped);
            let delay = self.stop_delay;
            let worker = Worker::spawn("osd-test", shutdown, move || {
                while !flag.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                std::thread::sleep(delay);
                stopped.fetch_add(1, Ordering::SeqCst);
            })
            .expect("the thread starts");
            Started {
                audio: self.audio.clone(),
                brightness: self.brightness.clone(),
                threads: vec![Box::new(worker)],
            }
        }

        fn on_reaped(&self) -> Box<dyn FnOnce() + Send> {
            let reaped = Arc::clone(&self.reaped);
            Box::new(move || {
                reaped.fetch_add(1, Ordering::SeqCst);
            })
        }
    }

    fn until(what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn turning_the_duty_on_while_the_old_workers_stop_never_blocks_the_loop() {
        let counting = Counting {
            stop_delay: Duration::from_millis(400),
            ..Counting::default()
        };
        let runtime = OsdRuntime::with(Box::new(counting.clone()), Box::new(NoTimer));
        runtime.apply(&settings(Duty::On));
        runtime.apply(&settings(Duty::Off));
        let asked = Instant::now();
        runtime.apply(&settings(Duty::On));
        assert!(
            asked.elapsed() < Duration::from_millis(200),
            "the loop waited for the reaper"
        );
        // Deferred: the old workers are still stopping, so no second set.
        assert!(!runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 1);
        runtime.resume();
        assert_eq!(counting.started.load(Ordering::SeqCst), 1);
        // The reaper says it is done (the loop hears it as a tick).
        until("the reaper", || counting.reaped.load(Ordering::SeqCst) == 1);
        assert_eq!(counting.stopped.load(Ordering::SeqCst), 1);
        runtime.resume();
        assert!(runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 2);
        // Turned off again before the reaper finished: nothing resumes.
        runtime.apply(&settings(Duty::Off));
        runtime.apply(&settings(Duty::On));
        runtime.apply(&settings(Duty::Off));
        until("the reaper", || counting.reaped.load(Ordering::SeqCst) == 2);
        runtime.resume();
        assert!(!runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 2);
    }

    /// A runtime whose brightness workers are only the returned queue, so a
    /// test reads what they were told. Every set of workers it starts shares
    /// that one queue.
    pub(in crate::osd) fn runtime_with_brightness_queue(
    ) -> (OsdRuntime, std::sync::mpsc::Receiver<BrightnessCommand>) {
        let (handle, commands) = crate::providers::brightness::test_handle();
        let counting = Counting {
            brightness: Some(handle),
            ..Counting::default()
        };
        (
            OsdRuntime::with(Box::new(counting), Box::new(NoTimer)),
            commands,
        )
    }

    /// What the brightness workers were told since the last look, in order.
    pub(in crate::osd) fn told(
        commands: &std::sync::mpsc::Receiver<BrightnessCommand>,
    ) -> Vec<BrightnessCommand> {
        std::iter::from_fn(|| commands.try_recv().ok()).collect()
    }

    /// The duty on and the `brightness` piece in the bar's end zone.
    fn piece_placed() -> Settings {
        control::test_settings(Duty::On, &["tray", "brightness"])
    }

    /// The duty on, without the `brightness` piece.
    fn piece_absent() -> Settings {
        control::test_settings(Duty::On, &["tray", "volume"])
    }

    #[test]
    fn a_start_with_the_piece_placed_and_the_duty_on_asks_the_ddc_worker_to_show_it() {
        // The shell's cold start (the defect of 1.13.0): the outputs were
        // announced before this runtime existed, so no output event reaches
        // it, and `main` calls `apply` once. That one call must be enough.
        let (runtime, commands) = runtime_with_brightness_queue();
        runtime.apply(&piece_placed());
        assert!(runtime.running());
        assert_eq!(told(&commands), [BrightnessCommand::Show]);
    }

    #[test]
    fn the_ddc_worker_hears_the_piece_once_when_it_is_placed_and_once_when_it_leaves() {
        let (runtime, commands) = runtime_with_brightness_queue();
        runtime.apply(&piece_placed());
        assert_eq!(told(&commands), [BrightnessCommand::Show]);
        // Every later settings write with the piece still placed: nothing.
        for _ in 0..10 {
            runtime.apply(&piece_placed());
        }
        assert_eq!(told(&commands), []);
        // An output is its own news, never a second `Show`.
        runtime.outputs_changed();
        assert_eq!(told(&commands), [BrightnessCommand::OutputsChanged]);
        // The piece leaves the layout: taken back once.
        runtime.apply(&piece_absent());
        runtime.apply(&piece_absent());
        assert_eq!(told(&commands), [BrightnessCommand::Hide]);
        // Placed again: a new consumer.
        runtime.apply(&piece_placed());
        assert_eq!(told(&commands), [BrightnessCommand::Show]);
        // The duty off: the workers that are about to stop hear the piece
        // leave (they share the test's queue with their successors).
        runtime.apply(&control::test_settings(Duty::Off, &["tray", "brightness"]));
        assert!(!runtime.running());
        assert_eq!(told(&commands), [BrightnessCommand::Hide]);
        // New workers hear it once more, and once only.
        runtime.stop();
        runtime.apply(&piece_placed());
        runtime.apply(&piece_placed());
        assert!(runtime.running());
        assert_eq!(told(&commands), [BrightnessCommand::Show]);
    }

    #[test]
    fn a_piece_placed_while_the_old_workers_stop_is_told_to_the_new_ones() {
        let (handle, commands) = crate::providers::brightness::test_handle();
        let counting = Counting {
            stop_delay: Duration::from_millis(400),
            brightness: Some(handle),
            ..Counting::default()
        };
        let runtime = OsdRuntime::with(Box::new(counting.clone()), Box::new(NoTimer));
        // Named layouts, never the default one: whether that places the
        // piece is not this test's to assume.
        runtime.apply(&piece_absent());
        runtime.apply(&control::test_settings(Duty::Off, &["tray", "volume"]));
        assert_eq!(told(&commands), []);
        // Deferred behind the reaper: there is no worker to tell yet.
        runtime.apply(&piece_placed());
        assert!(!runtime.running());
        assert_eq!(told(&commands), []);
        until("the reaper", || counting.reaped.load(Ordering::SeqCst) == 1);
        runtime.resume();
        assert!(runtime.running());
        assert_eq!(told(&commands), [BrightnessCommand::Show]);
    }

    #[test]
    fn the_duty_on_without_the_piece_tells_the_ddc_worker_nothing() {
        let (runtime, commands) = runtime_with_brightness_queue();
        runtime.apply(&piece_absent());
        runtime.apply(&piece_absent());
        assert!(runtime.running());
        // Neither `Show` nor, for a piece that was never told, `Hide`.
        assert_eq!(told(&commands), []);
        // And with the duty off there is no worker to tell.
        runtime.apply(&control::test_settings(Duty::Off, &["tray", "brightness"]));
        assert_eq!(told(&commands), []);
    }

    /// A runtime whose audio worker is only the returned queue, so a test
    /// reads what it was told.
    fn runtime_with_audio_queue() -> (OsdRuntime, std::sync::mpsc::Receiver<AudioCommand>) {
        let (provider, commands) = audio::test_handle();
        let counting = Counting {
            audio: Some(provider),
            ..Counting::default()
        };
        (
            OsdRuntime::with(Box::new(counting), Box::new(NoTimer)),
            commands,
        )
    }

    /// What the audio worker was told since the last look, in order.
    fn told_audio(commands: &std::sync::mpsc::Receiver<AudioCommand>) -> Vec<AudioCommand> {
        std::iter::from_fn(|| commands.try_recv().ok()).collect()
    }

    #[test]
    fn the_graph_gate_follows_the_volume_piece() {
        let placed = || control::test_settings(Duty::On, &["tray", "volume"]);
        let absent = || control::test_settings(Duty::On, &["tray", "brightness"]);
        let (runtime, commands) = runtime_with_audio_queue();
        // A start with the piece placed opens the gate once, and every later
        // settings write with the piece still placed says nothing.
        runtime.apply(&placed());
        assert!(runtime.running());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(true)]);
        for _ in 0..10 {
            runtime.apply(&placed());
        }
        assert_eq!(told_audio(&commands), []);
        // The piece leaves the layout: the gate closes, once.
        runtime.apply(&absent());
        runtime.apply(&absent());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(false)]);
        // Placed again: open again, once.
        runtime.apply(&placed());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(true)]);
        // The duty off: the worker about to stop hears the gate close.
        runtime.apply(&control::test_settings(Duty::Off, &["tray", "volume"]));
        assert!(!runtime.running());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(false)]);
        // New workers hear it once more, and once only.
        runtime.stop();
        runtime.apply(&placed());
        runtime.apply(&placed());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(true)]);
        // Without the piece, a start never opens it and never closes it.
        runtime.apply(&absent());
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(false)]);
        runtime.stop();
        runtime.apply(&absent());
        assert_eq!(told_audio(&commands), []);
    }

    #[test]
    fn a_refused_graph_gate_is_asked_again_when_a_volume_popup_opens() {
        let placed = || control::test_settings(Duty::On, &["tray", "volume"]);
        let (runtime, commands) = runtime_with_audio_queue();
        // Workers up without the piece, then a full queue when it is placed.
        runtime.apply(&control::test_settings(Duty::On, &["tray", "brightness"]));
        let Some(worker) = runtime.audio() else {
            panic!("the duty runs an audio worker");
        };
        for _ in 0..audio::COMMAND_QUEUE {
            assert!(worker.request(AudioCommand::ReadBoth).is_ok());
        }
        runtime.apply(&placed());
        assert_eq!(told_audio(&commands).len(), audio::COMMAND_QUEUE);
        // Nothing asks it again by itself; the popup's opening does, once.
        runtime.retry_graph_gate();
        assert_eq!(told_audio(&commands), [AudioCommand::Graph(true)]);
        runtime.retry_graph_gate();
        assert_eq!(told_audio(&commands), []);
    }

    struct NoTimer;

    impl TimerLoop for NoTimer {
        fn arm(&mut self, _: Instant, _: OsdTick) {}
    }

    fn settings(osd: Duty) -> Settings {
        Settings {
            duties: Duties {
                osd,
                ..Duties::default()
            },
            ..Settings::default()
        }
    }

    fn runtime() -> (OsdRuntime, Counting) {
        let counting = Counting::default();
        (
            OsdRuntime::with(Box::new(counting.clone()), Box::new(NoTimer)),
            counting,
        )
    }

    #[test]
    fn the_duty_off_starts_no_worker() {
        let (runtime, counting) = runtime();
        runtime.apply(&quiet_settings());
        runtime.apply(&settings(Duty::Off));
        assert!(!runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 0);
        assert!(runtime.audio().is_none() && runtime.brightness().is_none());
        // Nothing to tell a worker that does not exist.
        runtime.outputs_changed();
    }

    #[test]
    fn the_defaults_start_the_workers() {
        // Since the cutover (CEL-2-M) the duty is on unless the file says
        // otherwise.
        let (runtime, counting) = runtime();
        runtime.apply(&Settings::default());
        assert!(runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 1);
        runtime.stop();
        assert_eq!(counting.stopped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn turning_the_duty_on_and_off_live_starts_and_stops_the_workers_once() {
        let (runtime, counting) = runtime();
        runtime.apply(&settings(Duty::On));
        runtime.apply(&settings(Duty::On));
        assert!(runtime.running());
        assert_eq!(counting.started.load(Ordering::SeqCst), 1, "idempotent");

        runtime.apply(&settings(Duty::Off));
        assert!(!runtime.running());
        // On again once the reaper is done: the old workers have stopped
        // before the new ones start.
        until("the reaper", || counting.reaped.load(Ordering::SeqCst) == 1);
        runtime.apply(&settings(Duty::On));
        assert_eq!(counting.stopped.load(Ordering::SeqCst), 1);
        assert_eq!(counting.started.load(Ordering::SeqCst), 2);

        runtime.stop();
        assert_eq!(counting.stopped.load(Ordering::SeqCst), 2);
        assert!(!runtime.running());
    }

    #[test]
    fn the_duty_has_its_own_flag_and_dropping_the_runtime_stops_its_workers() {
        let (runtime, counting) = runtime();
        runtime.apply(&settings(Duty::On));
        drop(runtime);
        assert_eq!(counting.stopped.load(Ordering::SeqCst), 1);
    }
}
