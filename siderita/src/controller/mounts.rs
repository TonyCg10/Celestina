//! Mountable devices in the sidebar: removable volumes (UDisks2) and phones
//! (Magnetita). Reading each list is inline and quick; mounting can block on a
//! polkit prompt so it runs on a worker thread and reports back on the Qt
//! thread. Each list also arms a once-per-controller hotplug watch, and the user
//! can hide devices they never want to see.

use core::pin::Pin;
use std::path::Path;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use super::qobject;

fn visible_location_name(path: &str, phones: &[crate::devices::Device]) -> String {
    let normalized = path.trim_end_matches('/');
    if normalized.is_empty() {
        return "/".to_owned();
    }

    if let Some(phone) = phones.iter().find(|phone| {
        !phone.mount_path.is_empty()
            && phone.mount_path.trim_end_matches('/') == normalized
            && !phone.name.is_empty()
    }) {
        return phone.name.clone();
    }

    Path::new(normalized)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| normalized.to_owned())
}

/// A mount point as the path key the sidebar navigates with. An unmounted
/// device has no path and keys to the empty string, which is what the rows read
/// as "not openable yet".
fn mount_key(mount: &str) -> QString {
    if mount.is_empty() {
        return QString::default();
    }
    QString::from(crate::pathkey::encode(Path::new(mount)).as_str())
}

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
        // Published as path keys, like every other path this bridge hands out,
        // so the sidebar can open a mounted volume without spelling its path.
        let mounts: QStringList = volumes
            .iter()
            .map(|volume| mount_key(&volume.mount_point))
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
            .map(|phone| mount_key(&phone.mount_path))
            .collect();

        self.as_mut().rust_mut().get_mut().phones = phones;
        self.as_mut().set_phone_names(names);
        self.as_mut().set_phone_types(types);
        self.as_mut().set_phone_mounts(mounts);
        let next_revision = self.phone_revision().wrapping_add(1);
        self.as_mut().set_phone_revision(next_revision);

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

    /// One atomic QML snapshot: id, name, type, connected, mounted, mount,
    /// player, title, artist, album, artwork, playing, can-pause/next/previous,
    /// length and position in milliseconds.
    pub fn phone_info(&self, index: i32) -> QStringList {
        let Some(phone) = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().phones.get(index))
        else {
            return QStringList::default();
        };
        let flag = |value| QString::from(if value { "1" } else { "0" });
        [
            QString::from(phone.id.as_str()),
            QString::from(phone.name.as_str()),
            QString::from(phone.device_type.as_str()),
            flag(phone.connected),
            flag(phone.mounted),
            mount_key(&phone.mount_path),
            QString::from(phone.media_player.as_str()),
            QString::from(phone.media_title.as_str()),
            QString::from(phone.media_artist.as_str()),
            QString::from(phone.media_album.as_str()),
            QString::from(phone.media_artwork_url.as_str()),
            flag(phone.media_playing),
            flag(phone.media_can_pause),
            flag(phone.media_can_next),
            flag(phone.media_can_previous),
            QString::from(phone.media_length.to_string().as_str()),
            QString::from(phone.media_position.to_string().as_str()),
        ]
        .into_iter()
        .collect()
    }

    /// Ask a connected phone to ring through Magnetita's stable D-Bus API.
    pub fn ring_phone(&self, index: i32) {
        if let Some(phone) = self.connected_phone(index) {
            crate::devices::ring(&phone.id);
        }
    }

    /// Forward only the three media actions Devices1 publishes.
    pub fn control_phone_media(&self, index: i32, action: &QString) {
        let action = action.to_string();
        if !matches!(action.as_str(), "PlayPause" | "Next" | "Previous") {
            return;
        }
        if let Some(phone) = self.connected_phone(index) {
            crate::devices::media_action(&phone.id, &action);
        }
    }

    /// Human-facing name for the location `key` names. Magnetita mounts by
    /// stable device id, but that transport detail must not leak into tabs,
    /// headings or crumbs. Takes a key and answers display text: this is the
    /// one direction ADR 0008 allows between the two representations.
    pub fn display_location_name(&self, key: &QString) -> QString {
        let Ok(path) = crate::pathkey::decode(key) else {
            return QString::default();
        };
        QString::from(visible_location_name(
            &path.to_string_lossy(),
            &self.rust().phones,
        ))
    }

    /// Send a local file to the connected phone (the "Enviar al móvil" menu
    /// item). Sends to the first connected phone; a no-op if none is connected.
    pub fn send_to_phone(mut self: Pin<&mut Self>, key: &QString) {
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };
        if let Some(phone) = self.rust().phones.iter().find(|phone| phone.connected) {
            crate::devices::send_file(&phone.id, &path.to_string_lossy());
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

    fn connected_phone(&self, index: i32) -> Option<&crate::devices::Device> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().phones.get(index))
            .filter(|phone| phone.connected)
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

#[cfg(test)]
mod tests {
    use super::visible_location_name;
    use crate::devices::Device;

    #[test]
    fn a_phone_mount_uses_the_device_name_not_its_stable_id() {
        let phones = vec![Device {
            name: "Galaxy S25 Ultra".to_owned(),
            mount_path: "/run/user/1000/magnetita/689da02afffe4b12".to_owned(),
            ..Device::default()
        }];

        assert_eq!(
            visible_location_name("/run/user/1000/magnetita/689da02afffe4b12/", &phones,),
            "Galaxy S25 Ultra"
        );
    }

    #[test]
    fn ordinary_locations_keep_their_basename() {
        assert_eq!(
            visible_location_name("/home/toni/Documentos", &[]),
            "Documentos"
        );
        assert_eq!(visible_location_name("/", &[]), "/");
    }
}
