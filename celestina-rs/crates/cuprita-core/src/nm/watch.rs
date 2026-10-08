//! The change watcher: one subscription to NetworkManager's signals, filtered
//! to the ones that change a snapshot and coalesced into one call per burst
//! by the shared watcher (`bus::watch_signals`).

use super::{IFACE_PROPERTIES, NM, NM_PATH};
use crate::bus::{self, signal_header};
use crate::error::NetworkError;

pub use crate::bus::WatchHandle;

/// Whether a NetworkManager signal can change what a snapshot shows: the
/// manager's and the devices' properties (Wi-Fi switch, device and active
/// states, the access-point list), devices and access points coming and
/// going, active connections changing state, and saved profiles added or
/// removed. Access points' own property changes (strength every scan) are
/// left out; their signal bars refresh with the next change that counts.
pub(super) fn relevant(path: &str, interface: &str, member: &str) -> bool {
    match member {
        "DeviceAdded" | "DeviceRemoved" | "AccessPointAdded" | "AccessPointRemoved"
        | "NewConnection" | "ConnectionRemoved" => true,
        "PropertiesChanged" if interface == IFACE_PROPERTIES => {
            path == NM_PATH
                || path.starts_with("/org/freedesktop/NetworkManager/Devices/")
                || path.starts_with("/org/freedesktop/NetworkManager/ActiveConnection/")
        }
        "StateChanged" => path.starts_with("/org/freedesktop/NetworkManager/ActiveConnection/"),
        _ => false,
    }
}

/// Calls `on_change` from a thread of its own whenever NetworkManager reports
/// a change that matters (see `relevant`), once per burst: signals arriving
/// within 300 ms of each other are coalesced into one call, made at most
/// 1.5 s after the burst's first signal.
pub fn watch(on_change: impl Fn() + Send + 'static) -> Result<WatchHandle, NetworkError> {
    bus::watch_signals(
        NM,
        "nm",
        |message| {
            let (path, interface, member) = signal_header(message);
            relevant(&path, &interface, &member)
        },
        on_change,
    )
    .map_err(NetworkError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nm::IFACE_WIRELESS;

    #[test]
    fn only_snapshot_changing_signals_count() {
        assert!(relevant(NM_PATH, IFACE_PROPERTIES, "PropertiesChanged"));
        assert!(relevant(
            "/org/freedesktop/NetworkManager/Devices/22",
            IFACE_PROPERTIES,
            "PropertiesChanged"
        ));
        assert!(relevant(
            "/org/freedesktop/NetworkManager/Devices/22",
            IFACE_WIRELESS,
            "AccessPointAdded"
        ));
        assert!(!relevant(
            "/org/freedesktop/NetworkManager/AccessPoint/3054",
            IFACE_PROPERTIES,
            "PropertiesChanged"
        ));
    }
}
