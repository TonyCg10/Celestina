//! `input` (capability 8): the phone as the desktop's trackpad and keyboard.
//! Motion may also travel as a QUIC datagram — same body, no envelope id —
//! because a stale sample is worth dropping; everything else is a stream.

use crate::bound::{self, MAX_TEXT};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// Phone → desktop: relative pointer motion in pixels. Kind 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerMove {
    pub dx: i16,
    pub dy: i16,
}

/// Which pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Phone → desktop: a button went down or up. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerButton {
    pub button: Button,
    pub pressed: bool,
}

/// Phone → desktop: scroll, in wheel steps of 1/120. Kind 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    pub dx: i16,
    pub dy: i16,
}

/// Phone → desktop: a key by Linux evdev code went down or up. Kind 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub code: u16,
    pub pressed: bool,
}

/// Phone → desktop: type this text, whatever the keymap. Kind 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub text: String,
}

impl Button {
    fn to_wire(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Middle => 2,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Left,
            1 => Self::Right,
            2 => Self::Middle,
            _ => return Err(DecodeError::OutOfRange("button")),
        })
    }
}

fn decode_pair(body: &[u8], what: &'static str) -> Result<(i16, i16), DecodeError> {
    let (mut x, mut y) = (None, None);
    codec::read(body, what, |k, d| {
        match k {
            0 => x = Some(bound::i16(d, "dx")?),
            1 => y = Some(bound::i16(d, "dy")?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((required(x, "dx")?, required(y, "dy")?))
}

impl PointerMove {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).i16(0, self.dx).i16(1, self.dy).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (dx, dy) = decode_pair(body, "pointer move")?;
        Ok(Self { dx, dy })
    }
}

impl Scroll {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).i16(0, self.dx).i16(1, self.dy).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (dx, dy) = decode_pair(body, "scroll")?;
        Ok(Self { dx, dy })
    }
}

impl PointerButton {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u8(0, self.button.to_wire())
            .bool(1, self.pressed)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut button, mut pressed) = (None, None);
        codec::read(body, "pointer button", |k, d| {
            match k {
                0 => button = Some(Button::from_wire(bound::u8(d, "button")?)?),
                1 => pressed = Some(bound::bool(d, "pressed")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            button: required(button, "button")?,
            pressed: required(pressed, "pressed")?,
        })
    }
}

impl Key {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).u16(0, self.code).bool(1, self.pressed).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut code, mut pressed) = (None, None);
        codec::read(body, "key", |k, d| {
            match k {
                0 => code = Some(bound::u16(d, "code")?),
                1 => pressed = Some(bound::bool(d, "pressed")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            code: required(code, "code")?,
            pressed: required(pressed, "pressed")?,
        })
    }
}

impl Text {
    pub const KIND: u16 = 5;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).text(0, &self.text).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut text = None;
        codec::read(body, "text", |k, d| {
            match k {
                0 => text = Some(bound::text(d, "text", MAX_TEXT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            text: required(text, "text")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::hex;

    #[test]
    fn every_event_round_trips_with_its_vector() {
        let m = PointerMove { dx: -3, dy: 12 };
        assert_eq!(hex(&m.encode()), "a20022010c");
        assert_eq!(PointerMove::decode(&m.encode()).unwrap(), m);
        let b = PointerButton {
            button: Button::Right,
            pressed: true,
        };
        assert_eq!(hex(&b.encode()), "a2000101f5");
        assert_eq!(PointerButton::decode(&b.encode()).unwrap(), b);
        let s = Scroll { dx: 0, dy: -120 };
        assert_eq!(Scroll::decode(&s.encode()).unwrap(), s);
        let k = Key {
            code: 30,
            pressed: false,
        };
        assert_eq!(hex(&k.encode()), "a200181e01f4");
        assert_eq!(Key::decode(&k.encode()).unwrap(), k);
        let t = Text {
            text: "caf\u{e9}".into(),
        };
        assert_eq!(Text::decode(&t.encode()).unwrap(), t);
    }

    #[test]
    fn motion_wider_than_i16_is_refused() {
        let bytes = Map::new(2).u32(0, 40_000).i16(1, 0).finish();
        assert_eq!(
            PointerMove::decode(&bytes),
            Err(DecodeError::OutOfRange("dx"))
        );
        assert_eq!(
            PointerButton::decode(&Map::new(2).u8(0, 3).bool(1, true).finish()),
            Err(DecodeError::OutOfRange("button"))
        );
    }
}
