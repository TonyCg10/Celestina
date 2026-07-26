//! Connected phones, read from Magnetita's `org.celestina.Devices1` contract.
//!
//! Magnetita (the daemon) holds the phone and mounts it; this reads the small
//! session-bus interface it publishes so the sidebar can draw the device and
//! open its mount. Read-only and best-effort: no Magnetita on the bus simply
//! means no devices, never an error the user must act on. The `Changed` signal
//! drives live refresh, the same way UDisks2's add/remove drives [`volumes`].
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

/// A device Magnetita reports as connected.
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
}

/// Lists the devices Magnetita reports. `Ok(vec![])` when Magnetita is not on the
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

/// bus — an empty list, not a failure to surface.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let Ok(connection) = Connection::session() else {
        return Ok(Vec::new());
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return Ok(Vec::new());
    };
    // No Magnetita on the bus, or any call error: nothing to show, quietly.
    let raw: Vec<HashMap<String, OwnedValue>> = match proxy.call("ListDevices", &()) {
        Ok(devices) => devices,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(raw.iter().map(parse_device).collect())
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
    }
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
