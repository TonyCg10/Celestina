// Copied from `celestina-rs/crates/celestina-shell-core/src/media.rs` at
// 497d60f86faa85a2e67b0244bb4627d508e06c8a, adapted: the player and text
// bounds, `clamp_text`, `resolve` (an explicit pin over `active`) and
// `cover_url_path` are new.

//! Which player the panel is showing.
//!
//! A session has more than one MPRIS player more often than not — a browser
//! publishes one per tab that ever made a sound — so "what is playing" is a
//! choice, and the choice is the part worth owning here. It has no IO in it: a
//! caller hands over what each player last said about itself and gets back the
//! one the panel should show.
//!
//! The rule is deliberately dull. Something that is playing beats something
//! that is not; between two of a kind, whichever spoke most recently. That is
//! what a person means by "the music", and it is stable — a paused tab does not
//! take the panel from the track that is actually running.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

/// The most players tracked at once; any further one is ignored.
pub const MAX_PLAYERS: usize = 16;
/// The longest text kept from a player (bus name, identity, track strings), in
/// bytes.
pub const MAX_TEXT_BYTES: usize = 256;

/// `text` cut to at most [`MAX_TEXT_BYTES`] bytes, never inside a character.
#[must_use]
pub fn clamp_text(text: &str) -> String {
    if text.len() <= MAX_TEXT_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// The player the panel shows: the pinned one while it is on the bus,
/// otherwise [`active`]. A pin to a player that left changes nothing.
#[must_use]
pub fn resolve<'a>(players: &'a [Player], pin: Option<&str>) -> Option<&'a Player> {
    pin.and_then(|name| players.iter().find(|player| player.bus_name == name))
        .or_else(|| active(players))
}

/// The local file a player's `mpris:artUrl` names, or `None` for anything the
/// shell will not open.
///
/// Only `file://` followed by an absolute path is accepted: there is no
/// fetcher, so `http(s)`, `data:` and relative names are refused, as is a host
/// part, an empty path, a NUL byte or a `..` segment (after percent-decoding).
#[must_use]
pub fn cover_url_path(url: &str) -> Option<PathBuf> {
    let encoded = url.strip_prefix("file://")?;
    if !encoded.starts_with('/') {
        return None;
    }
    let bytes = percent_decode(encoded)?;
    if bytes.contains(&0)
        || bytes
            .split(|byte| *byte == b'/')
            .any(|segment| segment == b"..")
    {
        return None;
    }
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            out.push(high << 4 | low);
            index += 3;
        } else {
            out.push(byte);
            index += 1;
        }
    }
    Some(out)
}

/// The value of one ASCII hex digit; anything else (a sign included) is
/// `None`.
fn hex_digit(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

/// What a player last said it was playing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The player's own `mpris:artUrl`, untouched. Whether it names something
    /// this shell will open is the adapter's decision, not this one's.
    pub art_url: String,
    /// Milliseconds, or `-1` when the player did not say.
    pub length_ms: i64,
}

impl Default for Track {
    /// An empty track whose length is *unknown* rather than zero. A derived
    /// default would say every silent player is playing something zero
    /// milliseconds long, which is a number nobody read.
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: String::new(),
            length_ms: -1,
        }
    }
}

/// One player on the session bus, as the adapter last read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    /// The well-known name, `org.mpris.MediaPlayer2.firefox.instance_1_15`.
    pub bus_name: String,
    /// What the player calls itself, or its bus name's last segment when it
    /// offers no `Identity`.
    pub identity: String,
    pub playing: bool,
    pub track: Track,
    /// Milliseconds, or `-1` when unknown.
    pub position_ms: i64,
    /// A monotonic counter: how recently this player last said anything. The
    /// adapter owns the clock, so this stays orderable without being a time.
    pub heard: u64,
}

/// The player the panel should show, or `None` when the session has none.
///
/// Ties break on the bus name so the same set of players always chooses the
/// same one — two browser tabs that appeared in the same instant must not make
/// the panel flicker between them.
#[must_use]
pub fn active(players: &[Player]) -> Option<&Player> {
    players.iter().max_by(|left, right| {
        left.playing
            .cmp(&right.playing)
            .then(left.heard.cmp(&right.heard))
            // Reversed: the earlier name wins a tie, and `max_by` keeps the
            // greatest.
            .then(right.bus_name.cmp(&left.bus_name))
    })
}

/// The player's own name from its bus name, for a player that offers no
/// `Identity`.
///
/// `org.mpris.MediaPlayer2.firefox.instance_1_15` is `firefox`: the segment
/// after the well-known prefix, without the instance the specification appends.
#[must_use]
pub fn identity_from_bus_name(bus_name: &str) -> String {
    const PREFIX: &str = "org.mpris.MediaPlayer2.";
    let rest = bus_name.strip_prefix(PREFIX).unwrap_or(bus_name);
    rest.split('.')
        .find(|segment| !segment.is_empty())
        .unwrap_or(rest)
        .to_owned()
}

/// The one line a person reads for "what is playing".
///
/// A track with no title has nothing to say; a track with no artist says only
/// its title. Magnetita composes the same line for the phone's now-playing
/// packet and keeps its own copy: that one belongs to a KDE Connect payload
/// this shell does not own, and moving it would put a Celestina delivery inside
/// another product's prefix. If the two ever have to agree, that is a suite
/// unit of its own rather than a change made in passing.
#[must_use]
pub fn now_playing_line(artist: &str, title: &str) -> String {
    match (artist.is_empty(), title.is_empty()) {
        (_, true) => String::new(),
        (true, false) => title.to_owned(),
        (false, false) => format!("{artist} - {title}"),
    }
}

/// Whether a name is a player at all.
#[must_use]
pub fn is_player_name(bus_name: &str) -> bool {
    bus_name.starts_with("org.mpris.MediaPlayer2.")
        && bus_name.len() > "org.mpris.MediaPlayer2.".len()
}

/// Where a playing track has reached, `elapsed_ms` after it was last measured.
///
/// Between two things a player says, the panel's progress is arithmetic rather
/// than a question: asking a player for its position once a second would be a
/// call per second per player for a number that moves at a known rate. A paused
/// track has not moved, and an unknown position stays unknown.
#[must_use]
pub fn advanced_position(position_ms: i64, playing: bool, elapsed_ms: i64) -> i64 {
    if position_ms < 0 || !playing || elapsed_ms <= 0 {
        return position_ms;
    }

    position_ms.saturating_add(elapsed_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(bus: &str, playing: bool, heard: u64) -> Player {
        Player {
            bus_name: bus.to_owned(),
            identity: identity_from_bus_name(bus),
            playing,
            track: Track::default(),
            position_ms: -1,
            heard,
        }
    }

    #[test]
    fn an_empty_track_has_an_unknown_length_rather_than_a_zero_one() {
        assert_eq!(Track::default().length_ms, -1);
    }

    #[test]
    fn nothing_playing_anywhere_is_no_player_at_all() {
        assert_eq!(active(&[]), None);
    }

    #[test]
    fn something_playing_beats_something_paused_however_recent() {
        let players = [
            player("org.mpris.MediaPlayer2.firefox.instance_1", true, 1),
            player("org.mpris.MediaPlayer2.vlc", false, 99),
        ];

        assert_eq!(
            active(&players).map(|player| player.bus_name.as_str()),
            Some("org.mpris.MediaPlayer2.firefox.instance_1")
        );
    }

    #[test]
    fn between_two_playing_the_one_that_spoke_last_wins() {
        let players = [
            player("org.mpris.MediaPlayer2.firefox.instance_1", true, 4),
            player("org.mpris.MediaPlayer2.spotify", true, 7),
        ];

        assert_eq!(
            active(&players).map(|player| player.bus_name.as_str()),
            Some("org.mpris.MediaPlayer2.spotify")
        );
    }

    #[test]
    fn with_nothing_playing_the_most_recent_is_still_what_the_panel_shows() {
        let players = [
            player("org.mpris.MediaPlayer2.firefox.instance_1", false, 4),
            player("org.mpris.MediaPlayer2.vlc", false, 7),
        ];

        assert_eq!(
            active(&players).map(|player| player.bus_name.as_str()),
            Some("org.mpris.MediaPlayer2.vlc")
        );
    }

    #[test]
    fn a_tie_always_chooses_the_same_player() {
        let players = [
            player("org.mpris.MediaPlayer2.b", true, 3),
            player("org.mpris.MediaPlayer2.a", true, 3),
        ];
        let reversed = [players[1].clone(), players[0].clone()];

        assert_eq!(active(&players), active(&reversed));
        assert_eq!(
            active(&players).map(|player| player.bus_name.as_str()),
            Some("org.mpris.MediaPlayer2.a")
        );
    }

    #[test]
    fn a_player_names_itself_from_its_bus_name_when_it_offers_nothing_better() {
        assert_eq!(
            identity_from_bus_name("org.mpris.MediaPlayer2.firefox.instance_1_15"),
            "firefox"
        );
        assert_eq!(identity_from_bus_name("org.mpris.MediaPlayer2.vlc"), "vlc");
        // Not a player name at all: answered rather than panicked over.
        assert_eq!(identity_from_bus_name("org.kde.Something"), "org");
    }

    #[test]
    fn only_the_specifications_own_names_are_players() {
        assert!(is_player_name("org.mpris.MediaPlayer2.vlc"));
        assert!(!is_player_name("org.mpris.MediaPlayer2."));
        assert!(!is_player_name("org.mpris.MediaPlayer2"));
        assert!(!is_player_name("org.freedesktop.Notifications"));
        assert!(!is_player_name(""));
    }

    #[test]
    fn what_is_playing_reads_as_one_line_or_as_nothing() {
        assert_eq!(
            now_playing_line("Kavinsky", "Nightcall"),
            "Kavinsky - Nightcall"
        );
        // A stream that names no artist says only its title.
        assert_eq!(now_playing_line("", "lofi radio"), "lofi radio");
        // No title is nothing to say, whatever else arrived.
        assert_eq!(now_playing_line("Kavinsky", ""), "");
        assert_eq!(now_playing_line("", ""), "");
    }

    #[test]
    fn progress_moves_only_while_something_is_playing() {
        assert_eq!(advanced_position(1_000, true, 500), 1_500);
        // Paused is where it was.
        assert_eq!(advanced_position(1_000, false, 500), 1_000);
        // Unknown stays unknown rather than becoming a number.
        assert_eq!(advanced_position(-1, true, 500), -1);
        assert_eq!(advanced_position(1_000, true, 0), 1_000);
        // A clock that went backwards changes nothing.
        assert_eq!(advanced_position(1_000, true, -5), 1_000);
    }

    #[test]
    fn a_pin_wins_over_the_active_player_while_it_is_present() {
        let players = [
            player("org.mpris.MediaPlayer2.a", true, 9),
            player("org.mpris.MediaPlayer2.b", false, 1),
        ];
        let pinned = resolve(&players, Some("org.mpris.MediaPlayer2.b"));
        assert_eq!(
            pinned.map(|p| p.bus_name.as_str()),
            Some("org.mpris.MediaPlayer2.b")
        );
        assert_eq!(resolve(&players, None), active(&players));
    }

    #[test]
    fn a_pin_to_a_gone_player_falls_back_to_the_active_one() {
        let players = [player("org.mpris.MediaPlayer2.a", false, 2)];
        assert_eq!(
            resolve(&players, Some("org.mpris.MediaPlayer2.gone")),
            active(&players)
        );
        assert_eq!(resolve(&[], Some("org.mpris.MediaPlayer2.gone")), None);
    }

    #[test]
    fn text_is_cut_on_a_character_boundary_within_the_bound() {
        assert_eq!(clamp_text("short"), "short");
        let exact = "a".repeat(MAX_TEXT_BYTES);
        assert_eq!(clamp_text(&exact), exact);
        // 2-byte characters straddle the bound only when it is odd.
        let wide = "\u{e9}".repeat(MAX_TEXT_BYTES);
        let cut = clamp_text(&wide);
        assert_eq!(cut.len(), MAX_TEXT_BYTES);
        let straddle = format!("a{}", "\u{e9}".repeat(MAX_TEXT_BYTES));
        let cut = clamp_text(&straddle);
        assert_eq!(cut.len(), MAX_TEXT_BYTES - 1);
        assert!(cut.is_char_boundary(cut.len()));
        assert_eq!(MAX_PLAYERS, 16);
    }

    #[test]
    fn only_absolute_file_urls_name_a_cover() {
        assert_eq!(
            cover_url_path("file:///tmp/cover.png"),
            Some(PathBuf::from("/tmp/cover.png"))
        );
        assert_eq!(
            cover_url_path("file:///a%20b.png"),
            Some(PathBuf::from("/a b.png"))
        );
        for refused in [
            "",
            "http://example.org/a.png",
            "https://example.org/a.png",
            "data:image/png;base64,AAAA",
            "cover.png",
            "/tmp/cover.png",
            "file://",
            "file://host/a.png",
            "file://localhost/a.png",
            "file://host/../etc/passwd",
            "file:///a/../b.png",
            "file:///a/%2e%2e/b.png",
            "file:///a%00b.png",
            "file:///a%zz.png",
            "file:///a%4",
            // `from_str_radix` takes a leading sign: both nibbles must be
            // ASCII hex digits.
            "file:///a%+1.png",
            "file:///a%-1.png",
            "file:///a%+f.png",
            "file:relative.png",
        ] {
            assert_eq!(cover_url_path(refused), None, "{refused:?}");
        }
    }
}
