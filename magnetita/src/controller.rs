//! The app's one QObject: a live view of the devices Magnetita reports.
//!
//! It reads [`crate::devices`] and publishes the result as parallel lists QML
//! iterates, refreshing itself off the `Changed` signal — the same shape as
//! Siderita's volume list. The app is a client; this holds no device state of
//! its own beyond the last snapshot.

use core::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    // snake_case Rust names surface to QML in camelCase (deviceNames, reload…).
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QStringList, device_names)]
        #[qproperty(QStringList, device_types)]
        #[qproperty(QStringList, device_mounts)]
        #[qproperty(QStringList, device_states)]
        #[qproperty(QStringList, device_fingerprints)]
        // Formatted battery per device ("58 %", "58 % ⚡", or "" if unknown).
        #[qproperty(QStringList, device_battery)]
        // Per-device charging flag ("true"/"false"), parallel to battery.
        #[qproperty(QStringList, device_charging)]
        // Per-device pairing flag ("true"/"false"), parallel to the lists above.
        #[qproperty(QStringList, device_paired)]
        // The phone's now-playing line ("Artista — Título"), "" when nothing is
        // playing (which hides the media card), and parallel "true"/"false"
        // flags for the play/pause state and whether next/prev are available.
        #[qproperty(QStringList, device_media)]
        #[qproperty(QStringList, device_media_titles)]
        #[qproperty(QStringList, device_media_artists)]
        #[qproperty(QStringList, device_media_albums)]
        #[qproperty(QStringList, device_media_artwork)]
        #[qproperty(QStringList, device_media_lengths)]
        #[qproperty(QStringList, device_media_positions)]
        #[qproperty(QStringList, device_media_playing)]
        #[qproperty(QStringList, device_media_next)]
        #[qproperty(QStringList, device_media_previous)]
        #[qproperty(QStringList, device_media_seek)]
        // The connection log — newest first — with a parallel failure flag
        // ("true"/"false") for red styling.
        #[qproperty(QStringList, log_lines)]
        #[qproperty(QStringList, log_failures)]
        // The Settings surface: paired devices (name + fingerprint + a
        // "true"/"false" connected flag) and the per-plugin toggles (label +
        // "true"/"false" enabled flag), all parallel lists.
        #[qproperty(QStringList, paired_names)]
        #[qproperty(QStringList, paired_fingerprints)]
        #[qproperty(QStringList, paired_connected)]
        #[qproperty(QStringList, plugin_labels)]
        #[qproperty(QStringList, plugin_enabled)]
        type DevicesModel = super::DevicesModelRust;

        /// Re-read the devices Magnetita reports.
        #[qinvokable]
        fn reload(self: Pin<&mut DevicesModel>);

        /// Open device `index`'s mount in the file manager.
        #[qinvokable]
        fn open_mount(self: Pin<&mut DevicesModel>, index: i32);

        /// Ask device `index` to pair.
        #[qinvokable]
        fn pair_device(self: Pin<&mut DevicesModel>, index: i32);

        /// Drop the pairing with device `index`.
        #[qinvokable]
        fn unpair_device(self: Pin<&mut DevicesModel>, index: i32);

        /// Ring device `index` (find-my-phone).
        #[qinvokable]
        fn ring_device(self: Pin<&mut DevicesModel>, index: i32);

        /// Toggle play/pause on device `index`'s current player.
        #[qinvokable]
        fn media_play_pause(self: Pin<&mut DevicesModel>, index: i32);

        /// Skip to the next track on device `index`.
        #[qinvokable]
        fn media_next(self: Pin<&mut DevicesModel>, index: i32);

        /// Skip to the previous track on device `index`.
        #[qinvokable]
        fn media_previous(self: Pin<&mut DevicesModel>, index: i32);

        /// Load the Settings surface: paired devices and plugin toggles.
        #[qinvokable]
        fn reload_settings(self: Pin<&mut DevicesModel>);

        /// Forget (unpair) paired device `index`.
        #[qinvokable]
        fn forget_paired(self: Pin<&mut DevicesModel>, index: i32);

        /// Flip plugin `index`'s enabled state and persist it.
        #[qinvokable]
        fn toggle_plugin(self: Pin<&mut DevicesModel>, index: i32);
    }

    impl cxx_qt::Threading for DevicesModel {}
}

#[derive(Default)]
pub struct DevicesModelRust {
    device_names: QStringList,
    device_types: QStringList,
    device_mounts: QStringList,
    device_states: QStringList,
    device_fingerprints: QStringList,
    device_battery: QStringList,
    device_charging: QStringList,
    device_paired: QStringList,
    device_media: QStringList,
    device_media_titles: QStringList,
    device_media_artists: QStringList,
    device_media_albums: QStringList,
    device_media_artwork: QStringList,
    device_media_lengths: QStringList,
    device_media_positions: QStringList,
    device_media_playing: QStringList,
    device_media_next: QStringList,
    device_media_previous: QStringList,
    device_media_seek: QStringList,
    log_lines: QStringList,
    log_failures: QStringList,
    paired_names: QStringList,
    paired_fingerprints: QStringList,
    paired_connected: QStringList,
    plugin_labels: QStringList,
    plugin_enabled: QStringList,
    watch_started: bool,
    event_watch_started: bool,
    devices: Vec<crate::devices::Device>,
    paired: Vec<crate::devices::Paired>,
    plugin_states: Vec<bool>,
}

/// The plugins the Settings surface shows, in order: (D-Bus key, Spanish label).
const PLUGINS: [(&str, &str); 6] = [
    ("battery", "Batería"),
    ("notifications", "Notificaciones del móvil"),
    ("clipboard", "Portapapeles"),
    ("share", "Compartir archivos"),
    ("findmyphone", "Sonar el móvil"),
    ("media", "Control de medios"),
];

impl qobject::DevicesModel {
    /// Reads the reported devices into the parallel lists, then arms the watch so
    /// later connect / mount / leave events refresh on their own.
    pub fn reload(mut self: Pin<&mut Self>) {
        let devices = crate::devices::list_devices().unwrap_or_default();

        let names: QStringList = devices
            .iter()
            .map(|device| QString::from(device.name.as_str()))
            .collect();
        let types: QStringList = devices
            .iter()
            .map(|device| QString::from(device.device_type.as_str()))
            .collect();
        let mounts: QStringList = devices
            .iter()
            .map(|device| QString::from(device.mount_path.as_str()))
            .collect();
        let states: QStringList = devices
            .iter()
            .map(|device| QString::from(state_label(device)))
            .collect();
        let fingerprints: QStringList = devices
            .iter()
            .map(|device| QString::from(device.fingerprint.as_str()))
            .collect();
        let paired: QStringList = devices
            .iter()
            .map(|device| QString::from(if device.paired { "true" } else { "false" }))
            .collect();
        let battery: QStringList = devices
            .iter()
            .map(|device| QString::from(battery_label(device).as_str()))
            .collect();
        let charging: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.charging)))
            .collect();
        let media: QStringList = devices
            .iter()
            .map(|device| QString::from(media_label(device).as_str()))
            .collect();
        let media_titles: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_title.as_str()))
            .collect();
        let media_artists: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_artist.as_str()))
            .collect();
        let media_albums: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_album.as_str()))
            .collect();
        let media_artwork: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_artwork_url.as_str()))
            .collect();
        let media_lengths: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_length.to_string().as_str()))
            .collect();
        let media_positions: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_position.to_string().as_str()))
            .collect();
        let media_playing: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_playing)))
            .collect();
        let media_next: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_can_next)))
            .collect();
        let media_previous: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_can_previous)))
            .collect();
        let media_seek: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_can_seek)))
            .collect();

        self.as_mut().rust_mut().get_mut().devices = devices;
        self.as_mut().set_device_names(names);
        self.as_mut().set_device_types(types);
        self.as_mut().set_device_mounts(mounts);
        self.as_mut().set_device_states(states);
        self.as_mut().set_device_fingerprints(fingerprints);
        self.as_mut().set_device_battery(battery);
        self.as_mut().set_device_charging(charging);
        self.as_mut().set_device_paired(paired);
        self.as_mut().set_device_media(media);
        self.as_mut().set_device_media_titles(media_titles);
        self.as_mut().set_device_media_artists(media_artists);
        self.as_mut().set_device_media_albums(media_albums);
        self.as_mut().set_device_media_artwork(media_artwork);
        self.as_mut().set_device_media_lengths(media_lengths);
        self.as_mut().set_device_media_positions(media_positions);
        self.as_mut().set_device_media_playing(media_playing);
        self.as_mut().set_device_media_next(media_next);
        self.as_mut().set_device_media_previous(media_previous);
        self.as_mut().set_device_media_seek(media_seek);

        self.as_mut().reload_log();
        self.as_mut().start_watch();
        self.as_mut().start_event_watch();
    }

    /// Ask device `index` to pair (its "Emparejar" button).
    pub fn pair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
        {
            crate::devices::request_pair(&device.id);
        }
    }

    /// Drop the pairing with device `index` (its "Desvincular" button).
    pub fn unpair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
        {
            crate::devices::unpair(&device.id);
        }
    }

    /// Ring device `index` (its "Sonar" button — find-my-phone).
    pub fn ring_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
        {
            crate::devices::ring(&device.id);
        }
    }

    /// Toggle play/pause on device `index`'s current player.
    pub fn media_play_pause(self: Pin<&mut Self>, index: i32) {
        self.media(index, "PlayPause");
    }

    /// Skip to the next track on device `index`.
    pub fn media_next(self: Pin<&mut Self>, index: i32) {
        self.media(index, "Next");
    }

    /// Skip to the previous track on device `index`.
    pub fn media_previous(self: Pin<&mut Self>, index: i32) {
        self.media(index, "Previous");
    }

    /// Forward a transport verb to device `index`'s active player.
    fn media(self: Pin<&mut Self>, index: i32, action: &str) {
        if let Some(device) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
        {
            crate::devices::media_action(&device.id, action);
        }
    }

    /// Load the Settings surface — the paired devices and the plugin toggles.
    pub fn reload_settings(mut self: Pin<&mut Self>) {
        let paired = crate::devices::list_paired();
        let flags = crate::devices::plugin_settings();

        let paired_names: QStringList = paired
            .iter()
            .map(|device| QString::from(device.name.as_str()))
            .collect();
        let paired_fingerprints: QStringList = paired
            .iter()
            .map(|device| QString::from(device.fingerprint.as_str()))
            .collect();
        let paired_connected: QStringList = paired
            .iter()
            .map(|device| QString::from(flag(device.connected)))
            .collect();

        // A key missing from the map means Magnetita is down; default to on.
        let states: Vec<bool> = PLUGINS
            .iter()
            .map(|(key, _)| flags.get(*key).copied().unwrap_or(true))
            .collect();
        let plugin_labels: QStringList = PLUGINS
            .iter()
            .map(|(_, label)| QString::from(*label))
            .collect();
        let plugin_enabled: QStringList =
            states.iter().map(|on| QString::from(flag(*on))).collect();

        self.as_mut().rust_mut().get_mut().paired = paired;
        self.as_mut().rust_mut().get_mut().plugin_states = states;
        self.as_mut().set_paired_names(paired_names);
        self.as_mut().set_paired_fingerprints(paired_fingerprints);
        self.as_mut().set_paired_connected(paired_connected);
        self.as_mut().set_plugin_labels(plugin_labels);
        self.as_mut().set_plugin_enabled(plugin_enabled);
    }

    /// Forget (unpair) paired device `index`, then refresh the surface.
    pub fn forget_paired(mut self: Pin<&mut Self>, index: i32) {
        let id = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().paired.get(i))
            .map(|device| device.id.clone());
        if let Some(id) = id {
            crate::devices::forget(&id);
        }
        self.as_mut().reload_settings();
    }

    /// Flip plugin `index`'s enabled state, persist it, and refresh.
    pub fn toggle_plugin(mut self: Pin<&mut Self>, index: i32) {
        let Some(i) = usize::try_from(index).ok() else {
            return;
        };
        let Some((key, _)) = PLUGINS.get(i) else {
            return;
        };
        let current = self.rust().plugin_states.get(i).copied().unwrap_or(true);
        crate::devices::set_plugin(key, !current);
        self.as_mut().reload_settings();
    }

    /// Re-read the connection log, newest first, into the parallel lists.
    pub fn reload_log(mut self: Pin<&mut Self>) {
        let entries = crate::devices::recent_log().unwrap_or_default();

        // Newest at the top.
        let lines: QStringList = entries
            .iter()
            .rev()
            .map(|entry| QString::from(format!("{} — {}", entry.device, entry.message).as_str()))
            .collect();
        let failures: QStringList = entries
            .iter()
            .rev()
            .map(|entry| QString::from(if entry.failure { "true" } else { "false" }))
            .collect();

        self.as_mut().set_log_lines(lines);
        self.as_mut().set_log_failures(failures);
    }

    /// Watches the daemon's `Event` signal to keep the log live.
    fn start_event_watch(mut self: Pin<&mut Self>) {
        if self.rust().event_watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().event_watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::watch_events(move || {
                let _ = qt.queue(|model: Pin<&mut qobject::DevicesModel>| {
                    model.reload_log();
                });
            });
            if let Err(error) = result {
                eprintln!("Magnetita: watch de eventos no disponible: {error}");
            }
        });
    }

    /// Starts, once, a thread that watches Magnetita's `Changed` signal and
    /// reloads on the Qt thread. Best-effort.
    fn start_watch(mut self: Pin<&mut Self>) {
        if self.rust().watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::watch_changes(move || {
                let _ = qt.queue(|model: Pin<&mut qobject::DevicesModel>| {
                    model.reload();
                });
            });
            if let Err(error) = result {
                eprintln!("Magnetita: watch de dispositivos no disponible: {error}");
            }
        });
    }

    /// Opens the device's mount in the file manager (Siderita). A device with no
    /// mount yet is a no-op.
    pub fn open_mount(self: Pin<&mut Self>, index: i32) {
        let mount = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().devices.get(index))
            .map(|device| device.mount_path.clone())
            .unwrap_or_default();
        if !mount.is_empty() {
            let _ = std::process::Command::new("xdg-open").arg(mount).spawn();
        }
    }
}

/// A device's battery as text: "🔋 58 %", "🔋 58 % ⚡" charging, "" when unknown.
fn battery_label(device: &crate::devices::Device) -> String {
    if device.battery < 0 {
        String::new()
    } else if device.charging {
        format!("🔋 {} % ⚡", device.battery)
    } else {
        format!("🔋 {} %", device.battery)
    }
}

/// The phone's now-playing as one line — "Artista — Título", or whichever half
/// it sent — or "" when nothing is playing (which hides the media card).
fn media_label(device: &crate::devices::Device) -> String {
    if device.media_player.is_empty() {
        return String::new();
    }
    match (device.media_artist.as_str(), device.media_title.as_str()) {
        ("", "") => String::new(),
        ("", title) => title.to_owned(),
        (artist, "") => artist.to_owned(),
        (artist, title) => format!("{artist} — {title}"),
    }
}

/// A boolean as the "true"/"false" string the parallel flag lists carry.
fn flag(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// A device's state, in the app's words.
fn state_label(device: &crate::devices::Device) -> &'static str {
    if device.mounted {
        "montado"
    } else if device.connected {
        "conectando…"
    } else {
        "desconectado"
    }
}
