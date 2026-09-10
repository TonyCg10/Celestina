//! Media state and control, the shapes both ends exchange: a player's
//! now-playing state, the transport actions, playback progress, and the
//! desktop's own players read from `playerctl`. Pure: the wire is
//! `magnetita-proto`'s, the daemon moves the state.

/// One player's now-playing state, the shape both ends exchange.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerState {
    /// The player this state is for (e.g. "Spotify").
    pub player: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The peer-local identifier for the current cover. It is echoed back in an
    /// album-art request; it is never treated as a local path.
    pub album_art_url: String,
    pub is_playing: bool,
    pub can_pause: bool,
    pub can_play: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_seek: bool,
    /// Track length in milliseconds, or -1 if unknown.
    pub length: i64,
    /// Playback position in milliseconds, or -1 if unknown.
    pub pos: i64,
    /// Volume 0–100, or -1 if unknown.
    pub volume: i32,
    /// The peer's own "artist - title" line, when it sends one.
    pub now_playing: String,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            player: String::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_art_url: String::new(),
            is_playing: false,
            can_pause: false,
            can_play: false,
            can_go_next: false,
            can_go_previous: false,
            can_seek: false,
            length: -1,
            pos: -1,
            volume: -1,
            now_playing: String::new(),
        }
    }
}

/// A transport verb accepted by the KDE Connect MPRIS boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

impl MediaAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Play" => Some(Self::Play),
            "Pause" => Some(Self::Pause),
            "PlayPause" => Some(Self::PlayPause),
            "Stop" => Some(Self::Stop),
            "Next" => Some(Self::Next),
            "Previous" => Some(Self::Previous),
            _ => None,
        }
    }

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

/// Presentation-independent classification of a reported playback position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackProgress {
    Unavailable,
    Finite { position_ms: i64, length_ms: i64 },
    Live,
}

/// Some Android players represent a live stream as a long rolling counter whose
/// reported position tracks its equally long "length". Keep that peer quirk out
/// of QML while requiring both a 24-hour floor and proximity to the live edge,
/// so ordinary long-form media remains finite.
pub fn playback_progress(position_ms: i64, length_ms: i64) -> PlaybackProgress {
    const LIVE_COUNTER_FLOOR_MS: i64 = 24 * 60 * 60 * 1000;
    const LIVE_EDGE_TOLERANCE_MS: i64 = 60 * 1000;

    if position_ms < 0 || length_ms <= 0 {
        return PlaybackProgress::Unavailable;
    }
    if length_ms >= LIVE_COUNTER_FLOOR_MS
        && position_ms >= length_ms.saturating_sub(LIVE_EDGE_TOLERANCE_MS)
        && position_ms <= length_ms.saturating_add(LIVE_EDGE_TOLERANCE_MS)
    {
        return PlaybackProgress::Live;
    }
    PlaybackProgress::Finite {
        position_ms: position_ms.clamp(0, length_ms),
        length_ms,
    }
}

/// The one `playerctl` metadata format this suite asks for: every field the
/// vocabulary above needs, tab-separated, in a single spawn.
///
/// Two consumers read it — the daemon answering a phone's now-playing request
/// and the shell's own media provider — so the format and the parser that
/// matches it live together here rather than being written twice.
pub const PLAYERCTL_FORMAT: &str =
    "{{title}}\t{{artist}}\t{{album}}\t{{mpris:length}}\t{{status}}\t{{volume}}";

/// Reads one line of [`PLAYERCTL_FORMAT`] output into a [`PlayerState`].
///
/// Pure, so the unit conversions (µs → ms, 0–1 → 0–100) are testable without
/// playerctl. Missing fields are normal — a browser tab has no album, a stream
/// no length — and anything unreadable becomes the vocabulary's "unknown"
/// (`-1`) rather than a zero that would read as a real value. `pos` is not
/// reported here: it moves every tick, so a caller that needs it asks for it
/// separately and knows when it was measured.
#[must_use]
pub fn parse_playerctl_state(player: &str, line: &str) -> PlayerState {
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
            (volume_unit.clamp(0.0, 1.0) * 100.0).round() as i32
        } else {
            -1
        },
        now_playing,
    }
}

/// One album-art payload the peer is offering on its separate TLS socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncomingAlbumArt {
    pub player: String,
    /// Opaque peer-local identifier from the preceding now-playing state.
    pub source_url: String,
    pub size: i64,
    pub port: u16,
    /// Packet id used only to give each cached image a distinct local URL.
    pub transfer_id: i64,
}

/// What a `kdeconnect.mpris.request` asked of us (the phone driving the desktop).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MprisRequest {
    /// Send back the list of desktop players.
    pub request_player_list: bool,
    /// The player the rest of the request is about.
    pub player: Option<String>,
    /// Send back that player's now-playing state.
    pub request_now_playing: bool,
    /// A transport action: "Play", "Pause", "PlayPause", "Stop", "Next",
    /// "Previous". Left as the peer's string; the daemon maps it to a command.
    pub action: Option<MediaAction>,
    /// Set that player's volume, 0–100.
    pub set_volume: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_unknown_media_values() {
        let state = PlayerState::default();
        assert_eq!(state.length, -1);
        assert_eq!(state.pos, -1);
        assert_eq!(state.volume, -1);
    }

    #[test]
    fn playback_progress_distinguishes_finite_live_and_unknown() {
        assert_eq!(playback_progress(-1, -1), PlaybackProgress::Unavailable);
        assert_eq!(
            playback_progress(150_000, 120_000),
            PlaybackProgress::Finite {
                position_ms: 120_000,
                length_ms: 120_000,
            }
        );
        assert_eq!(
            playback_progress(4_796_000_000, 4_796_000_000),
            PlaybackProgress::Live
        );
        assert_eq!(
            playback_progress(60_000, 172_800_000),
            PlaybackProgress::Finite {
                position_ms: 60_000,
                length_ms: 172_800_000,
            }
        );
    }
}
