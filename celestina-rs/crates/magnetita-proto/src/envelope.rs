//! The envelope: one message of one capability, on one wire, with a version.
//!
//! Every byte sequence the link exchanges is one envelope: a CBOR map with
//! five integer keys. `body` is opaque here — a byte string the capability's
//! own module encodes and decodes — so the envelope can be routed without
//! understanding it, and a capability can change its messages without
//! touching the envelope's vector.
//!
//! ```text
//! {0: version u8, 1: capability u16, 2: kind u16, 3: id u32, 4: body bytes}
//! ```
//!
//! Unknown keys are skipped. A missing key is refused. An unsupported
//! version is refused before anything else is looked at, which is what makes
//! version the first key.

use minicbor::{Decoder, Encoder};

use crate::bound::{self, MAX_BYTES};
use crate::error::DecodeError;

/// The wire version this build speaks. Bumped only for a change that neither
/// an added key nor an added capability can express.
pub const PROTOCOL_VERSION: u8 = 1;

const KEY_VERSION: u32 = 0;
const KEY_CAPABILITY: u32 = 1;
const KEY_KIND: u32 = 2;
const KEY_ID: u32 = 3;
const KEY_BODY: u32 = 4;

/// One message on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    /// Which capability's module owns `body`; see [`crate::capability`].
    pub capability: u16,
    /// Which message of that capability this is; each module names its kinds.
    pub kind: u16,
    /// A per-connection counter, so a reply can name the message it answers.
    pub id: u32,
    /// The capability's encoded message, at most [`MAX_BYTES`] long.
    pub body: Vec<u8>,
}

impl Envelope {
    /// Encodes with the version this build speaks.
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        // Writing to a Vec cannot fail; the unwraps are the encoder's
        // Infallible error type, not a runtime possibility.
        e.map(5)
            .unwrap()
            .u32(KEY_VERSION)
            .unwrap()
            .u8(PROTOCOL_VERSION)
            .unwrap()
            .u32(KEY_CAPABILITY)
            .unwrap()
            .u16(self.capability)
            .unwrap()
            .u32(KEY_KIND)
            .unwrap()
            .u16(self.kind)
            .unwrap()
            .u32(KEY_ID)
            .unwrap()
            .u32(self.id)
            .unwrap()
            .u32(KEY_BODY)
            .unwrap()
            .bytes(&self.body)
            .unwrap();
        e.into_writer()
    }

    /// Decodes one envelope, refusing the wrong version, a missing key, an
    /// oversized body, or bytes that are not this shape.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        bound::check_message_size(bytes)?;
        let mut d = Decoder::new(bytes);
        let pairs = bound::map(&mut d, "envelope", 16)?;
        let (mut version, mut capability, mut kind, mut id, mut body) =
            (None, None, None, None, None);
        for _ in 0..pairs {
            match bound::key(&mut d)? {
                KEY_VERSION => {
                    let v = bound::u8(&mut d, "version")?;
                    if v != PROTOCOL_VERSION {
                        return Err(DecodeError::UnsupportedVersion(v));
                    }
                    version = Some(v);
                }
                KEY_CAPABILITY => capability = Some(bound::u16(&mut d, "capability")?),
                KEY_KIND => kind = Some(bound::u16(&mut d, "kind")?),
                KEY_ID => id = Some(bound::u32(&mut d, "id")?),
                KEY_BODY => body = Some(bound::bytes(&mut d, "body", MAX_BYTES)?),
                _ => bound::skip(&mut d)?,
            }
        }
        version.ok_or(DecodeError::MissingField("version"))?;
        Ok(Self {
            capability: capability.ok_or(DecodeError::MissingField("capability"))?,
            kind: kind.ok_or(DecodeError::MissingField("kind"))?,
            id: id.ok_or(DecodeError::MissingField("id"))?,
            body: body.ok_or(DecodeError::MissingField("body"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The committed wire bytes of one envelope. Changing them is a protocol
    /// change and must bump [`PROTOCOL_VERSION`] or add a key, never edit this.
    const VECTOR: &str = "a500010101021903e8031a00bc614e0443010203";

    fn sample() -> Envelope {
        Envelope {
            capability: 1,
            kind: 1000,
            id: 12_345_678,
            body: vec![1, 2, 3],
        }
    }

    #[test]
    fn golden_vector_round_trips() {
        assert_eq!(hex(&sample().encode()), VECTOR);
        assert_eq!(Envelope::decode(&unhex(VECTOR)).unwrap(), sample());
    }

    #[test]
    fn an_unknown_key_is_skipped() {
        // Same as the vector plus {9: "future"} — six pairs, new key ignored.
        let mut bytes = unhex(
            "a6000101010219 03e8031a00bc614e0443010203"
                .replace(' ', "")
                .as_str(),
        );
        bytes.extend_from_slice(&unhex("0966667574757265"));
        assert_eq!(Envelope::decode(&bytes).unwrap(), sample());
    }

    #[test]
    fn a_future_version_is_refused_first() {
        let mut bytes = unhex(VECTOR);
        bytes[2] = 2; // the version value, right after key 0
        assert_eq!(
            Envelope::decode(&bytes),
            Err(DecodeError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn a_missing_key_is_refused() {
        // Four pairs: no body.
        let bytes = unhex("a400010101021903e8031a00bc614e");
        assert_eq!(
            Envelope::decode(&bytes),
            Err(DecodeError::MissingField("body"))
        );
    }

    #[test]
    fn an_oversized_body_is_refused_before_it_is_copied() {
        let mut e = Encoder::new(Vec::new());
        e.map(5)
            .unwrap()
            .u32(0)
            .unwrap()
            .u8(1)
            .unwrap()
            .u32(1)
            .unwrap()
            .u16(1)
            .unwrap();
        e.u32(2)
            .unwrap()
            .u16(1)
            .unwrap()
            .u32(3)
            .unwrap()
            .u32(1)
            .unwrap()
            .u32(4)
            .unwrap();
        // Header claims MAX_BYTES + 1 bytes of body; the buffer holds none.
        e.bytes_len(MAX_BYTES as u64 + 1).unwrap();
        let bytes = e.into_writer();
        assert_eq!(
            Envelope::decode(&bytes),
            Err(DecodeError::Malformed("cbor"))
        );
        // With the payload present the header is refused on its own, before
        // a byte of the body is copied.
        let mut full = bytes.clone();
        full.extend(std::iter::repeat_n(0u8, MAX_BYTES + 1));
        assert_eq!(
            Envelope::decode(&full),
            Err(DecodeError::TooLong {
                what: "body",
                max: MAX_BYTES,
                len: MAX_BYTES + 1
            })
        );
        // And an input larger than any message is refused before decoding.
        let huge = vec![0u8; bound::MAX_MESSAGE + 1];
        assert_eq!(
            Envelope::decode(&huge),
            Err(DecodeError::Oversized {
                max: bound::MAX_MESSAGE,
                len: huge.len()
            })
        );
    }

    #[test]
    fn a_body_at_the_bound_is_accepted() {
        let big = Envelope {
            body: vec![7; MAX_BYTES],
            ..sample()
        };
        assert_eq!(Envelope::decode(&big.encode()).unwrap(), big);
    }

    #[test]
    fn truncated_bytes_are_refused() {
        let bytes = unhex(VECTOR);
        assert_eq!(
            Envelope::decode(&bytes[..bytes.len() - 1]),
            Err(DecodeError::Malformed("cbor"))
        );
    }

    #[test]
    fn not_a_map_is_refused() {
        assert_eq!(
            Envelope::decode(&[0x01]),
            Err(DecodeError::Malformed("cbor"))
        );
    }
}
