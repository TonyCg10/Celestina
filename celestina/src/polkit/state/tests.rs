//! The prompt state against fakes of its three boundaries: the agent (over the
//! real pure [`Queue`], so the queue's own limits are in play), the helper's
//! thread, and the app's host.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;
use std::time::{Duration, Instant};

use celestina_paint::{TextStyle, Theme};
use celestina_shell::polkit::{
    decode_line, Admission, CancelOutcome, Cookie, Ended, Ending, HelperLine, Identity, PamText,
    Promotion, Queue, Reply, Request, RequestId, FOCUS_GRACE, FOCUS_GUARD, IDLE_TIMEOUT,
    KEYBOARD_PROOF, REGRAB_GAP,
};
use celestina_ui::{Secret, SecretFieldState, ViewKind};
use celestina_wayland::{
    Anchoring, InputEvent, KeyboardInteractivity, Layer, LayerSpec, Modifiers, OutputId,
    OutputInfo, RuntimeError, SurfaceId,
};

use super::*;
use crate::polkit::view::{self, Focus};
use crate::providers::polkit::helper::{AnswerRefused, HelperInput};
use crate::providers::Reaper;

const RETURN: u32 = 0xff0d;
const ESCAPE: u32 = 0xff1b;
const TAB: u32 = 0xff09;
const ACCOUNT_LINE: &str = "toni:x:1000:1000::/home/toni:/bin/zsh\n";

// ---- the agent ------------------------------------------------------------------------

struct FakeAgent {
    origin: Instant,
    queue: RefCell<Queue>,
    replies: RefCell<Vec<(RequestId, Reply)>>,
    ends: RefCell<Vec<(Ending, bool)>>,
    /// The request each `end_active` named.
    ended_ids: RefCell<Vec<RequestId>>,
    /// The agent never hears of the lock (the state must not depend on it).
    deaf: Cell<bool>,
    /// polkitd cancels the request right after the agent promoted it.
    race_cancel: Cell<bool>,
}

impl FakeAgent {
    fn new(origin: Instant) -> Rc<FakeAgent> {
        let mut queue = Queue::new();
        queue.set_duty(true);
        Rc::new(FakeAgent {
            origin,
            queue: RefCell::new(queue),
            replies: RefCell::new(Vec::new()),
            ends: RefCell::new(Vec::new()),
            ended_ids: RefCell::new(Vec::new()),
            deaf: Cell::new(false),
            race_cancel: Cell::new(false),
        })
    }

    fn moment(&self, at: Instant) -> Duration {
        at.saturating_duration_since(self.origin)
    }

    /// What `BeginAuthentication` does at the agent: admit or refuse.
    fn admit(&self, request: Request, now: Instant) -> Option<PolkitEvent> {
        let id = request.id();
        match self.queue.borrow_mut().admit(request, self.moment(now)) {
            Admission::Accepted => Some(PolkitEvent::Admitted(id)),
            Admission::Refused(refusal) => {
                self.replies.borrow_mut().push((id, refusal.reply()));
                None
            }
        }
    }

    /// What `CancelAuthentication` does at the agent.
    fn cancel(&self, cookie: &str, now: Instant) -> Option<PolkitEvent> {
        let cookie = Cookie::new(cookie).ok()?;
        let outcome = self.queue.borrow_mut().cancel(&cookie, self.moment(now));
        match outcome {
            CancelOutcome::Unknown => None,
            CancelOutcome::Waiting(id) => {
                self.replies.borrow_mut().push((id, Reply::Cancelled));
                None
            }
            CancelOutcome::Active(Ended { id, reply, .. }) => {
                if let Some(reply) = reply {
                    self.replies.borrow_mut().push((id, reply));
                }
                Some(PolkitEvent::Ended {
                    id,
                    ending: Ending::CancelledByPolkit,
                })
            }
        }
    }

    fn replies(&self) -> Vec<(RequestId, Reply)> {
        self.replies.borrow().clone()
    }

    fn ends(&self) -> Vec<(Ending, bool)> {
        self.ends.borrow().clone()
    }

    fn ended_ids(&self) -> Vec<RequestId> {
        self.ended_ids.borrow().clone()
    }
}

impl AgentPort for Rc<FakeAgent> {
    fn set_locked(&self, locked: bool) {
        if !self.deaf.get() {
            self.queue.borrow_mut().set_locked(locked);
        }
    }

    fn promote(&self, now: Instant) -> Promotion {
        let promotion = self.queue.borrow_mut().promote(self.moment(now));
        if let Promotion::Refused { id, refusal } = promotion {
            self.replies.borrow_mut().push((id, refusal.reply()));
        }
        if let (Promotion::Show(_), true) = (&promotion, self.race_cancel.get()) {
            // polkitd's cancel won the race for the table.
            let moment = self.moment(now);
            let _ = self
                .queue
                .borrow_mut()
                .end_active(Ending::CancelledByPolkit, false, moment);
        }
        promotion
    }

    fn prompt(&self, id: RequestId) -> Option<Prompt> {
        let queue = self.queue.borrow();
        let request = queue.active().filter(|r| r.id() == id)?;
        let display = request.display();
        Some(Prompt {
            id,
            message: display.message.to_owned(),
            action_id: display.action_id.to_owned(),
            account: display.account.to_owned(),
        })
    }

    fn helper_input(&self, id: RequestId) -> Option<HelperInput> {
        let queue = self.queue.borrow();
        let request = queue.active().filter(|r| r.id() == id)?;
        HelperInput::from_request(request)
    }

    fn end_active(
        &self,
        id: RequestId,
        ending: Ending,
        attempted: bool,
        now: Instant,
    ) -> Option<Ended> {
        self.ends.borrow_mut().push((ending, attempted));
        self.ended_ids.borrow_mut().push(id);
        if self.queue.borrow().active().map(Request::id) != Some(id) {
            return None;
        }
        let ended = self
            .queue
            .borrow_mut()
            .end_active(ending, attempted, self.moment(now))?;
        if let Some(reply) = ended.reply {
            self.replies.borrow_mut().push((ended.id, reply));
        }
        Some(ended)
    }

    fn retire(self: Box<Self>) -> Reaper {
        Reaper::default()
    }
}

// ---- the helper ---------------------------------------------------------------------------

#[derive(Default)]
struct HelperLog {
    spawned: Vec<(RequestId, String)>,
    answers: Vec<Vec<u8>>,
    cancelled: usize,
}

struct FakeHelpers {
    log: Rc<RefCell<HelperLog>>,
    fail: Rc<Cell<bool>>,
}

impl HelperSpawner for FakeHelpers {
    fn spawn(&self, id: RequestId, input: HelperInput) -> io::Result<Box<dyn HelperHandle>> {
        if self.fail.get() {
            return Err(io::Error::other("no thread"));
        }
        self.log
            .borrow_mut()
            .spawned
            .push((id, input.user().to_owned()));
        Ok(Box::new(FakeHandle(Rc::clone(&self.log))))
    }
}

struct FakeHandle(Rc<RefCell<HelperLog>>);

impl HelperHandle for FakeHandle {
    fn answer(&self, secret: Secret) -> Result<(), AnswerRefused> {
        let mut bytes = Vec::new();
        assert!(secret.write_line_to(&mut bytes).is_ok());
        self.0.borrow_mut().answers.push(bytes);
        Ok(())
    }

    fn cancel(self: Box<Self>) {
        self.0.borrow_mut().cancelled += 1;
    }
}

// ---- the host -------------------------------------------------------------------------------

#[derive(Default)]
struct FakeHost {
    next: u32,
    created: Vec<(SurfaceId, LayerSpec)>,
    destroyed: Vec<SurfaceId>,
    redraws: Vec<SurfaceId>,
    keyboard_closed: usize,
    notices: Vec<Notice>,
    refuse_output: Option<OutputId>,
}

impl PromptHost for FakeHost {
    fn create_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        if self.refuse_output == Some(spec.output) {
            return Err(RuntimeError::UnknownOutput(spec.output));
        }
        self.next += 1;
        let id = SurfaceId(100 + self.next);
        self.created.push((id, spec));
        Ok(id)
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.destroyed.push(id);
    }

    fn request_redraw(&mut self, id: SurfaceId) {
        self.redraws.push(id);
    }

    fn close_keyboard_surfaces(&mut self) {
        self.keyboard_closed += 1;
    }

    fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }
}

// ---- the fixture ----------------------------------------------------------------------------

const SIZE: (f32, f32) = (1280.0, 800.0);

struct Fx {
    state: PolkitState,
    agent: Rc<FakeAgent>,
    log: Rc<RefCell<HelperLog>>,
    spawn_fails: Rc<Cell<bool>>,
    host: FakeHost,
    t0: Instant,
    number: u64,
}

fn pam(line: &str) -> PamText {
    match decode_line(line.as_bytes()) {
        Ok(HelperLine::PromptSecret(text) | HelperLine::Error(text) | HelperLine::Info(text)) => {
            text
        }
        other => panic!("not a text line: {other:?}"),
    }
}

fn info(id: u32, name: &str) -> OutputInfo {
    OutputInfo {
        id: OutputId(id),
        name: name.to_owned(),
        logical_size: (2560, 1440),
        scale: 1.5,
        refresh_mhz: 60_000,
    }
}

fn fake_measure(s: &str, _: &TextStyle, max: Option<f32>) -> (f32, f32) {
    let width = 7.0 * s.chars().count() as f32;
    match max {
        Some(max) if width > max => (max, 16.0 * (width / max).ceil()),
        _ => (width, 16.0),
    }
}

impl Fx {
    fn new() -> Fx {
        let t0 = Instant::now();
        let agent = FakeAgent::new(t0);
        let log = Rc::new(RefCell::new(HelperLog::default()));
        let spawn_fails = Rc::new(Cell::new(false));
        let mut state = PolkitState::new(
            Box::new(Rc::clone(&agent)),
            Box::new(FakeHelpers {
                log: Rc::clone(&log),
                fail: Rc::clone(&spawn_fails),
            }),
            t0,
            Theme::dark(),
            false,
        );
        let mut host = FakeHost::default();
        state.output_added(&info(1, "DP-1"), &mut host);
        state.output_added(&info(2, "HDMI-A-1"), &mut host);
        Fx {
            state,
            agent,
            log,
            spawn_fails,
            host,
            t0,
            number: 0,
        }
    }

    fn at(&self, ms: u64) -> Instant {
        self.t0 + Duration::from_millis(ms)
    }

    fn ctx(&self, ms: u64) -> Ctx<'static> {
        Ctx {
            now: self.at(ms),
            focused_output: Some("HDMI-A-1"),
        }
    }

    fn request(&mut self, cookie: &str, message: &str) -> Request {
        self.number += 1;
        match Request::new(
            RequestId::new(self.number),
            cookie,
            "org.example.Run_Thing",
            message,
            &[Identity::User(1000)],
            1000,
            ACCOUNT_LINE,
        ) {
            Ok(request) => request,
            Err(refusal) => panic!("a valid request was refused: {refusal:?}"),
        }
    }

    /// `BeginAuthentication` for `cookie` at `ms`.
    fn admit(&mut self, ms: u64, cookie: &str, message: &str) -> RequestId {
        let request = self.request(cookie, message);
        let id = request.id();
        if let Some(event) = self.agent.admit(request, self.at(ms)) {
            let ctx = self.ctx(ms);
            self.state.on_event(event, &ctx, &mut self.host);
        }
        id
    }

    /// A request shown at 0 ms whose card committed at 10 ms.
    fn show(&mut self) -> RequestId {
        let id = self.admit(0, "cookie-1", "Run a program as root");
        self.commit(10);
        id
    }

    fn commit(&mut self, ms: u64) {
        let card = self.card();
        self.state.surface_committed(card, SIZE, self.at(ms));
    }

    fn card(&self) -> SurfaceId {
        match self.state.card_surface() {
            Some(card) => card,
            None => panic!("no prompt is up"),
        }
    }

    fn enter(&mut self, serial: u32, ms: u64) -> InputOutcome {
        let card = self.card();
        let ev = InputEvent::FocusEnter {
            serial,
            at: self.at(ms),
        };
        self.state.input(card, ev, self.at(ms))
    }

    fn leave(&mut self, ms: u64) -> InputOutcome {
        let card = self.card();
        self.state.input(card, InputEvent::FocusLost, self.at(ms))
    }

    fn key(&mut self, serial: u32, keysym: u32, text: Option<&str>, ms: u64) -> InputOutcome {
        let card = self.card();
        let ev = InputEvent::Key {
            keysym,
            utf8: text.map(str::to_owned),
            modifiers: Modifiers::default(),
            pressed: true,
            serial,
        };
        self.state.input(card, ev, self.at(ms))
    }

    fn typed(&mut self, serial: u32, text: &str, ms: u64) {
        let mut serial = serial;
        for c in text.chars() {
            let s = c.to_string();
            self.key(serial, u32::from(c), Some(&s), ms);
            serial = serial.wrapping_add(1);
        }
    }

    fn tick(&mut self, ms: u64) {
        self.state.tick(self.at(ms));
    }

    fn settle(&mut self, ms: u64) {
        let ctx = self.ctx(ms);
        self.state.settle(&ctx, &mut self.host);
    }

    fn helper(&mut self, id: RequestId, event: HelperEvent, ms: u64) {
        let ctx = self.ctx(ms);
        self.state.on_helper(id, event, &ctx, &mut self.host);
    }

    /// The card has the keyboard (enter at 100 ms, serial 10), the guard
    /// window has passed, the helper is open and PAM asked at 610 ms.
    fn ready(&mut self) -> RequestId {
        let id = self.show();
        self.enter(10, 100);
        self.tick(100 + FOCUS_GUARD.as_millis() as u64);
        self.helper(
            id,
            HelperEvent::Prompt(pam("PAM_PROMPT_ECHO_OFF Password: ")),
            610,
        );
        id
    }

    fn answers(&self) -> Vec<Vec<u8>> {
        self.log.borrow().answers.clone()
    }

    fn spawned(&self) -> usize {
        self.log.borrow().spawned.len()
    }

    fn is_up(&self) -> bool {
        self.state.card_surface().is_some()
    }

    fn texts(&self) -> Vec<String> {
        let Some(active) = self.state.active.as_ref() else {
            return Vec::new();
        };
        let view = view::card_surface(&Theme::dark(), SIZE, &active.model());
        let mut out = Vec::new();
        collect(&view, &mut out);
        out
    }

    fn card_view(&self) -> celestina_ui::View {
        let Some(active) = self.state.active.as_ref() else {
            panic!("no prompt is up");
        };
        view::card_surface(&Theme::dark(), SIZE, &active.model())
    }

    fn lay_out_card(&mut self, ms: u64) {
        let card = self.card();
        let theme = Theme::dark();
        assert!(self
            .state
            .lay_out(card, SIZE, &theme, self.at(ms), fake_measure));
    }

    fn exclusive_surfaces(&self) -> usize {
        self.host
            .created
            .iter()
            .filter(|(_, spec)| spec.keyboard == KeyboardInteractivity::Exclusive)
            .count()
    }
}

fn collect(view: &celestina_ui::View, out: &mut Vec<String>) {
    if let ViewKind::Text { text, .. } = &view.kind {
        out.push(text.clone());
    }
    for child in &view.children {
        collect(child, out);
    }
}

fn spec_of(host: &FakeHost, surface: SurfaceId) -> LayerSpec {
    match host.created.iter().find(|(id, _)| *id == surface) {
        Some((_, spec)) => spec.clone(),
        None => panic!("surface {surface:?} was never created"),
    }
}

// ---- activation and surfaces ----------------------------------------------------------------------

#[test]
fn the_card_goes_on_the_focused_output_with_the_exclusive_keyboard_and_the_one_namespace() {
    let mut fx = Fx::new();
    fx.show();
    let spec = spec_of(&fx.host, fx.card());
    assert_eq!(
        spec,
        LayerSpec {
            output: OutputId(2),
            namespace: "celestina-polkit-prompt",
            layer: Layer::Overlay,
            anchor: Anchoring::Fill,
            size: (0, 0),
            exclusive_zone: -1,
            keyboard: KeyboardInteractivity::Exclusive,
        }
    );
    assert_eq!(NAMESPACE, "celestina-polkit-prompt", "the niri rule's name");
    assert_eq!(fx.exclusive_surfaces(), 1);
}

#[test]
fn without_a_focused_output_the_first_one_takes_the_card() {
    let mut fx = Fx::new();
    let request = fx.request("cookie-1", "m");
    let id = request.id();
    let event = fx.agent.admit(request, fx.at(0));
    assert_eq!(event, Some(PolkitEvent::Admitted(id)));
    let ctx = Ctx {
        now: fx.at(0),
        focused_output: Some("DP-9"),
    };
    fx.state
        .on_event(PolkitEvent::Admitted(id), &ctx, &mut fx.host);
    assert_eq!(spec_of(&fx.host, fx.card()).output, OutputId(1));
}

#[test]
fn other_outputs_get_dimming_surfaces_without_the_keyboard() {
    let mut fx = Fx::new();
    fx.show();
    assert_eq!(fx.host.created.len(), 2);
    let (dim, spec) = &fx.host.created[1];
    assert_ne!(*dim, fx.card());
    assert_eq!(spec.output, OutputId(1));
    assert_eq!(spec.keyboard, KeyboardInteractivity::None);
    assert_eq!(spec.namespace, NAMESPACE);
    assert_eq!(spec.layer, Layer::Overlay);
    assert_eq!(spec.anchor, Anchoring::Fill);
    assert_eq!(spec.exclusive_zone, -1);
    // A press on a dim changes nothing.
    let outcome = fx.state.input(
        *dim,
        InputEvent::PointerPress {
            x: 5.0,
            y: 5.0,
            button: 0x110,
            serial: 3,
        },
        fx.at(50),
    );
    assert_eq!(outcome, InputOutcome::default());
    assert!(fx.is_up());
}

#[test]
fn a_new_output_gets_a_dimming_surface() {
    let mut fx = Fx::new();
    fx.show();
    let before = fx.host.created.len();
    fx.state.output_added(&info(3, "eDP-1"), &mut fx.host);
    assert_eq!(fx.host.created.len(), before + 1);
    let (_, spec) = &fx.host.created[before];
    assert_eq!(spec.output, OutputId(3));
    assert_eq!(spec.keyboard, KeyboardInteractivity::None);
    // Its removal ends nothing: it is only a dim.
    fx.state.output_removed(OutputId(3), fx.at(60));
    assert!(fx.is_up());
    assert!(!fx.state.needs_settle(fx.at(60)));
    // And an output that appears with no prompt up is only remembered.
    let mut idle = Fx::new();
    idle.state.output_added(&info(4, "DP-2"), &mut idle.host);
    assert!(idle.host.created.is_empty());
}

#[test]
fn other_keyboard_surfaces_are_closed_when_a_prompt_activates_and_refused_until_it_ends() {
    let mut fx = Fx::new();
    assert!(!fx.state.keyboard_claimed());
    fx.ready();
    assert_eq!(fx.host.keyboard_closed, 1, "closed once, before the card");
    assert!(fx.state.keyboard_claimed(), "the app opens none meanwhile");
    // It ends; the surfaces still stand until settled, so still refused.
    let outcome = fx.key(11, ESCAPE, None, 700);
    assert!(outcome.settle);
    assert!(fx.state.keyboard_claimed());
    fx.settle(701);
    assert!(!fx.state.keyboard_claimed(), "free again once it is gone");
}

#[test]
fn locked_session_refuses_without_mapping() {
    let mut fx = Fx::new();
    // Admitted while unlocked, locked before the loop maps it.
    let request = fx.request("cookie-1", "m");
    let id = request.id();
    let event = fx.agent.admit(request, fx.at(0));
    fx.state.set_locked(true, fx.at(1));
    if let Some(event) = event {
        let ctx = fx.ctx(2);
        fx.state.on_event(event, &ctx, &mut fx.host);
    }
    assert!(fx.host.created.is_empty(), "nothing was mapped");
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert!(!fx.state.keyboard_claimed());
    // And a request while locked is refused at the agent, never shown.
    let second = fx.admit(10, "cookie-2", "m");
    assert_eq!(fx.agent.replies().last(), Some(&(second, Reply::Cancelled)));
    assert!(fx.host.created.is_empty());
}

#[test]
fn the_state_fails_closed_on_its_own_when_the_agent_never_heard_of_the_lock() {
    let mut fx = Fx::new();
    fx.agent.deaf.set(true);
    let request = fx.request("cookie-1", "m");
    let id = request.id();
    let event = fx.agent.admit(request, fx.at(0));
    fx.state.set_locked(true, fx.at(1));
    if let Some(event) = event {
        let ctx = fx.ctx(2);
        fx.state.on_event(event, &ctx, &mut fx.host);
    }
    assert!(fx.host.created.is_empty(), "never mapped while locked");
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::NotShown, false)]);
    assert!(fx.host.notices.is_empty());
}

#[test]
fn locking_the_session_ends_a_shown_prompt_at_once() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.state.set_locked(true, fx.at(800));
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.log.borrow().cancelled, 1, "the socket closed");
    fx.settle(801);
    assert_eq!(fx.host.destroyed.len(), 2);
    assert!(fx.host.notices.is_empty(), "no notice over a lock screen");
}

#[test]
fn a_surface_the_runtime_refuses_cancels_the_request_with_a_notice() {
    let mut fx = Fx::new();
    fx.host.refuse_output = Some(OutputId(2));
    let id = fx.admit(0, "cookie-1", "m");
    assert!(fx.host.created.is_empty());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    fx.settle(1);
    assert_eq!(fx.host.notices, vec![Notice::PromptCouldNotBeShown]);
}

// ---- the keyboard proof, F-6 -----------------------------------------------------------------------

#[test]
fn keyboard_proof_failure_cancels_and_raises_the_notice() {
    let mut fx = Fx::new();
    let id = fx.show();
    fx.tick(10 + KEYBOARD_PROOF.as_millis() as u64 - 1);
    assert!(fx.is_up(), "inside the proof");
    fx.tick(10 + KEYBOARD_PROOF.as_millis() as u64);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::PromptUnavailable, false)]);
    assert_eq!(fx.spawned(), 0, "no helper connection was ever opened");
    fx.settle(700);
    assert_eq!(fx.host.destroyed.len(), 2, "the card and the dim go");
    assert_eq!(fx.host.notices, vec![Notice::PromptCouldNotBeShown]);
}

#[test]
fn the_compositor_closing_the_cards_output_before_the_first_enter_cancels_without_a_helper() {
    let mut fx = Fx::new();
    let id = fx.show();
    let card = fx.card();
    // The runtime destroyed the card's surface with its output.
    fx.state.output_removed(OutputId(2), fx.at(50));
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.spawned(), 0);
    fx.settle(51);
    assert!(
        !fx.host.destroyed.contains(&card),
        "a surface the runtime already destroyed is not destroyed twice"
    );
    assert_eq!(fx.host.destroyed.len(), 1, "only the dim");
    assert_eq!(fx.host.notices, vec![Notice::PromptCouldNotBeShown]);
}

#[test]
fn a_card_the_compositor_never_configures_is_given_up() {
    let mut fx = Fx::new();
    let id = fx.admit(0, "cookie-1", "m");
    fx.tick(MAP_TIMEOUT.as_millis() as u64 - 1);
    assert!(fx.is_up());
    assert_eq!(fx.state.next_wake(fx.at(0)), Some(MAP_TIMEOUT));
    fx.tick(MAP_TIMEOUT.as_millis() as u64);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
}

#[test]
fn the_helper_opens_only_after_the_enter_and_the_guard_window() {
    let mut fx = Fx::new();
    fx.show();
    fx.tick(300);
    assert_eq!(fx.spawned(), 0, "no enter yet");
    fx.enter(10, 100);
    fx.tick(100 + FOCUS_GUARD.as_millis() as u64 - 1);
    assert_eq!(fx.spawned(), 0, "inside the guard window");
    fx.tick(100 + FOCUS_GUARD.as_millis() as u64);
    assert_eq!(fx.spawned(), 1);
    assert_eq!(fx.log.borrow().spawned[0].1, "toni");
    fx.tick(2_000);
    assert_eq!(fx.spawned(), 1, "once");
}

#[test]
fn an_unreachable_helper_ends_the_request_failed() {
    let mut fx = Fx::new();
    fx.spawn_fails.set(true);
    let id = fx.show();
    fx.enter(10, 100);
    fx.tick(500);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Failed)]);
    // F-11: the card closes with a problem notice (a banner takes no keyboard).
    fx.settle(500);
    assert_eq!(fx.host.notices, vec![Notice::HelperUnavailable]);
}

#[test]
fn a_helper_socket_that_is_not_trusted_ends_the_request_failed_with_the_notice() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.helper(id, HelperEvent::Ended(Ending::HelperUnreachable), 700);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Failed)]);
    assert_eq!(fx.host.notices, vec![Notice::HelperUnavailable]);
}

/// The warning that a request was cancelled unshown is the one a client that
/// suppressed the prompt must not be able to hide, so it is raised when there
/// was something to say: not when polkitd had cancelled the request first.
#[test]
fn no_notice_is_raised_for_a_request_polkitd_had_already_cancelled() {
    // Cancelled between the card being mapped and the loop giving it up.
    let mut fx = Fx::new();
    fx.admit(0, "cookie-1", "m");
    assert!(fx.agent.cancel("cookie-1", fx.at(100)).is_some());
    fx.tick(MAP_TIMEOUT.as_millis() as u64);
    assert!(!fx.is_up());
    assert!(fx.host.notices.is_empty(), "{:?}", fx.host.notices);

    // Cancelled between the agent promoting it and the loop asking for its text.
    let mut fx = Fx::new();
    fx.agent.race_cancel.set(true);
    fx.admit(0, "cookie-1", "m");
    assert!(!fx.is_up());
    assert!(fx.host.notices.is_empty(), "{:?}", fx.host.notices);
}

// ---- the guard window and the serials -----------------------------------------------------------------

#[test]
fn keys_inside_the_guard_window_are_ignored() {
    let mut fx = Fx::new();
    fx.ready();
    // The keyboard goes and comes back: a new guard window opens with asking
    // still true, so only the window can drop what is typed in it.
    fx.leave(700);
    fx.enter(30, 800);
    fx.typed(31, "q", 900);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    fx.typed(32, "z", 800 + FOCUS_GUARD.as_millis() as u64);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::HasContent));
    fx.key(33, RETURN, None, 1_300);
    assert_eq!(fx.answers(), vec![b"z\n".to_vec()]);
}

#[test]
fn a_key_pressed_before_the_enter_is_ignored() {
    let mut fx = Fx::new();
    fx.ready();
    // Older serials: a key held from before the enter, and one from before the
    // serial counter wrapped.
    fx.typed(9, "a", 700);
    fx.typed(u32::MAX, "c", 700);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    fx.typed(11, "b", 700);
    fx.key(12, RETURN, None, 700);
    assert_eq!(fx.answers(), vec![b"b\n".to_vec()]);
}

#[test]
fn enter_on_an_empty_field_is_ignored() {
    let mut fx = Fx::new();
    fx.ready();
    let outcome = fx.key(11, RETURN, None, 700);
    assert_eq!(outcome, InputOutcome::default());
    assert!(fx.answers().is_empty());
    assert!(fx.is_up());
}

#[test]
fn typing_before_pam_asks_is_dropped() {
    let mut fx = Fx::new();
    let id = fx.show();
    fx.enter(10, 100);
    fx.tick(500);
    fx.typed(11, "early", 600);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    fx.helper(
        id,
        HelperEvent::Prompt(pam("PAM_PROMPT_ECHO_OFF Password: ")),
        650,
    );
    fx.typed(20, "x", 700);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::HasContent));
}

// ---- loss, grace and the idle cap ----------------------------------------------------------------------

#[test]
fn focus_loss_wipes_the_partial_secret_and_cancels_after_the_grace() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "abc", 700);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::HasContent));
    fx.leave(1_000);
    assert_eq!(
        fx.state.field_state(),
        Some(SecretFieldState::Empty),
        "wiped at once"
    );
    let grace = FOCUS_GRACE.as_millis() as u64;
    fx.tick(1_000 + grace - 1);
    assert!(fx.is_up());
    fx.tick(1_000 + grace);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::KeyboardLost, false)]);
    assert_eq!(fx.log.borrow().cancelled, 1, "the socket is closed");
    assert!(fx.answers().is_empty(), "nothing typed was ever sent");
}

#[test]
fn focus_returning_inside_the_grace_accepts_typing_again_with_an_empty_field() {
    let mut fx = Fx::new();
    fx.ready();
    fx.typed(11, "abc", 700);
    fx.leave(1_000);
    fx.enter(40, 1_500);
    fx.tick(1_500 + FOCUS_GRACE.as_millis() as u64 + 500);
    assert!(fx.is_up(), "the grace was kept");
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    fx.typed(41, "z", 1_500 + FOCUS_GUARD.as_millis() as u64);
    fx.key(42, RETURN, None, 2_000);
    assert_eq!(fx.answers(), vec![b"z\n".to_vec()], "abc was not kept");
}

#[test]
fn idle_timeout_is_absolute_and_not_extended_by_typing() {
    let mut fx = Fx::new();
    let id = fx.ready();
    let idle = 10 + IDLE_TIMEOUT.as_millis() as u64;
    // Typing all the way up to the end does not move it.
    for (serial, t) in (11..).zip([30_000, 90_000, idle - 2_000, idle - 2]) {
        fx.typed(serial, "k", t);
        fx.tick(t);
        assert!(fx.is_up(), "still up at {t} ms");
    }
    fx.tick(idle - 1);
    assert!(fx.is_up());
    fx.tick(idle);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::IdleTimeout, false)]);
}

// ---- Escape, clicks, focus -------------------------------------------------------------------------------

#[test]
fn escape_cancels_and_a_click_outside_does_nothing() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.lay_out_card(620);
    let card = fx.card();
    let press = |x, y, serial| InputEvent::PointerPress {
        x,
        y,
        button: 0x110,
        serial,
    };
    let release = |x, y, serial| InputEvent::PointerRelease {
        x,
        y,
        button: 0x110,
        serial,
    };
    // Far outside the card, on the dim.
    let a = fx.state.input(card, press(3.0, 3.0, 5), fx.at(700));
    let b = fx.state.input(card, release(3.0, 3.0, 6), fx.at(701));
    assert_eq!((a.settle, b.settle), (false, false));
    assert!(fx.is_up());
    assert!(fx.agent.replies().is_empty());
    // Escape gives the request up.
    let outcome = fx.key(11, ESCAPE, None, 800);
    assert!(outcome.settle);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::Escaped, false)]);
}

#[test]
fn escape_inside_the_guard_window_is_ignored_like_any_key() {
    let mut fx = Fx::new();
    fx.show();
    fx.enter(10, 100);
    fx.key(11, ESCAPE, None, 200);
    assert!(fx.is_up());
}

fn center_of(fx: &mut Fx, key: celestina_ui::Key) -> (f32, f32) {
    let Some(active) = fx.state.active.as_ref() else {
        panic!("no prompt");
    };
    let tree = &active.surfaces[0].tree;
    let Some(node) = tree.node_by_key(key) else {
        panic!("no node");
    };
    let rect = tree.rect(node);
    (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
}

#[test]
fn clicking_the_cancel_control_cancels_and_clicking_authenticate_sends() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "pw", 700);
    fx.lay_out_card(720);
    let card = fx.card();
    let (x, y) = center_of(&mut fx, view::authenticate_key());
    let press = InputEvent::PointerPress {
        x,
        y,
        button: 0x110,
        serial: 70,
    };
    let release = InputEvent::PointerRelease {
        x,
        y,
        button: 0x110,
        serial: 71,
    };
    fx.state.input(card, press, fx.at(800));
    fx.state.input(card, release, fx.at(801));
    assert_eq!(fx.answers(), vec![b"pw\n".to_vec()]);
    assert!(fx.is_up());

    let (x, y) = center_of(&mut fx, view::cancel_key());
    fx.state.input(
        card,
        InputEvent::PointerPress {
            x,
            y,
            button: 0x110,
            serial: 72,
        },
        fx.at(900),
    );
    let outcome = fx.state.input(
        card,
        InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 73,
        },
        fx.at(901),
    );
    assert!(outcome.settle);
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::Escaped, true)]);
}

#[test]
fn tab_walks_the_controls_and_enter_acts_on_the_focused_one() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "pw", 700);
    fx.key(13, TAB, None, 710);
    assert_eq!(
        fx.state.active.as_ref().map(|a| a.focus),
        Some(Focus::Authenticate)
    );
    // Typing does not reach the field from a control.
    fx.typed(14, "x", 720);
    fx.key(15, RETURN, None, 730);
    assert_eq!(fx.answers(), vec![b"pw\n".to_vec()], "the control sent it");
    fx.key(16, TAB, None, 740);
    assert_eq!(
        fx.state.active.as_ref().map(|a| a.focus),
        Some(Focus::Cancel)
    );
    let outcome = fx.key(17, RETURN, None, 750);
    assert!(outcome.settle);
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
}

#[test]
fn enter_on_the_authenticate_control_with_nothing_typed_sends_nothing() {
    let mut fx = Fx::new();
    fx.ready();
    fx.key(11, TAB, None, 700);
    fx.key(12, RETURN, None, 710);
    assert!(fx.answers().is_empty());
    assert!(fx.is_up());
}

// ---- the conversation ---------------------------------------------------------------------------------------

#[test]
fn a_submitted_answer_is_moved_to_the_helper_and_the_card_keeps_nothing() {
    let mut fx = Fx::new();
    fx.ready();
    fx.typed(11, "hunter2", 700);
    fx.key(18, RETURN, None, 710);
    assert_eq!(fx.answers(), vec![b"hunter2\n".to_vec()]);
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    assert_eq!(
        fx.state.active.as_ref().map(|a| a.phase),
        Some(Phase::Checking)
    );
    // While it is with the helper, nothing is taken.
    fx.typed(30, "again", 800);
    fx.key(40, RETURN, None, 810);
    assert_eq!(fx.answers().len(), 1);
}

#[test]
fn a_wrong_answer_shows_the_attempts_left_and_asks_again() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "bad", 700);
    fx.key(14, RETURN, None, 710);
    fx.helper(id, HelperEvent::Failure, 800);
    let shown = fx.texts();
    assert!(shown.contains(&copy::polkit_attempts_left(1)), "{shown:?}");
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
    // The next conversation asks again; the card takes a second answer.
    fx.helper(
        id,
        HelperEvent::Prompt(pam("PAM_PROMPT_ECHO_OFF Password: ")),
        850,
    );
    fx.typed(20, "good", 900);
    fx.key(24, RETURN, None, 910);
    assert_eq!(fx.answers(), vec![b"bad\n".to_vec(), b"good\n".to_vec()]);
    // The second wrong answer ends it: attempted, so not a dismissal.
    fx.helper(id, HelperEvent::Ended(Ending::AttemptsExhausted), 1_000);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::AttemptsExhausted, true)]);
}

#[test]
fn only_the_helpers_success_ends_with_the_normal_reply() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "pw", 700);
    fx.key(13, RETURN, None, 710);
    fx.helper(id, HelperEvent::Ended(Ending::Authenticated), 800);
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Normal)]);
    fx.settle(801);
    assert_eq!(fx.host.destroyed.len(), 2);
    assert!(fx.host.notices.is_empty());
}

#[test]
fn pam_text_is_shown_sanitised_and_a_stale_conversation_is_ignored() {
    let mut fx = Fx::new();
    let id = fx.show();
    fx.enter(10, 100);
    fx.tick(500);
    // ESC, NUL and a right-to-left override arrive as octal escapes.
    fx.helper(
        id,
        HelperEvent::Prompt(pam(
            r"PAM_PROMPT_ECHO_OFF \033[31mPass\000word\342\200\256:",
        )),
        600,
    );
    fx.helper(
        id,
        HelperEvent::Notice(pam(r"PAM_TEXT_INFO Touch\nthe key")),
        601,
    );
    let shown = fx.texts().join("|");
    assert!(
        !shown.chars().any(|c| c.is_control() || c == '\u{202e}'),
        "{shown:?}"
    );
    assert!(shown.contains("Password"), "{shown:?}");
    // Another request's line changes nothing.
    fx.helper(
        RequestId::new(99),
        HelperEvent::Ended(Ending::Authenticated),
        602,
    );
    assert!(fx.is_up());
    assert!(fx.agent.replies().is_empty());
}

// ---- the card's text --------------------------------------------------------------------------------------------

#[test]
fn the_card_text_comes_only_from_the_sanitised_request() {
    let mut fx = Fx::new();
    let request = fx.request("cookie-1", "Run\nthis\u{202e} now \u{1b}[31m as root");
    let message = request.display().message.to_owned();
    let action = request.display().action_id.to_owned();
    let id = request.id();
    if let Some(event) = fx.agent.admit(request, fx.at(0)) {
        let ctx = fx.ctx(0);
        fx.state.on_event(event, &ctx, &mut fx.host);
    }
    assert!(fx.is_up());
    assert!(!message.chars().any(|c| c.is_control() || c == '\u{202e}'));
    let mut allowed = vec![
        copy::POLKIT_TITLE.to_owned(),
        copy::POLKIT_ACTION_LABEL.to_owned(),
        copy::POLKIT_ACCOUNT_LABEL.to_owned(),
        copy::POLKIT_CANCEL.to_owned(),
        copy::POLKIT_AUTHENTICATE.to_owned(),
        copy::POLKIT_PREPARING.to_owned(),
        message,
        action,
        "toni".to_owned(),
    ];
    allowed.sort();
    let mut shown = fx.texts();
    shown.sort();
    assert_eq!(shown, allowed, "request {id:?}");
}

#[test]
fn the_card_shows_no_per_character_glyphs() {
    let mut fx = Fx::new();
    fx.ready();
    fx.typed(11, "\u{d1}", 700);
    let one = fx.card_view();
    fx.typed(12, &"\u{d1}".repeat(40), 710);
    let many = fx.card_view();
    assert_eq!(one, many, "one character and forty look the same");
    assert!(!fx.texts().iter().any(|t| t.contains('\u{d1}')));
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::HasContent));
}

// ---- the queue --------------------------------------------------------------------------------------------------------

#[test]
fn a_cancel_for_a_queued_request_removes_it_unshown_and_replies_once() {
    let mut fx = Fx::new();
    let a = fx.ready();
    let before = fx.host.created.len();
    let b = fx.admit(800, "cookie-2", "second");
    assert_eq!(fx.host.created.len(), before, "waiting, not shown");
    assert!(fx.agent.cancel("cookie-2", fx.at(900)).is_none());
    assert_eq!(fx.agent.replies(), vec![(b, Reply::Cancelled)]);
    // The first ends; the second was gone and is never shown.
    fx.key(11, ESCAPE, None, 1_000);
    fx.settle(1_001);
    fx.settle(1_001 + REGRAB_GAP.as_millis() as u64 + 1);
    assert_eq!(fx.host.created.len(), before);
    let replies = fx.agent.replies();
    assert_eq!(replies.iter().filter(|(id, _)| *id == b).count(), 1);
    assert_eq!(replies.iter().filter(|(id, _)| *id == a).count(), 1);
}

#[test]
fn the_next_request_waits_for_the_regrab_gap_and_gets_its_own_surfaces() {
    let mut fx = Fx::new();
    fx.ready();
    let b = fx.admit(800, "cookie-2", "second");
    fx.key(11, ESCAPE, None, 1_000);
    fx.settle(1_000);
    assert!(!fx.is_up(), "not before the gap");
    let gap = REGRAB_GAP.as_millis() as u64;
    assert!(!fx.state.needs_settle(fx.at(1_000 + gap - 1)));
    assert_eq!(
        fx.state.next_wake(fx.at(1_000)),
        Some(Duration::from_millis(gap))
    );
    assert!(fx.state.needs_settle(fx.at(1_000 + gap)));
    let before = fx.host.created.len();
    fx.settle(1_000 + gap);
    assert!(fx.is_up());
    assert_eq!(fx.host.created.len(), before + 2);
    assert_eq!(fx.host.keyboard_closed, 2);
    assert_eq!(fx.agent.ends().len(), 1);
    let _ = b;
}

#[test]
fn the_focused_output_changing_while_a_prompt_is_up_moves_nothing() {
    let mut fx = Fx::new();
    fx.ready();
    let card = fx.card();
    // niri now says DP-1 is focused, and a second request arrives.
    let ctx = Ctx {
        now: fx.at(800),
        focused_output: Some("DP-1"),
    };
    fx.state.settle(&ctx, &mut fx.host);
    let request = fx.request("cookie-2", "second");
    if let Some(event) = fx.agent.admit(request, fx.at(810)) {
        let ctx = Ctx {
            now: fx.at(810),
            focused_output: Some("DP-1"),
        };
        fx.state.on_event(event, &ctx, &mut fx.host);
    }
    assert_eq!(fx.card(), card, "the card stays where it is");
    assert_eq!(fx.exclusive_surfaces(), 1, "no second keyboard surface");
    assert_eq!(spec_of(&fx.host, card).output, OutputId(2));
}

#[test]
fn three_dismissals_in_a_row_raise_the_cooldown_notice() {
    let mut fx = Fx::new();
    let mut ms = 0;
    for round in 0..3u32 {
        let id = fx.admit(ms, &format!("cookie-{round}"), "m");
        fx.commit(ms + 10);
        fx.enter(10 + round, ms + 100);
        fx.tick(ms + 600);
        fx.key(20 + round, ESCAPE, None, ms + 700);
        fx.settle(ms + 700);
        assert!(fx.agent.replies().contains(&(id, Reply::Cancelled)));
        ms += 5_000;
    }
    assert_eq!(fx.host.notices, vec![Notice::CooldownStarted]);
    // During the cool-down a request is refused at the agent and never shown.
    let created = fx.host.created.len();
    let refused = fx.admit(ms, "cookie-late", "m");
    assert_eq!(
        fx.agent.replies().last(),
        Some(&(refused, Reply::Cancelled))
    );
    assert_eq!(fx.host.created.len(), created);
}

#[test]
fn a_cancel_from_polkit_for_the_active_request_closes_the_prompt() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "abc", 700);
    let event = fx.agent.cancel("cookie-1", fx.at(800));
    let Some(event) = event else {
        panic!("the active request ends");
    };
    let ctx = fx.ctx(800);
    fx.state.on_event(event, &ctx, &mut fx.host);
    assert!(!fx.is_up());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.log.borrow().cancelled, 1);
    assert_eq!(fx.host.destroyed.len(), 2);
}

#[test]
fn text_input_and_the_clipboard_never_reach_the_field() {
    let mut fx = Fx::new();
    fx.ready();
    let card = fx.card();
    for ev in [
        InputEvent::Preedit {
            text: "ni".to_owned(),
            cursor: 1,
        },
        InputEvent::CommitText("nihao".to_owned()),
        InputEvent::Paste("from the clipboard".to_owned()),
    ] {
        fx.state.input(card, ev, fx.at(700));
    }
    assert_eq!(fx.state.field_state(), Some(SecretFieldState::Empty));
}

#[test]
fn the_wake_up_follows_the_prompts_deadlines() {
    let mut fx = Fx::new();
    assert_eq!(fx.state.next_wake(fx.at(0)), None);
    fx.show();
    let proof = KEYBOARD_PROOF;
    assert_eq!(fx.state.next_wake(fx.at(10)), Some(proof));
    fx.enter(10, 100);
    assert_eq!(fx.state.next_wake(fx.at(100)), Some(FOCUS_GUARD));
}

// ---- the duty's end: teardown, and the non-dumpable window --------------------------------------

/// A dumpable flag in a cell, for the window the state owns.
#[derive(Clone)]
struct CellFlag(Rc<Cell<rustix::process::DumpableBehavior>>);

impl crate::polkit::dumpable::Dumpable for CellFlag {
    fn current(&mut self) -> io::Result<rustix::process::DumpableBehavior> {
        Ok(self.0.get())
    }

    fn set(&mut self, behavior: rustix::process::DumpableBehavior) -> io::Result<()> {
        self.0.set(behavior);
        Ok(())
    }
}

fn windowed(fx: &mut Fx) -> Rc<Cell<rustix::process::DumpableBehavior>> {
    let flag = Rc::new(Cell::new(rustix::process::DumpableBehavior::Dumpable));
    fx.state
        .set_dumpable(crate::polkit::dumpable::DumpableWindow::over(Box::new(
            CellFlag(Rc::clone(&flag)),
        )));
    flag
}

#[test]
fn switching_off_with_an_answer_in_flight_leaves_a_dropped_answer_and_a_cancelled_reply() {
    let mut fx = Fx::new();
    let id = fx.ready();
    fx.typed(11, "hunter2", 700);
    fx.key(18, RETURN, None, 710);
    assert_eq!(fx.answers().len(), 1, "the answer is with the helper");
    assert_eq!(fx.log.borrow().cancelled, 0);

    let card = fx.card();
    let mut host = std::mem::take(&mut fx.host);
    fx.state.shut_down(fx.at(800), &mut host);
    fx.host = host;

    // The conversation is closed (its socket with it), the card holds nothing,
    // the reply is Cancelled, and every surface is gone.
    assert_eq!(fx.log.borrow().cancelled, 1, "the helper was cancelled");
    assert_eq!(fx.state.field_state(), None);
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends(), vec![(Ending::ShellExit, true)]);
    assert!(fx.host.destroyed.contains(&card));
    assert!(!fx.state.keyboard_claimed());

    // The helper's stale line that may still arrive is ignored.
    fx.helper(id, HelperEvent::Ended(Ending::Authenticated), 810);
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    assert_eq!(fx.agent.ends().len(), 1);
}

#[test]
fn shutting_down_cancels_the_helper_the_dims_and_the_card_and_raises_no_notice() {
    let mut fx = Fx::new();
    let id = fx.ready();
    let created: Vec<SurfaceId> = fx.host.created.iter().map(|(s, _)| *s).collect();
    assert!(created.len() >= 2, "a card and a dim");
    let mut host = std::mem::take(&mut fx.host);
    fx.state.shut_down(fx.at(800), &mut host);
    fx.host = host;
    assert_eq!(fx.log.borrow().cancelled, 1);
    for surface in &created {
        assert!(fx.host.destroyed.contains(surface), "{surface:?}");
    }
    assert!(fx.host.notices.is_empty());
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    // Idempotent: a second shut down finds nothing.
    let mut host = std::mem::take(&mut fx.host);
    fx.state.shut_down(fx.at(900), &mut host);
    fx.host = host;
    assert_eq!(fx.agent.ends().len(), 1);
}

#[test]
fn dropping_the_state_with_a_prompt_up_cancels_the_helper() {
    let mut fx = Fx::new();
    fx.ready();
    fx.typed(11, "secret-in-the-field", 700);
    let log = Rc::clone(&fx.log);
    drop(fx);
    assert_eq!(log.borrow().cancelled, 1);
}

#[test]
fn the_state_opens_the_dumpable_window_while_a_prompt_is_up_and_restores_it() {
    use rustix::process::DumpableBehavior::{Dumpable, NotDumpable};
    let mut fx = Fx::new();
    let flag = windowed(&mut fx);
    assert_eq!(flag.get(), Dumpable);
    let id = fx.ready();
    assert_eq!(flag.get(), NotDumpable, "a request is active");
    fx.helper(id, HelperEvent::Ended(Ending::HelperProtocol), 800);
    assert_eq!(flag.get(), Dumpable, "the last request ended");

    // A shut down and a drop close it too.
    let mut fx = Fx::new();
    let flag = windowed(&mut fx);
    fx.show();
    assert_eq!(flag.get(), NotDumpable);
    let mut host = std::mem::take(&mut fx.host);
    fx.state.shut_down(fx.at(100), &mut host);
    assert_eq!(flag.get(), Dumpable);

    let mut fx = Fx::new();
    let flag = windowed(&mut fx);
    fx.show();
    drop(fx);
    assert_eq!(flag.get(), Dumpable);
}

#[test]
fn every_end_names_the_request_it_ends() {
    let mut fx = Fx::new();
    let first = fx.ready();
    fx.key(11, ESCAPE, None, 800);
    assert_eq!(fx.agent.ended_ids(), vec![first]);
    // The next request is its own: an ending of the first one never reaches it.
    let second = fx.admit(5_000, "cookie-2", "Another program");
    assert_ne!(first, second);
    fx.settle(5_000);
    assert!(fx.is_up(), "the second request is shown");
    fx.helper(first, HelperEvent::Ended(Ending::Authenticated), 5_100);
    assert!(
        fx.is_up(),
        "a stale conversation ends nothing of the second"
    );
    assert_eq!(fx.agent.ended_ids(), vec![first]);
}

#[test]
fn a_handed_off_agent_is_never_asked_again() {
    let mut fx = Fx::new();
    let id = fx.ready();
    let mut host = std::mem::take(&mut fx.host);
    fx.state.shut_down(fx.at(800), &mut host);
    fx.host = host;
    assert_eq!(fx.agent.replies(), vec![(id, Reply::Cancelled)]);
    // Returns at once with the (here empty) reaper of the agent.
    let reaper = fx.state.hand_off_agent();
    reaper.wait();
    // A request the old agent still holds is not promoted by this state.
    let waiting = fx.request("cookie-late", "late");
    assert!(fx.agent.admit(waiting, fx.at(10_000)).is_some());
    fx.settle(10_000);
    assert!(!fx.is_up(), "nothing asks the old agent for a prompt");
    fx.state.set_locked(true, fx.at(10_100));
    assert_eq!(fx.agent.ends().len(), 1);
}
