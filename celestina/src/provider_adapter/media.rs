//! What the desktop is playing, and the cover it points at.

use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use magnetita_core::mpris::{self, MediaAction, PlaybackProgress};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded};

/// While something is playing the panel is worth updating at this rate; a
/// position that moves faster than a person reads it is not worth a subprocess.
const INTERVAL: Duration = Duration::from_secs(2);
/// With no player at all there is nothing to poll for, so the helper asks less
/// often rather than spawning `playerctl` twice a second all day.
const IDLE_INTERVAL: Duration = Duration::from_secs(5);

pub const NAME: &str = "media";

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: media: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

/// A transport verb on whatever the panel is currently showing. The vocabulary
/// is the suite's own, not a second dialect.
pub fn action(verb: &str) -> Result<(), String> {
    let action = MediaAction::parse(verb)
        .ok_or_else(|| format!("'{NAME}' does not serve the verb '{verb}'"))?;
    let player = active_player().ok_or_else(|| "no player is running".to_owned())?;
    let subcommand = match action {
        MediaAction::Play => "play",
        MediaAction::Pause => "pause",
        MediaAction::PlayPause => "play-pause",
        MediaAction::Stop => "stop",
        MediaAction::Next => "next",
        MediaAction::Previous => "previous",
    };

    run_bounded("playerctl", &["--player", &player, subcommand])
        .map(|_| ())
        .ok_or_else(|| format!("{player} did not accept {subcommand}"))
}

/// The player the session is actually listening to: playerctl lists the most
/// recently active first, which is the one a panel should be showing.
fn active_player() -> Option<String> {
    run_bounded("playerctl", &["--list-all"])?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
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

/// Where the player is now, in milliseconds, read from the microseconds
/// playerctl reports beside the metadata. Kept as integers throughout: a
/// position is an exact count, and turning it into a float to divide would only
/// add rounding to a number the panel shows to the second.
fn position_ms(field: &str) -> i64 {
    field
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|microseconds| *microseconds >= 0)
        .map_or(-1, |microseconds| microseconds / 1_000)
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    loop {
        let Some(player) = active_player() else {
            // No player is not a paused player: the widget goes away rather
            // than keeping the last thing that ever played. The provider stays
            // registered, so a transport command is still answered by media
            // itself — "no player is running" — instead of reading as though
            // this helper had no media provider at all.
            lock_runtime(runtime).withdraw(id);
            thread::sleep(IDLE_INTERVAL);
            continue;
        };

        // One spawn for everything: the shared format, plus the two fields the
        // suite's vocabulary deliberately leaves out because they move on their
        // own. The shared parser reads what it knows and ignores the rest.
        let format = format!(
            "{}\t{{{{position}}}}\t{{{{mpris:artUrl}}}}",
            mpris::PLAYERCTL_FORMAT
        );
        if let Some(line) = run_bounded(
            "playerctl",
            &["--player", &player, "metadata", "--format", &format],
        ) {
            let state = mpris::parse_playerctl_state(&player, &line);
            let mut appended = line.trim_end_matches(['\r', '\n']).split('\t').skip(6);
            let position = appended.next().map_or(-1, position_ms);
            let artwork = appended.next().and_then(artwork_path);

            let mut payload = Payload::new();
            payload.insert("player".to_owned(), Value::from(state.player.clone()));
            payload.insert("title".to_owned(), Value::from(state.title.clone()));
            payload.insert("artist".to_owned(), Value::from(state.artist.clone()));
            payload.insert(
                "nowPlaying".to_owned(),
                Value::from(state.now_playing.clone()),
            );
            payload.insert("playing".to_owned(), Value::from(state.is_playing));
            if let Some(artwork) = artwork {
                payload.insert("artPath".to_owned(), Value::from(artwork));
            }
            match mpris::playback_progress(position, state.length) {
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

        thread::sleep(INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::artwork_path;

    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

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
