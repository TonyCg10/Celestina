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
    /// The phone's now-playing, for the media card. `media_player` empty means
    /// nothing is playing; the `can_*` flags gate the transport buttons.
    pub media_player: String,
    pub media_title: String,
    pub media_artist: String,
    pub media_album: String,
    pub media_playing: bool,
    pub media_can_pause: bool,
    pub media_can_next: bool,
    pub media_can_previous: bool,
    pub media_can_seek: bool,
    pub media_length: i64,
    pub media_position: i64,
    pub media_artwork_url: String,
}

/// A paired device from the trust store, for the Settings surface — includes
/// devices remembered but currently offline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paired {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub connected: bool,
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

/// Ask Magnetita to drive the phone's media: "PlayPause", "Next" or "Previous".
pub fn media_action(device_id: &str, action: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call("MediaAction", &(device_id, action));
}

/// The paired devices (from the trust store), for the Settings surface. Empty
/// when Magnetita is not up.
pub fn list_paired() -> Vec<Paired> {
    let Ok(connection) = Connection::session() else {
        return Vec::new();
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return Vec::new();
    };
    let raw: Vec<HashMap<String, OwnedValue>> = match proxy.call("ListPaired", &()) {
        Ok(paired) => paired,
        Err(_) => return Vec::new(),
    };
    raw.iter().map(parse_paired).collect()
}

/// Ask Magnetita to forget (unpair) a device — connected or not.
pub fn forget(device_id: &str) {
    call_method("Forget", device_id);
}

/// The per-plugin toggles, as a `name → enabled` map. Empty when Magnetita is
/// not up (the controller then shows every plugin as on, the default).
pub fn plugin_settings() -> HashMap<String, bool> {
    let Ok(connection) = Connection::session() else {
        return HashMap::new();
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return HashMap::new();
    };
    proxy.call("PluginSettings", &()).unwrap_or_default()
}

/// Enable or disable a plugin (best-effort).
pub fn set_plugin(plugin: &str, enabled: bool) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return;
    };
    let _: Result<(), zbus::Error> = proxy.call("SetPlugin", &(plugin, enabled));
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
        battery: i32_field(dict, "battery"),
        charging: bool_field(dict, "charging"),
        fingerprint: str_field(dict, "fingerprint"),
        media_player: str_field(dict, "mediaPlayer"),
        media_title: str_field(dict, "mediaTitle"),
        media_artist: str_field(dict, "mediaArtist"),
        media_album: str_field(dict, "mediaAlbum"),
        media_playing: bool_field(dict, "mediaPlaying"),
        media_can_pause: bool_field(dict, "mediaCanPause"),
        media_can_next: bool_field(dict, "mediaCanNext"),
        media_can_previous: bool_field(dict, "mediaCanPrevious"),
        media_can_seek: bool_field(dict, "mediaCanSeek"),
        media_length: i64_field_or(dict, "mediaLength", -1),
        media_position: i64_field_or(dict, "mediaPosition", -1),
        media_artwork_url: str_field(dict, "mediaArtworkUrl"),
    }
}

fn parse_paired(dict: &HashMap<String, OwnedValue>) -> Paired {
    Paired {
        id: str_field(dict, "id"),
        name: str_field(dict, "name"),
        fingerprint: str_field(dict, "fingerprint"),
        connected: bool_field(dict, "connected"),
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
    i64_field_or(dict, key, 0)
}

fn i64_field_or(dict: &HashMap<String, OwnedValue>, key: &str, default: i64) -> i64 {
    dict.get(key)
        .and_then(|value| i64::try_from(value.clone()).ok())
        .unwrap_or(default)
}

/// A D-Bus `i` (int32) field — the battery is sent as one, so `i64::try_from`
/// (which only matches `x`) would miss it and read 0.
fn i32_field(dict: &HashMap<String, OwnedValue>, key: &str) -> i32 {
    dict.get(key)
        .and_then(|value| i32::try_from(value.clone()).ok())
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::parse_device;
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    #[test]
    fn a_new_media_snapshot_parses_artwork_and_progress() {
        let fields = [
            ("mediaAlbum", Value::from("Lista")),
            ("mediaCanSeek", Value::from(true)),
            ("mediaLength", Value::from(120_000_i64)),
            ("mediaPosition", Value::from(30_000_i64)),
            (
                "mediaArtworkUrl",
                Value::from("file:///run/user/1000/cover.img"),
            ),
        ];
        let dict: HashMap<String, OwnedValue> = fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("basic test value converts"),
                )
            })
            .collect();
        let device = parse_device(&dict);
        assert_eq!(device.media_album, "Lista");
        assert!(device.media_can_seek);
        assert_eq!(device.media_length, 120_000);
        assert_eq!(device.media_position, 30_000);
        assert_eq!(device.media_artwork_url, "file:///run/user/1000/cover.img");
    }

    #[test]
    fn an_older_daemon_without_progress_keys_stays_compatible() {
        let device = parse_device(&HashMap::new());
        assert_eq!(device.media_length, -1);
        assert_eq!(device.media_position, -1);
        assert!(device.media_artwork_url.is_empty());
    }

    #[test]
    fn a_battery_snapshot_preserves_charging_state() {
        let fields = [
            ("battery", Value::from(47_i32)),
            ("charging", Value::from(true)),
        ];
        let dict: HashMap<String, OwnedValue> = fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("basic test value converts"),
                )
            })
            .collect();

        let device = parse_device(&dict);
        assert_eq!(device.battery, 47);
        assert!(device.charging);
    }
}
