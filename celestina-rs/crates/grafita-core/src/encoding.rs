//! The encodings Grafita can map back to the user's bytes without guessing.
//!
//! Only reversible mappings live here. A byte stream is editable when decoding
//! it and re-encoding the result reproduces the original bytes by construction,
//! which is why the Unicode forms carried are UTF-8 and the two BOM-marked
//! UTF-16 ones and nothing that would need statistical detection.
//!
//! The single-byte encodings satisfy the same rule for a different reason: each
//! is a table, and a table that maps distinct bytes to distinct characters is
//! reversible by inspection. That is why they are generated rather than
//! written, and why the generator refuses a table whose bytes collide. None of
//! them is ever concluded from a file, because nothing in the bytes says which
//! one it is; a caller names it.

mod multibyte;
mod tables;

use std::error::Error;
use std::fmt;

pub use multibyte::MultiByte;
pub use tables::SingleByte;

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
    /// UTF-16 little endian with no mark. Named, never concluded: an unmarked
    /// UTF-16 file looks like binary to the probe and there is nothing in it
    /// that proves the byte order.
    Utf16LeBare,
    /// UTF-16 big endian with no mark, on the same terms.
    Utf16BeBare,
    /// UTF-32 little endian, always named. It carries no mark here even though
    /// the encoding has one, because `FF FE 00 00` starts with the UTF-16 LE
    /// mark and a reader cannot tell them apart without looking further; this
    /// crate refuses to make that guess.
    Utf32LeBare,
    /// UTF-32 big endian, on the same terms.
    Utf32BeBare,
    /// One of the catalogued single-byte tables, which a caller names because
    /// no byte-order mark or byte pattern can prove it.
    SingleByte(SingleByte),
    /// One of the catalogued multi-byte encodings. Named like the tables, and
    /// additionally verified per document, because these are not bijective:
    /// only re-encoding the whole file proves this one reproduces it.
    MultiByte(MultiByte),
}

impl Encoding {
    /// The byte-order mark this encoding writes back, empty for plain UTF-8.
    #[must_use]
    pub const fn byte_order_mark(self) -> &'static [u8] {
        match self {
            Self::Utf8
            | Self::SingleByte(_)
            | Self::Utf16LeBare
            | Self::Utf16BeBare
            | Self::Utf32LeBare
            | Self::Utf32BeBare
            | Self::MultiByte(_) => &[],
            Self::Utf8Bom => UTF8_BOM,
            Self::Utf16Le => UTF16LE_BOM,
            Self::Utf16Be => UTF16BE_BOM,
        }
    }

    /// A stable identifier for logs and host-visible state.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Utf16LeBare => "UTF-16 LE unmarked",
            Self::Utf16BeBare => "UTF-16 BE unmarked",
            Self::Utf32LeBare => "UTF-32 LE",
            Self::Utf32BeBare => "UTF-32 BE",
            Self::SingleByte(table) => table.label(),
            Self::MultiByte(encoding) => encoding.label(),
        }
    }

    /// Every encoding a caller may name, in catalogue order.
    ///
    /// The Unicode forms come first because they are also the ones a file can
    /// prove for itself; the tables follow in the order the generator lists.
    #[must_use]
    pub fn catalogue() -> Vec<Self> {
        let mut all = vec![
            Self::Utf8,
            Self::Utf8Bom,
            Self::Utf16Le,
            Self::Utf16Be,
            Self::Utf16LeBare,
            Self::Utf16BeBare,
            Self::Utf32LeBare,
            Self::Utf32BeBare,
        ];
        all.extend(SingleByte::catalogue().map(Self::SingleByte));
        all.extend(MultiByte::catalogue().map(Self::MultiByte));
        all
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
            Self::Utf16Le | Self::Utf16LeBare => decode_utf16(body, mark.len(), u16::from_le_bytes),
            Self::Utf16Be | Self::Utf16BeBare => decode_utf16(body, mark.len(), u16::from_be_bytes),
            Self::Utf32LeBare => decode_utf32(body, u32::from_le_bytes),
            Self::Utf32BeBare => decode_utf32(body, u32::from_be_bytes),
            Self::SingleByte(table) => decode_single_byte(body, table),
            Self::MultiByte(encoding) => decode_multi_byte(body, encoding),
        }
    }

    /// Encodes text back into the file's bytes, byte-order mark included.
    ///
    /// Text that came from [`Encoding::decode`] and was not edited re-encodes to
    /// the original bytes: UTF-8 is copied verbatim, UTF-16 round-trips because
    /// decoding rejected anything but well-formed unit sequences, and a table
    /// round-trips because its characters are distinct.
    ///
    /// Editing can introduce a character the encoding has no byte for — an
    /// emoji typed into a `windows-1252` file. That is refused here rather than
    /// written as a substitute, because a substitute is exactly the silent loss
    /// the document contract exists to prevent. The Unicode encodings cannot
    /// fail; the signature is shared so no caller has to know which is which.
    pub fn encode(self, text: &str) -> Result<Vec<u8>, EncodeError> {
        let mark = self.byte_order_mark();
        match self {
            Self::Utf8 | Self::Utf8Bom => {
                let mut bytes = Vec::with_capacity(mark.len() + text.len());
                bytes.extend_from_slice(mark);
                bytes.extend_from_slice(text.as_bytes());
                Ok(bytes)
            }
            Self::Utf16Le | Self::Utf16Be | Self::Utf16LeBare | Self::Utf16BeBare => {
                let little = matches!(self, Self::Utf16Le | Self::Utf16LeBare);
                let mut bytes = Vec::with_capacity(mark.len() + text.len() * 2);
                bytes.extend_from_slice(mark);
                for unit in text.encode_utf16() {
                    if little {
                        bytes.extend_from_slice(&unit.to_le_bytes());
                    } else {
                        bytes.extend_from_slice(&unit.to_be_bytes());
                    }
                }
                Ok(bytes)
            }
            Self::Utf32LeBare | Self::Utf32BeBare => {
                let little = self == Self::Utf32LeBare;
                let mut bytes = Vec::with_capacity(text.len() * 4);
                for character in text.chars() {
                    let point = u32::from(character);
                    if little {
                        bytes.extend_from_slice(&point.to_le_bytes());
                    } else {
                        bytes.extend_from_slice(&point.to_be_bytes());
                    }
                }
                Ok(bytes)
            }
            Self::SingleByte(table) => encode_single_byte(text, table),
            Self::MultiByte(encoding) => encode_multi_byte(text, encoding),
        }
    }
}

impl MultiByte {
    /// A stable identifier for logs and host-visible state.
    #[must_use]
    pub fn label(self) -> &'static str {
        multibyte::label_of(self)
    }

    /// Every catalogued multi-byte encoding, in the generator's order.
    pub fn catalogue() -> impl Iterator<Item = Self> {
        multibyte::CATALOGUE.into_iter()
    }
}

/// Decodes a byte stream in one of the multi-byte encodings.
///
/// The first byte decides the length, which is possible because no catalogued
/// encoding uses a byte as both a character and a lead. A NUL is refused rather
/// than decoded: the table would map it to `U+0000`, and a NUL is this crate's
/// oldest evidence that a file is not text.
fn decode_multi_byte(body: &[u8], encoding: MultiByte) -> Result<String, DecodeError> {
    let singles = multibyte::singles_of(encoding);
    let pairs = multibyte::pairs_of(encoding);
    let mut text = String::with_capacity(body.len());
    let mut offset = 0;
    while offset < body.len() {
        let byte = body[offset];
        let point = singles[usize::from(byte)];
        if point != 0 {
            push_point(&mut text, point, offset)?;
            offset += 1;
            continue;
        }
        let Some(trail) = body.get(offset + 1) else {
            return Err(DecodeError::IncompleteSequence { offset });
        };
        let key = (u16::from(byte) << 8) | u16::from(*trail);
        match pairs.binary_search_by_key(&key, |(candidate, _point)| *candidate) {
            Ok(index) => push_point(&mut text, pairs[index].1, offset)?,
            Err(_) => {
                return Err(DecodeError::UnassignedSequence {
                    sequence: key,
                    encoding,
                    offset,
                })
            }
        }
        offset += 2;
    }
    Ok(text)
}

fn push_point(text: &mut String, point: u16, offset: usize) -> Result<(), DecodeError> {
    match char::from_u32(u32::from(point)) {
        Some(character) => {
            text.push(character);
            Ok(())
        }
        None => Err(DecodeError::NotAScalarValue {
            point: u32::from(point),
            offset,
        }),
    }
}

/// Encodes text in one of the multi-byte encodings.
///
/// The reverse map is built here rather than generated, for the same reason as
/// the single-byte one: one direction cannot then drift from the other. Where a
/// character has more than one encoding the shortest and then the lowest
/// sequence wins, deterministically; a document that used the other form fails
/// `open_with`'s byte comparison and is refused instead of being edited into a
/// file that does not match itself.
fn encode_multi_byte(text: &str, encoding: MultiByte) -> Result<Vec<u8>, EncodeError> {
    let singles = multibyte::singles_of(encoding);
    let pairs = multibyte::pairs_of(encoding);
    let mut reverse: Vec<(u16, u16)> = Vec::with_capacity(256 + pairs.len());
    for (byte, point) in singles.iter().enumerate() {
        if *point != 0 || byte == 0 {
            if let Ok(byte) = u16::try_from(byte) {
                reverse.push((*point, byte));
            }
        }
    }
    for (key, point) in pairs {
        reverse.push((*point, *key));
    }
    // Sorting by (character, sequence) makes the first entry for a character
    // the lowest sequence, and a single byte always sorts below a pair because
    // its value is below 0x0100.
    reverse.sort_unstable();

    let mut bytes = Vec::with_capacity(text.len());
    for (offset, character) in text.char_indices() {
        let Ok(point) = u16::try_from(u32::from(character)) else {
            return Err(EncodeError {
                character,
                offset,
                encoding: Encoding::MultiByte(encoding),
            });
        };
        let found = reverse
            .binary_search_by_key(&(point, 0), |(candidate, sequence)| (*candidate, *sequence))
            .unwrap_or_else(|index| index);
        match reverse.get(found) {
            Some((candidate, sequence)) if *candidate == point => {
                if *sequence <= 0xFF {
                    bytes.push(u8::try_from(*sequence).map_err(|_| EncodeError {
                        character,
                        offset,
                        encoding: Encoding::MultiByte(encoding),
                    })?);
                } else {
                    bytes.extend_from_slice(&sequence.to_be_bytes());
                }
            }
            _ => {
                return Err(EncodeError {
                    character,
                    offset,
                    encoding: Encoding::MultiByte(encoding),
                })
            }
        }
    }
    Ok(bytes)
}

impl SingleByte {
    /// A stable identifier for logs and host-visible state.
    #[must_use]
    pub fn label(self) -> &'static str {
        tables::label_of(self)
    }

    /// Every catalogued table, in the order the generator lists them.
    pub fn catalogue() -> impl Iterator<Item = Self> {
        tables::CATALOGUE.into_iter()
    }

    fn table(self) -> &'static [char; 128] {
        tables::table_of(self)
    }
}

fn decode_utf32(body: &[u8], read: fn([u8; 4]) -> u32) -> Result<String, DecodeError> {
    if body.len() % 4 != 0 {
        return Err(DecodeError::OddUtf32Length { length: body.len() });
    }
    let mut text = String::with_capacity(body.len() / 4);
    for (index, quad) in body.chunks_exact(4).enumerate() {
        let point = read([quad[0], quad[1], quad[2], quad[3]]);
        match char::from_u32(point) {
            Some(character) => text.push(character),
            None => {
                return Err(DecodeError::NotAScalarValue {
                    point,
                    offset: index * 4,
                })
            }
        }
    }
    Ok(text)
}

fn decode_single_byte(body: &[u8], table: SingleByte) -> Result<String, DecodeError> {
    let characters = table.table();
    let mut text = String::with_capacity(body.len());
    for (offset, byte) in body.iter().enumerate() {
        let Some(index) = usize::from(*byte).checked_sub(0x80) else {
            text.push(char::from(*byte));
            continue;
        };
        let character = characters[index];
        if character == tables::UNASSIGNED {
            return Err(DecodeError::UnassignedByte {
                byte: *byte,
                encoding: table,
                offset,
            });
        }
        text.push(character);
    }
    Ok(text)
}

fn encode_single_byte(text: &str, table: SingleByte) -> Result<Vec<u8>, EncodeError> {
    // The reverse map is built per call rather than generated. A table has 128
    // entries, so sorting them costs less than the read that produced the text,
    // and one direction of the mapping cannot drift from the other.
    let mut reverse: [(char, u8); 128] = [('\0', 0); 128];
    for (index, character) in table.table().iter().enumerate() {
        let byte = u8::try_from(0x80 + index).unwrap_or(u8::MAX);
        reverse[index] = (*character, byte);
    }
    reverse.sort_unstable_by_key(|(character, _)| *character);

    let mut bytes = Vec::with_capacity(text.len());
    for (offset, character) in text.char_indices() {
        if character.is_ascii() {
            bytes.push(character as u8);
            continue;
        }
        match reverse.binary_search_by_key(&character, |(character, _)| *character) {
            Ok(index) if reverse[index].0 != tables::UNASSIGNED => bytes.push(reverse[index].1),
            _ => {
                return Err(EncodeError {
                    character,
                    offset,
                    encoding: Encoding::SingleByte(table),
                })
            }
        }
    }
    Ok(bytes)
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

/// Why text could not be written back as bytes.
///
/// Only a single-byte encoding can produce this: the Unicode encodings carry
/// every character there is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodeError {
    /// The character with no byte in this encoding.
    pub character: char,
    /// Where it sits in the text, in bytes of the text's own UTF-8.
    pub offset: usize,
    /// The encoding that has no byte for it.
    pub encoding: Encoding,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} has no byte for '{}'",
            self.encoding.label(),
            self.character
        )
    }
}

impl Error for EncodeError {}

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
    /// A UTF-32 stream does not divide into four-byte units.
    OddUtf32Length { length: usize },
    /// A UTF-32 unit is not a Unicode scalar value, so it is not this encoding.
    NotAScalarValue { point: u32, offset: usize },
    /// A UTF-16 surrogate had no pair, so no reversible text exists.
    UnpairedSurrogate { offset: usize },
    /// A byte the named single-byte encoding assigns no character to. The file
    /// is not this encoding, or not text at all.
    UnassignedByte {
        byte: u8,
        encoding: SingleByte,
        offset: usize,
    },
    /// A byte pair the named multi-byte encoding assigns no character to.
    UnassignedSequence {
        sequence: u16,
        encoding: MultiByte,
        offset: usize,
    },
    /// A multi-byte sequence ran off the end of the file.
    IncompleteSequence { offset: usize },
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
            Self::OddUtf32Length { length } => {
                write!(formatter, "a UTF-32 stream cannot have {length} bytes")
            }
            Self::NotAScalarValue { point, offset } => write!(
                formatter,
                "the four bytes at {offset} are {point:#010X}, which is no character"
            ),
            Self::UnpairedSurrogate { offset } => {
                write!(
                    formatter,
                    "an unpaired UTF-16 surrogate sits at byte {offset}"
                )
            }
            Self::UnassignedByte {
                byte,
                encoding,
                offset,
            } => write!(
                formatter,
                "{} assigns no character to the {byte:#04X} at byte {offset}",
                encoding.label()
            ),
            Self::UnassignedSequence {
                sequence,
                encoding,
                offset,
            } => write!(
                formatter,
                "{} assigns no character to the {sequence:#06X} at byte {offset}",
                encoding.label()
            ),
            Self::IncompleteSequence { offset } => write!(
                formatter,
                "a multi-byte sequence starts at {offset} and the file ends"
            ),
        }
    }
}

impl Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::{DecodeError, EncodeError, Encoding, MultiByte, SingleByte};

    #[test]
    fn every_encoding_round_trips_its_own_bytes() {
        let cases = [
            (Encoding::Utf8, "línea\ntabla\t".to_owned()),
            (Encoding::Utf8Bom, "con marca\r\n".to_owned()),
            (Encoding::Utf16Le, "emoji 🜲 y ñ\n".to_owned()),
            (Encoding::Utf16Be, "emoji 🜲 y ñ\n".to_owned()),
        ];

        for (encoding, text) in cases {
            let bytes = encoding
                .encode(&text)
                .expect("a Unicode encoding carries every character");
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

    #[test]
    fn every_table_maps_every_byte_it_assigns_back_to_itself() {
        // This is the whole argument for letting these encodings edit a native
        // document, so it is proved for all 256 bytes of all of them rather
        // than sampled.
        for table in SingleByte::catalogue() {
            let encoding = Encoding::SingleByte(table);
            for byte in 0..=u8::MAX {
                match encoding.decode(&[byte]) {
                    Ok(text) => {
                        assert_eq!(text.chars().count(), 1, "{} {byte:#04X}", table.label());
                        assert_eq!(
                            encoding.encode(&text),
                            Ok(vec![byte]),
                            "{} {byte:#04X}",
                            table.label()
                        );
                    }
                    Err(DecodeError::UnassignedByte {
                        byte: reported,
                        encoding: reported_encoding,
                        offset,
                    }) => {
                        assert!(byte >= 0xA0, "{} {byte:#04X}", table.label());
                        assert_eq!(reported, byte);
                        assert_eq!(reported_encoding, table);
                        assert_eq!(offset, 0);
                    }
                    Err(other) => panic!("{} {byte:#04X}: {other}", table.label()),
                }
            }
        }
    }

    #[test]
    fn a_table_refuses_the_character_it_has_no_byte_for() {
        let windows = Encoding::SingleByte(SingleByte::Windows1252);
        assert_eq!(windows.encode("euro €"), Ok(b"euro \x80".to_vec()));
        assert_eq!(
            windows.encode("façade\n"),
            Ok(vec![b'f', b'a', 0xE7, b'a', b'd', b'e', b'\n'])
        );
        assert_eq!(
            windows.encode("nota 🜲"),
            Err(EncodeError {
                character: '🜲',
                offset: 5,
                encoding: windows,
            })
        );
        // The same character is fine in every Unicode encoding, so the refusal
        // is about this table and not about the character.
        assert!(Encoding::Utf8.encode("nota 🜲").is_ok());
    }

    #[test]
    fn an_unassigned_byte_is_refused_where_it_sits() {
        // ISO-8859-7 assigns no character to 0xAE. A file carrying it is not
        // Greek text, and decoding it to a substitute would not write back.
        let greek = Encoding::SingleByte(SingleByte::Iso8859_7);
        assert_eq!(
            greek.decode(b"alfa \xAE"),
            Err(DecodeError::UnassignedByte {
                byte: 0xAE,
                encoding: SingleByte::Iso8859_7,
                offset: 5,
            })
        );
    }

    #[test]
    fn a_table_is_never_concluded_from_the_bytes() {
        // Nothing in a single-byte file says which table it is. The mark reader
        // is the only thing that concludes an encoding, and it knows none of
        // them; a caller has to name one.
        let latin = Encoding::SingleByte(SingleByte::Iso8859_1);
        let bytes = latin.encode("façade\n").expect("latin-1 carries these");
        assert_eq!(Encoding::from_byte_order_mark(&bytes), None);
        assert!(Encoding::catalogue().contains(&latin));
    }

    #[test]
    fn the_catalogue_carries_the_unicode_forms_first_and_no_duplicates() {
        let all = Encoding::catalogue();
        assert_eq!(
            &all[..4],
            &[
                Encoding::Utf8,
                Encoding::Utf8Bom,
                Encoding::Utf16Le,
                Encoding::Utf16Be
            ]
        );
        let mut labels: Vec<&str> = all.iter().map(|encoding| encoding.label()).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count, "two encodings share a label");
    }

    #[test]
    fn a_multi_byte_encoding_reads_and_writes_the_text_it_carries() {
        // 私 is two bytes in every one of these; the ASCII around it stays one.
        let cases = [
            (
                MultiByte::ShiftJis,
                "私 wa\n",
                vec![0x8E, 0x84, b' ', b'w', b'a', b'\n'],
            ),
            (
                MultiByte::Gbk,
                "私 wa\n",
                vec![0xCB, 0xBD, b' ', b'w', b'a', b'\n'],
            ),
        ];
        for (table, text, expected) in cases {
            let encoding = Encoding::MultiByte(table);
            assert_eq!(
                encoding.encode(text),
                Ok(expected.clone()),
                "{}",
                table.label()
            );
            assert_eq!(
                encoding.decode(&expected),
                Ok(text.to_owned()),
                "{}",
                table.label()
            );
        }
    }

    #[test]
    fn a_multi_byte_stream_that_is_not_this_encoding_is_refused() {
        let shift = Encoding::MultiByte(MultiByte::ShiftJis);
        // A lead byte with the file ending after it.
        assert_eq!(
            shift.decode(&[b'a', 0x8E]),
            Err(DecodeError::IncompleteSequence { offset: 1 })
        );
        // A pair the encoding assigns nothing to.
        assert!(matches!(
            shift.decode(&[0x85, 0x40]),
            Err(DecodeError::UnassignedSequence { .. })
        ));
        // A character it has no sequence for at all.
        assert!(matches!(
            shift.encode("emoji 🜲"),
            Err(EncodeError {
                character: '🜲', ..
            })
        ));
    }

    #[test]
    fn every_multi_byte_encoding_round_trips_its_own_output() {
        // Not a bijectivity claim, which these do not have: this only proves
        // the two directions of each table agree with each other. What proves a
        // *file* is safe is `open_with` comparing bytes.
        for table in MultiByte::catalogue() {
            let encoding = Encoding::MultiByte(table);
            let text = "ascii 123\n";
            let bytes = encoding.encode(text).expect("ASCII is in every one");
            assert_eq!(bytes, text.as_bytes(), "{}", table.label());
            assert_eq!(
                encoding.decode(&bytes),
                Ok(text.to_owned()),
                "{}",
                table.label()
            );
        }
    }
}
