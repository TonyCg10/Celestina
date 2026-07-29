//! The `kdeconnect.mpris` plugin — media control, both directions.
//!
//! Two packet types carry it. `kdeconnect.mpris` reports players and now-playing
//! state; `kdeconnect.mpris.request` asks for that state or sends a transport
//! action. The two ends are symmetric, so this module has both halves:
//!
//! - **Desktop drives the phone.** We send a `mpris.request` — list the players,
//!   report a player's now-playing, or act (play/pause, next, previous) — and
//!   read the `kdeconnect.mpris` the phone answers with. This is the app's
//!   now-playing card and its transport buttons.
//! - **The phone drives the desktop.** We read a `mpris.request` from the phone
//!   and answer with a `kdeconnect.mpris` describing our own players (built from
//!   whatever the desktop is playing), and run the action it asked for.
//!
//! Pure: this is only the wire shapes. Reading desktop players and moving the
//! phone's state are the daemon's, over its trusted link.

use serde_json::{json, Value};

use crate::packet::NetworkPacket;

/// A media report: player list and/or now-playing state.
pub const TYPE_MPRIS: &str = "kdeconnect.mpris";

/// A request: ask for state, or send a transport action.
pub const TYPE_MPRIS_REQUEST: &str = "kdeconnect.mpris.request";

/// One player's now-playing state, the shape both ends exchange.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

/// What a `kdeconnect.mpris` packet carried: a refreshed player list, a
/// player's state, or both (a peer may restate the list alongside a state).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MprisUpdate {
    pub players: Option<Vec<String>>,
    /// Whether the peer can answer album-art requests with a bounded payload.
    /// Only player-list packets normally carry this capability.
    pub supports_album_art_payload: Option<bool>,
    pub state: Option<PlayerState>,
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
    pub action: Option<String>,
    /// Set that player's volume, 0–100.
    pub set_volume: Option<i32>,
}

// --- Desktop drives the phone: requests we send, reports we read. ------------

/// Ask the phone for the list of its players.
pub fn request_player_list(id: i64) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_MPRIS_REQUEST, json!({ "requestPlayerList": true }))
}

/// Ask the phone for one player's now-playing state (and volume).
pub fn request_now_playing(id: i64, player: &str) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_MPRIS_REQUEST,
        json!({ "player": player, "requestNowPlaying": true, "requestVolume": true }),
    )
}

/// Ask the phone to transfer the cover identified by its own `albumArtUrl`.
pub fn request_album_art(id: i64, player: &str, album_art_url: &str) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_MPRIS_REQUEST,
        json!({ "player": player, "albumArtUrl": album_art_url }),
    )
}

/// Send a transport action to one of the phone's players. `action` is a KDE
/// Connect verb: "Play", "Pause", "PlayPause", "Stop", "Next", "Previous".
pub fn action(id: i64, player: &str, action: &str) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_MPRIS_REQUEST,
        json!({ "player": player, "action": action }),
    )
}

/// Read a `kdeconnect.mpris` report, or `None` for a different type or an empty
/// body (neither a list nor a state).
pub fn read_mpris(packet: &NetworkPacket) -> Option<MprisUpdate> {
    if !packet.is(TYPE_MPRIS) {
        return None;
    }
    let body = packet.body.as_object()?;

    let players = body
        .get("playerList")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        });

    let supports_album_art_payload = body.get("supportAlbumArtPayload").and_then(Value::as_bool);

    // A state packet names a `player` and carries at least one now-playing
    // field; a bare list packet also has `player` on some peers, so require a
    // real field before treating it as a state.
    let state = body
        .get("player")
        .and_then(Value::as_str)
        .filter(|_| {
            body.contains_key("title")
                || body.contains_key("isPlaying")
                || body.contains_key("nowPlaying")
        })
        .map(|player| PlayerState {
            player: player.to_owned(),
            title: string_field(body, "title"),
            artist: string_field(body, "artist"),
            album: string_field(body, "album"),
            album_art_url: string_field(body, "albumArtUrl"),
            is_playing: bool_field(body, "isPlaying"),
            can_pause: bool_field(body, "canPause"),
            can_play: bool_field(body, "canPlay"),
            can_go_next: bool_field(body, "canGoNext"),
            can_go_previous: bool_field(body, "canGoPrevious"),
            can_seek: bool_field(body, "canSeek"),
            length: int_field(body, "length", -1),
            pos: int_field(body, "pos", -1),
            volume: int_field(body, "volume", -1) as i32,
            now_playing: string_field(body, "nowPlaying"),
        });

    if players.is_none() && supports_album_art_payload.is_none() && state.is_none() {
        return None;
    }
    Some(MprisUpdate {
        players,
        supports_album_art_payload,
        state,
    })
}

/// Read an album-art payload announcement. The source URL remains opaque and
/// the transfer port must stay inside KDE Connect's payload range.
pub fn read_album_art(packet: &NetworkPacket) -> Option<IncomingAlbumArt> {
    if !packet.is(TYPE_MPRIS) {
        return None;
    }
    let body = packet.body.as_object()?;
    if !bool_field(body, "transferringAlbumArt") {
        return None;
    }
    let player = body.get("player")?.as_str()?.to_owned();
    let source_url = body.get("albumArtUrl")?.as_str()?.to_owned();
    if player.is_empty() || source_url.is_empty() {
        return None;
    }
    let size = packet.payload_size?;
    let port = packet
        .payload_transfer_info
        .as_ref()
        .and_then(|info| info.get("port"))
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())?;
    if !(1739..=1764).contains(&port) {
        return None;
    }
    Some(IncomingAlbumArt {
        player,
        source_url,
        size,
        port,
        transfer_id: packet.id,
    })
}

// --- The phone drives the desktop: requests we read, reports we send. --------

/// Read a `kdeconnect.mpris.request` from the phone, or `None` for a different
/// type or a request that asks nothing we act on.
pub fn read_mpris_request(packet: &NetworkPacket) -> Option<MprisRequest> {
    if !packet.is(TYPE_MPRIS_REQUEST) {
        return None;
    }
    let body = packet.body.as_object()?;
    let request = MprisRequest {
        request_player_list: bool_field(body, "requestPlayerList"),
        player: body
            .get("player")
            .and_then(Value::as_str)
            .map(str::to_owned),
        request_now_playing: bool_field(body, "requestNowPlaying"),
        action: body
            .get("action")
            .and_then(Value::as_str)
            .map(str::to_owned),
        set_volume: body
            .get("setVolume")
            .and_then(Value::as_i64)
            .map(|v| v as i32),
    };
    // Ignore a request that carries nothing actionable.
    if !request.request_player_list
        && !request.request_now_playing
        && request.action.is_none()
        && request.set_volume.is_none()
    {
        return None;
    }
    Some(request)
}

/// Announce our desktop players to the phone. `supportAlbumArtPayload` is false:
/// we do not stream cover art (yet).
pub fn player_list_packet(id: i64, players: &[String]) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_MPRIS,
        json!({ "playerList": players, "supportAlbumArtPayload": false }),
    )
}

/// Report one desktop player's now-playing state to the phone.
pub fn state_packet(id: i64, state: &PlayerState) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_MPRIS,
        json!({
            "player": state.player,
            "title": state.title,
            "artist": state.artist,
            "album": state.album,
            "albumArtUrl": state.album_art_url,
            "isPlaying": state.is_playing,
            "canPause": state.can_pause,
            "canPlay": state.can_play,
            "canGoNext": state.can_go_next,
            "canGoPrevious": state.can_go_previous,
            "canSeek": state.can_seek,
            "length": state.length,
            "pos": state.pos,
            "volume": state.volume,
            "nowPlaying": state.now_playing,
        }),
    )
}

type Body = serde_json::Map<String, Value>;

fn string_field(body: &Body, key: &str) -> String {
    body.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn bool_field(body: &Body, key: &str) -> bool {
    body.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn int_field(body: &Body, key: &str, default: i64) -> i64 {
    body.get(key).and_then(Value::as_i64).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_list_request_asks_for_the_list() {
        let packet = request_player_list(1);
        assert!(packet.is(TYPE_MPRIS_REQUEST));
        assert_eq!(packet.body["requestPlayerList"], true);
    }

    #[test]
    fn an_action_names_its_player_and_verb() {
        let packet = action(1, "Spotify", "PlayPause");
        assert!(packet.is(TYPE_MPRIS_REQUEST));
        assert_eq!(packet.body["player"], "Spotify");
        assert_eq!(packet.body["action"], "PlayPause");
    }

    #[test]
    fn a_player_list_report_parses_the_names() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris","body":{
            "playerList":["Spotify","Firefox"],"supportAlbumArtPayload":true}}"#;
        let update = read_mpris(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert_eq!(
            update.players,
            Some(vec!["Spotify".to_owned(), "Firefox".to_owned()])
        );
        assert_eq!(update.supports_album_art_payload, Some(true));
        assert!(update.state.is_none());
    }

    #[test]
    fn a_state_report_parses_now_playing_and_caps() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris","body":{
            "player":"Spotify","title":"Song","artist":"Band","album":"LP",
            "isPlaying":true,"canPause":true,"canGoNext":true,"canGoPrevious":false,
            "length":210000,"pos":15000,"volume":80,"nowPlaying":"Band - Song"}}"#;
        let update = read_mpris(&NetworkPacket::parse(raw).unwrap()).unwrap();
        let state = update.state.unwrap();
        assert_eq!(state.player, "Spotify");
        assert_eq!(state.title, "Song");
        assert_eq!(state.artist, "Band");
        assert_eq!(state.album_art_url, "");
        assert!(state.is_playing);
        assert!(state.can_go_next);
        assert!(!state.can_go_previous);
        assert_eq!(state.length, 210_000);
        assert_eq!(state.volume, 80);
    }

    #[test]
    fn a_bare_list_packet_is_not_mistaken_for_a_state() {
        // A `player` with no now-playing field must not synthesise a blank state.
        let raw = r#"{"id":1,"type":"kdeconnect.mpris","body":{
            "playerList":["Spotify"],"player":"Spotify"}}"#;
        let update = read_mpris(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert!(update.players.is_some());
        assert!(update.state.is_none());
    }

    #[test]
    fn an_empty_mpris_packet_is_none() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris","body":{}}"#;
        assert!(read_mpris(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn an_album_art_request_echoes_the_peer_identifier() {
        let packet = request_album_art(4, "Spotify", "file:///phone/cover.png");
        assert_eq!(packet.body["player"], "Spotify");
        assert_eq!(packet.body["albumArtUrl"], "file:///phone/cover.png");
    }

    #[test]
    fn an_album_art_payload_keeps_only_fetchable_transfer_data() {
        let raw = r#"{"id":9,"type":"kdeconnect.mpris","body":{
            "player":"Spotify","albumArtUrl":"file:///phone/cover.png",
            "transferringAlbumArt":true},"payloadSize":2048,
            "payloadTransferInfo":{"port":1741}}"#;
        assert_eq!(
            read_album_art(&NetworkPacket::parse(raw).unwrap()),
            Some(IncomingAlbumArt {
                player: "Spotify".to_owned(),
                source_url: "file:///phone/cover.png".to_owned(),
                size: 2048,
                port: 1741,
                transfer_id: 9,
            })
        );
    }

    #[test]
    fn album_art_outside_the_payload_port_range_is_refused() {
        let raw = r#"{"id":9,"type":"kdeconnect.mpris","body":{
            "player":"Spotify","albumArtUrl":"cover","transferringAlbumArt":true},
            "payloadSize":2048,"payloadTransferInfo":{"port":8080}}"#;
        assert!(read_album_art(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn a_request_for_the_player_list_reads_back() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris.request","body":{
            "requestPlayerList":true}}"#;
        let request = read_mpris_request(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert!(request.request_player_list);
        assert!(request.action.is_none());
    }

    #[test]
    fn a_request_action_reads_its_player_and_verb() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris.request","body":{
            "player":"Firefox","action":"Next"}}"#;
        let request = read_mpris_request(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert_eq!(request.player.as_deref(), Some("Firefox"));
        assert_eq!(request.action.as_deref(), Some("Next"));
    }

    #[test]
    fn an_empty_request_is_none() {
        let raw = r#"{"id":1,"type":"kdeconnect.mpris.request","body":{"player":"X"}}"#;
        assert!(read_mpris_request(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn our_player_list_and_state_round_trip() {
        let players = vec!["mpv".to_owned()];
        let list = player_list_packet(1, &players);
        assert_eq!(read_mpris(&list).unwrap().players, Some(players));

        let state = PlayerState {
            player: "mpv".to_owned(),
            title: "T".to_owned(),
            artist: "A".to_owned(),
            is_playing: true,
            can_go_next: true,
            length: 1000,
            pos: 0,
            volume: 50,
            ..PlayerState::default()
        };
        let packet = state_packet(2, &state);
        assert_eq!(read_mpris(&packet).unwrap().state, Some(state));
    }
}
