// language-contract: allow-non-english
// (the tests type international input: accents, symbols, emoji)
//! The lock process's state machine (design §4.5), pure: no Wayland, no
//! process, no clock of its own. Time comes in as an `Instant` with every
//! input; what the process must do comes out as [`Action`]s the driver
//! carries out. Everything below is therefore tested without a compositor.
//!
//! ```text
//! Connecting -> Requesting -> Prompt <-> Verifying -> Unlocking -> (exit 0)
//! ```
//!
//! The rules that matter, each with its test:
//!
//! * the only way into `Unlocking` is the [`Authenticated`] token of the
//!   current attempt; every other verdict, timeout or stale message returns to
//!   or stays in `Prompt` (TM-1, TM-2);
//! * Enter starts an attempt only with a non-empty field and no wait running;
//!   keys during `Verifying` are dropped; the field is emptied at submit;
//! * the delay is `Backoff`'s: 2, 4, 8, 16, 30, 30 s after refusals, 2 s and
//!   no count after anything that proved nothing;
//! * a key that would take the secret past 512 bytes is ignored (T-K1);
//! * after the fade delay without input every output is drawn black; the next
//!   key or pointer motion brings the prompt back and the field survives;
//! * the prompt follows the keyboard focus, else the pointer, else the first
//!   output by connector name, and moves when its output goes away;
//! * in rehearsal an `Authenticated` outcome is refused too (T-M5).
//!
//! A [`Scene`] is what a view may know: how many characters were typed, never
//! which. Nothing is rendered before `locked`.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use celestina_lock_proto::{
    Backoff, LockExit, StatusLine, ATTEMPT_DEADLINE, PROTOCOL_VERSION, REAP_WAIT,
};
use celestina_wayland::{InputEvent, OutputId, OutputInfo};

use crate::field::{Field, Secret};
use crate::verify::{Authenticated, Denial, Outcome};

/// How long without input before every output goes black. A proposal of the
/// design (`lock.idle_fade_after`, 30 s); it becomes a theme token when the
/// painter takes its sizes and colours from `celestina-tokens`.
pub const FADE_AFTER: Duration = Duration::from_secs(30);

/// Extra time the machine waits, beyond the worker's own deadline and reap
/// bound, before it gives up on a verifier thread that never answered.
const WATCHDOG_MARGIN: Duration = Duration::from_secs(1);

/// The countdown shows whole seconds, so it ticks once a second.
const COUNTDOWN_STEP: Duration = Duration::from_secs(1);

/// The failures from which the lockout hint is shown (`pam_faillock`, deny 3).
pub const FAILLOCK_HINT_FROM: u8 = 3;

// X11 keysym values (`XK_*`), the numbers `xkbcommon` reports.
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_RETURN: u32 = 0xff0d;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_KP_ENTER: u32 = 0xff8d;

/// Why an unlock that was sent could not be confirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnlockFailure {
    /// A protocol error, no capability, or anything else that leaves the
    /// compositor alive: the lock died, the session is still locked, and the
    /// shell relaunches (exit 1 after `requested`).
    Died,
    /// The Wayland connection broke while waiting: the compositor is gone
    /// (exit 16, not relaunched).
    ConnectionLost,
}

impl UnlockFailure {
    /// The exit that stands for this failure.
    #[must_use]
    pub const fn exit(self) -> LockExit {
        match self {
            Self::Died => LockExit::EarlyError,
            Self::ConnectionLost => LockExit::ConnectionLost,
        }
    }
}

/// What a lock process is told by the world, already free of toolkit types
/// that only the toolkit can build.
#[derive(Debug)]
pub enum Input {
    /// The globals are bound and the lock was requested.
    Connected,
    /// The compositor confirmed the lock.
    Locked,
    /// The compositor ended the lock request.
    Finished,
    OutputAdded(OutputInfo),
    OutputRemoved(OutputId),
    SurfaceConfigured {
        output: OutputId,
        size: (u32, u32),
    },
    Keyboard {
        present: bool,
    },
    /// Pointer or keyboard input on the lock surface of `output`.
    Input {
        output: OutputId,
        event: InputEvent,
    },
    /// An attempt ended.
    Verdict {
        attempt: u64,
        outcome: Outcome,
    },
    /// The compositor processed the unlock (the sync round trip returned).
    UnlockConfirmed,
    /// The unlock was sent but its confirmation failed.
    UnlockFailed(UnlockFailure),
    /// What is on screen is stale although the scene did not change (the
    /// output's scale changed, or a frame was refused and a buffer came
    /// back): draw the scene again.
    Redraw,
    /// The deadline the machine armed has passed.
    Tick,
}

/// What the process must do.
#[derive(Debug)]
pub enum Action {
    /// Print a status line for the shell.
    Report(StatusLine),
    /// Draw this.
    Render(Scene),
    /// Start the verifier for this attempt with this secret.
    StartAttempt { attempt: u64, secret: Secret },
    /// End the session lock. The only action that carries the token.
    Unlock(Authenticated),
    /// Wake the machine at this instant with [`Input::Tick`] (`None` cancels).
    Arm(Option<Instant>),
    /// End the process with this status.
    Exit(LockExit),
}

/// Where the machine is (design §4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Connecting,
    Requesting,
    Prompt,
    Verifying {
        attempt: u64,
        deadline: Instant,
    },
    Unlocking,
    /// An exit was decided; nothing more is handled.
    Ended,
}

/// What the status line of the prompt says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Nothing to say.
    Idle,
    /// An attempt is running.
    Checking,
    Refused,
    Account,
    Unavailable,
    Unsupported,
    ServiceMissing,
}

impl From<Denial> for Status {
    fn from(denial: Denial) -> Self {
        match denial {
            Denial::Refused => Self::Refused,
            Denial::Account => Self::Account,
            Denial::Unavailable | Denial::Timeout => Self::Unavailable,
            Denial::Unsupported => Self::Unsupported,
            Denial::ServiceMissing => Self::ServiceMissing,
        }
    }
}

/// What a view draws once the lock is confirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The prompt accepts typing.
    Prompt,
    /// An attempt is running; typing is dropped.
    Checking,
}

/// One output as the view needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneOutput {
    pub id: OutputId,
    /// The connector name (`DP-1`).
    pub name: String,
    /// The logical size of its lock surface, once configured.
    pub size: Option<(u32, u32)>,
    pub scale: f64,
}

/// Everything a view may know. The secret is not in it: only how many
/// characters were typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Every output, by connector name.
    pub outputs: Vec<SceneOutput>,
    /// The output that holds the prompt; `None` when there is no output.
    pub prompt: Option<OutputId>,
    pub phase: Phase,
    /// Characters typed: the bullets.
    pub field_chars: usize,
    pub status: Status,
    /// Whole seconds of delay left (rounded up), while one runs.
    pub wait_secs: Option<u32>,
    /// From the third counted failure, the hint about the account lockout.
    pub faillock_hint: bool,
    pub caps_lock: bool,
    /// The seat has a keyboard.
    pub keyboard: bool,
    /// Every output is drawn black, the prompt included.
    pub faded: bool,
    /// This is a rehearsal: the lock never unlocks.
    pub rehearsal: bool,
}

/// How the machine is set up at start.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// `--rehearsal`: attempts go to the rehearsal service and the machine
    /// never unlocks.
    pub rehearsal: bool,
    /// The production service file was not found when the lock started.
    pub service_missing: bool,
    /// Failures the previous lock counted (the bundle's header).
    pub prior_failures: u8,
    pub fade_after: Duration,
}

impl Config {
    #[must_use]
    pub const fn new(rehearsal: bool, service_missing: bool, prior_failures: u8) -> Self {
        Self {
            rehearsal,
            service_missing,
            prior_failures,
            fade_after: FADE_AFTER,
        }
    }
}

struct OutputEntry {
    name: String,
    size: Option<(u32, u32)>,
    scale: f64,
}

/// The state machine.
pub struct Machine {
    cfg: Config,
    state: State,
    field: Field,
    status: Status,
    backoff: Backoff,
    wait_until: Option<Instant>,
    faded: bool,
    last_activity: Instant,
    attempt: u64,
    outputs: BTreeMap<OutputId, OutputEntry>,
    keyboard: bool,
    caps_lock: bool,
    keyboard_focus: Option<OutputId>,
    pointer_output: Option<OutputId>,
    rendered: Option<Scene>,
    armed: Option<Instant>,
}

impl Machine {
    #[must_use]
    pub fn new(cfg: Config, now: Instant) -> Self {
        Self {
            cfg,
            state: State::Connecting,
            field: Field::new(),
            status: Status::Idle,
            backoff: Backoff::with_prior_failures(cfg.prior_failures),
            wait_until: None,
            faded: false,
            last_activity: now,
            attempt: 0,
            outputs: BTreeMap::new(),
            keyboard: false,
            caps_lock: false,
            keyboard_focus: None,
            pointer_output: None,
            rendered: None,
            armed: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Failures counted so far.
    #[must_use]
    pub fn failures(&self) -> u8 {
        self.backoff.failures()
    }

    /// Takes one input in and says what to do about it.
    pub fn handle(&mut self, input: Input, now: Instant) -> Vec<Action> {
        let mut out = Vec::new();
        if self.state == State::Ended {
            return out;
        }
        match input {
            Input::Connected => self.connected(&mut out),
            Input::Locked => {
                // Normally `Connected` came first; if the compositor was
                // quicker than the message, the lines still come in order.
                self.connected(&mut out);
                self.locked(now, &mut out);
            }
            Input::Finished => self.finished(&mut out),
            Input::OutputAdded(info) => self.output_added(&info),
            Input::OutputRemoved(id) => self.output_removed(id),
            Input::SurfaceConfigured { output, size } => {
                if let Some(entry) = self.outputs.get_mut(&output) {
                    entry.size = Some(size);
                }
            }
            Input::Keyboard { present } => self.keyboard = present,
            Input::Redraw => self.rendered = None,
            Input::Input { output, event } => self.input(output, &event, now, &mut out),
            Input::Verdict { attempt, outcome } => self.verdict(attempt, outcome, now, &mut out),
            Input::UnlockConfirmed => {
                if self.state == State::Unlocking {
                    out.push(Action::Report(StatusLine::Unlocked));
                    self.exit(LockExit::Unlocked, &mut out);
                }
            }
            Input::UnlockFailed(failure) => {
                if self.state == State::Unlocking {
                    // Sent but unconfirmed (finding I-A). Unless the
                    // connection itself broke, this is a death: the
                    // compositor may have killed the client with a protocol
                    // error, or sent nothing at all, and the session is still
                    // locked. Status 1 after `requested` is "any other
                    // status" of design §4.3 (b): the shell relaunches
                    // (F29). Only a lost connection is 16, which the shell
                    // does not relaunch.
                    tracing::error!(?failure, "the unlock was not confirmed");
                    self.exit(failure.exit(), &mut out);
                }
            }
            Input::Tick => self.tick(now, &mut out),
        }
        self.settle(now, &mut out);
        out
    }

    fn connected(&mut self, out: &mut Vec<Action>) {
        if self.state == State::Connecting {
            self.state = State::Requesting;
            out.push(Action::Report(StatusLine::Hello(PROTOCOL_VERSION)));
            out.push(Action::Report(StatusLine::Requested));
        }
    }

    fn locked(&mut self, now: Instant, out: &mut Vec<Action>) {
        if self.state == State::Requesting {
            self.state = State::Prompt;
            self.last_activity = now;
            self.status = if self.cfg.service_missing {
                Status::ServiceMissing
            } else {
                Status::Idle
            };
            out.push(Action::Report(StatusLine::Locked));
        }
    }

    fn finished(&mut self, out: &mut Vec<Action>) {
        match self.state {
            State::Connecting | State::Requesting => {
                self.exit(LockExit::FinishedBeforeLocked, out);
            }
            State::Prompt | State::Verifying { .. } => {
                self.exit(LockExit::FinishedAfterLocked, out);
            }
            // The unlock is under way; the compositor's answer to it is not
            // an ending of its own.
            State::Unlocking | State::Ended => {}
        }
    }

    fn exit(&mut self, code: LockExit, out: &mut Vec<Action>) {
        self.state = State::Ended;
        out.push(Action::Exit(code));
    }

    fn output_added(&mut self, info: &OutputInfo) {
        let entry = self.outputs.entry(info.id).or_insert(OutputEntry {
            name: info.name.clone(),
            size: None,
            scale: info.scale,
        });
        entry.name.clone_from(&info.name);
        entry.scale = info.scale;
    }

    fn output_removed(&mut self, id: OutputId) {
        self.outputs.remove(&id);
        if self.keyboard_focus == Some(id) {
            self.keyboard_focus = None;
        }
        if self.pointer_output == Some(id) {
            self.pointer_output = None;
        }
    }

    fn activity(&mut self, now: Instant) {
        self.last_activity = now;
        self.faded = false;
    }

    fn input(&mut self, output: OutputId, event: &InputEvent, now: Instant, out: &mut Vec<Action>) {
        match event {
            InputEvent::FocusEnter => self.keyboard_focus = Some(output),
            InputEvent::FocusLost => {
                if self.keyboard_focus == Some(output) {
                    self.keyboard_focus = None;
                }
            }
            InputEvent::PointerMotion { .. }
            | InputEvent::PointerPress { .. }
            | InputEvent::Axis { .. } => {
                self.pointer_output = Some(output);
                self.activity(now);
            }
            InputEvent::PointerLeave => {
                if self.pointer_output == Some(output) {
                    self.pointer_output = None;
                }
            }
            // The state with no key behind it: Caps Lock already latched when
            // the keyboard enters shows before the first key (finding M2).
            InputEvent::Modifiers(modifiers) => self.caps_lock = modifiers.caps_lock,
            InputEvent::Key {
                keysym,
                utf8,
                modifiers,
                pressed,
                ..
            } => {
                self.caps_lock = modifiers.caps_lock;
                if *pressed {
                    self.key(*keysym, utf8.as_deref(), now, out);
                }
            }
            _ => {}
        }
    }

    fn key(&mut self, keysym: u32, text: Option<&str>, now: Instant, out: &mut Vec<Action>) {
        // A key brings the prompt back and is then handled like any other.
        self.activity(now);
        if self.state != State::Prompt {
            // During `Verifying` the secret already left and the next one
            // starts empty; before `locked` there is no prompt.
            return;
        }
        match keysym {
            KEY_RETURN | KEY_KP_ENTER => self.enter(now, out),
            KEY_BACKSPACE => self.field.pop(),
            KEY_ESCAPE => self.field.clear(),
            _ => {
                if let Some(text) = text {
                    // Refused text (a control key, past 512 bytes) is simply
                    // not taken.
                    let _ = self.field.push(text);
                }
            }
        }
    }

    fn enter(&mut self, now: Instant, out: &mut Vec<Action>) {
        if self.field.is_empty() {
            return;
        }
        if self.wait_until.is_some_and(|until| until > now) {
            return;
        }
        self.attempt += 1;
        let attempt = self.attempt;
        self.state = State::Verifying {
            attempt,
            deadline: now + ATTEMPT_DEADLINE + REAP_WAIT + WATCHDOG_MARGIN,
        };
        self.status = Status::Checking;
        self.wait_until = None;
        out.push(Action::StartAttempt {
            attempt,
            secret: self.field.take(),
        });
    }

    fn verdict(&mut self, attempt: u64, outcome: Outcome, now: Instant, out: &mut Vec<Action>) {
        let State::Verifying {
            attempt: current, ..
        } = self.state
        else {
            tracing::debug!(attempt, "verdict discarded: no attempt is running");
            return;
        };
        if attempt != current {
            tracing::debug!(
                attempt,
                current,
                "verdict discarded: not the current attempt"
            );
            return;
        }
        match outcome {
            Outcome::Authenticated(token) if !self.cfg.rehearsal => {
                self.state = State::Unlocking;
                out.push(Action::Unlock(token));
            }
            Outcome::Authenticated(token) => {
                drop(token);
                tracing::error!(
                    "the rehearsal service authenticated: refused, a rehearsal never unlocks"
                );
                self.deny(Denial::Unavailable, now, out);
            }
            Outcome::Denied(denial) => self.deny(denial, now, out),
        }
    }

    fn deny(&mut self, denial: Denial, now: Instant, out: &mut Vec<Action>) {
        let wait = self.backoff.record(denial.verdict());
        self.wait_until = Some(now + wait);
        self.status = denial.into();
        self.state = State::Prompt;
        // The person reads the answer: the fade counts from here.
        self.activity(now);
        out.push(Action::Report(StatusLine::Attempt(denial.kind())));
    }

    fn tick(&mut self, now: Instant, out: &mut Vec<Action>) {
        match self.state {
            State::Verifying { deadline, .. } if now >= deadline => {
                // The worker never answered: whatever it says later is stale.
                tracing::error!("the verify worker did not answer in time");
                self.deny(Denial::Timeout, now, out);
            }
            State::Prompt if !self.faded && now >= self.last_activity + self.cfg.fade_after => {
                self.faded = true;
            }
            _ => {}
        }
    }

    /// The output that holds the prompt: keyboard focus, else the pointer,
    /// else the first by connector name.
    fn prompt_output(&self) -> Option<OutputId> {
        let live = |id: &OutputId| self.outputs.contains_key(id);
        self.keyboard_focus
            .filter(live)
            .or_else(|| self.pointer_output.filter(live))
            .or_else(|| {
                self.outputs
                    .iter()
                    .min_by(|a, b| a.1.name.cmp(&b.1.name).then(a.0.cmp(b.0)))
                    .map(|(id, _)| *id)
            })
    }

    fn wait_secs(&self, now: Instant) -> Option<u32> {
        let left = self.wait_until?.checked_duration_since(now)?;
        if left.is_zero() {
            return None;
        }
        let secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        Some(u32::try_from(secs).unwrap_or(u32::MAX))
    }

    /// What a view may draw now.
    #[must_use]
    pub fn scene(&self, now: Instant) -> Scene {
        let mut outputs: Vec<SceneOutput> = self
            .outputs
            .iter()
            .map(|(id, entry)| SceneOutput {
                id: *id,
                name: entry.name.clone(),
                size: entry.size,
                scale: entry.scale,
            })
            .collect();
        outputs.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        Scene {
            outputs,
            prompt: self.prompt_output(),
            phase: if matches!(self.state, State::Verifying { .. }) {
                Phase::Checking
            } else {
                Phase::Prompt
            },
            field_chars: self.field.chars(),
            status: self.status,
            wait_secs: self.wait_secs(now),
            faillock_hint: self.backoff.failures() >= FAILLOCK_HINT_FROM,
            caps_lock: self.caps_lock,
            keyboard: self.keyboard,
            faded: self.faded,
            rehearsal: self.cfg.rehearsal,
        }
    }

    /// When the machine next needs a [`Input::Tick`].
    fn next_wake(&self, now: Instant) -> Option<Instant> {
        match self.state {
            State::Prompt if !self.faded => {
                let fade = self.last_activity + self.cfg.fade_after;
                let countdown = self
                    .wait_until
                    .filter(|until| *until > now)
                    .map(|until| (now + COUNTDOWN_STEP).min(until));
                Some(countdown.map_or(fade, |step| step.min(fade)))
            }
            State::Verifying { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    /// Adds the render and the timer the new state calls for.
    fn settle(&mut self, now: Instant, out: &mut Vec<Action>) {
        if matches!(self.state, State::Prompt | State::Verifying { .. }) {
            let scene = self.scene(now);
            if self.rendered.as_ref() != Some(&scene) {
                self.rendered = Some(scene.clone());
                out.push(Action::Render(scene));
            }
        }
        let wake = self.next_wake(now);
        if wake != self.armed {
            self.armed = wake;
            out.push(Action::Arm(wake));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::test_token;
    use celestina_wayland::Modifiers;

    const SECS: fn(u64) -> Duration = Duration::from_secs;

    struct Rig {
        machine: Machine,
        now: Instant,
    }

    impl Rig {
        fn new(cfg: Config) -> Self {
            let now = Instant::now();
            Self {
                machine: Machine::new(cfg, now),
                now,
            }
        }

        /// A lock that is up: connected and locked, one output.
        fn locked() -> Self {
            Self::locked_with(Config::new(false, false, 0))
        }

        fn locked_with(cfg: Config) -> Self {
            let mut rig = Self::new(cfg);
            rig.feed(Input::Keyboard { present: true });
            rig.feed(Input::OutputAdded(info(1, "DP-1")));
            rig.feed(Input::Connected);
            rig.feed(Input::Locked);
            assert_eq!(rig.machine.state(), State::Prompt);
            rig
        }

        fn feed(&mut self, input: Input) -> Vec<Action> {
            self.machine.handle(input, self.now)
        }

        fn advance(&mut self, by: Duration) {
            self.now += by;
        }

        fn tick(&mut self) -> Vec<Action> {
            self.feed(Input::Tick)
        }

        fn key_event(keysym: u32, text: Option<&str>) -> InputEvent {
            InputEvent::Key {
                keysym,
                utf8: text.map(str::to_owned),
                modifiers: Modifiers::default(),
                pressed: true,
                serial: 0,
            }
        }

        fn press(&mut self, keysym: u32, text: Option<&str>) -> Vec<Action> {
            self.feed(Input::Input {
                output: OutputId(1),
                event: Self::key_event(keysym, text),
            })
        }

        fn type_text(&mut self, text: &str) {
            for ch in text.chars() {
                let s = ch.to_string();
                self.press(u32::from(ch), Some(&s));
            }
        }

        fn enter(&mut self) -> Vec<Action> {
            self.press(KEY_RETURN, Some("\r"))
        }

        fn scene(&self) -> Scene {
            self.machine.scene(self.now)
        }
    }

    fn info(id: u32, name: &str) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_owned(),
            logical_size: (1920, 1080),
            scale: 1.0,
            refresh_mhz: 60_000,
        }
    }

    fn reports(actions: &[Action]) -> Vec<StatusLine> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Report(line) => Some(line.clone()),
                _ => None,
            })
            .collect()
    }

    fn exit_of(actions: &[Action]) -> Option<LockExit> {
        actions.iter().find_map(|a| match a {
            Action::Exit(code) => Some(*code),
            _ => None,
        })
    }

    fn started(actions: &[Action]) -> Vec<(u64, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::StartAttempt { attempt, secret } => {
                    Some((*attempt, secret.as_bytes().to_vec()))
                }
                _ => None,
            })
            .collect()
    }

    fn unlocks(actions: &[Action]) -> usize {
        actions
            .iter()
            .filter(|a| matches!(a, Action::Unlock(_)))
            .count()
    }

    fn renders(actions: &[Action]) -> usize {
        actions
            .iter()
            .filter(|a| matches!(a, Action::Render(_)))
            .count()
    }

    fn denied(attempt: u64, denial: Denial) -> Input {
        Input::Verdict {
            attempt,
            outcome: Outcome::Denied(denial),
        }
    }

    fn authenticated(attempt: u64) -> Input {
        Input::Verdict {
            attempt,
            outcome: Outcome::Authenticated(test_token()),
        }
    }

    #[test]
    fn t_m1_every_transition_of_section_4_5() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        assert_eq!(rig.machine.state(), State::Connecting);

        // Connecting -> Requesting: hello, then requested.
        let actions = rig.feed(Input::Connected);
        assert_eq!(
            reports(&actions),
            [StatusLine::Hello(1), StatusLine::Requested]
        );
        assert_eq!(rig.machine.state(), State::Requesting);
        // A second Connected changes nothing.
        assert!(reports(&rig.feed(Input::Connected)).is_empty());

        // Requesting -> Prompt on locked.
        let actions = rig.feed(Input::Locked);
        assert_eq!(reports(&actions), [StatusLine::Locked]);
        assert_eq!(rig.machine.state(), State::Prompt);
        // `locked` twice prints once.
        assert!(reports(&rig.feed(Input::Locked)).is_empty());

        // Prompt -> Verifying on Enter with a field.
        rig.type_text("pw");
        let actions = rig.enter();
        assert_eq!(started(&actions), [(1, b"pw".to_vec())]);
        assert!(matches!(
            rig.machine.state(),
            State::Verifying { attempt: 1, .. }
        ));

        // Verifying -> Prompt on a denial.
        let actions = rig.feed(denied(1, Denial::Refused));
        assert_eq!(
            reports(&actions),
            [StatusLine::Attempt(
                celestina_lock_proto::AttemptKind::Refused
            )]
        );
        assert_eq!(rig.machine.state(), State::Prompt);

        // Verifying -> Unlocking -> exit 0 on the current attempt's token.
        rig.advance(SECS(3));
        rig.type_text("pw");
        rig.enter();
        let actions = rig.feed(authenticated(2));
        assert_eq!(unlocks(&actions), 1);
        assert_eq!(rig.machine.state(), State::Unlocking);
        // Nothing is said or ended until the compositor confirmed.
        assert!(reports(&actions).is_empty());
        assert_eq!(exit_of(&actions), None);
        let actions = rig.feed(Input::UnlockConfirmed);
        assert_eq!(reports(&actions), [StatusLine::Unlocked]);
        assert_eq!(exit_of(&actions), Some(LockExit::Unlocked));
        assert_eq!(rig.machine.state(), State::Ended);
        // An ended machine hears nothing.
        assert!(rig.press(u32::from('a'), Some("a")).is_empty());
    }

    #[test]
    fn locked_before_connected_still_prints_the_lines_in_order() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        let actions = rig.feed(Input::Locked);
        assert_eq!(
            reports(&actions),
            [
                StatusLine::Hello(1),
                StatusLine::Requested,
                StatusLine::Locked
            ]
        );
        assert_eq!(rig.machine.state(), State::Prompt);
    }

    #[test]
    fn finished_before_locked_exits_10_and_after_locked_exits_13() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::Connected);
        assert_eq!(
            exit_of(&rig.feed(Input::Finished)),
            Some(LockExit::FinishedBeforeLocked)
        );

        let mut rig = Rig::locked();
        assert_eq!(
            exit_of(&rig.feed(Input::Finished)),
            Some(LockExit::FinishedAfterLocked)
        );

        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        assert_eq!(
            exit_of(&rig.feed(Input::Finished)),
            Some(LockExit::FinishedAfterLocked)
        );

        // While the unlock is under way the compositor's answer is no ending.
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        rig.feed(authenticated(1));
        assert_eq!(exit_of(&rig.feed(Input::Finished)), None);
        assert_eq!(rig.machine.state(), State::Unlocking);
    }

    #[test]
    fn an_unconfirmed_unlock_exits_as_a_death_never_as_unlocked() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        rig.feed(authenticated(1));
        let actions = rig.feed(Input::UnlockFailed(UnlockFailure::Died));
        assert!(reports(&actions).is_empty(), "no `unlocked` line");
        // Finding I-A: an unconfirmed unlock is a death (status 1 after
        // `requested`), which the shell answers by relaunching; only a lost
        // connection is 16.
        assert_eq!(exit_of(&actions), Some(LockExit::EarlyError));
        assert_eq!(LockExit::EarlyError.code(), 1);
    }

    #[test]
    fn an_unlock_that_lost_the_connection_exits_16_and_prints_nothing() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        rig.feed(authenticated(1));
        let actions = rig.feed(Input::UnlockFailed(UnlockFailure::ConnectionLost));
        assert!(reports(&actions).is_empty(), "no `unlocked` line");
        assert_eq!(exit_of(&actions), Some(LockExit::ConnectionLost));
    }

    #[test]
    fn t_m2_stale_verdicts_are_discarded() {
        let mut rig = Rig::locked();
        // No attempt is running: a verdict, even an authenticated one, is
        // nothing.
        assert_eq!(unlocks(&rig.feed(authenticated(1))), 0);
        assert_eq!(rig.machine.state(), State::Prompt);

        rig.type_text("pw");
        rig.enter(); // attempt 1
                     // A verdict of another attempt number does not touch the running one.
        assert_eq!(unlocks(&rig.feed(authenticated(7))), 0);
        assert!(matches!(
            rig.machine.state(),
            State::Verifying { attempt: 1, .. }
        ));
        assert!(reports(&rig.feed(denied(0, Denial::Refused))).is_empty());

        // The watchdog gives up on attempt 1; its late `Authenticated` is
        // stale and, once attempt 2 runs, so is attempt 1's.
        rig.advance(SECS(30));
        let actions = rig.tick();
        assert_eq!(
            reports(&actions),
            [StatusLine::Attempt(
                celestina_lock_proto::AttemptKind::Timeout
            )]
        );
        assert_eq!(unlocks(&rig.feed(authenticated(1))), 0);
        rig.advance(SECS(5));
        rig.type_text("pw");
        rig.enter(); // attempt 2
        assert_eq!(unlocks(&rig.feed(authenticated(1))), 0);
        assert!(matches!(
            rig.machine.state(),
            State::Verifying { attempt: 2, .. }
        ));
        assert_eq!(unlocks(&rig.feed(authenticated(2))), 1);
    }

    #[test]
    fn t_m3_delay_schedule_2_4_8_16_30_30() {
        let mut rig = Rig::locked();
        let mut waits = Vec::new();
        for attempt in 1..=6 {
            rig.type_text("pw");
            rig.enter();
            rig.feed(denied(attempt, Denial::Refused));
            waits.push(rig.scene().wait_secs);
            rig.advance(SECS(31));
            rig.tick();
        }
        assert_eq!(
            waits,
            [Some(2), Some(4), Some(8), Some(16), Some(30), Some(30)]
        );
        assert_eq!(rig.machine.failures(), 6);
        // Account refusals count like wrong passwords.
        rig.type_text("pw");
        rig.enter();
        rig.feed(denied(7, Denial::Account));
        assert_eq!(rig.scene().wait_secs, Some(30));
    }

    #[test]
    fn t_m4_unavailable_waits_2_s_and_does_not_count() {
        for denial in [
            Denial::Unavailable,
            Denial::Timeout,
            Denial::Unsupported,
            Denial::ServiceMissing,
        ] {
            let mut rig = Rig::locked_with(Config::new(false, false, 2));
            rig.type_text("pw");
            rig.enter();
            rig.feed(denied(1, denial));
            assert_eq!(rig.scene().wait_secs, Some(2), "{denial:?}");
            assert_eq!(rig.machine.failures(), 2, "{denial:?}");
            assert_eq!(rig.machine.state(), State::Prompt);
        }
    }

    #[test]
    fn t_m5_rehearsal_never_reaches_unlocking() {
        let mut rig = Rig::locked_with(Config::new(true, false, 0));
        assert!(rig.scene().rehearsal);
        rig.type_text("pw");
        rig.enter();
        let actions = rig.feed(authenticated(1));
        assert_eq!(unlocks(&actions), 0);
        assert_eq!(rig.machine.state(), State::Prompt);
        assert_eq!(rig.scene().status, Status::Unavailable);
        assert_eq!(exit_of(&actions), None);
    }

    #[test]
    fn enter_with_an_empty_field_does_nothing() {
        let mut rig = Rig::locked();
        let actions = rig.enter();
        assert!(started(&actions).is_empty());
        assert_eq!(rig.machine.state(), State::Prompt);
        // Control keys leave the field empty too.
        rig.press(0xff09, Some("\t"));
        rig.press(0xff1b, Some("\u{1b}"));
        assert!(started(&rig.enter()).is_empty());
    }

    #[test]
    fn enter_while_waiting_does_nothing_and_keeps_the_countdown() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        rig.feed(denied(1, Denial::Refused));
        assert_eq!(rig.scene().wait_secs, Some(2));
        rig.type_text("next");
        rig.advance(Duration::from_millis(500));
        let actions = rig.enter();
        assert!(started(&actions).is_empty());
        assert_eq!(rig.scene().wait_secs, Some(2), "1.5 s left rounds up to 2");
        assert_eq!(rig.scene().field_chars, 4, "what was typed stays");
        rig.advance(SECS(2));
        rig.tick();
        assert_eq!(rig.scene().wait_secs, None);
        assert_eq!(started(&rig.enter()), [(2, b"next".to_vec())]);
    }

    #[test]
    fn double_enter_starts_one_attempt() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        let first = rig.enter();
        let second = rig.enter();
        assert_eq!(started(&first).len(), 1);
        assert!(started(&second).is_empty());
        assert!(matches!(
            rig.machine.state(),
            State::Verifying { attempt: 1, .. }
        ));
    }

    #[test]
    fn keys_during_verifying_are_dropped() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        assert_eq!(rig.scene().field_chars, 0, "the secret already left");
        rig.type_text("late");
        rig.press(KEY_BACKSPACE, None);
        assert_eq!(rig.scene().field_chars, 0);
        rig.feed(denied(1, Denial::Refused));
        assert_eq!(rig.scene().field_chars, 0, "the next one starts empty");
    }

    #[test]
    fn a_verdict_after_the_prompt_output_vanished_unlocks_or_denies_once_and_the_prompt_moves() {
        for authenticated_verdict in [true, false] {
            let mut rig = Rig::locked();
            rig.feed(Input::OutputAdded(info(2, "HDMI-A-1")));
            rig.feed(Input::Input {
                output: OutputId(1),
                event: InputEvent::FocusEnter,
            });
            assert_eq!(rig.scene().prompt, Some(OutputId(1)));
            rig.type_text("pw");
            rig.enter();
            rig.feed(Input::OutputRemoved(OutputId(1)));
            assert_eq!(rig.scene().prompt, Some(OutputId(2)), "the prompt moves");
            let verdict = if authenticated_verdict {
                authenticated(1)
            } else {
                denied(1, Denial::Refused)
            };
            let actions = rig.feed(verdict);
            assert_eq!(unlocks(&actions), usize::from(authenticated_verdict));
            // Exactly once: the same verdict again is stale.
            let again = rig.feed(if authenticated_verdict {
                authenticated(1)
            } else {
                denied(1, Denial::Refused)
            });
            assert_eq!(unlocks(&again), 0);
            assert!(reports(&again).is_empty());
            if !authenticated_verdict {
                assert_eq!(rig.scene().prompt, Some(OutputId(2)));
                assert_eq!(rig.machine.failures(), 1, "counted once");
            }
        }
    }

    #[test]
    fn t_k1_a_key_past_512_bytes_is_ignored_so_the_secret_is_never_refused_as_oversized() {
        let mut rig = Rig::locked();
        for _ in 0..256 {
            rig.press(0xe9, Some("é"));
        }
        // The 513th byte, alone or as the start of a wider character, is not
        // taken.
        rig.press(0x61, Some("a"));
        rig.press(0x20ac, Some("€"));
        rig.press(0xe9, Some("é"));
        assert_eq!(rig.scene().field_chars, 256);
        let actions = rig.enter();
        let started = started(&actions);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].1.len(), 512);
        assert_eq!(started[0].1, "é".repeat(256).into_bytes());
    }

    #[test]
    fn backspace_removes_the_last_char_and_escape_clears() {
        let mut rig = Rig::locked();
        rig.type_text("añ€");
        assert_eq!(rig.scene().field_chars, 3);
        rig.press(KEY_BACKSPACE, None);
        assert_eq!(rig.scene().field_chars, 2);
        rig.press(KEY_BACKSPACE, None);
        rig.press(KEY_BACKSPACE, None);
        rig.press(KEY_BACKSPACE, None);
        assert_eq!(rig.scene().field_chars, 0);
        rig.type_text("abc");
        rig.press(KEY_ESCAPE, Some("\u{1b}"));
        assert_eq!(rig.scene().field_chars, 0);
        assert!(started(&rig.enter()).is_empty());
    }

    #[test]
    fn keypad_enter_and_key_releases() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        // A release is not a key press: it neither types nor submits.
        let release = InputEvent::Key {
            keysym: KEY_RETURN,
            utf8: Some("\r".to_owned()),
            modifiers: Modifiers::default(),
            pressed: false,
            serial: 0,
        };
        let actions = rig.feed(Input::Input {
            output: OutputId(1),
            event: release,
        });
        assert!(started(&actions).is_empty());
        assert_eq!(started(&rig.press(KEY_KP_ENTER, None)).len(), 1);
    }

    #[test]
    fn t_m6_fade_after_the_token_delay_a_key_or_motion_restores_and_the_field_survives() {
        let mut rig = Rig::locked();
        rig.type_text("abc");
        assert!(!rig.scene().faded);
        // Just before the delay: nothing.
        rig.advance(SECS(29));
        rig.tick();
        assert!(!rig.scene().faded);
        rig.advance(SECS(2));
        let actions = rig.tick();
        assert!(rig.scene().faded, "black after 30 s without input");
        assert_eq!(renders(&actions), 1);
        assert_eq!(rig.scene().field_chars, 3, "the field survives");
        // No more wake-ups while black.
        assert!(matches!(actions.last(), Some(Action::Arm(None))));

        // A key restores the prompt and is handled like any other.
        rig.press(u32::from('d'), Some("d"));
        assert!(!rig.scene().faded);
        assert_eq!(rig.scene().field_chars, 4);

        // Motion restores it too, without typing.
        rig.advance(SECS(31));
        rig.tick();
        assert!(rig.scene().faded);
        rig.feed(Input::Input {
            output: OutputId(1),
            event: InputEvent::PointerMotion { x: 1.0, y: 1.0 },
        });
        assert!(!rig.scene().faded);
        assert_eq!(rig.scene().field_chars, 4);
    }

    #[test]
    fn the_fade_counts_from_the_last_input_not_from_the_lock() {
        let mut rig = Rig::locked();
        rig.advance(SECS(20));
        rig.press(u32::from('a'), Some("a"));
        rig.advance(SECS(20));
        rig.tick();
        assert!(
            !rig.scene().faded,
            "40 s after the lock, 20 s after the key"
        );
        rig.advance(SECS(11));
        rig.tick();
        assert!(rig.scene().faded);
    }

    #[test]
    fn t_o1_to_t_o3_outputs_added_removed_and_zero_outputs_at_locked() {
        // T-O1: outputs before and after `locked` all appear, ordered by name.
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::OutputAdded(info(5, "HDMI-A-1")));
        rig.feed(Input::Connected);
        rig.feed(Input::Locked);
        rig.feed(Input::OutputAdded(info(2, "DP-1")));
        let names: Vec<String> = rig.scene().outputs.into_iter().map(|o| o.name).collect();
        assert_eq!(names, ["DP-1", "HDMI-A-1"]);
        // Describing an output again adds nothing.
        rig.feed(Input::OutputAdded(info(2, "DP-1")));
        assert_eq!(rig.scene().outputs.len(), 2);
        // Its lock surface's size is recorded when configured.
        rig.feed(Input::SurfaceConfigured {
            output: OutputId(2),
            size: (3840, 2160),
        });
        assert_eq!(rig.scene().outputs[0].size, Some((3840, 2160)));

        // T-O2: a removed output goes, the field and the lock stay.
        rig.type_text("abc");
        rig.feed(Input::OutputRemoved(OutputId(2)));
        assert_eq!(rig.scene().outputs.len(), 1);
        assert_eq!(rig.scene().field_chars, 3);
        assert_eq!(rig.scene().prompt, Some(OutputId(5)));
        // Removing it twice, or an unknown one, is harmless.
        rig.feed(Input::OutputRemoved(OutputId(2)));
        rig.feed(Input::OutputRemoved(OutputId(99)));

        // T-O3: no output at all when locked (F39), then one appears.
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::Connected);
        let actions = rig.feed(Input::Locked);
        assert_eq!(reports(&actions), [StatusLine::Locked]);
        assert_eq!(rig.scene().prompt, None);
        assert!(rig.scene().outputs.is_empty());
        let actions = rig.feed(Input::OutputAdded(info(1, "DP-1")));
        assert_eq!(renders(&actions), 1, "the new output gets its prompt");
        assert_eq!(rig.scene().prompt, Some(OutputId(1)));
    }

    #[test]
    fn prompt_follows_the_keyboard_then_the_pointer_and_defaults_to_the_first_by_name() {
        let mut rig = Rig::locked();
        rig.feed(Input::OutputAdded(info(2, "HDMI-A-1")));
        rig.feed(Input::OutputAdded(info(3, "DP-2")));
        // Neither keyboard nor pointer: the first by connector name, not by id.
        assert_eq!(rig.scene().prompt, Some(OutputId(1)));
        // The pointer, before any keyboard enter.
        rig.feed(Input::Input {
            output: OutputId(2),
            event: InputEvent::PointerMotion { x: 0.0, y: 0.0 },
        });
        assert_eq!(rig.scene().prompt, Some(OutputId(2)));
        // The keyboard focus wins over the pointer.
        rig.feed(Input::Input {
            output: OutputId(3),
            event: InputEvent::FocusEnter,
        });
        assert_eq!(rig.scene().prompt, Some(OutputId(3)));
        // Losing the keyboard falls back to the pointer, then to the first.
        rig.feed(Input::Input {
            output: OutputId(3),
            event: InputEvent::FocusLost,
        });
        assert_eq!(rig.scene().prompt, Some(OutputId(2)));
        rig.feed(Input::Input {
            output: OutputId(2),
            event: InputEvent::PointerLeave,
        });
        assert_eq!(rig.scene().prompt, Some(OutputId(1)));
        // A focused output that goes away is forgotten.
        rig.feed(Input::Input {
            output: OutputId(3),
            event: InputEvent::FocusEnter,
        });
        rig.feed(Input::OutputRemoved(OutputId(3)));
        assert_eq!(rig.scene().prompt, Some(OutputId(1)));
    }

    #[test]
    fn prior_failures_from_the_header_seeds_the_backoff() {
        let mut rig = Rig::locked_with(Config::new(false, false, 3));
        assert_eq!(rig.machine.failures(), 3);
        assert!(rig.scene().faillock_hint, "from the third failure");
        assert_eq!(rig.scene().wait_secs, None, "seeding imposes no wait");
        rig.type_text("pw");
        rig.enter();
        rig.feed(denied(1, Denial::Refused));
        assert_eq!(rig.scene().wait_secs, Some(16), "the schedule continues");
        assert_eq!(rig.machine.failures(), 4);

        let fresh = Rig::locked();
        assert!(!fresh.scene().faillock_hint);
    }

    #[test]
    fn a_missing_service_is_said_from_the_start() {
        let rig = Rig::locked_with(Config::new(false, true, 0));
        assert_eq!(rig.scene().status, Status::ServiceMissing);
        let rig = Rig::locked();
        assert_eq!(rig.scene().status, Status::Idle);
    }

    #[test]
    fn a_redraw_renders_the_unchanged_scene_again_and_never_before_locked() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::OutputAdded(info(1, "DP-1")));
        assert_eq!(
            renders(&rig.feed(Input::Redraw)),
            0,
            "nothing before locked"
        );
        rig.feed(Input::Connected);
        rig.feed(Input::Locked);
        assert_eq!(renders(&rig.feed(Input::Redraw)), 1);
        assert_eq!(renders(&rig.tick()), 0, "and only once");
    }

    #[test]
    fn nothing_is_rendered_before_locked_and_only_changes_render_after() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        assert_eq!(renders(&rig.feed(Input::OutputAdded(info(1, "DP-1")))), 0);
        assert_eq!(renders(&rig.feed(Input::Connected)), 0);
        assert_eq!(renders(&rig.press(u32::from('a'), Some("a"))), 0);
        assert_eq!(rig.scene().field_chars, 0, "no typing before the prompt");
        assert_eq!(renders(&rig.feed(Input::Locked)), 1);
        // A key that changes nothing visible renders nothing.
        assert_eq!(renders(&rig.press(0xff09, Some("\t"))), 0);
        assert_eq!(renders(&rig.press(u32::from('a'), Some("a"))), 1);
        // A tick that changes nothing renders nothing.
        assert_eq!(renders(&rig.tick()), 0);
    }

    #[test]
    fn the_countdown_renders_once_a_second_and_never_when_nothing_changes() {
        let mut rig = Rig::locked();
        rig.type_text("pw");
        rig.enter();
        rig.feed(denied(1, Denial::Refused));
        rig.advance(SECS(5));
        rig.feed(Input::Tick);
        rig.type_text("x");
        rig.enter();
        let actions = rig.feed(denied(2, Denial::Refused));
        assert_eq!(rig.scene().wait_secs, Some(4));
        // The next wake is a second away, not 4 s.
        let wake = actions.iter().rev().find_map(|a| match a {
            Action::Arm(at) => Some(*at),
            _ => None,
        });
        assert_eq!(wake, Some(Some(rig.now + SECS(1))));
        rig.advance(SECS(1));
        assert_eq!(renders(&rig.tick()), 1);
        assert_eq!(rig.scene().wait_secs, Some(3));
    }

    #[test]
    fn the_machine_arms_the_fade_then_the_attempt_deadline_then_nothing() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::OutputAdded(info(1, "DP-1")));
        let locked = rig.now;
        rig.feed(Input::Connected);
        let actions = rig.feed(Input::Locked);
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Arm(Some(at)) if *at == locked + FADE_AFTER)));
        rig.type_text("pw");
        let actions = rig.enter();
        let deadline = rig.now + ATTEMPT_DEADLINE + REAP_WAIT + WATCHDOG_MARGIN;
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Arm(Some(at)) if *at == deadline)));
        let actions = rig.feed(authenticated(1));
        assert!(actions.iter().any(|a| matches!(a, Action::Arm(None))));
    }

    #[test]
    fn caps_lock_and_the_keyboard_come_from_the_events() {
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::OutputAdded(info(1, "DP-1")));
        rig.feed(Input::Connected);
        rig.feed(Input::Locked);
        assert!(!rig.scene().keyboard, "the seat has not reported one");
        rig.feed(Input::Keyboard { present: true });
        assert!(rig.scene().keyboard);
        rig.feed(Input::Keyboard { present: false });
        assert!(!rig.scene().keyboard);
        let caps = InputEvent::Key {
            keysym: 0xffe5,
            utf8: None,
            modifiers: Modifiers {
                caps_lock: true,
                ..Modifiers::default()
            },
            pressed: false,
            serial: 0,
        };
        rig.feed(Input::Input {
            output: OutputId(1),
            event: caps,
        });
        assert!(rig.scene().caps_lock, "any key event carries the state");
        rig.press(u32::from('a'), Some("a"));
        assert!(!rig.scene().caps_lock);
    }

    #[test]
    fn caps_lock_latched_before_any_key_is_shown_from_the_modifiers_event() {
        // Finding M2: the compositor reports the modifier state when the
        // keyboard enters; nothing was typed yet.
        let mut rig = Rig::new(Config::new(false, false, 0));
        rig.feed(Input::OutputAdded(info(1, "DP-1")));
        rig.feed(Input::Connected);
        rig.feed(Input::Locked);
        assert!(!rig.scene().caps_lock);
        let latched = Modifiers {
            caps_lock: true,
            ..Modifiers::default()
        };
        rig.feed(Input::Input {
            output: OutputId(1),
            event: InputEvent::Modifiers(latched),
        });
        assert!(rig.scene().caps_lock);
        rig.feed(Input::Input {
            output: OutputId(1),
            event: InputEvent::Modifiers(Modifiers::default()),
        });
        assert!(!rig.scene().caps_lock);
    }

    #[test]
    fn the_secret_never_shows_in_a_debug_of_an_action_or_an_input() {
        let mut rig = Rig::locked();
        rig.type_text("hunter2");
        let actions = rig.enter();
        let text = format!("{actions:?}");
        assert!(!text.contains("hunter2"), "{text}");
        assert!(text.contains("Secret(..)"), "{text}");
        let key = Input::Input {
            output: OutputId(1),
            event: Rig::key_event(u32::from('h'), Some("h")),
        };
        let text = format!("{key:?}");
        assert!(text.contains("redacted"), "{text}");
    }
}
