//! The KDE Connect wire format: one JSON object per line.
//!
//! Every message is a [`NetworkPacket`] — a `type`, a monotonically-increasing
//! `id` (a millisecond timestamp, supplied by the caller so this stays pure and
//! testable), and a `body` whose shape depends on the type. A packet that
//! carries a file also names a `payloadSize` and, out of band, a
//! `payloadTransferInfo` port on which a second TLS socket streams the bytes.
//!
//! On the wire a packet is a single line terminated by `\n`, because the
//! transport is a line-delimited TLS stream, not a length-framed one. Parsing
//! therefore works one line at a time; this module owns the envelope, and each
//! plugin owns the shape of its own `body`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One KDE Connect packet: the envelope shared by every plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkPacket {
    /// A monotonically-increasing id; KDE Connect uses a millisecond timestamp.
    /// It is a field, not a clock read, so the core never touches the wall time.
    pub id: i64,
    /// The packet type, e.g. `kdeconnect.identity` or `kdeconnect.battery`.
    #[serde(rename = "type")]
    pub packet_type: String,
    /// The type-specific payload. Kept opaque here so the envelope does not need
    /// to know every plugin; each plugin deserializes it into its own struct.
    pub body: Value,
    /// Bytes of an out-of-band payload (a file), or `-1` for a stream of unknown
    /// length. Absent for the ordinary control packets.
    #[serde(rename = "payloadSize", skip_serializing_if = "Option::is_none")]
    pub payload_size: Option<i64>,
    /// How to fetch that payload — `{ "port": N }` for a second TLS socket.
    #[serde(
        rename = "payloadTransferInfo",
        skip_serializing_if = "Option::is_none"
    )]
    pub payload_transfer_info: Option<Value>,
}

impl NetworkPacket {
    /// A control packet of `packet_type` carrying `body`, stamped `id`.
    pub fn new(id: i64, packet_type: impl Into<String>, body: Value) -> Self {
        NetworkPacket {
            id,
            packet_type: packet_type.into(),
            body,
            payload_size: None,
            payload_transfer_info: None,
        }
    }

    /// Parses one line off the wire (the trailing newline already stripped).
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim_end_matches(['\r', '\n']))
    }

    /// The single line to put on the wire — no trailing newline; the caller adds
    /// the `\n` the transport delimits on. Serialization of a `NetworkPacket`
    /// cannot fail (its shape is always valid JSON), so this does not return a
    /// `Result`.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("a NetworkPacket is always valid JSON")
    }

    /// True if this packet is of the given type — a small readability helper for
    /// the plugin dispatch that reads many packets.
    pub fn is(&self, packet_type: &str) -> bool {
        self.packet_type == packet_type
    }
}

#[cfg(test)]
mod tests {
    use super::NetworkPacket;
    use serde_json::json;

    #[test]
    fn a_control_packet_round_trips_through_a_line() {
        let packet = NetworkPacket::new(1_700_000_000_000, "kdeconnect.ping", json!({}));
        let line = packet.to_line();
        assert!(
            !line.contains('\n'),
            "the line carries no newline of its own"
        );
        // A control packet omits the payload fields entirely.
        assert!(!line.contains("payloadSize"));
        assert_eq!(NetworkPacket::parse(&line).unwrap(), packet);
    }

    #[test]
    fn parse_strips_the_transport_newline() {
        let packet =
            NetworkPacket::parse("{\"id\":7,\"type\":\"kdeconnect.ping\",\"body\":{}}\n").unwrap();
        assert_eq!(packet.id, 7);
        assert!(packet.is("kdeconnect.ping"));
    }

    #[test]
    fn a_payload_packet_keeps_its_size_and_transfer_port() {
        let raw = r#"{"id":9,"type":"kdeconnect.share.request","body":{"filename":"a.jpg"},"payloadSize":1024,"payloadTransferInfo":{"port":1739}}"#;
        let packet = NetworkPacket::parse(raw).unwrap();
        assert_eq!(packet.payload_size, Some(1024));
        assert_eq!(packet.payload_transfer_info.unwrap()["port"], 1739);
    }

    #[test]
    fn a_malformed_line_is_an_error_not_a_panic() {
        assert!(NetworkPacket::parse("not json").is_err());
    }
}
