//! `media` (capability 6): what is playing on either side, and the buttons.
//! The desktop projects its MPRIS players; the phone its `MediaSession`s.

use crate::bound::{self, MAX_IDENT, MAX_TEXT};
use crate::codec::{self, count, required, Map};
use crate::error::DecodeError;

/// Either direction: one player's state. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaState {
    /// The sender's name for the player, at most [`MAX_IDENT`] bytes.
    pub player: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: u64,
    /// 0 when unknown.
    pub length_ms: u64,
    pub can_seek: bool,
    pub can_next: bool,
    pub can_previous: bool,
    /// 0–100.
    pub volume: u8,
}

/// What a button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaButton {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
}

impl MediaButton {
    fn to_wire(self) -> u8 {
        match self {
            Self::Play => 0,
            Self::Pause => 1,
            Self::PlayPause => 2,
            Self::Next => 3,
            Self::Previous => 4,
            Self::Stop => 5,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Play,
            1 => Self::Pause,
            2 => Self::PlayPause,
            3 => Self::Next,
            4 => Self::Previous,
            5 => Self::Stop,
            _ => return Err(DecodeError::OutOfRange("button")),
        })
    }
}

/// Either direction: press a button, seek, or set the volume. Kind 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaCommand {
    pub player: String,
    pub button: Option<MediaButton>,
    pub seek_ms: Option<u64>,
    /// 0–100.
    pub volume: Option<u8>,
}

/// Either direction: send every player's state now. Kind 3, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MediaRequest;

impl MediaState {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(11)
            .text(0, &self.player)
            .text(1, &self.title)
            .text(2, &self.artist)
            .text(3, &self.album)
            .bool(4, self.playing)
            .u64(5, self.position_ms)
            .u64(6, self.length_ms)
            .bool(7, self.can_seek)
            .bool(8, self.can_next)
            .bool(9, self.can_previous)
            .u8(10, self.volume)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut player = None;
        let (mut title, mut artist, mut album) = (String::new(), String::new(), String::new());
        let (mut playing, mut position, mut length) = (false, 0, 0);
        let (mut seek, mut next, mut previous, mut volume) = (false, false, false, 0u8);
        codec::read(body, "media state", |k, d| {
            match k {
                0 => player = Some(bound::text(d, "player", MAX_IDENT)?),
                1 => title = bound::text(d, "title", MAX_TEXT)?,
                2 => artist = bound::text(d, "artist", MAX_TEXT)?,
                3 => album = bound::text(d, "album", MAX_TEXT)?,
                4 => playing = bound::bool(d, "playing")?,
                5 => position = bound::u64(d, "position_ms")?,
                6 => length = bound::u64(d, "length_ms")?,
                7 => seek = bound::bool(d, "can_seek")?,
                8 => next = bound::bool(d, "can_next")?,
                9 => previous = bound::bool(d, "can_previous")?,
                10 => volume = bound::u8(d, "volume")?,
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        if volume > 100 {
            return Err(DecodeError::OutOfRange("volume"));
        }
        Ok(Self {
            player: required(player, "player")?,
            title,
            artist,
            album,
            playing,
            position_ms: position,
            length_ms: length,
            can_seek: seek,
            can_next: next,
            can_previous: previous,
            volume,
        })
    }
}

impl MediaCommand {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(count(
            1,
            &[
                self.button.is_some(),
                self.seek_ms.is_some(),
                self.volume.is_some(),
            ],
        ))
        .text(0, &self.player);
        if let Some(b) = self.button {
            m = m.u8(1, b.to_wire());
        }
        if let Some(s) = self.seek_ms {
            m = m.u64(2, s);
        }
        if let Some(v) = self.volume {
            m = m.u8(3, v);
        }
        m.finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut player, mut button, mut seek, mut volume) = (None, None, None, None);
        codec::read(body, "media command", |k, d| {
            match k {
                0 => player = Some(bound::text(d, "player", MAX_IDENT)?),
                1 => button = Some(MediaButton::from_wire(bound::u8(d, "button")?)?),
                2 => seek = Some(bound::u64(d, "seek_ms")?),
                3 => volume = Some(bound::u8(d, "volume")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        if volume.is_some_and(|v| v > 100) {
            return Err(DecodeError::OutOfRange("volume"));
        }
        if button.is_none() && seek.is_none() && volume.is_none() {
            return Err(DecodeError::Malformed("empty command"));
        }
        Ok(Self {
            player: required(player, "player")?,
            button,
            seek_ms: seek,
            volume,
        })
    }
}

impl MediaRequest {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "media request", |_, _| Ok(false))?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    fn sample() -> MediaState {
        MediaState {
            player: "mpv".into(),
            title: "Onimusha".into(),
            artist: "BaityVods".into(),
            album: String::new(),
            playing: true,
            position_ms: 24_945_784,
            length_ms: 0,
            can_seek: true,
            can_next: false,
            can_previous: false,
            volume: 70,
        }
    }

    const VECTOR: &str = "ab00636d707601684f6e696d7573686102694261697479566f6473036004f5051a017ca478060007f508f409f40a1846";

    #[test]
    fn golden_vector_round_trips() {
        assert_eq!(hex(&sample().encode()), VECTOR);
        assert_eq!(MediaState::decode(&unhex(VECTOR)).unwrap(), sample());
    }

    #[test]
    fn a_state_needs_only_the_player() {
        let bytes = Map::new(1).text(0, "mpv").finish();
        let m = MediaState::decode(&bytes).unwrap();
        assert_eq!(m.player, "mpv");
        assert!(!m.playing && m.title.is_empty());
        assert_eq!(
            MediaState::decode(&Map::new(0).finish()),
            Err(DecodeError::MissingField("player"))
        );
    }

    #[test]
    fn commands_carry_what_they_carry_and_never_nothing() {
        let c = MediaCommand {
            player: "mpv".into(),
            button: Some(MediaButton::Next),
            seek_ms: None,
            volume: None,
        };
        assert_eq!(hex(&c.encode()), "a200636d70760103");
        assert_eq!(MediaCommand::decode(&c.encode()).unwrap(), c);
        let s = MediaCommand {
            player: "mpv".into(),
            button: None,
            seek_ms: Some(1500),
            volume: Some(100),
        };
        assert_eq!(MediaCommand::decode(&s.encode()).unwrap(), s);
        let empty = MediaCommand {
            player: "mpv".into(),
            button: None,
            seek_ms: None,
            volume: None,
        };
        assert_eq!(
            MediaCommand::decode(&empty.encode()),
            Err(DecodeError::Malformed("empty command"))
        );
        let loud = MediaCommand {
            volume: Some(101),
            ..s.clone()
        };
        assert_eq!(
            MediaCommand::decode(&loud.encode()),
            Err(DecodeError::OutOfRange("volume"))
        );
        let bytes = Map::new(2).text(0, "mpv").u8(1, 9).finish();
        assert_eq!(
            MediaCommand::decode(&bytes),
            Err(DecodeError::OutOfRange("button"))
        );
    }
}
