//! The loop's state of the polkit prompt (CEL-2-I; design sections 4.3, 5 and
//! 6.3 to 6.5): the one active request, its surfaces, the keyboard proof, the
//! guard and loss rules, and the card's input.
//!
//! The state is the glue between three owners and decides nothing they own:
//! the agent ([`AgentPort`]) owns the request table, the queue's limits and
//! every reply to polkitd; the pure domain ([`PromptClock`]) owns the
//! prompt's deadlines; the helper's thread ([`HelperHandle`]) owns the socket.
//! What is decided here is only what the person can do at the card, and when:
//!
//! - **No helper before the keyboard is proven.** The conversation opens
//!   only once the card's surface has received `wl_keyboard.enter` and the
//!   guard window after it has passed (N-24).
//! - **No key from before the enter.** A key whose serial is not newer than
//!   the enter's began in another client; a key inside the guard window, or
//!   while the keyboard is away, is dropped; an Enter on an empty field is
//!   dropped (N-8).
//! - **Loss wipes.** `wl_keyboard.leave` wipes the partial answer at once;
//!   typing resumes, on an empty field, if the keyboard returns inside
//!   `FOCUS_GRACE`, and the request is cancelled if it does not (F-7).
//! - **The answer is moved.** Enter moves the one [`Secret`] into the helper's
//!   thread; the card holds nothing after (S-7).
//! - **Escape cancels; a click outside does nothing.**
//!
//! Nothing here blocks on IO. A call that can end the prompt but cannot
//! destroy a surface (an input or a frame) only marks the surfaces for
//! [`PolkitState::settle`], which the app runs from a message it posts to
//! itself, as for the launcher.

use std::time::{Duration, Instant};

use celestina_paint::{silhouette_slices, Painter, Rect, Role, Theme};
use celestina_shell::polkit::{
    Ended, Ending, Moment, Promotion, PromptClock, RequestId, MAX_ATTEMPTS, REGRAB_GAP,
};
use celestina_ui::{
    focus_ring, Interaction, Secret, SecretField, SecretFieldState, SecretKey, Tree, UiEvent,
    FOCUS_WIDTH,
};
use celestina_wayland::{
    Anchoring, FrameCx, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputId, OutputInfo,
    SurfaceId,
};

use super::dumpable::DumpableWindow;
use super::view::{self, CardModel, Focus, Phase};
use super::{AgentPort, HelperHandle, HelperSpawner, NoAgent, Notice, PromptHost, NAMESPACE};
use crate::copy;
use crate::island::state::SILHOUETTE_STEP;
use crate::providers::polkit::helper::HelperEvent;
use crate::providers::polkit::{PolkitEvent, Prompt};
use crate::providers::Reaper;

/// How long a shown card may take to receive its first configure and commit
/// before the request is given up (F-6: the prompt "cannot be mapped"). The
/// keyboard proof counts from that commit, so without this a surface the
/// compositor never configures would wait for the idle cap.
pub const MAP_TIMEOUT: Duration = Duration::from_secs(2);

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const XKB_KEY_RETURN: u32 = 0xff0d;
const XKB_KEY_KP_ENTER: u32 = 0xff8d;
const XKB_KEY_ESCAPE: u32 = 0xff1b;
const XKB_KEY_SPACE: u32 = 0x20;

/// What the loop knows at the moment of a call that may map a prompt.
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'a> {
    /// The moment, from the loop's monotonic clock.
    pub now: Instant,
    /// The output niri has focused, by connector name, if it said.
    pub focused_output: Option<&'a str>,
}

/// What one input event did, for the app to apply to the surface it arrived
/// on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputOutcome {
    /// The card looks different: a frame is due.
    pub redraw: bool,
    /// The prompt ended: the app posts a [`crate::message::Message::PolkitSettle`]
    /// and releases the keyboard of the surface the event came on.
    pub settle: bool,
}

struct Known {
    id: OutputId,
    name: String,
}

/// One surface of the prompt.
struct Surf {
    id: SurfaceId,
    output: OutputId,
    tree: Tree,
    input: Interaction,
    size: (f32, f32),
    /// The view must be built again before the next layout.
    stale: bool,
    theme: Theme,
}

impl Surf {
    fn new(id: SurfaceId, output: OutputId, theme: &Theme, reduced: bool) -> Surf {
        Surf {
            id,
            output,
            tree: Tree::new(reduced, theme),
            input: Interaction::new(),
            size: (0.0, 0.0),
            stale: true,
            theme: theme.clone(),
        }
    }
}

/// The active request: its card (always `surfaces[0]`), its dims and
/// everything the person has typed or been told.
struct Active {
    id: RequestId,
    prompt: Prompt,
    surfaces: Vec<Surf>,
    clock: PromptClock,
    shown_at: Instant,
    /// The card's first commit after the compositor's configure.
    committed: bool,
    /// The serial of the card's `wl_keyboard.enter`.
    enter_serial: Option<u32>,
    secret: Secret,
    field: SecretField,
    focus: Focus,
    phase: Phase,
    pam_prompt: Option<String>,
    pam_line: Option<String>,
    failure: Option<String>,
    failed: u32,
    /// An answer was sent for this request, in any conversation.
    answered: bool,
    helper: Option<Box<dyn HelperHandle>>,
    helper_tried: bool,
}

impl Active {
    fn model(&self) -> CardModel<'_> {
        CardModel {
            message: &self.prompt.message,
            action_id: &self.prompt.action_id,
            account: &self.prompt.account,
            pam_prompt: self.pam_prompt.as_deref(),
            pam_line: self.pam_line.as_deref(),
            failure: self.failure.as_deref(),
            field: self.field.state(),
            focus: self.focus,
            phase: self.phase,
        }
    }

    fn card(&self) -> Option<&Surf> {
        self.surfaces.first()
    }

    fn mark_stale(&mut self) {
        for surface in &mut self.surfaces {
            surface.stale = true;
        }
    }

    /// Moves the answer to the helper, if one is wanted and typed.
    fn submit(&mut self) -> bool {
        if self.phase != Phase::Asking || self.secret.is_empty() {
            return false;
        }
        let Some(helper) = self.helper.as_ref() else {
            return false;
        };
        // The one move of the answer: the card is left with a fresh, empty
        // buffer and the old one is the helper thread's.
        let secret = std::mem::replace(&mut self.secret, Secret::new());
        self.field = SecretField::new();
        let sent = helper.answer(secret).is_ok();
        if sent {
            self.answered = true;
            self.phase = Phase::Checking;
            self.failure = None;
        }
        self.mark_stale();
        sent
    }
}

/// Whether a key's serial is newer than the enter's (wrapping): a key that is
/// not began before the keyboard came to this card.
fn newer(key: u32, enter: u32) -> bool {
    key.wrapping_sub(enter) as i32 > 0
}

/// Drops an event that may carry typed text, wiping the text: the discarded
/// key goes through the toolkit's field, which zeroes what it consumes.
fn discard(ev: InputEvent) {
    let mut scratch = Secret::new();
    let _ = SecretField::new().handle(&mut scratch, ev);
}

fn layer(output: OutputId, keyboard: KeyboardInteractivity) -> LayerSpec {
    LayerSpec {
        output,
        namespace: NAMESPACE,
        layer: Layer::Overlay,
        anchor: Anchoring::Fill,
        size: (0, 0),
        exclusive_zone: -1,
        keyboard,
    }
}

/// What the loop keeps of the polkit prompt.
pub struct PolkitState {
    agent: Box<dyn AgentPort>,
    helpers: Box<dyn HelperSpawner>,
    origin: Instant,
    theme: Theme,
    reduced: bool,
    /// The session is locked; read synchronously before any surface is mapped
    /// and failing closed (N-26). Nothing sets it until the lock screen does.
    locked: bool,
    outputs: Vec<Known>,
    active: Option<Active>,
    /// Surfaces of an ended prompt, destroyed by [`PolkitState::settle`].
    garbage: Vec<SurfaceId>,
    notices: Vec<Notice>,
    last_end: Option<Instant>,
    /// When the next request may take the keyboard, after the regrab gap.
    retry_at: Option<Instant>,
    /// Open while a request is active (design S-9).
    dumpable: DumpableWindow,
}

impl std::fmt::Debug for PolkitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolkitState")
            .field("active", &self.active.as_ref().map(|a| a.id))
            .field("garbage", &self.garbage.len())
            .finish_non_exhaustive()
    }
}

impl PolkitState {
    /// The state over `agent` and `helpers`; every moment it is given is
    /// measured from `origin`.
    #[must_use]
    pub fn new(
        agent: Box<dyn AgentPort>,
        helpers: Box<dyn HelperSpawner>,
        origin: Instant,
        theme: Theme,
        reduced: bool,
    ) -> PolkitState {
        PolkitState {
            agent,
            helpers,
            origin,
            theme,
            reduced,
            locked: false,
            outputs: Vec::new(),
            active: None,
            garbage: Vec::new(),
            notices: Vec::new(),
            last_end: None,
            retry_at: None,
            dumpable: DumpableWindow::inert(),
        }
    }

    /// Takes the window the process's dumpable flag is toggled through while
    /// a request is active. The state starts with an inert one, so only the
    /// duty's launcher decides that the real flag is touched.
    pub fn set_dumpable(&mut self, window: DumpableWindow) {
        self.dumpable = window;
    }

    /// Whether a request is active (its card is up or being torn down).
    #[must_use]
    pub fn request_active(&self) -> bool {
        self.active.is_some()
    }

    /// The duty ends: the active request is answered `Cancelled`, its helper
    /// conversation is cancelled (the socket closes and an answer on its way
    /// is dropped), the typed text is wiped and every surface is destroyed.
    /// Requests that were only waiting are answered by the agent when it is
    /// dropped next. Raises nothing: the person switched the duty off.
    pub fn shut_down(&mut self, now: Instant, host: &mut dyn PromptHost) {
        self.finish(Ending::ShellExit, now);
        self.notices.clear();
        self.retry_at = None;
        for surface in self.garbage.drain(..) {
            host.destroy_surface(surface);
        }
    }

    /// Gives the agent up to a reaper without waiting for it (N-22), leaving
    /// this state with one that asks and answers nothing. Call it after
    /// [`PolkitState::shut_down`]; the reaper is for whoever starts the next
    /// agent.
    #[must_use]
    pub fn hand_off_agent(&mut self) -> Reaper {
        std::mem::replace(&mut self.agent, Box::new(NoAgent)).retire()
    }

    fn moment(&self, at: Instant) -> Moment {
        at.saturating_duration_since(self.origin)
    }

    /// The theme and motion preference in force.
    pub fn set_appearance(&mut self, theme: Theme, reduced: bool) {
        self.theme = theme;
        self.reduced = reduced;
        if let Some(active) = self.active.as_mut() {
            active.mark_stale();
        }
    }

    /// Whether the prompt holds, or until [`PolkitState::settle`] still has,
    /// a surface that asks for the keyboard. The app opens no launcher or
    /// Island page while it does (design section 5.1, N-25).
    #[must_use]
    pub fn keyboard_claimed(&self) -> bool {
        self.active.is_some() || !self.garbage.is_empty()
    }

    /// Whether `surface` is one of the prompt's, live or about to be destroyed.
    #[must_use]
    pub fn owns(&self, surface: SurfaceId) -> bool {
        self.garbage.contains(&surface)
            || self
                .active
                .as_ref()
                .is_some_and(|a| a.surfaces.iter().any(|s| s.id == surface))
    }

    /// The card's surface while a prompt is up.
    #[must_use]
    pub fn card_surface(&self) -> Option<SurfaceId> {
        self.active.as_ref()?.card().map(|s| s.id)
    }

    /// The locked state, read by the loop before any surface is mapped. A
    /// prompt that is up when the session locks ends at once; nothing is
    /// shown while it stays locked.
    pub fn set_locked(&mut self, locked: bool, now: Instant) {
        self.locked = locked;
        self.agent.set_locked(locked);
        if locked {
            self.finish(Ending::NotShown, now);
        }
    }

    /// An output appeared: it is a place a prompt can go, and a prompt that
    /// is already up dims it too.
    pub fn output_added(&mut self, info: &OutputInfo, host: &mut dyn PromptHost) {
        match self.outputs.iter_mut().find(|known| known.id == info.id) {
            Some(known) => known.name.clone_from(&info.name),
            None => self.outputs.push(Known {
                id: info.id,
                name: info.name.clone(),
            }),
        }
        let (theme, reduced) = (self.theme.clone(), self.reduced);
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.surfaces.iter().any(|s| s.output == info.id) {
            return;
        }
        match host.create_surface(layer(info.id, KeyboardInteractivity::None)) {
            Ok(id) => active
                .surfaces
                .push(Surf::new(id, info.id, &theme, reduced)),
            Err(err) => {
                tracing::warn!(output = info.id.0, %err, "polkit: no dim on the new output")
            }
        }
    }

    /// An output went away, its surfaces with it. The prompt on it ends (F-6);
    /// a dim on it is forgotten.
    pub fn output_removed(&mut self, id: OutputId, now: Instant) {
        self.outputs.retain(|known| known.id != id);
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let card_gone = active.card().is_some_and(|s| s.output == id);
        // The runtime destroyed them: they are not the garbage's to destroy.
        active.surfaces.retain(|s| s.output != id);
        if card_gone {
            self.finish(Ending::PromptUnavailable, now);
        }
    }

    /// What the agent told the loop.
    pub fn on_event(&mut self, ev: PolkitEvent, ctx: &Ctx<'_>, host: &mut dyn PromptHost) {
        match ev {
            PolkitEvent::Admitted(_) => {}
            PolkitEvent::Ended { id, .. } => {
                // The agent ended it and decided its reply: only the local
                // teardown is left.
                if self.active.as_ref().is_some_and(|a| a.id == id) {
                    self.retire(ctx.now);
                }
            }
            PolkitEvent::Registration(_) => return,
        }
        self.settle(ctx, host);
    }

    /// What request `id`'s helper conversation said.
    pub fn on_helper(
        &mut self,
        id: RequestId,
        ev: HelperEvent,
        ctx: &Ctx<'_>,
        host: &mut dyn PromptHost,
    ) {
        let Some(active) = self.active.as_mut().filter(|a| a.id == id) else {
            // A line from a conversation that already ended.
            return;
        };
        match ev {
            HelperEvent::Prompt(text) => {
                active.pam_prompt = Some(text.display());
                active.phase = Phase::Asking;
            }
            HelperEvent::Notice(text) => active.pam_line = Some(text.display()),
            HelperEvent::Failure => {
                active.failed += 1;
                active.secret.clear();
                active.field = SecretField::new();
                active.failure = Some(copy::polkit_attempts_left(
                    MAX_ATTEMPTS.saturating_sub(active.failed),
                ));
                active.pam_prompt = None;
                active.phase = Phase::Preparing;
            }
            HelperEvent::Ended(ending) => {
                self.finish(ending, ctx.now);
                self.settle(ctx, host);
                return;
            }
        }
        active.mark_stale();
        if let Some(card) = active.card() {
            host.request_redraw(card.id);
        }
    }

    /// Looks at the deadlines at `now`: gives up a prompt whose proof, loss
    /// grace or idle cap is due, and opens the helper once the card has the
    /// keyboard and the guard window has passed.
    pub fn tick(&mut self, now: Instant) {
        let moment = self.moment(now);
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let unmapped = !active.committed && now >= active.shown_at + MAP_TIMEOUT;
        if let Some(ending) = active
            .clock
            .due(moment)
            .or(unmapped.then_some(Ending::PromptUnavailable))
        {
            self.finish(ending, now);
            return;
        }
        let wants_helper =
            active.helper.is_none() && !active.helper_tried && active.clock.may_open_helper(moment);
        if wants_helper {
            self.open_helper(now);
        }
    }

    fn open_helper(&mut self, now: Instant) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.helper_tried = true;
        let id = active.id;
        // The cookie moves from the agent's record into the helper's thread
        // in this one call; this state never holds it.
        let Some(input) = self.agent.helper_input(id) else {
            return;
        };
        match self.helpers.spawn(id, input) {
            Ok(helper) => {
                if let Some(active) = self.active.as_mut() {
                    active.helper = Some(helper);
                }
            }
            Err(err) => {
                tracing::warn!(error = %crate::error_chain(&err), "polkit: the helper thread did not start");
                self.finish(Ending::HelperUnreachable, now);
            }
        }
    }

    /// Whether [`PolkitState::settle`] has work: surfaces to destroy, notices
    /// to raise, or a waiting request whose regrab gap has passed.
    #[must_use]
    pub fn needs_settle(&self, now: Instant) -> bool {
        !self.garbage.is_empty()
            || !self.notices.is_empty()
            || (self.active.is_none() && self.retry_at.is_some_and(|at| now >= at))
    }

    /// Does what an ended prompt leaves: destroys its surfaces, raises its
    /// notices and shows the next waiting request, if one may be shown now.
    pub fn settle(&mut self, ctx: &Ctx<'_>, host: &mut dyn PromptHost) {
        self.tick(ctx.now);
        for surface in self.garbage.drain(..) {
            host.destroy_surface(surface);
        }
        for notice in self.notices.drain(..) {
            host.notice(notice);
        }
        self.pump(ctx, host);
    }

    /// How long until the next moment something here can change on its own, for
    /// the app's timer; `None` when nothing is pending.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        let floor = Duration::from_millis(1);
        match self.active.as_ref() {
            Some(active) => {
                let moment = self.moment(now);
                let clock = active
                    .clock
                    .next_deadline(moment)
                    .map(|at| at.saturating_sub(moment));
                let map = (!active.committed)
                    .then(|| (active.shown_at + MAP_TIMEOUT).saturating_duration_since(now));
                [clock, map].into_iter().flatten().min()
            }
            None => self.retry_at.map(|at| at.saturating_duration_since(now)),
        }
        .map(|wait| wait.max(floor))
    }

    fn retry_instant(&self, now: Instant) -> Instant {
        let at = self
            .last_end
            .map_or(now + REGRAB_GAP, |ended| ended + REGRAB_GAP);
        at.max(now + Duration::from_millis(1))
    }

    /// Shows the front of the queue if nothing is shown and the agent allows.
    fn pump(&mut self, ctx: &Ctx<'_>, host: &mut dyn PromptHost) {
        if self.active.is_some() {
            return;
        }
        self.retry_at = None;
        loop {
            match self.agent.promote(ctx.now) {
                Promotion::Idle => return,
                Promotion::Later(_) => {
                    self.retry_at = Some(self.retry_instant(ctx.now));
                    return;
                }
                // The agent answered it; look at the next.
                Promotion::Refused { .. } => {}
                Promotion::Show(id) => {
                    if self.activate(id, ctx, host) {
                        return;
                    }
                }
            }
        }
    }

    /// Maps the prompt of request `id`, which the agent made the active one.
    /// When it cannot be shown the agent is told so and `false` is returned.
    fn activate(&mut self, id: RequestId, ctx: &Ctx<'_>, host: &mut dyn PromptHost) -> bool {
        let now = ctx.now;
        if self.locked {
            // Failing closed: never map while the session is locked.
            self.end_unshown(id, Ending::NotShown, now);
            return false;
        }
        let target = ctx
            .focused_output
            .and_then(|name| self.outputs.iter().find(|known| known.name == name))
            .or_else(|| self.outputs.first())
            .map(|known| known.id);
        let (Some(target), Some(prompt)) = (target, self.agent.prompt(id)) else {
            self.end_unshown(id, Ending::PromptUnavailable, now);
            return false;
        };
        // The launcher and an open Island page go first: two exclusive
        // overlays leave the choice to the compositor and starve the proof.
        host.close_keyboard_surfaces();
        let card = match host.create_surface(layer(target, KeyboardInteractivity::Exclusive)) {
            Ok(card) => card,
            Err(err) => {
                tracing::warn!(%err, "polkit: the card has no surface");
                self.end_unshown(id, Ending::PromptUnavailable, now);
                return false;
            }
        };
        let mut surfaces = vec![Surf::new(card, target, &self.theme, self.reduced)];
        for known in self.outputs.iter().filter(|known| known.id != target) {
            match host.create_surface(layer(known.id, KeyboardInteractivity::None)) {
                Ok(dim) => surfaces.push(Surf::new(dim, known.id, &self.theme, self.reduced)),
                Err(err) => {
                    tracing::warn!(output = known.id.0, %err, "polkit: no dim on this output");
                }
            }
        }
        self.dumpable.open();
        self.active = Some(Active {
            id,
            prompt,
            surfaces,
            clock: PromptClock::new(self.moment(now)),
            shown_at: now,
            committed: false,
            enter_serial: None,
            secret: Secret::new(),
            field: SecretField::new(),
            focus: Focus::Field,
            phase: Phase::Preparing,
            pam_prompt: None,
            pam_line: None,
            failure: None,
            failed: 0,
            answered: false,
            helper: None,
            helper_tried: false,
        });
        true
    }

    /// The agent's request `id` was promoted but never mapped: end it.
    fn end_unshown(&mut self, id: RequestId, ending: Ending, now: Instant) {
        let ended = self.agent.end_active(id, ending, false, now);
        self.last_end = Some(now);
        self.raise_notices(ending, ended);
    }

    /// Queues what the end of a request owes the person. Nothing when the
    /// agent had nothing to end: polkitd cancelled the request first, so
    /// there is no suppressed prompt to warn about.
    fn raise_notices(&mut self, ending: Ending, ended: Option<Ended>) {
        let Some(ended) = ended else {
            return;
        };
        match ending {
            Ending::PromptUnavailable => self.notices.push(Notice::PromptCouldNotBeShown),
            Ending::HelperUnreachable => self.notices.push(Notice::HelperUnavailable),
            _ => {}
        }
        if ended.cooldown_started {
            self.notices.push(Notice::CooldownStarted);
        }
    }

    /// Tears the active prompt down locally: the helper's socket closes, the
    /// answer and the typed text are wiped by ownership ending, and the
    /// surfaces wait for [`PolkitState::settle`]. Returns the request and
    /// whether an answer had been sent for it.
    fn retire(&mut self, now: Instant) -> Option<(RequestId, bool)> {
        let mut active = self.active.take()?;
        active.secret.clear();
        if let Some(helper) = active.helper.take() {
            helper.cancel();
        }
        // The window covers the wipe and the helper's end.
        self.dumpable.close();
        self.garbage.extend(active.surfaces.iter().map(|s| s.id));
        self.last_end = Some(now);
        Some((active.id, active.answered))
    }

    /// Ends the active request with `ending`: tears the prompt down, tells the
    /// agent (which replies to polkitd from its own thread) and queues what
    /// the end owes the person.
    fn finish(&mut self, ending: Ending, now: Instant) {
        let Some((id, attempted)) = self.retire(now) else {
            return;
        };
        let ended = self.agent.end_active(id, ending, attempted, now);
        self.raise_notices(ending, ended);
    }

    // ---- input -------------------------------------------------------------------

    /// One input event on `surface`.
    pub fn input(&mut self, surface: SurfaceId, ev: InputEvent, now: Instant) -> InputOutcome {
        self.tick(now);
        let mut outcome = InputOutcome::default();
        let is_card = self.card_surface() == Some(surface);
        if !is_card {
            // A dim takes no keyboard and acts on nothing; a stray press on
            // it does not deny.
            discard(ev);
            return outcome;
        }
        let act = match ev {
            InputEvent::FocusEnter { serial, at } => {
                self.keyboard_entered(serial, at);
                outcome.redraw = true;
                Act::None
            }
            InputEvent::FocusLost => {
                self.keyboard_left(now);
                outcome.redraw = true;
                Act::None
            }
            ev @ InputEvent::Key { .. } => self.key(ev, now, &mut outcome),
            ev @ (InputEvent::PointerMotion { .. }
            | InputEvent::PointerPress { .. }
            | InputEvent::PointerRelease { .. }
            | InputEvent::PointerLeave
            | InputEvent::Axis { .. }) => self.pointer(&ev, now, &mut outcome),
            // The input method's text and the clipboard's are never accepted.
            ev => {
                discard(ev);
                Act::None
            }
        };
        match act {
            Act::None => {}
            Act::Submit => {
                if let Some(active) = self.active.as_mut() {
                    outcome.redraw |= active.submit();
                }
            }
            Act::End(ending) => {
                self.finish(ending, now);
                outcome.settle = true;
            }
        }
        outcome.settle |= self.needs_settle(now);
        outcome
    }

    fn keyboard_entered(&mut self, serial: u32, at: Instant) {
        let moment = self.moment(at);
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.enter_serial = Some(serial);
        active.clock.keyboard_entered(moment);
        let _ = active
            .field
            .handle(&mut active.secret, InputEvent::FocusEnter { serial, at });
        active.mark_stale();
    }

    /// The keyboard left: the partial answer is wiped now, and typing waits
    /// for the keyboard to return.
    fn keyboard_left(&mut self, now: Instant) {
        let moment = self.moment(now);
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.clock.keyboard_left(moment);
        active.enter_serial = None;
        let _ = active
            .field
            .handle(&mut active.secret, InputEvent::FocusLost);
        active.mark_stale();
    }

    fn key(&mut self, ev: InputEvent, now: Instant, outcome: &mut InputOutcome) -> Act {
        let moment = self.moment(now);
        let Some(active) = self.active.as_mut() else {
            discard(ev);
            return Act::None;
        };
        let InputEvent::Key {
            keysym,
            pressed,
            serial,
            ..
        } = &ev
        else {
            discard(ev);
            return Act::None;
        };
        let (keysym, pressed, serial) = (*keysym, *pressed, *serial);
        // A release is nothing; a press counts only when it began after the
        // enter and outside the guard window (N-8, N-24).
        let accepted = pressed
            && active
                .enter_serial
                .is_some_and(|enter| newer(serial, enter))
            && active.clock.accepts_key(moment);
        if !accepted {
            discard(ev);
            return Act::None;
        }
        if keysym == XKB_KEY_ESCAPE {
            discard(ev);
            return Act::End(Ending::Escaped);
        }
        if keysym == XKB_KEY_TAB || keysym == XKB_KEY_ISO_LEFT_TAB {
            let backwards = keysym == XKB_KEY_ISO_LEFT_TAB
                || matches!(&ev, InputEvent::Key { modifiers, .. } if modifiers.shift);
            active.focus = if backwards {
                active.focus.previous()
            } else {
                active.focus.next()
            };
            active.mark_stale();
            outcome.redraw = true;
            discard(ev);
            return Act::None;
        }
        let activates = matches!(keysym, XKB_KEY_RETURN | XKB_KEY_KP_ENTER | XKB_KEY_SPACE);
        match active.focus {
            Focus::Cancel => {
                discard(ev);
                if activates {
                    Act::End(Ending::Escaped)
                } else {
                    Act::None
                }
            }
            _ if active.phase != Phase::Asking => {
                discard(ev);
                Act::None
            }
            Focus::Authenticate => {
                discard(ev);
                if activates {
                    Act::Submit
                } else {
                    Act::None
                }
            }
            Focus::Field => match active.field.handle(&mut active.secret, ev) {
                SecretKey::Submit => Act::Submit,
                SecretKey::Cancel => Act::End(Ending::Escaped),
                SecretKey::Edited => {
                    active.mark_stale();
                    outcome.redraw = true;
                    Act::None
                }
                SecretKey::Refused | SecretKey::Ignored => Act::None,
            },
        }
    }

    fn pointer(&mut self, ev: &InputEvent, now: Instant, outcome: &mut InputOutcome) -> Act {
        let Some(active) = self.active.as_mut() else {
            return Act::None;
        };
        let Some(card) = active.surfaces.first_mut() else {
            return Act::None;
        };
        let before = card.tree.highlight();
        let events = card.input.pointer(&mut card.tree, ev, now);
        outcome.redraw |= !events.is_empty() || card.tree.highlight() != before;
        let mut act = Act::None;
        for event in events {
            let UiEvent::Click(key) = event else {
                continue;
            };
            if key == view::cancel_key() {
                act = Act::End(Ending::Escaped);
            } else if key == view::authenticate_key() {
                active.focus = Focus::Authenticate;
                act = Act::Submit;
            } else if key == view::field_key() {
                active.focus = Focus::Field;
                active.mark_stale();
                outcome.redraw = true;
            }
            // A click anywhere else, outside the card included, does nothing:
            // a stray click must not deny.
        }
        act
    }

    // ---- frames -------------------------------------------------------------------

    /// The first frame of the card after the compositor's configure: the
    /// keyboard proof and the idle cap start (`size` is that frame's).
    pub fn surface_committed(&mut self, surface: SurfaceId, size: (f32, f32), now: Instant) {
        let moment = self.moment(now);
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if size.0 > 0.0
            && size.1 > 0.0
            && !active.committed
            && active.card().is_some_and(|card| card.id == surface)
        {
            active.committed = true;
            active.clock.committed(moment);
        }
    }

    /// Builds the surface's view when it is stale and lays its tree out with
    /// `measure`. The frame and the tests share it.
    fn lay_out(
        &mut self,
        surface: SurfaceId,
        size: (f32, f32),
        theme: &Theme,
        now: Instant,
        measure: impl FnMut(&str, &celestina_paint::TextStyle, Option<f32>) -> (f32, f32),
    ) -> bool {
        let reduced = self.reduced;
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        let Some(index) = active.surfaces.iter().position(|s| s.id == surface) else {
            return false;
        };
        let stale = {
            let surf = &active.surfaces[index];
            surf.stale || surf.size != size || &surf.theme != theme
        };
        let view = stale.then(|| {
            if index == 0 {
                view::card_surface(theme, size, &active.model())
            } else {
                view::dim(size)
            }
        });
        let Some(surf) = active.surfaces.get_mut(index) else {
            return false;
        };
        surf.tree.set_reduced_motion(reduced);
        if &surf.theme != theme {
            surf.theme = theme.clone();
            surf.tree.set_theme(theme);
        }
        if let Some(view) = view {
            surf.size = size;
            surf.tree.apply(view, now);
            surf.stale = false;
        }
        surf.tree.layout_measured(size, measure);
        if let Some(fault) = surf.tree.take_fault() {
            tracing::warn!(%fault, "polkit: layout fault");
        }
        true
    }

    /// Draws one frame of a prompt surface. Returns whether `cx.surface` is
    /// one of the prompt's, so the app skips it otherwise.
    pub fn frame(&mut self, cx: &mut FrameCx<'_>, painter: &mut Painter) -> bool {
        if !self.owns(cx.surface) {
            return false;
        }
        let now = cx.now;
        self.surface_committed(cx.surface, cx.size, now);
        let theme = painter.theme().clone();
        let dpr = cx.scale as f32;
        let laid_out = self.lay_out(cx.surface, cx.size, &theme, now, |text, style, max| {
            painter.text().measure(text, style, max, dpr)
        });
        if !laid_out {
            // About to be destroyed: nothing to draw.
            return true;
        }
        let wake = self.next_wake(now);
        let Some(active) = self.active.as_mut() else {
            return true;
        };
        let has_keyboard = active.clock.has_keyboard();
        let focus = active.focus;
        let card_id = active.card().map(|c| c.id);
        let Some(surf) = active.surfaces.iter_mut().find(|s| s.id == cx.surface) else {
            return true;
        };
        let is_card = card_id == Some(surf.id);
        painter.begin(cx.size, dpr, cx.blur_available);
        surf.tree.paint(painter, now);
        if is_card && has_keyboard {
            if let Some(node) = surf.tree.node_by_key(view::focus_key(focus)) {
                let radius = match focus {
                    Focus::Field => theme.top_bar / 2.0,
                    Focus::Authenticate | Focus::Cancel => theme.radius_button,
                };
                let (rect, radius) = focus_ring(surf.tree.rect(node), radius);
                painter.stroke_rounded(rect, radius, FOCUS_WIDTH, theme.color(Role::FocusRing));
            }
        }
        painter.end();
        let blur = if is_card {
            surf.tree
                .node_by_key(view::card_key())
                .map(|node| {
                    silhouette_slices(surf.tree.rect(node), theme.radius_lg, SILHOUETTE_STEP)
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        cx.ctl.set_blur_region(&blur);
        // The whole surface takes the pointer: nothing under the dim is
        // reachable, and a click outside the card does nothing.
        cx.ctl.set_input_region(&[Rect::whole(cx.size)]);
        if surf.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
        if let Some(wait) = wake {
            cx.ctl.redraw_after(wait);
        }
        true
    }

    /// Whether the card's tree is still animating at `now`.
    #[must_use]
    pub fn animating(&self, now: Instant) -> bool {
        self.active
            .as_ref()
            .and_then(Active::card)
            .is_some_and(|card| card.tree.is_animating(now))
    }

    /// Whether anything typed is in the field, for the tests and the app's
    /// accessibility name; never the content or its length.
    #[must_use]
    pub fn field_state(&self) -> Option<SecretFieldState> {
        self.active.as_ref().map(|a| a.field.state())
    }
}

/// What a key or a click asks the state to do after the borrow of the active
/// request ends.
enum Act {
    None,
    Submit,
    End(Ending),
}

impl Drop for PolkitState {
    /// A state dropped with a prompt up (the shell exits, or the duty is
    /// switched off) closes the helper's conversation and wipes the text
    /// before the agent behind it answers `Cancelled` and unregisters.
    fn drop(&mut self) {
        if let Some(mut active) = self.active.take() {
            active.secret.clear();
            if let Some(helper) = active.helper.take() {
                helper.cancel();
            }
        }
        // The window restores the previous value in its own `Drop`.
    }
}

#[cfg(test)]
mod tests;
