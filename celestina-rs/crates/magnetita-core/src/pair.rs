//! Pairing — the trust handshake, as a pure state machine.
//!
//! Two devices trust each other by exchanging a `kdeconnect.pair` packet whose
//! body is `{ "pair": true }` to ask (or confirm) and `{ "pair": false }` to
//! reject or unpair. One side asks, the other's user accepts or rejects, and on
//! a mutual `true` both pin the peer's TLS certificate (that pinning lives in the
//! transport, not here). A request that goes unanswered expires after about
//! thirty seconds.
//!
//! This module is the protocol logic with **no clock and no sockets**: it takes
//! an event — a request, an arrived packet, the user's answer, a timeout — and
//! returns the next state plus the one thing the transport must do, [`PairAction`].
//! The thirty-second timer is the caller's to run; when it fires the caller calls
//! [`Pairing::timeout`]. Keeping the trust logic pure is deliberate: it is the
//! part that must not be wrong, so it is the part that is exhaustively testable
//! without a phone.

use serde::{Deserialize, Serialize};

use crate::packet::NetworkPacket;

/// The `type` of a pairing packet.
pub const TYPE_PAIR: &str = "kdeconnect.pair";

/// The pairing window before an unanswered request is dropped.
pub const TIMEOUT_SECS: u64 = 30;

/// Where a device stands in the handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PairState {
    /// No trust; nothing pending.
    #[default]
    Unpaired,
    /// We asked and are waiting for the peer's answer.
    Requested,
    /// The peer asked; we owe an [`accept`](Pairing::accept) or
    /// [`reject`](Pairing::reject). The caller should prompt the user.
    RequestedByPeer,
    /// Trusted both ways.
    Paired,
}

/// What the transport must do after an event — the machine's only side effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairAction {
    /// Nothing goes on the wire.
    None,
    /// Send a `kdeconnect.pair` packet carrying this `pair` flag.
    Send(bool),
}

/// The pairing state machine for one peer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pairing {
    state: PairState,
}

impl Pairing {
    /// A peer we have never trusted.
    pub fn new() -> Self {
        Pairing::default()
    }

    /// A peer already trusted — restored from the on-disk trust store at startup,
    /// so a known device is paired before the first packet.
    pub fn paired() -> Self {
        Pairing {
            state: PairState::Paired,
        }
    }

    pub fn state(&self) -> PairState {
        self.state
    }

    pub fn is_paired(&self) -> bool {
        self.state == PairState::Paired
    }

    /// The local side asks to pair. From unpaired this sends the request; if the
    /// peer has *already* asked, asking back is the same as accepting.
    pub fn request(&mut self) -> PairAction {
        match self.state {
            PairState::Unpaired => {
                self.state = PairState::Requested;
                PairAction::Send(true)
            }
            PairState::RequestedByPeer => {
                self.state = PairState::Paired;
                PairAction::Send(true)
            }
            // Already asked, or already paired — asking again says nothing new.
            PairState::Requested | PairState::Paired => PairAction::None,
        }
    }

    /// A `kdeconnect.pair { pair }` arrived from the peer.
    pub fn received(&mut self, pair: bool) -> PairAction {
        if !pair {
            // A rejection or an unpair always drops the trust, from any state.
            self.state = PairState::Unpaired;
            return PairAction::None;
        }
        match self.state {
            // They want to pair and we had not asked — now the user must answer.
            PairState::Unpaired => {
                self.state = PairState::RequestedByPeer;
                PairAction::None
            }
            // They accepted the request we sent.
            PairState::Requested => {
                self.state = PairState::Paired;
                PairAction::None
            }
            // A duplicate request while one is already pending — nothing changes.
            PairState::RequestedByPeer => PairAction::None,
            // A peer re-asking while already paired: re-confirm so it does not
            // strand thinking we dropped it.
            PairState::Paired => PairAction::Send(true),
        }
    }

    /// The local user accepts the peer's pending request.
    pub fn accept(&mut self) -> PairAction {
        if self.state == PairState::RequestedByPeer {
            self.state = PairState::Paired;
            PairAction::Send(true)
        } else {
            PairAction::None
        }
    }

    /// The local user rejects the peer's pending request.
    pub fn reject(&mut self) -> PairAction {
        if self.state == PairState::RequestedByPeer {
            self.state = PairState::Unpaired;
            PairAction::Send(false)
        } else {
            PairAction::None
        }
    }

    /// The pairing window expired. A request we sent is withdrawn (and the peer
    /// told, so it drops its prompt); a request we never answered is just let go
    /// — the peer's own timer will fire.
    pub fn timeout(&mut self) -> PairAction {
        match self.state {
            PairState::Requested => {
                self.state = PairState::Unpaired;
                PairAction::Send(false)
            }
            PairState::RequestedByPeer => {
                self.state = PairState::Unpaired;
                PairAction::None
            }
            PairState::Unpaired | PairState::Paired => PairAction::None,
        }
    }

    /// Drop an established pairing (or cancel a pending one), telling the peer.
    pub fn unpair(&mut self) -> PairAction {
        let had_trust = self.state != PairState::Unpaired;
        self.state = PairState::Unpaired;
        if had_trust {
            PairAction::Send(false)
        } else {
            PairAction::None
        }
    }
}

/// The `{ "pair": bool }` body of a pairing packet.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct PairBody {
    pair: bool,
}

/// Builds a `kdeconnect.pair` packet with the given flag, stamped `id`. This is
/// what a [`PairAction::Send`] turns into on the wire.
pub fn pair_packet(id: i64, pair: bool) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_PAIR,
        serde_json::to_value(PairBody { pair }).expect("a pair body is always valid JSON"),
    )
}

/// Reads the `pair` flag out of a packet, or `None` if it is not a well-formed
/// pairing packet.
pub fn read_pair(packet: &NetworkPacket) -> Option<bool> {
    if !packet.is(TYPE_PAIR) {
        return None;
    }
    serde_json::from_value::<PairBody>(packet.body.clone())
        .ok()
        .map(|body| body.pair)
}

#[cfg(test)]
mod tests {
    use super::{pair_packet, read_pair, PairAction, PairState, Pairing, TYPE_PAIR};

    #[test]
    fn we_ask_and_the_peer_accepts() {
        let mut p = Pairing::new();
        assert_eq!(p.request(), PairAction::Send(true));
        assert_eq!(p.state(), PairState::Requested);
        // The peer's acceptance arrives; nothing more to send, now trusted.
        assert_eq!(p.received(true), PairAction::None);
        assert!(p.is_paired());
    }

    #[test]
    fn we_ask_and_the_peer_rejects() {
        let mut p = Pairing::new();
        p.request();
        assert_eq!(p.received(false), PairAction::None);
        assert_eq!(p.state(), PairState::Unpaired);
    }

    #[test]
    fn our_request_times_out_and_withdraws() {
        let mut p = Pairing::new();
        p.request();
        // We tell the peer to drop the pending request.
        assert_eq!(p.timeout(), PairAction::Send(false));
        assert_eq!(p.state(), PairState::Unpaired);
    }

    #[test]
    fn the_peer_asks_and_we_accept() {
        let mut p = Pairing::new();
        assert_eq!(p.received(true), PairAction::None);
        assert_eq!(p.state(), PairState::RequestedByPeer);
        assert_eq!(p.accept(), PairAction::Send(true));
        assert!(p.is_paired());
    }

    #[test]
    fn the_peer_asks_and_we_reject() {
        let mut p = Pairing::new();
        p.received(true);
        assert_eq!(p.reject(), PairAction::Send(false));
        assert_eq!(p.state(), PairState::Unpaired);
    }

    #[test]
    fn a_peer_request_we_never_answer_times_out_quietly() {
        let mut p = Pairing::new();
        p.received(true);
        // We never committed, so we send nothing; the peer's timer will fire.
        assert_eq!(p.timeout(), PairAction::None);
        assert_eq!(p.state(), PairState::Unpaired);
    }

    #[test]
    fn a_mutual_request_pairs_without_a_prompt() {
        let mut p = Pairing::new();
        // The peer asked first; then the local user also hits pair.
        p.received(true);
        assert_eq!(p.state(), PairState::RequestedByPeer);
        assert_eq!(p.request(), PairAction::Send(true));
        assert!(p.is_paired());
    }

    #[test]
    fn an_unpair_packet_drops_trust() {
        let mut p = Pairing::paired();
        assert!(p.is_paired());
        assert_eq!(p.received(false), PairAction::None);
        assert_eq!(p.state(), PairState::Unpaired);
    }

    #[test]
    fn we_unpair_and_tell_the_peer() {
        let mut p = Pairing::paired();
        assert_eq!(p.unpair(), PairAction::Send(false));
        assert_eq!(p.state(), PairState::Unpaired);
        // Unpairing an already-unpaired peer says nothing.
        assert_eq!(p.unpair(), PairAction::None);
    }

    #[test]
    fn a_restored_pairing_re_confirms_a_re_asking_peer() {
        let mut p = Pairing::paired();
        assert_eq!(p.received(true), PairAction::Send(true));
        assert!(p.is_paired());
    }

    #[test]
    fn asking_or_accepting_out_of_turn_is_a_no_op() {
        let mut p = Pairing::paired();
        assert_eq!(p.request(), PairAction::None);
        assert_eq!(p.accept(), PairAction::None);
        assert_eq!(p.reject(), PairAction::None);
        assert!(p.is_paired());
    }

    #[test]
    fn the_pair_packet_round_trips() {
        let packet = pair_packet(7, true);
        assert!(packet.is(TYPE_PAIR));
        assert_eq!(read_pair(&packet), Some(true));
        assert_eq!(read_pair(&pair_packet(8, false)), Some(false));
    }

    #[test]
    fn read_pair_ignores_a_non_pair_packet() {
        let ping = crate::packet::NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert_eq!(read_pair(&ping), None);
    }
}
