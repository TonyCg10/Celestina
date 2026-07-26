//! `org.celestina.Devices1` — the suite's first internal contract.
//!
//! Magnetita holds the phone; Siderita wants to draw it. Rather than reach into
//! Magnetita's private state, Siderita reads this: a session-bus interface that
//! lists the connected devices and, for each, the one thing a file manager needs
//! — the **mount path** — plus name, type and state. The filesystem carries the
//! bytes; this contract carries what a directory listing cannot.
//!
//! It is deliberately small and versioned by its name (`…Devices1`). Each device
//! is a `a{sv}` dict so a key can be added without breaking a consumer, and a
//! `Changed` signal tells consumers to re-read [`list_devices`](Devices::list_devices)
//! rather than have them poll. Battery is carried now (as `-1`, unknown) so the
//! shape is stable before CP3 fills it.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use zbus::zvariant::{OwnedValue, Value};

/// The bus name Magnetita owns.
pub const BUS_NAME: &str = "org.celestina.Magnetita";
/// The object path the device list lives at.
pub const OBJECT_PATH: &str = "/org/celestina/Devices1";
/// The interface name.
pub const INTERFACE: &str = "org.celestina.Devices1";
/// The signal emitted when the device set or any device's state changes.
pub const CHANGED_SIGNAL: &str = "Changed";

/// One connected device, as the contract exposes it.
#[derive(Clone, Debug)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    /// "phone", "tablet", "laptop", "desktop", "tv", or "unknown".
    pub device_type: String,
    pub connected: bool,
    pub mounted: bool,
    /// The local path the device is mounted at, or empty when not mounted.
    pub mount_path: String,
    /// Battery percent, or -1 when unknown (until CP3 reports it).
    pub battery: i32,
}

impl DeviceEntry {
    /// The device as a `a{sv}` dict for the wire.
    fn to_dict(&self) -> HashMap<String, OwnedValue> {
        let fields = [
            ("id", Value::from(self.id.clone())),
            ("name", Value::from(self.name.clone())),
            ("type", Value::from(self.device_type.clone())),
            ("connected", Value::from(self.connected)),
            ("mounted", Value::from(self.mounted)),
            ("mountPath", Value::from(self.mount_path.clone())),
            ("battery", Value::from(self.battery)),
        ];
        fields
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("a basic value always converts"),
                )
            })
            .collect()
    }
}

/// The connected devices, keyed by id and shared between the daemon (which
/// writes) and the served interface (which reads).
pub type Registry = Arc<Mutex<BTreeMap<String, DeviceEntry>>>;

/// The object served at [`OBJECT_PATH`].
pub struct Devices {
    registry: Registry,
}

impl Devices {
    pub fn new(registry: Registry) -> Devices {
        Devices { registry }
    }
}

#[zbus::interface(name = "org.celestina.Devices1")]
impl Devices {
    /// The connected devices, each a dict with keys `id`, `name`, `type`,
    /// `connected`, `mounted`, `mountPath`, `battery`.
    fn list_devices(&self) -> Vec<HashMap<String, OwnedValue>> {
        self.registry
            .lock()
            .unwrap()
            .values()
            .map(DeviceEntry::to_dict)
            .collect()
    }
}
