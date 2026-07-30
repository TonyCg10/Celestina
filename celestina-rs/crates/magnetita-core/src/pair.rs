//! Pairing — the trust handshake, as a pure state machine.
//!
//! Protocol v8 distinguishes a fresh request from an acceptance: a fresh
//! `{ "pair": true }` carries a Unix `timestamp`, while an acceptance does not.
//! The timestamp is also part of the short verification code shown by both
//! devices, so accepting a missing or stale value would both break
//! interoperability and weaken the human verification step.

use serde::{Deserialize, Serialize};

use crate::packet::NetworkPacket;

/// The `type` of a pairing packet.
pub const TYPE_PAIR: &str = "kdeconnect.pair";
/// The pairing window before an unanswered request is dropped.
pub const TIMEOUT_SECS: u64 = 30;
/// Maximum clock difference accepted for a protocol-v8 pairing request.
pub const MAX_CLOCK_SKEW_SECS: u64 = 30 * 60;

/// Where a device stands in the handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PairState {
    #[default]
    Unpaired,
    /// We asked and are waiting for the peer's answer.
    Requested,
    /// The peer asked; the local user must accept or reject.
    RequestedByPeer,
    Paired,
}

/// Parsed wire body and transport intent for a pairing packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairMessage {
    pub pair: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

impl PairMessage {
    pub const fn response(pair: bool) -> Self {
        Self {
            pair,
            timestamp: None,
        }
    }

    pub const fn request(timestamp: i64) -> Self {
        Self {
            pair: true,
            timestamp: Some(timestamp),
        }
    }
}

/// What the transport must do after a state-machine event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairAction {
    None,
    Send(PairMessage),
}

/// Why a fresh protocol-v8 request was rejected before prompting the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairError {
    MissingTimestamp,
    ClockSkew,
}

/// Material that exists only for a pairing completed on this live session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairVerification {
    /// Present for v8 requests; legacy protocols hash only the two keys.
    pub timestamp: Option<i64>,
}

/// The pairing state machine for one peer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pairing {
    state: PairState,
    timestamp: Option<i64>,
    fresh_exchange: bool,
}

impl Pairing {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restore trust from disk. A reconnect has no active pairing timestamp and
    /// therefore no new verification code to claim to the user.
    pub fn paired() -> Self {
        Self {
            state: PairState::Paired,
            timestamp: None,
            fresh_exchange: false,
        }
    }

    pub fn state(&self) -> PairState {
        self.state
    }

    pub fn is_paired(&self) -> bool {
        self.state == PairState::Paired
    }

    pub fn verification(&self) -> Option<PairVerification> {
        self.fresh_exchange.then_some(PairVerification {
            timestamp: self.timestamp,
        })
    }

    /// Ask locally. A fresh request always carries the timestamp KDE Connect
    /// emits; asking while a peer request is pending is an acceptance instead.
    pub fn request(&mut self, now: i64) -> PairAction {
        match self.state {
            PairState::Unpaired => {
                self.state = PairState::Requested;
                self.timestamp = Some(now);
                self.fresh_exchange = true;
                PairAction::Send(PairMessage::request(now))
            }
            PairState::RequestedByPeer => {
                self.state = PairState::Paired;
                PairAction::Send(PairMessage::response(true))
            }
            PairState::Requested | PairState::Paired => PairAction::None,
        }
    }

    /// Apply an arrived packet. Only a fresh request needs a v8 timestamp; a
    /// `true` received while we are `Requested` is the peer's timestamp-less
    /// acceptance of our own request.
    pub fn received(
        &mut self,
        message: PairMessage,
        protocol_version: i32,
        now: i64,
    ) -> Result<PairAction, PairError> {
        if !message.pair {
            self.state = PairState::Unpaired;
            self.timestamp = None;
            self.fresh_exchange = false;
            return Ok(PairAction::None);
        }

        match self.state {
            PairState::Requested => {
                self.state = PairState::Paired;
                Ok(PairAction::None)
            }
            PairState::RequestedByPeer => Ok(PairAction::None),
            PairState::Paired | PairState::Unpaired => {
                // KDE Connect deliberately drops an old pairing before treating
                // another `true` as a fresh request; auto-confirming can loop.
                self.state = PairState::Unpaired;
                self.timestamp = None;
                self.fresh_exchange = false;
                self.receive_new_request(message, protocol_version, now)
            }
        }
    }

    fn receive_new_request(
        &mut self,
        message: PairMessage,
        protocol_version: i32,
        now: i64,
    ) -> Result<PairAction, PairError> {
        if protocol_version >= 8 {
            let timestamp = message.timestamp.ok_or(PairError::MissingTimestamp)?;
            if timestamp.abs_diff(now) > MAX_CLOCK_SKEW_SECS {
                return Err(PairError::ClockSkew);
            }
        }
        self.timestamp = message.timestamp;
        self.fresh_exchange = true;
        self.state = PairState::RequestedByPeer;
        Ok(PairAction::None)
    }

    pub fn accept(&mut self) -> PairAction {
        if self.state == PairState::RequestedByPeer {
            self.state = PairState::Paired;
            PairAction::Send(PairMessage::response(true))
        } else {
            PairAction::None
        }
    }

    pub fn reject(&mut self) -> PairAction {
        if self.state == PairState::RequestedByPeer {
            self.state = PairState::Unpaired;
            self.timestamp = None;
            self.fresh_exchange = false;
            PairAction::Send(PairMessage::response(false))
        } else {
            PairAction::None
        }
    }

    pub fn timeout(&mut self) -> PairAction {
        let action = match self.state {
            PairState::Requested | PairState::RequestedByPeer => {
                PairAction::Send(PairMessage::response(false))
            }
            PairState::Unpaired | PairState::Paired => PairAction::None,
        };
        if matches!(
            self.state,
            PairState::Requested | PairState::RequestedByPeer
        ) {
            self.state = PairState::Unpaired;
            self.timestamp = None;
            self.fresh_exchange = false;
        }
        action
    }

    pub fn unpair(&mut self) -> PairAction {
        let had_trust = self.state != PairState::Unpaired;
        self.state = PairState::Unpaired;
        self.timestamp = None;
        self.fresh_exchange = false;
        if had_trust {
            PairAction::Send(PairMessage::response(false))
        } else {
            PairAction::None
        }
    }
}

/// Build a timestamp-less acceptance, rejection or unpair packet.
pub fn pair_response_packet(id: i64, pair: bool) -> NetworkPacket {
    pair_message_packet(id, PairMessage::response(pair))
}

/// Build a fresh pairing request carrying the timestamp required by v8.
pub fn pair_request_packet(id: i64, timestamp: i64) -> NetworkPacket {
    pair_message_packet(id, PairMessage::request(timestamp))
}

/// Serialize an already-decided pairing message.
pub fn pair_message_packet(id: i64, message: PairMessage) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_PAIR,
        serde_json::to_value(message).expect("a pair body is always valid JSON"),
    )
}

pub fn read_pair(packet: &NetworkPacket) -> Result<Option<PairMessage>, serde_json::Error> {
    if !packet.is(TYPE_PAIR) {
        return Ok(None);
    }
    serde_json::from_value(packet.body.clone()).map(Some)
}

#[cfg(test)]
mod tests {
    use super::{
        pair_request_packet, pair_response_packet, read_pair, PairAction, PairError, PairMessage,
        PairState, Pairing, MAX_CLOCK_SKEW_SECS, TYPE_PAIR,
    };

    const NOW: i64 = 1_700_000_000;

    #[test]
    fn we_ask_with_a_timestamp_and_the_peer_accepts_without_one() {
        let mut pairing = Pairing::new();
        assert_eq!(
            pairing.request(NOW),
            PairAction::Send(PairMessage::request(NOW))
        );
        assert_eq!(
            pairing
                .received(PairMessage::response(true), 8, NOW)
                .unwrap(),
            PairAction::None
        );
        assert!(pairing.is_paired());
        assert_eq!(
            pairing.verification().map(|material| material.timestamp),
            Some(Some(NOW))
        );
    }

    #[test]
    fn a_fresh_v8_request_requires_a_recent_timestamp() {
        let mut missing = Pairing::new();
        assert_eq!(
            missing.received(PairMessage::response(true), 8, NOW),
            Err(PairError::MissingTimestamp)
        );
        assert_eq!(missing.state(), PairState::Unpaired);

        let mut stale = Pairing::new();
        assert_eq!(
            stale.received(
                PairMessage::request(NOW - MAX_CLOCK_SKEW_SECS as i64 - 1),
                8,
                NOW,
            ),
            Err(PairError::ClockSkew)
        );
        assert_eq!(stale.state(), PairState::Unpaired);
    }

    #[test]
    fn the_skew_boundary_is_accepted_and_v7_remains_compatible() {
        let mut v8 = Pairing::new();
        assert_eq!(
            v8.received(
                PairMessage::request(NOW - MAX_CLOCK_SKEW_SECS as i64),
                8,
                NOW,
            ),
            Ok(PairAction::None)
        );
        assert_eq!(v8.state(), PairState::RequestedByPeer);

        let mut v7 = Pairing::new();
        assert_eq!(
            v7.received(PairMessage::response(true), 7, NOW),
            Ok(PairAction::None)
        );
        assert_eq!(v7.state(), PairState::RequestedByPeer);
    }

    #[test]
    fn the_peer_asks_and_we_accept_or_reject() {
        let mut accepted = Pairing::new();
        accepted
            .received(PairMessage::request(NOW), 8, NOW)
            .unwrap();
        assert_eq!(
            accepted.accept(),
            PairAction::Send(PairMessage::response(true))
        );
        assert!(accepted.is_paired());

        let mut rejected = Pairing::new();
        rejected
            .received(PairMessage::request(NOW), 8, NOW)
            .unwrap();
        assert_eq!(
            rejected.reject(),
            PairAction::Send(PairMessage::response(false))
        );
        assert_eq!(rejected.state(), PairState::Unpaired);
    }

    #[test]
    fn rejection_timeout_and_unpair_clear_the_timestamp() {
        let mut pairing = Pairing::new();
        pairing.request(NOW);
        assert_eq!(
            pairing
                .received(PairMessage::response(false), 8, NOW)
                .unwrap(),
            PairAction::None
        );
        assert_eq!(pairing.verification(), None);

        let mut incoming = Pairing::new();
        incoming
            .received(PairMessage::request(NOW), 8, NOW)
            .unwrap();
        assert_eq!(
            incoming.timeout(),
            PairAction::Send(PairMessage::response(false))
        );

        pairing.request(NOW);
        assert_eq!(
            pairing.timeout(),
            PairAction::Send(PairMessage::response(false))
        );
        assert_eq!(pairing.verification(), None);

        let mut paired = Pairing::paired();
        assert_eq!(
            paired.unpair(),
            PairAction::Send(PairMessage::response(false))
        );
    }

    #[test]
    fn a_paired_peer_reasking_starts_a_new_explicit_request() {
        let mut pairing = Pairing::paired();
        assert_eq!(
            pairing.received(PairMessage::request(NOW), 8, NOW).unwrap(),
            PairAction::None
        );
        assert_eq!(pairing.state(), PairState::RequestedByPeer);
    }

    #[test]
    fn asking_or_accepting_out_of_turn_is_a_no_op() {
        let mut paired = Pairing::paired();
        assert_eq!(paired.request(NOW), PairAction::None);
        assert_eq!(paired.accept(), PairAction::None);
        assert_eq!(paired.reject(), PairAction::None);
        assert!(paired.is_paired());
    }

    #[test]
    fn wire_packets_preserve_request_timestamp_and_responses() {
        let request = pair_request_packet(7, NOW);
        assert!(request.is(TYPE_PAIR));
        assert_eq!(
            read_pair(&request).unwrap(),
            Some(PairMessage::request(NOW))
        );
        assert_eq!(
            read_pair(&pair_response_packet(8, false)).unwrap(),
            Some(PairMessage::response(false))
        );
    }

    #[test]
    fn read_pair_ignores_a_non_pair_packet() {
        let ping = crate::packet::NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert_eq!(read_pair(&ping).unwrap(), None);
    }

    #[test]
    fn malformed_pair_bodies_are_not_confused_with_other_packets() {
        let malformed = crate::packet::NetworkPacket::new(
            2,
            TYPE_PAIR,
            serde_json::json!({"pair": "yes", "timestamp": []}),
        );
        assert!(read_pair(&malformed).is_err());
    }
}
