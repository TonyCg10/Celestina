//! Ping — the one plugin CP0 needs: a liveness poke with an empty body.
//!
//! It is one-way by design in KDE Connect: a `kdeconnect.ping` arrives and the
//! peer simply notes it (the reference app shows a notification). "Both ways"
//! in CP0 means each side sends its own; there is no pong. So this module is
//! only the packet — sending is [`ping_packet`], receiving is handled by the
//! session, which turns it into a [`Pinged`](crate::event::ConnectionEvent::Pinged)
//! event.

use crate::packet::NetworkPacket;

/// The ping packet type.
pub const TYPE_PING: &str = "kdeconnect.ping";

/// An empty `kdeconnect.ping`, stamped `id`.
pub fn ping_packet(id: i64) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_PING, serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::{ping_packet, TYPE_PING};

    #[test]
    fn a_ping_is_typed_and_empty() {
        let packet = ping_packet(3);
        assert!(packet.is(TYPE_PING));
        assert_eq!(packet.body, serde_json::json!({}));
        assert!(packet.payload_size.is_none());
    }
}
