//! Why a decode was refused — typed, so the link can log it and the peer can
//! never turn a refusal into a string it chose.

use std::fmt;

/// A decode that did not produce a message, and the one reason it did not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The bytes were not the CBOR shape this message expects.
    Malformed(&'static str),
    /// A required field was absent from the map.
    MissingField(&'static str),
    /// A string or byte string declared more bytes than its bound allows.
    TooLong {
        what: &'static str,
        max: usize,
        len: usize,
    },
    /// A list or map declared more items than its bound allows.
    TooMany {
        what: &'static str,
        max: usize,
        len: usize,
    },
    /// The whole input is larger than any message may be.
    Oversized { max: usize, len: usize },
    /// The peer speaks a protocol version this build does not.
    UnsupportedVersion(u8),
    /// A numeric field held a value outside its type.
    OutOfRange(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed {what}"),
            Self::MissingField(what) => write!(f, "missing field {what}"),
            Self::TooLong { what, max, len } => {
                write!(f, "{what} is {len} bytes, more than the {max} allowed")
            }
            Self::TooMany { what, max, len } => {
                write!(f, "{what} has {len} items, more than the {max} allowed")
            }
            Self::Oversized { max, len } => {
                write!(f, "message is {len} bytes, more than the {max} allowed")
            }
            Self::UnsupportedVersion(v) => write!(f, "unsupported protocol version {v}"),
            Self::OutOfRange(what) => write!(f, "{what} is out of range"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<minicbor::decode::Error> for DecodeError {
    fn from(_: minicbor::decode::Error) -> Self {
        // The library's message names byte offsets and CBOR majors; what the
        // caller needs to know is only that the bytes were not CBOR of the
        // expected shape. The peer never gets to author the reason.
        Self::Malformed("cbor")
    }
}
