//! What the desktop is playing, and the cover it points at.
//!
//! MPRIS is an event source, so this listens to it rather than asking. Two
//! match rules carry everything: names appearing and disappearing under
//! `org.mpris.MediaPlayer2.*`, and whatever a player says at
//! `/org/mpris/MediaPlayer2` — `PropertiesChanged` when the track or the
//! transport moves, `Seeked` when somebody scrubs.
//!
//! It used to spawn `playerctl` on a timer: every 500 ms for the first ten
//! seconds, then every five with no player and every two with one. That is a
//! subprocess a second, all day, for a reading that changes when somebody
//! presses a key — and it made a track take seconds to appear. Nothing here
//! spawns anything now. The only clock left advances the progress bar between
//! two things a player said, which is arithmetic rather than a question, and
//! re-reads the current player occasionally so a missed signal cannot strand
//! the panel on a stale track.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::bounded;
use celestina_shell_core::media::{self, Player, Track};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId, MAX_TEXT_UNITS};
use magnetita_core::mpris::{self, MediaAction, PlaybackProgress};
use serde_json::Value;
use zbus::blocking::{Connection, MessageIterator};
use zbus::names::BusName;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

use super::tools::lock_runtime;

/// How often the progress of a playing track is recomputed. It is a redraw, not
/// a question: nothing leaves this process on a tick.
const PROGRESS_TICK: Duration = Duration::from_secs(1);
/// How often the current player is read again even though it said nothing.
///
/// A bounded backstop, not a poll: a player that crashes mid-signal, or a
/// `PropertiesChanged` this process missed while it was busy, would otherwise
/// leave the panel showing a track that ended. One `GetAll` per half minute
/// against one player is the price of that not being possible.
const RECONCILE: Duration = Duration::from_secs(30);

const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

pub const NAME: &str = "media";

/// Every player the session has, and the connection that hears them.
///
/// A module singleton for the same reason the brightness target is one: there
/// is exactly one helper process and one media provider in it, and threading a
/// bus connection through every provider's signature to reach this one would
/// put media's business in all of them.
static PLAYERS: OnceLock<Mutex<HashMap<String, Tracked>>> = OnceLock::new();
static BUS: OnceLock<Connection> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);

/// One player, as this process last heard it, and when that was.
///
/// `measured` is not part of the pure reading: it is how long ago the position
/// below was true, which only the side holding a clock can know.
struct Tracked {
    player: Player,
    measured: Instant,
}

fn players() -> &'static Mutex<HashMap<String, Tracked>> {
    PLAYERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_players() -> std::sync::MutexGuard<'static, HashMap<String, Tracked>> {
    match players().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: media: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());

    let connection = match Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            // No session bus is no media, and it is not a reason to fail the
            // whole helper: every other provider still works.
            eprintln!("celestina-provider-adapter: media: no session bus: {error}");
            return Ok(());
        }
    };
    let _ = BUS.set(connection.clone());

    let events_runtime = Arc::clone(runtime);
    let events_id = id.clone();
    let events_connection = connection.clone();
    thread::Builder::new()
        .name("media-events".to_owned())
        .spawn(move || listen(&events_connection, &events_runtime, &events_id))?;

    let progress_runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name("media-progress".to_owned())
        .spawn(move || tick(&connection, &progress_runtime, &id))?;
    Ok(())
}

/// A transport verb on whatever the panel is currently showing.
///
/// The player is asked over its own interface. Nothing is painted here: what
/// the panel shows next is whatever the player says in the `PropertiesChanged`
/// that follows, exactly as before — a request that the player ignores changes
/// nothing on screen, which is the truth.
pub fn action(verb: &str) -> Result<(), String> {
    let action = MediaAction::parse(verb)
        .ok_or_else(|| format!("'{NAME}' does not serve the verb '{verb}'"))?;
    let member = match action {
        MediaAction::Play => "Play",
        MediaAction::Pause => "Pause",
        MediaAction::PlayPause => "PlayPause",
        MediaAction::Stop => "Stop",
        MediaAction::Next => "Next",
        MediaAction::Previous => "Previous",
    };

    let connection = BUS
        .get()
        .ok_or_else(|| "media has no session bus".to_owned())?;
    let chosen = {
        let tracked = lock_players();
        let known: Vec<Player> = tracked.values().map(|entry| entry.player.clone()).collect();
        media::active(&known).map(|player| player.bus_name.clone())
    };
    let player = chosen.ok_or_else(|| "no player is running".to_owned())?;
    let destination =
        BusName::try_from(player.clone()).map_err(|_| format!("'{player}' is not a bus name"))?;

    connection
        .call_method(
            Some(destination),
            PLAYER_PATH,
            Some(PLAYER_INTERFACE),
            member,
            &(),
        )
        .map(|_| ())
        .map_err(|error| format!("{player} did not accept {member}: {error}"))
}

/// The cover a player points at, if the panel can show it.
///
/// Only a local file is accepted. A player that names an `https://` cover is
/// naming something this shell would have to download, and a shell that
/// downloads what a media player tells it to is a shell with a fetcher in it;
/// the panel simply shows no cover there. What is accepted is still checked
/// like anything else from another process: bounded in size and starting like
/// an image, before Qt ever opens it.
fn artwork_path(url: &str) -> Option<String> {
    let path = url.trim().strip_prefix("file://")?;
    if path.is_empty() || !std::path::Path::new(path).is_absolute() {
        return None;
    }

    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file()
        || !celestina_core::image::is_artwork_size(i64::try_from(metadata.len()).ok()?)
    {
        return None;
    }

    let mut header = [0_u8; celestina_core::image::IMAGE_HEADER_BYTES];
    let read = std::fs::File::open(path).ok()?.read(&mut header).ok()?;
    if !celestina_core::image::is_supported_image_header(&header[..read]) {
        return None;
    }

    Some(path.to_owned())
}

fn text_of(value: Option<&OwnedValue>) -> String {
    value
        .and_then(|value| <&str>::try_from(value).ok())
        .unwrap_or_default()
        .to_owned()
}

/// `xesam:artist` is a list; the panel shows one line, so the first name is the
/// artist and the rest belong to a tag editor.
fn first_artist(value: Option<&OwnedValue>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Ok(names) = Vec::<String>::try_from(value.clone()) {
        return names
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_default();
    }
    text_of(Some(value))
}

/// Microseconds as the specification counts them, in the milliseconds the
/// suite's vocabulary uses. Anything negative or unreadable is `-1`: unknown,
/// which is not zero.
fn millis_of(value: Option<&OwnedValue>) -> i64 {
    value
        .and_then(|value| i64::try_from(value).ok())
        .filter(|microseconds| *microseconds >= 0)
        .map_or(-1, |microseconds| microseconds / 1_000)
}

fn track_of(metadata: Option<&OwnedValue>) -> Track {
    let Some(fields) =
        metadata.and_then(|value| HashMap::<String, OwnedValue>::try_from(value.clone()).ok())
    else {
        return Track::default();
    };

    Track {
        title: text_of(fields.get("xesam:title")),
        artist: first_artist(fields.get("xesam:artist")),
        album: text_of(fields.get("xesam:album")),
        art_url: text_of(fields.get("mpris:artUrl")),
        length_ms: millis_of(fields.get("mpris:length")),
    }
}

/// Reads one player and records it. Returns false when the player did not
/// answer, which is how a name that has gone is noticed without waiting for its
/// `NameOwnerChanged`.
fn read_player(connection: &Connection, bus_name: &str) -> bool {
    let Ok(destination) = BusName::try_from(bus_name.to_owned()) else {
        return false;
    };

    let Ok(reply) = connection.call_method(
        Some(destination.clone()),
        PLAYER_PATH,
        Some(PROPERTIES_INTERFACE),
        "GetAll",
        &(PLAYER_INTERFACE,),
    ) else {
        return false;
    };
    let Ok(properties) = reply.body().deserialize::<HashMap<String, OwnedValue>>() else {
        return false;
    };

    // The player's own name for itself. A player that does not offer one is not
    // a broken player — the bus name says enough — so this never fails a read.
    let identity = connection
        .call_method(
            Some(destination),
            PLAYER_PATH,
            Some(PROPERTIES_INTERFACE),
            "Get",
            &(ROOT_INTERFACE, "Identity"),
        )
        .ok()
        .and_then(|reply| reply.body().deserialize::<OwnedValue>().ok())
        .map(|value| text_of(Some(&value)))
        .filter(|identity| !identity.is_empty())
        .unwrap_or_else(|| media::identity_from_bus_name(bus_name));

    let playing = properties
        .get("PlaybackStatus")
        .and_then(|value| <&str>::try_from(value).ok())
        .is_some_and(|status| status.eq_ignore_ascii_case("Playing"));

    let mut tracked = lock_players();
    let heard = next_heard(&tracked);
    tracked.insert(
        bus_name.to_owned(),
        Tracked {
            player: Player {
                bus_name: bus_name.to_owned(),
                identity,
                playing,
                track: track_of(properties.get("Metadata")),
                position_ms: millis_of(properties.get("Position")),
                heard,
            },
            measured: Instant::now(),
        },
    );
    true
}

/// One past the highest anybody has been heard. The counter is the ordering the
/// pure choice uses; it is never a time, so it cannot go backwards.
fn next_heard(tracked: &HashMap<String, Tracked>) -> u64 {
    tracked
        .values()
        .map(|entry| entry.player.heard)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

/// The players on the bus right now, asked once at startup. After this the
/// answer arrives as signals.
fn seed(connection: &Connection) -> Vec<String> {
    let Ok(reply) = connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "ListNames",
        &(),
    ) else {
        return Vec::new();
    };

    reply
        .body()
        .deserialize::<Vec<String>>()
        .unwrap_or_default()
        .into_iter()
        .filter(|name| media::is_player_name(name))
        .collect()
}

fn add_match_rules(connection: &Connection) -> zbus::Result<()> {
    // Names appearing and disappearing under the player namespace, and whatever
    // a player says at its own object path. Two rules cover `PropertiesChanged`
    // and `Seeked` between them, because both are signals from that path.
    let names = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg0ns("org.mpris.MediaPlayer2")?
        .build();
    let players = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path(PLAYER_PATH)?
        .build();

    let bus = zbus::blocking::fdo::DBusProxy::new(connection)?;
    bus.add_match_rule(names)?;
    bus.add_match_rule(players)?;
    Ok(())
}

/// The unique bus name behind each player. A player's own signals arrive from
/// its unique name, never the well-known one it registered, so without this a
/// `PropertiesChanged` could not be attributed to the player that sent it.
fn owner_of(connection: &Connection, bus_name: &str) -> Option<String> {
    connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetNameOwner",
            &(bus_name,),
        )
        .ok()
        .and_then(|reply| reply.body().deserialize::<String>().ok())
}

fn listen(connection: &Connection, runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    if let Err(error) = add_match_rules(connection) {
        eprintln!("celestina-provider-adapter: media: {error}");
        return;
    }

    // Every player that already exists. A shell starting while something is
    // playing is the ordinary case, not a race to lose.
    let mut owners: HashMap<String, String> = HashMap::new();
    for bus_name in seed(connection) {
        if read_player(connection, &bus_name) {
            if let Some(owner) = owner_of(connection, &bus_name) {
                owners.insert(owner, bus_name);
            }
        }
    }
    STARTED.store(true, Ordering::Release);
    publish(runtime, id);

    for message in MessageIterator::from(connection) {
        let Ok(message) = message else {
            continue;
        };
        let header = message.header();
        let member = header.member().map(|member| member.as_str().to_owned());
        let interface = header.interface().map(|name| name.as_str().to_owned());

        match (interface.as_deref(), member.as_deref()) {
            (Some("org.freedesktop.DBus"), Some("NameOwnerChanged")) => {
                let Ok((name, _old, new)) =
                    message.body().deserialize::<(String, String, String)>()
                else {
                    continue;
                };
                if !media::is_player_name(&name) {
                    continue;
                }

                if new.is_empty() {
                    lock_players().remove(&name);
                    owners.retain(|_, player| player != &name);
                } else {
                    owners.insert(new, name.clone());
                    read_player(connection, &name);
                }
            }
            (Some(PROPERTIES_INTERFACE), Some("PropertiesChanged"))
            | (Some(PLAYER_INTERFACE), Some("Seeked")) => {
                let Some(sender) = header.sender().map(|sender| sender.as_str().to_owned()) else {
                    continue;
                };
                let Some(bus_name) = owners.get(&sender).cloned() else {
                    continue;
                };
                // The signal says something moved; what it moved to is read
                // from the player rather than pieced together from the changed
                // set, which may be partial and may name properties this panel
                // does not show.
                read_player(connection, &bus_name);
            }
            _ => continue,
        }

        publish(runtime, id);
    }
}

/// Advances a playing track's progress, and occasionally asks the current
/// player what it is really doing.
fn tick(connection: &Connection, runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let mut reconciled = Instant::now();
    loop {
        thread::sleep(PROGRESS_TICK);
        if !STARTED.load(Ordering::Acquire) {
            continue;
        }

        let reconciled_now = if reconciled.elapsed() >= RECONCILE {
            reconciled = Instant::now();
            reconcile_with(|bus_name| read_player(connection, bus_name))
        } else {
            false
        };

        // Nothing is published while nothing plays: a paused panel redrawing
        // once a second is a frame per second for a picture that did not move.
        let playing = {
            let tracked = lock_players();
            tracked.values().any(|entry| entry.player.playing)
        };
        // Reconciliation publishes even when it just removed the last playing
        // peer. Otherwise the branch above would clear the internal player but
        // leave its previous payload in the complete provider frame forever —
        // exactly the stale state this backstop exists to prevent.
        if reconciled_now || playing {
            publish(runtime, id);
        }
    }
}

/// Re-reads the player the panel currently prefers.
///
/// `true` means a player was reconciled and the provider owes a publication,
/// including when that read failed and removed the last player. Taking the
/// read as a closure keeps the state transition independently testable without
/// a real bus or a thirty-second wait.
fn reconcile_with(mut read: impl FnMut(&str) -> bool) -> bool {
    let current = {
        let tracked = lock_players();
        let known: Vec<Player> = tracked.values().map(|entry| entry.player.clone()).collect();
        media::active(&known).map(|player| player.bus_name.clone())
    };
    let Some(bus_name) = current else {
        return false;
    };

    // A player that no longer answers is gone, whatever the bus has got round
    // to saying about its name. The caller publishes the resulting state.
    if !read(&bus_name) {
        lock_players().remove(&bus_name);
    }
    true
}

/// The reading the panel shows, from whichever player the pure choice picked.
fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let (chosen, elapsed_ms) = {
        let tracked = lock_players();
        let known: Vec<Player> = tracked.values().map(|entry| entry.player.clone()).collect();
        let Some(player) = media::active(&known) else {
            // No player is not a paused player: the widget goes away rather
            // than keeping the last thing that ever played. The provider stays
            // registered, so a transport command is still answered by media
            // itself — "no player is running" — instead of reading as though
            // this helper had no media provider at all.
            drop(tracked);
            lock_runtime(runtime).withdraw(id);
            return;
        };

        let elapsed = tracked
            .get(&player.bus_name)
            .map_or(0, |entry| entry.measured.elapsed().as_millis());
        (player.clone(), i64::try_from(elapsed).unwrap_or(i64::MAX))
    };

    let position = media::advanced_position(chosen.position_ms, chosen.playing, elapsed_ms);
    let now_playing = media::now_playing_line(&chosen.track.artist, &chosen.track.title);

    // A track's own text belongs to whoever is playing it and has no length any
    // player promises. Publishing it whole would make the frame refuse itself
    // over a title, so what the panel shows is a bounded prefix and the rest of
    // the bar keeps updating.
    let mut payload = Payload::new();
    payload.insert(
        "player".to_owned(),
        Value::from(bounded(&chosen.identity, MAX_TEXT_UNITS)),
    );
    payload.insert(
        "title".to_owned(),
        Value::from(bounded(&chosen.track.title, MAX_TEXT_UNITS)),
    );
    payload.insert(
        "artist".to_owned(),
        Value::from(bounded(&chosen.track.artist, MAX_TEXT_UNITS)),
    );
    payload.insert(
        "nowPlaying".to_owned(),
        Value::from(bounded(&now_playing, MAX_TEXT_UNITS)),
    );
    payload.insert("playing".to_owned(), Value::from(chosen.playing));
    // A path is not text to cut: a shortened one names a different file or
    // none. An artwork reference longer than a field may carry is therefore
    // dropped, and the panel shows the track without a picture rather than
    // losing the whole reading.
    if let Some(artwork) = artwork_path(&chosen.track.art_url)
        .filter(|path| path.encode_utf16().count() <= MAX_TEXT_UNITS)
    {
        payload.insert("artPath".to_owned(), Value::from(artwork));
    }
    match mpris::playback_progress(position, chosen.track.length_ms) {
        PlaybackProgress::Finite {
            position_ms,
            length_ms,
        } => {
            payload.insert("progress".to_owned(), Value::from("finite"));
            payload.insert("positionMs".to_owned(), Value::from(position_ms));
            payload.insert("lengthMs".to_owned(), Value::from(length_ms));
        }
        PlaybackProgress::Live => {
            payload.insert("progress".to_owned(), Value::from("live"));
        }
        PlaybackProgress::Unavailable => {
            payload.insert("progress".to_owned(), Value::from("unavailable"));
        }
    }

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: media: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        artwork_path, first_artist, lock_players, millis_of, next_heard, reconcile_with, text_of,
        track_of, Tracked,
    };

    use std::collections::HashMap;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Instant;

    use celestina_shell_core::media::{Player, Track};
    use zbus::zvariant::{OwnedValue, Value as ZValue};

    const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    static PLAYER_STATE_TEST: Mutex<()> = Mutex::new(());

    fn owned(value: ZValue<'static>) -> OwnedValue {
        OwnedValue::try_from(value).expect("a value")
    }

    /// The metadata Firefox publishes, in the shape it publishes it: the artist
    /// is a list, the length is microseconds, and the cover is a URL.
    #[test]
    fn a_players_own_metadata_becomes_the_track_the_panel_shows() {
        let mut fields: HashMap<String, OwnedValue> = HashMap::new();
        fields.insert("xesam:title".to_owned(), owned(ZValue::from("Nightcall")));
        fields.insert(
            "xesam:artist".to_owned(),
            owned(ZValue::from(vec![
                "Kavinsky".to_owned(),
                "Lovefoxxx".to_owned(),
            ])),
        );
        fields.insert("xesam:album".to_owned(), owned(ZValue::from("OutRun")));
        fields.insert(
            "mpris:length".to_owned(),
            owned(ZValue::from(258_000_000_i64)),
        );
        fields.insert(
            "mpris:artUrl".to_owned(),
            owned(ZValue::from("https://example.invalid/cover.png")),
        );

        let metadata = owned(ZValue::from(fields));
        let track = track_of(Some(&metadata));

        assert_eq!(track.title, "Nightcall");
        // One line means one artist; the rest belong to a tag editor.
        assert_eq!(track.artist, "Kavinsky");
        assert_eq!(track.album, "OutRun");
        assert_eq!(track.length_ms, 258_000);
        assert_eq!(track.art_url, "https://example.invalid/cover.png");
    }

    #[test]
    fn a_player_that_says_nothing_leaves_an_empty_track_rather_than_a_wrong_one() {
        let track = track_of(None);

        assert!(track.title.is_empty());
        assert!(track.artist.is_empty());
        // Unknown, which is not a length of zero.
        assert_eq!(track.length_ms, -1);
    }

    #[test]
    fn an_unreadable_number_is_unknown_rather_than_zero() {
        assert_eq!(millis_of(None), -1);
        assert_eq!(millis_of(Some(&owned(ZValue::from(-1_i64)))), -1);
        assert_eq!(millis_of(Some(&owned(ZValue::from("soon")))), -1);
        assert_eq!(millis_of(Some(&owned(ZValue::from(1_500_i64)))), 1);
    }

    #[test]
    fn an_artist_field_of_the_wrong_shape_is_read_as_far_as_it_can_be() {
        // Some players publish a plain string where the specification says a
        // list. Refusing it would lose a name the panel could have shown.
        assert_eq!(
            first_artist(Some(&owned(ZValue::from("Kavinsky")))),
            "Kavinsky"
        );
        assert_eq!(first_artist(None), "");
        assert_eq!(
            first_artist(Some(&owned(ZValue::from(Vec::<String>::new())))),
            ""
        );
        assert_eq!(text_of(None), "");
    }

    /// The order the pure choice uses only means anything if it keeps moving.
    #[test]
    fn each_player_heard_from_is_ordered_after_every_earlier_one() {
        let mut tracked: HashMap<String, Tracked> = HashMap::new();
        assert_eq!(next_heard(&tracked), 1);

        tracked.insert(
            "org.mpris.MediaPlayer2.vlc".to_owned(),
            Tracked {
                player: Player {
                    bus_name: "org.mpris.MediaPlayer2.vlc".to_owned(),
                    identity: "vlc".to_owned(),
                    playing: false,
                    track: Track::default(),
                    position_ms: -1,
                    heard: 7,
                },
                measured: Instant::now(),
            },
        );
        assert_eq!(next_heard(&tracked), 8);
    }

    #[test]
    fn an_unreadable_reconciled_player_is_removed_and_owed_as_a_change() {
        let _serial = PLAYER_STATE_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let bus_name = "org.mpris.MediaPlayer2.unreadable";
        {
            let mut tracked = lock_players();
            tracked.clear();
            tracked.insert(
                bus_name.to_owned(),
                Tracked {
                    player: Player {
                        bus_name: bus_name.to_owned(),
                        identity: "Unreadable".to_owned(),
                        playing: true,
                        track: Track::default(),
                        position_ms: 12_000,
                        heard: 1,
                    },
                    measured: Instant::now(),
                },
            );
        }

        assert!(reconcile_with(|name| {
            assert_eq!(name, bus_name);
            false
        }));
        assert!(lock_players().is_empty());
    }

    /// A file under the test runner's own temporary directory, named for the
    /// case that made it, removed when the case is done.
    struct Fixture {
        path: PathBuf,
    }

    impl Fixture {
        fn with(name: &str, bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("celestina-art-{name}"));
            let mut file = fs::File::create(&path).expect("a writable temporary file");
            file.write_all(bytes).expect("written");
            Self { path }
        }

        fn sized(name: &str, len: u64) -> Self {
            let path = std::env::temp_dir().join(format!("celestina-art-{name}"));
            let file = fs::File::create(&path).expect("a writable temporary file");
            file.set_len(len).expect("sized");
            Self { path }
        }

        fn url(&self) -> String {
            format!("file://{}", self.path.display())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn a_local_cover_that_starts_like_an_image_is_shown() {
        let cover = Fixture::with("cover.png", PNG_HEADER);

        assert_eq!(
            artwork_path(&cover.url()).as_deref(),
            Some(cover.path.to_str().expect("utf-8 path"))
        );
    }

    #[test]
    fn a_file_that_only_claims_to_be_a_cover_is_refused() {
        // A renamed archive is the classic thing to hand an image decoder.
        let liar = Fixture::with("liar.png", b"PK\x03\x04 a zip pretending to be art");

        assert_eq!(artwork_path(&liar.url()), None);
    }

    #[test]
    fn a_cover_the_panel_would_have_to_download_is_not_shown() {
        // Naming an `https://` cover asks this shell to fetch it. It will not:
        // there is no fetcher here, and the widget simply shows no cover.
        assert_eq!(
            artwork_path("https://i.scdn.co/image/ab67616d0000b273"),
            None
        );
        assert_eq!(artwork_path("data:image/png;base64,iVBORw0KGgo="), None);
        assert_eq!(artwork_path(""), None);
    }

    #[test]
    fn a_relative_or_missing_path_is_refused() {
        assert_eq!(artwork_path("file://cover.png"), None);
        assert_eq!(
            artwork_path("file:///nowhere/celestina-no-such-cover.png"),
            None
        );
    }

    #[test]
    fn a_file_sized_cover_is_refused_before_it_is_read() {
        let huge = Fixture::sized(
            "huge.png",
            u64::try_from(celestina_core::image::MAX_ARTWORK_BYTES).expect("positive") + 1,
        );
        let empty = Fixture::with("empty.png", b"");

        assert_eq!(artwork_path(&huge.url()), None);
        assert_eq!(artwork_path(&empty.url()), None);
    }
}
