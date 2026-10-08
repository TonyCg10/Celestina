//! The change watcher: one subscription to BlueZ's signals, filtered to the
//! ones that change a snapshot and coalesced into one call per burst by the
//! shared watcher (`bus::watch_signals`).

use std::collections::HashMap;

use zbus::zvariant::OwnedValue;
use zbus::Message;

use super::{BLUEZ, IFACE_ADAPTER, IFACE_BATTERY, IFACE_DEVICE};
use crate::bus::{self, signal_header};
use crate::error::BluetoothError;

pub use crate::bus::WatchHandle;

/// Device properties that change constantly during a search and that no row
/// shows: signal strength and advertisement payloads.
const NOISE: &[&str] = &[
    "RSSI",
    "TxPower",
    "ManufacturerData",
    "ServiceData",
    "AdvertisingData",
    "AdvertisingFlags",
];

/// Whether a `PropertiesChanged` on `interface` naming `changed` properties
/// (changed or invalidated) can change what a snapshot shows: any change to
/// the adapter or a battery, and a device change beyond the search noise.
#[must_use]
pub(super) fn relevant_change(interface: &str, changed: &[&str]) -> bool {
    match interface {
        IFACE_ADAPTER | IFACE_BATTERY => true,
        IFACE_DEVICE => changed.iter().any(|key| !NOISE.contains(key)),
        _ => false,
    }
}

fn relevant(message: &Message) -> bool {
    let (_, interface, member) = signal_header(message);
    match member.as_str() {
        // Devices found or forgotten, batteries appearing.
        "InterfacesAdded" | "InterfacesRemoved" => true,
        "PropertiesChanged" if interface == "org.freedesktop.DBus.Properties" => {
            let Ok((changed_interface, changed, invalidated)) =
                message
                    .body()
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            else {
                return false;
            };
            let keys: Vec<&str> = changed
                .keys()
                .map(String::as_str)
                .chain(invalidated.iter().map(String::as_str))
                .collect();
            relevant_change(&changed_interface, &keys)
        }
        _ => false,
    }
}

/// Calls `on_change` from a thread of its own whenever BlueZ reports a change
/// that matters — devices added or removed through its object manager, and
/// property changes on `Adapter1`, `Device1` (see `relevant_change`) and
/// `Battery1` — once per burst: signals within 300 ms of each other are
/// coalesced into one call, made at most 1.5 s after the burst's first.
pub fn watch(on_change: impl Fn() + Send + 'static) -> Result<WatchHandle, BluetoothError> {
    bus::watch_signals(BLUEZ, "bluez", relevant, on_change).map_err(BluetoothError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_snapshot_changing_properties_count() {
        assert!(relevant_change(IFACE_ADAPTER, &["Discovering"]));
        assert!(relevant_change(IFACE_BATTERY, &["Percentage"]));
        assert!(relevant_change(IFACE_DEVICE, &["Connected"]));
        assert!(relevant_change(IFACE_DEVICE, &["RSSI", "Paired"]));
        assert!(!relevant_change(IFACE_DEVICE, &["RSSI"]));
        assert!(!relevant_change(
            IFACE_DEVICE,
            &["RSSI", "ManufacturerData"]
        ));
        assert!(!relevant_change("org.bluez.MediaTransport1", &["State"]));
    }
}
