// Written for the shell's energy duties (CEL-2-B, T4), following the shape
// of `nightlight.rs`: its own Wayland connection, a `Transport` seam so the
// controller is tested without a display, a bounded command queue, and
// bounded waits so the worker's join is bounded. The connection, its pump
// and the command queue's refusal rule are the shared `wire.rs`.

//! The idle duty: one worker that owns its own Wayland connection, binds
//! `wl_seat` and `ext_idle_notifier_v1`, and keeps two plain idle
//! notifications alive while the duty is on.
//!
//! The decisions live in [`celestina_shell::idle::IdleChain`]; this module
//! only transports them:
//!
//! - the notifications are plain (`get_idle_notification`), never the
//!   input-only kind, so another client's idle inhibitor is honoured by the
//!   compositor before `idled` is ever sent;
//! - while the duty is off no protocol object exists: the notifications are
//!   destroyed on `set_enabled(false)` and created again on `true`;
//! - `set_config` destroys both notifications and creates them with the new
//!   delays, and the chain starts from a clean slate;
//! - every [`IdleAction`] the chain returns goes to the `perform` closure
//!   given to [`spawn`]; this module never launches a process and never
//!   talks to the compositor's own IPC;
//! - a missing connection or global reports [`IdleEvent::Unavailable`] and
//!   the worker does nothing more until it stops.
//!
//! Commands cross into the worker through a queue of
//! [`COMMAND_QUEUE`](super::wire::COMMAND_QUEUE). Every wait is bounded, so
//! [`crate::providers::Worker::join`] is too.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::idle::{IdleAction, IdleChain, IdleConfig, Refusal, Step};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1 as notification_proto, ext_idle_notifier_v1 as notifier_proto,
};

use super::wire::{self, Link, QueueRefusal, Wire, COMMAND_QUEUE, POLL, SYNC_TIMEOUT, UNAVAILABLE};
use crate::providers::Worker;

/// How many protocol events one dispatch may carry before the compositor is
/// treated as hostile.
const MAX_EVENTS: usize = 256;

/// What the idle duty reports to the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdleEvent {
    /// The two idle notifications exist; the chain is listening.
    Armed,
    /// The duty cannot work in this session; it does nothing.
    Unavailable(String),
    /// The chain refused to act on a step, with the reason.
    Refused(String),
}

/// Why a handle method refused a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IdleRefused {
    /// [`COMMAND_QUEUE`] commands are already waiting.
    #[error("the idle worker is still applying earlier commands")]
    Busy,
    /// The worker has stopped.
    #[error("the idle worker has stopped")]
    Gone,
}

impl QueueRefusal for IdleRefused {
    const BUSY: Self = Self::Busy;
    const GONE: Self = Self::Gone;
}

/// What the loop can ask of the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Enabled(bool),
    Caffeine(bool),
    Config(IdleConfig),
    Locker(bool),
}

/// The loop's side of the idle worker.
#[derive(Clone, Debug)]
pub struct IdleHandle {
    commands: SyncSender<Command>,
}

impl IdleHandle {
    /// Turns the duty on or off. Off destroys the notifications.
    ///
    /// # Errors
    ///
    /// [`IdleRefused::Busy`] when the queue is full, [`IdleRefused::Gone`]
    /// when the worker has stopped.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), IdleRefused> {
        self.send(Command::Enabled(enabled))
    }

    /// Holds (or releases) the chain: while on, no step acts.
    ///
    /// # Errors
    ///
    /// As [`IdleHandle::set_enabled`].
    pub fn set_caffeine(&self, caffeine: bool) -> Result<(), IdleRefused> {
        self.send(Command::Caffeine(caffeine))
    }

    /// Replaces the delays; the notifications are rebuilt with them.
    ///
    /// # Errors
    ///
    /// As [`IdleHandle::set_enabled`].
    pub fn set_config(&self, config: IdleConfig) -> Result<(), IdleRefused> {
        self.send(Command::Config(config))
    }

    /// Tells the chain whether a locker exists to lock before suspending.
    ///
    /// # Errors
    ///
    /// As [`IdleHandle::set_enabled`].
    pub fn set_locker_available(&self, available: bool) -> Result<(), IdleRefused> {
        self.send(Command::Locker(available))
    }

    fn send(&self, command: Command) -> Result<(), IdleRefused> {
        wire::offer(&self.commands, command)
    }
}

/// Starts the idle worker.
///
/// `publish` receives every [`IdleEvent`] and returns whether the loop still
/// listens; once it returns `false` the worker releases its notifications
/// and stops. `perform` receives every [`IdleAction`] the chain decides, on
/// the worker thread; it must not block for long. The worker also stops when
/// `shutdown` is raised or every handle is dropped.
///
/// The chain starts disabled, without a locker and with
/// [`IdleConfig::default`]; the caller sends the real state right after.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    publish: impl FnMut(IdleEvent) -> bool + Send + 'static,
    perform: impl FnMut(IdleAction) + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(IdleHandle, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let worker_shutdown = Arc::clone(shutdown);
    let worker = Worker::spawn("idle", shutdown, move || {
        run(
            WaylandTransport::connect(),
            &queue,
            Sink::new(publish, perform),
            &worker_shutdown,
            POLL,
        );
    })?;
    Ok((IdleHandle { commands }, worker))
}

/// The connection itself is gone, cannot be trusted, or never offered the
/// duty.
#[derive(Debug, thiserror::Error)]
enum Lost {
    #[error("the compositor offers no seat")]
    NoSeat,
    #[error("the compositor offers no ext-idle-notify")]
    NoNotifier,
    #[error("the compositor withdrew the seat or the idle notifier")]
    GlobalGone,
    #[error(transparent)]
    Wire(#[from] wire::Lost),
}

/// What the protocol reports, reduced to what the duty needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signal {
    Idled(Step),
    Resumed(Step),
}

/// The protocol seam: the two notifications, addressed by step.
trait Transport {
    /// Destroys any live notification and creates one per step with its
    /// timeout in milliseconds. Requests are queued until
    /// [`Transport::flush`].
    fn arm(&mut self, steps: [(Step, u32); 2]) -> Result<(), Lost>;
    /// Destroys every notification; none exists afterwards.
    fn disarm(&mut self);
    fn flush(&mut self) -> Result<(), Lost>;
    /// Waits at most `timeout` for events and returns them.
    fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost>;
}

/// A delay as the protocol's millisecond timeout.
fn timeout_ms(delay: Duration) -> u32 {
    u32::try_from(delay.as_millis()).unwrap_or(u32::MAX)
}

/// The chain plus the notifications it needs, over a [`Transport`].
struct Duty<T: Transport> {
    transport: T,
    chain: IdleChain,
    armed: bool,
}

impl<T: Transport> Duty<T> {
    fn new(transport: T) -> Self {
        Self {
            transport,
            chain: IdleChain::new(IdleConfig::default()),
            armed: false,
        }
    }

    /// Applies `command` to the chain; the actions it decides are returned.
    /// Call [`Duty::reconcile`] afterwards.
    fn command(&mut self, command: Command) -> Vec<IdleAction> {
        match command {
            Command::Enabled(enabled) => self.chain.set_enabled(enabled),
            Command::Caffeine(caffeine) => self.chain.set_caffeine(caffeine),
            Command::Locker(available) => self.chain.set_locker_available(available),
            Command::Config(config) => {
                // New delays mean new notifications and no memory of the old
                // ones: a fresh chain carrying the current switches.
                let (enabled, caffeine, locker) = (
                    self.chain.enabled(),
                    self.chain.caffeine(),
                    self.chain.locker_available(),
                );
                self.chain = IdleChain::new(config);
                let mut actions = self.chain.set_enabled(enabled);
                actions.extend(self.chain.set_caffeine(caffeine));
                actions.extend(self.chain.set_locker_available(locker));
                if self.armed {
                    // Rebuilt by `reconcile` with the new delays.
                    self.transport.disarm();
                    self.armed = false;
                }
                actions
            }
        }
    }

    fn signal(&mut self, signal: Signal) -> Vec<IdleAction> {
        match signal {
            Signal::Idled(step) => self.chain.idled(step),
            Signal::Resumed(step) => self.chain.resumed(step),
        }
    }

    /// Makes the notifications match the duty: present while it is on,
    /// absent while it is off.
    fn reconcile(&mut self) -> Result<(), Lost> {
        let wanted = self.chain.enabled();
        if wanted && !self.armed {
            let steps = self
                .chain
                .steps()
                .map(|(step, delay)| (step, timeout_ms(delay)));
            self.transport.arm(steps)?;
            self.transport.flush()?;
            self.armed = true;
            return Ok(());
        }
        if !wanted && self.armed {
            self.release();
            self.transport.flush()?;
        }
        Ok(())
    }

    /// Destroys the notifications and forgets what they said: a `resumed`
    /// that can no longer arrive must not leave a step marked handled.
    fn release(&mut self) {
        self.transport.disarm();
        self.armed = false;
        // The chain's reset on resume, not an action: it returns none.
        let _ = self.chain.resumed(Step::ScreenOff);
    }
}

/// Where the duty's outputs go.
struct Sink<P, F> {
    publish: P,
    perform: F,
    told_armed: bool,
    told_unavailable: bool,
}

impl<P: FnMut(IdleEvent) -> bool, F: FnMut(IdleAction)> Sink<P, F> {
    fn new(publish: P, perform: F) -> Self {
        Self {
            publish,
            perform,
            told_armed: false,
            told_unavailable: false,
        }
    }

    /// Performs `actions` in order and reports each refusal. Returns whether
    /// the loop still listens.
    fn actions(&mut self, actions: Vec<IdleAction>) -> bool {
        let mut listening = true;
        for action in actions {
            if let IdleAction::Refused(refusal) = action {
                listening &= (self.publish)(IdleEvent::Refused(refusal_text(refusal)));
            }
            (self.perform)(action);
        }
        listening
    }

    /// Reports `Armed` once per arming.
    fn armed(&mut self, armed: bool) -> bool {
        if armed == self.told_armed {
            return true;
        }
        self.told_armed = armed;
        !armed || (self.publish)(IdleEvent::Armed)
    }

    /// Reports `Unavailable` once.
    fn unavailable(&mut self, reason: String) -> bool {
        if self.told_unavailable {
            return true;
        }
        self.told_unavailable = true;
        (self.publish)(IdleEvent::Unavailable(reason))
    }
}

fn refusal_text(refusal: Refusal) -> String {
    match refusal {
        Refusal::NoLocker => "no locker is available to lock before suspending".to_owned(),
    }
}

/// Why [`serve`] returned without losing the connection.
#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Shutdown,
    LoopGone,
    HandleGone,
}

/// The worker's loop over a connected transport.
fn serve<T: Transport, P: FnMut(IdleEvent) -> bool, F: FnMut(IdleAction)>(
    duty: &mut Duty<T>,
    commands: &Receiver<Command>,
    sink: &mut Sink<P, F>,
    shutdown: &AtomicBool,
    poll_every: Duration,
) -> Result<Exit, Lost> {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return Ok(Exit::Shutdown);
        }
        let mut handle_gone = false;
        // At most one queue's worth per round, so a chatty handle cannot
        // starve the connection.
        for _ in 0..=COMMAND_QUEUE {
            match commands.try_recv() {
                Ok(command) => {
                    let actions = duty.command(command);
                    if !sink.actions(actions) {
                        return Ok(Exit::LoopGone);
                    }
                    duty.reconcile()?;
                    if !sink.armed(duty.armed) {
                        return Ok(Exit::LoopGone);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    handle_gone = true;
                    break;
                }
            }
        }
        if handle_gone {
            return Ok(Exit::HandleGone);
        }
        for signal in duty.transport.poll(poll_every)? {
            let actions = duty.signal(signal);
            if !sink.actions(actions) {
                return Ok(Exit::LoopGone);
            }
        }
    }
}

/// Answers nothing until shutdown: the duty is unavailable, so every command
/// is a no-op.
fn idle_unavailable(commands: &Receiver<Command>, shutdown: &AtomicBool) {
    while !shutdown.load(Ordering::Acquire) {
        match commands.recv_timeout(POLL) {
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn run<T: Transport, P: FnMut(IdleEvent) -> bool, F: FnMut(IdleAction)>(
    connected: Result<T, Lost>,
    commands: &Receiver<Command>,
    mut sink: Sink<P, F>,
    shutdown: &AtomicBool,
    poll_every: Duration,
) {
    let transport = match connected {
        Ok(transport) => transport,
        Err(lost) => {
            let reason = crate::error_chain(&lost);
            tracing::warn!("idle: {reason}");
            if sink.unavailable(UNAVAILABLE.to_owned()) {
                idle_unavailable(commands, shutdown);
            }
            return;
        }
    };
    let mut duty = Duty::new(transport);
    match serve(&mut duty, commands, &mut sink, shutdown, poll_every) {
        Ok(_) => {
            duty.release();
            let _ = duty.transport.flush();
        }
        Err(lost) => {
            let reason = crate::error_chain(&lost);
            tracing::warn!("idle: {reason}");
            duty.release();
            let _ = duty.transport.flush();
            if sink.unavailable(reason) {
                idle_unavailable(commands, shutdown);
            }
        }
    }
}

// The Wayland transport.

struct WaylandState {
    seat: Option<(u32, wl_seat::WlSeat)>,
    notifier: Option<(u32, notifier_proto::ExtIdleNotifierV1)>,
    notifications: Vec<notification_proto::ExtIdleNotificationV1>,
    /// Identifies the live set of notifications; events of an older set are
    /// stale and ignored.
    generation: u64,
    gone: bool,
}

/// What the idle connection dispatches into.
type IdleWire = Wire<WaylandState, Signal>;

/// The idle duty's own reason to stop pumping: a global it needs is gone.
fn intact(state: &WaylandState) -> Result<(), Lost> {
    if state.gone {
        return Err(Lost::GlobalGone);
    }
    Ok(())
}

struct WaylandTransport {
    link: Link<WaylandState, Signal>,
}

impl WaylandTransport {
    /// Opens the worker's own connection and discovers the seat and the
    /// idle notifier.
    fn connect() -> Result<Self, Lost> {
        let state = WaylandState {
            seat: None,
            notifier: None,
            notifications: Vec::new(),
            generation: 0,
            gone: false,
        };
        let mut link = Link::connect(state, MAX_EVENTS)?;
        link.sync(Instant::now() + SYNC_TIMEOUT, intact)?;
        if link.wire.state.seat.is_none() {
            return Err(Lost::NoSeat);
        }
        if link.wire.state.notifier.is_none() {
            return Err(Lost::NoNotifier);
        }
        Ok(Self { link })
    }
}

impl Transport for WaylandTransport {
    fn arm(&mut self, steps: [(Step, u32); 2]) -> Result<(), Lost> {
        self.disarm();
        let Link { qh, wire, .. } = &mut self.link;
        let state = &mut wire.state;
        let (Some((_, seat)), Some((_, notifier))) = (state.seat.as_ref(), state.notifier.as_ref())
        else {
            return Err(Lost::GlobalGone);
        };
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        let created: Vec<_> = steps
            .into_iter()
            .map(|(step, timeout)| {
                // The plain notification, not the input-only one: other
                // clients' inhibitors are honoured.
                notifier.get_idle_notification(timeout, seat, qh, (step, generation))
            })
            .collect();
        state.notifications = created;
        Ok(())
    }

    fn disarm(&mut self) {
        let wire = &mut self.link.wire;
        // Older events still in flight are stale from here on.
        wire.state.generation = wire.state.generation.wrapping_add(1);
        wire.clear_events();
        for notification in wire.state.notifications.drain(..) {
            notification.destroy();
        }
    }

    fn flush(&mut self) -> Result<(), Lost> {
        Ok(self.link.flush()?)
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost> {
        self.link.pump(timeout, intact)?;
        Ok(self.link.wire.take_events())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for IdleWire {
    fn event(
        wire: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let state = &mut wire.state;
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_seat::WlSeat::interface().name && state.seat.is_none() => {
                let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(1), qh, ());
                state.seat = Some((name, seat));
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == notifier_proto::ExtIdleNotifierV1::interface().name
                && state.notifier.is_none() =>
            {
                let notifier = registry.bind::<notifier_proto::ExtIdleNotifierV1, _, _>(
                    name,
                    version.min(1),
                    qh,
                    (),
                );
                state.notifier = Some((name, notifier));
            }
            wl_registry::Event::GlobalRemove { name }
                if state
                    .seat
                    .as_ref()
                    .is_some_and(|(global, _)| *global == name)
                    || state
                        .notifier
                        .as_ref()
                        .is_some_and(|(global, _)| *global == name) =>
            {
                state.gone = true;
            }
            _ => {}
        }
    }
}

delegate_noop!(IdleWire: ignore wl_seat::WlSeat);
delegate_noop!(IdleWire: notifier_proto::ExtIdleNotifierV1);

impl Dispatch<notification_proto::ExtIdleNotificationV1, (Step, u64)> for IdleWire {
    fn event(
        wire: &mut Self,
        _: &notification_proto::ExtIdleNotificationV1,
        event: notification_proto::Event,
        (step, generation): &(Step, u64),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if *generation != wire.state.generation {
            return;
        }
        match event {
            notification_proto::Event::Idled => wire.push(Signal::Idled(*step)),
            notification_proto::Event::Resumed => wire.push(Signal::Resumed(*step)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use std::thread;

    #[derive(Default)]
    struct Shared {
        /// The notifications alive now, with their timeouts.
        live: Vec<(Step, u32)>,
        /// Every `arm` ever made.
        arms: Vec<[(Step, u32); 2]>,
        disarms: usize,
        incoming: VecDeque<Signal>,
        lost: bool,
    }

    #[derive(Clone, Default)]
    struct Fake {
        shared: Arc<Mutex<Shared>>,
    }

    fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
        shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    impl Fake {
        fn state(&self) -> MutexGuard<'_, Shared> {
            lock(&self.shared)
        }

        fn say(&self, signal: Signal) {
            self.state().incoming.push_back(signal);
        }
    }

    impl Transport for Fake {
        fn arm(&mut self, steps: [(Step, u32); 2]) -> Result<(), Lost> {
            let mut shared = self.state();
            shared.live = steps.to_vec();
            shared.arms.push(steps);
            Ok(())
        }

        fn disarm(&mut self) {
            let mut shared = self.state();
            shared.live.clear();
            shared.disarms += 1;
        }

        fn flush(&mut self) -> Result<(), Lost> {
            if self.state().lost {
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            Ok(())
        }

        fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost> {
            thread::sleep(timeout.min(Duration::from_millis(2)));
            let mut shared = self.state();
            if shared.lost {
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            // A destroyed notification cannot speak.
            if shared.live.is_empty() {
                shared.incoming.clear();
            }
            Ok(shared.incoming.drain(..).collect())
        }
    }

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn config(screen_off: u64, suspend: u64) -> IdleConfig {
        IdleConfig::new(secs(screen_off), secs(suspend)).unwrap_or_default()
    }

    fn on(duty: &mut Duty<Fake>, command: Command) -> Vec<IdleAction> {
        let actions = duty.command(command);
        assert!(duty.reconcile().is_ok());
        actions
    }

    #[test]
    fn events_become_actions_in_order() {
        let mut duty = Duty::new(Fake::default());
        assert!(on(&mut duty, Command::Locker(true)).is_empty());
        assert!(on(&mut duty, Command::Enabled(true)).is_empty());

        assert_eq!(
            duty.signal(Signal::Idled(Step::ScreenOff)),
            [IdleAction::PowerOffMonitors]
        );
        assert_eq!(
            duty.signal(Signal::Idled(Step::LockAndSuspend)),
            [IdleAction::LockAndSuspend]
        );
        assert!(duty
            .signal(Signal::Resumed(Step::LockAndSuspend))
            .is_empty());
        assert_eq!(
            duty.signal(Signal::Idled(Step::ScreenOff)),
            [IdleAction::PowerOffMonitors],
            "a resume starts the chain over"
        );
    }

    #[test]
    fn enabling_creates_both_notifications_with_the_chain_delays() {
        let mut duty = Duty::new(Fake::default());
        assert!(duty.reconcile().is_ok());
        assert!(duty.transport.state().arms.is_empty(), "off holds nothing");

        on(&mut duty, Command::Enabled(true));
        assert_eq!(
            duty.transport.state().live,
            [(Step::ScreenOff, 20_000), (Step::LockAndSuspend, 240_000)]
        );
        assert!(duty.armed);
    }

    #[test]
    fn a_disabled_duty_holds_no_notification_and_forgets_the_old_ones() {
        let mut duty = Duty::new(Fake::default());
        on(&mut duty, Command::Locker(true));
        on(&mut duty, Command::Enabled(true));
        assert_eq!(
            duty.signal(Signal::Idled(Step::ScreenOff)),
            [IdleAction::PowerOffMonitors]
        );

        on(&mut duty, Command::Enabled(false));
        assert!(duty.transport.state().live.is_empty());
        assert!(!duty.armed);

        // The user came back while the duty was off: no `resumed` could
        // arrive, yet the next idle period must act again.
        on(&mut duty, Command::Enabled(true));
        assert_eq!(duty.transport.state().arms.len(), 2);
        assert_eq!(
            duty.signal(Signal::Idled(Step::ScreenOff)),
            [IdleAction::PowerOffMonitors]
        );
    }

    #[test]
    fn a_config_change_rearms_with_the_new_delays() {
        let mut duty = Duty::new(Fake::default());
        on(&mut duty, Command::Enabled(true));
        on(&mut duty, Command::Config(config(30, 600)));

        let shared = duty.transport.state();
        assert_eq!(shared.arms.len(), 2);
        assert_eq!(shared.disarms, 1);
        assert_eq!(
            shared.live,
            [(Step::ScreenOff, 30_000), (Step::LockAndSuspend, 600_000)]
        );
        drop(shared);
        assert!(duty.chain.enabled());
    }

    #[test]
    fn a_config_change_keeps_caffeine_and_the_locker() {
        let mut duty = Duty::new(Fake::default());
        on(&mut duty, Command::Locker(true));
        on(&mut duty, Command::Caffeine(true));
        on(&mut duty, Command::Enabled(true));
        on(&mut duty, Command::Config(config(30, 600)));

        assert!(duty.chain.caffeine() && duty.chain.locker_available());
        assert!(duty.signal(Signal::Idled(Step::ScreenOff)).is_empty());
        assert_eq!(
            on(&mut duty, Command::Caffeine(false)),
            [IdleAction::PowerOffMonitors],
            "releasing caffeine acts on a step that is already idle"
        );
    }

    #[test]
    fn a_config_change_while_off_creates_nothing() {
        let mut duty = Duty::new(Fake::default());
        on(&mut duty, Command::Config(config(30, 600)));
        assert!(duty.transport.state().arms.is_empty());
        on(&mut duty, Command::Enabled(true));
        assert_eq!(duty.transport.state().live[0], (Step::ScreenOff, 30_000));
    }

    #[test]
    fn the_longest_delay_fits_the_protocol_timeout() {
        assert_eq!(timeout_ms(celestina_shell::idle::MAX_DELAY), 86_400_000);
        assert_eq!(timeout_ms(Duration::from_secs(u64::MAX / 4)), u32::MAX);
    }

    #[test]
    fn the_queue_is_bounded_and_a_stopped_worker_is_gone() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = IdleHandle { commands };
        for _ in 0..COMMAND_QUEUE {
            assert_eq!(handle.set_caffeine(true), Ok(()));
        }
        assert_eq!(handle.set_enabled(true), Err(IdleRefused::Busy));
        assert_eq!(handle.set_locker_available(true), Err(IdleRefused::Busy));
        assert_eq!(handle.set_config(config(30, 60)), Err(IdleRefused::Busy));
        drop(queue);
        assert_eq!(handle.set_enabled(true), Err(IdleRefused::Gone));
    }

    /// Runs the worker's loop over a fake until the test is done.
    struct Rig {
        fake: Fake,
        handle: IdleHandle,
        shutdown: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
        events: Arc<Mutex<Vec<IdleEvent>>>,
        performed: Arc<Mutex<Vec<IdleAction>>>,
    }

    impl Rig {
        fn start(connected: Result<Fake, Lost>) -> Self {
            let fake = connected.as_ref().map(Clone::clone).unwrap_or_default();
            let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
            let shutdown = Arc::new(AtomicBool::new(false));
            let events = Arc::new(Mutex::new(Vec::new()));
            let performed = Arc::new(Mutex::new(Vec::new()));
            let (heard, done) = (Arc::clone(&events), Arc::clone(&performed));
            let stop = Arc::clone(&shutdown);
            let thread = thread::spawn(move || {
                let sink = Sink::new(
                    move |event| {
                        let mut heard = heard.lock().unwrap_or_else(PoisonError::into_inner);
                        heard.push(event);
                        true
                    },
                    move |action| {
                        let mut done = done.lock().unwrap_or_else(PoisonError::into_inner);
                        done.push(action);
                    },
                );
                run(connected, &queue, sink, &stop, Duration::from_millis(1));
            });
            Self {
                fake,
                handle: IdleHandle { commands },
                shutdown,
                thread: Some(thread),
                events,
                performed,
            }
        }

        fn performed(&self) -> Vec<IdleAction> {
            self.performed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn events(&self) -> Vec<IdleEvent> {
            self.events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn wait(&self, what: &str, mut ready: impl FnMut(&Self) -> bool) {
            let deadline = Instant::now() + secs(5);
            while !ready(self) {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                thread::sleep(Duration::from_millis(2));
            }
        }

        fn finish(mut self) {
            self.shutdown.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                assert!(thread.join().is_ok());
            }
        }
    }

    #[test]
    fn the_worker_arms_performs_and_releases_on_shutdown() {
        let rig = Rig::start(Ok(Fake::default()));
        assert_eq!(rig.handle.set_locker_available(true), Ok(()));
        assert_eq!(rig.handle.set_enabled(true), Ok(()));
        rig.wait("arming", |rig| !rig.fake.state().live.is_empty());
        rig.wait("the Armed event", |rig| rig.events() == [IdleEvent::Armed]);

        rig.fake.say(Signal::Idled(Step::ScreenOff));
        rig.fake.say(Signal::Idled(Step::LockAndSuspend));
        rig.wait("both actions", |rig| rig.performed().len() == 2);
        assert_eq!(
            rig.performed(),
            [IdleAction::PowerOffMonitors, IdleAction::LockAndSuspend]
        );

        let fake = rig.fake.clone();
        rig.finish();
        assert!(fake.state().live.is_empty(), "shutdown destroys them");
    }

    #[test]
    fn caffeine_suppresses_the_chain_through_the_worker() {
        let rig = Rig::start(Ok(Fake::default()));
        assert_eq!(rig.handle.set_caffeine(true), Ok(()));
        assert_eq!(rig.handle.set_enabled(true), Ok(()));
        rig.wait("arming", |rig| !rig.fake.state().live.is_empty());

        rig.fake.say(Signal::Idled(Step::ScreenOff));
        thread::sleep(Duration::from_millis(60));
        assert!(rig.performed().is_empty(), "caffeine holds every step");

        assert_eq!(rig.handle.set_caffeine(false), Ok(()));
        rig.wait("the held step", |rig| rig.performed().len() == 1);
        assert_eq!(rig.performed(), [IdleAction::PowerOffMonitors]);
        rig.finish();
    }

    #[test]
    fn a_missing_locker_is_published_as_refused_and_still_performed() {
        let rig = Rig::start(Ok(Fake::default()));
        assert_eq!(rig.handle.set_enabled(true), Ok(()));
        rig.wait("arming", |rig| !rig.fake.state().live.is_empty());

        rig.fake.say(Signal::Idled(Step::LockAndSuspend));
        rig.wait("the refusal", |rig| rig.performed().len() == 2);
        assert_eq!(
            rig.performed(),
            [
                IdleAction::PowerOffMonitors,
                IdleAction::Refused(Refusal::NoLocker)
            ]
        );
        assert!(rig
            .events()
            .iter()
            .any(|event| matches!(event, IdleEvent::Refused(reason) if !reason.is_empty())));
        rig.finish();
    }

    #[test]
    fn a_missing_global_reports_unavailable_and_does_nothing() {
        let rig = Rig::start(Err(Lost::NoNotifier));
        rig.wait("the Unavailable event", |rig| !rig.events().is_empty());
        assert_eq!(
            rig.events(),
            [IdleEvent::Unavailable(UNAVAILABLE.to_owned())]
        );
        assert_eq!(rig.handle.set_enabled(true), Ok(()));
        assert_eq!(rig.handle.set_caffeine(true), Ok(()));
        thread::sleep(Duration::from_millis(60));
        assert!(rig.performed().is_empty());
        assert_eq!(rig.events().len(), 1);
        rig.finish();
    }

    #[test]
    fn a_lost_connection_fails_dark_and_reports_unavailable() {
        let rig = Rig::start(Ok(Fake::default()));
        assert_eq!(rig.handle.set_enabled(true), Ok(()));
        rig.wait("arming", |rig| !rig.fake.state().live.is_empty());

        rig.fake.state().lost = true;
        rig.wait("the Unavailable event", |rig| rig.events().len() == 2);
        assert!(matches!(
            rig.events().last(),
            Some(IdleEvent::Unavailable(reason)) if !reason.is_empty()
        ));
        assert!(rig.fake.state().live.is_empty());
        rig.finish();
    }

    #[test]
    fn a_worker_whose_handles_are_gone_stops_by_itself() {
        let rig = Rig::start(Ok(Fake::default()));
        let Rig {
            handle,
            thread,
            fake,
            ..
        } = rig;
        assert_eq!(handle.set_enabled(true), Ok(()));
        drop(handle);
        if let Some(thread) = thread {
            assert!(thread.join().is_ok());
        }
        assert!(fake.state().live.is_empty());
    }
}
