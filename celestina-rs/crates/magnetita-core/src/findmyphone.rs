//! The `kdeconnect.findmyphone` plugin — ring a misplaced phone.
//!
//! One-way and trivial: a `kdeconnect.findmyphone.request` with an empty body
//! makes the phone ring at full volume until dismissed on the device. We only
//! ever send it, so this is just the packet; the app's "Sonar" button drives it.

use serde_json::json;

use crate::packet::NetworkPacket;

/// The request that rings the phone.
pub const TYPE_FINDMYPHONE_REQUEST: &str = "kdeconnect.findmyphone.request";

/// The ring request, stamped `id`.
pub fn request(id: i64) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_FINDMYPHONE_REQUEST, json!({}))
}

#[cfg(test)]
mod tests {
    use super::{request, TYPE_FINDMYPHONE_REQUEST};

    #[test]
    fn the_request_is_typed_and_empty() {
        let packet = request(1);
        assert!(packet.is(TYPE_FINDMYPHONE_REQUEST));
        assert_eq!(packet.body, serde_json::json!({}));
    }
}
