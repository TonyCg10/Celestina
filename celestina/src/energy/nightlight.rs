// Ported from `celestina-qt/src/provider_adapter/nightlight.rs` at
// c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413, adapted: the provider envelope
// (`ProviderRuntime`, `Payload`, `Switch`, the request permits and the
// settings it read and wrote) is dropped; a `NightLightHandle` sends
// `set(Option<u32>)` over a bounded queue that refuses when full, and the
// worker publishes `NightLightEvent`s through a callback; the controller
// state machine is separated from the protocol by a `Transport` seam so it
// is tested without a display; a refused output is left neutral on its own
// instead of failing the whole sweep, and is rebuilt after 1 s doubling to
// 30 s (the halted `retry_backoff`, counted from the first refusal) while
// the light stays on; a warmth change between two warm points lands in one
// frame (the halted `settle_to`); diagnostics go through `tracing`; errors
// are typed; and the connection, its bounded pump and the command queue's
// refusal rule are the shared `wire.rs`, which retries a full socket instead
// of failing dark.

//! The forced night light: one worker that owns its own Wayland connection
//! and every `wlr-gamma-control` object and file descriptor on it.
//!
//! The pure whitepoint, the 300 ms transition and the gamma ramp live in
//! [`celestina_shell::nightlight`]; this module only transports them (plan
//! D3):
//!
//! - a controller exists per output only while the light is on; switching
//!   off sweeps every output back to neutral and then destroys the
//!   controllers, so the compositor restores its own gamma by protocol, and
//!   the same destruction happens on hot-unplug and on shutdown;
//! - each table travels in a fresh `memfd`, written, rewound and kept open
//!   until a display sync confirms it;
//! - a controller the compositor refuses (`failed`) is destroyed at once, so
//!   that output is at the compositor's own gamma, never left at a table half
//!   way; the light reports [`NightLightEvent::NotApplied`] and the output is
//!   tried again after a backoff of 1 s doubling to 30 s;
//! - any error of the connection itself fails dark: every controller is
//!   destroyed, the light reports `NotApplied` and the worker idles.
//!
//! Commands cross into the worker through a queue of
//! [`COMMAND_QUEUE`](super::wire::COMMAND_QUEUE); the
//! worker coalesces whatever is waiting to the latest command. Every wait is
//! bounded, so [`crate::providers::Worker::join`] is too.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::nightlight::{
    gamma_ramp, transition, valid_ramp_size, TransitionFrame, Whitepoint,
};
use rustix::fs::{memfd_create, MemfdFlags};
use rustix::io::Errno;
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1 as manager_proto, zwlr_gamma_control_v1 as gamma_proto,
};

use super::wire::{self, Link, QueueRefusal, Wire, COMMAND_QUEUE, POLL, SYNC_TIMEOUT, UNAVAILABLE};
use crate::providers::Worker;

/// How many outputs the worker tracks; a session has a handful.
const MAX_OUTPUTS: usize = 32;
/// How many protocol events one dispatch may carry before the compositor is
/// treated as hostile.
const MAX_EVENTS: usize = 4096;
/// How many create-and-sweep rounds one reconciliation runs; anything left
/// over is picked up on the next poll.
const MAX_ROUNDS: usize = 3;

/// What the night light reports to the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NightLightEvent {
    /// Every output holds the requested warmth.
    Applied,
    /// The light was asked for and is not (or not everywhere) applied; the
    /// outputs it does not hold are at the compositor's own gamma.
    NotApplied(String),
    /// The light is off and no gamma controller exists.
    Off,
}

/// Why [`NightLightHandle::set`] refused a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NightLightRefused {
    /// [`COMMAND_QUEUE`] commands are already waiting.
    #[error("the night light is still applying earlier commands")]
    Busy,
    /// The worker has stopped.
    #[error("the night-light worker has stopped")]
    Gone,
}

impl QueueRefusal for NightLightRefused {
    const BUSY: Self = Self::Busy;
    const GONE: Self = Self::Gone;
}

/// The loop's side of the night-light worker.
#[derive(Clone, Debug)]
pub struct NightLightHandle {
    commands: SyncSender<Option<u32>>,
}

impl NightLightHandle {
    /// Asks for the light at `kelvin` (clamped to the whitepoint's range), or
    /// off with `None`. The outcome arrives later as a [`NightLightEvent`].
    /// The worker drains the queue every poll and during a sweep, so a full
    /// queue is a stuck worker.
    ///
    /// # Errors
    ///
    /// [`NightLightRefused::Busy`] when the queue is full,
    /// [`NightLightRefused::Gone`] when the worker has stopped.
    pub fn set(&self, kelvin: Option<u32>) -> Result<(), NightLightRefused> {
        wire::offer(&self.commands, kelvin)
    }
}

/// Starts the night-light worker.
///
/// `publish` receives every change of [`NightLightEvent`] and returns
/// whether the loop still listens; once it returns `false` the worker
/// restores every output and stops. The worker also stops when `shutdown`
/// is raised or every handle is dropped, and restores the outputs first.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    publish: impl FnMut(NightLightEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(NightLightHandle, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let worker_shutdown = Arc::clone(shutdown);
    let worker = Worker::spawn("night-light", shutdown, move || {
        run(&queue, Publisher::new(publish), &worker_shutdown);
    })?;
    Ok((NightLightHandle { commands }, worker))
}

/// How long an output waits before its refused controller is rebuilt, after
/// `attempts` earlier rebuilds of it: 1 s, doubling, to a 30 s ceiling.
///
/// Rebuilding is not invisible: a new controller starts a fresh warm sweep
/// on that output. Most refusals are transient (an output still settling
/// after a hotplug), so the first retry is quick; an output that simply
/// cannot hold a controller climbs to the ceiling and stops costing the
/// session anything visible.
fn retry_backoff(attempts: u32) -> Duration {
    const CEILING: Duration = Duration::from_secs(30);
    let seconds = 1u64 << attempts.min(5);
    Duration::from_secs(seconds).min(CEILING)
}

/// The connection itself is gone or cannot be trusted.
#[derive(Debug, thiserror::Error)]
enum Lost {
    #[error("the compositor offers no wlr gamma control")]
    NoManager,
    #[error("the compositor withdrew wlr gamma control")]
    ManagerGone,
    #[error(transparent)]
    Wire(#[from] wire::Lost),
}

/// One output's controller could not be created or fed.
#[derive(Debug, thiserror::Error)]
enum OutputError {
    #[error("output {0} or the gamma manager is gone")]
    Unknown(u32),
    #[error("cannot create a gamma table")]
    Memfd(#[source] Errno),
    #[error("cannot write a gamma table")]
    Write(#[source] io::Error),
}

/// What the protocol reports, reduced to what the state machine needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GammaEvent {
    OutputAdded(u32),
    OutputRemoved(u32),
    GammaSize {
        output: u32,
        size: u32,
    },
    /// The compositor refused or revoked the output's controller; the
    /// transport has already destroyed it.
    Failed(u32),
    ManagerGone,
}

/// The protocol seam: per-output controllers addressed by the output's
/// global name. Requests are queued until [`Transport::flush`].
trait Transport {
    /// Creates a controller for `output`, replacing any it had.
    fn create(&mut self, output: u32) -> Result<(), OutputError>;
    /// Destroys `output`'s controller, if it has one; the compositor then
    /// restores its own gamma there.
    fn destroy(&mut self, output: u32);
    /// Queues `ramp` (red, green, blue) for `output`'s controller.
    fn set_gamma(&mut self, output: u32, ramp: &[u16]) -> Result<(), OutputError>;
    fn flush(&mut self) -> Result<(), Lost>;
    /// Waits until the compositor has handled everything sent so far, at
    /// most until `deadline`, and returns what it reported meanwhile.
    fn sync(&mut self, deadline: Instant) -> Result<Vec<GammaEvent>, Lost>;
    /// Waits at most `timeout` for events and returns them.
    fn poll(&mut self, timeout: Duration) -> Result<Vec<GammaEvent>, Lost>;
}

/// Why a reconciliation stopped before it finished.
#[derive(Debug)]
enum Halt {
    Lost(Lost),
    /// The pacer was told to stop (shutdown) in the middle of a sweep.
    Aborted,
}

impl From<Lost> for Halt {
    fn from(lost: Lost) -> Self {
        Self::Lost(lost)
    }
}

#[derive(Debug)]
struct Control {
    /// Unknown until the compositor sends it.
    size: Option<u32>,
    /// The whitepoint of the last table sent.
    applied: Whitepoint,
}

#[derive(Debug, Default)]
struct Slot {
    control: Option<Control>,
    /// How many times this output's controller was refused while the light
    /// has been on. Switching off forgets it.
    attempts: u32,
    /// The soonest a refused controller may be rebuilt.
    retry_at: Option<Instant>,
}

/// The controller state machine over a [`Transport`].
struct Lights<T: Transport> {
    transport: T,
    outputs: BTreeMap<u32, Slot>,
    /// `None` is off.
    target: Option<Whitepoint>,
}

impl<T: Transport> Lights<T> {
    fn new(transport: T) -> Self {
        Self {
            transport,
            outputs: BTreeMap::new(),
            target: None,
        }
    }

    fn set_target(&mut self, target: Option<Whitepoint>) {
        if target.is_none() {
            for slot in self.outputs.values_mut() {
                slot.attempts = 0;
                slot.retry_at = None;
            }
        }
        self.target = target;
    }

    /// Destroys `output`'s controller and schedules its rebuild.
    fn fail_output(&mut self, output: u32, now: Instant) {
        let Some(slot) = self.outputs.get_mut(&output) else {
            return;
        };
        if slot.control.take().is_some() {
            self.transport.destroy(output);
        }
        let waited = slot.attempts;
        slot.attempts = slot.attempts.saturating_add(1);
        slot.retry_at = Some(now + retry_backoff(waited));
    }

    fn control_mut(&mut self, output: u32) -> Option<&mut Control> {
        self.outputs
            .get_mut(&output)
            .and_then(|slot| slot.control.as_mut())
    }

    fn handle(&mut self, events: Vec<GammaEvent>, now: Instant) -> Result<(), Lost> {
        for event in events {
            match event {
                GammaEvent::OutputAdded(output) => {
                    if self.outputs.len() < MAX_OUTPUTS {
                        self.outputs.entry(output).or_default();
                    }
                }
                GammaEvent::OutputRemoved(output) => {
                    if let Some(slot) = self.outputs.remove(&output) {
                        if slot.control.is_some() {
                            self.transport.destroy(output);
                        }
                    }
                }
                GammaEvent::GammaSize { output, size } => {
                    let usable = valid_ramp_size(size);
                    let Some(control) = self.control_mut(output) else {
                        continue;
                    };
                    if usable {
                        control.size = Some(size);
                    } else {
                        tracing::warn!(
                            "night light: output {output} reports an unusable gamma size {size}"
                        );
                        self.fail_output(output, now);
                    }
                }
                GammaEvent::Failed(output) => {
                    if self.control_mut(output).is_some() {
                        tracing::warn!("night light: the compositor refused output {output}");
                        self.fail_output(output, now);
                    }
                }
                GammaEvent::ManagerGone => return Err(Lost::ManagerGone),
            }
        }
        Ok(())
    }

    /// Creates the controllers that are due; returns whether any was.
    fn create_due(&mut self, now: Instant) -> bool {
        if self.target.is_none() {
            return false;
        }
        let due: Vec<u32> = self
            .outputs
            .iter()
            .filter(|(_, slot)| slot.control.is_none() && slot.retry_at.is_none_or(|at| now >= at))
            .map(|(output, _)| *output)
            .collect();
        let mut created = false;
        for output in due {
            match self.transport.create(output) {
                Ok(()) => {
                    if let Some(slot) = self.outputs.get_mut(&output) {
                        slot.control = Some(Control {
                            size: None,
                            applied: Whitepoint::NEUTRAL,
                        });
                    }
                    created = true;
                }
                Err(error) => {
                    tracing::warn!("night light: {}", crate::error_chain(&error));
                    self.fail_output(output, now);
                }
            }
        }
        created
    }

    /// The frames each ready output still has to travel. Leaving or reaching
    /// neutral sweeps; moving between two warm points lands in one frame,
    /// because a person dragging the warmth is already the motion.
    fn plans(&self) -> Vec<(u32, Vec<TransitionFrame>)> {
        let target = self.target.unwrap_or(Whitepoint::NEUTRAL);
        self.outputs
            .iter()
            .filter_map(|(output, slot)| {
                let control = slot.control.as_ref()?;
                if control.size.is_none() || control.applied == target {
                    return None;
                }
                let frames =
                    if control.applied == Whitepoint::NEUTRAL || target == Whitepoint::NEUTRAL {
                        transition(control.applied, target)
                    } else {
                        vec![TransitionFrame {
                            offset: Duration::ZERO,
                            whitepoint: target,
                        }]
                    };
                Some((*output, frames))
            })
            .collect()
    }

    /// Sends `output` its table for `whitepoint`; a failure fails the output.
    fn apply(&mut self, output: u32, whitepoint: Whitepoint, now: Instant) {
        let Some(control) = self.control_mut(output) else {
            return;
        };
        let Some(ramp) = control.size.and_then(|size| gamma_ramp(size, whitepoint)) else {
            self.fail_output(output, now);
            return;
        };
        match self.transport.set_gamma(output, &ramp) {
            Ok(()) => {
                if let Some(control) = self.control_mut(output) {
                    control.applied = whitepoint;
                }
            }
            Err(error) => {
                tracing::warn!("night light: {}", crate::error_chain(&error));
                self.fail_output(output, now);
            }
        }
    }

    /// Plays `plans` frame by frame, each frame released by `pace` with the
    /// instant it is due, then confirms the last one with a sync.
    fn sweep(
        &mut self,
        plans: &[(u32, Vec<TransitionFrame>)],
        pace: &mut dyn FnMut(Instant) -> bool,
        now: Instant,
    ) -> Result<(), Halt> {
        let Some(schedule) = plans
            .iter()
            .map(|(_, frames)| frames)
            .max_by_key(|frames| frames.len())
        else {
            return Ok(());
        };
        let started = Instant::now();
        for (index, frame) in schedule.iter().enumerate() {
            if !pace(started + frame.offset) {
                return Err(Halt::Aborted);
            }
            for (output, frames) in plans {
                if let Some(frame) = frames.get(index) {
                    self.apply(*output, frame.whitepoint, now);
                }
            }
            self.transport.flush()?;
        }
        let events = self.transport.sync(Instant::now() + SYNC_TIMEOUT)?;
        self.handle(events, now)?;
        Ok(())
    }

    /// Brings every output to the target: creates what is due, sweeps what
    /// is ready, and releases every controller once the light is off.
    fn reconcile(
        &mut self,
        now: Instant,
        pace: &mut dyn FnMut(Instant) -> bool,
    ) -> Result<(), Halt> {
        for _ in 0..MAX_ROUNDS {
            if self.create_due(now) {
                self.transport.flush()?;
                let events = self.transport.sync(Instant::now() + SYNC_TIMEOUT)?;
                self.handle(events, now)?;
            }
            let plans = self.plans();
            if plans.is_empty() {
                break;
            }
            self.sweep(&plans, pace, now)?;
        }
        if self.target.is_none() && self.holds_any() {
            self.release_all();
            self.transport.flush()?;
            let events = self.transport.sync(Instant::now() + SYNC_TIMEOUT)?;
            self.handle(events, now)?;
        }
        Ok(())
    }

    fn holds_any(&self) -> bool {
        self.outputs.values().any(|slot| slot.control.is_some())
    }

    fn release_all(&mut self) {
        for (output, slot) in &mut self.outputs {
            if slot.control.take().is_some() {
                self.transport.destroy(*output);
            }
        }
    }

    /// What to report now; `None` while a controller has not answered yet.
    fn status(&self) -> Option<NightLightEvent> {
        let Some(target) = self.target else {
            return (!self.holds_any()).then_some(NightLightEvent::Off);
        };
        if self.outputs.is_empty() {
            return Some(NightLightEvent::NotApplied(
                "the compositor reports no output".to_owned(),
            ));
        }
        let mut pending = false;
        for (output, slot) in &self.outputs {
            match &slot.control {
                None => {
                    return Some(NightLightEvent::NotApplied(format!(
                        "output {output} refused gamma control"
                    )))
                }
                Some(control) if control.size.is_none() || control.applied != target => {
                    pending = true;
                }
                Some(_) => {}
            }
        }
        (!pending).then_some(NightLightEvent::Applied)
    }

    /// Restores every output and releases every controller, best effort:
    /// the sweep back to neutral when `sweep` and the connection allow it,
    /// then destruction, which restores the compositor's gamma by protocol.
    fn teardown(&mut self, sweep: bool, pace: &mut dyn FnMut(Instant) -> bool) {
        self.target = None;
        if sweep {
            let plans = self.plans();
            if let Err(Halt::Lost(lost)) = self.sweep(&plans, pace, Instant::now()) {
                tracing::warn!("night light: {}", crate::error_chain(&lost));
            }
        }
        self.release_all();
        if self.transport.flush().is_ok() {
            let _ = self.transport.sync(Instant::now() + SYNC_TIMEOUT);
        }
    }
}

/// Publishes only changes.
struct Publisher<F> {
    publish: F,
    last: Option<NightLightEvent>,
}

impl<F: FnMut(NightLightEvent) -> bool> Publisher<F> {
    fn new(publish: F) -> Self {
        Self {
            publish,
            last: None,
        }
    }

    /// Returns whether the loop still listens.
    fn say(&mut self, event: NightLightEvent) -> bool {
        if self.last.as_ref() == Some(&event) {
            return true;
        }
        self.last = Some(event.clone());
        (self.publish)(event)
    }
}

/// Why [`serve`] returned without losing the connection.
#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Shutdown,
    LoopGone,
    HandleGone,
}

/// Waits until `at` in steps of [`POLL`], moving waiting commands into
/// `pending` so the queue never fills behind a sweep; `false` once
/// `shutdown` is raised.
fn wait_until(
    at: Instant,
    commands: &Receiver<Option<u32>>,
    pending: &mut Option<Option<u32>>,
    shutdown: &AtomicBool,
) -> bool {
    loop {
        while let Ok(command) = commands.try_recv() {
            *pending = Some(command);
        }
        if shutdown.load(Ordering::Acquire) {
            return false;
        }
        match at.checked_duration_since(Instant::now()) {
            Some(remaining) if !remaining.is_zero() => thread::sleep(remaining.min(POLL)),
            _ => return true,
        }
    }
}

/// Sleeps until `at`, ignoring shutdown: a restoration sweep is bounded by
/// its own 300 ms.
fn sleep_until(at: Instant) -> bool {
    if let Some(remaining) = at.checked_duration_since(Instant::now()) {
        thread::sleep(remaining);
    }
    true
}

/// The worker's loop over a connected transport. `paced` is false only in
/// tests, where frames are not spaced in time.
fn serve<T: Transport, F: FnMut(NightLightEvent) -> bool>(
    lights: &mut Lights<T>,
    commands: &Receiver<Option<u32>>,
    out: &mut Publisher<F>,
    shutdown: &AtomicBool,
    paced: bool,
) -> Result<Exit, Lost> {
    let mut pending: Option<Option<u32>> = None;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return Ok(Exit::Shutdown);
        }
        let mut handle_gone = false;
        loop {
            match commands.try_recv() {
                Ok(command) => pending = Some(command),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    handle_gone = true;
                    break;
                }
            }
        }
        if let Some(kelvin) = pending.take() {
            lights.set_target(kelvin.map(Whitepoint::for_temperature));
        }
        let events = lights
            .transport
            .poll(if paced { POLL } else { Duration::ZERO })?;
        let now = Instant::now();
        lights.handle(events, now)?;
        let mut pace = |at: Instant| {
            if paced {
                wait_until(at, commands, &mut pending, shutdown)
            } else {
                !shutdown.load(Ordering::Acquire)
            }
        };
        match lights.reconcile(now, &mut pace) {
            Ok(()) => {}
            Err(Halt::Aborted) => return Ok(Exit::Shutdown),
            Err(Halt::Lost(lost)) => return Err(lost),
        }
        if let Some(status) = lights.status() {
            if !out.say(status) {
                return Ok(Exit::LoopGone);
            }
        }
        if handle_gone && pending.is_none() {
            return Ok(Exit::HandleGone);
        }
    }
}

/// Answers commands without a connection until shutdown: off is off, and
/// anything else is not applied.
fn idle_unavailable<F: FnMut(NightLightEvent) -> bool>(
    commands: &Receiver<Option<u32>>,
    out: &mut Publisher<F>,
    shutdown: &AtomicBool,
) {
    while !shutdown.load(Ordering::Acquire) {
        let event = match commands.recv_timeout(POLL) {
            Ok(None) => NightLightEvent::Off,
            Ok(Some(_)) => NightLightEvent::NotApplied(UNAVAILABLE.to_owned()),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if !out.say(event) {
            return;
        }
    }
}

fn run<F: FnMut(NightLightEvent) -> bool>(
    commands: &Receiver<Option<u32>>,
    mut out: Publisher<F>,
    shutdown: &AtomicBool,
) {
    let (transport, events) = match WaylandTransport::connect() {
        Ok(connected) => connected,
        Err(lost) => {
            tracing::warn!("night light: {}", crate::error_chain(&lost));
            if out.say(NightLightEvent::NotApplied(UNAVAILABLE.to_owned())) {
                idle_unavailable(commands, &mut out, shutdown);
            }
            return;
        }
    };
    let mut lights = Lights::new(transport);
    let outcome = lights
        .handle(events, Instant::now())
        .and_then(|()| serve(&mut lights, commands, &mut out, shutdown, true));
    match outcome {
        Ok(_) => {
            lights.teardown(true, &mut sleep_until);
            let _ = out.say(NightLightEvent::Off);
        }
        Err(lost) => {
            // Fail dark: destroy what is left; a dead connection has already
            // made the compositor restore every output.
            let reason = crate::error_chain(&lost);
            tracing::warn!("night light: {reason}");
            lights.teardown(false, &mut sleep_until);
            drop(lights);
            if out.say(NightLightEvent::NotApplied(reason)) {
                idle_unavailable(commands, &mut out, shutdown);
            }
        }
    }
}

// The Wayland transport.

struct WaylandControl {
    proxy: gamma_proto::ZwlrGammaControlV1,
    generation: u64,
    /// Tables sent since the last confirmed sync.
    tables: Vec<File>,
}

struct WaylandOutput {
    proxy: wl_output::WlOutput,
    control: Option<WaylandControl>,
}

struct WaylandState {
    manager: Option<(u32, manager_proto::ZwlrGammaControlManagerV1)>,
    outputs: BTreeMap<u32, WaylandOutput>,
    next_generation: u64,
}

/// What the gamma connection dispatches into.
type GammaWire = Wire<WaylandState, GammaEvent>;

/// The night light has no reason of its own to stop pumping: a withdrawn
/// manager arrives as [`GammaEvent::ManagerGone`].
fn intact(_: &WaylandState) -> Result<(), Lost> {
    Ok(())
}

struct WaylandTransport {
    link: Link<WaylandState, GammaEvent>,
}

impl WaylandTransport {
    /// Opens the worker's own connection and discovers the outputs and the
    /// gamma manager.
    fn connect() -> Result<(Self, Vec<GammaEvent>), Lost> {
        let state = WaylandState {
            manager: None,
            outputs: BTreeMap::new(),
            next_generation: 0,
        };
        let mut transport = Self {
            link: Link::connect(state, MAX_EVENTS)?,
        };
        let events = transport.sync(Instant::now() + SYNC_TIMEOUT)?;
        if transport.link.wire.state.manager.is_none() {
            return Err(Lost::NoManager);
        }
        Ok((transport, events))
    }
}

impl Transport for WaylandTransport {
    fn create(&mut self, output: u32) -> Result<(), OutputError> {
        let Link { qh, wire, .. } = &mut self.link;
        let WaylandState {
            manager,
            outputs,
            next_generation,
        } = &mut wire.state;
        let (Some((_, manager)), Some(slot)) = (manager.as_ref(), outputs.get_mut(&output)) else {
            return Err(OutputError::Unknown(output));
        };
        if let Some(old) = slot.control.take() {
            old.proxy.destroy();
        }
        *next_generation = next_generation.wrapping_add(1);
        let generation = *next_generation;
        let proxy = manager.get_gamma_control(&slot.proxy, qh, (output, generation));
        slot.control = Some(WaylandControl {
            proxy,
            generation,
            tables: Vec::new(),
        });
        Ok(())
    }

    fn destroy(&mut self, output: u32) {
        if let Some(control) = self
            .link
            .wire
            .state
            .outputs
            .get_mut(&output)
            .and_then(|slot| slot.control.take())
        {
            control.proxy.destroy();
        }
    }

    fn set_gamma(&mut self, output: u32, ramp: &[u16]) -> Result<(), OutputError> {
        let Some(control) = self
            .link
            .wire
            .state
            .outputs
            .get_mut(&output)
            .and_then(|slot| slot.control.as_mut())
        else {
            return Err(OutputError::Unknown(output));
        };
        let mut bytes = Vec::with_capacity(ramp.len().saturating_mul(2));
        for sample in ramp {
            bytes.extend_from_slice(&sample.to_ne_bytes());
        }
        // Each table gets storage of its own: one memfd reused for the next
        // frame could let the compositor read that frame under an older
        // request. The compositor reads from the descriptor's offset, so the
        // rewind is part of the protocol.
        let fd = memfd_create("celestina-night-light", MemfdFlags::CLOEXEC)
            .map_err(OutputError::Memfd)?;
        let mut table = File::from(fd);
        table
            .write_all(&bytes)
            .and_then(|()| table.seek(SeekFrom::Start(0)).map(|_| ()))
            .map_err(OutputError::Write)?;
        control.proxy.set_gamma(table.as_fd());
        control.tables.push(table);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Lost> {
        Ok(self.link.flush()?)
    }

    fn sync(&mut self, deadline: Instant) -> Result<Vec<GammaEvent>, Lost> {
        self.link.sync(deadline, intact)?;
        // Everything sent before the sync has been read by the compositor.
        for slot in self.link.wire.state.outputs.values_mut() {
            if let Some(control) = slot.control.as_mut() {
                control.tables.clear();
            }
        }
        Ok(self.link.wire.take_events())
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<GammaEvent>, Lost> {
        self.link.pump(timeout, intact)?;
        Ok(self.link.wire.take_events())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for GammaWire {
    fn event(
        wire: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_output::WlOutput::interface().name => {
                if wire.state.outputs.len() < MAX_OUTPUTS && !wire.state.outputs.contains_key(&name)
                {
                    let proxy =
                        registry.bind::<wl_output::WlOutput, _, _>(name, version.min(1), qh, name);
                    wire.state.outputs.insert(
                        name,
                        WaylandOutput {
                            proxy,
                            control: None,
                        },
                    );
                    wire.push(GammaEvent::OutputAdded(name));
                }
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == manager_proto::ZwlrGammaControlManagerV1::interface().name
                && wire.state.manager.is_none() =>
            {
                let manager = registry.bind::<manager_proto::ZwlrGammaControlManagerV1, _, _>(
                    name,
                    version.min(1),
                    qh,
                    (),
                );
                wire.state.manager = Some((name, manager));
            }
            wl_registry::Event::GlobalRemove { name } => {
                if wire
                    .state
                    .manager
                    .as_ref()
                    .is_some_and(|(global, _)| *global == name)
                {
                    if let Some((_, manager)) = wire.state.manager.take() {
                        manager.destroy();
                    }
                    wire.push(GammaEvent::ManagerGone);
                }
                if let Some(output) = wire.state.outputs.remove(&name) {
                    if let Some(control) = output.control {
                        control.proxy.destroy();
                    }
                    wire.push(GammaEvent::OutputRemoved(name));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for GammaWire {
    fn event(
        _: &mut Self,
        _: &wl_output::WlOutput,
        _: wl_output::Event,
        _: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<gamma_proto::ZwlrGammaControlV1, (u32, u64)> for GammaWire {
    fn event(
        wire: &mut Self,
        _: &gamma_proto::ZwlrGammaControlV1,
        event: gamma_proto::Event,
        (output, generation): &(u32, u64),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(slot) = wire.state.outputs.get_mut(output) else {
            return;
        };
        // An event for a controller this output no longer holds is stale.
        if slot
            .control
            .as_ref()
            .is_none_or(|control| control.generation != *generation)
        {
            return;
        }
        match event {
            gamma_proto::Event::GammaSize { size } => wire.push(GammaEvent::GammaSize {
                output: *output,
                size,
            }),
            gamma_proto::Event::Failed => {
                if let Some(control) = slot.control.take() {
                    control.proxy.destroy();
                }
                wire.push(GammaEvent::Failed(*output));
            }
            _ => {}
        }
    }
}

delegate_noop!(GammaWire: ignore manager_proto::ZwlrGammaControlManagerV1);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const SIZE: u32 = 256;

    /// How the fake compositor answers a new controller for an output.
    #[derive(Clone, Copy, Debug)]
    enum Answer {
        Size(u32),
        Refuse,
        /// Sends its size, then refuses once it is given a table.
        RefuseOnSet,
    }

    /// A compositor without a socket: it records every request and answers
    /// on the next sync or poll, destroying a refused controller as the
    /// Wayland transport does while dispatching.
    #[derive(Default)]
    struct Fake {
        answers: BTreeMap<u32, Answer>,
        queued: Vec<GammaEvent>,
        live: BTreeSet<u32>,
        created: Vec<u32>,
        destroyed: Vec<u32>,
        ramps: BTreeMap<u32, Vec<Vec<u16>>>,
        lost: bool,
    }

    impl Fake {
        fn with(outputs: &[(u32, Answer)]) -> (Self, Vec<GammaEvent>) {
            let fake = Self {
                answers: outputs.iter().copied().collect(),
                ..Self::default()
            };
            let events = outputs
                .iter()
                .map(|(output, _)| GammaEvent::OutputAdded(*output))
                .collect();
            (fake, events)
        }

        fn last(&self, output: u32) -> Option<&Vec<u16>> {
            self.ramps.get(&output).and_then(|ramps| ramps.last())
        }

        fn count(&self, output: u32) -> usize {
            self.ramps.get(&output).map_or(0, Vec::len)
        }

        fn created(&self, output: u32) -> usize {
            self.created.iter().filter(|o| **o == output).count()
        }
    }

    impl Transport for Fake {
        fn create(&mut self, output: u32) -> Result<(), OutputError> {
            let answer = *self
                .answers
                .get(&output)
                .ok_or(OutputError::Unknown(output))?;
            self.created.push(output);
            self.live.insert(output);
            self.queued.push(match answer {
                Answer::Size(size) => GammaEvent::GammaSize { output, size },
                Answer::RefuseOnSet => GammaEvent::GammaSize { output, size: SIZE },
                Answer::Refuse => GammaEvent::Failed(output),
            });
            Ok(())
        }

        fn destroy(&mut self, output: u32) {
            self.destroyed.push(output);
            self.live.remove(&output);
        }

        fn set_gamma(&mut self, output: u32, ramp: &[u16]) -> Result<(), OutputError> {
            if !self.live.contains(&output) {
                return Err(OutputError::Unknown(output));
            }
            self.ramps.entry(output).or_default().push(ramp.to_vec());
            if matches!(self.answers.get(&output), Some(Answer::RefuseOnSet))
                && !self.queued.contains(&GammaEvent::Failed(output))
            {
                self.queued.push(GammaEvent::Failed(output));
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), Lost> {
            if self.lost {
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            Ok(())
        }

        fn sync(&mut self, _: Instant) -> Result<Vec<GammaEvent>, Lost> {
            self.flush()?;
            let events = std::mem::take(&mut self.queued);
            for event in &events {
                if let GammaEvent::Failed(output) = event {
                    self.live.remove(output);
                }
            }
            Ok(events)
        }

        fn poll(&mut self, _: Duration) -> Result<Vec<GammaEvent>, Lost> {
            self.sync(Instant::now())
        }
    }

    fn lights(outputs: &[(u32, Answer)]) -> Lights<Fake> {
        let (fake, events) = Fake::with(outputs);
        let mut lights = Lights::new(fake);
        lights
            .handle(events, Instant::now())
            .expect("adding outputs cannot lose the connection");
        lights
    }

    fn warm() -> Whitepoint {
        Whitepoint::for_temperature(2700)
    }

    fn ramp(whitepoint: Whitepoint) -> Vec<u16> {
        gamma_ramp(SIZE, whitepoint).expect("the fixture size is valid")
    }

    /// Reconciles at `now` without pacing and returns how many frames were
    /// released.
    fn reconcile(lights: &mut Lights<Fake>, now: Instant) -> usize {
        let mut frames = 0;
        lights
            .reconcile(now, &mut |_| {
                frames += 1;
                true
            })
            .expect("the fake connection holds");
        frames
    }

    const THREE: [(u32, Answer); 3] = [
        (1, Answer::Size(SIZE)),
        (2, Answer::Size(SIZE)),
        (3, Answer::Size(SIZE)),
    ];

    #[test]
    fn switching_on_creates_one_controller_per_output_and_sweeps_all_of_them() {
        let mut lights = lights(&THREE);
        assert_eq!(lights.status(), Some(NightLightEvent::Off));
        assert!(
            lights.transport.created.is_empty(),
            "off costs no controller"
        );

        lights.set_target(Some(warm()));
        let frames = reconcile(&mut lights, Instant::now());

        assert_eq!(lights.transport.created, vec![1, 2, 3]);
        assert_eq!(frames, 19, "one 300 ms sweep, paced once per frame");
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), 19);
            assert_eq!(lights.transport.last(output), Some(&ramp(warm())));
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));

        // A second pass owes nothing.
        assert_eq!(reconcile(&mut lights, Instant::now()), 0);
        assert_eq!(lights.transport.created.len(), 3);
    }

    #[test]
    fn switching_off_sweeps_to_neutral_then_destroys_every_controller() {
        let mut lights = lights(&THREE);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        lights.set_target(None);
        let frames = reconcile(&mut lights, Instant::now());

        assert_eq!(frames, 19);
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), 38);
            assert_eq!(
                lights.transport.last(output),
                Some(&ramp(Whitepoint::NEUTRAL))
            );
        }
        assert_eq!(lights.transport.destroyed, vec![1, 2, 3]);
        assert!(lights.transport.live.is_empty());
        assert_eq!(lights.status(), Some(NightLightEvent::Off));
    }

    #[test]
    fn a_refused_controller_leaves_its_output_neutral_and_reports_not_applied() {
        let mut lights = lights(&[
            (1, Answer::Size(SIZE)),
            (2, Answer::Refuse),
            (3, Answer::Size(SIZE)),
        ]);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        assert!(
            !lights.transport.live.contains(&2),
            "no controller holds it"
        );
        assert_eq!(lights.transport.count(2), 0, "it never received a table");
        assert_eq!(lights.transport.last(1), Some(&ramp(warm())));
        assert_eq!(lights.transport.last(3), Some(&ramp(warm())));
        assert_eq!(
            lights.status(),
            Some(NightLightEvent::NotApplied(
                "output 2 refused gamma control".to_owned()
            ))
        );
    }

    #[test]
    fn a_controller_refused_mid_sweep_is_destroyed_not_left_half_way() {
        let mut lights = lights(&[(1, Answer::RefuseOnSet), (2, Answer::Size(SIZE))]);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        // Destroyed, so the compositor holds its own gamma there rather than
        // whatever table the refusal interrupted.
        assert!(!lights.transport.live.contains(&1));
        assert!(lights.transport.destroyed.contains(&1));
        assert!(lights
            .outputs
            .get(&1)
            .is_some_and(|slot| slot.control.is_none()));
        assert_eq!(lights.transport.last(2), Some(&ramp(warm())));
        assert!(matches!(
            lights.status(),
            Some(NightLightEvent::NotApplied(_))
        ));
    }

    #[test]
    fn the_backoff_starts_at_one_second_and_doubles_to_thirty() {
        let waits: Vec<u64> = (0..8).map(|n| retry_backoff(n).as_secs()).collect();
        assert_eq!(waits, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn a_refused_output_is_rebuilt_only_after_its_backoff() {
        let mut lights = lights(&[(1, Answer::Refuse)]);
        let start = Instant::now();
        lights.set_target(Some(warm()));
        reconcile(&mut lights, start);
        assert_eq!(lights.transport.created(1), 1);

        let mut at = start;
        for wait in [1, 2, 4, 8, 16, 30, 30] {
            let due = at + Duration::from_secs(wait);
            reconcile(&mut lights, due - Duration::from_millis(1));
            let before = lights.transport.created(1);
            reconcile(&mut lights, due);
            assert_eq!(
                lights.transport.created(1),
                before + 1,
                "rebuilt once {wait} s after the last refusal"
            );
            at = due;
        }

        // The output recovers: the next due rebuild sweeps it warm.
        lights.transport.answers.insert(1, Answer::Size(SIZE));
        reconcile(&mut lights, at + Duration::from_secs(30));
        assert_eq!(lights.transport.last(1), Some(&ramp(warm())));
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
    }

    #[test]
    fn switching_off_forgets_the_backoff() {
        let mut lights = lights(&[(1, Answer::Refuse)]);
        let start = Instant::now();
        lights.set_target(Some(warm()));
        reconcile(&mut lights, start);
        reconcile(&mut lights, start + Duration::from_secs(1));
        assert_eq!(lights.outputs.get(&1).map(|slot| slot.attempts), Some(2));

        lights.set_target(None);
        reconcile(&mut lights, start + Duration::from_secs(1));
        assert_eq!(lights.status(), Some(NightLightEvent::Off));

        lights.set_target(Some(warm()));
        let before = lights.transport.created(1);
        reconcile(&mut lights, start + Duration::from_secs(1));
        assert_eq!(lights.transport.created(1), before + 1, "no wait inherited");
    }

    #[test]
    fn hot_unplug_destroys_that_controller_and_hotplug_sweeps_only_the_new_output() {
        let mut lights = lights(&[(1, Answer::Size(SIZE)), (2, Answer::Size(SIZE))]);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        lights
            .handle(vec![GammaEvent::OutputRemoved(2)], Instant::now())
            .expect("a removal is not a loss");
        assert_eq!(lights.transport.destroyed, vec![2]);
        assert!(!lights.outputs.contains_key(&2));
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));

        lights.transport.answers.insert(3, Answer::Size(SIZE));
        lights
            .handle(vec![GammaEvent::OutputAdded(3)], Instant::now())
            .expect("an addition is not a loss");
        reconcile(&mut lights, Instant::now());
        assert_eq!(lights.transport.count(1), 19, "the lit output is untouched");
        assert_eq!(
            lights.transport.count(3),
            19,
            "the new one sweeps from neutral"
        );
        assert_eq!(lights.transport.last(3), Some(&ramp(warm())));
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
    }

    #[test]
    fn a_warmth_change_while_on_lands_in_one_frame() {
        let mut lights = lights(&THREE);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        let milder = Whitepoint::for_temperature(4000);
        lights.set_target(Some(milder));
        assert_eq!(reconcile(&mut lights, Instant::now()), 1);
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), 20);
            assert_eq!(lights.transport.last(output), Some(&ramp(milder)));
        }
        assert_eq!(lights.transport.created.len(), 3, "no controller rebuilt");
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
    }

    #[test]
    fn teardown_restores_every_output_and_releases_every_controller() {
        let mut lights = lights(&THREE);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        lights.teardown(true, &mut |_| true);

        for output in [1, 2, 3] {
            assert_eq!(
                lights.transport.last(output),
                Some(&ramp(Whitepoint::NEUTRAL))
            );
        }
        assert!(lights.transport.live.is_empty());
        assert_eq!(lights.status(), Some(NightLightEvent::Off));
    }

    #[test]
    fn a_sweep_aborted_by_shutdown_still_ends_with_every_controller_destroyed() {
        let mut lights = lights(&THREE);
        lights.set_target(Some(warm()));
        let mut frames = 0;
        let halted = lights.reconcile(Instant::now(), &mut |_| {
            frames += 1;
            frames < 5
        });
        assert!(matches!(halted, Err(Halt::Aborted)));

        lights.teardown(true, &mut |_| true);
        assert!(lights.transport.live.is_empty());
        for output in [1, 2, 3] {
            assert_eq!(
                lights.transport.last(output),
                Some(&ramp(Whitepoint::NEUTRAL))
            );
        }
    }

    #[test]
    fn a_lost_connection_or_a_withdrawn_manager_halts_the_reconciliation() {
        let mut lost = lights(&THREE);
        lost.set_target(Some(warm()));
        lost.transport.lost = true;
        assert!(matches!(
            lost.reconcile(Instant::now(), &mut |_| true),
            Err(Halt::Lost(_))
        ));

        let mut withdrawn = lights(&THREE);
        assert!(matches!(
            withdrawn.handle(vec![GammaEvent::ManagerGone], Instant::now()),
            Err(Lost::ManagerGone)
        ));
    }

    #[test]
    fn an_unusable_gamma_size_is_a_refusal() {
        let mut lights = lights(&[(1, Answer::Size(1))]);
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());
        assert_eq!(lights.transport.count(1), 0);
        assert!(!lights.transport.live.contains(&1));
        assert!(matches!(
            lights.status(),
            Some(NightLightEvent::NotApplied(_))
        ));
    }

    #[test]
    fn the_command_queue_holds_four_and_refuses_the_fifth() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = NightLightHandle { commands };
        for _ in 0..COMMAND_QUEUE {
            assert_eq!(handle.set(Some(2700)), Ok(()));
        }
        assert_eq!(handle.set(None), Err(NightLightRefused::Busy));

        drop(queue);
        assert_eq!(handle.set(None), Err(NightLightRefused::Gone));
    }

    #[test]
    fn the_publisher_reports_changes_only() {
        let mut said = Vec::new();
        let mut out = Publisher::new(|event| {
            said.push(event);
            true
        });
        assert!(out.say(NightLightEvent::Off));
        assert!(out.say(NightLightEvent::Off));
        assert!(out.say(NightLightEvent::Applied));
        drop(out);
        assert_eq!(said, vec![NightLightEvent::Off, NightLightEvent::Applied]);
    }

    /// Waits for the next published event, bounded.
    fn next(events: &Receiver<NightLightEvent>) -> NightLightEvent {
        events
            .recv_timeout(Duration::from_secs(5))
            .expect("the worker publishes within five seconds")
    }

    #[test]
    fn the_worker_follows_set_on_and_off_and_restores_on_shutdown() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = NightLightHandle { commands };
        let (said, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let worker = thread::spawn(move || {
            let mut lights = lights(&THREE);
            let mut out = Publisher::new(|event| said.send(event).is_ok());
            let exit = serve(&mut lights, &queue, &mut out, &worker_shutdown, false);
            lights.teardown(true, &mut |_| true);
            (exit.ok(), lights.transport)
        });

        assert_eq!(next(&events), NightLightEvent::Off);
        handle.set(Some(2700)).expect("the queue has room");
        assert_eq!(next(&events), NightLightEvent::Applied);
        handle.set(None).expect("the queue has room");
        assert_eq!(next(&events), NightLightEvent::Off);
        handle.set(Some(3000)).expect("the queue has room");
        assert_eq!(next(&events), NightLightEvent::Applied);

        shutdown.store(true, Ordering::Release);
        let (exit, fake) = worker.join().expect("the worker does not panic");
        assert_eq!(exit, Some(Exit::Shutdown));
        assert!(fake.live.is_empty(), "shutdown released every controller");
        for output in [1, 2, 3] {
            assert_eq!(fake.created(output), 2, "one controller per lit period");
            assert_eq!(fake.last(output), Some(&ramp(Whitepoint::NEUTRAL)));
        }
    }

    #[test]
    fn the_worker_serves_the_last_command_then_stops_when_every_handle_is_dropped() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = NightLightHandle { commands };
        handle.set(None).expect("the queue has room");
        handle.set(Some(2700)).expect("the queue has room");
        drop(handle);
        let shutdown = AtomicBool::new(false);
        let mut lights = lights(&THREE);
        let mut said = Vec::new();
        let mut out = Publisher::new(|event| {
            said.push(event);
            true
        });

        let exit = serve(&mut lights, &queue, &mut out, &shutdown, false);
        drop(out);

        assert_eq!(exit.ok(), Some(Exit::HandleGone));
        assert_eq!(
            said,
            vec![NightLightEvent::Applied],
            "waiting commands coalesce to the latest"
        );
    }

    #[test]
    fn without_a_connection_off_is_off_and_on_is_unavailable() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = NightLightHandle { commands };
        handle.set(Some(2700)).expect("the queue has room");
        handle.set(None).expect("the queue has room");
        drop(handle);
        let shutdown = AtomicBool::new(false);
        let mut said = Vec::new();
        let mut out = Publisher::new(|event| {
            said.push(event);
            true
        });

        idle_unavailable(&queue, &mut out, &shutdown);
        drop(out);

        assert_eq!(
            said,
            vec![
                NightLightEvent::NotApplied(UNAVAILABLE.to_owned()),
                NightLightEvent::Off
            ]
        );
    }
}
