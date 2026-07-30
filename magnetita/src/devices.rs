//! The `org.celestina.Devices1` client — the app reads the same contract Siderita
//! does, from the magnetitad service.
//!
//! Read-only and best-effort: no Magnetita on the bus means no devices, never an
//! error the user must act on. The `Changed` signal drives live refresh.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::mpsc;
use std::time::Duration;

use magnetita_core::MediaAction;
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

const SERVICE: &str = "org.celestina.Magnetita";
const OBJECT: &str = "/org/celestina/Devices1";
const INTERFACE: &str = "org.celestina.Devices1";

/// A device Magnetita reports as connected.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Symmetric short code for the active pairing exchange, empty otherwise.
    pub verification_key: String,
    /// The phone's now-playing, for the media card. `media_player` empty means
    /// nothing is playing; the `can_*` flags gate the transport buttons.
    pub media_player: String,
    pub media_title: String,
    pub media_artist: String,
    pub media_album: String,
    pub media_now_playing: String,
    pub media_playing: bool,
    pub media_can_pause: bool,
    pub media_can_play: bool,
    pub media_can_next: bool,
    pub media_can_previous: bool,
    pub media_can_seek: bool,
    pub media_length: i64,
    pub media_position: i64,
    pub media_artwork_url: String,
}

impl Default for Device {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            device_type: String::new(),
            connected: false,
            mounted: false,
            paired: false,
            mount_path: String::new(),
            battery: -1,
            charging: false,
            verification_key: String::new(),
            media_player: String::new(),
            media_title: String::new(),
            media_artist: String::new(),
            media_album: String::new(),
            media_now_playing: String::new(),
            media_playing: false,
            media_can_pause: false,
            media_can_play: false,
            media_can_next: false,
            media_can_previous: false,
            media_can_seek: false,
            media_length: -1,
            media_position: -1,
            media_artwork_url: String::new(),
        }
    }
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

/// One coherent Settings read. Either both daemon methods answered or this
/// snapshot is unavailable and the previous confirmed values stay untouched.
pub struct SettingsSnapshot {
    pub paired: Vec<Paired>,
    pub plugins: HashMap<String, bool>,
}

/// One connection-log entry from the daemon.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogEntry {
    pub device: String,
    pub message: String,
    pub failure: bool,
    pub time_ms: i64,
}

fn confirmed<T, E: Display>(context: &str, result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{context}: {error}"))
}

/// Lists the devices Magnetita reports. An empty confirmed list is distinct
/// from an unavailable bus or daemon.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let connection = confirmed("bus de sesión no disponible", Connection::session())?;
    let proxy = confirmed(
        "Magnetita no disponible",
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE),
    )?;
    let raw: Vec<HashMap<String, OwnedValue>> =
        confirmed("ListDevices falló", proxy.call("ListDevices", &()))?;
    Ok(raw.iter().map(parse_device).collect())
}

/// The recent confirmed connection log, oldest first.
pub fn recent_log() -> Result<Vec<LogEntry>, String> {
    let connection = confirmed("bus de sesión no disponible", Connection::session())?;
    let proxy = confirmed(
        "Magnetita no disponible",
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE),
    )?;
    let raw: Vec<HashMap<String, OwnedValue>> =
        confirmed("RecentLog falló", proxy.call("RecentLog", &()))?;
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
/// match rule is set up even if Magnetita is not up yet. A second bus watcher
/// reports service owner acquisition/loss, so stale device or log snapshots are
/// cleared when the daemon exits without getting a chance to emit a signal.
fn watch<F: Fn() + Send + 'static>(signal: &'static str, on_change: F) -> Result<(), String> {
    let connection =
        Connection::session().map_err(|error| format!("bus de sesión no disponible: {error}"))?;
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, INTERFACE)
        .map_err(|error| format!("Magnetita no disponible: {error}"))?;
    let signals = proxy
        .receive_signal(signal)
        .map_err(|error| format!("Magnetita: {error}"))?;

    let (tx, rx) = mpsc::channel::<()>();
    let signal_tx = tx.clone();
    std::thread::spawn(move || {
        for _ in signals {
            if signal_tx.send(()).is_err() {
                break;
            }
        }
    });

    let owner_tx = tx;
    std::thread::spawn(move || {
        let Ok(connection) = Connection::session() else {
            return;
        };
        let Ok(proxy) = DBusProxy::new(&connection) else {
            return;
        };
        let Ok(changes) = proxy.receive_name_owner_changed_with_args(&[(0, SERVICE)]) else {
            return;
        };
        for _ in changes {
            if owner_tx.send(()).is_err() {
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
pub fn request_pair(device_id: &str) -> Result<(), String> {
    call_method("RequestPair", device_id)
}

/// Ask Magnetita to drop the pairing (best-effort).
pub fn unpair(device_id: &str) -> Result<(), String> {
    call_method("Unpair", device_id)
}

/// Ask Magnetita to ring the device (find-my-phone).
pub fn ring(device_id: &str) -> Result<(), String> {
    call_method("Ring", device_id)
}

/// Ask Magnetita to drive the phone's media. The enum is converted to the
/// stable D-Bus string only at this external boundary.
pub fn media_action(device_id: &str, action: MediaAction) -> Result<(), String> {
    let connection = Connection::session().map_err(|error| error.to_string())?;
    let proxy =
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE).map_err(|error| error.to_string())?;
    proxy
        .call("MediaAction", &(device_id, action.as_str()))
        .map_err(|error| error.to_string())
}

/// Ask Magnetita to forget (unpair) a device — connected or not.
pub fn forget(device_id: &str) -> Result<(), String> {
    call_method("Forget", device_id)
}

/// Read the Settings surface without publishing one method's success beside
/// another method's failure as if both formed a confirmed snapshot.
pub fn settings_snapshot() -> Result<SettingsSnapshot, String> {
    let connection = confirmed("bus de sesión no disponible", Connection::session())?;
    let proxy = confirmed(
        "Magnetita no disponible",
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE),
    )?;
    let raw: Vec<HashMap<String, OwnedValue>> =
        confirmed("ListPaired falló", proxy.call("ListPaired", &()))?;
    let plugins = confirmed("PluginSettings falló", proxy.call("PluginSettings", &()))?;
    Ok(SettingsSnapshot {
        paired: raw.iter().map(parse_paired).collect(),
        plugins,
    })
}

/// Enable or disable a plugin (best-effort).
pub fn set_plugin(plugin: &str, enabled: bool) -> Result<(), String> {
    let connection = Connection::session().map_err(|error| error.to_string())?;
    let proxy =
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE).map_err(|error| error.to_string())?;
    proxy
        .call("SetPlugin", &(plugin, enabled))
        .map_err(|error| error.to_string())
}

fn call_method(method: &'static str, device_id: &str) -> Result<(), String> {
    let connection = Connection::session().map_err(|error| error.to_string())?;
    let proxy =
        Proxy::new(&connection, SERVICE, OBJECT, INTERFACE).map_err(|error| error.to_string())?;
    proxy
        .call(method, &(device_id,))
        .map_err(|error| error.to_string())
}

fn parse_device(dict: &HashMap<String, OwnedValue>) -> Device {
    let media_player = str_field(dict, "mediaPlayer");
    // `mediaCanPlay` was added to the stable dictionary after `mediaPlayer`.
    // Against an older daemon, preserve the former usable play/pause control
    // whenever a player is confirmed; an explicit false from a new daemon is
    // still authoritative.
    let media_can_play =
        optional_bool_field(dict, "mediaCanPlay").unwrap_or(!media_player.is_empty());
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
        verification_key: str_field(dict, "verificationKey"),
        media_player,
        media_title: str_field(dict, "mediaTitle"),
        media_artist: str_field(dict, "mediaArtist"),
        media_album: str_field(dict, "mediaAlbum"),
        media_now_playing: str_field(dict, "mediaNowPlaying"),
        media_playing: bool_field(dict, "mediaPlaying"),
        media_can_pause: bool_field(dict, "mediaCanPause"),
        media_can_play,
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
    optional_bool_field(dict, key).unwrap_or(false)
}

fn optional_bool_field(dict: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    dict.get(key)
        .and_then(|value| bool::try_from(value.clone()).ok())
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
    use super::{confirmed, parse_device};
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    #[test]
    fn an_unavailable_read_is_not_a_confirmed_empty_snapshot() {
        let failure = confirmed::<Vec<i32>, _>("ListDevices falló", Err("offline"));
        assert_eq!(failure.unwrap_err(), "ListDevices falló: offline");
        assert_eq!(
            confirmed("ListDevices", Ok::<_, &str>(Vec::<i32>::new())).unwrap(),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn a_new_media_snapshot_parses_artwork_and_progress() {
        let fields = [
            ("mediaAlbum", Value::from("Lista")),
            ("mediaCanSeek", Value::from(true)),
            ("mediaCanPlay", Value::from(true)),
            ("mediaCanPause", Value::from(true)),
            ("mediaNowPlaying", Value::from("Canal - Vídeo")),
            ("mediaLength", Value::from(120_000_i64)),
            ("mediaPosition", Value::from(30_000_i64)),
            ("verificationKey", Value::from("7C6FA008")),
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
        assert!(device.media_can_play);
        assert!(device.media_can_pause);
        assert_eq!(device.media_now_playing, "Canal - Vídeo");
        assert_eq!(device.media_length, 120_000);
        assert_eq!(device.media_position, 30_000);
        assert_eq!(device.media_artwork_url, "file:///run/user/1000/cover.img");
        assert_eq!(device.verification_key, "7C6FA008");
    }

    #[test]
    fn an_older_daemon_without_progress_keys_stays_compatible() {
        let fields = [("mediaPlayer", Value::from("Spotify"))];
        let dict = fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("basic test value converts"),
                )
            })
            .collect();
        let device = parse_device(&dict);
        assert!(device.media_can_play);
        assert_eq!(device.media_length, -1);
        assert_eq!(device.media_position, -1);
        assert!(device.media_artwork_url.is_empty());
        assert!(device.verification_key.is_empty());
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
