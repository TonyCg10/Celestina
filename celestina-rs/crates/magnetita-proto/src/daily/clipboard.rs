//! `clipboard` (capability 2): text either way, and a request for the
//! phone's, which Android only honours while the app is in front.

use crate::bound::{self, MAX_CLIPBOARD};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// Either direction: the clipboard now holds this text. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardText {
    /// At most [`MAX_CLIPBOARD`] bytes.
    pub text: String,
}

impl ClipboardText {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).text(0, &self.text).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut text = None;
        codec::read(body, "clipboard", |k, d| {
            match k {
                0 => text = Some(bound::text(d, "text", MAX_CLIPBOARD)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            text: required(text, "text")?,
        })
    }
}

/// Desktop → phone: send your clipboard if you may. Kind 2, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ClipboardRequest;

impl ClipboardRequest {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "clipboard request", |_, _| Ok(false))?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a10065686f6c6120";

    #[test]
    fn golden_vector_round_trips() {
        let m = ClipboardText {
            text: "hola ".into(),
        };
        assert_eq!(hex(&m.encode()), VECTOR);
        assert_eq!(ClipboardText::decode(&unhex(VECTOR)).unwrap(), m);
    }

    #[test]
    fn text_over_the_clipboard_bound_is_refused() {
        let m = ClipboardText {
            text: "x".repeat(MAX_CLIPBOARD + 1),
        };
        assert_eq!(
            ClipboardText::decode(&m.encode()),
            Err(DecodeError::TooLong {
                what: "text",
                max: MAX_CLIPBOARD,
                len: MAX_CLIPBOARD + 1
            })
        );
    }
}
