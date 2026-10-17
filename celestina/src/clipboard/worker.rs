// Written for the clipboard history (CEL-2-G, T3) in the shape of the energy
// workers (`energy/idle.rs`, `energy/nightlight.rs`): its own Wayland
// connection over the shared pump in `energy/wire.rs`, a `Transport` seam so
// the rules are tested without a display, a bounded command queue and bounded
// waits so the join is bounded. The protocol conversation follows, as
// reference only, the halted shell's provider adapter: today
// `celestina-qt/src/provider_adapter/clipboard.rs`, which at
// 5924ad457b55d56dd52a7c7d5c69bc255117c5c4 (the commit it was read at) lived
// at `celestina/src/provider_adapter/clipboard.rs`, before the halted tree
// moved to `celestina-qt/` on 2026-10-14. Its skip-the-next-selection echo
// flag is deliberately not carried over: the echo is recognised by the
// identity of the source that sent it.

//! The clipboard worker: one thread that owns its own Wayland connection,
//! binds `ext_data_control_manager_v1` and the seat's data device, and keeps
//! the history of the regular selection.
//!
//! What may be kept is decided by [`celestina_shell::clipboard`]; this module
//! transports it (design §1–§3, §5, §7):
//!
//! - only the regular selection is watched; a primary-selection offer is
//!   destroyed as it arrives;
//! - a selection whose MIME types carry an exclusion hint is destroyed
//!   without anything being read, and so is one that offers neither text nor
//!   `image/png`;
//! - otherwise one text MIME or `image/png` is read through a pipe, on this
//!   thread and never the loop's, in non-blocking chunks, bounded in bytes
//!   ([`MAX_TEXT_BYTES`], [`MAX_IMAGE_BYTES`]) and in time ([`READ_DEADLINE`]).
//!   An owner that answers too much, too slowly, or something that is not
//!   what it offered costs that selection only, never the connection;
//! - a burst of selections in one dispatch reads only the newest;
//! - choosing an entry makes this worker the selection owner with one source
//!   that offers the entry's MIME types plus one private identity type. The
//!   compositor echoes the selection back as an ordinary offer; the identity
//!   type marks it as this worker's own, and it is skipped without a read (a
//!   read would wait on this very thread to answer it). The source answers
//!   `send` with bounded writes and is dropped, with its data, once
//!   cancelled;
//! - while paused, selections are destroyed unread; the connection stays;
//! - the persisted history is loaded here, on this thread, through the
//!   caller's [`Persist`]; a file that held more than it gave back (expired
//!   or refused entries) is rewritten, or deleted when nothing is left, at
//!   once;
//! - every change of the history is persisted through [`Persist`], unless the
//!   worker has been told to stop, and published as a [`ClipboardEvent`]
//!   carrying bounded previews only and the snapshot number the rows belong
//!   to; `select` and `remove` from a surface name that snapshot, and a
//!   command for an older one is refused rather than applied to whatever
//!   entry now sits at that position;
//! - once the entry this worker serves as the selection leaves the history
//!   (removed, expired, evicted, cleared), the source and its data go too.
//!
//! Contents never reach a log, an error, a panic or a `Debug`: tracing
//! carries counts, byte sizes, MIME types and reasons. Every wait is bounded
//! by [`POLL`] against the shutdown flag, so [`Worker::join`] is too.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina_shell::clipboard::{
    choose, is_excluded, Choice, Content, History, EXCLUSION_HINTS, IMAGE_READ_DEADLINE,
    MAX_IMAGE_BYTES, MAX_PUBLISHED_ROWS, MAX_TEXT_BYTES,
};
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use rustix::io::Errno;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device_proto, ext_data_control_manager_v1 as manager_proto,
    ext_data_control_offer_v1 as offer_proto, ext_data_control_source_v1 as source_proto,
};

use crate::energy::wire::{self, Link, QueueRefusal, Wire, COMMAND_QUEUE, POLL, UNAVAILABLE};
use crate::providers::Worker;

use super::thumb::{Thumb, Thumbs};

/// How many protocol events one dispatch may carry before the compositor is
/// treated as hostile. Offered MIME types are not counted here: they are
/// bounded per offer instead.
const MAX_EVENTS: usize = 1024;
/// How long a selection owner has to answer a read, text or image alike.
pub const READ_DEADLINE: Duration = IMAGE_READ_DEADLINE;
/// How long a client pasting from this worker's source has to take the data.
pub const WRITE_DEADLINE: Duration = Duration::from_secs(2);
/// One read or write of a pipe moves at most this much.
const CHUNK_BYTES: usize = 64 * 1024;
/// How long the first sync after connecting may take. The nest of the smoke
/// starts beside other processes on a loaded machine and answered after more
/// than the pump's one second; this wait is on the worker's own thread and
/// stops with the shell's flags, so a longer one costs nothing at rest.
const STARTUP_SYNC_TIMEOUT: Duration = Duration::from_secs(5);
/// How many times the worker connects before it reports the duty unavailable,
/// when only the compositor's answer was late.
const CONNECT_ATTEMPTS: usize = 2;
/// The most MIME types kept for one offer; exclusion hints are always kept.
const MAX_OFFER_MIMES: usize = 64;
/// The longest MIME type kept; a longer one is not a MIME type this worker
/// reads.
const MAX_MIME_BYTES: usize = 255;
/// The private MIME type a source of this worker offers beside the entry's,
/// followed by the session's nonce and the source's number. It is how the
/// echo of the worker's own selection is recognised.
const IDENTITY_MIME: &str = "application/x-celestina-clipboard-source;id=";
/// The most characters of a foreign MIME type a log line carries.
const MAX_LOGGED_MIME_CHARS: usize = 64;

/// A MIME type another client chose, as a log line may carry it: printable
/// ASCII only (anything else becomes `?`), at most [`MAX_LOGGED_MIME_CHARS`]
/// characters, with the full length when it was cut.
fn loggable_mime(mime: &str) -> String {
    let mut shown: String = mime
        .chars()
        .take(MAX_LOGGED_MIME_CHARS)
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '?'
            }
        })
        .collect();
    if mime.chars().count() > MAX_LOGGED_MIME_CHARS {
        shown.push_str(&format!("... ({} bytes)", mime.len()));
    }
    shown
}

/// The seconds since the Unix epoch, the clock [`spawn`]'s callers normally
/// pass. A clock before the epoch reads as zero, which expires every entry
/// rather than extending any.
#[must_use]
pub fn system_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// What one published row shows. Never the entry itself: a bounded,
/// whitespace-collapsed line for text, and only a size for an image.
#[derive(Clone, PartialEq, Eq)]
pub enum Preview {
    /// The entry's [`preview`](celestina_shell::clipboard::preview).
    Text(String),
    /// An image entry, by its size in bytes and its bounded thumbnail (none
    /// only when the entry's thumbnail is not at hand).
    Png { bytes: usize, thumb: Option<Thumb> },
}

impl fmt::Debug for Preview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter
                .debug_struct("Text")
                .field("bytes", &text.len())
                .finish(),
            Self::Png { bytes, thumb } => formatter
                .debug_struct("Png")
                .field("bytes", bytes)
                .field("thumb", thumb)
                .finish(),
        }
    }
}

/// One published row of the history: its position, which `select` and
/// `remove` address together with the snapshot of the
/// [`ClipboardEvent::Changed`] that carried it, and its preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub index: usize,
    pub preview: Preview,
}

/// What the clipboard worker reports to the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardEvent {
    /// The history changed: at most [`MAX_PUBLISHED_ROWS`] rows, newest
    /// first, how many entries it holds in all, and the snapshot number of
    /// these rows. Every `Changed` of one worker has a larger snapshot than
    /// the one before; a `select` or `remove` that names a snapshot other
    /// than the latest is refused.
    Changed {
        rows: Vec<Row>,
        total: usize,
        snapshot: u64,
    },
    /// The duty cannot work in this session, or lost its connection; the
    /// worker does nothing more.
    Unavailable(String),
    /// A command could not be carried out, with the reason.
    Refused(String),
    /// Reading is paused (`true`) or running again (`false`).
    Paused(bool),
}

/// What [`Persist::load`] found to start from.
#[derive(Debug, Default)]
pub struct Restored {
    /// The history to start from.
    pub history: History,
    /// Whether what is persisted holds more than `history` (entries the load
    /// left out, or a file it refused), so it must be rewritten at once.
    pub stale: bool,
}

/// Where the history comes from at start and goes after every change. The
/// worker never touches a file itself.
pub trait Persist {
    /// Reads the persisted history as it stands at `now`. Runs once, on the
    /// worker thread, never on the loop. A failure is the implementation's
    /// to log; it answers an empty history then.
    fn load(&mut self, now: u64) -> Restored;
    /// Writes `history` as it is at `now`.
    ///
    /// # Errors
    ///
    /// Why it was not written; the worker logs it and carries on.
    fn save(&mut self, history: &History, now: u64) -> Result<(), PersistError>;
    /// Removes what was written, because the history is empty.
    ///
    /// # Errors
    ///
    /// As [`Persist::save`].
    fn delete(&mut self) -> Result<(), PersistError>;
}

/// Why [`Persist`] failed. It must not carry content.
pub type PersistError = Box<dyn std::error::Error + Send + Sync>;

/// Why a handle method refused a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClipboardRefused {
    /// [`COMMAND_QUEUE`] commands are already waiting.
    #[error("the clipboard worker is still applying earlier commands")]
    Busy,
    /// The worker has stopped.
    #[error("the clipboard worker has stopped")]
    Gone,
}

impl QueueRefusal for ClipboardRefused {
    const BUSY: Self = Self::Busy;
    const GONE: Self = Self::Gone;
}

/// What the loop can ask of the worker. Indices are rows' positions; `seen`
/// is the snapshot the rows were shown from, `None` for the latest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Select { index: usize, seen: Option<u64> },
    Remove { index: usize, seen: Option<u64> },
    Clear,
    Pause,
    Resume,
}

/// The loop's side of the clipboard worker.
#[derive(Clone, Debug)]
pub struct ClipboardHandle {
    commands: SyncSender<Command>,
    stop: Arc<AtomicBool>,
}

impl ClipboardHandle {
    /// Makes entry `index` the selection again, offering it to every client.
    /// `seen` is the snapshot of the rows the index was read from; the
    /// worker refuses the command when its history has changed since. `None`
    /// addresses the latest rows, whatever they are.
    ///
    /// # Errors
    ///
    /// [`ClipboardRefused::Busy`] when the queue is full,
    /// [`ClipboardRefused::Gone`] when the worker has stopped. A missing
    /// entry, or a stale snapshot, is reported later as
    /// [`ClipboardEvent::Refused`].
    pub fn select(&self, index: usize, seen: Option<u64>) -> Result<(), ClipboardRefused> {
        self.send(Command::Select { index, seen })
    }

    /// Removes entry `index` of the rows of snapshot `seen` (`None`: the
    /// latest).
    ///
    /// # Errors
    ///
    /// As [`ClipboardHandle::select`].
    pub fn remove(&self, index: usize, seen: Option<u64>) -> Result<(), ClipboardRefused> {
        self.send(Command::Remove { index, seen })
    }

    /// Empties the history.
    ///
    /// # Errors
    ///
    /// As [`ClipboardHandle::select`].
    pub fn clear(&self) -> Result<(), ClipboardRefused> {
        self.send(Command::Clear)
    }

    /// Stops reading selections; the connection stays.
    ///
    /// # Errors
    ///
    /// As [`ClipboardHandle::select`].
    pub fn pause(&self) -> Result<(), ClipboardRefused> {
        self.send(Command::Pause)
    }

    /// Reads selections again.
    ///
    /// # Errors
    ///
    /// As [`ClipboardHandle::select`].
    pub fn resume(&self) -> Result<(), ClipboardRefused> {
        self.send(Command::Resume)
    }

    /// Asks the worker to stop: it releases its source and its connection
    /// within one [`POLL`], a read or a paste under way included. Never
    /// refused, not even by a full queue, and it does not raise the shell's
    /// shutdown flag, so a duty turned off live stops alone.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    fn send(&self, command: Command) -> Result<(), ClipboardRefused> {
        wire::offer(&self.commands, command)
    }
}

/// Starts the clipboard worker. With `load`, its history starts from
/// [`Persist::load`], read on the worker thread; without, it starts empty
/// (the duty was turned off and on again, which forgets everything).
///
/// `publish` receives every [`ClipboardEvent`] and returns whether the loop
/// still listens; once it returns `false` the worker stops. The first event
/// is the initial `Changed`. `persist` is called on the worker thread after
/// every change, never once the worker is told to stop; `now` supplies the
/// seconds the entries are stamped and expired with ([`system_now`] outside
/// tests). The worker also stops on [`ClipboardHandle::stop`], when
/// `shutdown` is raised, or once every handle is dropped.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    load: bool,
    persist: impl Persist + Send + 'static,
    now: impl Fn() -> u64 + Send + 'static,
    publish: impl FnMut(ClipboardEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(ClipboardHandle, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let stop = Arc::new(AtomicBool::new(false));
    let flags = Flags {
        shutdown: Arc::clone(shutdown),
        stop: Arc::clone(&stop),
    };
    let worker = Worker::spawn("clipboard", shutdown, move || {
        let parts = Parts {
            history: History::new(),
            load,
            persist,
            clock: now,
            publish,
            identity: Identity::new(session_nonce()),
            deadlines: Deadlines::default(),
        };
        run(connect_with_retry(&flags), parts, &queue, &flags, POLL);
    })?;
    Ok((ClipboardHandle { commands, stop }, worker))
}

/// The connection itself is gone, cannot be trusted, or never offered the
/// duty.
#[derive(Debug, thiserror::Error)]
enum Lost {
    #[error("the compositor offers no seat")]
    NoSeat,
    #[error("the compositor offers no ext-data-control")]
    NoManager,
    #[error("the compositor withdrew the seat or the data-control manager")]
    GlobalGone,
    #[error("the compositor finished the data-control device")]
    Finished,
    #[error(transparent)]
    Wire(#[from] wire::Lost),
}

/// The transport's name for one offer it holds.
type OfferId = u64;
/// The transport's name for one source of this worker.
type SourceId = u64;

/// What the protocol reports, reduced to what the duty needs.
#[derive(Debug)]
enum Signal {
    /// A new regular selection and its (bounded) MIME types.
    Selection { offer: OfferId, mimes: Vec<String> },
    /// The regular selection is empty.
    Cleared,
    /// A client asks a source of this worker for `mime`, written into `fd`.
    SendRequest {
        source: SourceId,
        mime: String,
        fd: OwnedFd,
    },
    /// A source of this worker is no longer the selection.
    Cancelled { source: SourceId },
}

/// The protocol seam.
trait Transport {
    /// Asks the owner of `offer` to write `mime` into `fd`. Queued until
    /// [`Transport::flush`]; an offer the transport no longer holds is
    /// ignored, and the read then ends empty.
    fn receive(&mut self, offer: OfferId, mime: &str, fd: BorrowedFd<'_>) -> Result<(), Lost>;
    /// Destroys `offer`; nothing more is read from it.
    fn discard(&mut self, offer: OfferId);
    /// Creates a source offering `mimes` and makes it the selection.
    fn own(&mut self, mimes: &[&str]) -> Result<SourceId, Lost>;
    /// Destroys `source`, if the transport still holds it.
    fn disown(&mut self, source: SourceId);
    fn flush(&mut self) -> Result<(), Lost>;
    /// Waits at most `timeout` for events and returns them.
    fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost>;
}

/// A random number for this worker, so the identity type of its sources is
/// not one another process could have chosen by accident.
fn session_nonce() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.finish()
}

/// The identity of this worker's sources: the MIME type each offers, and
/// the test that recognises it in an offer.
struct Identity {
    prefix: String,
    issued: u64,
}

impl Identity {
    fn new(nonce: u64) -> Self {
        Self {
            prefix: format!("{IDENTITY_MIME}{nonce:016x}-"),
            issued: 0,
        }
    }

    /// The identity type of the next source.
    fn next(&mut self) -> String {
        self.issued = self.issued.wrapping_add(1);
        format!("{}{}", self.prefix, self.issued)
    }

    /// Whether an offer comes from one of this worker's sources: the echo of
    /// its own selection, current or earlier.
    fn recognises<S: AsRef<str>>(&self, mimes: &[S]) -> bool {
        mimes
            .iter()
            .any(|mime| mime.as_ref().starts_with(&self.prefix))
    }

    /// Whether `mime` is an identity type, which a paste is never given.
    fn is_identity(mime: &str) -> bool {
        mime.starts_with(IDENTITY_MIME)
    }
}

/// How long a read and a write may take.
#[derive(Clone, Copy, Debug)]
struct Deadlines {
    read: Duration,
    write: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            read: READ_DEADLINE,
            write: WRITE_DEADLINE,
        }
    }
}

/// Why one transfer was abandoned. Carries no byte of it.
#[derive(Debug, PartialEq, Eq)]
enum Dropped {
    /// The other end did not finish within the deadline.
    Deadline,
    /// The other end sent more than the bound.
    TooLarge,
    /// The pipe failed.
    Io(ErrorKind),
    /// The worker is stopping.
    Stopped,
}

impl fmt::Display for Dropped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deadline => formatter.write_str("the other end did not finish in time"),
            Self::TooLarge => formatter.write_str("the other end sent more than the bound"),
            Self::Io(kind) => write!(formatter, "the pipe failed: {kind}"),
            Self::Stopped => formatter.write_str("the worker is stopping"),
        }
    }
}

impl From<io::Error> for Dropped {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

impl From<Errno> for Dropped {
    fn from(errno: Errno) -> Self {
        Self::Io(io::Error::from(errno).kind())
    }
}

fn set_nonblocking(fd: &OwnedFd) -> Result<(), Dropped> {
    let flags = fcntl_getfl(fd)?;
    fcntl_setfl(fd, flags | OFlags::NONBLOCK)?;
    Ok(())
}

/// The time left before `deadline`, or [`Dropped::Deadline`].
fn remaining(deadline: Instant) -> Result<Duration, Dropped> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(Dropped::Deadline)
}

/// Waits at most `timeout` for `fd` to be ready for `flags`. A wake for any
/// reason, or none, returns; the caller retries its read or write.
fn wait_for(fd: &File, flags: PollFlags, timeout: Duration) -> Result<(), Dropped> {
    let mut fds = [PollFd::new(fd, flags)];
    let timeout = Timespec::try_from(timeout).unwrap_or(Timespec {
        tv_sec: 0,
        tv_nsec: 25_000_000,
    });
    match poll(&mut fds, Some(&timeout)) {
        Ok(_) | Err(Errno::INTR) => Ok(()),
        Err(errno) => Err(errno.into()),
    }
}

/// Reads `fd` to its end in non-blocking chunks: at most `limit` bytes,
/// until `deadline`, and only while `stop` says to go on. Waits are at most
/// [`POLL`] long, so `stop` is looked at often.
fn read_bounded(
    fd: OwnedFd,
    limit: usize,
    deadline: Instant,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Dropped> {
    set_nonblocking(&fd)?;
    let mut file = File::from(fd);
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    loop {
        if stop() {
            return Err(Dropped::Stopped);
        }
        let left = remaining(deadline)?;
        match file.read(&mut chunk) {
            Ok(0) => return Ok(bytes),
            Ok(read) => {
                if bytes.len() + read > limit {
                    return Err(Dropped::TooLarge);
                }
                bytes.extend_from_slice(&chunk[..read]);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                wait_for(&file, PollFlags::IN, left.min(POLL))?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
}

/// Writes all of `bytes` into `fd` in non-blocking chunks, until `deadline`
/// and only while `stop` says to go on. The descriptor is closed on return,
/// which is the reader's end of file.
fn write_bounded(
    fd: OwnedFd,
    bytes: &[u8],
    deadline: Instant,
    stop: &dyn Fn() -> bool,
) -> Result<(), Dropped> {
    set_nonblocking(&fd)?;
    let mut file = File::from(fd);
    let mut written = 0;
    while written < bytes.len() {
        if stop() {
            return Err(Dropped::Stopped);
        }
        let left = remaining(deadline)?;
        let end = bytes.len().min(written + CHUNK_BYTES);
        match file.write(&bytes[written..end]) {
            Ok(0) => return Err(Dropped::Io(ErrorKind::WriteZero)),
            Ok(count) => written += count,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                wait_for(&file, PollFlags::OUT, left.min(POLL))?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// The rows a surface may show of `history`.
fn rows(history: &History, thumbs: &Thumbs) -> Vec<Row> {
    history
        .entries()
        .iter()
        .take(MAX_PUBLISHED_ROWS)
        .enumerate()
        .map(|(index, entry)| Row {
            index,
            preview: entry.preview().map_or_else(
                || Preview::Png {
                    bytes: entry.content().len(),
                    thumb: thumbs.get(entry.content()),
                },
                Preview::Text,
            ),
        })
        .collect()
}

/// The source this worker holds the selection with, and what it serves.
struct Owned {
    source: SourceId,
    content: Content,
}

/// Everything the duty is built from besides its transport.
struct Parts<P, C, F> {
    /// The history to start from when `load` is false.
    history: History,
    /// Whether to start from [`Persist::load`] instead.
    load: bool,
    persist: P,
    clock: C,
    publish: F,
    identity: Identity,
    deadlines: Deadlines,
}

/// The history and the selection it may own, over a [`Transport`].
struct Duty<T, P, C, F> {
    transport: T,
    history: History,
    load: bool,
    persist: P,
    clock: C,
    publish: F,
    listening: bool,
    paused: bool,
    owned: Option<Owned>,
    identity: Identity,
    deadlines: Deadlines,
    thumbs: Thumbs,
    /// The snapshot number of the last [`ClipboardEvent::Changed`].
    snapshot: u64,
    /// The worker's stop flags: nothing is persisted once they are raised.
    halt: Flags,
}

impl<T, P, C, F> Duty<T, P, C, F>
where
    T: Transport,
    P: Persist,
    C: Fn() -> u64,
    F: FnMut(ClipboardEvent) -> bool,
{
    fn new(transport: T, parts: Parts<P, C, F>, halt: Flags) -> Self {
        Self {
            transport,
            history: parts.history,
            load: parts.load,
            persist: parts.persist,
            clock: parts.clock,
            publish: parts.publish,
            listening: true,
            paused: false,
            owned: None,
            identity: parts.identity,
            deadlines: parts.deadlines,
            thumbs: Thumbs::default(),
            snapshot: 0,
            halt,
        }
    }

    fn say(&mut self, event: ClipboardEvent) {
        if self.listening {
            self.listening = (self.publish)(event);
        }
    }

    fn publish_rows(&mut self) {
        self.thumbs.keep_only(&self.history);
        self.snapshot = self.snapshot.wrapping_add(1);
        let event = ClipboardEvent::Changed {
            rows: rows(&self.history, &self.thumbs),
            total: self.history.len(),
            snapshot: self.snapshot,
        };
        self.say(event);
    }

    /// Persists and publishes the history after a change. Persisting is
    /// skipped once the worker is told to stop: the duty may be turning off,
    /// and a write now could recreate the file it is deleting (the duty's
    /// [`super::store::Keeper`] refuses such a write as well).
    fn commit(&mut self, now: u64) {
        self.history.expire(now);
        self.release_if_gone();
        if self.halt.raised() {
            tracing::debug!("clipboard: stopping; the change is not persisted");
        } else {
            let outcome = if self.history.is_empty() {
                self.persist.delete()
            } else {
                self.persist.save(&self.history, now)
            };
            if let Err(error) = outcome {
                tracing::warn!(
                    "clipboard: the history was not persisted: {}",
                    crate::error_chain(&*error)
                );
            }
        }
        self.publish_rows();
    }

    /// Gives up the selection this worker owns once its entry is no longer
    /// in the history, as `clear` does: a removed or expired entry must not
    /// go on being served.
    fn release_if_gone(&mut self) {
        let gone = self
            .owned
            .as_ref()
            .is_some_and(|owned| !self.history.holds(&owned.content));
        if gone {
            tracing::debug!("clipboard: the selected entry left the history; its source goes");
            self.release();
        }
    }

    /// Loads the persisted history when asked to, and publishes the starting
    /// history, dropping what expired while the shell was not running. A
    /// persisted history that held more than it gave back is rewritten (or
    /// deleted, when nothing is left) now.
    fn start(&mut self) {
        let now = (self.clock)();
        let mut stale = false;
        if self.load {
            let restored = self.persist.load(now);
            self.history = restored.history;
            stale = restored.stale;
        }
        if self.history.expire(now) || stale {
            self.commit(now);
        } else {
            self.publish_rows();
        }
    }

    /// Drops entries that have expired since the last change.
    fn tick(&mut self) {
        let now = (self.clock)();
        if self.history.expire(now) {
            self.commit(now);
        }
    }

    /// Whether a command naming snapshot `seen` still addresses the rows
    /// this worker published last; says why not when it does not.
    fn current(&mut self, seen: Option<u64>) -> bool {
        match seen {
            Some(seen) if seen != self.snapshot => {
                self.say(ClipboardEvent::Refused(
                    "the clipboard history changed since it was shown".to_owned(),
                ));
                false
            }
            _ => true,
        }
    }

    fn command(&mut self, command: Command) -> Result<(), Lost> {
        match command {
            Command::Select { index, seen } => {
                if self.current(seen) {
                    return self.select(index);
                }
            }
            Command::Remove { index, seen } => {
                if self.current(seen) {
                    if self.history.remove(index) {
                        self.commit((self.clock)());
                    } else {
                        self.say(ClipboardEvent::Refused(format!(
                            "there is no clipboard entry {index}"
                        )));
                    }
                }
            }
            Command::Clear => {
                // Clearing must not keep serving the cleared entry: the
                // selection this worker owns, with its data, goes too.
                self.release();
                if self.history.clear() {
                    self.commit((self.clock)());
                }
            }
            Command::Pause | Command::Resume => {
                // Logged once, by the loop, from the event.
                self.paused = command == Command::Pause;
                self.say(ClipboardEvent::Paused(self.paused));
            }
        }
        Ok(())
    }

    /// Moves entry `index` to the front and becomes the selection owner with
    /// a fresh source offering it.
    fn select(&mut self, index: usize) -> Result<(), Lost> {
        let now = (self.clock)();
        let Some(selected) = self.history.select(index, now) else {
            self.say(ClipboardEvent::Refused(format!(
                "there is no clipboard entry {index}"
            )));
            return Ok(());
        };
        let content = selected.entry.content().clone();
        let identity = self.identity.next();
        let mut mimes = content.mimes().to_vec();
        mimes.push(&identity);
        let source = self.transport.own(&mimes)?;
        tracing::debug!(
            kind = ?content.kind(),
            bytes = content.len(),
            "clipboard: an entry became the selection"
        );
        if let Some(previous) = self.owned.replace(Owned { source, content }) {
            self.transport.disown(previous.source);
        }
        self.transport.flush()?;
        self.commit(now);
        Ok(())
    }

    /// Handles one poll's signals. Of the selections among them only the
    /// newest is read; the others are destroyed unread.
    fn signals(&mut self, signals: Vec<Signal>, stop: &dyn Fn() -> bool) -> Result<(), Lost> {
        let mut newest: Option<(OfferId, Vec<String>)> = None;
        let mut superseded = 0_usize;
        for signal in signals {
            match signal {
                Signal::Selection { offer, mimes } => {
                    if let Some((older, _)) = newest.replace((offer, mimes)) {
                        self.transport.discard(older);
                        superseded += 1;
                    }
                }
                Signal::Cleared => {
                    if let Some((older, _)) = newest.take() {
                        self.transport.discard(older);
                        superseded += 1;
                    }
                }
                Signal::SendRequest { source, mime, fd } => self.send(source, &mime, fd, stop),
                Signal::Cancelled { source } => self.cancelled(source),
            }
        }
        if superseded > 0 {
            tracing::debug!(
                superseded,
                "clipboard: newer selections superseded older ones"
            );
        }
        if let Some((offer, mimes)) = newest {
            self.selection(offer, &mimes, stop)?;
        }
        self.transport.flush()
    }

    /// Decides on one new selection and, when it may be kept, reads it.
    fn selection(
        &mut self,
        offer: OfferId,
        mimes: &[String],
        stop: &dyn Fn() -> bool,
    ) -> Result<(), Lost> {
        if self.paused {
            self.transport.discard(offer);
            return Ok(());
        }
        if self.identity.recognises(mimes) {
            // The echo of this worker's own source: reading it would wait on
            // this very thread to answer, and the history already has it.
            self.transport.discard(offer);
            tracing::debug!("clipboard: skipped the echo of its own selection");
            return Ok(());
        }
        let (mime, limit) = match choose(mimes) {
            Choice::Excluded => {
                self.transport.discard(offer);
                tracing::info!(
                    mimes = mimes.len(),
                    "clipboard: skipped a selection that carries an exclusion hint"
                );
                return Ok(());
            }
            Choice::Nothing => {
                self.transport.discard(offer);
                tracing::debug!(
                    mimes = mimes.len(),
                    "clipboard: a selection offered neither text nor image/png"
                );
                return Ok(());
            }
            Choice::Text(mime) => (mime, MAX_TEXT_BYTES),
            Choice::Png(mime) => (mime, MAX_IMAGE_BYTES),
        };
        let read = self.receive(offer, mime, limit, stop);
        self.transport.discard(offer);
        let bytes = match read? {
            Ok(bytes) => bytes,
            Err(dropped) => {
                tracing::debug!(
                    %dropped,
                    mime = loggable_mime(mime),
                    "clipboard: dropped a selection"
                );
                return Ok(());
            }
        };
        let read_bytes = bytes.len();
        let content = if limit == MAX_TEXT_BYTES {
            Content::text_from_bytes(bytes)
        } else {
            Content::png(bytes)
        };
        let Some(content) = content else {
            tracing::debug!(
                bytes = read_bytes,
                mime = loggable_mime(mime),
                "clipboard: dropped a selection that is not recordable"
            );
            return Ok(());
        };
        // An image is recorded only with a thumbnail: bytes the bounded
        // decoder refuses are not an image worth keeping, and the decode runs
        // here, on the worker, never on the loop.
        let thumb = match content.as_png().map(Thumb::from_png) {
            Some(Ok(thumb)) => Some(thumb),
            Some(Err(error)) => {
                tracing::debug!(
                    bytes = read_bytes,
                    reason = %crate::error_chain(&error),
                    "clipboard: dropped an image that does not decode within bounds"
                );
                return Ok(());
            }
            None => None,
        };
        // The decode is bounded but not interruptible: a stop raised during
        // it is honoured here, before anything is recorded or persisted.
        if stop() {
            return Ok(());
        }
        let kind = content.kind();
        let now = (self.clock)();
        if self.history.record(content, now) {
            if let (Some(thumb), Some(entry)) = (thumb, self.history.entries().first()) {
                self.thumbs.insert(entry.content(), thumb);
            }
            tracing::debug!(?kind, bytes = read_bytes, "clipboard: recorded a selection");
            self.commit(now);
        }
        Ok(())
    }

    /// Asks the owner of `offer` for `mime` through a fresh pipe and reads
    /// the answer, bounded. The outer error is a lost connection; the inner
    /// one costs this selection only.
    fn receive(
        &mut self,
        offer: OfferId,
        mime: &str,
        limit: usize,
        stop: &dyn Fn() -> bool,
    ) -> Result<Result<Vec<u8>, Dropped>, Lost> {
        let (reader, writer) = match io::pipe() {
            Ok(pipe) => pipe,
            Err(error) => return Ok(Err(error.into())),
        };
        self.transport.receive(offer, mime, writer.as_fd())?;
        self.transport.flush()?;
        // The owner holds its own copy of the write end now; this one must
        // close so the owner's close is the reader's end of file.
        drop(writer);
        let deadline = Instant::now() + self.deadlines.read;
        Ok(read_bounded(OwnedFd::from(reader), limit, deadline, stop))
    }

    /// Serves a paste from this worker's current source; anything else gets
    /// a closed descriptor and no byte.
    fn send(&mut self, source: SourceId, mime: &str, fd: OwnedFd, stop: &dyn Fn() -> bool) {
        let Some(owned) = self.owned.as_ref().filter(|owned| owned.source == source) else {
            return;
        };
        let offered = !Identity::is_identity(mime)
            && owned
                .content
                .mimes()
                .iter()
                .any(|offered| offered.eq_ignore_ascii_case(mime));
        if !offered {
            tracing::debug!(
                mime = loggable_mime(mime),
                "clipboard: a paste asked for a type not offered"
            );
            return;
        }
        let bytes = owned
            .content
            .as_text()
            .map(str::as_bytes)
            .or_else(|| owned.content.as_png())
            .unwrap_or_default();
        let deadline = Instant::now() + self.deadlines.write;
        if let Err(dropped) = write_bounded(fd, bytes, deadline, stop) {
            tracing::debug!(%dropped, bytes = bytes.len(), "clipboard: a paste was abandoned");
        }
    }

    /// Another client took the selection: the source and its data go.
    fn cancelled(&mut self, source: SourceId) {
        if self
            .owned
            .as_ref()
            .is_some_and(|owned| owned.source == source)
        {
            self.owned = None;
        }
        self.transport.disown(source);
    }

    /// Gives up the selection, as the worker stops.
    fn release(&mut self) {
        if let Some(owned) = self.owned.take() {
            self.transport.disown(owned.source);
        }
        let _ = self.transport.flush();
    }
}

/// The two flags that end the worker.
#[derive(Clone)]
struct Flags {
    /// The shell's own shutdown flag.
    shutdown: Arc<AtomicBool>,
    /// This worker's own stop, raised by [`ClipboardHandle::stop`].
    stop: Arc<AtomicBool>,
}

impl Flags {
    fn raised(&self) -> bool {
        self.shutdown.load(Ordering::Acquire) || self.stop.load(Ordering::Acquire)
    }
}

/// Why [`serve`] returned without losing the connection.
#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Stopped,
    LoopGone,
    HandleGone,
}

/// The worker's loop over a connected transport.
fn serve<T, P, C, F>(
    duty: &mut Duty<T, P, C, F>,
    commands: &Receiver<Command>,
    flags: &Flags,
    poll_every: Duration,
) -> Result<Exit, Lost>
where
    T: Transport,
    P: Persist,
    C: Fn() -> u64,
    F: FnMut(ClipboardEvent) -> bool,
{
    let stop = || flags.raised();
    loop {
        if !duty.listening {
            return Ok(Exit::LoopGone);
        }
        if flags.raised() {
            return Ok(Exit::Stopped);
        }
        // At most one queue's worth per round, so a chatty handle cannot
        // starve the connection.
        for _ in 0..=COMMAND_QUEUE {
            match commands.try_recv() {
                Ok(command) => duty.command(command)?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(Exit::HandleGone),
            }
        }
        let signals = duty.transport.poll(poll_every)?;
        duty.signals(signals, &stop)?;
        duty.tick();
    }
}

/// Answers nothing until it is told to stop: the duty is unavailable, so
/// every command is a no-op.
fn idle_unavailable(commands: &Receiver<Command>, flags: &Flags) {
    while !flags.raised() {
        match commands.recv_timeout(POLL) {
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn run<T, P, C, F>(
    connected: Result<T, Lost>,
    mut parts: Parts<P, C, F>,
    commands: &Receiver<Command>,
    flags: &Flags,
    poll_every: Duration,
) where
    T: Transport,
    P: Persist,
    C: Fn() -> u64,
    F: FnMut(ClipboardEvent) -> bool,
{
    let transport = match connected {
        Ok(transport) => transport,
        Err(lost) => {
            tracing::warn!("clipboard: {}", crate::error_chain(&lost));
            if (parts.publish)(ClipboardEvent::Unavailable(UNAVAILABLE.to_owned())) {
                idle_unavailable(commands, flags);
            }
            return;
        }
    };
    let mut duty = Duty::new(transport, parts, flags.clone());
    duty.start();
    match serve(&mut duty, commands, flags, poll_every) {
        Ok(exit) => {
            tracing::debug!(?exit, "clipboard: the worker stops");
            duty.release();
        }
        Err(lost) => {
            let reason = crate::error_chain(&lost);
            tracing::warn!("clipboard: {reason}");
            duty.release();
            duty.say(ClipboardEvent::Unavailable(reason));
            if duty.listening {
                idle_unavailable(commands, flags);
            }
        }
    }
}

// The Wayland transport.

/// The MIME types one offer has announced so far, bounded, and whether any
/// of them was an exclusion hint.
#[derive(Default)]
struct Offered {
    mimes: Vec<String>,
    excluded: bool,
}

impl Offered {
    /// The list the duty decides on: the kept types, plus one canonical
    /// exclusion hint when any spelling of one was offered.
    fn into_mimes(self) -> Vec<String> {
        let mut mimes = self.mimes;
        if self.excluded {
            mimes.extend(EXCLUSION_HINTS.first().map(|hint| (*hint).to_owned()));
        }
        mimes
    }
}

/// The MIME types of one offer, filled by its `offer` events.
#[derive(Default)]
struct OfferMimes(Mutex<Offered>);

/// Keeps `mime` in an offer's list when it fits the bounds. An exclusion
/// hint, in any spelling, only sets the offer's flag: it is never stored, so
/// a long list cannot hide it and thousands of spellings cost nothing.
fn accept_mime(offered: &mut Offered, mime: String) {
    if is_excluded(&[&mime]) {
        offered.excluded = true;
        return;
    }
    if offered.mimes.len() < MAX_OFFER_MIMES && mime.len() <= MAX_MIME_BYTES {
        offered.mimes.push(mime);
    }
}

struct WaylandState {
    seat: Option<(u32, wl_seat::WlSeat)>,
    manager: Option<(u32, manager_proto::ExtDataControlManagerV1)>,
    device: Option<device_proto::ExtDataControlDeviceV1>,
    /// The selection offers handed to the duty and not yet discarded.
    offers: HashMap<OfferId, offer_proto::ExtDataControlOfferV1>,
    /// This worker's sources not yet disowned.
    sources: HashMap<SourceId, source_proto::ExtDataControlSourceV1>,
    next_offer: OfferId,
    next_source: SourceId,
    gone: bool,
    finished: bool,
}

/// What the clipboard connection dispatches into.
type ClipboardWire = Wire<WaylandState, Signal>;

/// The clipboard's own reasons to stop pumping.
fn intact(state: &WaylandState) -> Result<(), Lost> {
    if state.gone {
        return Err(Lost::GlobalGone);
    }
    if state.finished {
        return Err(Lost::Finished);
    }
    Ok(())
}

/// Connects, trying again when only the first sync was late (and no stop was
/// asked meanwhile). Every other loss is final.
fn connect_with_retry(flags: &Flags) -> Result<WaylandTransport, Lost> {
    let mut attempt = 1;
    loop {
        match WaylandTransport::connect() {
            Err(Lost::Wire(wire::Lost::Timeout))
                if attempt < CONNECT_ATTEMPTS && !flags.raised() =>
            {
                tracing::debug!(
                    attempt,
                    "clipboard: the first sync was late; connecting again"
                );
                attempt += 1;
            }
            other => return other,
        }
    }
}

struct WaylandTransport {
    link: Link<WaylandState, Signal>,
}

impl WaylandTransport {
    /// Opens the worker's own connection, discovers the seat and the
    /// data-control manager, and asks for the seat's data device.
    fn connect() -> Result<Self, Lost> {
        let state = WaylandState {
            seat: None,
            manager: None,
            device: None,
            offers: HashMap::new(),
            sources: HashMap::new(),
            next_offer: 0,
            next_source: 0,
            gone: false,
            finished: false,
        };
        let mut link = Link::connect(state, MAX_EVENTS)?;
        link.sync(Instant::now() + STARTUP_SYNC_TIMEOUT, intact)?;
        let Link { qh, wire, .. } = &mut link;
        let state = &mut wire.state;
        let Some((_, seat)) = state.seat.as_ref() else {
            return Err(Lost::NoSeat);
        };
        let Some((_, manager)) = state.manager.as_ref() else {
            return Err(Lost::NoManager);
        };
        state.device = Some(manager.get_data_device(seat, qh, ()));
        link.flush()?;
        Ok(Self { link })
    }
}

impl Transport for WaylandTransport {
    fn receive(&mut self, offer: OfferId, mime: &str, fd: BorrowedFd<'_>) -> Result<(), Lost> {
        if let Some(offer) = self.link.wire.state.offers.get(&offer) {
            offer.receive(mime.to_owned(), fd);
        }
        Ok(())
    }

    fn discard(&mut self, offer: OfferId) {
        if let Some(offer) = self.link.wire.state.offers.remove(&offer) {
            offer.destroy();
        }
    }

    fn own(&mut self, mimes: &[&str]) -> Result<SourceId, Lost> {
        let Link { qh, wire, .. } = &mut self.link;
        let state = &mut wire.state;
        state.next_source = state.next_source.wrapping_add(1);
        let id = state.next_source;
        let (Some((_, manager)), Some(device)) = (state.manager.as_ref(), state.device.as_ref())
        else {
            return Err(Lost::GlobalGone);
        };
        let source = manager.create_data_source(qh, id);
        for mime in mimes {
            source.offer((*mime).to_owned());
        }
        device.set_selection(Some(&source));
        state.sources.insert(id, source);
        Ok(id)
    }

    fn disown(&mut self, source: SourceId) {
        if let Some(source) = self.link.wire.state.sources.remove(&source) {
            source.destroy();
        }
    }

    fn flush(&mut self) -> Result<(), Lost> {
        Ok(self.link.flush()?)
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost> {
        self.link.pump(timeout, intact)?;
        Ok(self.link.wire.take_events())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ClipboardWire {
    fn event(
        wire: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let state = &mut wire.state;
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_seat::WlSeat::interface().name && state.seat.is_none() => {
                let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(1), qh, ());
                state.seat = Some((name, seat));
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == manager_proto::ExtDataControlManagerV1::interface().name
                && state.manager.is_none() =>
            {
                let manager = registry.bind::<manager_proto::ExtDataControlManagerV1, _, _>(
                    name,
                    version.min(1),
                    qh,
                    (),
                );
                state.manager = Some((name, manager));
            }
            wl_registry::Event::GlobalRemove { name }
                if state
                    .seat
                    .as_ref()
                    .is_some_and(|(global, _)| *global == name)
                    || state
                        .manager
                        .as_ref()
                        .is_some_and(|(global, _)| *global == name) =>
            {
                state.gone = true;
            }
            _ => {}
        }
    }
}

delegate_noop!(ClipboardWire: ignore wl_seat::WlSeat);
delegate_noop!(ClipboardWire: manager_proto::ExtDataControlManagerV1);

impl Dispatch<device_proto::ExtDataControlDeviceV1, ()> for ClipboardWire {
    fn event(
        wire: &mut Self,
        _: &device_proto::ExtDataControlDeviceV1,
        event: device_proto::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device_proto::Event::Selection { id: Some(offer) } => {
                let mimes = offer
                    .data::<OfferMimes>()
                    .map(|mimes| {
                        std::mem::take(&mut *mimes.0.lock().unwrap_or_else(PoisonError::into_inner))
                            .into_mimes()
                    })
                    .unwrap_or_default();
                let state = &mut wire.state;
                state.next_offer = state.next_offer.wrapping_add(1);
                let id = state.next_offer;
                state.offers.insert(id, offer);
                wire.push(Signal::Selection { offer: id, mimes });
            }
            device_proto::Event::Selection { id: None } => wire.push(Signal::Cleared),
            // The primary selection is never watched, but its offer is still
            // this client's to release.
            device_proto::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
            device_proto::Event::Finished => wire.state.finished = true,
            _ => {}
        }
    }

    wayland_client::event_created_child!(ClipboardWire, device_proto::ExtDataControlDeviceV1, [
        device_proto::EVT_DATA_OFFER_OPCODE => (offer_proto::ExtDataControlOfferV1, OfferMimes::default()),
    ]);
}

impl Dispatch<offer_proto::ExtDataControlOfferV1, OfferMimes> for ClipboardWire {
    fn event(
        _: &mut Self,
        _: &offer_proto::ExtDataControlOfferV1,
        event: offer_proto::Event,
        mimes: &OfferMimes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer_proto::Event::Offer { mime_type } = event {
            let mut list = mimes.0.lock().unwrap_or_else(PoisonError::into_inner);
            accept_mime(&mut list, mime_type);
        }
    }
}

impl Dispatch<source_proto::ExtDataControlSourceV1, SourceId> for ClipboardWire {
    fn event(
        wire: &mut Self,
        _: &source_proto::ExtDataControlSourceV1,
        event: source_proto::Event,
        source: &SourceId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            source_proto::Event::Send { mime_type, fd } => wire.push(Signal::SendRequest {
                source: *source,
                mime: mime_type,
                fd,
            }),
            source_proto::Event::Cancelled => wire.push(Signal::Cancelled { source: *source }),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::clipboard::{MAX_AGE_SECS, PNG_MIME, PREVIEW_UNITS};
    use std::collections::{HashSet, VecDeque};
    use std::sync::MutexGuard;
    use std::thread;

    /// An arbitrary fixed "now", in seconds.
    const NOW: u64 = 1_800_000_000;
    const SECRET: &str = "hunter2-correct-horse";
    const HINT: &str = "x-kde-passwordManagerHint";

    /// What a fake selection owner does when asked for data.
    #[derive(Clone)]
    enum Owner {
        /// Writes these bytes and closes.
        Answer(Vec<u8>),
        /// Keeps the pipe open and writes nothing.
        Silent,
    }

    #[derive(Default)]
    struct Shared {
        incoming: VecDeque<Signal>,
        owners: HashMap<OfferId, Owner>,
        receives: Vec<(OfferId, String)>,
        discarded: Vec<OfferId>,
        /// Every source created, with the types it offers.
        sources: Vec<(SourceId, Vec<String>)>,
        live_sources: HashSet<SourceId>,
        /// The compositor echoes a new selection back to its setter.
        echo: bool,
        held: Vec<OwnedFd>,
        lost: bool,
    }

    /// A fake data-control compositor: scripted offers and owners.
    #[derive(Clone, Default)]
    struct Fake {
        shared: Arc<Mutex<Shared>>,
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    impl Fake {
        fn state(&self) -> MutexGuard<'_, Shared> {
            lock(&self.shared)
        }

        /// A new selection `offer`, offering `mimes`, owned by `owner`.
        fn offer(&self, offer: OfferId, mimes: &[&str], owner: Owner) {
            let mut shared = self.state();
            shared.owners.insert(offer, owner);
            shared.incoming.push_back(Signal::Selection {
                offer,
                mimes: mimes.iter().map(|mime| (*mime).to_owned()).collect(),
            });
        }

        fn say(&self, signal: Signal) {
            self.state().incoming.push_back(signal);
        }

        fn received(&self) -> Vec<OfferId> {
            self.state()
                .receives
                .iter()
                .map(|(offer, _)| *offer)
                .collect()
        }
    }

    impl Transport for Fake {
        fn receive(&mut self, offer: OfferId, mime: &str, fd: BorrowedFd<'_>) -> Result<(), Lost> {
            let mut shared = self.state();
            shared.receives.push((offer, mime.to_owned()));
            let Ok(fd) = fd.try_clone_to_owned() else {
                return Ok(());
            };
            match shared.owners.get(&offer).cloned() {
                Some(Owner::Answer(bytes)) => {
                    // The owner writes from its own thread, as a real one
                    // would; a closed reader ends it with a broken pipe.
                    thread::spawn(move || {
                        let mut file = File::from(fd);
                        let _ = file.write_all(&bytes);
                    });
                }
                Some(Owner::Silent) => shared.held.push(fd),
                None => drop(fd),
            }
            Ok(())
        }

        fn discard(&mut self, offer: OfferId) {
            self.state().discarded.push(offer);
        }

        fn own(&mut self, mimes: &[&str]) -> Result<SourceId, Lost> {
            let mut shared = self.state();
            let id = SourceId::try_from(shared.sources.len()).unwrap_or(SourceId::MAX) + 1;
            let mimes: Vec<String> = mimes.iter().map(|mime| (*mime).to_owned()).collect();
            shared.sources.push((id, mimes.clone()));
            shared.live_sources.insert(id);
            if shared.echo {
                shared.incoming.push_back(Signal::Selection {
                    offer: 10_000 + id,
                    mimes,
                });
            }
            Ok(id)
        }

        fn disown(&mut self, source: SourceId) {
            self.state().live_sources.remove(&source);
        }

        fn flush(&mut self) -> Result<(), Lost> {
            if self.state().lost {
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            Ok(())
        }

        fn poll(&mut self, timeout: Duration) -> Result<Vec<Signal>, Lost> {
            thread::sleep(timeout.min(Duration::from_millis(2)));
            let mut shared = self.state();
            if shared.lost {
                return Err(Lost::Wire(wire::Lost::Timeout));
            }
            Ok(shared.incoming.drain(..).collect())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Load { now: u64 },
        Save { entries: usize, now: u64 },
        Delete,
    }

    #[derive(Clone, Default)]
    struct Store {
        calls: Arc<Mutex<Vec<Call>>>,
        /// What a load answers: a history and whether the file is stale.
        persisted: Arc<Mutex<(History, bool)>>,
    }

    impl Store {
        fn calls(&self) -> Vec<Call> {
            lock(&self.calls).clone()
        }
    }

    impl Persist for Store {
        fn load(&mut self, now: u64) -> Restored {
            lock(&self.calls).push(Call::Load { now });
            let (history, stale) = lock(&self.persisted).clone();
            Restored { history, stale }
        }

        fn save(&mut self, history: &History, now: u64) -> Result<(), PersistError> {
            lock(&self.calls).push(Call::Save {
                entries: history.len(),
                now,
            });
            Ok(())
        }

        fn delete(&mut self) -> Result<(), PersistError> {
            lock(&self.calls).push(Call::Delete);
            Ok(())
        }
    }

    type Heard = Arc<Mutex<Vec<ClipboardEvent>>>;
    type Clock = Box<dyn Fn() -> u64 + Send>;
    type Publish = Box<dyn FnMut(ClipboardEvent) -> bool + Send>;
    type TestDuty = Duty<Fake, Store, Clock, Publish>;

    fn short_deadlines() -> Deadlines {
        Deadlines {
            read: Duration::from_millis(150),
            write: Duration::from_millis(150),
        }
    }

    fn parts(history: History, store: &Store, heard: &Heard) -> Parts<Store, Clock, Publish> {
        let heard = Arc::clone(heard);
        Parts {
            history,
            load: false,
            persist: store.clone(),
            clock: Box::new(|| NOW),
            publish: Box::new(move |event| {
                lock(&heard).push(event);
                true
            }),
            identity: Identity::new(0x5eed),
            deadlines: short_deadlines(),
        }
    }

    /// Stop flags that are never raised.
    fn idle_flags() -> Flags {
        Flags {
            shutdown: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    struct Bench {
        duty: TestDuty,
        fake: Fake,
        store: Store,
        heard: Heard,
    }

    impl Bench {
        fn new() -> Self {
            Self::with(History::new())
        }

        fn with(history: History) -> Self {
            let fake = Fake::default();
            let store = Store::default();
            let heard = Heard::default();
            let duty = Duty::new(fake.clone(), parts(history, &store, &heard), idle_flags());
            Self {
                duty,
                fake,
                store,
                heard,
            }
        }

        /// One round of the loop's signal handling.
        fn pump(&mut self) {
            let signals = self.fake.state().incoming.drain(..).collect();
            assert!(self.duty.signals(signals, &|| false).is_ok());
        }

        fn texts(&self) -> Vec<String> {
            self.duty
                .history
                .entries()
                .iter()
                .filter_map(|entry| entry.content().as_text().map(str::to_owned))
                .collect()
        }

        fn heard(&self) -> Vec<ClipboardEvent> {
            lock(&self.heard).clone()
        }
    }

    fn text(value: &str) -> Content {
        Content::text(value.to_owned()).unwrap_or_else(|| panic!("recordable test text"))
    }

    fn text_history(values: &[&str]) -> History {
        let mut history = History::new();
        for value in values.iter().rev() {
            history.record(text(value), NOW);
        }
        history
    }

    /// A real 2 x 2 PNG whose red channel is `tag`: the worker decodes
    /// every image it records, so a stand-in of a signature and a byte would
    /// be dropped.
    fn png_bytes(tag: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([tag, 0, 0, 255]));
        let mut out = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encode a png");
        out.into_inner()
    }

    /// A signature followed by a byte: passes the recording rule, does not
    /// decode.
    fn undecodable_png() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 9]
    }

    // --- Reading a selection. ---

    #[test]
    fn a_text_selection_is_read_recorded_persisted_and_published() {
        let mut bench = Bench::new();
        bench.fake.offer(
            1,
            &["text/html", "TEXT/PLAIN;charset=UTF-8"],
            Owner::Answer(b"copied text".to_vec()),
        );
        bench.pump();

        assert_eq!(
            bench.fake.state().receives,
            [(1, "TEXT/PLAIN;charset=UTF-8".to_owned())]
        );
        assert_eq!(bench.fake.state().discarded, [1]);
        assert_eq!(bench.texts(), ["copied text"]);
        assert_eq!(bench.duty.history.entries()[0].captured_at(), NOW);
        assert_eq!(
            bench.store.calls(),
            [Call::Save {
                entries: 1,
                now: NOW
            }]
        );
        assert_eq!(
            bench.heard(),
            [ClipboardEvent::Changed {
                rows: vec![Row {
                    index: 0,
                    preview: Preview::Text("copied text".to_owned())
                }],
                total: 1,
                snapshot: 1,
            }]
        );
    }

    #[test]
    fn a_hinted_selection_is_destroyed_without_any_read() {
        let mut bench = Bench::new();
        bench.fake.offer(
            1,
            &["text/plain", HINT],
            Owner::Answer(SECRET.as_bytes().to_vec()),
        );
        bench.pump();

        assert!(bench.fake.received().is_empty(), "nothing was asked for");
        assert_eq!(bench.fake.state().discarded, [1]);
        assert!(bench.duty.history.is_empty());
        assert!(bench.store.calls().is_empty());
        assert!(bench.heard().is_empty());
    }

    #[test]
    fn an_image_selection_is_published_by_size_only() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["image/png"], Owner::Answer(png_bytes(3)));
        bench.pump();

        assert_eq!(bench.fake.state().receives, [(1, PNG_MIME.to_owned())]);
        assert_eq!(bench.duty.history.len(), 1);
        assert_eq!(
            bench.heard(),
            [ClipboardEvent::Changed {
                rows: vec![Row {
                    index: 0,
                    preview: Preview::Png {
                        bytes: png_bytes(3).len(),
                        thumb: Thumb::from_png(&png_bytes(3)).ok(),
                    }
                }],
                total: 1,
                snapshot: 1,
            }]
        );
    }

    #[test]
    fn an_image_that_does_not_decode_is_not_recorded() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["image/png"], Owner::Answer(undecodable_png()));
        bench.pump();
        assert_eq!(bench.fake.received(), [1], "it was read");
        assert!(bench.duty.history.is_empty());
        assert!(bench.heard().is_empty(), "nothing changed, nothing said");
        assert_eq!(bench.store.calls(), []);
    }

    #[test]
    fn an_image_row_carries_a_bounded_thumbnail_and_never_the_bytes() {
        let mut bench = Bench::new();
        let big = {
            let image = image::RgbaImage::from_pixel(640, 480, image::Rgba([9, 8, 7, 255]));
            let mut out = io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut out, image::ImageFormat::Png)
                .expect("encode a png");
            out.into_inner()
        };
        bench.fake.offer(1, &["image/png"], Owner::Answer(big));
        bench.pump();
        let Some(ClipboardEvent::Changed { rows, .. }) = bench.heard().pop() else {
            panic!("the history changed");
        };
        let Preview::Png {
            thumb: Some(thumb), ..
        } = &rows[0].preview
        else {
            panic!("an image row with a thumbnail");
        };
        assert_eq!((thumb.width(), thumb.height()), (64, 48));
        let printed = format!("{rows:?}");
        assert!(
            printed.contains("64") && !printed.contains("[9, 8, 7"),
            "{printed}"
        );
    }

    #[test]
    fn thumbnails_follow_their_entries_through_select_remove_and_clear() {
        let mut bench = Bench::new();
        for (offer, tag) in [(1, 10), (2, 20)] {
            bench
                .fake
                .offer(offer, &["image/png"], Owner::Answer(png_bytes(tag)));
            bench.pump();
        }
        let thumb_red = |event: &ClipboardEvent, row: usize| match event {
            ClipboardEvent::Changed { rows, .. } => match &rows[row].preview {
                Preview::Png {
                    thumb: Some(thumb), ..
                } => thumb.rgba()[0],
                other => panic!("no thumbnail: {other:?}"),
            },
            other => panic!("not a change: {other:?}"),
        };
        let heard = bench.heard();
        let last = heard.last().expect("an event");
        assert_eq!((thumb_red(last, 0), thumb_red(last, 1)), (20, 10));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());
        let heard = bench.heard();
        let last = heard.last().expect("an event");
        assert_eq!((thumb_red(last, 0), thumb_red(last, 1)), (10, 20));
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 0,
                seen: None
            })
            .is_ok());
        assert_eq!(bench.duty.thumbs.len(), 1);
        assert!(bench.duty.command(Command::Clear).is_ok());
        assert!(bench.duty.thumbs.is_empty());
    }

    #[test]
    fn a_selection_without_text_or_png_is_destroyed_unread() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["text/uri-list", "image/jpeg"], Owner::Silent);
        bench.pump();
        assert!(bench.fake.received().is_empty());
        assert_eq!(bench.fake.state().discarded, [1]);
        assert!(bench.duty.history.is_empty());
    }

    #[test]
    fn a_silent_owner_costs_its_selection_only() {
        let mut bench = Bench::new();
        bench.fake.offer(1, &["text/plain"], Owner::Silent);
        let started = Instant::now();
        bench.pump();
        let waited = started.elapsed();

        assert!(waited >= short_deadlines().read, "{waited:?}");
        assert!(waited < Duration::from_secs(2), "{waited:?}");
        assert_eq!(bench.fake.state().discarded, [1]);
        assert!(bench.duty.history.is_empty());

        // The connection is still in use: the next selection is read.
        bench
            .fake
            .offer(2, &["text/plain"], Owner::Answer(b"next".to_vec()));
        bench.pump();
        assert_eq!(bench.texts(), ["next"]);
    }

    #[test]
    fn an_oversized_owner_costs_its_selection_only() {
        let mut bench = Bench::new();
        bench.fake.offer(
            1,
            &["text/plain"],
            Owner::Answer(vec![b'x'; MAX_TEXT_BYTES + 1]),
        );
        bench.pump();
        assert!(bench.duty.history.is_empty());

        let mut image = png_bytes(0);
        image.resize(MAX_IMAGE_BYTES + 1, 0);
        bench.fake.offer(2, &["image/png"], Owner::Answer(image));
        bench.pump();
        assert!(bench.duty.history.is_empty());

        bench.fake.offer(
            3,
            &["text/plain"],
            Owner::Answer(vec![b'y'; MAX_TEXT_BYTES]),
        );
        bench.pump();
        assert_eq!(bench.duty.history.len(), 1, "the bound itself is kept");
        assert_eq!(bench.fake.state().discarded, [1, 2, 3]);
    }

    #[test]
    fn an_owner_that_answers_what_it_did_not_offer_is_dropped() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["image/png"], Owner::Answer(b"GIF89a".to_vec()));
        bench.pump();
        bench
            .fake
            .offer(2, &["text/plain"], Owner::Answer(vec![0xff, 0xfe, 0x00]));
        bench.pump();
        assert_eq!(bench.fake.received(), [1, 2]);
        assert!(bench.duty.history.is_empty());
    }

    #[test]
    fn a_flood_of_selections_reads_only_the_newest() {
        let mut bench = Bench::new();
        for offer in 1..=500 {
            bench.fake.offer(
                offer,
                &["text/plain"],
                Owner::Answer(format!("copy {offer}").into_bytes()),
            );
        }
        bench.pump();

        assert_eq!(bench.fake.received(), [500]);
        assert_eq!(bench.fake.state().discarded.len(), 500, "every offer died");
        assert_eq!(bench.texts(), ["copy 500"]);
    }

    #[test]
    fn a_cleared_selection_supersedes_an_older_one() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["text/plain"], Owner::Answer(b"gone".to_vec()));
        bench.fake.say(Signal::Cleared);
        bench.pump();
        assert!(bench.fake.received().is_empty());
        assert_eq!(bench.fake.state().discarded, [1]);
    }

    // --- Owning the selection. ---

    #[test]
    fn the_echo_of_its_own_source_is_skipped_by_identity() {
        let mut bench = Bench::with(text_history(&["one", "two"]));
        bench.fake.state().echo = true;
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());

        let (source, offered) = bench.fake.state().sources[0].clone();
        assert_eq!(source, 1);
        assert_eq!(offered[..2], ["text/plain;charset=utf-8", "text/plain"]);
        assert!(offered[2].starts_with(IDENTITY_MIME), "{offered:?}");
        assert_eq!(bench.texts(), ["two", "one"]);

        bench.pump();
        assert!(bench.fake.received().is_empty(), "no read against itself");
        assert_eq!(bench.fake.state().discarded, [10_001]);
        assert_eq!(bench.texts(), ["two", "one"]);

        // A later source is recognised just the same, and so is an earlier
        // one whose echo arrives late.
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());
        bench.fake.say(Signal::Selection {
            offer: 7,
            mimes: offered,
        });
        bench.pump();
        assert!(bench.fake.received().is_empty());
        assert_eq!(bench.texts(), ["one", "two"]);
    }

    #[test]
    fn another_identity_is_not_its_own() {
        let ours = Identity::new(1);
        let mut theirs = Identity::new(2);
        let foreign = theirs.next();
        assert!(!ours.recognises(&["text/plain", foreign.as_str()]));
        assert!(!ours.recognises::<&str>(&[]));
        assert!(Identity::is_identity(&foreign));
    }

    /// Reads what a paste receives through a pipe whose write end is given
    /// to the worker.
    fn paste(bench: &mut Bench, source: SourceId, mime: &str) -> Vec<u8> {
        let (mut reader, writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        bench.fake.say(Signal::SendRequest {
            source,
            mime: mime.to_owned(),
            fd: OwnedFd::from(writer),
        });
        bench.pump();
        let mut bytes = Vec::new();
        assert!(reader.read_to_end(&mut bytes).is_ok());
        bytes
    }

    #[test]
    fn a_selected_entry_is_served_until_cancelled_then_dropped() {
        let mut bench = Bench::with(text_history(&["first", "second"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());
        assert_eq!(paste(&mut bench, 1, "text/plain"), b"second");
        assert_eq!(paste(&mut bench, 1, "TEXT/PLAIN;CHARSET=UTF-8"), b"second");
        assert!(paste(&mut bench, 1, "text/html").is_empty(), "not offered");
        let identity = bench.fake.state().sources[0].1[2].clone();
        assert!(paste(&mut bench, 1, &identity).is_empty());
        assert!(paste(&mut bench, 9, "text/plain").is_empty(), "not ours");

        bench.fake.say(Signal::Cancelled { source: 1 });
        bench.pump();
        assert!(bench.duty.owned.is_none(), "the data is dropped");
        assert!(bench.fake.state().live_sources.is_empty());
        assert!(paste(&mut bench, 1, "text/plain").is_empty());
    }

    #[test]
    fn clear_gives_up_the_selection_it_owns_and_drops_its_data() {
        let mut bench = Bench::with(text_history(&["secret", "other"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert_eq!(paste(&mut bench, 1, "text/plain"), b"secret");
        assert!(bench.duty.command(Command::Clear).is_ok());
        assert!(bench.duty.owned.is_none(), "the data is dropped");
        assert!(bench.fake.state().live_sources.is_empty());
        assert!(paste(&mut bench, 1, "text/plain").is_empty());
        assert!(bench.duty.history.is_empty());
    }

    #[test]
    fn a_new_selection_of_its_own_replaces_the_previous_source() {
        let mut bench = Bench::with(text_history(&["a", "b"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());
        assert_eq!(
            bench.fake.state().live_sources,
            HashSet::from([2]),
            "the first source was destroyed"
        );
        assert_eq!(paste(&mut bench, 2, "text/plain"), b"b");
    }

    #[test]
    fn an_image_entry_is_offered_and_served_as_png() {
        let mut bench = Bench::new();
        bench
            .fake
            .offer(1, &["image/png"], Owner::Answer(png_bytes(5)));
        bench.pump();
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert_eq!(bench.fake.state().sources[0].1[0], PNG_MIME);
        assert_eq!(paste(&mut bench, 1, "image/png"), png_bytes(5));
    }

    #[test]
    fn selecting_a_missing_entry_is_refused_and_owns_nothing() {
        let mut bench = Bench::with(text_history(&["only"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 3,
                seen: None
            })
            .is_ok());
        assert!(bench.fake.state().sources.is_empty());
        assert!(matches!(
            bench.heard().last(),
            Some(ClipboardEvent::Refused(reason)) if reason.contains('3')
        ));
    }

    // --- Commands. ---

    #[test]
    fn pause_reads_nothing_until_resume() {
        let mut bench = Bench::new();
        assert!(bench.duty.command(Command::Pause).is_ok());
        assert_eq!(bench.heard(), [ClipboardEvent::Paused(true)]);
        bench
            .fake
            .offer(1, &["text/plain"], Owner::Answer(b"while paused".to_vec()));
        bench.pump();
        assert!(bench.fake.received().is_empty());
        assert_eq!(bench.fake.state().discarded, [1]);
        assert!(bench.duty.history.is_empty());

        assert!(bench.duty.command(Command::Resume).is_ok());
        bench
            .fake
            .offer(2, &["text/plain"], Owner::Answer(b"after".to_vec()));
        bench.pump();
        assert_eq!(bench.texts(), ["after"]);
        assert_eq!(bench.heard()[1], ClipboardEvent::Paused(false));
    }

    #[test]
    fn remove_and_clear_persist_and_publish() {
        let mut bench = Bench::with(text_history(&["a", "b", "c"]));
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 1,
                seen: None
            })
            .is_ok());
        assert_eq!(bench.texts(), ["a", "c"]);
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 7,
                seen: None
            })
            .is_ok());
        assert!(bench.duty.command(Command::Clear).is_ok());
        assert!(bench.duty.command(Command::Clear).is_ok());
        assert_eq!(
            bench.store.calls(),
            [
                Call::Save {
                    entries: 2,
                    now: NOW
                },
                Call::Delete
            ]
        );
        let heard = bench.heard();
        assert_eq!(heard.len(), 3);
        assert!(matches!(&heard[1], ClipboardEvent::Refused(_)));
        assert_eq!(
            heard[2],
            ClipboardEvent::Changed {
                rows: Vec::new(),
                total: 0,
                snapshot: 2,
            }
        );
    }

    #[test]
    fn the_start_publishes_the_history_and_drops_what_expired() {
        let mut history = History::new();
        history.record(text("old"), NOW - MAX_AGE_SECS);
        history.record(text("fresh"), NOW - 1);
        let mut bench = Bench::with(history);
        bench.duty.start();
        assert_eq!(bench.texts(), ["fresh"]);
        assert_eq!(
            bench.store.calls(),
            [Call::Save {
                entries: 1,
                now: NOW
            }]
        );
        assert!(matches!(
            bench.heard().as_slice(),
            [ClipboardEvent::Changed { total: 1, .. }]
        ));
    }

    #[test]
    fn a_command_for_rows_of_an_older_snapshot_is_refused() {
        let mut bench = Bench::with(text_history(&["a", "b", "c"]));
        bench.duty.start();
        let shown = bench.duty.snapshot;
        assert_eq!(shown, 1);
        // A copy arrives between the page showing the rows and the click.
        bench
            .fake
            .offer(1, &["text/plain"], Owner::Answer(b"new".to_vec()));
        bench.pump();
        assert_eq!(bench.texts(), ["new", "a", "b", "c"]);

        for command in [
            Command::Remove {
                index: 1,
                seen: Some(shown),
            },
            Command::Select {
                index: 1,
                seen: Some(shown),
            },
        ] {
            assert!(bench.duty.command(command).is_ok());
            assert!(matches!(
                bench.heard().last(),
                Some(ClipboardEvent::Refused(reason)) if reason.contains("changed")
            ));
        }
        assert_eq!(bench.texts(), ["new", "a", "b", "c"], "nothing was touched");
        assert!(bench.fake.state().sources.is_empty(), "nothing was owned");

        // The current snapshot, or none (the socket's verbs), is obeyed.
        let current = bench.duty.snapshot;
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 1,
                seen: Some(current),
            })
            .is_ok());
        assert_eq!(bench.texts(), ["new", "b", "c"]);
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 0,
                seen: None,
            })
            .is_ok());
        assert_eq!(bench.texts(), ["b", "c"]);
    }

    #[test]
    fn every_change_carries_a_larger_snapshot() {
        let mut bench = Bench::with(text_history(&["a", "b"]));
        bench.duty.start();
        assert!(bench
            .duty
            .command(Command::Select {
                index: 1,
                seen: None
            })
            .is_ok());
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 1,
                seen: None
            })
            .is_ok());
        let snapshots: Vec<u64> = bench
            .heard()
            .iter()
            .filter_map(|event| match event {
                ClipboardEvent::Changed { snapshot, .. } => Some(*snapshot),
                _ => None,
            })
            .collect();
        assert_eq!(snapshots, [1, 2, 3]);
    }

    #[test]
    fn removing_the_served_entry_gives_up_the_selection() {
        let mut bench = Bench::with(text_history(&["secret", "other"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert_eq!(paste(&mut bench, 1, "text/plain"), b"secret");
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 0,
                seen: None
            })
            .is_ok());
        assert!(bench.duty.owned.is_none(), "the data is dropped");
        assert!(bench.fake.state().live_sources.is_empty());
        assert!(paste(&mut bench, 1, "text/plain").is_empty());
        assert_eq!(bench.texts(), ["other"]);
    }

    #[test]
    fn removing_another_entry_keeps_serving_the_selection() {
        let mut bench = Bench::with(text_history(&["served", "other"]));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 1,
                seen: None
            })
            .is_ok());
        assert!(bench.duty.owned.is_some());
        assert_eq!(paste(&mut bench, 1, "text/plain"), b"served");
    }

    #[test]
    fn expiry_of_the_served_entry_gives_up_the_selection() {
        let clock = Arc::new(Mutex::new(NOW));
        let mut bench = Bench::with(text_history(&["served"]));
        let reading = Arc::clone(&clock);
        bench.duty.clock = Box::new(move || *lock(&reading));
        assert!(bench
            .duty
            .command(Command::Select {
                index: 0,
                seen: None
            })
            .is_ok());
        assert!(bench.duty.owned.is_some());
        *lock(&clock) = NOW + 100;
        bench
            .fake
            .offer(1, &["text/plain"], Owner::Answer(b"fresh".to_vec()));
        bench.pump();
        assert!(bench.duty.owned.is_some(), "still in the history");
        *lock(&clock) = NOW + MAX_AGE_SECS;
        bench.duty.tick();
        assert_eq!(bench.texts(), ["fresh"]);
        assert!(bench.duty.owned.is_none(), "an expired entry is not served");
        assert!(bench.fake.state().live_sources.is_empty());
    }

    #[test]
    fn nothing_is_persisted_once_the_worker_is_told_to_stop() {
        let mut bench = Bench::with(text_history(&["a", "b"]));
        bench.duty.halt.stop.store(true, Ordering::Release);
        assert!(bench
            .duty
            .command(Command::Remove {
                index: 0,
                seen: None
            })
            .is_ok());
        assert!(bench.duty.command(Command::Clear).is_ok());
        assert_eq!(bench.store.calls(), [], "no save and no delete");
        assert!(bench.duty.history.is_empty(), "memory still follows");
    }

    #[test]
    fn the_start_loads_on_the_worker_and_rewrites_a_stale_file() {
        let mut bench = Bench::new();
        bench.duty.load = true;
        *lock(&bench.store.persisted) = (text_history(&["kept"]), true);
        bench.duty.start();
        assert_eq!(bench.texts(), ["kept"]);
        assert_eq!(
            bench.store.calls(),
            [
                Call::Load { now: NOW },
                Call::Save {
                    entries: 1,
                    now: NOW
                }
            ],
            "what the load left out is gone from disk at once"
        );
    }

    #[test]
    fn a_stale_file_with_nothing_left_is_deleted_at_start() {
        let mut bench = Bench::new();
        bench.duty.load = true;
        *lock(&bench.store.persisted) = (History::new(), true);
        bench.duty.start();
        assert_eq!(bench.store.calls(), [Call::Load { now: NOW }, Call::Delete]);
    }

    #[test]
    fn a_clean_file_is_not_rewritten_at_start_and_no_load_reads_nothing() {
        let mut bench = Bench::new();
        bench.duty.load = true;
        *lock(&bench.store.persisted) = (text_history(&["kept"]), false);
        bench.duty.start();
        assert_eq!(bench.store.calls(), [Call::Load { now: NOW }]);

        let mut fresh = Bench::new();
        *lock(&fresh.store.persisted) = (text_history(&["on disk"]), true);
        fresh.duty.start();
        assert!(
            fresh.store.calls().is_empty(),
            "turned on again: starts empty"
        );
        assert!(fresh.duty.history.is_empty());
    }

    #[test]
    fn rows_are_bounded_previews() {
        let long = "word ".repeat(1000);
        let mut history = History::new();
        for index in 0..MAX_PUBLISHED_ROWS + 5 {
            history.record(text(&format!("{index} {long}")), NOW);
        }
        let published = rows(&history, &Thumbs::default());
        assert_eq!(published.len(), MAX_PUBLISHED_ROWS);
        assert_eq!(published[3].index, 3);
        for row in published {
            let Preview::Text(shown) = row.preview else {
                panic!("a text row");
            };
            assert!(shown.encode_utf16().count() <= PREVIEW_UNITS);
        }
    }

    #[test]
    fn events_and_rows_never_print_content() {
        let event = ClipboardEvent::Changed {
            rows: vec![Row {
                index: 0,
                preview: Preview::Text(SECRET.to_owned()),
            }],
            total: 1,
            snapshot: 1,
        };
        let printed = format!("{event:?}");
        assert!(!printed.contains("hunter"), "{printed}");
        assert!(printed.contains(&format!("bytes: {}", SECRET.len())));
    }

    #[test]
    fn a_long_mime_list_never_hides_a_hint() {
        let mut offered = Offered::default();
        for index in 0..MAX_OFFER_MIMES * 2 {
            accept_mime(&mut offered, format!("application/x-filler-{index}"));
        }
        accept_mime(&mut offered, "x".repeat(MAX_MIME_BYTES + 1));
        accept_mime(&mut offered, HINT.to_owned());
        accept_mime(&mut offered, HINT.to_owned());
        let mimes = offered.into_mimes();
        assert_eq!(mimes.len(), MAX_OFFER_MIMES + 1);
        assert_eq!(choose(&mimes), Choice::Excluded);
    }

    #[test]
    fn thousands_of_spellings_of_a_hint_cost_one_flag() {
        let mut offered = Offered::default();
        accept_mime(&mut offered, "text/plain".to_owned());
        // Every ASCII case variant of the hint's first twelve letters.
        for bits in 0_u32..4096 {
            let spelled: String = HINT
                .chars()
                .enumerate()
                .map(|(at, c)| {
                    if at < 12 && bits & (1 << at) != 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect();
            accept_mime(&mut offered, spelled);
        }
        assert!(offered.excluded);
        assert_eq!(offered.mimes, ["text/plain"], "no spelling was stored");
        let mimes = offered.into_mimes();
        assert_eq!(mimes.len(), 2);
        assert_eq!(choose(&mimes), Choice::Excluded);
    }

    #[test]
    fn a_foreign_mime_type_is_logged_bounded_and_printable() {
        assert_eq!(loggable_mime("text/plain"), "text/plain");
        assert_eq!(loggable_mime("a\nb\u{1b}[31m"), "a?b?[31m");
        let long = "x".repeat(300);
        let shown = loggable_mime(&long);
        assert!(shown.starts_with(&"x".repeat(MAX_LOGGED_MIME_CHARS)));
        assert!(shown.ends_with("(300 bytes)"), "{shown}");
        assert!(shown.len() < 100);
    }

    // --- Bounded pipes. ---

    #[test]
    fn a_bounded_read_stops_on_its_deadline_its_bound_and_its_stop() {
        let (reader, writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        let deadline = Instant::now() + Duration::from_millis(40);
        let outcome = read_bounded(OwnedFd::from(reader), 16, deadline, &|| false);
        assert_eq!(outcome, Err(Dropped::Deadline));
        drop(writer);

        let (reader, mut writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        assert!(writer.write_all(&[1; 17]).is_ok());
        let deadline = Instant::now() + Duration::from_secs(5);
        let outcome = read_bounded(OwnedFd::from(reader), 16, deadline, &|| false);
        assert_eq!(outcome, Err(Dropped::TooLarge));

        let (reader, writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        let outcome = read_bounded(OwnedFd::from(reader), 16, deadline, &|| true);
        assert_eq!(outcome, Err(Dropped::Stopped));
        drop(writer);

        let (reader, mut writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        assert!(writer.write_all(&[2; 16]).is_ok());
        drop(writer);
        let outcome = read_bounded(OwnedFd::from(reader), 16, deadline, &|| false);
        assert_eq!(outcome, Ok(vec![2; 16]));
    }

    #[test]
    fn a_bounded_write_gives_up_on_a_reader_that_never_reads() {
        let (reader, writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        let bytes = vec![0_u8; 4 * 1024 * 1024];
        let deadline = Instant::now() + Duration::from_millis(60);
        let outcome = write_bounded(OwnedFd::from(writer), &bytes, deadline, &|| false);
        assert_eq!(outcome, Err(Dropped::Deadline));
        drop(reader);

        let (reader, writer) = io::pipe().unwrap_or_else(|error| panic!("pipe: {error}"));
        drop(reader);
        let deadline = Instant::now() + Duration::from_secs(5);
        let outcome = write_bounded(OwnedFd::from(writer), b"x", deadline, &|| false);
        assert_eq!(outcome, Err(Dropped::Io(ErrorKind::BrokenPipe)));
    }

    // --- The worker thread. ---

    #[test]
    fn the_queue_is_bounded_and_a_stopped_worker_is_gone() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let handle = ClipboardHandle {
            commands,
            stop: Arc::new(AtomicBool::new(false)),
        };
        for _ in 0..COMMAND_QUEUE {
            assert_eq!(handle.pause(), Ok(()));
        }
        assert_eq!(handle.select(0, None), Err(ClipboardRefused::Busy));
        assert_eq!(handle.remove(0, None), Err(ClipboardRefused::Busy));
        assert_eq!(handle.clear(), Err(ClipboardRefused::Busy));
        assert_eq!(handle.resume(), Err(ClipboardRefused::Busy));
        handle.stop();
        assert!(handle.stop.load(Ordering::Acquire), "stop is never refused");
        drop(queue);
        assert_eq!(handle.select(0, None), Err(ClipboardRefused::Gone));
    }

    /// Runs the worker's loop over a fake until the test is done.
    struct Rig {
        fake: Fake,
        store: Store,
        handle: ClipboardHandle,
        shutdown: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
        heard: Heard,
    }

    impl Rig {
        fn start(connected: Result<Fake, Lost>, history: History, deadlines: Deadlines) -> Self {
            let fake = connected.as_ref().map(Clone::clone).unwrap_or_default();
            let store = Store::default();
            let heard = Heard::default();
            let mut parts = parts(history, &store, &heard);
            parts.deadlines = deadlines;
            let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
            let shutdown = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let flags = Flags {
                shutdown: Arc::clone(&shutdown),
                stop: Arc::clone(&stop),
            };
            let thread = thread::spawn(move || {
                run(connected, parts, &queue, &flags, Duration::from_millis(1));
            });
            Self {
                fake,
                store,
                handle: ClipboardHandle { commands, stop },
                shutdown,
                thread: Some(thread),
                heard,
            }
        }

        fn heard(&self) -> Vec<ClipboardEvent> {
            lock(&self.heard).clone()
        }

        fn wait(&self, what: &str, mut ready: impl FnMut(&Self) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !ready(self) {
                assert!(Instant::now() < deadline, "timed out waiting for {what}");
                thread::sleep(Duration::from_millis(2));
            }
        }

        /// Raises the shutdown flag and returns how long the join took.
        fn finish(mut self) -> Duration {
            let started = Instant::now();
            self.shutdown.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                assert!(thread.join().is_ok());
            }
            started.elapsed()
        }
    }

    #[test]
    fn the_worker_records_selects_and_releases_its_source_on_shutdown() {
        let rig = Rig::start(Ok(Fake::default()), History::new(), short_deadlines());
        rig.wait("the initial rows", |rig| rig.heard().len() == 1);
        rig.fake
            .offer(1, &["text/plain"], Owner::Answer(b"live copy".to_vec()));
        rig.wait("the recorded copy", |rig| rig.heard().len() == 2);
        assert_eq!(rig.handle.select(0, None), Ok(()));
        rig.wait("the source", |rig| {
            !rig.fake.state().live_sources.is_empty()
        });

        let fake = rig.fake.clone();
        rig.finish();
        assert!(fake.state().live_sources.is_empty(), "shutdown releases it");
    }

    #[test]
    fn shutdown_during_a_silent_read_is_bounded_by_the_poll() {
        let deadlines = Deadlines {
            read: Duration::from_secs(30),
            write: Duration::from_secs(30),
        };
        let rig = Rig::start(Ok(Fake::default()), History::new(), deadlines);
        rig.fake.offer(1, &["text/plain"], Owner::Silent);
        rig.wait("the read", |rig| !rig.fake.received().is_empty());
        let took = rig.finish();
        assert!(took < Duration::from_millis(500), "{took:?}");
    }

    #[test]
    fn stop_ends_the_worker_without_the_shell_shutdown_flag() {
        let rig = Rig::start(
            Ok(Fake::default()),
            text_history(&["kept"]),
            short_deadlines(),
        );
        assert_eq!(rig.handle.select(0, None), Ok(()));
        rig.wait("the source", |rig| {
            !rig.fake.state().live_sources.is_empty()
        });
        rig.handle.stop();
        let Rig {
            thread,
            fake,
            shutdown,
            store,
            ..
        } = rig;
        if let Some(thread) = thread {
            assert!(thread.join().is_ok());
        }
        assert!(!shutdown.load(Ordering::Acquire));
        assert!(fake.state().live_sources.is_empty());
        assert!(
            !store.calls().contains(&Call::Delete),
            "stopping deletes nothing; turning the duty off is the caller's"
        );
    }

    #[test]
    fn a_worker_whose_handles_are_gone_stops_by_itself() {
        let rig = Rig::start(Ok(Fake::default()), History::new(), short_deadlines());
        let Rig { handle, thread, .. } = rig;
        drop(handle);
        if let Some(thread) = thread {
            assert!(thread.join().is_ok());
        }
    }

    #[test]
    fn a_missing_global_reports_unavailable_and_does_nothing() {
        let rig = Rig::start(Err(Lost::NoManager), History::new(), short_deadlines());
        rig.wait("the Unavailable event", |rig| !rig.heard().is_empty());
        assert_eq!(
            rig.heard(),
            [ClipboardEvent::Unavailable(UNAVAILABLE.to_owned())]
        );
        assert_eq!(rig.handle.clear(), Ok(()));
        thread::sleep(Duration::from_millis(30));
        assert_eq!(rig.heard().len(), 1);
        let took = rig.finish();
        assert!(took < Duration::from_millis(500), "{took:?}");
    }

    #[test]
    fn a_lost_connection_reports_unavailable_and_releases_the_source() {
        let rig = Rig::start(
            Ok(Fake::default()),
            text_history(&["kept"]),
            short_deadlines(),
        );
        assert_eq!(rig.handle.select(0, None), Ok(()));
        rig.wait("the source", |rig| {
            !rig.fake.state().live_sources.is_empty()
        });
        rig.fake.state().lost = true;
        rig.wait("the Unavailable event", |rig| {
            matches!(rig.heard().last(), Some(ClipboardEvent::Unavailable(_)))
        });
        assert!(rig.fake.state().live_sources.is_empty());
        assert!(rig.store.calls().iter().all(|call| call != &Call::Delete));
        rig.finish();
    }

    #[test]
    fn the_system_clock_reads_seconds_since_the_epoch() {
        assert!(system_now() > 1_600_000_000);
    }
}
