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
}
