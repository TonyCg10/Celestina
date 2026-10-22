//! The Magnetita worker (CEL-2-J): the author's phone, read from the session
//! bus off the event loop's thread.
//!
//! Magnetita publishes `org.celestina.Devices1` at `/org/celestina/Devices1`
//! under the name `org.celestina.Magnetita`. `ListDevices` returns one
//! `a{sv}` per device (`id`, `name`, `type`, `connected`, `battery`,
//! `charging`, `mediaPlayer`, `mediaTitle`, `mediaArtist`, `mediaPlaying`,
//! `mediaCanPause`, `mediaCanNext`, `mediaCanPrevious`), the `Changed` signal
//! says the listing is stale, and `MediaAction(device_id, action)` and
//! `Ring(device_id)` act on a device.
//!
//! **Comparison with Siderita's reader** (`siderita/src/devices.rs`). Same
//! interface, same dictionary keys, same reading of `Changed` as "read again";
//! each client parses the dictionary itself. No code is shared on purpose: the
//! shared contract is the stable IPC interface, not a Rust type, and a shared
//! parser would couple a Qt sidebar and this shell to one release of each
//! other. The two readers differ where their consumers do. Siderita merges
//! `ListPaired` so offline phones stay listed, keeps one connection for its
//! life and lets a call activate Magnetita; this worker wants one connected
//! phone ([`celestina_shell::phone::reduce`]), reads only `ListDevices`,
//! never activates Magnetita (it asks the bus who owns the name first, as the
//! shell never starts a service, design D2), and treats the dictionary as
//! hostile (below). Magnetita's `MediaAction` verbs are the six of
//! `magnetita-core`'s `MediaAction`; [`MediaKind`] names exactly those.
//!
//! Three named threads, all stopped by one [`Worker`]:
//!
//! - **Reader** (`magnetita-signals`). Blocks on the bus connection's message
//!   stream and turns the two events that matter into a wake-up: Magnetita's
//!   `Changed` signal and a change of owner of its name
//!   (`NameOwnerChanged`). It ends when the connection is closed, which is how
//!   the session stops it.
//! - **Session** (`magnetita`). Connects (the session bus, or the address of
//!   [`MagnetitaConfig`]), installs the two match rules, reads, and then waits
//!   for wake-ups. A burst of wake-ups is *coalesced*: it waits until the bus
//!   has been quiet for [`COALESCE`] (at most [`MAX_COALESCE`]) and reads
//!   once. Only the session publishes, and only what it has just read: an
//!   event is dropped if the shutdown flag was raised meanwhile, and an event
//!   equal to the last one published is not repeated.
//! - **Actions** (`magnetita-actions`). [`MagnetitaHandle::request`] queues a
//!   [`PhoneRequest`] (at most [`ACTION_QUEUE`] wait); this thread makes the
//!   call, bounded by [`CALL_TIMEOUT`], so neither the loop nor the session
//!   waits for Magnetita's answer.
//!
//! **Absent is a state, not an error.** No owner of the name means
//! [`MagnetitaEvent::Absent`]; so does an unreachable bus or a failing call.
//! While the state is not healthy the session reads again after a backoff of
//! [`MIN_BACKOFF`] doubling to [`MAX_BACKOFF`], in [`POLL`] slices that look at
//! the shutdown flag; the owner change brings it back at once when the name
//! appears. Each change of state writes one journal line; a repeated attempt
//! writes none.
//!
//! **Hostile input.** The dictionaries are parsed key by key: a value of the
//! wrong type is the key's default, never an error; at most
//! [`phone::MAX_DEVICES`] rows are kept; text is cut to the bounds of
//! [`celestina_shell::phone`] before it is stored and a device id longer than
//! [`MAX_ID_UNITS`] makes the row unusable (a truncated id would name another
//! device). **Actions out** are sent only for an id that the last listing
//! contained, checked when the request is queued and again when it is made.

use std::collections::{BTreeSet, HashMap};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::bounded::bounded;
use celestina_shell::phone::{
    self, DeviceRow, PhoneState, MAX_DEVICES, MAX_MEDIA_UNITS, MAX_NAME_UNITS, MAX_PLAYER_UNITS,
};
use zbus::blocking::connection::Builder;
use zbus::blocking::{fdo::DBusProxy, Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

use super::Worker;

/// The well-known name Magnetita owns on the session bus.
pub const SERVICE: &str = "org.celestina.Magnetita";
/// The object the interface is served at.
pub const OBJECT_PATH: &str = "/org/celestina/Devices1";
/// The stable interface of the contract.
pub const INTERFACE: &str = "org.celestina.Devices1";
/// The signal that says the listing is stale.
pub const CHANGED_SIGNAL: &str = "Changed";
/// The first wait before reading or connecting again.
pub const MIN_BACKOFF: Duration = Duration::from_millis(500);
/// The longest wait before reading or connecting again.
pub const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// How often a blocked wait looks at the shutdown flag.
pub const POLL: Duration = Duration::from_millis(100);
/// The bus must be quiet this long before a burst of wake-ups is read.
pub const COALESCE: Duration = Duration::from_millis(50);
/// A burst that never goes quiet is read after this long anyway.
pub const MAX_COALESCE: Duration = Duration::from_millis(500);
/// The longest one call to Magnetita may take. It bounds the session's read
/// and an action; it is also the worst case the shutdown waits for.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(2);
/// The actions that may wait for the action thread. They are deliberate
/// gestures; past this a binding is repeating itself.
pub const ACTION_QUEUE: usize = 8;
/// The most UTF-16 units of a reason that reaches the journal.
pub const MAX_REASON: usize = 240;
/// The longest device id kept, in UTF-16 code units.
pub const MAX_ID_UNITS: usize = 128;
/// The longest device type kept, in UTF-16 code units.
const MAX_KIND_UNITS: usize = 32;

/// Where the bus is. [`MagnetitaConfig::session`] in production; tests name a
/// private bus.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MagnetitaConfig {
    /// A bus address (`unix:path=...`), or `None` for the session bus the
    /// environment names.
    pub address: Option<String>,
}

impl MagnetitaConfig {
    /// The session bus.
    #[must_use]
    pub fn session() -> Self {
        Self::default()
    }

    /// The bus at exactly `address`.
    #[must_use]
    pub fn at(address: impl Into<String>) -> Self {
        Self {
            address: Some(address.into()),
        }
    }
}

/// What the worker tells the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MagnetitaEvent {
    /// The phone as the last listing shows it ([`PhoneState::Absent`] when no
    /// device is connected).
    State(PhoneState),
    /// Magnetita cannot be asked: nobody owns its name, the bus is out of
    /// reach or the listing failed. The piece draws nothing.
    Absent,
}

/// The transport verbs `Devices1.MediaAction` accepts, and no others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

impl MediaKind {
    /// The spelling on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Pause => "Pause",
            Self::PlayPause => "PlayPause",
            Self::Stop => "Stop",
            Self::Next => "Next",
            Self::Previous => "Previous",
        }
    }
}

/// What the loop asks of the phone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhoneRequest {
    /// Drive the device's media player.
    MediaAction {
        device_id: String,
        action: MediaKind,
    },
    /// Make the device ring.
    Ring { device_id: String },
}

impl PhoneRequest {
    /// The device the request is for.
    #[must_use]
    pub fn device_id(&self) -> &str {
        match self {
            Self::MediaAction { device_id, .. } | Self::Ring { device_id } => device_id,
        }
    }

    const fn method(&self) -> &'static str {
        match self {
            Self::MediaAction { .. } => "MediaAction",
            Self::Ring { .. } => "Ring",
        }
    }
}

/// Why a request was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("the device is not in the last listing")]
    UnknownDevice,
    #[error("the Magnetita action queue is full")]
    Full,
    #[error("the Magnetita worker has stopped")]
    Stopped,
}

/// The way into the worker: queues a request, never waits for it.
#[derive(Clone, Debug)]
pub struct MagnetitaHandle {
    actions: SyncSender<PhoneRequest>,
    known: Arc<Mutex<BTreeSet<String>>>,
}

impl MagnetitaHandle {
    /// Queues `request`.
    ///
    /// # Errors
    ///
    /// [`RequestError::UnknownDevice`] when the last listing did not contain
    /// the device, [`RequestError::Full`] when [`ACTION_QUEUE`] requests wait
    /// already, [`RequestError::Stopped`] when the worker has stopped.
    pub fn request(&self, request: PhoneRequest) -> Result<(), RequestError> {
        if !lock(&self.known).contains(request.device_id()) {
            return Err(RequestError::UnknownDevice);
        }
        self.actions.try_send(request).map_err(|err| match err {
            TrySendError::Full(_) => RequestError::Full,
            TrySendError::Disconnected(_) => RequestError::Stopped,
        })
    }
}

/// Starts the worker on the bus `config` names, publishing through
/// `publish`, which returns `false` once nobody listens (the worker then
/// stops). It also stops when `shutdown` is raised; the returned [`Worker`]
/// joins it.
///
/// # Errors
///
/// The failure to start either thread.
pub fn spawn(
    config: &MagnetitaConfig,
    publish: impl FnMut(MagnetitaEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(MagnetitaHandle, Worker)> {
    let known = Arc::new(Mutex::new(BTreeSet::new()));
    let shared = Arc::new(Shared {
        shutdown: Arc::clone(shutdown),
        stopped: AtomicBool::new(false),
        connection: Mutex::new(None),
        known: Arc::clone(&known),
    });
    let (actions, queue) = mpsc::sync_channel(ACTION_QUEUE);
    let handle = MagnetitaHandle { actions, known };

    let action_shared = Arc::clone(&shared);
    let action_worker = Worker::spawn("magnetita-actions", shutdown, move || {
        run_actions(&action_shared, &queue);
    })?;
    let address = config.address.clone();
    let worker = Worker::spawn("magnetita", shutdown, move || {
        run_session(&shared, address.as_deref(), Box::new(publish));
        shared.stopped.store(true, Ordering::Release);
        // The session ends on shutdown or when nobody listens; the action
        // thread is stopped with it.
        action_worker.join();
    })?;
    Ok((handle, worker))
}

type Publish = Box<dyn FnMut(MagnetitaEvent) -> bool + Send>;

/// What the threads share.
struct Shared {
    shutdown: Arc<AtomicBool>,
    /// Raised when the session ends for any reason.
    stopped: AtomicBool,
    /// The live connection, for the action thread.
    connection: Mutex<Option<Connection>>,
    /// The ids of the last listing.
    known: Arc<Mutex<BTreeSet<String>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl Shared {
    fn running(&self) -> bool {
        !self.shutdown.load(Ordering::Acquire) && !self.stopped.load(Ordering::Acquire)
    }

    /// Waits `delay` in [`POLL`] slices.
    fn pause(&self, delay: Duration) {
        let until = Instant::now() + delay;
        while self.running() {
            let now = Instant::now();
            if now >= until {
                break;
            }
            thread::sleep(POLL.min(until - now));
        }
    }
}

/// A doubling wait between [`MIN_BACKOFF`] and [`MAX_BACKOFF`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Backoff(Duration);

impl Backoff {
    const fn new() -> Self {
        Self(MIN_BACKOFF)
    }

    /// The wait to use now; the next one is twice as long, up to the bound.
    fn step(&mut self) -> Duration {
        let now = self.0;
        self.0 = (now * 2).min(MAX_BACKOFF);
        now
    }
}

/// How the link to Magnetita stands, for the journal.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Link {
    /// The bus could not be used.
    Bus(String),
    /// The bus is fine and nobody owns the name.
    NameAbsent,
    /// Somebody owns the name, but the listing failed.
    Failing(String),
    /// The listing was read.
    Up,
}

/// Writes one journal line per change of [`Link`] and publishes an event
/// only when it differs from the last one.
struct Reporter {
    publish: Publish,
    link: Option<Link>,
    last: Option<MagnetitaEvent>,
    listening: bool,
}

impl Reporter {
    fn new(publish: Publish) -> Self {
        Self {
            publish,
            link: None,
            last: None,
            listening: true,
        }
    }

    fn link(&mut self, link: Link) {
        if self.link.as_ref() == Some(&link) {
            return;
        }
        match &link {
            Link::Bus(why) => tracing::info!("magnetita: no bus: {why}"),
            Link::NameAbsent => tracing::info!("magnetita: absent: nobody owns {SERVICE}"),
            Link::Failing(why) => tracing::warn!("magnetita: listing failed: {why}"),
            Link::Up => tracing::info!("magnetita: listing"),
        }
        self.link = Some(link);
    }

    fn emit(&mut self, event: MagnetitaEvent) {
        if !self.listening || self.last.as_ref() == Some(&event) {
            return;
        }
        self.last = Some(event.clone());
        if !(self.publish)(event) {
            self.listening = false;
        }
    }
}

fn reason(err: &dyn std::error::Error) -> String {
    bounded(&crate::error_chain(err), MAX_REASON)
}

/// Why a bus session ended.
#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("cannot connect to the bus")]
    Connect(#[source] zbus::Error),
    #[error("cannot watch Magnetita's signals")]
    Watch(#[source] zbus::Error),
    #[error("cannot start the signal reader")]
    Reader(#[source] io::Error),
    #[error("the bus connection ended")]
    Lost,
}

/// Why one reading failed.
#[derive(Debug, thiserror::Error)]
enum ReadError {
    #[error("cannot ask the bus who owns {SERVICE}")]
    Owner(#[source] zbus::Error),
    #[error("ListDevices failed")]
    Call(#[source] zbus::Error),
    #[error("ListDevices answered with an unexpected body")]
    Body(#[source] zbus::Error),
}

/// What one reading found.
enum Reading {
    NameAbsent,
    Devices(Vec<DeviceRow>),
}

fn run_session(shared: &Shared, address: Option<&str>, publish: Publish) {
    let mut report = Reporter::new(publish);
    let mut backoff = Backoff::new();
    while shared.running() && report.listening {
        if let Err(err) = session(shared, address, &mut report, &mut backoff) {
            report.link(Link::Bus(reason(&err)));
            report.emit(MagnetitaEvent::Absent);
        }
        if !shared.running() || !report.listening {
            break;
        }
        shared.pause(backoff.step());
    }
}

fn connect(address: Option<&str>) -> Result<Connection, SessionError> {
    let builder = match address {
        Some(address) => Builder::address(address),
        None => Builder::session(),
    }
    .map_err(SessionError::Connect)?;
    builder
        .method_timeout(CALL_TIMEOUT)
        .build()
        .map_err(SessionError::Connect)
}

/// The two rules whose messages wake the reader.
fn match_rules() -> zbus::Result<[MatchRule<'static>; 2]> {
    let changed = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(SERVICE)?
        .path(OBJECT_PATH)?
        .interface(INTERFACE)?
        .member(CHANGED_SIGNAL)?
        .build();
    let owner = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg(0, SERVICE)?
        .build();
    Ok([changed, owner])
}

/// Whether `message` is one of the two events that make the listing stale.
fn wakes(message: &zbus::Message) -> bool {
    let header = message.header();
    if header.message_type() != MessageType::Signal {
        return false;
    }
    let interface = header.interface().map(|name| name.as_str());
    let member = header.member().map(|name| name.as_str());
    match (interface, member) {
        (Some(INTERFACE), Some(CHANGED_SIGNAL)) => true,
        (Some("org.freedesktop.DBus"), Some("NameOwnerChanged")) => message
            .body()
            .deserialize::<(String, String, String)>()
            .is_ok_and(|(name, _, _)| name == SERVICE),
        _ => false,
    }
}

/// Turns the bus's messages into wake-ups until the connection closes. A
/// wake-up already pending covers the next, so a full channel is not an
/// error.
fn reader(messages: MessageIterator, wake: &SyncSender<()>) {
    for message in messages {
        match message {
            Ok(message) if wakes(&message) => {
                if let Err(TrySendError::Disconnected(())) = wake.try_send(()) {
                    return;
                }
            }
            Ok(_) => {}
            Err(_) => return,
        }
    }
}

fn session(
    shared: &Shared,
    address: Option<&str>,
    report: &mut Reporter,
    backoff: &mut Backoff,
) -> Result<(), SessionError> {
    let connection = connect(address)?;
    // The stream exists before the rules do, so no wake-up is missed between
    // them; the first reading happens after both.
    let messages = MessageIterator::from(&connection);
    let dbus = DBusProxy::new(&connection).map_err(SessionError::Watch)?;
    for rule in match_rules().map_err(SessionError::Watch)? {
        dbus.add_match_rule(rule)
            .map_err(|err| SessionError::Watch(err.into()))?;
    }
    let (wake, woken) = mpsc::sync_channel(1);
    let signals = thread::Builder::new()
        .name("magnetita-signals".to_owned())
        .spawn(move || reader(messages, &wake))
        .map_err(SessionError::Reader)?;
    *backoff = Backoff::new();
    *lock(&shared.connection) = Some(connection.clone());

    let ended = serve(shared, &connection, &dbus, &woken, report);

    *lock(&shared.connection) = None;
    lock(&shared.known).clear();
    // Closing the connection ends the reader's stream. A graceful shutdown
    // would wait for the reader's own reference to it, which is the thing
    // being stopped.
    if let Err(err) = connection.close() {
        tracing::debug!("magnetita: closing the bus connection: {}", reason(&err));
    }
    if signals.join().is_err() {
        tracing::error!("the magnetita signal reader panicked");
    }
    ended
}

/// Reads, then reads again on every coalesced wake-up and, while the state is
/// not healthy, after each backoff. `Ok` only when the shell is stopping.
fn serve(
    shared: &Shared,
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    woken: &Receiver<()>,
    report: &mut Reporter,
) -> Result<(), SessionError> {
    let mut retry = Backoff::new();
    let mut due: Option<Instant> = None;
    let mut read_now = true;
    while shared.running() && report.listening {
        if read_now {
            let healthy = read_and_report(shared, connection, dbus, report);
            due = if healthy {
                retry = Backoff::new();
                None
            } else {
                Some(Instant::now() + retry.step())
            };
            read_now = false;
            continue;
        }
        match woken.recv_timeout(POLL) {
            Ok(()) => {
                settle(woken);
                read_now = true;
            }
            Err(RecvTimeoutError::Timeout) => read_now = due.is_some_and(|at| at <= Instant::now()),
            Err(RecvTimeoutError::Disconnected) => return Err(SessionError::Lost),
        }
    }
    Ok(())
}

/// Swallows the wake-ups of a burst: returns once the bus has been quiet for
/// [`COALESCE`], or after [`MAX_COALESCE`] of noise.
fn settle(woken: &Receiver<()>) {
    let deadline = Instant::now() + MAX_COALESCE;
    while woken.recv_timeout(COALESCE).is_ok() {
        if Instant::now() >= deadline {
            break;
        }
    }
}

/// One reading and its report. Returns whether the state is healthy (the
/// listing was read), which stops the retry timer.
fn read_and_report(
    shared: &Shared,
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    report: &mut Reporter,
) -> bool {
    let reading = read(connection, dbus);
    // Only what is current is published: a shutdown raised during the call
    // drops its answer.
    if !shared.running() {
        return true;
    }
    match reading {
        Ok(Reading::Devices(rows)) => {
            *lock(&shared.known) = rows
                .iter()
                .map(|row| row.id.clone())
                .filter(|id| !id.is_empty())
                .collect();
            report.link(Link::Up);
            report.emit(MagnetitaEvent::State(phone::reduce(&rows)));
            true
        }
        Ok(Reading::NameAbsent) => {
            lock(&shared.known).clear();
            report.link(Link::NameAbsent);
            report.emit(MagnetitaEvent::Absent);
            false
        }
        Err(err) => {
            lock(&shared.known).clear();
            report.link(Link::Failing(reason(&err)));
            report.emit(MagnetitaEvent::Absent);
            false
        }
    }
}

/// Asks the bus whether anybody owns the name, so a missing Magnetita is not
/// activated by the call, and then lists.
fn read(connection: &Connection, dbus: &DBusProxy<'_>) -> Result<Reading, ReadError> {
    let name =
        zbus::names::BusName::try_from(SERVICE).map_err(|err| ReadError::Owner(err.into()))?;
    let owned = dbus
        .name_has_owner(name)
        .map_err(|err| ReadError::Owner(err.into()))?;
    if !owned {
        return Ok(Reading::NameAbsent);
    }
    let reply = connection
        .call_method(
            Some(SERVICE),
            OBJECT_PATH,
            Some(INTERFACE),
            "ListDevices",
            &(),
        )
        .map_err(ReadError::Call)?;
    let listing: Vec<HashMap<String, OwnedValue>> =
        reply.body().deserialize().map_err(ReadError::Body)?;
    Ok(Reading::Devices(parse_listing(&listing)))
}

/// The rows of a listing: at most [`MAX_DEVICES`], each usable one parsed.
#[must_use]
pub fn parse_listing(listing: &[HashMap<String, OwnedValue>]) -> Vec<DeviceRow> {
    listing
        .iter()
        .take(MAX_DEVICES)
        .filter_map(parse_device)
        .collect()
}

/// One device dictionary, read as hostile: a value of the wrong type is the
/// key's default, text is bounded, and a row without a usable id (empty or
/// longer than [`MAX_ID_UNITS`]) is no row.
#[must_use]
pub fn parse_device(dict: &HashMap<String, OwnedValue>) -> Option<DeviceRow> {
    let id = text(dict, "id", usize::MAX);
    if id.is_empty() || id.encode_utf16().count() > MAX_ID_UNITS {
        return None;
    }
    Some(DeviceRow {
        id,
        name: text(dict, "name", MAX_NAME_UNITS),
        kind: text(dict, "type", MAX_KIND_UNITS),
        connected: flag(dict, "connected"),
        battery: dict
            .get("battery")
            .and_then(|value| value.downcast_ref::<i32>().ok())
            .unwrap_or(-1),
        charging: flag(dict, "charging"),
        media_player: text(dict, "mediaPlayer", MAX_PLAYER_UNITS),
        media_title: text(dict, "mediaTitle", MAX_MEDIA_UNITS),
        media_artist: text(dict, "mediaArtist", MAX_MEDIA_UNITS),
        media_playing: flag(dict, "mediaPlaying"),
        media_can_pause: flag(dict, "mediaCanPause"),
        media_can_next: flag(dict, "mediaCanNext"),
        media_can_previous: flag(dict, "mediaCanPrevious"),
    })
}

/// A string value cut to `limit` UTF-16 units (`usize::MAX` keeps it whole),
/// or empty when the key is missing or of another type.
fn text(dict: &HashMap<String, OwnedValue>, key: &str, limit: usize) -> String {
    dict.get(key)
        .and_then(|value| value.downcast_ref::<&str>().ok())
        .map(|value| {
            if limit == usize::MAX {
                value.to_owned()
            } else {
                bounded(value, limit)
            }
        })
        .unwrap_or_default()
}

fn flag(dict: &HashMap<String, OwnedValue>, key: &str) -> bool {
    dict.get(key)
        .and_then(|value| value.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

fn run_actions(shared: &Shared, queue: &Receiver<PhoneRequest>) {
    while shared.running() {
        match queue.recv_timeout(POLL) {
            Ok(request) => perform(shared, &request),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Makes the call for `request` if its device is still in the last listing
/// and there is a connection. A failure is journalled, bounded, and nothing
/// else: the next `Changed` shows whatever really happened.
fn perform(shared: &Shared, request: &PhoneRequest) {
    if !lock(&shared.known).contains(request.device_id()) {
        tracing::info!(
            "magnetita: {} dropped: the device left the listing",
            request.method()
        );
        return;
    }
    let Some(connection) = lock(&shared.connection).clone() else {
        tracing::info!("magnetita: {} dropped: no connection", request.method());
        return;
    };
    let sent = match request {
        PhoneRequest::MediaAction { device_id, action } => connection.call_method(
            Some(SERVICE),
            OBJECT_PATH,
            Some(INTERFACE),
            "MediaAction",
            &(device_id.as_str(), action.as_str()),
        ),
        PhoneRequest::Ring { device_id } => connection.call_method(
            Some(SERVICE),
            OBJECT_PATH,
            Some(INTERFACE),
            "Ring",
            &(device_id.as_str(),),
        ),
    };
    if let Err(err) = sent {
        tracing::warn!("magnetita: {} failed: {}", request.method(), reason(&err));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use celestina_shell::phone::Phone;
    use zbus::zvariant::Value;

    use super::*;
    use crate::tray::test_bus::PrivateBus;

    /// Long enough for a bus round trip plus a coalescing window, still
    /// bounded.
    const DEADLINE: Duration = Duration::from_secs(5);

    fn value(raw: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(raw.into()).expect("a basic value converts")
    }

    fn phone_dict(id: &str, battery: i32) -> HashMap<String, OwnedValue> {
        let mut dict = HashMap::new();
        dict.insert("id".to_owned(), value(id.to_owned()));
        dict.insert("name".to_owned(), value(format!("name-{id}")));
        dict.insert("type".to_owned(), value("phone".to_owned()));
        dict.insert("connected".to_owned(), value(true));
        dict.insert("battery".to_owned(), value(battery));
        dict.insert("charging".to_owned(), value(false));
        dict
    }

    // ---- parsing -------------------------------------------------------

    #[test]
    fn a_well_formed_dictionary_is_read_key_by_key() {
        let mut dict = phone_dict("p1", 64);
        dict.insert("charging".to_owned(), value(true));
        dict.insert("mediaPlayer".to_owned(), value("Player".to_owned()));
        dict.insert("mediaTitle".to_owned(), value("Title".to_owned()));
        dict.insert("mediaArtist".to_owned(), value("Artist".to_owned()));
        dict.insert("mediaPlaying".to_owned(), value(true));
        dict.insert("mediaCanPause".to_owned(), value(true));
        dict.insert("mediaCanNext".to_owned(), value(true));
        let row = parse_device(&dict).expect("a usable row");
        assert_eq!(row.id, "p1");
        assert_eq!(row.kind, "phone");
        assert!(row.connected && row.charging && row.media_playing);
        assert_eq!(row.battery, 64);
        assert_eq!(row.media_player, "Player");
        assert!(row.media_can_pause && row.media_can_next && !row.media_can_previous);
    }

    #[test]
    fn values_of_the_wrong_type_are_the_default_of_their_key() {
        let mut dict = phone_dict("p1", 50);
        dict.insert("name".to_owned(), value(7_i32));
        dict.insert("type".to_owned(), value(true));
        dict.insert("connected".to_owned(), value("yes".to_owned()));
        dict.insert("battery".to_owned(), value("high".to_owned()));
        dict.insert("charging".to_owned(), value(1_u32));
        dict.insert("mediaPlayer".to_owned(), value(3.5_f64));
        dict.insert("mediaPlaying".to_owned(), value(1_i32));
        let row = parse_device(&dict).expect("the id is fine");
        assert_eq!(row.name, "");
        assert_eq!(row.kind, "");
        assert!(!row.connected && !row.charging && !row.media_playing);
        assert_eq!(row.battery, -1);
        assert_eq!(row.media_player, "");
    }

    #[test]
    fn a_row_without_a_usable_id_is_no_row() {
        let mut dict = phone_dict("p1", 50);
        dict.insert("id".to_owned(), value(9_i32));
        assert_eq!(parse_device(&dict), None);
        dict.insert("id".to_owned(), value(String::new()));
        assert_eq!(parse_device(&dict), None);
        dict.insert("id".to_owned(), value("x".repeat(MAX_ID_UNITS + 1)));
        assert_eq!(parse_device(&dict), None);
        dict.insert("id".to_owned(), value("x".repeat(MAX_ID_UNITS)));
        assert!(parse_device(&dict).is_some());
        assert_eq!(parse_device(&HashMap::new()), None);
    }

    #[test]
    fn long_text_is_bounded_when_it_is_parsed() {
        let mut dict = phone_dict("p1", 50);
        dict.insert("name".to_owned(), value("n".repeat(100_000)));
        dict.insert("mediaTitle".to_owned(), value("\u{1f600}".repeat(50_000)));
        let row = parse_device(&dict).expect("a row");
        assert!(row.name.encode_utf16().count() <= MAX_NAME_UNITS);
        assert!(row.media_title.encode_utf16().count() <= MAX_MEDIA_UNITS);
    }

    #[test]
    fn a_listing_keeps_at_most_the_bound_in_the_order_it_came() {
        let listing: Vec<_> = (0..MAX_DEVICES + 20)
            .map(|i| phone_dict(&i.to_string(), 10))
            .collect();
        let rows = parse_listing(&listing);
        assert_eq!(rows.len(), MAX_DEVICES);
        assert_eq!(rows[0].id, "0");
        assert_eq!(rows[MAX_DEVICES - 1].id, (MAX_DEVICES - 1).to_string());
    }

    #[test]
    fn media_kinds_are_exactly_the_six_wire_verbs() {
        let verbs = [
            (MediaKind::Play, "Play"),
            (MediaKind::Pause, "Pause"),
            (MediaKind::PlayPause, "PlayPause"),
            (MediaKind::Stop, "Stop"),
            (MediaKind::Next, "Next"),
            (MediaKind::Previous, "Previous"),
        ];
        for (kind, wire) in verbs {
            assert_eq!(kind.as_str(), wire);
        }
    }

    #[test]
    fn the_backoff_doubles_from_half_a_second_to_thirty() {
        let mut backoff = Backoff::new();
        let waits: Vec<_> = (0..9).map(|_| backoff.step()).collect();
        assert_eq!(waits[0], Duration::from_millis(500));
        assert_eq!(waits[1], Duration::from_secs(1));
        assert_eq!(waits[2], Duration::from_secs(2));
        assert_eq!(waits[6], Duration::from_secs(30));
        assert_eq!(waits[8], MAX_BACKOFF);
    }

    // ---- the worker on a private bus -----------------------------------

    #[derive(Default)]
    struct FakeState {
        devices: Vec<HashMap<String, OwnedValue>>,
        listings: usize,
        media: Vec<(String, String)>,
        rings: Vec<String>,
    }

    /// A Magnetita that is only the contract.
    struct Fake(Arc<Mutex<FakeState>>);

    #[zbus::interface(name = "org.celestina.Devices1")]
    impl Fake {
        fn list_devices(&self) -> Vec<HashMap<String, OwnedValue>> {
            let mut state = lock(&self.0);
            state.listings += 1;
            state.devices.clone()
        }

        fn media_action(&self, device_id: String, action: String) {
            lock(&self.0).media.push((device_id, action));
        }

        fn ring(&self, device_id: String) {
            lock(&self.0).rings.push(device_id);
        }
    }

    struct Service {
        connection: Connection,
        state: Arc<Mutex<FakeState>>,
    }

    impl Service {
        /// Serves the object and claims the name.
        fn start(bus: &PrivateBus, devices: Vec<HashMap<String, OwnedValue>>) -> Self {
            let state = Arc::new(Mutex::new(FakeState {
                devices,
                ..FakeState::default()
            }));
            let connection = bus
                .builder()
                .serve_at(OBJECT_PATH, Fake(Arc::clone(&state)))
                .and_then(|builder| builder.name(SERVICE))
                .and_then(Builder::build)
                .expect("the fake Magnetita starts");
            Self { connection, state }
        }

        fn set(&self, devices: Vec<HashMap<String, OwnedValue>>) {
            lock(&self.state).devices = devices;
        }

        fn changed(&self) {
            self.connection
                .emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, CHANGED_SIGNAL, &())
                .expect("the signal is emitted");
        }

        fn listings(&self) -> usize {
            lock(&self.state).listings
        }

        fn disappear(&self) {
            self.connection
                .release_name(SERVICE)
                .expect("the name is released");
        }

        fn reappear(&self) {
            self.connection
                .request_name(SERVICE)
                .expect("the name is claimed again");
        }
    }

    struct Harness {
        events: Receiver<MagnetitaEvent>,
        handle: MagnetitaHandle,
        worker: Worker,
        shutdown: Arc<AtomicBool>,
    }

    impl Harness {
        fn start(address: &str) -> Self {
            let (sender, events) = mpsc::channel();
            let shutdown = Arc::new(AtomicBool::new(false));
            let (handle, worker) = spawn(
                &MagnetitaConfig::at(address),
                move |event| sender.send(event).is_ok(),
                &shutdown,
            )
            .expect("the worker starts");
            Self {
                events,
                handle,
                worker,
                shutdown,
            }
        }

        fn next(&self) -> MagnetitaEvent {
            self.events
                .recv_timeout(DEADLINE)
                .expect("an event arrives in time")
        }

        /// Waits for an event that satisfies `wanted`, dropping the earlier
        /// ones.
        fn until(&self, wanted: impl Fn(&MagnetitaEvent) -> bool) -> MagnetitaEvent {
            let deadline = Instant::now() + DEADLINE;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.events.recv_timeout(left) {
                    Ok(event) if wanted(&event) => return event,
                    Ok(_) => {}
                    Err(_) => panic!("the wanted event never arrived"),
                }
            }
        }

        fn stops_within(self, wait: Duration) -> bool {
            self.worker.join_within(wait)
        }
    }

    fn present(event: &MagnetitaEvent) -> Option<&Phone> {
        match event {
            MagnetitaEvent::State(PhoneState::Present(phone)) => Some(phone),
            _ => None,
        }
    }

    fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + DEADLINE;
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("{what} never held");
    }

    macro_rules! bus_or_skip {
        ($name:expr) => {
            match PrivateBus::start($name) {
                Some(bus) => bus,
                None => return,
            }
        };
    }

    #[test]
    fn absent_then_present_then_changed_then_absent_again() {
        let bus = bus_or_skip!("magnetita-lifecycle");
        let harness = Harness::start(bus.address());
        assert_eq!(harness.next(), MagnetitaEvent::Absent);

        let service = Service::start(&bus, vec![phone_dict("p1", 80)]);
        let event = harness.until(|event| present(event).is_some());
        let phone = present(&event).expect("present");
        assert_eq!((phone.id.as_str(), phone.battery), ("p1", Some(80)));

        service.set(vec![phone_dict("p1", 79)]);
        service.changed();
        let event = harness.until(|event| present(event).is_some_and(|p| p.battery == Some(79)));
        assert!(present(&event).is_some());

        service.disappear();
        assert_eq!(
            harness.until(|e| *e == MagnetitaEvent::Absent),
            MagnetitaEvent::Absent
        );

        // It comes back on the owner change alone, with no signal.
        service.set(vec![phone_dict("p2", 10)]);
        service.reappear();
        let event = harness.until(|event| present(event).is_some());
        assert_eq!(present(&event).map(|p| p.id.as_str()), Some("p2"));
        assert!(harness.stops_within(Duration::from_secs(3)));
    }

    #[test]
    fn a_service_with_no_connected_device_is_a_present_service_without_a_phone() {
        let bus = bus_or_skip!("magnetita-nophone");
        let mut off = phone_dict("p1", 50);
        off.insert("connected".to_owned(), value(false));
        let _service = Service::start(&bus, vec![off]);
        let harness = Harness::start(bus.address());
        assert_eq!(harness.next(), MagnetitaEvent::State(PhoneState::Absent));
        assert!(harness.stops_within(Duration::from_secs(3)));
    }

    #[test]
    fn a_burst_of_signals_is_read_once() {
        let bus = bus_or_skip!("magnetita-burst");
        let service = Service::start(&bus, vec![phone_dict("p1", 90)]);
        let harness = Harness::start(bus.address());
        harness.until(|event| present(event).is_some());
        let before = service.listings();

        service.set(vec![phone_dict("p1", 11)]);
        for _ in 0..40 {
            service.changed();
        }
        harness.until(|event| present(event).is_some_and(|p| p.battery == Some(11)));
        // Let any straggling read show itself.
        thread::sleep(MAX_COALESCE + Duration::from_millis(300));

        let reads = service.listings() - before;
        assert!(
            (1..=2).contains(&reads),
            "40 signals caused {reads} readings"
        );
        assert!(harness.stops_within(Duration::from_secs(3)));
    }

    #[test]
    fn requests_go_out_only_for_a_listed_device() {
        let bus = bus_or_skip!("magnetita-requests");
        let service = Service::start(&bus, vec![phone_dict("p1", 90)]);
        let harness = Harness::start(bus.address());
        harness.until(|event| present(event).is_some());

        let ghost = PhoneRequest::Ring {
            device_id: "ghost".to_owned(),
        };
        assert_eq!(
            harness.handle.request(ghost),
            Err(RequestError::UnknownDevice)
        );
        let ghost = PhoneRequest::MediaAction {
            device_id: "ghost".to_owned(),
            action: MediaKind::Next,
        };
        assert_eq!(
            harness.handle.request(ghost),
            Err(RequestError::UnknownDevice)
        );

        let media = PhoneRequest::MediaAction {
            device_id: "p1".to_owned(),
            action: MediaKind::PlayPause,
        };
        assert_eq!(harness.handle.request(media), Ok(()));
        let ring = PhoneRequest::Ring {
            device_id: "p1".to_owned(),
        };
        assert_eq!(harness.handle.request(ring), Ok(()));
        eventually("both calls reach the service", || {
            let state = lock(&service.state);
            state.media == [("p1".to_owned(), "PlayPause".to_owned())]
                && state.rings == ["p1".to_owned()]
        });

        // The device leaves: its requests are refused from then on.
        service.set(Vec::new());
        service.changed();
        harness.until(|event| *event == MagnetitaEvent::State(PhoneState::Absent));
        let ring = PhoneRequest::Ring {
            device_id: "p1".to_owned(),
        };
        assert_eq!(
            harness.handle.request(ring),
            Err(RequestError::UnknownDevice)
        );
        assert_eq!(lock(&service.state).rings.len(), 1);

        let handle = harness.handle.clone();
        assert!(harness.stops_within(Duration::from_secs(3)));
        let ring = PhoneRequest::Ring {
            device_id: "p1".to_owned(),
        };
        assert!(handle.request(ring).is_err());
    }

    #[test]
    fn a_hostile_service_cannot_break_the_reading() {
        let bus = bus_or_skip!("magnetita-hostile");
        // 70 junk rows (wrong types everywhere, none connected) and then a
        // real phone beyond the bound of 64: it must not be seen.
        let junk = |i: usize| {
            let mut dict = HashMap::new();
            dict.insert("id".to_owned(), value(format!("junk{i}")));
            dict.insert("connected".to_owned(), value("yes".to_owned()));
            dict.insert("battery".to_owned(), value(u8::MAX));
            dict.insert("name".to_owned(), value(i32::MAX));
            dict
        };
        let mut listing: Vec<_> = (0..70).map(junk).collect();
        listing.push(phone_dict("late", 50));
        let service = Service::start(&bus, listing);
        let harness = Harness::start(bus.address());
        assert_eq!(harness.next(), MagnetitaEvent::State(PhoneState::Absent));

        // Now a connected phone first, with every other key hostile.
        let mut hostile = phone_dict("p1", 500);
        hostile.insert("name".to_owned(), value("n".repeat(200_000)));
        hostile.insert("mediaPlayer".to_owned(), value("p".repeat(200_000)));
        hostile.insert("mediaTitle".to_owned(), value(42_i64));
        hostile.insert("mediaCanNext".to_owned(), value("true".to_owned()));
        service.set(vec![hostile]);
        service.changed();
        let event = harness.until(|event| present(event).is_some());
        let phone = present(&event).expect("present");
        assert_eq!(phone.battery, None);
        assert!(phone.name.encode_utf16().count() <= MAX_NAME_UNITS);
        let media = phone.media.as_ref().expect("a player");
        assert!(media.player.encode_utf16().count() <= MAX_PLAYER_UNITS);
        assert_eq!(media.title, "");
        assert!(!media.can_next);
        assert!(harness.stops_within(Duration::from_secs(3)));
    }

    #[test]
    fn an_unreachable_bus_is_absent_once_and_stops_promptly() {
        let dir =
            std::env::temp_dir().join(format!("celestina-magnetita-nobus-{}", std::process::id()));
        let harness = Harness::start(&format!("unix:path={}/missing", dir.display()));
        assert_eq!(harness.next(), MagnetitaEvent::Absent);
        // Two more attempts happen in this time; none repeats the event.
        assert!(harness.events.recv_timeout(MIN_BACKOFF * 3).is_err());
        let started = Instant::now();
        assert!(harness.stops_within(Duration::from_secs(3)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn shutdown_while_connected_is_prompt_and_raises_the_flag() {
        let bus = bus_or_skip!("magnetita-shutdown");
        let _service = Service::start(&bus, vec![phone_dict("p1", 90)]);
        let harness = Harness::start(bus.address());
        harness.until(|event| present(event).is_some());
        let flag = Arc::clone(&harness.shutdown);
        let started = Instant::now();
        assert!(harness.stops_within(Duration::from_secs(3)));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(flag.load(Ordering::Acquire));
    }
}
