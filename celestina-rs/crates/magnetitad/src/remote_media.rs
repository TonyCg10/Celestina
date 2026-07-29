//! Per-link coordination for the phone player Magnetita controls.
//!
//! Protocol shapes remain in `magnetita-core`; this module owns only polling
//! cadence and which confirmed player/artwork identifier is current on one live
//! connection.

use magnetita_core::{MprisUpdate, PlayerState};
use magnetita_net::{Device, LinkError};

const PLAYER_POLL_MS: i64 = 5000;
const POSITION_POLL_MS: i64 = 1000;

pub enum Report {
    NoChange,
    Cleared,
    State(PlayerState),
}

#[derive(Default)]
pub struct RemoteMedia {
    player: Option<String>,
    playing: bool,
    supports_artwork: bool,
    requested_artwork: Option<String>,
    last_player_poll: i64,
    last_position_poll: i64,
}

impl RemoteMedia {
    /// Refresh the list periodically, plus position once per pump tick only
    /// while playing. A full refresh also asks for the current player's state.
    pub fn poll(&mut self, device: &mut Device, now: i64) -> Result<(), LinkError> {
        if now - self.last_player_poll >= PLAYER_POLL_MS {
            self.last_player_poll = now;
            device.send(magnetita_core::mpris::request_player_list)?;
            if let Some(player) = self.player.as_deref() {
                self.last_position_poll = now;
                device.send(|id| magnetita_core::mpris::request_now_playing(id, player))?;
            }
        } else if self.playing && now - self.last_position_poll >= POSITION_POLL_MS {
            self.last_position_poll = now;
            if let Some(player) = self.player.as_deref() {
                device.send(|id| magnetita_core::mpris::request_now_playing(id, player))?;
            }
        }
        Ok(())
    }

    pub fn handle(
        &mut self,
        update: MprisUpdate,
        device: &mut Device,
        now: i64,
    ) -> Result<Report, LinkError> {
        if let Some(supports) = update.supports_album_art_payload {
            self.supports_artwork = supports;
        }
        if let Some(players) = update.players {
            if players.is_empty() {
                self.clear();
                return Ok(Report::Cleared);
            }
            if self.player.is_none() {
                let first = players[0].clone();
                self.player = Some(first.clone());
                self.last_position_poll = now;
                device.send(|id| magnetita_core::mpris::request_now_playing(id, &first))?;
            }
        }
        let Some(state) = update.state else {
            return Ok(Report::NoChange);
        };
        self.player = Some(state.player.clone());
        self.playing = state.is_playing;
        if self.should_request_artwork(&state) {
            device.send(|id| {
                magnetita_core::request_album_art(id, &state.player, &state.album_art_url)
            })?;
        }
        Ok(Report::State(state))
    }

    pub fn send_action(&self, device: &mut Device, action: &str) -> Result<(), LinkError> {
        if let Some(player) = self.player.as_deref() {
            device.send(|id| magnetita_core::mpris::action(id, player, action))?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.player = None;
        self.playing = false;
        self.requested_artwork = None;
    }

    fn should_request_artwork(&mut self, state: &PlayerState) -> bool {
        if state.album_art_url.is_empty() {
            self.requested_artwork = None;
            return false;
        }
        if !self.supports_artwork
            || self.requested_artwork.as_deref() == Some(state.album_art_url.as_str())
        {
            return false;
        }
        self.requested_artwork = Some(state.album_art_url.clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteMedia;
    use magnetita_core::PlayerState;

    fn state(url: &str) -> PlayerState {
        PlayerState {
            album_art_url: url.to_owned(),
            ..PlayerState::default()
        }
    }

    #[test]
    fn artwork_requests_require_support_and_are_deduplicated() {
        let mut media = RemoteMedia::default();
        assert!(!media.should_request_artwork(&state("cover-a")));
        media.supports_artwork = true;
        assert!(media.should_request_artwork(&state("cover-a")));
        assert!(!media.should_request_artwork(&state("cover-a")));
        assert!(media.should_request_artwork(&state("cover-b")));
    }

    #[test]
    fn an_empty_cover_resets_the_request_key() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        assert!(media.should_request_artwork(&state("cover-a")));
        assert!(!media.should_request_artwork(&state("")));
        assert!(media.should_request_artwork(&state("cover-a")));
    }
}
