//! The media provider against a real bus and a real MPRIS player.
//!
//! The provider used to spawn `playerctl` on a timer, and the two things that
//! change with signals are exactly the two a unit test cannot show:
//!
//! - a player that was **already** on the bus when the helper started is found
//!   without anything polling for it;
//! - a player that says something afterwards — a pause, a new track, a name
//!   that goes away — reaches the panel because it said so, not because a clock
//!   came round.
//!
//! The bus is private and started here, so nothing in this test can reach the
//! author's session or see what they are really playing.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value as ZValue};

const PLAYER_NAME: &str = "org.mpris.MediaPlayer2.celestinatest";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";
/// Generous against process start plus a bus round trip, and still bounded: a
/// test that waits forever is a test that hangs a pipeline.
const DEADLINE: Duration = Duration::from_secs(15);
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

/// A private session bus, torn down with the test whatever it fails on.
struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Option<Self> {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--print-address", "--nofork", "--nopidfile"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdout = daemon.stdout.take()?;
        let mut address = String::new();
        BufReader::new(stdout).read_line(&mut address).ok()?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = daemon.kill();
            return None;
        }
        Some(Self { daemon, address })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// A player that publishes exactly what the specification says and nothing
/// else. It is deliberately minimal: the point is that the provider works from
/// what a player really offers, not from what one particular player offers.
struct FakePlayer {
    playing: bool,
    title: String,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl FakePlayer {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        if self.playing { "Playing" } else { "Paused" }.to_owned()
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        // A fixed twelve seconds in: the provider's own arithmetic moves it,
        // and a position that changed by itself would hide that.
        let _ = &self.title;
        12_000_000
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "xesam:title".to_owned(),
            OwnedValue::try_from(ZValue::from(self.title.clone())).expect("a title"),
        );
        metadata.insert(
            "xesam:artist".to_owned(),
            OwnedValue::try_from(ZValue::from(vec!["Kavinsky".to_owned()])).expect("an artist"),
        );
        metadata.insert(
            "mpris:length".to_owned(),
            OwnedValue::try_from(ZValue::from(258_000_000_i64)).expect("a length"),
        );
        metadata
    }
}

/// One provider helper, with its stdin held open so it does not exit, and its
/// frames readable.
struct Helper {
    process: Child,
    frames: BufReader<ChildStdout>,
    config_directory: PathBuf,
}

impl Helper {
    fn start(address: &str) -> Self {
        // A helper restores the person's persisted session choices. Integration
        // tests must never inherit those.
        let config_directory = std::env::temp_dir().join(format!(
            "celestina-media-test-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&config_directory).expect("the test config directory exists");
        let mut process = Command::new(env!("CARGO_BIN_EXE_celestina-provider-adapter"))
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env("XDG_CONFIG_HOME", &config_directory)
            // Connecting this helper to the author's real compositor would
            // import live clipboard frames into a test about media.
            .env("WAYLAND_DISPLAY", "celestina-test-no-wayland")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the provider helper starts");
        let frames = BufReader::new(process.stdout.take().expect("the helper's frames"));
        Self {
            process,
            frames,
            config_directory,
        }
    }

    /// Reads frames until one satisfies `wanted`, or the deadline passes.
    fn wait_for<T>(&mut self, mut wanted: impl FnMut(&Value) -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + DEADLINE;
        let mut line = String::new();
        while Instant::now() < deadline {
            line.clear();
            if self.frames.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(found) = wanted(&frame) {
                return Some(found);
            }
        }
        None
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = fs::remove_dir_all(&self.config_directory);
    }
}

/// The `media` provider's published payload, if the frame carries one.
fn media(frame: &Value) -> Option<&Value> {
    frame.get("providers")?.get("media")
}

fn serve_player(address: &str, playing: bool, title: &str) -> Connection {
    zbus::blocking::connection::Builder::address(address)
        .expect("a connection to the private bus")
        .name(PLAYER_NAME)
        .expect("the player name is free")
        .serve_at(
            PLAYER_PATH,
            FakePlayer {
                playing,
                title: title.to_owned(),
            },
        )
        .expect("the player object is exported")
        .build()
        .expect("the player connects")
}

/// The whole migration in one case: a player already on the bus is found, what
/// it says next reaches the panel, and its leaving takes the widget with it.
#[test]
fn a_player_is_found_followed_and_forgotten_without_anything_polling_for_it() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("media_signals: no dbus-daemon; skipping");
        return;
    };

    // Playing *before* the helper exists. This is the live failure the whole
    // provider was rewritten for: a full start used to miss an already-playing
    // source that was plainly there.
    let player = serve_player(&bus.address, true, "Nightcall");

    let mut helper = Helper::start(&bus.address);
    let first = helper
        .wait_for(|frame| media(frame).cloned())
        .expect("the helper publishes the player that was already there");
    assert_eq!(
        first.get("title").and_then(Value::as_str),
        Some("Nightcall")
    );
    assert_eq!(
        first.get("artist").and_then(Value::as_str),
        Some("Kavinsky")
    );
    assert_eq!(
        first.get("nowPlaying").and_then(Value::as_str),
        Some("Kavinsky - Nightcall")
    );
    assert_eq!(first.get("playing").and_then(Value::as_bool), Some(true));
    // The player named itself nothing, so its bus name is the name shown.
    assert_eq!(
        first.get("player").and_then(Value::as_str),
        Some("celestinatest")
    );

    // What the player says next: a pause, announced the way MPRIS announces it.
    {
        let interface = player
            .object_server()
            .interface::<_, FakePlayer>(PLAYER_PATH)
            .expect("the exported player");
        let mut state = interface.get_mut();
        state.playing = false;
        state.title = "Odd Look".to_owned();
        zbus::block_on(state.playback_status_changed(interface.signal_emitter()))
            .expect("the status change is announced");
        zbus::block_on(state.metadata_changed(interface.signal_emitter()))
            .expect("the metadata change is announced");
    }

    let paused = helper
        .wait_for(|frame| {
            let reading = media(frame)?;
            (!reading.get("playing")?.as_bool()?).then(|| reading.clone())
        })
        .expect("the helper follows what the player said");
    assert_eq!(
        paused.get("title").and_then(Value::as_str),
        Some("Odd Look")
    );

    // And the name going away takes the widget with it: no player is not a
    // paused player, and the panel must not keep the last thing that played.
    drop(player);
    assert!(
        helper
            .wait_for(|frame| {
                let providers = frame.get("providers")?;
                // The key is gone from a complete frame, which is how this
                // channel says a provider withdrew.
                providers.is_object().then_some(())?;
                providers.get("media").is_none().then_some(())
            })
            .is_some(),
        "the media reading leaves with its player"
    );
}

/// A session with no player publishes nothing, and — the part that used to cost
/// a subprocess every five seconds — asks nobody about it either.
#[test]
fn a_session_with_no_player_publishes_no_media() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("media_signals: no dbus-daemon; skipping");
        return;
    };

    let mut helper = Helper::start(&bus.address);
    // Several complete frames go by; none of them carries a media reading.
    let mut seen = 0;
    let leaked = helper.wait_for(|frame| {
        let providers = frame.get("providers")?;
        if providers.get("media").is_some() {
            return Some(true);
        }
        seen += 1;
        (seen >= 5).then_some(false)
    });

    assert_eq!(leaked, Some(false), "media was published with no player");
}
