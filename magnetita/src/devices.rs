//! The `org.celestina.Devices1` client — the app reads the same contract Siderita
//! does, from the magnetitad service.
//!
//! Read-only and best-effort: no Magnetita on the bus means no devices, never an
//! error the user must act on. The `Changed` signal drives live refresh.

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
    pub device_type: String,
    pub connected: bool,
    pub mounted: bool,
    pub paired: bool,
    pub mount_path: String,
    /// Battery percent, or -1 when unknown.
    pub battery: i32,
    pub charging: bool,
    /// The peer certificate fingerprint — the verification key to show.
    pub fingerprint: String,
}

/// One connection-log entry from the daemon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogEntry {
    pub device: String,
    pub message: String,
    pub failure: bool,
    pub time_ms: i64,
}

/// Lists the devices Magnetita reports. `Ok(vec![])` when Magnetita is not on the
/// bus — an empty list, not a failure.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let Ok(connection) = Connection::session() else {
        return Ok(Vec::new());
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return Ok(Vec::new());
    };
    let raw: Vec<HashMap<String, OwnedValue>> = match proxy.call("ListDevices", &()) {
        Ok(devices) => devices,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(raw.iter().map(parse_device).collect())
}

/// The recent connection log, oldest first. Empty when Magnetita is not up.
pub fn recent_log() -> Result<Vec<LogEntry>, String> {
    let Ok(connection) = Connection::session() else {
        return Ok(Vec::new());
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return Ok(Vec::new());
    };
    let raw: Vec<HashMap<String, OwnedValue>> = match proxy.call("RecentLog", &()) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(raw.iter().map(parse_log).collect())
}

/// Blocks watching the device-set `Changed` signal, coalesced.
pub fn watch_changes<F: Fn() + Send + 'static>(on_change: F) -> Result<(), String> {
    watch("Changed", on_change)
}

/// Blocks watching the connection-log `Event` signal, coalesced.
pub fn watch_events<F: Fn() + Send + 'static>(on_event: F) -> Result<(), String> {
    watch("Event", on_event)
}

/// The shared signal-watch loop: subscribe, coalesce a burst, call back. The
/// match rule is set up even if Magnetita is not up yet, so it fires once it
/// appears.
fn watch<F: Fn() + Send + 'static>(signal: &'static str, on_change: F) -> Result<(), String> {
    let connection =
        Connection::session().map_err(|error| format!("bus de sesión no disponible: {error}"))?;
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE)
        .map_err(|error| format!("Magnetita no disponible: {error}"))?;
    let signals = proxy
        .receive_signal(signal)
        .map_err(|error| format!("Magnetita: {error}"))?;

    let (tx, rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        for _ in signals {
            if tx.send(()).is_err() {
                break;
            }
        }
    });

    while rx.recv().is_ok() {
        while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}
        on_change();
    }
    drop(connection);
    Ok(())
}

/// Ask Magnetita to pair with the connected device (best-effort).
pub fn request_pair(device_id: &str) {
    call_method("RequestPair", device_id);
}

/// Ask Magnetita to drop the pairing (best-effort).
pub fn unpair(device_id: &str) {
    call_method("Unpair", device_id);
}

/// Ask Magnetita to ring the device (find-my-phone).
pub fn ring(device_id: &str) {
    call_method("Ring", device_id);
}

fn call_method(method: &'static str, device_id: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call(method, &(device_id,));
}

fn parse_device(dict: &HashMap<String, OwnedValue>) -> Device {
    Device {
        id: str_field(dict, "id"),
        name: str_field(dict, "name"),
        device_type: str_field(dict, "type"),
        connected: bool_field(dict, "connected"),
        mounted: bool_field(dict, "mounted"),
        paired: bool_field(dict, "paired"),
        mount_path: str_field(dict, "mountPath"),
        battery: i64_field(dict, "battery") as i32,
        charging: bool_field(dict, "charging"),
        fingerprint: str_field(dict, "fingerprint"),
    }
}

fn parse_log(dict: &HashMap<String, OwnedValue>) -> LogEntry {
    LogEntry {
        device: str_field(dict, "device"),
        message: str_field(dict, "message"),
        failure: bool_field(dict, "failure"),
        time_ms: i64_field(dict, "time"),
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

fn i64_field(dict: &HashMap<String, OwnedValue>, key: &str) -> i64 {
    dict.get(key)
        .and_then(|value| i64::try_from(value.clone()).ok())
        .unwrap_or(0)
}
