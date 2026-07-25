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
use crate::pair::{read_pair, PairAction, PairState, Pairing};
use crate::ping::TYPE_PING;

/// A packet the transport should send, as intent — it stamps the id and
/// serializes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outgoing {
    /// A `kdeconnect.pair` with this flag.
    Pair(bool),
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
#[derive(Clone, Debug, Default)]
pub struct Session {
    peer: Option<Identity>,
    pairing: Pairing,
}

impl Session {
    /// A session with a device we have never trusted.
    pub fn new() -> Self {
        Session::default()
    }

    /// A session with a device already in the trust store — paired before the
    /// first packet, so a known phone reconnects without re-pairing.
    pub fn restored() -> Self {
        Session {
            peer: None,
            pairing: Pairing::paired(),
        }
    }

    pub fn is_paired(&self) -> bool {
        self.pairing.is_paired()
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
    pub fn handle(&mut self, packet: &NetworkPacket) -> Reaction {
        if let Some(pair) = read_pair(packet) {
            return self.received_pair(pair);
        }
        if packet.is(TYPE_PING) {
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
    pub fn request_pairing(&mut self) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.request();
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
    fn received_pair(&mut self, pair: bool) -> Reaction {
        let before = self.pairing.state();
        let action = self.pairing.received(pair);
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
            _ => Vec::new(),
        };
        reaction(action, events)
    }
}

/// Turns a pairing action plus already-decided events into a [`Reaction`].
fn reaction(action: PairAction, events: Vec<ConnectionEvent>) -> Reaction {
    let send = match action {
        PairAction::Send(pair) => vec![Outgoing::Pair(pair)],
        PairAction::None => Vec::new(),
    };
    Reaction { send, events }
}

#[cfg(test)]
mod tests {
    use super::{Outgoing, Reaction, Session};
    use crate::event::{ConnectionEvent, LostReason};
    use crate::identity::Identity;
    use crate::pair::pair_packet;
    use crate::ping::ping_packet;

    #[test]
    fn a_peer_request_prompts_and_accepting_pairs() {
        let mut s = Session::new();
        // The phone asks; we prompt, sending nothing yet.
        let r = s.handle(&pair_packet(1, true));
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
                send: vec![Outgoing::Pair(true)],
                events: vec![ConnectionEvent::Paired]
            }
        );
        assert!(s.is_paired());
    }

    #[test]
    fn we_ask_and_the_peer_accepts() {
        let mut s = Session::new();
        assert_eq!(
            s.request_pairing(),
            Reaction {
                send: vec![Outgoing::Pair(true)],
                events: vec![ConnectionEvent::Pairing]
            }
        );
        let r = s.handle(&pair_packet(2, true));
        assert_eq!(r.events, vec![ConnectionEvent::Paired]);
        assert!(s.is_paired());
    }

    #[test]
    fn a_rejected_request_is_logged_with_its_reason() {
        let mut s = Session::new();
        s.request_pairing();
        let r = s.handle(&pair_packet(3, false));
        assert_eq!(
            r.events,
            vec![ConnectionEvent::Lost(LostReason::PairRejected)]
        );
        assert!(!s.is_paired());
    }

    #[test]
    fn our_request_timing_out_is_logged_and_withdrawn() {
        let mut s = Session::new();
        s.request_pairing();
        let r = s.pairing_timeout();
        assert_eq!(r.send, vec![Outgoing::Pair(false)]);
        assert_eq!(
            r.events,
            vec![ConnectionEvent::Lost(LostReason::PairTimedOut)]
        );
    }

    #[test]
    fn a_ping_is_noted() {
        let mut s = Session::restored();
        assert_eq!(
            s.handle(&ping_packet(4)),
            Reaction {
                send: vec![],
                events: vec![ConnectionEvent::Pinged]
            }
        );
    }

    #[test]
    fn an_identity_over_the_link_is_recorded_as_the_peer() {
        let mut s = Session::new();
        let phone = Identity::desktop("p1", "Pixel"); // shape only; type is irrelevant here
        let r = s.handle(&phone.to_packet(5));
        assert_eq!(r.events, vec![ConnectionEvent::Identified]);
        assert_eq!(s.peer().unwrap().device_id, "p1");
    }

    #[test]
    fn a_restored_pair_is_dropped_by_an_unpair_packet() {
        let mut s = Session::restored();
        assert!(s.is_paired());
        let r = s.handle(&pair_packet(6, false));
        assert_eq!(r.events, vec![ConnectionEvent::Unpaired]);
        assert!(!s.is_paired());
    }

    #[test]
    fn an_unknown_plugin_packet_changes_nothing() {
        let mut s = Session::restored();
        let odd =
            crate::packet::NetworkPacket::new(7, "kdeconnect.telephony", serde_json::json!({}));
        assert_eq!(s.handle(&odd), Reaction::default());
        assert!(s.is_paired());
    }

    #[test]
    fn send_ping_asks_the_transport_for_a_ping() {
        let s = Session::restored();
        assert_eq!(s.send_ping().send, vec![Outgoing::Ping]);
    }
}
