//! The BlueZ client: `Bluetooth` over BlueZ's D-Bus API on the system bus, the
//! pairing agent Cuprita exports (`agent`), and a watcher that reports
//! BlueZ's changes (`watch`). Every call blocks, so all three belong on worker
//! threads, never on the Qt thread.
//!
//! Reading is unprivileged. BlueZ's own D-Bus policy decides who may pair,
//! connect or forget; a refusal comes back as `BluetoothError::Denied`.

mod agent;
mod watch;

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::bluetooth::Bluetooth;
use crate::bus;
use crate::error::BluetoothError;
use crate::model::{BluetoothDevice, BluetoothSnapshot, DeviceKind};
use crate::order::order_devices;

pub use agent::{serve_agent, AgentHandle, AGENT_PATH};
pub use watch::{watch, WatchHandle};

const BLUEZ: &str = "org.bluez";
const IFACE_ADAPTER: &str = "org.bluez.Adapter1";
const IFACE_DEVICE: &str = "org.bluez.Device1";
const IFACE_BATTERY: &str = "org.bluez.Battery1";
const IFACE_AGENT_MANAGER: &str = "org.bluez.AgentManager1";
const IFACE_OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const AGENT_MANAGER_PATH: &str = "/org/bluez";
/// The longest any BlueZ call may take on the client's connection. `Pair()`
/// returns only when the pairing ends, and BlueZ waits about a minute for
/// each agent answer; 90 s leaves a slow PIN entry room, while a BlueZ that
/// stopped answering still frees the worker. zbus sets this per connection
/// (`connection::Builder::method_timeout`), so every call shares it.
const CALL_TIMEOUT: Duration = Duration::from_secs(90);

/// One object's interfaces and their properties.
type Interfaces = HashMap<String, Props>;
type Props = HashMap<String, OwnedValue>;
type ManagedObjects = HashMap<OwnedObjectPath, Interfaces>;

/// The device a `pair` call is pairing right now, by object path. The agent
/// authorises it without asking: the person already chose it.
static PAIRING: Mutex<Option<String>> = Mutex::new(None);

fn set_pairing(path: Option<String>) {
    *PAIRING.lock().unwrap_or_else(PoisonError::into_inner) = path;
}

fn is_pairing(path: &str) -> bool {
    PAIRING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_deref()
        == Some(path)
}

/// A device's freedesktop icon name as its kind.
#[must_use]
pub fn icon_to_kind(icon: &str) -> DeviceKind {
    match icon {
        "audio-headset" | "audio-headphones" | "audio-card" => DeviceKind::Audio,
        "phone" => DeviceKind::Phone,
        "computer" => DeviceKind::Computer,
        _ if icon.starts_with("input-") => DeviceKind::Input,
        _ => DeviceKind::Other,
    }
}

fn text(props: &Props, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| String::try_from(v.clone()).ok())
}

fn flag(props: &Props, key: &str) -> bool {
    props
        .get(key)
        .and_then(|v| bool::try_from(v.clone()).ok())
        .unwrap_or(false)
}

/// One `Device1`'s properties (and its `Battery1` percentage, when it has
/// one) as a device row. The name is BlueZ's `Alias`: the person's rename,
/// else the device's own name, else its address.
#[must_use]
pub fn device_from_props(props: &Props, battery: Option<u8>) -> BluetoothDevice {
    let address = text(props, "Address").unwrap_or_default();
    let name = text(props, "Alias")
        .or_else(|| text(props, "Name"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| address.clone());
    BluetoothDevice {
        name,
        kind: icon_to_kind(&text(props, "Icon").unwrap_or_default()),
        paired: flag(props, "Paired"),
        connected: flag(props, "Connected"),
        trusted: flag(props, "Trusted"),
        battery: battery.map(|b| b.min(100)),
        address,
    }
}

/// A device seen during a search that never said its name: a beacon or a
/// stranger's gadget. Listed only once it is paired.
#[must_use]
pub fn anonymous(props: &Props) -> bool {
    !flag(props, "Paired") && text(props, "Name").is_none_or(|n| n.is_empty())
}

/// The object path BlueZ gives the device `address` under `adapter`
/// (`…/hci0/dev_AA_BB_CC_DD_EE_FF`); `None` for anything but six hex pairs.
#[must_use]
pub fn device_path(adapter: &str, address: &str) -> Option<String> {
    let parts: Vec<&str> = address.split(':').collect();
    let valid = parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()));
    valid.then(|| format!("{adapter}/dev_{}", parts.join("_").to_ascii_uppercase()))
}

/// The error names BlueZ answers a cancelled or refused pairing with.
fn pairing_refused(name: &str) -> bool {
    name.ends_with(".AuthenticationCanceled") || name.ends_with(".AuthenticationRejected")
}

/// A D-Bus failure as the section's error. BlueZ's `NotReady` means the
/// adapter is off; a refused pairing reads as cancelled.
fn map_error(error: zbus::Error) -> BluetoothError {
    match bus::error_name(&error) {
        Some((name, _)) if pairing_refused(&name) => BluetoothError::pairing_cancelled(),
        Some((name, _)) if name.ends_with(".NotReady") => BluetoothError::Unavailable,
        _ => bus::fault(error).into(),
    }
}

fn map_fdo(error: zbus::fdo::Error) -> BluetoothError {
    map_error(zbus::Error::FDO(Box::new(error)))
}

/// A command's result, with the BlueZ errors `harmless` accepts read as done
/// (`AlreadyConnected` for a connect, say).
fn tolerate(
    result: zbus::Result<()>,
    harmless: impl Fn(&str, Option<&str>) -> bool,
) -> Result<(), BluetoothError> {
    match result {
        Ok(()) => Ok(()),
        Err(error) => match bus::error_name(&error) {
            Some((name, detail)) if harmless(&name, detail.as_deref()) => Ok(()),
            _ => Err(map_error(error)),
        },
    }
}

fn proxy<'a>(
    connection: &Connection,
    path: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>, BluetoothError> {
    zbus::blocking::proxy::Builder::new(connection)
        .destination(BLUEZ)
        .and_then(|b| b.path(path))
        .and_then(|b| b.interface(interface))
        .map(|b| b.cache_properties(CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(map_error)
}

/// The `Bluetooth` backend over BlueZ. It drives the first adapter BlueZ
/// lists, which is the only one on the machines Cuprita targets.
pub struct BluezBluetooth {
    connection: Connection,
    /// The adapter's object path, as of the last snapshot.
    adapter: Option<String>,
}

impl BluezBluetooth {
    /// Connects to the system bus. BlueZ itself is reached lazily: a missing
    /// service reports `Unavailable` on the first read.
    pub fn connect_system() -> Result<Self, BluetoothError> {
        let connection = zbus::blocking::connection::Builder::system()
            .map(|b| b.method_timeout(CALL_TIMEOUT))
            .and_then(|b| b.build())
            .map_err(map_error)?;
        Ok(Self {
            connection,
            adapter: None,
        })
    }

    fn managed_objects(&self) -> Result<ManagedObjects, BluetoothError> {
        proxy(&self.connection, "/", IFACE_OBJECT_MANAGER)?
            .call("GetManagedObjects", &())
            .map_err(map_error)
    }

    /// The adapter's path: the one the last snapshot found, or a fresh look.
    fn adapter(&mut self) -> Result<String, BluetoothError> {
        if let Some(adapter) = &self.adapter {
            return Ok(adapter.clone());
        }
        let objects = self.managed_objects()?;
        let adapter = first_adapter(&objects)
            .map(|(path, _)| path.to_owned())
            .ok_or(BluetoothError::Unavailable)?;
        self.adapter = Some(adapter.clone());
        Ok(adapter)
    }

    fn device(&mut self, address: &str) -> Result<String, BluetoothError> {
        let adapter = self.adapter()?;
        device_path(&adapter, address).ok_or_else(|| BluetoothError::NotFound(address.to_owned()))
    }

    fn call_device(
        &mut self,
        address: &str,
        method: &str,
    ) -> Result<zbus::Result<()>, BluetoothError> {
        let path = self.device(address)?;
        let proxy = proxy(&self.connection, &path, IFACE_DEVICE)?;
        Ok(proxy.call::<_, _, ()>(method, &()))
    }
}

/// Abandons the pairing under way with `address` (`Device1.CancelPairing`),
/// on a connection of its own: the client's worker is blocked inside that
/// pairing's `Pair()`. Blocking: call it off the Qt thread.
pub fn cancel_pairing(address: &str) -> Result<(), BluetoothError> {
    let connection = Connection::system().map_err(map_error)?;
    let objects: ManagedObjects = proxy(&connection, "/", IFACE_OBJECT_MANAGER)?
        .call("GetManagedObjects", &())
        .map_err(map_error)?;
    let adapter = first_adapter(&objects)
        .map(|(path, _)| path.to_owned())
        .ok_or(BluetoothError::Unavailable)?;
    let path = device_path(&adapter, address)
        .ok_or_else(|| BluetoothError::NotFound(address.to_owned()))?;
    let device = proxy(&connection, &path, IFACE_DEVICE)?;
    tolerate(device.call("CancelPairing", &()), |name, _| {
        name.ends_with(".DoesNotExist")
    })
}

/// The adapter with the lowest path (`hci0` before `hci1`) and its properties.
fn first_adapter(objects: &ManagedObjects) -> Option<(&str, &Props)> {
    objects
        .iter()
        .filter_map(|(path, interfaces)| {
            interfaces
                .get(IFACE_ADAPTER)
                .map(|props| (path.as_str(), props))
        })
        .min_by_key(|(path, _)| *path)
}

/// Whether `path` is a device under `adapter`.
fn under(adapter: &str, path: &str) -> bool {
    path.strip_prefix(adapter)
        .is_some_and(|rest| rest.starts_with("/dev_") && !rest[1..].contains('/'))
}

impl Bluetooth for BluezBluetooth {
    fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError> {
        let objects = self.managed_objects()?;
        let Some((adapter, props)) = first_adapter(&objects) else {
            self.adapter = None;
            return Err(BluetoothError::Unavailable);
        };
        self.adapter = Some(adapter.to_owned());
        let powered = flag(props, "Powered");
        let discovering = flag(props, "Discovering");
        // As with the fake: an adapter that is off lists nothing.
        let mut devices: Vec<BluetoothDevice> = if powered {
            objects
                .iter()
                .filter(|(path, _)| under(adapter, path.as_str()))
                .filter_map(|(_, interfaces)| {
                    let props = interfaces.get(IFACE_DEVICE)?;
                    if anonymous(props) {
                        return None;
                    }
                    let battery = interfaces
                        .get(IFACE_BATTERY)
                        .and_then(|b| b.get("Percentage"))
                        .and_then(|v| u8::try_from(v.clone()).ok());
                    Some(device_from_props(props, battery))
                })
                .collect()
        } else {
            Vec::new()
        };
        order_devices(&mut devices);
        Ok(BluetoothSnapshot {
            powered,
            discovering,
            devices,
        })
    }

    fn set_powered(&mut self, on: bool) -> Result<(), BluetoothError> {
        let adapter = self.adapter()?;
        let adapter = proxy(&self.connection, &adapter, IFACE_ADAPTER)?;
        adapter.set_property("Powered", on).map_err(map_fdo)
    }

    /// A search belongs to the connection that started it: BlueZ ends it when
    /// Cuprita exits. Starting one already under way, or stopping one that
    /// Cuprita did not start, is not an error.
    fn set_discovering(&mut self, on: bool) -> Result<(), BluetoothError> {
        let adapter = self.adapter()?;
        let proxy = proxy(&self.connection, &adapter, IFACE_ADAPTER)?;
        if on {
            tolerate(proxy.call("StartDiscovery", &()), |name, _| {
                name.ends_with(".InProgress")
            })
        } else {
            tolerate(proxy.call("StopDiscovery", &()), |name, detail| {
                name.ends_with(".Failed")
                    && detail.is_some_and(|d| d.contains("No discovery started"))
            })
        }
    }

    /// Pairs, asking through the agent as BlueZ needs, then trusts the device
    /// so it reconnects without asking again.
    fn pair(&mut self, address: &str) -> Result<(), BluetoothError> {
        let path = self.device(address)?;
        let device = proxy(&self.connection, &path, IFACE_DEVICE)?;
        set_pairing(Some(path.clone()));
        let paired = tolerate(device.call("Pair", &()), |name, _| {
            name.ends_with(".AlreadyExists")
        });
        set_pairing(None);
        paired?;
        // Paired already; a trust that fails only means a later reconnect
        // asks the agent again.
        if let Err(error) = device.set_property("Trusted", true) {
            eprintln!("Cuprita: the device {path} could not be trusted: {error}");
        }
        Ok(())
    }

    fn connect(&mut self, address: &str) -> Result<(), BluetoothError> {
        let result = self.call_device(address, "Connect")?;
        tolerate(result, |name, _| name.ends_with(".AlreadyConnected"))
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BluetoothError> {
        let result = self.call_device(address, "Disconnect")?;
        tolerate(result, |name, _| name.ends_with(".NotConnected"))
    }

    fn forget(&mut self, address: &str) -> Result<(), BluetoothError> {
        let path = self.device(address)?;
        let adapter = self.adapter()?;
        let device = OwnedObjectPath::try_from(path.as_str())
            .map_err(|e| BluetoothError::Failed(e.to_string()))?;
        let adapter = proxy(&self.connection, &adapter, IFACE_ADAPTER)?;
        let removed = adapter.call::<_, _, ()>("RemoveDevice", &(device,));
        removed.map_err(|error| match bus::error_name(&error) {
            Some((name, _)) if name.ends_with(".DoesNotExist") => {
                BluetoothError::NotFound(address.to_owned())
            }
            _ => map_error(error),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn value<'a>(v: impl Into<Value<'a>>) -> OwnedValue {
        v.into().try_to_owned().unwrap()
    }

    fn props(pairs: &[(&str, OwnedValue)]) -> Props {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.try_clone().unwrap()))
            .collect()
    }

    #[test]
    fn icons_map_to_kinds() {
        assert_eq!(icon_to_kind("audio-headset"), DeviceKind::Audio);
        assert_eq!(icon_to_kind("audio-headphones"), DeviceKind::Audio);
        assert_eq!(icon_to_kind("audio-card"), DeviceKind::Audio);
        assert_eq!(icon_to_kind("input-gaming"), DeviceKind::Input);
        assert_eq!(icon_to_kind("input-keyboard"), DeviceKind::Input);
        assert_eq!(icon_to_kind("phone"), DeviceKind::Phone);
        assert_eq!(icon_to_kind("computer"), DeviceKind::Computer);
        assert_eq!(icon_to_kind("camera-video"), DeviceKind::Other);
        assert_eq!(icon_to_kind(""), DeviceKind::Other);
    }

    #[test]
    fn a_paired_headset_becomes_a_row() {
        let p = props(&[
            ("Address", value("AA:BB:CC:00:00:01")),
            ("Name", value("WH-1000XM4")),
            ("Alias", value("Auriculares")),
            ("Icon", value("audio-headset")),
            ("Paired", value(true)),
            ("Connected", value(true)),
            ("Trusted", value(true)),
        ]);
        let device = device_from_props(&p, Some(120));
        assert_eq!(
            device,
            BluetoothDevice {
                address: "AA:BB:CC:00:00:01".to_owned(),
                name: "Auriculares".to_owned(),
                kind: DeviceKind::Audio,
                paired: true,
                connected: true,
                trusted: true,
                battery: Some(100),
            }
        );
        assert!(!anonymous(&p));
    }

    #[test]
    fn a_nameless_stranger_is_anonymous_and_falls_back_to_its_address() {
        let p = props(&[("Address", value("AA:BB:CC:00:00:09"))]);
        let device = device_from_props(&p, None);
        assert_eq!(device.name, "AA:BB:CC:00:00:09");
        assert_eq!(device.kind, DeviceKind::Other);
        assert!(!device.paired && !device.connected && !device.trusted);
        assert_eq!(device.battery, None);
        assert!(anonymous(&p));
    }

    #[test]
    fn device_paths_follow_bluez_naming() {
        assert_eq!(
            device_path("/org/bluez/hci0", "aa:bb:cc:00:00:01").as_deref(),
            Some("/org/bluez/hci0/dev_AA_BB_CC_00_00_01")
        );
        assert_eq!(device_path("/org/bluez/hci0", "AA:BB:CC:00:00"), None);
        assert_eq!(device_path("/org/bluez/hci0", "AA:BB:CC:00:00:0G"), None);
        assert_eq!(device_path("/org/bluez/hci0", "../../x"), None);
    }

    #[test]
    fn only_the_adapters_own_devices_count() {
        assert!(under(
            "/org/bluez/hci0",
            "/org/bluez/hci0/dev_AA_BB_CC_00_00_01"
        ));
        assert!(!under("/org/bluez/hci0", "/org/bluez/hci0"));
        assert!(!under(
            "/org/bluez/hci0",
            "/org/bluez/hci1/dev_AA_BB_CC_00_00_01"
        ));
        assert!(!under(
            "/org/bluez/hci0",
            "/org/bluez/hci0/dev_AA_BB_CC_00_00_01/sep1"
        ));
    }

    #[test]
    fn bluez_errors_map_to_the_section_errors() {
        let error = |name: &str| {
            zbus::Error::MethodError(
                zbus::names::OwnedErrorName::try_from(name).unwrap(),
                Some("detail".to_owned()),
                zbus::message::Message::method_call("/", "Ping")
                    .unwrap()
                    .build(&())
                    .unwrap(),
            )
        };
        assert_eq!(
            map_error(error("org.bluez.Error.AuthenticationCanceled")),
            BluetoothError::pairing_cancelled()
        );
        assert_eq!(
            map_error(error("org.bluez.Error.AuthenticationRejected")),
            BluetoothError::pairing_cancelled()
        );
        assert_eq!(
            map_error(error("org.bluez.Error.NotReady")),
            BluetoothError::Unavailable
        );
        assert_eq!(
            map_error(error("org.bluez.Error.Failed")),
            BluetoothError::Failed("detail".to_owned())
        );
        assert_eq!(
            tolerate(Err(error("org.bluez.Error.AlreadyConnected")), |name, _| {
                name.ends_with(".AlreadyConnected")
            }),
            Ok(())
        );
        assert_eq!(
            tolerate(Err(error("org.bluez.Error.Failed")), |name, _| {
                name.ends_with(".AlreadyConnected")
            }),
            Err(BluetoothError::Failed("detail".to_owned()))
        );
    }
}
