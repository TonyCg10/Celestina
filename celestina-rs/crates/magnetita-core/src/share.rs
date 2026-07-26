//! The `kdeconnect.share.request` plugin — files from the phone.
//!
//! A shared file is not in the packet: the control packet names the file and its
//! `payloadSize`, and points at a `payloadTransferInfo` port where the phone has
//! opened a *second* TLS socket serving the bytes. This module decodes that into
//! an [`IncomingFile`]; the daemon dials the payload port and streams it to disk
//! (the transport's job, in magnetita-net).
//!
//! A share can instead be text or a URL (no payload) — those have no `filename`
//! or size, so [`read_share`] returns `None` and the daemon leaves them for a
//! later, non-file path. CP3 does the file direction, phone → PC.

use serde_json::Value;

use crate::packet::NetworkPacket;

/// The share packet type (file, text, or URL).
pub const TYPE_SHARE_REQUEST: &str = "kdeconnect.share.request";

/// A file the phone is sending: its name, its size, and the port to fetch it on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncomingFile {
    pub filename: String,
    /// Bytes to read from the payload socket, or -1 for a stream of unknown
    /// length (read until the peer closes).
    pub size: i64,
    /// The TCP port of the phone's payload socket.
    pub port: u16,
}

/// Reads a file share, or `None` when the packet is another type or a non-file
/// share (text/URL, which carry no `filename` + payload).
pub fn read_share(packet: &NetworkPacket) -> Option<IncomingFile> {
    if !packet.is(TYPE_SHARE_REQUEST) {
        return None;
    }
    let filename = packet
        .body
        .as_object()?
        .get("filename")
        .and_then(Value::as_str)?
        .to_owned();
    let size = packet.payload_size?;
    let port = packet
        .payload_transfer_info
        .as_ref()
        .and_then(|info| info.get("port"))
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())?;
    Some(IncomingFile {
        filename,
        size,
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::{read_share, IncomingFile};
    use crate::packet::NetworkPacket;

    #[test]
    fn a_file_share_names_the_file_size_and_port() {
        let raw = r#"{"id":1,"type":"kdeconnect.share.request",
            "body":{"filename":"foto.jpg"},
            "payloadSize":204800,"payloadTransferInfo":{"port":1740}}"#;
        assert_eq!(
            read_share(&NetworkPacket::parse(raw).unwrap()),
            Some(IncomingFile {
                filename: "foto.jpg".to_owned(),
                size: 204800,
                port: 1740,
            })
        );
    }

    #[test]
    fn a_text_share_has_no_file() {
        // A shared clipboard text: no filename, no payload.
        let raw = r#"{"id":1,"type":"kdeconnect.share.request","body":{"text":"hola"}}"#;
        assert!(read_share(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn a_file_without_a_transfer_port_is_not_fetchable() {
        let raw = r#"{"id":1,"type":"kdeconnect.share.request",
            "body":{"filename":"foto.jpg"},"payloadSize":1024}"#;
        assert!(read_share(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn a_non_share_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_share(&ping).is_none());
    }
}
