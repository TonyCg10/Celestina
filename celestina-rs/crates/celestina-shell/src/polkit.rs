//! The pure domain of the polkit agent (CEL-2-I).
//!
//! When a program asks polkit to authorize an action, polkitd calls the
//! authentication agent registered for the person's session; this shell is that
//! agent when the `polkit` duty is on. This module is everything about that
//! agent that needs neither a bus, a socket nor a surface: what a request is,
//! which requests may be shown and when, how a helper's lines are read, how the
//! conversation with the helper may end, and what each end replies to polkitd
//! (design `docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`,
//! sections 4.4 to 4.6 and 6.4). It knows nothing about Wayland, D-Bus or
//! threads, and reads no clock: every time is a [`Moment`] the caller passes in.
//!
//! One rule outranks the rest, and the types are shaped to make it checkable:
//! **the agent never grants.** Only polkit's own helper, running PAM as
//! root, tells polkitd that a password was right. A normal reply to
//! `BeginAuthentication` exists for exactly one [`Ending`],
//! [`Ending::Authenticated`], reached only from the helper's `SUCCESS` line;
//! every fault, timeout, cancellation and unknown line is an error reply. The
//! `only_success_yields_a_normal_reply` test is an exhaustive `match` over
//! [`Ending`], so a new way to end cannot be added without deciding its reply.
//!
//! Pieces:
//!
//! - [`Cookie`], [`Request`], [`Identity`], [`choose_identity`] and
//!   [`passwd_name`]: a request as polkitd sent it, validated, with its text
//!   put through the display sanitiser, and the one account it may be
//!   authenticated for.
//! - [`Queue`]: one active request, a few waiting, a rate limit, a cool-down
//!   after repeated dismissals and a gap before the keyboard is taken again.
//! - [`PromptClock`]: the prompt's deadlines (keyboard proof, guard window,
//!   loss grace, absolute idle cap).
//! - [`decode_line`], [`HelperLine`], [`PamText`] and [`Exchange`]: the helper's
//!   line protocol, strictly decoded, and the attempt counter.
//! - [`Ending`], [`Reply`] and [`Refusal`]: how requests end and what polkitd
//!   is told.
//! - [`RegistrationState`]: the duty's registration with polkitd, never
//!   retried after a refusal.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::display_text::{sanitise_display, Limit};

/// A point on the caller's monotonic clock: the time since an origin the caller
/// chooses and keeps. This module only compares and adds [`Moment`]s.
pub type Moment = Duration;

// ---- constants (design section 6.4) -------------------------------------------------

/// Wrong answers allowed in one request before it is cancelled. Two, because
/// this host's `pam_faillock` locks the account at three (design section 2).
pub const MAX_ATTEMPTS: u32 = 2;
/// The longest cookie accepted, in bytes.
pub const MAX_COOKIE_BYTES: usize = 256;
/// The longest raw helper line accepted, in bytes: the worst case of
/// `g_strescape` is four bytes per input byte, over a 512-byte answer buffer.
pub const MAX_HELPER_LINE: usize = 4096;
/// The most lines one helper conversation may send.
pub const MAX_HELPER_LINES: usize = 64;
/// How much of a PAM message is shown.
pub const MAX_PROMPT_TEXT: Limit = Limit::new(200, 512);
/// How much of polkit's message is shown.
pub const MAX_MESSAGE: Limit = Limit::new(300, 512);
/// The longest action id accepted, in bytes.
pub const MAX_ACTION_ID: usize = 128;
/// Requests that may wait behind the active one.
pub const MAX_QUEUED: usize = 3;
/// Prompts that may be shown in one [`RATE_WINDOW`], counted when shown.
pub const MAX_PROMPTS_PER_MINUTE: usize = 6;
/// The span [`MAX_PROMPTS_PER_MINUTE`] is counted over.
pub const RATE_WINDOW: Duration = Duration::from_secs(60);
/// Prompts in a row that end without an attempt before the cool-down.
pub const BACKOFF_DISMISSALS: u32 = 3;
/// How long requests are refused unshown after [`BACKOFF_DISMISSALS`].
pub const BACKOFF_COOLDOWN: Duration = Duration::from_secs(60);
/// The least time between one prompt ending and the next taking the keyboard.
pub const REGRAB_GAP: Duration = Duration::from_secs(3);
/// How long after its first commit a card has to receive the keyboard.
pub const KEYBOARD_PROOF: Duration = Duration::from_millis(500);
/// How long after the keyboard enters the card ignores keys.
pub const FOCUS_GUARD: Duration = Duration::from_millis(400);
/// How long a lost keyboard may stay lost before the request is cancelled.
pub const FOCUS_GRACE: Duration = Duration::from_secs(2);
/// The absolute life of a mapped prompt; typing does not extend it.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// How long the helper may stay silent after an answer.
pub const HELPER_SILENCE: Duration = Duration::from_secs(60);
/// The longest account name accepted from `/etc/passwd`, in bytes.
pub const MAX_USER_NAME_BYTES: usize = 256;
/// The most of `/etc/passwd` read; a larger file is refused, not truncated.
pub const MAX_PASSWD_BYTES: usize = 1 << 20;

// ---- cookie --------------------------------------------------------------------------

/// Why a cookie was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookieError {
    /// There was no cookie.
    Empty,
    /// It was longer than [`MAX_COOKIE_BYTES`].
    TooLong,
    /// It held a byte that is not printable ASCII.
    NotPrintableAscii,
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "the cookie is empty",
            Self::TooLong => "the cookie is too long",
            Self::NotPrintableAscii => "the cookie holds a byte that is not printable ASCII",
        })
    }
}

impl Error for CookieError {}

/// The cookie polkitd gave for one request: a bearer secret the helper needs.
///
/// Validated at construction (printable ASCII, 1 to [`MAX_COOKIE_BYTES`]
/// bytes), wiped when dropped, and deliberately without `Clone`, `Debug`,
/// `Display`, `Serialize` or equality, so it cannot reach a log, a message or a
/// file by accident. [`Cookie::as_bytes`] is the one way to read it, for the
/// line written to the helper.
pub struct Cookie(Zeroizing<Vec<u8>>);

impl Cookie {
    /// The cookie `text`, if it is acceptable.
    ///
    /// # Errors
    ///
    /// [`CookieError`] says why not.
    pub fn new(text: &str) -> Result<Self, CookieError> {
        if text.is_empty() {
            return Err(CookieError::Empty);
        }
        if text.len() > MAX_COOKIE_BYTES {
            return Err(CookieError::TooLong);
        }
        if !text.bytes().all(|byte| (0x20..=0x7E).contains(&byte)) {
            return Err(CookieError::NotPrintableAscii);
        }
        let mut bytes = Vec::with_capacity(text.len());
        bytes.extend_from_slice(text.as_bytes());
        Ok(Self(Zeroizing::new(bytes)))
    }

    /// The cookie's bytes, for the helper's input line.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Whether both cookies hold the same bytes.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.0.as_slice() == other.0.as_slice()
    }
}

/// Whether `id` is an action id the agent shows: 1 to [`MAX_ACTION_ID`] bytes
/// of `[A-Za-z0-9._-]`. Real ids carry capitals and underscores
/// (`org.freedesktop.NetworkManager.*`, `id.waydro.Initializer.Init`).
#[must_use]
pub fn valid_action_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ACTION_ID
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

// ---- identity -------------------------------------------------------------------------

/// One identity polkitd offered for a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Identity {
    /// `unix-user` with this uid.
    User(u32),
    /// `unix-group` with this gid. Polkitd expands groups before calling, so
    /// one that still arrives is never used.
    Group(u32),
    /// Any other kind.
    Other,
}

/// The identity to authenticate as: the person's own uid if polkitd offered it,
/// and otherwise none, so the request is refused (design Q11). An identity the
/// agent was not offered is never invented.
#[must_use]
pub fn choose_identity(offered: &[Identity], own_uid: u32) -> Option<u32> {
    offered
        .contains(&Identity::User(own_uid))
        .then_some(own_uid)
}

/// The account name for `uid` in the text of `/etc/passwd`.
///
/// The text is bounded ([`MAX_PASSWD_BYTES`]; larger is refused, never
/// truncated). Lines that are blank, comments, NIS markers or malformed are
/// skipped; the first entry for the uid decides, so a bad name is never
/// skipped for a later good one. A name that is empty, longer than
/// [`MAX_USER_NAME_BYTES`], or changed by the display sanitiser (it holds a
/// control, a bidirectional or invisible character, or edge whitespace) is
/// refused: the name goes on a line the helper reads, and it is shown.
#[must_use]
pub fn passwd_name(uid: u32, passwd: &str) -> Option<String> {
    if passwd.len() > MAX_PASSWD_BYTES {
        return None;
    }
    for entry in passwd.lines() {
        let mut fields = entry.splitn(4, ':');
        let (Some(name), Some(_password), Some(entry_uid)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if entry_uid.is_empty()
            || !entry_uid.bytes().all(|byte| byte.is_ascii_digit())
            || entry_uid.parse::<u32>() != Ok(uid)
        {
            continue;
        }
        return acceptable_user_name(name).then(|| name.to_owned());
    }
    None
}

fn acceptable_user_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_USER_NAME_BYTES
        && sanitise_display(name, Limit::new(MAX_USER_NAME_BYTES, MAX_USER_NAME_BYTES)) == name
}

// ---- request ---------------------------------------------------------------------------

/// The shell's own number for a request, never the cookie. Surfaces and the
/// loop name a request by this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(u64);

impl RequestId {
    /// The id with this number.
    #[must_use]
    pub const fn new(number: u64) -> Self {
        Self(number)
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// What a card shows of a request, all sanitised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestDisplay<'a> {
    /// Polkit's message, as polkitd worded it, sanitised and bounded.
    pub message: &'a str,
    /// The action id, on its own line.
    pub action_id: &'a str,
    /// The account being asked for.
    pub account: &'a str,
}

/// One `BeginAuthentication`, validated: the cookie, the action id, polkit's
/// message after the sanitiser, and the one account to authenticate as.
///
/// There is no `Clone` and no `Debug`: it owns a [`Cookie`].
pub struct Request {
    id: RequestId,
    cookie: Cookie,
    action_id: String,
    message: String,
    user: String,
}

impl Request {
    /// A request from the body of a `BeginAuthentication` call.
    ///
    /// `identities` are what polkitd offered, `own_uid` is the shell's uid and
    /// `passwd` is the text of `/etc/passwd`. `icon_name` and `details` are not
    /// parameters: the agent ignores them (design N-20).
    ///
    /// # Errors
    ///
    /// [`Refusal::Malformed`] for a bad cookie or action id (F-2) and
    /// [`Refusal::NoIdentity`] when the person's own account is not offered or
    /// cannot be named (F-4).
    pub fn new(
        id: RequestId,
        cookie: &str,
        action_id: &str,
        message: &str,
        identities: &[Identity],
        own_uid: u32,
        passwd: &str,
    ) -> Result<Self, Refusal> {
        let cookie = Cookie::new(cookie).map_err(|_| Refusal::Malformed)?;
        if !valid_action_id(action_id) {
            return Err(Refusal::Malformed);
        }
        let uid = choose_identity(identities, own_uid).ok_or(Refusal::NoIdentity)?;
        let user = passwd_name(uid, passwd).ok_or(Refusal::NoIdentity)?;
        Ok(Self {
            id,
            cookie,
            action_id: action_id.to_owned(),
            message: sanitise_display(message, MAX_MESSAGE),
            user,
        })
    }

    /// The shell's number for this request.
    #[must_use]
    pub fn id(&self) -> RequestId {
        self.id
    }

    /// The cookie.
    #[must_use]
    pub fn cookie(&self) -> &Cookie {
        &self.cookie
    }

    /// The account name the helper is told to authenticate.
    #[must_use]
    pub fn helper_user(&self) -> &str {
        &self.user
    }

    /// The text a card shows: the only source of its description (design
    /// N-18).
    #[must_use]
    pub fn display(&self) -> RequestDisplay<'_> {
        RequestDisplay {
            message: &self.message,
            action_id: &self.action_id,
            account: &self.user,
        }
    }
}

// ---- how requests end and what polkitd is told ------------------------------------------

/// What the agent answers to a `BeginAuthentication` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// A method return. Polkitd decides on its own state: only the helper can
    /// have set it.
    Normal,
    /// `org.freedesktop.PolicyKit1.Error.Cancelled`: polkitd records
    /// "dismissed".
    Cancelled,
    /// `org.freedesktop.PolicyKit1.Error.Failed`: a fault, not a dismissal.
    Failed,
    /// The bus's `AccessDenied`, for a caller that is not polkitd.
    AccessDenied,
}

/// How a request ends: one variant per row of the failure table (design
/// section 6.4) that ends a request. Rows F-16 (a retry), F-22 (a cancel for
/// an unknown cookie), F-24 and F-25 (registration) end nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// F-1: the sender is not the current polkitd, or none is known.
    RefusedSender,
    /// F-2: a bad cookie or action id.
    Malformed,
    /// F-3: the cookie is already pending.
    DuplicateCookie,
    /// F-4: no usable identity.
    NoIdentity,
    /// F-5: queue full, rate limit, cool-down, locked or duty off.
    NotShown,
    /// F-6: the prompt could not be mapped or never got the keyboard.
    PromptUnavailable,
    /// F-7: the keyboard was lost and did not return.
    KeyboardLost,
    /// F-8: polkitd's owner changed or vanished.
    OwnerChanged,
    /// F-9: the shell is exiting or the duty was switched off.
    ShellExit,
    /// F-10: the shell crashed (polkitd cancels by itself).
    ShellCrash,
    /// F-11: the helper's socket could not be reached or trusted.
    HelperUnreachable,
    /// F-12: an unknown, oversized or badly encoded line, a flood, or a
    /// `FAILURE` when no answer had been sent.
    HelperProtocol,
    /// F-13: end of input before a verdict.
    HelperEof,
    /// F-14: no line within [`HELPER_SILENCE`] after the answer.
    HelperSilent,
    /// F-15: PAM asked for an echoed answer.
    EchoOnPrompt,
    /// F-17: the second wrong answer.
    AttemptsExhausted,
    /// F-18: the helper's `SUCCESS`.
    Authenticated,
    /// F-19: Escape or the cancel control.
    Escaped,
    /// F-20: [`IDLE_TIMEOUT`].
    IdleTimeout,
    /// F-21: `CancelAuthentication` for this request.
    CancelledByPolkit,
    /// F-23: a worker panicked.
    WorkerPanic,
}

impl Ending {
    /// Every ending, once.
    pub const ALL: [Ending; 21] = [
        Self::RefusedSender,
        Self::Malformed,
        Self::DuplicateCookie,
        Self::NoIdentity,
        Self::NotShown,
        Self::PromptUnavailable,
        Self::KeyboardLost,
        Self::OwnerChanged,
        Self::ShellExit,
        Self::ShellCrash,
        Self::HelperUnreachable,
        Self::HelperProtocol,
        Self::HelperEof,
        Self::HelperSilent,
        Self::EchoOnPrompt,
        Self::AttemptsExhausted,
        Self::Authenticated,
        Self::Escaped,
        Self::IdleTimeout,
        Self::CancelledByPolkit,
        Self::WorkerPanic,
    ];

    /// The reply to the `BeginAuthentication` call, or `None` when there is no
    /// one to answer (the owner is gone, or the shell is). Only
    /// [`Ending::Authenticated`] is [`Reply::Normal`].
    #[must_use]
    pub const fn reply(self) -> Option<Reply> {
        match self {
            Self::Authenticated => Some(Reply::Normal),
            Self::RefusedSender => Some(Reply::AccessDenied),
            Self::Malformed
            | Self::NoIdentity
            | Self::HelperUnreachable
            | Self::HelperProtocol
            | Self::HelperEof
            | Self::HelperSilent => Some(Reply::Failed),
            Self::DuplicateCookie
            | Self::NotShown
            | Self::PromptUnavailable
            | Self::KeyboardLost
            | Self::ShellExit
            | Self::EchoOnPrompt
            | Self::AttemptsExhausted
            | Self::Escaped
            | Self::IdleTimeout
            | Self::CancelledByPolkit
            | Self::WorkerPanic => Some(Reply::Cancelled),
            Self::OwnerChanged | Self::ShellCrash => None,
        }
    }

    /// The row of the failure table (`F-n`) this ending is.
    #[must_use]
    pub const fn row(self) -> u8 {
        match self {
            Self::RefusedSender => 1,
            Self::Malformed => 2,
            Self::DuplicateCookie => 3,
            Self::NoIdentity => 4,
            Self::NotShown => 5,
            Self::PromptUnavailable => 6,
            Self::KeyboardLost => 7,
            Self::OwnerChanged => 8,
            Self::ShellExit => 9,
            Self::ShellCrash => 10,
            Self::HelperUnreachable => 11,
            Self::HelperProtocol => 12,
            Self::HelperEof => 13,
            Self::HelperSilent => 14,
            Self::EchoOnPrompt => 15,
            Self::AttemptsExhausted => 17,
            Self::Authenticated => 18,
            Self::Escaped => 19,
            Self::IdleTimeout => 20,
            Self::CancelledByPolkit => 21,
            Self::WorkerPanic => 23,
        }
    }

    /// Whether this ending, with no answer sent, counts toward the cool-down:
    /// the person (or the prompt's failure to appear) dismissed it. Escape, the
    /// idle timeout and a proof failure, as the design lists them.
    #[must_use]
    pub const fn is_dismissal(self) -> bool {
        matches!(
            self,
            Self::Escaped | Self::IdleTimeout | Self::PromptUnavailable
        )
    }
}

/// Why a request was refused before it was shown (rows F-1 to F-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// F-1: the caller is not the current polkitd.
    NotPolkit,
    /// F-2: a bad cookie or action id.
    Malformed,
    /// F-3: the cookie is already pending.
    DuplicateCookie,
    /// F-4: no usable identity.
    NoIdentity,
    /// F-5: the active request and [`MAX_QUEUED`] waiting ones exist.
    QueueFull,
    /// F-5: [`MAX_PROMPTS_PER_MINUTE`] were already shown.
    RateLimited,
    /// F-5: the cool-down after [`BACKOFF_DISMISSALS`] is running.
    Cooldown,
    /// F-5: the session is locked.
    Locked,
    /// F-5: the duty is off.
    DutyOff,
}

impl Refusal {
    /// The ending a refused request has.
    #[must_use]
    pub const fn ending(self) -> Ending {
        match self {
            Self::NotPolkit => Ending::RefusedSender,
            Self::Malformed => Ending::Malformed,
            Self::DuplicateCookie => Ending::DuplicateCookie,
            Self::NoIdentity => Ending::NoIdentity,
            Self::QueueFull | Self::RateLimited | Self::Cooldown | Self::Locked | Self::DutyOff => {
                Ending::NotShown
            }
        }
    }

    /// The reply to the call: never [`Reply::Normal`].
    #[must_use]
    pub const fn reply(self) -> Reply {
        match self {
            Self::NotPolkit => Reply::AccessDenied,
            Self::Malformed | Self::NoIdentity => Reply::Failed,
            Self::DuplicateCookie
            | Self::QueueFull
            | Self::RateLimited
            | Self::Cooldown
            | Self::Locked
            | Self::DutyOff => Reply::Cancelled,
        }
    }
}

// ---- the queue ---------------------------------------------------------------------------

/// Whether a request was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// It waits its turn; call [`Queue::promote`].
    Accepted,
    /// It is refused, never shown; answer with [`Refusal::reply`]. The request
    /// was dropped, and its cookie wiped.
    Refused(Refusal),
}

/// What [`Queue::promote`] decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Promotion {
    /// Nothing to do: a request is active or none waits.
    Idle,
    /// The next request may not take the keyboard before this moment.
    Later(Moment),
    /// This request is now the active one and was counted as shown.
    Show(RequestId),
    /// A waiting request could not be shown after all; answer it with
    /// [`Refusal::reply`]. Promote again for the next.
    Refused {
        /// The refused request.
        id: RequestId,
        /// Why.
        refusal: Refusal,
    },
}

/// How an active request ended in the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ended {
    /// The request that ended.
    pub id: RequestId,
    /// What to answer its `BeginAuthentication` call, if anyone can be told.
    pub reply: Option<Reply>,
    /// Whether this ending started the cool-down; raise the one notice then.
    pub cooldown_started: bool,
}

/// What [`Queue::cancel`] found for a `CancelAuthentication`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelOutcome {
    /// No such pending cookie: ignore it (F-22). The cancel call itself is
    /// answered normally either way.
    Unknown,
    /// A waiting request was removed without ever being shown; answer its begin
    /// call with [`Ending::CancelledByPolkit`]'s reply.
    Waiting(RequestId),
    /// The active request ended; close its surfaces and answer its begin call.
    Active(Ended),
}

struct Active {
    request: Request,
}

/// The requests of one agent: one active, up to [`MAX_QUEUED`] waiting, the
/// rate limit, the cool-down and the regrab gap, with the clock and the
/// `duty_on` and `locked` inputs all injected.
pub struct Queue {
    duty_on: bool,
    locked: bool,
    active: Option<Active>,
    waiting: VecDeque<Request>,
    shown: VecDeque<Moment>,
    dismissals: u32,
    cooldown_until: Option<Moment>,
    last_ended: Option<Moment>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    /// An empty queue with the duty off and the session unlocked.
    #[must_use]
    pub fn new() -> Self {
        Self {
            duty_on: false,
            locked: false,
            active: None,
            waiting: VecDeque::new(),
            shown: VecDeque::new(),
            dismissals: 0,
            cooldown_until: None,
            last_ended: None,
        }
    }

    /// Whether the `polkit` duty is on. While off nothing is admitted or shown.
    pub fn set_duty(&mut self, on: bool) {
        self.duty_on = on;
    }

    /// Whether the session is locked, read synchronously by the caller and
    /// failing closed when unknown (design N-26). While locked nothing is
    /// admitted or shown.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    /// The active request, if any.
    #[must_use]
    pub fn active(&self) -> Option<&Request> {
        self.active.as_ref().map(|active| &active.request)
    }

    /// How many requests wait behind the active one.
    #[must_use]
    pub fn waiting_len(&self) -> usize {
        self.waiting.len()
    }

    fn holds(&self, cookie: &Cookie) -> bool {
        self.active()
            .into_iter()
            .chain(self.waiting.iter())
            .any(|request| request.cookie().same_as(cookie))
    }

    fn prune(&mut self, now: Moment) {
        while self
            .shown
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= RATE_WINDOW)
        {
            self.shown.pop_front();
        }
    }

    fn in_cooldown(&self, now: Moment) -> bool {
        self.cooldown_until.is_some_and(|until| now < until)
    }

    fn gate(&mut self, now: Moment) -> Option<Refusal> {
        self.prune(now);
        if !self.duty_on {
            Some(Refusal::DutyOff)
        } else if self.locked {
            Some(Refusal::Locked)
        } else if self.in_cooldown(now) {
            Some(Refusal::Cooldown)
        } else if self.shown.len() >= MAX_PROMPTS_PER_MINUTE {
            Some(Refusal::RateLimited)
        } else {
            None
        }
    }

    /// Take a request or refuse it. Refused requests are never shown, and are
    /// dropped (their cookie wiped).
    pub fn admit(&mut self, request: Request, now: Moment) -> Admission {
        if !self.duty_on {
            return Admission::Refused(Refusal::DutyOff);
        }
        if self.locked {
            return Admission::Refused(Refusal::Locked);
        }
        if self.holds(request.cookie()) {
            return Admission::Refused(Refusal::DuplicateCookie);
        }
        if let Some(refusal) = self.gate(now) {
            return Admission::Refused(refusal);
        }
        let room = MAX_QUEUED + usize::from(self.active.is_none());
        if self.waiting.len() >= room {
            return Admission::Refused(Refusal::QueueFull);
        }
        self.waiting.push_back(request);
        Admission::Accepted
    }

    /// Decide what the front of the queue does at `now`. Call it after an
    /// admission, after an ending and when a [`Promotion::Later`] moment
    /// arrives, until it answers [`Promotion::Idle`] or [`Promotion::Later`].
    pub fn promote(&mut self, now: Moment) -> Promotion {
        if self.active.is_some() || self.waiting.is_empty() {
            return Promotion::Idle;
        }
        if let Some(refusal) = self.gate(now) {
            return self.refuse_front(refusal);
        }
        if let Some(ended) = self.last_ended {
            let earliest = ended + REGRAB_GAP;
            if now < earliest {
                return Promotion::Later(earliest);
            }
        }
        match self.waiting.pop_front() {
            Some(request) => {
                let id = request.id();
                self.shown.push_back(now);
                self.active = Some(Active { request });
                Promotion::Show(id)
            }
            None => Promotion::Idle,
        }
    }

    fn refuse_front(&mut self, refusal: Refusal) -> Promotion {
        match self.waiting.pop_front() {
            Some(request) => Promotion::Refused {
                id: request.id(),
                refusal,
            },
            None => Promotion::Idle,
        }
    }

    /// End the active request. `attempted` is whether any answer was sent for
    /// it ([`Exchange::has_answered`]): an attempt breaks a row of dismissals.
    /// `None` when nothing was active.
    pub fn end_active(&mut self, ending: Ending, attempted: bool, now: Moment) -> Option<Ended> {
        let active = self.active.take()?;
        self.last_ended = Some(now);
        let mut cooldown_started = false;
        if attempted {
            self.dismissals = 0;
        } else if ending.is_dismissal() {
            self.dismissals += 1;
            if self.dismissals >= BACKOFF_DISMISSALS {
                self.dismissals = 0;
                self.cooldown_until = Some(now + BACKOFF_COOLDOWN);
                cooldown_started = true;
            }
        }
        Some(Ended {
            id: active.request.id(),
            reply: ending.reply(),
            cooldown_started,
        })
    }

    /// Handle `CancelAuthentication(cookie)`: remove the waiting request or end
    /// the active one that holds it.
    pub fn cancel(&mut self, cookie: &Cookie, now: Moment) -> CancelOutcome {
        if let Some(position) = self
            .waiting
            .iter()
            .position(|request| request.cookie().same_as(cookie))
        {
            return match self.waiting.remove(position) {
                Some(request) => CancelOutcome::Waiting(request.id()),
                None => CancelOutcome::Unknown,
            };
        }
        let is_active = self
            .active()
            .is_some_and(|request| request.cookie().same_as(cookie));
        if is_active {
            // The caller's `Exchange` knows whether an answer was sent; a
            // cancellation by polkitd is neither a dismissal nor an attempt, so
            // the flag does not matter for it.
            if let Some(ended) = self.end_active(Ending::CancelledByPolkit, false, now) {
                return CancelOutcome::Active(ended);
            }
        }
        CancelOutcome::Unknown
    }

    /// Take every request out, the active one first, for a teardown (F-8, F-9).
    /// The rate limit and the cool-down are kept.
    pub fn drain(&mut self) -> Vec<RequestId> {
        let mut ids: Vec<RequestId> = self
            .active
            .take()
            .map(|a| a.request.id())
            .into_iter()
            .collect();
        ids.extend(self.waiting.drain(..).map(|request| request.id()));
        ids
    }
}

// ---- the prompt's clocks -------------------------------------------------------------------

/// The deadlines of one shown prompt (design section 5.3), as pure state:
/// the keyboard proof, the guard window after the keyboard enters, the grace
/// after it is lost, and the absolute idle cap. The caller feeds it what the
/// compositor said and asks [`PromptClock::due`] on its timer.
#[derive(Clone, Copy, Debug)]
pub struct PromptClock {
    shown_at: Moment,
    committed_at: Option<Moment>,
    entered_at: Option<Moment>,
    ever_entered: bool,
    left_at: Option<Moment>,
}

impl PromptClock {
    /// A prompt that was shown (promoted) at `shown_at`.
    #[must_use]
    pub fn new(shown_at: Moment) -> Self {
        Self {
            shown_at,
            committed_at: None,
            entered_at: None,
            ever_entered: false,
            left_at: None,
        }
    }

    /// The card's first commit after the compositor's configure: the prompt is
    /// mapped and the keyboard proof and the idle cap start. Only the first
    /// counts.
    pub fn committed(&mut self, now: Moment) {
        self.committed_at.get_or_insert(now);
    }

    /// `wl_keyboard.enter` reached the card.
    pub fn keyboard_entered(&mut self, now: Moment) {
        self.entered_at = Some(now);
        self.ever_entered = true;
        self.left_at = None;
    }

    /// `wl_keyboard.leave`. Ignored when the card never had the keyboard.
    pub fn keyboard_left(&mut self, now: Moment) {
        if self.entered_at.is_some() && self.left_at.is_none() {
            self.left_at = Some(now);
            self.entered_at = None;
        }
    }

    /// Whether the card holds the keyboard now.
    #[must_use]
    pub fn has_keyboard(&self) -> bool {
        self.entered_at.is_some() && self.left_at.is_none()
    }

    fn input_open(&self, now: Moment) -> bool {
        self.entered_at
            .is_some_and(|entered| self.left_at.is_none() && now >= entered + FOCUS_GUARD)
    }

    /// Whether a key may be used at `now`: the card has the keyboard and the
    /// guard window has passed. Whether the key *began* after the enter (a key
    /// held from before is not the person answering this prompt) is a question
    /// of serials, which this clock does not see: the loop's state compares
    /// the key's serial with the enter's before it asks.
    #[must_use]
    pub fn accepts_key(&self, now: Moment) -> bool {
        self.input_open(now)
    }

    /// Whether the helper may be contacted: the card has the keyboard and the
    /// guard window has passed (design N-24).
    #[must_use]
    pub fn may_open_helper(&self, now: Moment) -> bool {
        self.input_open(now)
    }

    /// The ending that is due at `now`, if one is. Checked in this order: the
    /// keyboard proof, a loss past the grace, the idle cap.
    #[must_use]
    pub fn due(&self, now: Moment) -> Option<Ending> {
        if let Some(committed) = self.committed_at {
            if !self.ever_entered && now >= committed + KEYBOARD_PROOF {
                return Some(Ending::PromptUnavailable);
            }
        }
        if let Some(left) = self.left_at {
            if now >= left + FOCUS_GRACE {
                return Some(Ending::KeyboardLost);
            }
        }
        let anchor = self.committed_at.unwrap_or(self.shown_at);
        (now >= anchor + IDLE_TIMEOUT).then_some(Ending::IdleTimeout)
    }

    /// The earliest moment after `now` at which [`PromptClock::due`] or
    /// [`PromptClock::may_open_helper`] can change its answer: the end of the
    /// keyboard proof, of the guard window, of the loss grace, or the idle cap.
    /// For the caller's timer; `None` is never returned while the prompt
    /// lives, because the idle cap always lies ahead until it is reached.
    #[must_use]
    pub fn next_deadline(&self, now: Moment) -> Option<Moment> {
        let mut best: Option<Moment> = None;
        let mut consider = |at: Moment| {
            if at > now {
                best = Some(best.map_or(at, |b| b.min(at)));
            }
        };
        if let Some(committed) = self.committed_at {
            if !self.ever_entered {
                consider(committed + KEYBOARD_PROOF);
            }
        }
        if let Some(entered) = self.entered_at {
            consider(entered + FOCUS_GUARD);
        }
        if let Some(left) = self.left_at {
            consider(left + FOCUS_GRACE);
        }
        consider(self.committed_at.unwrap_or(self.shown_at) + IDLE_TIMEOUT);
        best
    }
}

// ---- the helper's lines ----------------------------------------------------------------------

/// Why a helper line was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineError {
    /// Longer than [`MAX_HELPER_LINE`] bytes.
    TooLong,
    /// A raw control byte: the helper escapes every one of them.
    RawControl,
    /// Not one of the six lines the helper speaks.
    Unknown,
    /// An escape outside the set `g_strescape` writes.
    BadEscape,
    /// The decoded text is not UTF-8.
    NotUtf8,
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLong => "the helper line is too long",
            Self::RawControl => "the helper line holds a raw control byte",
            Self::Unknown => "the helper line is not one the helper speaks",
            Self::BadEscape => "the helper line holds an escape the helper never writes",
            Self::NotUtf8 => "the helper line is not UTF-8 once decoded",
        })
    }
}

impl Error for LineError {}

/// Text PAM sent, decoded and validated as UTF-8 but **not yet safe to paint**.
/// The only way to read it for painting is [`PamText::display`], which puts it
/// through the display sanitiser, so "decode, then sanitise" cannot be done in
/// the wrong order or skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PamText(String);

impl PamText {
    /// The text to paint: sanitised and bounded by [`MAX_PROMPT_TEXT`].
    #[must_use]
    pub fn display(&self) -> String {
        sanitise_display(&self.0, MAX_PROMPT_TEXT)
    }

    #[cfg(test)]
    fn decoded(&self) -> &str {
        &self.0
    }
}

/// One line of the helper's protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelperLine {
    /// `PAM_PROMPT_ECHO_OFF`: ask for an answer that is not echoed.
    PromptSecret(PamText),
    /// `PAM_PROMPT_ECHO_ON`: an echoed prompt, which the agent refuses.
    PromptVisible(PamText),
    /// `PAM_ERROR_MSG`.
    Error(PamText),
    /// `PAM_TEXT_INFO`.
    Info(PamText),
    /// `SUCCESS`: the helper has already told polkitd.
    Success,
    /// `FAILURE`.
    Failure,
}

/// Read one helper line, given without its terminating newline.
///
/// Strict, because the helper is trusted for its verdict only: the line is at
/// most [`MAX_HELPER_LINE`] bytes; holds no raw control byte (the helper
/// escapes them); is `SUCCESS` or `FAILURE` as the whole line, or one of the
/// four `PAM_*` verbs followed by one space and text; and the text is decoded
/// with exactly the escapes `g_strescape` writes (`\\ \" \n \t \r \b \f \v \a`
/// and one to three octal digits, at most `\377`), anything else being refused.
/// UTF-8 is validated after decoding, because the helper writes non-ASCII text
/// as octal.
///
/// # Errors
///
/// [`LineError`] says why the line is refused; every refusal ends the
/// conversation as a fault, never as a verdict.
pub fn decode_line(raw: &[u8]) -> Result<HelperLine, LineError> {
    if raw.len() > MAX_HELPER_LINE {
        return Err(LineError::TooLong);
    }
    if raw.iter().any(|byte| *byte < 0x20 || *byte == 0x7F) {
        return Err(LineError::RawControl);
    }
    match raw {
        b"SUCCESS" => return Ok(HelperLine::Success),
        b"FAILURE" => return Ok(HelperLine::Failure),
        _ => {}
    }
    type Make = fn(PamText) -> HelperLine;
    const VERBS: [(&[u8], Make); 4] = [
        (b"PAM_PROMPT_ECHO_OFF ", HelperLine::PromptSecret),
        (b"PAM_PROMPT_ECHO_ON ", HelperLine::PromptVisible),
        (b"PAM_ERROR_MSG ", HelperLine::Error),
        (b"PAM_TEXT_INFO ", HelperLine::Info),
    ];
    for (verb, make) in VERBS {
        if let Some(text) = raw.strip_prefix(verb) {
            let decoded = unescape(text)?;
            let text = String::from_utf8(decoded).map_err(|_| LineError::NotUtf8)?;
            return Ok(make(PamText(text)));
        }
    }
    Err(LineError::Unknown)
}

fn unescape(text: &[u8]) -> Result<Vec<u8>, LineError> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            out.push(byte);
            continue;
        }
        let Some(escape) = bytes.next() else {
            return Err(LineError::BadEscape);
        };
        let decoded = match escape {
            b'\\' => b'\\',
            b'"' => b'"',
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'b' => 0x08,
            b'f' => 0x0C,
            b'v' => 0x0B,
            b'a' => 0x07,
            b'0'..=b'7' => {
                let mut value = u32::from(escape - b'0');
                for _ in 0..2 {
                    match bytes.peek() {
                        Some(digit @ b'0'..=b'7') => {
                            value = value * 8 + u32::from(digit - b'0');
                            bytes.next();
                        }
                        _ => break,
                    }
                }
                u8::try_from(value).map_err(|_| LineError::BadEscape)?
            }
            _ => return Err(LineError::BadEscape),
        };
        out.push(decoded);
    }
    Ok(out)
}

// ---- one request's conversation with the helper -------------------------------------------------

/// What the caller does after feeding the [`Exchange`] an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Show this text and wait for the person's secret.
    Ask(PamText),
    /// Show this text (PAM's words) and keep reading.
    Notice(PamText),
    /// A wrong answer below the cap (F-16): close this conversation's socket,
    /// wipe, show PAM's error text and open a new conversation with the same
    /// cookie.
    Retry,
    /// The request is over with this ending.
    End(Ending),
    /// The conversation already ended, or the event means nothing now.
    Ignore,
}

/// The helper conversation of one request: counts the lines, the answers sent
/// and the wrong ones, and decides what each helper event means. One lives as
/// long as the request; a retry reuses it across conversations.
#[derive(Debug)]
pub struct Exchange {
    failed_attempts: u32,
    answered_here: bool,
    answered_ever: bool,
    awaiting_helper: bool,
    lines: usize,
    done: bool,
}

impl Default for Exchange {
    fn default() -> Self {
        Self::new()
    }
}

impl Exchange {
    /// The conversation of a request that has just opened its first socket.
    #[must_use]
    pub fn new() -> Self {
        Self {
            failed_attempts: 0,
            answered_here: false,
            answered_ever: false,
            awaiting_helper: true,
            lines: 0,
            done: false,
        }
    }

    /// Wrong answers so far in this request.
    #[must_use]
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// Whether any answer has been sent for this request, in any conversation.
    #[must_use]
    pub fn has_answered(&self) -> bool {
        self.answered_ever
    }

    /// The secret was written to the helper.
    pub fn answer_sent(&mut self) {
        if !self.done {
            self.answered_here = true;
            self.answered_ever = true;
            self.awaiting_helper = true;
        }
    }

    fn end(&mut self, ending: Ending) -> Step {
        self.done = true;
        Step::End(ending)
    }

    /// A line the helper sent, decoded.
    pub fn on_line(&mut self, line: HelperLine) -> Step {
        if self.done {
            return Step::Ignore;
        }
        self.lines += 1;
        if self.lines > MAX_HELPER_LINES {
            return self.end(Ending::HelperProtocol);
        }
        match line {
            HelperLine::PromptSecret(text) => {
                self.awaiting_helper = false;
                Step::Ask(text)
            }
            HelperLine::PromptVisible(_) => self.end(Ending::EchoOnPrompt),
            HelperLine::Error(text) | HelperLine::Info(text) => Step::Notice(text),
            HelperLine::Success => self.end(Ending::Authenticated),
            HelperLine::Failure if !self.answered_here => self.end(Ending::HelperProtocol),
            HelperLine::Failure => {
                self.failed_attempts += 1;
                if self.failed_attempts < MAX_ATTEMPTS {
                    self.answered_here = false;
                    self.awaiting_helper = true;
                    self.lines = 0;
                    Step::Retry
                } else {
                    self.end(Ending::AttemptsExhausted)
                }
            }
        }
    }

    /// A line the decoder refused (F-12).
    pub fn on_unreadable_line(&mut self) -> Step {
        if self.done {
            Step::Ignore
        } else {
            self.end(Ending::HelperProtocol)
        }
    }

    /// The helper closed the socket or a read failed before a verdict (F-13).
    pub fn on_eof(&mut self) -> Step {
        if self.done {
            Step::Ignore
        } else {
            self.end(Ending::HelperEof)
        }
    }

    /// [`HELPER_SILENCE`] passed with no line. Only a helper that owes a line
    /// (it has not asked and is not waiting for the person) can be silent
    /// (F-14).
    pub fn on_silence(&mut self) -> Step {
        if self.done || !self.awaiting_helper {
            Step::Ignore
        } else {
            self.end(Ending::HelperSilent)
        }
    }

    /// The socket could not be reached or trusted (F-11).
    pub fn on_unreachable(&mut self) -> Step {
        if self.done {
            Step::Ignore
        } else {
            self.end(Ending::HelperUnreachable)
        }
    }
}

// ---- registration -----------------------------------------------------------------------------------

/// Where the duty stands with polkitd.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationState {
    /// The duty is off.
    Off,
    /// On, and not registered: polkitd is absent, or the first attempt is out.
    Waiting,
    /// polkitd accepted this agent.
    Registered,
    /// Another agent holds the session (F-24). Final until the person acts.
    Refused,
    /// No system bus or no session (F-25), with the reason. Final until the
    /// person acts.
    Unavailable(String),
}

/// What happened to the registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationEvent {
    /// The duty was switched on.
    SwitchedOn,
    /// The duty was switched off.
    SwitchedOff,
    /// The registration call was answered: accepted.
    Accepted,
    /// The registration call was answered: another agent holds the session.
    Held,
    /// There is no system bus, or no session could be resolved.
    Unavailable(String),
    /// polkitd has no owner on the bus right now.
    PolkitdAbsent,
    /// polkitd has a new owner.
    OwnerAppeared,
    /// polkitd's owner vanished.
    OwnerLost,
}

/// The state after an event, and whether to call `RegisterAuthenticationAgent`
/// now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    /// The new state.
    pub state: RegistrationState,
    /// Whether to register now (once).
    pub register: bool,
}

impl RegistrationState {
    /// The transition for `event`. A refused or unavailable registration never
    /// tries again by itself: only [`RegistrationEvent::SwitchedOff`] and a
    /// later [`RegistrationEvent::SwitchedOn`] do (design section 4.5).
    #[must_use]
    pub fn on(self, event: RegistrationEvent) -> Transition {
        let stay = |state: RegistrationState| Transition {
            state,
            register: false,
        };
        match (self, event) {
            (_, RegistrationEvent::SwitchedOff) => stay(Self::Off),
            (Self::Off, RegistrationEvent::SwitchedOn) => Transition {
                state: Self::Waiting,
                register: true,
            },
            (Self::Off, _) => stay(Self::Off),
            (state @ (Self::Refused | Self::Unavailable(_)), _) => stay(state),
            (_, RegistrationEvent::Held) => stay(Self::Refused),
            (_, RegistrationEvent::Unavailable(reason)) => stay(Self::Unavailable(reason)),
            (_, RegistrationEvent::Accepted) => stay(Self::Registered),
            (_, RegistrationEvent::SwitchedOn | RegistrationEvent::PolkitdAbsent) => {
                stay(Self::Waiting)
            }
            (Self::Waiting | Self::Registered, RegistrationEvent::OwnerAppeared) => Transition {
                state: Self::Waiting,
                register: true,
            },
            (_, RegistrationEvent::OwnerLost) => stay(Self::Waiting),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Debug, Display};
    use std::time::Duration;

    // ---- helpers -----------------------------------------------------

    const PASSWD: &str = "root:x:0:0::/root:/bin/bash\ntoni:x:1000:1000:Toni:/home/toni:/bin/zsh\n";
    const OWN_UID: u32 = 1000;

    fn secs(n: u64) -> Moment {
        Duration::from_secs(n)
    }

    fn millis(n: u64) -> Moment {
        Duration::from_millis(n)
    }

    fn request(n: u64) -> Request {
        Request::new(
            RequestId::new(n),
            &format!("cookie-{n}"),
            "org.example.act",
            "Do the thing",
            &[Identity::User(OWN_UID)],
            OWN_UID,
            PASSWD,
        )
        .expect("a well-formed request")
    }

    fn queue() -> Queue {
        let mut queue = Queue::new();
        queue.set_duty(true);
        queue
    }

    /// Admit, promote and see it shown; returns the id shown.
    fn show(queue: &mut Queue, n: u64, now: Moment) -> RequestId {
        assert_eq!(queue.admit(request(n), now), Admission::Accepted);
        match queue.promote(now) {
            Promotion::Show(id) => id,
            other => panic!("expected a shown request, got {other:?}"),
        }
    }

    fn line(text: &str) -> HelperLine {
        decode_line(text.as_bytes()).expect("a well-formed line")
    }

    macro_rules! assert_not_impl {
        ($type:ty: $($trait:path),+ $(,)?) => {
            $({
                trait AmbiguousIfImpl<A> {
                    fn check() {}
                }
                impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
                impl<T: ?Sized + $trait> AmbiguousIfImpl<u8> for T {}
                let _ = <$type as AmbiguousIfImpl<_>>::check;
            })+
        };
    }

    // ---- cookie, action id, request ------------------------------------

    #[test]
    fn cookie_over_the_limit_or_non_ascii_is_refused() {
        assert!(Cookie::new(&"a".repeat(MAX_COOKIE_BYTES)).is_ok());
        assert_eq!(
            Cookie::new(&"a".repeat(MAX_COOKIE_BYTES + 1)).err(),
            Some(CookieError::TooLong)
        );
        assert_eq!(Cookie::new("").err(), Some(CookieError::Empty));
        for bad in [
            "ca\u{f1}",
            "a\nb",
            "a\rb",
            "a\0b",
            "a\u{7f}b",
            "\u{202E}x",
            "tab\there",
        ] {
            assert_eq!(
                Cookie::new(bad).err(),
                Some(CookieError::NotPrintableAscii),
                "{bad:?}"
            );
        }
        let ok = Cookie::new("3-5f1c-1-9a").expect("a real-looking cookie");
        assert_eq!(ok.as_bytes(), b"3-5f1c-1-9a");
    }

    #[test]
    fn cookie_compares_by_content_without_implementing_equality() {
        let a = Cookie::new("same").expect("cookie");
        let b = Cookie::new("same").expect("cookie");
        let c = Cookie::new("other").expect("cookie");
        assert!(a.same_as(&b));
        assert!(!a.same_as(&c));
    }

    #[test]
    fn cookie_and_request_leak_nothing_through_standard_traits() {
        assert_not_impl!(Cookie: Clone, Debug, Display, serde::Serialize);
        assert_not_impl!(Request: Clone, Debug, Display, serde::Serialize);
    }

    #[test]
    fn action_id_accepts_capitals_and_underscore_and_refuses_controls() {
        for good in [
            "org.freedesktop.NetworkManager.enable-disable-wifi",
            "id.waydro.Initializer.Init",
            "a_b",
            "org.freedesktop.policykit.exec",
        ] {
            assert!(valid_action_id(good), "{good}");
        }
        assert!(valid_action_id(&"a".repeat(MAX_ACTION_ID)));
        for bad in [
            "",
            "a b",
            "a\nb",
            "a\u{202E}b",
            "a/b",
            "\u{e9}",
            "a\0",
            "a:b",
        ] {
            assert!(!valid_action_id(bad), "{bad:?}");
        }
        assert!(!valid_action_id(&"a".repeat(MAX_ACTION_ID + 1)));
    }

    #[test]
    fn a_request_keeps_the_polkit_text_after_the_sanitiser() {
        let request = Request::new(
            RequestId::new(1),
            "c-1",
            "org.freedesktop.policykit.exec",
            "Run\n/usr/bin/evil\u{202E} as root\u{1b}[0m",
            &[Identity::User(OWN_UID)],
            OWN_UID,
            PASSWD,
        )
        .expect("request");
        let shown = request.display();
        assert_eq!(shown.message, "Run /usr/bin/evil as root[0m");
        assert_eq!(shown.action_id, "org.freedesktop.policykit.exec");
        assert_eq!(shown.account, "toni");
        assert_eq!(request.helper_user(), "toni");
        assert_eq!(request.id(), RequestId::new(1));
        assert!(request
            .cookie()
            .same_as(&Cookie::new("c-1").expect("cookie")));
    }

    #[test]
    fn a_long_message_is_cut_at_the_message_limit() {
        let request = Request::new(
            RequestId::new(1),
            "c",
            "a.b",
            &"x".repeat(10_000),
            &[Identity::User(OWN_UID)],
            OWN_UID,
            PASSWD,
        )
        .expect("request");
        let message = request.display().message;
        assert!(message.len() <= MAX_MESSAGE.bytes());
        assert!(message.chars().count() <= MAX_MESSAGE.graphemes());
        assert!(message.ends_with('\u{2026}'));
    }

    // ---- identity ---------------------------------------------------------

    #[test]
    fn identity_prefers_own_uid_and_otherwise_refuses() {
        let list = [
            Identity::Group(1000),
            Identity::Other,
            Identity::User(0),
            Identity::User(1000),
        ];
        assert_eq!(choose_identity(&list, 1000), Some(1000));
        assert_eq!(choose_identity(&[Identity::User(0)], 1000), None);
        assert_eq!(choose_identity(&[Identity::Group(1000)], 1000), None);
        assert_eq!(choose_identity(&[Identity::Other], 1000), None);
        assert_eq!(choose_identity(&[], 1000), None);
    }

    #[test]
    fn passwd_name_reads_the_entry_of_the_uid() {
        assert_eq!(passwd_name(1000, PASSWD).as_deref(), Some("toni"));
        assert_eq!(passwd_name(0, PASSWD).as_deref(), Some("root"));
        let noisy = "# comment\n\n+::::::\nbad line\nx:x:notanumber:1::/:\ntoni:x:1000:1000::/h:/s";
        assert_eq!(passwd_name(1000, noisy).as_deref(), Some("toni"));
        // A uid is plain digits; a sign or a suffix is not a uid.
        assert_eq!(passwd_name(1000, "a:x:+1000:1::/:\n"), None);
        assert_eq!(passwd_name(1000, "a:x:1000x:1::/:\n"), None);
    }

    #[test]
    fn passwd_name_refuses_controls_and_missing_users() {
        assert_eq!(passwd_name(4242, PASSWD), None);
        assert_eq!(passwd_name(1000, ""), None);
        for name in [
            "ev\til",
            "e\u{1b}vil",
            "e\u{202E}vil",
            " lead",
            "trail ",
            "",
            "a\u{7f}",
        ] {
            let text = format!("{name}:x:1000:1000::/h:/s\n");
            assert_eq!(passwd_name(1000, &text), None, "{name:?}");
        }
        // The first entry of a uid decides: a bad name is not skipped for a
        // later good one.
        assert_eq!(
            passwd_name(1000, "bad\tname:x:1000:1::/:\ngood:x:1000:1::/:\n"),
            None
        );
        let long = format!("{}:x:1000:1::/:\n", "a".repeat(MAX_USER_NAME_BYTES + 1));
        assert_eq!(passwd_name(1000, &long), None);
        let fits = format!("{}:x:1000:1::/:\n", "a".repeat(MAX_USER_NAME_BYTES));
        assert!(passwd_name(1000, &fits).is_some());
    }

    #[test]
    fn passwd_name_is_bounded() {
        let mut huge = "x:x:5:5::/:\n".repeat(MAX_PASSWD_BYTES / 10);
        huge.push_str("toni:x:1000:1000::/:\n");
        assert!(huge.len() > MAX_PASSWD_BYTES);
        assert_eq!(passwd_name(1000, &huge), None);
    }

    // ---- the failure table, rows F-1 to F-5: the request is refused --------

    #[test]
    fn f01_a_sender_that_is_not_polkitd_is_access_denied() {
        assert_eq!(Refusal::NotPolkit.reply(), Reply::AccessDenied);
        assert_eq!(Refusal::NotPolkit.ending(), Ending::RefusedSender);
        assert_eq!(Ending::RefusedSender.row(), 1);
    }

    #[test]
    fn f02_a_malformed_cookie_or_action_id_is_failed() {
        let make = |cookie: &str, action: &str| {
            Request::new(
                RequestId::new(1),
                cookie,
                action,
                "m",
                &[Identity::User(OWN_UID)],
                OWN_UID,
                PASSWD,
            )
            .err()
        };
        for (cookie, action) in [("", "a.b"), ("a\nb", "a.b"), ("ok", ""), ("ok", "bad id")] {
            assert_eq!(make(cookie, action), Some(Refusal::Malformed));
        }
        assert_eq!(
            make(&"c".repeat(MAX_COOKIE_BYTES + 1), "a.b"),
            Some(Refusal::Malformed)
        );
        assert_eq!(Refusal::Malformed.reply(), Reply::Failed);
        assert_eq!(Ending::Malformed.row(), 2);
    }

    #[test]
    fn f03_a_duplicate_cookie_refuses_only_the_second() {
        // Same as duplicate_cookie_refuses_only_the_second, from the table.
        let mut queue = queue();
        assert_eq!(queue.admit(request(1), secs(0)), Admission::Accepted);
        let again = Request::new(
            RequestId::new(2),
            "cookie-1",
            "a.b",
            "m",
            &[Identity::User(OWN_UID)],
            OWN_UID,
            PASSWD,
        )
        .expect("request");
        assert_eq!(
            queue.admit(again, secs(0)),
            Admission::Refused(Refusal::DuplicateCookie)
        );
        assert_eq!(Refusal::DuplicateCookie.reply(), Reply::Cancelled);
        assert_eq!(Ending::DuplicateCookie.row(), 3);
    }

    #[test]
    fn f04_no_usable_identity_is_failed() {
        let make = |identities: &[Identity], passwd: &str| {
            Request::new(
                RequestId::new(1),
                "c",
                "a.b",
                "m",
                identities,
                OWN_UID,
                passwd,
            )
            .err()
        };
        assert_eq!(make(&[], PASSWD), Some(Refusal::NoIdentity));
        assert_eq!(
            make(&[Identity::User(0)], PASSWD),
            Some(Refusal::NoIdentity)
        );
        assert_eq!(
            make(&[Identity::Group(1000)], PASSWD),
            Some(Refusal::NoIdentity)
        );
        // The uid is offered but /etc/passwd cannot name it.
        assert_eq!(
            make(&[Identity::User(OWN_UID)], ""),
            Some(Refusal::NoIdentity)
        );
        assert_eq!(
            make(&[Identity::User(OWN_UID)], "a\tb:x:1000:1::/:\n"),
            Some(Refusal::NoIdentity)
        );
        assert_eq!(Refusal::NoIdentity.reply(), Reply::Failed);
        assert_eq!(Ending::NoIdentity.row(), 4);
    }

    #[test]
    fn f05_queue_full_rate_cooldown_locked_or_duty_off_is_cancelled_unshown() {
        // Duty off.
        let mut off = Queue::new();
        assert_eq!(
            off.admit(request(1), secs(0)),
            Admission::Refused(Refusal::DutyOff)
        );
        // Locked.
        let mut locked = queue();
        locked.set_locked(true);
        assert_eq!(
            locked.admit(request(1), secs(0)),
            Admission::Refused(Refusal::Locked)
        );
        // Queue full.
        let mut full = queue();
        for n in 1..=(MAX_QUEUED as u64 + 1) {
            assert_eq!(full.admit(request(n), secs(0)), Admission::Accepted);
        }
        assert_eq!(
            full.admit(request(99), secs(0)),
            Admission::Refused(Refusal::QueueFull)
        );
        for refusal in [
            Refusal::DutyOff,
            Refusal::Locked,
            Refusal::QueueFull,
            Refusal::RateLimited,
            Refusal::Cooldown,
        ] {
            assert_eq!(refusal.reply(), Reply::Cancelled);
            assert_eq!(refusal.ending(), Ending::NotShown);
        }
        assert_eq!(Ending::NotShown.row(), 5);
        // Nothing refused was ever shown.
        assert!(off.active().is_none());
        assert_eq!(locked.promote(secs(0)), Promotion::Idle);
    }

    // ---- queue ----------------------------------------------------------------

    #[test]
    fn queue_refuses_beyond_max_queued() {
        let mut queue = queue();
        // One active slot plus MAX_QUEUED waiting.
        for n in 1..=(MAX_QUEUED as u64 + 1) {
            assert_eq!(queue.admit(request(n), secs(0)), Admission::Accepted, "{n}");
        }
        let extra = request(50);
        assert_eq!(
            queue.admit(extra, secs(0)),
            Admission::Refused(Refusal::QueueFull)
        );
        // After the first one is shown the count is the same.
        assert_eq!(queue.promote(secs(0)), Promotion::Show(RequestId::new(1)));
        assert_eq!(queue.waiting_len(), MAX_QUEUED);
        assert_eq!(
            queue.admit(request(51), secs(0)),
            Admission::Refused(Refusal::QueueFull)
        );
    }

    #[test]
    fn queue_shows_one_at_a_time_in_arrival_order() {
        let mut queue = queue();
        assert_eq!(queue.admit(request(1), secs(0)), Admission::Accepted);
        assert_eq!(queue.admit(request(2), secs(0)), Admission::Accepted);
        assert_eq!(queue.promote(secs(0)), Promotion::Show(RequestId::new(1)));
        assert_eq!(queue.promote(secs(1)), Promotion::Idle);
        assert_eq!(queue.active().map(Request::id), Some(RequestId::new(1)));
        let ended = queue
            .end_active(Ending::AttemptsExhausted, true, secs(5))
            .expect("an active request");
        assert_eq!(ended.id, RequestId::new(1));
        assert_eq!(
            queue.promote(secs(5) + REGRAB_GAP),
            Promotion::Show(RequestId::new(2))
        );
    }

    #[test]
    fn rate_limit_counts_when_shown() {
        let mut queue = queue();
        // Six prompts shown, three seconds apart.
        for n in 1..=MAX_PROMPTS_PER_MINUTE as u64 {
            let at = secs(3 * (n - 1));
            show(&mut queue, n, at);
            queue.end_active(Ending::AttemptsExhausted, true, at);
        }
        // The seventh, inside the minute, is refused at once.
        assert_eq!(
            queue.admit(request(7), secs(20)),
            Admission::Refused(Refusal::RateLimited)
        );
        // Once the first show has left the window there is room again.
        assert_eq!(queue.admit(request(8), RATE_WINDOW), Admission::Accepted);
        assert_eq!(
            queue.promote(RATE_WINDOW),
            Promotion::Show(RequestId::new(8))
        );
    }

    #[test]
    fn rate_limit_does_not_count_requests_that_are_only_waiting() {
        let mut queue = queue();
        // Five shown...
        for n in 1..=5u64 {
            let at = secs(3 * (n - 1));
            show(&mut queue, n, at);
            queue.end_active(Ending::AttemptsExhausted, true, at);
        }
        // ...and three more queued: waiting is not showing, all are accepted.
        for n in 6..=8u64 {
            assert_eq!(
                queue.admit(request(n), secs(15)),
                Admission::Accepted,
                "{n}"
            );
        }
        // The sixth shows; the seventh would be the seventh in a minute and
        // is refused when its turn comes, never shown.
        assert_eq!(queue.promote(secs(15)), Promotion::Show(RequestId::new(6)));
        queue.end_active(Ending::AttemptsExhausted, true, secs(16));
        assert_eq!(
            queue.promote(secs(16) + REGRAB_GAP),
            Promotion::Refused {
                id: RequestId::new(7),
                refusal: Refusal::RateLimited
            }
        );
        assert_eq!(
            queue.promote(secs(16) + REGRAB_GAP),
            Promotion::Refused {
                id: RequestId::new(8),
                refusal: Refusal::RateLimited
            }
        );
        assert_eq!(queue.promote(secs(40)), Promotion::Idle);
    }

    #[test]
    fn backoff_after_three_dismissals_and_ends_after_the_cooldown() {
        let mut queue = queue();
        let mut now = secs(0);
        for n in 1..=u64::from(BACKOFF_DISMISSALS) {
            show(&mut queue, n, now);
            now += secs(1);
            let ended = queue
                .end_active(Ending::Escaped, false, now)
                .expect("an active request");
            assert_eq!(ended.reply, Some(Reply::Cancelled));
            assert_eq!(
                ended.cooldown_started,
                n == u64::from(BACKOFF_DISMISSALS),
                "{n}"
            );
            now += REGRAB_GAP;
        }
        // In the cool-down a request is refused at once, unshown.
        assert_eq!(
            queue.admit(request(10), now),
            Admission::Refused(Refusal::Cooldown)
        );
        // One that was already waiting is refused when its turn comes.
        // (Admitted before the cool-down would have started: build it so.)
        let mut waiting = self::queue();
        let mut at = secs(0);
        for n in 1..=u64::from(BACKOFF_DISMISSALS) {
            show(&mut waiting, n, at);
            if n == u64::from(BACKOFF_DISMISSALS) {
                assert_eq!(waiting.admit(request(20), at), Admission::Accepted);
            }
            at += secs(1);
            waiting.end_active(Ending::IdleTimeout, false, at);
            at += REGRAB_GAP;
        }
        assert_eq!(
            waiting.promote(at),
            Promotion::Refused {
                id: RequestId::new(20),
                refusal: Refusal::Cooldown
            }
        );
        // After the cool-down the next request is shown again.
        let later = now + BACKOFF_COOLDOWN;
        assert_eq!(queue.admit(request(11), later), Admission::Accepted);
        assert_eq!(queue.promote(later), Promotion::Show(RequestId::new(11)));
    }

    #[test]
    fn backoff_counts_only_dismissals_in_a_row() {
        let mut queue = queue();
        let mut now = secs(0);
        let script = [
            (Ending::Escaped, false),
            (Ending::IdleTimeout, false),
            // An attempt breaks the row.
            (Ending::AttemptsExhausted, true),
            (Ending::PromptUnavailable, false),
            (Ending::Escaped, false),
        ];
        for (n, (ending, attempted)) in script.into_iter().enumerate() {
            show(&mut queue, n as u64 + 1, now);
            now += secs(1);
            let ended = queue.end_active(ending, attempted, now).expect("active");
            assert!(!ended.cooldown_started, "{ending:?}");
            now += secs(12);
        }
        // A request ended by polkit itself is neither a dismissal nor an attempt.
        show(&mut queue, 6, now);
        now += secs(1);
        queue.end_active(Ending::CancelledByPolkit, false, now);
        now += secs(12);
        show(&mut queue, 7, now);
        let ended = queue
            .end_active(Ending::Escaped, false, now + secs(1))
            .expect("active");
        assert!(
            ended.cooldown_started,
            "the third dismissal since the attempt"
        );
    }

    #[test]
    fn regrab_gap_is_respected() {
        let mut queue = queue();
        show(&mut queue, 1, secs(10));
        queue.end_active(Ending::AttemptsExhausted, true, secs(12));
        assert_eq!(queue.admit(request(2), secs(12)), Admission::Accepted);
        assert_eq!(
            queue.promote(secs(12)),
            Promotion::Later(secs(12) + REGRAB_GAP)
        );
        assert_eq!(
            queue.promote(secs(14)),
            Promotion::Later(secs(12) + REGRAB_GAP)
        );
        assert_eq!(
            queue.promote(secs(12) + REGRAB_GAP),
            Promotion::Show(RequestId::new(2))
        );
    }

    #[test]
    fn a_waiting_request_is_refused_when_the_session_locks_or_the_duty_goes_off() {
        let mut queue = queue();
        assert_eq!(queue.admit(request(1), secs(0)), Admission::Accepted);
        queue.set_locked(true);
        assert_eq!(
            queue.promote(secs(0)),
            Promotion::Refused {
                id: RequestId::new(1),
                refusal: Refusal::Locked
            }
        );
        queue.set_locked(false);
        assert_eq!(queue.admit(request(2), secs(0)), Admission::Accepted);
        queue.set_duty(false);
        assert_eq!(
            queue.promote(secs(0)),
            Promotion::Refused {
                id: RequestId::new(2),
                refusal: Refusal::DutyOff
            }
        );
    }

    #[test]
    fn cancel_removes_a_waiting_request_without_showing_it() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        assert_eq!(queue.admit(request(2), secs(1)), Admission::Accepted);
        let cookie = Cookie::new("cookie-2").expect("cookie");
        assert_eq!(
            queue.cancel(&cookie, secs(2)),
            CancelOutcome::Waiting(RequestId::new(2))
        );
        assert_eq!(queue.waiting_len(), 0);
        // The begin call of a removed request is answered Cancelled.
        assert_eq!(Ending::CancelledByPolkit.reply(), Some(Reply::Cancelled));
        queue.end_active(Ending::AttemptsExhausted, true, secs(10));
        assert_eq!(queue.promote(secs(20)), Promotion::Idle);
    }

    #[test]
    fn f21_cancel_for_a_pending_cookie_ends_the_active_request() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        let cookie = Cookie::new("cookie-1").expect("cookie");
        match queue.cancel(&cookie, secs(5)) {
            CancelOutcome::Active(ended) => {
                assert_eq!(ended.id, RequestId::new(1));
                assert_eq!(ended.reply, Some(Reply::Cancelled));
            }
            other => panic!("expected the active request to end, got {other:?}"),
        }
        assert!(queue.active().is_none());
        assert_eq!(Ending::CancelledByPolkit.row(), 21);
    }

    #[test]
    fn f22_cancel_for_an_unknown_cookie_is_ignored() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        let stranger = Cookie::new("nobody-asked").expect("cookie");
        assert_eq!(queue.cancel(&stranger, secs(1)), CancelOutcome::Unknown);
        // The active request is untouched.
        assert_eq!(queue.active().map(Request::id), Some(RequestId::new(1)));
    }

    #[test]
    fn duplicate_cookie_refuses_only_the_second() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        assert_eq!(queue.admit(request(2), secs(0)), Admission::Accepted);
        for same_as in ["cookie-1", "cookie-2"] {
            let again = Request::new(
                RequestId::new(9),
                same_as,
                "a.b",
                "m",
                &[Identity::User(OWN_UID)],
                OWN_UID,
                PASSWD,
            )
            .expect("request");
            assert_eq!(
                queue.admit(again, secs(0)),
                Admission::Refused(Refusal::DuplicateCookie),
                "{same_as}"
            );
        }
        // The first is still the active one and the second still waits.
        assert_eq!(queue.active().map(Request::id), Some(RequestId::new(1)));
        assert_eq!(queue.waiting_len(), 1);
    }

    #[test]
    fn f08_and_f09_drain_hands_back_every_request_active_first() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        assert_eq!(queue.admit(request(2), secs(0)), Admission::Accepted);
        assert_eq!(queue.admit(request(3), secs(0)), Admission::Accepted);
        let ids = queue.drain();
        assert_eq!(
            ids,
            [RequestId::new(1), RequestId::new(2), RequestId::new(3)]
        );
        assert!(queue.active().is_none());
        assert_eq!(queue.waiting_len(), 0);
        // An owner change gets no reply (the owner is gone); an exit gets one.
        assert_eq!(Ending::OwnerChanged.reply(), None);
        assert_eq!(Ending::OwnerChanged.row(), 8);
        assert_eq!(Ending::ShellExit.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::ShellExit.row(), 9);
    }

    #[test]
    fn f10_and_f23_a_crash_has_no_reply_and_a_panic_is_cancelled() {
        assert_eq!(Ending::ShellCrash.reply(), None);
        assert_eq!(Ending::ShellCrash.row(), 10);
        assert_eq!(Ending::WorkerPanic.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::WorkerPanic.row(), 23);
    }

    #[test]
    fn f19_escape_is_cancelled_and_counts_as_a_dismissal() {
        let mut queue = queue();
        show(&mut queue, 1, secs(0));
        let ended = queue
            .end_active(Ending::Escaped, false, secs(3))
            .expect("active");
        assert_eq!(ended.reply, Some(Reply::Cancelled));
        assert_eq!(Ending::Escaped.row(), 19);
        assert!(Ending::Escaped.is_dismissal());
        assert!(Ending::IdleTimeout.is_dismissal());
        assert!(Ending::PromptUnavailable.is_dismissal());
        assert!(!Ending::AttemptsExhausted.is_dismissal());
        assert!(!Ending::CancelledByPolkit.is_dismissal());
    }

    #[test]
    fn ending_nothing_is_nothing() {
        let mut queue = queue();
        assert!(queue.end_active(Ending::Escaped, false, secs(0)).is_none());
    }

    // ---- the prompt's clocks: F-6, F-7, F-20 ------------------------------------

    #[test]
    fn f06_no_keyboard_enter_within_the_proof_cancels_and_opens_no_helper() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        assert!(!clock.may_open_helper(secs(1) + KEYBOARD_PROOF));
        assert_eq!(clock.due(secs(1) + KEYBOARD_PROOF - millis(1)), None);
        assert_eq!(
            clock.due(secs(1) + KEYBOARD_PROOF),
            Some(Ending::PromptUnavailable)
        );
        assert!(
            !clock.may_open_helper(secs(60)),
            "no conversation without the keyboard"
        );
        assert_eq!(Ending::PromptUnavailable.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::PromptUnavailable.row(), 6);
    }

    #[test]
    fn a_keyboard_enter_inside_the_proof_satisfies_it() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_entered(secs(1) + millis(100));
        assert_eq!(clock.due(secs(1) + KEYBOARD_PROOF + secs(1)), None);
        assert!(clock.has_keyboard());
    }

    #[test]
    fn keys_inside_the_guard_window_are_ignored_and_the_helper_waits_for_it() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_entered(secs(2));
        let inside = secs(2) + FOCUS_GUARD - millis(1);
        let after = secs(2) + FOCUS_GUARD;
        assert!(!clock.accepts_key(inside));
        assert!(!clock.may_open_helper(inside));
        assert!(clock.accepts_key(after));
        assert!(clock.may_open_helper(after));
    }

    #[test]
    fn keys_are_refused_before_any_enter() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        assert!(!clock.accepts_key(secs(5)));
        assert!(!clock.has_keyboard());
    }

    #[test]
    fn f07_keyboard_loss_closes_input_and_cancels_after_the_grace() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_entered(secs(2));
        clock.keyboard_left(secs(10));
        assert!(!clock.has_keyboard());
        assert!(!clock.accepts_key(secs(10) + millis(1)));
        assert!(!clock.may_open_helper(secs(10) + millis(1)));
        assert_eq!(clock.due(secs(10) + FOCUS_GRACE - millis(1)), None);
        assert_eq!(
            clock.due(secs(10) + FOCUS_GRACE),
            Some(Ending::KeyboardLost)
        );
        assert_eq!(Ending::KeyboardLost.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::KeyboardLost.row(), 7);
    }

    #[test]
    fn focus_returning_inside_the_grace_accepts_typing_after_a_new_guard_window() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_entered(secs(2));
        clock.keyboard_left(secs(10));
        clock.keyboard_entered(secs(10) + millis(500));
        assert_eq!(clock.due(secs(10) + FOCUS_GRACE + secs(1)), None);
        let back = secs(10) + millis(500);
        assert!(!clock.accepts_key(back + FOCUS_GUARD - millis(1)));
        assert!(clock.accepts_key(back + FOCUS_GUARD));
    }

    #[test]
    fn a_leave_for_a_surface_that_never_had_the_keyboard_is_ignored() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_left(secs(1) + millis(10));
        // The proof still runs as if nothing happened.
        assert_eq!(
            clock.due(secs(1) + KEYBOARD_PROOF),
            Some(Ending::PromptUnavailable)
        );
    }

    #[test]
    fn f20_the_idle_timeout_is_absolute_and_not_extended_by_activity() {
        let mut clock = PromptClock::new(secs(0));
        clock.committed(secs(1));
        clock.keyboard_entered(secs(1) + millis(50));
        // Typing all along changes nothing: the clock has no input to extend.
        for t in (2..119).step_by(7) {
            assert!(clock.accepts_key(secs(t)));
            assert_eq!(clock.due(secs(t)), None, "{t}");
        }
        assert_eq!(clock.due(secs(1) + IDLE_TIMEOUT - millis(1)), None);
        assert_eq!(clock.due(secs(1) + IDLE_TIMEOUT), Some(Ending::IdleTimeout));
        // Even a leave and an enter just before the end do not move it.
        clock.keyboard_left(secs(1) + IDLE_TIMEOUT - secs(1));
        clock.keyboard_entered(secs(1) + IDLE_TIMEOUT - millis(500));
        assert_eq!(clock.due(secs(1) + IDLE_TIMEOUT), Some(Ending::IdleTimeout));
        assert_eq!(Ending::IdleTimeout.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::IdleTimeout.row(), 20);
    }

    #[test]
    fn next_deadline_names_the_next_moment_the_clock_can_change_its_answer() {
        let mut clock = PromptClock::new(secs(0));
        // Not committed yet: only the idle cap is ahead.
        assert_eq!(clock.next_deadline(secs(0)), Some(IDLE_TIMEOUT));
        clock.committed(secs(1));
        assert_eq!(
            clock.next_deadline(secs(1)),
            Some(secs(1) + KEYBOARD_PROOF),
            "the proof is the nearest"
        );
        clock.keyboard_entered(secs(1) + millis(100));
        assert_eq!(
            clock.next_deadline(secs(1) + millis(100)),
            Some(secs(1) + millis(100) + FOCUS_GUARD)
        );
        // Past the guard window the idle cap is all that is left.
        assert_eq!(clock.next_deadline(secs(2)), Some(secs(1) + IDLE_TIMEOUT));
        clock.keyboard_left(secs(3));
        assert_eq!(clock.next_deadline(secs(3)), Some(secs(3) + FOCUS_GRACE));
        // A deadline that has passed is not offered again.
        assert_eq!(
            clock.next_deadline(secs(3) + FOCUS_GRACE),
            Some(secs(1) + IDLE_TIMEOUT)
        );
        assert_eq!(clock.next_deadline(secs(1) + IDLE_TIMEOUT), None);
    }

    #[test]
    fn a_card_that_is_never_committed_still_times_out_from_when_it_was_shown() {
        let clock = PromptClock::new(secs(5));
        assert_eq!(clock.due(secs(5) + IDLE_TIMEOUT - millis(1)), None);
        assert_eq!(clock.due(secs(5) + IDLE_TIMEOUT), Some(Ending::IdleTimeout));
    }

    // ---- the helper's lines -------------------------------------------------------

    #[test]
    fn decode_line_accepts_each_listed_escape() {
        let parsed = line(r#"PAM_ERROR_MSG a\\b\"c\nd\te\rf\bg\fh\vi\aj"#);
        let HelperLine::Error(text) = parsed else {
            panic!("expected an error line");
        };
        assert_eq!(text.decoded(), "a\\b\"c\nd\te\rf\u{8}g\u{c}h\u{b}i\u{7}j");
    }

    #[test]
    fn decode_line_accepts_one_two_and_three_digit_octal() {
        let decode = |raw: &str| match line(raw) {
            HelperLine::Info(text) => text.decoded().to_owned(),
            other => panic!("expected an info line, got {other:?}"),
        };
        assert_eq!(decode(r"PAM_TEXT_INFO \101"), "A");
        assert_eq!(decode(r"PAM_TEXT_INFO \12"), "\n");
        assert_eq!(decode(r"PAM_TEXT_INFO \7"), "\u{7}");
        assert_eq!(decode(r"PAM_TEXT_INFO \0"), "\0");
        // At most three digits are the escape; a fourth is text.
        assert_eq!(decode(r"PAM_TEXT_INFO \1011"), "A1");
        // The largest value is one byte; whether it is text is the UTF-8
        // check's business, after decoding.
        assert_eq!(decode_line(br"PAM_TEXT_INFO \377"), Err(LineError::NotUtf8));
        assert_eq!(decode(r"PAM_TEXT_INFO \177"), "\u{7f}");
    }

    #[test]
    fn decode_line_refuses_an_unknown_escape() {
        for bad in [
            r"PAM_ERROR_MSG \x41",
            r"PAM_ERROR_MSG \e",
            r"PAM_ERROR_MSG \'",
            r"PAM_ERROR_MSG \8",
            r"PAM_ERROR_MSG \777",
            r"PAM_ERROR_MSG \400",
            r"PAM_ERROR_MSG abc\",
            r"PAM_ERROR_MSG \ ",
            r"PAM_ERROR_MSG \u0041",
        ] {
            assert_eq!(
                decode_line(bad.as_bytes()),
                Err(LineError::BadEscape),
                "{bad}"
            );
        }
    }

    #[test]
    fn decode_line_validates_utf8_after_decoding() {
        // Octal pairs forming one valid character (U+00F1, an international
        // input vector: PAM text arrives in the helper's locale).
        let HelperLine::Error(text) = line(r"PAM_ERROR_MSG a\303\261b") else {
            panic!("expected an error line");
        };
        assert_eq!(text.decoded(), "a\u{f1}b");
        // A lone lead byte, once decoded, is invalid.
        assert_eq!(
            decode_line(br"PAM_ERROR_MSG bad\303"),
            Err(LineError::NotUtf8)
        );
        assert_eq!(
            decode_line(br"PAM_ERROR_MSG \377\376"),
            Err(LineError::NotUtf8)
        );
        // Raw bytes are held to the same rule.
        assert_eq!(decode_line(b"PAM_ERROR_MSG \xff"), Err(LineError::NotUtf8));
        let HelperLine::Error(raw) =
            decode_line("PAM_ERROR_MSG caf\u{e9}".as_bytes()).expect("raw utf-8")
        else {
            panic!("expected an error line");
        };
        assert_eq!(raw.decoded(), "caf\u{e9}");
    }

    #[test]
    fn escaped_hostile_text_is_decoded_and_then_removed_by_the_sanitiser() {
        // ESC, NUL and the three bytes of U+202E, all escaped by the helper.
        let HelperLine::Error(text) = line(r"PAM_ERROR_MSG \033[31mred\000\342\200\256gnp\n.exe")
        else {
            panic!("expected an error line");
        };
        assert!(text.decoded().contains('\u{1b}'));
        assert!(text.decoded().contains('\u{202E}'));
        assert_eq!(text.display(), "[31mredgnp .exe");
    }

    #[test]
    fn decode_line_refuses_a_line_over_the_limit() {
        let prefix = "PAM_ERROR_MSG ";
        let at_limit = format!("{prefix}{}", "a".repeat(MAX_HELPER_LINE - prefix.len()));
        assert_eq!(at_limit.len(), MAX_HELPER_LINE);
        assert!(decode_line(at_limit.as_bytes()).is_ok());
        let over = format!("{at_limit}a");
        assert_eq!(decode_line(over.as_bytes()), Err(LineError::TooLong));
        assert_eq!(decode_line(&[b'x'; 1 << 20]), Err(LineError::TooLong));
    }

    #[test]
    fn decode_line_success_and_failure_only_as_whole_lines() {
        assert_eq!(decode_line(b"SUCCESS"), Ok(HelperLine::Success));
        assert_eq!(decode_line(b"FAILURE"), Ok(HelperLine::Failure));
        for bad in [
            "SUCCESS ",
            "SUCCESS extra",
            "SUCCESSFUL",
            " SUCCESS",
            "success",
            "FAILURE.",
            "FAIL",
            "SUCC\\145SS",
            "",
            "PAM_ERROR_MSG",
            "PAM_PROMPT_ECHO_OFFPassword:",
            "PAM_ERROR_MSGx",
            "HELLO",
        ] {
            assert_eq!(
                decode_line(bad.as_bytes()),
                Err(LineError::Unknown),
                "{bad:?}"
            );
        }
        // Inside a message they are only text.
        let HelperLine::Error(text) = line("PAM_ERROR_MSG SUCCESS") else {
            panic!("expected an error line");
        };
        assert_eq!(text.decoded(), "SUCCESS");
    }

    #[test]
    fn decode_line_refuses_raw_control_bytes() {
        for bad in [
            &b"PAM_ERROR_MSG a\nb"[..],
            b"PAM_ERROR_MSG a\x1b[0m",
            b"PAM_ERROR_MSG a\0b",
            b"SUCCESS\n",
            b"SUCCESS\r",
            b"PAM_ERROR_MSG a\x7f",
        ] {
            assert_eq!(decode_line(bad), Err(LineError::RawControl), "{bad:?}");
        }
    }

    #[test]
    fn decode_line_prompt_text_is_returned_undecorated() {
        let HelperLine::PromptSecret(text) = line("PAM_PROMPT_ECHO_OFF Password: ") else {
            panic!("expected a secret prompt");
        };
        assert_eq!(text.decoded(), "Password: ");
        assert!(matches!(
            line("PAM_PROMPT_ECHO_ON login: "),
            HelperLine::PromptVisible(t) if t.decoded() == "login: "
        ));
        assert!(matches!(
            line("PAM_ERROR_MSG "),
            HelperLine::Error(t) if t.decoded().is_empty()
        ));
        assert!(matches!(
            line("PAM_TEXT_INFO Place your finger"),
            HelperLine::Info(t) if t.decoded() == "Place your finger"
        ));
    }

    #[test]
    fn pam_text_displays_within_the_prompt_limit() {
        let HelperLine::Info(text) =
            decode_line(format!("PAM_TEXT_INFO {}", "a".repeat(2000)).as_bytes()).expect("line")
        else {
            panic!("expected an info line");
        };
        let shown = text.display();
        assert!(shown.len() <= MAX_PROMPT_TEXT.bytes());
        assert!(shown.ends_with('\u{2026}'));
    }

    // ---- one request's conversation with the helper --------------------------------

    #[test]
    fn f11_an_unreachable_helper_is_failed() {
        let mut exchange = Exchange::new();
        assert_eq!(
            exchange.on_unreachable(),
            Step::End(Ending::HelperUnreachable)
        );
        assert_eq!(Ending::HelperUnreachable.reply(), Some(Reply::Failed));
        assert_eq!(Ending::HelperUnreachable.row(), 11);
    }

    #[test]
    fn f12_failure_without_an_answer_is_failed_and_does_not_retry() {
        let mut exchange = Exchange::new();
        assert_eq!(
            exchange.on_line(line("FAILURE")),
            Step::End(Ending::HelperProtocol)
        );
        // Nothing after the end retries or reopens.
        assert_eq!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ignore
        );
        assert_eq!(Ending::HelperProtocol.reply(), Some(Reply::Failed));
        assert_eq!(Ending::HelperProtocol.row(), 12);
    }

    #[test]
    fn f12_an_unreadable_line_or_a_flood_is_failed() {
        let mut exchange = Exchange::new();
        assert_eq!(
            exchange.on_unreadable_line(),
            Step::End(Ending::HelperProtocol)
        );
        let mut flood = Exchange::new();
        for n in 0..MAX_HELPER_LINES {
            assert!(
                matches!(flood.on_line(line("PAM_TEXT_INFO hi")), Step::Notice(_)),
                "line {n}"
            );
        }
        assert_eq!(
            flood.on_line(line("PAM_TEXT_INFO hi")),
            Step::End(Ending::HelperProtocol)
        );
    }

    #[test]
    fn f13_eof_before_a_verdict_is_failed() {
        let mut exchange = Exchange::new();
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        exchange.answer_sent();
        assert_eq!(exchange.on_eof(), Step::End(Ending::HelperEof));
        assert_eq!(Ending::HelperEof.reply(), Some(Reply::Failed));
        assert_eq!(Ending::HelperEof.row(), 13);
    }

    #[test]
    fn f14_silence_after_the_answer_is_failed_but_waiting_for_the_person_is_not() {
        let mut exchange = Exchange::new();
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        // The helper is waiting for the person: its silence means nothing.
        assert_eq!(exchange.on_silence(), Step::Ignore);
        exchange.answer_sent();
        assert_eq!(exchange.on_silence(), Step::End(Ending::HelperSilent));
        assert_eq!(Ending::HelperSilent.reply(), Some(Reply::Failed));
        assert_eq!(Ending::HelperSilent.row(), 14);
    }

    #[test]
    fn f15_an_echoed_prompt_is_refused() {
        let mut exchange = Exchange::new();
        assert_eq!(
            exchange.on_line(line("PAM_PROMPT_ECHO_ON login: ")),
            Step::End(Ending::EchoOnPrompt)
        );
        assert_eq!(Ending::EchoOnPrompt.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::EchoOnPrompt.row(), 15);
    }

    #[test]
    fn f16_failure_after_an_answer_below_the_cap_starts_a_new_conversation() {
        let mut exchange = Exchange::new();
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        exchange.answer_sent();
        assert!(matches!(
            exchange.on_line(line("PAM_ERROR_MSG Authentication failure")),
            Step::Notice(_)
        ));
        assert_eq!(exchange.on_line(line("FAILURE")), Step::Retry);
        assert_eq!(exchange.failed_attempts(), 1);
        assert!(exchange.has_answered(), "the request has had an attempt");
        // The new conversation starts clean: the next FAILURE with no answer
        // in it is a protocol fault, not another attempt.
        let mut fresh = exchange;
        assert!(matches!(
            fresh.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        assert_eq!(
            fresh.on_line(line("FAILURE")),
            Step::End(Ending::HelperProtocol)
        );
    }

    #[test]
    fn f17_second_failure_after_an_answer_is_cancelled() {
        let mut exchange = Exchange::new();
        for _ in 0..(MAX_ATTEMPTS - 1) {
            assert!(matches!(
                exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
                Step::Ask(_)
            ));
            exchange.answer_sent();
            assert_eq!(exchange.on_line(line("FAILURE")), Step::Retry);
        }
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        exchange.answer_sent();
        assert_eq!(
            exchange.on_line(line("FAILURE")),
            Step::End(Ending::AttemptsExhausted)
        );
        assert_eq!(exchange.failed_attempts(), MAX_ATTEMPTS);
        assert_eq!(Ending::AttemptsExhausted.reply(), Some(Reply::Cancelled));
        assert_eq!(Ending::AttemptsExhausted.row(), 17);
    }

    #[test]
    fn f18_success_with_or_without_a_prompt_is_the_one_normal_reply() {
        let mut after_prompt = Exchange::new();
        assert!(matches!(
            after_prompt.on_line(line("PAM_PROMPT_ECHO_OFF Password: ")),
            Step::Ask(_)
        ));
        after_prompt.answer_sent();
        assert_eq!(
            after_prompt.on_line(line("SUCCESS")),
            Step::End(Ending::Authenticated)
        );
        let mut bare = Exchange::new();
        assert_eq!(
            bare.on_line(line("SUCCESS")),
            Step::End(Ending::Authenticated)
        );
        assert_eq!(Ending::Authenticated.reply(), Some(Reply::Normal));
        assert_eq!(Ending::Authenticated.row(), 18);
    }

    #[test]
    fn more_lines_after_a_verdict_are_ignored() {
        let mut exchange = Exchange::new();
        assert_eq!(
            exchange.on_line(line("SUCCESS")),
            Step::End(Ending::Authenticated)
        );
        assert_eq!(exchange.on_line(line("FAILURE")), Step::Ignore);
        assert_eq!(exchange.on_line(line("PAM_ERROR_MSG late")), Step::Ignore);
        assert_eq!(exchange.on_eof(), Step::Ignore);
        assert_eq!(exchange.on_unreadable_line(), Step::Ignore);
        assert_eq!(exchange.on_silence(), Step::Ignore);
    }

    #[test]
    fn a_prompt_after_an_answer_in_the_same_conversation_is_asked_again() {
        // PAM may ask twice in one conversation (a second factor).
        let mut exchange = Exchange::new();
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF One: ")),
            Step::Ask(_)
        ));
        exchange.answer_sent();
        assert!(matches!(
            exchange.on_line(line("PAM_PROMPT_ECHO_OFF Two: ")),
            Step::Ask(_)
        ));
        exchange.answer_sent();
        assert_eq!(exchange.on_line(line("FAILURE")), Step::Retry);
        assert_eq!(exchange.failed_attempts(), 1);
    }

    // ---- the invariant of the failure table -----------------------------------------

    #[test]
    fn only_success_yields_a_normal_reply() {
        // An exhaustive match with no wildcard: a new `Ending` does not
        // compile until someone decides what it replies.
        fn expected(ending: Ending) -> Option<Reply> {
            match ending {
                Ending::Authenticated => Some(Reply::Normal),
                Ending::RefusedSender => Some(Reply::AccessDenied),
                Ending::Malformed
                | Ending::NoIdentity
                | Ending::HelperUnreachable
                | Ending::HelperProtocol
                | Ending::HelperEof
                | Ending::HelperSilent => Some(Reply::Failed),
                Ending::DuplicateCookie
                | Ending::NotShown
                | Ending::PromptUnavailable
                | Ending::KeyboardLost
                | Ending::ShellExit
                | Ending::EchoOnPrompt
                | Ending::AttemptsExhausted
                | Ending::Escaped
                | Ending::IdleTimeout
                | Ending::CancelledByPolkit
                | Ending::WorkerPanic => Some(Reply::Cancelled),
                Ending::OwnerChanged | Ending::ShellCrash => None,
            }
        }
        // Every variant, found through the same exhaustive match.
        fn ordinal(ending: Ending) -> usize {
            match ending {
                Ending::RefusedSender => 0,
                Ending::Malformed => 1,
                Ending::DuplicateCookie => 2,
                Ending::NoIdentity => 3,
                Ending::NotShown => 4,
                Ending::PromptUnavailable => 5,
                Ending::KeyboardLost => 6,
                Ending::OwnerChanged => 7,
                Ending::ShellExit => 8,
                Ending::ShellCrash => 9,
                Ending::HelperUnreachable => 10,
                Ending::HelperProtocol => 11,
                Ending::HelperEof => 12,
                Ending::HelperSilent => 13,
                Ending::EchoOnPrompt => 14,
                Ending::AttemptsExhausted => 15,
                Ending::Authenticated => 16,
                Ending::Escaped => 17,
                Ending::IdleTimeout => 18,
                Ending::CancelledByPolkit => 19,
                Ending::WorkerPanic => 20,
            }
        }
        let mut seen: Vec<usize> = Ending::ALL.iter().map(|ending| ordinal(*ending)).collect();
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..Ending::ALL.len()).collect::<Vec<_>>(),
            "ALL lists every variant once"
        );
        let mut normal = 0;
        for ending in Ending::ALL {
            assert_eq!(ending.reply(), expected(ending), "{ending:?}");
            if ending.reply() == Some(Reply::Normal) {
                normal += 1;
                assert_eq!(ending, Ending::Authenticated);
            }
        }
        assert_eq!(normal, 1);
        // The rows are unique and the ones this enum does not carry are the
        // ones that are not endings of a request.
        let mut rows: Vec<u8> = Ending::ALL.iter().map(|ending| ending.row()).collect();
        rows.sort_unstable();
        rows.dedup();
        assert_eq!(rows.len(), Ending::ALL.len());
        for missing in [16, 22, 24, 25] {
            assert!(!rows.contains(&missing), "F-{missing} is not an ending");
        }
    }

    #[test]
    fn a_refusal_replies_what_its_ending_replies() {
        for refusal in [
            Refusal::NotPolkit,
            Refusal::Malformed,
            Refusal::DuplicateCookie,
            Refusal::NoIdentity,
            Refusal::QueueFull,
            Refusal::RateLimited,
            Refusal::Cooldown,
            Refusal::Locked,
            Refusal::DutyOff,
        ] {
            assert_eq!(
                Some(refusal.reply()),
                refusal.ending().reply(),
                "{refusal:?}"
            );
            assert_ne!(refusal.reply(), Reply::Normal, "a refusal never grants");
        }
    }

    // ---- registration: F-8, F-24, F-25 ------------------------------------------------

    #[test]
    fn f24_a_refused_registration_is_final_until_the_person_acts() {
        let state = RegistrationState::Off;
        let t = state.on(RegistrationEvent::SwitchedOn);
        assert_eq!(
            t,
            Transition {
                state: RegistrationState::Waiting,
                register: true
            }
        );
        let t = t.state.on(RegistrationEvent::Held);
        assert_eq!(
            t,
            Transition {
                state: RegistrationState::Refused,
                register: false
            }
        );
        // Nothing retries: not a new owner, not an owner loss, not another
        // switch-on without a switch-off in between.
        for event in [
            RegistrationEvent::OwnerAppeared,
            RegistrationEvent::OwnerLost,
            RegistrationEvent::OwnerAppeared,
            RegistrationEvent::SwitchedOn,
        ] {
            let again = RegistrationState::Refused.on(event.clone());
            assert_eq!(
                again,
                Transition {
                    state: RegistrationState::Refused,
                    register: false
                },
                "{event:?}"
            );
        }
        // Switching off and on again is the retry, and it tries once.
        let off = RegistrationState::Refused.on(RegistrationEvent::SwitchedOff);
        assert_eq!(
            off,
            Transition {
                state: RegistrationState::Off,
                register: false
            }
        );
        let on = off.state.on(RegistrationEvent::SwitchedOn);
        assert_eq!(
            on,
            Transition {
                state: RegistrationState::Waiting,
                register: true
            }
        );
    }

    #[test]
    fn f25_no_bus_or_no_session_is_unavailable_and_attempts_nothing() {
        let t = RegistrationState::Waiting.on(RegistrationEvent::Unavailable("no session".into()));
        assert_eq!(
            t,
            Transition {
                state: RegistrationState::Unavailable("no session".into()),
                register: false
            }
        );
        // It stays unavailable on owner changes; the person switches it off
        // and on to try again.
        let again = t.state.clone().on(RegistrationEvent::OwnerAppeared);
        assert_eq!(
            again,
            Transition {
                state: t.state,
                register: false
            }
        );
    }

    #[test]
    fn registration_follows_polkitd_across_restarts() {
        let registered = RegistrationState::Waiting.on(RegistrationEvent::Accepted);
        assert_eq!(
            registered,
            Transition {
                state: RegistrationState::Registered,
                register: false
            }
        );
        // polkitd goes away: the registration is gone with it; wait.
        let lost = registered.state.on(RegistrationEvent::OwnerLost);
        assert_eq!(
            lost,
            Transition {
                state: RegistrationState::Waiting,
                register: false
            }
        );
        // A new owner: register once.
        let back = lost.state.on(RegistrationEvent::OwnerAppeared);
        assert_eq!(
            back,
            Transition {
                state: RegistrationState::Waiting,
                register: true
            }
        );
        // polkitd absent at activation: wait without registering.
        let absent = RegistrationState::Waiting.on(RegistrationEvent::PolkitdAbsent);
        assert_eq!(
            absent,
            Transition {
                state: RegistrationState::Waiting,
                register: false
            }
        );
        // Off stays off whatever happens on the bus.
        for event in [
            RegistrationEvent::OwnerAppeared,
            RegistrationEvent::OwnerLost,
            RegistrationEvent::Accepted,
        ] {
            assert_eq!(
                RegistrationState::Off.on(event).state,
                RegistrationState::Off
            );
        }
    }

    #[test]
    fn f08_a_new_owner_is_followed_by_one_registration_and_a_refusal_then_is_final() {
        let registered = RegistrationState::Registered;
        let back = registered.on(RegistrationEvent::OwnerAppeared);
        assert_eq!(
            back,
            Transition {
                state: RegistrationState::Waiting,
                register: true
            }
        );
        let refused = back.state.on(RegistrationEvent::Held);
        assert_eq!(refused.state, RegistrationState::Refused);
        assert!(!refused.register);
        assert_eq!(Ending::OwnerChanged.row(), 8);
    }
}
