//! Removable-volume discovery and mount / unmount via UDisks2 on the system bus.
//!
//! The listing is read-only and safe to call anytime; mount and unmount act on
//! real devices and may prompt for authorization through polkit. Everything is
//! delegated to `org.freedesktop.UDisks2` — the desktop's own volume daemon —
//! rather than touching `/proc/mounts` or `mount(8)` directly.

use std::collections::HashMap;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

pub(crate) const UDISKS: &str = "org.freedesktop.UDisks2";
pub(crate) const IFACE_BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub(crate) const IFACE_FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
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
    /// The filesystem label, empty when it has none.
    pub label: String,
    /// The filesystem type as UDisks2 names it (`vfat`, `exfat`, `ext4`…).
    pub fs_type: String,
    /// The block's size in bytes.
    pub size: u64,
    /// Whether its drive is part of the running system (see
    /// [`crate::drives::backs_system`]): never renamed or formatted.
    pub system: bool,
}

/// Lists the mountable removable filesystems UDisks2 reports, over the
/// connection [`crate::devicemodel`] keeps. Read-only, and blocking: never on
/// the Qt thread.
pub fn list_volumes(connection: &Connection) -> Result<Vec<Volume>, String> {
    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        connection,
        UDISKS,
        "/org/freedesktop/UDisks2",
    )
    .map_err(|error| format!("UDisks2 no disponible: {error}"))?;

    let objects = manager
        .get_managed_objects()
        .map_err(|error| format!("No se pudieron enumerar los volúmenes: {error}"))?;

    // One look at the whole object tree answers the system-drive question for
    // every volume in this listing.
    let is_system = crate::drives::system_check(connection);
    let mut volumes = Vec::new();
    for (path, interfaces) in &objects {
        // Only objects that are a mountable filesystem block.
        if !interfaces.contains_key(IFACE_FILESYSTEM) || !interfaces.contains_key(IFACE_BLOCK) {
            continue;
        }
        let path = path.as_str();
        let Ok(block) = Proxy::new(connection, UDISKS, path, IFACE_BLOCK) else {
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
        if !drive_is_removable(connection, &drive_path) {
            continue;
        }

        let device = block
            .get_property::<Vec<u8>>("Device")
            .map(|bytes| c_string(&bytes))
            .unwrap_or_default();
        let label = block.get_property::<String>("IdLabel").unwrap_or_default();

        let filesystem = Proxy::new(connection, UDISKS, path, IFACE_FILESYSTEM)
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
            fs_type: block.get_property::<String>("IdType").unwrap_or_default(),
            size: block.get_property::<u64>("Size").unwrap_or(0),
            system: is_system(path),
            label,
        });
    }

    volumes.sort_by_key(|v| v.name.to_lowercase());
    Ok(volumes)
}

/// Calls `on_change` whenever UDisks2 reports a device added or removed — a
/// hotplug — from threads of its own, for as long as the connection lives.
/// Plugging one drive exposes several interfaces at once; the caller coalesces
/// the burst.
pub fn forward_changes<F: Fn() + Send + Sync + 'static>(
    connection: &Connection,
    on_change: F,
) -> Result<(), String> {
    let manager = zbus::blocking::fdo::ObjectManagerProxy::new(
        connection,
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

    // One feeder thread per signal; the payloads are irrelevant — any add or
    // remove means "re-enumerate".
    let on_change = std::sync::Arc::new(on_change);
    let on_removed = std::sync::Arc::clone(&on_change);
    std::thread::Builder::new()
        .name("siderita-udisks-signal".to_owned())
        .spawn(move || {
            for _ in added {
                on_change();
            }
        })
        .map_err(|error| format!("UDisks2: {error}"))?;
    std::thread::Builder::new()
        .name("siderita-udisks-signal".to_owned())
        .spawn(move || {
            for _ in removed {
                on_removed();
            }
        })
        .map_err(|error| format!("UDisks2: {error}"))?;
    Ok(())
}

/// A connection to the system bus, where UDisks2 lives.
pub fn system_bus() -> Result<Connection, String> {
    Connection::system().map_err(|error| format!("UDisks2 no disponible: {error}"))
}

/// Mounts the volume at `object_path`, returning its mount point. May prompt for
/// authorization via polkit.
pub fn mount(object_path: &str) -> Result<String, String> {
    let connection = system_bus()?;
    let filesystem = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let options: HashMap<&str, Value> = HashMap::new();
    filesystem
        .call::<_, _, String>("Mount", &(options,))
        .map_err(udisks_error)
}

/// Unmounts the volume at `object_path`. May prompt for authorization.
pub fn unmount(object_path: &str) -> Result<(), String> {
    let connection = system_bus()?;
    let filesystem = Proxy::new(&connection, UDISKS, object_path, IFACE_FILESYSTEM)
        .map_err(|error| format!("UDisks2: {error}"))?;
    let options: HashMap<&str, Value> = HashMap::new();
    filesystem
        .call::<_, _, ()>("Unmount", &(options,))
        .map_err(udisks_error)
}

pub(crate) fn drive_is_removable(connection: &Connection, drive_path: &str) -> bool {
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
pub(crate) fn c_string(bytes: &[u8]) -> String {
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
