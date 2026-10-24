//! The helper conversation of the polkit agent (CEL-2-I; design
//! `docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`,
//! sections 3 facts 9 to 11, 4.4 and 6.3 N-16 and N-17).
//!
//! polkit's own `polkit-agent-helper-1` runs PAM as root and tells polkitd the
//! verdict itself. This module is the agent's half of the conversation over
//! the helper's socket: it connects, writes `<user>\n<cookie>\n`, reads the
//! helper's lines and writes the person's answer. It never verifies a password
//! and never reports a success; `SUCCESS` is only *shown*.
//!
//! Everything about what a line means is the pure domain's
//! (`celestina_shell::polkit`: [`decode_line`], [`Exchange`], [`Ending`]); this
//! module is the IO adapter that feeds it, so no decoding and no attempt
//! counting is repeated here. What this module owns is what the pure domain
//! cannot see:
//!
//! - **Trust before the first byte** (N-16). The socket path is one constant.
//!   Before connecting, `/run/polkit` must be a real directory owned by root
//!   that neither group nor others can write, and the socket path a real
//!   socket owned by root (no mode is required: the socket is `0666` by
//!   design). After connecting, `SO_PEERCRED` must say the listener is root.
//!   That proves "a root listener", not "the helper". Nothing a setting or the
//!   environment says can change the path; the only other way to reach
//!   [`connect_at`] is a test.
//! - **Bounded reads** (N-17). A line is read in chunks and never buffered
//!   past [`MAX_HELPER_LINE`] bytes without its newline; every read carries a
//!   short timeout, so the shutdown flag and the helper's silence are looked at
//!   at a bounded interval.
//! - **The secret's one-way trip** (design S-7). The card moves a [`Secret`]
//!   through a one-slot channel; the helper thread writes it with
//!   [`Secret::write_line_to`] and drops it, on every path.
//!
//! The thread that owns a conversation ([`Helper`]) reconnects by itself after
//! a wrong answer below the cap (row F-16, same cookie), and ends on every
//! other [`Ending`]. It never starts before the loop starts it, and the loop
//! does that only once the card has the keyboard (N-24).

use std::fs::Metadata;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::polkit::{
    decode_line, Cookie, Ending, Exchange, PamText, Request, Step, MAX_HELPER_LINE,
    MAX_USER_NAME_BYTES,
};
use celestina_ui::Secret;

use crate::providers::{Reaper, Worker};

/// The helper's socket: the only place the agent connects to (design N-16).
pub const HELPER_SOCKET: &str = "/run/polkit/agent-helper.socket";
/// How often a blocked read or wait looks at the shutdown flag.
pub const POLL: Duration = Duration::from_millis(100);
/// The longest a write to the helper may take.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long [`Helper::cancel`] waits for the thread to close its socket.
pub const CANCEL_WAIT: Duration = Duration::from_secs(2);
/// The most bytes asked of the socket at once.
const READ_CHUNK: usize = 1024;

// ---- what the helper conversation needs of a request ------------------------------------

/// The account and the cookie a conversation is opened with.
///
/// The cookie is a copy of the request record's, wiped when this is dropped:
/// it is made on demand for the helper's thread, so the request table's lock
/// is never held across the socket's IO. No `Clone` and no `Debug`: it owns a
/// [`Cookie`].
pub struct HelperInput {
    user: String,
    cookie: Cookie,
}

impl HelperInput {
    /// A conversation for `user`, with `cookie`.
    #[must_use]
    pub fn new(user: String, cookie: Cookie) -> HelperInput {
        HelperInput { user, cookie }
    }

    /// The input of `request`: its validated account name and a fresh copy of
    /// its cookie. `None` is not expected (a cookie is printable ASCII).
    #[must_use]
    pub fn from_request(request: &Request) -> Option<HelperInput> {
        let text = std::str::from_utf8(request.cookie().as_bytes()).ok()?;
        let cookie = Cookie::new(text).ok()?;
        Some(HelperInput::new(request.helper_user().to_owned(), cookie))
    }

    /// The account the helper is told to authenticate.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The cookie, for the helper's second line.
    #[must_use]
    pub fn cookie(&self) -> &Cookie {
        &self.cookie
    }
}

// ---- trust ---------------------------------------------------------------------------------

/// Who must own what for the helper's socket to be trusted. Production trusts
/// root and only root ([`Trust::ROOT`]); a test stands its own uid in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Trust {
    /// Who must own the directory and the socket path.
    pub(crate) files: u32,
    /// Who the listener behind the socket must be.
    pub(crate) listener: u32,
}

impl Trust {
    /// Root owns the files and listens.
    pub(crate) const ROOT: Trust = Trust {
        files: 0,
        listener: 0,
    };
}

/// How long a conversation waits. Production waits [`POLL`] between looks at
/// the shutdown flag and [`celestina_shell::polkit::HELPER_SILENCE`] for a
/// helper that owes a line; a test shortens both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Timing {
    pub(crate) poll: Duration,
    pub(crate) silence: Duration,
}

impl Timing {
    pub(crate) const PRODUCTION: Timing = Timing {
        poll: POLL,
        silence: celestina_shell::polkit::HELPER_SILENCE,
    };
}

/// What is known about a path, as std reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Facts {
    pub(crate) is_dir: bool,
    pub(crate) is_socket: bool,
    pub(crate) uid: u32,
    pub(crate) mode: u32,
}

impl Facts {
    fn of(metadata: &Metadata) -> Facts {
        Facts {
            is_dir: metadata.file_type().is_dir(),
            is_socket: metadata.file_type().is_socket(),
            uid: metadata.uid(),
            mode: metadata.permissions().mode(),
        }
    }
}

/// Why the helper's socket was not used. Every one ends the request as
/// [`Ending::HelperUnreachable`] (row F-11).
#[derive(Debug, thiserror::Error)]
pub enum ConnectRefusal {
    /// The account name cannot go on the helper's first line.
    #[error("the account name cannot go on the helper's first line")]
    BadUser,
    /// A path could not be inspected (it does not exist, or may not be seen).
    #[error("the helper's socket cannot be inspected")]
    Inspect(#[source] io::Error),
    /// The helper's directory is not a directory (a symlink is not followed).
    #[error("the helper's directory is not a directory")]
    DirectoryNotADirectory,
    /// The helper's directory is not owned by the trusted uid.
    #[error("the helper's directory is not owned by root")]
    DirectoryNotOwned,
    /// Group or others can write to the helper's directory.
    #[error("the helper's directory is writable by group or others")]
    DirectoryWritable,
    /// The socket path is not a socket (a symlink is not followed).
    #[error("the helper's socket path is not a socket")]
    SocketNotASocket,
    /// The socket path is not owned by the trusted uid.
    #[error("the helper's socket is not owned by root")]
    SocketNotOwned,
    /// The connection failed.
    #[error("cannot connect to the helper's socket")]
    Connect(#[source] io::Error),
    /// The kernel would not say who listens.
    #[error("cannot tell who listens on the helper's socket")]
    ListenerUnknown(#[source] io::Error),
    /// The listener is not the trusted uid.
    #[error("the helper's socket is not served by root")]
    ListenerNotTrusted,
    /// The first two lines could not be written.
    #[error("cannot write to the helper's socket")]
    Write(#[source] io::Error),
}

/// The directory's judgement: a real directory, owned by the trusted uid,
/// that neither group nor others can write.
pub(crate) fn check_directory(facts: &Facts, trusted: u32) -> Result<(), ConnectRefusal> {
    if !facts.is_dir {
        return Err(ConnectRefusal::DirectoryNotADirectory);
    }
    if facts.uid != trusted {
        return Err(ConnectRefusal::DirectoryNotOwned);
    }
    if facts.mode & 0o022 != 0 {
        return Err(ConnectRefusal::DirectoryWritable);
    }
    Ok(())
}

/// The socket path's judgement: a real socket owned by the trusted uid. Its
/// mode is deliberately not looked at: polkit makes it `0666`.
pub(crate) fn check_socket(facts: &Facts, trusted: u32) -> Result<(), ConnectRefusal> {
    if !facts.is_socket {
        return Err(ConnectRefusal::SocketNotASocket);
    }
    if facts.uid != trusted {
        return Err(ConnectRefusal::SocketNotOwned);
    }
    Ok(())
}

/// The listener's judgement, from `SO_PEERCRED`.
pub(crate) fn check_listener(uid: u32, trusted: u32) -> Result<(), ConnectRefusal> {
    if uid == trusted {
        Ok(())
    } else {
        Err(ConnectRefusal::ListenerNotTrusted)
    }
}

/// Whether `user` may go on the helper's first line: not empty, not longer
/// than the pure domain allows, and without any control character (a newline
/// would split the line, a NUL would end it).
fn acceptable_user(user: &str) -> bool {
    !user.is_empty() && user.len() <= MAX_USER_NAME_BYTES && !user.chars().any(char::is_control)
}

// ---- events ---------------------------------------------------------------------------------

/// What a conversation tells the loop. Built from the pure
/// [`celestina_shell::polkit::Step`]s; there is no `Success` of its own: the
/// helper's `SUCCESS` is [`Ending::Authenticated`], the one ending that
/// replies normally, so the invariant has one owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelperEvent {
    /// PAM asks for an answer that is not echoed: ask the person. Its text is
    /// decoded and validated; paint it only through [`PamText::display`].
    Prompt(PamText),
    /// PAM's words (an error or an information line): show them.
    Notice(PamText),
    /// The answer was wrong and attempts remain (F-16): this conversation is
    /// closed, wipe the field, and a new one with the same cookie follows.
    Failure,
    /// The request is over with this ending.
    Ended(Ending),
}

// ---- the answer's type ----------------------------------------------------------------------

/// What is written to the helper as the person's answer: a [`Secret`] in
/// production. A trait so the tests can watch the answer being consumed and
/// dropped on every path without reading freed memory.
pub trait Answer: Send + 'static {
    /// Writes the answer and the newline that ends its line.
    ///
    /// # Errors
    ///
    /// The write's failure.
    fn write_line_to(&self, out: &mut dyn Write) -> io::Result<()>;
}

impl Answer for Secret {
    fn write_line_to(&self, mut out: &mut dyn Write) -> io::Result<()> {
        Secret::write_line_to(self, &mut out)
    }
}

// ---- one conversation ------------------------------------------------------------------------

/// What reading the socket produced.
enum Next {
    Line(Vec<u8>),
    TooLong,
    Eof,
    Silent,
    Halted,
}

/// One connection to the helper: owns the socket and the bytes read from it
/// that do not yet make a line. Dropping it closes the socket.
pub struct Conversation {
    stream: UnixStream,
    pending: Vec<u8>,
    timing: Timing,
    /// When the helper last spoke a whole line (or the conversation began, or
    /// the answer was sent).
    quiet_since: Instant,
    /// A prompt was shown and not yet answered: the only time an answer may
    /// be written.
    prompt_open: bool,
    finished: bool,
}

/// Opens a conversation with the helper at the constant production path.
///
/// # Errors
///
/// [`ConnectRefusal`] says why the socket was not trusted or reached.
pub fn connect(input: &HelperInput) -> Result<Conversation, ConnectRefusal> {
    connect_at(
        Path::new(HELPER_SOCKET),
        input.user(),
        input.cookie(),
        Trust::ROOT,
        Timing::PRODUCTION,
    )
}

/// [`connect`] at `path`, judged by `trust`. Private to the module and its
/// tests: nothing outside can choose the path or the trust (design N-16).
fn connect_at(
    path: &Path,
    user: &str,
    cookie: &Cookie,
    trust: Trust,
    timing: Timing,
) -> Result<Conversation, ConnectRefusal> {
    if !acceptable_user(user) {
        return Err(ConnectRefusal::BadUser);
    }
    if let Some(directory) = path.parent() {
        let metadata = std::fs::symlink_metadata(directory).map_err(ConnectRefusal::Inspect)?;
        check_directory(&Facts::of(&metadata), trust.files)?;
    }
    let metadata = std::fs::symlink_metadata(path).map_err(ConnectRefusal::Inspect)?;
    check_socket(&Facts::of(&metadata), trust.files)?;
    let mut stream = UnixStream::connect(path).map_err(ConnectRefusal::Connect)?;
    let credentials = rustix::net::sockopt::socket_peercred(&stream)
        .map_err(|errno| ConnectRefusal::ListenerUnknown(io::Error::from(errno)))?;
    check_listener(credentials.uid.as_raw(), trust.listener)?;
    stream
        .set_read_timeout(Some(timing.poll))
        .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
        .map_err(ConnectRefusal::Write)?;
    // Four writes and no joined copy: the cookie is a bearer secret.
    stream
        .write_all(user.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.write_all(cookie.as_bytes()))
        .and_then(|()| stream.write_all(b"\n"))
        .map_err(ConnectRefusal::Write)?;
    Ok(Conversation {
        stream,
        pending: Vec::new(),
        timing,
        quiet_since: Instant::now(),
        prompt_open: false,
        finished: false,
    })
}

impl Conversation {
    /// The next thing the helper's conversation does: `None` once `halted`
    /// says so or the conversation is over.
    ///
    /// Blocks, looking at `halted` every poll. The pure `exchange` decides
    /// what each line, an unreadable one, end of input or silence means.
    pub fn next_event(
        &mut self,
        exchange: &mut Exchange,
        halted: &dyn Fn() -> bool,
    ) -> Option<HelperEvent> {
        while !self.finished {
            let step = match self.next(halted) {
                Next::Halted => return None,
                Next::Line(line) => {
                    self.quiet_since = Instant::now();
                    match decode_line(&line) {
                        Ok(decoded) => exchange.on_line(decoded),
                        Err(refusal) => {
                            tracing::debug!(%refusal, "polkit: a helper line was refused");
                            exchange.on_unreadable_line()
                        }
                    }
                }
                Next::TooLong => exchange.on_unreadable_line(),
                Next::Eof => exchange.on_eof(),
                Next::Silent => {
                    let step = exchange.on_silence();
                    if step == Step::Ignore {
                        // The helper owes nothing (the person is typing): the
                        // prompt's own deadline bounds that wait.
                        self.quiet_since = Instant::now();
                    }
                    step
                }
            };
            match step {
                Step::Ask(text) => {
                    self.prompt_open = true;
                    return Some(HelperEvent::Prompt(text));
                }
                Step::Notice(text) => return Some(HelperEvent::Notice(text)),
                Step::Retry => {
                    self.finished = true;
                    return Some(HelperEvent::Failure);
                }
                Step::End(ending) => {
                    self.finished = true;
                    return Some(HelperEvent::Ended(ending));
                }
                Step::Ignore => {}
            }
        }
        None
    }

    /// Reads until a whole line, the bound, the end of input, silence or a
    /// halt. A line is at most [`MAX_HELPER_LINE`] bytes of content: no more
    /// than one byte past that is ever asked of the socket without a newline
    /// having been seen.
    fn next(&mut self, halted: &dyn Fn() -> bool) -> Next {
        loop {
            if let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = self.pending.drain(..=end).collect();
                line.pop();
                return Next::Line(line);
            }
            if self.pending.len() > MAX_HELPER_LINE {
                self.pending.clear();
                return Next::TooLong;
            }
            if halted() {
                return Next::Halted;
            }
            let mut chunk = [0u8; READ_CHUNK];
            let room = (MAX_HELPER_LINE + 1 - self.pending.len()).min(READ_CHUNK);
            let Some(window) = chunk.get_mut(..room) else {
                return Next::Eof;
            };
            match self.stream.read(window) {
                Ok(0) => return Next::Eof,
                Ok(count) => {
                    if let Some(read) = chunk.get(..count) {
                        self.pending.extend_from_slice(read);
                    }
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    if self.quiet_since.elapsed() >= self.timing.silence {
                        return Next::Silent;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
                Err(err) => {
                    tracing::debug!(error = %err, "polkit: reading the helper failed");
                    return Next::Eof;
                }
            }
        }
    }

    /// Writes the person's `answer` to the helper and drops it.
    ///
    /// Only an open prompt takes an answer: with none the answer is dropped
    /// unwritten (a line too many would answer the helper's *next* PAM
    /// prompt). A write that fails ends the conversation, as the helper's end
    /// of input does.
    ///
    /// # Errors
    ///
    /// The [`Ending`] when the write failed.
    pub fn answer(&mut self, exchange: &mut Exchange, answer: impl Answer) -> Result<(), Ending> {
        if !self.prompt_open || self.finished {
            tracing::debug!("polkit: an answer with no open prompt is dropped");
            return Ok(());
        }
        self.prompt_open = false;
        let written = answer
            .write_line_to(&mut self.stream)
            .and_then(|()| self.stream.flush());
        // Consumed on every path: the answer ends here.
        drop(answer);
        match written {
            Ok(()) => {
                exchange.answer_sent();
                self.quiet_since = Instant::now();
                Ok(())
            }
            Err(err) => {
                tracing::debug!(error = %err, "polkit: writing the answer failed");
                self.finished = true;
                match exchange.on_eof() {
                    Step::End(ending) => Err(ending),
                    _ => Ok(()),
                }
            }
        }
    }
}

// ---- the thread ------------------------------------------------------------------------------

/// Why [`Helper::answer`] kept the answer: it is dropped (wiped) either way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerRefused {
    /// An answer is already waiting for the thread.
    Busy,
    /// The conversation is over.
    Over,
}

type Publish = Box<dyn Fn(HelperEvent) -> bool + Send>;

/// A request's helper conversations, on their own thread: the loop's handle.
/// Dropping it, or [`Helper::cancel`], closes the socket and drops any answer
/// still on its way.
pub struct Helper<S: Answer = Secret> {
    answers: SyncSender<S>,
    /// The thread's own flag: raised by [`Helper::cancel`] before anything
    /// else, so what the thread says from then on is not published.
    halt: Arc<AtomicBool>,
    worker: Option<Worker>,
}

impl<S: Answer> Helper<S> {
    /// Moves `answer` to the thread through the one-slot channel. Refused, it
    /// is dropped here.
    ///
    /// # Errors
    ///
    /// [`AnswerRefused`] when the thread cannot take it.
    pub fn answer(&self, answer: S) -> Result<(), AnswerRefused> {
        match self.answers.try_send(answer) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(AnswerRefused::Busy),
            Err(TrySendError::Disconnected(_)) => Err(AnswerRefused::Over),
        }
    }

    /// Closes the conversation: the thread closes its socket and drops an
    /// answer on its way, and nothing it says from now on is published.
    ///
    /// The loop calls this, so it **does not wait** for the thread (N-22):
    /// the flag is raised and the answer channel closed here, and the thread
    /// is handed to a [`Reaper`], which joins it within [`CANCEL_WAIT`] and
    /// leaves one that is stuck in a call without a timeout. The reaper is
    /// returned for a caller that wants to wait; the loop drops it.
    pub fn cancel(mut self) -> Reaper {
        let Some(worker) = self.worker.take() else {
            return Reaper::default();
        };
        self.halt.store(true, Ordering::Release);
        // Drop the sender: a thread waiting for an answer wakes.
        let Helper { answers, .. } = self;
        drop(answers);
        Reaper::spawn("polkit-helper", move || {
            if !worker.join_within(CANCEL_WAIT) {
                tracing::warn!("polkit: the helper thread did not stop in time");
            }
        })
    }
}

/// Starts the conversations of one request on a thread of their own.
///
/// `publish` receives the events and returns `false` once nobody listens;
/// `shutdown` is the shell's own flag, which stops the thread too.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    input: HelperInput,
    publish: impl Fn(HelperEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<Helper> {
    spawn_with(input, publish, shutdown, connect)
}

/// [`spawn`] with the way a conversation is opened chosen by the caller:
/// private, so only the production [`spawn`] and the tests can reach it.
fn spawn_with<S: Answer>(
    input: HelperInput,
    publish: impl Fn(HelperEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
    connector: impl FnMut(&HelperInput) -> Result<Conversation, ConnectRefusal> + Send + 'static,
) -> io::Result<Helper<S>> {
    let (answers, inbox) = mpsc::sync_channel(1);
    // The thread's own flag: a finished conversation must not raise the
    // shell's (the worker raises its flag when it goes away).
    let own = Arc::new(AtomicBool::new(false));
    let shell = Arc::clone(shutdown);
    let flag = Arc::clone(&own);
    let publish: Publish = Box::new(publish);
    let worker = Worker::spawn("polkit-helper", &own, move || {
        let halted = || flag.load(Ordering::Acquire) || shell.load(Ordering::Acquire);
        run(&input, connector, &inbox, &*publish, &halted);
    })?;
    Ok(Helper {
        answers,
        halt: own,
        worker: Some(worker),
    })
}

/// The thread's body: conversations until an ending, a halt, or nobody
/// listening. Everything it owns (the socket, the cookie, the answer in the
/// channel) is dropped when it returns, by any path including a panic.
///
/// Nothing is published once `halted` says so: a conversation the loop
/// cancelled, or one the shell is leaving, must not end a request that is no
/// longer its own (the loop also ignores an id it does not know, but the
/// thread does not rely on that).
fn run<S: Answer>(
    input: &HelperInput,
    mut connector: impl FnMut(&HelperInput) -> Result<Conversation, ConnectRefusal>,
    inbox: &Receiver<S>,
    publish: &dyn Fn(HelperEvent) -> bool,
    halted: &dyn Fn() -> bool,
) {
    let emit = |event: HelperEvent| !halted() && publish(event);
    let mut exchange = Exchange::new();
    loop {
        if halted() {
            return;
        }
        let mut conversation = match connector(input) {
            Ok(conversation) => conversation,
            Err(refusal) => {
                tracing::warn!(error = %crate::error_chain(&refusal), "polkit: the helper's socket was not used");
                if let Step::End(ending) = exchange.on_unreachable() {
                    emit(HelperEvent::Ended(ending));
                }
                return;
            }
        };
        loop {
            let Some(event) = conversation.next_event(&mut exchange, halted) else {
                return;
            };
            match event {
                HelperEvent::Prompt(text) => {
                    if !emit(HelperEvent::Prompt(text)) {
                        return;
                    }
                    let Some(answer) = wait_for_answer(inbox, halted) else {
                        return;
                    };
                    if let Err(ending) = conversation.answer(&mut exchange, answer) {
                        emit(HelperEvent::Ended(ending));
                        return;
                    }
                }
                HelperEvent::Failure => {
                    if !emit(HelperEvent::Failure) {
                        return;
                    }
                    // Closes this socket; the next conversation reuses the
                    // exchange and so the attempt count.
                    break;
                }
                HelperEvent::Ended(ending) => {
                    emit(HelperEvent::Ended(ending));
                    return;
                }
                notice @ HelperEvent::Notice(_) => {
                    if !emit(notice) {
                        return;
                    }
                }
            }
        }
    }
}

/// The person's answer, once it arrives; `None` when halted or when the
/// handle is gone.
fn wait_for_answer<S>(inbox: &Receiver<S>, halted: &dyn Fn() -> bool) -> Option<S> {
    loop {
        if halted() {
            return None;
        }
        match inbox.recv_timeout(POLL) {
            Ok(answer) => return Some(answer),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

#[cfg(test)]
mod tests;
