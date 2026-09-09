//! `share` (capability 5): files and text either way. A file's bytes travel
//! on their own QUIC stream, named by `transfer`; these messages are the
//! offer, the answer and the end of that stream.

use crate::bound::{self, MAX_FILENAME, MAX_IDENT, MAX_TEXT};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// Either direction: a file is coming. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareOffer {
    /// Chosen by the sender, unique on this connection.
    pub transfer: u32,
    /// A file name, not a path: at most [`MAX_FILENAME`] bytes and no `/`.
    pub name: String,
    pub size: u64,
    /// A MIME type, at most [`MAX_IDENT`] bytes; empty when unknown.
    pub mime: String,
}

/// Receiver → sender: send from `offset` on the stream. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareAccept {
    pub transfer: u32,
    /// Bytes already held from an earlier attempt, so a transfer resumes.
    pub offset: u64,
}

/// Receiver → sender: not taking it. Kind 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareReject {
    pub transfer: u32,
}

/// Either direction: the stream carried every byte, or was abandoned. Kind 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareDone {
    pub transfer: u32,
    pub complete: bool,
}

/// Either direction: a URL or a snippet, no stream needed. Kind 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareText {
    pub text: String,
}

impl ShareOffer {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(4)
            .u32(0, self.transfer)
            .text(1, &self.name)
            .u64(2, self.size)
            .text(3, &self.mime)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut transfer, mut name, mut size, mut mime) = (None, None, None, None);
        codec::read(body, "offer", |k, d| {
            match k {
                0 => transfer = Some(bound::u32(d, "transfer")?),
                1 => name = Some(bound::text(d, "name", MAX_FILENAME)?),
                2 => size = Some(bound::u64(d, "size")?),
                3 => mime = Some(bound::text(d, "mime", MAX_IDENT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let name = required(name, "name")?;
        if name.is_empty()
            || name.contains('/')
            || name.contains('\0')
            || name == "."
            || name == ".."
        {
            return Err(DecodeError::Malformed("file name"));
        }
        Ok(Self {
            transfer: required(transfer, "transfer")?,
            name,
            size: required(size, "size")?,
            mime: mime.unwrap_or_default(),
        })
    }
}

fn decode_transfer(
    body: &[u8],
    what: &'static str,
) -> Result<(u32, Option<u64>, Option<bool>), DecodeError> {
    let (mut transfer, mut offset, mut complete) = (None, None, None);
    codec::read(body, what, |k, d| {
        match k {
            0 => transfer = Some(bound::u32(d, "transfer")?),
            1 => offset = Some(bound::u64(d, "offset")?),
            2 => complete = Some(bound::bool(d, "complete")?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((required(transfer, "transfer")?, offset, complete))
}

impl ShareAccept {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u32(0, self.transfer)
            .u64(1, self.offset)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (transfer, offset, _) = decode_transfer(body, "accept")?;
        Ok(Self {
            transfer,
            offset: offset.unwrap_or(0),
        })
    }
}

impl ShareReject {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).u32(0, self.transfer).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            transfer: decode_transfer(body, "reject")?.0,
        })
    }
}

impl ShareDone {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u32(0, self.transfer)
            .bool(2, self.complete)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (transfer, _, complete) = decode_transfer(body, "done")?;
        Ok(Self {
            transfer,
            complete: required(complete, "complete")?,
        })
    }
}

impl ShareText {
    pub const KIND: u16 = 5;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).text(0, &self.text).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut text = None;
        codec::read(body, "share text", |k, d| {
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
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a400070168696d672e6a7065670219c350036a696d6167652f6a706567";

    #[test]
    fn golden_vector_round_trips() {
        let m = ShareOffer {
            transfer: 7,
            name: "img.jpeg".into(),
            size: 50000,
            mime: "image/jpeg".into(),
        };
        assert_eq!(hex(&m.encode()), VECTOR);
        assert_eq!(ShareOffer::decode(&unhex(VECTOR)).unwrap(), m);
    }

    #[test]
    fn a_path_is_not_a_file_name() {
        for bad in ["../x", "a/b", "", ".", "..", "a\0b"] {
            let m = ShareOffer {
                transfer: 1,
                name: bad.into(),
                size: 1,
                mime: String::new(),
            };
            assert_eq!(
                ShareOffer::decode(&m.encode()),
                Err(DecodeError::Malformed("file name")),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_answers_round_trip() {
        let a = ShareAccept {
            transfer: 7,
            offset: 4096,
        };
        assert_eq!(hex(&a.encode()), "a2000701191000");
        assert_eq!(ShareAccept::decode(&a.encode()).unwrap(), a);
        assert_eq!(
            ShareAccept::decode(&Map::new(1).u32(0, 7).finish()).unwrap(),
            ShareAccept {
                transfer: 7,
                offset: 0
            }
        );
        let r = ShareReject { transfer: 7 };
        assert_eq!(ShareReject::decode(&r.encode()).unwrap(), r);
        let d = ShareDone {
            transfer: 7,
            complete: true,
        };
        assert_eq!(hex(&d.encode()), "a2000702f5");
        assert_eq!(ShareDone::decode(&d.encode()).unwrap(), d);
        assert_eq!(
            ShareDone::decode(&r.encode()),
            Err(DecodeError::MissingField("complete"))
        );
        let t = ShareText {
            text: "https://example.org".into(),
        };
        assert_eq!(ShareText::decode(&t.encode()).unwrap(), t);
    }
}
