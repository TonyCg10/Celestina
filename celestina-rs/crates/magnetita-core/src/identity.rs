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

/// The protocol version this core speaks. 8 is the current KDE Connect version:
/// its handshake still sends the identity in the clear before TLS, and *also*
/// re-exchanges it encrypted once the channel is up. We must declare 8 so a v8
/// peer takes the same encrypted-re-exchange path we do — declaring less makes
/// the peer skip it while we wait for it, and the link stalls.
pub const PROTOCOL_VERSION: i32 = 8;

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
    /// Our own identity — what this desktop announces. `device_id` is stable and
    /// bound to the trust key; `device_name` is what the phone shows. The
    /// capabilities are the plugin packet types we actually handle: at CP0 just
    /// ping, growing as Magnetita earns each plugin. We always name a `tcp_port`
    /// because our announce invites the phone to connect back.
    pub fn desktop(device_id: impl Into<String>, device_name: impl Into<String>) -> Identity {
        Identity {
            device_id: device_id.into(),
            device_name: device_name.into(),
            device_type: DeviceType::Desktop,
            protocol_version: PROTOCOL_VERSION,
            // What we accept, and what we send. A peer only sends a packet type
            // the other side lists as *incoming*, so `kdeconnect.sftp` must be
            // here for the phone to answer our mount request — declaring only
            // ping is why an unlisted plugin stays silent. At CP2: ping both
            // ways, plus the sftp plugin (we send the request, receive the reply).
            incoming_capabilities: vec![
                crate::ping::TYPE_PING.to_owned(),
                crate::sftp::TYPE_SFTP.to_owned(),
                crate::battery::TYPE_BATTERY.to_owned(),
                crate::notification::TYPE_NOTIFICATION.to_owned(),
                crate::clipboard::TYPE_CLIPBOARD.to_owned(),
                crate::clipboard::TYPE_CLIPBOARD_CONNECT.to_owned(),
                crate::share::TYPE_SHARE_REQUEST.to_owned(),
                // Media, both ways: we receive the phone's now-playing reports,
                // and receive its requests to drive the desktop's own players.
                crate::mpris::TYPE_MPRIS.to_owned(),
                crate::mpris::TYPE_MPRIS_REQUEST.to_owned(),
            ],
            outgoing_capabilities: vec![
                crate::ping::TYPE_PING.to_owned(),
                crate::sftp::TYPE_SFTP_REQUEST.to_owned(),
                crate::battery::TYPE_BATTERY_REQUEST.to_owned(),
                crate::findmyphone::TYPE_FINDMYPHONE_REQUEST.to_owned(),
                // We push our clipboard too, so the phone treats us as a full
                // bidirectional clipboard peer and auto-syncs both ways.
                crate::clipboard::TYPE_CLIPBOARD.to_owned(),
                crate::clipboard::TYPE_CLIPBOARD_CONNECT.to_owned(),
                // We request and drive the phone's players, and report our own.
                crate::mpris::TYPE_MPRIS.to_owned(),
                crate::mpris::TYPE_MPRIS_REQUEST.to_owned(),
            ],
            tcp_port: Some(DEFAULT_PORT),
        }
    }

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

    #[test]
    fn our_desktop_identity_offers_the_plugins_we_handle() {
        let me = Identity::desktop("celestina-abc", "toni's Celestina");
        assert_eq!(me.device_type, DeviceType::Desktop);
        assert_eq!(me.tcp_port, Some(super::DEFAULT_PORT));
        assert!(me.can_send(crate::ping::TYPE_PING));
        assert!(me.can_receive(crate::ping::TYPE_PING));
        // CP2: we send the sftp *request* and receive the sftp *reply*.
        assert!(me.can_send(crate::sftp::TYPE_SFTP_REQUEST));
        assert!(me.can_receive(crate::sftp::TYPE_SFTP));
        // We send the request, never the reply.
        assert!(!me.can_send(crate::sftp::TYPE_SFTP));
    }
}
