//! The bound rule: no peer-chosen length is trusted until it is checked.
//!
//! CBOR announces the length of a string, a byte string, an array or a map
//! before its contents. That is the moment to refuse — before allocating,
//! before copying, before a single byte of the value exists in this process
//! as a `String`. Every reader here does exactly that and nothing else; the
//! message modules never call `Decoder::str` directly.

use minicbor::Decoder;

use crate::error::DecodeError;

/// Longest text the wire may carry in one field: a name, a title, a body.
pub const MAX_TEXT: usize = 4096;
/// Longest short identifier: a device id, a capability name, a fingerprint.
pub const MAX_IDENT: usize = 128;
/// Most items one list or map may declare.
pub const MAX_LIST: usize = 256;
/// Largest byte string one field may carry: a thumbnail, a proof, a chunk.
pub const MAX_BYTES: usize = 1 << 20;
/// Largest encoded message the link will hand to this crate at all.
pub const MAX_MESSAGE: usize = MAX_BYTES + 4096;

/// Refuses an input larger than any message may be, before decoding starts.
pub fn check_message_size(bytes: &[u8]) -> Result<(), DecodeError> {
    if bytes.len() > MAX_MESSAGE {
        return Err(DecodeError::Oversized {
            max: MAX_MESSAGE,
            len: bytes.len(),
        });
    }
    Ok(())
}

/// Reads a text string of at most `max` bytes, or refuses.
pub fn text(d: &mut Decoder<'_>, what: &'static str, max: usize) -> Result<String, DecodeError> {
    // Peek the declared length through the header; the payload is only
    // touched once the length is acceptable.
    let mut probe = d.probe();
    let declared = probe.str()?.len();
    if declared > max {
        return Err(DecodeError::TooLong {
            what,
            max,
            len: declared,
        });
    }
    Ok(d.str()?.to_owned())
}

/// Reads a byte string of at most `max` bytes, or refuses.
pub fn bytes(d: &mut Decoder<'_>, what: &'static str, max: usize) -> Result<Vec<u8>, DecodeError> {
    let mut probe = d.probe();
    let declared = probe.bytes()?.len();
    if declared > max {
        return Err(DecodeError::TooLong {
            what,
            max,
            len: declared,
        });
    }
    Ok(d.bytes()?.to_vec())
}

/// Reads the header of a definite-length array of at most `max` items.
pub fn array(d: &mut Decoder<'_>, what: &'static str, max: usize) -> Result<usize, DecodeError> {
    let Some(len) = d.array()? else {
        return Err(DecodeError::Malformed("indefinite array"));
    };
    let len = usize::try_from(len).map_err(|_| DecodeError::OutOfRange(what))?;
    if len > max {
        return Err(DecodeError::TooMany { what, max, len });
    }
    Ok(len)
}

/// Reads the header of a definite-length map of at most `max` pairs.
pub fn map(d: &mut Decoder<'_>, what: &'static str, max: usize) -> Result<usize, DecodeError> {
    let Some(len) = d.map()? else {
        return Err(DecodeError::Malformed("indefinite map"));
    };
    let len = usize::try_from(len).map_err(|_| DecodeError::OutOfRange(what))?;
    if len > max {
        return Err(DecodeError::TooMany { what, max, len });
    }
    Ok(len)
}

/// Reads an integer map key.
pub fn key(d: &mut Decoder<'_>) -> Result<u32, DecodeError> {
    d.u32().map_err(DecodeError::from)
}

/// Reads a `u8` field, refusing anything wider.
pub fn u8(d: &mut Decoder<'_>, what: &'static str) -> Result<u8, DecodeError> {
    d.u8().map_err(|_| DecodeError::OutOfRange(what))
}

/// Reads a `u16` field, refusing anything wider.
pub fn u16(d: &mut Decoder<'_>, what: &'static str) -> Result<u16, DecodeError> {
    d.u16().map_err(|_| DecodeError::OutOfRange(what))
}

/// Reads a `u32` field, refusing anything wider.
pub fn u32(d: &mut Decoder<'_>, what: &'static str) -> Result<u32, DecodeError> {
    d.u32().map_err(|_| DecodeError::OutOfRange(what))
}

/// Skips one value of any shape — the way an unknown key is tolerated.
pub fn skip(d: &mut Decoder<'_>) -> Result<(), DecodeError> {
    d.skip().map_err(DecodeError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    fn encoded_text(len: usize) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.str(&"a".repeat(len)).unwrap();
        e.into_writer()
    }

    #[test]
    fn text_at_the_bound_is_read_and_one_over_is_refused() {
        let ok = encoded_text(MAX_IDENT);
        assert_eq!(
            text(&mut Decoder::new(&ok), "id", MAX_IDENT).unwrap().len(),
            MAX_IDENT
        );
        let over = encoded_text(MAX_IDENT + 1);
        assert_eq!(
            text(&mut Decoder::new(&over), "id", MAX_IDENT),
            Err(DecodeError::TooLong {
                what: "id",
                max: MAX_IDENT,
                len: MAX_IDENT + 1
            })
        );
    }

    #[test]
    fn a_length_larger_than_the_input_is_refused_without_reading_it() {
        // Header claims 100 000 bytes; only three follow. The probe fails on
        // the short buffer and nothing is copied.
        let mut lying = vec![0x7a, 0x00, 0x01, 0x86, 0xa0];
        lying.extend_from_slice(b"abc");
        assert_eq!(
            text(&mut Decoder::new(&lying), "id", MAX_TEXT),
            Err(DecodeError::Malformed("cbor"))
        );
    }

    #[test]
    fn indefinite_containers_are_refused() {
        assert_eq!(
            array(&mut Decoder::new(&[0x9f]), "list", MAX_LIST),
            Err(DecodeError::Malformed("indefinite array"))
        );
        assert_eq!(
            map(&mut Decoder::new(&[0xbf]), "map", MAX_LIST),
            Err(DecodeError::Malformed("indefinite map"))
        );
    }

    #[test]
    fn too_many_items_are_refused_at_the_header() {
        let mut e = Encoder::new(Vec::new());
        e.array(MAX_LIST as u64 + 1).unwrap();
        let bytes = e.into_writer();
        assert_eq!(
            array(&mut Decoder::new(&bytes), "list", MAX_LIST),
            Err(DecodeError::TooMany {
                what: "list",
                max: MAX_LIST,
                len: MAX_LIST + 1
            })
        );
    }

    #[test]
    fn a_wide_integer_does_not_fit_a_narrow_field() {
        let mut e = Encoder::new(Vec::new());
        e.u32(70_000).unwrap();
        let bytes = e.into_writer();
        assert_eq!(
            u16(&mut Decoder::new(&bytes), "version"),
            Err(DecodeError::OutOfRange("version"))
        );
    }
}
