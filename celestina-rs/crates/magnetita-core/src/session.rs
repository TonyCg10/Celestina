//! The device session — the pure brain of one link.
//!
//! Once the transport has an encrypted, framed channel to a device, everything
//! it does with a packet is decided here: hold the peer's [`Identity`] and its
//! [`Pairing`] state, react to an arrived packet, and drive pairing and ping
//! from the local user's actions. Each call returns a [`Reaction`] — the packets
//! the transport should send and the [`ConnectionEvent`]s the log should record.
//!
//! It stays pure: no sockets, no clock. The outgoing packets are named as intent
//! ([`Outgoing`]) for the transport to stamp with the current time and serialize,
//! and the ~30 s pairing timer is the transport's to run — it calls
//! [`Session::pairing_timeout`] when it fires. So the logic that turns a packet
//! into a decision is testable, in full, with plain packets and no phone.

use crate::event::{ConnectionEvent, LostReason};
use crate::identity::Identity;
use crate::packet::NetworkPacket;
use crate::pair::{read_pair, PairAction, PairMessage, PairState, PairVerification, Pairing};
use crate::ping::TYPE_PING;

/// A packet the transport should send, as intent — it stamps the id and
/// serializes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outgoing {
    /// A complete `kdeconnect.pair` body, including a timestamp only for a
    /// fresh request.
    Pair(PairMessage),
    /// A `kdeconnect.ping`.
    Ping,
}

/// What one event produced: packets to send and events to record. Empty by
/// default — most packets change nothing observable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub send: Vec<Outgoing>,
    pub events: Vec<ConnectionEvent>,
}

/// One device's live session over an already-trusted channel.
#[derive(Clone, Debug)]
pub struct Session {
    peer: Option<Identity>,
    pairing: Pairing,
    protocol_version: i32,
}

impl Session {
    /// A session with a device we have never trusted.
    pub fn new(protocol_version: i32) -> Self {
        Self {
            peer: None,
            pairing: Pairing::new(),
            protocol_version,
        }
    }

    /// A session with a device already in the trust store — paired before the
    /// first packet, so a known phone reconnects without re-pairing.
    pub fn restored(protocol_version: i32) -> Self {
        Session {
            peer: None,
            pairing: Pairing::paired(),
            protocol_version,
        }
    }

    pub fn is_paired(&self) -> bool {
        self.pairing.is_paired()
    }

    pub fn verification(&self) -> Option<PairVerification> {
        self.pairing.verification()
    }

    /// The peer has asked to pair and is waiting for an explicit local answer.
    /// Distinguishes a pending *incoming* request from our own outgoing one,
    /// which look alike as a
    /// [`Pairing`](crate::event::ConnectionEvent::Pairing) event.
    pub fn peer_wants_to_pair(&self) -> bool {
        self.pairing.state() == PairState::RequestedByPeer
    }

    pub fn peer(&self) -> Option<&Identity> {
        self.peer.as_ref()
    }

    /// The peer's identity arrived over the link (the first thing after TLS, and
    /// again if it re-announces).
    pub fn set_peer(&mut self, identity: Identity) -> Reaction {
        self.peer = Some(identity);
        Reaction {
            send: Vec::new(),
            events: vec![ConnectionEvent::Identified],
        }
    }

    /// A packet arrived over the trusted channel. Pairing, ping and a re-sent
    /// identity are handled; a plugin we do not implement yet is ignored, not an
    /// error.
    pub fn handle(&mut self, packet: &NetworkPacket, now: i64) -> Reaction {
        match read_pair(packet) {
            Ok(Some(message)) => return self.received_pair(message, now),
            Ok(None) => {}
            Err(_) => {
                return Reaction {
                    send: Vec::new(),
                    events: vec![ConnectionEvent::Lost(LostReason::PairInvalid)],
                };
            }
        }
        if packet.is(TYPE_PING) && self.is_paired() {
            return Reaction {
                send: Vec::new(),
                events: vec![ConnectionEvent::Pinged],
            };
        }
        if let Some(identity) = Identity::from_packet(packet) {
            return self.set_peer(identity);
        }
        Reaction::default()
    }

    /// The local user asks to pair.
    pub fn request_pairing(&mut self, now: i64) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.request(now);
        let events = match (before, self.pairing.state()) {
            (PairState::Unpaired, PairState::Requested) => vec![ConnectionEvent::Pairing],
            // A peer request was already pending; asking back completes it.
            (PairState::RequestedByPeer, PairState::Paired) => vec![ConnectionEvent::Paired],
            _ => Vec::new(),
        };
        reaction(action, events)
    }

    /// The local user accepts the peer's pending request.
    pub fn accept_pairing(&mut self) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.accept();
        let events = if before == PairState::RequestedByPeer && self.pairing.is_paired() {
            vec![ConnectionEvent::Paired]
        } else {
            Vec::new()
        };
        reaction(action, events)
    }

    /// The local user rejects the peer's pending request. The result is obvious
    /// to the UI, so nothing is logged beyond telling the peer.
    pub fn reject_pairing(&mut self) -> Reaction {
        reaction(self.pairing.reject(), Vec::new())
    }

    /// The pairing window expired with no answer.
    pub fn pairing_timeout(&mut self) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.timeout();
        let events = match before {
            PairState::Requested | PairState::RequestedByPeer => {
                vec![ConnectionEvent::Lost(LostReason::PairTimedOut)]
            }
            _ => Vec::new(),
        };
        reaction(action, events)
    }

    /// Drop an established pairing.
    pub fn unpair(&mut self) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.unpair();
        let events = if before == PairState::Paired {
            vec![ConnectionEvent::Unpaired]
        } else {
            Vec::new()
        };
        reaction(action, events)
    }

    /// Send a ping — the CP0 liveness poke.
    pub fn send_ping(&self) -> Reaction {
        Reaction {
            send: vec![Outgoing::Ping],
            events: Vec::new(),
        }
    }

    /// A `kdeconnect.pair { pair }` arrived from the peer.
    fn received_pair(&mut self, message: PairMessage, now: i64) -> Reaction {
        let before = self.pairing.state();
        let action = match self.pairing.received(message, self.protocol_version, now) {
            Ok(action) => action,
            Err(_) => {
                let mut events = Vec::new();
                if before == PairState::Paired {
                    events.push(ConnectionEvent::Unpaired);
                }
                events.push(ConnectionEvent::Lost(LostReason::PairInvalid));
                return Reaction {
                    send: Vec::new(),
                    events,
                };
            }
        };
        let events = match (before, self.pairing.state()) {
            (PairState::Requested, PairState::Paired) => vec![ConnectionEvent::Paired],
            // A request arrived and we had not asked — the app must prompt.
            (PairState::Unpaired, PairState::RequestedByPeer) => vec![ConnectionEvent::Pairing],
            // Our pending request was refused.
            (PairState::Requested, PairState::Unpaired) => {
                vec![ConnectionEvent::Lost(LostReason::PairRejected)]
            }
            // A pending peer request was withdrawn.
            (PairState::RequestedByPeer, PairState::Unpaired) => {
                vec![ConnectionEvent::Lost(LostReason::PairRejected)]
            }
            // An established pairing was dropped by the peer.
            (PairState::Paired, PairState::Unpaired) => vec![ConnectionEvent::Unpaired],
            (PairState::Paired, PairState::RequestedByPeer) => {
                vec![ConnectionEvent::Unpaired, ConnectionEvent::Pairing]
            }
            _ => Vec::new(),
        };
        reaction(action, events)
    }
}

/// Turns a pairing action plus already-decided events into a [`Reaction`].
fn reaction(action: PairAction, events: Vec<ConnectionEvent>) -> Reaction {
    let send = match action {
        PairAction::Send(message) => vec![Outgoing::Pair(message)],
        PairAction::None => Vec::new(),
    };
    Reaction { send, events }
}

#[cfg(test)]
mod tests {
    use super::{Outgoing, Reaction, Session};
    use crate::event::{ConnectionEvent, LostReason};
    use crate::identity::Identity;
    use crate::pair::{pair_request_packet, pair_response_packet, PairMessage, TYPE_PAIR};
    use crate::ping::ping_packet;

    const PROTOCOL: i32 = 8;
    const NOW: i64 = 1_700_000_000;

    #[test]
    fn a_peer_request_prompts_and_accepting_pairs() {
        let mut s = Session::new(PROTOCOL);
        let r = s.handle(&pair_request_packet(1, NOW), NOW);
        assert_eq!(
            r,
            Reaction {
                send: vec![],
                events: vec![ConnectionEvent::Pairing]
            }
        );
        // The user accepts; we confirm and are paired.
        let r = s.accept_pairing();
        assert_eq!(
            r,
            Reaction {
                send: vec![Outgoing::Pair(PairMessage::response(true))],
                events: vec![ConnectionEvent::Paired]
            }
        );
        assert!(s.is_paired());
        assert_eq!(
            s.verification().map(|material| material.timestamp),
            Some(Some(NOW))
        );
    }

    #[test]
    fn we_ask_and_the_peer_accepts() {
        let mut s = Session::new(PROTOCOL);
        assert_eq!(
            s.request_pairing(NOW),
            Reaction {
                send: vec![Outgoing::Pair(PairMessage::request(NOW))],
                events: vec![ConnectionEvent::Pairing]
            }
        );
        let r = s.handle(&pair_response_packet(2, true), NOW);
        assert_eq!(r.events, vec![ConnectionEvent::Paired]);
        assert!(s.is_paired());
    }

    #[test]
    fn a_rejected_request_is_logged_with_its_reason() {
        let mut s = Session::new(PROTOCOL);
        s.request_pairing(NOW);
        let r = s.handle(&pair_response_packet(3, false), NOW);
        assert_eq!(
            r.events,
            vec![ConnectionEvent::Lost(LostReason::PairRejected)]
        );
        assert!(!s.is_paired());
    }

    #[test]
    fn our_request_timing_out_is_logged_and_withdrawn() {
        let mut s = Session::new(PROTOCOL);
        s.request_pairing(NOW);
        let r = s.pairing_timeout();
        assert_eq!(r.send, vec![Outgoing::Pair(PairMessage::response(false))]);
        assert_eq!(
            r.events,
            vec![ConnectionEvent::Lost(LostReason::PairTimedOut)]
        );
    }

    #[test]
    fn only_a_paired_peer_can_surface_a_ping() {
        let mut s = Session::restored(PROTOCOL);
        assert_eq!(
            s.handle(&ping_packet(4), NOW),
            Reaction {
                send: vec![],
                events: vec![ConnectionEvent::Pinged]
            }
        );
        assert_eq!(
            Session::new(PROTOCOL).handle(&ping_packet(5), NOW),
            Reaction::default()
        );
    }

    #[test]
    fn an_identity_over_the_link_is_recorded_as_the_peer() {
        let mut s = Session::new(PROTOCOL);
        let phone = Identity::desktop("p1", "Pixel"); // shape only; type is irrelevant here
        let r = s.handle(&phone.to_packet(5), NOW);
        assert_eq!(r.events, vec![ConnectionEvent::Identified]);
        assert_eq!(s.peer().unwrap().device_id, "p1");
    }

    #[test]
    fn a_restored_pair_is_dropped_by_an_unpair_packet() {
        let mut s = Session::restored(PROTOCOL);
        assert!(s.is_paired());
        let r = s.handle(&pair_response_packet(6, false), NOW);
        assert_eq!(r.events, vec![ConnectionEvent::Unpaired]);
        assert!(!s.is_paired());
    }

    #[test]
    fn an_unknown_plugin_packet_changes_nothing() {
        let mut s = Session::restored(PROTOCOL);
        let odd =
            crate::packet::NetworkPacket::new(7, "kdeconnect.telephony", serde_json::json!({}));
        assert_eq!(s.handle(&odd, NOW), Reaction::default());
        assert!(s.is_paired());
    }

    #[test]
    fn send_ping_asks_the_transport_for_a_ping() {
        let s = Session::restored(PROTOCOL);
        assert_eq!(s.send_ping().send, vec![Outgoing::Ping]);
    }

    #[test]
    fn a_malformed_v8_request_never_prompts_or_pairs() {
        let mut missing = Session::new(PROTOCOL);
        assert_eq!(
            missing.handle(&pair_response_packet(8, true), NOW).events,
            vec![ConnectionEvent::Lost(LostReason::PairInvalid)]
        );
        assert!(!missing.is_paired());

        let mut stale = Session::new(PROTOCOL);
        assert_eq!(
            stale
                .handle(&pair_request_packet(9, NOW - 1_801), NOW)
                .events,
            vec![ConnectionEvent::Lost(LostReason::PairInvalid)]
        );
        assert!(!stale.is_paired());
    }

    #[test]
    fn a_paired_peer_reasking_is_unpaired_and_must_be_confirmed_again() {
        let mut session = Session::restored(PROTOCOL);
        assert_eq!(
            session.handle(&pair_request_packet(10, NOW), NOW).events,
            vec![ConnectionEvent::Unpaired, ConnectionEvent::Pairing]
        );
        assert!(!session.is_paired());
        assert!(session.peer_wants_to_pair());
    }

    #[test]
    fn a_malformed_pair_body_is_reported_instead_of_ignored() {
        let packet = crate::packet::NetworkPacket::new(
            11,
            TYPE_PAIR,
            serde_json::json!({"pair": "true", "timestamp": {}}),
        );
        assert_eq!(
            Session::new(PROTOCOL).handle(&packet, NOW).events,
            vec![ConnectionEvent::Lost(LostReason::PairInvalid)]
        );
    }
}
