//! When the shell's own session lock is started, what its confirmations mean,
//! and the only way to mint a proof that the session is covered (CEL-2-H,
//! design `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`
//! §4.4).
//!
//! The lock itself is another process (`celestina-lock`) that talks to the
//! compositor; this module is the shell's side of the conversation, a state
//! machine with no IO, no thread and no clock of its own. Workers feed it
//! [`Event`]s, each with the time they happened, and carry out the [`Action`]s
//! it answers with.
//!
//! Three rules shape every answer.
//!
//! **Coverage is only what this shell's own child said.** The policy never
//! believes a file, a process id or another locker. A shell restarted under a
//! running lock is [`State::Idle`]; its next request asks the compositor,
//! which refuses while that lock lives (exit 10), and that refusal never
//! suspends anything. The module imports nothing that could read a file or a
//! process, and no event carries a path or an id.
//!
//! **A suspend needs a proof.** [`LockedProof`] has no public constructor; the
//! policy mints one only while it is [`State::Locked`], and only from a request
//! to suspend or from the `locked` line of the child it started. Each proof
//! carries the lock *generation*, a counter the policy bumps on every
//! `unlocked` line, every child exit and every time the lock becomes
//! unavailable. The logind worker compares a proof's generation with the
//! current one, through a [`GenerationReader`], immediately before it suspends,
//! and drops a stale proof (design §4.9).
//!
//! **The sleep inhibitor is held exactly while it can help.** A logind delay
//! inhibitor is held iff the duty is on, the lock is available, the session is
//! not [`State::Locked`] and no sleep is being prepared. Once logind announced
//! a sleep the policy never takes a new one (logind refuses it with
//! `OperationInProgress`, spike Q17); it only lets go of the one it holds, as
//! soon as the lock is confirmed or can no longer be, and takes it again when
//! the sleep is over.
//!
//! # What the workers must keep true
//!
//! The policy trusts its callers on two points. Events about the child
//! (`Line`, `Exit`, `SpawnFailed`) come only from the child the last
//! [`Action::Spawn`] started, and a new child is started only after the
//! previous one's exit was delivered (or it was killed through its pidfd and
//! reaped). And every [`Action::Suspend`] is checked against the child's pidfd
//! and a [`GenerationReader`] by the worker that hands it to logind.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_lock_proto::{AttemptKind, LockExit, StatusLine, PROTOCOL_VERSION};

/// A `Spawning` child that has not printed `requested` by now never asked to
/// lock, so it is killed.
pub const SPAWN_WATCHDOG: Duration = Duration::from_secs(5);
/// A `Requested` child that has not printed `locked` by now is logged, never
/// killed: it may be about to lock.
pub const REQUESTED_WATCHDOG: Duration = Duration::from_secs(10);
/// An `Ending` child that has not exited by now has unlocked by protocol.
pub const ENDING_WATCHDOG: Duration = Duration::from_secs(5);
/// How long a request to suspend waits for the lock to be confirmed.
pub const INTENT_WINDOW: Duration = Duration::from_secs(10);
/// How long to wait before asking the compositor again after it refused.
pub const RETRY_DELAY: Duration = Duration::from_millis(500);
/// The span in which retries and relaunches are counted.
pub const WINDOW: Duration = Duration::from_secs(60);
/// Retries after the compositor refused a lock (exit 10) within [`WINDOW`].
pub const MAX_REFUSAL_RETRIES: usize = 2;
/// Relaunches of a lock that died within [`WINDOW`].
pub const MAX_RELAUNCHES: usize = 3;

/// Who asked to suspend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The idle chain's second step. Input cancels it.
    Idle,
    /// A person's `lock suspend`. Pressing the binding is input too, so
    /// input never cancels it.
    Verb,
}

/// Why the lock is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// `duties.lock` is off.
    DutyOff,
    /// A binary is not pinned (missing, or it failed the `fstat` checks). A
    /// missing PAM service file is *not* a reason: the lock still covers.
    NotPinned,
}

/// What the policy believes about the lock process it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Duty off, or a binary not pinned.
    Unavailable(Why),
    /// Available, no lock child of this shell runs.
    Idle,
    /// A spawn was handed to the lock worker.
    Spawning { since: Instant },
    /// The child printed `requested`.
    Requested { since: Instant },
    /// **This shell's** child printed `locked` and has not exited.
    Locked,
    /// The child printed `unlocked`; waiting for its exit.
    Ending { since: Instant },
}

/// Whether logind has announced a sleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sleep {
    Awake,
    /// `PrepareForSleep(true)` arrived and `false` has not.
    Preparing,
}

/// The delay inhibitor, as the policy asked for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inhibitor {
    /// Asked for with [`Action::HoldInhibitor`] and not reported failed.
    Held,
    /// Not held.
    Released,
    /// The worker reported the last request failed; not asked for again until
    /// something changes.
    Failed,
}

/// Why taking the inhibitor failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InhibitorFailure {
    /// The system bus or logind did not answer.
    Bus,
    /// logind refused because a sleep is already pending
    /// (`OperationInProgress`): the expected answer to a late request.
    SleepInProgress,
}

/// How the lock child ended, as the shell reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildExit {
    /// An exit status of the table of design §4.3 (b).
    Status(LockExit),
    /// A signal, or a status the table does not name: the lock died.
    Died,
}

impl ChildExit {
    /// Reads a finished child: its exit `code` if it exited, `signaled` if a
    /// signal ended it.
    #[must_use]
    pub fn from_wait(code: Option<i32>, signaled: bool) -> Self {
        if signaled {
            return Self::Died;
        }
        code.and_then(LockExit::from_status)
            .map_or(Self::Died, Self::Status)
    }
}

/// What happened, as the workers report it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// `duties.lock` was switched.
    Duty(bool),
    /// Both binaries are (or are no longer) pinned and pass the checks.
    Pinned(bool),
    /// The PAM service file is missing, found when the duty turned on.
    PamFileMissing,
    /// The `lock` verb.
    Lock,
    /// The `lock suspend` verb, or the idle chain's second step.
    LockAndSuspend(Origin),
    /// A status line of the current child.
    Line(StatusLine),
    /// The current child ended.
    Exit(ChildExit),
    /// The spawn the worker was asked for did not start a child.
    SpawnFailed,
    /// The one-second timer; it runs only while [`LockPolicy::wants_tick`].
    Tick,
    /// Input came back after the idle chain asked to lock and suspend.
    IdleResumed,
    /// logind's `PrepareForSleep`.
    PrepareForSleep(bool),
    /// The inhibitor the policy asked for was not taken.
    InhibitorFailed(InhibitorFailure),
    /// The sleep worker (re)connected to the system bus: whatever stopped it
    /// from taking the inhibitor may be over, so a failed one is asked for
    /// again.
    SleepBusBack,
}

/// Why a desktop notice is posted. Reasons, never text: the words are the
/// product's (`celestina/src/copy.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// An explicit request met an unavailable lock.
    Unavailable(Why),
    /// An explicit request ended without a lock child that locked.
    NotLocked,
    /// The compositor refused the lock and the retries ran out.
    CompositorRefused,
    /// A suspend was dropped because the lock was not confirmed in time.
    NotConfirmed,
    /// The lock died more often than it is relaunched.
    RelaunchLimit,
    /// The PAM service file is not installed.
    PamMissing,
}

/// How loud a log line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

/// Why the policy wants a line in the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogReason {
    /// The duty turned on: a reminder to turn Noctalia's idle lock off.
    DutyOn,
    /// An explicit request was refused because the lock is unavailable.
    RequestRefused(Why),
    /// The idle chain's step was refused because the lock is unavailable.
    IdleRequestRefused(Why),
    /// A request to suspend was recorded.
    IntentSet(Origin),
    /// A request to suspend was not served in time.
    IntentExpired,
    /// Input came back and cancelled the request idle made.
    IntentCancelledByInput,
    /// The child said a protocol this shell does not speak.
    UnknownProtocol,
    /// A line that does not fit the state the policy is in.
    UnexpectedLine,
    /// An exit with no child to account for it.
    StrayExit,
    /// The child exited before it asked to lock.
    ExitBeforeRequest,
    /// The spawn did not start a child.
    SpawnFailed,
    /// A `Spawning` child was killed after [`SPAWN_WATCHDOG`].
    SpawnWatchdog,
    /// The compositor refused the lock; asking again.
    RetryingAfterRefusal,
    /// The compositor refused the lock and the retries ran out.
    CompositorRefused,
    /// The lock died after it asked to lock; a new one is started.
    Relaunching,
    /// The lock died more often than it is relaunched.
    RelaunchLimit,
    /// The compositor ended the lock (`finished` after `locked`).
    FinishedAfterLocked,
    /// The Wayland connection was lost: the session it protected is gone.
    ConnectionLost,
    /// The child was not confirmed `locked` within [`REQUESTED_WATCHDOG`].
    RequestedTimeout,
    /// The child did not exit within [`ENDING_WATCHDOG`] of unlocking.
    EndingTimeout,
    /// A sleep was announced while the lock is unavailable.
    SleepWhileUncovered,
    /// The sleep ended and the session had not been locked first.
    SleptBeforeLocked,
    /// The PAM service file is missing.
    PamFileMissing,
    /// The worker could not take the inhibitor.
    InhibitorFailed(InhibitorFailure),
}

impl LogReason {
    /// How loud the line is.
    #[must_use]
    pub fn level(self) -> Level {
        match self {
            Self::DutyOn
            | Self::IntentSet(_)
            | Self::IntentCancelledByInput
            | Self::RetryingAfterRefusal
            | Self::Relaunching
            | Self::IdleRequestRefused(_)
            | Self::StrayExit
            | Self::InhibitorFailed(InhibitorFailure::SleepInProgress) => Level::Info,
            Self::RequestRefused(_)
            | Self::IntentExpired
            | Self::UnknownProtocol
            | Self::UnexpectedLine
            | Self::ExitBeforeRequest
            | Self::SpawnFailed
            | Self::SpawnWatchdog
            | Self::CompositorRefused
            | Self::EndingTimeout
            | Self::SleptBeforeLocked
            | Self::PamFileMissing
            | Self::InhibitorFailed(InhibitorFailure::Bus) => Level::Warn,
            Self::RelaunchLimit
            | Self::FinishedAfterLocked
            | Self::ConnectionLost
            | Self::RequestedTimeout
            | Self::SleepWhileUncovered => Level::Error,
        }
    }
}

/// What the workers must do.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Start a lock child after `after`, handing it `prior_failures` in the
    /// bundle header.
    Spawn { prior_failures: u8, after: Duration },
    /// Kill the child through its pidfd. Only ever for a child that never
    /// asked to lock or that already said `unlocked`, so killing it uncovers
    /// nothing. The policy expects no further event from that child.
    KillChild,
    /// Take the logind delay inhibitor.
    HoldInhibitor,
    /// Let the logind delay inhibitor go.
    ReleaseInhibitor,
    /// Suspend the machine: the session is covered, here is the proof.
    Suspend(LockedProof),
    /// Post a desktop notice (rate limited by the poster).
    Notice(Notice),
    /// Tell the idle chain whether the lock is available.
    PublishAvailability(bool),
    /// Write a line to the log.
    Log(LogReason),
}

/// The proof that this shell's lock was confirmed `locked` and has not ended.
///
/// It has no public constructor and is not `Clone`: the policy mints it in
/// [`State::Locked`] only, and whoever suspends consumes it.
#[derive(Debug, PartialEq, Eq)]
pub struct LockedProof {
    generation: u64,
}

impl LockedProof {
    /// The lock generation the proof was minted in.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// A read-only view of the policy's generation counter. Only the policy
/// writes it.
#[derive(Clone, Debug)]
pub struct GenerationReader(Arc<AtomicU64>);

impl GenerationReader {
    /// The current generation.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Whether `proof` was minted in the current generation, that is, nothing
    /// has ended or replaced the lock since.
    #[must_use]
    pub fn is_current(&self, proof: &LockedProof) -> bool {
        self.current() == proof.generation
    }
}

/// A pending request to lock and then suspend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Intent {
    pub origin: Origin,
    /// After this the lock is no longer waited for.
    pub deadline: Instant,
}

/// The shell's lock policy: states and events of design §4.4.
#[derive(Debug)]
pub struct LockPolicy {
    state: State,
    duty: bool,
    pinned: bool,
    sleep: Sleep,
    inhibitor: Inhibitor,
    intent: Option<Intent>,
    /// A `lock` asked for while the child was ending, served by its exit.
    pending_lock: bool,
    /// Whether a person asked for the lock in progress (a verb): its failure
    /// is posted as a notice.
    explicit: bool,
    /// Counted `attempt refused|account` lines since the last `unlocked`.
    failures: u8,
    refusal_retries: VecDeque<Instant>,
    relaunches: VecDeque<Instant>,
    requested_timed_out: bool,
    generation: Arc<AtomicU64>,
}

impl Default for LockPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl LockPolicy {
    /// A policy with the duty off: unavailable until the workers say
    /// otherwise.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Unavailable(Why::DutyOff),
            duty: false,
            pinned: false,
            sleep: Sleep::Awake,
            inhibitor: Inhibitor::Released,
            intent: None,
            pending_lock: false,
            explicit: false,
            failures: 0,
            refusal_retries: VecDeque::new(),
            relaunches: VecDeque::new(),
            requested_timed_out: false,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Whether the idle chain's second step may be accepted: the duty is on
    /// and both binaries are pinned.
    #[must_use]
    pub fn available(&self) -> bool {
        self.duty && self.pinned
    }

    #[must_use]
    pub fn sleep(&self) -> Sleep {
        self.sleep
    }

    #[must_use]
    pub fn inhibitor(&self) -> Inhibitor {
        self.inhibitor
    }

    #[must_use]
    pub fn intent(&self) -> Option<Intent> {
        self.intent
    }

    /// The failures counted since the last unlock.
    #[must_use]
    pub fn failures(&self) -> u8 {
        self.failures
    }

    /// Whether the one-second timer must run: a child is being waited for, or
    /// a request to suspend is pending.
    #[must_use]
    pub fn wants_tick(&self) -> bool {
        !matches!(self.state, State::Idle | State::Unavailable(_)) || self.intent.is_some()
    }

    /// The current lock generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// A reader for the logind worker's pre-suspend check.
    #[must_use]
    pub fn generation_reader(&self) -> GenerationReader {
        GenerationReader(Arc::clone(&self.generation))
    }

    /// Takes one event in at time `now` and returns what to do about it.
    pub fn handle(&mut self, now: Instant, event: Event) -> Vec<Action> {
        let was_available = self.available();
        let mut out = Vec::new();
        match event {
            Event::Duty(on) => {
                let turned_on = on && !self.duty;
                self.duty = on;
                self.retry_inhibitor();
                self.availability_changed();
                if turned_on {
                    out.push(Action::Log(LogReason::DutyOn));
                }
            }
            Event::Pinned(pinned) => {
                self.pinned = pinned;
                self.retry_inhibitor();
                self.availability_changed();
            }
            Event::PamFileMissing => {
                out.push(Action::Log(LogReason::PamFileMissing));
                out.push(Action::Notice(Notice::PamMissing));
            }
            Event::Lock => self.on_lock(now, &mut out),
            Event::LockAndSuspend(origin) => self.on_lock_and_suspend(origin, now, &mut out),
            Event::Line(line) => self.on_line(line, now, &mut out),
            Event::Exit(exit) => self.on_exit(exit, now, &mut out),
            Event::SpawnFailed => {
                if matches!(self.state, State::Spawning { .. }) {
                    self.give_up(Notice::NotLocked, LogReason::SpawnFailed, &mut out);
                }
            }
            Event::Tick => self.on_tick(now, &mut out),
            Event::IdleResumed => {
                if self
                    .intent
                    .is_some_and(|intent| intent.origin == Origin::Idle)
                {
                    self.intent = None;
                    out.push(Action::Log(LogReason::IntentCancelledByInput));
                }
            }
            Event::PrepareForSleep(true) => self.on_sleep_announced(now, &mut out),
            Event::PrepareForSleep(false) => {
                let was_preparing = self.sleep == Sleep::Preparing;
                self.sleep = Sleep::Awake;
                // Only the end of a sleep that was being prepared changes
                // anything. The worker also reports the property's value on
                // every request, and a `false` that finds the policy awake
                // must not ask a failed inhibitor again (that would loop:
                // request, report, retry, request).
                if was_preparing {
                    self.retry_inhibitor();
                    if !matches!(self.state, State::Locked) {
                        out.push(Action::Log(LogReason::SleptBeforeLocked));
                    }
                }
            }
            Event::SleepBusBack => self.retry_inhibitor(),
            Event::InhibitorFailed(failure) => {
                if self.inhibitor == Inhibitor::Held {
                    self.inhibitor = Inhibitor::Failed;
                }
                out.push(Action::Log(LogReason::InhibitorFailed(failure)));
            }
        }
        self.sync_inhibitor(&mut out);
        if self.available() != was_available {
            out.push(Action::PublishAvailability(self.available()));
        }
        out
    }

    /// A proof, if and only if the session is covered by this shell's lock.
    fn mint(&self) -> Option<LockedProof> {
        match self.state {
            State::Locked => Some(LockedProof {
                generation: self.generation(),
            }),
            State::Unavailable(_)
            | State::Idle
            | State::Spawning { .. }
            | State::Requested { .. }
            | State::Ending { .. } => None,
        }
    }

    fn bump_generation(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// A failed inhibitor is asked for again once something has changed.
    fn retry_inhibitor(&mut self) {
        if self.inhibitor == Inhibitor::Failed {
            self.inhibitor = Inhibitor::Released;
        }
    }

    /// Takes or lets go of the inhibitor so the invariant holds: held iff the
    /// lock is available, the session is uncovered and awake. While a sleep is
    /// prepared only a lock still being waited for keeps one.
    fn sync_inhibitor(&mut self, out: &mut Vec<Action>) {
        let covered = matches!(self.state, State::Locked);
        let usable = self.available();
        let waiting_for_lock = matches!(
            self.state,
            State::Spawning { .. } | State::Requested { .. } | State::Ending { .. }
        );
        match self.inhibitor {
            Inhibitor::Held => {
                let keep = usable && !covered && (self.sleep == Sleep::Awake || waiting_for_lock);
                if !keep {
                    self.inhibitor = Inhibitor::Released;
                    out.push(Action::ReleaseInhibitor);
                }
            }
            Inhibitor::Released => {
                if usable && !covered && self.sleep == Sleep::Awake {
                    self.inhibitor = Inhibitor::Held;
                    out.push(Action::HoldInhibitor);
                }
            }
            Inhibitor::Failed => {}
        }
    }

    /// Lets the inhibitor go if a sleep is being prepared, even though the
    /// state does not change (a lock that is not coming in time).
    fn release_if_preparing(&mut self, out: &mut Vec<Action>) {
        if self.sleep == Sleep::Preparing && self.inhibitor == Inhibitor::Held {
            self.inhibitor = Inhibitor::Released;
            out.push(Action::ReleaseInhibitor);
        }
    }

    /// The duty or a pin changed: the state follows availability.
    fn availability_changed(&mut self) {
        let was_available = !matches!(self.state, State::Unavailable(_));
        if self.available() {
            if !was_available {
                self.state = State::Idle;
            }
            return;
        }
        let why = if self.duty {
            Why::NotPinned
        } else {
            Why::DutyOff
        };
        if was_available {
            // The running lock, if any, is not touched: only PAM ends it. The
            // shell just stops believing it can vouch for it.
            self.bump_generation();
            self.forget_requests();
            // Nothing is guarded any more, so no sleep is being prepared as
            // far as the policy goes: a later run learns the truth again from
            // the sleep worker's first report.
            self.sleep = Sleep::Awake;
            self.failures = 0;
            self.refusal_retries.clear();
            self.relaunches.clear();
        }
        self.state = State::Unavailable(why);
    }

    /// Drops every request that was waiting for a lock.
    fn forget_requests(&mut self) {
        self.intent = None;
        self.pending_lock = false;
        self.explicit = false;
        self.requested_timed_out = false;
    }

    /// Back to idle with nothing pending.
    fn settle_idle(&mut self) {
        self.forget_requests();
        self.state = State::Idle;
    }

    /// The attempt to lock failed: back to idle, and tell the person if they
    /// asked.
    fn give_up(&mut self, notice: Notice, reason: LogReason, out: &mut Vec<Action>) {
        out.push(Action::Log(reason));
        if self.explicit {
            out.push(Action::Notice(notice));
        }
        self.settle_idle();
    }

    fn spawn(&mut self, now: Instant, after: Duration, out: &mut Vec<Action>) {
        self.state = State::Spawning { since: now };
        self.requested_timed_out = false;
        out.push(Action::Spawn {
            prior_failures: self.failures,
            after,
        });
    }

    fn set_intent(&mut self, origin: Origin, now: Instant, out: &mut Vec<Action>) {
        // One intent at a time: a verb's request is not downgraded to idle's.
        let keep = origin == Origin::Idle
            && self
                .intent
                .is_some_and(|intent| intent.origin == Origin::Verb);
        if !keep {
            self.intent = Some(Intent {
                origin,
                deadline: now + INTENT_WINDOW,
            });
        }
        if origin == Origin::Verb {
            self.explicit = true;
        }
        out.push(Action::Log(LogReason::IntentSet(origin)));
    }

    fn on_lock(&mut self, now: Instant, out: &mut Vec<Action>) {
        match self.state {
            State::Unavailable(why) => {
                out.push(Action::Log(LogReason::RequestRefused(why)));
                out.push(Action::Notice(Notice::Unavailable(why)));
            }
            State::Idle => {
                self.explicit = true;
                self.spawn(now, Duration::ZERO, out);
            }
            State::Spawning { .. } | State::Requested { .. } => self.explicit = true,
            State::Locked => {}
            State::Ending { .. } => {
                self.explicit = true;
                self.pending_lock = true;
            }
        }
    }

    fn on_lock_and_suspend(&mut self, origin: Origin, now: Instant, out: &mut Vec<Action>) {
        match self.state {
            State::Unavailable(why) => {
                if origin == Origin::Verb {
                    out.push(Action::Log(LogReason::RequestRefused(why)));
                    out.push(Action::Notice(Notice::Unavailable(why)));
                } else {
                    // Idle already refused its own step: nobody is there.
                    out.push(Action::Log(LogReason::IdleRequestRefused(why)));
                }
            }
            State::Idle => {
                self.set_intent(origin, now, out);
                self.spawn(now, Duration::ZERO, out);
            }
            State::Spawning { .. } | State::Requested { .. } | State::Ending { .. } => {
                self.set_intent(origin, now, out);
            }
            State::Locked => {
                out.push(Action::Log(LogReason::IntentSet(origin)));
                if let Some(proof) = self.mint() {
                    out.push(Action::Suspend(proof));
                }
            }
        }
    }

    fn on_line(&mut self, line: StatusLine, now: Instant, out: &mut Vec<Action>) {
        match (self.state, line) {
            (_, StatusLine::Hello(version)) => {
                if version != PROTOCOL_VERSION {
                    out.push(Action::Log(LogReason::UnknownProtocol));
                }
            }
            (State::Spawning { .. }, StatusLine::Requested) => {
                self.state = State::Requested { since: now };
                self.requested_timed_out = false;
            }
            (State::Spawning { .. } | State::Requested { .. }, StatusLine::Locked) => {
                self.on_locked(now, out);
            }
            (State::Locked, StatusLine::Attempt(kind)) => {
                if matches!(kind, AttemptKind::Refused | AttemptKind::Account) {
                    self.failures = self.failures.saturating_add(1);
                }
            }
            (State::Locked, StatusLine::Unlocked) => {
                self.state = State::Ending { since: now };
                self.failures = 0;
                self.bump_generation();
                self.retry_inhibitor();
                if self.sleep == Sleep::Preparing {
                    // A sleep is under way and the session was just
                    // uncovered: the exit asks for a lock again at once.
                    self.pending_lock = true;
                }
            }
            _ => out.push(Action::Log(LogReason::UnexpectedLine)),
        }
    }

    fn on_locked(&mut self, now: Instant, out: &mut Vec<Action>) {
        self.state = State::Locked;
        self.explicit = false;
        self.requested_timed_out = false;
        self.refusal_retries.clear();
        // The inhibitor goes first: logind should not wait for it while the
        // suspend that follows is already on its way.
        self.sync_inhibitor(out);
        match self.intent.take() {
            Some(intent) if now < intent.deadline => {
                if let Some(proof) = self.mint() {
                    out.push(Action::Suspend(proof));
                }
            }
            Some(intent) => {
                out.push(Action::Log(LogReason::IntentExpired));
                if intent.origin == Origin::Verb {
                    out.push(Action::Notice(Notice::NotConfirmed));
                }
            }
            None => {}
        }
    }

    fn on_exit(&mut self, exit: ChildExit, now: Instant, out: &mut Vec<Action>) {
        if matches!(self.state, State::Spawning { .. })
            && exit == ChildExit::Status(LockExit::Unlocked)
        {
            // A child that has not said `requested` cannot exit 0, so this is
            // the old child of `Ending`, which left by itself just as the
            // watchdog fired: the worker published its exit before it read
            // `KillChild`, and the policy is already `Spawning` (finding
            // m-1). Taking it for the new child's would drop a lock that is
            // about to run, and with it the sleep inhibitor.
            out.push(Action::Log(LogReason::StrayExit));
            return;
        }
        self.bump_generation();
        self.retry_inhibitor();
        if exit == ChildExit::Status(LockExit::ConnectionLost)
            && !matches!(self.state, State::Idle | State::Unavailable(_))
        {
            // The compositor is gone and so is the session it protected.
            out.push(Action::Log(LogReason::ConnectionLost));
            self.settle_idle();
            return;
        }
        match self.state {
            State::Unavailable(_) | State::Idle => out.push(Action::Log(LogReason::StrayExit)),
            State::Spawning { .. } => match exit {
                ChildExit::Status(LockExit::FinishedBeforeLocked) => self.refused(now, out),
                _ => self.give_up(Notice::NotLocked, LogReason::ExitBeforeRequest, out),
            },
            State::Requested { .. } => match exit {
                ChildExit::Status(LockExit::FinishedBeforeLocked) => self.refused(now, out),
                _ => self.died(now, out),
            },
            State::Locked => match exit {
                ChildExit::Status(LockExit::FinishedAfterLocked) => {
                    out.push(Action::Log(LogReason::FinishedAfterLocked));
                    self.settle_idle();
                }
                _ => self.died(now, out),
            },
            State::Ending { .. } => {
                let serve = self.pending_lock || self.intent.is_some();
                self.pending_lock = false;
                if serve {
                    self.spawn(now, Duration::ZERO, out);
                } else {
                    self.settle_idle();
                }
            }
        }
    }

    /// The compositor refused the lock: another client holds it, or the
    /// compositor could not render a locked frame. Ask again a couple of
    /// times, then give up. Never suspends.
    fn refused(&mut self, now: Instant, out: &mut Vec<Action>) {
        Self::prune(&mut self.refusal_retries, now);
        if self.refusal_retries.len() < MAX_REFUSAL_RETRIES {
            self.refusal_retries.push_back(now);
            self.spawn(now, RETRY_DELAY, out);
            out.push(Action::Log(LogReason::RetryingAfterRefusal));
        } else {
            self.give_up(Notice::CompositorRefused, LogReason::CompositorRefused, out);
        }
    }

    /// The lock died after it asked to lock: start another with the failures
    /// so far, within the relaunch bound.
    fn died(&mut self, now: Instant, out: &mut Vec<Action>) {
        Self::prune(&mut self.relaunches, now);
        if self.relaunches.len() < MAX_RELAUNCHES {
            self.relaunches.push_back(now);
            // A relaunch that cannot start leaves the session uncovered, so
            // the person is told, whoever asked for the lock (finding M1).
            self.explicit = true;
            self.spawn(now, Duration::ZERO, out);
            out.push(Action::Log(LogReason::Relaunching));
        } else {
            // Posted whoever asked: the session may be sitting under a dead
            // lock and the person should know.
            out.push(Action::Log(LogReason::RelaunchLimit));
            out.push(Action::Notice(Notice::RelaunchLimit));
            self.settle_idle();
        }
    }

    fn prune(times: &mut VecDeque<Instant>, now: Instant) {
        while times
            .front()
            .is_some_and(|time| now.saturating_duration_since(*time) > WINDOW)
        {
            times.pop_front();
        }
    }

    fn on_tick(&mut self, now: Instant, out: &mut Vec<Action>) {
        match self.state {
            State::Spawning { since } if now.saturating_duration_since(since) > SPAWN_WATCHDOG => {
                // It never asked to lock, so killing it uncovers nothing. The
                // worker keeps the child until it is reaped, queues the next
                // spawn behind it and publishes nothing of the dead child.
                out.push(Action::KillChild);
                self.bump_generation();
                self.give_up(Notice::NotLocked, LogReason::SpawnWatchdog, out);
            }
            State::Requested { since }
                if !self.requested_timed_out
                    && now.saturating_duration_since(since) > REQUESTED_WATCHDOG =>
            {
                // It may be about to lock: leave it. Stop waiting to suspend.
                self.requested_timed_out = true;
                out.push(Action::Log(LogReason::RequestedTimeout));
                self.drop_intent_noticing(out);
                self.release_if_preparing(out);
            }
            State::Ending { since } if now.saturating_duration_since(since) > ENDING_WATCHDOG => {
                // The unlock already happened by protocol, so ending the
                // process uncovers nothing. It is killed through its pidfd,
                // and the worker never publishes anything of the killed child
                // (findings I2 and I-B). The forced exit is the exit of the
                // `Ending` row: a request made meanwhile (a sleep, a verb) is
                // served now (finding M-c), the spawn waiting behind the
                // reaping; with none, the policy is `Idle`.
                out.push(Action::Log(LogReason::EndingTimeout));
                out.push(Action::KillChild);
                self.bump_generation();
                let serve = self.pending_lock || self.intent.is_some();
                self.pending_lock = false;
                if serve {
                    self.spawn(now, Duration::ZERO, out);
                } else {
                    self.settle_idle();
                }
            }
            _ => {}
        }
        if self.intent.is_some_and(|intent| now >= intent.deadline) {
            out.push(Action::Log(LogReason::IntentExpired));
            self.drop_intent_noticing(out);
        }
    }

    /// Drops the pending request to suspend; a person's gets a notice.
    fn drop_intent_noticing(&mut self, out: &mut Vec<Action>) {
        if let Some(intent) = self.intent.take() {
            if intent.origin == Origin::Verb {
                out.push(Action::Notice(Notice::NotConfirmed));
            }
        }
    }

    /// `PrepareForSleep(true)`, or the sleep property read as `true`. The
    /// announcement is acted on whenever no lock child runs, even when a
    /// sleep was already being prepared: a lost `false` followed by the next
    /// real `true` must not let the machine sleep uncovered. Reports that
    /// repeat the same fact are harmless because a running child
    /// (`Spawning`, `Requested`, `Locked`) and an `Ending` one are already
    /// taken care of.
    fn on_sleep_announced(&mut self, now: Instant, out: &mut Vec<Action>) {
        let already = self.sleep == Sleep::Preparing;
        self.sleep = Sleep::Preparing;
        match self.state {
            State::Locked | State::Spawning { .. } | State::Requested { .. } => {}
            State::Idle => {
                // The compositor decides: a lock that is running answers the
                // request with exit 10 and the inhibitor is let go then.
                self.spawn(now, Duration::ZERO, out);
            }
            State::Ending { .. } => self.pending_lock = true,
            State::Unavailable(_) => {
                if !already {
                    out.push(Action::Log(LogReason::SleepWhileUncovered));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn status(exit: LockExit) -> Event {
        Event::Exit(ChildExit::Status(exit))
    }

    fn line(line: StatusLine) -> Event {
        Event::Line(line)
    }

    fn attempt(kind: AttemptKind) -> Event {
        line(StatusLine::Attempt(kind))
    }

    /// The actions without the log lines: what the workers would carry out.
    fn quiet(actions: &[Action]) -> Vec<&Action> {
        actions
            .iter()
            .filter(|action| !matches!(action, Action::Log(_)))
            .collect()
    }

    fn spawn(prior_failures: u8) -> Action {
        Action::Spawn {
            prior_failures,
            after: Duration::ZERO,
        }
    }

    fn spawns(actions: &[Action]) -> Vec<&Action> {
        actions
            .iter()
            .filter(|action| matches!(action, Action::Spawn { .. }))
            .collect()
    }

    fn has_proof(actions: &[Action]) -> bool {
        actions
            .iter()
            .any(|action| matches!(action, Action::Suspend(_)))
    }

    fn notices(actions: &[Action]) -> Vec<Notice> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Notice(notice) => Some(*notice),
                _ => None,
            })
            .collect()
    }

    fn logs(actions: &[Action], reason: LogReason) -> bool {
        actions.contains(&Action::Log(reason))
    }

    /// A policy and the clock that drives it, with a model of the inhibitor
    /// that only the policy's own actions can change. Every event is checked
    /// against the invariants of design §4.4.
    struct Rig {
        policy: LockPolicy,
        now: Instant,
        held: bool,
    }

    impl Rig {
        /// Duty off, nothing pinned: the state a fresh shell starts in.
        fn off() -> Self {
            Self {
                policy: LockPolicy::new(),
                now: Instant::now(),
                held: false,
            }
        }

        /// The duty on and both binaries pinned: `Idle` with the inhibitor
        /// held.
        fn idle() -> Self {
            let mut rig = Self::off();
            rig.send(Event::Duty(true));
            let actions = rig.send(Event::Pinned(true));
            assert!(actions.contains(&Action::HoldInhibitor));
            assert!(actions.contains(&Action::PublishAvailability(true)));
            assert_eq!(rig.policy.state(), State::Idle);
            rig
        }

        /// A lock process asked for and confirmed by a `Lock` verb.
        fn locked() -> Self {
            let mut rig = Self::idle();
            rig.send(Event::Lock);
            rig.send(line(StatusLine::Hello(1)));
            rig.send(line(StatusLine::Requested));
            rig.send(line(StatusLine::Locked));
            assert_eq!(rig.policy.state(), State::Locked);
            rig
        }

        fn send(&mut self, event: Event) -> Vec<Action> {
            if matches!(event, Event::InhibitorFailed(_)) {
                // The worker reports that the inhibitor it was asked for was
                // never taken.
                self.held = false;
            }
            let actions = self.policy.handle(self.now, event);
            for action in &actions {
                match action {
                    Action::HoldInhibitor => {
                        assert!(!self.held, "held twice");
                        assert_eq!(
                            self.policy.sleep(),
                            Sleep::Awake,
                            "an inhibitor was taken while preparing"
                        );
                        self.held = true;
                    }
                    Action::ReleaseInhibitor => {
                        assert!(self.held, "released while not held");
                        self.held = false;
                    }
                    _ => {}
                }
            }
            self.check();
            actions
        }

        fn after(&mut self, delay: Duration, event: Event) -> Vec<Action> {
            self.now += delay;
            self.send(event)
        }

        fn tick(&mut self, delay: Duration) -> Vec<Action> {
            self.after(delay, Event::Tick)
        }

        /// The invariants of design §4.4, whatever just happened.
        fn check(&self) {
            let policy = &self.policy;
            let state = policy.state();
            assert_eq!(self.held, policy.inhibitor() == Inhibitor::Held);
            if matches!(state, State::Locked) {
                assert!(!self.held, "held while locked");
            }
            if self.held {
                assert!(policy.available(), "held while unavailable");
            }
            if policy.sleep() == Sleep::Preparing
                && matches!(state, State::Idle | State::Unavailable(_) | State::Locked)
            {
                assert!(!self.held, "held while preparing with no lock to wait for");
            }
            if policy.sleep() == Sleep::Awake
                && policy.available()
                && !matches!(state, State::Locked)
                && policy.inhibitor() != Inhibitor::Failed
            {
                assert!(self.held, "not held while awake, available and uncovered");
            }
            assert_eq!(
                policy.wants_tick(),
                !matches!(state, State::Idle | State::Unavailable(_)) || policy.intent().is_some()
            );
        }
    }

    // --- The event table of design §4.4, one test per row. ---

    #[test]
    fn a_fresh_policy_is_unavailable_and_asks_for_nothing() {
        let rig = Rig::off();
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
        assert!(!rig.policy.available());
        assert!(!rig.policy.wants_tick());
        rig.check();
    }

    #[test]
    fn t_p1_availability_needs_the_duty_and_both_binaries() {
        let mut rig = Rig::off();
        rig.send(Event::Pinned(true));
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
        let actions = rig.send(Event::Duty(true));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(actions.contains(&Action::PublishAvailability(true)));
        assert!(actions.contains(&Action::HoldInhibitor));
        assert!(logs(&actions, LogReason::DutyOn));

        // A binary that stops passing the checks makes the lock unavailable.
        let actions = rig.send(Event::Pinned(false));
        assert_eq!(rig.policy.state(), State::Unavailable(Why::NotPinned));
        assert!(actions.contains(&Action::PublishAvailability(false)));
        assert!(actions.contains(&Action::ReleaseInhibitor));
        // Repeating what is already true publishes nothing.
        assert!(!rig
            .send(Event::Pinned(false))
            .iter()
            .any(|action| matches!(action, Action::PublishAvailability(_))));
        // The duty off wins as the stated reason.
        rig.send(Event::Duty(false));
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
    }

    #[test]
    fn t_p1_lock_verb_in_idle_spawns() {
        let mut rig = Rig::idle();
        let actions = rig.send(Event::Lock);
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        assert!(rig.policy.intent().is_none());
        assert!(rig.policy.wants_tick());
    }

    #[test]
    fn t_p1_lock_and_suspend_in_idle_spawns_and_sets_the_intent() {
        for origin in [Origin::Idle, Origin::Verb] {
            let mut rig = Rig::idle();
            let actions = rig.send(Event::LockAndSuspend(origin));
            assert_eq!(quiet(&actions), [&spawn(0)], "{origin:?}");
            let intent = rig.policy.intent().expect("an intent");
            assert_eq!(intent.origin, origin);
            assert_eq!(intent.deadline, rig.now + secs(10));
        }
    }

    #[test]
    fn t_p1_lock_and_suspend_while_spawning_or_requested_sets_the_intent() {
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        let actions = rig.send(Event::LockAndSuspend(Origin::Verb));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.intent().map(|i| i.origin), Some(Origin::Verb));

        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        rig.send(line(StatusLine::Requested));
        let actions = rig.send(Event::LockAndSuspend(Origin::Idle));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.intent().map(|i| i.origin), Some(Origin::Idle));
        assert!(matches!(rig.policy.state(), State::Requested { .. }));
    }

    #[test]
    fn t_p1_lock_and_suspend_in_locked_suspends_with_a_proof() {
        for origin in [Origin::Idle, Origin::Verb] {
            let mut rig = Rig::locked();
            let generation = rig.policy.generation();
            let actions = rig.send(Event::LockAndSuspend(origin));
            let proofs: Vec<&Action> = quiet(&actions);
            assert_eq!(proofs.len(), 1, "{origin:?}");
            assert_eq!(proofs[0], &Action::Suspend(LockedProof { generation }));
            assert_eq!(rig.policy.state(), State::Locked);
        }
    }

    #[test]
    fn t_p1_lock_and_suspend_in_ending_sets_the_intent_served_by_the_next_lock() {
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        let actions = rig.send(Event::LockAndSuspend(Origin::Verb));
        assert!(quiet(&actions).is_empty());
        assert!(!has_proof(&actions));
        assert!(rig.policy.intent().is_some());
        // The exit serves the intent with a new lock, which then suspends.
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        rig.send(line(StatusLine::Requested));
        let actions = rig.send(line(StatusLine::Locked));
        assert!(has_proof(&actions));
    }

    #[test]
    fn t_p1_lock_while_locking_or_locked_does_nothing() {
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        for state_line in [None, Some(StatusLine::Requested), Some(StatusLine::Locked)] {
            if let Some(state_line) = state_line {
                rig.send(line(state_line));
            }
            let actions = rig.send(Event::Lock);
            assert!(quiet(&actions).is_empty(), "{:?}", rig.policy.state());
        }
        assert_eq!(rig.policy.state(), State::Locked);
    }

    #[test]
    fn lock_during_ending_serves_one_pending_request() {
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        assert!(matches!(rig.policy.state(), State::Ending { .. }));
        // Asking twice remembers one request.
        assert!(quiet(&rig.send(Event::Lock)).is_empty());
        assert!(quiet(&rig.send(Event::Lock)).is_empty());
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        // The request is spent: the next exit with nothing pending is idle.
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        rig.send(line(StatusLine::Unlocked));
        let actions = rig.send(status(LockExit::Unlocked));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.state(), State::Idle);
    }

    #[test]
    fn t_p1_lock_verbs_in_unavailable_post_a_notice_only_for_the_verb() {
        let mut rig = Rig::off();
        let actions = rig.send(Event::Lock);
        assert_eq!(notices(&actions), [Notice::Unavailable(Why::DutyOff)]);
        assert!(logs(&actions, LogReason::RequestRefused(Why::DutyOff)));
        let actions = rig.send(Event::LockAndSuspend(Origin::Verb));
        assert_eq!(notices(&actions), [Notice::Unavailable(Why::DutyOff)]);
        // Idle already refused its own step: a log, no notice, no spawn.
        let actions = rig.send(Event::LockAndSuspend(Origin::Idle));
        assert!(notices(&actions).is_empty());
        assert!(logs(&actions, LogReason::IdleRequestRefused(Why::DutyOff)));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
    }

    #[test]
    fn t_p1_a_failed_spawn_returns_to_idle_and_drops_the_intent() {
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Verb));
        let actions = rig.send(Event::SpawnFailed);
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(rig.policy.intent().is_none());
        assert_eq!(notices(&actions), [Notice::NotLocked]);
        // A spawn failure outside Spawning is nothing to act on.
        assert!(quiet(&rig.send(Event::SpawnFailed)).is_empty());
        assert_eq!(rig.policy.state(), State::Idle);
    }

    #[test]
    fn t_p1_hello_changes_nothing_and_requested_moves_on() {
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        let before = rig.policy.state();
        assert!(quiet(&rig.send(line(StatusLine::Hello(1)))).is_empty());
        assert_eq!(rig.policy.state(), before);
        // A protocol this shell does not speak is logged, never obeyed.
        let actions = rig.send(line(StatusLine::Hello(2)));
        assert!(logs(&actions, LogReason::UnknownProtocol));
        assert_eq!(rig.policy.state(), before);
        rig.send(line(StatusLine::Requested));
        assert!(matches!(rig.policy.state(), State::Requested { .. }));
    }

    #[test]
    fn t_p1_the_locked_line_releases_the_inhibitor_and_suspends_a_pending_intent() {
        for lines_before in [vec![], vec![StatusLine::Requested]] {
            let mut rig = Rig::idle();
            rig.send(Event::LockAndSuspend(Origin::Verb));
            for before in lines_before {
                rig.send(line(before));
            }
            assert!(rig.held);
            let actions = rig.send(line(StatusLine::Locked));
            assert_eq!(rig.policy.state(), State::Locked);
            assert!(!rig.held);
            // The release comes before the suspend, so logind is not kept
            // waiting for an inhibitor that is about to be dropped.
            let release = actions
                .iter()
                .position(|action| action == &Action::ReleaseInhibitor);
            let suspend = actions
                .iter()
                .position(|action| matches!(action, Action::Suspend(_)));
            assert!(release.is_some() && suspend.is_some() && release < suspend);
            assert!(rig.policy.intent().is_none(), "the intent is spent");
        }
        // Without an intent only the release happens.
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        let actions = rig.send(line(StatusLine::Locked));
        assert_eq!(quiet(&actions), [&Action::ReleaseInhibitor]);
    }

    #[test]
    fn t_p1_an_expired_intent_is_not_served_by_a_late_locked_line() {
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Verb));
        rig.send(line(StatusLine::Requested));
        // The deadline passes between ticks: the locked line arrives after it.
        let actions = rig.after(secs(11), line(StatusLine::Locked));
        assert!(!has_proof(&actions));
        assert!(logs(&actions, LogReason::IntentExpired));
        assert_eq!(notices(&actions), [Notice::NotConfirmed]);
        assert_eq!(rig.policy.state(), State::Locked);
    }

    #[test]
    fn t_p1_attempt_lines_count_only_the_explicit_refusals() {
        let mut rig = Rig::locked();
        for kind in [
            AttemptKind::Unavailable,
            AttemptKind::Timeout,
            AttemptKind::Unsupported,
            AttemptKind::ServiceMissing,
        ] {
            rig.send(attempt(kind));
            assert_eq!(rig.policy.failures(), 0, "{kind:?}");
        }
        rig.send(attempt(AttemptKind::Refused));
        rig.send(attempt(AttemptKind::Account));
        assert_eq!(rig.policy.failures(), 2);
        // Outside Locked an attempt line is not believed.
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        rig.send(attempt(AttemptKind::Refused));
        assert_eq!(rig.policy.failures(), 0);
    }

    #[test]
    fn t_p1_the_unlocked_line_ends_the_lock_and_exit_zero_returns_to_idle() {
        let mut rig = Rig::locked();
        rig.send(attempt(AttemptKind::Refused));
        let generation = rig.policy.generation();
        let actions = rig.send(line(StatusLine::Unlocked));
        assert!(matches!(rig.policy.state(), State::Ending { .. }));
        assert_eq!(rig.policy.failures(), 0);
        assert_eq!(rig.policy.generation(), generation + 1);
        // Awake: the inhibitor is back the moment the session is uncovered.
        assert_eq!(quiet(&actions), [&Action::HoldInhibitor]);
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(quiet(&actions).is_empty());
        assert!(rig.held);
    }

    #[test]
    fn unlocked_while_preparing_does_not_take_an_inhibitor() {
        // Review focus 3: an inhibitor taken after logind announced the sleep
        // would not delay it (Q17: logind refuses it), so none is taken.
        let mut rig = Rig::locked();
        rig.send(Event::PrepareForSleep(true));
        assert_eq!(rig.policy.sleep(), Sleep::Preparing);
        let actions = rig.send(line(StatusLine::Unlocked));
        assert!(quiet(&actions).is_empty());
        assert!(!rig.held);
        // The session was just uncovered with a sleep under way: the exit
        // spawns a lock again at once.
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(!rig.held);
        // When the sleep ends the inhibitor returns unless the lock is up.
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        rig.send(Event::PrepareForSleep(false));
        assert!(!rig.held, "locked again, so no inhibitor");
        rig.send(line(StatusLine::Unlocked));
        assert!(rig.held, "re-held once uncovered and awake");
    }

    #[test]
    fn prepare_for_sleep_in_ending_keeps_the_inhibitor_and_spawns_again() {
        // Review focus 2.
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        assert!(rig.held, "re-held in Ending while awake");
        let actions = rig.send(Event::PrepareForSleep(true));
        assert!(
            quiet(&actions).is_empty(),
            "the inhibitor stays, nothing more"
        );
        assert!(rig.held);
        assert!(!has_proof(&actions));
        // A suspend asked for in Ending is not served from Ending.
        let actions = rig.send(Event::LockAndSuspend(Origin::Verb));
        assert!(!has_proof(&actions));
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(rig.held, "kept until the new lock is confirmed");
        let actions = rig.send(line(StatusLine::Requested));
        assert!(quiet(&actions).is_empty());
        let actions = rig.send(line(StatusLine::Locked));
        assert!(!rig.held);
        assert!(has_proof(&actions), "the intent is served by the new lock");
    }

    #[test]
    fn prepare_for_sleep_in_ending_alone_asks_for_a_lock_when_the_child_exits() {
        // Finding I1: no intent and no pending verb, so only the sleep can
        // be what makes the exit spawn again. A policy that ignored the
        // announcement in `Ending` would settle idle and let the machine
        // sleep uncovered.
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        assert!(rig.policy.intent().is_none());
        rig.send(Event::PrepareForSleep(true));
        let actions = rig.send(status(LockExit::Unlocked));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
    }

    #[test]
    fn a_second_true_with_no_lock_running_asks_for_a_lock_again() {
        // Finding C2: a `false` was lost, the policy still says `Preparing`,
        // and the next real `true` must not be ignored.
        let mut rig = Rig::idle();
        let actions = rig.send(Event::PrepareForSleep(true));
        assert_eq!(spawns(&actions).len(), 1);
        rig.send(Event::SpawnFailed);
        assert_eq!(rig.policy.state(), State::Idle);
        assert_eq!(rig.policy.sleep(), Sleep::Preparing);
        let actions = rig.send(Event::PrepareForSleep(true));
        assert_eq!(spawns(&actions).len(), 1, "{actions:?}");
        // While a child runs, a repeated `true` changes nothing.
        let actions = rig.send(Event::PrepareForSleep(true));
        assert!(quiet(&actions).is_empty(), "{actions:?}");
    }

    #[test]
    fn a_resynchronised_false_ends_a_stuck_preparing() {
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        rig.send(Event::SpawnFailed);
        assert_eq!(rig.policy.sleep(), Sleep::Preparing);
        assert!(!rig.held);
        // The worker read the property: the sleep is over.
        let actions = rig.send(Event::PrepareForSleep(false));
        assert_eq!(rig.policy.sleep(), Sleep::Awake);
        assert!(actions.contains(&Action::HoldInhibitor), "{actions:?}");
    }

    #[test]
    fn a_false_that_finds_the_policy_awake_does_not_retry_a_failed_inhibitor() {
        // The worker reports the property on every request; a `false` that
        // changes nothing must not turn a failed inhibitor into another
        // request, or request and report would feed each other.
        let mut rig = Rig::idle();
        rig.send(Event::InhibitorFailed(InhibitorFailure::Bus));
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Failed);
        let actions = rig.send(Event::PrepareForSleep(false));
        assert!(quiet(&actions).is_empty(), "{actions:?}");
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Failed);
    }

    #[test]
    fn the_sleep_bus_coming_back_asks_for_a_failed_inhibitor_again() {
        // Finding I4.
        let mut rig = Rig::idle();
        rig.send(Event::InhibitorFailed(InhibitorFailure::Bus));
        assert!(!rig.held);
        let actions = rig.send(Event::SleepBusBack);
        assert!(actions.contains(&Action::HoldInhibitor), "{actions:?}");
        assert!(rig.held);
        // With nothing failed it changes nothing.
        assert!(quiet(&rig.send(Event::SleepBusBack)).is_empty());
    }

    #[test]
    fn turning_the_duty_off_forgets_a_sleep_that_was_being_prepared() {
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        assert_eq!(rig.policy.sleep(), Sleep::Preparing);
        rig.send(Event::Duty(false));
        assert_eq!(rig.policy.sleep(), Sleep::Awake);
        // And on again: the next announcement is acted on.
        rig.send(Event::Duty(true));
        let actions = rig.send(Event::PrepareForSleep(true));
        assert_eq!(spawns(&actions).len(), 1);
    }

    #[test]
    fn a_relaunch_that_cannot_start_tells_the_person() {
        // Finding M1: the lock died after it asked to lock; the replacement
        // does not start. Nobody had asked with a verb, yet the session may
        // be uncovered, so the notice is posted.
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        let actions = rig.send(Event::Exit(ChildExit::Died));
        assert_eq!(spawns(&actions).len(), 1);
        let actions = rig.send(Event::SpawnFailed);
        assert_eq!(notices(&actions), [Notice::NotLocked], "{actions:?}");
    }

    // --- Exits. ---

    #[test]
    fn t_p1_exit_10_retries_after_half_a_second_then_goes_idle() {
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        for retry in 1..=2 {
            let actions = rig.send(status(LockExit::FinishedBeforeLocked));
            assert_eq!(
                quiet(&actions),
                [&Action::Spawn {
                    prior_failures: 0,
                    after: Duration::from_millis(500)
                }],
                "retry {retry}"
            );
            assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        }
        let actions = rig.send(status(LockExit::FinishedBeforeLocked));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(logs(&actions, LogReason::CompositorRefused));
        // The author asked for this lock, so the author is told.
        assert_eq!(notices(&actions), [Notice::CompositorRefused]);
        assert!(quiet(&actions)
            .iter()
            .all(|a| matches!(a, Action::Notice(_))));
    }

    #[test]
    fn t_p9_exit_10_retries_twice_never_suspends_and_releases_the_inhibitor_when_preparing() {
        let mut rig = Rig::idle();
        // The system is going to sleep and the shell has no lock running.
        let actions = rig.send(Event::PrepareForSleep(true));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(rig.held, "kept while a lock is being asked for");
        // Another client holds the session lock: the compositor refuses.
        let mut spawns = 0;
        for _ in 0..3 {
            let actions = rig.send(status(LockExit::FinishedBeforeLocked));
            assert!(!has_proof(&actions));
            spawns += actions
                .iter()
                .filter(|action| matches!(action, Action::Spawn { .. }))
                .count();
        }
        assert_eq!(spawns, 2, "two retries, then it stops");
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(!rig.held, "released after the last retry so logind goes on");
        // No notice for a refusal nobody asked for.
        rig.send(Event::PrepareForSleep(false));
        assert!(rig.held, "re-taken when the sleep is over");
    }

    #[test]
    fn t_p9_the_retry_window_is_sixty_seconds() {
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        rig.send(status(LockExit::FinishedBeforeLocked));
        rig.send(status(LockExit::FinishedBeforeLocked));
        // The two retries age out; the third refusal is retried again.
        let actions = rig.after(secs(61), status(LockExit::FinishedBeforeLocked));
        assert!(actions
            .iter()
            .any(|action| matches!(action, Action::Spawn { .. })));
    }

    #[test]
    fn t_p1_exits_that_never_requested_a_lock_return_to_idle() {
        for exit in [
            LockExit::EarlyError,
            LockExit::Usage,
            LockExit::NoLockManager,
            LockExit::NoConnection,
            LockExit::SetupFailed,
        ] {
            let mut rig = Rig::idle();
            rig.send(Event::LockAndSuspend(Origin::Verb));
            let actions = rig.send(status(exit));
            assert_eq!(rig.policy.state(), State::Idle, "{exit:?}");
            assert!(rig.policy.intent().is_none(), "{exit:?}");
            assert_eq!(notices(&actions), [Notice::NotLocked], "{exit:?}");
            assert!(!actions
                .iter()
                .any(|action| matches!(action, Action::Spawn { .. })));
        }
        // Idle-origin: the same state, no notice.
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Idle));
        let actions = rig.send(status(LockExit::NoLockManager));
        assert!(notices(&actions).is_empty());
        assert_eq!(rig.policy.state(), State::Idle);
        // A child that dies before it asked for the lock is the same failure.
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        rig.send(Event::Exit(ChildExit::Died));
        assert_eq!(rig.policy.state(), State::Idle);
    }

    #[test]
    fn exit_16_never_relaunches() {
        for stage in 0..4 {
            let mut rig = Rig::idle();
            rig.send(Event::LockAndSuspend(Origin::Verb));
            if stage >= 1 {
                rig.send(line(StatusLine::Requested));
            }
            if stage >= 2 {
                rig.send(line(StatusLine::Locked));
            }
            if stage >= 3 {
                rig.send(line(StatusLine::Unlocked));
            }
            let actions = rig.send(status(LockExit::ConnectionLost));
            assert_eq!(rig.policy.state(), State::Idle, "stage {stage}");
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, Action::Spawn { .. })),
                "stage {stage}"
            );
            assert!(logs(&actions, LogReason::ConnectionLost), "stage {stage}");
            assert!(rig.policy.intent().is_none());
        }
    }

    #[test]
    fn exit_13_never_relaunches() {
        let mut rig = Rig::locked();
        let actions = rig.send(status(LockExit::FinishedAfterLocked));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(!actions
            .iter()
            .any(|action| matches!(action, Action::Spawn { .. })));
        assert!(logs(&actions, LogReason::FinishedAfterLocked));
        // Uncovered and awake: the inhibitor is back.
        assert!(rig.held);
        // Asleep: it is not taken again.
        let mut rig = Rig::locked();
        rig.send(Event::PrepareForSleep(true));
        rig.send(status(LockExit::FinishedAfterLocked));
        assert!(!rig.held);
    }

    #[test]
    fn a_death_after_requested_relaunches_with_the_carried_failures() {
        for locked in [false, true] {
            let mut rig = Rig::idle();
            rig.send(Event::Lock);
            rig.send(line(StatusLine::Requested));
            if locked {
                rig.send(line(StatusLine::Locked));
            }
            let actions = rig.send(Event::Exit(ChildExit::Died));
            assert_eq!(spawns(&actions), [&spawn(0)], "locked: {locked}");
            assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        }
        // An exit status the table does not name is a death too, and so is a
        // zero that was never announced by an `unlocked` line.
        for exit in [
            ChildExit::Status(LockExit::Unlocked),
            ChildExit::Status(LockExit::EarlyError),
            ChildExit::Status(LockExit::SetupFailed),
            ChildExit::Status(LockExit::NoLockManager),
        ] {
            let mut rig = Rig::locked();
            let actions = rig.send(Event::Exit(exit));
            assert_eq!(spawns(&actions), [&spawn(0)], "{exit:?}");
        }
    }

    #[test]
    fn t_p4_relaunch_bound_three_per_sixty_seconds() {
        fn die_and_confirm(rig: &mut Rig, delay: Duration) -> Vec<Action> {
            let actions = rig.after(delay, Event::Exit(ChildExit::Died));
            if matches!(rig.policy.state(), State::Spawning { .. }) {
                rig.send(line(StatusLine::Requested));
                rig.send(line(StatusLine::Locked));
            }
            actions
        }
        let mut rig = Rig::locked();
        for n in 1..=3 {
            let actions = die_and_confirm(&mut rig, secs(10));
            assert!(
                actions
                    .iter()
                    .any(|action| matches!(action, Action::Spawn { .. })),
                "relaunch {n}"
            );
        }
        // The fourth death within sixty seconds of the first stops it.
        let actions = die_and_confirm(&mut rig, secs(10));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(!actions
            .iter()
            .any(|action| matches!(action, Action::Spawn { .. })));
        assert_eq!(notices(&actions), [Notice::RelaunchLimit]);
        assert!(logs(&actions, LogReason::RelaunchLimit));
        assert!(rig.held, "uncovered and awake again");

        // Once the first relaunches are older than sixty seconds they stop
        // counting.
        let mut rig = Rig::locked();
        for _ in 0..3 {
            die_and_confirm(&mut rig, secs(10));
        }
        let actions = die_and_confirm(&mut rig, secs(45));
        assert!(actions
            .iter()
            .any(|action| matches!(action, Action::Spawn { .. })));
    }

    #[test]
    fn t_p5_failures_carry_over_to_the_relaunch() {
        let mut rig = Rig::locked();
        rig.send(attempt(AttemptKind::Refused));
        rig.send(attempt(AttemptKind::Timeout));
        rig.send(attempt(AttemptKind::Refused));
        rig.send(attempt(AttemptKind::Account));
        assert_eq!(rig.policy.failures(), 3);
        let actions = rig.send(Event::Exit(ChildExit::Died));
        assert_eq!(spawns(&actions), [&spawn(3)]);
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        rig.send(attempt(AttemptKind::Refused));
        let actions = rig.send(Event::Exit(ChildExit::Died));
        assert_eq!(spawns(&actions), [&spawn(4)]);
        // An unlock forgets them.
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        rig.send(line(StatusLine::Unlocked));
        rig.send(status(LockExit::Unlocked));
        let actions = rig.send(Event::Lock);
        assert_eq!(spawns(&actions), [&spawn(0)]);
        // The count saturates instead of wrapping.
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        for _ in 0..300 {
            rig.send(attempt(AttemptKind::Refused));
        }
        assert_eq!(rig.policy.failures(), u8::MAX);
    }

    // --- Watchdogs and intents. ---

    #[test]
    fn watchdog_spawning_5s_requested_10s_ending_5s() {
        // Spawning: killed through the pidfd, because it never asked to lock.
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Verb));
        assert!(
            quiet(&rig.tick(secs(5))).is_empty(),
            "5 s is not older than 5 s"
        );
        let actions = rig.tick(SECOND);
        assert!(actions.contains(&Action::KillChild));
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(rig.policy.intent().is_none());
        assert_eq!(notices(&actions), [Notice::NotLocked]);

        // Requested: never killed (it may be about to lock). The intent goes
        // once, the state stays, an error is logged once. The intent is asked
        // for late, so its own deadline does not come first.
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        rig.send(line(StatusLine::Requested));
        rig.after(secs(9), Event::LockAndSuspend(Origin::Verb));
        assert!(
            quiet(&rig.tick(SECOND)).is_empty(),
            "10 s is not older than 10 s"
        );
        let actions = rig.tick(SECOND);
        assert!(!actions.contains(&Action::KillChild));
        assert!(logs(&actions, LogReason::RequestedTimeout));
        assert_eq!(notices(&actions), [Notice::NotConfirmed]);
        assert!(rig.policy.intent().is_none());
        assert!(matches!(rig.policy.state(), State::Requested { .. }));
        let actions = rig.tick(SECOND);
        assert!(!logs(&actions, LogReason::RequestedTimeout), "once");
        // If it does lock after all, it is locked, and nothing suspends.
        let actions = rig.send(line(StatusLine::Locked));
        assert!(!has_proof(&actions));
        assert_eq!(rig.policy.state(), State::Locked);

        // Ending: the unlock already happened by protocol.
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        assert!(quiet(&rig.tick(secs(5))).is_empty());
        let actions = rig.tick(SECOND);
        assert!(logs(&actions, LogReason::EndingTimeout));
        assert!(
            actions.contains(&Action::KillChild),
            "the policy is idle from here on, so the child is ended (I2)"
        );
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(rig.held);
    }

    #[test]
    fn the_ending_watchdog_serves_a_pending_request_instead_of_dropping_it() {
        // Finding M-c: a request made while `Ending` is served by the exit;
        // the watchdog forces that exit, so it serves it too (design §4.4,
        // the row of `exit 0 after unlocked`). The worker starts the spawn
        // after the killed child is reaped (I2).
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        rig.send(Event::Lock);
        let actions = rig.tick(secs(6));
        assert!(logs(&actions, LogReason::EndingTimeout));
        assert!(actions.contains(&Action::KillChild));
        assert_eq!(spawns(&actions), [&spawn(0)], "{actions:?}");
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        // And with nothing pending it settles idle as before.
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        let actions = rig.tick(secs(6));
        assert!(spawns(&actions).is_empty(), "{actions:?}");
        assert_eq!(rig.policy.state(), State::Idle);
    }

    #[test]
    fn the_natural_exit_of_the_old_child_just_after_the_ending_watchdog_is_not_the_new_ones() {
        // Finding m-1: the old child in `Ending` leaves by itself just as the
        // watchdog fires. The worker publishes its `Exit(0)` before it reads
        // `KillChild`, by which time the policy is already `Spawning` (M-c
        // served the pending request). A child that has not said `requested`
        // cannot exit 0, so that exit is the old child's: ignored, never a
        // give-up that would drop the lock and the sleep inhibitor.
        let mut rig = Rig::locked();
        rig.send(line(StatusLine::Unlocked));
        rig.send(Event::PrepareForSleep(true));
        assert!(rig.held);
        let actions = rig.tick(secs(6));
        assert!(actions.contains(&Action::KillChild));
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        let actions = rig.send(status(LockExit::Unlocked));
        assert!(logs(&actions, LogReason::StrayExit), "{actions:?}");
        assert!(quiet(&actions).is_empty(), "{actions:?}");
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        assert!(rig.held, "the sleep is still waiting for the lock");
        // The new child goes on and locks.
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        assert_eq!(rig.policy.state(), State::Locked);
    }

    #[test]
    fn a_requested_timeout_while_preparing_releases_the_inhibitor() {
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        rig.send(line(StatusLine::Requested));
        assert!(rig.held);
        let actions = rig.tick(secs(11));
        assert!(actions.contains(&Action::ReleaseInhibitor));
        assert!(!rig.held);
        assert!(matches!(rig.policy.state(), State::Requested { .. }));
    }

    #[test]
    fn t_p1_the_intent_deadline_drops_it_with_a_notice_for_a_verb_only() {
        for (origin, noticed) in [(Origin::Verb, true), (Origin::Idle, false)] {
            let mut rig = Rig::idle();
            // The lock is confirmed late: nothing in the state machine ages
            // before the deadline, so the deadline does it.
            rig.send(Event::LockAndSuspend(origin));
            rig.send(line(StatusLine::Requested));
            assert!(rig.policy.intent().is_some());
            let actions = rig.tick(secs(10));
            assert!(rig.policy.intent().is_none(), "{origin:?}");
            assert!(logs(&actions, LogReason::IntentExpired), "{origin:?}");
            assert_eq!(notices(&actions).len(), usize::from(noticed), "{origin:?}");
        }
    }

    #[test]
    fn t_p7_idle_intent_dropped_by_resumed_verb_intent_not() {
        // Someone is back: what idle asked for is cancelled.
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Idle));
        let actions = rig.send(Event::IdleResumed);
        assert!(logs(&actions, LogReason::IntentCancelledByInput));
        assert!(rig.policy.intent().is_none());
        rig.send(line(StatusLine::Requested));
        assert!(!has_proof(&rig.send(line(StatusLine::Locked))));

        // Pressing the binding is input too, and must not cancel itself.
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Verb));
        rig.send(Event::IdleResumed);
        assert!(rig.policy.intent().is_some());
        rig.send(line(StatusLine::Requested));
        assert!(has_proof(&rig.send(line(StatusLine::Locked))));

        // One intent at a time, the verb's winning over idle's in both
        // orders.
        for orders in [
            [Origin::Idle, Origin::Verb],
            [Origin::Verb, Origin::Idle],
            [Origin::Verb, Origin::Verb],
        ] {
            let mut rig = Rig::idle();
            for origin in orders {
                rig.send(Event::LockAndSuspend(origin));
            }
            rig.send(Event::IdleResumed);
            assert!(rig.policy.intent().is_some(), "{orders:?}");
            rig.send(line(StatusLine::Requested));
            let actions = rig.send(line(StatusLine::Locked));
            let suspends = actions
                .iter()
                .filter(|action| matches!(action, Action::Suspend(_)))
                .count();
            assert_eq!(suspends, 1, "{orders:?}");
        }
        // Resumed with nothing pending is nothing.
        let mut rig = Rig::idle();
        assert!(quiet(&rig.send(Event::IdleResumed)).is_empty());
    }

    // --- Sleep. ---

    #[test]
    fn t_p1_prepare_for_sleep_true_in_every_state() {
        // Locked: nothing to do; the inhibitor is already released.
        let mut rig = Rig::locked();
        assert!(quiet(&rig.send(Event::PrepareForSleep(true))).is_empty());
        assert_eq!(rig.policy.sleep(), Sleep::Preparing);
        assert_eq!(rig.policy.state(), State::Locked);

        // Idle: ask the compositor; the inhibitor stays for now.
        let mut rig = Rig::idle();
        let actions = rig.send(Event::PrepareForSleep(true));
        assert_eq!(quiet(&actions), [&spawn(0)]);
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
        assert!(rig.held);

        // Spawning and Requested: keep the inhibitor until locked or failure.
        for requested in [false, true] {
            let mut rig = Rig::idle();
            rig.send(Event::Lock);
            if requested {
                rig.send(line(StatusLine::Requested));
            }
            let before = rig.policy.state();
            assert!(quiet(&rig.send(Event::PrepareForSleep(true))).is_empty());
            assert_eq!(rig.policy.state(), before);
            assert!(rig.held);
            let actions = rig.send(line(StatusLine::Locked));
            assert_eq!(quiet(&actions), [&Action::ReleaseInhibitor]);
        }

        // Unavailable: the session sleeps uncovered, and that is logged.
        let mut rig = Rig::off();
        let actions = rig.send(Event::PrepareForSleep(true));
        assert!(logs(&actions, LogReason::SleepWhileUncovered));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));

        // A second announcement of the same sleep changes nothing.
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        assert!(quiet(&rig.send(Event::PrepareForSleep(true))).is_empty());
    }

    #[test]
    fn t_p1_prepare_for_sleep_false_holds_the_inhibitor_unless_locked() {
        let mut rig = Rig::locked();
        rig.send(Event::PrepareForSleep(true));
        let actions = rig.send(Event::PrepareForSleep(false));
        assert!(quiet(&actions).is_empty());
        assert_eq!(rig.policy.sleep(), Sleep::Awake);
        assert!(!rig.held);

        // The sleep went ahead before the lock was confirmed: a warning, and
        // the inhibitor comes back.
        let mut rig = Rig::idle();
        rig.send(Event::PrepareForSleep(true));
        rig.send(status(LockExit::FinishedBeforeLocked));
        rig.send(status(LockExit::FinishedBeforeLocked));
        rig.send(status(LockExit::FinishedBeforeLocked));
        assert!(!rig.held);
        let actions = rig.send(Event::PrepareForSleep(false));
        assert!(logs(&actions, LogReason::SleptBeforeLocked));
        assert!(actions.contains(&Action::HoldInhibitor));
    }

    #[test]
    fn a_failed_inhibitor_is_retried_only_when_something_changes() {
        let mut rig = Rig::idle();
        // The bus refused: not held, and not asked for again at once.
        let actions = rig.send(Event::InhibitorFailed(InhibitorFailure::Bus));
        assert!(logs(
            &actions,
            LogReason::InhibitorFailed(InhibitorFailure::Bus)
        ));
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Failed);
        assert!(!rig.held);
        assert!(quiet(&rig.send(Event::IdleResumed)).is_empty());
        assert!(quiet(&rig.tick(secs(30))).is_empty());
        assert!(quiet(&rig.send(Event::Lock)).len() == 1, "only the spawn");
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Failed);
        // A sleep that ends, a lock that ends, the duty toggled: each is a
        // reason to try again.
        rig.send(Event::PrepareForSleep(true));
        let actions = rig.send(Event::PrepareForSleep(false));
        assert!(actions.contains(&Action::HoldInhibitor));
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Held);

        let mut rig = Rig::idle();
        rig.send(Event::InhibitorFailed(InhibitorFailure::Bus));
        rig.send(Event::Duty(false));
        let actions = rig.send(Event::Duty(true));
        assert!(actions.contains(&Action::HoldInhibitor));

        // A report about an inhibitor the policy no longer wants changes
        // nothing.
        let mut rig = Rig::locked();
        rig.send(Event::InhibitorFailed(InhibitorFailure::Bus));
        assert_eq!(rig.policy.inhibitor(), Inhibitor::Released);
    }

    #[test]
    fn a_sleep_in_progress_is_the_expected_answer_to_a_late_inhibit() {
        // Q17: logind refuses a delay inhibitor requested while a sleep is
        // pending with OperationInProgress. The race: the lock ends, the
        // policy asks for an inhibitor, and logind had already started the
        // sleep.
        let mut rig = Rig::locked();
        let actions = rig.send(line(StatusLine::Unlocked));
        assert!(actions.contains(&Action::HoldInhibitor));
        let actions = rig.send(Event::InhibitorFailed(InhibitorFailure::SleepInProgress));
        assert!(logs(
            &actions,
            LogReason::InhibitorFailed(InhibitorFailure::SleepInProgress)
        ));
        assert!(quiet(&actions).is_empty(), "no retry while the sleep runs");
        assert_eq!(
            LogReason::InhibitorFailed(InhibitorFailure::SleepInProgress).level(),
            Level::Info
        );
        assert_eq!(
            LogReason::InhibitorFailed(InhibitorFailure::Bus).level(),
            Level::Warn
        );
        // The announcement follows, and when the sleep ends the policy asks
        // again.
        rig.send(Event::PrepareForSleep(true));
        let actions = rig.send(Event::PrepareForSleep(false));
        assert!(actions.contains(&Action::HoldInhibitor));
    }

    // --- Duty off, PAM file, notices. ---

    #[test]
    fn duty_off_does_not_touch_the_running_lock() {
        let mut rig = Rig::locked();
        let actions = rig.send(Event::Duty(false));
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
        // Nothing that would end, kill or replace the child.
        assert!(!actions.contains(&Action::KillChild));
        assert!(!actions
            .iter()
            .any(|action| matches!(action, Action::Spawn { .. } | Action::Suspend(_))));
        // Its later lines and exit are not believed (no policy state to act
        // on), and a new request is refused with a notice.
        assert!(quiet(&rig.send(line(StatusLine::Unlocked))).is_empty());
        assert!(quiet(&rig.send(status(LockExit::Unlocked))).is_empty());
        assert_eq!(rig.policy.state(), State::Unavailable(Why::DutyOff));
        // Duty off in any state releases the inhibitor.
        let mut rig = Rig::idle();
        let actions = rig.send(Event::Duty(false));
        assert!(actions.contains(&Action::ReleaseInhibitor));
        assert!(actions.contains(&Action::PublishAvailability(false)));
        let mut rig = Rig::idle();
        rig.send(Event::LockAndSuspend(Origin::Verb));
        rig.send(Event::Duty(false));
        assert!(rig.policy.intent().is_none());
        assert!(!rig.policy.wants_tick());
    }

    #[test]
    fn proof_generation_bumps_on_unlocked_exit_and_duty_off() {
        let reader_of = |rig: &Rig| rig.policy.generation_reader();

        // A proof is current only until the lock changes.
        let mut rig = Rig::locked();
        let reader = reader_of(&rig);
        let proof = rig.policy.mint().expect("locked");
        assert!(reader.is_current(&proof));

        rig.send(line(StatusLine::Unlocked));
        assert!(!reader.is_current(&proof), "unlocked line");

        let mut rig = Rig::locked();
        let reader = reader_of(&rig);
        let proof = rig.policy.mint().expect("locked");
        rig.send(Event::Exit(ChildExit::Died));
        assert!(!reader.is_current(&proof), "child exit");

        let mut rig = Rig::locked();
        let reader = reader_of(&rig);
        let proof = rig.policy.mint().expect("locked");
        rig.send(Event::Duty(false));
        assert!(!reader.is_current(&proof), "duty off");

        let mut rig = Rig::locked();
        let reader = reader_of(&rig);
        let proof = rig.policy.mint().expect("locked");
        rig.send(Event::Pinned(false));
        assert!(!reader.is_current(&proof), "a binary no longer pinned");

        // A proof minted before cannot become current again.
        let mut rig = Rig::locked();
        let reader = reader_of(&rig);
        let old = rig.policy.mint().expect("locked");
        rig.send(Event::Exit(ChildExit::Died));
        rig.send(line(StatusLine::Requested));
        rig.send(line(StatusLine::Locked));
        let new = rig.policy.mint().expect("locked again");
        assert!(!reader.is_current(&old));
        assert!(reader.is_current(&new));
        assert_ne!(old.generation(), new.generation());

        // The reader is the policy's shared cell.
        assert_eq!(reader.current(), rig.policy.generation());
    }

    #[test]
    fn t_p6_a_missing_pam_file_does_not_change_availability() {
        let mut rig = Rig::idle();
        let actions = rig.send(Event::PamFileMissing);
        assert_eq!(notices(&actions), [Notice::PamMissing]);
        assert!(logs(&actions, LogReason::PamFileMissing));
        assert!(rig.policy.available());
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(!actions
            .iter()
            .any(|action| matches!(action, Action::PublishAvailability(_))));
        assert!(rig.held);
        // The lock still covers: the request is served as usual.
        assert_eq!(quiet(&rig.send(Event::Lock)).len(), 1);
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
    }

    #[test]
    fn notice_exactly_for_the_explicit_refusals_of_section_4_9() {
        // Explicit requests that end in a refusal are noticed; the same
        // endings for idle or sleep requests only reach the log.
        struct Case {
            name: &'static str,
            start: Event,
            end: fn(&mut Rig) -> Vec<Action>,
            notice: Option<Notice>,
        }
        fn exit(exit: LockExit) -> Event {
            status(exit)
        }
        let cases = [
            Case {
                name: "lock verb, spawn exits 1",
                start: Event::Lock,
                end: |rig| rig.send(exit(LockExit::EarlyError)),
                notice: Some(Notice::NotLocked),
            },
            Case {
                name: "lock suspend verb, spawn exits 11",
                start: Event::LockAndSuspend(Origin::Verb),
                end: |rig| rig.send(exit(LockExit::NoLockManager)),
                notice: Some(Notice::NotLocked),
            },
            Case {
                name: "idle, spawn exits 11",
                start: Event::LockAndSuspend(Origin::Idle),
                end: |rig| rig.send(exit(LockExit::NoLockManager)),
                notice: None,
            },
            Case {
                name: "sleep, spawn exits 15",
                start: Event::PrepareForSleep(true),
                end: |rig| rig.send(exit(LockExit::SetupFailed)),
                notice: None,
            },
            Case {
                name: "lock verb, spawn fails",
                start: Event::Lock,
                end: |rig| rig.send(Event::SpawnFailed),
                notice: Some(Notice::NotLocked),
            },
            Case {
                name: "idle, spawn fails",
                start: Event::LockAndSuspend(Origin::Idle),
                end: |rig| rig.send(Event::SpawnFailed),
                notice: None,
            },
            Case {
                name: "lock verb, the compositor refuses",
                start: Event::Lock,
                end: |rig| {
                    let mut all = Vec::new();
                    for _ in 0..3 {
                        all.extend(rig.send(exit(LockExit::FinishedBeforeLocked)));
                    }
                    all
                },
                notice: Some(Notice::CompositorRefused),
            },
            Case {
                name: "idle, the compositor refuses",
                start: Event::LockAndSuspend(Origin::Idle),
                end: |rig| {
                    let mut all = Vec::new();
                    for _ in 0..3 {
                        all.extend(rig.send(exit(LockExit::FinishedBeforeLocked)));
                    }
                    all
                },
                notice: None,
            },
            Case {
                name: "lock suspend verb, never confirmed",
                start: Event::LockAndSuspend(Origin::Verb),
                end: |rig| {
                    rig.send(line(StatusLine::Requested));
                    rig.tick(secs(11))
                },
                notice: Some(Notice::NotConfirmed),
            },
            Case {
                name: "idle, never confirmed",
                start: Event::LockAndSuspend(Origin::Idle),
                end: |rig| {
                    rig.send(line(StatusLine::Requested));
                    rig.tick(secs(11))
                },
                notice: None,
            },
            Case {
                name: "someone is back",
                start: Event::LockAndSuspend(Origin::Idle),
                end: |rig| rig.send(Event::IdleResumed),
                notice: None,
            },
        ];
        for case in cases {
            let mut rig = Rig::idle();
            let mut actions = rig.send(case.start);
            actions.extend((case.end)(&mut rig));
            let found = notices(&actions);
            assert_eq!(
                found,
                case.notice.into_iter().collect::<Vec<_>>(),
                "{}",
                case.name
            );
        }
    }

    // --- What the policy never believes. ---

    #[test]
    fn t_p3_proof_only_in_locked() {
        // From every state but Locked, asking to suspend yields no proof.
        type Build = fn() -> Rig;
        let builders: [(&str, Build); 5] = [
            ("unavailable", Rig::off),
            ("idle", Rig::idle),
            ("spawning", || {
                let mut rig = Rig::idle();
                rig.send(Event::Lock);
                rig
            }),
            ("requested", || {
                let mut rig = Rig::idle();
                rig.send(Event::Lock);
                rig.send(line(StatusLine::Requested));
                rig
            }),
            ("ending", || {
                let mut rig = Rig::locked();
                rig.send(line(StatusLine::Unlocked));
                rig
            }),
        ];
        for (name, build) in builders {
            let mut rig = build();
            assert!(rig.policy.mint().is_none(), "{name}");
            for origin in [Origin::Idle, Origin::Verb] {
                let actions = rig.send(Event::LockAndSuspend(origin));
                assert!(!has_proof(&actions), "{name} {origin:?}");
            }
        }
        // In Locked there is one, with the current generation.
        let rig = Rig::locked();
        let proof = rig.policy.mint().expect("a proof in Locked");
        assert_eq!(proof.generation(), rig.policy.generation());
    }

    #[test]
    fn t_p3_a_proof_is_minted_only_by_a_locked_line_or_a_request_in_locked() {
        let mut rng = Xorshift(0xDEAD_BEEF_CAFE_F00D);
        for _ in 0..200 {
            let mut rig = Rig::off();
            for _ in 0..80 {
                let event = random_event(&mut rng, &mut rig);
                let was_locked = matches!(rig.policy.state(), State::Locked);
                let actions = rig.send(event.clone());
                if has_proof(&actions) {
                    assert_eq!(rig.policy.state(), State::Locked, "{event:?}");
                    let fresh_lock = matches!(event, Event::Line(StatusLine::Locked));
                    let asked = matches!(event, Event::LockAndSuspend(_));
                    assert!(
                        (asked && was_locked) || fresh_lock,
                        "a proof from {event:?}"
                    );
                    for action in &actions {
                        if let Action::Suspend(proof) = action {
                            assert_eq!(proof.generation(), rig.policy.generation());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn t_p8_no_event_or_input_derives_from_a_file_or_a_pid() {
        // The policy's module imports no filesystem, process or path type,
        // and no event carries a path or a process id, so nothing a file or a
        // pid says can reach it.
        let source = include_str!("lock.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the production part");
        let code: String = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "std::fs",
            "std::process",
            "std::path",
            "std::os",
            "std::env",
            "std::io",
            "PathBuf",
            "Path::",
            "&Path",
            "File::",
            ": File",
            "Pid",
            "pid",
            "/proc",
            "/run",
            "unsafe",
            "unwrap()",
            "expect(",
            "panic!",
        ] {
            assert!(!code.contains(forbidden), "{forbidden} in the policy");
        }
    }

    #[test]
    fn t_x1_stale_foreign_and_other_display_files_leave_the_policy_idle_without_a_proof() {
        // A shell restarted under a running lock, a stale state file or the
        // state file of another display are all invisible to the policy: it
        // starts Idle and can only learn a lock from its own child's lines.
        let rig = Rig::idle();
        assert_eq!(rig.policy.state(), State::Idle);
        assert!(rig.policy.mint().is_none());

        // The lock that is already running holds the session: this shell's
        // lock request is refused by the compositor (exit 10) and neither
        // suspends nor mints anything, however it is asked.
        for request in [
            Event::Lock,
            Event::LockAndSuspend(Origin::Verb),
            Event::LockAndSuspend(Origin::Idle),
            Event::PrepareForSleep(true),
        ] {
            let mut rig = Rig::idle();
            let mut all = rig.send(request.clone());
            for _ in 0..3 {
                all.extend(rig.send(status(LockExit::FinishedBeforeLocked)));
            }
            assert!(!has_proof(&all), "{request:?}");
            assert_eq!(rig.policy.state(), State::Idle, "{request:?}");
            assert!(rig.policy.mint().is_none(), "{request:?}");
            all.extend(rig.send(Event::LockAndSuspend(Origin::Verb)));
            assert!(!has_proof(&all), "{request:?}");
        }

        // Lines and exits of a child this shell does not have mint nothing.
        let mut rig = Rig::idle();
        for event in [
            line(StatusLine::Hello(1)),
            line(StatusLine::Requested),
            line(StatusLine::Locked),
            attempt(AttemptKind::Refused),
            line(StatusLine::Unlocked),
            status(LockExit::Unlocked),
            Event::Exit(ChildExit::Died),
        ] {
            let actions = rig.send(event.clone());
            assert!(!has_proof(&actions), "{event:?}");
            assert_eq!(rig.policy.state(), State::Idle, "{event:?}");
        }
        assert!(rig.policy.mint().is_none());
    }

    #[test]
    fn unexpected_lines_are_logged_and_change_nothing() {
        let mut rig = Rig::locked();
        for event in [
            line(StatusLine::Locked),
            line(StatusLine::Requested),
            line(StatusLine::Hello(1)),
        ] {
            let before = rig.policy.state();
            rig.send(event);
            assert_eq!(rig.policy.state(), before);
        }
        let mut rig = Rig::idle();
        rig.send(Event::Lock);
        // `unlocked` without `locked` is not believed.
        let actions = rig.send(line(StatusLine::Unlocked));
        assert!(logs(&actions, LogReason::UnexpectedLine));
        assert!(matches!(rig.policy.state(), State::Spawning { .. }));
    }

    // --- Random sequences. ---

    /// A seeded xorshift: noise for the properties, not entropy.
    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    /// The next event of a random run; time advances by itself on ticks.
    fn random_event(rng: &mut Xorshift, rig: &mut Rig) -> Event {
        const EXITS: [LockExit; 9] = [
            LockExit::Unlocked,
            LockExit::EarlyError,
            LockExit::Usage,
            LockExit::FinishedBeforeLocked,
            LockExit::NoLockManager,
            LockExit::NoConnection,
            LockExit::FinishedAfterLocked,
            LockExit::SetupFailed,
            LockExit::ConnectionLost,
        ];
        const KINDS: [AttemptKind; 6] = [
            AttemptKind::Refused,
            AttemptKind::Account,
            AttemptKind::Unavailable,
            AttemptKind::Timeout,
            AttemptKind::Unsupported,
            AttemptKind::ServiceMissing,
        ];
        let origin = if rng.below(2) == 0 {
            Origin::Idle
        } else {
            Origin::Verb
        };
        match rng.below(20) {
            0 => Event::Duty(rng.below(4) != 0),
            1 => Event::Pinned(rng.below(4) != 0),
            2 => Event::Lock,
            3 | 4 => Event::LockAndSuspend(origin),
            5 => line(StatusLine::Hello(1)),
            6 => line(StatusLine::Requested),
            7 | 8 => line(StatusLine::Locked),
            9 => attempt(KINDS[rng.below(6) as usize]),
            10 => line(StatusLine::Unlocked),
            11 => status(EXITS[rng.below(9) as usize]),
            12 => Event::Exit(ChildExit::Died),
            13 => Event::SpawnFailed,
            14 | 15 => {
                rig.now += Duration::from_millis(rng.below(12_000));
                Event::Tick
            }
            16 => Event::IdleResumed,
            17 | 18 => Event::PrepareForSleep(rng.below(2) == 0),
            _ => match rng.below(4) {
                0 => Event::InhibitorFailed(InhibitorFailure::Bus),
                1 => Event::InhibitorFailed(InhibitorFailure::SleepInProgress),
                2 => Event::SleepBusBack,
                _ => Event::PamFileMissing,
            },
        }
    }

    #[test]
    fn t_p2_inhibitor_invariant_over_random_event_sequences() {
        // `Rig::send` asserts the invariant after every event: held only when
        // available, never while locked, never taken while preparing, held
        // whenever awake, available and uncovered (unless the bus refused),
        // and the model of the actions always agrees with the policy.
        let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..500 {
            let mut rig = Rig::off();
            for _ in 0..120 {
                let event = random_event(&mut rng, &mut rig);
                rig.send(event);
            }
        }
    }

    #[test]
    fn refusal_retries_and_relaunches_are_counted_separately() {
        // A relaunch does not use up the retries a refusal is allowed.
        let mut rig = Rig::locked();
        let relaunch = rig.send(Event::Exit(ChildExit::Died));
        assert_eq!(spawns(&relaunch).len(), 1);
        let retried: Vec<usize> = (0..3)
            .map(|_| spawns(&rig.send(status(LockExit::FinishedBeforeLocked))).len())
            .collect();
        assert_eq!(retried, [1, 1, 0]);
        assert_eq!(rig.policy.state(), State::Idle);
    }

    #[test]
    fn a_child_exit_is_read_from_its_status_and_signal() {
        assert_eq!(
            ChildExit::from_wait(Some(10), false),
            ChildExit::Status(LockExit::FinishedBeforeLocked)
        );
        assert_eq!(
            ChildExit::from_wait(Some(0), false),
            ChildExit::Status(LockExit::Unlocked)
        );
        for died in [
            ChildExit::from_wait(Some(9), false),
            ChildExit::from_wait(Some(137), false),
            ChildExit::from_wait(Some(-1), false),
            ChildExit::from_wait(None, true),
            ChildExit::from_wait(None, false),
            // A signal wins over whatever status accompanies it.
            ChildExit::from_wait(Some(0), true),
            ChildExit::from_wait(Some(16), true),
        ] {
            assert_eq!(died, ChildExit::Died);
        }
    }
}
