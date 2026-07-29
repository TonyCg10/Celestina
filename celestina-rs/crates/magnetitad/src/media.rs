//! Desktop media players, for the phone to see and drive — the
//! phone-drives-the-desktop half of `kdeconnect.mpris`.
//!
//! Shells out to `playerctl`, the standard MPRIS command-line tool, the same
//! best-effort way the daemon uses `wl-paste` for the clipboard: no playerctl
//! simply means the desktop advertises no players, never an error. Reading
//! `org.mpris.MediaPlayer2` off the bus directly would buy nothing one small,
//! already-packaged tool does not already do.

use std::process::Command;

use magnetita_core::PlayerState;

/// The desktop's MPRIS players, in playerctl's order (most-recently-active
/// first). Empty when playerctl is absent or nothing is playing.
pub fn players() -> Vec<String> {
    let Ok(output) = Command::new("playerctl").arg("--list-all").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One player's now-playing state, or `None` if playerctl cannot read it.
pub fn state(player: &str) -> Option<PlayerState> {
    // One metadata call yields every field, tab-separated, in a single spawn.
    let format = "{{title}}\t{{artist}}\t{{album}}\t{{mpris:length}}\t{{status}}\t{{volume}}";
    let output = Command::new("playerctl")
        .args(["--player", player, "metadata", "--format", format])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_state(
        player,
        &String::from_utf8_lossy(&output.stdout),
    ))
}

/// Run a KDE Connect transport verb on a desktop player. Best-effort; an unknown
/// verb is a no-op rather than an error.
pub fn control(player: &str, action: &str) {
    let subcommand = match action {
        "Play" => "play",
        "Pause" => "pause",
        "PlayPause" => "play-pause",
        "Stop" => "stop",
        "Next" => "next",
        "Previous" => "previous",
        _ => return,
    };
    let _ = Command::new("playerctl")
        .args(["--player", player, subcommand])
        .status();
}

/// Set a desktop player's volume (0–100). Best-effort.
pub fn set_volume(player: &str, volume: i32) {
    let level = f64::from(volume.clamp(0, 100)) / 100.0;
    let _ = Command::new("playerctl")
        .args(["--player", player, "volume", &format!("{level:.2}")])
        .status();
}

/// Turn one playerctl metadata line into a [`PlayerState`]. Pure, so the field
/// unit conversions (µs → ms, 0–1 → 0–100) are testable without playerctl. We
/// do not report `pos`: it moves every tick and the phone's widget only needs it
/// for a seek bar we do not drive, so it stays unknown.
fn parse_state(player: &str, line: &str) -> PlayerState {
    let mut fields = line.trim_end_matches(['\r', '\n']).split('\t');
    let title = fields.next().unwrap_or_default().to_owned();
    let artist = fields.next().unwrap_or_default().to_owned();
    let album = fields.next().unwrap_or_default().to_owned();
    let length_us = fields
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1);
    let status = fields.next().unwrap_or_default().trim();
    let volume_unit: f64 = fields
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1.0);

    let now_playing = match (artist.is_empty(), title.is_empty()) {
        (_, true) => String::new(),
        (true, false) => title.clone(),
        (false, false) => format!("{artist} - {title}"),
    };

    PlayerState {
        player: player.to_owned(),
        title,
        artist,
        album,
        album_art_url: String::new(),
        is_playing: status.eq_ignore_ascii_case("Playing"),
        // playerctl controls generic players; report the transport as available
        // and let the player itself no-op what it cannot do.
        can_pause: true,
        can_play: true,
        can_go_next: true,
        can_go_previous: true,
        can_seek: false,
        length: if length_us >= 0 { length_us / 1000 } else { -1 },
        pos: -1,
        volume: if volume_unit >= 0.0 {
            (volume_unit * 100.0).round() as i32
        } else {
            -1
        },
        now_playing,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_state;

    #[test]
    fn a_full_metadata_line_parses_with_unit_conversions() {
        // length in µs → ms; volume 0–1 → 0–100; status → is_playing.
        let line = "Song\tBand\tLP\t210000000\tPlaying\t0.8\n";
        let state = parse_state("Spotify", line);
        assert_eq!(state.player, "Spotify");
        assert_eq!(state.title, "Song");
        assert_eq!(state.artist, "Band");
        assert_eq!(state.album, "LP");
        assert!(state.is_playing);
        assert_eq!(state.length, 210_000); // 210 s in ms
        assert_eq!(state.volume, 80);
        assert_eq!(state.now_playing, "Band - Song");
    }

    #[test]
    fn a_paused_player_is_not_playing() {
        let state = parse_state("mpv", "T\tA\t\t-1\tPaused\t1.0");
        assert!(!state.is_playing);
        assert_eq!(state.length, -1);
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn a_track_without_an_artist_now_plays_just_the_title() {
        let state = parse_state("Firefox", "A tab\t\t\t-1\tPlaying\t-1");
        assert_eq!(state.now_playing, "A tab");
        assert_eq!(state.volume, -1); // unknown volume stays -1
    }
}
