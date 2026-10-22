// Written for CEL-2-C with `celestina-qt/src/provider_adapter/brightness.rs`
// at c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413 as reference (read, not
// copied): the `CELESTINA_DDC` switch and its reading, the session lease path
// and its patience, the 20 s operation bound, the newest-target-per-monitor
// shape and the one-flag redetection come from there; the provider envelope,
// the module singletons, the periodic refresh and the journal are dropped.

//! The brightness workers (CEL-2-C, plan decisions 5 and 6): every blocking
//! step of reading and setting a monitor's brightness, off the loop.
//!
//! Two sources, each on its own named thread behind a [`Worker`] guard:
//!
//! - **Backlight** (`/sys/class/backlight/*`): `actual_brightness` is
//!   `sysfs_notify`-ed by the kernel, so the thread waits on it with
//!   `poll(POLLPRI | POLLERR)` and re-reads after each event. There is no
//!   timer that reads anything: the poll wakes every [`BACKLIGHT_WAKE`] only
//!   to look at the shutdown flag and the command queue.
//! - **DDC/CI** (`ddcutil`): one thread owns every `ddcutil` child. It is
//!   lazy: nothing runs until a consumer asks ([`BrightnessCommand::Detect`],
//!   [`BrightnessCommand::Read`] or a DDC [`BrightnessCommand::Change`]); the
//!   worker existing at all is the duty being on. Detection is then cached;
//!   [`BrightnessCommand::OutputsChanged`] sets one flag, and only once a
//!   detection happened, so a burst of outputs is one re-detection, and it
//!   waits [`OUTPUTS_REDETECT_FLOOR`] after the last detection. A consumer
//!   asking again after a detection that found nothing waits `REDETECT`
//!   (30 s), so key presses never become a `detect` each. There is no
//!   periodic read: the monitor's own buttons are not observed. A read
//!   follows each of our writes, one precedes the first write to a monitor
//!   whose level is unknown (published, so the display has a baseline), and
//!   one read per monitor is asked for when the slider popup opens.
//!
//! Two GPU losses were recorded with concurrent `ddcutil` children on one
//! bus, so every DDC operation:
//!
//! - holds the session lease `$XDG_RUNTIME_DIR/celestina-ddc.lock` (`flock`),
//!   waiting at most [`LEASE_PATIENCE`] and then refusing;
//! - runs at most one child, bounded by [`DDC_TIMEOUT`]; a child past it, or
//!   one running at shutdown, is killed and reaped. A child that will not be
//!   reaped keeps the lease and blocks every later operation until it is;
//! - is measured: an overlap inside this process is logged, not assumed away.
//!
//! Commands cross a [`COMMAND_QUEUE`]-deep queue that refuses (logged) beyond
//! it and never blocks the loop. While an operation runs the worker keeps
//! folding the queue into its plan, so a burst of steps for one monitor is
//! one write of the newest target. `CELESTINA_DDC=0|false`, read once,
//! forbids every `ddcutil` call.
//!
//! Every effect (children, sysfs, clock, lease) sits behind
//! [`DdcEnvironment`] and [`BacklightEnvironment`] so tests use fakes and no
//! test ever reaches a bus.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::audio::{stepped, Direction, CEILING_PERCENT, STEP_PERCENT};
use celestina_shell::brightness::{detection_is_due, parse_brightness, parse_detect, DdcDisplay};
use celestina_wayland::Sender;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::fs::{fcntl_setfl, flock, FlockOperation, OFlags};
use rustix::io::Errno;

use super::Worker;
use crate::message::Message;

/// Commands that may wait for a worker; beyond it the handle refuses.
pub const COMMAND_QUEUE: usize = 8;
/// The bound of one `ddcutil` operation: a cold first read takes about ten
/// seconds on the author's hardware, a warm one about one.
pub const DDC_TIMEOUT: Duration = Duration::from_secs(20);
/// How long an operation waits for the session lease before refusing; longer
/// than [`DDC_TIMEOUT`] because the likeliest holder is another process's own
/// bounded operation.
pub const LEASE_PATIENCE: Duration = Duration::from_secs(25);
/// The bound of one `brightnessctl` write of a backlight.
pub const BACKLIGHT_TIMEOUT: Duration = Duration::from_secs(5);
/// The variable that switches DDC off for this process (`0` or `false`).
pub const DDC_ENABLED_VAR: &str = "CELESTINA_DDC";
/// The most backlight devices observed.
pub const MAX_BACKLIGHTS: usize = 8;
/// How often the backlight thread wakes, reading nothing, to look at the
/// shutdown flag and its commands.
pub const BACKLIGHT_WAKE: Duration = Duration::from_millis(100);
/// The most bytes of a child's output kept; the rest is read and dropped.
pub const MAX_CHILD_OUTPUT: usize = 64 * 1024;
/// The least time between the end of a detection and a re-detection asked
/// for by outputs appearing or going: a burst of outputs (the shell starting
/// on three monitors, a dock) is one follow-up, never back-to-back children
/// on the bus. A consumer asking again after a detection that found nothing
/// waits the longer [`REDETECT`](celestina_shell::brightness::REDETECT).
pub const OUTPUTS_REDETECT_FLOOR: Duration = Duration::from_secs(5);

/// How often an idle DDC worker looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(100);
/// How often a waiting operation (lease or child) looks at its queue, the
/// shutdown flag and the child.
const TICK: Duration = Duration::from_millis(20);
/// How often a busy lease is tried again.
const LEASE_RETRY: Duration = Duration::from_millis(100);
/// How long a killed child may take to be reaped before it is abandoned.
const REAP_GRACE: Duration = Duration::from_secs(2);
/// The quiet gap after a backlight notification: a kernel fade is a burst of
/// notifications, read as one, and a device that keeps signalling cannot
/// spin the thread.
const BACKLIGHT_SETTLE: Duration = Duration::from_millis(16);
/// The most raw entries of `/sys/class/backlight` looked at.
const MAX_BACKLIGHT_ENTRIES: usize = 64;
/// The longest sysfs number read.
const MAX_SYSFS_BYTES: usize = 32;
/// The longest backlight device name accepted.
const MAX_DEVICE_NAME: usize = 64;
/// The most steps one pending relative target accumulates; twenty steps of
/// [`STEP_PERCENT`] cross the whole range.
const MAX_PENDING_STEPS: i8 = 20;

/// Where a brightness comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrightnessSource {
    /// A kernel backlight; `monitor` is its sysfs device name.
    Backlight,
    /// A monitor over DDC/CI; `monitor` is the connector niri names its
    /// output by.
    Ddc,
}

/// One monitor's brightness as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrightnessEvent {
    /// The connector (DDC) or the device name (backlight).
    pub monitor: String,
    /// Whole percent of the monitor's own maximum; `None` is *unknown*
    /// (detected and not read, or the read failed), never zero.
    pub percent: Option<u8>,
    pub source: BrightnessSource,
}

/// A change a consumer asks of one monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// To this percent (clamped to [`CEILING_PERCENT`]).
    Set(u8),
    /// One [`STEP_PERCENT`] step from the current reading.
    Step(Direction),
}

/// What the loop asks of the brightness workers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrightnessCommand {
    /// A DDC consumer exists (the piece is placed): detect once, then cache.
    /// After a detection that found nothing, it asks again, no sooner than
    /// `REDETECT` after it.
    Detect,
    /// The slider popup opened: read every detected DDC monitor once.
    Read,
    /// Change one monitor. A DDC change is also a consumer.
    Change {
        source: BrightnessSource,
        monitor: String,
        change: Change,
    },
    /// An output appeared or went: re-detect, once, if a detection already
    /// happened, no sooner than [`OUTPUTS_REDETECT_FLOOR`] after it;
    /// otherwise nothing.
    OutputsChanged,
}

/// Why a child operation did not produce an answer.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("cannot start the child")]
    Spawn(#[source] io::Error),
    #[error("cannot wait for the child")]
    Wait(#[source] io::Error),
    #[error("the child exited with {0}")]
    Failed(ExitStatus),
    #[error("the child did not finish in time and was killed")]
    Timeout,
    #[error("the operation was cancelled and the child killed")]
    Cancelled,
    /// The child was killed and has not been reaped: it may still be alive.
    #[error("the child was killed and could not be reaped")]
    Abandoned,
}

/// Why a DDC operation did not run or did not answer.
#[derive(Debug, thiserror::Error)]
enum OpError {
    #[error("the session's DDC lease cannot be opened")]
    Lease(#[source] io::Error),
    #[error("another process kept the DDC lease past the patience")]
    LeaseRefused,
    #[error("shutdown was asked for")]
    Cancelled,
    #[error("an abandoned ddcutil child may still be alive")]
    ChildAlive,
    #[error("ddcutil failed")]
    Run(#[source] RunError),
    #[error("ddcutil answered something that is not a reading")]
    Unreadable,
}

/// The DDC worker's effects.
pub trait DdcEnvironment: Send + 'static {
    /// Held while an operation runs; dropping it releases the lease.
    type Lease: Send + 'static;

    /// Runs `ddcutil args`, bounded by `timeout`. `tick` is called every
    /// [`TICK`] while it runs and returning `false` cancels it; either way a
    /// stopped child is killed and reaped, or kept as abandoned.
    ///
    /// # Errors
    ///
    /// The child could not run, failed, timed out, was cancelled or abandoned.
    fn ddcutil(
        &mut self,
        args: &[String],
        timeout: Duration,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<String, RunError>;

    /// Whether no child of an earlier operation is left alive (it reaps one
    /// that has since exited).
    fn reap_abandoned(&mut self) -> bool;

    /// Takes the session lease without waiting: `None` when another holder
    /// has it.
    ///
    /// # Errors
    ///
    /// The lease file cannot be opened or locked.
    fn try_lease(&mut self) -> io::Result<Option<Self::Lease>>;

    /// The monotonic clock.
    fn now(&mut self) -> Instant;

    /// Waits `duration`.
    fn sleep(&mut self, duration: Duration);
}

/// The backlight worker's effects.
pub trait BacklightEnvironment: Send + 'static {
    /// The devices to observe (at most [`MAX_BACKLIGHTS`]), opened for
    /// [`BacklightEnvironment::wait`].
    fn devices(&mut self) -> Vec<String>;

    /// The device's brightness in whole percent; `None` when its maximum is
    /// zero or its text is not a number.
    ///
    /// # Errors
    ///
    /// The device cannot be read.
    fn read(&mut self, device: &str) -> io::Result<Option<u8>>;

    /// Waits at most `timeout` for a notification and names the devices
    /// that signalled.
    ///
    /// # Errors
    ///
    /// The wait itself failed.
    fn wait(&mut self, timeout: Duration) -> io::Result<Vec<String>>;

    /// Stops observing `device` (it can no longer be read).
    fn forget(&mut self, device: &str);

    /// Sets `device` to `percent`.
    ///
    /// # Errors
    ///
    /// The setter could not run or failed.
    fn set(
        &mut self,
        device: &str,
        percent: u8,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<(), RunError>;

    /// Waits `duration`.
    fn sleep(&mut self, duration: Duration);
}

/// Whether a value of [`DDC_ENABLED_VAR`] allows DDC: absent, empty or
/// unreadable allows it (a typo must not remove a working control); only `0`
/// or `false`, in any case and with surrounding blanks, forbids it.
fn ddc_requested(value: Option<&str>) -> bool {
    match value.map(|value| value.trim().to_ascii_lowercase()) {
        None => true,
        Some(value) => !matches!(value.as_str(), "0" | "false"),
    }
}

/// [`ddc_requested`] of this process's environment, read once.
#[must_use]
pub fn ddc_allowed() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    *ALLOWED.get_or_init(|| {
        let allowed = ddc_requested(std::env::var(DDC_ENABLED_VAR).ok().as_deref());
        if !allowed {
            tracing::warn!("brightness: {DDC_ENABLED_VAR} forbids every ddcutil call");
        }
        allowed
    })
}

/// The in-process overlap instrument: the lease already serializes
/// operations, so an overlap here would mean the lease failed; it is logged,
/// never asserted, because crashing would lose the record.
static IN_FLIGHT: InFlight = InFlight::new();

struct InFlight(AtomicBool);

impl InFlight {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// Marks an operation running; the flag reports whether another one
    /// already was.
    fn enter(&self) -> (InFlightGuard<'_>, bool) {
        let overlapped = self.0.swap(true, Ordering::AcqRel);
        (InFlightGuard(&self.0), overlapped)
    }
}

struct InFlightGuard<'a>(&'a AtomicBool);

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The loop's handle on both workers.
#[derive(Clone, Debug)]
pub struct BrightnessHandle {
    ddc: SyncSender<BrightnessCommand>,
    backlight: SyncSender<(String, Change)>,
}

impl BrightnessHandle {
    /// Queues a command for the worker that owns it; never blocks.
    ///
    /// # Errors
    ///
    /// The command back, logged, when [`COMMAND_QUEUE`] commands already
    /// wait or the worker has stopped.
    pub fn request(&self, command: BrightnessCommand) -> Result<(), BrightnessCommand> {
        let refused = match command {
            BrightnessCommand::Change {
                source: BrightnessSource::Backlight,
                monitor,
                change,
            } => match self.backlight.try_send((monitor, change)) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full((monitor, change))) => (true, monitor, change),
                Err(TrySendError::Disconnected((monitor, change))) => (false, monitor, change),
            },
            other => {
                return match self.ddc.try_send(other) {
                    Ok(()) => Ok(()),
                    Err(TrySendError::Full(refused)) => Err(refuse(refused, true)),
                    Err(TrySendError::Disconnected(refused)) => Err(refuse(refused, false)),
                };
            }
        };
        let (full, monitor, change) = refused;
        Err(refuse(
            BrightnessCommand::Change {
                source: BrightnessSource::Backlight,
                monitor,
                change,
            },
            full,
        ))
    }
}

/// A handle whose DDC commands land in the returned queue (and whose
/// backlight commands are refused), for the tests of the handle's users.
#[cfg(test)]
pub(crate) fn test_handle() -> (BrightnessHandle, Receiver<BrightnessCommand>) {
    let (ddc, commands) = mpsc::sync_channel(COMMAND_QUEUE);
    let (backlight, _) = mpsc::sync_channel(COMMAND_QUEUE);
    (BrightnessHandle { ddc, backlight }, commands)
}

fn refuse(command: BrightnessCommand, full: bool) -> BrightnessCommand {
    if full {
        tracing::warn!(?command, "brightness: the command queue is full; refused");
    } else {
        tracing::warn!(?command, "brightness: the worker has stopped; refused");
    }
    command
}

/// Both worker threads; joining or dropping this stops and joins them.
#[derive(Debug)]
pub struct BrightnessWorkers {
    ddc: Worker,
    backlight: Worker,
}

impl BrightnessWorkers {
    /// Stops both workers and waits for them.
    pub fn join(self) {
        self.ddc.join();
        self.backlight.join();
    }

    /// [`BrightnessWorkers::join`], waiting at most `wait` in all; returns
    /// whether both stopped in time.
    pub fn join_within(self, wait: Duration) -> bool {
        let started = Instant::now();
        let ddc = self.ddc.join_within(wait);
        let backlight = self
            .backlight
            .join_within(wait.saturating_sub(started.elapsed()));
        ddc && backlight
    }
}

/// Starts both workers on the real system, publishing into `sender`.
///
/// # Errors
///
/// The failure to start a thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(BrightnessHandle, BrightnessWorkers)> {
    spawn_with(
        SystemDdc::default(),
        SysfsBacklight::new(PathBuf::from("/sys/class/backlight")),
        ddc_allowed(),
        shutdown,
        move |event| sender.send(Message::Brightness(event)).is_ok(),
    )
}

/// [`spawn`] with its effects, the DDC switch and its sink given; `publish`
/// returns whether the event was accepted (false ends that worker).
///
/// # Errors
///
/// The failure to start a thread.
pub fn spawn_with<P>(
    ddc: impl DdcEnvironment,
    backlight: impl BacklightEnvironment,
    ddc_allowed: bool,
    shutdown: &Arc<AtomicBool>,
    publish: P,
) -> io::Result<(BrightnessHandle, BrightnessWorkers)>
where
    P: FnMut(BrightnessEvent) -> bool + Clone + Send + 'static,
{
    let (ddc_commands, ddc_queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let (backlight_commands, backlight_queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let ddc_publish = publish.clone();
    let ddc_worker = Worker::spawn("brightness-ddc", shutdown, move || {
        run_ddc(ddc, ddc_allowed, &ddc_queue, &flag, ddc_publish);
    })?;
    let flag = Arc::clone(shutdown);
    let backlight_worker = Worker::spawn("brightness-backlight", shutdown, move || {
        run_backlight(backlight, &backlight_queue, &flag, publish);
    })?;
    Ok((
        BrightnessHandle {
            ddc: ddc_commands,
            backlight: backlight_commands,
        },
        BrightnessWorkers {
            ddc: ddc_worker,
            backlight: backlight_worker,
        },
    ))
}

// ---------------------------------------------------------------------------
// Targets

/// What one monitor is owed: the newest absolute target, or the sum of the
/// steps asked since the last write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Absolute(u8),
    Relative(i8),
}

impl Target {
    fn from_change(change: Change) -> Self {
        match change {
            Change::Set(percent) => Self::Absolute(percent.min(CEILING_PERCENT)),
            Change::Step(Direction::Up) => Self::Relative(1),
            Change::Step(Direction::Down) => Self::Relative(-1),
        }
    }

    /// This target, then `change`: an absolute target replaces whatever was
    /// owed; a step moves an absolute target, or adds to the steps owed.
    fn followed_by(self, change: Change) -> Self {
        match (self, Self::from_change(change)) {
            (_, absolute @ Self::Absolute(_)) => absolute,
            (Self::Absolute(base), Self::Relative(step)) => Self::Absolute(apply_steps(base, step)),
            (Self::Relative(owed), Self::Relative(step)) => Self::Relative(
                owed.saturating_add(step)
                    .clamp(-MAX_PENDING_STEPS, MAX_PENDING_STEPS),
            ),
        }
    }

    /// Whether this target needs the current reading to resolve.
    fn needs_reading(self) -> bool {
        matches!(self, Self::Relative(_))
    }

    /// The percent to write; a relative target needs `current`.
    fn resolve(self, current: Option<u8>) -> Option<u8> {
        match self {
            Self::Absolute(percent) => Some(percent),
            Self::Relative(steps) => current.map(|current| apply_steps(current, steps)),
        }
    }
}

/// `steps` steps of [`STEP_PERCENT`] from `current`, through the shared step
/// arithmetic (saturating, ceiling [`CEILING_PERCENT`]).
fn apply_steps(current: u8, steps: i8) -> u8 {
    let direction = if steps < 0 {
        Direction::Down
    } else {
        Direction::Up
    };
    (0..steps.unsigned_abs()).fold(current.min(CEILING_PERCENT), |level, _| {
        stepped(level, direction, STEP_PERCENT)
    })
}

fn sanitize(text: &str) -> String {
    celestina_shell::bounded::bounded(text, celestina_shell::bounded::MAX_TEXT_UNITS)
}

// ---------------------------------------------------------------------------
// DDC worker

/// What consumers have asked of the DDC worker and it still owes.
#[derive(Debug, Default)]
struct Plan {
    /// A consumer exists; nothing runs before.
    consumer: bool,
    /// A detection has happened (successful or not).
    detected: bool,
    /// The last detection found nothing.
    found_none: bool,
    /// A consumer asked again after a detection that found nothing: one
    /// re-detection is owed once
    /// [`REDETECT`](celestina_shell::brightness::REDETECT) has passed since
    /// it.
    retry: bool,
    /// Outputs appeared or went after a detection: one re-detection is owed
    /// once [`OUTPUTS_REDETECT_FLOOR`] has passed since it.
    outputs_changed: bool,
    /// The popup asked for one read of every monitor.
    read_all: bool,
    /// The newest target per connector.
    pending: BTreeMap<String, Target>,
    /// Commands seen while DDC is forbidden (logged once).
    forbidden_seen: bool,
}

impl Plan {
    fn absorb(&mut self, command: BrightnessCommand) {
        match command {
            BrightnessCommand::Detect => {
                self.consumer = true;
                self.retry_if_found_none();
            }
            BrightnessCommand::Read => {
                self.consumer = true;
                self.read_all = true;
            }
            BrightnessCommand::Change {
                source: BrightnessSource::Ddc,
                monitor,
                change,
            } => {
                self.consumer = true;
                // A change for a monitor nobody found is a reason to look
                // again, on the same floor as a consumer asking again.
                self.retry_if_found_none();
                let target = self.pending.get(&monitor).map_or_else(
                    || Target::from_change(change),
                    |owed| owed.followed_by(change),
                );
                self.pending.insert(monitor, target);
            }
            // Routed to the backlight worker by the handle; never here.
            BrightnessCommand::Change {
                source: BrightnessSource::Backlight,
                ..
            } => {}
            // One flag, and only once something was detected: before that the
            // first detection will see every output anyway.
            BrightnessCommand::OutputsChanged => {
                if self.detected {
                    self.outputs_changed = true;
                }
            }
        }
    }

    fn retry_if_found_none(&mut self) {
        if self.detected && self.found_none {
            self.retry = true;
        }
    }

    fn clear(&mut self) {
        let forbidden_seen = self.forbidden_seen;
        *self = Self {
            forbidden_seen,
            ..Self::default()
        };
    }

    fn wants_ddc(&self) -> bool {
        self.consumer
    }
}

/// The DDC worker's state between operations.
struct Ddc<E: DdcEnvironment> {
    env: E,
    plan: Plan,
    displays: Vec<DdcDisplay>,
    levels: BTreeMap<String, Option<u8>>,
    /// Monitors the popup's read still owes.
    to_read: BTreeSet<String>,
    /// When the last detection ended, on the environment's clock.
    last_detect: Option<Instant>,
    /// The lease kept while an abandoned child may be alive.
    held: Option<E::Lease>,
}

/// Folds every waiting command into `plan`; returns whether the queue is
/// still connected.
fn drain(queue: &Receiver<BrightnessCommand>, plan: &mut Plan) -> bool {
    for _ in 0..COMMAND_QUEUE * 2 {
        match queue.try_recv() {
            Ok(command) => plan.absorb(command),
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
    true
}

fn run_ddc<E: DdcEnvironment>(
    env: E,
    allowed: bool,
    queue: &Receiver<BrightnessCommand>,
    shutdown: &AtomicBool,
    mut publish: impl FnMut(BrightnessEvent) -> bool,
) {
    let mut ddc = Ddc {
        env,
        plan: Plan::default(),
        displays: Vec::new(),
        levels: BTreeMap::new(),
        to_read: BTreeSet::new(),
        last_detect: None,
        held: None,
    };
    let mut busy = false;
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        if ddc.held.is_some() && ddc.env.reap_abandoned() {
            tracing::warn!("brightness: the abandoned ddcutil child is reaped; DDC resumes");
            ddc.held = None;
        }
        if !busy {
            match queue.recv_timeout(POLL) {
                Ok(command) => ddc.plan.absorb(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        if !drain(queue, &mut ddc.plan) {
            break;
        }
        if !allowed {
            if ddc.plan.wants_ddc() && !ddc.plan.forbidden_seen {
                tracing::warn!(
                    "brightness: DDC is switched off by {DDC_ENABLED_VAR}; nothing runs"
                );
                ddc.plan.forbidden_seen = true;
            }
            ddc.plan.clear();
            busy = false;
            continue;
        }
        match ddc.step(queue, shutdown) {
            Step::Idle => busy = false,
            Step::Worked(events) => {
                busy = true;
                for event in events {
                    if !publish(event) {
                        tracing::debug!("brightness: the loop is gone; the DDC worker stops");
                        return;
                    }
                }
            }
            Step::Stop => break,
        }
    }
    if ddc.held.is_some() && !ddc.env.reap_abandoned() {
        tracing::error!("brightness: a killed ddcutil child is still not reaped at shutdown");
    }
}

/// The outcome of one unit of DDC work.
enum Step {
    Idle,
    Worked(Vec<BrightnessEvent>),
    Stop,
}

impl<E: DdcEnvironment> Ddc<E> {
    /// Runs at most one unit of work: a detection, one monitor's write and
    /// read-back, or one popup read.
    fn step(&mut self, queue: &Receiver<BrightnessCommand>, shutdown: &AtomicBool) -> Step {
        if !self.plan.wants_ddc() {
            return Step::Idle;
        }
        if !self.plan.detected || self.redetect_due() {
            return self.detect(queue, shutdown);
        }
        if let Some((monitor, target)) = self.plan.pending.pop_first() {
            return self.apply(&monitor, target, queue, shutdown);
        }
        if self.plan.read_all {
            self.plan.read_all = false;
            self.to_read = self
                .displays
                .iter()
                .map(|display| display.connector.clone())
                .collect();
        }
        if let Some(monitor) = self.to_read.pop_first() {
            return match self.read(&monitor, queue, shutdown) {
                Err(OpError::Cancelled) => Step::Stop,
                Ok(_) | Err(_) => Step::Worked(vec![self.event(&monitor)]),
            };
        }
        Step::Idle
    }

    /// Whether a re-detection owed is due: outputs changing wait
    /// [`OUTPUTS_REDETECT_FLOOR`] after the last detection, a consumer
    /// asking again after finding nothing waits [`REDETECT`](celestina_shell::brightness::REDETECT)
    /// (`detection_is_due`'s interval for a worker that knows no monitor). A
    /// request not yet due stays owed and the idle loop looks again.
    fn redetect_due(&mut self) -> bool {
        if !self.plan.retry && !self.plan.outputs_changed {
            return false;
        }
        let since = match self.last_detect {
            Some(at) => self.env.now().saturating_duration_since(at),
            None => Duration::MAX,
        };
        let outputs = self.plan.outputs_changed && since >= OUTPUTS_REDETECT_FLOOR;
        let retry = self.plan.retry && detection_is_due(!self.plan.found_none, false, since);
        outputs || retry
    }

    fn event(&self, monitor: &str) -> BrightnessEvent {
        BrightnessEvent {
            monitor: monitor.to_owned(),
            percent: self.levels.get(monitor).copied().flatten(),
            source: BrightnessSource::Ddc,
        }
    }

    fn detect(&mut self, queue: &Receiver<BrightnessCommand>, shutdown: &AtomicBool) -> Step {
        // Taken before the operation: a request that arrives during it stays
        // set for the next turn instead of being lost.
        self.plan.retry = false;
        self.plan.outputs_changed = false;
        self.plan.detected = true;
        let args = vec!["detect".to_owned(), "--brief".to_owned()];
        let displays = match self.operation("detect", &args, queue, shutdown) {
            Ok(listing) => parse_detect(&listing)
                .into_iter()
                .map(|display| DdcDisplay {
                    number: display.number,
                    connector: sanitize(&display.connector),
                })
                .collect::<Vec<_>>(),
            Err(OpError::Cancelled) => return Step::Stop,
            Err(error) => {
                tracing::warn!(
                    "brightness: DDC detection failed: {}",
                    crate::error_chain(&error)
                );
                Vec::new()
            }
        };
        tracing::info!(
            displays = displays.len(),
            "brightness: DDC detection found {} monitor(s)",
            displays.len()
        );
        self.last_detect = Some(self.env.now());
        self.plan.found_none = displays.is_empty();
        let mut levels = BTreeMap::new();
        for display in &displays {
            let known = self.levels.get(&display.connector).copied().flatten();
            levels.insert(display.connector.clone(), known);
        }
        self.levels = levels;
        self.displays = displays;
        self.to_read
            .retain(|monitor| self.levels.contains_key(monitor));
        Step::Worked(
            self.displays
                .iter()
                .map(|display| self.event(&display.connector))
                .collect(),
        )
    }

    fn apply(
        &mut self,
        monitor: &str,
        target: Target,
        queue: &Receiver<BrightnessCommand>,
        shutdown: &AtomicBool,
    ) -> Step {
        if !self.levels.contains_key(monitor) {
            tracing::warn!(
                monitor,
                "brightness: no DDC monitor on this output; the change is dropped"
            );
            return Step::Worked(Vec::new());
        }
        let mut current = self.levels.get(monitor).copied().flatten();
        let mut events = Vec::new();
        if current.is_none() {
            // A step from a value nobody has read would be a guess; and the
            // level before any write is published, or the display would take
            // the read-back as the monitor's baseline and never show the
            // first change after start.
            match self.read(monitor, queue, shutdown) {
                Ok(level) => {
                    current = Some(level);
                    events.push(self.event(monitor));
                }
                Err(OpError::Cancelled) => return Step::Stop,
                Err(error) => {
                    tracing::warn!(
                        monitor,
                        "brightness: cannot read before writing: {}",
                        crate::error_chain(&error)
                    );
                    if target.needs_reading() {
                        return Step::Worked(vec![self.event(monitor)]);
                    }
                }
            }
        }
        let Some(wanted) = target.resolve(current) else {
            return Step::Worked(vec![self.event(monitor)]);
        };
        if Some(wanted) == current {
            if events.is_empty() {
                events.push(self.event(monitor));
            }
            return Step::Worked(events);
        }
        let Some(number) = self.number(monitor) else {
            return Step::Worked(events);
        };
        let args = vec![
            "--display".to_owned(),
            number.to_string(),
            "setvcp".to_owned(),
            "10".to_owned(),
            wanted.to_string(),
        ];
        match self.operation("write", &args, queue, shutdown) {
            Ok(_) => {}
            Err(OpError::Cancelled) => return Step::Stop,
            Err(error) => {
                tracing::warn!(
                    monitor,
                    "brightness: DDC write failed: {}",
                    crate::error_chain(&error)
                );
                events.push(self.event(monitor));
                return Step::Worked(events);
            }
        }
        // What the monitor settled on, not what it was asked for.
        self.to_read.remove(monitor);
        match self.read(monitor, queue, shutdown) {
            Err(OpError::Cancelled) => Step::Stop,
            Ok(_) | Err(_) => {
                events.push(self.event(monitor));
                Step::Worked(events)
            }
        }
    }

    fn number(&self, monitor: &str) -> Option<u8> {
        self.displays
            .iter()
            .find(|display| display.connector == monitor)
            .map(|display| display.number)
    }

    /// Reads `monitor` and records the answer (unknown on failure).
    fn read(
        &mut self,
        monitor: &str,
        queue: &Receiver<BrightnessCommand>,
        shutdown: &AtomicBool,
    ) -> Result<u8, OpError> {
        let number = self.number(monitor).ok_or(OpError::Unreadable)?;
        let args = vec![
            "--display".to_owned(),
            number.to_string(),
            "getvcp".to_owned(),
            "10".to_owned(),
            "--brief".to_owned(),
        ];
        let result = self
            .operation("read", &args, queue, shutdown)
            .and_then(|reading| parse_brightness(&reading).ok_or(OpError::Unreadable));
        match &result {
            Ok(level) => {
                self.levels.insert(monitor.to_owned(), Some(*level));
            }
            Err(OpError::Cancelled) => {}
            Err(error) => {
                tracing::warn!(
                    monitor,
                    "brightness: DDC read failed: {}",
                    crate::error_chain(error)
                );
                self.levels.insert(monitor.to_owned(), None);
            }
        }
        result
    }

    /// One `ddcutil` operation under the lease, folding the queue into the
    /// plan while it waits.
    fn operation(
        &mut self,
        what: &'static str,
        args: &[String],
        queue: &Receiver<BrightnessCommand>,
        shutdown: &AtomicBool,
    ) -> Result<String, OpError> {
        if self.held.is_some() {
            if !self.env.reap_abandoned() {
                return Err(OpError::ChildAlive);
            }
            self.held = None;
        }
        let plan = &mut self.plan;
        let mut tick = || {
            drain(queue, plan);
            !shutdown.load(Ordering::Acquire)
        };
        let lease = acquire_lease(&mut self.env, &mut tick)?;
        let (_flight, overlapped) = IN_FLIGHT.enter();
        if overlapped {
            tracing::error!(
                operation = what,
                "brightness: two DDC operations overlap in this process"
            );
        }
        tracing::info!(operation = what, "brightness: ddcutil starts");
        let started = Instant::now();
        let result = self.env.ddcutil(args, DDC_TIMEOUT, &mut tick);
        tracing::info!(
            operation = what,
            elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ok = result.is_ok(),
            "brightness: ddcutil ends"
        );
        match result {
            Ok(output) => Ok(output),
            Err(RunError::Cancelled) => Err(OpError::Cancelled),
            Err(RunError::Abandoned) => {
                tracing::error!(
                    operation = what,
                    "brightness: a killed ddcutil child was not reaped; DDC holds its lease until it is"
                );
                self.held = Some(lease);
                if shutdown.load(Ordering::Acquire) {
                    Err(OpError::Cancelled)
                } else {
                    Err(OpError::Run(RunError::Abandoned))
                }
            }
            Err(error) => Err(OpError::Run(error)),
        }
    }
}

/// Takes the session lease, waiting at most [`LEASE_PATIENCE`]; `tick`
/// returning `false` (shutdown) ends the wait.
fn acquire_lease<E: DdcEnvironment>(
    env: &mut E,
    tick: &mut dyn FnMut() -> bool,
) -> Result<E::Lease, OpError> {
    let deadline = env.now() + LEASE_PATIENCE;
    let mut waited = false;
    loop {
        match env.try_lease() {
            Ok(Some(lease)) => {
                if waited {
                    tracing::warn!("brightness: the DDC lease is taken after waiting");
                }
                return Ok(lease);
            }
            Ok(None) => {}
            Err(error) => {
                // No lease means no serialization: refuse the operation.
                tracing::error!(%error, "brightness: the DDC lease is unavailable; refused");
                return Err(OpError::Lease(error));
            }
        }
        if !waited {
            waited = true;
            tracing::warn!("brightness: another process holds the DDC lease; waiting");
        }
        if !tick() {
            return Err(OpError::Cancelled);
        }
        if env.now() >= deadline {
            tracing::error!("brightness: the DDC lease stayed busy; refused");
            return Err(OpError::LeaseRefused);
        }
        env.sleep(LEASE_RETRY);
    }
}

// ---------------------------------------------------------------------------
// Backlight worker

fn run_backlight(
    mut env: impl BacklightEnvironment,
    queue: &Receiver<(String, Change)>,
    shutdown: &AtomicBool,
    mut publish: impl FnMut(BrightnessEvent) -> bool,
) {
    let mut levels: BTreeMap<String, Option<u8>> = BTreeMap::new();
    for device in env.devices() {
        let level = env.read(&device).unwrap_or_else(|error| {
            tracing::warn!(device, %error, "brightness: cannot read the backlight");
            None
        });
        levels.insert(device, level);
    }
    if levels.is_empty() {
        // Nothing to observe: the thread ends; a backlight change is then
        // refused by the queue as a stopped worker.
        return;
    }
    let event = |device: &str, percent: Option<u8>| BrightnessEvent {
        monitor: device.to_owned(),
        percent,
        source: BrightnessSource::Backlight,
    };
    for (device, level) in &levels {
        if !publish(event(device, *level)) {
            return;
        }
    }
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        // Commands first: a key press waits at most one wake.
        let mut owed: BTreeMap<String, Target> = BTreeMap::new();
        loop {
            match queue.try_recv() {
                Ok((device, change)) => {
                    let target = owed
                        .get(&device)
                        .map_or_else(|| Target::from_change(change), |t| t.followed_by(change));
                    owed.insert(device, target);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        let mut changed: BTreeSet<String> = BTreeSet::new();
        for (device, target) in owed {
            let Some(current) = levels.get(&device).copied() else {
                tracing::warn!(
                    device,
                    "brightness: no such backlight; the change is dropped"
                );
                continue;
            };
            let current = match current {
                Some(level) => Some(level),
                None => env.read(&device).ok().flatten(),
            };
            let Some(wanted) = target.resolve(current) else {
                continue;
            };
            if Some(wanted) == current {
                continue;
            }
            let mut tick = || !shutdown.load(Ordering::Acquire);
            match env.set(&device, wanted, &mut tick) {
                Ok(()) => {}
                Err(RunError::Cancelled) => return,
                Err(error) => {
                    tracing::warn!(
                        device,
                        "brightness: cannot set the backlight: {}",
                        crate::error_chain(&error)
                    );
                }
            }
            changed.insert(device);
        }
        match env.wait(BACKLIGHT_WAKE) {
            Ok(signalled) => {
                if !signalled.is_empty() {
                    env.sleep(BACKLIGHT_SETTLE);
                }
                changed.extend(signalled);
            }
            Err(error) => {
                tracing::warn!(%error, "brightness: cannot wait on the backlight");
                env.sleep(BACKLIGHT_WAKE);
            }
        }
        for device in changed {
            if !levels.contains_key(&device) {
                continue;
            }
            let level = match env.read(&device) {
                Ok(level) => level,
                Err(error) => {
                    tracing::warn!(device, %error, "brightness: the backlight went away");
                    env.forget(&device);
                    levels.remove(&device);
                    if !publish(event(&device, None)) {
                        return;
                    }
                    continue;
                }
            };
            if levels.insert(device.clone(), level) != Some(level)
                && !publish(event(&device, level))
            {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The real system

/// The path of the session's DDC lease: shared with every program of the
/// suite that speaks DDC, so the fallback without `XDG_RUNTIME_DIR` is the
/// same as theirs.
fn lease_path() -> PathBuf {
    let mut base =
        std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    base.push("celestina-ddc.lock");
    base
}

/// Real `ddcutil`, lease file and clock.
#[derive(Debug, Default)]
pub struct SystemDdc {
    /// A killed child that was not reaped in time.
    abandoned: Option<Child>,
}

impl DdcEnvironment for SystemDdc {
    type Lease = File;

    fn ddcutil(
        &mut self,
        args: &[String],
        timeout: Duration,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<String, RunError> {
        run_bounded("ddcutil", args, timeout, tick, &mut self.abandoned)
    }

    fn reap_abandoned(&mut self) -> bool {
        reap(&mut self.abandoned)
    }

    fn try_lease(&mut self) -> io::Result<Option<File>> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(lease_path())?;
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(file)),
            Err(Errno::WOULDBLOCK) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

/// Whether no abandoned child is left alive, reaping one that exited.
fn reap(abandoned: &mut Option<Child>) -> bool {
    let Some(child) = abandoned.as_mut() else {
        return true;
    };
    match child.try_wait() {
        Ok(Some(_)) | Err(_) => {
            *abandoned = None;
            true
        }
        Ok(None) => false,
    }
}

/// Runs `program args` with no stdin and stderr, keeping at most
/// [`MAX_CHILD_OUTPUT`] bytes of stdout, until it exits, `timeout` passes or
/// `tick` returns `false`. A stopped child is killed and reaped within
/// [`REAP_GRACE`], or kept in `abandoned`. Refuses to start while an earlier
/// child is still abandoned, so at most one child is ever alive.
fn run_bounded(
    program: &str,
    args: &[String],
    timeout: Duration,
    tick: &mut dyn FnMut() -> bool,
    abandoned: &mut Option<Child>,
) -> Result<String, RunError> {
    if !reap(abandoned) {
        return Err(RunError::Abandoned);
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("LC_ALL", "C")
        .spawn()
        .map_err(RunError::Spawn)?;
    let Some(mut stdout) = child.stdout.take() else {
        return Err(stop(
            child,
            RunError::Wait(io::Error::other("no stdout")),
            abandoned,
        ));
    };
    if let Err(error) = fcntl_setfl(&stdout, OFlags::NONBLOCK) {
        return Err(stop(child, RunError::Wait(error.into()), abandoned));
    }
    let started = Instant::now();
    let mut output = Vec::new();
    loop {
        drain_pipe(&mut stdout, &mut output);
        match child.try_wait() {
            Ok(Some(status)) => {
                drain_pipe(&mut stdout, &mut output);
                return if status.success() {
                    Ok(String::from_utf8_lossy(&output).into_owned())
                } else {
                    Err(RunError::Failed(status))
                };
            }
            Ok(None) => {}
            Err(error) => return Err(stop(child, RunError::Wait(error), abandoned)),
        }
        if started.elapsed() >= timeout {
            tracing::warn!(program, "brightness: the child timed out; killing it");
            return Err(stop(child, RunError::Timeout, abandoned));
        }
        if !tick() {
            return Err(stop(child, RunError::Cancelled, abandoned));
        }
        thread::sleep(TICK);
    }
}

/// Reads what the non-blocking pipe has, keeping at most
/// [`MAX_CHILD_OUTPUT`] bytes.
fn drain_pipe(stdout: &mut ChildStdout, output: &mut Vec<u8>) {
    let mut buffer = [0u8; 4096];
    // Bounded: a child cannot keep this loop alive faster than it writes, and
    // each turn of the caller's loop drains at most this much.
    for _ in 0..64 {
        match stdout.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => {
                let room = MAX_CHILD_OUTPUT.saturating_sub(output.len());
                output.extend_from_slice(&buffer[..read.min(room)]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

/// Kills `child` and reaps it within [`REAP_GRACE`]; returns `why`, or
/// [`RunError::Abandoned`] with the child kept when it would not be reaped.
fn stop(mut child: Child, why: RunError, abandoned: &mut Option<Child>) -> RunError {
    let _ = child.kill();
    let deadline = Instant::now() + REAP_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return why,
            Ok(None) if Instant::now() < deadline => thread::sleep(TICK),
            Ok(None) => {
                *abandoned = Some(child);
                return RunError::Abandoned;
            }
        }
    }
}

/// One observed backlight device.
#[derive(Debug)]
struct Device {
    name: String,
    actual: File,
    max: u64,
}

/// The real `/sys/class/backlight` and `brightnessctl`.
#[derive(Debug)]
pub struct SysfsBacklight {
    root: PathBuf,
    devices: Vec<Device>,
    abandoned: Option<Child>,
}

impl SysfsBacklight {
    /// Observes the devices under `root`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            devices: Vec::new(),
            abandoned: None,
        }
    }

    fn open(&self, name: &str) -> io::Result<Device> {
        let dir = self.root.join(name);
        let max = read_number(&File::open(dir.join("max_brightness"))?)?;
        let actual = File::open(dir.join("actual_brightness"))?;
        // `sysfs_notify` signals a reader whose last read predates it: read
        // once now so the first poll waits for a real change.
        read_number(&actual)?;
        Ok(Device {
            name: name.to_owned(),
            actual,
            max,
        })
    }
}

/// Whether `name` can be a backlight device passed to `brightnessctl`.
fn valid_device_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DEVICE_NAME
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

/// Reads one bounded decimal number from the start of a sysfs file.
fn read_number(file: &File) -> io::Result<u64> {
    let mut buffer = [0u8; MAX_SYSFS_BYTES];
    let read = file.read_at(&mut buffer, 0)?;
    parse_sysfs_number(&buffer[..read])
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not a sysfs number"))
}

fn parse_sysfs_number(bytes: &[u8]) -> Option<u64> {
    std::str::from_utf8(bytes).ok()?.trim().parse().ok()
}

/// Whole percent of `max`, rounded down; `None` for a zero maximum.
fn backlight_percent(actual: u64, max: u64) -> Option<u8> {
    if max == 0 {
        return None;
    }
    u8::try_from(actual.min(max).saturating_mul(100) / max).ok()
}

impl BacklightEnvironment for SysfsBacklight {
    fn devices(&mut self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .take(MAX_BACKLIGHT_ENTRIES)
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| valid_device_name(name))
            .collect();
        names.sort();
        self.devices = names
            .iter()
            .filter_map(|name| match self.open(name) {
                Ok(device) => Some(device),
                Err(error) => {
                    tracing::warn!(device = %name, %error, "brightness: cannot open the backlight");
                    None
                }
            })
            .take(MAX_BACKLIGHTS)
            .collect();
        self.devices
            .iter()
            .map(|device| device.name.clone())
            .collect()
    }

    fn read(&mut self, device: &str) -> io::Result<Option<u8>> {
        let device = self
            .devices
            .iter()
            .find(|candidate| candidate.name == device)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok(backlight_percent(read_number(&device.actual)?, device.max))
    }

    fn wait(&mut self, timeout: Duration) -> io::Result<Vec<String>> {
        if self.devices.is_empty() {
            thread::sleep(timeout);
            return Ok(Vec::new());
        }
        let mut fds: Vec<PollFd<'_>> = self
            .devices
            .iter()
            .map(|device| PollFd::new(&device.actual, PollFlags::PRI | PollFlags::ERR))
            .collect();
        let timeout = Timespec::try_from(timeout)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "timeout out of range"))?;
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) | Err(Errno::INTR) => return Ok(Vec::new()),
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        let signalled: Vec<bool> = fds.iter().map(|fd| !fd.revents().is_empty()).collect();
        drop(fds);
        Ok(self
            .devices
            .iter()
            .zip(signalled)
            .filter(|(_, signalled)| *signalled)
            .map(|(device, _)| device.name.clone())
            .collect())
    }

    fn forget(&mut self, device: &str) {
        self.devices.retain(|candidate| candidate.name != device);
    }

    fn set(
        &mut self,
        device: &str,
        percent: u8,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<(), RunError> {
        if !valid_device_name(device) {
            return Err(RunError::Spawn(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a backlight device name",
            )));
        }
        let args = [
            "--class=backlight".to_owned(),
            format!("--device={device}"),
            "set".to_owned(),
            format!("{}%", percent.min(CEILING_PERCENT)),
        ];
        run_bounded(
            "brightnessctl",
            &args,
            BACKLIGHT_TIMEOUT,
            tick,
            &mut self.abandoned,
        )
        .map(drop)
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::brightness::REDETECT;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Condvar, Mutex};

    const DETECT: &str = "Display 1\n   DRM connector:    card1-DP-1\n\n\
                          Display 2\n   DRM connector:    card1-DP-2\n";

    /// One fake bus shared by every fake environment of a test, so two
    /// workers contend exactly as two processes would.
    #[derive(Default)]
    struct Bus {
        calls: Mutex<Vec<Vec<String>>>,
        active: AtomicUsize,
        max_active: AtomicUsize,
        /// The session lease (another process holds it when `true`).
        leased: Mutex<bool>,
        lease_attempts: AtomicUsize,
        /// Brightness per display number.
        values: Mutex<BTreeMap<u8, u8>>,
        detect: Mutex<String>,
        /// While `true`, every operation blocks (ticking, if `tick_while_gated`).
        gate: Mutex<bool>,
        opened: Condvar,
        tick_while_gated: AtomicBool,
        /// Operations that hit the gate.
        gated: AtomicUsize,
        /// Answer every operation with a timeout.
        time_out: AtomicBool,
        /// Simulated time.
        clock: Mutex<Duration>,
    }

    impl Bus {
        fn new() -> Arc<Self> {
            let bus = Self::default();
            *bus.detect.lock().expect("lock") = DETECT.to_owned();
            bus.values.lock().expect("lock").extend([(1, 50), (2, 30)]);
            Arc::new(bus)
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().expect("lock").clone()
        }

        fn count(&self, verb: &str) -> usize {
            self.calls()
                .iter()
                .filter(|args| args.iter().any(|arg| arg == verb))
                .count()
        }

        fn writes(&self) -> Vec<Vec<String>> {
            self.calls()
                .into_iter()
                .filter(|args| args.iter().any(|arg| arg == "setvcp"))
                .collect()
        }

        fn close_gate(&self) {
            *self.gate.lock().expect("lock") = true;
        }

        /// Moves the simulated clock on.
        fn advance(&self, by: Duration) {
            *self.clock.lock().expect("lock") += by;
        }

        fn open_gate(&self) {
            *self.gate.lock().expect("lock") = false;
            self.opened.notify_all();
        }
    }

    struct FakeDdc(Arc<Bus>);

    struct FakeLease(Arc<Bus>);

    impl Drop for FakeLease {
        fn drop(&mut self) {
            *self.0.leased.lock().expect("lock") = false;
        }
    }

    impl DdcEnvironment for FakeDdc {
        type Lease = FakeLease;

        fn ddcutil(
            &mut self,
            args: &[String],
            _timeout: Duration,
            tick: &mut dyn FnMut() -> bool,
        ) -> Result<String, RunError> {
            let bus = &self.0;
            let now = bus.active.fetch_add(1, Ordering::AcqRel) + 1;
            bus.max_active.fetch_max(now, Ordering::AcqRel);
            bus.calls.lock().expect("lock").push(args.to_vec());
            let result = serve(bus, args, tick);
            bus.active.fetch_sub(1, Ordering::AcqRel);
            result
        }

        fn reap_abandoned(&mut self) -> bool {
            true
        }

        fn try_lease(&mut self) -> io::Result<Option<FakeLease>> {
            self.0.lease_attempts.fetch_add(1, Ordering::AcqRel);
            let mut leased = self.0.leased.lock().expect("lock");
            if *leased {
                return Ok(None);
            }
            *leased = true;
            Ok(Some(FakeLease(Arc::clone(&self.0))))
        }

        fn now(&mut self) -> Instant {
            static BASE: OnceLock<Instant> = OnceLock::new();
            *BASE.get_or_init(Instant::now) + *self.0.clock.lock().expect("lock")
        }

        fn sleep(&mut self, duration: Duration) {
            *self.0.clock.lock().expect("lock") += duration;
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// The fake `ddcutil`: blocks while the gate is closed, then answers
    /// from the bus's values.
    fn serve(
        bus: &Bus,
        args: &[String],
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<String, RunError> {
        let mut gated = false;
        loop {
            let closed = *bus.gate.lock().expect("lock");
            if !closed {
                break;
            }
            if !gated {
                gated = true;
                bus.gated.fetch_add(1, Ordering::AcqRel);
            }
            if bus.tick_while_gated.load(Ordering::Acquire) && !tick() {
                return Err(RunError::Cancelled);
            }
            let guard = bus.gate.lock().expect("lock");
            let _ = bus
                .opened
                .wait_timeout(guard, Duration::from_millis(2))
                .expect("lock");
        }
        // Give a concurrent worker the chance to overlap.
        thread::sleep(Duration::from_millis(2));
        if !tick() {
            return Err(RunError::Cancelled);
        }
        if bus.time_out.load(Ordering::Acquire) {
            return Err(RunError::Timeout);
        }
        let display = args
            .iter()
            .position(|arg| arg == "--display")
            .and_then(|at| args.get(at + 1))
            .and_then(|number| number.parse::<u8>().ok());
        if args.iter().any(|arg| arg == "detect") {
            return Ok(bus.detect.lock().expect("lock").clone());
        }
        let display = display.ok_or(RunError::Timeout)?;
        if args.iter().any(|arg| arg == "setvcp") {
            let value = args.last().and_then(|v| v.parse().ok()).unwrap_or(0);
            bus.values.lock().expect("lock").insert(display, value);
            return Ok(String::new());
        }
        let value = bus.values.lock().expect("lock").get(&display).copied();
        Ok(value.map_or_else(String::new, |v| format!("VCP 10 C {v} 100\n")))
    }

    /// No backlight at all, as on the author's machine.
    struct NoBacklight;

    impl BacklightEnvironment for NoBacklight {
        fn devices(&mut self) -> Vec<String> {
            Vec::new()
        }
        fn read(&mut self, _: &str) -> io::Result<Option<u8>> {
            Err(io::ErrorKind::NotFound.into())
        }
        fn wait(&mut self, timeout: Duration) -> io::Result<Vec<String>> {
            thread::sleep(timeout);
            Ok(Vec::new())
        }
        fn forget(&mut self, _: &str) {}
        fn set(&mut self, _: &str, _: u8, _: &mut dyn FnMut() -> bool) -> Result<(), RunError> {
            Err(RunError::Timeout)
        }
        fn sleep(&mut self, _: Duration) {}
    }

    struct Rig {
        handle: BrightnessHandle,
        workers: BrightnessWorkers,
        events: Receiver<BrightnessEvent>,
    }

    fn rig_with(bus: &Arc<Bus>, allowed: bool, backlight: impl BacklightEnvironment) -> Rig {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let (handle, workers) = spawn_with(
            FakeDdc(Arc::clone(bus)),
            backlight,
            allowed,
            &shutdown,
            move |event| tx.send(event).is_ok(),
        )
        .expect("the threads start");
        Rig {
            handle,
            workers,
            events,
        }
    }

    fn rig(bus: &Arc<Bus>) -> Rig {
        rig_with(bus, true, NoBacklight)
    }

    fn next(rig: &Rig) -> BrightnessEvent {
        rig.events
            .recv_timeout(Duration::from_secs(5))
            .expect("an event")
    }

    fn ddc(monitor: &str, percent: Option<u8>) -> BrightnessEvent {
        BrightnessEvent {
            monitor: monitor.to_owned(),
            percent,
            source: BrightnessSource::Ddc,
        }
    }

    fn change(monitor: &str, change: Change) -> BrightnessCommand {
        BrightnessCommand::Change {
            source: BrightnessSource::Ddc,
            monitor: monitor.to_owned(),
            change,
        }
    }

    fn until(what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn only_a_readable_refusal_turns_ddc_off() {
        for allowed in [
            None,
            Some(""),
            Some("  "),
            Some("1"),
            Some("true"),
            Some("00"),
        ] {
            assert!(ddc_requested(allowed), "{allowed:?}");
        }
        for refused in ["0", "false", "FALSE", " 0 "] {
            assert!(!ddc_requested(Some(refused)), "{refused}");
        }
    }

    #[test]
    fn nothing_runs_before_a_consumer_and_an_early_output_change_is_not_one() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig
            .handle
            .request(BrightnessCommand::OutputsChanged)
            .is_ok());
        thread::sleep(Duration::from_millis(150));
        assert!(bus.calls().is_empty());
        assert_eq!(bus.lease_attempts.load(Ordering::Acquire), 0);

        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        // Known but unread: unknown, never zero.
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        assert_eq!(bus.count("detect"), 1);
        assert_eq!(bus.count("getvcp"), 0, "no read without a reason");

        // A second consumer uses the cache.
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        thread::sleep(Duration::from_millis(150));
        assert_eq!(bus.count("detect"), 1);
        rig.workers.join();
    }

    #[test]
    fn nothing_runs_with_ddc_switched_off() {
        let bus = Bus::new();
        let rig = rig_with(&bus, false, NoBacklight);
        for command in [
            BrightnessCommand::Detect,
            BrightnessCommand::Read,
            change("DP-1", Change::Set(10)),
            BrightnessCommand::OutputsChanged,
        ] {
            assert!(rig.handle.request(command).is_ok());
        }
        thread::sleep(Duration::from_millis(200));
        assert!(bus.calls().is_empty());
        assert_eq!(bus.lease_attempts.load(Ordering::Acquire), 0);
        assert!(rig.events.try_recv().is_err());
        rig.workers.join();
    }

    #[test]
    fn a_write_is_read_back_and_a_step_from_unknown_reads_first() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig
            .handle
            .request(change("DP-1", Change::Step(Direction::Up)))
            .is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        assert_eq!(next(&rig), ddc("DP-1", Some(50)));
        assert_eq!(next(&rig), ddc("DP-1", Some(55)));
        let verbs: Vec<String> = bus
            .calls()
            .iter()
            .filter_map(|args| {
                args.iter()
                    .find(|arg| matches!(arg.as_str(), "detect" | "getvcp" | "setvcp"))
                    .cloned()
            })
            .collect();
        assert_eq!(verbs, ["detect", "getvcp", "setvcp", "getvcp"]);
        rig.workers.join();
    }

    #[test]
    fn a_burst_for_one_monitor_is_one_write_of_the_newest_target() {
        let bus = Bus::new();
        bus.tick_while_gated.store(true, Ordering::Release);
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));

        // The first write blocks on the bus; ten steps arrive meanwhile.
        bus.close_gate();
        assert!(rig.handle.request(change("DP-1", Change::Set(40))).is_ok());
        until("the first write", || bus.gated.load(Ordering::Acquire) == 1);
        for _ in 0..10 {
            assert!(rig
                .handle
                .request(change("DP-1", Change::Step(Direction::Down)))
                .is_ok());
            thread::sleep(Duration::from_millis(3));
        }
        assert!(rig.handle.request(change("DP-1", Change::Set(70))).is_ok());
        assert!(rig
            .handle
            .request(change("DP-1", Change::Step(Direction::Up)))
            .is_ok());
        thread::sleep(Duration::from_millis(30));
        bus.open_gate();

        // The level before the first write (read while the burst arrived).
        assert_eq!(next(&rig), ddc("DP-1", Some(50)));
        assert_eq!(next(&rig), ddc("DP-1", Some(40)));
        assert_eq!(next(&rig), ddc("DP-1", Some(75)));
        let writes = bus.writes();
        assert_eq!(writes.len(), 2, "{writes:?}");
        assert_eq!(writes[1].last().map(String::as_str), Some("75"));
        rig.workers.join();
    }

    #[test]
    fn two_workers_on_one_bus_never_overlap() {
        let bus = Bus::new();
        let first = rig(&bus);
        let second = rig(&bus);
        for round in 0..6u8 {
            for rig in [&first, &second] {
                assert!(rig
                    .handle
                    .request(change("DP-1", Change::Set(round * 10)))
                    .is_ok());
                assert!(rig.handle.request(BrightnessCommand::Read).is_ok());
            }
            thread::sleep(Duration::from_millis(5));
        }
        until("both workers to read back", || bus.count("getvcp") >= 4);
        thread::sleep(Duration::from_millis(200));
        assert!(bus.calls().len() > 4);
        assert_eq!(bus.max_active.load(Ordering::Acquire), 1);
        first.workers.join();
        second.workers.join();
    }

    #[test]
    fn a_full_queue_refuses_and_hands_the_command_back() {
        let bus = Bus::new();
        let rig = rig(&bus);
        bus.close_gate();
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        until("the detection to block", || {
            bus.gated.load(Ordering::Acquire) == 1
        });
        for percent in 0..COMMAND_QUEUE {
            let command = change("DP-1", Change::Set(u8::try_from(percent).expect("small")));
            assert!(rig.handle.request(command).is_ok());
        }
        let refused = rig.handle.request(change("DP-1", Change::Set(99)));
        assert_eq!(refused, Err(change("DP-1", Change::Set(99))));
        bus.open_gate();
        rig.workers.join();
    }

    #[test]
    fn a_busy_lease_waits_and_then_refuses() {
        let bus = Bus::new();
        *bus.leased.lock().expect("lock") = true;
        let mut env = FakeDdc(Arc::clone(&bus));
        let mut ticks = 0;
        let refused = acquire_lease(&mut env, &mut || {
            ticks += 1;
            true
        });
        assert!(matches!(refused, Err(OpError::LeaseRefused)));
        assert!(*bus.clock.lock().expect("lock") >= LEASE_PATIENCE);
        assert!(ticks > 1, "the queue is folded while waiting");

        // Released while waiting: taken.
        let release = Arc::clone(&bus);
        let mut tries = 0;
        let taken = acquire_lease(&mut env, &mut || {
            tries += 1;
            if tries == 3 {
                *release.leased.lock().expect("lock") = false;
            }
            true
        });
        assert!(taken.is_ok());

        // Shutdown while waiting: cancelled at once.
        drop(taken);
        *bus.leased.lock().expect("lock") = true;
        assert!(matches!(
            acquire_lease(&mut env, &mut || false),
            Err(OpError::Cancelled)
        ));
    }

    #[test]
    fn a_held_lease_runs_nothing_and_the_worker_goes_on_after() {
        let bus = Bus::new();
        *bus.leased.lock().expect("lock") = true;
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        // The fake clock runs out the patience: the detection is refused.
        until("the refusal", || {
            *bus.clock.lock().expect("lock") >= LEASE_PATIENCE
        });
        thread::sleep(Duration::from_millis(50));
        assert!(bus.calls().is_empty());
        // Found nothing, so a consumer asking again past the floor is
        // answered, now on a free bus.
        *bus.leased.lock().expect("lock") = false;
        bus.advance(REDETECT);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(bus.count("detect"), 1);
        rig.workers.join();
    }

    #[test]
    fn a_burst_of_output_changes_is_one_redetection() {
        let bus = Bus::new();
        bus.tick_while_gated.store(true, Ordering::Release);
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));

        bus.close_gate();
        bus.advance(OUTPUTS_REDETECT_FLOOR);
        assert!(rig
            .handle
            .request(BrightnessCommand::OutputsChanged)
            .is_ok());
        until("the redetection to block", || {
            bus.gated.load(Ordering::Acquire) == 1
        });
        // Outputs keep arriving during it: one more, not five.
        for _ in 0..5 {
            assert!(rig
                .handle
                .request(BrightnessCommand::OutputsChanged)
                .is_ok());
        }
        thread::sleep(Duration::from_millis(30));
        bus.open_gate();
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        // The follow-up keeps the floor after the redetection too.
        thread::sleep(Duration::from_millis(150));
        assert_eq!(bus.count("detect"), 2);
        bus.advance(OUTPUTS_REDETECT_FLOOR);
        until("the follow-up", || bus.count("detect") >= 3);
        thread::sleep(Duration::from_millis(200));
        assert_eq!(bus.count("detect"), 3);
        rig.workers.join();
    }

    #[test]
    fn asking_again_after_finding_nothing_waits_for_the_redetect_floor() {
        let bus = Bus::new();
        bus.detect.lock().expect("lock").clear();
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        until("the first detection", || bus.count("detect") == 1);
        // Twenty consumers (key presses, settings writes, outputs) ask again
        // at once: none of them is a reason for another child on the bus.
        for _ in 0..20 {
            let _ = rig.handle.request(BrightnessCommand::Detect);
            let _ = rig
                .handle
                .request(change("DP-1", Change::Step(Direction::Up)));
            thread::sleep(Duration::from_millis(2));
        }
        thread::sleep(Duration::from_millis(200));
        assert_eq!(bus.count("detect"), 1, "{:?}", bus.calls());
        // Once the floor has passed, the request still owed is answered once.
        bus.advance(REDETECT);
        until("the retry", || bus.count("detect") == 2);
        thread::sleep(Duration::from_millis(200));
        assert_eq!(bus.count("detect"), 2);
        assert!(bus.writes().is_empty(), "no monitor, no write");
        rig.workers.join();
    }

    #[test]
    fn outputs_arriving_right_after_a_detection_wait_for_the_short_floor() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        // Three outputs announced at start, after the first detection began.
        for _ in 0..3 {
            assert!(rig
                .handle
                .request(BrightnessCommand::OutputsChanged)
                .is_ok());
        }
        thread::sleep(Duration::from_millis(200));
        assert_eq!(bus.count("detect"), 1, "not back to back on the bus");
        bus.advance(OUTPUTS_REDETECT_FLOOR);
        until("the follow-up", || bus.count("detect") == 2);
        thread::sleep(Duration::from_millis(200));
        assert_eq!(bus.count("detect"), 2, "one follow-up, not three");
        rig.workers.join();
    }

    #[test]
    fn the_first_write_publishes_the_level_before_it_so_it_is_a_change() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig.handle.request(change("DP-1", Change::Set(70))).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        // The level before the write is the baseline the display compares
        // the read-back with.
        assert_eq!(next(&rig), ddc("DP-1", Some(50)));
        assert_eq!(next(&rig), ddc("DP-1", Some(70)));
        let verbs: Vec<String> = bus
            .calls()
            .iter()
            .filter_map(|args| {
                args.iter()
                    .find(|arg| matches!(arg.as_str(), "detect" | "getvcp" | "setvcp"))
                    .cloned()
            })
            .collect();
        assert_eq!(verbs, ["detect", "getvcp", "setvcp", "getvcp"]);
        // A known level is not read again before the next write.
        assert!(rig.handle.request(change("DP-1", Change::Set(20))).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", Some(20)));
        assert_eq!(bus.count("getvcp"), 3);
        rig.workers.join();
    }

    #[test]
    fn a_popup_read_reads_each_monitor_once() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Read).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        assert_eq!(next(&rig), ddc("DP-1", Some(50)));
        assert_eq!(next(&rig), ddc("DP-2", Some(30)));
        thread::sleep(Duration::from_millis(150));
        assert_eq!(bus.count("getvcp"), 2, "no periodic read");
        rig.workers.join();
    }

    #[test]
    fn a_timed_out_operation_leaves_the_monitor_unknown_and_the_worker_going() {
        let bus = Bus::new();
        let rig = rig(&bus);
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        bus.time_out.store(true, Ordering::Release);
        assert!(rig.handle.request(BrightnessCommand::Read).is_ok());
        assert_eq!(next(&rig), ddc("DP-1", None));
        assert_eq!(next(&rig), ddc("DP-2", None));
        bus.time_out.store(false, Ordering::Release);
        assert!(rig.handle.request(change("DP-2", Change::Set(80))).is_ok());
        // Unknown again, so the level before the write is read first.
        assert_eq!(next(&rig), ddc("DP-2", Some(30)));
        assert_eq!(next(&rig), ddc("DP-2", Some(80)));
        rig.workers.join();
    }

    #[test]
    fn a_join_stops_a_worker_in_the_middle_of_an_operation() {
        let bus = Bus::new();
        bus.tick_while_gated.store(true, Ordering::Release);
        let rig = rig(&bus);
        bus.close_gate();
        assert!(rig.handle.request(BrightnessCommand::Detect).is_ok());
        until("the operation to start", || {
            bus.gated.load(Ordering::Acquire) == 1
        });
        let started = Instant::now();
        assert!(rig.workers.join_within(Duration::from_secs(2)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(bus.active.load(Ordering::Acquire), 0);
        // The worker is gone: commands are refused.
        assert!(rig.handle.request(BrightnessCommand::Read).is_err());
        assert!(!*bus.leased.lock().expect("lock"), "the lease is released");
    }

    #[test]
    fn the_runner_kills_and_reaps_a_child_past_its_timeout() {
        let mut abandoned = None;
        let started = Instant::now();
        let result = run_bounded(
            "sleep",
            &["5".to_owned()],
            Duration::from_millis(150),
            &mut || true,
            &mut abandoned,
        );
        assert!(matches!(result, Err(RunError::Timeout)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(abandoned.is_none(), "the child was reaped");

        // Cancelled the same way, and an exiting child answers its output.
        let result = run_bounded(
            "sleep",
            &["5".to_owned()],
            DDC_TIMEOUT,
            &mut || false,
            &mut abandoned,
        );
        assert!(matches!(result, Err(RunError::Cancelled)), "{result:?}");
        let echoed = run_bounded(
            "echo",
            &["VCP 10 C 5 10".to_owned()],
            DDC_TIMEOUT,
            &mut || true,
            &mut abandoned,
        );
        assert_eq!(echoed.ok().as_deref(), Some("VCP 10 C 5 10\n"));
    }

    #[test]
    fn the_overlap_instrument_reports_a_second_entry() {
        let flight = InFlight::new();
        let (first, overlapped) = flight.enter();
        assert!(!overlapped);
        let (_second, overlapped) = flight.enter();
        assert!(overlapped);
        drop(first);
        drop(_second);
        assert!(!flight.enter().1);
    }

    #[test]
    fn targets_keep_the_newest_and_sum_steps_within_range() {
        let up = Change::Step(Direction::Up);
        let down = Change::Step(Direction::Down);
        assert_eq!(Target::from_change(up).followed_by(up), Target::Relative(2));
        assert_eq!(
            Target::Relative(5).followed_by(Change::Set(20)),
            Target::Absolute(20)
        );
        assert_eq!(Target::Absolute(20).followed_by(down), Target::Absolute(15));
        assert_eq!(Target::from_change(Change::Set(200)), Target::Absolute(100));
        let mut many = Target::from_change(up);
        for _ in 0..100 {
            many = many.followed_by(up);
        }
        assert_eq!(many, Target::Relative(MAX_PENDING_STEPS));
        assert_eq!(many.resolve(Some(10)), Some(100));
        assert_eq!(Target::Relative(-3).resolve(Some(10)), Some(0));
        assert_eq!(Target::Relative(1).resolve(None), None);
    }

    #[test]
    fn backlight_numbers_are_bounded_and_percent_of_max() {
        assert_eq!(parse_sysfs_number(b"512\n"), Some(512));
        assert_eq!(parse_sysfs_number(b"-1\n"), None);
        assert_eq!(parse_sysfs_number(b"x"), None);
        assert_eq!(backlight_percent(256, 512), Some(50));
        assert_eq!(backlight_percent(600, 512), Some(100));
        assert_eq!(backlight_percent(1, 0), None);
        assert!(valid_device_name("intel_backlight"));
        assert!(valid_device_name("amdgpu_bl1"));
        for bad in ["", "..", ".hidden", "a/b", "a b", "a\nb"] {
            assert!(!valid_device_name(bad), "{bad}");
        }
    }

    /// A backlight whose notifications the test triggers.
    struct FakeBacklight {
        level: Arc<Mutex<u8>>,
        notify: Receiver<()>,
        sets: Arc<Mutex<Vec<u8>>>,
    }

    impl BacklightEnvironment for FakeBacklight {
        fn devices(&mut self) -> Vec<String> {
            vec!["panel".to_owned()]
        }
        fn read(&mut self, _: &str) -> io::Result<Option<u8>> {
            Ok(Some(*self.level.lock().expect("lock")))
        }
        fn wait(&mut self, timeout: Duration) -> io::Result<Vec<String>> {
            Ok(match self.notify.recv_timeout(timeout) {
                Ok(()) => vec!["panel".to_owned()],
                Err(_) => Vec::new(),
            })
        }
        fn forget(&mut self, _: &str) {}
        fn set(
            &mut self,
            _: &str,
            percent: u8,
            _: &mut dyn FnMut() -> bool,
        ) -> Result<(), RunError> {
            self.sets.lock().expect("lock").push(percent);
            *self.level.lock().expect("lock") = percent;
            Ok(())
        }
        fn sleep(&mut self, _: Duration) {}
    }

    #[test]
    fn a_backlight_is_published_on_notification_only_and_set_on_command() {
        let bus = Bus::new();
        let level = Arc::new(Mutex::new(40));
        let sets = Arc::new(Mutex::new(Vec::new()));
        let (notify_tx, notify) = mpsc::channel();
        let rig = rig_with(
            &bus,
            true,
            FakeBacklight {
                level: Arc::clone(&level),
                notify,
                sets: Arc::clone(&sets),
            },
        );
        let backlight = |percent| BrightnessEvent {
            monitor: "panel".to_owned(),
            percent: Some(percent),
            source: BrightnessSource::Backlight,
        };
        assert_eq!(next(&rig), backlight(40));
        // No notification, no event: nothing is polled.
        assert!(rig.events.recv_timeout(Duration::from_millis(250)).is_err());
        *level.lock().expect("lock") = 60;
        notify_tx.send(()).expect("the worker listens");
        assert_eq!(next(&rig), backlight(60));

        let command = BrightnessCommand::Change {
            source: BrightnessSource::Backlight,
            monitor: "panel".to_owned(),
            change: Change::Step(Direction::Down),
        };
        assert!(rig.handle.request(command).is_ok());
        assert_eq!(next(&rig), backlight(55));
        assert_eq!(*sets.lock().expect("lock"), [55]);
        assert!(bus.calls().is_empty(), "a backlight never touches DDC");
        rig.workers.join();
    }

    #[test]
    fn the_real_sysfs_reader_reads_a_device_directory() {
        let root = std::env::temp_dir().join(format!("celestina-bl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let device = root.join("panel_bl");
        std::fs::create_dir_all(&device).expect("a scratch directory");
        std::fs::write(device.join("max_brightness"), b"200\n").expect("a file");
        std::fs::write(device.join("actual_brightness"), b"50\n").expect("a file");
        std::fs::create_dir_all(root.join("bad name")).expect("a scratch directory");
        let mut sysfs = SysfsBacklight::new(root.clone());
        let devices = sysfs.devices();
        let level = sysfs.read("panel_bl").expect("readable");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(devices, ["panel_bl"]);
        assert_eq!(level, Some(25));
    }
}
