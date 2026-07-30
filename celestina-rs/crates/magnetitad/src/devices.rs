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
use std::time::Duration;

use celestina_core::Generation;
use magnetita_core::{MediaAction, PlayerState};
use magnetita_net::TrustStore;
use zbus::zvariant::{OwnedValue, Value};

use crate::lock::LockOk;
use crate::revocation::{RequestError, Revocations};
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
/// Local action bursts are bounded per live device; revocation has its own
/// tombstone path and can never be trapped behind this queue.
const COMMAND_QUEUE_CAPACITY: usize = 32;
/// `Device::pump` wakes once per second, so two seconds covers one in-flight
/// read plus command processing without letting a D-Bus call hang forever.
const FORGET_ACK_TIMEOUT: Duration = Duration::from_secs(2);

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
    /// Daemon-private authorization epoch. Payloads may publish only into the
    /// exact pairing generation which started them.
    pub(crate) pair_generation: Generation,
    /// The local path the device is mounted at, or empty when not mounted.
    pub mount_path: String,
    /// Battery percent, or -1 when unknown.
    pub battery: i32,
    /// Whether the phone is charging.
    pub charging: bool,
    /// Stable SHA-256 certificate fingerprint pinned by the trust store.
    pub fingerprint: String,
    /// Short symmetric code for the pairing completed on this live link. Empty
    /// on a restored session because there is no fresh pairing timestamp.
    pub verification_key: String,
    /// The phone's currently-reported media, for the app's now-playing card.
    /// `media_player` is empty when nothing is playing; the rest is that
    /// player's state, and the `can_*` flags gate the transport buttons.
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
    /// Percent-encoded local file URL exposed to QML, empty until a requested
    /// cover has been received and verified.
    pub media_artwork_url: String,
    /// Peer-local art identifier and generated local path stay daemon-private.
    pub(crate) media_artwork_source: String,
    pub(crate) media_artwork_path: PathBuf,
}

impl DeviceEntry {
    pub fn connected(id: String, name: String, device_type: String, fingerprint: String) -> Self {
        Self {
            id,
            name,
            device_type,
            connected: true,
            mounted: false,
            paired: false,
            pair_generation: Generation::INITIAL,
            mount_path: String::new(),
            battery: -1,
            charging: false,
            fingerprint,
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
            media_artwork_source: String::new(),
            media_artwork_path: PathBuf::new(),
        }
    }

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
            (
                "verificationKey",
                Value::from(self.verification_key.clone()),
            ),
            ("mediaPlayer", Value::from(self.media_player.clone())),
            ("mediaTitle", Value::from(self.media_title.clone())),
            ("mediaArtist", Value::from(self.media_artist.clone())),
            ("mediaAlbum", Value::from(self.media_album.clone())),
            (
                "mediaNowPlaying",
                Value::from(self.media_now_playing.clone()),
            ),
            ("mediaPlaying", Value::from(self.media_playing)),
            ("mediaCanPause", Value::from(self.media_can_pause)),
            ("mediaCanPlay", Value::from(self.media_can_play)),
            ("mediaCanNext", Value::from(self.media_can_next)),
            ("mediaCanPrevious", Value::from(self.media_can_previous)),
            ("mediaCanSeek", Value::from(self.media_can_seek)),
            ("mediaLength", Value::from(self.media_length)),
            ("mediaPosition", Value::from(self.media_position)),
            (
                "mediaArtworkUrl",
                Value::from(self.media_artwork_url.clone()),
            ),
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

/// Replace the confirmed media snapshot. A changed cover identifier clears the
/// previous local URL immediately and returns its generated path for cleanup.
pub fn set_media(
    registry: &Registry,
    device_id: &str,
    state: Option<&PlayerState>,
) -> Option<PathBuf> {
    let mut registry = registry.lock_ok();
    let entry = registry.get_mut(device_id)?;
    let (next_player, next_artwork_source) = state
        .map(|state| (state.player.as_str(), state.album_art_url.as_str()))
        .unwrap_or_default();
    let stale_artwork =
        if entry.media_player != next_player || entry.media_artwork_source != next_artwork_source {
            entry.media_artwork_source = next_artwork_source.to_owned();
            entry.media_artwork_url.clear();
            (!entry.media_artwork_path.as_os_str().is_empty())
                .then(|| std::mem::take(&mut entry.media_artwork_path))
        } else {
            None
        };

    match state {
        Some(state) => {
            entry.media_player = state.player.clone();
            entry.media_title = state.title.clone();
            entry.media_artist = state.artist.clone();
            entry.media_album = state.album.clone();
            entry.media_now_playing = state.now_playing.clone();
            entry.media_playing = state.is_playing;
            entry.media_can_pause = state.can_pause;
            entry.media_can_play = state.can_play;
            entry.media_can_next = state.can_go_next;
            entry.media_can_previous = state.can_go_previous;
            entry.media_can_seek = state.can_seek;
            entry.media_length = state.length;
            entry.media_position = state.pos;
        }
        None => {
            entry.media_player.clear();
            entry.media_title.clear();
            entry.media_artist.clear();
            entry.media_album.clear();
            entry.media_now_playing.clear();
            entry.media_playing = false;
            entry.media_can_pause = false;
            entry.media_can_play = false;
            entry.media_can_next = false;
            entry.media_can_previous = false;
            entry.media_can_seek = false;
            entry.media_length = -1;
            entry.media_position = -1;
        }
    }
    stale_artwork
}

/// Publish a verified local cover only if the device is still showing the
/// player/source pair that requested it. Returns `None` for a stale transfer;
/// `Some(previous)` means the URL was installed and the old path may be deleted.
#[cfg(test)]
fn install_artwork(
    registry: &Registry,
    device_id: &str,
    player: &str,
    source_url: &str,
    local_path: PathBuf,
    local_url: String,
) -> Option<Option<PathBuf>> {
    let mut registry = registry.lock_ok();
    let entry = registry.get_mut(device_id)?;
    install_artwork_entry(entry, player, source_url, local_path, local_url)
}

/// Variant used while a caller already owns the registry entry, so publication
/// can be serialized with another state boundary without locking recursively.
pub(crate) fn install_artwork_entry(
    entry: &mut DeviceEntry,
    player: &str,
    source_url: &str,
    local_path: PathBuf,
    local_url: String,
) -> Option<Option<PathBuf>> {
    if entry.media_player != player || entry.media_artwork_source != source_url {
        return None;
    }
    let previous = std::mem::replace(&mut entry.media_artwork_path, local_path);
    let previous = (!previous.as_os_str().is_empty()).then_some(previous);
    entry.media_artwork_url = local_url;
    Some(previous)
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

/// Publish or clear the short code for the current live pairing exchange.
pub fn set_verification_key(registry: &Registry, device_id: &str, key: &str) -> bool {
    let mut registry = registry.lock_ok();
    let Some(entry) = registry.get_mut(device_id) else {
        return false;
    };
    if entry.verification_key == key {
        return false;
    }
    entry.verification_key = key.to_owned();
    true
}

/// A control action the app asks of a live link — delivered to that link's own
/// thread, which owns the [`Device`](magnetita_core::Session) and can act on it.
#[derive(Clone, Debug)]
pub enum Command {
    /// Ask the device to pair, carrying the revocation ordering point observed
    /// before this command entered the bounded queue.
    RequestPair { observed: Generation },
    /// Ring the device (find-my-phone).
    Ring,
    /// Send this local file to the device.
    SendFile(String),
    /// Send a media transport verb ("PlayPause", "Next", "Previous") to the
    /// phone's active player.
    Media(MediaAction),
}

/// The per-device command channels, keyed by device id. A link registers its
/// sender while it runs; the served interface looks one up to forward a request.
pub type Commands = Arc<Mutex<HashMap<String, std::sync::mpsc::SyncSender<Command>>>>;

pub fn command_channel() -> (
    std::sync::mpsc::SyncSender<Command>,
    std::sync::mpsc::Receiver<Command>,
) {
    std::sync::mpsc::sync_channel(COMMAND_QUEUE_CAPACITY)
}

/// The object served at [`OBJECT_PATH`].
pub struct Devices {
    registry: Registry,
    log: Log,
    commands: Commands,
    /// The pinned peers — for listing and forgetting from the Settings surface,
    /// including devices that are not currently connected.
    trust: Arc<Mutex<TrustStore>>,
    /// Pairing revocations are tombstones, not ordinary queued commands: they
    /// must win over an already-read `Paired` event.
    revocations: Arc<Revocations>,
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
        revocations: Arc<Revocations>,
        settings: Arc<Mutex<Settings>>,
        settings_path: PathBuf,
    ) -> Devices {
        Devices {
            registry,
            log,
            commands,
            trust,
            revocations,
            settings,
            settings_path,
        }
    }

    /// Forwards a command to the device's link thread, if it is connected.
    fn forward(&self, device_id: &str, command: Command) -> zbus::fdo::Result<()> {
        let sender = self
            .commands
            .lock_ok()
            .get(device_id)
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::Failed("el dispositivo no está conectado".into()))?;
        sender.try_send(command).map_err(|error| {
            zbus::fdo::Error::Failed(match error {
                std::sync::mpsc::TrySendError::Full(_) => {
                    "la cola del dispositivo está ocupada; inténtalo de nuevo".to_owned()
                }
                std::sync::mpsc::TrySendError::Disconnected(_) => {
                    "el enlace se cerró antes de recibir la acción".to_owned()
                }
            })
        })
    }

    /// Whether a device id is currently connected (has a live entry).
    fn is_connected(&self, device_id: &str) -> bool {
        self.registry
            .lock_ok()
            .get(device_id)
            .map(|entry| entry.connected)
            .unwrap_or(false)
    }

    /// Persist a local trust revocation, then wait until any live session has
    /// applied the same generation. The peer notification is best-effort; the
    /// durable local boundary does not depend on a healthy socket.
    fn revoke(&self, device_id: &str) -> zbus::fdo::Result<()> {
        let generation = self
            .revocations
            .request_if_and_apply(
                device_id,
                || self.is_connected(device_id) || self.trust.lock_ok().is_trusted(device_id),
                || self.trust.lock_ok().forget(device_id),
            )
            .map_err(|error| match error {
                RequestError::Generation(error) => {
                    zbus::fdo::Error::Failed(format!("no se pudo registrar la revocación: {error}"))
                }
                RequestError::Apply(error) => zbus::fdo::Error::IOError(error.to_string()),
            })?;
        let Some(generation) = generation else {
            return Ok(());
        };
        if !self.is_connected(device_id) {
            self.revocations
                .clear_if(device_id, || !self.is_connected(device_id));
            return Ok(());
        }
        if self
            .revocations
            .wait_applied(device_id, generation, FORGET_ACK_TIMEOUT)
        {
            return Ok(());
        }
        if !self.is_connected(device_id) {
            self.revocations
                .clear_if(device_id, || !self.is_connected(device_id));
            return Ok(());
        }
        Err(zbus::fdo::Error::Failed(
            "el enlace no aplicó Desvincular dentro del límite".to_owned(),
        ))
    }
}

#[zbus::interface(name = "org.celestina.Devices1")]
impl Devices {
    /// The connected devices, each a dict with keys `id`, `name`, `type`,
    /// `connected`, `mounted`, `paired`, `mountPath`, `battery`, `fingerprint`,
    /// `verificationKey`.
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
    fn request_pair(&self, device_id: String) -> zbus::fdo::Result<()> {
        let observed = self.revocations.observe_pair();
        self.forward(&device_id, Command::RequestPair { observed })
    }

    /// Drop the pairing with the connected device (the app's "Desvincular").
    fn unpair(&self, device_id: String) -> zbus::fdo::Result<()> {
        self.revoke(&device_id)
    }

    /// Ring the connected device (the app's "Sonar" — find-my-phone).
    fn ring(&self, device_id: String) -> zbus::fdo::Result<()> {
        self.forward(&device_id, Command::Ring)
    }

    /// Send a local file to the connected device (Siderita's "Enviar al móvil").
    fn send_file(&self, device_id: String, path: String) -> zbus::fdo::Result<()> {
        self.forward(&device_id, Command::SendFile(path))
    }

    /// Drive the phone's media: `action` is "PlayPause", "Next" or "Previous"
    /// (the app's transport buttons on its now-playing card).
    fn media_action(&self, device_id: String, action: String) -> zbus::fdo::Result<()> {
        let action = MediaAction::parse(&action)
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("acción multimedia desconocida".into()))?;
        self.forward(&device_id, Command::Media(action))
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

    /// Forget durably and wait for any live session to cross the same barrier.
    fn forget(&self, device_id: String) -> zbus::fdo::Result<()> {
        self.revoke(&device_id)
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
    fn set_plugin(&self, plugin: String, enabled: bool) -> zbus::fdo::Result<()> {
        let mut settings = self.settings.lock_ok();
        match settings.update(&self.settings_path, &plugin, enabled) {
            Ok(true) => Ok(()),
            Ok(false) => Err(zbus::fdo::Error::InvalidArgs(format!(
                "unknown plugin {plugin}"
            ))),
            Err(error) => Err(zbus::fdo::Error::IOError(error.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_channel, install_artwork, set_media, set_verification_key, Command, DeviceEntry,
        Registry, COMMAND_QUEUE_CAPACITY,
    };
    use crate::lock::LockOk;
    use magnetita_core::PlayerState;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn registry() -> Registry {
        let entry = DeviceEntry::connected(
            "phone".to_owned(),
            "Galaxy".to_owned(),
            "phone".to_owned(),
            "fingerprint".to_owned(),
        );
        Arc::new(Mutex::new(BTreeMap::from([("phone".to_owned(), entry)])))
    }

    fn state(artwork: &str) -> PlayerState {
        PlayerState {
            player: "YouTube".to_owned(),
            title: "Video".to_owned(),
            artist: "Canal".to_owned(),
            album: "Lista".to_owned(),
            album_art_url: artwork.to_owned(),
            is_playing: true,
            can_pause: true,
            can_play: true,
            can_seek: true,
            length: 120_000,
            pos: 30_000,
            ..PlayerState::default()
        }
    }

    #[test]
    fn media_snapshot_exposes_progress_and_backward_compatible_dict_keys() {
        let registry = registry();
        assert!(set_media(&registry, "phone", Some(&state("cover-a"))).is_none());
        let guard = registry.lock_ok();
        let entry = guard.get("phone").expect("fixture device exists");
        assert_eq!(entry.media_album, "Lista");
        assert_eq!(entry.media_length, 120_000);
        assert_eq!(entry.media_position, 30_000);
        assert!(entry.media_can_seek);
        assert!(entry.media_can_play);
        let dict = entry.to_dict();
        for key in [
            "mediaPlayer",
            "mediaArtworkUrl",
            "mediaLength",
            "mediaPosition",
            "mediaCanSeek",
            "mediaCanPlay",
            "mediaNowPlaying",
            "verificationKey",
        ] {
            assert!(dict.contains_key(key), "missing Devices1 key {key}");
        }
    }

    #[test]
    fn pairing_code_is_published_and_cleared_without_touching_the_fingerprint() {
        let registry = registry();
        assert!(set_verification_key(&registry, "phone", "7C6FA008"));
        {
            let guard = registry.lock_ok();
            let entry = guard.get("phone").unwrap();
            assert_eq!(entry.verification_key, "7C6FA008");
            assert_eq!(entry.fingerprint, "fingerprint");
        }
        assert!(set_verification_key(&registry, "phone", ""));
        assert!(!set_verification_key(&registry, "phone", ""));
    }

    #[test]
    fn a_device_command_burst_has_a_hard_queue_bound() {
        let (commands, _receiver) = command_channel();
        for _ in 0..COMMAND_QUEUE_CAPACITY {
            commands.try_send(Command::Ring).unwrap();
        }
        assert!(matches!(
            commands.try_send(Command::Ring),
            Err(std::sync::mpsc::TrySendError::Full(_))
        ));
    }

    #[test]
    fn artwork_is_installed_only_for_the_current_player_and_source() {
        let registry = registry();
        set_media(&registry, "phone", Some(&state("cover-a")));
        assert_eq!(
            install_artwork(
                &registry,
                "phone",
                "YouTube",
                "cover-a",
                PathBuf::from("/run/user/1000/a.img"),
                "file:///run/user/1000/a.img".to_owned(),
            ),
            Some(None)
        );
        assert!(install_artwork(
            &registry,
            "phone",
            "YouTube",
            "old-cover",
            PathBuf::from("/run/user/1000/old.img"),
            "file:///run/user/1000/old.img".to_owned(),
        )
        .is_none());

        let stale = set_media(&registry, "phone", Some(&state("cover-b")));
        assert_eq!(stale, Some(PathBuf::from("/run/user/1000/a.img")));
        let guard = registry.lock_ok();
        let entry = guard.get("phone").expect("fixture device exists");
        assert!(entry.media_artwork_url.is_empty());
    }

    #[test]
    fn changing_player_clears_artwork_even_when_the_source_string_is_reused() {
        let registry = registry();
        set_media(&registry, "phone", Some(&state("shared-cover")));
        install_artwork(
            &registry,
            "phone",
            "YouTube",
            "shared-cover",
            PathBuf::from("/run/user/1000/shared.img"),
            "file:///run/user/1000/shared.img".to_owned(),
        )
        .expect("current artwork installs");

        let mut next = state("shared-cover");
        next.player = "Spotify".to_owned();
        assert_eq!(
            set_media(&registry, "phone", Some(&next)),
            Some(PathBuf::from("/run/user/1000/shared.img"))
        );
        let guard = registry.lock_ok();
        assert!(guard
            .get("phone")
            .expect("fixture device exists")
            .media_artwork_url
            .is_empty());
    }
}
