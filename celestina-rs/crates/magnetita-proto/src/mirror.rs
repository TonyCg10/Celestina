//! `mirror` (capability 9): the phone's screen on the desktop, and the
//! desktop's touches on the phone. Video and audio travel on their own QUIC
//! unidirectional streams as raw elementary streams; these messages open,
//! describe and close them, and carry input back.

use crate::bound;
use crate::codec::{self, count, required, Map};
use crate::error::DecodeError;

/// Which encoder the phone should use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Hevc,
    H264,
}

/// Desktop → phone: capture and stream. Kind 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorStart {
    /// Longest edge the phone should scale to; 0 for native.
    pub max_size: u16,
    pub fps: u8,
    pub bitrate_kbps: u32,
    pub codec: Codec,
    pub audio: bool,
}

/// Phone → desktop: streaming, with what the encoder actually produces.
/// The video stream's first bytes are the codec configuration. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorStarted {
    pub width: u16,
    pub height: u16,
    pub codec: Codec,
    /// Present when an audio stream was opened too.
    pub audio: bool,
}

/// Either direction: stop. Kind 3, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MirrorStop;

/// Desktop → phone: send a key frame now, so a window that opens or
/// reopens mid-stream decodes from its next frame. Kind 7, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MirrorKeyframe;

/// A touch phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
}

/// Desktop → phone: a touch, in the phone's native pixels. Kind 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorTouch {
    pub action: TouchAction,
    pub x: u16,
    pub y: u16,
    /// Finger index for multi-touch; 0 for the first.
    pub pointer: u8,
}

/// Desktop → phone: an Android key code went down or up. Kind 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorKey {
    pub keycode: u16,
    pub pressed: bool,
}

/// A navigation gesture the phone performs as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalAction {
    Back,
    Home,
    Recents,
}

/// Desktop → phone: navigation. Kind 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorGlobal {
    pub action: GlobalAction,
}

impl Codec {
    fn to_wire(self) -> u8 {
        match self {
            Self::Hevc => 0,
            Self::H264 => 1,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Hevc,
            1 => Self::H264,
            _ => return Err(DecodeError::OutOfRange("codec")),
        })
    }
}

impl TouchAction {
    fn to_wire(self) -> u8 {
        match self {
            Self::Down => 0,
            Self::Move => 1,
            Self::Up => 2,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Down,
            1 => Self::Move,
            2 => Self::Up,
            _ => return Err(DecodeError::OutOfRange("action")),
        })
    }
}

impl GlobalAction {
    fn to_wire(self) -> u8 {
        match self {
            Self::Back => 0,
            Self::Home => 1,
            Self::Recents => 2,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Back,
            1 => Self::Home,
            2 => Self::Recents,
            _ => return Err(DecodeError::OutOfRange("action")),
        })
    }
}

impl MirrorStart {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(5)
            .u16(0, self.max_size)
            .u8(1, self.fps)
            .u32(2, self.bitrate_kbps)
            .u8(3, self.codec.to_wire())
            .bool(4, self.audio)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut size, mut fps, mut bitrate, mut codec, mut audio) = (None, None, None, None, None);
        codec::read(body, "mirror start", |k, d| {
            match k {
                0 => size = Some(bound::u16(d, "max_size")?),
                1 => fps = Some(bound::u8(d, "fps")?),
                2 => bitrate = Some(bound::u32(d, "bitrate_kbps")?),
                3 => codec = Some(Codec::from_wire(bound::u8(d, "codec")?)?),
                4 => audio = Some(bound::bool(d, "audio")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let fps = required(fps, "fps")?;
        if fps == 0 {
            return Err(DecodeError::OutOfRange("fps"));
        }
        Ok(Self {
            max_size: size.unwrap_or(0),
            fps,
            bitrate_kbps: required(bitrate, "bitrate_kbps")?,
            codec: required(codec, "codec")?,
            audio: audio.unwrap_or(false),
        })
    }
}

impl MirrorStarted {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(count(3, &[self.audio]))
            .u16(0, self.width)
            .u16(1, self.height)
            .u8(2, self.codec.to_wire())
            .finish_with_audio(self.audio)
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut width, mut height, mut codec, mut audio) = (None, None, None, false);
        codec::read(body, "mirror started", |k, d| {
            match k {
                0 => width = Some(bound::u16(d, "width")?),
                1 => height = Some(bound::u16(d, "height")?),
                2 => codec = Some(Codec::from_wire(bound::u8(d, "codec")?)?),
                3 => audio = bound::bool(d, "audio")?,
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let (width, height) = (required(width, "width")?, required(height, "height")?);
        if width == 0 || height == 0 {
            return Err(DecodeError::OutOfRange("size"));
        }
        Ok(Self {
            width,
            height,
            codec: required(codec, "codec")?,
            audio,
        })
    }
}

trait FinishWithAudio {
    fn finish_with_audio(self, audio: bool) -> Vec<u8>;
}

impl FinishWithAudio for Map {
    fn finish_with_audio(self, audio: bool) -> Vec<u8> {
        if audio {
            self.bool(3, true).finish()
        } else {
            self.finish()
        }
    }
}

impl MirrorStop {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "mirror stop", |_, _| Ok(false))?;
        Ok(Self)
    }
}

impl MirrorKeyframe {
    pub const KIND: u16 = 7;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "mirror keyframe", |_, _| Ok(false))?;
        Ok(Self)
    }
}

impl MirrorTouch {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(4)
            .u8(0, self.action.to_wire())
            .u16(1, self.x)
            .u16(2, self.y)
            .u8(3, self.pointer)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut action, mut x, mut y, mut pointer) = (None, None, None, 0);
        codec::read(body, "touch", |k, d| {
            match k {
                0 => action = Some(TouchAction::from_wire(bound::u8(d, "action")?)?),
                1 => x = Some(bound::u16(d, "x")?),
                2 => y = Some(bound::u16(d, "y")?),
                3 => pointer = bound::u8(d, "pointer")?,
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            action: required(action, "action")?,
            x: required(x, "x")?,
            y: required(y, "y")?,
            pointer,
        })
    }
}

impl MirrorKey {
    pub const KIND: u16 = 5;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u16(0, self.keycode)
            .bool(1, self.pressed)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut keycode, mut pressed) = (None, None);
        codec::read(body, "mirror key", |k, d| {
            match k {
                0 => keycode = Some(bound::u16(d, "keycode")?),
                1 => pressed = Some(bound::bool(d, "pressed")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            keycode: required(keycode, "keycode")?,
            pressed: required(pressed, "pressed")?,
        })
    }
}

impl MirrorGlobal {
    pub const KIND: u16 = 6;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).u8(0, self.action.to_wire()).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut action = None;
        codec::read(body, "global", |k, d| {
            match k {
                0 => action = Some(GlobalAction::from_wire(bound::u8(d, "action")?)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            action: required(action, "action")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const START: &str = "a50019043801183c02192ee0030004f5";

    #[test]
    fn golden_vectors_round_trip() {
        let s = MirrorStart {
            max_size: 1080,
            fps: 60,
            bitrate_kbps: 12_000,
            codec: Codec::Hevc,
            audio: true,
        };
        assert_eq!(hex(&s.encode()), START);
        assert_eq!(MirrorStart::decode(&unhex(START)).unwrap(), s);
        let started = MirrorStarted {
            width: 1080,
            height: 2340,
            codec: Codec::Hevc,
            audio: false,
        };
        assert_eq!(hex(&started.encode()), "a300190438011909240200");
        assert_eq!(MirrorStarted::decode(&started.encode()).unwrap(), started);
        let with_audio = MirrorStarted {
            audio: true,
            ..started
        };
        assert_eq!(
            MirrorStarted::decode(&with_audio.encode()).unwrap(),
            with_audio
        );
        let t = MirrorTouch {
            action: TouchAction::Down,
            x: 720,
            y: 1560,
            pointer: 0,
        };
        assert_eq!(hex(&t.encode()), "a40000011902d0021906180300");
        assert_eq!(MirrorTouch::decode(&t.encode()).unwrap(), t);
        let k = MirrorKey {
            keycode: 4,
            pressed: true,
        };
        assert_eq!(MirrorKey::decode(&k.encode()).unwrap(), k);
        let g = MirrorGlobal {
            action: GlobalAction::Home,
        };
        assert_eq!(hex(&g.encode()), "a10001");
        assert_eq!(MirrorGlobal::decode(&g.encode()).unwrap(), g);
        assert_eq!(
            MirrorStop::decode(&MirrorStop.encode()).unwrap(),
            MirrorStop
        );
    }

    #[test]
    fn nonsense_numbers_are_refused() {
        let zero_fps = MirrorStart {
            fps: 0,
            ..MirrorStart {
                max_size: 0,
                fps: 1,
                bitrate_kbps: 1,
                codec: Codec::H264,
                audio: false,
            }
        };
        assert_eq!(
            MirrorStart::decode(&zero_fps.encode()),
            Err(DecodeError::OutOfRange("fps"))
        );
        let flat = MirrorStarted {
            width: 0,
            height: 10,
            codec: Codec::Hevc,
            audio: false,
        };
        assert_eq!(
            MirrorStarted::decode(&flat.encode()),
            Err(DecodeError::OutOfRange("size"))
        );
        assert_eq!(
            MirrorGlobal::decode(&Map::new(1).u8(0, 3).finish()),
            Err(DecodeError::OutOfRange("action"))
        );
        assert_eq!(
            MirrorStart::decode(
                &Map::new(5)
                    .u16(0, 0)
                    .u8(1, 30)
                    .u32(2, 1)
                    .u8(3, 2)
                    .bool(4, false)
                    .finish()
            ),
            Err(DecodeError::OutOfRange("codec"))
        );
    }
}
