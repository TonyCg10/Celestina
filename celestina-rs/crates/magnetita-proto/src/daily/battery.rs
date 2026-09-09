//! `battery` (capability 1): the phone's charge, and a request for it.

use crate::bound;
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// Phone → desktop, whenever it changes and on request. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatteryStatus {
    /// 0–100.
    pub level: u8,
    pub charging: bool,
    /// The phone's own low-battery threshold was crossed.
    pub low: bool,
}

impl BatteryStatus {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(3)
            .u8(0, self.level)
            .bool(1, self.charging)
            .bool(2, self.low)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut level, mut charging, mut low) = (None, None, None);
        codec::read(body, "battery", |k, d| {
            match k {
                0 => level = Some(bound::u8(d, "level")?),
                1 => charging = Some(bound::bool(d, "charging")?),
                2 => low = Some(bound::bool(d, "low")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let level = required(level, "level")?;
        if level > 100 {
            return Err(DecodeError::OutOfRange("level"));
        }
        Ok(Self {
            level,
            charging: required(charging, "charging")?,
            low: required(low, "low")?,
        })
    }
}

/// Desktop → phone: send the status now. Kind 2, empty body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BatteryRequest;

impl BatteryRequest {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(0).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        codec::read(body, "battery request", |_, _| Ok(false))?;
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a300185501f502f4";

    #[test]
    fn golden_vector_round_trips() {
        let m = BatteryStatus {
            level: 85,
            charging: true,
            low: false,
        };
        assert_eq!(hex(&m.encode()), VECTOR);
        assert_eq!(BatteryStatus::decode(&unhex(VECTOR)).unwrap(), m);
        assert_eq!(hex(&BatteryRequest.encode()), "a0");
        assert_eq!(
            BatteryRequest::decode(&unhex("a0")).unwrap(),
            BatteryRequest
        );
    }

    #[test]
    fn a_level_over_100_is_refused() {
        let bytes = Map::new(3)
            .u8(0, 101)
            .bool(1, false)
            .bool(2, false)
            .finish();
        assert_eq!(
            BatteryStatus::decode(&bytes),
            Err(DecodeError::OutOfRange("level"))
        );
    }

    #[test]
    fn a_request_tolerates_unknown_fields() {
        assert_eq!(
            BatteryRequest::decode(&Map::new(1).u8(9, 1).finish()).unwrap(),
            BatteryRequest
        );
    }
}
