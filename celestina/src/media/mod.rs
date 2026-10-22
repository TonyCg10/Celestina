//! Media (CEL-2-D): the MPRIS client worker and the covers it decodes.
//!
//! [`mpris`] owns a session-bus connection of its own, follows the players
//! on it and publishes [`MediaEvent`]s; the loop sends it [`MediaCommand`]s
//! through a [`MediaHandle`]. Which player is shown is the pure core's rule
//! (`celestina_shell::media::resolve`): the worker only feeds it.
//!
//! Covers are not decoded by the worker itself. It hands the players' raw
//! `mpris:artUrl` values and the shown player to a [`CoverSource`] on its
//! own thread ([`covers::Covers`] in production), and publishes whatever
//! events that source returns.

use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Instant;

pub mod control;
pub mod cover;
pub mod covers;
pub mod mpris;

/// What the loop asks of the shown player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    /// `Previous` on the shown player.
    Previous,
    /// `PlayPause` on the shown player.
    PlayPause,
    /// `Next` on the shown player.
    Next,
    /// Pins the player with this bus name for the session (`None` unpins).
    /// A name that is not a tracked player is refused; the pin drops when
    /// the pinned player leaves the bus.
    Pin(Option<String>),
}

/// One player as the page and the piece show it: bounded, display-ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerView {
    /// The player's well-known name, `org.mpris.MediaPlayer2.vlc`.
    pub bus_name: String,
    /// What the player calls itself, or its bus name's last segment.
    pub identity: String,
    /// Whether it says it is playing.
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The player's raw `mpris:artUrl` (empty when absent, wrong-typed or
    /// longer than [`mpris::MAX_ART_URL_BYTES`]). Whether it names a file
    /// the shell opens is the cover step's decision.
    pub art_url: String,
    /// Milliseconds, or `-1` when unknown.
    pub length_ms: i64,
    /// Milliseconds reached, floored to the whole second (the precision the
    /// page shows), or `-1` when unknown.
    pub position_ms: i64,
    /// Whether this is the pinned player.
    pub pinned: bool,
}

/// What the worker publishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaEvent {
    /// The players now on the bus, sorted by bus name, and the bus name of
    /// the one shown (`None` when there is none). Published once the bus
    /// was first read, and again whenever any of it changed.
    Players {
        players: Vec<PlayerView>,
        active: Option<String>,
    },
    /// The session bus cannot be followed; the reason is bounded. The
    /// worker tries again later.
    Unavailable(String),
    /// The cover of one player's `art_url`. `cover` is `None` when there is
    /// none: the url was refused or failed to decode, or (with an empty
    /// `art_url`) the player left the bus or lost its url.
    Cover {
        bus_name: String,
        art_url: String,
        cover: Option<Arc<cover::Cover>>,
    },
}

/// Turns players' art urls into cover events, on the media worker's thread
/// (never the loop's).
///
/// [`CoverSource::update`] is called whenever the set of
/// `(bus_name, art_url)` pairs or the shown player changed, with the
/// players just published; [`CoverSource::poll`] on every turn of the
/// worker, so work the source deferred is done once it is due. Either may
/// block for a bounded decode; whatever they return is published in order.
pub trait CoverSource: Send {
    /// The events the new art urls produce. `shown` is the bus name of the
    /// player the page and the piece show (the pinned one, else the
    /// active one), the only one worth a decode.
    fn update(&mut self, players: &[PlayerView], shown: Option<&str>) -> Vec<MediaEvent>;

    /// The events of deferred work that is now due. Nothing by default.
    fn poll(&mut self) -> Vec<MediaEvent> {
        Vec::new()
    }
}

/// The cover source that decodes nothing: the worker's tests, which are
/// about the bus and not about covers.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCovers;

impl CoverSource for NoCovers {
    fn update(&mut self, _players: &[PlayerView], _shown: Option<&str>) -> Vec<MediaEvent> {
        Vec::new()
    }
}

/// A command as it waits in the queue: stamped when it was queued, so the
/// worker can drop one that waited past [`mpris::COMMAND_MAX_AGE`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub command: MediaCommand,
    pub queued_at: Instant,
}

/// Why a command was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// [`mpris::COMMAND_QUEUE`] commands are waiting already.
    #[error("the media command queue is full")]
    Full,
    /// The worker has stopped.
    #[error("the media worker has stopped")]
    Stopped,
}

/// The loop's way into the media worker: queues a command, never waits.
#[derive(Clone, Debug)]
pub struct MediaHandle {
    commands: SyncSender<Queued>,
}

impl MediaHandle {
    pub(crate) fn new(commands: SyncSender<Queued>) -> Self {
        Self { commands }
    }

    /// Queues `command` for the worker, stamped now.
    ///
    /// # Errors
    ///
    /// [`RequestError`] when the queue is full or the worker has stopped.
    pub fn command(&self, command: MediaCommand) -> Result<(), RequestError> {
        let queued = Queued {
            command,
            queued_at: Instant::now(),
        };
        self.commands.try_send(queued).map_err(|err| match err {
            TrySendError::Full(_) => RequestError::Full,
            TrySendError::Disconnected(_) => RequestError::Stopped,
        })
    }
}
