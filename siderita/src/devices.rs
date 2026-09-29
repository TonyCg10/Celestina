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
//! Every call here blocks on the session bus, and a Magnetita that is
//! activatable but slow to start holds a call for up to the bus's default
//! timeout. None of them may run on the Qt thread: [`crate::devicemodel`]
//! calls them on its own worker, over one connection it keeps.
//!
//! [`volumes`]: crate::volumes

use std::collections::HashMap;
use std::path::Path;

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

/// The Magnetita proxy on `connection`. Built per call because a proxy is
/// cheap; the connection under it is the one [`crate::devicemodel`] keeps.
fn proxy(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(connection, SERVICE, OBJECT, INTERFACE)
}

/// Send a local file to a device via Magnetita (best-effort — no Magnetita
/// simply does nothing).
///
/// `SendFileUri`, not `SendFile`: the argument is the percent-encoded `file://`
/// URI `celestina_core::file_uri::from_path` writes, which is the same spelling
/// the portal and the clipboard already carry and the only one that survives a
/// name that is not valid UTF-8 (ADR 0008). `SendFile` takes a plain path and
/// stays on the daemon for compatibility with other callers, but a path put
/// through `to_string_lossy` to reach it names a file Magnetita will not find.
pub fn send_file(connection: &Connection, device_id: &str, path: &Path) {
    let (Ok(magnetita), Some(uri)) = (proxy(connection), celestina_core::file_uri::from_path(path))
    else {
        return;
    };
    let _: Result<(), zbus::Error> = magnetita.call("SendFileUri", &(device_id, uri.as_str()));
}

/// Ask Magnetita to ring a connected phone (best-effort).
pub fn ring(connection: &Connection, device_id: &str) {
    if let Ok(magnetita) = proxy(connection) {
        let _: Result<(), zbus::Error> = magnetita.call("Ring", &(device_id,));
    }
}

/// Drive a connected phone's active player (best-effort).
pub fn media_action(connection: &Connection, device_id: &str, action: &str) {
    if let Ok(magnetita) = proxy(connection) {
        let _: Result<(), zbus::Error> = magnetita.call("MediaAction", &(device_id, action));
    }
}

/// Lists the devices Magnetita reports: an empty list when Magnetita is not on
/// the bus, which is not a failure to surface.
pub fn list_devices(connection: &Connection) -> Vec<Device> {
    let Ok(magnetita) = proxy(connection) else {
        return Vec::new();
    };
    let live_raw: Vec<HashMap<String, OwnedValue>> =
        magnetita.call("ListDevices", &()).unwrap_or_default();
    let paired_raw: Vec<HashMap<String, OwnedValue>> =
        magnetita.call("ListPaired", &()).unwrap_or_default();
    let live = live_raw.iter().map(parse_device).collect();
    let paired = paired_raw.iter().map(parse_paired_device).collect();
    merge_devices(live, paired)
}

/// Calls `on_change` each time Magnetita emits `Changed`, from a thread of its
/// own, for as long as the connection lives. The match rule is set up even if
/// Magnetita is not up yet, so it fires once Magnetita appears and emits.
pub fn forward_changes<F: Fn() + Send + 'static>(
    connection: &Connection,
    on_change: F,
) -> Result<(), String> {
    let magnetita = proxy(connection).map_err(|error| format!("Magnetita: {error}"))?;
    let changed = magnetita
        .receive_signal("Changed")
        .map_err(|error| format!("Magnetita: {error}"))?;
    std::thread::Builder::new()
        .name("siderita-magnetita-signal".to_owned())
        .spawn(move || {
            for _ in changed {
                on_change();
            }
        })
        .map(drop)
        .map_err(|error| format!("Magnetita: {error}"))
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
