//! The encodings Grafita can map back to the user's bytes without guessing.
//!
//! Only reversible mappings live here. A byte stream is editable when decoding
//! it and re-encoding the result reproduces the original bytes by construction,
//! which is why the first milestone carries UTF-8 and the two BOM-marked UTF-16
//! forms and nothing that would need statistical detection.

use std::error::Error;
use std::fmt;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16BE_BOM: &[u8] = &[0xFE, 0xFF];

/// A reversible text encoding Grafita can both read and write.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Encoding {
    /// UTF-8 with no byte-order mark: the common case for source and config.
    Utf8,
    /// UTF-8 preceded by a byte-order mark, which saving must preserve.
    Utf8Bom,
    /// UTF-16 little endian, only ever accepted with its byte-order mark.
    Utf16Le,
    /// UTF-16 big endian, only ever accepted with its byte-order mark.
    Utf16Be,
}

impl Encoding {
    /// The byte-order mark this encoding writes back, empty for plain UTF-8.
    #[must_use]
    pub const fn byte_order_mark(self) -> &'static [u8] {
        match self {
            Self::Utf8 => &[],
            Self::Utf8Bom => UTF8_BOM,
            Self::Utf16Le => UTF16LE_BOM,
            Self::Utf16Be => UTF16BE_BOM,
        }
    }

    /// A stable identifier for logs and host-visible state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
        }
    }

    /// Reads the encoding a byte-order mark declares, if any.
    ///
    /// Absence of a mark is not an answer: plain UTF-8 is only concluded once
    /// [`Encoding::decode`] proves the bytes actually decode.
    #[must_use]
    pub fn from_byte_order_mark(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(UTF8_BOM) {
            Some(Self::Utf8Bom)
        } else if bytes.starts_with(UTF16LE_BOM) {
            Some(Self::Utf16Le)
        } else if bytes.starts_with(UTF16BE_BOM) {
            Some(Self::Utf16Be)
        } else {
            None
        }
    }

    /// Decodes a complete file, byte-order mark included, into text.
    pub fn decode(self, bytes: &[u8]) -> Result<String, DecodeError> {
        let mark = self.byte_order_mark();
        let body = match bytes.strip_prefix(mark) {
            Some(body) => body,
            None if mark.is_empty() => bytes,
            None => return Err(DecodeError::MissingByteOrderMark { encoding: self }),
        };
        match self {
            Self::Utf8 | Self::Utf8Bom => decode_utf8(body, mark.len()),
            Self::Utf16Le => decode_utf16(body, mark.len(), u16::from_le_bytes),
            Self::Utf16Be => decode_utf16(body, mark.len(), u16::from_be_bytes),
        }
    }

    /// Encodes text back into the file's bytes, byte-order mark included.
    ///
    /// Text that came from [`Encoding::decode`] and was not edited re-encodes to
    /// the original bytes: UTF-8 is copied verbatim and UTF-16 round-trips
    /// because decoding rejected anything but well-formed unit sequences.
    #[must_use]
    pub fn encode(self, text: &str) -> Vec<u8> {
        let mark = self.byte_order_mark();
        match self {
            Self::Utf8 | Self::Utf8Bom => {
                let mut bytes = Vec::with_capacity(mark.len() + text.len());
                bytes.extend_from_slice(mark);
                bytes.extend_from_slice(text.as_bytes());
                bytes
            }
            Self::Utf16Le | Self::Utf16Be => {
                let mut bytes = Vec::with_capacity(mark.len() + text.len() * 2);
                bytes.extend_from_slice(mark);
                for unit in text.encode_utf16() {
                    if self == Self::Utf16Le {
                        bytes.extend_from_slice(&unit.to_le_bytes());
                    } else {
                        bytes.extend_from_slice(&unit.to_be_bytes());
                    }
                }
                bytes
            }
        }
    }
}

fn decode_utf8(body: &[u8], mark_length: usize) -> Result<String, DecodeError> {
    match std::str::from_utf8(body) {
        Ok(text) => Ok(text.to_owned()),
        Err(error) => Err(DecodeError::InvalidUtf8 {
            offset: mark_length + error.valid_up_to(),
            truncated: error.error_len().is_none(),
        }),
    }
}

fn decode_utf16(
    body: &[u8],
    mark_length: usize,
    read: fn([u8; 2]) -> u16,
) -> Result<String, DecodeError> {
    if body.len() % 2 != 0 {
        return Err(DecodeError::OddUtf16Length {
            length: mark_length + body.len(),
        });
    }
    let units = body.chunks_exact(2).map(|pair| read([pair[0], pair[1]]));
    let mut text = String::with_capacity(body.len() / 2);
    for (index, decoded) in char::decode_utf16(units).enumerate() {
        match decoded {
            Ok(character) => text.push(character),
            Err(_) => {
                return Err(DecodeError::UnpairedSurrogate {
                    offset: mark_length + index * 2,
                })
            }
        }
    }
    Ok(text)
}

/// Why a byte stream could not be mapped to text reversibly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// The bytes lack the byte-order mark the requested encoding needs.
    MissingByteOrderMark { encoding: Encoding },
    /// The bytes are not UTF-8 from `offset` on.
    ///
    /// `truncated` marks a sequence cut off by the end of the inspected slice,
    /// which is expected when only a prefix was read and is not, by itself,
    /// proof that the file is unreadable.
    InvalidUtf8 { offset: usize, truncated: bool },
    /// A UTF-16 stream ended in the middle of a code unit.
    OddUtf16Length { length: usize },
    /// A UTF-16 surrogate had no pair, so no reversible text exists.
    UnpairedSurrogate { offset: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingByteOrderMark { encoding } => write!(
                formatter,
                "the bytes do not carry the {} byte-order mark",
                encoding.label()
            ),
            Self::InvalidUtf8 {
                offset,
                truncated: true,
            } => write!(formatter, "a UTF-8 sequence is cut off at byte {offset}"),
            Self::InvalidUtf8 {
                offset,
                truncated: false,
            } => write!(formatter, "byte {offset} is not valid UTF-8"),
            Self::OddUtf16Length { length } => {
                write!(formatter, "a UTF-16 stream cannot have {length} bytes")
            }
            Self::UnpairedSurrogate { offset } => {
                write!(
                    formatter,
                    "an unpaired UTF-16 surrogate sits at byte {offset}"
                )
            }
        }
    }
}

impl Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::{DecodeError, Encoding};

    #[test]
    fn every_encoding_round_trips_its_own_bytes() {
        let cases = [
            (Encoding::Utf8, "línea\ntabla\t".to_owned()),
            (Encoding::Utf8Bom, "con marca\r\n".to_owned()),
            (Encoding::Utf16Le, "emoji 🜲 y ñ\n".to_owned()),
            (Encoding::Utf16Be, "emoji 🜲 y ñ\n".to_owned()),
        ];

        for (encoding, text) in cases {
            let bytes = encoding.encode(&text);
            assert!(
                bytes.starts_with(encoding.byte_order_mark()),
                "{encoding:?}"
            );
            assert_eq!(encoding.decode(&bytes), Ok(text), "{encoding:?}");
        }
    }

    #[test]
    fn byte_order_marks_are_recognised_and_plain_utf8_is_not_guessed() {
        assert_eq!(
            Encoding::from_byte_order_mark(&[0xEF, 0xBB, 0xBF, b'a']),
            Some(Encoding::Utf8Bom)
        );
        assert_eq!(
            Encoding::from_byte_order_mark(&[0xFF, 0xFE, b'a', 0]),
            Some(Encoding::Utf16Le)
        );
        assert_eq!(
            Encoding::from_byte_order_mark(&[0xFE, 0xFF, 0, b'a']),
            Some(Encoding::Utf16Be)
        );
        assert_eq!(Encoding::from_byte_order_mark(b"plain"), None);
    }

    #[test]
    fn malformed_streams_report_where_they_break() {
        assert_eq!(
            Encoding::Utf8.decode(&[b'a', 0xFF]),
            Err(DecodeError::InvalidUtf8 {
                offset: 1,
                truncated: false
            })
        );
        assert_eq!(
            Encoding::Utf8Bom.decode(&[0xEF, 0xBB, 0xBF, 0xC3]),
            Err(DecodeError::InvalidUtf8 {
                offset: 3,
                truncated: true
            })
        );
        assert_eq!(
            Encoding::Utf16Le.decode(&[0xFF, 0xFE, 0x41]),
            Err(DecodeError::OddUtf16Length { length: 3 })
        );
        assert_eq!(
            Encoding::Utf16Le.decode(&[0xFF, 0xFE, 0x00, 0xD8]),
            Err(DecodeError::UnpairedSurrogate { offset: 2 })
        );
        assert_eq!(
            Encoding::Utf8Bom.decode(b"no mark"),
            Err(DecodeError::MissingByteOrderMark {
                encoding: Encoding::Utf8Bom
            })
        );
    }
}
