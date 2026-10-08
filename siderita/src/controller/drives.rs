//! language-contract: product-copy
//!
//! Renaming and formatting removable volumes from the sidebar. The work is
//! UDisks2's (`crate::drives`) and can wait on a polkit prompt or on a disk
//! being overwritten for minutes, so it runs on a worker thread and reports
//! back on the Qt thread through the notice queue and `op_error`, exactly as
//! unmounting does.
//!
//! A volume is named by its device node, never by its row: the listing is
//! sorted by name and rebuilt on every hotplug, so a position taken when a menu
//! opened can point at another drive by the time the person confirms. The
//! device is looked up when the call arrives, and `crate::drives` checks again
//! on the worker that the UDisks2 object still is that device, that it is
//! removable and that it is not part of the running system.

use core::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use super::notices::NoticeTone;
use super::qobject;
use crate::volumes::Volume;

/// A change to a removable volume, as the worker runs it.
enum DriveWork {
    Rename(String),
    Format {
        fstype: String,
        label: String,
        quick: bool,
        whole_disk: bool,
    },
}

/// The listed volume whose device node is `device`, wherever it sorts now.
fn find_volume<'a>(volumes: &'a [Volume], device: &str) -> Option<&'a Volume> {
    if device.is_empty() {
        return None;
    }
    volumes.iter().find(|volume| volume.device == device)
}

impl qobject::SideritaController {
    /// Renames the volume whose device node is `device` to `label` (FAT labels
    /// in capitals). An invalid label is reported in `op_error` and nothing
    /// runs; renaming to the current label is a no-op.
    pub fn rename_volume(self: Pin<&mut Self>, device: &QString, label: &QString) {
        self.run_drive_work(&device.to_string(), DriveWork::Rename(label.to_string()));
    }

    /// Formats the volume whose device node is `device` as `fs` — or, with
    /// `whole_disk`, wipes its whole drive into one new partition. `quick`
    /// false overwrites with zeros.
    pub fn format_volume(
        self: Pin<&mut Self>,
        device: &QString,
        fs: &QString,
        label: &QString,
        quick: bool,
        whole_disk: bool,
    ) {
        self.run_drive_work(
            &device.to_string(),
            DriveWork::Format {
                fstype: fs.to_string(),
                label: label.to_string(),
                quick,
                whole_disk,
            },
        );
    }

    /// Why `label` cannot name a `fs` filesystem once normalized; empty when it
    /// can.
    pub fn label_error(&self, fs: &QString, label: &QString) -> QString {
        crate::drives::label_error(&fs.to_string(), &label.to_string())
            .map(|error| QString::from(error.as_str()))
            .unwrap_or_default()
    }

    fn run_drive_work(mut self: Pin<&mut Self>, device: &str, work: DriveWork) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(volume) = find_volume(&self.rust().volumes, device).cloned() else {
            self.as_mut()
                .set_op_error(QString::from("El dispositivo ya no está conectado"));
            return;
        };
        if volume.system {
            self.as_mut()
                .set_op_error(QString::from("Siderita no modifica los discos del sistema"));
            return;
        }
        let unknown = matches!(&work, DriveWork::Format { fstype, .. }
            if !crate::drives::FS_TYPES.contains(&fstype.as_str()));
        if unknown {
            self.as_mut()
                .set_op_error(QString::from("Sistema de archivos no admitido"));
            return;
        }
        let (fstype, label) = match &work {
            DriveWork::Rename(label) => (volume.fs_type.as_str(), label.as_str()),
            DriveWork::Format { fstype, label, .. } => (fstype.as_str(), label.as_str()),
        };
        if let Some(error) = crate::drives::label_error(fstype, label) {
            self.as_mut().set_op_error(QString::from(error.as_str()));
            return;
        }
        if matches!(&work, DriveWork::Rename(label)
            if crate::drives::normalize_label(&volume.fs_type, label) == volume.label)
        {
            return;
        }
        // Another tab may be renaming or formatting right now.
        let Some(claim) = crate::drives::claim() else {
            self.as_mut()
                .set_op_error(QString::from("Ya hay otra operación de disco en curso"));
            return;
        };

        let (running, done, icon) = match work {
            DriveWork::Rename(_) => ("Cambiando el nombre…", "Nombre cambiado", "pencil"),
            DriveWork::Format { .. } => ("Formateando…", "Disco formateado", "eraser"),
        };
        self.as_mut().set_volume_busy(true);
        let notice = self
            .as_mut()
            .push_notice(running, icon, NoticeTone::Info, true);

        let path = volume.object_path;
        let device = volume.device;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = match work {
                DriveWork::Rename(label) => crate::drives::set_label(&path, &device, &label),
                DriveWork::Format {
                    fstype,
                    label,
                    quick,
                    whole_disk: false,
                } => crate::drives::format_partition(&path, &device, &fstype, &label, quick),
                DriveWork::Format {
                    fstype,
                    label,
                    quick,
                    whole_disk: true,
                } => crate::drives::format_whole_disk(&path, &device, &fstype, &label, quick),
            };
            drop(claim);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                // Reloaded either way: a failed format can still have changed
                // the partition table or left the volume unmounted.
                crate::devicemodel::reload_volumes();
                match result {
                    Ok(()) => controller.as_mut().settle_notice(notice, done),
                    // As with unmounting, the failure speaks through
                    // `op_error`; the running notice just goes.
                    Err(error) => {
                        controller.as_mut().drop_notice(notice);
                        controller
                            .as_mut()
                            .set_op_error(QString::from(error.as_str()));
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::find_volume;
    use crate::volumes::Volume;

    fn volume(name: &str, device: &str) -> Volume {
        Volume {
            object_path: format!("/org/freedesktop/UDisks2/block_devices/{name}"),
            name: name.to_owned(),
            device: device.to_owned(),
            mount_point: String::new(),
            label: name.to_owned(),
            fs_type: "vfat".to_owned(),
            size: 0,
            system: false,
        }
    }

    #[test]
    fn a_volume_is_found_by_its_device_after_a_reorder() {
        let before = [volume("USB", "/dev/sdb1")];
        // A stick that sorts first is plugged in: the row moved, the device did not.
        let after = [volume("AAA", "/dev/sdc1"), volume("USB", "/dev/sdb1")];
        let chosen = find_volume(&before, "/dev/sdb1").map(|v| v.device.clone());
        assert_eq!(chosen.as_deref(), Some("/dev/sdb1"));
        let found = find_volume(&after, "/dev/sdb1").expect("still listed");
        assert_eq!(found.object_path, before[0].object_path);
    }

    #[test]
    fn a_volume_that_went_away_is_not_found() {
        let after = [volume("AAA", "/dev/sdc1")];
        assert!(find_volume(&after, "/dev/sdb1").is_none());
        assert!(find_volume(&after, "").is_none());
    }
}
