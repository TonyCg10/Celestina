//! The `kdeconnect.notification` plugin — the phone's notifications, decoded.
//!
//! The phone sends one of these when a notification appears, updates, or is
//! dismissed (`isCancel`), keyed by a stable `id` so an update replaces and a
//! cancel withdraws. This module only decodes the packet into a [`Notification`];
//! the daemon posts it to `org.freedesktop.Notifications`, mapping the phone's id
//! to the desktop server's so a later replace or cancel finds it.
//!
//! We accept notifications (an incoming capability) but send nothing here — the
//! request for *existing* notifications on connect is the daemon's choice, kept
//! out of the wire types.

use serde_json::Value;

use crate::packet::NetworkPacket;

/// The notification packet type.
pub const TYPE_NOTIFICATION: &str = "kdeconnect.notification";

/// The type that asks the phone to (re)send its current notifications.
pub const TYPE_NOTIFICATION_REQUEST: &str = "kdeconnect.notification.request";

/// A phone notification, or its withdrawal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    /// Stable per-notification id — the key for replace and cancel.
    pub id: String,
    /// The app that raised it, e.g. "WhatsApp". Empty on a cancel.
    pub app_name: String,
    pub title: String,
    pub text: String,
    /// True when the phone is *withdrawing* this notification, not raising it.
    pub is_cancel: bool,
}

/// Reads a `kdeconnect.notification`, or `None` for a different type or one with
/// no `id` (the one field both a raise and a cancel must carry).
pub fn read_notification(packet: &NetworkPacket) -> Option<Notification> {
    if !packet.is(TYPE_NOTIFICATION) {
        return None;
    }
    let body = packet.body.as_object()?;
    let id = body.get("id").and_then(Value::as_str)?.to_owned();
    Some(Notification {
        id,
        app_name: string_field(body, "appName"),
        title: string_field(body, "title"),
        text: string_field(body, "text"),
        is_cancel: body
            .get("isCancel")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn string_field(body: &serde_json::Map<String, Value>, key: &str) -> String {
    body.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{read_notification, Notification};
    use crate::packet::NetworkPacket;

    #[test]
    fn a_notification_parses_its_fields() {
        let raw = r#"{"id":1,"type":"kdeconnect.notification","body":{
            "id":"0|com.whatsapp|1|null|10123","appName":"WhatsApp",
            "title":"Alice","text":"¿Vienes?","isClearable":true,"time":"1700"}}"#;
        assert_eq!(
            read_notification(&NetworkPacket::parse(raw).unwrap()),
            Some(Notification {
                id: "0|com.whatsapp|1|null|10123".to_owned(),
                app_name: "WhatsApp".to_owned(),
                title: "Alice".to_owned(),
                text: "¿Vienes?".to_owned(),
                is_cancel: false,
            })
        );
    }

    #[test]
    fn a_cancel_carries_only_its_id() {
        let raw = r#"{"id":1,"type":"kdeconnect.notification","body":{
            "id":"0|com.whatsapp|1|null|10123","isCancel":true}}"#;
        let notification = read_notification(&NetworkPacket::parse(raw).unwrap()).unwrap();
        assert!(notification.is_cancel);
        assert_eq!(notification.id, "0|com.whatsapp|1|null|10123");
        assert!(notification.app_name.is_empty());
    }

    #[test]
    fn a_notification_without_an_id_is_ignored() {
        let raw = r#"{"id":1,"type":"kdeconnect.notification","body":{"title":"x"}}"#;
        assert!(read_notification(&NetworkPacket::parse(raw).unwrap()).is_none());
    }

    #[test]
    fn a_non_notification_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_notification(&ping).is_none());
    }
}
