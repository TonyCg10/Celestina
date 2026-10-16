//! The compositor: niri's event stream reduced to a [`NiriModel`], and the
//! focus requests the Island sends back.
//!
//! Started from the spike's `isla-spike/src/niri.rs` (outside this
//! repository) and extended: the model carries windows, outputs and each
//! workspace's column count, text from the compositor is bounded, bursts are
//! coalesced, the worker stops on the shell's shutdown flag, reconnection is
//! logged once per state change, and requests go through a bounded queue to a
//! second worker (design §5.6).
//!
//! Two workers, both started by [`spawn`]:
//!
//! - **stream** — opens `$NIRI_SOCKET`, sends `EventStream` and reads lines.
//!   Each line is parsed on its own ([`parse_event`]) rather than through
//!   `niri_ipc::socket::Socket::read_events`, because the author's patched
//!   niri emits events newer than niri-ipc 26.4.0 (`MinimizedWindowsChanged`):
//!   an unknown variant is logged once per name and skipped, and the stream
//!   continues. A line that is complete JSON but does not decode is logged once
//!   per name and skipped too. Only a closed connection (which is also what a
//!   truncated line means) or a failure to connect ends the session; the worker
//!   then publishes an empty model, so the Island shows its fallback, and
//!   reconnects after one second.
//! - **requests** — takes [`NiriRequest`]s from a queue of [`REQUEST_QUEUE`]
//!   and sends each as a typed `Request::Action` over a short-lived socket of
//!   its own. A full queue refuses at once ([`NiriProvider::request`]), never
//!   building a backlog.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::bounded::{bounded, MAX_TEXT_UNITS};
use celestina_shell::coalesce::Coalescer;
use celestina_shell::workspace_groups::MAX_NAME_CHARS;
use celestina_shell::workspace_map::MAX_APP_ID_CHARS;
use celestina_wayland::Sender;
use niri_ipc::socket::{Socket, SOCKET_PATH_ENV};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use niri_ipc::{Action, Event, Reply, Request, Response, WorkspaceReferenceArg};

use crate::message::Message;
use crate::providers::Worker;

/// How many focus requests may wait for the request worker. A person cannot
/// click faster than niri answers; a full queue is a stuck compositor, and
/// the request is refused visibly rather than queued behind it.
pub const REQUEST_QUEUE: usize = 32;
/// How long the stream worker waits before it connects again.
pub const RECONNECT_DELAY: Duration = Duration::from_secs(1);
/// The longest line the stream worker reads. A `WindowsChanged` for a few
/// hundred windows is tens of kilobytes; a line past this is a broken peer.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
/// How many workspaces the model carries, after sorting.
pub const MAX_WORKSPACES: usize = 256;
/// How many windows the model carries, after sorting.
pub const MAX_WINDOWS: usize = 1024;
/// How many distinct skipped variant names are remembered for the
/// once-per-name log. Past this, further new names are not logged.
pub const MAX_SKIPPED_NAMES: usize = 64;
/// The longest variant name [`variant_name`] reports.
const MAX_VARIANT_NAME: usize = 64;
/// How often a blocked worker looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(250);

/// One workspace, as the Island shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: u64,
    /// Its index on its output, from 1.
    pub idx: u8,
    pub name: Option<String>,
    /// The output it is on now.
    pub output: Option<String>,
    /// Whether it is the active workspace of its output.
    pub is_active: bool,
    /// Whether it is the focused workspace of the session.
    pub is_focused: bool,
    /// How many columns of the scrolling layout hold its tiled windows.
    pub columns: u16,
}

/// One window, as the Island shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
}

/// The compositor's state, reduced to what the shell draws.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NiriModel {
    /// Sorted by output, then index, then id.
    pub workspaces: Vec<Workspace>,
    /// Sorted by workspace, then id.
    pub windows: Vec<Window>,
    /// The outputs that hold a workspace, sorted. niri-ipc's
    /// `EventStreamState` carries no output list of its own, and every
    /// connected output holds at least one workspace.
    pub outputs: Vec<String>,
}

/// What the shell may ask the compositor to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NiriRequest {
    FocusWorkspace(u64),
    FocusWindow(u64),
}

impl NiriRequest {
    /// The typed niri action this request sends.
    #[must_use]
    pub fn action(self) -> Action {
        match self {
            Self::FocusWorkspace(id) => Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Id(id),
            },
            Self::FocusWindow(id) => Action::FocusWindow { id },
        }
    }
}

/// The loop's handle on the compositor: the bounded request queue.
#[derive(Debug)]
pub struct NiriProvider {
    pub requests: SyncSender<NiriRequest>,
}

impl NiriProvider {
    /// Queues a request for the request worker.
    ///
    /// # Errors
    ///
    /// [`Message::NiriRequestRefused`] when [`REQUEST_QUEUE`] requests are
    /// already waiting or the worker has stopped; the caller handles it as it
    /// handles any message, by showing the failure.
    pub fn request(&self, request: NiriRequest) -> Result<(), Message> {
        match self.requests.try_send(request) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(refused) | TrySendError::Disconnected(refused)) => {
                Err(Message::NiriRequestRefused(refused))
            }
        }
    }
}

/// Why one line of the event stream did not become an event.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The line ends before its JSON value does: the peer closed the
    /// connection mid-line. A protocol error that ends the session.
    #[error("the line ends before its JSON value does")]
    Truncated(#[source] serde_json::Error),
    /// The line is not a niri-ipc 26.4.0 event although it is complete: a
    /// known variant whose body changed shape, or not JSON at all. Logged once
    /// per name and skipped.
    #[error("the {} event does not decode", .variant.as_deref().unwrap_or("unnamed"))]
    Invalid {
        variant: Option<String>,
        #[source]
        source: serde_json::Error,
    },
}

impl ParseError {
    /// The line's outer key, when it had one worth logging.
    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        match self {
            Self::Truncated(_) => None,
            Self::Invalid { variant, .. } => variant.as_deref(),
        }
    }
}

/// Parses one line of the event stream.
///
/// `Ok(None)` is an event this niri-ipc does not know (its name is the
/// line's outer key, [`variant_name`], logged once per name by the caller).
///
/// # Errors
///
/// [`ParseError::Truncated`] for a line that ends inside its JSON value, and
/// [`ParseError::Invalid`] for anything else that does not decode.
pub fn parse_event(line: &str) -> Result<Option<Event>, ParseError> {
    match serde_json::from_str(line) {
        Ok(event) => Ok(Some(event)),
        Err(error) if error.is_eof() => Err(ParseError::Truncated(error)),
        Err(error) => {
            let variant = variant_name(line);
            let unknown = error.is_data()
                && variant.as_deref().is_some_and(|name| {
                    error
                        .to_string()
                        .starts_with(&format!("unknown variant `{name}`"))
                });
            if unknown {
                Ok(None)
            } else {
                Err(ParseError::Invalid {
                    variant,
                    source: error,
                })
            }
        }
    }
}

/// The variant a line names: the outer key of `{"Name":…}`, or the string of
/// a unit variant `"Name"`. Only a plain identifier of at most 64 characters
/// counts, so a hostile line cannot put anything else in a log.
#[must_use]
pub fn variant_name(line: &str) -> Option<String> {
    let line = line.trim_start();
    let quoted = match line.strip_prefix('{') {
        Some(object) => object.trim_start(),
        None => line,
    };
    let rest = quoted.strip_prefix('"')?;
    let name = &rest[..rest.find('"')?];
    let plain = !name.is_empty()
        && name.len() <= MAX_VARIANT_NAME
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    plain.then(|| name.to_owned())
}

/// Reduces niri's own event-stream state to the shell's model.
#[must_use]
pub fn reduce(state: &EventStreamState) -> NiriModel {
    let mut columns: HashMap<u64, BTreeSet<usize>> = HashMap::new();
    for window in state.windows.windows.values() {
        if let (Some(workspace), Some((column, _))) =
            (window.workspace_id, window.layout.pos_in_scrolling_layout)
        {
            columns.entry(workspace).or_default().insert(column);
        }
    }

    let mut workspaces: Vec<Workspace> = state
        .workspaces
        .workspaces
        .values()
        .map(|workspace| Workspace {
            id: workspace.id,
            idx: workspace.idx,
            name: workspace
                .name
                .as_deref()
                .map(|name| bounded(name, MAX_NAME_CHARS)),
            output: workspace
                .output
                .as_deref()
                .map(|output| bounded(output, MAX_NAME_CHARS)),
            is_active: workspace.is_active,
            is_focused: workspace.is_focused,
            columns: columns
                .get(&workspace.id)
                .map_or(0, |set| u16::try_from(set.len()).unwrap_or(u16::MAX)),
        })
        .collect();
    workspaces.sort_by(|a, b| {
        a.output
            .cmp(&b.output)
            .then(a.idx.cmp(&b.idx))
            .then(a.id.cmp(&b.id))
    });
    workspaces.truncate(MAX_WORKSPACES);

    let mut windows: Vec<Window> = state
        .windows
        .windows
        .values()
        .map(|window| Window {
            id: window.id,
            title: window
                .title
                .as_deref()
                .map(|title| bounded(title, MAX_TEXT_UNITS)),
            app_id: window
                .app_id
                .as_deref()
                .map(|app_id| bounded(app_id, MAX_APP_ID_CHARS)),
            workspace_id: window.workspace_id,
            is_focused: window.is_focused,
        })
        .collect();
    windows.sort_by(|a, b| a.workspace_id.cmp(&b.workspace_id).then(a.id.cmp(&b.id)));
    windows.truncate(MAX_WINDOWS);

    let mut outputs: Vec<String> = workspaces
        .iter()
        .filter_map(|workspace| workspace.output.clone())
        .collect();
    outputs.dedup();

    NiriModel {
        workspaces,
        windows,
        outputs,
    }
}

/// Whether a connection failure is worth a log line: the first failure after
/// a success (or at start) is, a retry once a second that fails the same way
/// is not, and the recovery is once more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionLog {
    failing: bool,
}

impl ConnectionLog {
    #[must_use]
    pub const fn new() -> Self {
        Self { failing: false }
    }

    /// A connection attempt or session failed. Returns whether to log it.
    pub fn failed(&mut self) -> bool {
        !std::mem::replace(&mut self.failing, true)
    }

    /// A session started. Returns whether to log the recovery.
    pub fn connected(&mut self) -> bool {
        std::mem::replace(&mut self.failing, false)
    }
}

/// The names of the variants already logged as skipped, bounded.
#[derive(Debug, Default)]
pub struct SkipLog {
    names: HashSet<String>,
}

impl SkipLog {
    /// Whether `name` is skipped for the first time (and should be logged).
    /// Once [`MAX_SKIPPED_NAMES`] are remembered, a new name is not logged.
    pub fn first(&mut self, name: &str) -> bool {
        if self.names.contains(name) || self.names.len() >= MAX_SKIPPED_NAMES {
            return false;
        }
        self.names.insert(name.to_owned())
    }

    /// How many names are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no name was skipped yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// Why a stream session ended.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("NIRI_SOCKET is not set")]
    NoSocket,
    #[error("cannot connect to niri")]
    Connect(#[source] io::Error),
    #[error("the niri connection failed")]
    Io(#[source] io::Error),
    #[error("niri closed the connection")]
    Closed,
    #[error("a line from niri is longer than the 4 MiB bound")]
    LineTooLong,
    #[error("niri refused the event stream: {0}")]
    Refused(String),
    #[error("niri answered the event stream request with {0}")]
    Unexpected(String),
    #[error("the event stream request could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("niri's reply to the event stream request does not decode")]
    Reply(#[source] serde_json::Error),
    #[error("a line from niri is not UTF-8")]
    NotText,
    #[error("a protocol error in the event stream")]
    Protocol(#[source] ParseError),
    /// An event names a workspace, window, cast or keyboard layout the stream
    /// never announced. niri-ipc's reducer would panic on it; the session is
    /// ended instead, and the next one starts from niri's full replay.
    #[error("the {0} event refers to an id the event stream never announced")]
    UnknownId(&'static str),
    /// The session panicked: a backstop for a reducer panic this module does
    /// not yet guard against. The worker survives it and reconnects.
    #[error("the event stream session panicked")]
    Panicked,
}

/// Refuses an event that niri-ipc 26.4.0's `EventStreamState::apply` would
/// `expect` on: one that names a workspace, window or cast the state does not
/// hold, or switches a keyboard layout before any was announced.
///
/// # Errors
///
/// [`SessionError::UnknownId`] naming the event.
pub fn check_applicable(state: &EventStreamState, event: &Event) -> Result<(), SessionError> {
    let workspaces = &state.workspaces.workspaces;
    let windows = &state.windows.windows;
    let unknown = match event {
        Event::WorkspaceActivated { id, .. } => {
            (!workspaces.contains_key(id)).then_some("WorkspaceActivated")
        }
        Event::WorkspaceActiveWindowChanged { workspace_id, .. } => {
            (!workspaces.contains_key(workspace_id)).then_some("WorkspaceActiveWindowChanged")
        }
        Event::WindowClosed { id } => (!windows.contains_key(id)).then_some("WindowClosed"),
        Event::WindowLayoutsChanged { changes } => changes
            .iter()
            .any(|(id, _)| !windows.contains_key(id))
            .then_some("WindowLayoutsChanged"),
        Event::KeyboardLayoutSwitched { .. } => state
            .keyboard_layouts
            .keyboard_layouts
            .is_none()
            .then_some("KeyboardLayoutSwitched"),
        Event::CastStopped { stream_id } => {
            (!state.casts.casts.contains_key(stream_id)).then_some("CastStopped")
        }
        _ => None,
    };
    unknown.map_or(Ok(()), |name| Err(SessionError::UnknownId(name)))
}

/// What one bounded read produced.
#[derive(Debug, PartialEq, Eq)]
enum LineRead {
    /// `line` holds one whole line, newline included.
    Line,
    /// The read timed out; whatever arrived is kept in `line` for the next
    /// read to complete.
    Pending,
    /// The peer closed the connection; `line` holds any partial line.
    Closed,
}

/// Reads at most up to the next newline into `line`, never letting it grow
/// past [`MAX_LINE_BYTES`] + 1 bytes. A timeout keeps the partial line.
fn read_line_bounded(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
) -> Result<LineRead, SessionError> {
    let room = (MAX_LINE_BYTES + 1).saturating_sub(line.len());
    let room = u64::try_from(room).unwrap_or(u64::MAX);
    match reader.by_ref().take(room).read_until(b'\n', line) {
        Ok(_) if line.ends_with(b"\n") => Ok(LineRead::Line),
        Ok(_) if line.len() > MAX_LINE_BYTES => Err(SessionError::LineTooLong),
        Ok(_) => Ok(LineRead::Closed),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            Ok(LineRead::Pending)
        }
        Err(error) => Err(SessionError::Io(error)),
    }
}

/// Starts the stream worker and the request worker.
///
/// The stream worker publishes [`Message::Niri`] through `sender`; the
/// returned [`NiriProvider`] queues requests for the request worker. When
/// `shutdown` is raised the stream worker stops within a quarter second and
/// the request worker once its current request (if any) has returned; the
/// stream worker also stops when the loop stops receiving, and the request
/// worker when the provider is dropped.
///
/// # Errors
///
/// The failure to start either thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(NiriProvider, Worker, Worker)> {
    let stream_shutdown = Arc::clone(shutdown);
    let path = std::env::var_os(SOCKET_PATH_ENV).map(PathBuf::from);
    let stream = Worker::spawn("niri-stream", shutdown, move || {
        let mut publish = |model: NiriModel| sender.send(Message::Niri(model)).is_ok();
        stream_worker(path.as_deref(), &mut publish, &stream_shutdown);
    })?;
    let (requests, queue) = mpsc::sync_channel(REQUEST_QUEUE);
    let request_shutdown = Arc::clone(shutdown);
    let request = Worker::spawn("niri-requests", shutdown, move || {
        request_worker(&queue, &request_shutdown);
    })?;
    Ok((NiriProvider { requests }, stream, request))
}

/// Why a session ended without an error.
#[derive(Debug)]
enum Ended {
    Shutdown,
    RuntimeGone,
}

/// Runs sessions against the socket at `path` until shutdown or until the
/// loop is gone, withdrawing the model whenever a session that published one
/// ends, and reconnecting after [`RECONNECT_DELAY`].
///
/// A panic inside a session (a reducer `expect` not caught by
/// [`check_applicable`]) is caught here and handled as a failed session, so
/// the thread never dies with a stale model on screen.
fn stream_worker(
    path: Option<&Path>,
    publish: &mut dyn FnMut(NiriModel) -> bool,
    shutdown: &AtomicBool,
) {
    let mut log = ConnectionLog::new();
    let mut skipped = SkipLog::default();
    while !shutdown.load(Ordering::Acquire) {
        let mut published = false;
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            session(
                path,
                &mut *publish,
                shutdown,
                &mut log,
                &mut skipped,
                &mut published,
            )
        }))
        .unwrap_or(Err(SessionError::Panicked));
        match outcome {
            Ok(Ended::Shutdown | Ended::RuntimeGone) => return,
            Err(error) => {
                if log.failed() {
                    tracing::warn!(
                        "niri: {}; reconnecting every {} s",
                        report(&error),
                        RECONNECT_DELAY.as_secs()
                    );
                }
            }
        }
        // The last model is no longer the compositor's state.
        if published && !publish(NiriModel::default()) {
            return;
        }
        if !sleep_unless_shutdown(RECONNECT_DELAY, shutdown) {
            return;
        }
    }
}

/// An error with its sources, on one line.
fn report(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Sleeps `total` in [`POLL`] slices; `false` when shutdown was raised.
fn sleep_unless_shutdown(total: Duration, shutdown: &AtomicBool) -> bool {
    let deadline = Instant::now() + total;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        thread::sleep(left.min(POLL));
    }
}

/// Milliseconds since `start`, for the coalescer.
fn millis_since(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// One connection to the socket at `path`, from the handshake until it ends.
///
/// Every changed model goes to `publish`, which answers `false` once the loop
/// is gone; `published` records whether anything went out, so the caller can
/// withdraw it when the connection is lost.
fn session(
    path: Option<&Path>,
    publish: &mut dyn FnMut(NiriModel) -> bool,
    shutdown: &AtomicBool,
    log: &mut ConnectionLog,
    skipped: &mut SkipLog,
    published: &mut bool,
) -> Result<Ended, SessionError> {
    let path = path.ok_or(SessionError::NoSocket)?;
    let stream = UnixStream::connect(path).map_err(SessionError::Connect)?;
    stream
        .set_write_timeout(Some(RECONNECT_DELAY))
        .map_err(SessionError::Io)?;
    let mut reader = BufReader::new(stream);

    let mut request = serde_json::to_vec(&Request::EventStream).map_err(SessionError::Encode)?;
    request.push(b'\n');
    reader
        .get_mut()
        .write_all(&request)
        .map_err(SessionError::Io)?;

    let mut line = Vec::new();
    if !next_line(&mut reader, &mut line, shutdown, None)? {
        return Ok(Ended::Shutdown);
    }
    let reply: Reply = serde_json::from_slice(&line).map_err(SessionError::Reply)?;
    match reply {
        Ok(Response::Handled) => {}
        Ok(other) => return Err(SessionError::Unexpected(format!("{other:?}"))),
        Err(message) => return Err(SessionError::Refused(bounded(&message, MAX_TEXT_UNITS))),
    }
    if log.connected() {
        tracing::info!("niri: connected again");
    }

    let start = Instant::now();
    let mut coalescer = Coalescer::default();
    let mut state = EventStreamState::default();
    let mut dirty = false;
    let mut last: Option<NiriModel> = None;
    loop {
        if dirty && coalescer.due(millis_since(start)) {
            let model = reduce(&state);
            if last.as_ref() != Some(&model) {
                if !publish(model.clone()) {
                    return Ok(Ended::RuntimeGone);
                }
                *published = true;
                last = Some(model);
            }
            coalescer.emitted(millis_since(start));
            dirty = false;
        }

        // While a model is owed, wake when it falls due; otherwise only to
        // look at the shutdown flag.
        let wait = if dirty {
            coalescer
                .wait_ms(millis_since(start))
                .map_or(POLL, |ms| Duration::from_millis(ms.max(1)).min(POLL))
        } else {
            POLL
        };
        line.clear();
        if !next_line(&mut reader, &mut line, shutdown, Some(wait))? {
            return Ok(Ended::Shutdown);
        }
        if line.is_empty() {
            // The wait for a due model elapsed with nothing new.
            continue;
        }

        let text = std::str::from_utf8(&line).map_err(|_| SessionError::NotText)?;
        match parse_event(text) {
            Ok(Some(event)) => {
                check_applicable(&state, &event)?;
                if state.apply(event).is_none() {
                    dirty = true;
                    coalescer.mark();
                }
            }
            Ok(None) => {
                if let Some(name) = variant_name(text) {
                    if skipped.first(&name) {
                        tracing::info!(
                            "niri: skipping the {name} event, unknown to niri-ipc 26.4.0"
                        );
                    }
                }
            }
            Err(error @ ParseError::Truncated(_)) => return Err(SessionError::Protocol(error)),
            Err(error) => {
                let name = error.variant().unwrap_or("unnamed").to_owned();
                if skipped.first(&name) {
                    tracing::warn!(
                        "niri: skipping a line: {}",
                        bounded(&report(&error), MAX_TEXT_UNITS)
                    );
                }
            }
        }
    }
}

/// Appends one whole line to `line`, reading in [`POLL`] slices so the
/// shutdown flag is seen at least that often.
///
/// Returns `Ok(false)` when shutdown was raised and `Ok(true)` when `line`
/// holds a whole line. With `wait`, it also returns `Ok(true)` with `line`
/// still empty once `wait` has passed and nothing has arrived; once part of a
/// line has arrived it waits for the rest, so a line is never split.
fn next_line(
    reader: &mut BufReader<UnixStream>,
    line: &mut Vec<u8>,
    shutdown: &AtomicBool,
    wait: Option<Duration>,
) -> Result<bool, SessionError> {
    let deadline = wait.map(|wait| Instant::now() + wait);
    loop {
        if shutdown.load(Ordering::Acquire) {
            return Ok(false);
        }
        // Once part of a line has arrived the deadline no longer applies (a
        // line is never split), so the read waits a whole slice for the rest.
        let timeout = match deadline {
            Some(deadline) if line.is_empty() => deadline
                .saturating_duration_since(Instant::now())
                .clamp(Duration::from_millis(1), POLL),
            _ => POLL,
        };
        reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .map_err(SessionError::Io)?;
        match read_line_bounded(reader, line)? {
            LineRead::Line => return Ok(true),
            LineRead::Closed if line.is_empty() => return Err(SessionError::Closed),
            LineRead::Closed => {
                // A line cut off by the end of the stream: report it as the
                // protocol error it is when it does not parse.
                let text = String::from_utf8_lossy(line);
                return match parse_event(&text) {
                    Err(error @ ParseError::Truncated(_)) => Err(SessionError::Protocol(error)),
                    _ => Err(SessionError::Closed),
                };
            }
            LineRead::Pending => {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) && line.is_empty() {
                    return Ok(true);
                }
            }
        }
    }
}

fn request_worker(queue: &Receiver<NiriRequest>, shutdown: &AtomicBool) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        match queue.recv_timeout(POLL) {
            Ok(request) => perform(request),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Sends one request over a socket of its own and logs a failure.
fn perform(request: NiriRequest) {
    let reply =
        Socket::connect().and_then(|mut socket| socket.send(Request::Action(request.action())));
    match reply {
        Ok(Ok(Response::Handled)) => {}
        Ok(Ok(other)) => tracing::warn!("niri: {request:?} answered with {other:?}"),
        Ok(Err(message)) => tracing::warn!(
            "niri: {request:?} refused: {}",
            bounded(&message, MAX_TEXT_UNITS)
        ),
        Err(error) => tracing::warn!("niri: {request:?} not sent: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::mpsc::sync_channel;

    fn ws(
        id: u64,
        idx: u8,
        name: Option<&str>,
        out: &str,
        active: bool,
        focused: bool,
    ) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some(out.to_owned()),
            is_urgent: false,
            is_active: active,
            is_focused: focused,
            active_window_id: None,
        }
    }

    fn window(
        id: u64,
        title: &str,
        workspace_id: Option<u64>,
        focused: bool,
        column: Option<usize>,
    ) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: Some(title.to_owned()),
            app_id: Some(format!("app.{id}")),
            pid: None,
            workspace_id,
            is_focused: focused,
            is_floating: column.is_none(),
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: column.map(|column| (column, 1)),
                tile_size: (800.0, 600.0),
                window_size: (800, 600),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    fn state() -> EventStreamState {
        let mut s = EventStreamState::default();
        s.apply(Event::WorkspacesChanged {
            workspaces: vec![
                ws(30, 2, None, "HDMI-A-1", false, false),
                ws(12, 2, Some("web"), "DP-1", false, false),
                ws(11, 1, None, "DP-1", true, true),
                ws(31, 1, None, "HDMI-A-1", true, false),
            ],
        });
        s
    }

    #[test]
    fn reduce_sorts_by_output_then_index_and_keeps_flags_and_names() {
        let m = reduce(&state());
        let order: Vec<u64> = m.workspaces.iter().map(|w| w.id).collect();
        assert_eq!(order, vec![11, 12, 31, 30]);
        assert_eq!(m.workspaces[0].output.as_deref(), Some("DP-1"));
        assert!(m.workspaces[0].is_active && m.workspaces[0].is_focused);
        assert_eq!(m.workspaces[1].name.as_deref(), Some("web"));
        assert!(m.workspaces[2].is_active && !m.workspaces[2].is_focused);
        assert!(m.windows.is_empty());
        assert_eq!(m.outputs, ["DP-1", "HDMI-A-1"]);
    }

    #[test]
    fn unknown_events_are_skipped_not_fatal() {
        assert!(parse_event(r#"{"MinimizedWindowsChanged":{"ids":[]}}"#)
            .unwrap()
            .is_none());
        assert!(
            parse_event(r#"{"WorkspaceActivated":{"id":1,"focused":true}}"#)
                .unwrap()
                .is_some()
        );
        assert!(parse_event(r#"{"WorkspaceActivated":{"id":"x"}}"#).is_err());
        assert!(parse_event("not json").is_err());
    }

    #[test]
    fn workspace_activated_moves_the_active_flag_on_its_output_only() {
        let mut s = state();
        s.apply(Event::WorkspaceActivated {
            id: 12,
            focused: true,
        });
        let m = reduce(&s);
        let flags: Vec<(u64, bool, bool)> = m
            .workspaces
            .iter()
            .map(|w| (w.id, w.is_active, w.is_focused))
            .collect();
        assert_eq!(
            flags,
            vec![
                (11, false, false),
                (12, true, true),
                (31, true, false),
                (30, false, false)
            ]
        );
    }

    #[test]
    fn reduce_lists_windows_with_their_workspace_and_focus() {
        let mut s = state();
        s.apply(Event::WindowsChanged {
            windows: vec![
                window(7, "editor", Some(11), true, Some(2)),
                window(5, "terminal", Some(11), false, Some(1)),
                window(9, "player", Some(31), false, None),
                window(8, "browser", Some(11), false, Some(2)),
            ],
        });

        let m = reduce(&s);

        let windows: Vec<(u64, Option<u64>, bool)> = m
            .windows
            .iter()
            .map(|w| (w.id, w.workspace_id, w.is_focused))
            .collect();
        assert_eq!(
            windows,
            vec![
                (5, Some(11), false),
                (7, Some(11), true),
                (8, Some(11), false),
                (9, Some(31), false)
            ]
        );
        assert_eq!(m.windows[1].title.as_deref(), Some("editor"));
        assert_eq!(m.windows[1].app_id.as_deref(), Some("app.7"));
        // Two columns hold the three tiled windows of workspace 11; the
        // floating window on 31 holds none.
        let columns: Vec<(u64, u16)> = m.workspaces.iter().map(|w| (w.id, w.columns)).collect();
        assert_eq!(columns, vec![(11, 2), (12, 0), (31, 0), (30, 0)]);
    }

    #[test]
    fn reduce_bounds_text_from_the_compositor() {
        let mut s = state();
        s.apply(Event::WindowsChanged {
            windows: vec![window(1, &"t".repeat(10_000), Some(11), false, Some(1))],
        });

        let m = reduce(&s);

        assert_eq!(
            m.windows[0].title.as_deref().map(str::len),
            Some(celestina_shell::bounded::MAX_TEXT_UNITS)
        );
    }

    #[test]
    fn a_truncated_line_is_a_protocol_error_not_a_skip() {
        let half = r#"{"WorkspaceActivated":{"id":1,"#;

        assert!(matches!(parse_event(half), Err(ParseError::Truncated(_))));
    }

    #[test]
    fn unknown_variant_name_is_taken_from_the_outer_key() {
        assert_eq!(
            variant_name(r#"{"MinimizedWindowsChanged":{"ids":[1,2]}}"#).as_deref(),
            Some("MinimizedWindowsChanged")
        );
        assert_eq!(
            variant_name(" { \"WorkspaceActivated\" :{}}").as_deref(),
            Some("WorkspaceActivated")
        );
        // A unit variant is a bare string.
        assert_eq!(variant_name(r#""Ping""#).as_deref(), Some("Ping"));
        // Nothing that is not a plain identifier is ever a name to log.
        assert_eq!(variant_name("not json"), None);
        assert_eq!(variant_name(r#"{"bad\"name":1}"#), None);
        assert_eq!(
            variant_name(&format!(r#"{{"{}":1}}"#, "N".repeat(500))),
            None
        );
        // A known variant whose body does not decode keeps its name for the
        // log line that reports it.
        match parse_event(r#"{"WorkspaceActivated":{"id":"x"}}"#) {
            Err(error) => assert_eq!(error.variant(), Some("WorkspaceActivated")),
            Ok(event) => panic!("decoded {event:?}"),
        }
    }

    #[test]
    fn request_queue_refuses_the_33rd_pending_request() {
        let (requests, _worker_side) = sync_channel(REQUEST_QUEUE);
        let provider = NiriProvider { requests };

        for id in 0..32 {
            assert_eq!(provider.request(NiriRequest::FocusWorkspace(id)), Ok(()));
        }
        assert_eq!(
            provider.request(NiriRequest::FocusWindow(33)),
            Err(Message::NiriRequestRefused(NiriRequest::FocusWindow(33)))
        );
    }

    #[test]
    fn a_stopped_request_worker_refuses_too() {
        let (requests, worker_side) = sync_channel(REQUEST_QUEUE);
        drop(worker_side);
        let provider = NiriProvider { requests };

        assert_eq!(
            provider.request(NiriRequest::FocusWorkspace(1)),
            Err(Message::NiriRequestRefused(NiriRequest::FocusWorkspace(1)))
        );
    }

    #[test]
    fn requests_are_the_typed_niri_actions() {
        assert!(matches!(
            NiriRequest::FocusWorkspace(4).action(),
            Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Id(4)
            }
        ));
        assert!(matches!(
            NiriRequest::FocusWindow(9).action(),
            Action::FocusWindow { id: 9 }
        ));
    }

    #[test]
    fn reconnect_backoff_logs_once_per_state_change() {
        let mut log = ConnectionLog::new();

        // The first failure is news; a minute of retries once a second is not.
        assert!(log.failed());
        for _ in 0..60 {
            assert!(!log.failed());
        }
        // Coming back is news once, and a later failure is news again.
        assert!(log.connected());
        assert!(!log.connected());
        assert!(log.failed());
        assert!(!log.failed());
    }

    #[test]
    fn a_line_is_read_whole_or_reported_as_the_end() {
        let mut reader = Cursor::new(b"{\"A\":1}\n{\"B\":".to_vec());
        let mut line = Vec::new();

        assert!(matches!(
            read_line_bounded(&mut reader, &mut line),
            Ok(LineRead::Line)
        ));
        assert_eq!(line, b"{\"A\":1}\n");

        line.clear();
        // The second line ends with the stream: the connection closed mid-line.
        assert!(matches!(
            read_line_bounded(&mut reader, &mut line),
            Ok(LineRead::Closed)
        ));
    }

    #[test]
    fn a_line_past_the_bound_is_refused_before_it_is_kept() {
        let mut reader = Cursor::new(vec![b'x'; MAX_LINE_BYTES + 10]);
        let mut line = Vec::new();

        assert!(matches!(
            read_line_bounded(&mut reader, &mut line),
            Err(SessionError::LineTooLong)
        ));
        assert!(line.len() <= MAX_LINE_BYTES + 1);
    }

    #[test]
    fn skipped_variants_are_logged_once_per_name_and_the_memory_is_bounded() {
        let mut skipped = SkipLog::default();

        assert!(skipped.first("MinimizedWindowsChanged"));
        assert!(!skipped.first("MinimizedWindowsChanged"));
        for index in 0..MAX_SKIPPED_NAMES * 2 {
            skipped.first(&format!("Name{index}"));
        }
        assert!(skipped.len() <= MAX_SKIPPED_NAMES);
    }

    /// A stand-in for niri on a socket in a fresh directory: it checks the
    /// request, answers it, writes `lines` (each a whole line unless it lacks
    /// its newline), waits `linger`, and closes.
    fn fake_niri(
        name: &str,
        lines: Vec<String>,
        linger: Duration,
    ) -> (PathBuf, thread::JoinHandle<()>) {
        fake_niri_sessions(name, vec![lines], linger)
    }

    /// [`fake_niri`] for several connections in turn, one list of lines each.
    fn fake_niri_sessions(
        name: &str,
        sessions: Vec<Vec<String>>,
        linger: Duration,
    ) -> (PathBuf, thread::JoinHandle<()>) {
        use std::os::unix::net::UnixListener;

        let directory =
            std::env::temp_dir().join(format!("celestina-niri-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a test directory");
        let path = directory.join("niri.sock");
        let listener = UnixListener::bind(&path).expect("a test socket");
        let server = thread::spawn(move || {
            for lines in sessions {
                let (stream, _) = listener.accept().expect("the shell connects");
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                reader.read_line(&mut request).expect("a request line");
                assert_eq!(request, "\"EventStream\"\n");
                let stream = reader.get_mut();
                stream
                    .write_all(b"{\"Ok\":\"Handled\"}\n")
                    .expect("the reply is written");
                for line in lines {
                    stream
                        .write_all(line.as_bytes())
                        .expect("a line is written");
                }
                thread::sleep(linger);
            }
            drop(listener);
            let _ = std::fs::remove_dir_all(&directory);
        });
        (path, server)
    }

    fn run_session(path: &Path) -> (Result<Ended, SessionError>, Vec<NiriModel>, SkipLog) {
        let shutdown = AtomicBool::new(false);
        let mut models = Vec::new();
        let mut publish = |model: NiriModel| {
            models.push(model);
            true
        };
        let mut log = ConnectionLog::new();
        let mut skipped = SkipLog::default();
        let mut published = false;
        let ended = session(
            Some(path),
            &mut publish,
            &shutdown,
            &mut log,
            &mut skipped,
            &mut published,
        );
        assert_eq!(published, !models.is_empty());
        (ended, models, skipped)
    }

    fn workspaces_line() -> String {
        let event = Event::WorkspacesChanged {
            workspaces: vec![
                ws(1, 1, None, "DP-1", true, true),
                ws(2, 2, None, "DP-1", false, false),
            ],
        };
        let mut line = serde_json::to_string(&event).expect("an event encodes");
        line.push('\n');
        line
    }

    #[test]
    fn a_session_skips_unknown_events_and_flushes_the_last_burst() {
        let (path, server) = fake_niri(
            "burst",
            vec![
                workspaces_line(),
                "{\"MinimizedWindowsChanged\":{\"ids\":[]}}\n".to_owned(),
                "{\"WorkspaceActivated\":{\"id\":2,\"focused\":true}}\n".to_owned(),
            ],
            Duration::from_millis(400),
        );

        let (ended, models, mut skipped) = run_session(&path);
        server.join().expect("the fake niri ends");

        assert!(matches!(ended, Err(SessionError::Closed)), "{ended:?}");
        // The first model goes out at once; the activation, inside the same
        // coalescing window, goes out when the window closes.
        assert_eq!(models.len(), 2, "{models:?}");
        assert!(models[0].workspaces[0].is_focused);
        assert!(models[1].workspaces[1].is_focused);
        assert!(!skipped.first("MinimizedWindowsChanged"));
    }

    #[test]
    fn a_session_cut_mid_line_ends_with_a_protocol_error() {
        let (path, server) = fake_niri(
            "cut",
            vec!["{\"WorkspaceActivated\":{\"id\":1,".to_owned()],
            Duration::ZERO,
        );

        let (ended, models, _) = run_session(&path);
        server.join().expect("the fake niri ends");

        assert!(
            matches!(ended, Err(SessionError::Protocol(ParseError::Truncated(_)))),
            "{ended:?}"
        );
        assert!(models.is_empty());
    }

    #[test]
    fn no_socket_is_a_session_error_not_a_panic() {
        let shutdown = AtomicBool::new(false);
        let mut publish = |_: NiriModel| true;
        let ended = session(
            None,
            &mut publish,
            &shutdown,
            &mut ConnectionLog::new(),
            &mut SkipLog::default(),
            &mut false,
        );
        assert!(matches!(ended, Err(SessionError::NoSocket)));
    }

    #[test]
    fn events_for_ids_the_stream_never_announced_are_refused() {
        let s = state();
        let refused = |event: Event| match check_applicable(&s, &event) {
            Err(SessionError::UnknownId(name)) => name,
            other => panic!("{event:?} was not refused: {other:?}"),
        };

        assert_eq!(
            refused(Event::WorkspaceActivated {
                id: 99,
                focused: true
            }),
            "WorkspaceActivated"
        );
        assert_eq!(
            refused(Event::WorkspaceActiveWindowChanged {
                workspace_id: 99,
                active_window_id: None
            }),
            "WorkspaceActiveWindowChanged"
        );
        assert_eq!(refused(Event::WindowClosed { id: 7 }), "WindowClosed");
        assert_eq!(
            refused(Event::WindowLayoutsChanged {
                changes: vec![(7, window(7, "w", Some(11), false, Some(1)).layout)]
            }),
            "WindowLayoutsChanged"
        );
        assert_eq!(
            refused(Event::KeyboardLayoutSwitched { idx: 1 }),
            "KeyboardLayoutSwitched"
        );
        assert_eq!(refused(Event::CastStopped { stream_id: 3 }), "CastStopped");

        // The same events for ids the state holds are applied as usual.
        let mut s = state();
        s.apply(Event::WindowsChanged {
            windows: vec![window(7, "w", Some(11), false, Some(1))],
        });
        for event in [
            Event::WorkspaceActivated {
                id: 12,
                focused: false,
            },
            Event::WindowLayoutsChanged {
                changes: vec![(7, window(7, "w", Some(11), false, Some(2)).layout)],
            },
            Event::WindowClosed { id: 7 },
        ] {
            assert!(check_applicable(&s, &event).is_ok(), "{event:?}");
            s.apply(event);
        }
    }

    #[test]
    fn an_event_for_an_id_the_stream_never_saw_ends_the_session_not_the_thread() {
        let unknown_close = "{\"WindowClosed\":{\"id\":999999}}\n".to_owned();
        let (path, server) = fake_niri_sessions(
            "unknown-id",
            vec![
                vec![workspaces_line(), unknown_close],
                vec![workspaces_line()],
            ],
            Duration::from_millis(300),
        );

        let shutdown = AtomicBool::new(false);
        let mut models = Vec::new();
        let mut publish = |model: NiriModel| {
            models.push(model);
            // The model, its withdrawal, and the model again after the
            // reconnection: then the test has seen enough.
            if models.len() == 3 {
                shutdown.store(true, Ordering::Release);
            }
            true
        };
        stream_worker(Some(&path), &mut publish, &shutdown);
        server.join().expect("the fake niri ends");

        assert_eq!(models.len(), 3, "{models:?}");
        assert_eq!(models[0].workspaces.len(), 2);
        assert_eq!(models[1], NiriModel::default());
        assert_eq!(models[2], models[0]);
    }

    #[test]
    fn a_panicking_session_is_a_failed_session_not_a_dead_thread() {
        let (path, server) = fake_niri_sessions(
            "panic",
            vec![vec![workspaces_line()], vec![workspaces_line()]],
            Duration::from_millis(300),
        );

        let shutdown = AtomicBool::new(false);
        let mut calls = 0;
        let mut models = Vec::new();
        // The first publication panics inside the session, standing in for a
        // reducer `expect` that `check_applicable` does not know about.
        let mut publish = |model: NiriModel| {
            calls += 1;
            assert!(calls > 1, "a panic inside the session, on purpose");
            models.push(model);
            shutdown.store(true, Ordering::Release);
            true
        };
        stream_worker(Some(&path), &mut publish, &shutdown);
        server.join().expect("the fake niri ends");

        // The worker survived the panic, reconnected and published again.
        assert_eq!(calls, 2);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].workspaces.len(), 2);
    }
}
