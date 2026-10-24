//! The polkit prompt on the loop's side (CEL-2-I; design
//! `docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`,
//! sections 4.3 and 5).
//!
//! When the agent ([`crate::providers::polkit`]) admits a request, the loop
//! shows it: a dimmed overlay on every output and, on one of them, a card that
//! holds the keyboard exclusively. [`state`] is everything that decides what
//! that card may do, [`view`] is the card as a pure view.
//!
//! The pieces here are the **boundary** of that state, kept as small traits so
//! the rules can be tested without a bus, a socket or a compositor:
//!
//! - [`AgentPort`]: what the state asks of the agent (the queue's front, the
//!   request's text, the helper's input, the end of a request).
//! - [`HelperSpawner`] and [`HelperHandle`]: the helper conversation's thread.
//! - [`PromptHost`]: what only the app can do (make and destroy surfaces,
//!   close the shell's other keyboard holders, raise a notice).
//!
//! **The cookie never enters the loop's `Message`**: [`AgentPort::helper_input`]
//! hands it to [`HelperSpawner::spawn`] in one move, and nothing here stores
//! it. **The secret is moved, never copied**: the card owns one
//! [`celestina_ui::Secret`] and submit moves it into [`HelperHandle::answer`].

pub mod dumpable;
pub mod duty;
pub mod state;
pub mod view;

use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use celestina_shell::notifications::{Incoming, Urgency};
use celestina_shell::polkit::{Ended, Ending, Promotion, RequestId};
use celestina_ui::Secret;
use celestina_wayland::{LayerSpec, RuntimeError, Sender, SurfaceId};

use crate::message::Message;
use crate::providers::polkit::helper::{self, AnswerRefused, Helper, HelperInput};
use crate::providers::polkit::{PolkitAgent, Prompt};
use crate::providers::Reaper;

pub use state::{InputOutcome, PolkitState};

/// The prompt surfaces' layer-shell namespace. The author's niri layer rule
/// depends on it, so it is written once, here, and a test cites it.
pub const NAMESPACE: &str = "celestina-polkit-prompt";

/// What the state asks of the polkit agent. The agent is the one owner of the
/// request table: the state never keeps a request, only its id.
pub trait AgentPort {
    /// Whether the session is locked; nothing is admitted or shown while it
    /// is (design N-26).
    fn set_locked(&self, locked: bool);
    /// What the front of the queue does at `now`.
    fn promote(&self, now: Instant) -> Promotion;
    /// The text of request `id` while it is the active one.
    fn prompt(&self, id: RequestId) -> Option<Prompt>;
    /// What the helper conversation of the active request `id` opens with.
    fn helper_input(&self, id: RequestId) -> Option<HelperInput>;
    /// Ends request `id` if it is the active one; its reply goes out from the
    /// agent's thread. A request that is not the active one is left alone,
    /// whoever asks.
    fn end_active(
        &self,
        id: RequestId,
        ending: Ending,
        attempted: bool,
        now: Instant,
    ) -> Option<Ended>;
    /// Gives the agent up without waiting for it (N-22): every request is
    /// answered, the agent unregistered and its threads joined on a reaper,
    /// which the next agent waits for before it registers.
    fn retire(self: Box<Self>) -> Reaper;
}

/// What the state holds once its agent was handed off: it asks nothing of
/// anyone and nothing is waiting.
struct NoAgent;

impl AgentPort for NoAgent {
    fn set_locked(&self, _locked: bool) {}

    fn promote(&self, _now: Instant) -> Promotion {
        Promotion::Idle
    }

    fn prompt(&self, _id: RequestId) -> Option<Prompt> {
        None
    }

    fn helper_input(&self, _id: RequestId) -> Option<HelperInput> {
        None
    }

    fn end_active(
        &self,
        _id: RequestId,
        _ending: Ending,
        _attempted: bool,
        _now: Instant,
    ) -> Option<Ended> {
        None
    }

    fn retire(self: Box<Self>) -> Reaper {
        Reaper::default()
    }
}

impl AgentPort for PolkitAgent {
    fn set_locked(&self, locked: bool) {
        PolkitAgent::set_locked(self, locked);
    }

    fn promote(&self, now: Instant) -> Promotion {
        PolkitAgent::promote(self, now)
    }

    fn prompt(&self, id: RequestId) -> Option<Prompt> {
        PolkitAgent::prompt(self, id)
    }

    fn helper_input(&self, id: RequestId) -> Option<HelperInput> {
        PolkitAgent::helper_input(self, id)
    }

    fn end_active(
        &self,
        id: RequestId,
        ending: Ending,
        attempted: bool,
        now: Instant,
    ) -> Option<Ended> {
        PolkitAgent::end_active(self, id, ending, attempted, now)
    }

    fn retire(self: Box<Self>) -> Reaper {
        PolkitAgent::retire(*self)
    }
}

/// A request's helper conversation, as the card sees it.
pub trait HelperHandle {
    /// Moves `secret` to the conversation's thread. Refused, it is dropped
    /// (wiped) here.
    ///
    /// # Errors
    ///
    /// [`AnswerRefused`] when the thread cannot take it.
    fn answer(&self, secret: Secret) -> Result<(), AnswerRefused>;
    /// Closes the conversation: the socket is closed and any answer on its
    /// way is dropped, by its thread. Returns at once (N-22); nothing the
    /// thread says afterwards is published.
    fn cancel(self: Box<Self>);
}

impl HelperHandle for Helper<Secret> {
    fn answer(&self, secret: Secret) -> Result<(), AnswerRefused> {
        Helper::answer(self, secret)
    }

    fn cancel(self: Box<Self>) {
        // The reaper joins the thread; the loop has nothing to wait for.
        drop(Helper::cancel(*self));
    }
}

/// Starts the helper conversation of a request. Called only once the card has
/// the keyboard and the guard window has passed (design N-24).
pub trait HelperSpawner {
    /// Opens the conversation of request `id` with `input`, which owns the
    /// account and a copy of the cookie.
    ///
    /// # Errors
    ///
    /// The failure to start the thread.
    fn spawn(&self, id: RequestId, input: HelperInput) -> io::Result<Box<dyn HelperHandle>>;
}

/// The production [`HelperSpawner`]: a thread per request that publishes what
/// the helper says into the loop as [`Message::PolkitHelper`].
pub struct ThreadHelpers {
    sender: Sender<Message>,
    shutdown: Arc<AtomicBool>,
}

impl ThreadHelpers {
    /// Helpers that publish through `sender` and stop with `shutdown`.
    #[must_use]
    pub fn new(sender: Sender<Message>, shutdown: Arc<AtomicBool>) -> ThreadHelpers {
        ThreadHelpers { sender, shutdown }
    }
}

impl HelperSpawner for ThreadHelpers {
    fn spawn(&self, id: RequestId, input: HelperInput) -> io::Result<Box<dyn HelperHandle>> {
        let sender = self.sender.clone();
        let helper = helper::spawn(
            input,
            move |event| sender.send(Message::PolkitHelper { id, event }).is_ok(),
            &self.shutdown,
        )?;
        Ok(Box::new(helper))
    }
}

/// A notice the shell raises about the prompt through its own notification
/// path. Non-stealing: a banner takes no keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A request ended `Cancelled` because the prompt could not be shown or
    /// never got the keyboard (F-6): so a client that suppresses the real
    /// prompt does not also hide that it happened.
    PromptCouldNotBeShown,
    /// Requests are being cancelled unshown for a while (consent fatigue,
    /// section 5.4).
    CooldownStarted,
    /// A request ended `Failed` because polkit's helper could not be used
    /// (F-11: its socket is missing, not root's, or refused the connection).
    HelperUnavailable,
}

/// What only the app can do for the prompt.
pub trait PromptHost {
    /// Creates a layer surface.
    ///
    /// # Errors
    ///
    /// The runtime's refusal.
    fn create_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError>;
    /// Destroys a surface this prompt created.
    fn destroy_surface(&mut self, id: SurfaceId);
    /// Asks for a frame of `id`.
    fn request_redraw(&mut self, id: SurfaceId);
    /// Closes the shell's other surfaces that hold or request the keyboard
    /// (the launcher, an open Island page, a menu of the bar): two exclusive
    /// overlays would leave the choice to the compositor and starve the
    /// keyboard proof.
    fn close_keyboard_surfaces(&mut self);
    /// Raises `notice` through the shell's own notification path.
    fn notice(&mut self, notice: Notice);
}

/// The notice as a notification the shell posts to itself: the shell's words
/// under its own name. The proof failure stays until dismissed, because a
/// client that suppressed the real prompt must not also hide that it
/// happened; the cool-down and the helper's absence are routine.
#[must_use]
pub fn notice_incoming(notice: Notice) -> Incoming {
    let (summary, body, urgency, expire_timeout) = match notice {
        Notice::PromptCouldNotBeShown => (
            crate::copy::POLKIT_UNSHOWN_SUMMARY,
            crate::copy::POLKIT_UNSHOWN_BODY,
            Urgency::Critical,
            0,
        ),
        Notice::CooldownStarted => (
            crate::copy::POLKIT_COOLDOWN_SUMMARY,
            crate::copy::POLKIT_COOLDOWN_BODY,
            Urgency::Normal,
            -1,
        ),
        Notice::HelperUnavailable => (
            crate::copy::POLKIT_HELPER_SUMMARY,
            crate::copy::POLKIT_HELPER_BODY,
            Urgency::Normal,
            -1,
        ),
    };
    Incoming {
        app_name: crate::copy::POLKIT_NOTICE_APP.to_owned(),
        summary: summary.to_owned(),
        body: body.to_owned(),
        urgency,
        expire_timeout,
        ..Incoming::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_proof_failure_notice_stays_until_dismissed_and_the_cooldown_one_does_not() {
        let shown = notice_incoming(Notice::PromptCouldNotBeShown);
        assert_eq!(shown.urgency, Urgency::Critical);
        assert_eq!(shown.expire_timeout, 0);
        assert_eq!(shown.summary, crate::copy::POLKIT_UNSHOWN_SUMMARY);
        assert_eq!(shown.app_name, crate::copy::POLKIT_NOTICE_APP);
        let cooldown = notice_incoming(Notice::CooldownStarted);
        assert_eq!(cooldown.urgency, Urgency::Normal);
        assert_eq!(cooldown.body, crate::copy::POLKIT_COOLDOWN_BODY);
        let helper = notice_incoming(Notice::HelperUnavailable);
        assert_eq!(helper.urgency, Urgency::Normal);
        assert_eq!(helper.summary, crate::copy::POLKIT_HELPER_SUMMARY);
    }
}
