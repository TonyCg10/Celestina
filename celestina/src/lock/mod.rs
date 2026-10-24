//! The shell's side of the session lock (CEL-2-H, design
//! `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`
//! §4.1 to §4.4, §4.9, §4.10).
//!
//! The lock itself is another process (`celestina-lock`); the policy that
//! decides when to start it, what its lines mean and when a suspend may go
//! ahead is `celestina_shell::lock::LockPolicy`, a state machine with no IO.
//! This module is what runs it:
//!
//! * [`LockHandle`] lives on the loop. It owns the policy, follows
//!   `duties.lock` live, feeds the policy every [`Event`] and carries out
//!   every [`Action`] it answers with, through the [`Effects`] seam.
//! * [`Live`] is the real [`Effects`]: the workers. [`supervisor`] runs the
//!   child, [`sleep`] holds logind's delay inhibitor and reports
//!   `PrepareForSleep`, [`tick`] is the policy's one-second timer,
//!   [`notify`] posts the notices, and one `logind` worker (the caffeine
//!   worker's code, tied to the policy's generation counter) is the only
//!   thing that suspends.
//! * [`bundle`] is the sealed memory file the child gets as stdin.
//!
//! **Duty off, nothing runs.** With `duties.lock` off there is no worker and
//! no inhibitor. Switching it off later stops the workers but never touches a
//! lock that is running: only PAM ends it, and it outlives the shell.
//!
//! **Runs.** Every start of the workers gets a run number; their events carry
//! it, and the handle drops an event whose run is not the current one, so a
//! worker that was just switched off cannot feed the policy of its successor.
//!
//! **No unlock.** Nothing in this module can unlock a session: there is no
//! verb, method or signal handler for it, and the only process that can end a
//! lock is the lock itself after PAM says yes.
#![forbid(unsafe_code)]

pub mod bundle;
pub mod notify;
pub mod sleep;
pub mod supervisor;
pub mod tick;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::lock::{
    Action, Event, GenerationReader, InhibitorFailure, Level, LockPolicy, LockedProof, LogReason,
    Notice, Origin,
};
use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::command::Verb;
use crate::energy::logind::{self, LogindBus, LogindConfig, LogindHandle};
use crate::energy::EnergyEvent;
use crate::message::Message;
use crate::providers::{Reaper, Worker};
use crate::wallpaper::thumbnail::ThumbnailStore;

use self::notify::{NoticeBus, Notifier};
use self::sleep::Sleep;
use self::supervisor::{Config, Supervisor, Suspender};
use self::tick::{Ticker, PERIOD};

/// What reaches the loop from the lock's side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockMessage {
    /// From the idle chain: its second step, or input coming back. It does
    /// not belong to a run of the workers.
    Idle(Event),
    /// From the workers of one run.
    Worker { run: u64, event: Event },
}

/// What the policy's actions turn into: the seam between the rules and the
/// system. Every method returns at once; nothing blocks the loop.
pub trait Effects {
    /// The duty turned on: start the workers of `run`.
    fn start(&self, run: u64);
    /// The duty turned off: stop the workers, off the loop. A running lock is
    /// not touched.
    fn stop(&self);
    /// Start a lock child after `after`. `false` if it was not queued.
    fn spawn_child(&self, prior_failures: u8, after: Duration) -> bool;
    /// Kill the child that never asked to lock.
    fn kill_child(&self);
    /// Take the delay inhibitor. `false` if the request was not queued.
    fn hold_inhibitor(&self) -> bool;
    /// Let the delay inhibitor go.
    fn release_inhibitor(&self);
    /// Hand a suspend proof on.
    fn suspend(&self, proof: LockedProof);
    /// Post a notice.
    fn notice(&self, notice: Notice, now: Instant);
    /// Tell the idle chain whether the lock is available.
    fn availability(&self, available: bool);
    /// Run (`true`) or park the policy's one-second timer.
    fn ticking(&self, on: bool);
    /// Wait for the workers that [`Effects::stop`] is stopping, and for the
    /// notice worker. The shell's exit calls this.
    fn join(&self);
}

struct Inner {
    policy: LockPolicy,
    effects: Box<dyn Effects>,
    clock: Box<dyn Fn() -> Instant>,
    /// The run number of the workers now alive (0 before the first start).
    run: u64,
    /// Whether the workers of [`Inner::run`] are alive.
    workers: bool,
    ticking: bool,
}

/// The lock's side of the shell, on the loop.
pub struct LockHandle {
    inner: RefCell<Inner>,
    /// The run number to give the next start.
    next_run: Cell<u64>,
}

impl std::fmt::Debug for LockHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.borrow();
        f.debug_struct("LockHandle")
            .field("state", &inner.policy.state())
            .field("workers", &inner.workers)
            .finish()
    }
}

impl LockHandle {
    /// A handle over the real system. `availability` is told, on the loop,
    /// whenever the lock becomes available or stops being so (the idle
    /// chain's second step follows it). `thumbnails` is where the wallpaper
    /// state keeps the pictures every lock child is given (CEL-2-H-b).
    /// Nothing runs until [`LockHandle::apply`] finds the duty on.
    #[must_use]
    pub fn new(
        sender: Sender<Message>,
        availability: Box<dyn Fn(bool)>,
        thumbnails: ThumbnailStore,
    ) -> Self {
        Self::with_effects(
            |generation| {
                Box::new(Live::production(
                    sender,
                    availability,
                    generation,
                    thumbnails,
                ))
            },
            Box::new(Instant::now),
        )
    }

    /// A handle over `effects`, built with the policy's generation reader,
    /// and a clock; for tests and for [`LockHandle::new`].
    #[must_use]
    pub fn with_effects(
        effects: impl FnOnce(GenerationReader) -> Box<dyn Effects>,
        clock: Box<dyn Fn() -> Instant>,
    ) -> Self {
        let policy = LockPolicy::new();
        let effects = effects(policy.generation_reader());
        Self {
            inner: RefCell::new(Inner {
                policy,
                effects,
                clock,
                run: 0,
                workers: false,
                ticking: false,
            }),
            next_run: Cell::new(1),
        }
    }

    /// Whether the idle chain's second step may be accepted.
    #[must_use]
    pub fn available(&self) -> bool {
        self.inner.borrow().policy.available()
    }

    /// Brings the workers to what `settings` ask for: the duty on has them,
    /// the duty off has none. Idempotent.
    pub fn apply(&self, settings: &Settings) {
        let on = settings.duties.lock == Duty::On;
        {
            let mut inner = self.inner.borrow_mut();
            if inner.workers == on {
                return;
            }
            if on {
                let run = self.next_run.get();
                self.next_run.set(run + 1);
                inner.run = run;
                inner.workers = true;
                inner.effects.start(run);
            }
        }
        self.feed(Event::Duty(on));
        if !on {
            // The workers that pinned the binaries are going: a later start
            // pins them again before the lock counts as available.
            self.feed(Event::Pinned(false));
            let mut inner = self.inner.borrow_mut();
            inner.workers = false;
            inner.ticking = false;
            inner.effects.stop();
        }
    }

    /// The shell is leaving: stops the workers and waits for them. A running
    /// lock is not touched.
    pub fn stop(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.workers {
            inner.workers = false;
            inner.ticking = false;
            inner.effects.stop();
        }
        inner.effects.join();
    }

    /// Takes in what a worker or the idle chain reported.
    pub fn message(&self, message: LockMessage) {
        match message {
            LockMessage::Idle(event) => self.feed(event),
            LockMessage::Worker { run, event } => {
                let current = {
                    let inner = self.inner.borrow();
                    inner.workers && inner.run == run
                };
                if current {
                    self.feed(event);
                } else {
                    tracing::debug!(run, ?event, "lock: an event of a stopped run was dropped");
                }
            }
        }
    }

    /// Runs a lock verb; `false` for any other verb.
    pub fn verb(&self, verb: &Verb) -> bool {
        match verb {
            Verb::Lock => self.feed(Event::Lock),
            Verb::LockAndSuspend => self.feed(Event::LockAndSuspend(Origin::Verb)),
            _ => return false,
        }
        true
    }

    /// Feeds `first` and everything it leads to into the policy and carries
    /// out the answers.
    fn feed(&self, first: Event) {
        let mut inner = self.inner.borrow_mut();
        let mut queue = VecDeque::from([first]);
        while let Some(event) = queue.pop_front() {
            let now = (inner.clock)();
            let actions = inner.policy.handle(now, event);
            for action in actions {
                inner.perform(action, now, &mut queue);
            }
        }
        let wanted = inner.policy.wants_tick();
        if inner.workers && wanted != inner.ticking {
            inner.ticking = wanted;
            inner.effects.ticking(wanted);
        }
    }
}

impl Inner {
    fn perform(&mut self, action: Action, now: Instant, queue: &mut VecDeque<Event>) {
        match action {
            Action::Spawn {
                prior_failures,
                after,
            } => {
                if !self.effects.spawn_child(prior_failures, after) {
                    queue.push_back(Event::SpawnFailed);
                }
            }
            Action::KillChild => self.effects.kill_child(),
            Action::HoldInhibitor => {
                if !self.effects.hold_inhibitor() {
                    queue.push_back(Event::InhibitorFailed(InhibitorFailure::Bus));
                }
            }
            Action::ReleaseInhibitor => self.effects.release_inhibitor(),
            Action::Suspend(proof) => self.effects.suspend(proof),
            Action::Notice(notice) => self.effects.notice(notice, now),
            Action::PublishAvailability(available) => self.effects.availability(available),
            Action::Log(reason) => log(reason),
        }
    }
}

/// Writes the policy's reason to the journal at the level the policy chose.
fn log(reason: LogReason) {
    let text = describe(reason);
    match reason.level() {
        Level::Info => tracing::info!("lock: {text}"),
        Level::Warn => tracing::warn!("lock: {text}"),
        Level::Error => tracing::error!("lock: {text}"),
    }
}

fn describe(reason: LogReason) -> String {
    match reason {
        LogReason::DutyOn => "the duty is on; turn off the other locker's idle lock (Noctalia) \
             and bind `celestina lock suspend` yourself"
            .to_owned(),
        LogReason::RequestRefused(why) => format!("the request was refused: {why:?}"),
        LogReason::IdleRequestRefused(why) => {
            format!("the idle chain's lock step was refused: {why:?}")
        }
        LogReason::IntentSet(origin) => format!("lock and suspend requested by {origin:?}"),
        LogReason::IntentExpired => "not suspended: the lock was not confirmed in time".to_owned(),
        LogReason::IntentCancelledByInput => {
            "input came back: the idle chain's lock and suspend was cancelled".to_owned()
        }
        LogReason::UnknownProtocol => "the lock speaks a protocol this shell does not".to_owned(),
        LogReason::UnexpectedLine => "a status line that does not fit the state".to_owned(),
        LogReason::StrayExit => "the lock process ended with nothing to account for it".to_owned(),
        LogReason::ExitBeforeRequest => "the lock process ended before it asked to lock".to_owned(),
        LogReason::SpawnFailed => "the lock process could not be started".to_owned(),
        LogReason::SpawnWatchdog => {
            "the lock process did not ask to lock within 5 s; it was killed".to_owned()
        }
        LogReason::RetryingAfterRefusal => {
            "the compositor refused the lock; asking again".to_owned()
        }
        LogReason::CompositorRefused => "the compositor refused the lock".to_owned(),
        LogReason::Relaunching => "the lock died; starting it again".to_owned(),
        LogReason::RelaunchLimit => {
            "the lock died too many times; not starting it again (use another console)".to_owned()
        }
        LogReason::FinishedAfterLocked => {
            "the compositor ended the lock by its own policy".to_owned()
        }
        LogReason::ConnectionLost => {
            "the lock lost its Wayland connection: the session it protected is gone".to_owned()
        }
        LogReason::RequestedTimeout => {
            "the lock asked to lock but was not confirmed within 10 s".to_owned()
        }
        LogReason::EndingTimeout => {
            "the lock did not exit within 5 s of unlocking; it unlocked by protocol".to_owned()
        }
        LogReason::SleepWhileUncovered => {
            "the machine is going to sleep and the lock is unavailable: the session sleeps \
             uncovered"
                .to_owned()
        }
        LogReason::SleptBeforeLocked => {
            "the machine slept before the lock was confirmed".to_owned()
        }
        LogReason::PamFileMissing => {
            "the PAM service celestina-lock is not installed: the lock will cover the screen but \
             cannot be unlocked (see celestina/README.md)"
                .to_owned()
        }
        LogReason::InhibitorFailed(failure) => {
            format!("the sleep inhibitor was not taken: {failure:?}")
        }
    }
}

/// A suspend through the lock's own logind worker.
struct LogindSuspender(LogindHandle);

impl Suspender for LogindSuspender {
    fn suspend(&self, proof: LockedProof) -> Result<(), String> {
        self.0.suspend(proof).map_err(|err| err.to_string())
    }
}

/// How the real workers are reached and where they connect.
pub struct Wiring {
    /// How a message reaches the shell's loop; `false` once the loop is gone.
    pub send: Arc<dyn Fn(Message) -> bool + Send + Sync>,
    /// Told whenever the lock becomes (un)available.
    pub availability: Box<dyn Fn(bool)>,
    /// The policy's generation counter, for the logind worker.
    pub generation: GenerationReader,
    /// How the lock is found and started; `Err` where it cannot be (this
    /// executable's own path is unknown), and then no worker looks for it.
    pub config: Result<Config, String>,
    /// The system bus: logind.
    pub system: LogindBus,
    /// The session bus: the notices.
    pub notices: NoticeBus,
}

/// The workers of one run.
struct Running {
    supervisor: Supervisor,
    sleep: Sleep,
    ticker: Ticker,
    suspender: Worker,
}

impl Running {
    fn stop(self) {
        let Running {
            supervisor,
            sleep,
            ticker,
            suspender,
        } = self;
        supervisor.stop();
        sleep.stop();
        ticker.stop();
        suspender.join_within(self::supervisor::JOIN_WAIT);
    }
}

/// The real effects: the workers.
pub struct Live {
    wiring: Wiring,
    running: RefCell<Option<Running>>,
    reaper: RefCell<Reaper>,
    notifier: Notifier,
}

impl Live {
    /// The production wiring: the system and session buses, the binaries
    /// beside this executable.
    fn production(
        sender: Sender<Message>,
        availability: Box<dyn Fn(bool)>,
        generation: GenerationReader,
        thumbnails: ThumbnailStore,
    ) -> Self {
        let send: Arc<dyn Fn(Message) -> bool + Send + Sync> =
            Arc::new(move |message| sender.send(message).is_ok());
        // Beside this executable, or nowhere: a lock found through the
        // working directory would be found through the environment.
        let config = Config::production(thumbnails)
            .map_err(|err| format!("this executable's path is unknown: {err}"));
        Self::new(Wiring {
            send,
            availability,
            generation,
            config,
            system: LogindBus::System,
            notices: NoticeBus::Session,
        })
    }

    /// Effects over `wiring`.
    #[must_use]
    pub fn new(wiring: Wiring) -> Self {
        let notifier = Notifier::new(wiring.notices.clone());
        Self {
            wiring,
            running: RefCell::new(None),
            reaper: RefCell::new(Reaper::default()),
            notifier,
        }
    }

    fn publisher(&self, run: u64) -> impl Fn(Event) -> bool + Send + 'static {
        let send = Arc::clone(&self.wiring.send);
        move |event| send(Message::Lock(LockMessage::Worker { run, event }))
    }

    fn start_workers(&self, run: u64) -> Result<Running, String> {
        let config = self.wiring.config.clone()?;
        let logind_flag = Arc::new(AtomicBool::new(false));
        let events = Arc::clone(&self.wiring.send);
        let (suspend_handle, suspender) = logind::spawn(
            LogindConfig {
                bus: self.wiring.system.clone(),
                generation: Some(self.wiring.generation.clone()),
            },
            move |event| events(Message::Energy(EnergyEvent::Logind(event))),
            &logind_flag,
        )
        .map_err(|err| format!("the logind worker did not start: {err}"))?;
        let supervisor = supervisor::spawn(
            config,
            self.publisher(run),
            Box::new(LogindSuspender(suspend_handle)),
            self.wiring.generation.clone(),
        )
        .map_err(|err| format!("the lock worker did not start: {err}"))?;
        let sleep = sleep::spawn(self.wiring.system.clone(), self.publisher(run))
            .map_err(|err| format!("the sleep worker did not start: {err}"))?;
        let ticker = Ticker::start(PERIOD, self.publisher(run))
            .map_err(|err| format!("the timer did not start: {err}"))?;
        Ok(Running {
            supervisor,
            sleep,
            ticker,
            suspender,
        })
    }
}

impl Effects for Live {
    fn start(&self, run: u64) {
        // The previous run's workers must be gone before another starts.
        self.reaper.borrow_mut().wait();
        match self.start_workers(run) {
            Ok(running) => *self.running.borrow_mut() = Some(running),
            Err(why) => tracing::warn!("lock: {why}"),
        }
    }

    fn stop(&self) {
        if let Some(running) = self.running.borrow_mut().take() {
            *self.reaper.borrow_mut() = Reaper::spawn("lock", move || running.stop());
        }
    }

    fn spawn_child(&self, prior_failures: u8, after: Duration) -> bool {
        self.running.borrow().as_ref().is_some_and(|running| {
            running
                .supervisor
                .handle()
                .spawn_child(prior_failures, after)
                .is_ok()
        })
    }

    fn kill_child(&self) {
        if let Some(running) = self.running.borrow().as_ref() {
            if let Err(err) = running.supervisor.handle().kill_child() {
                tracing::warn!(%err, "lock: the kill was not queued");
            }
        }
    }

    fn hold_inhibitor(&self) -> bool {
        self.running
            .borrow()
            .as_ref()
            .is_some_and(|running| running.sleep.hold().is_ok())
    }

    fn release_inhibitor(&self) {
        if let Some(running) = self.running.borrow().as_ref() {
            if let Err(err) = running.sleep.release() {
                tracing::warn!(%err, "lock: the release was not queued");
            }
        }
    }

    fn suspend(&self, proof: LockedProof) {
        let queued = self
            .running
            .borrow()
            .as_ref()
            .map(|running| running.supervisor.handle().suspend(proof));
        match queued {
            Some(Ok(())) => {}
            Some(Err(err)) => tracing::warn!(%err, "lock: the suspend was not queued"),
            None => tracing::warn!("lock: no lock worker; the suspend was dropped"),
        }
    }

    fn notice(&self, notice: Notice, now: Instant) {
        self.notifier.post(notice, now);
    }

    fn availability(&self, available: bool) {
        (self.wiring.availability)(available);
    }

    fn ticking(&self, on: bool) {
        if let Some(running) = self.running.borrow().as_ref() {
            running.ticker.set(on);
        }
    }

    fn join(&self) {
        self.reaper.borrow_mut().wait();
        self.notifier.stop();
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(running) = self.running.borrow_mut().take() {
            running.stop();
        }
        self.join();
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A policy driven to `Locked` through its public events, and the proof
    /// it hands out for a `lock suspend`.
    pub(crate) fn locked() -> (LockPolicy, LockedProof) {
        use celestina_lock_proto::StatusLine;
        let mut policy = LockPolicy::new();
        let now = Instant::now();
        policy.handle(now, Event::Duty(true));
        policy.handle(now, Event::Pinned(true));
        policy.handle(now, Event::Lock);
        policy.handle(now, Event::Line(StatusLine::Requested));
        policy.handle(now, Event::Line(StatusLine::Locked));
        let proof = policy
            .handle(now, Event::LockAndSuspend(Origin::Verb))
            .into_iter()
            .find_map(|action| match action {
                Action::Suspend(proof) => Some(proof),
                _ => None,
            })
            .expect("a locked policy mints a proof for a suspend");
        (policy, proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_lock_proto::{AttemptKind, StatusLine};
    use celestina_shell::lock::{ChildExit, Why};
    use std::sync::Mutex;

    /// Everything the effects were asked, in order.
    #[derive(Clone, Default)]
    struct Calls(Arc<Mutex<Vec<String>>>);

    impl Calls {
        fn say(&self, line: impl Into<String>) {
            self.0.lock().expect("calls").push(line.into());
        }

        fn all(&self) -> Vec<String> {
            self.0.lock().expect("calls").clone()
        }

        fn starting(&self, prefix: &str) -> Vec<String> {
            self.all()
                .into_iter()
                .filter(|line| line.starts_with(prefix))
                .collect()
        }
    }

    struct Fake {
        calls: Calls,
        refuse_spawn: bool,
        refuse_hold: bool,
    }

    impl Effects for Fake {
        fn start(&self, run: u64) {
            self.calls.say(format!("start {run}"));
        }
        fn stop(&self) {
            self.calls.say("stop");
        }
        fn spawn_child(&self, prior_failures: u8, after: Duration) -> bool {
            self.calls
                .say(format!("spawn {prior_failures} {}ms", after.as_millis()));
            !self.refuse_spawn
        }
        fn kill_child(&self) {
            self.calls.say("kill");
        }
        fn hold_inhibitor(&self) -> bool {
            self.calls.say("hold");
            !self.refuse_hold
        }
        fn release_inhibitor(&self) {
            self.calls.say("release");
        }
        fn suspend(&self, proof: LockedProof) {
            self.calls.say(format!("suspend {}", proof.generation()));
        }
        fn notice(&self, notice: Notice, _now: Instant) {
            self.calls.say(format!("notice {notice:?}"));
        }
        fn availability(&self, available: bool) {
            self.calls.say(format!("available {available}"));
        }
        fn ticking(&self, on: bool) {
            self.calls.say(format!("ticking {on}"));
        }
        fn join(&self) {
            self.calls.say("join");
        }
    }

    fn handle_with(refuse_spawn: bool, refuse_hold: bool) -> (LockHandle, Calls) {
        let calls = Calls::default();
        let fake = Fake {
            calls: calls.clone(),
            refuse_spawn,
            refuse_hold,
        };
        let handle = LockHandle::with_effects(|_| Box::new(fake), Box::new(Instant::now));
        (handle, calls)
    }

    fn handle() -> (LockHandle, Calls) {
        handle_with(false, false)
    }

    fn settings(lock: Duty) -> Settings {
        let mut settings = Settings::default();
        settings.duties.lock = lock;
        settings
    }

    fn worker(handle: &LockHandle, run: u64, event: Event) {
        handle.message(LockMessage::Worker { run, event });
    }

    /// The duty on and both binaries pinned, run 1.
    fn available(handle: &LockHandle) {
        handle.apply(&settings(Duty::On));
        worker(handle, 1, Event::Pinned(true));
    }

    fn locked(handle: &LockHandle) {
        handle.verb(&Verb::Lock);
        worker(handle, 1, Event::Line(StatusLine::Requested));
        worker(handle, 1, Event::Line(StatusLine::Locked));
    }

    #[test]
    fn the_duty_off_starts_nothing_and_the_duty_on_starts_the_workers_once() {
        let (handle, calls) = handle();
        handle.apply(&settings(Duty::Off));
        assert!(calls.all().is_empty(), "{:?}", calls.all());
        handle.apply(&settings(Duty::On));
        handle.apply(&settings(Duty::On));
        assert_eq!(calls.starting("start"), ["start 1"]);
        handle.apply(&settings(Duty::Off));
        handle.apply(&settings(Duty::Off));
        assert_eq!(calls.starting("stop"), ["stop"]);
        handle.apply(&settings(Duty::On));
        assert_eq!(calls.starting("start"), ["start 1", "start 2"]);
    }

    #[test]
    fn idle_step_2_is_accepted_only_when_the_policy_publishes_availability() {
        let (handle, calls) = handle();
        assert!(!handle.available());
        handle.apply(&settings(Duty::On));
        assert!(!handle.available(), "the binaries are not pinned yet");
        assert!(calls.starting("available").is_empty());
        worker(&handle, 1, Event::Pinned(true));
        assert!(handle.available());
        assert_eq!(calls.starting("available"), ["available true"]);
        // A binary that fails its check withdraws it.
        worker(&handle, 1, Event::Pinned(false));
        assert_eq!(
            calls.starting("available"),
            ["available true", "available false"]
        );
        // And the duty off keeps it withdrawn.
        worker(&handle, 1, Event::Pinned(true));
        handle.apply(&settings(Duty::Off));
        assert_eq!(
            calls.starting("available").last().unwrap(),
            "available false"
        );
    }

    #[test]
    fn a_missing_pam_file_posts_the_notice_and_availability_stays() {
        let (handle, calls) = handle();
        available(&handle);
        worker(&handle, 1, Event::PamFileMissing);
        assert_eq!(calls.starting("notice"), ["notice PamMissing"]);
        assert!(handle.available());
    }

    #[test]
    fn events_of_a_stopped_run_are_dropped() {
        let (handle, calls) = handle();
        available(&handle);
        handle.apply(&settings(Duty::Off));
        handle.apply(&settings(Duty::On));
        // Run 1's last word arrives after run 2 started.
        worker(&handle, 1, Event::Pinned(true));
        assert!(!handle.available(), "run 1 cannot feed run 2");
        worker(&handle, 2, Event::Pinned(true));
        assert!(handle.available());
        drop(calls);
    }

    #[test]
    fn idle_resumed_drops_only_idle_origin_intents() {
        let (handle, calls) = handle();
        available(&handle);
        // An idle-origin request, then input: the lock is not suspended.
        handle.message(LockMessage::Idle(Event::LockAndSuspend(Origin::Idle)));
        handle.message(LockMessage::Idle(Event::IdleResumed));
        worker(&handle, 1, Event::Line(StatusLine::Requested));
        worker(&handle, 1, Event::Line(StatusLine::Locked));
        assert!(calls.starting("suspend").is_empty(), "{:?}", calls.all());
        // A verb's request is not cancelled by input.
        let (handle, calls) = self::handle();
        available(&handle);
        handle.verb(&Verb::LockAndSuspend);
        handle.message(LockMessage::Idle(Event::IdleResumed));
        worker(&handle, 1, Event::Line(StatusLine::Requested));
        worker(&handle, 1, Event::Line(StatusLine::Locked));
        assert_eq!(calls.starting("suspend").len(), 1, "{:?}", calls.all());
    }

    #[test]
    fn refusals_of_explicit_requests_post_a_notice_and_idle_refusals_do_not() {
        let (handle, calls) = handle();
        // The duty is off: a verb is refused with a notice.
        handle.verb(&Verb::LockAndSuspend);
        handle.verb(&Verb::Lock);
        assert_eq!(
            calls.starting("notice"),
            ["notice Unavailable(DutyOff)", "notice Unavailable(DutyOff)"]
        );
        // The idle chain's step is refused in the log only.
        let (handle, calls) = self::handle();
        handle.message(LockMessage::Idle(Event::LockAndSuspend(Origin::Idle)));
        assert!(calls.starting("notice").is_empty(), "{:?}", calls.all());
        assert!(calls.starting("spawn").is_empty());
    }

    #[test]
    fn a_verb_that_cannot_start_the_child_posts_one_notice_and_goes_idle() {
        let (handle, calls) = handle_with(true, false);
        available(&handle);
        handle.verb(&Verb::LockAndSuspend);
        assert_eq!(calls.starting("spawn"), ["spawn 0 0ms"]);
        assert_eq!(calls.starting("notice"), ["notice NotLocked"]);
        // Idle again: a second request spawns again.
        handle.verb(&Verb::Lock);
        assert_eq!(calls.starting("spawn").len(), 2);
    }

    #[test]
    fn one_suspend_intent_at_a_time_and_the_proof_goes_out_once() {
        let (handle, calls) = handle();
        available(&handle);
        handle.verb(&Verb::LockAndSuspend);
        handle.verb(&Verb::LockAndSuspend);
        handle.message(LockMessage::Idle(Event::LockAndSuspend(Origin::Idle)));
        assert_eq!(calls.starting("spawn").len(), 1, "one child");
        worker(&handle, 1, Event::Line(StatusLine::Requested));
        worker(&handle, 1, Event::Line(StatusLine::Locked));
        assert_eq!(calls.starting("suspend").len(), 1, "{:?}", calls.all());
    }

    #[test]
    fn the_delay_inhibitor_follows_the_policy_and_a_refused_hold_is_a_failure() {
        let (handle, calls) = handle();
        available(&handle);
        assert_eq!(calls.starting("hold"), ["hold"]);
        locked(&handle);
        assert_eq!(calls.starting("release"), ["release"]);

        let (handle, calls) = handle_with(false, true);
        available(&handle);
        // The queue refused it: the policy is told and does not ask again
        // until something changes.
        assert_eq!(calls.starting("hold"), ["hold"]);
        worker(&handle, 1, Event::Pinned(true));
        assert_eq!(calls.starting("hold"), ["hold", "hold"]);
    }

    #[test]
    fn the_timer_runs_only_while_the_policy_wants_it() {
        let (handle, calls) = handle();
        available(&handle);
        assert!(calls.starting("ticking").is_empty());
        handle.verb(&Verb::Lock);
        assert_eq!(calls.starting("ticking"), ["ticking true"]);
        worker(&handle, 1, Event::Line(StatusLine::Requested));
        worker(&handle, 1, Event::Line(StatusLine::Locked));
        worker(&handle, 1, Event::Line(StatusLine::Unlocked));
        worker(
            &handle,
            1,
            Event::Exit(ChildExit::Status(celestina_lock_proto::LockExit::Unlocked)),
        );
        assert_eq!(
            calls.starting("ticking"),
            ["ticking true", "ticking false"],
            "{:?}",
            calls.all()
        );
    }

    #[test]
    fn a_shell_restarted_under_a_running_lock_is_idle_and_asks_the_compositor() {
        // A new shell knows nothing of a lock that runs: it is `Idle`, a
        // request spawns a child (which the compositor refuses with exit 10),
        // and no proof or suspend comes of it.
        let (handle, calls) = handle();
        available(&handle);
        handle.verb(&Verb::LockAndSuspend);
        assert_eq!(calls.starting("spawn"), ["spawn 0 0ms"]);
        for _ in 0..3 {
            worker(
                &handle,
                1,
                Event::Exit(ChildExit::Status(
                    celestina_lock_proto::LockExit::FinishedBeforeLocked,
                )),
            );
        }
        assert!(calls.starting("suspend").is_empty(), "{:?}", calls.all());
        assert!(
            calls
                .starting("notice")
                .contains(&"notice CompositorRefused".to_owned()),
            "{:?}",
            calls.all()
        );
        // The retries waited half a second each.
        assert_eq!(
            calls.starting("spawn"),
            ["spawn 0 0ms", "spawn 0 500ms", "spawn 0 500ms"]
        );
    }

    #[test]
    fn attempt_lines_carry_the_failures_to_the_relaunch() {
        let (handle, calls) = handle();
        available(&handle);
        locked(&handle);
        worker(
            &handle,
            1,
            Event::Line(StatusLine::Attempt(AttemptKind::Refused)),
        );
        worker(
            &handle,
            1,
            Event::Line(StatusLine::Attempt(AttemptKind::Refused)),
        );
        worker(&handle, 1, Event::Exit(ChildExit::Died));
        assert_eq!(calls.starting("spawn").last().unwrap(), "spawn 2 0ms");
    }

    #[test]
    fn why_texts_exist_for_every_reason() {
        for why in [Why::DutyOff, Why::NotPinned] {
            assert!(!describe(LogReason::RequestRefused(why)).is_empty());
        }
    }
}
