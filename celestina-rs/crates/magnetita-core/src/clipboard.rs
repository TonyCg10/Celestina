//! The `kdeconnect.clipboard` plugin — the phone's clipboard, opt-in.
//!
//! When clipboard sync is on, the phone sends its clipboard text as it changes
//! (`kdeconnect.clipboard`) and once on connect (`kdeconnect.clipboard.connect`,
//! with a timestamp we ignore). Both carry the text under `content`; this decodes
//! it and the daemon puts it on the desktop clipboard. Sync is bidirectional:
//! the daemon also pushes the desktop's clipboard as it changes (a `wl-paste`
//! watch) — the remaining ceiling is the phone's (Android forbids background
//! clipboard reads, so phone → desktop only arrives on a manual send).
//!
//! Both directions pass through [`is_syncable`]. Clipboard sync is a courtesy
//! for text a person copied, so this is the layer that decides what counts as
//! that — not the transport, and not whatever framing an adapter happens to
//! use to move a value between processes.

use serde_json::{json, Value};

use crate::packet::NetworkPacket;

/// A clipboard change.
pub const TYPE_CLIPBOARD: &str = "kdeconnect.clipboard";

/// The clipboard sent once on connect (carries a timestamp we do not need).
pub const TYPE_CLIPBOARD_CONNECT: &str = "kdeconnect.clipboard.connect";

/// The largest clipboard text either end will accept. A person's copied text
/// does not reach this; a payload that does is a document or a mis-typed binary
/// selection, and syncing it only floods the peer's clipboard history.
pub const MAX_CLIPBOARD_BYTES: usize = 64 * 1024;

/// Whether `text` is clipboard content worth syncing.
///
/// Empty carries nothing. A NUL means some layer decoded bytes that were never
/// text — a lossy UTF-8 decode of an image selection produces exactly that, a
/// string of replacement characters that still carries the original NULs — and
/// such a value must not reach a peer under any framing. Oversized content is
/// refused rather than truncated: half a clipboard is not what was copied.
pub fn is_syncable(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CLIPBOARD_BYTES && !text.contains('\0')
}

/// Reads the text out of either clipboard packet, or `None` for another type, a
/// body with no `content`, or content [`is_syncable`] rejects. Incoming content
/// is filtered by the same rule as outgoing: what a peer sends is applied to
/// this desktop's clipboard, where it becomes the next thing we would send.
pub fn read_clipboard(packet: &NetworkPacket) -> Option<String> {
    if !packet.is(TYPE_CLIPBOARD) && !packet.is(TYPE_CLIPBOARD_CONNECT) {
        return None;
    }
    packet
        .body
        .as_object()?
        .get("content")
        .and_then(Value::as_str)
        .filter(|content| is_syncable(content))
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
    use super::{
        clipboard_packet, is_syncable, read_clipboard, MAX_CLIPBOARD_BYTES, TYPE_CLIPBOARD,
    };
    use crate::packet::NetworkPacket;

    /// What a lossy UTF-8 decode of a binary selection looks like: replacement
    /// characters around the NULs the original bytes carried.
    fn lossily_decoded_binary() -> String {
        "\u{fffd}PNG\r\n\u{1a}\n\0\0\0\rIHDR\0\0\0@".to_owned()
    }

    #[test]
    fn ordinary_copied_text_is_syncable() {
        assert!(is_syncable("hola mundo"));
        assert!(is_syncable("varias\nlíneas\ty tabuladores"));
        assert!(is_syncable(&"a".repeat(MAX_CLIPBOARD_BYTES)));
    }

    #[test]
    fn an_empty_selection_is_not_syncable() {
        assert!(!is_syncable(""));
    }

    #[test]
    fn content_carrying_a_nul_is_not_syncable() {
        assert!(!is_syncable("\0"));
        assert!(!is_syncable("texto\0con nul"));
        assert!(!is_syncable(&lossily_decoded_binary()));
    }

    #[test]
    fn oversized_content_is_refused_not_truncated() {
        assert!(!is_syncable(&"a".repeat(MAX_CLIPBOARD_BYTES + 1)));
    }

    #[test]
    fn a_clipboard_packet_carrying_a_nul_is_dropped() {
        let packet = clipboard_packet(1, &lossily_decoded_binary());
        assert!(read_clipboard(&packet).is_none());
    }

    #[test]
    fn an_oversized_clipboard_packet_is_dropped() {
        let packet = clipboard_packet(1, &"a".repeat(MAX_CLIPBOARD_BYTES + 1));
        assert!(read_clipboard(&packet).is_none());
    }

    #[test]
    fn an_empty_clipboard_packet_is_dropped() {
        let raw = r#"{"id":1,"type":"kdeconnect.clipboard","body":{"content":""}}"#;
        assert!(read_clipboard(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

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
