//! Removable-volume discovery and mount / unmount via UDisks2 on the system bus.
//!
//! The listing is read-only and safe to call anytime; mount and unmount act on
//! real devices and may prompt for authorization through polkit. Everything is
//! delegated to `org.freedesktop.UDisks2` — the desktop's own volume daemon —
//! rather than touching `/proc/mounts` or `mount(8)` directly.

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

const UDISKS: &str = "org.freedesktop.UDisks2";
const IFACE_BLOCK: &str = "org.freedesktop.UDisks2.Block";
const IFACE_FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const IFACE_DRIVE: &str = "org.freedesktop.UDisks2.Drive";

/// A removable filesystem UDisks2 knows about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Volume {
    /// The UDisks2 block object path — the handle for mount / unmount.
    pub object_path: String,
    /// A label if the filesystem has one, else the device node's base name.
    pub name: String,
    /// The device node, e.g. `/dev/sdb1`.
    pub device: String,
    /// Where it is mounted, or empty when it is not mounted.
    pub mount_point: String,
}

/// Lists the mountable removable filesystems UDisks2 reports. Read-only.
pub fn list_volumes() -> Result<Vec<Volume>, String> {
    let connection =
        Connection::system().map_err(|error| format!("UDisks2 no disponible: {error}"))?;

    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        &connection,
        UDISKS,
        "/org/freedesktop/UDisks2",
    )
    .map_err(|error| format!("UDisks2 no disponible: {error}"))?;

    let objects = manager
        .get_managed_objects()
        .map_err(|error| format!("No se pudieron enumerar los volúmenes: {error}"))?;

    let mut volumes = Vec::new();
    for (path, interfaces) in &objects {
        // Only objects that are a mountable filesystem block.
        if !interfaces.contains_key(IFACE_FILESYSTEM) || !interfaces.contains_key(IFACE_BLOCK) {
            continue;
        }
        let path = path.as_str();
        let Ok(block) = Proxy::new(&connection, UDISKS, path, IFACE_BLOCK) else {
            continue;
        };

        // Skip system disks and anything UDisks2 hints we should ignore.
        if block.get_property::<bool>("HintSystem").unwrap_or(false)
            || block.get_property::<bool>("HintIgnore").unwrap_or(false)
        {
            continue;
        }

        // The backing drive must be removable (USB stick, SD card, optical…).
        let drive_path = block
            .get_property::<zbus::zvariant::OwnedObjectPath>("Drive")
            .map(|drive| drive.as_str().to_owned())
            .unwrap_or_default();
        if !drive_is_removable(&connection, &drive_path) {
            continue;
        }

        let device = block
            .get_property::<Vec<u8>>("Device")
            .map(|bytes| c_string(&bytes))
            .unwrap_or_default();
        let label = block.get_property::<String>("IdLabel").unwrap_or_default();

        let filesystem = Proxy::new(&connection, UDISKS, path, IFACE_FILESYSTEM)
            .map_err(|error| format!("UDisks2: {error}"))?;
        let mount_point = filesystem
            .get_property::<Vec<Vec<u8>>>("MountPoints")
            .ok()
            .and_then(|points| points.into_iter().next())
            .map(|bytes| c_string(&bytes))
            .unwrap_or_default();

        volumes.push(Volume {
            object_path: path.to_owned(),
            name: display_name(&label, &device),
            device,
            mount_point,
        });
    }

    volumes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(volumes)
}

/// Blocks, invoking `on_change` whenever UDisks2 reports a device added or
/// removed — a hotplug — so the caller can reload the list. Meant to run on a
/// worker thread; returns only on a fatal bus error. Plugging one drive exposes
/// several interfaces at once, so a burst is coalesced (300 ms quiet window)
/// into a single `on_change` rather than a storm of reloads.
pub fn watch_changes<F: Fn() + Send + 'static>(on_change: F) -> Result<(), String> {
    let connection =
        Connection::system().map_err(|error| format!("UDisks2 no disponible: {error}"))?;
    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        &connection,
        UDISKS,
        "/org/freedesktop/UDisks2",
    )
    .map_err(|error| format!("UDisks2 no disponible: {error}"))?;

    let added = manager
        .receive_interfaces_added()
        .map_err(|error| format!("UDisks2: {error}"))?;
    let removed = manager
        .receive_interfaces_removed()
        .map_err(|error| format!("UDisks2: {error}"))?;

    // One feeder thread per signal pushes a tick into a coalescing channel; the
    // signal payloads are irrelevant — any add/remove means "re-enumerate".
    let (tx, rx) = mpsc::channel::<()>();
    let tx_removed = tx.clone();
    std::thread::spawn(move || {
        for _ in added {
            if tx.send(()).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        for _ in removed {
            if tx_removed.send(()).is_err() {
                break;
            }
        }
    });

    while rx.recv().is_ok() {
        // Drain the rest of the burst, then reload once it settles.
        while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
        on_change();
    }
    Ok(())
}

/// Mounts the volume at `object_path`, returning its mount point. May prompt for
/// authorization via polkit.
pub fn mount(object_path: &str) -> Result<String, String> {
    let connection =
        Connection::system().map_err(|error| format!("UDisks2 no disponible: {error}"))?;
    let filesystem = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let options: HashMap<&str, Value> = HashMap::new();
    filesystem
        .call::<_, _, String>("Mount", &(options,))
        .map_err(udisks_error)
}

/// Unmounts the volume at `object_path`. May prompt for authorization.
pub fn unmount(object_path: &str) -> Result<(), String> {
    let connection =
        Connection::system().map_err(|error| format!("UDisks2 no disponible: {error}"))?;
    let filesystem = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let options: HashMap<&str, Value> = HashMap::new();
    filesystem
        .call::<_, _, ()>("Unmount", &(options,))
        .map_err(udisks_error)
}

fn drive_is_removable(connection: &Connection, drive_path: &str) -> bool {
    if drive_path.is_empty() || drive_path == "/" {
        return false;
    }
    let Ok(drive) = Proxy::new(connection, UDISKS, drive_path, IFACE_DRIVE) else {
        return false;
    };
    drive.get_property::<bool>("Removable").unwrap_or(false)
        || drive
            .get_property::<bool>("MediaRemovable")
            .unwrap_or(false)
}

/// Turns a UDisks2 D-Bus error into a short user-facing message, unwrapping the
/// polkit "not authorized" case into something readable.
fn udisks_error(error: zbus::Error) -> String {
    let text = error.to_string();
    if text.contains("NotAuthorized") {
        "No autorizado para montar o desmontar el volumen".to_owned()
    } else {
        format!("UDisks2: {text}")
    }
}

/// Decodes a UDisks2 NUL-terminated C string (device node / mount path).
fn c_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// A volume's display name: its filesystem label, or the device's base name when
/// it has none (e.g. `sdb1`).
fn display_name(label: &str, device: &str) -> String {
    if !label.is_empty() {
        return label.to_owned();
    }
    device
        .rsplit('/')
        .next()
        .filter(|base| !base.is_empty())
        .unwrap_or(device)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{c_string, display_name};

    #[test]
    fn c_string_stops_at_the_nul() {
        assert_eq!(c_string(b"/dev/sdb1\0\0"), "/dev/sdb1");
        assert_eq!(c_string(b"/mnt/usb"), "/mnt/usb");
    }

    #[test]
    fn display_name_prefers_the_label() {
        assert_eq!(display_name("MI USB", "/dev/sdb1"), "MI USB");
    }

    #[test]
    fn display_name_falls_back_to_the_device_base() {
        assert_eq!(display_name("", "/dev/sdb1"), "sdb1");
        assert_eq!(display_name("", "sdc"), "sdc");
    }
}
