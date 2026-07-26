//! The `kdeconnect.battery` plugin — the phone's charge, for the sidebar and app.
//!
//! The phone reports its battery unprompted on connect and on every change, and
//! also answers a `kdeconnect.battery.request`. We send the request on connect
//! (so a missed unsolicited report is not a blank battery) and read every report
//! into a [`Battery`] the daemon puts on `org.celestina.Devices1` — the field the
//! sidebar and the app already show.
//!
//! Pure: this decodes packets; storing and serving the value is the daemon's.

use serde_json::{json, Value};

use crate::packet::NetworkPacket;

/// The battery report packet type.
pub const TYPE_BATTERY: &str = "kdeconnect.battery";

/// The packet type that asks the phone to report its battery.
pub const TYPE_BATTERY_REQUEST: &str = "kdeconnect.battery.request";

/// A phone's battery state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// Charge percent 0–100, or -1 if the phone did not say.
    pub charge: i32,
    pub charging: bool,
}

/// The request that asks the phone to report its battery now.
pub fn request(id: i64) -> NetworkPacket {
    NetworkPacket::new(id, TYPE_BATTERY_REQUEST, json!({ "request": true }))
}

/// Reads a `kdeconnect.battery` report, or `None` for a different type. A report
/// missing `currentCharge` yields `charge = -1` rather than nothing, so a partial
/// report still clears "unknown".
pub fn read_battery(packet: &NetworkPacket) -> Option<Battery> {
    if !packet.is(TYPE_BATTERY) {
        return None;
    }
    let body = packet.body.as_object()?;
    let charge = body
        .get("currentCharge")
        .and_then(Value::as_i64)
        .map(|charge| charge as i32)
        .unwrap_or(-1);
    let charging = body
        .get("isCharging")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(Battery { charge, charging })
}

#[cfg(test)]
mod tests {
    use super::{read_battery, request, Battery, TYPE_BATTERY_REQUEST};
    use crate::packet::NetworkPacket;

    #[test]
    fn the_request_asks_for_a_report() {
        let packet = request(1);
        assert!(packet.is(TYPE_BATTERY_REQUEST));
        assert_eq!(packet.body["request"], true);
    }

    #[test]
    fn a_report_parses_charge_and_charging() {
        let raw = r#"{"id":1,"type":"kdeconnect.battery","body":{
            "currentCharge":83,"isCharging":true,"thresholdEvent":0}}"#;
        assert_eq!(
            read_battery(&NetworkPacket::parse(raw).unwrap()),
            Some(Battery {
                charge: 83,
                charging: true
            })
        );
    }

    #[test]
    fn a_report_without_charge_is_unknown_not_absent() {
        let raw = r#"{"id":1,"type":"kdeconnect.battery","body":{"isCharging":false}}"#;
        assert_eq!(
            read_battery(&NetworkPacket::parse(raw).unwrap()),
            Some(Battery {
                charge: -1,
                charging: false
            })
        );
    }

    #[test]
    fn a_non_battery_packet_is_ignored() {
        let ping = NetworkPacket::new(1, "kdeconnect.ping", serde_json::json!({}));
        assert!(read_battery(&ping).is_none());
    }
}
