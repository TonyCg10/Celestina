//! Energy and comfort (CEL-2-B): the idle chain, caffeine, the forced night
//! light and the session verbs. Every duty is off by default (parity design
//! §4.1); each worker owns its own blocking IO.
//!
//! This module is the wiring. [`EnergyHandle`] holds the workers the settings
//! ask for and nothing else: a duty that is `on` has its worker, a duty that is
//! `off` has none, and [`EnergyHandle::apply`] moves from one to the other
//! whenever the shell's settings change, with no restart. Starting a worker is
//! behind the [`Spawner`] seam so the rules are tested without a Wayland
//! connection or a system bus.
//!
//! Each worker gets a shutdown flag of its own: [`crate::providers::Worker`]
//! raises the flag it was given when it is joined, and joining the idle worker
//! must not stop the shell.

pub mod idle;
pub mod logind;
pub mod nightlight;
pub mod verbs;
pub mod wire;

use std::cell::RefCell;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::idle::{IdleAction, IdleConfig};
use celestina_shell::session::PowerAction;
use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::niri::NiriRequest;
use crate::providers::Worker;

use self::idle::{IdleEvent, IdleHandle};
use self::logind::{Caffeine, LogindConfig, LogindEvent, LogindHandle, Outcome};
use self::nightlight::{NightLightEvent, NightLightHandle};

/// How long stopping one worker waits for it.
const JOIN_WAIT: Duration = Duration::from_secs(2);

/// What the energy workers report to the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnergyEvent {
    NightLight(NightLightEvent),
    Idle(IdleEvent),
    Logind(LogindEvent),
}

impl EnergyEvent {
    /// Writes the event to the journal. The shell shows nothing for these yet;
    /// a refusal or a failure is the line a person reads to learn why a key
    /// did nothing.
    pub fn log(&self) {
        match self {
            Self::NightLight(NightLightEvent::Applied) => {
                tracing::info!("night light: applied");
            }
            Self::NightLight(NightLightEvent::NotApplied(why)) => {
                tracing::warn!("night light: not applied: {why}");
            }
            Self::NightLight(NightLightEvent::Off) => tracing::info!("night light: off"),
            Self::Idle(IdleEvent::Armed) => tracing::info!("idle: armed"),
            Self::Idle(IdleEvent::Unavailable(why)) => {
                tracing::warn!("idle: unavailable: {why}");
            }
            Self::Idle(IdleEvent::Refused(why)) => tracing::warn!("idle: refused: {why}"),
            Self::Logind(LogindEvent::Caffeine(Caffeine::Held)) => {
                tracing::info!("caffeine: held");
            }
            Self::Logind(LogindEvent::Caffeine(Caffeine::Released)) => {
                tracing::info!("caffeine: released");
            }
            Self::Logind(LogindEvent::Caffeine(Caffeine::Unavailable(why))) => {
                tracing::warn!("caffeine: unavailable: {why}");
            }
            Self::Logind(LogindEvent::Power(action, Outcome::Done)) => {
                tracing::info!("session {action:?}: logind accepted it");
            }
            Self::Logind(LogindEvent::Power(action, Outcome::Refused(why))) => {
                tracing::warn!("session {action:?}: refused: {why}");
            }
            Self::Logind(LogindEvent::Power(action, Outcome::Unavailable(why))) => {
                tracing::warn!("session {action:?}: unavailable: {why}");
            }
            Self::Logind(LogindEvent::Power(action, Outcome::Failed(why))) => {
                tracing::warn!("session {action:?}: failed: {why}");
            }
        }
    }
}

/// The loop's side of the idle worker, as the wiring needs it.
pub trait IdleControl: Send {
    /// # Errors
    ///
    /// Why the worker did not take the command.
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`IdleControl::set_enabled`].
    fn set_caffeine(&self, caffeine: bool) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`IdleControl::set_enabled`].
    fn set_config(&self, config: IdleConfig) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`IdleControl::set_enabled`].
    fn set_locker_available(&self, available: bool) -> Result<(), String>;
}

impl IdleControl for IdleHandle {
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        IdleHandle::set_enabled(self, enabled).map_err(|e| e.to_string())
    }
    fn set_caffeine(&self, caffeine: bool) -> Result<(), String> {
        IdleHandle::set_caffeine(self, caffeine).map_err(|e| e.to_string())
    }
    fn set_config(&self, config: IdleConfig) -> Result<(), String> {
        IdleHandle::set_config(self, config).map_err(|e| e.to_string())
    }
    fn set_locker_available(&self, available: bool) -> Result<(), String> {
        IdleHandle::set_locker_available(self, available).map_err(|e| e.to_string())
    }
}

/// The loop's side of the night-light worker.
pub trait NightLightControl: Send {
    /// `Some(kelvin)` forces that warmth, `None` restores the outputs.
    ///
    /// # Errors
    ///
    /// Why the worker did not take the command.
    fn set(&self, kelvin: Option<u32>) -> Result<(), String>;
}

impl NightLightControl for NightLightHandle {
    fn set(&self, kelvin: Option<u32>) -> Result<(), String> {
        NightLightHandle::set(self, kelvin).map_err(|e| e.to_string())
    }
}

/// The loop's side of the logind worker.
pub trait LogindControl: Send {
    /// # Errors
    ///
    /// Why the worker did not take the request.
    fn caffeine(&self, held: bool) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`LogindControl::caffeine`].
    fn power(&self, action: PowerAction) -> Result<(), String>;
}

impl LogindControl for LogindHandle {
    fn caffeine(&self, held: bool) -> Result<(), String> {
        LogindHandle::caffeine(self, held).map_err(|e| e.to_string())
    }
    fn power(&self, action: PowerAction) -> Result<(), String> {
        LogindHandle::power(self, action).map_err(|e| e.to_string())
    }
}

/// How workers come to exist: the seam between the rules and the system.
pub trait Spawner {
    /// # Errors
    ///
    /// The failure to start the thread.
    fn idle(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn IdleControl>, Worker)>;
    /// # Errors
    ///
    /// The failure to start the thread.
    fn night_light(
        &self,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn NightLightControl>, Worker)>;
    /// # Errors
    ///
    /// The failure to start the thread.
    fn logind(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn LogindControl>, Worker)>;
    /// Asks the compositor for `request` without waiting.
    ///
    /// # Errors
    ///
    /// Why it was not queued: no compositor, a full queue, a stopped worker.
    fn compositor(&self, request: NiriRequest) -> Result<(), String>;
}

/// The compositor's request queue, when there is one.
pub type CompositorQueue = Option<SyncSender<NiriRequest>>;

/// Queues `request` on `queue` without blocking.
fn queue_request(queue: &CompositorQueue, request: NiriRequest) -> Result<(), String> {
    match queue {
        None => Err("no compositor connection".to_owned()),
        Some(queue) => queue
            .try_send(request)
            .map_err(|_| "the compositor's request queue is full or stopped".to_owned()),
    }
}

/// What the idle worker does for each action of the chain.
fn perform_idle(queue: &CompositorQueue, action: IdleAction) {
    match action {
        IdleAction::PowerOffMonitors => {
            if let Err(why) = queue_request(queue, NiriRequest::PowerOffMonitors) {
                tracing::warn!("idle: the outputs were not powered off: {why}");
            }
        }
        IdleAction::LockAndSuspend => {
            // The chain refuses this step while no locker is available, and
            // none is until CEL-2-H, so it never arrives; if it did, nothing
            // here may suspend an unlocked session.
            tracing::warn!("idle: lock and suspend has no locker; nothing was done");
        }
        // The worker publishes the refusal as `IdleEvent::Refused`.
        IdleAction::Refused(_) => {}
    }
}

/// The real spawner: Wayland and the system bus.
struct Live {
    sender: Sender<Message>,
    compositor: CompositorQueue,
}

impl Spawner for Live {
    fn idle(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn IdleControl>, Worker)> {
        let sender = self.sender.clone();
        let queue = self.compositor.clone();
        let (handle, worker) = idle::spawn(
            move |event| {
                sender
                    .send(Message::Energy(EnergyEvent::Idle(event)))
                    .is_ok()
            },
            move |action| perform_idle(&queue, action),
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }

    fn night_light(
        &self,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn NightLightControl>, Worker)> {
        let sender = self.sender.clone();
        let (handle, worker) = nightlight::spawn(
            move |event| {
                sender
                    .send(Message::Energy(EnergyEvent::NightLight(event)))
                    .is_ok()
            },
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }

    fn logind(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn LogindControl>, Worker)> {
        let sender = self.sender.clone();
        let (handle, worker) = logind::spawn(
            LogindConfig::default(),
            move |event| {
                sender
                    .send(Message::Energy(EnergyEvent::Logind(event)))
                    .is_ok()
            },
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }

    fn compositor(&self, request: NiriRequest) -> Result<(), String> {
        queue_request(&self.compositor, request)
    }
}

/// A running worker.
struct Slot<C: ?Sized> {
    control: Box<C>,
    worker: Worker,
}

impl<C: ?Sized> Slot<C> {
    /// Joins the worker on this thread, which releases what it held. The exit
    /// path uses this one, so gamma is restored before the process ends.
    fn stop(self) {
        let Slot { control, worker } = self;
        // Dropping the control disconnects the queue, so the worker also ends
        // by itself; the flag is raised by the join.
        drop(control);
        worker.join_within(JOIN_WAIT);
    }
}

impl<C: ?Sized + Send + 'static> Slot<C> {
    /// Hands the slot to a short-lived named thread that joins it within
    /// [`JOIN_WAIT`] and logs the outcome, so the loop that turned a duty off
    /// never blocks on a worker. The returned [`Reaper`] must be waited for
    /// before the same duty starts again (see [`Reaper::wait`]).
    fn reap(self, what: &'static str) -> Reaper {
        let spawned = std::thread::Builder::new()
            .name(format!("reap-{what}"))
            .spawn(move || self.stop());
        match spawned {
            Ok(handle) => Reaper(Some(handle)),
            Err(err) => {
                // The closure, and the slot in it, was dropped: the worker's
                // `Drop` has joined it on this thread.
                tracing::warn!(%err, "no thread to reap the {what} worker; it was joined here");
                Reaper(None)
            }
        }
    }
}

/// The thread that is joining a worker whose duty was turned off.
#[derive(Default)]
struct Reaper(Option<std::thread::JoinHandle<()>>);

impl Reaper {
    /// Waits for the pending reap, if any. A duty that is switched off and on
    /// again while its old worker is still stopping would otherwise hold two
    /// gamma clients, or two idle notification sets, at once. The wait is
    /// bounded: the reaper itself gives up on its worker after [`JOIN_WAIT`],
    /// and a worker left behind that way is logged and is the one overlap this
    /// allows (a body blocked in a read without a timeout).
    fn wait(&mut self) {
        if let Some(handle) = self.0.take() {
            if handle.join().is_err() {
                tracing::error!("a worker reaper panicked");
            }
        }
    }
}

/// What has been successfully handed to the idle worker.
///
/// A command is recorded only when the worker took it: a refusal (a full
/// queue, a worker that is gone) leaves it unrecorded, so the next
/// [`EnergyHandle::apply`] sends it again.
#[derive(Default)]
struct IdleSent {
    config: bool,
    locker: bool,
    caffeine: Option<bool>,
    enabled: bool,
}

struct IdleSlot {
    slot: Slot<dyn IdleControl>,
    sent: IdleSent,
}

impl IdleSlot {
    /// Sends what the worker has not taken yet, in the order that starts the
    /// chain clean (configuration, locker, caffeine, then enabled), and stops
    /// at the first refusal: the rest waits for the next call, so a queue that
    /// holds fewer commands than there are to send loses none.
    fn sync(&mut self, caffeine: bool) {
        let control = &*self.slot.control;
        let sent = &mut self.sent;
        if !sent.config {
            if !note("idle", control.set_config(IdleConfig::default())) {
                return;
            }
            sent.config = true;
        }
        if !sent.locker {
            if !note("idle", control.set_locker_available(false)) {
                return;
            }
            sent.locker = true;
        }
        if sent.caffeine != Some(caffeine) {
            if !note("idle", control.set_caffeine(caffeine)) {
                return;
            }
            sent.caffeine = Some(caffeine);
        }
        if !sent.enabled && note("idle", control.set_enabled(true)) {
            sent.enabled = true;
        }
    }
}

struct NightSlot {
    slot: Slot<dyn NightLightControl>,
    /// The warmth the worker last took: `None` when it took nothing yet.
    sent: Option<Option<u32>>,
}

struct LogindSlot {
    slot: Slot<dyn LogindControl>,
    /// Whether the worker last took "hold the inhibitor".
    held: bool,
    /// Whether a session verb used this worker, so switching caffeine off must
    /// not take it away from the verbs.
    session_used: bool,
}

/// Which workers exist right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Running {
    pub idle: bool,
    pub night_light: bool,
    pub logind: bool,
}

struct Inner {
    spawner: Box<dyn Spawner>,
    idle: Option<IdleSlot>,
    night: Option<NightSlot>,
    logind: Option<LogindSlot>,
    idle_reaper: Reaper,
    night_reaper: Reaper,
    logind_reaper: Reaper,
}

/// The energy duties of the shell: the workers the settings ask for.
pub struct EnergyHandle {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for EnergyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnergyHandle")
            .field("running", &self.running())
            .finish()
    }
}

impl EnergyHandle {
    /// The handle over the real system: workers publish into `sender`, and the
    /// compositor's actions go through `compositor`, the niri request queue.
    /// Nothing runs until [`EnergyHandle::apply`] finds a duty on.
    #[must_use]
    pub fn new(sender: Sender<Message>, compositor: CompositorQueue) -> Self {
        Self::with_spawner(Box::new(Live { sender, compositor }))
    }

    /// The handle over `spawner`, for tests.
    #[must_use]
    pub fn with_spawner(spawner: Box<dyn Spawner>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                idle: None,
                night: None,
                logind: None,
                idle_reaper: Reaper::default(),
                night_reaper: Reaper::default(),
                logind_reaper: Reaper::default(),
            }),
        }
    }

    /// Which workers exist.
    #[must_use]
    pub fn running(&self) -> Running {
        let inner = self.inner.borrow();
        Running {
            idle: inner.idle.is_some(),
            night_light: inner.night.is_some(),
            logind: inner.logind.is_some(),
        }
    }

    /// Brings the workers to what `settings` ask for: a duty that is `on` has
    /// its worker, told the person's switches; one that is `off` has none (its
    /// worker is joined by a reaper thread, never on this one).
    ///
    /// Idempotent, so it is called at start and after every settings change.
    /// What a worker refused (a full queue) is not recorded as sent, so the
    /// next call sends it again.
    pub fn apply(&self, settings: &Settings) {
        let mut inner = self.inner.borrow_mut();
        let caffeine_on = settings.duties.caffeine == Duty::On;
        let caffeine_wanted = caffeine_on && settings.energy.caffeine;
        inner.apply_idle(settings.duties.idle == Duty::On, caffeine_wanted);
        inner.apply_night_light(
            settings.duties.nightlight == Duty::On,
            settings
                .energy
                .night_light
                .then_some(settings.energy.night_light_kelvin),
        );
        inner.apply_caffeine(caffeine_on, caffeine_wanted);
    }

    /// Asks for a session action. Log out goes to the compositor; the rest to
    /// logind, whose worker is started for it when no duty has done so.
    ///
    /// # Errors
    ///
    /// Why the request was not queued. How it ends is reported later as an
    /// [`EnergyEvent`].
    pub fn power(&self, action: PowerAction) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();
        if action == PowerAction::LogOut {
            return inner.spawner.compositor(NiriRequest::Quit);
        }
        if inner.logind.is_none() {
            inner.start_logind()?;
        }
        let Some(logind) = inner.logind.as_mut() else {
            return Err("logind is not running".to_owned());
        };
        logind.session_used = true;
        logind.slot.control.power(action)
    }

    /// Stops every worker, releasing gamma, the inhibitor and the idle
    /// notifications, and waits for them (and for any reaper still joining
    /// one): the process must not end with gamma still set. Also what dropping
    /// the handle does.
    pub fn stop(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(idle) = inner.idle.take() {
            let _ = idle.slot.control.set_enabled(false);
            idle.slot.stop();
        }
        if let Some(night) = inner.night.take() {
            let _ = night.slot.control.set(None);
            night.slot.stop();
        }
        if let Some(logind) = inner.logind.take() {
            let _ = logind.slot.control.caffeine(false);
            logind.slot.stop();
        }
        inner.idle_reaper.wait();
        inner.night_reaper.wait();
        inner.logind_reaper.wait();
    }
}

impl Drop for EnergyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Logs a command the worker did not take. Returns whether it did.
fn note(what: &str, result: Result<(), String>) -> bool {
    match result {
        Ok(()) => true,
        Err(why) => {
            tracing::warn!("{what}: the worker did not take the command: {why}");
            false
        }
    }
}

impl Inner {
    fn apply_idle(&mut self, on: bool, caffeine: bool) {
        match (on, self.idle.is_some()) {
            (true, false) => {
                // The previous idle worker, if its duty was just turned off,
                // must be gone before another connects.
                self.idle_reaper.wait();
                match self.spawner.idle(&new_flag()) {
                    Ok((control, worker)) => {
                        let mut idle = IdleSlot {
                            slot: Slot { control, worker },
                            sent: IdleSent::default(),
                        };
                        idle.sync(caffeine);
                        self.idle = Some(idle);
                        tracing::info!("idle duty on");
                    }
                    Err(err) => tracing::warn!("idle: the worker did not start: {err}"),
                }
            }
            (true, true) => {
                if let Some(idle) = self.idle.as_mut() {
                    idle.sync(caffeine);
                }
            }
            (false, true) => {
                if let Some(idle) = self.idle.take() {
                    note("idle", idle.slot.control.set_enabled(false));
                    self.idle_reaper = idle.slot.reap("idle");
                    tracing::info!("idle duty off");
                }
            }
            (false, false) => {}
        }
    }

    fn apply_night_light(&mut self, on: bool, kelvin: Option<u32>) {
        match (on, self.night.is_some()) {
            (true, false) => {
                // Two gamma clients on one output would fight over the ramps.
                self.night_reaper.wait();
                match self.spawner.night_light(&new_flag()) {
                    Ok((control, worker)) => {
                        let mut night = NightSlot {
                            slot: Slot { control, worker },
                            sent: None,
                        };
                        night.sync(kelvin);
                        self.night = Some(night);
                        tracing::info!("night-light duty on");
                    }
                    Err(err) => tracing::warn!("night light: the worker did not start: {err}"),
                }
            }
            (true, true) => {
                if let Some(night) = self.night.as_mut() {
                    night.sync(kelvin);
                }
            }
            (false, true) => {
                if let Some(night) = self.night.take() {
                    note("night light", night.slot.control.set(None));
                    self.night_reaper = night.slot.reap("night-light");
                    tracing::info!("night-light duty off");
                }
            }
            (false, false) => {}
        }
    }

    fn apply_caffeine(&mut self, duty_on: bool, held: bool) {
        if duty_on {
            if self.logind.is_none() && held {
                if let Err(why) = self.start_logind() {
                    tracing::warn!("caffeine: {why}");
                    return;
                }
            }
            if let Some(logind) = self.logind.as_mut() {
                if logind.held != held && note("caffeine", logind.slot.control.caffeine(held)) {
                    logind.held = held;
                }
            }
        } else if let Some(mut logind) = self.logind.take() {
            if logind.held && note("caffeine", logind.slot.control.caffeine(false)) {
                logind.held = false;
            }
            if logind.session_used {
                // A session verb started it; the verbs keep it. A release it
                // refused stays recorded as held, so the next call retries.
                self.logind = Some(logind);
            } else {
                // Stopping the worker drops the inhibitor with it.
                self.logind_reaper = logind.slot.reap("logind");
            }
        }
    }

    fn start_logind(&mut self) -> Result<(), String> {
        self.logind_reaper.wait();
        let (control, worker) = self
            .spawner
            .logind(&new_flag())
            .map_err(|err| format!("the logind worker did not start: {err}"))?;
        self.logind = Some(LogindSlot {
            slot: Slot { control, worker },
            held: false,
            session_used: false,
        });
        Ok(())
    }
}

impl NightSlot {
    /// Sends `kelvin` unless the worker already took exactly that.
    fn sync(&mut self, kelvin: Option<u32>) {
        if self.sent != Some(kelvin) && note("night light", self.slot.control.set(kelvin)) {
            self.sent = Some(kelvin);
        }
    }
}

/// A shutdown flag for one worker.
fn new_flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use celestina_shell::settings::{Duties, Energy};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::sync_channel;
    use std::sync::Mutex;

    pub(crate) type Log = Arc<Mutex<Vec<String>>>;

    pub(crate) fn said(log: &Log) -> Vec<String> {
        log.lock().expect("log").clone()
    }

    /// A worker that waits for its flag, then for `release`, and records that
    /// it stopped.
    fn fake_worker(
        name: &'static str,
        shutdown: &Arc<AtomicBool>,
        log: &Log,
        release: &Arc<AtomicBool>,
    ) -> Worker {
        let flag = Arc::clone(shutdown);
        let log = Arc::clone(log);
        let release = Arc::clone(release);
        Worker::spawn(name, shutdown, move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            while !release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(2));
            }
            log.lock().expect("log").push(format!("{name} stopped"));
        })
        .expect("a thread")
    }

    /// Command lines (as logged, with the worker's name) the fake refuses once.
    pub(crate) type Busy = Arc<Mutex<Vec<String>>>;

    struct FakeControl {
        name: &'static str,
        log: Log,
        busy: Busy,
    }

    impl FakeControl {
        fn say(&self, line: String) -> Result<(), String> {
            let line = format!("{} {line}", self.name);
            let mut busy = self.busy.lock().expect("busy");
            if let Some(at) = busy.iter().position(|b| *b == line) {
                busy.remove(at);
                self.log.lock().expect("log").push(format!("{line} (busy)"));
                return Err("the command queue is full".to_owned());
            }
            self.log.lock().expect("log").push(line);
            Ok(())
        }
    }

    impl IdleControl for FakeControl {
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.say(format!("enabled {enabled}"))
        }
        fn set_caffeine(&self, caffeine: bool) -> Result<(), String> {
            self.say(format!("caffeine {caffeine}"))
        }
        fn set_config(&self, config: IdleConfig) -> Result<(), String> {
            assert_eq!(config, IdleConfig::default());
            self.say("config default".to_owned())
        }
        fn set_locker_available(&self, available: bool) -> Result<(), String> {
            self.say(format!("locker {available}"))
        }
    }

    impl NightLightControl for FakeControl {
        fn set(&self, kelvin: Option<u32>) -> Result<(), String> {
            self.say(format!("set {kelvin:?}"))
        }
    }

    impl LogindControl for FakeControl {
        fn caffeine(&self, held: bool) -> Result<(), String> {
            self.say(format!("caffeine {held}"))
        }
        fn power(&self, action: PowerAction) -> Result<(), String> {
            self.say(format!("power {action:?}"))
        }
    }

    pub(crate) struct Fake {
        log: Log,
        compositor: CompositorQueue,
        busy: Busy,
        release: Arc<AtomicBool>,
    }

    impl Fake {
        pub(crate) fn handle() -> (EnergyHandle, Log) {
            let (handle, log, _, _) = Fake::handle_with_controls();
            (handle, log)
        }

        /// The handle with the lines to refuse once, and the gate that holds
        /// every fake worker after its flag is raised while it is `false`.
        fn handle_with_controls() -> (EnergyHandle, Log, Busy, Arc<AtomicBool>) {
            let log = Log::default();
            let busy = Busy::default();
            let release = Arc::new(AtomicBool::new(true));
            let spawner = Fake {
                log: Arc::clone(&log),
                compositor: None,
                busy: Arc::clone(&busy),
                release: Arc::clone(&release),
            };
            (
                EnergyHandle::with_spawner(Box::new(spawner)),
                log,
                busy,
                release,
            )
        }

        fn pair(&self, name: &'static str, shutdown: &Arc<AtomicBool>) -> (FakeControl, Worker) {
            self.log
                .lock()
                .expect("log")
                .push(format!("{name} spawned"));
            (
                FakeControl {
                    name,
                    log: Arc::clone(&self.log),
                    busy: Arc::clone(&self.busy),
                },
                fake_worker(name, shutdown, &self.log, &self.release),
            )
        }
    }

    impl Spawner for Fake {
        fn idle(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(Box<dyn IdleControl>, Worker)> {
            let (control, worker) = self.pair("idle", shutdown);
            Ok((Box::new(control), worker))
        }
        fn night_light(
            &self,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn NightLightControl>, Worker)> {
            let (control, worker) = self.pair("night", shutdown);
            Ok((Box::new(control), worker))
        }
        fn logind(
            &self,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn LogindControl>, Worker)> {
            let (control, worker) = self.pair("logind", shutdown);
            Ok((Box::new(control), worker))
        }
        fn compositor(&self, request: NiriRequest) -> Result<(), String> {
            queue_request(&self.compositor, request)
        }
    }

    fn settings(duties: Duties, energy: Energy) -> Settings {
        Settings {
            duties,
            energy,
            ..Settings::default()
        }
    }

    const ON: Duty = Duty::On;
    const OFF: Duty = Duty::Off;

    #[test]
    fn default_settings_start_nothing() {
        let (handle, log) = Fake::handle();
        handle.apply(&Settings::default());
        assert_eq!(handle.running(), Running::default());
        // Switches without their duty do nothing either.
        handle.apply(&settings(
            Duties::default(),
            Energy {
                caffeine: true,
                night_light: true,
                ..Energy::default()
            },
        ));
        assert_eq!(handle.running(), Running::default());
        assert!(said(&log).is_empty(), "{:?}", said(&log));
    }

    #[test]
    fn a_duty_on_starts_exactly_its_worker() {
        for (duties, expected) in [
            (
                Duties {
                    idle: ON,
                    ..Duties::default()
                },
                Running {
                    idle: true,
                    ..Running::default()
                },
            ),
            (
                Duties {
                    nightlight: ON,
                    ..Duties::default()
                },
                Running {
                    night_light: true,
                    ..Running::default()
                },
            ),
        ] {
            let (handle, _log) = Fake::handle();
            handle.apply(&settings(duties, Energy::default()));
            assert_eq!(handle.running(), expected);
        }
    }

    #[test]
    fn the_caffeine_duty_starts_logind_only_to_hold() {
        let (handle, log) = Fake::handle();
        let duties = Duties {
            caffeine: ON,
            ..Duties::default()
        };
        // Duty on, switch off: nothing is held, so nothing is started.
        handle.apply(&settings(duties, Energy::default()));
        assert_eq!(handle.running(), Running::default());
        handle.apply(&settings(
            duties,
            Energy {
                caffeine: true,
                ..Energy::default()
            },
        ));
        assert_eq!(
            handle.running(),
            Running {
                logind: true,
                ..Running::default()
            }
        );
        assert_eq!(said(&log), ["logind spawned", "logind caffeine true"]);
    }

    #[test]
    fn the_idle_chain_follows_caffeine_and_starts_clean() {
        let (handle, log) = Fake::handle();
        let duties = Duties {
            idle: ON,
            caffeine: ON,
            ..Duties::default()
        };
        let caffeine = Energy {
            caffeine: true,
            ..Energy::default()
        };
        handle.apply(&settings(duties, caffeine));
        assert_eq!(
            said(&log)
                .iter()
                .filter(|l| l.starts_with("idle "))
                .cloned()
                .collect::<Vec<_>>(),
            [
                "idle spawned",
                "idle config default",
                "idle locker false",
                "idle caffeine true",
                "idle enabled true"
            ]
        );
        log.lock().expect("log").clear();
        handle.apply(&settings(duties, Energy::default()));
        assert!(said(&log).contains(&"idle caffeine false".to_owned()));
        assert!(said(&log).contains(&"logind caffeine false".to_owned()));
        // The same settings again say nothing.
        log.lock().expect("log").clear();
        handle.apply(&settings(duties, Energy::default()));
        assert!(said(&log).is_empty());
        // Caffeine is only honoured while its duty is on.
        handle.apply(&settings(
            Duties {
                idle: ON,
                ..Duties::default()
            },
            caffeine,
        ));
        assert!(!said(&log).contains(&"idle caffeine true".to_owned()));
    }

    #[test]
    fn the_night_light_follows_the_switch_and_the_kelvin() {
        let (handle, log) = Fake::handle();
        let duties = Duties {
            nightlight: ON,
            ..Duties::default()
        };
        handle.apply(&settings(duties, Energy::default()));
        handle.apply(&settings(
            duties,
            Energy {
                night_light: true,
                ..Energy::default()
            },
        ));
        handle.apply(&settings(
            duties,
            Energy {
                night_light: true,
                night_light_kelvin: 3300,
                ..Energy::default()
            },
        ));
        handle.apply(&settings(duties, Energy::default()));
        assert_eq!(
            said(&log),
            [
                "night spawned",
                "night set None",
                "night set Some(2700)",
                "night set Some(3300)",
                "night set None"
            ]
        );
    }

    #[test]
    fn duties_follow_the_settings_on_and_off_and_the_reaper_joins() {
        let (handle, log) = Fake::handle();
        let all = Duties {
            idle: ON,
            caffeine: ON,
            nightlight: ON,
            ..Duties::default()
        };
        let energy = Energy {
            caffeine: true,
            night_light: true,
            ..Energy::default()
        };
        handle.apply(&settings(all, energy));
        assert_eq!(
            handle.running(),
            Running {
                idle: true,
                night_light: true,
                logind: true
            }
        );
        log.lock().expect("log").clear();
        handle.apply(&settings(Duties { idle: OFF, ..all }, energy));
        assert_eq!(
            handle.running(),
            Running {
                idle: false,
                night_light: true,
                logind: true
            }
        );
        // The idle worker is joined by a reaper, not by `apply`; dropping the
        // handle waits for it (the exit path), so its line is there then.
        assert!(said(&log).contains(&"idle enabled false".to_owned()));
        handle.apply(&settings(Duties::default(), energy));
        assert_eq!(handle.running(), Running::default());
        handle.stop();
        let lines = said(&log);
        assert!(lines.contains(&"idle stopped".to_owned()), "{lines:?}");
        assert!(lines.contains(&"night set None".to_owned()), "{lines:?}");
        assert!(lines.contains(&"night stopped".to_owned()));
        assert!(lines.contains(&"logind caffeine false".to_owned()));
        assert!(lines.contains(&"logind stopped".to_owned()));
        // And on again.
        handle.apply(&settings(all, energy));
        assert!(handle.running().idle && handle.running().night_light);
    }

    #[test]
    fn session_verbs_reach_logind_without_any_duty_and_keep_it() {
        let (handle, log) = Fake::handle();
        handle.apply(&Settings::default());
        assert_eq!(handle.running(), Running::default());
        handle.power(PowerAction::Reboot).expect("queued");
        handle.power(PowerAction::Suspend).expect("queued");
        assert_eq!(
            said(&log),
            [
                "logind spawned",
                "logind power Reboot",
                "logind power Suspend"
            ]
        );
        assert!(handle.running().logind);
        // Switching a caffeine duty on and off does not take it from the verbs.
        let duties = Duties {
            caffeine: ON,
            ..Duties::default()
        };
        let held = Energy {
            caffeine: true,
            ..Energy::default()
        };
        handle.apply(&settings(duties, held));
        handle.apply(&Settings::default());
        assert!(handle.running().logind);
        assert!(said(&log).contains(&"logind caffeine false".to_owned()));
    }

    #[test]
    fn log_out_goes_to_the_compositor_not_to_logind() {
        let (queue, taken) = sync_channel(4);
        let log = Log::default();
        let handle = EnergyHandle::with_spawner(Box::new(Fake {
            log: Arc::clone(&log),
            compositor: Some(queue),
            busy: Busy::default(),
            release: Arc::new(AtomicBool::new(true)),
        }));
        handle.power(PowerAction::LogOut).expect("queued");
        assert_eq!(taken.try_recv(), Ok(NiriRequest::Quit));
        assert!(said(&log).is_empty());
        // Without a compositor the refusal is stated.
        let (bare, _log) = Fake::handle();
        assert!(bare.power(PowerAction::LogOut).is_err());
    }

    #[test]
    fn dropping_the_handle_joins_every_worker() {
        let (handle, log) = Fake::handle();
        handle.apply(&settings(
            Duties {
                idle: ON,
                nightlight: ON,
                caffeine: ON,
                ..Duties::default()
            },
            Energy {
                caffeine: true,
                ..Energy::default()
            },
        ));
        drop(handle);
        let lines = said(&log);
        for stopped in ["idle stopped", "night stopped", "logind stopped"] {
            assert!(lines.contains(&stopped.to_owned()), "{lines:?}");
        }
    }

    #[test]
    fn idle_actions_become_compositor_requests_and_refusals_nothing() {
        let (queue, taken) = sync_channel(4);
        let queue = Some(queue);
        perform_idle(&queue, IdleAction::PowerOffMonitors);
        assert_eq!(taken.try_recv(), Ok(NiriRequest::PowerOffMonitors));
        perform_idle(
            &queue,
            IdleAction::Refused(celestina_shell::idle::Refusal::NoLocker),
        );
        perform_idle(&queue, IdleAction::LockAndSuspend);
        assert!(taken.try_recv().is_err());
        // No compositor: logged, never a panic.
        perform_idle(&None, IdleAction::PowerOffMonitors);
    }

    fn lines_of(log: &Log, prefix: &str) -> Vec<String> {
        said(log)
            .into_iter()
            .filter(|l| l.starts_with(prefix))
            .collect()
    }

    #[test]
    fn a_refused_night_light_command_is_retried_by_the_next_apply() {
        let (handle, log, busy, _) = Fake::handle_with_controls();
        let duties = Duties {
            nightlight: ON,
            ..Duties::default()
        };
        let warm = Energy {
            night_light: true,
            ..Energy::default()
        };
        busy.lock()
            .expect("busy")
            .push("night set Some(2700)".to_owned());
        handle.apply(&settings(duties, warm));
        // The refusal is not remembered as sent: the same settings send again.
        handle.apply(&settings(duties, warm));
        // And once taken, the same settings say nothing more.
        handle.apply(&settings(duties, warm));
        assert_eq!(
            lines_of(&log, "night "),
            [
                "night spawned",
                "night set Some(2700) (busy)",
                "night set Some(2700)"
            ]
        );
    }

    #[test]
    fn a_refused_caffeine_command_is_retried_by_the_next_apply() {
        let (handle, log, busy, _) = Fake::handle_with_controls();
        let duties = Duties {
            idle: ON,
            caffeine: ON,
            ..Duties::default()
        };
        let held = Energy {
            caffeine: true,
            ..Energy::default()
        };
        {
            let mut busy = busy.lock().expect("busy");
            busy.push("logind caffeine true".to_owned());
            busy.push("idle caffeine true".to_owned());
        }
        handle.apply(&settings(duties, held));
        handle.apply(&settings(duties, held));
        handle.apply(&settings(duties, held));
        assert_eq!(
            lines_of(&log, "logind "),
            [
                "logind spawned",
                "logind caffeine true (busy)",
                "logind caffeine true"
            ]
        );
        let idle = lines_of(&log, "idle ");
        assert_eq!(
            idle.iter().filter(|l| **l == "idle caffeine true").count(),
            1
        );
        assert_eq!(idle.last().map(String::as_str), Some("idle enabled true"));
    }

    #[test]
    fn a_full_idle_start_up_queue_sends_the_rest_in_order_on_later_applies() {
        let (handle, log, busy, _) = Fake::handle_with_controls();
        let duties = Duties {
            idle: ON,
            ..Duties::default()
        };
        // The queue takes the first command and refuses the second, as a
        // 1-slot queue would; nothing after it may jump ahead of it.
        busy.lock()
            .expect("busy")
            .push("idle locker false".to_owned());
        handle.apply(&settings(duties, Energy::default()));
        assert_eq!(
            lines_of(&log, "idle "),
            [
                "idle spawned",
                "idle config default",
                "idle locker false (busy)"
            ]
        );
        handle.apply(&settings(duties, Energy::default()));
        assert_eq!(
            lines_of(&log, "idle "),
            [
                "idle spawned",
                "idle config default",
                "idle locker false (busy)",
                "idle locker false",
                "idle caffeine false",
                "idle enabled true"
            ]
        );
    }

    #[test]
    fn a_refused_release_of_the_inhibitor_is_retried_while_the_verbs_keep_it() {
        let (handle, log, busy, _) = Fake::handle_with_controls();
        let duties = Duties {
            caffeine: ON,
            ..Duties::default()
        };
        let held = Energy {
            caffeine: true,
            ..Energy::default()
        };
        handle.power(PowerAction::Suspend).expect("queued");
        handle.apply(&settings(duties, held));
        busy.lock()
            .expect("busy")
            .push("logind caffeine false".to_owned());
        handle.apply(&settings(Duties::default(), held));
        handle.apply(&settings(Duties::default(), held));
        let lines = lines_of(&log, "logind caffeine");
        assert_eq!(
            lines,
            [
                "logind caffeine true",
                "logind caffeine false (busy)",
                "logind caffeine false"
            ]
        );
        assert!(handle.running().logind);
    }

    #[test]
    fn turning_a_duty_off_does_not_wait_for_the_worker_and_on_waits_for_it() {
        let (handle, log, _, release) = Fake::handle_with_controls();
        let duties = Duties {
            nightlight: ON,
            ..Duties::default()
        };
        release.store(false, Ordering::Release);
        handle.apply(&settings(duties, Energy::default()));
        // The worker is held after its flag: `apply` still returns, joined by
        // the reaper, and the old worker has not finished.
        handle.apply(&settings(Duties::default(), Energy::default()));
        assert_eq!(handle.running(), Running::default());
        assert!(!said(&log).contains(&"night stopped".to_owned()));
        // Let it finish shortly while the duty comes back on: the new worker
        // is only spawned after the old one is gone.
        let opener = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            release.store(true, Ordering::Release);
        });
        handle.apply(&settings(duties, Energy::default()));
        opener.join().expect("opener");
        let lines = lines_of(&log, "night ");
        let stopped = lines
            .iter()
            .position(|l| l == "night stopped")
            .expect("stopped");
        let respawned = lines
            .iter()
            .rposition(|l| l == "night spawned")
            .expect("spawned again");
        assert!(stopped < respawned, "{lines:?}");
        assert!(handle.running().night_light);
    }
}
