//! Known phones, read from Magnetita's `org.celestina.Devices1` contract.
//!
//! Magnetita (the daemon) holds the phone and mounts it; this reads the small
//! session-bus interface it publishes so the sidebar can draw the device and
//! open its mount. Paired devices remain visible while offline by merging
//! `ListPaired` with the richer live records from `ListDevices`. Read-only and
//! best-effort: no Magnetita on the bus simply means no devices, never an error
//! the user must act on. The `Changed` signal drives live refresh, the same way
//! UDisks2's add/remove drives [`volumes`].
//!
//! [`volumes`]: crate::volumes

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

const SERVICE: &str = "org.celestina.Magnetita";
const OBJECT: &str = "/org/celestina/Devices1";
const INTERFACE: &str = "org.celestina.Devices1";

/// A device Magnetita knows, with live-only fields empty while it is offline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// "phone", "tablet", "laptop", "desktop", "tv", "unknown".
    pub device_type: String,
    pub connected: bool,
    pub mounted: bool,
    /// Where it is mounted locally, or empty when not (yet) mounted.
    pub mount_path: String,
    pub media_player: String,
    pub media_title: String,
    pub media_artist: String,
    pub media_album: String,
    pub media_artwork_url: String,
    pub media_playing: bool,
    pub media_can_pause: bool,
    pub media_can_next: bool,
    pub media_can_previous: bool,
    pub media_length: i64,
    pub media_position: i64,
}

/// Send a local file to a device via Magnetita (best-effort — no bus or no
/// Magnetita simply does nothing).
pub fn send_file(device_id: &str, path: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call("SendFile", &(device_id, path));
}

/// Ask Magnetita to ring a connected phone (best-effort).
pub fn ring(device_id: &str) {
    call_device_method("Ring", device_id);
}

/// Drive a connected phone's active player (best-effort).
pub fn media_action(device_id: &str, action: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call("MediaAction", &(device_id, action));
}

/// Lists the devices Magnetita reports. `Ok(vec![])` when Magnetita is not on
/// the bus — an empty list, not a failure to surface.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let Ok(connection) = Connection::session() else {
        return Ok(Vec::new());
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return Ok(Vec::new());
    };
    let live_raw: Vec<HashMap<String, OwnedValue>> =
        proxy.call("ListDevices", &()).unwrap_or_default();
    let paired_raw: Vec<HashMap<String, OwnedValue>> =
        proxy.call("ListPaired", &()).unwrap_or_default();
    let live = live_raw.iter().map(parse_device).collect();
    let paired = paired_raw.iter().map(parse_paired_device).collect();
    Ok(merge_devices(live, paired))
}

/// Blocks watching the `Changed` signal, calling `on_change` (coalesced over a
/// short burst) each time the device set or a device's state changes. The match
/// rule is set up even if Magnetita is not up yet, so it fires once Magnetita
/// appears and emits.
pub fn watch_changes<F: Fn() + Send + 'static>(on_change: F) -> Result<(), String> {
    let connection =
        Connection::session().map_err(|error| format!("bus de sesión no disponible: {error}"))?;
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE)
        .map_err(|error| format!("Magnetita no disponible: {error}"))?;
    let changed = proxy
        .receive_signal("Changed")
        .map_err(|error| format!("Magnetita: {error}"))?;

    let (tx, rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        for _ in changed {
            if tx.send(()).is_err() {
                break;
            }
        }
    });

    while rx.recv().is_ok() {
        // Drain a burst (a connect that also mounts fires twice), then reload.
        while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}
        on_change();
    }
    // Keep the connection alive for the whole watch.
    drop(connection);
    Ok(())
}

fn parse_device(dict: &HashMap<String, OwnedValue>) -> Device {
    Device {
        id: str_field(dict, "id"),
        name: str_field(dict, "name"),
        device_type: str_field(dict, "type"),
        connected: bool_field(dict, "connected"),
        mounted: bool_field(dict, "mounted"),
        mount_path: str_field(dict, "mountPath"),
        media_player: str_field(dict, "mediaPlayer"),
        media_title: str_field(dict, "mediaTitle"),
        media_artist: str_field(dict, "mediaArtist"),
        media_album: str_field(dict, "mediaAlbum"),
        media_artwork_url: str_field(dict, "mediaArtworkUrl"),
        media_playing: bool_field(dict, "mediaPlaying"),
        media_can_pause: bool_field(dict, "mediaCanPause"),
        media_can_next: bool_field(dict, "mediaCanNext"),
        media_can_previous: bool_field(dict, "mediaCanPrevious"),
        media_length: i64_field(dict, "mediaLength", -1),
        media_position: i64_field(dict, "mediaPosition", -1),
    }
}

fn parse_paired_device(dict: &HashMap<String, OwnedValue>) -> Device {
    Device {
        id: str_field(dict, "id"),
        name: str_field(dict, "name"),
        connected: bool_field(dict, "connected"),
        ..Device::default()
    }
}

fn merge_devices(mut live: Vec<Device>, paired: Vec<Device>) -> Vec<Device> {
    for known in paired {
        if known.id.is_empty() || live.iter().any(|device| device.id == known.id) {
            continue;
        }
        live.push(known);
    }
    live
}

fn call_device_method(method: &'static str, device_id: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call(method, &(device_id,));
}

fn str_field(dict: &HashMap<String, OwnedValue>, key: &str) -> String {
    dict.get(key)
        .and_then(|value| String::try_from(value.clone()).ok())
        .unwrap_or_default()
}

fn bool_field(dict: &HashMap<String, OwnedValue>, key: &str) -> bool {
    dict.get(key)
        .and_then(|value| bool::try_from(value.clone()).ok())
        .unwrap_or(false)
}

fn i64_field(dict: &HashMap<String, OwnedValue>, key: &str, fallback: i64) -> i64 {
    dict.get(key)
        .and_then(|value| i64::try_from(value.clone()).ok())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::{merge_devices, Device};

    #[test]
    fn paired_devices_stay_visible_while_offline() {
        let devices = merge_devices(
            vec![Device {
                id: "online".to_owned(),
                name: "Galaxy".to_owned(),
                connected: true,
                mounted: true,
                ..Device::default()
            }],
            vec![
                Device {
                    id: "online".to_owned(),
                    name: "Old duplicate".to_owned(),
                    ..Device::default()
                },
                Device {
                    id: "offline".to_owned(),
                    name: "Pixel".to_owned(),
                    ..Device::default()
                },
            ],
        );

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Galaxy");
        assert_eq!(devices[1].name, "Pixel");
        assert!(!devices[1].connected);
        assert!(devices[1].mount_path.is_empty());
    }

    #[test]
    fn malformed_paired_records_do_not_create_blank_rows() {
        assert!(merge_devices(Vec::new(), vec![Device::default()]).is_empty());
    }
}
