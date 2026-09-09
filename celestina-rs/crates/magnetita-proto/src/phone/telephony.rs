//! `telephony` (capability 12): what the phone is doing with a call, and the
//! three things the desktop may do about it.

use crate::bound::{self, MAX_IDENT};
use crate::codec::{self, count, required, Map};
use crate::error::DecodeError;

/// Where a call is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallState {
    Ringing,
    Answered,
    Missed,
    Ended,
}

/// Phone → desktop: a call changed state. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallEvent {
    pub state: CallState,
    /// The number as the phone has it, at most [`MAX_IDENT`] bytes.
    pub number: String,
    /// The contact name the phone resolved, when it could.
    pub name: Option<String>,
    pub timestamp_ms: u64,
}

/// What the desktop may do about a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallAction {
    Mute,
    Answer,
    HangUp,
}

/// Desktop → phone: do this. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallCommand {
    pub action: CallAction,
}

impl CallState {
    fn to_wire(self) -> u8 {
        match self {
            Self::Ringing => 0,
            Self::Answered => 1,
            Self::Missed => 2,
            Self::Ended => 3,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Ringing,
            1 => Self::Answered,
            2 => Self::Missed,
            3 => Self::Ended,
            _ => return Err(DecodeError::OutOfRange("state")),
        })
    }
}

impl CallAction {
    fn to_wire(self) -> u8 {
        match self {
            Self::Mute => 0,
            Self::Answer => 1,
            Self::HangUp => 2,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        Ok(match v {
            0 => Self::Mute,
            1 => Self::Answer,
            2 => Self::HangUp,
            _ => return Err(DecodeError::OutOfRange("action")),
        })
    }
}

impl CallEvent {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(count(3, &[self.name.is_some()]))
            .u8(0, self.state.to_wire())
            .text(1, &self.number);
        if let Some(n) = &self.name {
            m = m.text(2, n);
        }
        m.u64(3, self.timestamp_ms).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut state, mut number, mut name, mut ts) = (None, None, None, None);
        codec::read(body, "call", |k, d| {
            match k {
                0 => state = Some(CallState::from_wire(bound::u8(d, "state")?)?),
                1 => number = Some(bound::text(d, "number", MAX_IDENT)?),
                2 => name = Some(bound::text(d, "name", MAX_IDENT)?),
                3 => ts = Some(bound::u64(d, "timestamp_ms")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            state: required(state, "state")?,
            number: required(number, "number")?,
            name,
            timestamp_ms: required(ts, "timestamp_ms")?,
        })
    }
}

impl CallCommand {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).u8(0, self.action.to_wire()).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut action = None;
        codec::read(body, "call command", |k, d| {
            match k {
                0 => action = Some(CallAction::from_wire(bound::u8(d, "action")?)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            action: required(action, "action")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a40000016c2b33343630303030303030300263416e61031b000001a0845c4063";

    #[test]
    fn golden_vector_round_trips() {
        let e = CallEvent {
            state: CallState::Ringing,
            number: "+34600000000".into(),
            name: Some("Ana".into()),
            timestamp_ms: 1_788_927_033_443,
        };
        assert_eq!(hex(&e.encode()), VECTOR);
        assert_eq!(CallEvent::decode(&unhex(VECTOR)).unwrap(), e);
        let anon = CallEvent { name: None, ..e };
        assert_eq!(CallEvent::decode(&anon.encode()).unwrap(), anon);
        let c = CallCommand {
            action: CallAction::HangUp,
        };
        assert_eq!(hex(&c.encode()), "a10002");
        assert_eq!(CallCommand::decode(&c.encode()).unwrap(), c);
    }

    #[test]
    fn unknown_states_and_actions_are_refused() {
        assert_eq!(
            CallCommand::decode(&Map::new(1).u8(0, 3).finish()),
            Err(DecodeError::OutOfRange("action"))
        );
        assert_eq!(
            CallEvent::decode(&Map::new(3).u8(0, 4).text(1, "1").u64(3, 1).finish()),
            Err(DecodeError::OutOfRange("state"))
        );
    }
}
