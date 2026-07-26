//! The `kdeconnect.clipboard` plugin — the phone's clipboard, opt-in.
//!
//! When clipboard sync is on, the phone sends its clipboard text as it changes
//! (`kdeconnect.clipboard`) and once on connect (`kdeconnect.clipboard.connect`,
//! with a timestamp we ignore). Both carry the text under `content`; this decodes
//! it and the daemon puts it on the desktop clipboard. Receiving is what CP3
//! implements — copy on the phone, paste on the desktop; pushing the desktop's
//! clipboard back is a follow-up (it needs a continuous clipboard watch).

use serde_json::{json, Value};

use crate::packet::NetworkPacket;

/// A clipboard change.
pub const TYPE_CLIPBOARD: &str = "kdeconnect.clipboard";

/// The clipboard sent once on connect (carries a timestamp we do not need).
pub const TYPE_CLIPBOARD_CONNECT: &str = "kdeconnect.clipboard.connect";

/// Reads the text out of either clipboard packet, or `None` for another type or
/// a body with no `content`.
pub fn read_clipboard(packet: &NetworkPacket) -> Option<String> {
    if !packet.is(TYPE_CLIPBOARD) && !packet.is(TYPE_CLIPBOARD_CONNECT) {
        return None;
    }
    packet
        .body
        .as_object()?
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// A `kdeconnect.clipboard` carrying `content` — pushes a *change* of the
/// desktop's clipboard to the phone.
pub fn clipboard_packet(id: i64, content: &str) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_CLIPBOARD, json!({ "content": content }))
}

/// A `kdeconnect.clipboard.connect` carrying `content` and the millisecond
/// `timestamp` it was set — sent on connect so both ends sync their initial
/// clipboard, the newer timestamp winning.
pub fn clipboard_connect_packet(id: i64, content: &str, timestamp: i64) -> NetworkPacket {
    NetworkPacket::new(
        id,
        TYPE_CLIPBOARD_CONNECT,
        json!({ "content": content, "timestamp": timestamp }),
    )
}

#[cfg(test)]
mod tests {
    use super::{clipboard_packet, read_clipboard, TYPE_CLIPBOARD};
    use crate::packet::NetworkPacket;

    #[test]
    fn a_clipboard_change_yields_its_text() {
        let raw = r#"{"id":1,"type":"kdeconnect.clipboard","body":{"content":"hola mundo"}}"#;
        assert_eq!(
            read_clipboard(&NetworkPacket::parse(raw).unwrap()).as_deref(),
            Some("hola mundo")
        );
    }

    #[test]
    fn the_connect_variant_is_read_too() {
        let raw = r#"{"id":1,"type":"kdeconnect.clipboard.connect","body":{
            "content":"al conectar","timestamp":1700000000000}}"#;
        assert_eq!(
            read_clipboard(&NetworkPacket::parse(raw).unwrap()).as_deref(),
            Some("al conectar")
        );
    }

    #[test]
    fn our_clipboard_packet_round_trips() {
        let packet = clipboard_packet(5, "copiado");
        assert!(packet.is(TYPE_CLIPBOARD));
        assert_eq!(read_clipboard(&packet).as_deref(), Some("copiado"));
    }

    #[test]
    fn a_non_clipboard_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_clipboard(&ping).is_none());
    }
}
