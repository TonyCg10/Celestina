//! The app's one QObject: a live view of the devices Magnetita reports.
//!
//! It reads [`crate::devices`] and publishes the result as parallel lists QML
//! iterates, refreshing itself off the `Changed` signal — the same shape as
//! Siderita's volume list. The app is a client; this holds no device state of
//! its own beyond the last snapshot.

use core::pin::Pin;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::thread::JoinHandle;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use magnetita_core::MediaAction;

use crate::projection::{
    battery_label, flag, media_label, next_toggle_value, progress_fields, state_label,
};

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
        #[qproperty(QStringList, device_verification_keys)]
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
        #[qproperty(QStringList, device_media_players)]
        #[qproperty(QStringList, device_media_titles)]
        #[qproperty(QStringList, device_media_artists)]
        #[qproperty(QStringList, device_media_albums)]
        #[qproperty(QStringList, device_media_now_playing)]
        #[qproperty(QStringList, device_media_artwork)]
        #[qproperty(QStringList, device_media_lengths)]
        #[qproperty(QStringList, device_media_positions)]
        #[qproperty(QStringList, device_media_playing)]
        #[qproperty(QStringList, device_media_play)]
        #[qproperty(QStringList, device_media_pause)]
        #[qproperty(QStringList, device_media_next)]
        #[qproperty(QStringList, device_media_previous)]
        #[qproperty(QStringList, device_media_seek)]
        #[qproperty(QStringList, device_media_progress)]
        #[qproperty(bool, devices_available)]
        // The connection log — newest first — with a parallel failure flag
        // ("true"/"false") for red styling.
        #[qproperty(QStringList, log_lines)]
        #[qproperty(QStringList, log_failures)]
        #[qproperty(bool, log_available)]
        // The Settings surface: paired devices (name + fingerprint + a
        // "true"/"false" connected flag) and the per-plugin toggles (label +
        // "true"/"false" enabled flag), all parallel lists.
        #[qproperty(QStringList, paired_names)]
        #[qproperty(QStringList, paired_fingerprints)]
        #[qproperty(QStringList, paired_connected)]
        #[qproperty(QStringList, plugin_labels)]
        #[qproperty(QStringList, plugin_enabled)]
        #[qproperty(bool, settings_available)]
        // The wireless screen mirror: the state in the author's words, whether
        // the phone is showing a pairing screen right now, and whether the
        // control should offer to stop rather than to start.
        #[qproperty(QString, mirror_label)]
        #[qproperty(bool, mirror_can_pair)]
        #[qproperty(bool, mirror_active)]
        #[qproperty(bool, mirror_available)]
        // The mirror options, each the daemon's own contract name so the
        // settings surface binds to one value rather than parsing a list.
        #[qproperty(QString, mirror_resolution)]
        #[qproperty(QString, mirror_rate)]
        #[qproperty(QString, mirror_quality)]
        #[qproperty(QString, mirror_audio)]
        #[qproperty(bool, mirror_screen_off)]
        #[qproperty(bool, mirror_stay_awake)]
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

        /// Re-read the mirror state. The daemon publishes no change signal for
        /// it — its state moves on the phone's schedule, not on a bus event —
        /// so the card polls this while it is on screen.
        #[qinvokable]
        fn reload_mirror(self: Pin<&mut DevicesModel>);

        /// Start mirroring, and keep mirroring across the phone's port changes.
        #[qinvokable]
        fn start_mirror(self: Pin<&mut DevicesModel>);

        /// Stop mirroring and stop reconnecting.
        #[qinvokable]
        fn stop_mirror(self: Pin<&mut DevicesModel>);

        /// Pair with the code the phone is showing.
        #[qinvokable]
        fn pair_mirror(self: Pin<&mut DevicesModel>, code: QString);

        /// Change one mirror option, by the daemon's contract names.
        #[qinvokable]
        fn set_mirror_option(self: Pin<&mut DevicesModel>, key: QString, value: QString);
    }

    impl cxx_qt::Threading for DevicesModel {}
}

#[derive(Default)]
pub struct DevicesModelRust {
    device_names: QStringList,
    device_types: QStringList,
    device_mounts: QStringList,
    device_states: QStringList,
    device_verification_keys: QStringList,
    device_battery: QStringList,
    device_charging: QStringList,
    device_paired: QStringList,
    device_media: QStringList,
    device_media_players: QStringList,
    device_media_titles: QStringList,
    device_media_artists: QStringList,
    device_media_albums: QStringList,
    device_media_now_playing: QStringList,
    device_media_artwork: QStringList,
    device_media_lengths: QStringList,
    device_media_positions: QStringList,
    device_media_playing: QStringList,
    device_media_play: QStringList,
    device_media_pause: QStringList,
    device_media_next: QStringList,
    device_media_previous: QStringList,
    device_media_seek: QStringList,
    device_media_progress: QStringList,
    devices_available: bool,
    log_lines: QStringList,
    log_failures: QStringList,
    log_available: bool,
    paired_names: QStringList,
    paired_fingerprints: QStringList,
    paired_connected: QStringList,
    plugin_labels: QStringList,
    plugin_enabled: QStringList,
    settings_available: bool,
    mirror_label: QString,
    mirror_can_pair: bool,
    mirror_active: bool,
    mirror_available: bool,
    mirror_resolution: QString,
    mirror_rate: QString,
    mirror_quality: QString,
    mirror_audio: QString,
    mirror_screen_off: bool,
    mirror_stay_awake: bool,
    watch_started: bool,
    event_watch_started: bool,
    device_reload_in_flight: bool,
    device_reload_pending: bool,
    log_reload_in_flight: bool,
    log_reload_pending: bool,
    settings_reload_in_flight: bool,
    settings_reload_pending: bool,
    devices: Vec<crate::devices::Device>,
    paired: Vec<crate::devices::Paired>,
    plugin_states: Vec<bool>,
    plugin_intents: Vec<Option<bool>>,
    command_sender: Option<SyncSender<ClientCommand>>,
    command_worker: Option<JoinHandle<()>>,
}

impl Drop for DevicesModelRust {
    fn drop(&mut self) {
        self.command_sender.take();
        if let Some(worker) = self.command_worker.take() {
            let _ = worker.join();
        }
    }
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

enum ClientCommand {
    Pair(String),
    Unpair(String),
    Ring(String),
    Media(String, MediaAction),
    Forget(String),
    SetPlugin(&'static str, bool),
    MirrorStart,
    MirrorStop,
    MirrorPair(String),
    MirrorOption(String, String),
}

impl ClientCommand {
    fn run(self) -> bool {
        let refresh_settings = matches!(&self, Self::Forget(_) | Self::SetPlugin(_, _));
        let result = match self {
            Self::Pair(id) => crate::devices::request_pair(&id),
            Self::Unpair(id) => crate::devices::unpair(&id),
            Self::Ring(id) => crate::devices::ring(&id),
            Self::Media(id, action) => crate::devices::media_action(&id, action),
            Self::Forget(id) => crate::devices::forget(&id),
            Self::SetPlugin(plugin, enabled) => crate::devices::set_plugin(plugin, enabled),
            Self::MirrorStart => crate::devices::mirror_start(),
            Self::MirrorStop => crate::devices::mirror_stop(),
            Self::MirrorPair(code) => crate::devices::mirror_pair(&code),
            Self::MirrorOption(key, value) => crate::devices::mirror_set_option(&key, &value),
        };
        if let Err(error) = result {
            eprintln!("magnetita: D-Bus action failed: {error}");
        }
        refresh_settings
    }
}

impl qobject::DevicesModel {
    /// Arms the signal watches and schedules D-Bus reads away from the GUI
    /// thread. Bursts coalesce to at most one follow-up snapshot.
    pub fn reload(mut self: Pin<&mut Self>) {
        self.as_mut().start_watch();
        self.as_mut().start_event_watch();
        self.as_mut().request_device_reload();
        self.as_mut().request_log_reload();
    }

    /// Serialize bounded UI actions through one worker. This preserves click
    /// order, avoids one detached thread per click, and lets destruction close
    /// the channel and join the worker deterministically.
    fn enqueue_command(mut self: Pin<&mut Self>, command: ClientCommand) -> bool {
        if self.rust().command_sender.is_none() {
            let (sender, receiver) = sync_channel::<ClientCommand>(32);
            let qt = self.qt_thread();
            let worker = std::thread::spawn(move || {
                while let Ok(command) = receiver.recv() {
                    if command.run() {
                        let _ = qt.queue(|model: Pin<&mut qobject::DevicesModel>| {
                            model.reload_settings();
                        });
                    }
                }
            });
            let state = self.as_mut().rust_mut().get_mut();
            state.command_sender = Some(sender);
            state.command_worker = Some(worker);
        }

        if let Some(sender) = self.rust().command_sender.as_ref() {
            match sender.try_send(command) {
                Ok(()) => return true,
                Err(TrySendError::Full(_)) => {
                    eprintln!("magnetita: UI action queue is full; action dropped");
                }
                Err(TrySendError::Disconnected(_)) => {
                    eprintln!("magnetita: UI action worker is unavailable");
                }
            }
        }
        false
    }

    fn request_device_reload(mut self: Pin<&mut Self>) {
        if self.rust().device_reload_in_flight {
            self.as_mut().rust_mut().get_mut().device_reload_pending = true;
            return;
        }
        self.as_mut().rust_mut().get_mut().device_reload_in_flight = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::list_devices();
            let _ = qt.queue(move |model: Pin<&mut qobject::DevicesModel>| {
                model.finish_device_reload(result);
            });
        });
    }

    fn finish_device_reload(
        mut self: Pin<&mut Self>,
        result: Result<Vec<crate::devices::Device>, String>,
    ) {
        match result {
            Ok(devices) => {
                self.as_mut().apply_devices(devices);
                self.as_mut().set_devices_available(true);
            }
            Err(error) => {
                eprintln!("magnetita: device snapshot unavailable: {error}");
                self.as_mut().set_devices_available(false);
            }
        }
        let pending = self.rust().device_reload_pending;
        let state = self.as_mut().rust_mut().get_mut();
        state.device_reload_in_flight = false;
        state.device_reload_pending = false;
        if pending {
            self.as_mut().request_device_reload();
        }
    }

    fn apply_devices(mut self: Pin<&mut Self>, devices: Vec<crate::devices::Device>) {
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
        let verification_keys: QStringList = devices
            .iter()
            .map(|device| QString::from(device.verification_key.as_str()))
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
        let media_players: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_player.as_str()))
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
        let media_now_playing: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_now_playing.as_str()))
            .collect();
        let media_artwork: QStringList = devices
            .iter()
            .map(|device| QString::from(device.media_artwork_url.as_str()))
            .collect();
        let playback: Vec<_> = devices.iter().map(progress_fields).collect();
        let media_lengths: QStringList = playback
            .iter()
            .map(|(_, length, _)| QString::from(length.to_string().as_str()))
            .collect();
        let media_positions: QStringList = playback
            .iter()
            .map(|(position, _, _)| QString::from(position.to_string().as_str()))
            .collect();
        let media_playing: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_playing)))
            .collect();
        let media_play: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_can_play)))
            .collect();
        let media_pause: QStringList = devices
            .iter()
            .map(|device| QString::from(flag(device.media_can_pause)))
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
        let media_progress: QStringList = playback
            .iter()
            .map(|(_, _, kind)| QString::from(*kind))
            .collect();

        self.as_mut().rust_mut().get_mut().devices = devices;
        self.as_mut().set_device_names(names);
        self.as_mut().set_device_types(types);
        self.as_mut().set_device_mounts(mounts);
        self.as_mut().set_device_states(states);
        self.as_mut()
            .set_device_verification_keys(verification_keys);
        self.as_mut().set_device_battery(battery);
        self.as_mut().set_device_charging(charging);
        self.as_mut().set_device_paired(paired);
        self.as_mut().set_device_media(media);
        self.as_mut().set_device_media_players(media_players);
        self.as_mut().set_device_media_titles(media_titles);
        self.as_mut().set_device_media_artists(media_artists);
        self.as_mut().set_device_media_albums(media_albums);
        self.as_mut()
            .set_device_media_now_playing(media_now_playing);
        self.as_mut().set_device_media_artwork(media_artwork);
        self.as_mut().set_device_media_lengths(media_lengths);
        self.as_mut().set_device_media_positions(media_positions);
        self.as_mut().set_device_media_playing(media_playing);
        self.as_mut().set_device_media_play(media_play);
        self.as_mut().set_device_media_pause(media_pause);
        self.as_mut().set_device_media_next(media_next);
        self.as_mut().set_device_media_previous(media_previous);
        self.as_mut().set_device_media_seek(media_seek);
        self.as_mut().set_device_media_progress(media_progress);
    }

    /// Ask device `index` to pair (its "Emparejar" button).
    pub fn pair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device_id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
            .map(|device| device.id.clone())
        {
            self.enqueue_command(ClientCommand::Pair(device_id));
        }
    }

    /// Drop the pairing with device `index` (its "Desvincular" button).
    pub fn unpair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device_id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
            .map(|device| device.id.clone())
        {
            self.enqueue_command(ClientCommand::Unpair(device_id));
        }
    }

    /// Ring device `index` (its "Sonar" button — find-my-phone).
    pub fn ring_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device_id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
            .map(|device| device.id.clone())
        {
            self.enqueue_command(ClientCommand::Ring(device_id));
        }
    }

    /// Toggle play/pause on device `index`'s current player.
    pub fn media_play_pause(self: Pin<&mut Self>, index: i32) {
        self.media(index, MediaAction::PlayPause);
    }

    /// Skip to the next track on device `index`.
    pub fn media_next(self: Pin<&mut Self>, index: i32) {
        self.media(index, MediaAction::Next);
    }

    /// Skip to the previous track on device `index`.
    pub fn media_previous(self: Pin<&mut Self>, index: i32) {
        self.media(index, MediaAction::Previous);
    }

    /// Forward a transport verb to device `index`'s active player.
    fn media(self: Pin<&mut Self>, index: i32, action: MediaAction) {
        if let Some(device_id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().devices.get(i))
            .map(|device| device.id.clone())
        {
            self.enqueue_command(ClientCommand::Media(device_id, action));
        }
    }

    /// Load the Settings surface — the paired devices and the plugin toggles.
    pub fn reload_settings(mut self: Pin<&mut Self>) {
        if self.rust().settings_reload_in_flight {
            self.as_mut().rust_mut().get_mut().settings_reload_pending = true;
            return;
        }
        self.as_mut().rust_mut().get_mut().settings_reload_in_flight = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::settings_snapshot();
            let _ = qt.queue(move |model: Pin<&mut qobject::DevicesModel>| {
                model.finish_settings_reload(result);
            });
        });
    }

    fn finish_settings_reload(
        mut self: Pin<&mut Self>,
        result: Result<crate::devices::SettingsSnapshot, String>,
    ) {
        match result {
            Ok(snapshot) => {
                self.as_mut().apply_settings(snapshot);
                self.as_mut().set_settings_available(true);
            }
            Err(error) => {
                eprintln!("magnetita: settings snapshot unavailable: {error}");
                self.as_mut().set_settings_available(false);
            }
        }
        let pending = self.rust().settings_reload_pending;
        let state = self.as_mut().rust_mut().get_mut();
        state.settings_reload_in_flight = false;
        state.settings_reload_pending = false;
        if pending {
            self.as_mut().reload_settings();
        }
    }

    fn apply_settings(mut self: Pin<&mut Self>, snapshot: crate::devices::SettingsSnapshot) {
        let paired = snapshot.paired;
        let flags = snapshot.plugins;
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

        // A missing key from an older daemon keeps that plugin's default-on
        // contract; transport failures never reach this confirmed snapshot.
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
        self.as_mut().rust_mut().get_mut().plugin_intents.clear();
        self.as_mut().set_paired_names(paired_names);
        self.as_mut().set_paired_fingerprints(paired_fingerprints);
        self.as_mut().set_paired_connected(paired_connected);
        self.as_mut().set_plugin_labels(plugin_labels);
        self.as_mut().set_plugin_enabled(plugin_enabled);
    }

    /// Forget (unpair) paired device `index`, then refresh the surface.
    pub fn forget_paired(self: Pin<&mut Self>, index: i32) {
        let id = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().paired.get(i))
            .map(|device| device.id.clone());
        let Some(id) = id else {
            return;
        };
        self.enqueue_command(ClientCommand::Forget(id));
    }

    /// Flip plugin `index`'s enabled state, persist it, and refresh.
    pub fn toggle_plugin(mut self: Pin<&mut Self>, index: i32) {
        let Some(i) = usize::try_from(index).ok() else {
            return;
        };
        let Some((key, _)) = PLUGINS.get(i) else {
            return;
        };
        let confirmed = self.rust().plugin_states.get(i).copied().unwrap_or(true);
        let pending = self.rust().plugin_intents.get(i).copied().flatten();
        let next = next_toggle_value(confirmed, pending);
        let state = self.as_mut().rust_mut().get_mut();
        state.plugin_intents.resize(PLUGINS.len(), None);
        state.plugin_intents[i] = Some(next);
        // The switch immediately re-binds to this confirmed snapshot in QML.
        // Publish the new value only after the worker has persisted it and a
        // fresh daemon read arrives; a click is a request, not state.
        if !self
            .as_mut()
            .enqueue_command(ClientCommand::SetPlugin(key, next))
        {
            self.as_mut().rust_mut().get_mut().plugin_intents[i] = None;
        }
    }

    fn request_log_reload(mut self: Pin<&mut Self>) {
        if self.rust().log_reload_in_flight {
            self.as_mut().rust_mut().get_mut().log_reload_pending = true;
            return;
        }
        self.as_mut().rust_mut().get_mut().log_reload_in_flight = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::recent_log();
            let _ = qt.queue(move |model: Pin<&mut qobject::DevicesModel>| {
                model.finish_log_reload(result);
            });
        });
    }

    fn finish_log_reload(
        mut self: Pin<&mut Self>,
        result: Result<Vec<crate::devices::LogEntry>, String>,
    ) {
        match result {
            Ok(entries) => {
                self.as_mut().apply_log(entries);
                self.as_mut().set_log_available(true);
            }
            Err(error) => {
                eprintln!("magnetita: log snapshot unavailable: {error}");
                self.as_mut().set_log_available(false);
            }
        }
        let pending = self.rust().log_reload_pending;
        let state = self.as_mut().rust_mut().get_mut();
        state.log_reload_in_flight = false;
        state.log_reload_pending = false;
        if pending {
            self.as_mut().request_log_reload();
        }
    }

    /// Apply a connection-log snapshot, newest first.
    fn apply_log(mut self: Pin<&mut Self>, entries: Vec<crate::devices::LogEntry>) {
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
                    model.request_log_reload();
                });
            });
            if let Err(error) = result {
                eprintln!("Magnetita: watch de eventos no disponible: {error}");
            }
        });
    }

    /// Starts, once, a thread that watches Magnetita's `Changed` signal and
    /// refreshes only the device snapshot. The independent `Event` watcher owns
    /// log refreshes, avoiding a second D-Bus round-trip on every 1 Hz media
    /// position update.
    fn start_watch(mut self: Pin<&mut Self>) {
        if self.rust().watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::watch_changes(move || {
                let _ = qt.queue(|model: Pin<&mut qobject::DevicesModel>| {
                    model.request_device_reload();
                });
            });
            if let Err(error) = result {
                eprintln!("Magnetita: watch de dispositivos no disponible: {error}");
            }
        });
    }

    /// Opens the device's mount in the file manager (Siderita). A device with no
    /// mount yet is a no-op.
    /// Reads the mirror snapshot off the GUI thread and applies it whole, so
    /// the state and the reason that explains it are never from different
    /// moments.
    pub fn reload_mirror(mut self: Pin<&mut Self>) {
        let qt = self.as_mut().qt_thread();
        std::thread::spawn(move || {
            let snapshot = crate::devices::mirror_snapshot();
            let _ = qt.queue(
                move |mut model: Pin<&mut qobject::DevicesModel>| match snapshot {
                    Ok(snapshot) => {
                        let label =
                            crate::projection::mirror_label(&snapshot.state, &snapshot.reason);
                        model.as_mut().set_mirror_label(QString::from(&label));
                        model.as_mut().set_mirror_can_pair(snapshot.can_pair);
                        model
                            .as_mut()
                            .set_mirror_active(crate::projection::mirror_is_active(
                                &snapshot.state,
                            ));
                        model.as_mut().set_mirror_available(true);
                        let option = |key: &str, fallback: &str| {
                            QString::from(
                                snapshot
                                    .options
                                    .get(key)
                                    .map(String::as_str)
                                    .unwrap_or(fallback),
                            )
                        };
                        model
                            .as_mut()
                            .set_mirror_resolution(option("resolution", "balanced"));
                        model.as_mut().set_mirror_rate(option("rate", "smooth"));
                        model
                            .as_mut()
                            .set_mirror_quality(option("quality", "everyday"));
                        model.as_mut().set_mirror_audio(option("audio", "phone"));
                        model.as_mut().set_mirror_screen_off(
                            snapshot.options.get("screenOff").map(String::as_str) == Some("true"),
                        );
                        model.as_mut().set_mirror_stay_awake(
                            snapshot.options.get("stayAwake").map(String::as_str) == Some("true"),
                        );
                    }
                    Err(error) => {
                        eprintln!("magnetita: mirror snapshot unavailable: {error}");
                        model.as_mut().set_mirror_available(false);
                        model.as_mut().set_mirror_can_pair(false);
                        model.as_mut().set_mirror_active(false);
                    }
                },
            );
        });
    }

    pub fn start_mirror(mut self: Pin<&mut Self>) {
        self.as_mut().enqueue_command(ClientCommand::MirrorStart);
    }

    pub fn stop_mirror(mut self: Pin<&mut Self>) {
        self.as_mut().enqueue_command(ClientCommand::MirrorStop);
    }

    pub fn set_mirror_option(mut self: Pin<&mut Self>, key: QString, value: QString) {
        self.as_mut().enqueue_command(ClientCommand::MirrorOption(
            key.to_string(),
            value.to_string(),
        ));
    }

    pub fn pair_mirror(mut self: Pin<&mut Self>, code: QString) {
        let code = code.to_string();
        // An empty code would make adb wait on stdin for one; refuse here.
        if code.is_empty() {
            return;
        }
        self.as_mut()
            .enqueue_command(ClientCommand::MirrorPair(code));
    }

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
