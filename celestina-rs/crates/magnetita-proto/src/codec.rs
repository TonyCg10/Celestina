//! The one way a message is written and read: a CBOR map with integer keys,
//! every length checked by [`bound`] on the way in.
//!
//! [`Map`] writes a message whose field count is known up front; optional
//! fields are counted by the caller and written only when present. [`read`]
//! walks a message's pairs and hands each key to the message's own matcher,
//! which returns `false` for a key it does not know so the value is skipped.
//! Nested maps and lists reuse the same two shapes through [`read_map`].

use minicbor::{Decoder, Encoder};

use crate::bound;
use crate::error::DecodeError;

/// A message under construction. Consumed by [`Map::finish`].
pub(crate) struct Map(Encoder<Vec<u8>>);

impl Map {
    /// Starts a map that will hold exactly `fields` pairs.
    pub fn new(fields: u64) -> Self {
        let mut e = Encoder::new(Vec::new());
        e.map(fields).unwrap();
        Self(e)
    }

    pub fn u8(mut self, key: u32, v: u8) -> Self {
        self.0.u32(key).unwrap().u8(v).unwrap();
        self
    }

    pub fn u16(mut self, key: u32, v: u16) -> Self {
        self.0.u32(key).unwrap().u16(v).unwrap();
        self
    }

    pub fn u32(mut self, key: u32, v: u32) -> Self {
        self.0.u32(key).unwrap().u32(v).unwrap();
        self
    }

    pub fn u64(mut self, key: u32, v: u64) -> Self {
        self.0.u32(key).unwrap().u64(v).unwrap();
        self
    }

    pub fn i16(mut self, key: u32, v: i16) -> Self {
        self.0.u32(key).unwrap().i16(v).unwrap();
        self
    }

    pub fn bool(mut self, key: u32, v: bool) -> Self {
        self.0.u32(key).unwrap().bool(v).unwrap();
        self
    }

    pub fn text(mut self, key: u32, v: &str) -> Self {
        self.0.u32(key).unwrap().str(v).unwrap();
        self
    }

    pub fn bytes(mut self, key: u32, v: &[u8]) -> Self {
        self.0.u32(key).unwrap().bytes(v).unwrap();
        self
    }

    /// Writes `key` followed by an array header; the caller then writes
    /// `len` items with [`Map::item`].
    pub fn list(mut self, key: u32, len: usize) -> Self {
        self.0.u32(key).unwrap().array(len as u64).unwrap();
        self
    }

    /// Writes `key` followed by one already-encoded nested map.
    pub fn nested(mut self, key: u32, encoded: &[u8]) -> Self {
        self.0.u32(key).unwrap();
        self.0.writer_mut().extend_from_slice(encoded);
        self
    }

    /// Writes one already-encoded item (a nested map) into an open list.
    pub fn item(mut self, encoded: &[u8]) -> Self {
        self.0.writer_mut().extend_from_slice(encoded);
        self
    }

    /// Writes one text item into an open list.
    pub fn text_item(mut self, v: &str) -> Self {
        self.0.str(v).unwrap();
        self
    }

    /// Writes one `u64` item into an open list.
    pub fn u64_item(mut self, v: u64) -> Self {
        self.0.u64(v).unwrap();
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.0.into_writer()
    }
}

/// How many fields a message has when some are optional.
pub(crate) fn count(required: u64, optional: &[bool]) -> u64 {
    required + optional.iter().filter(|p| **p).count() as u64
}

/// Reads one whole message: size check, map header, one call per pair.
pub(crate) fn read(
    body: &[u8],
    what: &'static str,
    field: impl FnMut(u32, &mut Decoder<'_>) -> Result<bool, DecodeError>,
) -> Result<(), DecodeError> {
    bound::check_message_size(body)?;
    let mut d = Decoder::new(body);
    read_map(&mut d, what, field)
}

/// Reads one map from an open decoder — a message or a nested item.
pub(crate) fn read_map(
    d: &mut Decoder<'_>,
    what: &'static str,
    mut field: impl FnMut(u32, &mut Decoder<'_>) -> Result<bool, DecodeError>,
) -> Result<(), DecodeError> {
    let pairs = bound::map(d, what, 32)?;
    for _ in 0..pairs {
        let key = bound::key(d)?;
        if !field(key, d)? {
            bound::skip(d)?;
        }
    }
    Ok(())
}

/// Reads a bounded list of items, each produced by `item` from the decoder.
pub(crate) fn read_list<T>(
    d: &mut Decoder<'_>,
    what: &'static str,
    max: usize,
    mut item: impl FnMut(&mut Decoder<'_>) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    let n = bound::array(d, what, max)?;
    let mut out = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        out.push(item(d)?);
    }
    Ok(out)
}

/// Reads a bounded list of bounded texts.
pub(crate) fn read_texts(
    d: &mut Decoder<'_>,
    what: &'static str,
    max_items: usize,
    max_len: usize,
) -> Result<Vec<String>, DecodeError> {
    read_list(d, what, max_items, |d| bound::text(d, what, max_len))
}

/// A required field that was never seen.
pub(crate) fn required<T>(v: Option<T>, what: &'static str) -> Result<T, DecodeError> {
    v.ok_or(DecodeError::MissingField(what))
}

/// Test helpers shared by every message module.
#[cfg(test)]
pub(crate) mod testing {
    pub fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
