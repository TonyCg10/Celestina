//! The MPRIS client: the players on the session bus, followed and commanded
//! (CEL-2-D, plan decisions 1, 4 and 5).
//!
//! Two threads on two connections of the worker's own, started by [`spawn`]:
//! the main one carries the signals and the transport commands (each call
//! bounded by [`CALL_TIMEOUT`]), the read one only the property reads, each
//! bounded by the shorter [`READ_TIMEOUT`] (zbus bounds every call of a
//! connection alike, so the two bounds need two connections).
//!
//! - **media** — connects (bounded by [`CONNECT_TIMEOUT`]), asks the bus for
//!   two signals (`NameOwnerChanged` for names under
//!   `org.mpris.MediaPlayer2`, and every signal at [`MPRIS_PATH`], which is
//!   where `PropertiesChanged` and `Seeked` come from), lists the names
//!   already there and reads each player once. It then loops: it applies
//!   what the signals said, reconciles the shown player with one `GetAll`
//!   every [`RECONCILE`], performs the loop's [`MediaCommand`]s on the
//!   player `celestina_shell::media::resolve` picks, and publishes a
//!   [`MediaEvent::Players`] whenever what a person would see changed. While
//!   something plays, its position advances by arithmetic
//!   (`advanced_position`), floored to the second, so a playing track costs
//!   one event per second and nothing on the bus; while nothing plays it
//!   costs nothing at all.
//! - **media-signals** — drains the connection's message stream without
//!   ever blocking (zbus stops reading the socket while a stream is full, so
//!   a stream left undrained during a slow call would stall that call's own
//!   reply), parses the signals it recognises into bounded deltas and keeps
//!   them for the media thread, only for the players it tracks: a signal's
//!   sender is checked against the tracked owners, and its size against
//!   [`MAX_SIGNAL_BODY_BYTES`], before its body is parsed. If the thread
//!   ends for any reason (a panic included) the media thread sees the
//!   connection as closed and reconnects.
//!
//! The shell claims no name: an MPRIS client owns none, and whatever else
//! follows the players (Noctalia, `playerctl`) keeps working beside it.
//!
//! Every input is hostile. Names, identities and track strings go through
//! `clamp_text`; at most [`MAX_PLAYERS`] players are tracked, and at most
//! [`MAX_NAMES_PER_OWNER`] of them per connection (the rest are ignored,
//! logged once, and picked up when a slot frees); a property of the wrong
//! type reads as empty or unknown. A player that does not answer costs one
//! [`READ_TIMEOUT`]: its identity is then not asked, and it is not read
//! again for [`QUARANTINE`], whatever it signals meanwhile (what a signal
//! itself carries still applies). The quarantine follows both its name and
//! its connection, so a name that changes hands or comes back, or another
//! name of the same connection, is not read early either. A player whose
//! read failed or was skipped is read again once its quarantine ends, at
//! most one such player per [`POLL`]. A command that waited longer
//! than [`COMMAND_MAX_AGE`] is dropped rather than performed late. The
//! shutdown flag is looked at between calls and at least every [`POLL`], so
//! the join is bounded. A bus that is gone or breaks publishes
//! [`MediaEvent::Unavailable`] and is tried again after [`RETRY`].

use std::collections::{HashMap, HashSet};
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::media::{
    advanced_position, clamp_text, identity_from_bus_name, is_player_name, resolve, Player, Track,
    MAX_PLAYERS, MAX_TEXT_BYTES,
};
use zbus::blocking::{connection::Builder, fdo::DBusProxy, Connection, MessageIterator};
use zbus::export::futures_core::Stream;
use zbus::message::Type;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{MatchRule, MessageStream};

use super::{CoverSource, MediaCommand, MediaEvent, MediaHandle, PlayerView, Queued};
use crate::providers::worker::Worker;
use crate::tray::item::{self, BusSignal};

/// The namespace every player's well-known name is under.
pub const PLAYER_NAMESPACE: &str = "org.mpris.MediaPlayer2";
/// Where every player serves its objects.
pub const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
/// The root interface (`Identity`).
pub const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
/// The player interface (status, metadata, transport).
pub const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
/// The standard properties interface.
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// How many commands may wait for the worker. The page's buttons and the
/// piece's middle click are deliberate gestures; past this a click repeats
/// itself, and refusing is more honest than performing it late.
pub const COMMAND_QUEUE: usize = 8;
/// The longest a transport command (and a call to the bus itself) may take.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(2);
/// The longest a property read of a player may take. Reads happen on the
/// worker's own initiative, one player after another, so a player that
/// never answers must cost the others little.
pub const READ_TIMEOUT: Duration = Duration::from_millis(500);
/// How long a player whose read timed out is not read again. Its signals
/// do not shorten it: a player can signal and still never answer.
pub const QUARANTINE: Duration = Duration::from_secs(30);
/// The longest a command may wait in the queue and still be performed.
pub const COMMAND_MAX_AGE: Duration = Duration::from_secs(2);
/// The most player names one connection (unique name) may hold and have
/// tracked: one process cannot take every slot.
pub const MAX_NAMES_PER_OWNER: usize = 2;
/// The largest signal body parsed; a larger one is dropped unread.
pub const MAX_SIGNAL_BODY_BYTES: usize = 256 * 1024;
/// The longest connecting to the bus may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// How often the worker looks at the shutdown flag, the signals and the
/// progress of a playing track.
pub const POLL: Duration = Duration::from_millis(100);
/// How often the shown player is read whole, as a backstop for a signal
/// that was lost or never sent.
pub const RECONCILE: Duration = Duration::from_secs(30);
/// How long a bus that could not be followed is left before it is tried
/// again.
pub const RETRY: Duration = Duration::from_secs(30);
/// The longest `mpris:artUrl` kept. Unlike a title, a url cut short would
/// name a different file, so a longer one is dropped whole.
pub const MAX_ART_URL_BYTES: usize = 4096;
/// How many name changes the signal thread keeps between two looks of the
/// worker; past this it asks for a full re-read of the bus instead.
pub const MAX_PENDING_OWNERS: usize = 256;
/// How many quarantined names and connections are remembered beyond the
/// players tracked; past this the one whose quarantine ends first goes.
const MAX_REMEMBERED_QUARANTINES: usize = 4 * MAX_PLAYERS;

/// Which bus the players are looked for on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MediaBus {
    /// The session bus: production.
    #[default]
    Session,
    /// A bus at this address: tests, on a private bus.
    Address(String),
}

impl MediaBus {
    fn builder(&self) -> zbus::Result<Builder<'static>> {
        match self {
            Self::Session => Builder::session(),
            Self::Address(address) => Builder::address(address.as_str()),
        }
    }
}

/// How the worker is built.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MediaConfig {
    /// Where the players are looked for.
    pub bus: MediaBus,
}

/// A failure to follow the bus, with its source.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// The bus could not be reached.
    #[error("cannot reach the session bus")]
    Connect(#[source] Box<zbus::Error>),
    /// The bus did not finish the connection within [`CONNECT_TIMEOUT`].
    #[error("the session bus did not answer within {} ms", CONNECT_TIMEOUT.as_millis())]
    ConnectTimeout,
    /// A thread the worker needs could not be started.
    #[error("cannot start the {0} thread")]
    Thread(&'static str, #[source] io::Error),
    /// The bus refused the signal subscriptions.
    #[error("cannot subscribe to the players' signals")]
    Subscribe(#[source] Box<zbus::Error>),
    /// The bus did not list its names.
    #[error("cannot list the names on the bus")]
    List(#[source] Box<zbus::Error>),
    /// The connection closed under the worker.
    #[error("the session bus connection closed")]
    Closed,
}

/// Why a command was not performed.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// There is no player to act on.
    #[error("no player is on the bus")]
    NoPlayer,
    /// A pin named a player that is not tracked.
    #[error("{0} is not a tracked player")]
    UnknownPlayer(String),
    /// The bus is not followed right now.
    #[error("the session bus is not followed")]
    Unavailable,
    /// The player's method failed or timed out.
    #[error("{player}'s {method} failed")]
    Call {
        player: String,
        method: &'static str,
        #[source]
        source: Box<zbus::Error>,
    },
}

/// A property read that failed, with its source.
#[derive(Debug, thiserror::Error)]
#[error("{method} {interface} on {player} failed")]
struct ReadError {
    player: String,
    method: &'static str,
    interface: &'static str,
    #[source]
    source: Box<zbus::Error>,
}

impl ReadError {
    /// Whether the player did not answer within [`READ_TIMEOUT`].
    fn timed_out(&self) -> bool {
        matches!(&*self.source, zbus::Error::InputOutput(io) if io.kind() == io::ErrorKind::TimedOut)
    }
}

/// Starts the worker with `config`, handing art urls to `covers` and
/// publishing through `publish`, which returns `false` once nobody listens
/// (the worker then stops). It also stops when `shutdown` is raised or
/// every [`MediaHandle`] is dropped.
///
/// # Errors
///
/// Returns the failure to start the thread.
pub fn spawn(
    config: MediaConfig,
    covers: Box<dyn CoverSource>,
    publish: impl FnMut(MediaEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(MediaHandle, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("media", shutdown, move || {
        let mut out = Out::new(publish, covers);
        run(&config, &mut out, &queue, &flag);
    })?;
    Ok((MediaHandle::new(commands), worker))
}

/// How a session with the bus ended.
enum Ended {
    /// The shutdown flag was raised.
    Shutdown,
    /// Nobody listens or commands any more.
    Gone,
    /// The bus could not be followed.
    Lost(MediaError),
}

fn run<P: FnMut(MediaEvent) -> bool>(
    config: &MediaConfig,
    out: &mut Out<P>,
    queue: &Receiver<Queued>,
    shutdown: &Arc<AtomicBool>,
) {
    loop {
        let ended = match Session::open(config, shutdown) {
            Ok(mut session) => session.serve(out, queue, shutdown),
            Err(err) => Ended::Lost(err),
        };
        let err = match ended {
            Ended::Shutdown | Ended::Gone => return,
            Ended::Lost(err) => err,
        };
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        tracing::warn!(error = %crate::error_chain(&err), "media: the session bus is not followed");
        if !out.unavailable(clamp_text(&crate::error_chain(&err))) {
            return;
        }
        if !wait_to_retry(queue, shutdown) {
            return;
        }
    }
}

/// Waits [`RETRY`] in [`POLL`] slices, refusing the commands that arrive.
/// `false` when the worker should stop instead.
fn wait_to_retry(queue: &Receiver<Queued>, shutdown: &AtomicBool) -> bool {
    let until = Instant::now() + RETRY;
    while Instant::now() < until {
        if shutdown.load(Ordering::Acquire) {
            return false;
        }
        match queue.recv_timeout(POLL) {
            Ok(queued) => refused(&queued.command, &CommandError::Unavailable),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
    true
}

fn refused(command: &MediaCommand, err: &CommandError) {
    tracing::info!(?command, error = %crate::error_chain(err), "media: a command was refused");
}

/// The command `queued` carries, or `None` when it waited longer than
/// [`COMMAND_MAX_AGE`] by `now`: a click performed that late is a surprise.
fn fresh(queued: Queued, now: Instant) -> Option<MediaCommand> {
    (now.saturating_duration_since(queued.queued_at) <= COMMAND_MAX_AGE).then_some(queued.command)
}

/// What the worker publishes, and the last of it, so only changes leave.
struct Out<P> {
    publish: P,
    covers: Box<dyn CoverSource>,
    last: Option<(Vec<PlayerView>, Option<String>)>,
    last_covers: Option<CoverInput>,
}

/// What the cover source was last handed: every `(bus_name, art_url)` and
/// the shown player.
type CoverInput = (Vec<(String, String)>, Option<String>);

impl<P: FnMut(MediaEvent) -> bool> Out<P> {
    fn new(publish: P, covers: Box<dyn CoverSource>) -> Self {
        Self {
            publish,
            covers,
            last: None,
            last_covers: None,
        }
    }

    /// Publishes the players when they differ from the last ones, then any
    /// cover events their new art urls or a new shown player produce.
    /// `false` once nobody listens.
    fn players(&mut self, players: Vec<PlayerView>, active: Option<String>) -> bool {
        if self
            .last
            .as_ref()
            .is_some_and(|(last, shown)| *last == players && *shown == active)
        {
            return true;
        }
        let art: Vec<(String, String)> = players
            .iter()
            .map(|p| (p.bus_name.clone(), p.art_url.clone()))
            .collect();
        let event = MediaEvent::Players {
            players: players.clone(),
            active: active.clone(),
        };
        if !(self.publish)(event) {
            return false;
        }
        let input = (art, active.clone());
        self.last = Some((players, active));
        if self.last_covers.as_ref() != Some(&input) {
            self.last_covers = Some(input);
            if let Some((players, shown)) = &self.last {
                for event in self.covers.update(players, shown.as_deref()) {
                    if !(self.publish)(event) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Publishes what the cover source deferred and is now due. `false`
    /// once nobody listens.
    fn poll_covers(&mut self) -> bool {
        for event in self.covers.poll() {
            if !(self.publish)(event) {
                return false;
            }
        }
        true
    }

    fn unavailable(&mut self, reason: String) -> bool {
        self.last = None;
        self.last_covers = None;
        (self.publish)(MediaEvent::Unavailable(reason))
    }
}

/// What a player's signals said since the worker last looked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Delta {
    playing: Option<bool>,
    track: Option<Track>,
    identity: Option<String>,
    position_ms: Option<i64>,
    /// A property the worker shows was invalidated: read the player again.
    reread: bool,
}

impl Delta {
    /// `self` with `newer` laid over it.
    fn merge(&mut self, newer: Delta) {
        if newer.playing.is_some() {
            self.playing = newer.playing;
        }
        if newer.track.is_some() {
            self.track = newer.track;
        }
        if newer.identity.is_some() {
            self.identity = newer.identity;
        }
        if newer.position_ms.is_some() {
            self.position_ms = newer.position_ms;
        }
        self.reread |= newer.reread;
    }
}

/// A signal the worker acts on.
#[derive(Debug, PartialEq, Eq)]
enum Signal {
    /// A player's name changed hands (`new_owner` empty: it left).
    Owner { name: String, new_owner: String },
    /// A player said something about itself.
    Changed { sender: String, delta: Delta },
}

/// What the signal thread hands the worker; every part is bounded.
#[derive(Debug, Default)]
struct Pending {
    /// Name changes in order, at most [`MAX_PENDING_OWNERS`].
    owners: Vec<(String, String)>,
    /// The name changes overflowed: the worker reads the bus again.
    resync: bool,
    /// Deltas by sender, only for senders in `known`.
    deltas: Vec<(String, Delta)>,
    /// The unique names of the tracked players (at most [`MAX_PLAYERS`]).
    known: HashSet<String>,
    /// The stream ended: the connection is gone.
    ended: bool,
}

impl Pending {
    fn record(&mut self, signal: Signal) {
        match signal {
            Signal::Owner { name, new_owner } => {
                if self.owners.len() >= MAX_PENDING_OWNERS {
                    self.owners.clear();
                    self.resync = true;
                }
                if !self.resync {
                    self.owners.push((name, new_owner));
                }
            }
            Signal::Changed { sender, delta } => {
                if !self.known.contains(&sender) {
                    return;
                }
                if let Some((_, kept)) = self.deltas.iter_mut().find(|(s, _)| *s == sender) {
                    kept.merge(delta);
                } else {
                    self.deltas.push((sender, delta));
                }
            }
        }
    }
}

type Shared = Arc<Mutex<Pending>>;

/// The lock, recovered if a thread panicked holding it: what it guards is a
/// queue of hints, and a half-recorded hint is harmless.
fn lock(pending: &Mutex<Pending>) -> MutexGuard<'_, Pending> {
    pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Recognises `message` as one of the signals the worker acts on.
///
/// `NameOwnerChanged` is the tray's recogniser's (it trusts only the bus
/// driver's broadcast), kept to names under the players' namespace. The
/// players' own signals count only from [`MPRIS_PATH`] (the stream also
/// carries signals a peer addresses to the shell directly), only from a
/// sender `is_known` accepts, and only up to [`MAX_SIGNAL_BODY_BYTES`]:
/// both are checked on the header, before the body is parsed.
fn recognise(message: &zbus::message::Message, is_known: impl Fn(&str) -> bool) -> Option<Signal> {
    let header = message.header();
    if header.message_type() != Type::Signal {
        return None;
    }
    let interface = header.interface()?.as_str();
    if interface == "org.freedesktop.DBus" {
        return match item::recognise(message)? {
            BusSignal::NameOwnerChanged { name, new_owner } if is_player_name(&name) => {
                Some(Signal::Owner { name, new_owner })
            }
            _ => None,
        };
    }
    if header.path()?.as_str() != MPRIS_PATH {
        return None;
    }
    let sender = header.sender()?.as_str();
    if !is_known(sender) || message.body().len() > MAX_SIGNAL_BODY_BYTES {
        return None;
    }
    let sender = sender.to_owned();
    let member = header.member()?.as_str();
    let delta = match (interface, member) {
        (PROPERTIES_INTERFACE, "PropertiesChanged") => {
            let (changed_on, changed, invalidated): (
                String,
                HashMap<String, OwnedValue>,
                Vec<String>,
            ) = message.body().deserialize().ok()?;
            properties_delta(&changed_on, &changed, &invalidated)?
        }
        (PLAYER_INTERFACE, "Seeked") => {
            let (micros,): (i64,) = message.body().deserialize().ok()?;
            Delta {
                position_ms: Some(micros_to_ms(&Value::I64(micros))),
                ..Delta::default()
            }
        }
        _ => return None,
    };
    Some(Signal::Changed { sender, delta })
}

/// The delta a `PropertiesChanged` on `interface` carries, or `None` for an
/// interface the worker does not show.
fn properties_delta(
    interface: &str,
    changed: &HashMap<String, OwnedValue>,
    invalidated: &[String],
) -> Option<Delta> {
    let mut delta = Delta::default();
    match interface {
        PLAYER_INTERFACE => {
            apply_player_properties(&mut delta, changed);
            delta.reread = invalidated
                .iter()
                .any(|name| matches!(name.as_str(), "PlaybackStatus" | "Metadata"));
        }
        ROOT_INTERFACE => {
            delta.identity = changed.get("Identity").map(|value| text(value));
            delta.reread = invalidated.iter().any(|name| name == "Identity");
        }
        _ => return None,
    }
    Some(delta)
}

/// What a player's properties map says, laid over `delta`.
fn apply_player_properties(delta: &mut Delta, properties: &HashMap<String, OwnedValue>) {
    if let Some(status) = properties.get("PlaybackStatus") {
        delta.playing = Some(matches!(peel(status), Value::Str(s) if s.as_str() == "Playing"));
    }
    if let Some(metadata) = properties.get("Metadata") {
        delta.track = Some(track(metadata));
    }
    if let Some(position) = properties.get("Position") {
        delta.position_ms = Some(micros_to_ms(position));
    }
}

/// `value` without the variants wrapped around it.
fn peel<'v, 'a>(mut value: &'v Value<'a>) -> &'v Value<'a> {
    while let Value::Value(inner) = value {
        value = inner;
    }
    value
}

/// A string property, bounded; anything else is empty.
fn text(value: &Value<'_>) -> String {
    match peel(value) {
        Value::Str(s) => clamp_text(s.as_str()),
        _ => String::new(),
    }
}

/// `xesam:artist`: a list joined by `, ` (a lone string is accepted too),
/// bounded; a non-string element is skipped, anything else is empty.
fn artists(value: &Value<'_>) -> String {
    match peel(value) {
        Value::Str(s) => clamp_text(s.as_str()),
        Value::Array(array) => {
            let mut joined = String::new();
            for element in array.inner() {
                if joined.len() > MAX_TEXT_BYTES {
                    break;
                }
                if let Value::Str(s) = peel(element) {
                    if s.as_str().is_empty() {
                        continue;
                    }
                    if !joined.is_empty() {
                        joined.push_str(", ");
                    }
                    joined.push_str(&clamp_text(s.as_str()));
                }
            }
            clamp_text(&joined)
        }
        _ => String::new(),
    }
}

/// `mpris:artUrl`, untouched but dropped whole when not a string or longer
/// than [`MAX_ART_URL_BYTES`].
fn art_url(value: &Value<'_>) -> String {
    match peel(value) {
        Value::Str(s) if s.as_str().len() <= MAX_ART_URL_BYTES => s.as_str().to_owned(),
        _ => String::new(),
    }
}

/// A duration or position in microseconds as milliseconds; `-1` for a
/// negative value or anything that is not an integer.
fn micros_to_ms(value: &Value<'_>) -> i64 {
    let micros: Option<i64> = match peel(value) {
        Value::I64(v) => Some(*v),
        Value::U64(v) => i64::try_from(*v).ok(),
        Value::I32(v) => Some(i64::from(*v)),
        Value::U32(v) => Some(i64::from(*v)),
        Value::I16(v) => Some(i64::from(*v)),
        Value::U16(v) => Some(i64::from(*v)),
        Value::U8(v) => Some(i64::from(*v)),
        _ => None,
    };
    micros.filter(|v| *v >= 0).map_or(-1, |v| v / 1000)
}

/// A player's `Metadata`; anything that is not a dictionary is an empty
/// track, and a wrong-typed entry reads as empty or unknown.
fn track(value: &Value<'_>) -> Track {
    let mut track = Track::default();
    let Value::Dict(dict) = peel(value) else {
        return track;
    };
    for (key, entry) in dict.iter() {
        let Value::Str(key) = peel(key) else {
            continue;
        };
        match key.as_str() {
            "xesam:title" => track.title = text(entry),
            "xesam:artist" => track.artist = artists(entry),
            "xesam:album" => track.album = text(entry),
            "mpris:artUrl" => track.art_url = art_url(entry),
            "mpris:length" => track.length_ms = micros_to_ms(entry),
            _ => {}
        }
    }
    track
}

/// One player the worker tracks.
#[derive(Debug)]
struct Tracked {
    player: Player,
    /// The unique name holding the player's name.
    owner: String,
    /// When `player.position_ms` was measured.
    measured: Instant,
    /// Not read before this instant: a read of it timed out. Only time
    /// lifts this.
    quarantined_until: Option<Instant>,
    /// Its last read failed or was skipped, so what is shown may be the
    /// defaults or stale: it is read again once it is not quarantined.
    unread: bool,
}

impl Tracked {
    /// Where the track has reached at `now`, unfloored, never past its
    /// length.
    fn position_at(&self, now: Instant) -> i64 {
        let elapsed = i64::try_from(now.saturating_duration_since(self.measured).as_millis())
            .unwrap_or(i64::MAX);
        let reached = advanced_position(self.player.position_ms, self.player.playing, elapsed);
        let length = self.player.track.length_ms;
        if length >= 0 && reached > length {
            length
        } else {
            reached
        }
    }

    fn quarantined(&self, now: Instant) -> bool {
        self.quarantined_until.is_some_and(|until| now < until)
    }

    /// Sets the position to `position_ms`, measured `now`.
    fn measure(&mut self, position_ms: i64, now: Instant) {
        self.player.position_ms = position_ms;
        self.measured = now;
    }
}

/// The signal thread, stopped and joined when dropped.
struct Signals {
    stop: Arc<AtomicBool>,
    worker: Option<Worker>,
}

impl Drop for Signals {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join();
        }
    }
}

/// Wakes the signal thread when its stream has something.
struct ChannelWaker(SyncSender<()>);

impl Wake for ChannelWaker {
    fn wake(self: Arc<Self>) {
        let _ = self.0.try_send(());
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.0.try_send(());
    }
}

/// Marks the stream ended when the signal thread leaves, however it leaves
/// (a panic included), so the media thread reconnects instead of following
/// a connection nobody drains.
struct EndOnExit<'a>(&'a Mutex<Pending>);

impl Drop for EndOnExit<'_> {
    fn drop(&mut self) {
        lock(self.0).ended = true;
    }
}

fn signal_worker(
    mut stream: MessageStream,
    pending: &Mutex<Pending>,
    stop: &AtomicBool,
    shutdown: &AtomicBool,
) {
    let _ended = EndOnExit(pending);
    let (wake, woken) = mpsc::sync_channel(1);
    let waker = Waker::from(Arc::new(ChannelWaker(wake)));
    let mut cx = Context::from_waker(&waker);
    while !stop.load(Ordering::Acquire) && !shutdown.load(Ordering::Acquire) {
        loop {
            match Pin::new(&mut stream).poll_next(&mut cx) {
                Poll::Ready(Some(Ok(message))) => {
                    let is_known = |sender: &str| lock(pending).known.contains(sender);
                    if let Some(signal) = recognise(&message, is_known) {
                        lock(pending).record(signal);
                    }
                }
                Poll::Ready(Some(Err(err))) => {
                    tracing::debug!(%err, "media: the message stream failed");
                }
                Poll::Ready(None) => return,
                Poll::Pending => break,
            }
        }
        match woken.recv_timeout(POLL) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// One connection to the bus and everything followed on it.
struct Session {
    /// Signals, the bus itself and transport commands ([`CALL_TIMEOUT`]).
    connection: Connection,
    /// Property reads only ([`READ_TIMEOUT`]).
    reads: Connection,
    dbus: DBusProxy<'static>,
    pending: Shared,
    // Dropped (stopped and joined) with the session.
    _signals: Signals,
    players: Vec<Tracked>,
    pin: Option<String>,
    heard: u64,
    /// A player was ignored for want of a slot: when one frees, the bus is
    /// read again to pick it up.
    overflowed: bool,
    /// The ignoring was logged (once per connection).
    overflow_logged: bool,
    /// A name past [`MAX_NAMES_PER_OWNER`] was ignored and logged (once per
    /// connection).
    owner_cap_logged: bool,
    reconcile_at: Instant,
    /// Quarantines by bus name and by owner, kept apart from the tracked
    /// players so leaving and coming back, or changing hands, keeps them;
    /// at most [`MAX_REMEMBERED_QUARANTINES`].
    remembered: Vec<(String, Instant)>,
}

impl Session {
    fn open(config: &MediaConfig, shutdown: &Arc<AtomicBool>) -> Result<Session, MediaError> {
        let connection = connect(&config.bus, CALL_TIMEOUT)?;
        let reads = connect(&config.bus, READ_TIMEOUT)?;
        // The stream exists before the subscriptions, so nothing they match
        // is missed between them and the first read of the bus.
        let stream = MessageIterator::from(&connection).into_inner();
        let dbus =
            DBusProxy::new(&connection).map_err(|err| MediaError::Subscribe(Box::new(err)))?;
        subscribe(&dbus).map_err(|err| MediaError::Subscribe(Box::new(err)))?;
        let pending = Shared::default();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let pending = Arc::clone(&pending);
            let stop_flag = Arc::clone(&stop);
            let parent = Arc::clone(shutdown);
            Worker::spawn("media-signals", &stop, move || {
                signal_worker(stream, &pending, &stop_flag, &parent);
            })
            .map_err(|err| MediaError::Thread("media-signals", err))?
        };
        let mut session = Session {
            connection,
            reads,
            dbus,
            pending,
            _signals: Signals {
                stop,
                worker: Some(worker),
            },
            players: Vec::new(),
            pin: None,
            heard: 0,
            overflowed: false,
            overflow_logged: false,
            owner_cap_logged: false,
            reconcile_at: Instant::now() + RECONCILE,
            remembered: Vec::new(),
        };
        session.scan(shutdown)?;
        Ok(session)
    }

    fn serve<P: FnMut(MediaEvent) -> bool>(
        &mut self,
        out: &mut Out<P>,
        queue: &Receiver<Queued>,
        shutdown: &AtomicBool,
    ) -> Ended {
        // The first read of the bus is always published, even when empty.
        let (players, active) = self.views(Instant::now());
        if !out.players(players, active) {
            return Ended::Gone;
        }
        loop {
            if shutdown.load(Ordering::Acquire) {
                return Ended::Shutdown;
            }
            if let Err(err) = self.take_pending(shutdown) {
                return Ended::Lost(err);
            }
            if Instant::now() >= self.reconcile_at {
                self.reconcile(shutdown);
            }
            self.retry_unread(shutdown);
            let (players, active) = self.views(Instant::now());
            if !out.players(players, active) || !out.poll_covers() {
                return Ended::Gone;
            }
            match queue.recv_timeout(POLL) {
                Ok(queued) => match fresh(queued, Instant::now()) {
                    Some(command) => {
                        if let Err(err) = self.perform(&command) {
                            refused(&command, &err);
                        }
                    }
                    None => tracing::info!("media: a command waited too long and was dropped"),
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ended::Gone,
            }
        }
    }

    fn next_heard(&mut self) -> u64 {
        self.heard = self.heard.saturating_add(1);
        self.heard
    }

    /// Reads which players are on the bus now, adding the new ones and
    /// dropping the ones that left.
    fn scan(&mut self, shutdown: &AtomicBool) -> Result<(), MediaError> {
        let names = self
            .dbus
            .list_names()
            .map_err(|err| MediaError::List(Box::new(err.into())))?;
        let mut present: Vec<(String, String)> = Vec::new();
        for name in names {
            if shutdown.load(Ordering::Acquire) {
                return Ok(());
            }
            let name = name.as_str();
            if !is_player_name(name) {
                continue;
            }
            let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
                continue;
            };
            if let Ok(owner) = self.dbus.get_name_owner(bus_name) {
                present.push((name.to_owned(), owner.as_str().to_owned()));
            }
        }
        present.sort();
        let gone: Vec<String> = self
            .players
            .iter()
            .filter(|t| !present.iter().any(|(name, _)| *name == t.player.bus_name))
            .map(|t| t.player.bus_name.clone())
            .collect();
        for name in gone {
            self.leave(&name);
        }
        for (name, owner) in present {
            if shutdown.load(Ordering::Acquire) {
                return Ok(());
            }
            self.appear(&name, &owner, shutdown);
        }
        self.tell_known();
        Ok(())
    }

    /// Applies what the signal thread kept since the last look.
    fn take_pending(&mut self, shutdown: &AtomicBool) -> Result<(), MediaError> {
        let (owners, resync, deltas, ended) = {
            let mut pending = lock(&self.pending);
            (
                std::mem::take(&mut pending.owners),
                std::mem::take(&mut pending.resync),
                std::mem::take(&mut pending.deltas),
                pending.ended,
            )
        };
        if ended {
            return Err(MediaError::Closed);
        }
        if resync {
            self.scan(shutdown)?;
        } else {
            let mut freed = false;
            for (name, new_owner) in owners {
                if shutdown.load(Ordering::Acquire) {
                    return Ok(());
                }
                if new_owner.is_empty() {
                    freed |= self.leave(&name);
                } else {
                    self.appear(&name, &new_owner, shutdown);
                }
            }
            if freed && self.overflowed {
                // A slot freed while a player was waiting for one.
                self.overflowed = false;
                self.scan(shutdown)?;
            }
            self.tell_known();
        }
        for (sender, delta) in deltas {
            if shutdown.load(Ordering::Acquire) {
                return Ok(());
            }
            self.apply(&sender, delta, shutdown);
        }
        Ok(())
    }

    /// Tells the signal thread whose signals to keep.
    fn tell_known(&self) {
        let known = self.players.iter().map(|t| t.owner.clone()).collect();
        lock(&self.pending).known = known;
    }

    /// `name` is held by `owner`: a new player is read, a known one whose
    /// owner changed is read again.
    fn appear(&mut self, name: &str, owner: &str, shutdown: &AtomicBool) {
        if let Some(index) = self.index_of(name) {
            if self.players[index].owner == owner {
                return;
            }
            if self.names_held_by(owner) >= MAX_NAMES_PER_OWNER {
                self.leave(name);
                self.owner_capped(owner);
                return;
            }
            // Churn buys no read: the quarantine its name had, or its new
            // owner has, stays.
            let inherited = self.remembered_quarantine(name, owner);
            let tracked = &mut self.players[index];
            tracked.owner = owner.to_owned();
            tracked.quarantined_until = tracked.quarantined_until.max(inherited);
            // Known before the read, so a signal it sends meanwhile counts.
            lock(&self.pending).known.insert(owner.to_owned());
            self.read_player(index, shutdown);
            return;
        }
        if self.names_held_by(owner) >= MAX_NAMES_PER_OWNER {
            self.owner_capped(owner);
            return;
        }
        if self.players.len() >= MAX_PLAYERS {
            if !self.overflow_logged {
                tracing::warn!(
                    "media: more than {MAX_PLAYERS} players are on the bus; the rest are ignored"
                );
                self.overflow_logged = true;
            }
            self.overflowed = true;
            return;
        }
        let heard = self.next_heard();
        let bus_name = clamp_text(name);
        let quarantined_until = self.remembered_quarantine(&bus_name, owner);
        self.players.push(Tracked {
            player: Player {
                identity: identity_from_bus_name(&bus_name),
                bus_name,
                playing: false,
                track: Track::default(),
                position_ms: -1,
                heard,
            },
            owner: owner.to_owned(),
            measured: Instant::now(),
            quarantined_until,
            unread: false,
        });
        // Known before the read, so a signal it sends meanwhile counts.
        lock(&self.pending).known.insert(owner.to_owned());
        let index = self.players.len() - 1;
        self.read_player(index, shutdown);
    }

    fn names_held_by(&self, owner: &str) -> usize {
        self.players.iter().filter(|t| t.owner == owner).count()
    }

    /// `owner` holds [`MAX_NAMES_PER_OWNER`] tracked names already: one more
    /// is ignored, and picked up by a re-read when a slot frees.
    fn owner_capped(&mut self, owner: &str) {
        if !self.owner_cap_logged {
            tracing::warn!(
                owner = %clamp_text(owner),
                "media: a connection holds more than {MAX_NAMES_PER_OWNER} player names; the rest are ignored"
            );
            self.owner_cap_logged = true;
        }
        self.overflowed = true;
    }

    /// `name` left the bus. Returns whether a tracked player went away.
    fn leave(&mut self, name: &str) -> bool {
        let before = self.players.len();
        self.players.retain(|t| t.player.bus_name != name);
        if self.pin.as_deref() == Some(name) {
            self.pin = None;
        }
        self.players.len() < before
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.players.iter().position(|t| t.player.bus_name == name)
    }

    /// Reads a player whole: its player properties and, when those
    /// answered, its identity. A read that fails leaves what was known (or
    /// the defaults); one that timed out quarantines the player. A
    /// quarantined player is not read. Either way it is left unread.
    fn read_player(&mut self, index: usize, shutdown: &AtomicBool) {
        if self.is_quarantined(index) || !self.read_player_properties(index) {
            self.mark_unread(index);
            return;
        }
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let Some(name) = self.players.get(index).map(|t| t.player.bus_name.clone()) else {
            return;
        };
        match self.get(&name, ROOT_INTERFACE, "Identity") {
            Ok(value) => {
                let identity = text(&value);
                if let Some(tracked) = self.players.get_mut(index) {
                    tracked.player.identity = if identity.is_empty() {
                        identity_from_bus_name(&name)
                    } else {
                        identity
                    };
                }
            }
            Err(err) => self.read_failed(index, "media: no identity", &err),
        }
    }

    fn mark_unread(&mut self, index: usize) {
        if let Some(tracked) = self.players.get_mut(index) {
            tracked.unread = true;
        }
    }

    /// Reads again the first player left unread whose quarantine has
    /// ended: at most one per call, so the players that answer slowly cost
    /// at most one [`READ_TIMEOUT`] a tick between them. One whose read
    /// fails again without timing out waits [`QUARANTINE`] too, rather than
    /// being asked every tick.
    fn retry_unread(&mut self, shutdown: &AtomicBool) {
        let now = Instant::now();
        let Some(index) = self
            .players
            .iter()
            .position(|t| t.unread && !t.quarantined(now))
        else {
            return;
        };
        self.read_player(index, shutdown);
        let now = Instant::now();
        if let Some(tracked) = self.players.get_mut(index) {
            if tracked.unread && !tracked.quarantined(now) {
                tracked.quarantined_until = Some(now + QUARANTINE);
            }
        }
    }

    /// When the quarantine remembered for `bus_name` or `owner` ends, if
    /// either still has one.
    fn remembered_quarantine(&self, bus_name: &str, owner: &str) -> Option<Instant> {
        let now = Instant::now();
        self.remembered
            .iter()
            .filter(|(key, until)| (key == bus_name || key == owner) && now < *until)
            .map(|(_, until)| *until)
            .max()
    }

    /// Remembers that `key` is quarantined until `until`, forgetting what
    /// has ended and, when full, the one that ends first.
    fn remember_quarantine(&mut self, key: &str, until: Instant) {
        let now = Instant::now();
        self.remembered
            .retain(|(known, ends)| now < *ends && known != key);
        if self.remembered.len() >= MAX_REMEMBERED_QUARANTINES {
            let first = self
                .remembered
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, ends))| *ends)
                .map(|(i, _)| i);
            if let Some(first) = first {
                self.remembered.swap_remove(first);
            }
        }
        self.remembered.push((key.to_owned(), until));
    }

    fn is_quarantined(&self, index: usize) -> bool {
        self.players
            .get(index)
            .is_some_and(|t| t.quarantined(Instant::now()))
    }

    /// Logs a failed read of the player at `index`, and quarantines it when
    /// the read timed out.
    fn read_failed(&mut self, index: usize, what: &'static str, err: &ReadError) {
        tracing::debug!(error = %crate::error_chain(err), "{what}");
        if !err.timed_out() {
            return;
        }
        let until = Instant::now() + QUARANTINE;
        let Some(tracked) = self.players.get_mut(index) else {
            return;
        };
        tracked.quarantined_until = Some(until);
        tracing::info!(
            bus_name = %tracked.player.bus_name,
            "media: a player did not answer; it is not read again for {} s",
            QUARANTINE.as_secs()
        );
        let (bus_name, owner) = (tracked.player.bus_name.clone(), tracked.owner.clone());
        self.remember_quarantine(&bus_name, until);
        self.remember_quarantine(&owner, until);
    }

    /// Reads the player properties; `false` when the read failed.
    fn read_player_properties(&mut self, index: usize) -> bool {
        let Some(name) = self.players.get(index).map(|t| t.player.bus_name.clone()) else {
            return false;
        };
        let properties = match self.get_all(&name, PLAYER_INTERFACE) {
            Ok(properties) => properties,
            Err(err) => {
                self.read_failed(index, "media: a player was not read", &err);
                return false;
            }
        };
        let mut delta = Delta::default();
        apply_player_properties(&mut delta, &properties);
        let now = Instant::now();
        if let Some(tracked) = self.players.get_mut(index) {
            tracked.player.playing = delta.playing.unwrap_or(false);
            tracked.player.track = delta.track.unwrap_or_default();
            tracked.measure(delta.position_ms.unwrap_or(-1), now);
            tracked.unread = false;
        }
        true
    }

    /// Lays a player's delta over every player `sender` holds. A signal
    /// never lifts a quarantine: what it carries applies, but the reads it
    /// would ask for wait until the quarantine ends.
    fn apply(&mut self, sender: &str, delta: Delta, shutdown: &AtomicBool) {
        let indices: Vec<usize> = (0..self.players.len())
            .filter(|i| self.players[*i].owner == sender)
            .collect();
        for index in indices {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            let heard = self.next_heard();
            let now = Instant::now();
            let Some(tracked) = self.players.get_mut(index) else {
                continue;
            };
            tracked.player.heard = heard;
            // Freeze the arithmetic before anything it depends on changes.
            let reached = tracked.position_at(now);
            tracked.measure(reached, now);
            let status_changed = delta
                .playing
                .is_some_and(|playing| playing != tracked.player.playing);
            if let Some(playing) = delta.playing {
                tracked.player.playing = playing;
            }
            if let Some(track) = &delta.track {
                tracked.player.track = track.clone();
            }
            if let Some(identity) = &delta.identity {
                tracked.player.identity = if identity.is_empty() {
                    identity_from_bus_name(&tracked.player.bus_name)
                } else {
                    identity.clone()
                };
            }
            if let Some(position) = delta.position_ms {
                tracked.measure(position, now);
            }
            if delta.reread {
                self.read_player(index, shutdown);
            } else if status_changed && delta.position_ms.is_none() {
                // `Position` is never signalled: a track that started or
                // stopped is asked where it is.
                self.read_position(index);
            }
        }
    }

    fn read_position(&mut self, index: usize) {
        if self.is_quarantined(index) {
            // The whole read once the quarantine ends brings the position.
            self.mark_unread(index);
            return;
        }
        let Some(name) = self.players.get(index).map(|t| t.player.bus_name.clone()) else {
            return;
        };
        match self.get(&name, PLAYER_INTERFACE, "Position") {
            Ok(value) => {
                let position = micros_to_ms(&value);
                if let Some(tracked) = self.players.get_mut(index) {
                    tracked.measure(position, Instant::now());
                }
            }
            Err(err) => self.read_failed(index, "media: no position", &err),
        }
    }

    /// Reads the shown player whole, as a backstop for lost signals.
    fn reconcile(&mut self, shutdown: &AtomicBool) {
        self.reconcile_at = Instant::now() + RECONCILE;
        let Some(name) = self.shown() else {
            return;
        };
        if let Some(index) = self.index_of(&name) {
            self.read_player(index, shutdown);
        }
    }

    fn core_players(&self) -> Vec<Player> {
        self.players.iter().map(|t| t.player.clone()).collect()
    }

    /// The bus name of the player shown and commanded.
    fn shown(&self) -> Option<String> {
        let players = self.core_players();
        resolve(&players, self.pin.as_deref()).map(|p| p.bus_name.clone())
    }

    /// What the page and the piece show at `now`.
    fn views(&self, now: Instant) -> (Vec<PlayerView>, Option<String>) {
        let mut views: Vec<PlayerView> = self
            .players
            .iter()
            .map(|tracked| {
                let player = &tracked.player;
                let reached = tracked.position_at(now);
                PlayerView {
                    bus_name: player.bus_name.clone(),
                    identity: player.identity.clone(),
                    playing: player.playing,
                    title: player.track.title.clone(),
                    artist: player.track.artist.clone(),
                    album: player.track.album.clone(),
                    art_url: player.track.art_url.clone(),
                    length_ms: player.track.length_ms,
                    position_ms: if reached < 0 {
                        -1
                    } else {
                        reached - reached % 1000
                    },
                    pinned: self.pin.as_deref() == Some(player.bus_name.as_str()),
                }
            })
            .collect();
        views.sort_by(|a, b| a.bus_name.cmp(&b.bus_name));
        (views, self.shown())
    }

    /// Performs `command` on the shown player.
    fn perform(&mut self, command: &MediaCommand) -> Result<(), CommandError> {
        let method = match command {
            MediaCommand::Pin(None) => {
                self.pin = None;
                return Ok(());
            }
            MediaCommand::Pin(Some(name)) => {
                if self.index_of(name).is_none() {
                    return Err(CommandError::UnknownPlayer(clamp_text(name)));
                }
                self.pin = Some(name.clone());
                return Ok(());
            }
            MediaCommand::Previous => "Previous",
            MediaCommand::PlayPause => "PlayPause",
            MediaCommand::Next => "Next",
        };
        let player = self.shown().ok_or(CommandError::NoPlayer)?;
        self.connection
            .call_method(
                Some(player.as_str()),
                MPRIS_PATH,
                Some(PLAYER_INTERFACE),
                method,
                &(),
            )
            .map(|_| ())
            .map_err(|source| CommandError::Call {
                player,
                method,
                source: Box::new(source),
            })
    }

    fn get_all(
        &self,
        player: &str,
        interface: &'static str,
    ) -> Result<HashMap<String, OwnedValue>, ReadError> {
        const METHOD: &str = "GetAll";
        let failed = |source| ReadError {
            player: player.to_owned(),
            method: METHOD,
            interface,
            source: Box::new(source),
        };
        let reply = self
            .reads
            .call_method(
                Some(player),
                MPRIS_PATH,
                Some(PROPERTIES_INTERFACE),
                METHOD,
                &(interface,),
            )
            .map_err(failed)?;
        reply.body().deserialize().map_err(failed)
    }

    fn get(
        &self,
        player: &str,
        interface: &'static str,
        property: &str,
    ) -> Result<OwnedValue, ReadError> {
        const METHOD: &str = "Get";
        let failed = |source| ReadError {
            player: player.to_owned(),
            method: METHOD,
            interface,
            source: Box::new(source),
        };
        let reply = self
            .reads
            .call_method(
                Some(player),
                MPRIS_PATH,
                Some(PROPERTIES_INTERFACE),
                METHOD,
                &(interface, property),
            )
            .map_err(failed)?;
        reply.body().deserialize().map_err(failed)
    }
}

/// Asks the bus for the two signals the worker acts on.
fn subscribe(dbus: &DBusProxy<'_>) -> zbus::Result<()> {
    let owners = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg0ns(PLAYER_NAMESPACE)?
        .build();
    dbus.add_match_rule(owners)?;
    let players = MatchRule::builder()
        .msg_type(Type::Signal)
        .path(MPRIS_PATH)?
        .build();
    dbus.add_match_rule(players)?;
    Ok(())
}

/// Connects to `bus` with every call bounded by `call_timeout`, on a helper
/// thread, waiting at most [`CONNECT_TIMEOUT`]: the handshake has no
/// timeout of its own. A helper still connecting after that is left behind
/// and ends with its attempt.
fn connect(bus: &MediaBus, call_timeout: Duration) -> Result<Connection, MediaError> {
    let builder = bus
        .builder()
        .map_err(|err| MediaError::Connect(Box::new(err)))?;
    let (done, result) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("media-connect".to_owned())
        .spawn(move || {
            let connection = builder.method_timeout(call_timeout).build();
            let _ = done.send(connection);
        })
        .map_err(|err| MediaError::Thread("media-connect", err))?;
    match result.recv_timeout(CONNECT_TIMEOUT) {
        Ok(connection) => connection.map_err(|err| MediaError::Connect(Box::new(err))),
        Err(_) => Err(MediaError::ConnectTimeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::test_bus::PrivateBus;

    const WAIT: Duration = Duration::from_secs(5);

    // ---- pure parsing -------------------------------------------------

    fn owned(value: Value<'_>) -> OwnedValue {
        OwnedValue::try_from(value).expect("an ownable value")
    }

    fn dict(entries: Vec<(&str, Value<'static>)>) -> Value<'static> {
        let map: HashMap<String, Value<'static>> = entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        Value::from(map)
    }

    #[test]
    fn metadata_is_read_into_a_bounded_track() {
        let metadata = dict(vec![
            ("xesam:title", Value::from("Nightcall")),
            ("xesam:artist", Value::from(vec!["Kavinsky", "Lovefoxxx"])),
            ("xesam:album", Value::from("OutRun")),
            ("mpris:artUrl", Value::from("file:///tmp/a.png")),
            ("mpris:length", Value::I64(258_000_000)),
        ]);
        let track = track(&Value::Value(Box::new(metadata)));
        assert_eq!(track.title, "Nightcall");
        assert_eq!(track.artist, "Kavinsky, Lovefoxxx");
        assert_eq!(track.album, "OutRun");
        assert_eq!(track.art_url, "file:///tmp/a.png");
        assert_eq!(track.length_ms, 258_000);
    }

    #[test]
    fn wrong_types_read_as_empty_or_unknown() {
        let metadata = dict(vec![
            ("xesam:title", Value::U32(7)),
            ("xesam:artist", Value::Bool(true)),
            ("xesam:album", Value::from(vec![1_u8, 2])),
            ("mpris:artUrl", Value::I64(3)),
            ("mpris:length", Value::from("long")),
        ]);
        assert_eq!(track(&metadata), Track::default());
        // Metadata that is not a dictionary at all is an empty track.
        assert_eq!(track(&Value::from("not a dict")), Track::default());
        // A negative or non-integer length is unknown; other widths count.
        assert_eq!(micros_to_ms(&Value::I64(-5)), -1);
        assert_eq!(micros_to_ms(&Value::F64(1.0)), -1);
        assert_eq!(micros_to_ms(&Value::U64(u64::MAX)), -1);
        assert_eq!(micros_to_ms(&Value::U32(4_000)), 4);
        // A status that is not a string is not playing.
        let mut delta = Delta::default();
        let properties = HashMap::from([("PlaybackStatus".to_owned(), owned(Value::U32(1)))]);
        apply_player_properties(&mut delta, &properties);
        assert_eq!(delta.playing, Some(false));
    }

    #[test]
    fn hostile_text_is_bounded() {
        let huge = "x".repeat(10 * 1024 * 1024);
        let many: Vec<String> = (0..100).map(|i| format!("artist number {i}")).collect();
        let metadata = dict(vec![
            ("xesam:title", Value::from(huge.clone())),
            ("xesam:artist", Value::from(many)),
            ("mpris:artUrl", Value::from(format!("file:///{huge}"))),
        ]);
        let track = track(&metadata);
        assert_eq!(track.title.len(), MAX_TEXT_BYTES);
        assert!(track.artist.len() <= MAX_TEXT_BYTES);
        assert!(track.artist.starts_with("artist number 0, artist number 1"));
        // A url cut short would name another file: it is dropped whole.
        assert_eq!(track.art_url, "");
    }

    #[test]
    fn deltas_merge_newest_first_and_only_for_known_senders() {
        let mut pending = Pending::default();
        let changed = |playing| Signal::Changed {
            sender: ":1.5".to_owned(),
            delta: Delta {
                playing: Some(playing),
                ..Delta::default()
            },
        };
        pending.record(changed(true));
        assert!(pending.deltas.is_empty(), "an unknown sender is dropped");
        pending.known.insert(":1.5".to_owned());
        pending.record(changed(true));
        pending.record(Signal::Changed {
            sender: ":1.5".to_owned(),
            delta: Delta {
                position_ms: Some(7),
                ..Delta::default()
            },
        });
        pending.record(changed(false));
        assert_eq!(pending.deltas.len(), 1);
        assert_eq!(pending.deltas[0].1.playing, Some(false));
        assert_eq!(pending.deltas[0].1.position_ms, Some(7));
    }

    #[test]
    fn too_many_name_changes_ask_for_a_full_reread() {
        let mut pending = Pending::default();
        for i in 0..=MAX_PENDING_OWNERS {
            pending.record(Signal::Owner {
                name: format!("org.mpris.MediaPlayer2.p{i}"),
                new_owner: ":1.1".to_owned(),
            });
        }
        assert!(pending.resync);
        assert!(pending.owners.is_empty());
    }

    // ---- the worker on a private bus -----------------------------------

    /// What a fake player serves and was asked.
    #[derive(Debug)]
    struct FakeData {
        status: OwnedValue,
        metadata: OwnedValue,
        position_us: i64,
        identity: String,
        calls: Vec<String>,
    }

    type Data = Arc<Mutex<FakeData>>;

    fn data(status: &str, title: &str) -> Data {
        Arc::new(Mutex::new(FakeData {
            status: owned(Value::from(status.to_owned())),
            metadata: owned(dict(vec![
                ("xesam:title", Value::from(title.to_owned())),
                ("xesam:artist", Value::from(vec!["Someone".to_owned()])),
                ("mpris:length", Value::I64(300_000_000)),
            ])),
            position_us: 0,
            identity: "Fake Player".to_owned(),
            calls: Vec::new(),
        }))
    }

    fn clone_value(value: &OwnedValue) -> OwnedValue {
        value.try_clone().expect("a clonable value")
    }

    struct FakeRoot(Data);

    #[zbus::interface(name = "org.mpris.MediaPlayer2")]
    impl FakeRoot {
        #[zbus(property)]
        fn identity(&self) -> String {
            self.0.lock().expect("fake lock").identity.clone()
        }
    }

    struct FakePlayer(Data);

    impl FakePlayer {
        fn record(&self, call: &str) {
            self.0
                .lock()
                .expect("fake lock")
                .calls
                .push(call.to_owned());
        }
    }

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl FakePlayer {
        fn play_pause(&self) {
            self.record("PlayPause");
        }

        fn next(&self) {
            self.record("Next");
        }

        fn previous(&self) {
            self.record("Previous");
        }

        // Typed `v` so a test can serve a status of the wrong type.
        #[zbus(property)]
        fn playback_status(&self) -> OwnedValue {
            clone_value(&self.0.lock().expect("fake lock").status)
        }

        #[zbus(property)]
        fn metadata(&self) -> OwnedValue {
            clone_value(&self.0.lock().expect("fake lock").metadata)
        }

        #[zbus(property)]
        fn position(&self) -> i64 {
            self.0.lock().expect("fake lock").position_us
        }
    }

    /// A fake player on the private bus, holding `name`.
    struct Fake {
        name: String,
        data: Data,
        connection: Connection,
    }

    impl Fake {
        fn calls(&self) -> Vec<String> {
            self.data.lock().expect("fake lock").calls.clone()
        }

        /// Emits `PropertiesChanged` on the player interface.
        fn changed(&self, properties: Vec<(&str, Value<'_>)>) {
            let map: HashMap<&str, Value<'_>> = properties.into_iter().collect();
            self.connection
                .emit_signal(
                    None::<&str>,
                    MPRIS_PATH,
                    PROPERTIES_INTERFACE,
                    "PropertiesChanged",
                    &(PLAYER_INTERFACE, map, Vec::<String>::new()),
                )
                .expect("the signal is sent");
        }

        fn seeked(&self, micros: i64) {
            self.connection
                .emit_signal(
                    None::<&str>,
                    MPRIS_PATH,
                    PLAYER_INTERFACE,
                    "Seeked",
                    &(micros,),
                )
                .expect("the signal is sent");
        }

        fn leave(&self) {
            let name =
                zbus::names::WellKnownName::try_from(self.name.as_str()).expect("a valid name");
            self.connection
                .release_name(name)
                .expect("the name is released");
        }
    }

    struct Fixture {
        // Field order is drop order: the worker stops before the bus goes.
        handle: Option<MediaHandle>,
        worker: Option<Worker>,
        events: Receiver<MediaEvent>,
        bus: PrivateBus,
    }

    impl Fixture {
        fn start(name: &str) -> Option<Fixture> {
            let bus = PrivateBus::start(&format!("media-{name}"))?;
            let config = MediaConfig {
                bus: MediaBus::Address(bus.address().to_owned()),
            };
            let (sent, events) = mpsc::channel();
            // A flag of the test's own: this worker's join stops nothing else.
            let shutdown = Arc::new(AtomicBool::new(false));
            let (handle, worker) = spawn(
                config,
                Box::new(super::super::NoCovers),
                move |event| sent.send(event).is_ok(),
                &shutdown,
            )
            .expect("the worker starts");
            Some(Fixture {
                handle: Some(handle),
                worker: Some(worker),
                events,
                bus,
            })
        }

        fn fake(&self, suffix: &str, data: Data) -> Fake {
            let name = format!("org.mpris.MediaPlayer2.{suffix}");
            let connection = self
                .bus
                .builder()
                .name(name.as_str())
                .expect("a valid name")
                .serve_at(MPRIS_PATH, FakeRoot(Arc::clone(&data)))
                .expect("a valid path")
                .serve_at(MPRIS_PATH, FakePlayer(Arc::clone(&data)))
                .expect("a valid path")
                .build()
                .expect("the fake player starts");
            Fake {
                name,
                data,
                connection,
            }
        }

        fn command(&self, command: MediaCommand) {
            self.handle
                .as_ref()
                .expect("the handle is kept")
                .command(command)
                .expect("the command is queued");
        }

        /// The first `Players` event that satisfies `wanted`.
        fn until(
            &self,
            wanted: impl Fn(&[PlayerView], Option<&str>) -> bool,
        ) -> (Vec<PlayerView>, Option<String>) {
            let deadline = Instant::now() + WAIT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.events.recv_timeout(left) {
                    Ok(MediaEvent::Players { players, active }) => {
                        if wanted(&players, active.as_deref()) {
                            return (players, active);
                        }
                    }
                    Ok(MediaEvent::Unavailable(reason)) => panic!("unavailable: {reason}"),
                    Ok(MediaEvent::Cover { .. }) => {}
                    Err(_) => panic!("no wanted Players event within {WAIT:?}"),
                }
            }
        }
    }

    fn named<'a>(players: &'a [PlayerView], suffix: &str) -> Option<&'a PlayerView> {
        let name = format!("org.mpris.MediaPlayer2.{suffix}");
        players.iter().find(|p| p.bus_name == name)
    }

    fn wait_for(what: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            if what() {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn an_empty_bus_is_published_once_read() {
        let Some(fixture) = Fixture::start("empty") else {
            return;
        };
        let (players, active) = fixture.until(|_, _| true);
        assert!(players.is_empty());
        assert_eq!(active, None);
    }

    #[test]
    fn a_player_appears_changes_seeks_and_leaves() {
        let Some(fixture) = Fixture::start("lifecycle") else {
            return;
        };
        fixture.until(|players, _| players.is_empty());
        let fake = fixture.fake("fake", data("Paused", "First"));
        let (players, active) = fixture.until(|players, _| !players.is_empty());
        let view = named(&players, "fake").expect("the fake is tracked");
        assert_eq!(view.identity, "Fake Player");
        assert_eq!(view.title, "First");
        assert_eq!(view.artist, "Someone");
        assert!(!view.playing);
        assert_eq!(view.length_ms, 300_000);
        assert_eq!(view.position_ms, 0);
        assert_eq!(active.as_deref(), Some(fake.name.as_str()));

        fake.changed(vec![(
            "Metadata",
            dict(vec![("xesam:title", Value::from("Second"))]),
        )]);
        fixture.until(|players, _| named(players, "fake").is_some_and(|p| p.title == "Second"));

        fake.seeked(42_000_000);
        fixture.until(|players, _| named(players, "fake").is_some_and(|p| p.position_ms == 42_000));

        fake.leave();
        fixture.until(|players, active| players.is_empty() && active.is_none());
    }

    #[test]
    fn a_player_already_on_the_bus_is_found_at_start() {
        let Some(fixture_bus) = PrivateBus::start("media-present") else {
            return;
        };
        // Started before the worker: found by the first read, not a signal.
        let data = data("Playing", "Early");
        let connection = fixture_bus
            .builder()
            .name("org.mpris.MediaPlayer2.early")
            .expect("a valid name")
            .serve_at(MPRIS_PATH, FakeRoot(Arc::clone(&data)))
            .expect("a valid path")
            .serve_at(MPRIS_PATH, FakePlayer(Arc::clone(&data)))
            .expect("a valid path")
            .build()
            .expect("the fake starts");
        let (sent, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_handle, worker) = spawn(
            MediaConfig {
                bus: MediaBus::Address(fixture_bus.address().to_owned()),
            },
            Box::new(super::super::NoCovers),
            move |event| sent.send(event).is_ok(),
            &shutdown,
        )
        .expect("the worker starts");
        match events.recv_timeout(WAIT) {
            Ok(MediaEvent::Players { players, active }) => {
                assert_eq!(players.len(), 1);
                assert_eq!(players[0].title, "Early");
                assert!(players[0].playing);
                assert_eq!(active.as_deref(), Some("org.mpris.MediaPlayer2.early"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(worker.join_within(Duration::from_secs(3)));
        drop(connection);
    }

    #[test]
    fn a_playing_track_advances_once_a_second_and_a_paused_one_is_silent() {
        let Some(fixture) = Fixture::start("progress") else {
            return;
        };
        let fake = fixture.fake("clock", data("Paused", "Tick"));
        fixture.until(|players, _| named(players, "clock").is_some());
        // Paused: nothing leaves the worker.
        assert!(fixture
            .events
            .recv_timeout(Duration::from_millis(1500))
            .is_err());

        fake.changed(vec![("PlaybackStatus", Value::from("Playing"))]);
        fixture.until(|players, _| named(players, "clock").is_some_and(|p| p.playing));
        let started = Instant::now();
        let mut positions = Vec::new();
        while started.elapsed() < Duration::from_millis(2600) {
            if let Ok(MediaEvent::Players { players, .. }) =
                fixture.events.recv_timeout(Duration::from_millis(100))
            {
                positions.push(named(&players, "clock").map_or(-2, |p| p.position_ms));
            }
        }
        assert!(
            (2..=3).contains(&positions.len()),
            "one event per second, not per poll: {positions:?}"
        );
        assert!(positions.iter().all(|p| p % 1000 == 0 && *p > 0));
        assert!(positions.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn hostile_metadata_is_bounded_and_wrong_types_degrade() {
        let Some(fixture) = Fixture::start("hostile") else {
            return;
        };
        let hostile = data("Playing", "");
        {
            let mut d = hostile.lock().expect("fake lock");
            let many: Vec<String> = (0..100).map(|i| format!("artist {i}")).collect();
            d.metadata = owned(dict(vec![
                ("xesam:title", Value::from("t".repeat(10 * 1024 * 1024))),
                ("xesam:artist", Value::from(many)),
                ("xesam:album", Value::U64(9)),
                ("mpris:length", Value::from("forever")),
            ]));
            d.identity = "i".repeat(100_000);
        }
        let fake = fixture.fake("hostile", hostile);
        let (players, _) = fixture.until(|players, _| named(players, "hostile").is_some());
        let view = named(&players, "hostile").expect("tracked");
        assert_eq!(view.title.len(), MAX_TEXT_BYTES);
        assert!(view.artist.len() <= MAX_TEXT_BYTES && view.artist.starts_with("artist 0"));
        assert_eq!(view.album, "");
        assert_eq!(view.length_ms, -1);
        assert_eq!(view.identity.len(), MAX_TEXT_BYTES);

        // A status of the wrong type, then metadata that is not a dict.
        fake.data.lock().expect("fake lock").status = owned(Value::U32(3));
        fake.changed(vec![
            ("PlaybackStatus", Value::U32(3)),
            ("Metadata", Value::from("nope")),
        ]);
        let (players, _) = fixture.until(|players, _| {
            named(players, "hostile").is_some_and(|p| !p.playing && p.title.is_empty())
        });
        assert_eq!(named(&players, "hostile").map(|p| p.length_ms), Some(-1));
    }

    #[test]
    fn the_seventeenth_player_is_ignored_until_a_slot_frees() {
        let Some(fixture) = Fixture::start("cap") else {
            return;
        };
        let fakes: Vec<Fake> = (0..MAX_PLAYERS)
            .map(|i| fixture.fake(&format!("p{i:02}"), data("Paused", "x")))
            .collect();
        fixture.until(|players, _| players.len() == MAX_PLAYERS);
        let extra = fixture.fake("zz17", data("Paused", "late"));
        // The seventeenth never shows while the others stay.
        let deadline = Instant::now() + Duration::from_millis(800);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Ok(MediaEvent::Players { players, .. }) = fixture.events.recv_timeout(left) {
                assert!(players.len() <= MAX_PLAYERS);
                assert!(named(&players, "zz17").is_none());
            }
        }
        fakes[0].leave();
        let (players, _) = fixture.until(|players, _| named(players, "zz17").is_some());
        assert_eq!(players.len(), MAX_PLAYERS);
        assert!(named(&players, "p00").is_none());
        drop(extra);
    }

    #[test]
    fn transport_commands_reach_the_shown_player() {
        let Some(fixture) = Fixture::start("commands") else {
            return;
        };
        let fake = fixture.fake("target", data("Playing", "Song"));
        fixture.until(|players, _| named(players, "target").is_some());
        fixture.command(MediaCommand::PlayPause);
        fixture.command(MediaCommand::Next);
        fixture.command(MediaCommand::Previous);
        assert!(
            wait_for(|| fake.calls() == ["PlayPause", "Next", "Previous"]),
            "calls: {:?}",
            fake.calls()
        );
    }

    #[test]
    fn a_pin_wins_carries_the_commands_and_drops_when_its_player_leaves() {
        let Some(fixture) = Fixture::start("pin") else {
            return;
        };
        let playing = fixture.fake("aplaying", data("Playing", "A"));
        let paused = fixture.fake("bpaused", data("Paused", "B"));
        fixture.until(|players, active| {
            players.len() == 2 && active == Some("org.mpris.MediaPlayer2.aplaying")
        });

        fixture.command(MediaCommand::Pin(Some(paused.name.clone())));
        let (players, _) =
            fixture.until(|_, active| active == Some("org.mpris.MediaPlayer2.bpaused"));
        assert!(named(&players, "bpaused").is_some_and(|p| p.pinned));
        assert!(named(&players, "aplaying").is_some_and(|p| !p.pinned));

        fixture.command(MediaCommand::Next);
        assert!(wait_for(|| paused.calls() == ["Next"]));
        assert!(playing.calls().is_empty());

        // A pin to a name that is not tracked changes nothing.
        fixture.command(MediaCommand::Pin(Some(
            "org.mpris.MediaPlayer2.nobody".to_owned(),
        )));
        paused.leave();
        let (players, _) = fixture.until(|players, active| {
            players.len() == 1 && active == Some("org.mpris.MediaPlayer2.aplaying")
        });
        assert!(players.iter().all(|p| !p.pinned));

        // The pin does not come back with the player.
        let again = fixture.fake("bpaused", data("Paused", "B"));
        let (players, active) = fixture.until(|players, _| players.len() == 2);
        assert_eq!(active.as_deref(), Some("org.mpris.MediaPlayer2.aplaying"));
        assert!(players.iter().all(|p| !p.pinned));
        drop(again);
    }

    #[test]
    fn a_command_with_no_player_is_refused_with_a_typed_error() {
        let Some(bus) = PrivateBus::start("media-refuse") else {
            return;
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut session = Session::open(
            &MediaConfig {
                bus: MediaBus::Address(bus.address().to_owned()),
            },
            &shutdown,
        )
        .expect("the session opens");
        assert!(matches!(
            session.perform(&MediaCommand::PlayPause),
            Err(CommandError::NoPlayer)
        ));
        assert!(matches!(
            session.perform(&MediaCommand::Next),
            Err(CommandError::NoPlayer)
        ));
        assert!(matches!(
            session.perform(&MediaCommand::Pin(Some(
                "org.mpris.MediaPlayer2.x".to_owned()
            ))),
            Err(CommandError::UnknownPlayer(_))
        ));
        assert!(session.perform(&MediaCommand::Pin(None)).is_ok());
        drop(session);
        drop(bus);
    }

    #[test]
    fn an_unreachable_bus_is_published_as_unavailable() {
        let dir = std::env::temp_dir().join(format!("celestina-media-none-{}", std::process::id()));
        let config = MediaConfig {
            bus: MediaBus::Address(format!("unix:path={}/no-bus", dir.display())),
        };
        let (sent, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (handle, worker) = spawn(
            config,
            Box::new(super::super::NoCovers),
            move |event| sent.send(event).is_ok(),
            &shutdown,
        )
        .expect("the worker starts");
        assert!(matches!(
            events.recv_timeout(WAIT),
            Ok(MediaEvent::Unavailable(reason)) if !reason.is_empty()
        ));
        // Commands while unavailable are taken and refused, not queued up.
        assert!(handle.command(MediaCommand::PlayPause).is_ok());
        let started = Instant::now();
        assert!(worker.join_within(Duration::from_secs(2)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            handle.command(MediaCommand::Next),
            Err(super::super::RequestError::Stopped)
        );
    }

    #[test]
    fn the_worker_joins_within_its_bound() {
        let Some(mut fixture) = Fixture::start("join") else {
            return;
        };
        let _fake = fixture.fake("busy", data("Playing", "x"));
        fixture.until(|players, _| !players.is_empty());
        let worker = fixture.worker.take().expect("the worker is kept");
        let started = Instant::now();
        assert!(worker.join_within(Duration::from_secs(2)));
        assert!(started.elapsed() < Duration::from_millis(800));
    }

    #[test]
    fn a_command_that_waited_too_long_is_dropped() {
        let now = Instant::now();
        let queued = |age: Duration| Queued {
            command: MediaCommand::Next,
            queued_at: now.checked_sub(age).unwrap_or(now),
        };
        assert_eq!(fresh(queued(Duration::ZERO), now), Some(MediaCommand::Next));
        assert_eq!(
            fresh(queued(COMMAND_MAX_AGE), now),
            Some(MediaCommand::Next)
        );
        let stale = queued(COMMAND_MAX_AGE + Duration::from_millis(1));
        assert_eq!(fresh(stale, now), None);
    }

    #[test]
    fn a_properties_changed_from_a_non_owner_is_ignored() {
        let Some(fixture) = Fixture::start("spoof") else {
            return;
        };
        let real = fixture.fake("real", data("Paused", "Real"));
        fixture.until(|players, _| named(players, "real").is_some_and(|p| p.title == "Real"));
        // Another connection speaks at the players' path: not an owner.
        let spoofer = fixture.bus.builder().build().expect("a spoofer");
        let spoof: HashMap<&str, Value<'_>> = HashMap::from([(
            "Metadata",
            dict(vec![("xesam:title", Value::from("Spoof"))]),
        )]);
        spoofer
            .emit_signal(
                None::<&str>,
                MPRIS_PATH,
                PROPERTIES_INTERFACE,
                "PropertiesChanged",
                &(PLAYER_INTERFACE, spoof, Vec::<String>::new()),
            )
            .expect("the spoof is sent");
        // The owner's own signal, sent after, still counts: signals flow.
        real.changed(vec![(
            "Metadata",
            dict(vec![("xesam:title", Value::from("After"))]),
        )]);
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match fixture.events.recv_timeout(left) {
                Ok(MediaEvent::Players { players, .. }) => {
                    let title = named(&players, "real").map(|p| p.title.clone());
                    assert_ne!(title.as_deref(), Some("Spoof"));
                    if title.as_deref() == Some("After") {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("the owner's change never arrived"),
            }
        }
    }

    /// A player whose `Player` properties never answer until released.
    struct Silent(Arc<AtomicBool>);

    impl Silent {
        fn hang(&self) {
            let until = Instant::now() + Duration::from_secs(10);
            while !self.0.load(Ordering::Acquire) && Instant::now() < until {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl Silent {
        #[zbus(property)]
        fn playback_status(&self) -> String {
            self.hang();
            "Playing".to_owned()
        }
    }

    /// Releases a [`Silent`] player when dropped.
    struct Release(Arc<AtomicBool>);

    impl Drop for Release {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[test]
    fn a_player_that_never_answers_does_not_stall_the_others() {
        let Some(mut fixture) = Fixture::start("silent") else {
            return;
        };
        fixture.until(|players, _| players.is_empty());
        let released = Arc::new(AtomicBool::new(false));
        // Dropped last, after `_release` let its getter return.
        let _silent = fixture
            .bus
            .builder()
            .name("org.mpris.MediaPlayer2.asilent")
            .expect("a valid name")
            .serve_at(MPRIS_PATH, Silent(Arc::clone(&released)))
            .expect("a valid path")
            .build()
            .expect("the silent player starts");
        let _release = Release(Arc::clone(&released));
        let started = Instant::now();
        let _good = fixture.fake("good", data("Paused", "Good"));
        let (players, _) = fixture.until(|players, _| {
            named(players, "good").is_some_and(|p| p.title == "Good")
                && named(players, "asilent").is_some()
        });
        let took = started.elapsed();
        assert!(
            took < READ_TIMEOUT * 3,
            "the silent player cost {took:?}, not one read bound"
        );
        // Tracked with the defaults: its identity was not asked.
        let view = named(&players, "asilent").expect("tracked");
        assert_eq!(view.identity, "asilent");
        assert!(!view.playing);

        let worker = fixture.worker.take().expect("the worker is kept");
        let stopping = Instant::now();
        assert!(worker.join_within(Duration::from_secs(2)));
        assert!(stopping.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_silent_player_that_keeps_signalling_does_not_stall_the_others() {
        let Some(fixture) = Fixture::start("chatty") else {
            return;
        };
        fixture.until(|players, _| players.is_empty());
        let released = Arc::new(AtomicBool::new(false));
        // Two names on one silent connection: every read of either hangs.
        let chatty = fixture
            .bus
            .builder()
            .name("org.mpris.MediaPlayer2.chatty1")
            .expect("a valid name")
            .serve_at(MPRIS_PATH, Silent(Arc::clone(&released)))
            .expect("a valid path")
            .build()
            .expect("the silent player starts");
        let second =
            zbus::names::WellKnownName::try_from("org.mpris.MediaPlayer2.chatty2").expect("a name");
        chatty.request_name(second).expect("the name is taken");
        let _release = Release(Arc::clone(&released));
        // It keeps flipping its status and invalidating its metadata.
        let stop = Arc::new(AtomicBool::new(false));
        let talker = {
            let connection = chatty.clone();
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let mut sent: u32 = 0;
                while !stop.load(Ordering::Acquire) {
                    // Mostly playing, so the worker's look sees it playing.
                    let playing = sent % 4 != 3;
                    sent = sent.wrapping_add(1);
                    let status = if playing { "Playing" } else { "Paused" };
                    let changed = HashMap::from([("PlaybackStatus", Value::from(status))]);
                    let _ = connection.emit_signal(
                        None::<&str>,
                        MPRIS_PATH,
                        PROPERTIES_INTERFACE,
                        "PropertiesChanged",
                        &(PLAYER_INTERFACE, changed, vec!["Metadata".to_owned()]),
                    );
                    thread::sleep(Duration::from_millis(50));
                }
            })
        };
        fixture.until(|players, _| {
            named(players, "chatty1").is_some() && named(players, "chatty2").is_some()
        });
        // Its signals still say what it is doing.
        fixture.until(|players, _| named(players, "chatty1").is_some_and(|p| p.playing));
        let started = Instant::now();
        let good = fixture.fake("good", data("Paused", "Good"));
        fixture.until(|players, _| named(players, "good").is_some_and(|p| p.title == "Good"));
        let took = started.elapsed();
        assert!(
            took < READ_TIMEOUT * 3,
            "the signalling silent player cost {took:?}"
        );
        fixture.command(MediaCommand::Pin(Some(good.name.clone())));
        fixture.command(MediaCommand::PlayPause);
        fixture.command(MediaCommand::Next);
        fixture.command(MediaCommand::Previous);
        assert!(
            wait_for(|| good.calls() == ["PlayPause", "Next", "Previous"]),
            "calls: {:?}",
            good.calls()
        );
        stop.store(true, Ordering::Release);
        talker.join().expect("the talker ends");
    }

    /// A player whose first read answers after [`READ_TIMEOUT`], and every
    /// later one at once.
    struct SlowOnce(Arc<std::sync::atomic::AtomicUsize>);

    #[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
    impl SlowOnce {
        #[zbus(property)]
        fn playback_status(&self) -> String {
            if self.0.fetch_add(1, Ordering::AcqRel) == 0 {
                thread::sleep(READ_TIMEOUT + Duration::from_millis(300));
            }
            "Playing".to_owned()
        }

        #[zbus(property)]
        fn metadata(&self) -> OwnedValue {
            owned(dict(vec![("xesam:title", Value::from("Late"))]))
        }
    }

    #[test]
    fn a_player_slow_only_at_first_is_read_once_its_quarantine_ends() {
        let Some(bus) = PrivateBus::start("media-slow-once") else {
            return;
        };
        let name = "org.mpris.MediaPlayer2.slow";
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let slow = bus
            .builder()
            .name(name)
            .expect("a valid name")
            .serve_at(MPRIS_PATH, SlowOnce(Arc::clone(&reads)))
            .expect("a valid path")
            .build()
            .expect("the slow player starts");
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut session = Session::open(
            &MediaConfig {
                bus: MediaBus::Address(bus.address().to_owned()),
            },
            &shutdown,
        )
        .expect("the session opens");
        let index = session.index_of(name).expect("tracked");
        assert!(session.players[index].unread);
        assert!(!session.players[index].player.playing);
        assert_eq!(reads.load(Ordering::Acquire), 1);

        // Quarantined: a tick does not read it.
        session.retry_unread(&shutdown);
        assert!(session.players[index].unread);

        // Its name changing hands buys no early read either.
        let owner = slow.unique_name().expect("a unique name").to_string();
        session.leave(name);
        let started = Instant::now();
        session.appear(name, &owner, &shutdown);
        assert!(started.elapsed() < READ_TIMEOUT);
        let index = session.index_of(name).expect("tracked again");
        assert!(session.players[index].unread);
        assert_eq!(reads.load(Ordering::Acquire), 1);

        // Once the quarantine ends (and the late answer is gone), one tick
        // reads it.
        thread::sleep(Duration::from_millis(400));
        session.players[index].quarantined_until = Some(Instant::now());
        session.retry_unread(&shutdown);
        let tracked = &session.players[index];
        assert!(!tracked.unread);
        assert!(tracked.player.playing);
        assert_eq!(tracked.player.track.title, "Late");
        drop(session);
        drop(slow);
    }

    #[test]
    fn one_connection_has_at_most_two_names_tracked() {
        let Some(fixture) = Fixture::start("owner-cap") else {
            return;
        };
        fixture.until(|players, _| players.is_empty());
        let data = data("Paused", "Many");
        let names = [
            "org.mpris.MediaPlayer2.m1",
            "org.mpris.MediaPlayer2.m2",
            "org.mpris.MediaPlayer2.m3",
        ];
        let many = fixture
            .bus
            .builder()
            .name(names[0])
            .expect("a valid name")
            .serve_at(MPRIS_PATH, FakeRoot(Arc::clone(&data)))
            .expect("a valid path")
            .serve_at(MPRIS_PATH, FakePlayer(Arc::clone(&data)))
            .expect("a valid path")
            .build()
            .expect("the player starts");
        for name in &names[1..] {
            let name = zbus::names::WellKnownName::try_from(*name).expect("a valid name");
            many.request_name(name).expect("the name is taken");
        }
        fixture.until(|players, _| players.len() == 2);
        let deadline = Instant::now() + Duration::from_millis(800);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Ok(MediaEvent::Players { players, .. }) = fixture.events.recv_timeout(left) {
                assert!(players.len() <= MAX_NAMES_PER_OWNER, "{players:?}");
            }
        }
        // One name goes: the waiting third takes its slot.
        let first = zbus::names::WellKnownName::try_from(names[0]).expect("a valid name");
        many.release_name(first).expect("the name is released");
        let (players, _) =
            fixture.until(|players, _| players.len() == 2 && named(players, "m1").is_none());
        assert!(named(&players, "m2").is_some() && named(&players, "m3").is_some());
    }

    #[test]
    fn the_shell_claims_no_name_on_the_bus() {
        let Some(fixture) = Fixture::start("noname") else {
            return;
        };
        fixture.until(|_, _| true);
        let probe = fixture.bus.builder().build().expect("a probe connection");
        let dbus = DBusProxy::new(&probe).expect("the bus proxy");
        let names = dbus.list_names().expect("the names");
        assert!(names
            .iter()
            .all(|name| name.as_str().starts_with(':') || name.as_str() == "org.freedesktop.DBus"));
    }
}
