//! One running window per application, reached over the session bus.
//!
//! Every first-party application owns a well-known name,
//! `org.celestina.<App>`, and serves one object at `/org/celestina/<App>` with
//! the interface [`INTERFACE`]: `Activate()` brings the window forward and
//! `Open(as paths)` hands it files or folders, each element a
//! [`crate::pathkey`] key so a name that is not UTF-8 crosses byte for byte
//! (ADR 0008).
//!
//! # Claim first
//!
//! A launch claims the name before it builds anything. The object is served on
//! the connection *before* the name is requested, so the instant the bus makes
//! this process the owner, a second launch can already be answered. The bus
//! serialises the requests: of two launches in the same instant exactly one is
//! primary owner. The other finds the name taken, hands its arguments to the
//! owner with a bounded wait and exits without a window. Claiming after the
//! window — what the first copies did — leaves a gap the width of the QML load
//! in which both launches see nobody and both open.
//!
//! The served object holds no Qt state. A request that arrives before the Qt
//! side is ready waits in a bounded [`Inbox`]; [`Owner::attach`] replays it
//! once the application's adapter can queue onto the Qt thread. The served
//! methods answer at once: the caller is a launcher waiting to exit.
//!
//! Reaching the bus is never required. Without a session bus, or with an owner
//! that does not answer within [`HAND_OFF_TIMEOUT`], [`claim`] says so once on
//! stderr and answers [`Claim::Unsettled`]: the launch opens its own window.
//!
//! # Other callers
//!
//! [`open_in`] is the client half for another process — Siderita opening a
//! document in Grafita — that wants an already running instance to take the
//! paths and spawns the program itself only when nobody owns the name.
//!
//! Everything here blocks and belongs on `main` before Qt starts, or on a
//! worker thread; never on the Qt thread.

use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::pathkey;

/// The interface every application serves at [`object_path`].
pub const INTERFACE: &str = "org.celestina.Application1";

/// How long a launch waits for the owner to answer before it gives up and
/// opens its own window: a hung owner must not hang the launch.
pub const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(3);

/// How many requests wait for the Qt side before it attaches. The window is
/// seconds away; a peer flooding the name loses its oldest requests instead of
/// growing memory without bound.
pub const INBOX_LIMIT: usize = 16;

/// The well-known bus name of one application, `org.celestina.<App>`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationName(pub &'static str);

/// The suite's activation names, one owner for each: an application claims
/// its own and Siderita reaches the others. Each is also the application's
/// freedesktop ID (desktop entry basename, icon, Wayland `app_id`).
pub const SIDERITA: ActivationName = ActivationName("org.celestina.Siderita");
pub const GRAFITA: ActivationName = ActivationName("org.celestina.Grafita");
pub const HEMATITA: ActivationName = ActivationName("org.celestina.Hematita");
pub const FLUORITA: ActivationName = ActivationName("org.celestina.Fluorita");
pub const CUPRITA: ActivationName = ActivationName("org.celestina.Cuprita");
pub const CALCITA: ActivationName = ActivationName("org.celestina.Calcita");
pub const SELENITA: ActivationName = ActivationName("org.celestina.Selenita");

/// The object path an application serves [`INTERFACE`] at: the name with its
/// dots as slashes, `org.celestina.Grafita` → `/org/celestina/Grafita`.
#[must_use]
pub fn object_path(name: &ActivationName) -> String {
    format!("/{}", name.0.replace('.', "/"))
}

/// Why the bus could not settle a claim or a hand-off.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivationError {
    /// No session bus, or the bus failed the exchange.
    Bus(String),
    /// The owner did not answer within the timeout.
    Timeout,
    /// The owner, or the bus on its behalf, answered with an error.
    Refused(String),
}

impl fmt::Display for ActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(detail) => write!(formatter, "session bus unavailable: {detail}"),
            Self::Timeout => formatter.write_str("the running instance did not answer in time"),
            Self::Refused(detail) => write!(formatter, "the running instance refused: {detail}"),
        }
    }
}

impl std::error::Error for ActivationError {}

impl From<zbus::Error> for ActivationError {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::InputOutput(io) if io.kind() == std::io::ErrorKind::TimedOut => {
                Self::Timeout
            }
            zbus::Error::MethodError(..) | zbus::Error::FDO(_) => Self::Refused(error.to_string()),
            _ => Self::Bus(error.to_string()),
        }
    }
}

/// What the application side does with a request, on the Qt thread.
///
/// Both methods are called from the bus thread (or from [`Owner::attach`])
/// and must only *queue* the work; they never block.
pub trait Activatable: Send + 'static {
    /// Queue a raise of the window to the Qt thread.
    fn activate(&self);
    /// Queue "open these" to the Qt thread. Never empty: an `Open` whose keys
    /// all fail to decode arrives as [`Activatable::activate`].
    fn open(&self, paths: Vec<PathBuf>);
}

/// One request from another launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Activate,
    Open(Vec<PathBuf>),
}

/// The requests between the bus and the application: kept, bounded and in
/// order, until [`Inbox::attach`], then handed straight through.
pub struct Inbox {
    served: Box<dyn Activatable>,
    attached: bool,
    pending: VecDeque<Request>,
    dropped: usize,
}

impl Inbox {
    #[must_use]
    pub fn new(served: Box<dyn Activatable>) -> Self {
        Self {
            served,
            attached: false,
            pending: VecDeque::new(),
            dropped: 0,
        }
    }

    /// Delivers `request` now if attached, or keeps it, dropping the oldest
    /// kept request beyond [`INBOX_LIMIT`].
    pub fn deliver(&mut self, request: Request) {
        if self.attached {
            self.dispatch(request);
            return;
        }
        if self.pending.len() >= INBOX_LIMIT {
            self.pending.pop_front();
            self.dropped += 1;
        }
        self.pending.push_back(request);
    }

    /// Replays every kept request in arrival order and delivers later ones
    /// directly. Attaching twice replays nothing new.
    pub fn attach(&mut self) {
        self.attached = true;
        while let Some(request) = self.pending.pop_front() {
            self.dispatch(request);
        }
    }

    /// How many requests were dropped for want of room before attaching.
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    fn dispatch(&self, request: Request) {
        match request {
            Request::Activate => self.served.activate(),
            Request::Open(paths) => self.served.open(paths),
        }
    }
}

type SharedInbox = Arc<Mutex<Inbox>>;

/// A poisoned inbox still holds valid requests: a panic elsewhere must not
/// strand the hand-off.
fn lock(inbox: &Mutex<Inbox>) -> MutexGuard<'_, Inbox> {
    inbox.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a launch does with its arguments, given whether someone owns the name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandOff {
    /// Nobody owns the name: serve it and open the window.
    Serve,
    /// An owner exists and there is nothing to open: ask it to raise itself.
    Activate,
    /// An owner exists: hand it the paths.
    Open,
}

/// The one rule behind both [`claim`] and [`open_in`].
#[must_use]
pub const fn decide(argv_paths_empty: bool, owner_present: bool) -> HandOff {
    match (owner_present, argv_paths_empty) {
        (false, _) => HandOff::Serve,
        (true, true) => HandOff::Activate,
        (true, false) => HandOff::Open,
    }
}

/// How a launch goes on after [`claim`].
pub enum Claim {
    /// This process owns the name: build the window, keep the [`Owner`] for
    /// the process lifetime and [`Owner::attach`] it once Qt is ready.
    Owner(Owner),
    /// A running instance took this launch: exit 0 without a window.
    HandedOff,
    /// The bus could not settle it (said on stderr): open a window anyway.
    Unsettled(ActivationError),
}

/// The claimed name. Dropping it releases the name, so it lives as long as the
/// process does.
pub struct Owner {
    _connection: zbus::blocking::Connection,
    inbox: SharedInbox,
}

impl Owner {
    /// The Qt side is ready: replay what arrived since the claim and deliver
    /// every later request straight to the served [`Activatable`].
    pub fn attach(&self) {
        lock(&self.inbox).attach();
    }
}

/// The served object: it only feeds the inbox.
struct Served {
    inbox: SharedInbox,
}

#[zbus::interface(name = "org.celestina.Application1")]
impl Served {
    fn activate(&self) {
        lock(&self.inbox).deliver(Request::Activate);
    }

    fn open(&self, paths: Vec<String>) {
        let decoded: Vec<PathBuf> = paths
            .iter()
            .filter_map(|key| match pathkey::decode(key) {
                Ok(path) => Some(path),
                Err(error) => {
                    eprintln!(
                        "activation: ignoring an Open element that is not a path key: {error}"
                    );
                    None
                }
            })
            .collect();
        let request = if decoded.is_empty() {
            Request::Activate
        } else {
            Request::Open(decoded)
        };
        lock(&self.inbox).deliver(request);
    }
}

/// Claims `name` for this launch, before any window exists.
///
/// Owning it answers [`Claim::Owner`]. Finding it owned hands `argv_paths` to
/// the owner (`Open`), or only asks it to raise itself (`Activate`) when there
/// are none, and answers [`Claim::HandedOff`]. Any failure is said once on
/// stderr and answers [`Claim::Unsettled`].
#[must_use]
pub fn claim(name: ActivationName, served: Box<dyn Activatable>, argv_paths: &[PathBuf]) -> Claim {
    let inbox: SharedInbox = Arc::new(Mutex::new(Inbox::new(served)));
    let path = object_path(&name);
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| {
            builder.serve_at(
                path.as_str(),
                Served {
                    inbox: Arc::clone(&inbox),
                },
            )
        })
        .map(|builder| builder.method_timeout(HAND_OFF_TIMEOUT))
        .and_then(zbus::blocking::connection::Builder::build);
    let connection = match connection {
        Ok(connection) => connection,
        Err(error) => return unsettled(name, error.into()),
    };
    // `DoNotQueue`: without it the bus answers `InQueue`, and a second
    // instance would sit in the queue and inherit the name when the first
    // exits, stranding a process with no window. The owner can also exit
    // between the bus saying "taken" and the hand-off reaching it; the name is
    // then free, so the request is made once more rather than opening a
    // window that owns nothing and that the next launch could not find.
    let flags = zbus::fdo::RequestNameFlags::DoNotQueue;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let taken = match connection.request_name_with_flags(name.0, flags.into()) {
            Ok(
                zbus::fdo::RequestNameReply::PrimaryOwner
                | zbus::fdo::RequestNameReply::AlreadyOwner,
            ) => {
                return Claim::Owner(Owner {
                    _connection: connection,
                    inbox,
                })
            }
            Ok(_) | Err(zbus::Error::NameTaken) => call_owner(&connection, name, argv_paths),
            Err(error) => return unsettled(name, error.into()),
        };
        match after_hand_off(taken, attempt) {
            Step::HandedOff => return Claim::HandedOff,
            Step::Retry => {}
            Step::Unsettled(error) => return unsettled(name, error),
        }
    }
}

/// What a claim does after trying to hand off to the owner it found.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    HandedOff,
    /// The owner left meanwhile: request the name again.
    Retry,
    Unsettled(ActivationError),
}

/// How many times a claim requests the name when the owner keeps leaving.
const CLAIM_ATTEMPTS: u32 = 2;

fn after_hand_off(result: Result<(), ActivationError>, attempt: u32) -> Step {
    match result {
        Ok(()) => Step::HandedOff,
        Err(error) if owner_gone(&error) && attempt < CLAIM_ATTEMPTS => Step::Retry,
        Err(error) => Step::Unsettled(error),
    }
}

/// Whether a failed call means nobody owns the name any more.
fn owner_gone(error: &ActivationError) -> bool {
    matches!(error, ActivationError::Refused(detail)
        if detail.contains("org.freedesktop.DBus.Error.ServiceUnknown")
            || detail.contains("org.freedesktop.DBus.Error.NameHasNoOwner"))
}

/// Whether a failed call means the owner serves an older interface.
fn owner_predates(error: &ActivationError) -> bool {
    matches!(error, ActivationError::Refused(detail)
        if detail.contains("org.freedesktop.DBus.Error.UnknownMethod")
            || detail.contains("org.freedesktop.DBus.Error.UnknownInterface")
            || detail.contains("org.freedesktop.DBus.Error.UnknownObject"))
}

fn unsettled(name: ActivationName, error: ActivationError) -> Claim {
    if owner_predates(&error) {
        eprintln!(
            "{}: the running instance predates the shared interface; close it once. \
             Opening a window anyway",
            name.0
        );
    } else {
        eprintln!(
            "{}: cannot keep to one window, opening one anyway: {error}",
            name.0
        );
    }
    Claim::Unsettled(error)
}

/// [`claim`] as `main` uses it: exits the process (status 0) when a running
/// instance took this launch, and answers the [`Owner`] to keep, or `None`
/// when the bus could not settle it (already said on stderr) and the launch
/// carries on standalone.
#[must_use]
pub fn claim_or_exit(
    name: ActivationName,
    served: Box<dyn Activatable>,
    argv_paths: &[PathBuf],
) -> Option<Owner> {
    match claim(name, served, argv_paths) {
        Claim::Owner(owner) => Some(owner),
        Claim::HandedOff => std::process::exit(0),
        Claim::Unsettled(_) => None,
    }
}

/// Hands `paths` to whoever owns `name`, if anyone does.
///
/// `Ok(true)`: an owner took them (or, with no paths, raised itself).
/// `Ok(false)`: nobody owns the name, and the caller should start the program
/// itself, passing the paths. Blocks for at most about `timeout` per call.
pub fn open_in(
    name: ActivationName,
    paths: &[PathBuf],
    timeout: Duration,
) -> Result<bool, ActivationError> {
    let connection = zbus::blocking::connection::Builder::session()
        .map(|builder| builder.method_timeout(timeout))
        .and_then(zbus::blocking::connection::Builder::build)?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection)?;
    let bus_name = zbus::names::BusName::try_from(name.0)
        .map_err(|error| ActivationError::Bus(error.to_string()))?;
    let owner_present = bus.name_has_owner(bus_name).map_err(zbus::Error::from)?;
    if decide(paths.is_empty(), owner_present) == HandOff::Serve {
        return Ok(false);
    }
    match call_owner(&connection, name, paths) {
        Ok(()) => Ok(true),
        // The owner left between the question and the call.
        Err(error) if owner_gone(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Calls `Open` with `paths` as keys, or `Activate` when there are none, on
/// the owner of `name`.
fn call_owner(
    connection: &zbus::blocking::Connection,
    name: ActivationName,
    paths: &[PathBuf],
) -> Result<(), ActivationError> {
    let path = object_path(&name);
    let proxy = zbus::blocking::Proxy::new(connection, name.0, path.as_str(), INTERFACE)?;
    match decide(paths.is_empty(), true) {
        HandOff::Open => {
            let keys: Vec<String> = paths
                .iter()
                .map(|path| pathkey::encode(&absolute(path)))
                .collect();
            proxy.call::<_, _, ()>("Open", &(keys,))?;
        }
        HandOff::Activate | HandOff::Serve => proxy.call::<_, _, ()>("Activate", &())?,
    }
    Ok(())
}

/// The path made absolute against this process's working directory: the owner
/// has its own, and a relative path would resolve against the wrong place.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{after_hand_off, ActivationError, Step};

    fn gone() -> ActivationError {
        ActivationError::Refused(
            "org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable".to_owned(),
        )
    }

    #[test]
    fn an_owner_that_left_is_claimed_again_once() {
        assert_eq!(after_hand_off(Err(gone()), 1), Step::Retry);
        assert_eq!(after_hand_off(Err(gone()), 2), Step::Unsettled(gone()));
        assert_eq!(after_hand_off(Ok(()), 1), Step::HandedOff);
        assert_eq!(
            after_hand_off(Err(ActivationError::Timeout), 1),
            Step::Unsettled(ActivationError::Timeout)
        );
    }
}
