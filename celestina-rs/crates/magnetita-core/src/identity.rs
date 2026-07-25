//! The identity packet — the first thing two devices exchange, and the only one
//! ever sent unencrypted: a device broadcasts it over UDP to announce itself,
//! and both ends resend it as the first line of a fresh TLS link. It says who
//! the device is and which packet types it can send and receive, so each side
//! knows what the other supports before a single plugin runs.
//!
//! It carries no secret — pairing and trust come later, over TLS — so leaving it
//! in the clear is the design, not a leak. The capabilities are advisory: a
//! device only offers what it can serve, and this side only drives the plugins
//! both sides list.

use serde::{Deserialize, Serialize};

use crate::packet::NetworkPacket;

/// The `type` of an identity packet.
pub const TYPE_IDENTITY: &str = "kdeconnect.identity";

/// The port KDE Connect listens and broadcasts on (UDP announce, TCP link).
pub const DEFAULT_PORT: u16 = 1716;

/// The protocol version this core speaks. 7 is the long-stable version the
/// current Android app and Valent interoperate on.
pub const PROTOCOL_VERSION: i32 = 7;

/// What a device calls itself — drives the icon and, later, per-type behaviour.
/// Unknown keeps a strange value from rejecting an otherwise-valid identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Laptop,
    Phone,
    Tablet,
    Tv,
    #[default]
    #[serde(other)]
    Unknown,
}

/// The body of an identity packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    /// A stable per-device id (the trust key is bound to it, not to the name).
    #[serde(rename = "deviceId")]
    pub device_id: String,
    /// The human name shown to the user.
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "deviceType", default)]
    pub device_type: DeviceType,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: i32,
    /// Packet types this device can receive (so we know what to send it).
    #[serde(rename = "incomingCapabilities", default)]
    pub incoming_capabilities: Vec<String>,
    /// Packet types this device can send (so we know what to expect from it).
    #[serde(rename = "outgoingCapabilities", default)]
    pub outgoing_capabilities: Vec<String>,
    /// The TCP port to connect back on. Present in a UDP announce; absent once a
    /// link is already open.
    #[serde(rename = "tcpPort", skip_serializing_if = "Option::is_none")]
    pub tcp_port: Option<u16>,
}

impl Identity {
    /// Wraps this identity in a [`NetworkPacket`] stamped `id`.
    pub fn to_packet(&self, id: i64) -> NetworkPacket {
        NetworkPacket::new(
            id,
            TYPE_IDENTITY,
            serde_json::to_value(self).expect("an Identity is always valid JSON"),
        )
    }

    /// Reads an identity out of a packet, or `None` if it is a different type or
    /// its body does not fit — a peer that sends a malformed identity is ignored,
    /// never trusted on a guess.
    pub fn from_packet(packet: &NetworkPacket) -> Option<Identity> {
        if !packet.is(TYPE_IDENTITY) {
            return None;
        }
        serde_json::from_value(packet.body.clone()).ok()
    }

    /// Whether the device both offers `outgoing` and accepts our matching send —
    /// the honest test before this side drives a plugin.
    pub fn can_send(&self, packet_type: &str) -> bool {
        self.outgoing_capabilities.iter().any(|c| c == packet_type)
    }

    pub fn can_receive(&self, packet_type: &str) -> bool {
        self.incoming_capabilities.iter().any(|c| c == packet_type)
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceType, Identity, PROTOCOL_VERSION, TYPE_IDENTITY};
    use crate::packet::NetworkPacket;

    fn a_desktop() -> Identity {
        Identity {
            device_id: "celestina-desktop".to_owned(),
            device_name: "toni's Celestina".to_owned(),
            device_type: DeviceType::Desktop,
            protocol_version: PROTOCOL_VERSION,
            incoming_capabilities: vec!["kdeconnect.share.request".to_owned()],
            outgoing_capabilities: vec!["kdeconnect.ping".to_owned()],
            tcp_port: Some(1716),
        }
    }

    #[test]
    fn an_identity_round_trips_through_a_packet() {
        let identity = a_desktop();
        let packet = identity.to_packet(42);
        assert!(packet.is(TYPE_IDENTITY));
        assert_eq!(Identity::from_packet(&packet).unwrap(), identity);
    }

    #[test]
    fn a_phone_announce_parses_with_its_type_and_port() {
        // Shaped like what the Android app broadcasts over UDP.
        let raw = r#"{"id":1700000000000,"type":"kdeconnect.identity","body":{
            "deviceId":"a1b2c3","deviceName":"Pixel","deviceType":"phone",
            "protocolVersion":7,"tcpPort":1716,
            "incomingCapabilities":["kdeconnect.share.request"],
            "outgoingCapabilities":["kdeconnect.battery","kdeconnect.sftp"]}}"#;
        let identity = Identity::from_packet(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert_eq!(identity.device_type, DeviceType::Phone);
        assert_eq!(identity.tcp_port, Some(1716));
        assert!(identity.can_send("kdeconnect.sftp"));
        assert!(!identity.can_send("kdeconnect.ping"));
    }

    #[test]
    fn an_unknown_device_type_does_not_reject_the_identity() {
        let raw = r#"{"id":1,"type":"kdeconnect.identity","body":{
            "deviceId":"x","deviceName":"Fridge","deviceType":"smartfridge",
            "protocolVersion":7}}"#;
        let identity = Identity::from_packet(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert_eq!(identity.device_type, DeviceType::Unknown);
    }

    #[test]
    fn a_non_identity_packet_yields_none() {
        let packet = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(Identity::from_packet(&packet).is_none());
    }
}
