// Adapted from `celestina-qt/src/provider_adapter/melibea.rs` at c7fb0b5d,
// read as reference and never edited: the bounded line read, the one-request
// short-lived action connection, the reply check (operation, requested and
// resolved window), the v2 → v1 retry of a hinted action and the confirmation
// by the authoritative projection (`Settles`). Rewritten for the shell's
// in-process workers: typed events through a callback instead of the
// provider runtime, a bounded action queue, the socket guard and the backoff
// of CEL-2-J decision D2, and the per-connection v1-only memory of D1.

//! The Melibea worker (CEL-2-J): the minimised windows of the session, read
//! from Melibea's socket off the event loop's thread.
//!
//! Two named threads, both stopped by one [`Worker`]:
//!
//! - **Subscription.** One long-lived connection sends `subscribe` (v1) and
//!   applies every state line to a [`Projection`]. Each line is at most
//!   [`MAX_MESSAGE_BYTES`]; reads time out every [`POLL`] so the shutdown flag
//!   is looked at. A revision gap, a broken line or a lost connection drops
//!   the projection ([`MelibeaEvent::Unavailable`]) and subscribes again for
//!   a fresh snapshot; an `unavailable` line withdraws it while staying
//!   connected, until Melibea's next snapshot. Reconnecting backs off from
//!   [`MIN_BACKOFF`] to [`MAX_BACKOFF`], doubling, in [`POLL`] slices.
//! - **Actions.** [`MelibeaHandle::request`] queues a [`BubbleAction`] (at most
//!   [`ACTION_QUEUE`] wait). Each goes over its own connection, one request,
//!   bounded by [`ACTION_TIMEOUT`]. Melibea's reply is published as
//!   [`MelibeaEvent::ActionResult`]; the action is settled
//!   ([`MelibeaEvent::ActionSettled`]) as confirmed only once the subscribed
//!   projection shows the requested state, so `close_requested` never removes
//!   a bubble by itself. `window_not_found` and `blocked` are reported, never
//!   retried. An action whose state does not show within
//!   [`CONFIRMATION_TIMEOUT`] is settled as unconfirmed.
//!
//! **Versions (D1).** Everything speaks v1 except an action that carries a
//! transition hint, which speaks v2. A Melibea that answers
//! `incompatible_version` gets the same action again as v1 without the hint,
//! and every later hinted action goes as v1 directly until the subscription
//! reconnects (the daemon may have been upgraded meanwhile).
//!
//! **Guard (D2).** The socket is `$MELIBEA_SOCKET` when it is an absolute
//! path, else `$XDG_RUNTIME_DIR/melibea.sock`. It is used only when it exists,
//! is a socket (a symbolic link is refused, not followed), belongs to this
//! process's uid and grants nothing to group or others. A missing socket is
//! [`MelibeaEvent::Absent`], an unacceptable one [`MelibeaEvent::Refused`].
//! The service itself is never started or asked about. Every change of state
//! writes one journal line; a repeated attempt writes none.
//!
//! Everything read from the socket is hostile: lines are bounded before they
//! are decoded, the projection validates every identity and text, and reasons
//! that reach the loop are cut to [`MAX_REASON`].

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::bounded::bounded;
use celestina_shell::melibea::{
    decode_message, encode_request, ClientEnvelope, ContractError, ErrorCode, Message, Operation,
    Projection, StateEffect, MAX_MESSAGE_BYTES,
};
pub use celestina_shell::melibea::{ActionStatus, Window, WindowTransition};

use super::Worker;

/// The variable that names the socket, when absolute.
pub const SOCKET_ENV: &str = "MELIBEA_SOCKET";
/// The directory the default socket lives in.
pub const RUNTIME_ENV: &str = "XDG_RUNTIME_DIR";
/// The default socket's name inside [`RUNTIME_ENV`].
pub const SOCKET_NAME: &str = "melibea.sock";
/// The first wait before connecting again.
pub const MIN_BACKOFF: Duration = Duration::from_millis(500);
/// The longest wait before connecting again.
pub const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// How often a blocked read or wait looks at the shutdown flag.
pub const POLL: Duration = Duration::from_millis(100);
/// The longest Melibea may take to answer one action.
pub const ACTION_TIMEOUT: Duration = Duration::from_secs(2);
/// The longest an accepted action waits for the projection to show it.
pub const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(4);
/// The actions that may wait for the worker. Restoring and closing are
/// deliberate gestures; past this a binding is repeating itself.
pub const ACTION_QUEUE: usize = 8;
/// The actions that may await confirmation at once.
pub const MAX_PENDING: usize = 8;
/// The most UTF-16 units of a reason that reaches the loop.
pub const MAX_REASON: usize = 240;
/// The longest one request write may take; requests are a few hundred bytes.
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);

/// Where the socket is looked for. [`MelibeaConfig::from_env`] in production;
/// tests name a socket in a directory of their own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MelibeaConfig {
    /// The value of [`SOCKET_ENV`].
    pub melibea_socket: Option<OsString>,
    /// The value of [`RUNTIME_ENV`].
    pub runtime_dir: Option<OsString>,
}

impl MelibeaConfig {
    /// The process environment, read once.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            melibea_socket: std::env::var_os(SOCKET_ENV),
            runtime_dir: std::env::var_os(RUNTIME_ENV),
        }
    }

    /// The socket at exactly `path`.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            melibea_socket: Some(path.into().into_os_string()),
            runtime_dir: None,
        }
    }
}

/// The socket path a configuration names, and whether a relative
/// [`SOCKET_ENV`] was set aside on the way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub path: Option<PathBuf>,
    pub ignored_relative: bool,
}

/// Resolves the socket path: an absolute [`SOCKET_ENV`], else
/// [`SOCKET_NAME`] in an absolute [`RUNTIME_ENV`], else none.
#[must_use]
pub fn resolve(config: &MelibeaConfig) -> Resolved {
    let absolute = |value: &Option<OsString>| {
        value
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let mut ignored_relative = false;
    if let Some(path) = absolute(&config.melibea_socket) {
        if path.is_absolute() {
            return Resolved {
                path: Some(path),
                ignored_relative,
            };
        }
        ignored_relative = true;
    }
    let path = absolute(&config.runtime_dir)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(SOCKET_NAME));
    Resolved {
        path,
        ignored_relative,
    }
}

/// Why a socket path is not used (D2).
#[derive(Debug, thiserror::Error)]
pub enum GuardError {
    #[error("{} does not exist", .0.display())]
    Missing(PathBuf),
    #[error("cannot inspect {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is a symbolic link", .0.display())]
    Symlink(PathBuf),
    #[error("{} is not a socket", .0.display())]
    NotSocket(PathBuf),
    #[error("{} belongs to uid {owner}, not {expected}", path.display())]
    Owner {
        path: PathBuf,
        owner: u32,
        expected: u32,
    },
    #[error("{} is open to group or others (mode {mode:o})", path.display())]
    Permissions { path: PathBuf, mode: u32 },
}

/// Accepts `path` only as a socket owned by `uid` and closed to group and
/// others. The link itself is inspected: a symbolic link is refused rather
/// than followed out of the runtime directory.
///
/// # Errors
///
/// [`GuardError`] naming the first property that does not hold.
pub fn check_socket(path: &Path, uid: u32) -> Result<(), GuardError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(GuardError::Missing(path.to_owned()));
        }
        Err(source) => {
            return Err(GuardError::Inspect {
                path: path.to_owned(),
                source,
            });
        }
    };
    let kind = meta.file_type();
    if kind.is_symlink() {
        return Err(GuardError::Symlink(path.to_owned()));
    }
    if !kind.is_socket() {
        return Err(GuardError::NotSocket(path.to_owned()));
    }
    if meta.uid() != uid {
        return Err(GuardError::Owner {
            path: path.to_owned(),
            owner: meta.uid(),
            expected: uid,
        });
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(GuardError::Permissions {
            path: path.to_owned(),
            mode,
        });
    }
    Ok(())
}

/// One queued action, named by the handle that queued it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u64);

/// What the loop asks Melibea to do with a window.
#[derive(Clone, Debug, PartialEq)]
pub enum BubbleAction {
    /// Bring a minimised window back, optionally with a presentation hint.
    Restore {
        window_id: u64,
        transition: Option<WindowTransition>,
    },
    /// Ask a minimised window to close. Melibea answers `close_requested`;
    /// the window leaves only when the projection says so.
    Close { window_id: u64 },
    /// Minimise whichever window niri finds focused (`window_id: null`),
    /// optionally with a presentation hint (`disabled` for reduced motion).
    MinimizeFocused {
        transition: Option<WindowTransition>,
    },
}

impl BubbleAction {
    const fn operation(&self) -> Operation {
        match self {
            Self::Restore { .. } => Operation::Restore,
            Self::Close { .. } => Operation::Close,
            Self::MinimizeFocused { .. } => Operation::Minimize,
        }
    }

    const fn window_id(&self) -> Option<u64> {
        match self {
            Self::Restore { window_id, .. } | Self::Close { window_id } => Some(*window_id),
            Self::MinimizeFocused { .. } => None,
        }
    }

    fn transition(&self) -> Option<&WindowTransition> {
        match self {
            Self::Restore { transition, .. } | Self::MinimizeFocused { transition } => {
                transition.as_ref()
            }
            Self::Close { .. } => None,
        }
    }

    /// The envelope for this action, with `transition` in place of its own.
    /// The protocol module chooses the version: v2 only with a hint.
    fn envelope(&self, transition: Option<WindowTransition>) -> Option<ClientEnvelope> {
        match self {
            Self::Restore { window_id, .. } => {
                ClientEnvelope::action_with_transition(Operation::Restore, *window_id, transition)
            }
            Self::Close { window_id } => ClientEnvelope::action(Operation::Close, *window_id),
            Self::MinimizeFocused { .. } => Some(ClientEnvelope::minimize_focused(transition)),
        }
    }
}

/// How an action ended. Every queued action ends in exactly one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settlement {
    /// The subscribed projection shows the requested state.
    Confirmed,
    /// Melibea did not act: `window_not_found` or `blocked`. Not retried.
    Refused(ActionStatus),
    /// Melibea accepted it, but the projection did not show the state in
    /// time or the subscription was lost first.
    Unconfirmed,
    /// It could not be asked or Melibea answered with an error; bounded.
    Failed(String),
}

/// What the worker tells the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MelibeaEvent {
    /// The complete current projection, in Melibea's order.
    Windows(Vec<Window>),
    /// No socket to connect to (or nobody listening on it).
    Absent,
    /// A socket path that is not trusted, or a subscription Melibea refused;
    /// the reason is bounded.
    Refused(String),
    /// The projection was withdrawn: the connection was lost or Melibea has
    /// no authoritative state for now.
    Unavailable,
    /// Melibea's reply to an action.
    ActionResult { id: ActionId, status: ActionStatus },
    /// How an action ended.
    ActionSettled {
        id: ActionId,
        settlement: Settlement,
    },
}

/// Why an action was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("the Melibea action queue is full")]
    Full,
    #[error("the Melibea worker has stopped")]
    Stopped,
}

#[derive(Debug)]
struct Queued {
    id: ActionId,
    action: BubbleAction,
}

/// The way into the worker: queues an action, never waits for it.
#[derive(Clone, Debug)]
pub struct MelibeaHandle {
    actions: SyncSender<Queued>,
    next: Arc<AtomicU64>,
}

impl MelibeaHandle {
    /// Queues `action` and returns the id its events will carry.
    ///
    /// # Errors
    ///
    /// [`RequestError`] when [`ACTION_QUEUE`] actions wait already or the
    /// worker has stopped.
    pub fn request(&self, action: BubbleAction) -> Result<ActionId, RequestError> {
        let id = ActionId(self.next.fetch_add(1, Ordering::Relaxed));
        self.actions
            .try_send(Queued { id, action })
            .map(|()| id)
            .map_err(|err| match err {
                TrySendError::Full(_) => RequestError::Full,
                TrySendError::Disconnected(_) => RequestError::Stopped,
            })
    }
}

/// Starts the worker with `config`, publishing through `publish`, which
/// returns `false` once nobody listens (both threads then stop). Both also
/// stop when `shutdown` is raised; the returned [`Worker`] joins them.
///
/// # Errors
///
/// The failure to start either thread.
pub fn spawn(
    config: &MelibeaConfig,
    publish: impl FnMut(MelibeaEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(MelibeaHandle, Worker)> {
    let resolved = resolve(config);
    if resolved.ignored_relative {
        tracing::warn!("melibea: {SOCKET_ENV} is not an absolute path; it is ignored");
    }
    let uid = rustix::process::getuid().as_raw();
    let shared = Arc::new(Shared {
        state: Mutex::new(State::default()),
        v1_only: AtomicBool::new(false),
        listening: AtomicBool::new(true),
        publisher: Mutex::new(Box::new(publish)),
        shutdown: Arc::clone(shutdown),
    });
    let (actions, queue) = mpsc::sync_channel(ACTION_QUEUE);
    let handle = MelibeaHandle {
        actions,
        next: Arc::new(AtomicU64::new(1)),
    };

    let action_shared = Arc::clone(&shared);
    let action_path = resolved.path.clone();
    let action_worker = Worker::spawn("melibea-actions", shutdown, move || {
        run_actions(&action_shared, &queue, action_path.as_deref(), uid);
    })?;
    let path = resolved.path;
    let worker = Worker::spawn("melibea", shutdown, move || {
        run_subscription(&shared, path.as_deref(), uid);
        // The subscription ends on shutdown or when nobody listens; the
        // action thread is stopped with it.
        action_worker.join();
    })?;
    Ok((handle, worker))
}

type Publish = Box<dyn FnMut(MelibeaEvent) -> bool + Send>;

/// What both threads share.
struct Shared {
    state: Mutex<State>,
    /// D1: this subscription's Melibea refused v2 once. Forgotten on every
    /// new subscription connection.
    v1_only: AtomicBool,
    /// Cleared once `publisher` returns `false`.
    listening: AtomicBool,
    publisher: Mutex<Publish>,
    shutdown: Arc<AtomicBool>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl Shared {
    fn running(&self) -> bool {
        !self.shutdown.load(Ordering::Acquire) && self.listening.load(Ordering::Acquire)
    }

    fn publish(&self, event: MelibeaEvent) {
        if !self.listening.load(Ordering::Acquire) {
            return;
        }
        let mut publisher = lock(&self.publisher);
        if !(publisher)(event) {
            self.listening.store(false, Ordering::Release);
        }
    }

    fn settle(&self, settled: Vec<(ActionId, Settlement)>) {
        for (id, settlement) in settled {
            self.publish(MelibeaEvent::ActionSettled { id, settlement });
        }
    }

    fn expire(&self) {
        let expired = lock(&self.state).expire(Instant::now());
        self.settle(expired);
    }

    /// Waits `delay` in [`POLL`] slices, expiring actions meanwhile.
    fn pause(&self, delay: Duration) {
        let until = Instant::now() + delay;
        while self.running() {
            let now = Instant::now();
            if now >= until {
                break;
            }
            self.expire();
            thread::sleep(POLL.min(until - now));
        }
    }
}

/// What the authoritative projection must say for an action to be confirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Settles {
    /// A minimise: the window is a bubble.
    WhenPresent,
    /// A restore or a close: the window is no longer a bubble.
    WhenAbsent,
}

impl Settles {
    const fn of(operation: Operation) -> Self {
        match operation {
            Operation::Minimize => Self::WhenPresent,
            Operation::Restore | Operation::Close => Self::WhenAbsent,
        }
    }

    fn holds(self, projection: &Projection, window_id: u64) -> bool {
        projection.ready()
            && match self {
                Self::WhenPresent => projection.contains(window_id),
                Self::WhenAbsent => !projection.contains(window_id),
            }
    }
}

#[derive(Debug)]
struct Pending {
    settles: Settles,
    /// The subscription connection it was reserved under.
    generation: u64,
    /// The window to watch and the deadline, once Melibea accepted it.
    armed: Option<(u64, Instant)>,
}

#[derive(Debug, Default)]
struct State {
    projection: Projection,
    /// Advances every time a subscription connection ends.
    generation: u64,
    pending: BTreeMap<ActionId, Pending>,
}

impl State {
    /// Reserves `id` before any IO: an action is sent only while an
    /// authoritative projection exists to confirm it against.
    fn reserve(&mut self, id: ActionId, settles: Settles) -> Result<(), ActionError> {
        if !self.projection.ready() {
            return Err(ActionError::NotReady);
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(ActionError::TooMany);
        }
        self.pending.insert(
            id,
            Pending {
                settles,
                generation: self.generation,
                armed: None,
            },
        );
        Ok(())
    }

    /// Melibea accepted `id` for `window_id`: watch for it, confirming at
    /// once when the projection already shows it.
    fn arm(&mut self, id: ActionId, window_id: u64, now: Instant) -> Option<Settlement> {
        let pending = self.pending.get(&id)?;
        if pending.generation != self.generation {
            self.pending.remove(&id);
            return Some(Settlement::Unconfirmed);
        }
        if pending.settles.holds(&self.projection, window_id) {
            self.pending.remove(&id);
            return Some(Settlement::Confirmed);
        }
        if let Some(pending) = self.pending.get_mut(&id) {
            pending.armed = Some((window_id, now + CONFIRMATION_TIMEOUT));
        }
        None
    }

    fn forget(&mut self, id: ActionId) {
        self.pending.remove(&id);
    }

    fn take(
        &mut self,
        mut which: impl FnMut(&Pending, &Projection) -> bool,
        settlement: &Settlement,
    ) -> Vec<(ActionId, Settlement)> {
        let ids: Vec<ActionId> = self
            .pending
            .iter()
            .filter(|(_, pending)| which(pending, &self.projection))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.pending.remove(&id).map(|_| (id, settlement.clone())))
            .collect()
    }

    /// The armed actions the projection now confirms.
    fn observe(&mut self) -> Vec<(ActionId, Settlement)> {
        self.take(
            |pending, projection| {
                pending
                    .armed
                    .is_some_and(|(window_id, _)| pending.settles.holds(projection, window_id))
            },
            &Settlement::Confirmed,
        )
    }

    fn expire(&mut self, now: Instant) -> Vec<(ActionId, Settlement)> {
        self.take(
            |pending, _| pending.armed.is_some_and(|(_, deadline)| deadline <= now),
            &Settlement::Unconfirmed,
        )
    }

    /// The subscription ended: drop the projection and every armed action;
    /// an action still in flight is settled when Melibea answers it.
    fn lose(&mut self) -> Vec<(ActionId, Settlement)> {
        self.projection = Projection::default();
        self.generation = self.generation.wrapping_add(1);
        self.take(
            |pending, _| pending.armed.is_some(),
            &Settlement::Unconfirmed,
        )
    }
}

/// How the link to Melibea stands, for the journal and the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Link {
    Subscribed,
    Absent(String),
    Refused(String),
    Unavailable(String),
}

/// The last [`Link`] reported: a change writes one line and one event; a
/// repeat writes nothing.
#[derive(Debug, Default)]
struct LinkLog {
    last: Option<Link>,
}

impl LinkLog {
    fn report(&mut self, shared: &Shared, link: Link) {
        if self.last.as_ref() == Some(&link) {
            return;
        }
        let event = match &link {
            Link::Subscribed => {
                tracing::info!("melibea: subscribed");
                None
            }
            Link::Absent(why) => {
                tracing::info!("melibea: absent: {why}");
                Some(MelibeaEvent::Absent)
            }
            Link::Refused(why) => {
                tracing::warn!("melibea: refused: {why}");
                Some(MelibeaEvent::Refused(why.clone()))
            }
            Link::Unavailable(why) => {
                tracing::warn!("melibea: unavailable: {why}");
                Some(MelibeaEvent::Unavailable)
            }
        };
        self.last = Some(link);
        if let Some(event) = event {
            shared.publish(event);
        }
    }
}

/// Why a subscription ended.
#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("no socket path: neither an absolute {SOCKET_ENV} nor an absolute {RUNTIME_ENV}")]
    NoPath,
    #[error("the Melibea socket is not used")]
    Guard(#[source] GuardError),
    #[error("cannot connect to Melibea")]
    Connect(#[source] io::Error),
    #[error("the Melibea subscription failed")]
    Io(#[source] io::Error),
    #[error("Melibea closed the subscription")]
    Closed,
    #[error("a line from Melibea is longer than {MAX_MESSAGE_BYTES} bytes")]
    LineTooLong,
    #[error("the subscription request could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("Melibea broke its protocol")]
    Contract(#[source] ContractError),
    #[error("Melibea refused the subscription ({code:?}): {message}")]
    Refused { code: ErrorCode, message: String },
    #[error("Melibea sent an action result on the subscription")]
    UnexpectedAction,
}

impl SessionError {
    fn link(&self) -> Link {
        let why = reason(self);
        match self {
            Self::NoPath | Self::Guard(GuardError::Missing(_)) => Link::Absent(why),
            Self::Guard(_) => Link::Refused(why),
            Self::Connect(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Link::Absent(why)
            }
            Self::Connect(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                Link::Refused(why)
            }
            Self::Refused {
                code: ErrorCode::IncompatibleVersion | ErrorCode::InvalidRequest,
                ..
            } => Link::Refused(why),
            _ => Link::Unavailable(why),
        }
    }
}

fn reason(err: &dyn std::error::Error) -> String {
    bounded(&crate::error_chain(err), MAX_REASON)
}

/// How one bounded read ended.
#[derive(Debug, PartialEq, Eq)]
enum LineRead {
    /// `line` holds one whole line, newline included.
    Line,
    /// The read timed out; the partial line is kept for the next read.
    Pending,
    /// The peer closed the connection.
    Closed,
    /// More than [`MAX_MESSAGE_BYTES`] arrived without a newline.
    TooLong,
}

/// Reads up to the next newline into `line`, never letting it grow past
/// [`MAX_MESSAGE_BYTES`] + 1 bytes. The same shape as the niri stream's
/// reader, with Melibea's bound instead of niri's.
fn read_line_bounded(reader: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<LineRead> {
    let room = (MAX_MESSAGE_BYTES + 1).saturating_sub(line.len());
    let room = u64::try_from(room).unwrap_or(u64::MAX);
    match reader.by_ref().take(room).read_until(b'\n', line) {
        Ok(_) if line.ends_with(b"\n") => Ok(LineRead::Line),
        Ok(_) if line.len() > MAX_MESSAGE_BYTES => Ok(LineRead::TooLong),
        Ok(_) => Ok(LineRead::Closed),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(LineRead::Pending)
        }
        Err(err) => Err(err),
    }
}

/// `line` without its newline (and a carriage return before it).
fn strip(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn run_subscription(shared: &Shared, path: Option<&Path>, uid: u32) {
    let mut link = LinkLog::default();
    let mut backoff = MIN_BACKOFF;
    while shared.running() {
        let ended = match path {
            Some(path) => session(shared, path, uid, &mut link, &mut backoff),
            None => Err(SessionError::NoPath),
        };
        let lost = lock(&shared.state).lose();
        shared.settle(lost);
        match ended {
            Ok(()) => break,
            Err(err) => link.report(shared, err.link()),
        }
        shared.pause(backoff);
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// One subscription connection, until it fails or the worker stops.
fn session(
    shared: &Shared,
    path: &Path,
    uid: u32,
    link: &mut LinkLog,
    backoff: &mut Duration,
) -> Result<(), SessionError> {
    check_socket(path, uid).map_err(SessionError::Guard)?;
    let request = encode_request(&ClientEnvelope::subscribe()).map_err(SessionError::Encode)?;
    let stream = UnixStream::connect(path).map_err(SessionError::Connect)?;
    stream
        .set_read_timeout(Some(POLL))
        .map_err(SessionError::Io)?;
    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .map_err(SessionError::Io)?;
    (&stream).write_all(&request).map_err(SessionError::Io)?;
    // A new connection may reach an upgraded daemon: v2 is tried again (D1).
    shared.v1_only.store(false, Ordering::Release);

    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while shared.running() {
        match read_line_bounded(&mut reader, &mut line).map_err(SessionError::Io)? {
            LineRead::Line => {
                let handled = handle(shared, strip(&line), link, backoff);
                line.clear();
                handled?;
            }
            LineRead::Pending => shared.expire(),
            LineRead::Closed => return Err(SessionError::Closed),
            LineRead::TooLong => return Err(SessionError::LineTooLong),
        }
    }
    Ok(())
}

/// Applies one subscription line and publishes what it changed.
fn handle(
    shared: &Shared,
    line: &[u8],
    link: &mut LinkLog,
    backoff: &mut Duration,
) -> Result<(), SessionError> {
    match decode_message(line).map_err(SessionError::Contract)? {
        message @ (Message::Snapshot { .. } | Message::Changes { .. }) => {
            let (windows, confirmed) = {
                let mut state = lock(&shared.state);
                let effect = state
                    .projection
                    .apply(message)
                    .map_err(SessionError::Contract)?;
                let windows =
                    (effect == StateEffect::Changed).then(|| state.projection.windows().to_vec());
                (windows, state.observe())
            };
            // Melibea is serving authoritative state again.
            *backoff = MIN_BACKOFF;
            link.report(shared, Link::Subscribed);
            if let Some(windows) = windows {
                shared.publish(MelibeaEvent::Windows(windows));
            }
            shared.settle(confirmed);
            Ok(())
        }
        Message::Unavailable { revision, reason } => {
            let why = bounded(&reason, MAX_REASON);
            lock(&shared.state)
                .projection
                .apply(Message::Unavailable { revision, reason })
                .map_err(SessionError::Contract)?;
            link.report(shared, Link::Unavailable(format!("Melibea says: {why}")));
            Ok(())
        }
        Message::Error(error) => Err(SessionError::Refused {
            code: error.code,
            message: bounded(&error.message, MAX_REASON),
        }),
        Message::ActionResult(_) => Err(SessionError::UnexpectedAction),
    }
}

/// Why one action could not be carried out.
#[derive(Debug, thiserror::Error)]
enum ActionError {
    #[error("Melibea has no authoritative window state to act on")]
    NotReady,
    #[error("{MAX_PENDING} Melibea actions already await confirmation")]
    TooMany,
    #[error("no Melibea socket path is configured")]
    NoPath,
    #[error("the Melibea socket is not used")]
    Guard(#[source] GuardError),
    #[error("cannot connect to Melibea")]
    Connect(#[source] io::Error),
    #[error("the Melibea action connection failed")]
    Io(#[source] io::Error),
    #[error("Melibea closed the action without an answer")]
    Closed,
    #[error("Melibea did not answer within {} ms", ACTION_TIMEOUT.as_millis())]
    TimedOut,
    #[error("the shell is stopping")]
    Stopping,
    #[error("a line from Melibea is longer than {MAX_MESSAGE_BYTES} bytes")]
    LineTooLong,
    #[error("the action has no protocol representation")]
    Unrepresentable,
    #[error("the action could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("Melibea's answer breaks its protocol")]
    Contract(#[source] ContractError),
    #[error("Melibea does not speak this request's protocol version")]
    IncompatibleVersion,
    #[error("Melibea refused the action ({code:?}): {message}")]
    Server { code: ErrorCode, message: String },
    #[error("Melibea answered for a different action or window")]
    Mismatch,
    #[error("Melibea answered without an action result")]
    NotAResult,
}

fn run_actions(shared: &Shared, queue: &Receiver<Queued>, path: Option<&Path>, uid: u32) {
    while shared.running() {
        match queue.recv_timeout(POLL) {
            Ok(queued) => perform(shared, queued, path, uid),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
    // Whatever still waits ends with the worker.
    while let Ok(Queued { id, .. }) = queue.try_recv() {
        shared.publish(MelibeaEvent::ActionSettled {
            id,
            settlement: Settlement::Failed(ActionError::Stopping.to_string()),
        });
    }
}

fn perform(shared: &Shared, queued: Queued, path: Option<&Path>, uid: u32) {
    let Queued { id, action } = queued;
    let operation = action.operation();
    let failed = |err: &ActionError| MelibeaEvent::ActionSettled {
        id,
        settlement: Settlement::Failed(reason(err)),
    };
    if let Err(err) = lock(&shared.state).reserve(id, Settles::of(operation)) {
        shared.publish(failed(&err));
        return;
    }
    let answered = match path {
        Some(path) => dispatch(shared, path, uid, &action),
        None => Err(ActionError::NoPath),
    };
    match answered {
        Ok((status, window_id)) => {
            shared.publish(MelibeaEvent::ActionResult { id, status });
            if !status.accepts_state_confirmation() {
                lock(&shared.state).forget(id);
                shared.publish(MelibeaEvent::ActionSettled {
                    id,
                    settlement: Settlement::Refused(status),
                });
                return;
            }
            let settled = lock(&shared.state).arm(id, window_id, Instant::now());
            if let Some(settlement) = settled {
                shared.publish(MelibeaEvent::ActionSettled { id, settlement });
            }
        }
        Err(err) => {
            lock(&shared.state).forget(id);
            tracing::warn!("melibea: {operation:?} failed: {}", reason(&err));
            shared.publish(failed(&err));
        }
    }
}

/// Sends `action`, as v2 when it carries a hint this subscription's Melibea
/// has not refused, and once more as v1 without it when Melibea answers
/// `incompatible_version` (D1). A refused envelope changed nothing, so the
/// retry is the same action, not a second one.
fn dispatch(
    shared: &Shared,
    path: &Path,
    uid: u32,
    action: &BubbleAction,
) -> Result<(ActionStatus, u64), ActionError> {
    let hint = action
        .transition()
        .filter(|_| !shared.v1_only.load(Ordering::Acquire))
        .cloned();
    let hinted = hint.is_some();
    let envelope = action.envelope(hint).ok_or(ActionError::Unrepresentable)?;
    match exchange(shared, path, uid, &envelope, action) {
        Err(ActionError::IncompatibleVersion) if hinted => {
            if !shared.v1_only.swap(true, Ordering::AcqRel) {
                tracing::info!(
                    "melibea: speaks v1 only; transitions are dropped until it reconnects"
                );
            }
            let plain = action.envelope(None).ok_or(ActionError::Unrepresentable)?;
            exchange(shared, path, uid, &plain, action)
        }
        answered => answered,
    }
}

/// One short-lived connection: one request, one bounded answer.
fn exchange(
    shared: &Shared,
    path: &Path,
    uid: u32,
    envelope: &ClientEnvelope,
    action: &BubbleAction,
) -> Result<(ActionStatus, u64), ActionError> {
    check_socket(path, uid).map_err(ActionError::Guard)?;
    let request = encode_request(envelope).map_err(ActionError::Encode)?;
    let stream = UnixStream::connect(path).map_err(ActionError::Connect)?;
    stream
        .set_read_timeout(Some(POLL))
        .map_err(ActionError::Io)?;
    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .map_err(ActionError::Io)?;
    (&stream).write_all(&request).map_err(ActionError::Io)?;

    let deadline = Instant::now() + ACTION_TIMEOUT;
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    loop {
        if !shared.running() {
            return Err(ActionError::Stopping);
        }
        match read_line_bounded(&mut reader, &mut line).map_err(ActionError::Io)? {
            LineRead::Line => break,
            LineRead::Pending if Instant::now() >= deadline => return Err(ActionError::TimedOut),
            LineRead::Pending => {}
            LineRead::Closed => return Err(ActionError::Closed),
            LineRead::TooLong => return Err(ActionError::LineTooLong),
        }
    }

    let requested = action.window_id();
    match decode_message(strip(&line)).map_err(ActionError::Contract)? {
        // A named action must be answered for exactly the window it named. A
        // focused minimise named none, so the reply says which window niri
        // resolved, and that is the one the confirmation watches.
        Message::ActionResult(result)
            if result.operation == action.operation() && result.requested_id == requested =>
        {
            let status = result.status;
            match (requested, result.window_id) {
                (Some(named), Some(resolved)) if named == resolved => Ok((status, resolved)),
                (None, Some(resolved)) => Ok((status, resolved)),
                // niri answers `window_not_found` with `window_id: null`, and
                // Melibea forwards it. A status that confirms nothing watches
                // no window, so a null id is acceptable there and is reported
                // as a refusal; an accepted status must name the window it
                // acted on, and a different id is always a mismatch.
                (named, None) if !status.accepts_state_confirmation() => {
                    Ok((status, named.unwrap_or(0)))
                }
                _ => Err(ActionError::Mismatch),
            }
        }
        Message::ActionResult(_) => Err(ActionError::Mismatch),
        Message::Error(error) if error.code == ErrorCode::IncompatibleVersion => {
            Err(ActionError::IncompatibleVersion)
        }
        Message::Error(error) => Err(ActionError::Server {
            code: error.code,
            message: bounded(&error.message, MAX_REASON),
        }),
        _ => Err(ActionError::NotAResult),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    use celestina_shell::melibea::BubbleAnchor;
    use serde_json::{json, Value};

    const WAIT: Duration = Duration::from_secs(5);
    const JOIN_WAIT: Duration = Duration::from_secs(2);

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("celestina-melibea-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a test directory");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .expect("a private test directory");
        dir
    }

    fn window(id: u64) -> Window {
        Window {
            id,
            app_id: Some("org.example.App".to_owned()),
            title: Some(format!("window {id}")),
            icon_name: None,
        }
    }

    fn window_json(id: u64) -> Value {
        json!({"id": id, "app_id": "org.example.App", "title": format!("window {id}"), "icon_name": null})
    }

    fn snapshot(revision: u64, ids: &[u64]) -> String {
        let windows: Vec<Value> = ids.iter().map(|id| window_json(*id)).collect();
        json!({"version": 1, "message": {"type": "snapshot", "revision": revision, "windows": windows}})
            .to_string()
    }

    fn removed(revision: u64, index: usize, id: u64) -> String {
        json!({"version": 1, "message": {"type": "changes", "revision": revision,
            "changes": [{"type": "removed", "index": index, "window_id": id}]}})
        .to_string()
    }

    fn result(version: u32, operation: &str, id: u64, status: &str) -> String {
        json!({"version": version, "message": {"type": "action_result", "operation": operation,
            "requested_id": id, "window_id": id, "status": status}})
        .to_string()
    }

    fn incompatible() -> String {
        json!({"version": 1, "message": {"type": "error", "code": "incompatible_version",
            "message": "unsupported version", "supported_versions": [1]}})
        .to_string()
    }

    /// A fake Melibea on a private socket: every connection's request is
    /// recorded; a `subscribe` gets its scripted lines and stays open, any
    /// other request gets the next queued reply and is closed.
    struct Fake {
        requests: Arc<Mutex<Vec<Value>>>,
        subscribers: Arc<Mutex<Vec<UnixStream>>>,
        replies: Arc<Mutex<VecDeque<String>>>,
        stop: Arc<AtomicBool>,
        server: Option<thread::JoinHandle<()>>,
    }

    impl Fake {
        fn start(path: &Path, sessions: Vec<Vec<String>>) -> Self {
            Self::start_mode(path, sessions, 0o600)
        }

        fn start_mode(path: &Path, sessions: Vec<Vec<String>>, mode: u32) -> Self {
            let listener = UnixListener::bind(path).expect("a test socket");
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .expect("the socket mode");
            listener.set_nonblocking(true).expect("a polled listener");
            let shared_requests: Arc<Mutex<Vec<Value>>> = Arc::default();
            let shared_subscribers: Arc<Mutex<Vec<UnixStream>>> = Arc::default();
            let shared_replies: Arc<Mutex<VecDeque<String>>> = Arc::default();
            let shared_sessions = Arc::new(Mutex::new(VecDeque::from(sessions)));
            let shared_stop: Arc<AtomicBool> = Arc::default();
            let (requests, subscribers, replies, sessions, stop) = (
                Arc::clone(&shared_requests),
                Arc::clone(&shared_subscribers),
                Arc::clone(&shared_replies),
                shared_sessions,
                Arc::clone(&shared_stop),
            );
            let server = thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                    };
                    stream.set_nonblocking(false).expect("a blocking stream");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .expect("a read timeout");
                    let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        continue;
                    }
                    let request: Value = serde_json::from_str(&line).expect("a JSON request");
                    let kind = request["request"]["type"].as_str().map(str::to_owned);
                    lock(&requests).push(request);
                    let mut stream = stream;
                    if kind.as_deref() == Some("subscribe") {
                        let script = lock(&sessions).pop_front().unwrap_or_default();
                        for line in script {
                            let _ = stream.write_all(format!("{line}\n").as_bytes());
                        }
                        lock(&subscribers).push(stream);
                    } else if let Some(reply) = lock(&replies).pop_front() {
                        let _ = stream.write_all(format!("{reply}\n").as_bytes());
                    }
                }
            });
            Self {
                requests: shared_requests,
                subscribers: shared_subscribers,
                replies: shared_replies,
                stop: shared_stop,
                server: Some(server),
            }
        }

        fn reply(&self, line: String) {
            lock(&self.replies).push_back(line);
        }

        fn push(&self, line: &str) {
            let mut subscribers = lock(&self.subscribers);
            let stream = subscribers.last_mut().expect("a subscriber");
            stream
                .write_all(format!("{line}\n").as_bytes())
                .expect("the line is written");
        }

        fn drop_subscribers(&self) {
            lock(&self.subscribers).clear();
        }

        fn requests(&self) -> Vec<Value> {
            lock(&self.requests).clone()
        }

        fn subscriptions(&self) -> usize {
            self.requests()
                .iter()
                .filter(|request| request["request"]["type"] == "subscribe")
                .count()
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(server) = self.server.take() {
                let _ = server.join();
            }
        }
    }

    struct Running {
        handle: MelibeaHandle,
        worker: Worker,
        events: Receiver<MelibeaEvent>,
    }

    fn start(config: &MelibeaConfig) -> Running {
        let (tx, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (handle, worker) = spawn(config, move |event| tx.send(event).is_ok(), &shutdown)
            .expect("the worker starts");
        Running {
            handle,
            worker,
            events,
        }
    }

    impl Running {
        /// The next event that `want` accepts, skipping others.
        fn until(&self, want: impl Fn(&MelibeaEvent) -> bool) -> MelibeaEvent {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.events.recv_timeout(left) {
                    Ok(event) if want(&event) => return event,
                    Ok(_) => {}
                    Err(_) => panic!("the expected event did not arrive"),
                }
            }
        }

        fn next(&self) -> MelibeaEvent {
            self.events.recv_timeout(WAIT).expect("an event")
        }

        /// Every event within `quiet`.
        fn drain(&self, quiet: Duration) -> Vec<MelibeaEvent> {
            let mut seen = Vec::new();
            while let Ok(event) = self.events.recv_timeout(quiet) {
                seen.push(event);
            }
            seen
        }

        fn stop(self) {
            assert!(self.worker.join_within(JOIN_WAIT), "the worker stops");
        }
    }

    fn windows(ids: &[u64]) -> MelibeaEvent {
        MelibeaEvent::Windows(ids.iter().map(|id| window(*id)).collect())
    }

    fn settled(event: &MelibeaEvent) -> bool {
        matches!(event, MelibeaEvent::ActionSettled { .. })
    }

    #[test]
    fn the_socket_path_is_an_absolute_override_or_the_runtime_directory() {
        let config = MelibeaConfig {
            melibea_socket: Some("/run/test/m.sock".into()),
            runtime_dir: Some("/run/user/1000".into()),
        };
        assert_eq!(
            resolve(&config).path,
            Some(PathBuf::from("/run/test/m.sock"))
        );

        let relative = MelibeaConfig {
            melibea_socket: Some("m.sock".into()),
            runtime_dir: Some("/run/user/1000".into()),
        };
        assert_eq!(
            resolve(&relative),
            Resolved {
                path: Some(PathBuf::from("/run/user/1000/melibea.sock")),
                ignored_relative: true,
            }
        );

        let nothing = MelibeaConfig {
            melibea_socket: Some("".into()),
            runtime_dir: Some("relative/dir".into()),
        };
        assert_eq!(resolve(&nothing).path, None);
        assert_eq!(resolve(&MelibeaConfig::default()).path, None);
    }

    #[test]
    fn the_guard_refuses_files_links_lax_modes_and_other_owners() {
        let dir = scratch("guard");
        let uid = rustix::process::getuid().as_raw();
        let socket = dir.join("melibea.sock");
        let _listener = UnixListener::bind(&socket).expect("a socket");
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(check_socket(&socket, uid).is_ok());
        assert!(matches!(
            check_socket(&socket, uid.wrapping_add(1)),
            Err(GuardError::Owner { .. })
        ));

        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660)).expect("mode");
        assert!(matches!(
            check_socket(&socket, uid),
            Err(GuardError::Permissions { mode: 0o660, .. })
        ));

        let file = dir.join("file.sock");
        std::fs::write(&file, b"").expect("a file");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(matches!(
            check_socket(&file, uid),
            Err(GuardError::NotSocket(_))
        ));

        let link = dir.join("link.sock");
        std::os::unix::fs::symlink(&socket, &link).expect("a link");
        assert!(matches!(
            check_socket(&link, uid),
            Err(GuardError::Symlink(_))
        ));

        assert!(matches!(
            check_socket(&dir.join("missing.sock"), uid),
            Err(GuardError::Missing(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_snapshot_then_changes_publish_the_whole_current_projection() {
        let dir = scratch("changes");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(&path, vec![vec![snapshot(1, &[1, 2]), removed(2, 0, 1)]]);
        let running = start(&MelibeaConfig::at(&path));

        assert_eq!(running.next(), windows(&[1, 2]));
        assert_eq!(running.next(), windows(&[2]));
        assert_eq!(
            fake.requests()[0],
            json!({"version": 1, "request": {"type": "subscribe"}})
        );
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_revision_gap_withdraws_the_projection_and_resubscribes() {
        let dir = scratch("gap");
        let path = dir.join("melibea.sock");
        let gap =
            json!({"version": 1, "message": {"type": "changes", "revision": 3, "changes": []}})
                .to_string();
        let fake = Fake::start(
            &path,
            vec![vec![snapshot(1, &[1]), gap], vec![snapshot(5, &[2])]],
        );
        let running = start(&MelibeaConfig::at(&path));

        assert_eq!(running.next(), windows(&[1]));
        assert_eq!(running.next(), MelibeaEvent::Unavailable);
        assert_eq!(running.next(), windows(&[2]));
        assert_eq!(fake.subscriptions(), 2);
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unavailable_withdraws_until_the_next_snapshot_on_the_same_connection() {
        let dir = scratch("unavailable");
        let path = dir.join("melibea.sock");
        let unavailable = json!({"version": 1, "message": {"type": "unavailable",
            "revision": 1, "reason": "niri connection lost"}})
        .to_string();
        let fake = Fake::start(
            &path,
            vec![vec![snapshot(1, &[1]), unavailable, snapshot(2, &[2])]],
        );
        let running = start(&MelibeaConfig::at(&path));

        assert_eq!(running.next(), windows(&[1]));
        assert_eq!(running.next(), MelibeaEvent::Unavailable);
        assert_eq!(running.next(), windows(&[2]));
        assert_eq!(fake.subscriptions(), 1);
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_oversized_line_drops_the_connection_and_resubscribes() {
        let dir = scratch("oversized");
        let path = dir.join("melibea.sock");
        let hostile = "x".repeat(MAX_MESSAGE_BYTES + 4096);
        let fake = Fake::start(
            &path,
            vec![vec![snapshot(1, &[1]), hostile], vec![snapshot(2, &[2])]],
        );
        let running = start(&MelibeaConfig::at(&path));

        assert_eq!(running.next(), windows(&[1]));
        assert_eq!(running.next(), MelibeaEvent::Unavailable);
        assert_eq!(running.next(), windows(&[2]));
        assert_eq!(fake.subscriptions(), 2);
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn untrusted_paths_are_refused_without_connecting() {
        let dir = scratch("untrusted");

        let file = dir.join("file.sock");
        std::fs::write(&file, b"").expect("a file");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("mode");
        let running = start(&MelibeaConfig::at(&file));
        assert!(matches!(running.next(), MelibeaEvent::Refused(_)));
        running.stop();

        let real = dir.join("real.sock");
        let fake = Fake::start(&real, vec![vec![snapshot(1, &[1])]]);
        let link = dir.join("link.sock");
        std::os::unix::fs::symlink(&real, &link).expect("a link");
        let running = start(&MelibeaConfig::at(&link));
        assert!(matches!(running.next(), MelibeaEvent::Refused(_)));
        running.stop();
        assert!(fake.requests().is_empty(), "a link is never followed");

        let lax = dir.join("lax.sock");
        let lax_fake = Fake::start_mode(&lax, vec![vec![snapshot(1, &[1])]], 0o666);
        let running = start(&MelibeaConfig::at(&lax));
        assert!(matches!(running.next(), MelibeaEvent::Refused(_)));
        // Still refused on the retry, and reported only once.
        assert!(running.drain(Duration::from_millis(800)).is_empty());
        running.stop();
        assert!(lax_fake.requests().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_socket_is_absent_once_and_connects_when_it_appears() {
        let dir = scratch("appears");
        let path = dir.join("melibea.sock");
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), MelibeaEvent::Absent);

        thread::sleep(Duration::from_millis(1200));
        let _fake = Fake::start(&path, vec![vec![snapshot(1, &[7])]]);
        let seen = {
            let mut seen = Vec::new();
            loop {
                let event = running.next();
                let done = event == windows(&[7]);
                seen.push(event);
                if done {
                    break seen;
                }
            }
        };
        assert_eq!(seen, vec![windows(&[7])], "the absence was reported once");
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_joins_promptly_while_backing_off_and_while_subscribed() {
        let dir = scratch("shutdown");
        let missing = start(&MelibeaConfig::at(dir.join("missing.sock")));
        assert_eq!(missing.next(), MelibeaEvent::Absent);
        thread::sleep(Duration::from_millis(1600));
        let started = Instant::now();
        missing.stop();
        assert!(started.elapsed() < Duration::from_millis(600));

        let path = dir.join("melibea.sock");
        let _fake = Fake::start(&path, vec![vec![snapshot(1, &[1])]]);
        let subscribed = start(&MelibeaConfig::at(&path));
        assert_eq!(subscribed.next(), windows(&[1]));
        let started = Instant::now();
        subscribed.stop();
        assert!(started.elapsed() < Duration::from_millis(600));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn close_requested_keeps_the_bubble_until_the_revision_removes_it() {
        let dir = scratch("close");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(&path, vec![vec![snapshot(1, &[1, 2])]]);
        fake.reply(result(1, "close", 1, "close_requested"));
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), windows(&[1, 2]));

        let id = running
            .handle
            .request(BubbleAction::Close { window_id: 1 })
            .expect("queued");
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionResult {
                id,
                status: ActionStatus::CloseRequested
            }
        );
        // Melibea's reply is not the state: nothing settles, nothing moves.
        assert!(running.drain(Duration::from_millis(400)).is_empty());

        fake.push(&removed(2, 0, 1));
        assert_eq!(running.next(), windows(&[2]));
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionSettled {
                id,
                settlement: Settlement::Confirmed
            }
        );
        assert_eq!(
            fake.requests()[1],
            json!({"version": 1, "request": {"type": "close", "window_id": 1}})
        );
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_or_blocked_window_is_reported_and_not_retried() {
        let dir = scratch("notfound");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(&path, vec![vec![snapshot(1, &[1])]]);
        fake.reply(result(1, "restore", 9, "window_not_found"));
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), windows(&[1]));

        let id = running
            .handle
            .request(BubbleAction::Restore {
                window_id: 9,
                transition: None,
            })
            .expect("queued");
        assert_eq!(
            running.until(settled),
            MelibeaEvent::ActionSettled {
                id,
                settlement: Settlement::Refused(ActionStatus::WindowNotFound)
            }
        );
        thread::sleep(Duration::from_millis(200));
        assert_eq!(fake.requests().len(), 2, "one subscription, one action");
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn null_id_result(operation: &str, requested: Option<u64>, status: &str) -> String {
        json!({"version": 1, "message": {"type": "action_result", "operation": operation,
            "requested_id": requested, "window_id": null, "status": status}})
        .to_string()
    }

    #[test]
    fn niri_answering_not_found_with_a_null_window_id_is_a_refusal() {
        let dir = scratch("nullid");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(&path, vec![vec![snapshot(1, &[1])]]);
        fake.reply(null_id_result("restore", Some(9), "window_not_found"));
        fake.reply(null_id_result("minimize", None, "window_not_found"));
        fake.reply(null_id_result("close", Some(1), "applied"));
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), windows(&[1]));

        let restore = running
            .handle
            .request(BubbleAction::Restore {
                window_id: 9,
                transition: None,
            })
            .expect("queued");
        assert_eq!(
            running.until(settled),
            MelibeaEvent::ActionSettled {
                id: restore,
                settlement: Settlement::Refused(ActionStatus::WindowNotFound)
            }
        );
        // A focused minimise with no focused window names none and gets none.
        let minimize = running
            .handle
            .request(BubbleAction::MinimizeFocused { transition: None })
            .expect("queued");
        assert_eq!(
            running.until(settled),
            MelibeaEvent::ActionSettled {
                id: minimize,
                settlement: Settlement::Refused(ActionStatus::WindowNotFound)
            }
        );
        // An accepted status must still name its window.
        let close = running
            .handle
            .request(BubbleAction::Close { window_id: 1 })
            .expect("queued");
        assert!(matches!(
            running.until(settled),
            MelibeaEvent::ActionSettled {
                id,
                settlement: Settlement::Failed(_)
            } if id == close
        ));
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_action_without_authoritative_state_is_not_sent() {
        let dir = scratch("notready");
        let running = start(&MelibeaConfig::at(dir.join("missing.sock")));
        assert_eq!(running.next(), MelibeaEvent::Absent);
        let id = running
            .handle
            .request(BubbleAction::Close { window_id: 1 })
            .expect("queued");
        assert!(matches!(
            running.next(),
            MelibeaEvent::ActionSettled { id: settled_id, settlement: Settlement::Failed(_) }
                if settled_id == id
        ));
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn incompatible_version_degrades_to_v1_and_is_remembered_until_reconnecting() {
        let dir = scratch("degrade");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(
            &path,
            vec![vec![snapshot(1, &[1, 2])], vec![snapshot(10, &[3])]],
        );
        fake.reply(incompatible());
        fake.reply(result(1, "restore", 1, "applied"));
        fake.reply(result(1, "restore", 2, "applied"));
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), windows(&[1, 2]));

        let anchor = BubbleAnchor::new("DP-1", 10., 0., 22., 22.).expect("an anchor");
        let first = running
            .handle
            .request(BubbleAction::Restore {
                window_id: 1,
                transition: Some(WindowTransition::Anchored { anchor }),
            })
            .expect("queued");
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionResult {
                id: first,
                status: ActionStatus::Applied
            }
        );
        let second = running
            .handle
            .request(BubbleAction::Restore {
                window_id: 2,
                transition: Some(WindowTransition::Disabled),
            })
            .expect("queued");
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionResult {
                id: second,
                status: ActionStatus::Applied
            }
        );
        let requests = fake.requests();
        assert_eq!(requests[1]["version"], 2);
        assert!(requests[1]["request"].get("transition").is_some());
        assert_eq!(
            requests[2],
            json!({"version": 1, "request": {"type": "restore", "window_id": 1}}),
            "the same action again, without the hint"
        );
        assert_eq!(
            requests[3],
            json!({"version": 1, "request": {"type": "restore", "window_id": 2}}),
            "remembered: no second v2 attempt"
        );

        // Reconnecting forgets: the daemon may have been upgraded.
        fake.drop_subscribers();
        assert_eq!(
            running.until(|e| matches!(e, MelibeaEvent::Windows(_))),
            windows(&[3])
        );
        fake.reply(result(2, "restore", 3, "applied"));
        let third = running
            .handle
            .request(BubbleAction::Restore {
                window_id: 3,
                transition: Some(WindowTransition::Disabled),
            })
            .expect("queued");
        assert_eq!(
            running.until(|e| matches!(e, MelibeaEvent::ActionResult { .. })),
            MelibeaEvent::ActionResult {
                id: third,
                status: ActionStatus::Applied
            }
        );
        let last = fake.requests().pop().expect("a request");
        assert_eq!(
            last,
            json!({"version": 2, "request": {"type": "restore", "window_id": 3,
                "transition": {"type": "disabled"}}})
        );
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_focused_minimize_is_confirmed_by_the_window_niri_resolved() {
        let dir = scratch("minimize");
        let path = dir.join("melibea.sock");
        let fake = Fake::start(&path, vec![vec![snapshot(1, &[])]]);
        fake.reply(
            json!({"version": 1, "message": {"type": "action_result", "operation": "minimize",
                "requested_id": null, "window_id": 5, "status": "applied"}})
            .to_string(),
        );
        let running = start(&MelibeaConfig::at(&path));
        assert_eq!(running.next(), windows(&[]));
        let id = running
            .handle
            .request(BubbleAction::MinimizeFocused { transition: None })
            .expect("queued");
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionResult {
                id,
                status: ActionStatus::Applied
            }
        );
        let added = json!({"version": 1, "message": {"type": "changes", "revision": 2,
            "changes": [{"type": "added", "index": 0, "window": window_json(5)}]}})
        .to_string();
        fake.push(&added);
        assert_eq!(running.next(), windows(&[5]));
        assert_eq!(
            running.next(),
            MelibeaEvent::ActionSettled {
                id,
                settlement: Settlement::Confirmed
            }
        );
        assert_eq!(
            fake.requests()[1],
            json!({"version": 1, "request": {"type": "minimize", "window_id": null}})
        );
        running.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_state_settles_every_accepted_action_exactly_once() {
        let mut state = State::default();
        state
            .projection
            .apply(Message::Snapshot {
                revision: 1,
                windows: vec![window(1)],
            })
            .expect("a snapshot");
        let now = Instant::now();
        state
            .reserve(ActionId(1), Settles::WhenAbsent)
            .expect("reserved");
        assert_eq!(state.arm(ActionId(1), 1, now), None);
        assert_eq!(
            state.expire(now + CONFIRMATION_TIMEOUT),
            vec![(ActionId(1), Settlement::Unconfirmed)]
        );
        assert!(state.expire(now + CONFIRMATION_TIMEOUT * 2).is_empty());

        // An action in flight when the subscription is lost is unconfirmed
        // once Melibea answers it, never confirmed against a new connection.
        state
            .reserve(ActionId(2), Settles::WhenAbsent)
            .expect("reserved");
        assert!(state.lose().is_empty(), "not armed yet");
        assert_eq!(
            state.arm(ActionId(2), 1, now),
            Some(Settlement::Unconfirmed)
        );
        assert!(matches!(
            state.reserve(ActionId(3), Settles::WhenAbsent),
            Err(ActionError::NotReady)
        ));
    }
}
