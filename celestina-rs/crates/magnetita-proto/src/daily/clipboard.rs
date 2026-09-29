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

/// Whether `text` is clipboard content worth syncing, in either direction:
/// the one rule both ends and both directions share, under the wire's
/// [`MAX_CLIPBOARD`] bound.
///
/// Empty carries nothing. A NUL means some layer decoded bytes that were never
/// text — a lossy UTF-8 decode of an image selection produces exactly that, a
/// string of replacement characters that still carries the original NULs — and
/// such a value must not reach a peer under any framing. Oversized content is
/// refused rather than truncated: half a clipboard is not what was copied.
pub fn syncable(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CLIPBOARD && !text.contains('\0')
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

    fn lossily_decoded_binary() -> String {
        "\u{fffd}PNG\r\n\u{1a}\n\0\0\0\rIHDR\0\0\0@".to_owned()
    }

    #[test]
    fn ordinary_copied_text_up_to_the_wire_bound_is_syncable() {
        assert!(syncable("hello world"));
        assert!(syncable("several\nlines\tand tabs, and \u{e9}"));
        assert!(syncable(&"a".repeat(MAX_CLIPBOARD)));
    }

    /// MAG-19: one bound. A text the wire carries is a text either end syncs.
    #[test]
    fn the_sync_rule_and_the_wire_share_one_bound() {
        let at_bound = ClipboardText {
            text: "a".repeat(MAX_CLIPBOARD),
        };
        let decoded = ClipboardText::decode(&at_bound.encode()).unwrap();
        assert!(syncable(&decoded.text));
        assert!(!syncable(&"a".repeat(MAX_CLIPBOARD + 1)));
    }

    #[test]
    fn empty_and_nul_carrying_content_is_not_syncable() {
        assert!(!syncable(""));
        assert!(!syncable("\0"));
        assert!(!syncable("text\0with a nul"));
        assert!(!syncable(&lossily_decoded_binary()));
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
