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
        type DevicesModel = super::DevicesModelRust;

        /// Re-read the devices Magnetita reports.
        #[qinvokable]
        fn reload(self: Pin<&mut DevicesModel>);

        /// Open device `index`'s mount in the file manager.
        #[qinvokable]
        fn open_mount(self: Pin<&mut DevicesModel>, index: i32);
    }

    impl cxx_qt::Threading for DevicesModel {}
}

#[derive(Default)]
pub struct DevicesModelRust {
    device_names: QStringList,
    device_types: QStringList,
    device_mounts: QStringList,
    device_states: QStringList,
    watch_started: bool,
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

        self.as_mut().rust_mut().get_mut().devices = devices;
        self.as_mut().set_device_names(names);
        self.as_mut().set_device_types(types);
        self.as_mut().set_device_mounts(mounts);
        self.as_mut().set_device_states(states);

        self.as_mut().start_watch();
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
