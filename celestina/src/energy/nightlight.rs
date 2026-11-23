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
// of failing dark. CEL-2-B-GAMMA added two rules after the first live run: a
// sweep confirms each frame with a display sync before it sends the next,
// skips the frames the compositor's pace left behind and never sends a
// table an output already shows, and a connection lost for a cause that may
// pass is opened again after the same backoff while the light is wanted,
// instead of leaving the worker idle for good.

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
//! - a table is not cheap for the compositor: niri applies each one in a
//!   blocking commit on its main thread, a refresh period or two of that
//!   output. So a sweep sends one frame to every output and waits for the
//!   display sync that confirms it before it sends the next: at most one
//!   table per output is ever in flight and the compositor's pace bounds the
//!   worker's. The frames that pace left behind are skipped, the last one,
//!   the target, never is, and the first frame of a transition, which is
//!   what the output already shows, is not sent;
//! - a controller the compositor refuses (`failed`) is destroyed at once, so
//!   that output is at the compositor's own gamma, never left at a table half
//!   way; the light reports [`NightLightEvent::NotApplied`] and the output is
//!   tried again after a backoff of 1 s doubling to 30 s;
//! - any error of the connection itself fails dark: every controller is
//!   destroyed and the light reports `NotApplied`. A cause that may pass (a
//!   sync not answered in time, a socket that broke or flooded, a withdrawn
//!   manager) is then tried again: while the light is still wanted the worker
//!   opens a new connection after the same backoff of 1 s doubling to 30 s,
//!   counted over the connections lost in a row and forgotten once one of
//!   them settles or the light is switched off, and sweeps to the warmth
//!   last asked for. A connection lost while the light is off reports `Off`:
//!   nothing was asked for, and nothing connects until something is;
//! - with no compositor to connect to, or none that offers
//!   `wlr-gamma-control`, waiting changes nothing: the light reports
//!   `NotApplied` and the worker idles, answering off as off.
//!
//! Commands cross into the worker through a queue of
//! [`COMMAND_QUEUE`](super::wire::COMMAND_QUEUE); the
//! worker coalesces whatever is waiting to the latest command, also while it
//! waits to connect again. Every wait is bounded, so
//! [`crate::providers::Worker::join`] is too.

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
    /// The worker drains the queue every poll, during a sweep and while it
    /// waits to connect again, so a full queue is a stuck worker.
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
        supervise(
            &mut WaylandTransport::connect,
            &Monotonic,
            &queue,
            &mut Publisher::new(publish),
            &worker_shutdown,
        );
    })?;
    Ok((NightLightHandle { commands }, worker))
}

/// How long an output waits before its refused controller is rebuilt, after
/// `attempts` earlier rebuilds of it: 1 s, doubling, to a 30 s ceiling. A
/// lost connection waits the same before the worker opens a new one, after
/// `attempts` earlier losses in a row.
///
/// Rebuilding is not invisible: a new controller starts a fresh warm sweep
/// on that output. Most refusals are transient (an output still settling
/// after a hotplug), so the first retry is quick; an output that simply
/// cannot hold a controller climbs to the ceiling and stops costing the
/// session anything visible. A connection whose sync is never answered
/// costs the compositor more than a second of its main thread per attempt,
/// which is why it climbs the same way.
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

impl Lost {
    /// Whether a new connection may fare better. No compositor to connect
    /// to and a compositor without gamma control do not change by waiting;
    /// every other loss is a connection that existed and then stalled or
    /// broke, the first sync of a new one included. A withdrawn manager
    /// counts as such: the next connection finds it back, or finds none and
    /// settles it.
    fn transient(&self) -> bool {
        !matches!(self, Self::NoManager | Self::Wire(wire::Lost::Connect(_)))
    }
}

/// The worker's time. A seam, so a sweep's pace and the wait for a new
/// connection are tested without waiting.
trait Clock: Clone {
    fn now(&self) -> Instant;
    /// Lets `wait` pass.
    fn sleep(&self, wait: Duration);
}

/// The system's monotonic clock.
#[derive(Clone, Copy, Debug)]
struct Monotonic;

impl Clock for Monotonic {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, wait: Duration) {
        thread::sleep(wait);
    }
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

/// What one finished sweep cost the compositor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Swept {
    /// How many frames of the schedule were sent.
    played: usize,
    /// How many frames the schedule had.
    scheduled: usize,
    /// The longest wait for the sync that confirms one frame.
    slowest_confirm: Duration,
}

/// The frame to play after `current` when the sweep is `elapsed` old: the
/// newest later frame that is already due, so a compositor slower than the
/// schedule skips what it could not show in time, else the frame right after
/// `current`. Never backwards and never past the last frame, which is the
/// target: `None` only once `current` is the last.
fn next_frame(offsets: &[Duration], current: usize, elapsed: Duration) -> Option<usize> {
    let next = current.checked_add(1)?;
    let later = offsets.get(next..).filter(|later| !later.is_empty())?;
    let newest_due = later.iter().rposition(|offset| *offset <= elapsed);
    Some(next + newest_due.unwrap_or(0))
}

/// The frame of one output's plan to send when the sweep plays the
/// schedule's frame `index`, having last played `played`: the plan's own
/// frame there, or its last one when the schedule is already beyond it, so a
/// skip never jumps over the end of a plan shorter than the schedule; and
/// nothing once that last frame has been sent.
fn frame_at(
    frames: &[TransitionFrame],
    played: Option<usize>,
    index: usize,
) -> Option<&TransitionFrame> {
    let last = frames.len().checked_sub(1)?;
    if played.is_some_and(|played| played >= last) {
        return None;
    }
    frames.get(index.min(last))
}

/// The controller state machine over a [`Transport`].
struct Lights<T: Transport, C: Clock> {
    transport: T,
    clock: C,
    outputs: BTreeMap<u32, Slot>,
    /// `None` is off.
    target: Option<Whitepoint>,
}

impl<T: Transport, C: Clock> Lights<T, C> {
    fn new(transport: T, clock: C) -> Self {
        Self {
            transport,
            clock,
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

    /// Destroys `output`'s controller and schedules its rebuild, counted
    /// from this instant: a refusal late in a long sweep waits its whole
    /// backoff.
    fn fail_output(&mut self, output: u32) {
        let now = self.clock.now();
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

    fn handle(&mut self, events: Vec<GammaEvent>) -> Result<(), Lost> {
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
                        self.fail_output(output);
                    }
                }
                GammaEvent::Failed(output) => {
                    if self.control_mut(output).is_some() {
                        tracing::warn!("night light: the compositor refused output {output}");
                        self.fail_output(output);
                    }
                }
                GammaEvent::ManagerGone => return Err(Lost::ManagerGone),
            }
        }
        Ok(())
    }

    /// Creates the controllers that are due; returns whether any was.
    fn create_due(&mut self) -> bool {
        if self.target.is_none() {
            return false;
        }
        let now = self.clock.now();
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
                    self.fail_output(output);
                }
            }
        }
        created
    }

    /// The frames each ready output still has to travel. Leaving or reaching
    /// neutral sweeps; moving between two warm points lands in one frame,
    /// because a person dragging the warmth is already the motion. A
    /// transition starts on what the output already shows, and that frame is
    /// left out: it would cost the compositor a commit for no change. The
    /// frames that remain keep their own instants.
    fn plans(&self) -> Vec<(u32, Vec<TransitionFrame>)> {
        let target = self.target.unwrap_or(Whitepoint::NEUTRAL);
        self.outputs
            .iter()
            .filter_map(|(output, slot)| {
                let control = slot.control.as_ref()?;
                if control.size.is_none() || control.applied == target {
                    return None;
                }
                let mut frames =
                    if control.applied == Whitepoint::NEUTRAL || target == Whitepoint::NEUTRAL {
                        transition(control.applied, target)
                    } else {
                        vec![TransitionFrame {
                            offset: Duration::ZERO,
                            whitepoint: target,
                        }]
                    };
                let shown = frames
                    .iter()
                    .take_while(|frame| frame.whitepoint == control.applied)
                    .count();
                frames.drain(..shown);
                Some((*output, frames))
            })
            .collect()
    }

    /// Sends `output` its table for `whitepoint`; a failure fails the output.
    fn apply(&mut self, output: u32, whitepoint: Whitepoint) {
        let Some(control) = self.control_mut(output) else {
            return;
        };
        let Some(ramp) = control.size.and_then(|size| gamma_ramp(size, whitepoint)) else {
            self.fail_output(output);
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
                self.fail_output(output);
            }
        }
    }

    /// Sends what is queued, waits until the compositor has handled it and
    /// handles what it reported meanwhile.
    fn confirm(&mut self) -> Result<(), Lost> {
        self.transport.flush()?;
        let events = self.transport.sync(self.clock.now() + SYNC_TIMEOUT)?;
        self.handle(events)
    }

    /// Plays `plans` frame by frame. `pace` releases each frame with the
    /// instant it is due; the frame goes to every output and is confirmed
    /// before the next one, so at most one table per output is in flight
    /// and the compositor's pace bounds this one. What that pace left behind
    /// is skipped ([`next_frame`]); every plan still ends on its last frame.
    /// An output refused or unplugged on the way is handled at once and gets
    /// no further frame. A finished sweep says in the journal, at debug
    /// level, how many frames it played and how long the slowest took.
    fn sweep(
        &mut self,
        plans: &[(u32, Vec<TransitionFrame>)],
        pace: &mut dyn FnMut(Instant) -> bool,
    ) -> Result<Swept, Halt> {
        let Some(schedule) = plans
            .iter()
            .map(|(_, frames)| frames)
            .max_by_key(|frames| frames.len())
        else {
            return Ok(Swept::default());
        };
        let offsets: Vec<Duration> = schedule.iter().map(|frame| frame.offset).collect();
        let mut swept = Swept {
            scheduled: offsets.len(),
            ..Swept::default()
        };
        let started = self.clock.now();
        let mut played = None;
        let mut index = 0;
        while let Some(offset) = offsets.get(index) {
            if !pace(started + *offset) {
                return Err(Halt::Aborted);
            }
            for (output, frames) in plans {
                if let Some(frame) = frame_at(frames, played, index) {
                    self.apply(*output, frame.whitepoint);
                }
            }
            let sent = self.clock.now();
            self.confirm()?;
            let confirmed = self.clock.now();
            swept.played += 1;
            swept.slowest_confirm = swept
                .slowest_confirm
                .max(confirmed.saturating_duration_since(sent));
            let elapsed = confirmed.saturating_duration_since(started);
            let Some(next) = next_frame(&offsets, index, elapsed) else {
                break;
            };
            played = Some(index);
            index = next;
        }
        tracing::debug!(
            "night light: sweep played {} of {} frames, slowest confirm {} ms",
            swept.played,
            swept.scheduled,
            swept.slowest_confirm.as_millis()
        );
        Ok(swept)
    }

    /// Brings every output to the target: creates what is due, sweeps what
    /// is ready, and releases every controller once the light is off.
    fn reconcile(&mut self, pace: &mut dyn FnMut(Instant) -> bool) -> Result<(), Halt> {
        for _ in 0..MAX_ROUNDS {
            if self.create_due() {
                self.confirm()?;
            }
            let plans = self.plans();
            if plans.is_empty() {
                break;
            }
            self.sweep(&plans, pace)?;
        }
        if self.target.is_none() && self.holds_any() {
            self.release_all();
            self.confirm()?;
        }
        Ok(())
    }

    /// Whether this connection has nothing left to do: every output that
    /// holds a controller shows the target, and with the light off no
    /// controller is left. An output that refuses its controller does not
    /// count against it: that is the output's own backoff, not the
    /// connection's.
    fn settled(&self) -> bool {
        match self.target {
            None => !self.holds_any(),
            Some(target) => self
                .outputs
                .values()
                .filter_map(|slot| slot.control.as_ref())
                .all(|control| control.size.is_some() && control.applied == target),
        }
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
            if let Err(Halt::Lost(lost)) = self.sweep(&plans, pace) {
                tracing::warn!("night light: {}", crate::error_chain(&lost));
            }
        }
        self.release_all();
        if self.transport.flush().is_ok() {
            let _ = self.transport.sync(self.clock.now() + SYNC_TIMEOUT);
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

/// What the worker keeps from one connection to the next.
#[derive(Debug, Default)]
struct Standing {
    /// The latest command taken off the queue and not acted on yet. A
    /// command that arrived during a sweep which then lost the connection
    /// waits here for [`wait_to_connect`].
    pending: Option<Option<u32>>,
    /// The warmth last asked for, in kelvin; `None` is off.
    wanted: Option<u32>,
    /// How many connections in a row were lost, which is what the next wait
    /// is measured by. Forgotten once a connection settles
    /// ([`Lights::settled`]: every output that takes a controller shows the
    /// target, or the light is off and nothing is held), so one output that
    /// keeps refusing does not make every later loss wait longer; and
    /// forgotten when the light is switched off, as each output's own
    /// refusals are.
    losses: u32,
}

impl Standing {
    /// Takes `kelvin` as what is wanted from now on.
    fn ask(&mut self, kelvin: Option<u32>) {
        if kelvin.is_none() {
            self.losses = 0;
        }
        self.wanted = kelvin;
    }
}

/// Moves the waiting commands into `pending`, the latest last; `true` once
/// every handle is gone.
fn drain(commands: &Receiver<Option<u32>>, pending: &mut Option<Option<u32>>) -> bool {
    loop {
        match commands.try_recv() {
            Ok(command) => *pending = Some(command),
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => return true,
        }
    }
}

/// Waits until `at` in steps of [`POLL`], moving waiting commands into
/// `pending` so the queue never fills behind a sweep; `false` once
/// `shutdown` is raised.
fn wait_until<C: Clock>(
    clock: &C,
    at: Instant,
    commands: &Receiver<Option<u32>>,
    pending: &mut Option<Option<u32>>,
    shutdown: &AtomicBool,
) -> bool {
    loop {
        drain(commands, pending);
        if shutdown.load(Ordering::Acquire) {
            return false;
        }
        match at.checked_duration_since(clock.now()) {
            Some(remaining) if !remaining.is_zero() => clock.sleep(remaining.min(POLL)),
            _ => return true,
        }
    }
}

/// Sleeps until `at`, ignoring shutdown: a restoration sweep is bounded by
/// its own 300 ms and the syncs that confirm its frames, and a frame that
/// is already due does not sleep.
fn sleep_until<C: Clock>(clock: &C, at: Instant) -> bool {
    if let Some(remaining) = at.checked_duration_since(clock.now()) {
        clock.sleep(remaining);
    }
    true
}

/// The worker's loop over a connected transport, starting from the target
/// `lights` already has. `paced` is false only in tests, where frames are
/// not spaced in time. `standing` follows the warmth asked for, keeps a
/// command this connection did not live to act on, and forgets the lost
/// connections once this one settles.
fn serve<T: Transport, C: Clock, F: FnMut(NightLightEvent) -> bool>(
    lights: &mut Lights<T, C>,
    commands: &Receiver<Option<u32>>,
    out: &mut Publisher<F>,
    shutdown: &AtomicBool,
    paced: bool,
    standing: &mut Standing,
) -> Result<Exit, Lost> {
    let clock = lights.clock.clone();
    loop {
        if shutdown.load(Ordering::Acquire) {
            return Ok(Exit::Shutdown);
        }
        let handle_gone = drain(commands, &mut standing.pending);
        if let Some(kelvin) = standing.pending.take() {
            standing.ask(kelvin);
            lights.set_target(kelvin.map(Whitepoint::for_temperature));
        }
        let events = lights
            .transport
            .poll(if paced { POLL } else { Duration::ZERO })?;
        lights.handle(events)?;
        let mut pace = |at: Instant| {
            if paced {
                wait_until(&clock, at, commands, &mut standing.pending, shutdown)
            } else {
                !shutdown.load(Ordering::Acquire)
            }
        };
        match lights.reconcile(&mut pace) {
            Ok(()) => {}
            Err(Halt::Aborted) => return Ok(Exit::Shutdown),
            Err(Halt::Lost(lost)) => return Err(lost),
        }
        if lights.settled() {
            standing.losses = 0;
        }
        if let Some(status) = lights.status() {
            if !out.say(status) {
                return Ok(Exit::LoopGone);
            }
        }
        if handle_gone && standing.pending.is_none() {
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

/// Waits, after a connection lost for `reason`, until the light is wanted
/// and `retry_at` has passed, in steps of [`POLL`]. It reports where the
/// light stands, first for the loss and then for every command: off is off,
/// also when the connection was lost with the light off, and nothing is
/// connected until the light is asked for again; a warmth is not applied
/// yet and becomes what the next connection sweeps to, without shortening
/// the wait. `false` when the worker has to stop instead: shutdown, a loop
/// that no longer listens, or every handle dropped.
fn wait_to_connect<C: Clock, F: FnMut(NightLightEvent) -> bool>(
    clock: &C,
    retry_at: Instant,
    reason: &str,
    commands: &Receiver<Option<u32>>,
    out: &mut Publisher<F>,
    shutdown: &AtomicBool,
    standing: &mut Standing,
) -> bool {
    let mut news = true;
    loop {
        let handle_gone = drain(commands, &mut standing.pending);
        if let Some(kelvin) = standing.pending.take() {
            standing.ask(kelvin);
            news = true;
        }
        if news {
            let event = match standing.wanted {
                None => NightLightEvent::Off,
                Some(_) => NightLightEvent::NotApplied(reason.to_owned()),
            };
            if !out.say(event) {
                return false;
            }
            news = false;
        }
        if handle_gone || shutdown.load(Ordering::Acquire) {
            return false;
        }
        let remaining = retry_at.saturating_duration_since(clock.now());
        if remaining.is_zero() {
            if standing.wanted.is_some() {
                return true;
            }
            clock.sleep(POLL);
        } else {
            clock.sleep(remaining.min(POLL));
        }
    }
}

/// The worker's life over the connections `connect` opens: serve one until
/// it ends; when it is lost, fail dark, and when the cause may pass, open
/// the next one after the backoff while the light is still wanted.
fn supervise<T: Transport, C: Clock, F: FnMut(NightLightEvent) -> bool>(
    connect: &mut dyn FnMut() -> Result<(T, Vec<GammaEvent>), Lost>,
    clock: &C,
    commands: &Receiver<Option<u32>>,
    out: &mut Publisher<F>,
    shutdown: &AtomicBool,
) {
    let mut standing = Standing::default();
    loop {
        let (lost, broken) = match connect() {
            Ok((transport, events)) => {
                let mut lights = Lights::new(transport, clock.clone());
                lights.set_target(standing.wanted.map(Whitepoint::for_temperature));
                let outcome = lights.handle(events).and_then(|()| {
                    serve(&mut lights, commands, out, shutdown, true, &mut standing)
                });
                match outcome {
                    Ok(_) => {
                        lights.teardown(true, &mut |at| sleep_until(clock, at));
                        let _ = out.say(NightLightEvent::Off);
                        return;
                    }
                    Err(lost) => (lost, Some(lights)),
                }
            }
            Err(lost) => (lost, None),
        };
        let reason = crate::error_chain(&lost);
        tracing::warn!("night light: {reason}");
        if let Some(mut lights) = broken {
            // Fail dark: destroy what is left, so no output keeps a table
            // half way; closing the connection then makes the compositor
            // restore every output by protocol.
            lights.teardown(false, &mut |at| sleep_until(clock, at));
        }
        if !lost.transient() {
            if out.say(NightLightEvent::NotApplied(UNAVAILABLE.to_owned())) {
                idle_unavailable(commands, out, shutdown);
            }
            return;
        }
        let wait = retry_backoff(standing.losses);
        standing.losses = standing.losses.saturating_add(1);
        let retry_at = clock.now() + wait;
        if !wait_to_connect(
            clock,
            retry_at,
            &reason,
            commands,
            out,
            shutdown,
            &mut standing,
        ) {
            return;
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
    use std::sync::Mutex;
    use wayland_client::backend::WaylandError;
    use wayland_client::ConnectError;

    const SIZE: u32 = 256;

    /// Something a test set to happen at a fake instant.
    type Alarm = (Instant, Box<dyn FnOnce() + Send>);

    /// Fake time. It moves only when the worker sleeps on it or a fake
    /// compositor takes its time; what a test set with [`Ticker::at`]
    /// happens when it gets there.
    #[derive(Clone)]
    struct Ticker {
        time: Arc<Mutex<Instant>>,
        alarms: Arc<Mutex<Vec<Alarm>>>,
        /// The shutdown flag of the worker a test runs on this time.
        shutdown: Arc<AtomicBool>,
    }

    impl Default for Ticker {
        fn default() -> Self {
            Self {
                time: Arc::new(Mutex::new(Instant::now())),
                alarms: Arc::default(),
                shutdown: Arc::default(),
            }
        }
    }

    impl Ticker {
        fn advance_to(&self, at: Instant) {
            let now = {
                let mut time = self.time.lock().expect("the fake time is not poisoned");
                *time = (*time).max(at);
                *time
            };
            let due: Vec<Alarm> = {
                let mut alarms = self.alarms.lock().expect("the alarms are not poisoned");
                let (due, later) = std::mem::take(&mut *alarms)
                    .into_iter()
                    .partition(|(when, _)| *when <= now);
                *alarms = later;
                due
            };
            for (_, ring) in due {
                ring();
            }
        }

        /// Runs `what` once the fake time reaches `when`.
        fn at(&self, when: Instant, what: impl FnOnce() + Send + 'static) {
            self.alarms
                .lock()
                .expect("the alarms are not poisoned")
                .push((when, Box::new(what)));
        }

        /// Raises the shutdown flag once the fake time reaches `when`.
        fn stop_at(&self, when: Instant) {
            let shutdown = Arc::clone(&self.shutdown);
            self.at(when, move || shutdown.store(true, Ordering::Release));
        }
    }

    impl Clock for Ticker {
        fn now(&self) -> Instant {
            *self.time.lock().expect("the fake time is not poisoned")
        }

        fn sleep(&self, wait: Duration) {
            self.advance_to(self.now() + wait);
        }
    }

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
    /// Wayland transport does while dispatching. Its time is a [`Ticker`]:
    /// a sync costs `sync_takes` of it and a poll its whole timeout.
    #[derive(Default)]
    struct Fake {
        answers: BTreeMap<u32, Answer>,
        queued: Vec<GammaEvent>,
        live: BTreeSet<u32>,
        created: Vec<u32>,
        destroyed: Vec<u32>,
        ramps: BTreeMap<u32, Vec<Vec<u16>>>,
        lost: bool,
        ticker: Ticker,
        /// How long this compositor takes to answer a sync.
        sync_takes: Duration,
        /// What every table sent since the last sync adds to that: the
        /// compositor applies each one before it answers.
        per_table: Duration,
        /// The tables sent since the last sync.
        in_flight: usize,
        /// The most tables one sync ever had before it.
        most_in_flight: usize,
        /// The longest it took to answer a sync.
        slowest_sync: Duration,
        /// Once it holds this many tables it answers no sync any more.
        silent_after: Option<usize>,
        /// How many polls the socket still lives; `None` is for ever.
        polls_left: Option<usize>,
        /// What it reports once it holds that many tables.
        later: Vec<(usize, GammaEvent)>,
        /// When the worker dropped this connection.
        closed: Option<Instant>,
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

        /// Every table it was given, over all outputs.
        fn tables(&self) -> usize {
            self.ramps.values().map(Vec::len).sum()
        }

        /// The table `output` shows: the last one its controller was given
        /// while that controller lives, and `None`, the compositor's own
        /// gamma, once it is destroyed.
        fn holds(&self, output: u32) -> Option<&Vec<u16>> {
            self.live
                .contains(&output)
                .then(|| self.last(output))
                .flatten()
        }

        /// What the compositor has to report now.
        fn answer(&mut self) -> Vec<GammaEvent> {
            let tables = self.tables();
            let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later)
                .into_iter()
                .partition(|(at, _)| *at <= tables);
            self.later = later;
            self.queued.extend(due.into_iter().map(|(_, event)| event));
            let events = std::mem::take(&mut self.queued);
            for event in &events {
                if let GammaEvent::Failed(output) | GammaEvent::OutputRemoved(output) = event {
                    self.live.remove(output);
                }
            }
            events
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
            self.in_flight += 1;
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

        fn sync(&mut self, deadline: Instant) -> Result<Vec<GammaEvent>, Lost> {
            self.flush()?;
            let in_flight = std::mem::take(&mut self.in_flight);
            self.most_in_flight = self.most_in_flight.max(in_flight);
            let tables = u32::try_from(in_flight).expect("a handful of tables");
            let cost = self.sync_takes + self.per_table * tables;
            let answered = self.ticker.now() + cost;
            let silent = self
                .silent_after
                .is_some_and(|tables| self.tables() >= tables);
            if silent || answered > deadline {
                // A compositor that does not answer in time costs the whole
                // wait.
                self.ticker.advance_to(deadline);
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            self.slowest_sync = self.slowest_sync.max(cost);
            self.ticker.advance_to(answered);
            Ok(self.answer())
        }

        fn poll(&mut self, timeout: Duration) -> Result<Vec<GammaEvent>, Lost> {
            self.flush()?;
            if let Some(left) = self.polls_left.as_mut() {
                let Some(fewer) = left.checked_sub(1) else {
                    return Err(Lost::Wire(wire::Lost::Read(WaylandError::Io(
                        io::Error::from(io::ErrorKind::ConnectionReset),
                    ))));
                };
                *left = fewer;
            }
            self.ticker.sleep(timeout);
            Ok(self.answer())
        }
    }

    fn lights(outputs: &[(u32, Answer)]) -> Lights<Fake, Ticker> {
        let (fake, events) = Fake::with(outputs);
        let clock = fake.ticker.clone();
        let mut lights = Lights::new(fake, clock);
        lights
            .handle(events)
            .expect("adding outputs cannot lose the connection");
        lights
    }

    fn warm() -> Whitepoint {
        Whitepoint::for_temperature(2700)
    }

    fn ramp(whitepoint: Whitepoint) -> Vec<u16> {
        gamma_ramp(SIZE, whitepoint).expect("the fixture size is valid")
    }

    /// Reconciles at `now`, which the fake time is moved to, without pacing
    /// and returns how many frames were released.
    fn reconcile(lights: &mut Lights<Fake, Ticker>, now: Instant) -> usize {
        lights.clock.advance_to(now);
        let mut frames = 0;
        lights
            .reconcile(&mut |_| {
                frames += 1;
                true
            })
            .expect("the fake connection holds");
        frames
    }

    /// The tables one whole sweep sends an output: the 19 frames of the
    /// transition without the first, which the output already shows.
    const SWEEP: usize = 18;

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
        assert_eq!(frames, SWEEP, "one 300 ms sweep, paced once per frame");
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), SWEEP);
            assert_ne!(
                lights.transport.ramps[&output][0],
                ramp(Whitepoint::NEUTRAL),
                "what the output already shows is not sent"
            );
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

        assert_eq!(frames, SWEEP);
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), 2 * SWEEP);
            assert_ne!(
                lights.transport.ramps[&output][SWEEP],
                ramp(warm()),
                "nor on the way back"
            );
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
        assert_eq!(
            lights.transport.count(1),
            1,
            "the refusal is handled before the next frame"
        );
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
            .handle(vec![GammaEvent::OutputRemoved(2)])
            .expect("a removal is not a loss");
        assert_eq!(lights.transport.destroyed, vec![2]);
        assert!(!lights.outputs.contains_key(&2));
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));

        lights.transport.answers.insert(3, Answer::Size(SIZE));
        lights
            .handle(vec![GammaEvent::OutputAdded(3)])
            .expect("an addition is not a loss");
        reconcile(&mut lights, Instant::now());
        assert_eq!(
            lights.transport.count(1),
            SWEEP,
            "the lit output is untouched"
        );
        assert_eq!(
            lights.transport.count(3),
            SWEEP,
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
            assert_eq!(lights.transport.count(output), SWEEP + 1);
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
        let halted = lights.reconcile(&mut |_| {
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
        assert!(matches!(lost.reconcile(&mut |_| true), Err(Halt::Lost(_))));

        let mut withdrawn = lights(&THREE);
        assert!(matches!(
            withdrawn.handle(vec![GammaEvent::ManagerGone]),
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
            let exit = serve(
                &mut lights,
                &queue,
                &mut out,
                &worker_shutdown,
                false,
                &mut Standing::default(),
            );
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

        let exit = serve(
            &mut lights,
            &queue,
            &mut out,
            &shutdown,
            false,
            &mut Standing::default(),
        );
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

    // The pacing of a sweep.

    /// The schedule of one whole transition.
    fn offsets() -> Vec<Duration> {
        transition(Whitepoint::NEUTRAL, warm())
            .iter()
            .map(|frame| frame.offset)
            .collect()
    }

    /// The frames a sweep plays against a compositor that takes `cost` to
    /// confirm each one, by [`next_frame`] alone.
    fn walk(offsets: &[Duration], cost: Duration) -> Vec<usize> {
        let mut played = vec![0];
        let mut index = 0;
        let mut at = Duration::ZERO;
        while played.len() <= offsets.len() {
            at = at.max(offsets[index]) + cost;
            let Some(next) = next_frame(offsets, index, at) else {
                break;
            };
            played.push(next);
            index = next;
        }
        played
    }

    #[test]
    fn a_compositor_that_keeps_the_pace_is_given_every_frame() {
        let offsets = offsets();
        let last = offsets.len() - 1;
        for current in 0..last {
            // Confirmed at once, a moment later, and just before the next
            // frame is due: the next frame is the next one.
            for elapsed in [
                Duration::ZERO,
                offsets[current],
                offsets[current] + Duration::from_millis(2),
                offsets[current + 1] - Duration::from_nanos(1),
                offsets[current + 1],
            ] {
                assert_eq!(
                    next_frame(&offsets, current, elapsed),
                    Some(current + 1),
                    "after frame {current}, {elapsed:?} into the sweep"
                );
            }
        }
        assert_eq!(
            walk(&offsets, Duration::from_millis(2)),
            (0..=last).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_slower_compositor_skips_to_the_newest_frame_already_due() {
        let offsets = offsets();
        // 100 ms in, frames 1 to 6 are due (frame 6 at exactly 100 ms).
        assert_eq!(next_frame(&offsets, 0, Duration::from_millis(100)), Some(6));
        assert_eq!(
            next_frame(&offsets, 2, offsets[7] + Duration::from_millis(1)),
            Some(7)
        );
        // Already past what is due: forward by one, never backwards.
        assert_eq!(
            next_frame(&offsets, 10, Duration::from_millis(100)),
            Some(11)
        );

        let played = walk(&offsets, Duration::from_millis(40));
        assert_eq!(played, vec![0, 2, 4, 7, 9, 12, 14, 16, 18]);
    }

    #[test]
    fn past_the_whole_transition_the_next_frame_is_the_target() {
        let offsets = offsets();
        let last = offsets.len() - 1;
        for current in 0..last {
            for elapsed in [offsets[last], Duration::from_secs(10), Duration::MAX] {
                assert_eq!(next_frame(&offsets, current, elapsed), Some(last));
            }
        }
        assert_eq!(walk(&offsets, Duration::from_secs(1)), vec![0, last]);
    }

    #[test]
    fn the_last_frame_has_no_next_and_a_schedule_of_one_ends_at_once() {
        let offsets = offsets();
        let last = offsets.len() - 1;
        for elapsed in [Duration::ZERO, Duration::from_secs(10)] {
            assert_eq!(next_frame(&offsets, last, elapsed), None);
            assert_eq!(next_frame(&offsets, last + 1, elapsed), None);
            assert_eq!(next_frame(&offsets, usize::MAX, elapsed), None);
            assert_eq!(next_frame(&[], 0, elapsed), None);
            assert_eq!(next_frame(&[Duration::ZERO], 0, elapsed), None);
        }
    }

    #[test]
    fn every_next_frame_is_later_due_or_adjacent_and_every_pace_ends_on_the_target() {
        let offsets = offsets();
        let last = offsets.len() - 1;
        for current in 0..last {
            for millis in 0..=400 {
                let elapsed = Duration::from_millis(millis);
                let next = next_frame(&offsets, current, elapsed)
                    .expect("a frame before the last has a next one");
                assert!(next > current && next <= last, "{current} -> {next}");
                assert!(
                    next == current + 1 || offsets[next] <= elapsed,
                    "a skipped-to frame is due: {current} -> {next} at {elapsed:?}"
                );
                assert!(
                    next == last || offsets[next + 1] > elapsed,
                    "no newer frame is due: {current} -> {next} at {elapsed:?}"
                );
            }
        }
        for millis in 0..=1200 {
            let played = walk(&offsets, Duration::from_millis(millis));
            assert_eq!(played.first(), Some(&0));
            assert_eq!(played.last(), Some(&last), "{millis} ms a frame");
            assert!(played.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    /// Reconciles with every frame released at the instant it is due, in
    /// fake time.
    fn reconcile_in_time(lights: &mut Lights<Fake, Ticker>) {
        let clock = lights.clock.clone();
        lights
            .reconcile(&mut |at| {
                clock.advance_to(at);
                true
            })
            .expect("the fake connection holds");
    }

    /// Creates the controllers that are due and plays the one sweep they
    /// owe, every frame released at the instant it is due, in fake time.
    fn sweep_in_time(lights: &mut Lights<Fake, Ticker>) -> Swept {
        assert!(lights.create_due());
        lights.confirm().expect("the fake connection holds");
        let plans = lights.plans();
        let clock = lights.clock.clone();
        lights
            .sweep(&plans, &mut |at| {
                clock.advance_to(at);
                true
            })
            .expect("the fake connection holds")
    }

    /// The tables of the frames `indices` of the sweep from neutral to warm.
    fn frames(indices: &[usize]) -> Vec<Vec<u16>> {
        let sweep = transition(Whitepoint::NEUTRAL, warm());
        indices
            .iter()
            .map(|index| ramp(sweep[*index].whitepoint))
            .collect()
    }

    #[test]
    fn a_sweep_the_compositor_confirms_in_time_plays_every_frame_but_the_one_already_shown() {
        let mut lights = lights(&THREE);
        lights.transport.sync_takes = Duration::from_millis(2);
        lights.set_target(Some(warm()));
        let swept = sweep_in_time(&mut lights);

        // Frame 0 of the transition is neutral, which a new controller's
        // output already shows.
        let every: Vec<usize> = (1..19).collect();
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.ramps.get(&output), Some(&frames(&every)));
            assert_ne!(
                lights.transport.ramps[&output][0],
                ramp(Whitepoint::NEUTRAL)
            );
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
        assert_eq!(
            swept,
            Swept {
                played: SWEEP,
                scheduled: SWEEP,
                slowest_confirm: Duration::from_millis(2),
            }
        );
    }

    #[test]
    fn a_sweep_the_compositor_paces_slower_skips_frames_and_ends_on_the_target() {
        let mut lights = lights(&THREE);
        // 40 ms to apply one frame's tables: more than two 16.7 ms frames.
        lights.transport.sync_takes = Duration::from_millis(40);
        lights.set_target(Some(warm()));
        let started = lights.clock.now();
        let swept = sweep_in_time(&mut lights);

        let played = frames(&[1, 3, 5, 8, 10, 13, 15, 17, 18]);
        for output in [1, 2, 3] {
            assert!(lights.transport.count(output) < SWEEP);
            assert_eq!(lights.transport.ramps.get(&output), Some(&played));
            assert_eq!(lights.transport.last(output), Some(&ramp(warm())));
            assert_eq!(lights.transport.holds(output), Some(&ramp(warm())));
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
        assert_eq!(
            swept,
            Swept {
                played: 9,
                scheduled: SWEEP,
                slowest_confirm: Duration::from_millis(40),
            }
        );
        // One sync for the controllers, the wait for the first frame's
        // instant, and one sync per frame played: the pace is the
        // compositor's, and nothing waits behind it.
        assert_eq!(
            lights.clock.now() - started,
            Duration::from_millis(40) * 10 + offsets()[1]
        );
    }

    /// The first live run (1.13.0, three outputs): niri applies each table
    /// in a blocking commit before it answers anything sent after it. The
    /// old sweep flushed every frame and synced once behind all of them, and
    /// that sync was not answered in time.
    #[test]
    fn a_compositor_that_blocks_on_every_table_never_has_more_than_one_per_output_to_apply() {
        let per_table = Duration::from_millis(25);
        let behind_one_sync = u32::try_from(SWEEP * THREE.len()).expect("54 tables");
        assert!(
            per_table * behind_one_sync > SYNC_TIMEOUT,
            "one sync behind the whole sweep would not be answered in time"
        );

        let mut lights = lights(&THREE);
        lights.transport.per_table = per_table;
        lights.set_target(Some(warm()));
        let swept = sweep_in_time(&mut lights);

        assert_eq!(lights.transport.most_in_flight, THREE.len());
        assert_eq!(lights.transport.slowest_sync, per_table * 3);
        assert!(lights.transport.slowest_sync < SYNC_TIMEOUT);
        let played = frames(&[1, 5, 10, 14, 18]);
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.ramps.get(&output), Some(&played));
            assert_eq!(lights.transport.holds(output), Some(&ramp(warm())));
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
        assert_eq!(
            swept,
            Swept {
                played: 5,
                scheduled: SWEEP,
                slowest_confirm: per_table * 3,
            }
        );
    }

    #[test]
    fn a_compositor_slower_than_the_whole_transition_gets_one_frame_and_the_target() {
        let mut lights = lights(&THREE);
        lights.transport.sync_takes = Duration::from_millis(400);
        lights.set_target(Some(warm()));
        reconcile_in_time(&mut lights);

        for output in [1, 2, 3] {
            assert_eq!(lights.transport.ramps.get(&output), Some(&frames(&[1, 18])));
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));

        // The way back is paced the same and ends neutral.
        lights.set_target(None);
        reconcile_in_time(&mut lights);
        for output in [1, 2, 3] {
            assert_eq!(lights.transport.count(output), 4);
            assert_eq!(
                lights.transport.last(output),
                Some(&ramp(Whitepoint::NEUTRAL))
            );
        }
        assert!(lights.transport.live.is_empty());
        assert_eq!(lights.status(), Some(NightLightEvent::Off));
    }

    #[test]
    fn a_plan_shorter_than_the_schedule_still_ends_on_its_last_frame() {
        let mut lights = lights(&[(1, Answer::Size(SIZE)), (2, Answer::Size(SIZE))]);
        lights.transport.sync_takes = Duration::from_millis(40);
        lights.set_target(Some(warm()));
        assert!(lights.create_due());
        lights.confirm().expect("the fake connection holds");

        // Hand-made plans, as `plans` never makes them: a whole transition
        // and the first six frames of one.
        let whole = transition(Whitepoint::NEUTRAL, warm());
        let plans = vec![(1, whole.clone()), (2, whole[..6].to_vec())];
        let clock = lights.clock.clone();
        let swept = lights
            .sweep(&plans, &mut |at| {
                clock.advance_to(at);
                true
            })
            .expect("the fake connection holds");

        assert_eq!(
            lights.transport.ramps.get(&1),
            Some(&frames(&[0, 2, 4, 7, 9, 12, 14, 16, 18]))
        );
        // The schedule jumps from frame 4 to frame 7, over this plan's end:
        // it gets its own last frame then, and nothing afterwards.
        assert_eq!(lights.transport.ramps.get(&2), Some(&frames(&[0, 2, 4, 5])));
        assert_eq!((swept.played, swept.scheduled), (9, 19));
    }

    #[test]
    fn a_refusal_late_in_a_long_sweep_waits_its_whole_backoff() {
        let mut lights = lights(&THREE);
        lights.transport.sync_takes = Duration::from_millis(400);
        // Reported with the sync that confirms the second frame, the target.
        lights.transport.later = vec![(6, GammaEvent::Failed(2))];
        lights.set_target(Some(warm()));
        let started = lights.clock.now();
        reconcile_in_time(&mut lights);

        let refused = lights.clock.now();
        assert!(
            refused > started + Duration::from_secs(1),
            "a backoff counted from the start of this sweep would be over"
        );
        assert_eq!(
            lights.outputs.get(&2).and_then(|slot| slot.retry_at),
            Some(refused + Duration::from_secs(1))
        );
        assert_eq!(lights.transport.created(2), 1);
        reconcile(&mut lights, refused + Duration::from_millis(999));
        assert_eq!(lights.transport.created(2), 1, "not before its second");
        reconcile(&mut lights, refused + Duration::from_secs(1));
        assert_eq!(lights.transport.created(2), 2);
    }

    #[test]
    fn a_connection_is_settled_when_only_refusing_outputs_are_short_of_the_target() {
        let mut lights = lights(&[
            (1, Answer::Size(SIZE)),
            (2, Answer::Refuse),
            (3, Answer::Size(SIZE)),
        ]);
        assert!(lights.settled(), "off, and nothing held");

        lights.set_target(Some(warm()));
        assert!(lights.create_due());
        assert!(!lights.settled(), "controllers that have not answered");
        lights.confirm().expect("the fake connection holds");
        assert!(!lights.settled(), "a sweep is owed");
        reconcile(&mut lights, Instant::now());
        assert!(matches!(
            lights.status(),
            Some(NightLightEvent::NotApplied(_))
        ));
        assert!(lights.settled(), "output 2 refuses; the others are warm");

        lights.set_target(None);
        assert!(!lights.settled(), "two controllers are still held");
        reconcile(&mut lights, Instant::now());
        assert!(lights.settled());
    }

    #[test]
    fn an_output_unplugged_mid_sweep_gets_no_further_frame_and_the_others_reach_the_target() {
        let mut lights = lights(&THREE);
        // Reported with the sync that confirms the second frame.
        lights.transport.later = vec![(6, GammaEvent::OutputRemoved(2))];
        lights.set_target(Some(warm()));
        reconcile(&mut lights, Instant::now());

        assert_eq!(lights.transport.count(2), 2);
        assert_eq!(lights.transport.destroyed, vec![2]);
        assert!(!lights.outputs.contains_key(&2));
        for output in [1, 3] {
            assert_eq!(lights.transport.count(output), SWEEP);
            assert_eq!(lights.transport.holds(output), Some(&ramp(warm())));
        }
        assert_eq!(lights.status(), Some(NightLightEvent::Applied));
    }

    // The worker over its connections, in fake time.

    /// A [`Fake`] the test still reads after the worker dropped it.
    struct Shared(Arc<Mutex<Fake>>);

    impl Shared {
        fn fake(&self) -> std::sync::MutexGuard<'_, Fake> {
            self.0.lock().expect("the fake compositor is not poisoned")
        }
    }

    impl Drop for Shared {
        fn drop(&mut self) {
            if let Ok(mut fake) = self.0.lock() {
                fake.closed = Some(fake.ticker.now());
            }
        }
    }

    impl Transport for Shared {
        fn create(&mut self, output: u32) -> Result<(), OutputError> {
            self.fake().create(output)
        }

        fn destroy(&mut self, output: u32) {
            self.fake().destroy(output);
        }

        fn set_gamma(&mut self, output: u32, ramp: &[u16]) -> Result<(), OutputError> {
            self.fake().set_gamma(output, ramp)
        }

        fn flush(&mut self) -> Result<(), Lost> {
            self.fake().flush()
        }

        fn sync(&mut self, deadline: Instant) -> Result<Vec<GammaEvent>, Lost> {
            self.fake().sync(deadline)
        }

        fn poll(&mut self, timeout: Duration) -> Result<Vec<GammaEvent>, Lost> {
            self.fake().poll(timeout)
        }
    }

    /// One scripted connection; each opens on [`THREE`] outputs.
    enum Attempt {
        /// A compositor that keeps the pace.
        Healthy,
        /// It answers no sync once it holds this many tables.
        Chokes(usize),
        /// Its socket is reset after this many polls.
        Dies(usize),
        /// It withdraws the gamma manager at once.
        Withdraws,
        /// It refuses `output` its controller for good, and its socket is
        /// reset after `polls` polls.
        RefusesThenDies { output: u32, polls: usize },
        /// No connection.
        Fails(Lost),
    }

    /// The test's side of a scripted worker, handed to its reaction to each
    /// published event.
    struct Stage {
        handle: Option<NightLightHandle>,
        ticker: Ticker,
    }

    impl Stage {
        fn set(&self, kelvin: Option<u32>) {
            self.handle
                .as_ref()
                .expect("the handle is still held")
                .set(kelvin)
                .expect("the queue has room");
        }
    }

    /// What a scripted worker did.
    struct Told {
        /// Every published event and its fake instant.
        said: Vec<(NightLightEvent, Instant)>,
        /// When each connection was asked for.
        connects: Vec<Instant>,
        /// The compositor behind each connection that opened.
        fakes: Vec<Arc<Mutex<Fake>>>,
        started: Instant,
        ended: Instant,
    }

    impl Told {
        fn kinds(&self) -> Vec<&'static str> {
            self.said
                .iter()
                .map(|(event, _)| match event {
                    NightLightEvent::Applied => "applied",
                    NightLightEvent::NotApplied(why) if why == UNAVAILABLE => "unavailable",
                    NightLightEvent::NotApplied(_) => "not applied",
                    NightLightEvent::Off => "off",
                })
                .collect()
        }

        fn fake(&self, connection: usize) -> std::sync::MutexGuard<'_, Fake> {
            self.fakes[connection]
                .lock()
                .expect("the fake compositor is not poisoned")
        }

        /// How long the worker waited between dropping connection
        /// `connection` and asking for the next one.
        fn waited_after(&self, connection: usize) -> Duration {
            let closed = self
                .fake(connection)
                .closed
                .expect("the worker dropped that connection");
            self.connects[connection + 1] - closed
        }

        /// Whether the worker was still running when [`RUN`] was over: it
        /// stops on the first poll after that.
        fn ran_to_the_end(&self) -> bool {
            let over = self.started + RUN;
            self.ended >= over && self.ended < over + POLL
        }
    }

    /// How long a scripted worker runs before its shutdown is raised.
    const RUN: Duration = Duration::from_secs(300);

    /// Runs the worker over `script` (then healthy connections) in fake
    /// time, with the light asked on at 2700 K before it starts and `mail`
    /// sent at those offsets; `react` sees every published event. The run
    /// ends when the worker does: its shutdown is raised after [`RUN`], or
    /// earlier by `react`.
    fn supervised(
        script: Vec<Attempt>,
        mail: &[(Duration, Option<u32>)],
        mut react: impl FnMut(&NightLightEvent, &mut Stage),
    ) -> Told {
        let ticker = Ticker::default();
        let started = ticker.now();
        ticker.stop_at(started + RUN);
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = NightLightHandle { commands };
        for (after, kelvin) in mail.iter().copied() {
            let handle = handle.clone();
            ticker.at(started + after, move || {
                let _ = handle.set(kelvin);
            });
        }
        let mut stage = Stage {
            handle: Some(handle),
            ticker: ticker.clone(),
        };
        stage.set(Some(2700));

        let mut script = script.into_iter();
        let mut connects = Vec::new();
        let mut fakes = Vec::new();
        let mut connect = || {
            connects.push(ticker.now());
            let (mut fake, events) = Fake::with(&THREE);
            fake.ticker = ticker.clone();
            match script.next().unwrap_or(Attempt::Healthy) {
                Attempt::Healthy => {}
                Attempt::Chokes(tables) => fake.silent_after = Some(tables),
                Attempt::Dies(polls) => fake.polls_left = Some(polls),
                Attempt::Withdraws => fake.queued.push(GammaEvent::ManagerGone),
                Attempt::RefusesThenDies { output, polls } => {
                    fake.answers.insert(output, Answer::Refuse);
                    fake.polls_left = Some(polls);
                }
                Attempt::Fails(lost) => return Err(lost),
            }
            let fake = Arc::new(Mutex::new(fake));
            fakes.push(Arc::clone(&fake));
            Ok((Shared(fake), events))
        };
        let mut said = Vec::new();
        let mut out = Publisher::new(|event: NightLightEvent| {
            // After `unavailable` the worker only idles, in real time: end
            // the run there.
            if event == NightLightEvent::NotApplied(UNAVAILABLE.to_owned()) {
                ticker.shutdown.store(true, Ordering::Release);
            }
            let at = ticker.now();
            react(&event, &mut stage);
            said.push((event, at));
            true
        });
        supervise(&mut connect, &ticker, &queue, &mut out, &ticker.shutdown);
        drop(out);
        Told {
            said,
            connects,
            fakes,
            started,
            ended: ticker.now(),
        }
    }

    fn timed_out() -> NightLightEvent {
        NightLightEvent::NotApplied(crate::error_chain(&Lost::Wire(wire::Lost::Timeout)))
    }

    /// Five frames on three outputs: the fake compositor stops answering in
    /// the middle of the first sweep.
    const MID_SWEEP: usize = 15;

    #[test]
    fn a_sync_lost_mid_sweep_fails_dark_and_the_light_comes_back_by_itself() {
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |_, _| {});

        // Before the fix the worker idled here for good and every later
        // command was answered `unavailable`.
        assert_eq!(told.kinds(), vec!["not applied", "applied", "off"]);
        assert_eq!(told.said[0].0, timed_out());

        // Fail dark: half way through the sweep every controller is
        // destroyed, so no output shows a table half way.
        let lost = told.fake(0);
        assert!(lost.live.is_empty());
        let mut destroyed = lost.destroyed.clone();
        destroyed.sort_unstable();
        assert_eq!(destroyed, vec![1, 2, 3]);
        for output in [1, 2, 3] {
            assert_eq!(lost.count(output), 5);
            assert_ne!(lost.last(output), Some(&ramp(warm())));
            assert_eq!(lost.holds(output), None);
        }
        drop(lost);

        // One second later, with no new command, a new connection sweeps
        // to the same warmth; the shutdown that ends the run restores it.
        assert_eq!(told.connects.len(), 2);
        assert_eq!(told.waited_after(0), Duration::from_secs(1));
        assert_eq!(told.connects[1], told.said[0].1 + Duration::from_secs(1));
        let back = told.fake(1);
        for output in [1, 2, 3] {
            assert_eq!(back.created(output), 1);
            assert_eq!(back.ramps[&output][SWEEP - 1], ramp(warm()));
            assert_eq!(back.count(output), 2 * SWEEP);
            assert_eq!(back.last(output), Some(&ramp(Whitepoint::NEUTRAL)));
        }
        assert!(back.live.is_empty());
    }

    #[test]
    fn two_losses_in_a_row_wait_one_second_then_two_and_an_applied_light_forgets_them() {
        let told = supervised(
            vec![
                Attempt::Chokes(MID_SWEEP),
                Attempt::Chokes(MID_SWEEP),
                Attempt::Dies(8),
            ],
            &[],
            |_, _| {},
        );

        // The second loss repeats the first one's reason, so it is not
        // published twice.
        assert_eq!(
            told.kinds(),
            vec!["not applied", "applied", "not applied", "applied", "off"]
        );
        assert_eq!(told.connects.len(), 4);
        assert_eq!(told.waited_after(0), Duration::from_secs(1));
        assert_eq!(told.waited_after(1), Duration::from_secs(2));
        assert_eq!(
            told.waited_after(2),
            Duration::from_secs(1),
            "the third connection applied the light, which forgot both losses"
        );
        for output in [1, 2, 3] {
            assert_eq!(told.fake(2).ramps[&output][SWEEP - 1], ramp(warm()));
            assert_eq!(told.fake(3).ramps[&output][SWEEP - 1], ramp(warm()));
        }
    }

    #[test]
    fn off_during_the_wait_is_off_and_nothing_is_connected() {
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |event, stage| {
            if matches!(event, NightLightEvent::NotApplied(_)) {
                stage.set(None);
            }
        });

        assert_eq!(told.kinds(), vec!["not applied", "off"]);
        assert_eq!(
            told.said[1].1,
            told.said[0].1 + POLL,
            "answered on the next poll"
        );
        assert_eq!(told.connects.len(), 1);
        assert!(
            told.ran_to_the_end(),
            "it idled past every backoff until the shutdown"
        );
    }

    #[test]
    fn a_new_warmth_during_the_wait_is_what_the_next_connection_sweeps_to() {
        let milder = Whitepoint::for_temperature(4000);
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |event, stage| {
            if matches!(event, NightLightEvent::NotApplied(_)) {
                stage.set(Some(4000));
            }
        });

        assert_eq!(told.kinds(), vec!["not applied", "applied", "off"]);
        assert_eq!(told.connects.len(), 2);
        assert_eq!(
            told.waited_after(0),
            Duration::from_secs(1),
            "a command does not shorten the wait"
        );
        let back = told.fake(1);
        for output in [1, 2, 3] {
            assert_eq!(back.ramps[&output][SWEEP - 1], ramp(milder));
            assert!(!back.ramps[&output].contains(&ramp(warm())));
        }
    }

    #[test]
    fn on_again_after_off_connects_at_once_when_the_backoff_is_long_over() {
        let mut step = 0;
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |event, stage| {
            step += 1;
            match (step, event) {
                (1, NightLightEvent::NotApplied(_)) => stage.set(None),
                (2, NightLightEvent::Off) => {
                    // Ten seconds later the light is asked for again.
                    stage.ticker.sleep(Duration::from_secs(10));
                    stage.set(Some(2700));
                }
                _ => {}
            }
        });

        assert_eq!(
            told.kinds(),
            vec!["not applied", "off", "not applied", "applied", "off"]
        );
        assert_eq!(told.said[2].0, timed_out(), "still for the same reason");
        assert_eq!(told.connects.len(), 2);
        assert_eq!(
            told.connects[1],
            told.said[1].1 + Duration::from_secs(10) + POLL,
            "on the poll that takes the command"
        );
    }

    #[test]
    fn off_asked_during_a_sweep_that_loses_the_connection_is_answered_and_nothing_connects() {
        // The sweep starts one poll in; 70 ms is between its second and its
        // third frame, and the fifth is the one never confirmed.
        let told = supervised(
            vec![Attempt::Chokes(MID_SWEEP)],
            &[(Duration::from_millis(70), None)],
            |_, _| {},
        );

        assert_eq!(told.fake(0).count(1), 5, "the command arrived mid-sweep");
        // Off was the last word before the loss, so the loss reports off.
        assert_eq!(told.kinds(), vec!["off"]);
        assert_eq!(told.connects.len(), 1);
        assert!(told.ran_to_the_end());
    }

    #[test]
    fn shutdown_during_the_wait_ends_it_within_one_poll() {
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |event, stage| {
            if matches!(event, NightLightEvent::NotApplied(_)) {
                stage
                    .ticker
                    .stop_at(stage.ticker.now() + Duration::from_millis(310));
            }
        });

        assert_eq!(told.kinds(), vec!["not applied"]);
        assert_eq!(told.connects.len(), 1);
        let waited = told.ended - told.said[0].1;
        assert!(
            waited >= Duration::from_millis(310) && waited <= Duration::from_millis(310) + POLL,
            "{waited:?}"
        );
    }

    #[test]
    fn a_dropped_handle_during_the_wait_ends_it_within_one_poll() {
        let told = supervised(vec![Attempt::Chokes(MID_SWEEP)], &[], |event, stage| {
            if matches!(event, NightLightEvent::NotApplied(_)) {
                stage.handle = None;
            }
        });

        assert_eq!(told.kinds(), vec!["not applied"]);
        assert_eq!(told.connects.len(), 1);
        assert_eq!(told.ended, told.said[0].1 + POLL);
    }

    #[test]
    fn a_first_sync_not_answered_in_time_is_tried_again() {
        let told = supervised(
            vec![Attempt::Fails(Lost::Wire(wire::Lost::Timeout))],
            &[],
            |_, _| {},
        );

        assert_eq!(told.kinds(), vec!["not applied", "applied", "off"]);
        assert_eq!(told.said[0].0, timed_out());
        assert_eq!(told.connects.len(), 2);
        assert_eq!(told.connects[1] - told.connects[0], Duration::from_secs(1));
    }

    #[test]
    fn a_withdrawn_manager_is_settled_by_the_next_connection() {
        let told = supervised(
            vec![Attempt::Withdraws, Attempt::Fails(Lost::NoManager)],
            &[],
            |_, _| {},
        );

        assert_eq!(told.kinds(), vec!["not applied", "unavailable"]);
        assert_eq!(
            told.said[0].0,
            NightLightEvent::NotApplied("the compositor withdrew wlr gamma control".to_owned())
        );
        assert_eq!(told.connects.len(), 2);
        assert_eq!(told.waited_after(0), Duration::from_secs(1));
    }

    #[test]
    fn an_output_that_keeps_refusing_does_not_make_later_losses_wait_longer() {
        let told = supervised(
            vec![
                Attempt::Chokes(MID_SWEEP),
                Attempt::Chokes(MID_SWEEP),
                Attempt::RefusesThenDies {
                    output: 2,
                    polls: 8,
                },
            ],
            &[],
            |_, _| {},
        );

        // The third connection never reports `Applied`: output 2 refuses.
        // It settles all the same, and that forgets the two losses.
        assert_eq!(
            told.kinds(),
            vec![
                "not applied",
                "not applied",
                "not applied",
                "applied",
                "off"
            ]
        );
        assert_eq!(
            told.said[1].0,
            NightLightEvent::NotApplied("output 2 refused gamma control".to_owned())
        );
        assert_eq!(told.connects.len(), 4);
        assert_eq!(told.waited_after(0), Duration::from_secs(1));
        assert_eq!(told.waited_after(1), Duration::from_secs(2));
        assert_eq!(told.waited_after(2), Duration::from_secs(1));
        for output in [1, 3] {
            assert_eq!(told.fake(2).ramps[&output][SWEEP - 1], ramp(warm()));
        }
    }

    #[test]
    fn switching_off_forgets_the_lost_connections() {
        let mut step = 0;
        let told = supervised(
            vec![
                Attempt::Chokes(MID_SWEEP),
                Attempt::Dies(0),
                Attempt::Chokes(MID_SWEEP),
            ],
            &[],
            |event, stage| {
                step += 1;
                match (step, event) {
                    // The second loss: off, then on again, within its wait.
                    (2, NightLightEvent::NotApplied(_)) => stage.set(None),
                    (3, NightLightEvent::Off) => stage.set(Some(2700)),
                    _ => {}
                }
            },
        );

        assert_eq!(
            told.kinds(),
            vec![
                "not applied",
                "not applied",
                "off",
                "not applied",
                "not applied",
                "applied",
                "off"
            ]
        );
        assert_eq!(told.connects.len(), 4);
        assert_eq!(told.waited_after(0), Duration::from_secs(1));
        assert_eq!(
            told.waited_after(1),
            Duration::from_secs(2),
            "the wait that was running holds"
        );
        assert_eq!(
            told.waited_after(2),
            Duration::from_secs(1),
            "the third loss in a row counts as the first"
        );
    }

    #[test]
    fn a_connection_lost_while_the_light_is_off_reports_off_and_connects_when_asked() {
        let mut step = 0;
        let told = supervised(
            vec![Attempt::Dies(40)],
            // Long after the loss and its backoff.
            &[(Duration::from_secs(10), Some(2700))],
            |event, stage| {
                step += 1;
                if (step, event) == (1, &NightLightEvent::Applied) {
                    stage.set(None);
                }
            },
        );

        // The socket is reset with the light off: the worker's own warning
        // is all the journal gets, and the light stays reported off.
        assert_eq!(
            told.kinds(),
            vec!["applied", "off", "not applied", "applied", "off"]
        );
        let lost = told.fake(0).closed.expect("the first connection was lost");
        assert!(lost > told.said[1].1 && lost < told.started + Duration::from_secs(5));
        assert_eq!(told.connects.len(), 2);
        let asked = told.started + Duration::from_secs(10);
        assert!(told.said[2].1 >= asked && told.said[2].1 < asked + POLL);
        assert_eq!(
            told.connects[1], told.said[2].1,
            "at once: the wait is over"
        );
    }

    #[test]
    fn only_no_compositor_and_no_gamma_manager_are_final() {
        assert!(!Lost::NoManager.transient());
        assert!(!Lost::Wire(wire::Lost::Connect(ConnectError::NoCompositor)).transient());

        let reset = || WaylandError::Io(io::Error::from(io::ErrorKind::ConnectionReset));
        for lost in [
            Lost::ManagerGone,
            Lost::Wire(wire::Lost::Timeout),
            Lost::Wire(wire::Lost::Flood(MAX_EVENTS)),
            Lost::Wire(wire::Lost::Flush(reset())),
            Lost::Wire(wire::Lost::Read(reset())),
            Lost::Wire(wire::Lost::Poll(Errno::IO)),
        ] {
            assert!(lost.transient(), "{lost}");
        }
    }

    #[test]
    fn no_compositor_or_no_gamma_manager_idles_as_unavailable_and_never_connects_again() {
        for lost in [
            Lost::NoManager,
            Lost::Wire(wire::Lost::Connect(ConnectError::NoCompositor)),
        ] {
            let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
            let handle = NightLightHandle { commands };
            handle.set(Some(2700)).expect("the queue has room");
            handle.set(None).expect("the queue has room");
            handle.set(Some(3000)).expect("the queue has room");
            drop(handle);
            let shutdown = AtomicBool::new(false);
            let mut said = Vec::new();
            let mut out = Publisher::new(|event| {
                said.push(event);
                true
            });
            let mut connects = 0;
            let mut first = Some(lost);
            let mut connect = || -> Result<(Shared, Vec<GammaEvent>), Lost> {
                connects += 1;
                Err(first.take().unwrap_or(Lost::NoManager))
            };

            supervise(
                &mut connect,
                &Ticker::default(),
                &queue,
                &mut out,
                &shutdown,
            );
            drop(out);

            assert_eq!(connects, 1);
            assert_eq!(
                said,
                vec![
                    NightLightEvent::NotApplied(UNAVAILABLE.to_owned()),
                    NightLightEvent::Off,
                    NightLightEvent::NotApplied(UNAVAILABLE.to_owned()),
                ]
            );
        }
    }
}
