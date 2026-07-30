//! Per-link coordination for the phone player Magnetita controls.
//!
//! Protocol shapes remain in `magnetita-core`; this module owns only polling
//! cadence and which confirmed player/artwork identifier is current on one live
//! connection.

use magnetita_core::{MediaAction, MprisUpdate, PlayerState};
use magnetita_net::{Device, LinkError};

const PLAYER_POLL_MS: i64 = 5000;
const POSITION_POLL_MS: i64 = 1000;
const ARTWORK_RETRY_BASE_MS: i64 = 2000;
const ARTWORK_RETRY_MAX_MS: i64 = 60_000;
const ARTWORK_REQUEST_TIMEOUT_MS: i64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArtworkKey {
    player: String,
    source_url: String,
}

struct ArtworkRequest {
    key: ArtworkKey,
    requested_at: i64,
}

struct ArtworkFailure {
    key: ArtworkKey,
    failures: u8,
    retry_after: i64,
}

pub enum Report {
    NoChange,
    Cleared,
    State(PlayerState),
}

fn report_without_state(selection_changed: bool) -> Report {
    if selection_changed {
        Report::Cleared
    } else {
        Report::NoChange
    }
}

#[derive(Default)]
pub struct RemoteMedia {
    player: Option<String>,
    playing: bool,
    supports_artwork: bool,
    requested_artwork: Option<ArtworkRequest>,
    artwork_failure: Option<ArtworkFailure>,
    installed_artwork: Option<ArtworkKey>,
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
        let mut selection_changed = false;
        if let Some(players) = update.players {
            if players.is_empty() {
                self.clear();
                return Ok(Report::Cleared);
            }
            if let Some(first) = self.select_player(&players) {
                selection_changed = true;
                self.last_position_poll = now;
                device.send(|id| magnetita_core::mpris::request_now_playing(id, &first))?;
            }
        }
        let Some(state) = update.state else {
            // Once the previous player vanished, clear its card until the
            // requested replacement snapshot arrives. Commands already target
            // the replacement, so keeping the old card would lie about which
            // player the controls operate.
            return Ok(report_without_state(selection_changed));
        };
        // A delayed report for a player that disappeared from the latest list
        // must not re-select it and retarget the controls behind the new card.
        if !self.accepts_state(&state) {
            return Ok(report_without_state(selection_changed));
        }
        self.player = Some(state.player.clone());
        self.playing = state.is_playing;
        if self.should_request_artwork(&state, now) {
            device.send(|id| {
                magnetita_core::request_album_art(id, &state.player, &state.album_art_url)
            })?;
        }
        Ok(Report::State(state))
    }

    pub fn send_action(&self, device: &mut Device, action: MediaAction) -> Result<(), LinkError> {
        if let Some(player) = self.player.as_deref() {
            device.send(|id| magnetita_core::mpris::action(id, player, action))?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.player = None;
        self.playing = false;
        self.requested_artwork = None;
        self.artwork_failure = None;
        self.installed_artwork = None;
    }

    fn select_player(&mut self, players: &[String]) -> Option<String> {
        if self
            .player
            .as_ref()
            .is_some_and(|current| players.contains(current))
        {
            return None;
        }

        let selected = players.first()?.clone();
        self.player = Some(selected.clone());
        self.playing = false;
        self.requested_artwork = None;
        self.artwork_failure = None;
        self.installed_artwork = None;
        Some(selected)
    }

    fn accepts_state(&self, state: &PlayerState) -> bool {
        self.player
            .as_deref()
            .is_none_or(|player| player == state.player)
    }

    pub fn artwork_succeeded(&mut self, player: &str, source_url: &str) {
        let key = ArtworkKey {
            player: player.to_owned(),
            source_url: source_url.to_owned(),
        };
        let tracked = self
            .requested_artwork
            .as_ref()
            .is_some_and(|request| request.key == key)
            || self
                .artwork_failure
                .as_ref()
                .is_some_and(|failure| failure.key == key);
        if !tracked {
            return;
        }
        self.requested_artwork = None;
        self.artwork_failure = None;
        self.installed_artwork = Some(key);
    }

    pub fn artwork_failed(&mut self, player: &str, source_url: &str, now: i64) {
        let key = ArtworkKey {
            player: player.to_owned(),
            source_url: source_url.to_owned(),
        };
        if self
            .requested_artwork
            .as_ref()
            .is_none_or(|request| request.key != key)
        {
            return;
        }

        self.record_artwork_failure(key, now);
    }

    fn record_artwork_failure(&mut self, key: ArtworkKey, now: i64) {
        self.requested_artwork = None;
        if self.installed_artwork.as_ref() == Some(&key) {
            self.installed_artwork = None;
        }
        let failures = self
            .artwork_failure
            .as_ref()
            .filter(|failure| failure.key == key)
            .map_or(1, |failure| failure.failures.saturating_add(1));
        let exponent = u32::from(failures.saturating_sub(1).min(5));
        let delay = ARTWORK_RETRY_BASE_MS
            .saturating_mul(1_i64 << exponent)
            .min(ARTWORK_RETRY_MAX_MS);
        self.artwork_failure = Some(ArtworkFailure {
            key,
            failures,
            retry_after: now.saturating_add(delay),
        });
    }

    fn should_request_artwork(&mut self, state: &PlayerState, now: i64) -> bool {
        if state.album_art_url.is_empty() {
            self.requested_artwork = None;
            self.artwork_failure = None;
            self.installed_artwork = None;
            return false;
        }
        if !self.supports_artwork {
            return false;
        }
        let key = ArtworkKey {
            player: state.player.clone(),
            source_url: state.album_art_url.clone(),
        };
        if self.installed_artwork.as_ref() == Some(&key) {
            return false;
        }
        self.installed_artwork = None;
        if let Some(request) = self.requested_artwork.as_ref() {
            if request.key == key {
                if now.saturating_sub(request.requested_at) < ARTWORK_REQUEST_TIMEOUT_MS {
                    return false;
                }
                self.record_artwork_failure(key, now);
                return false;
            }
        }
        if let Some(failure) = self.artwork_failure.as_ref() {
            if failure.key == key {
                if now < failure.retry_after {
                    return false;
                }
            } else {
                self.artwork_failure = None;
            }
        }
        self.requested_artwork = Some(ArtworkRequest {
            key,
            requested_at: now,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{report_without_state, RemoteMedia, Report};
    use magnetita_core::PlayerState;

    fn state_for(player: &str, url: &str) -> PlayerState {
        PlayerState {
            player: player.to_owned(),
            album_art_url: url.to_owned(),
            ..PlayerState::default()
        }
    }

    fn state(url: &str) -> PlayerState {
        state_for("Player", url)
    }

    #[test]
    fn artwork_requests_require_support_and_are_deduplicated() {
        let mut media = RemoteMedia::default();
        assert!(!media.should_request_artwork(&state("cover-a"), 0));
        media.supports_artwork = true;
        assert!(media.should_request_artwork(&state("cover-a"), 0));
        assert!(!media.should_request_artwork(&state("cover-a"), 0));
        assert!(media.should_request_artwork(&state("cover-b"), 0));
    }

    #[test]
    fn an_empty_cover_resets_the_request_key() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        assert!(media.should_request_artwork(&state("cover-a"), 0));
        assert!(!media.should_request_artwork(&state(""), 0));
        assert!(media.should_request_artwork(&state("cover-a"), 0));
    }

    #[test]
    fn a_vanished_player_selects_the_first_current_player() {
        let mut media = RemoteMedia {
            player: Some("A".to_owned()),
            playing: true,
            requested_artwork: Some(super::ArtworkRequest {
                key: super::ArtworkKey {
                    player: "A".to_owned(),
                    source_url: "old-cover".to_owned(),
                },
                requested_at: 0,
            }),
            ..RemoteMedia::default()
        };
        assert_eq!(
            media.select_player(&["B".to_owned(), "C".to_owned()]),
            Some("B".to_owned())
        );
        assert_eq!(media.player.as_deref(), Some("B"));
        assert!(!media.playing);
        assert!(media.requested_artwork.is_none());
    }

    #[test]
    fn a_current_player_is_not_reselected() {
        let mut media = RemoteMedia {
            player: Some("B".to_owned()),
            ..RemoteMedia::default()
        };
        assert_eq!(media.select_player(&["A".to_owned(), "B".to_owned()]), None);
    }

    #[test]
    fn a_delayed_state_cannot_reselect_a_vanished_player() {
        let media = RemoteMedia {
            player: Some("B".to_owned()),
            ..RemoteMedia::default()
        };
        let stale = state_for("A", "old-cover");
        assert!(!media.accepts_state(&stale));
        assert_eq!(media.player.as_deref(), Some("B"));
    }

    #[test]
    fn a_reselected_player_clears_the_stale_card_until_its_state_arrives() {
        assert!(matches!(report_without_state(true), Report::Cleared));
        assert!(matches!(report_without_state(false), Report::NoChange));
    }

    #[test]
    fn a_failed_artwork_transfer_rearms_only_the_matching_source() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        assert!(media.should_request_artwork(&state("cover-a"), 0));
        media.artwork_failed("Player", "old-cover", 100);
        assert!(!media.should_request_artwork(&state("cover-a"), 100));
        media.artwork_failed("Player", "cover-a", 100);
        assert!(!media.should_request_artwork(&state("cover-a"), 2099));
        assert!(media.should_request_artwork(&state("cover-a"), 2100));
    }

    #[test]
    fn repeated_artwork_failures_back_off_with_a_bound() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        let state = state("cover-a");
        let mut now = 0;
        for expected_delay in [2000, 4000, 8000, 16_000, 32_000, 60_000, 60_000] {
            assert!(media.should_request_artwork(&state, now));
            media.artwork_failed("Player", "cover-a", now);
            assert!(!media.should_request_artwork(&state, now + expected_delay - 1));
            now += expected_delay;
        }
    }

    #[test]
    fn the_artwork_identity_includes_the_player() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        assert!(media.should_request_artwork(&state_for("Player A", "cover"), 0));
        assert!(media.should_request_artwork(&state_for("Player B", "cover"), 1));
    }

    #[test]
    fn an_unanswered_artwork_request_times_out_then_backs_off() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        let state = state("cover-a");
        assert!(media.should_request_artwork(&state, 0));
        assert!(!media.should_request_artwork(&state, 9_999));
        assert!(!media.should_request_artwork(&state, 10_000));
        assert!(!media.should_request_artwork(&state, 11_999));
        assert!(media.should_request_artwork(&state, 12_000));
    }

    #[test]
    fn installed_artwork_stays_deduplicated_after_the_request_timeout() {
        let mut media = RemoteMedia {
            supports_artwork: true,
            ..RemoteMedia::default()
        };
        let state = state("cover-a");
        assert!(media.should_request_artwork(&state, 0));
        media.artwork_succeeded("Player", "cover-a");
        assert!(!media.should_request_artwork(&state, 10_000));
        assert!(!media.should_request_artwork(&state, 120_000));
        assert!(media.should_request_artwork(&state_for("Player", "cover-b"), 120_001));
    }
}
