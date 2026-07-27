//! Mountable devices in the sidebar: removable volumes (UDisks2) and phones
//! (Magnetita). Reading each list is inline and quick; mounting can block on a
//! polkit prompt so it runs on a worker thread and reports back on the Qt
//! thread. Each list also arms a once-per-controller hotplug watch, and the user
//! can hide devices they never want to see.

use core::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use super::qobject;

impl qobject::SideritaController {
    /// Reads the removable volumes UDisks2 reports and publishes them to the
    /// sidebar (parallel name / device / mount-point lists), keeping the full
    /// records for mount / unmount by index. Read-only and quick — runs inline.
    pub fn load_volumes(mut self: Pin<&mut Self>) {
        let mut volumes = match crate::volumes::list_volumes() {
            Ok(volumes) => volumes,
            Err(error) => {
                self.as_mut().set_op_error(QString::from(error.as_str()));
                return;
            }
        };

        // Drop the devices the user hid (read fresh so a hide in another tab is
        // honoured here too).
        let hidden = crate::settings::load().hidden_devices;
        volumes.retain(|volume| !hidden.iter().any(|name| name == &volume.name));
        self.as_mut()
            .set_hidden_device_count(hidden.len().min(i32::MAX as usize) as i32);

        let names: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.name.as_str()))
            .collect();
        let devices: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.device.as_str()))
            .collect();
        let mounts: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.mount_point.as_str()))
            .collect();

        self.as_mut().rust_mut().get_mut().volumes = volumes;
        self.as_mut().set_volume_names(names);
        self.as_mut().set_volume_devices(devices);
        self.as_mut().set_volume_mounts(mounts);

        // First load also arms the hotplug watch, so later plug/unplug events
        // refresh the list on their own.
        self.as_mut().start_volume_watch();
    }

    /// Starts, once per controller, a background thread that watches UDisks2 for
    /// a device being added or removed and reloads the list on the Qt thread —
    /// so plugging or unplugging a drive updates "Dispositivos" without a manual
    /// refresh. Best-effort: an unavailable bus just logs and gives up.
    fn start_volume_watch(mut self: Pin<&mut Self>) {
        if self.rust().volume_watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().volume_watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::watch_changes(move || {
                let _ = qt.queue(|controller: Pin<&mut qobject::SideritaController>| {
                    controller.load_volumes();
                });
            });
            if let Err(error) = result {
                eprintln!("Siderita: watch de dispositivos no disponible: {error}");
            }
        });
    }

    /// Reads the phones Magnetita reports and publishes them to the sidebar
    /// (parallel name / type / mount-path lists), keeping the records for
    /// open-by-index. Read-only and quick — runs inline. Also arms the watch so
    /// later connect / mount / leave events refresh on their own.
    pub fn load_phones(mut self: Pin<&mut Self>) {
        let phones = crate::devices::list_devices().unwrap_or_default();

        let names: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.name.as_str()))
            .collect();
        let types: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.device_type.as_str()))
            .collect();
        let mounts: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.mount_path.as_str()))
            .collect();

        self.as_mut().rust_mut().get_mut().phones = phones;
        self.as_mut().set_phone_names(names);
        self.as_mut().set_phone_types(types);
        self.as_mut().set_phone_mounts(mounts);

        self.as_mut().start_phone_watch();
    }

    /// Starts, once per controller, a thread that watches Magnetita's `Changed`
    /// signal and reloads the phone list on the Qt thread — so a phone
    /// connecting, mounting or leaving updates "Dispositivos" without a manual
    /// refresh. Best-effort: an unavailable bus just logs and gives up.
    fn start_phone_watch(mut self: Pin<&mut Self>) {
        if self.rust().phone_watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().phone_watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::watch_changes(move || {
                let _ = qt.queue(|controller: Pin<&mut qobject::SideritaController>| {
                    controller.load_phones();
                });
            });
            if let Err(error) = result {
                eprintln!("Siderita: watch de Magnetita no disponible: {error}");
            }
        });
    }

    /// Opens the phone at `index` by navigating to its mount path. A phone that
    /// is connected but not yet mounted has no path, so this is a no-op until it
    /// is — the sidebar reflects that by not offering it as openable.
    pub fn open_phone(mut self: Pin<&mut Self>, index: i32) {
        let mount = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().phones.get(index))
            .map(|phone| phone.mount_path.clone())
            .unwrap_or_default();
        if !mount.is_empty() {
            self.as_mut().open_location(&QString::from(mount.as_str()));
        }
    }

    /// Send a local file to the connected phone (the "Enviar al móvil" menu
    /// item). Sends to the first connected phone; a no-op if none is connected.
    pub fn send_to_phone(self: Pin<&mut Self>, path: &QString) {
        if let Some(phone) = self.rust().phones.first() {
            crate::devices::send_file(&phone.id, &path.to_string());
        }
    }

    /// Mounts the volume at `index` on a worker thread — mounting can block on a
    /// polkit authorization prompt, so it must never run on the Qt thread — then
    /// refreshes the list (or reports the failure) back on the Qt thread.
    pub fn mount_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Montando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::mount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(_) => controller.as_mut().load_volumes(),
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    /// Unmounts the volume at `index` on a worker thread, then refreshes.
    pub fn unmount_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Desmontando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::unmount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(()) => controller.as_mut().load_volumes(),
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    /// Opens the volume at `index`: navigates to its mount point, mounting it
    /// first (on a worker thread) if it is not yet mounted.
    pub fn open_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        let mounted_at = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().volumes.get(index))
            .map(|volume| volume.mount_point.clone())
            .unwrap_or_default();

        if !mounted_at.is_empty() {
            self.as_mut()
                .open_location(&QString::from(mounted_at.as_str()));
            return;
        }

        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Montando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::mount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(mount_point) => {
                        controller.as_mut().load_volumes();
                        if !mount_point.is_empty() {
                            controller
                                .as_mut()
                                .open_location(&QString::from(mount_point.as_str()));
                        }
                    }
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    fn volume_path(&self, index: i32) -> Option<String> {
        let index = usize::try_from(index).ok()?;
        self.rust()
            .volumes
            .get(index)
            .map(|volume| volume.object_path.clone())
    }

    /// Hides a removable device (by its display name) from the sidebar and
    /// remembers the choice; the list is re-read so it disappears at once.
    pub fn hide_device(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        if name.is_empty() {
            return;
        }
        let mut settings = crate::settings::load();
        if !settings.hidden_devices.contains(&name) {
            settings.hidden_devices.push(name);
            let _ = crate::settings::save(&settings);
        }
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().load_volumes();
    }

    /// Un-hides every previously-hidden device.
    pub fn unhide_all_devices(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.hidden_devices.clear();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().load_volumes();
    }
}
