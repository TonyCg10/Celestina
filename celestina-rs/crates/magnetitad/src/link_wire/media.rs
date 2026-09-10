//! Media on the own wire, both ways: the phone's player onto the desktop's
//! now-playing card and the desktop's buttons back to it; the desktop's
//! MPRIS players to the phone through the same `playerctl` worker the KDE
//! Connect wire uses, and the phone's buttons, seek and volume back.
//!
//! The worker is per session and exists only while the media setting is on;
//! the desktop's states go out while the phone has asked for them recently,
//! so an idle phone costs no subprocess.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use magnetita_core::{MediaAction, MprisRequest, PlayerState};
use magnetita_proto::capability;
use magnetita_proto::daily::media::{MediaButton, MediaCommand, MediaRequest, MediaState};
use magnetita_proto::Envelope;

use crate::lock::LockOk;
use crate::media::{Reply, Worker};

/// How long one `MediaRequest` keeps the desktop's states flowing.
const INTEREST: Duration = Duration::from_secs(90);
/// How often the desktop's player states are re-read while wanted.
const STATE_EVERY: Duration = Duration::from_secs(2);
/// How often the player list is re-read while wanted.
const LIST_EVERY: Duration = Duration::from_secs(10);

/// The media state of one session.
pub(crate) struct SessionMedia {
    worker: Mutex<Option<Worker>>,
    /// The phone's current player, the one the desktop's buttons address.
    phone_player: Mutex<String>,
    /// Desktop players the phone has been told about.
    players: Mutex<BTreeSet<String>>,
    wanted_until: Mutex<Option<Instant>>,
    last_list: Mutex<Option<Instant>>,
    last_states: Mutex<Option<Instant>>,
}

impl Default for SessionMedia {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
            phone_player: Mutex::new(String::new()),
            players: Mutex::new(BTreeSet::new()),
            wanted_until: Mutex::new(None),
            last_list: Mutex::new(None),
            last_states: Mutex::new(None),
        }
    }
}

fn envelope(kind: u16, body: Vec<u8>) -> Envelope {
    Envelope {
        capability: capability::MEDIA,
        kind,
        id: 0,
        body,
    }
}

/// The phone's wire state as the registry's shape; an empty player name
/// means nothing is playing.
pub(crate) fn player_state(state: &MediaState) -> Option<PlayerState> {
    if state.player.is_empty() {
        return None;
    }
    let now_playing = match (state.artist.is_empty(), state.title.is_empty()) {
        (false, false) => format!("{} - {}", state.artist, state.title),
        (true, false) => state.title.clone(),
        (false, true) => state.artist.clone(),
        (true, true) => String::new(),
    };
    Some(PlayerState {
        player: state.player.clone(),
        title: state.title.clone(),
        artist: state.artist.clone(),
        album: state.album.clone(),
        album_art_url: String::new(),
        is_playing: state.playing,
        can_pause: true,
        can_play: true,
        can_go_next: state.can_next,
        can_go_previous: state.can_previous,
        can_seek: state.can_seek,
        length: if state.length_ms == 0 {
            -1
        } else {
            state.length_ms as i64
        },
        pos: state.position_ms as i64,
        volume: i32::from(state.volume),
        now_playing,
    })
}

/// A desktop player's state as the wire carries it.
pub(crate) fn media_state(state: &PlayerState) -> MediaState {
    MediaState {
        player: state.player.clone(),
        title: state.title.clone(),
        artist: state.artist.clone(),
        album: state.album.clone(),
        playing: state.is_playing,
        position_ms: state.pos.max(0) as u64,
        length_ms: state.length.max(0) as u64,
        can_seek: state.can_seek,
        can_next: state.can_go_next,
        can_previous: state.can_go_previous,
        volume: state.volume.clamp(0, 100) as u8,
    }
}

fn action_for(button: MediaButton) -> MediaAction {
    match button {
        MediaButton::Play => MediaAction::Play,
        MediaButton::Pause => MediaAction::Pause,
        MediaButton::PlayPause => MediaAction::PlayPause,
        MediaButton::Next => MediaAction::Next,
        MediaButton::Previous => MediaAction::Previous,
        MediaButton::Stop => MediaAction::Stop,
    }
}

fn button_for(action: MediaAction) -> MediaButton {
    match action {
        MediaAction::Play => MediaButton::Play,
        MediaAction::Pause => MediaButton::Pause,
        MediaAction::PlayPause => MediaButton::PlayPause,
        MediaAction::Next => MediaButton::Next,
        MediaAction::Previous => MediaButton::Previous,
        MediaAction::Stop => MediaButton::Stop,
    }
}

impl SessionMedia {
    /// Matches the worker to the setting; false when `playerctl` cannot be
    /// driven, which is a degradation, not an error.
    pub(crate) fn set_active(&self, active: bool) {
        let mut worker = self.worker.lock_ok();
        if active && worker.is_none() {
            *worker = Worker::new().ok();
        } else if !active {
            *worker = None;
        }
    }

    /// The phone reported its player: remember which one the buttons address.
    pub(crate) fn note_phone_state(&self, state: &MediaState) {
        *self.phone_player.lock_ok() = state.player.clone();
    }

    /// The desktop's transport verb for the phone's current player.
    pub(crate) fn command_for_phone(&self, action: MediaAction) -> Option<Envelope> {
        let player = self.phone_player.lock_ok().clone();
        if player.is_empty() {
            return None;
        }
        Some(envelope(
            MediaCommand::KIND,
            MediaCommand {
                player,
                button: Some(button_for(action)),
                seek_ms: None,
                volume: None,
            }
            .encode(),
        ))
    }

    /// The phone asked for the desktop's players: states flow for a while.
    pub(crate) fn wanted(&self) {
        *self.wanted_until.lock_ok() = Some(Instant::now() + INTEREST);
        *self.last_list.lock_ok() = None;
        *self.last_states.lock_ok() = None;
    }

    /// The phone pressed a button, sought or set the volume on a desktop player.
    pub(crate) fn drive(&self, command: &MediaCommand) {
        let request = MprisRequest {
            request_player_list: false,
            player: Some(command.player.clone()),
            request_now_playing: true,
            action: command.button.map(action_for),
            set_volume: command.volume.map(i32::from),
        };
        if let Some(worker) = self.worker.lock_ok().as_ref() {
            worker.submit(request);
        }
    }

    /// Called on the session's tick: re-reads what the phone wants and
    /// returns the states to send.
    pub(crate) fn tick(&self) -> Vec<Envelope> {
        let mut out = Vec::new();
        let now = Instant::now();
        let wanted = self.wanted_until.lock_ok().is_some_and(|until| until > now);
        let worker = self.worker.lock_ok();
        let Some(worker) = worker.as_ref() else {
            return out;
        };
        if wanted {
            let list_due = self
                .last_list
                .lock_ok()
                .is_none_or(|at| now.duration_since(at) >= LIST_EVERY);
            if list_due {
                *self.last_list.lock_ok() = Some(now);
                worker.submit(MprisRequest {
                    request_player_list: true,
                    ..Default::default()
                });
            }
            let states_due = self
                .last_states
                .lock_ok()
                .is_none_or(|at| now.duration_since(at) >= STATE_EVERY);
            if states_due {
                *self.last_states.lock_ok() = Some(now);
                for player in self.players.lock_ok().iter() {
                    worker.submit(MprisRequest {
                        player: Some(player.clone()),
                        request_now_playing: true,
                        ..Default::default()
                    });
                }
            }
        }
        while let Some(reply) = worker.try_reply() {
            match reply {
                Reply::Players(list) => {
                    let mut players = self.players.lock_ok();
                    let known: BTreeSet<String> = list.into_iter().collect();
                    // A player that left is reported once with an empty state.
                    for gone in players.difference(&known) {
                        out.push(envelope(
                            MediaState::KIND,
                            MediaState {
                                player: gone.clone(),
                                title: String::new(),
                                artist: String::new(),
                                album: String::new(),
                                playing: false,
                                position_ms: 0,
                                length_ms: 0,
                                can_seek: false,
                                can_next: false,
                                can_previous: false,
                                volume: 0,
                            }
                            .encode(),
                        ));
                    }
                    *players = known;
                }
                Reply::State(state) => {
                    out.push(envelope(MediaState::KIND, media_state(&state).encode()));
                }
            }
        }
        out
    }

    pub(crate) fn request() -> Envelope {
        envelope(MediaRequest::KIND, MediaRequest.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phone_state_maps_to_the_registry_shape_and_an_empty_player_clears() {
        let state = MediaState {
            player: "YT Music".into(),
            title: "Song".into(),
            artist: "Band".into(),
            album: "Album".into(),
            playing: true,
            position_ms: 1500,
            length_ms: 0,
            can_seek: true,
            can_next: true,
            can_previous: false,
            volume: 40,
        };
        let mapped = player_state(&state).unwrap();
        assert_eq!(mapped.now_playing, "Band - Song");
        assert_eq!(mapped.length, -1, "an unknown length is -1 in the registry");
        assert_eq!((mapped.pos, mapped.volume), (1500, 40));
        assert!(player_state(&MediaState {
            player: String::new(),
            ..state
        })
        .is_none());
    }

    #[test]
    fn the_desktop_buttons_address_the_phone_player_last_reported() {
        let media = SessionMedia::default();
        assert!(media.command_for_phone(MediaAction::Next).is_none());
        media.note_phone_state(&MediaState {
            player: "Spotify".into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            playing: false,
            position_ms: 0,
            length_ms: 0,
            can_seek: false,
            can_next: true,
            can_previous: true,
            volume: 50,
        });
        let env = media.command_for_phone(MediaAction::Next).unwrap();
        let command = MediaCommand::decode(&env.body).unwrap();
        assert_eq!(
            (command.player.as_str(), command.button),
            ("Spotify", Some(MediaButton::Next))
        );
    }
}
