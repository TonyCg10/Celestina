//! `find` (capability 4): make the phone ring, and stop it.

use crate::codec::{self, Map};
use crate::error::DecodeError;

/// Desktop → phone: ring at full volume until stopped. Kind 1, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FindRing;

/// Desktop → phone: stop ringing. Kind 2, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FindStop;

impl FindRing {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "find", |_, _| Ok(false))?;
        Ok(Self)
    }
}

impl FindStop {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "find stop", |_, _| Ok(false))?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_are_the_empty_map() {
        assert_eq!(FindRing.encode(), vec![0xa0]);
        assert_eq!(FindRing::decode(&[0xa0]).unwrap(), FindRing);
        assert_eq!(FindStop::decode(&[0xa0]).unwrap(), FindStop);
        assert_eq!(
            FindRing::decode(&[0x01]),
            Err(DecodeError::Malformed("cbor"))
        );
    }
}
