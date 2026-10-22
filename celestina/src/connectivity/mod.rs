//! Connectivity and power: the network, Bluetooth and power workers and the
//! state they publish (CEL-2-F).
//!
//! The network and Bluetooth services are each one **lane**: a [`Worker`]
//! thread that owns its client, reads the service, publishes the shell's
//! bounded section of `celestina_shell::connectivity` and performs the
//! requests the pages send. Both lanes run one loop ([`Provider`] is what
//! differs):
//!
//! - **Reading.** The first read happens when the lane starts; after it, a
//!   change watcher (the service's signals, coalesced by the client) marks the
//!   state dirty and the lane re-reads, with a safety re-read every
//!   [`Timing::reread`] (30 s) and every [`Timing::pending_reread`] (2 s)
//!   while a request waits for its confirmation or the provider follows one
//!   to its outcome. A section is published only when it differs from the
//!   last one published.
//! - **One watcher per lane.** The watcher is started once, after the first
//!   good read, and lives as long as the lane: it subscribes by the
//!   service's bus name, so it outlasts the service's restarts. It is never
//!   started while the service is absent, because a watcher's connection and
//!   threads are freed only by the service's next signal, and an absent
//!   service sends none (a watcher per retry would leak one connection and
//!   three threads each time). A watcher that could not start is tried again
//!   after the next outage.
//! - **Absence (D10).** A client that cannot open or a read that fails
//!   publishes [`Service::Unavailable`] once, closes the client (not the
//!   watcher), and retries after a backoff of 5 s doubling to 60 s; the first
//!   good read resets it.
//! - **Requests (D3, D5).** [`ConnectivityHandle::send`] queues an [`Action`]
//!   on its lane's bounded queue ([`QUEUE`]) and refuses the rest, never
//!   waiting. The lane checks it with [`validate`] against the *last
//!   confirmed* section; a refusal is published as
//!   [`ConnectivityEvent::Refused`] and never reaches the client. An accepted
//!   request is published [`ConnectivityEvent::Pending`] and is confirmed or
//!   failed by a later observation, by the client's error, or by the pending
//!   window's end ([`ConnectivityEvent::Resolved`]). No optimistic state.
//! - **Waking.** A lane sleeps on its queue until its next clock (a read, a
//!   retry, a pending window's end), at most [`Timing::poll`] (1 s), so at
//!   rest it wakes once a second, not ten times. A request wakes it at once,
//!   and so does [`ConnectivityHandle::stop`], which drops the queue.
//! - **Shutdown.** Every wait carries a timeout ([`Timing::poll`]), so a lane
//!   looks at its flag at a bounded interval; [`ConnectivityHandle::stop`]
//!   raises it and joins every worker within one shared [`JOIN_WAIT`]. A client call itself
//!   cannot be interrupted (NetworkManager's client waits up to 120 s for a
//!   polkit prompt); a lane stuck in one is reported and left behind.
//!
//! The power worker (UPower and power-profiles-daemon) lives in `power` with
//! its own polling loop (it has no change watcher, so it does not fit
//! [`Provider`]); the handle starts it beside the lanes with the handle's own
//! shutdown flag, routes the power actions to its queue, wraps its news as
//! [`ConnectivityEvent::Power`] and joins it in [`ConnectivityHandle::stop`].
//!
//! [`forward`] is the one way a page's or a piece's request reaches a worker:
//! the loop's [`state::ConnectivityState`] checks it first and only an
//! accepted request is queued.

pub mod power;

pub mod bluetooth;
pub mod editor;
pub mod network;
pub mod state;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::connectivity::{
    validate, Action, BtSection, Editor, Expected, Failure, Inventory, NetSection, PendingSet,
    Refusal, Resolution, Section, Service, Step,
};

use crate::providers::worker::Worker;
use power::{PowerCommand, PowerEvent, PowerProvider};
use state::ConnectivityState;

/// A fixed token for the kind of `step`, for the journal: a step holds a
/// row id or a device address (service data), so it is never logged itself.
pub(super) const fn step_kind(step: &Step) -> &'static str {
    match step {
        Step::Refresh(_) => "refresh",
        Step::SetWifiEnabled(_) => "set-wifi-enabled",
        Step::Join(_) => "join",
        Step::Leave(_) => "leave",
        Step::SetBluetoothPowered(_) => "set-bluetooth-powered",
        Step::ConnectDevice(_) => "connect-device",
        Step::DisconnectDevice(_) => "disconnect-device",
        Step::SetProfile(_) => "set-profile",
        Step::OpenEditor(_) => "open-editor",
    }
}

pub use bluetooth::BluetoothProvider;
pub use network::NetworkProvider;

/// The requests that may wait for one lane. Every page action is a single,
/// deliberate gesture; past this a binding is repeating itself, and refusing
/// is more honest than performing a stale request late (D6).
pub const QUEUE: usize = 4;
/// The longest [`ConnectivityHandle::stop`] waits for all the workers
/// together.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);
/// The least each worker's join is given once the shared deadline has
/// passed, so a worker that already ended is still seen as stopped.
const JOIN_FLOOR: Duration = Duration::from_millis(10);

/// Keeps a client's change watcher alive; dropping it stops the watcher.
pub type WatchGuard = Box<dyn Send>;

/// Where a lane's events go. Returns `false` once nobody listens, and the
/// lane then stops.
pub type Publish = Arc<dyn Fn(ConnectivityEvent) -> bool + Send + Sync>;

/// What the connectivity workers hand to the event loop.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectivityEvent {
    /// NetworkManager's state, whenever it changed.
    Network(Service<NetSection>),
    /// BlueZ's state, whenever it changed; `Unavailable` also when there is
    /// no adapter.
    Bluetooth(Service<BtSection>),
    /// The power worker's news: the battery, the profiles, a request's end.
    Power(PowerEvent),
    /// A request passed [`validate`]; its row is pending until a
    /// [`ConnectivityEvent::Resolved`] for the same subject.
    Pending(Expected),
    /// A pending request was confirmed by an observation, or failed by kind
    /// (the client's error, an observed failure, or the window's end).
    Resolved(Resolution),
    /// A request was refused by [`validate`] against the last confirmed
    /// state, before any call; it was dropped.
    Refused(Action, Refusal),
}

/// Which worker serves an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// The network lane.
    Network,
    /// The Bluetooth lane.
    Bluetooth,
    /// The power worker (UPower and power-profiles-daemon).
    Power,
    /// No worker: the editing UI is launched by the app (`launcher::spawn`).
    Editor,
}

/// The worker `action` belongs to.
#[must_use]
pub fn destination(action: &Action) -> Destination {
    match action {
        Action::Refresh(Section::Network)
        | Action::SetWifiEnabled(_)
        | Action::JoinSaved(_)
        | Action::Leave(_) => Destination::Network,
        Action::Refresh(Section::Bluetooth)
        | Action::SetBluetoothPowered(_)
        | Action::ConnectDevice(_)
        | Action::DisconnectDevice(_) => Destination::Bluetooth,
        Action::Refresh(Section::Battery | Section::Profiles)
        | Action::SetProfile(_)
        | Action::CycleProfile => Destination::Power,
        Action::OpenEditor(_) => Destination::Editor,
    }
}

/// Why a request was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// [`QUEUE`] requests are waiting on the worker already.
    #[error("the {0:?} queue is full")]
    Full(Destination),
    /// The worker has stopped.
    #[error("the {0:?} worker has stopped")]
    Stopped(Destination),
    /// The worker was never started (or could not start).
    #[error("no {0:?} worker is running")]
    NotStarted(Destination),
    /// The worker did not take it: its queue is full or it has stopped (the
    /// power worker's queue does not say which).
    #[error("the {0:?} worker did not take the request")]
    Refused(Destination),
    /// The action is not a worker's: opening an editor is the app's.
    #[error("opening an editor is not a worker request")]
    NotForWorkers,
}

/// How often a lane looks, re-reads and retries. [`Timing::default`] is
/// production; tests shorten it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// The longest a lane sleeps on its queue before looking at its flag and
    /// its clocks; it wakes earlier when one of its clocks is due.
    pub poll: Duration,
    /// The safety re-read when the watcher says nothing.
    pub reread: Duration,
    /// The re-read while a request is pending.
    pub pending_reread: Duration,
    /// The first retry after the service went missing.
    pub backoff_first: Duration,
    /// The longest retry interval.
    pub backoff_max: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(1),
            reread: Duration::from_secs(30),
            pending_reread: Duration::from_secs(2),
            backoff_first: Duration::from_secs(5),
            backoff_max: Duration::from_secs(60),
        }
    }
}

/// What a lane needs of its service; implemented by [`NetworkProvider`] and
/// [`BluetoothProvider`].
pub trait Provider: Send + 'static {
    /// The section this lane publishes.
    type State: Clone + PartialEq + Send;
    /// The lane's name, for its thread and the journal.
    const WHAT: &'static str;
    /// The section the lane serves.
    const SECTION: Section;

    /// Opens the client.
    ///
    /// # Errors
    ///
    /// The failure by kind; the lane publishes `Unavailable` and retries.
    fn open(&mut self) -> Result<(), Failure>;
    /// Drops the client. The watcher is the lane's and is not touched.
    fn close(&mut self);
    /// Starts the service's change watcher, which raises `dirty` on every
    /// coalesced burst; `None` when it could not start. The lane calls it
    /// at most once per outage, and only after a good read.
    fn watch(&mut self, dirty: Arc<AtomicBool>) -> Option<WatchGuard>;
    /// Reads the service and converts it into the shell's bounded section.
    ///
    /// # Errors
    ///
    /// The failure by kind; the lane degrades to `Unavailable`.
    fn read(&mut self) -> Result<Self::State, Failure>;
    /// `state` as the inventory [`validate`] checks against.
    fn inventory(state: Option<&Self::State>) -> Inventory<'_>;
    /// Performs one checked step; `state` is the inventory it was checked
    /// against.
    ///
    /// # Errors
    ///
    /// The client's failure by kind; the request's pending entry fails.
    fn perform(&mut self, step: &Step, state: Option<&Self::State>) -> Result<(), Failure>;
    /// Failures the last reads observed for pending requests (a join that
    /// NetworkManager gave up on).
    fn take_observed(&mut self) -> Vec<(Expected, Failure)>;
    /// A request has been resolved. A confirmation may leave the provider
    /// following the request to its outcome (a join confirmed while
    /// NetworkManager activates it can still fail); a failure ends it.
    fn settled(&mut self, resolution: &Resolution);
    /// Whether the provider follows a confirmed request to its outcome; the
    /// lane keeps its fast re-read while so.
    fn following(&self) -> bool {
        false
    }
    /// The event that carries this lane's state.
    fn event(state: Service<Self::State>) -> ConnectivityEvent;
}

/// Where and how often the power worker reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerSetup {
    /// The bus UPower and power-profiles-daemon are read on.
    pub bus: power::Bus,
    /// The worker's intervals.
    pub timing: power::Timing,
}

/// The providers a [`ConnectivityHandle`] starts; a worker left `None` is not
/// started.
#[derive(Debug, Default)]
pub struct Providers {
    /// The network lane's provider.
    pub network: Option<NetworkProvider>,
    /// The Bluetooth lane's provider.
    pub bluetooth: Option<BluetoothProvider>,
    /// The power worker's bus and intervals.
    pub power: Option<PowerSetup>,
}

impl Providers {
    /// The providers over the system bus.
    #[must_use]
    pub fn system() -> Self {
        Self {
            network: Some(NetworkProvider::system()),
            bluetooth: Some(BluetoothProvider::system()),
            power: Some(PowerSetup {
                bus: power::Bus::System,
                timing: power::Timing::default(),
            }),
        }
    }
}

/// One started lane: its queue and its thread. The queue is dropped before
/// the thread is joined, so a lane waiting on it ends by itself as well.
struct LaneHandle {
    requests: SyncSender<Action>,
    worker: Worker,
}

/// The started power worker: its queue and its thread.
struct PowerHandle {
    requests: PowerProvider,
    worker: Worker,
}

/// The way into the connectivity workers: queues requests, never waits for
/// them, and joins the workers at [`ConnectivityHandle::stop`]. Every worker
/// shares the handle's own shutdown flag (not the shell's: stopping
/// connectivity tells nothing to the rest of the process).
pub struct ConnectivityHandle {
    network: Option<LaneHandle>,
    bluetooth: Option<LaneHandle>,
    power: Option<PowerHandle>,
    shutdown: Arc<AtomicBool>,
}

impl std::fmt::Debug for ConnectivityHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectivityHandle")
            .field("network", &self.network.is_some())
            .field("bluetooth", &self.bluetooth.is_some())
            .field("power", &self.power.is_some())
            .finish_non_exhaustive()
    }
}

impl ConnectivityHandle {
    /// Starts every worker over the system bus, publishing through
    /// `publish`. A worker whose thread cannot start is logged and its
    /// sections published `Unavailable`; the others run.
    #[must_use]
    pub fn start(publish: impl Fn(ConnectivityEvent) -> bool + Send + Sync + 'static) -> Self {
        Self::start_with(Providers::system(), Arc::new(publish), Timing::default())
    }

    /// Starts the workers of `providers`, the lanes with `timing`; tests
    /// pass fakes and a private bus.
    #[must_use]
    pub fn start_with(providers: Providers, publish: Publish, timing: Timing) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        Self {
            network: providers
                .network
                .and_then(|provider| spawn_lane(provider, &publish, timing, &shutdown)),
            bluetooth: providers
                .bluetooth
                .and_then(|provider| spawn_lane(provider, &publish, timing, &shutdown)),
            power: providers
                .power
                .and_then(|setup| spawn_power(setup, &publish, &shutdown)),
            shutdown,
        }
    }

    /// Queues `action` on its worker. Never waits: a full queue, a stopped or
    /// missing worker, or an action no worker serves is refused, logged and
    /// returned, so the caller can show it (a full queue reads
    /// `Failure::Busy`).
    ///
    /// # Errors
    ///
    /// [`RequestError`] naming why the action was not queued.
    pub fn send(&self, action: Action) -> Result<(), RequestError> {
        let to = destination(&action);
        let lane = match to {
            Destination::Network => self.network.as_ref(),
            Destination::Bluetooth => self.bluetooth.as_ref(),
            Destination::Power => {
                let result = self.send_power(&action);
                if let Err(err) = &result {
                    tracing::warn!(%err, "connectivity: request not queued");
                }
                return result;
            }
            Destination::Editor => {
                tracing::warn!("connectivity: an editor request reached the workers");
                return Err(RequestError::NotForWorkers);
            }
        };
        let result = match lane {
            None => Err(RequestError::NotStarted(to)),
            Some(lane) => lane.requests.try_send(action).map_err(|err| match err {
                TrySendError::Full(_) => RequestError::Full(to),
                TrySendError::Disconnected(_) => RequestError::Stopped(to),
            }),
        };
        if let Err(err) = &result {
            tracing::warn!(%err, "connectivity: request not queued");
        }
        result
    }

    fn send_power(&self, action: &Action) -> Result<(), RequestError> {
        let power = self
            .power
            .as_ref()
            .ok_or(RequestError::NotStarted(Destination::Power))?;
        // `destination` sent only the power actions here.
        let command = PowerCommand::from_action(action).ok_or(RequestError::NotForWorkers)?;
        power
            .requests
            .request(command)
            .map_err(|_| RequestError::Refused(Destination::Power))
    }

    /// Stops every worker and joins them within one shared [`JOIN_WAIT`]:
    /// the flag is raised for all of them first, so they stop together, and
    /// each join gets what is left of the one deadline (workers stuck in a
    /// client call cost the exit [`JOIN_WAIT`] in all, not each). Returns
    /// whether all of them stopped in time.
    pub fn stop(self) -> bool {
        let Self {
            network,
            bluetooth,
            power,
            shutdown,
        } = self;
        shutdown.store(true, Ordering::Release);
        let mut workers = Vec::with_capacity(3);
        for lane in [network, bluetooth].into_iter().flatten() {
            let LaneHandle { requests, worker } = lane;
            drop(requests);
            workers.push(worker);
        }
        if let Some(PowerHandle { requests, worker }) = power {
            drop(requests);
            workers.push(worker);
        }
        let deadline = Instant::now() + JOIN_WAIT;
        let mut stopped = true;
        for worker in workers {
            let left = deadline.saturating_duration_since(Instant::now());
            stopped &= worker.join_within(left.max(JOIN_FLOOR));
        }
        stopped
    }
}

/// Journals the first answer of each power service (battery, then profiles),
/// so the journal says whether the worker reached them. Later changes are
/// silent: the journal is not a stream of percentages.
fn announce_power(event: &PowerEvent, announced: &mut [bool; 2]) {
    if let Some((name, word)) = first_power_answer(event, announced) {
        tracing::info!("power: {name} {word}");
    }
}

/// The service name and its first answer, `None` for a later answer or a
/// request's end.
fn first_power_answer(
    event: &PowerEvent,
    announced: &mut [bool; 2],
) -> Option<(&'static str, &'static str)> {
    let (slot, name, word) = match event {
        PowerEvent::Battery(service) => (0, "battery", service_word(service)),
        PowerEvent::Profiles(service) => (1, "profiles", service_word(service)),
        PowerEvent::Resolved(_) | PowerEvent::Refused(_) => return None,
    };
    if announced[slot] {
        return None;
    }
    announced[slot] = true;
    Some((name, word))
}

fn service_word<T>(service: &Service<T>) -> &'static str {
    match service.ready() {
        Some(_) => "ready",
        None => "unavailable",
    }
}

fn spawn_power(
    setup: PowerSetup,
    publish: &Publish,
    shutdown: &Arc<AtomicBool>,
) -> Option<PowerHandle> {
    let sink = Arc::clone(publish);
    let mut announced = [false; 2];
    let started = power::spawn_with(setup.bus, setup.timing, shutdown, move |event| {
        announce_power(&event, &mut announced);
        sink(ConnectivityEvent::Power(event))
    });
    match started {
        Ok((requests, worker)) => Some(PowerHandle { requests, worker }),
        Err(err) => {
            tracing::error!(%err, "the power worker did not start");
            publish(ConnectivityEvent::Power(PowerEvent::Battery(
                Service::Unavailable,
            )));
            publish(ConnectivityEvent::Power(PowerEvent::Profiles(
                Service::Unavailable,
            )));
            None
        }
    }
}

/// What became of a request [`forward`] was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Forwarded {
    /// Checked by the state and queued on its worker; its row is pending.
    Sent,
    /// The state's own check refused it; nothing was queued and the row
    /// shows the refusal.
    Refused(Refusal),
    /// Checked, but its worker did not take it; the row shows the failure.
    Undelivered(RequestError),
    /// An editing UI to start: the caller's (`editor::open`), never a
    /// worker's.
    Editor(Editor),
}

/// The failure a row shows for a request its worker did not take.
#[must_use]
pub fn undelivered(err: RequestError) -> Failure {
    match err {
        RequestError::Full(_) | RequestError::Refused(_) => Failure::Busy,
        RequestError::Stopped(_) | RequestError::NotStarted(_) => Failure::Unreachable,
        RequestError::NotForWorkers => Failure::Other,
    }
}

/// The one way a page's row or a piece's gesture reaches a worker:
/// [`ConnectivityState::begin`] checks `action` against the loop's last
/// confirmed state and marks its row; only an accepted action is queued on
/// `handle`, and one the worker does not take fails its row. An editor is
/// handed back to the caller unchecked (it targets no row).
pub fn forward(
    state: &mut ConnectivityState,
    handle: Option<&ConnectivityHandle>,
    action: Action,
) -> Forwarded {
    if let Action::OpenEditor(editor) = action {
        return Forwarded::Editor(editor);
    }
    if let Err(refusal) = state.begin(&action) {
        tracing::info!(refusal = refusal.key(), "connectivity: request refused");
        return Forwarded::Refused(refusal);
    }
    let sent = match handle {
        Some(handle) => handle.send(action.clone()),
        None => Err(RequestError::NotStarted(destination(&action))),
    };
    match sent {
        Ok(()) => Forwarded::Sent,
        Err(err) => {
            state.fail(&action, undelivered(err));
            Forwarded::Undelivered(err)
        }
    }
}

fn spawn_lane<P: Provider>(
    provider: P,
    publish: &Publish,
    timing: Timing,
    shutdown: &Arc<AtomicBool>,
) -> Option<LaneHandle> {
    let (requests, queue) = mpsc::sync_channel(QUEUE);
    let lane = Lane::new(provider, Arc::clone(publish), timing);
    let flag = Arc::clone(shutdown);
    match Worker::spawn(P::WHAT, shutdown, move || lane.run(&queue, &flag)) {
        Ok(worker) => Some(LaneHandle { requests, worker }),
        Err(err) => {
            tracing::error!(%err, "the {} worker did not start", P::WHAT);
            publish(P::event(Service::Unavailable));
            None
        }
    }
}

/// One lane's loop state.
struct Lane<P: Provider> {
    provider: P,
    publish: Publish,
    timing: Timing,
    /// Raised by the change watcher (and after a request): re-read now.
    dirty: Arc<AtomicBool>,
    /// The change watcher: one for the lane's whole life (see the module).
    watch: Option<WatchGuard>,
    /// Whether a watcher was tried since the last outage.
    watch_tried: bool,
    open: bool,
    /// The last confirmed section; `None` while the service is unavailable.
    state: Option<P::State>,
    /// The last state published, to publish only changes.
    published: Option<Service<P::State>>,
    pending: PendingSet,
    retry_at: Instant,
    backoff: Duration,
    next_read: Instant,
    /// Whether anyone still listens.
    alive: bool,
}

impl<P: Provider> Lane<P> {
    fn new(provider: P, publish: Publish, timing: Timing) -> Self {
        let now = Instant::now();
        Self {
            provider,
            publish,
            timing,
            dirty: Arc::new(AtomicBool::new(false)),
            watch: None,
            watch_tried: false,
            open: false,
            state: None,
            published: None,
            pending: PendingSet::new(),
            retry_at: now,
            backoff: timing.backoff_first,
            next_read: now,
            alive: true,
        }
    }

    fn run(mut self, queue: &Receiver<Action>, shutdown: &AtomicBool) {
        while self.alive && !shutdown.load(Ordering::Acquire) {
            let now = Instant::now();
            if !self.open {
                if now >= self.retry_at {
                    self.try_open();
                }
            } else if self.dirty.swap(false, Ordering::AcqRel) || now >= self.next_read {
                self.read();
            }
            // The clock after the work above: a slow open or read must not
            // leave a deadline that is already past.
            let now = Instant::now();
            if self.pending.next_deadline().is_some_and(|due| due <= now) {
                self.resolve(now);
            }
            if !self.alive || shutdown.load(Ordering::Acquire) {
                break;
            }
            match queue.recv_timeout(self.wait(Instant::now())) {
                Ok(action) => self.handle(action, Instant::now()),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.watch = None;
        self.provider.close();
    }

    /// How long the lane may sleep at `now`: until its next clock (a read
    /// while open, a retry while not, a pending window's end), at most
    /// [`Timing::poll`]; not at all while open with a re-read asked for.
    fn wait(&self, now: Instant) -> Duration {
        if self.open && self.dirty.load(Ordering::Acquire) {
            return Duration::ZERO;
        }
        let due = if self.open {
            self.next_read
        } else {
            self.retry_at
        };
        let due = self
            .pending
            .next_deadline()
            .map_or(due, |deadline| deadline.min(due));
        due.saturating_duration_since(now).min(self.timing.poll)
    }

    /// Starts the lane's one watcher after a good read, unless one runs or
    /// was already tried since the last outage.
    fn ensure_watch(&mut self) {
        if self.watch.is_some() || self.watch_tried {
            return;
        }
        self.watch_tried = true;
        self.watch = self.provider.watch(Arc::clone(&self.dirty));
        if self.watch.is_some() {
            // A change between that read and the subscription is not lost.
            self.dirty.store(true, Ordering::Release);
        }
    }

    fn say(&mut self, event: ConnectivityEvent) {
        if self.alive && !(self.publish)(event) {
            self.alive = false;
        }
    }

    fn publish_state(&mut self, state: Service<P::State>) {
        if self.published.as_ref() != Some(&state) {
            self.published = Some(state.clone());
            self.say(P::event(state));
        }
    }

    /// Opens the provider and reads it. Every clock it sets counts from when
    /// the call returned, so an open slower than the backoff does not make
    /// the next retry due at once.
    fn try_open(&mut self) {
        let result = self.provider.open();
        match result {
            Ok(()) => {
                self.open = true;
                self.read();
            }
            Err(failure) => self.degrade(Instant::now(), failure),
        }
    }

    /// Reads the provider. The next read is scheduled from when the read
    /// returned: a snapshot slower than [`Timing::pending_reread`] would
    /// otherwise leave the next read already due, and the lane would re-read
    /// back to back with no sleep.
    fn read(&mut self) {
        let result = self.provider.read();
        let now = Instant::now();
        match result {
            Ok(state) => {
                if self.state.is_none() {
                    tracing::info!("{}: ready", P::WHAT);
                }
                self.state = Some(state.clone());
                self.publish_state(Service::Ready(state));
                self.backoff = self.timing.backoff_first;
                self.ensure_watch();
                self.resolve(now);
                let next = if self.pending.is_empty() && !self.provider.following() {
                    self.timing.reread
                } else {
                    self.timing.pending_reread
                };
                self.next_read = now + next;
            }
            Err(failure) => self.degrade(now, failure),
        }
    }

    fn degrade(&mut self, now: Instant, failure: Failure) {
        if self.open {
            self.provider.close();
            self.open = false;
        }
        self.state = None;
        if self.watch.is_none() {
            // A watcher that could not start is tried again once the
            // service is back.
            self.watch_tried = false;
        }
        if self.published != Some(Service::Unavailable) {
            tracing::info!(
                ?failure,
                retry_ms = self.backoff.as_millis(),
                "{}: unavailable",
                P::WHAT
            );
        }
        self.publish_state(Service::Unavailable);
        self.retry_at = now + self.backoff;
        self.backoff = (self.backoff * 2).min(self.timing.backoff_max);
    }

    /// Confirms or times out pending requests against the last state, then
    /// fails those the provider saw fail.
    fn resolve(&mut self, now: Instant) {
        let mut resolved = {
            let inventory = P::inventory(self.state.as_ref());
            self.pending.observe(&inventory, now)
        };
        for (expected, failure) in self.provider.take_observed() {
            // A join confirmed when NetworkManager took it has no pending
            // entry left when it fails afterwards: its failure is published
            // all the same, and the loop's state shows it on the row.
            let resolution = self
                .pending
                .fail(&expected, failure)
                .unwrap_or(Resolution::Failed(expected, failure));
            resolved.push(resolution);
        }
        for resolution in resolved {
            self.provider.settled(&resolution);
            self.say(ConnectivityEvent::Resolved(resolution));
        }
    }

    fn handle(&mut self, action: Action, now: Instant) {
        let checked = {
            let inventory = P::inventory(self.state.as_ref());
            validate(&action, &inventory)
        };
        let checked = match checked {
            Ok(checked) => checked,
            Err(refusal) => {
                tracing::info!(refusal = refusal.key(), "{}: request refused", P::WHAT);
                self.say(ConnectivityEvent::Refused(action, refusal));
                return;
            }
        };
        if let Step::Refresh(_) = checked.step() {
            if self.open {
                self.dirty.store(true, Ordering::Release);
            } else {
                // Asked for by the person: retry now, from the first backoff.
                self.retry_at = now;
                self.backoff = self.timing.backoff_first;
            }
            return;
        }
        let expected = checked.expected();
        if let Some(expected) = &expected {
            self.pending.start(expected.clone(), now);
            self.say(ConnectivityEvent::Pending(expected.clone()));
        }
        if let Err(failure) = self.provider.perform(checked.step(), self.state.as_ref()) {
            if let Some(expected) = &expected {
                if let Some(resolution) = self.pending.fail(expected, failure) {
                    self.provider.settled(&resolution);
                    self.say(ConnectivityEvent::Resolved(resolution));
                }
            }
        }
        // The observation after a request is its truth.
        self.dirty.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluetooth::tests::{BtCall, BtScript};
    use celestina_shell::connectivity::{BtAddress, Editor, RowId};
    use cuprita_core::error::{BluetoothError, NetworkError};
    use network::tests::{NetCall, NetScript};
    use std::sync::Mutex;

    const WAIT: Duration = Duration::from_secs(5);

    fn fast() -> Timing {
        Timing {
            poll: Duration::from_millis(10),
            reread: Duration::from_secs(30),
            pending_reread: Duration::from_millis(50),
            backoff_first: Duration::from_millis(20),
            backoff_max: Duration::from_millis(80),
        }
    }

    fn id(raw: &str) -> RowId {
        RowId::new(raw).expect("id")
    }

    fn address(raw: &str) -> BtAddress {
        BtAddress::parse(raw).expect("address")
    }

    /// Both lanes over recording fakes, their events in a channel.
    struct Rig {
        handle: ConnectivityHandle,
        events: Receiver<ConnectivityEvent>,
        net: network::tests::Script,
        bt: bluetooth::tests::Script,
    }

    impl Rig {
        fn start(net_gone: bool) -> Self {
            let net = Arc::new(Mutex::new(NetScript {
                gone: net_gone,
                ..NetScript::default()
            }));
            let bt = Arc::new(Mutex::new(BtScript::default()));
            let (network, _) = network::tests::recording(&net);
            let providers = Providers {
                network: Some(network),
                bluetooth: Some(bluetooth::tests::recording(&bt)),
                power: None,
            };
            let (sender, events) = mpsc::channel();
            let handle = ConnectivityHandle::start_with(
                providers,
                Arc::new(move |event| sender.send(event).is_ok()),
                fast(),
            );
            Self {
                handle,
                events,
                net,
                bt,
            }
        }

        /// Waits for the first event `wanted` keeps, discarding the others.
        fn expect(&self, what: &str, wanted: impl Fn(&ConnectivityEvent) -> bool) {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.events.recv_timeout(left) {
                    Ok(event) if wanted(&event) => return,
                    Ok(_) => {}
                    Err(_) => panic!("no {what} within {WAIT:?}"),
                }
            }
        }

        /// Waits until both lanes published a first state, in either order.
        fn ready(&self) {
            let deadline = Instant::now() + WAIT;
            let (mut network, mut bluetooth) = (false, false);
            while !(network && bluetooth) {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.events.recv_timeout(left) {
                    Ok(ConnectivityEvent::Network(Service::Ready(_))) => network = true,
                    Ok(ConnectivityEvent::Bluetooth(Service::Ready(_))) => bluetooth = true,
                    Ok(_) => {}
                    Err(_) => panic!("no first states within {WAIT:?}"),
                }
            }
        }

        fn net_calls(&self) -> Vec<NetCall> {
            self.net.lock().expect("script").calls.clone()
        }

        fn bt_calls(&self) -> Vec<BtCall> {
            self.bt.lock().expect("script").calls.clone()
        }
    }

    #[test]
    fn a_saved_join_is_pending_then_confirmed_by_observation_with_no_secret() {
        let rig = Rig::start(false);
        rig.ready();

        rig.handle
            .send(Action::JoinSaved(id("home-wifi")))
            .expect("queued");

        let expected = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        rig.expect("pending", |e| {
            *e == ConnectivityEvent::Pending(expected.clone())
        });
        rig.expect("confirmation", |e| {
            *e == ConnectivityEvent::Resolved(Resolution::Confirmed(expected.clone()))
        });
        assert!(rig.net_calls().contains(&NetCall::Connect {
            id: "home-wifi".to_owned(),
            secret: false,
        }));
        assert!(rig.handle.stop());
    }

    #[test]
    fn refused_requests_never_reach_a_client() {
        let rig = Rig::start(false);
        rig.ready();
        let refusals = [
            (Action::JoinSaved(id("nowhere")), Refusal::UnknownId),
            (Action::JoinSaved(id("office-vpn")), Refusal::KindNotAllowed),
            (Action::Leave(id("office-vpn")), Refusal::KindNotAllowed),
            (Action::JoinSaved(id("open-cafe")), Refusal::NotKnown),
            (Action::Leave(id("home-wifi")), Refusal::NotActive),
            (
                Action::ConnectDevice(address("AA:BB:CC:00:00:02")),
                Refusal::NotPaired,
            ),
            (
                Action::ConnectDevice(address("AA:BB:CC:00:00:99")),
                Refusal::UnknownId,
            ),
        ];
        let net_before = rig.net_calls().len();
        let bt_before = rig.bt_calls().len();

        for (action, refusal) in refusals {
            rig.handle.send(action.clone()).expect("queued");
            rig.expect("refusal", |e| {
                *e == ConnectivityEvent::Refused(action.clone(), refusal)
            });
        }

        assert!(rig.net_calls()[net_before..]
            .iter()
            .all(|call| *call == NetCall::Snapshot));
        assert!(rig.bt_calls()[bt_before..]
            .iter()
            .all(|call| *call == BtCall::Snapshot));
        assert!(rig.handle.stop());
    }

    #[test]
    fn every_action_reaches_only_the_calls_the_contract_allows() {
        let rig = Rig::start(false);
        rig.ready();
        let headphones = address("AA:BB:CC:00:00:01");
        let actions = [
            Action::SetWifiEnabled(false),
            Action::SetWifiEnabled(true),
            Action::JoinSaved(id("home-wifi")),
            Action::Leave(id("home-wifi")),
            Action::JoinSaved(id("wired")),
            Action::Leave(id("wired")),
            Action::JoinSaved(id("office-vpn")),
            Action::JoinSaved(id("open-cafe")),
            Action::ConnectDevice(headphones.clone()),
            Action::DisconnectDevice(headphones),
            Action::ConnectDevice(address("AA:BB:CC:00:00:02")),
            Action::SetBluetoothPowered(false),
            Action::SetBluetoothPowered(true),
        ];
        for action in actions {
            // One at a time, so the queue never fills and each is checked
            // against the state the previous one left.
            rig.handle.send(action).expect("queued");
            rig.expect("an outcome", |e| {
                matches!(
                    e,
                    ConnectivityEvent::Resolved(Resolution::Confirmed(_))
                        | ConnectivityEvent::Refused(..)
                )
            });
        }
        rig.handle
            .send(Action::Refresh(Section::Network))
            .expect("queued");
        rig.handle
            .send(Action::Refresh(Section::Bluetooth))
            .expect("queued");

        let net_calls = rig.net_calls();
        assert!(net_calls.contains(&NetCall::SetWifiEnabled(false)));
        for call in net_calls {
            match call {
                NetCall::Snapshot | NetCall::SetWifiEnabled(_) | NetCall::Disconnect(_) => {}
                NetCall::Connect { secret, .. } => assert!(!secret, "a secret was passed"),
                other => panic!("forbidden network call {other:?}"),
            }
        }
        let bt_calls = rig.bt_calls();
        assert!(bt_calls.contains(&BtCall::SetPowered(false)));
        for call in bt_calls {
            match call {
                BtCall::Snapshot
                | BtCall::SetPowered(_)
                | BtCall::Connect(_)
                | BtCall::Disconnect(_) => {}
                other => panic!("forbidden Bluetooth call {other:?}"),
            }
        }
        assert!(rig.handle.stop());
    }

    #[test]
    fn client_errors_fail_the_pending_row_by_kind() {
        let rig = Rig::start(false);
        rig.ready();
        let cases = [
            (NetworkError::Denied, Failure::Denied),
            (NetworkError::Busy, Failure::Busy),
            (NetworkError::NotFound(String::new()), Failure::NotFound),
            (NetworkError::password_required(), Failure::NeedsSecret),
            (NetworkError::service_failed(), Failure::Other),
            (NetworkError::Unavailable, Failure::Unreachable),
        ];
        let expected = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        for (error, failure) in cases {
            rig.net.lock().expect("script").fail_next.push_back(error);
            rig.handle
                .send(Action::JoinSaved(id("home-wifi")))
                .expect("queued");
            rig.expect("the failure", |e| {
                *e == ConnectivityEvent::Resolved(Resolution::Failed(expected.clone(), failure))
            });
        }

        let device = address("AA:BB:CC:00:00:01");
        rig.bt
            .lock()
            .expect("script")
            .fail_next
            .push_back(BluetoothError::Denied);
        rig.handle
            .send(Action::ConnectDevice(device.clone()))
            .expect("queued");
        rig.expect("the Bluetooth failure", |e| {
            *e == ConnectivityEvent::Resolved(Resolution::Failed(
                Expected::BtConnected {
                    address: device.clone(),
                    connected: true,
                },
                Failure::Denied,
            ))
        });
        assert!(rig.handle.stop());
    }

    #[test]
    fn a_vanished_service_degrades_once_and_recovers() {
        let rig = Rig::start(false);
        rig.ready();

        rig.net.lock().expect("script").gone = true;
        rig.handle
            .send(Action::Refresh(Section::Network))
            .expect("queued");
        rig.expect("unavailable", |e| {
            *e == ConnectivityEvent::Network(Service::Unavailable)
        });
        // Several retries fail while it is gone; none publishes again.
        std::thread::sleep(Duration::from_millis(200));
        while let Ok(event) = rig.events.try_recv() {
            assert!(
                !matches!(event, ConnectivityEvent::Network(_)),
                "republished {event:?}"
            );
        }
        // While unavailable, a request is refused for lack of an inventory.
        rig.handle
            .send(Action::SetWifiEnabled(false))
            .expect("queued");
        rig.expect("refusal", |e| {
            *e == ConnectivityEvent::Refused(
                Action::SetWifiEnabled(false),
                Refusal::NoInventory(Section::Network),
            )
        });

        rig.net.lock().expect("script").gone = false;
        rig.expect("recovery", |e| {
            matches!(e, ConnectivityEvent::Network(Service::Ready(_)))
        });
        assert!(rig.handle.stop());
    }

    #[test]
    fn an_absent_service_at_start_is_unavailable_until_it_appears() {
        let rig = Rig::start(true);
        rig.expect("unavailable", |e| {
            *e == ConnectivityEvent::Network(Service::Unavailable)
        });
        rig.net.lock().expect("script").gone = false;
        rig.expect("ready", |e| {
            matches!(e, ConnectivityEvent::Network(Service::Ready(_)))
        });
        assert!(rig.handle.stop());
    }

    fn snapshots(script: &network::tests::Script) -> usize {
        script
            .lock()
            .expect("script")
            .calls
            .iter()
            .filter(|call| **call == NetCall::Snapshot)
            .count()
    }

    #[test]
    fn failed_retries_start_no_watcher_and_a_lane_keeps_one_for_life() {
        let net = Arc::new(Mutex::new(NetScript {
            unreadable: true,
            ..NetScript::default()
        }));
        let (network, watchers) = network::tests::recording(&net);
        let (sender, events) = mpsc::channel();
        let handle = ConnectivityHandle::start_with(
            Providers {
                network: Some(network),
                bluetooth: None,
                power: None,
            },
            Arc::new(move |event| sender.send(event).is_ok()),
            fast(),
        );
        let wait_for = |what: &str, wanted: &dyn Fn(&ConnectivityEvent) -> bool| {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match events.recv_timeout(left) {
                    Ok(event) if wanted(&event) => return,
                    Ok(_) => {}
                    Err(_) => panic!("no {what} within {WAIT:?}"),
                }
            }
        };
        let retries = |at_least: usize| {
            let deadline = Instant::now() + WAIT;
            while snapshots(&net) < at_least {
                assert!(Instant::now() < deadline, "the lane stopped retrying");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let started = || *watchers.lock().expect("counter");

        // The client opens, its reads fail: retry after retry, no watcher.
        wait_for("unavailable", &|e| {
            *e == ConnectivityEvent::Network(Service::Unavailable)
        });
        retries(5);
        assert_eq!(started(), 0, "a watcher per failed retry leaks");

        // The service answers: one watcher.
        net.lock().expect("script").unreadable = false;
        wait_for("ready", &|e| {
            matches!(e, ConnectivityEvent::Network(Service::Ready(_)))
        });
        assert_eq!(started(), 1);

        // An outage and a recovery keep that one.
        net.lock().expect("script").unreadable = true;
        let before = snapshots(&net);
        handle
            .send(Action::Refresh(Section::Network))
            .expect("queued");
        wait_for("unavailable again", &|e| {
            *e == ConnectivityEvent::Network(Service::Unavailable)
        });
        retries(before + 4);
        net.lock().expect("script").unreadable = false;
        wait_for("ready again", &|e| {
            matches!(e, ConnectivityEvent::Network(Service::Ready(_)))
        });
        assert_eq!(started(), 1, "one watcher for the lane's life");
        assert!(handle.stop());
    }

    #[test]
    fn a_join_that_fails_after_connecting_fails_its_row() {
        let rig = Rig::start(false);
        let mut state = state_of(&rig);
        rig.net.lock().expect("script").force_state = Some((
            "home-wifi".to_owned(),
            cuprita_core::model::NetworkState::Connecting,
        ));
        let expected = Expected::NetActive {
            id: id("home-wifi"),
            active: true,
        };
        assert_eq!(
            forward(
                &mut state,
                Some(&rig.handle),
                Action::JoinSaved(id("home-wifi"))
            ),
            Forwarded::Sent
        );
        let take = |state: &mut ConnectivityState,
                    what: &str,
                    wanted: &dyn Fn(&ConnectivityEvent) -> bool| {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match rig.events.recv_timeout(left) {
                    Ok(event) => {
                        state.apply(&event);
                        if wanted(&event) {
                            return;
                        }
                    }
                    Err(_) => panic!("no {what} within {WAIT:?}"),
                }
            }
        };
        // NetworkManager takes the activation: the join is confirmed.
        take(&mut state, "the confirmation", &|e| {
            *e == ConnectivityEvent::Resolved(Resolution::Confirmed(expected.clone()))
        });
        assert_eq!(state.net_status(&id("home-wifi")), state::RowStatus::Idle);

        // Then it gives up on it: no secret. The row says so.
        rig.net.lock().expect("script").force_state = Some((
            "home-wifi".to_owned(),
            cuprita_core::model::NetworkState::Failed,
        ));
        take(&mut state, "the failure", &|e| {
            *e == ConnectivityEvent::Resolved(Resolution::Failed(
                expected.clone(),
                Failure::NeedsSecret,
            ))
        });
        assert_eq!(
            state.net_status(&id("home-wifi")),
            state::RowStatus::Failed(Failure::NeedsSecret)
        );
        assert!(rig.handle.stop());
    }

    #[test]
    fn a_lane_sleeps_until_its_next_clock_and_at_most_a_second() {
        let net = Arc::new(Mutex::new(NetScript::default()));
        let (network, _) = network::tests::recording(&net);
        let timing = Timing::default();
        assert_eq!(timing.poll, Duration::from_secs(1));
        let mut lane = Lane::new(network, Arc::new(|_| true), timing);
        let now = Instant::now();
        let ms = Duration::from_millis;

        // Closed: until the retry.
        lane.retry_at = now;
        assert_eq!(lane.wait(now), Duration::ZERO);
        lane.retry_at = now + ms(300);
        assert_eq!(lane.wait(now), ms(300));
        lane.retry_at = now + timing.backoff_max;
        assert_eq!(lane.wait(now), timing.poll);

        // Open at rest: a second, not the next re-read 30 s away.
        lane.open = true;
        lane.next_read = now + timing.reread;
        assert_eq!(lane.wait(now), timing.poll);
        lane.next_read = now + ms(40);
        assert_eq!(lane.wait(now), ms(40));

        // A pending window that ends sooner wakes it then.
        lane.next_read = now + timing.reread;
        let since = now
            .checked_sub(celestina_shell::connectivity::PENDING_WINDOW - ms(200))
            .expect("a clock this old");
        lane.pending.start(Expected::WifiEnabled(false), since);
        assert_eq!(lane.wait(now), ms(200));
    }

    #[test]
    fn a_lane_with_a_reread_asked_for_does_not_sleep() {
        let net = Arc::new(Mutex::new(NetScript::default()));
        let (network, _) = network::tests::recording(&net);
        let mut lane = Lane::new(network, Arc::new(|_| true), Timing::default());
        let now = Instant::now();
        lane.next_read = now + Duration::from_secs(30);
        lane.dirty.store(true, Ordering::Release);
        // Closed, a raised flag waits for the retry, as before.
        lane.retry_at = now + Duration::from_millis(300);
        assert_eq!(lane.wait(now), Duration::from_millis(300));
        lane.open = true;
        assert_eq!(lane.wait(now), Duration::ZERO);
        lane.dirty.store(false, Ordering::Release);
        assert_eq!(lane.wait(now), Timing::default().poll);
    }

    #[test]
    fn a_slow_read_or_open_schedules_from_when_it_returned() {
        // Review C #1: a read slower than the pending re-read used to leave
        // the next read already due, and the lane re-read with no sleep.
        let delay = Duration::from_millis(120);
        let net = Arc::new(Mutex::new(NetScript {
            delay,
            ..NetScript::default()
        }));
        let (network, _) = network::tests::recording(&net);
        let timing = fast();
        assert!(delay > timing.pending_reread && delay > timing.backoff_first);
        let mut lane = Lane::new(network, Arc::new(|_| true), timing);

        // An open that fails slowly: the retry counts from its end.
        net.lock().expect("script").gone = true;
        let before = Instant::now();
        lane.try_open();
        assert!(!lane.open);
        assert!(lane.retry_at >= before + delay + timing.backoff_first);
        assert!(lane.wait(Instant::now()) > Duration::ZERO);

        // A slow read with a request pending: the 50 ms re-read counts from
        // the read's end, so the lane sleeps before the next one.
        net.lock().expect("script").gone = false;
        lane.try_open();
        assert!(lane.open);
        lane.pending
            .start(Expected::WifiEnabled(false), Instant::now());
        lane.dirty.store(false, Ordering::Release);
        let before = Instant::now();
        lane.read();
        assert!(lane.next_read >= before + delay + timing.pending_reread);
        lane.dirty.store(false, Ordering::Release);
        assert!(lane.wait(Instant::now()) > Duration::ZERO);
    }

    #[test]
    fn stopping_stuck_workers_costs_one_shared_bound() {
        let net = Arc::new(Mutex::new(NetScript::default()));
        let bt = Arc::new(Mutex::new(BtScript::default()));
        let (network, _) = network::tests::recording(&net);
        let (entered, inside) = mpsc::channel();
        // Every publish blocks well past the bound: both lanes are stuck.
        let publish: Publish = Arc::new(move |_event| {
            let _ = entered.send(());
            std::thread::sleep(JOIN_WAIT * 3);
            true
        });
        let handle = ConnectivityHandle::start_with(
            Providers {
                network: Some(network),
                bluetooth: Some(bluetooth::tests::recording(&bt)),
                power: None,
            },
            publish,
            fast(),
        );
        for _ in 0..2 {
            inside.recv_timeout(WAIT).expect("both lanes publish");
        }

        let started = Instant::now();
        assert!(!handle.stop(), "stuck workers are reported");
        let took = started.elapsed();
        assert!(
            took < JOIN_WAIT + Duration::from_millis(500),
            "two stuck lanes took {took:?}"
        );
    }

    #[test]
    fn a_full_queue_is_refused_without_waiting() {
        let net = Arc::new(Mutex::new(NetScript::default()));
        let (network, _) = network::tests::recording(&net);
        let (entered, inside) = mpsc::channel();
        let (release, gate) = mpsc::channel::<()>();
        let gate = Mutex::new(Some(gate));
        // The first publish blocks the lane, so nothing drains its queue.
        let publish: Publish = Arc::new(move |_event| {
            let held = gate.lock().expect("gate").take();
            if let Some(gate) = held {
                let _ = entered.send(());
                let _ = gate.recv_timeout(WAIT);
            }
            true
        });
        let handle = ConnectivityHandle::start_with(
            Providers {
                network: Some(network),
                bluetooth: None,
                power: None,
            },
            publish,
            fast(),
        );
        inside.recv_timeout(WAIT).expect("the lane publishes");

        for _ in 0..QUEUE {
            handle
                .send(Action::Refresh(Section::Network))
                .expect("room in the queue");
        }
        let started = Instant::now();
        assert_eq!(
            handle.send(Action::Refresh(Section::Network)),
            Err(RequestError::Full(Destination::Network))
        );
        assert!(started.elapsed() < Duration::from_millis(100));

        release.send(()).expect("release");
        assert!(handle.stop());
    }

    #[test]
    fn actions_without_a_running_worker_are_refused() {
        let handle =
            ConnectivityHandle::start_with(Providers::default(), Arc::new(|_| true), fast());
        assert_eq!(
            handle.send(Action::OpenEditor(Editor::Network)),
            Err(RequestError::NotForWorkers)
        );
        assert_eq!(
            handle.send(Action::CycleProfile),
            Err(RequestError::NotStarted(Destination::Power))
        );
        assert_eq!(
            handle.send(Action::Refresh(Section::Battery)),
            Err(RequestError::NotStarted(Destination::Power))
        );
        assert_eq!(
            handle.send(Action::SetWifiEnabled(true)),
            Err(RequestError::NotStarted(Destination::Network))
        );
        assert!(handle.stop());
    }

    #[test]
    fn stopping_joins_every_lane_within_its_bound() {
        let rig = Rig::start(false);
        rig.ready();
        let started = Instant::now();
        assert!(rig.handle.stop());
        assert!(started.elapsed() < JOIN_WAIT);
    }

    #[test]
    fn a_lane_stops_when_nobody_listens() {
        let net = Arc::new(Mutex::new(NetScript::default()));
        let (network, _) = network::tests::recording(&net);
        let handle = ConnectivityHandle::start_with(
            Providers {
                network: Some(network),
                bluetooth: None,
                power: None,
            },
            Arc::new(|_| false),
            fast(),
        );
        let deadline = Instant::now() + WAIT;
        while handle.send(Action::Refresh(Section::Network))
            != Err(RequestError::Stopped(Destination::Network))
        {
            assert!(Instant::now() < deadline, "the lane kept running");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(handle.stop());
    }

    /// A state fed with the rig's first network and Bluetooth news, as the
    /// loop's would be.
    fn state_of(rig: &Rig) -> ConnectivityState {
        let mut state = ConnectivityState::default();
        let deadline = Instant::now() + WAIT;
        while state.network().ready().is_none() || state.bluetooth().ready().is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            let event = rig.events.recv_timeout(left).expect("first states");
            state.apply(&event);
        }
        state
    }

    #[test]
    fn a_request_the_state_refuses_never_reaches_a_worker() {
        let rig = Rig::start(false);
        let mut state = state_of(&rig);
        let (net_before, bt_before) = (rig.net_calls().len(), rig.bt_calls().len());
        let refused = [
            (Action::JoinSaved(id("nowhere")), Refusal::UnknownId),
            (Action::JoinSaved(id("office-vpn")), Refusal::KindNotAllowed),
            (Action::JoinSaved(id("open-cafe")), Refusal::NotKnown),
            (Action::Leave(id("home-wifi")), Refusal::NotActive),
            (
                Action::ConnectDevice(address("AA:BB:CC:00:00:02")),
                Refusal::NotPaired,
            ),
            (
                Action::DisconnectDevice(address("AA:BB:CC:00:00:99")),
                Refusal::UnknownId,
            ),
            // Nothing is known of the power services yet.
            (
                Action::CycleProfile,
                Refusal::NoInventory(Section::Profiles),
            ),
        ];
        for (action, refusal) in refused {
            assert_eq!(
                forward(&mut state, Some(&rig.handle), action.clone()),
                Forwarded::Refused(refusal),
                "{action:?}"
            );
        }
        // An empty state refuses before any inventory exists.
        assert_eq!(
            forward(
                &mut ConnectivityState::default(),
                Some(&rig.handle),
                Action::SetWifiEnabled(false)
            ),
            Forwarded::Refused(Refusal::NoInventory(Section::Network))
        );
        // A request that is accepted goes through, after all of them.
        assert_eq!(
            forward(
                &mut state,
                Some(&rig.handle),
                Action::JoinSaved(id("home-wifi"))
            ),
            Forwarded::Sent
        );
        assert_eq!(
            state.net_status(&id("home-wifi")),
            state::RowStatus::Pending
        );
        // The lanes saw none of the refused ones: no refusal of theirs up to
        // the accepted row's confirmation, and their clients only read, then
        // joined that one row.
        rig.expect("the confirmed join", |e| {
            assert!(!matches!(e, ConnectivityEvent::Refused(..)), "{e:?}");
            matches!(
                e,
                ConnectivityEvent::Resolved(Resolution::Confirmed(Expected::NetActive { .. }))
            )
        });
        let net: Vec<NetCall> = rig.net_calls()[net_before..]
            .iter()
            .filter(|call| **call != NetCall::Snapshot)
            .cloned()
            .collect();
        assert_eq!(
            net,
            [NetCall::Connect {
                id: "home-wifi".to_owned(),
                secret: false,
            }]
        );
        assert!(rig.bt_calls()[bt_before..]
            .iter()
            .all(|call| *call == BtCall::Snapshot));
        assert!(rig.handle.stop());
    }

    #[test]
    fn an_editor_is_the_callers_and_an_untaken_request_fails_its_row() {
        let mut state = state::sample::ready();
        let nothing =
            ConnectivityHandle::start_with(Providers::default(), Arc::new(|_| true), fast());
        assert_eq!(
            forward(
                &mut state,
                Some(&nothing),
                Action::OpenEditor(Editor::Network)
            ),
            Forwarded::Editor(Editor::Network)
        );
        assert_eq!(
            forward(&mut state, Some(&nothing), Action::SetWifiEnabled(false)),
            Forwarded::Undelivered(RequestError::NotStarted(Destination::Network))
        );
        assert_eq!(
            state.wifi_status(),
            state::RowStatus::Failed(Failure::Unreachable)
        );
        // No handle at all (the loop has not started it): the same.
        assert_eq!(
            forward(&mut state, None, Action::CycleProfile),
            Forwarded::Undelivered(RequestError::NotStarted(Destination::Power))
        );
        assert_eq!(
            undelivered(RequestError::Full(Destination::Power)),
            Failure::Busy
        );
        assert_eq!(
            undelivered(RequestError::Refused(Destination::Power)),
            Failure::Busy
        );
        assert!(nothing.stop());
    }

    fn fake_bus(name: &str) -> Option<celestina_fake_system::FakeSystem> {
        match celestina_fake_system::FakeSystem::start(name, celestina_fake_system::Services::all())
        {
            Ok(system) => Some(system),
            Err(celestina_fake_system::FakeError::DaemonMissing) => {
                eprintln!("dbus-daemon is not installed: the test is skipped");
                None
            }
            Err(error) => panic!("{error}"),
        }
    }

    fn fast_power(system: &celestina_fake_system::FakeSystem) -> PowerSetup {
        PowerSetup {
            bus: power::Bus::Address(system.address().to_owned()),
            timing: power::Timing {
                poll: Duration::from_millis(40),
                retry_start: Duration::from_millis(40),
                retry_max: Duration::from_millis(160),
            },
        }
    }

    #[test]
    fn power_requests_reach_the_power_worker_and_its_news_comes_wrapped() {
        let Some(system) = fake_bus("conn-power") else {
            return;
        };
        let (sender, events) = mpsc::channel();
        let handle = ConnectivityHandle::start_with(
            Providers {
                power: Some(fast_power(&system)),
                ..Providers::default()
            },
            Arc::new(move |event| sender.send(event).is_ok()),
            fast(),
        );
        let wait_for = |what: &str, wanted: &dyn Fn(&ConnectivityEvent) -> bool| {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match events.recv_timeout(left) {
                    Ok(event) if wanted(&event) => return event,
                    Ok(_) => {}
                    Err(_) => panic!("no {what} within {WAIT:?}"),
                }
            }
        };
        wait_for("profiles", &|e| {
            matches!(
                e,
                ConnectivityEvent::Power(PowerEvent::Profiles(Service::Ready(_)))
            )
        });
        let performance =
            celestina_shell::connectivity::ProfileName::new("performance").expect("profile");
        handle
            .send(Action::SetProfile(performance.clone()))
            .expect("queued");
        wait_for("the confirmation", &|e| {
            *e == ConnectivityEvent::Power(PowerEvent::Resolved(Resolution::Confirmed(
                Expected::Profile(performance.clone()),
            )))
        });
        assert_eq!(system.calls().all(), ["ppd:Set ActiveProfile=performance"]);
        let started = Instant::now();
        assert!(handle.stop());
        assert!(started.elapsed() < JOIN_WAIT);
    }

    #[test]
    fn stopping_joins_all_three_workers_within_the_bound() {
        let Some(system) = fake_bus("conn-stop") else {
            return;
        };
        let net = Arc::new(Mutex::new(NetScript::default()));
        let bt = Arc::new(Mutex::new(BtScript::default()));
        let (network, _) = network::tests::recording(&net);
        let (sender, events) = mpsc::channel();
        let handle = ConnectivityHandle::start_with(
            Providers {
                network: Some(network),
                bluetooth: Some(bluetooth::tests::recording(&bt)),
                power: Some(fast_power(&system)),
            },
            Arc::new(move |event| sender.send(event).is_ok()),
            fast(),
        );
        let (mut seen_net, mut seen_bt, mut seen_power) = (false, false, false);
        let deadline = Instant::now() + WAIT;
        while !(seen_net && seen_bt && seen_power) {
            let left = deadline.saturating_duration_since(Instant::now());
            match events.recv_timeout(left).expect("every worker publishes") {
                ConnectivityEvent::Network(_) => seen_net = true,
                ConnectivityEvent::Bluetooth(_) => seen_bt = true,
                ConnectivityEvent::Power(_) => seen_power = true,
                _ => {}
            }
        }
        assert_eq!(
            format!("{handle:?}"),
            "ConnectivityHandle { network: true, bluetooth: true, power: true, .. }"
        );
        let started = Instant::now();
        assert!(handle.stop(), "every worker stopped in time");
        assert!(started.elapsed() < JOIN_WAIT);
        // Every worker is gone and has let go of its publisher: once what
        // they sent is read, the channel is closed.
        let _ = events.try_iter().count();
        assert_eq!(events.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }

    #[test]
    fn every_action_has_one_destination() {
        assert_eq!(
            destination(&Action::Refresh(Section::Battery)),
            Destination::Power
        );
        assert_eq!(
            destination(&Action::Refresh(Section::Profiles)),
            Destination::Power
        );
        assert_eq!(
            destination(&Action::OpenEditor(Editor::Bluetooth)),
            Destination::Editor
        );
        assert_eq!(
            destination(&Action::DisconnectDevice(address("AA:BB:CC:00:00:01"))),
            Destination::Bluetooth
        );
        assert_eq!(destination(&Action::Leave(id("x"))), Destination::Network);
    }

    #[test]
    fn the_journal_names_the_first_answer_of_each_power_service_only() {
        let mut announced = [false; 2];
        let battery = PowerEvent::Battery(Service::Unavailable);
        let profiles = PowerEvent::Profiles(Service::Unavailable);
        assert_eq!(
            first_power_answer(&battery, &mut announced),
            Some(("battery", "unavailable"))
        );
        assert_eq!(first_power_answer(&battery, &mut announced), None);
        assert_eq!(
            first_power_answer(&profiles, &mut announced),
            Some(("profiles", "unavailable"))
        );
        assert_eq!(first_power_answer(&profiles, &mut announced), None);
    }
}
