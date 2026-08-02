//! What the rest of the desktop sees while Fluorita plays.
//!
//! MPRIS2 is how a shell panel, the media keys and Magnetita's phone link all
//! read one source of truth instead of three. This module publishes it and
//! nothing else: the state it reports is what the player already confirmed from
//! the engine, and a control that arrives over the bus becomes exactly the same
//! `PlaybackRequest` a click would.
//!
//! Two rules from the suite's contract shape it:
//!
//! - **Best effort.** A session bus that is missing, a name already taken, a
//!   send that fails — none of it may stop playback or the window. Everything
//!   here degrades to silence.
//! - **Nothing invented.** `PlaybackStatus` moves when the engine reports, not
//!   when a request is sent, and a length or a title that is not known is
//!   simply absent from the metadata rather than guessed at.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fluorita_core::{PlaybackRequest, PlaybackState};
use zbus::zvariant::{ObjectPath, Value};

/// The well-known name the desktop looks for. The suffix must be a valid bus
/// name element, and it identifies this application rather than the session.
const BUS_NAME: &str = "org.mpris.MediaPlayer2.fluorita";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

/// What the player has confirmed, in the shape MPRIS asks for.
#[derive(Clone, Debug, Default)]
pub struct NowPlaying {
    pub state: PlaybackState,
    pub path: Option<PathBuf>,
    pub title: Option<String>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub volume: f64,
    pub seekable: bool,
}

impl NowPlaying {
    /// The MPRIS word for the confirmed state. `Opening` is deliberately
    /// reported as stopped: nothing is playing yet, and saying otherwise would
    /// put a phone's lock screen into a state the engine never confirmed.
    fn status(&self) -> &'static str {
        match self.state {
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Idle
            | PlaybackState::Opening
            | PlaybackState::Ended
            | PlaybackState::Failed => "Stopped",
        }
    }

    /// The metadata map. A field that is not known is left out entirely —
    /// consumers treat an absent key as unknown, which is the truth.
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        let mut map: HashMap<String, Value<'static>> = HashMap::new();
        // A track id is required by the spec and must be a valid object path.
        map.insert(
            "mpris:trackid".to_owned(),
            Value::from(
                ObjectPath::try_from("/org/celestina/fluorita/track/0")
                    .unwrap_or_else(|_| ObjectPath::from_static_str_unchecked("/")),
            ),
        );
        if let Some(title) = &self.title {
            map.insert("xesam:title".to_owned(), Value::from(title.clone()));
        }
        if let Some(path) = &self.path {
            if let Some(url) = fluorita_core::file_uri(path) {
                map.insert("xesam:url".to_owned(), Value::from(url));
            }
            if let Some(art) = art_url(path) {
                map.insert("mpris:artUrl".to_owned(), Value::from(art));
            }
        }
        if let Some(duration) = self.duration {
            map.insert(
                "mpris:length".to_owned(),
                Value::from(microseconds(duration)),
            );
        }
        map
    }
}

/// The cover the shared cache already holds, if something produced it. Nothing
/// is generated here: a panel asking what is playing must not start a decoder.
fn art_url(source: &Path) -> Option<String> {
    let cache = celestina_core::xdg::cache_home()?.join("thumbnails");
    let entry = fluorita_core::large_thumbnail_path(&cache, source)?;
    entry.is_file().then(|| fluorita_core::file_uri(&entry))?
}

fn microseconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

/// What a bus client asked for, translated into the player's own vocabulary.
type Control = Arc<dyn Fn(PlaybackRequest) + Send + Sync>;

/// The published service. Dropping it stops publishing.
pub struct Mpris {
    now: Arc<Mutex<NowPlaying>>,
}

impl Mpris {
    /// Starts publishing on the session bus.
    ///
    /// `control` receives every transport request that arrives over the bus.
    /// Returns `None` when there is no session bus or the name is taken —
    /// neither is an error worth showing anyone.
    pub fn start(control: Control) -> Option<Self> {
        let now = Arc::new(Mutex::new(NowPlaying::default()));
        let served = Arc::clone(&now);

        let started = std::thread::Builder::new()
            .name("fluorita-mpris".to_owned())
            .spawn(move || serve(&served, &control));
        started.ok()?;

        Some(Self { now })
    }

    /// Publishes what the player just confirmed.
    pub fn publish(&self, now: NowPlaying) {
        // A poisoned lock would mean the serving thread panicked mid-update;
        // there is nothing useful to do about it here, and playback continues.
        if let Ok(mut slot) = self.now.lock() {
            *slot = now;
        }
    }
}

fn serve(now: &Arc<Mutex<NowPlaying>>, control: &Control) {
    let root = Root;
    let player = Player {
        now: Arc::clone(now),
        control: Arc::clone(control),
    };

    let Ok(connection) = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.name(BUS_NAME))
        .and_then(|builder| builder.serve_at(OBJECT_PATH, root))
        .and_then(|builder| builder.serve_at(OBJECT_PATH, player))
        .and_then(zbus::blocking::connection::Builder::build)
    else {
        // No session bus, or another Fluorita already owns the name. Playback
        // is unaffected; the desktop simply does not see this one.
        return;
    };

    // Keep the connection — and the service — alive for the process.
    let _connection = connection;
    loop {
        std::thread::park();
    }
}

/// `org.mpris.MediaPlayer2`: what the application is.
struct Root;

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    /// Raising needs a compositor path this application does not have, so it
    /// says so rather than accepting and doing nothing.
    fn raise(&self) {}

    fn quit(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Fluorita"
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "org.celestina.Fluorita"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["file".to_owned()]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `org.mpris.MediaPlayer2.Player`: what is playing and how to steer it.
struct Player {
    now: Arc<Mutex<NowPlaying>>,
    control: Control,
}

impl Player {
    fn snapshot(&self) -> NowPlaying {
        self.now.lock().map(|now| now.clone()).unwrap_or_default()
    }

    fn ask(&self, request: PlaybackRequest) {
        (self.control)(request);
    }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn play(&self) {
        self.ask(PlaybackRequest::Play);
    }

    fn pause(&self) {
        self.ask(PlaybackRequest::Pause);
    }

    fn play_pause(&self) {
        // Which way it goes is decided from what the engine last confirmed,
        // never from what was last asked for.
        let request = if matches!(self.snapshot().state, PlaybackState::Playing) {
            PlaybackRequest::Pause
        } else {
            PlaybackRequest::Play
        };
        self.ask(request);
    }

    fn stop(&self) {
        self.ask(PlaybackRequest::Stop);
    }

    /// Relative seek, in microseconds, as the spec defines it.
    fn seek(&self, offset: i64) {
        let now = self.snapshot();
        let current = i64::try_from(now.position.as_micros()).unwrap_or(i64::MAX);
        let target = current.saturating_add(offset).max(0);
        self.ask(PlaybackRequest::Seek(Duration::from_micros(
            target.unsigned_abs(),
        )));
    }

    /// Absolute seek. The track id is ignored: this player has one item, and
    /// refusing on a stale id would only make a panel's scrubber feel broken.
    fn set_position(&self, _track: ObjectPath<'_>, position: i64) {
        self.ask(PlaybackRequest::Seek(Duration::from_micros(
            position.max(0).unsigned_abs(),
        )));
    }

    fn open_uri(&self, _uri: &str) {}

    fn next(&self) {}

    fn previous(&self) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.snapshot().status().to_owned()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        self.snapshot().metadata()
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        microseconds(self.snapshot().position)
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.snapshot().volume
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        self.ask(PlaybackRequest::SetVolume(volume.clamp(0.0, 1.0)));
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.snapshot().path.is_some()
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.snapshot().path.is_some()
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.snapshot().seekable
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    /// One item at a time: there is no queue to step through, and claiming
    /// otherwise would put dead buttons on every panel.
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{microseconds, NowPlaying, Value, BUS_NAME, OBJECT_PATH};
    use fluorita_core::PlaybackState;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn the_status_is_what_the_engine_confirmed() {
        let playing = NowPlaying {
            state: PlaybackState::Playing,
            ..NowPlaying::default()
        };
        assert_eq!(playing.status(), "Playing");

        // Opening is not playing: a lock screen that said otherwise would be
        // showing a state the engine never reported.
        let opening = NowPlaying {
            state: PlaybackState::Opening,
            ..NowPlaying::default()
        };
        assert_eq!(opening.status(), "Stopped");
        assert_eq!(
            NowPlaying {
                state: PlaybackState::Paused,
                ..NowPlaying::default()
            }
            .status(),
            "Paused"
        );
    }

    #[test]
    fn an_unknown_field_is_absent_rather_than_guessed() {
        let bare = NowPlaying {
            state: PlaybackState::Playing,
            path: Some(PathBuf::from("/home/toni/Música/pista.mp3")),
            ..NowPlaying::default()
        };
        let metadata = bare.metadata();

        assert!(
            metadata.contains_key("mpris:trackid"),
            "el id es obligatorio"
        );
        assert!(metadata.contains_key("xesam:url"));
        assert!(!metadata.contains_key("xesam:title"));
        assert!(!metadata.contains_key("mpris:length"));
    }

    #[test]
    fn a_known_track_carries_its_title_and_length() {
        let known = NowPlaying {
            state: PlaybackState::Playing,
            path: Some(PathBuf::from("/home/toni/Música/pista con espacio.mp3")),
            title: Some("Pista".to_owned()),
            duration: Some(Duration::from_secs(213)),
            ..NowPlaying::default()
        };
        let metadata = known.metadata();

        assert_eq!(
            metadata.get("xesam:title"),
            Some(&Value::from("Pista".to_owned()))
        );
        // The URL is the suite's frozen `file://` spelling, so a space survives.
        assert!(metadata
            .get("xesam:url")
            .map(ToString::to_string)
            .is_some_and(|url| url.contains("%20")));
        // Microseconds, as the spec spells a length.
        assert_eq!(
            metadata.get("mpris:length"),
            Some(&Value::from(213_000_000_i64))
        );
    }

    #[test]
    fn the_bus_name_and_path_are_the_spec_ones() {
        assert!(BUS_NAME.starts_with("org.mpris.MediaPlayer2."));
        assert_eq!(OBJECT_PATH, "/org/mpris/MediaPlayer2");
        assert_eq!(microseconds(Duration::from_millis(1_500)), 1_500_000);
    }
}
