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
        // Per-device pairing flag ("true"/"false"), parallel to the lists above.
        #[qproperty(QStringList, device_paired)]
        // The connection log — newest first — with a parallel failure flag
        // ("true"/"false") for red styling.
        #[qproperty(QStringList, log_lines)]
        #[qproperty(QStringList, log_failures)]
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
    device_paired: QStringList,
    log_lines: QStringList,
    log_failures: QStringList,
    watch_started: bool,
    event_watch_started: bool,
    devices: Vec<crate::devices::Device>,
}

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

        self.as_mut().rust_mut().get_mut().devices = devices;
        self.as_mut().set_device_names(names);
        self.as_mut().set_device_types(types);
        self.as_mut().set_device_mounts(mounts);
        self.as_mut().set_device_states(states);
        self.as_mut().set_device_fingerprints(fingerprints);
        self.as_mut().set_device_battery(battery);
        self.as_mut().set_device_paired(paired);

        self.as_mut().reload_log();
        self.as_mut().start_watch();
        self.as_mut().start_event_watch();
    }

    /// Ask device `index` to pair (its "Emparejar" button).
    pub fn pair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device) = usize::try_from(index).ok().and_then(|i| self.rust().devices.get(i)) {
            crate::devices::request_pair(&device.id);
        }
    }

    /// Drop the pairing with device `index` (its "Desvincular" button).
    pub fn unpair_device(self: Pin<&mut Self>, index: i32) {
        if let Some(device) = usize::try_from(index).ok().and_then(|i| self.rust().devices.get(i)) {
            crate::devices::unpair(&device.id);
        }
    }

    /// Re-read the connection log, newest first, into the parallel lists.
    pub fn reload_log(mut self: Pin<&mut Self>) {
        let entries = crate::devices::recent_log().unwrap_or_default();

        // Newest at the top.
        let lines: QStringList = entries
            .iter()
            .rev()
            .map(|entry| {
                QString::from(format!("{} — {}", entry.device, entry.message).as_str())
            })
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
