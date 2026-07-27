//! `org.celestina.Devices1` — the suite's first internal contract.
//!
//! Magnetita holds the phone; Siderita wants to draw it. Rather than reach into
//! Magnetita's private state, Siderita reads this: a session-bus interface that
//! lists the connected devices and, for each, the one thing a file manager needs
//! — the **mount path** — plus name, type and state. The filesystem carries the
//! bytes; this contract carries what a directory listing cannot.
//!
//! It is deliberately small and versioned by its name (`…Devices1`). Each device
//! is a `a{sv}` dict so a key can be added without breaking a consumer, and a
//! `Changed` signal tells consumers to re-read [`list_devices`](Devices::list_devices)
//! rather than have them poll. Battery is carried now (as `-1`, unknown) so the
//! shape is stable before CP3 fills it.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use magnetita_net::TrustStore;
use zbus::zvariant::{OwnedValue, Value};

use crate::lock::LockOk;
use crate::settings::Settings;

/// The bus name Magnetita owns.
pub const BUS_NAME: &str = "org.celestina.Magnetita";
/// The object path the device list lives at.
pub const OBJECT_PATH: &str = "/org/celestina/Devices1";
/// The interface name.
pub const INTERFACE: &str = "org.celestina.Devices1";
/// The signal emitted when the device set or any device's state changes.
pub const CHANGED_SIGNAL: &str = "Changed";
/// The signal emitted when a new connection-log entry is recorded.
pub const EVENT_SIGNAL: &str = "Event";
/// How many recent log entries to keep for the app to read on open.
const LOG_CAPACITY: usize = 200;

/// One connected device, as the contract exposes it.
#[derive(Clone, Debug)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    /// "phone", "tablet", "laptop", "desktop", "tv", or "unknown".
    pub device_type: String,
    pub connected: bool,
    pub mounted: bool,
    /// Whether we trust this device (paired). A connected-but-unpaired device is
    /// waiting on a pairing.
    pub paired: bool,
    /// The local path the device is mounted at, or empty when not mounted.
    pub mount_path: String,
    /// Battery percent, or -1 when unknown.
    pub battery: i32,
    /// Whether the phone is charging.
    pub charging: bool,
    /// The peer certificate's SHA-256 fingerprint — the verification key a human
    /// compares to be sure of no impostor.
    pub fingerprint: String,
    /// The phone's currently-reported media, for the app's now-playing card.
    /// `media_player` is empty when nothing is playing; the rest is that
    /// player's state, and the `can_*` flags gate the transport buttons.
    pub media_player: String,
    pub media_title: String,
    pub media_artist: String,
    pub media_playing: bool,
    pub media_can_pause: bool,
    pub media_can_next: bool,
    pub media_can_previous: bool,
}

impl DeviceEntry {
    /// The device as a `a{sv}` dict for the wire.
    fn to_dict(&self) -> HashMap<String, OwnedValue> {
        let fields = [
            ("id", Value::from(self.id.clone())),
            ("name", Value::from(self.name.clone())),
            ("type", Value::from(self.device_type.clone())),
            ("connected", Value::from(self.connected)),
            ("mounted", Value::from(self.mounted)),
            ("paired", Value::from(self.paired)),
            ("mountPath", Value::from(self.mount_path.clone())),
            ("battery", Value::from(self.battery)),
            ("charging", Value::from(self.charging)),
            ("fingerprint", Value::from(self.fingerprint.clone())),
            ("mediaPlayer", Value::from(self.media_player.clone())),
            ("mediaTitle", Value::from(self.media_title.clone())),
            ("mediaArtist", Value::from(self.media_artist.clone())),
            ("mediaPlaying", Value::from(self.media_playing)),
            ("mediaCanPause", Value::from(self.media_can_pause)),
            ("mediaCanNext", Value::from(self.media_can_next)),
            ("mediaCanPrevious", Value::from(self.media_can_previous)),
        ];
        fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("a basic value always converts"),
                )
            })
            .collect()
    }
}

/// The connected devices, keyed by id and shared between the daemon (which
/// writes) and the served interface (which reads).
pub type Registry = Arc<Mutex<BTreeMap<String, DeviceEntry>>>;

/// One connection-log entry — the app's answer to "why won't it connect".
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub device: String,
    /// The line in the user's words (Spanish, this being a personal suite).
    pub message: String,
    /// A failure worth surfacing in red, versus an informational milestone.
    pub failure: bool,
    /// Millisecond wall-clock stamp for ordering.
    pub time_ms: i64,
}

impl LogEntry {
    fn to_dict(&self) -> HashMap<String, OwnedValue> {
        let fields = [
            ("device", Value::from(self.device.clone())),
            ("message", Value::from(self.message.clone())),
            ("failure", Value::from(self.failure)),
            ("time", Value::from(self.time_ms)),
        ];
        fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("a basic value always converts"),
                )
            })
            .collect()
    }
}

/// The recent connection log, oldest first, capped so it never grows unbounded.
pub type Log = Arc<Mutex<VecDeque<LogEntry>>>;

/// Appends an entry, dropping the oldest past [`LOG_CAPACITY`].
pub fn push_log(log: &Log, entry: LogEntry) {
    let mut log = log.lock_ok();
    log.push_back(entry);
    while log.len() > LOG_CAPACITY {
        log.pop_front();
    }
}

/// A control action the app asks of a live link — delivered to that link's own
/// thread, which owns the [`Device`](magnetita_core::Session) and can act on it.
#[derive(Clone, Debug)]
pub enum Command {
    /// Ask the device to pair.
    RequestPair,
    /// Drop the pairing.
    Unpair,
    /// Ring the device (find-my-phone).
    Ring,
    /// Push this text to the device as a clipboard change.
    SendClipboard(String),
    /// Send this local file to the device.
    SendFile(String),
    /// Send a media transport verb ("PlayPause", "Next", "Previous") to the
    /// phone's active player.
    Media(String),
}

/// The per-device command channels, keyed by device id. A link registers its
/// sender while it runs; the served interface looks one up to forward a request.
pub type Commands = Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<Command>>>>;

/// The object served at [`OBJECT_PATH`].
pub struct Devices {
    registry: Registry,
    log: Log,
    commands: Commands,
    /// The pinned peers — for listing and forgetting from the Settings surface,
    /// including devices that are not currently connected.
    trust: Arc<Mutex<TrustStore>>,
    /// The per-plugin toggles the app reads and writes.
    settings: Arc<Mutex<Settings>>,
    /// Where those toggles persist.
    settings_path: PathBuf,
}

impl Devices {
    pub fn new(
        registry: Registry,
        log: Log,
        commands: Commands,
        trust: Arc<Mutex<TrustStore>>,
        settings: Arc<Mutex<Settings>>,
        settings_path: PathBuf,
    ) -> Devices {
        Devices {
            registry,
            log,
            commands,
            trust,
            settings,
            settings_path,
        }
    }

    /// Forwards a command to the device's link thread, if it is connected.
    fn forward(&self, device_id: &str, command: Command) {
        if let Some(sender) = self.commands.lock_ok().get(device_id) {
            let _ = sender.send(command);
        }
    }

    /// Whether a device id is currently connected (has a live entry).
    fn is_connected(&self, device_id: &str) -> bool {
        self.registry
            .lock_ok()
            .get(device_id)
            .map(|entry| entry.connected)
            .unwrap_or(false)
    }
}

#[zbus::interface(name = "org.celestina.Devices1")]
impl Devices {
    /// The connected devices, each a dict with keys `id`, `name`, `type`,
    /// `connected`, `mounted`, `paired`, `mountPath`, `battery`, `fingerprint`.
    fn list_devices(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.registry
            .lock_ok()
            .values()
            .map(DeviceEntry::to_dict)
            .collect()
    }

    /// The recent connection log, oldest first — each a dict `device`, `message`,
    /// `failure`, `time`. Read on open; the `Event` signal marks new entries.
    fn recent_log(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.log.lock_ok().iter().map(LogEntry::to_dict).collect()
    }

    /// Ask the connected device to pair (the app's "Emparejar").
    fn request_pair(&self, device_id: String) {
        self.forward(&device_id, Command::RequestPair);
    }

    /// Drop the pairing with the connected device (the app's "Desvincular").
    fn unpair(&self, device_id: String) {
        self.forward(&device_id, Command::Unpair);
    }

    /// Ring the connected device (the app's "Sonar" — find-my-phone).
    fn ring(&self, device_id: String) {
        self.forward(&device_id, Command::Ring);
    }

    /// Send a local file to the connected device (Siderita's "Enviar al móvil").
    fn send_file(&self, device_id: String, path: String) {
        self.forward(&device_id, Command::SendFile(path));
    }

    /// Drive the phone's media: `action` is "PlayPause", "Next" or "Previous"
    /// (the app's transport buttons on its now-playing card).
    fn media_action(&self, device_id: String, action: String) {
        self.forward(&device_id, Command::Media(action));
    }

    /// The paired devices, each a dict `id`, `name`, `fingerprint`, `connected`
    /// — including devices remembered but not currently online. The Settings
    /// surface lists these so a pairing can be dropped even when the phone is off.
    fn list_paired(&self) -> Vec<HashMap<String, OwnedValue>> {
        let peers: Vec<_> = self.trust.lock_ok().peers().collect();
        peers
            .into_iter()
            .map(|peer| {
                let connected = self.is_connected(&peer.device_id);
                let fields = [
                    ("id", Value::from(peer.device_id)),
                    ("name", Value::from(peer.device_name)),
                    ("fingerprint", Value::from(peer.fingerprint)),
                    ("connected", Value::from(connected)),
                ];
                fields
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            key.to_owned(),
                            OwnedValue::try_from(value).expect("a basic value always converts"),
                        )
                    })
                    .collect()
            })
            .collect()
    }

    /// Forget a pairing. A connected device is unpaired over its live link (so
    /// the phone learns too, and the link thread drops the pin); an offline one
    /// simply has its pin removed here.
    fn forget(&self, device_id: String) {
        if self.is_connected(&device_id) {
            self.forward(&device_id, Command::Unpair);
        } else {
            let _ = self.trust.lock_ok().forget(&device_id);
        }
    }

    /// The per-plugin toggles, as a `name → enabled` dict (the app's switches).
    fn plugin_settings(&self) -> HashMap<String, bool> {
        self.settings
            .lock_ok()
            .entries()
            .iter()
            .map(|(name, enabled)| ((*name).to_owned(), *enabled))
            .collect()
    }

    /// Enable or disable a plugin and persist. An unknown name is ignored.
    fn set_plugin(&self, plugin: String, enabled: bool) {
        let mut settings = self.settings.lock_ok();
        if settings.set(&plugin, enabled) {
            let _ = settings.save(&self.settings_path);
        }
    }
}
