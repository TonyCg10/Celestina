//! `magnetitad`: the trusted channel to the phone.
//!
//! One wire, the suite's own (ADR 0001): a QUIC link with pinned
//! certificates that the phone's application dials, paired by QR. The daemon
//! keeps the device identity and certificate, the trust store of pinned
//! phones, the registry `org.celestina.Devices1` publishes, the plugin
//! settings, and the mirror (`org.celestina.Mirror1`). Every capability
//! (battery, clipboard, notifications, share, media, commands, input, mirror,
//! contacts, SMS, telephony, storage) runs in `link_wire`.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use celestina_core::xdg;
use magnetita_net::{DeviceCert, PayloadLimiter, TrustStore};

mod artwork;
mod clipboard;
mod device_identity;
mod devices;
mod incoming_file;
mod link_wire;
mod lock;
mod media;
mod mirror;
mod mirror_discovery;
mod mount;
mod notify;
mod revocation;
mod runtime;
mod session_registration;
mod settings;
mod subprocess;
use devices::{push_log, Commands, Devices, Log, LogEntry, Registry};
use lock::LockOk;
use revocation::Revocations;
use runtime::{log, millis};
use settings::Settings;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            log("error", &e.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

/// The shared state every session reads: the trust store, the settings, the
/// registry the contract exposes, and the session-bus connection used to
/// announce changes.
struct Daemon {
    /// Shared with the served `Devices1` object so its Settings surface can list
    /// and forget paired peers, not only the sessions that pin them.
    trust: Arc<Mutex<TrustStore>>,
    /// The per-plugin toggles, shared the same way: the app writes them (through
    /// the served object, which owns persistence), the sessions read them to
    /// gate each plugin's behaviour.
    settings: Arc<Mutex<Settings>>,
    devices: Registry,
    log: Log,
    commands: Commands,
    pending_clipboards: clipboard::PendingClipboards,
    revocations: Arc<Revocations>,
    payloads: PayloadLimiter,
    dbus: Option<zbus::blocking::Connection>,
    /// The bounded phone-id to server-id map behind notification replace and
    /// withdraw, owned by the module that posts them.
    notifications: notify::Mirror,
    /// The last clipboard value we synced (sent or received), so our own
    /// wl-copy of a received clipboard is not echoed back and no loop forms.
    last_clipboard: Mutex<String>,
}

fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_home()
        .ok_or("no XDG config home to store the device identity")?
        .join("magnetita");
    fs::create_dir_all(&dir)?;
    if let Err(error) = artwork::sweep() {
        log("artwork", &format!("cache unavailable: {error}"));
    }

    let device_id = device_identity::ensure(&dir)?;
    let cert = DeviceCert::ensure(&dir, &device_id)?;

    // The trust store and plugin settings are shared with the served interface,
    // so the app can list/forget paired peers and toggle plugins, not just the
    // sessions. Built before serving so both sides hold the same handle.
    let trust = Arc::new(Mutex::new(TrustStore::load(&dir.join("trust.json"))?));
    let settings_path = dir.join("settings.json");
    let settings = Arc::new(Mutex::new(Settings::load(&settings_path)));

    // Serve org.celestina.Devices1 (best-effort: no session bus just means
    // Siderita cannot draw the phone, not that the link fails).
    let registry: Registry = Arc::new(Mutex::new(BTreeMap::new()));
    let event_log: Log = Arc::new(Mutex::new(VecDeque::new()));
    let commands: Commands = Arc::new(Mutex::new(HashMap::new()));
    let revocations = Arc::new(Revocations::new());
    // Owned here so it is joined at exit: a scrcpy this daemon started must never outlive it.
    let (mirror_handle, _mirror_worker) =
        mirror::start(dir.join("mirror.json"), dir.join("mirror-endpoint"));
    let pairing = link_wire::PairingArm::default();
    let dbus = match devices::serve(
        Devices::new(
            Arc::clone(&registry),
            Arc::clone(&event_log),
            Arc::clone(&commands),
            Arc::clone(&trust),
            Arc::clone(&revocations),
            Arc::clone(&settings),
            settings_path,
        )
        .with_own_pairing(link_wire::own_pairing(&pairing, &device_id, &cert)?),
        mirror_handle,
    ) {
        Ok(connection) => {
            log(
                "dbus",
                &format!("serving {} and {}", devices::INTERFACE, mirror::INTERFACE),
            );
            Some(connection)
        }
        Err(e) => {
            log("dbus", &format!("unavailable: {e}"));
            None
        }
    };

    let daemon = Arc::new(Daemon {
        trust,
        settings,
        devices: registry,
        log: event_log,
        commands,
        pending_clipboards: clipboard::PendingClipboards::default(),
        revocations,
        payloads: PayloadLimiter::new(),
        dbus,
        notifications: notify::Mirror::default(),
        last_clipboard: Mutex::new(String::new()),
    });

    // Watch the desktop clipboard and push changes to connected phones.
    let clipboard_daemon = Arc::clone(&daemon);
    clipboard::spawn_watch(move |text| clipboard_daemon.push_clipboard(text));

    log("id", &device_id);
    log("cert", &cert.fingerprint()?);

    // A previous run killed mid-session can leave a dead mount behind; clear it.
    mount::clear_stale();

    // The own wire, on its single runtime thread; it holds every session.
    let _own_wire = link_wire::install(Arc::clone(&daemon), cert.clone(), device_id, pairing);

    log("ready", "listening for the phone on the own wire");
    loop {
        thread::park();
    }
}

impl Daemon {
    /// A desktop clipboard change: push it to every connected device, unless it
    /// is the value we just received from a phone (our own wl-copy echo), which
    /// would otherwise loop back and forth forever.
    fn push_clipboard(&self, text: String) {
        if !magnetita_core::clipboard::is_syncable(&text) || !self.settings.lock_ok().clipboard {
            return;
        }
        {
            let mut last = self.last_clipboard.lock_ok();
            if *last == text {
                return;
            }
            *last = text.clone();
        }
        let device_ids: Vec<_> = self.commands.lock_ok().keys().cloned().collect();
        self.pending_clipboards.replace_for(device_ids, text);
    }

    /// Reflect a device's battery report into the registry.
    fn set_battery(&self, device_id: &str, charge: i32, charging: bool) {
        if let Some(entry) = self.devices.lock_ok().get_mut(device_id) {
            entry.battery = charge;
            entry.charging = charging;
        }
    }

    /// Reflect a device's mount state into the registry.
    fn set_mount(&self, device_id: &str, path: Option<&Path>) {
        if let Some(entry) = self.devices.lock_ok().get_mut(device_id) {
            match path {
                Some(path) => {
                    entry.mounted = true;
                    entry.mount_path = path.to_string_lossy().into_owned();
                }
                None => {
                    entry.mounted = false;
                    entry.mount_path.clear();
                }
            }
        }
    }

    /// Tell consumers of the contract that the device set or a device's state
    /// changed, so they re-read it. Best-effort; a broken bus is not fatal.
    fn notify_change(&self) {
        if let Some(connection) = &self.dbus {
            let _ = connection.emit_signal(
                Option::<&str>::None,
                devices::OBJECT_PATH,
                devices::INTERFACE,
                devices::CHANGED_SIGNAL,
                &(),
            );
        }
    }
}

/// Record a connection-log line for the app, and signal that a new entry landed.
/// Best-effort on the bus; the entry is kept regardless so the app sees it on
/// its next read.
fn ui_log(daemon: &Daemon, device: &str, message: &str, failure: bool) {
    push_log(
        &daemon.log,
        LogEntry {
            device: device.to_owned(),
            message: message.to_owned(),
            failure,
            time_ms: millis(),
        },
    );
    if let Some(connection) = &daemon.dbus {
        let _ = connection.emit_signal(
            Option::<&str>::None,
            devices::OBJECT_PATH,
            devices::INTERFACE,
            devices::EVENT_SIGNAL,
            &(),
        );
    }
}
