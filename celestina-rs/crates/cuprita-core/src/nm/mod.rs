//! The NetworkManager client: `Network` over NetworkManager's D-Bus API on the
//! system bus, plus a watcher that reports its changes. Every call blocks, so
//! both belong on a worker thread, never on the Qt thread.
//!
//! Reading is unprivileged. Activating, deactivating or deleting a connection
//! may ask polkit, which prompts in its own window; a refusal comes back as
//! `NetworkError::Denied`.

mod join;
mod watch;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use zbus::blocking::{Connection, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::DBusError;

use crate::error::NetworkError;
use crate::model::{Network as NetworkEntry, NetworkKind, NetworkSnapshot, NetworkState, Security};
use crate::network::Network;
use crate::order::order_networks;

pub use join::{join_outcome, JoinOutcome};
pub use watch::{watch, WatchHandle};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const IFACE_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const IFACE_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const IFACE_AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const IFACE_ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const IFACE_SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const IFACE_CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const IFACE_PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// `NM_DEVICE_TYPE_ETHERNET` and `NM_DEVICE_TYPE_WIFI`.
const DEVICE_ETHERNET: u32 = 1;
const DEVICE_WIFI: u32 = 2;
/// `NM_802_11_AP_SEC_KEY_MGMT_802_1X`.
const KEY_MGMT_802_1X: u32 = 0x200;
/// `NM_802_11_AP_FLAGS_PRIVACY`: WEP when no WPA or RSN flag is set.
const AP_PRIVACY: u32 = 0x1;
/// `NM_ACTIVE_CONNECTION_STATE_*`.
const ACTIVE_ACTIVATING: u32 = 1;
const ACTIVE_ACTIVATED: u32 = 2;
#[cfg(test)]
const ACTIVE_DEACTIVATING: u32 = 3;
const ACTIVE_DEACTIVATED: u32 = 4;
/// The empty object path NetworkManager uses for "none".
const NO_PATH: &str = "/";

/// Where the client's notices go: the controller turns them into the
/// window's notice pill. Called from the client's own threads.
pub type Notify = Arc<dyn Fn(String) + Send + Sync>;

/// The SSIDs whose last join failed; their rows read `Failed` until a join
/// succeeds. Shared with the threads that follow a join.
type FailedJoins = Arc<Mutex<HashSet<Vec<u8>>>>;

/// One access point as a network row. `known` says a saved connection carries
/// its SSID and `active` that the device is on it.
#[must_use]
pub fn ap_to_network(
    ssid: &[u8],
    strength: u8,
    flags: u32,
    wpa_flags: u32,
    rsn_flags: u32,
    known: bool,
    active: bool,
) -> NetworkEntry {
    let security = if rsn_flags & KEY_MGMT_802_1X != 0 || wpa_flags & KEY_MGMT_802_1X != 0 {
        Security::Enterprise
    } else if rsn_flags | wpa_flags != 0 || flags & AP_PRIVACY != 0 {
        // WEP reads as protected; `connect` refuses it (see `wep_only`).
        Security::Psk
    } else {
        Security::Open
    };
    NetworkEntry {
        id: wifi_id(ssid),
        kind: NetworkKind::Wifi,
        name: String::from_utf8_lossy(ssid).into_owned(),
        state: if active {
            NetworkState::Connected
        } else {
            NetworkState::Disconnected
        },
        signal: Some(strength.min(100)),
        security,
        known,
    }
}

/// An access point that offers only WEP: the privacy flag without any WPA or
/// RSN flag.
#[must_use]
pub fn wep_only(flags: u32, wpa_flags: u32, rsn_flags: u32) -> bool {
    flags & AP_PRIVACY != 0 && wpa_flags | rsn_flags == 0
}

/// A device's `NMDeviceState` as the row's state.
#[must_use]
pub fn device_state_to_state(state: u32) -> NetworkState {
    match state {
        100 => NetworkState::Connected,
        40..=90 => NetworkState::Connecting,
        120 => NetworkState::Failed,
        _ => NetworkState::Disconnected,
    }
}

/// A Wi-Fi row's state: from the active connection carrying its SSID when
/// there is one (`NMActiveConnectionState`), and `Failed` while its last join
/// failed and nothing newer is under way.
#[must_use]
pub fn wifi_row_state(active: Option<u32>, failed: bool) -> NetworkState {
    match active {
        Some(ACTIVE_ACTIVATING) => NetworkState::Connecting,
        Some(ACTIVE_ACTIVATED) => NetworkState::Connected,
        _ if failed => NetworkState::Failed,
        _ => NetworkState::Disconnected,
    }
}

/// An active connection's `NMActiveConnectionState` as a VPN row's state.
fn active_state_to_state(state: u32) -> NetworkState {
    wifi_row_state(Some(state), false)
}

/// A Wi-Fi row's id: the SSID's bytes in hex, so two SSIDs that decode to the
/// same text still make two rows.
fn wifi_id(ssid: &[u8]) -> String {
    let hex: String = ssid.iter().map(|b| format!("{b:02x}")).collect();
    format!("wifi:{hex}")
}

/// A D-Bus error name and its detail as the section's error. NetworkManager
/// names a polkit refusal `org.freedesktop.NetworkManager.PermissionDenied`.
fn map_error_name(name: &str, detail: Option<String>) -> NetworkError {
    if name.ends_with("PermissionDenied") || name.ends_with("AccessDenied") {
        NetworkError::Denied
    } else if name.ends_with("ServiceUnknown") || name.ends_with("NameHasNoOwner") {
        NetworkError::Unavailable
    } else {
        NetworkError::Failed(detail.unwrap_or_else(|| name.to_owned()))
    }
}

fn map_error(error: zbus::Error) -> NetworkError {
    match error {
        zbus::Error::FDO(inner) => map_fdo(*inner),
        zbus::Error::MethodError(name, detail, _) => map_error_name(name.as_str(), detail),
        zbus::Error::InputOutput(_) | zbus::Error::Address(_) => NetworkError::Unavailable,
        other => NetworkError::Failed(other.to_string()),
    }
}

fn map_fdo(error: zbus::fdo::Error) -> NetworkError {
    match error {
        zbus::fdo::Error::ZBus(inner) => map_error(inner),
        other => {
            let detail = other.description().map(str::to_owned);
            map_error_name(other.name().as_str(), detail)
        }
    }
}

/// What a row's id leads back to when a command names it. Rebuilt with every
/// snapshot: NetworkManager's object paths are only valid while they exist.
#[derive(Debug, Clone)]
enum Route {
    Ethernet {
        device: OwnedObjectPath,
        active: Option<OwnedObjectPath>,
    },
    Wifi {
        device: OwnedObjectPath,
        ap: OwnedObjectPath,
        ssid: Vec<u8>,
        security: Security,
        wep: bool,
        saved: Option<OwnedObjectPath>,
        active: Option<OwnedObjectPath>,
    },
    Vpn {
        connection: OwnedObjectPath,
        active: Option<OwnedObjectPath>,
    },
}

/// A saved connection profile, as much of it as the rows need.
struct Saved {
    path: OwnedObjectPath,
    kind: String,
    id: String,
    uuid: String,
    interface: String,
    ssid: Option<Vec<u8>>,
}

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

/// The best access point seen so far for one SSID, across every radio.
struct WifiCandidate {
    row: NetworkEntry,
    route: Route,
}

/// The `Network` backend over NetworkManager.
pub struct NmNetwork {
    connection: Connection,
    /// Cuprita's own flag, in memory only: NetworkManager has no airplane
    /// mode of its own.
    airplane: bool,
    routes: HashMap<String, Route>,
    failed: FailedJoins,
    notify: Option<Notify>,
    /// The Wi-Fi radio could not be read on the last snapshot: said once.
    radio_failed: bool,
}

impl NmNetwork {
    /// Connects to the system bus. NetworkManager itself is reached lazily:
    /// a missing service reports `Unavailable` on the first read.
    pub fn connect_system() -> Result<Self, NetworkError> {
        let connection = Connection::system().map_err(map_error)?;
        Ok(Self {
            connection,
            airplane: false,
            routes: HashMap::new(),
            failed: Arc::default(),
            notify: None,
            radio_failed: false,
        })
    }

    /// Where notices that arrive outside a command go: a join that failed
    /// after its call returned, a radio that cannot be read.
    pub fn set_notify(&mut self, notify: impl Fn(String) + Send + Sync + 'static) {
        self.notify = Some(Arc::new(notify));
    }

    fn say(&self, text: String) {
        if let Some(notify) = &self.notify {
            notify(text);
        }
    }

    fn proxy<'a>(&self, path: &'a str, interface: &'a str) -> Result<Proxy<'a>, NetworkError> {
        proxy(&self.connection, path, interface)
    }

    fn manager(&self) -> Result<Proxy<'static>, NetworkError> {
        self.proxy(NM_PATH, NM)
    }

    fn saved_connections(&self) -> Result<Vec<Saved>, NetworkError> {
        let paths: Vec<OwnedObjectPath> = self
            .proxy(SETTINGS_PATH, IFACE_SETTINGS)?
            .call("ListConnections", &())
            .map_err(map_error)?;
        let mut saved = Vec::with_capacity(paths.len());
        for path in paths {
            // A profile that vanished or cannot be read is skipped, not fatal.
            let Ok(settings) = self.proxy(path.as_str(), IFACE_CONNECTION).and_then(|p| {
                p.call::<_, _, Settings>("GetSettings", &())
                    .map_err(map_error)
            }) else {
                continue;
            };
            let text = |section: &str, key: &str| {
                settings
                    .get(section)
                    .and_then(|s| s.get(key))
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .unwrap_or_default()
            };
            let ssid = settings
                .get("802-11-wireless")
                .and_then(|s| s.get("ssid"))
                .and_then(|v| Vec::<u8>::try_from(v.clone()).ok());
            saved.push(Saved {
                kind: text("connection", "type"),
                id: text("connection", "id"),
                uuid: text("connection", "uuid"),
                interface: text("connection", "interface-name"),
                ssid,
                path,
            });
        }
        Ok(saved)
    }

    /// Every active connection, by the saved profile it activates: its own
    /// path and its state.
    fn active_connections(
        &self,
        manager: &Proxy<'_>,
    ) -> Result<HashMap<String, (OwnedObjectPath, u32)>, NetworkError> {
        let paths: Vec<OwnedObjectPath> = manager
            .get_property("ActiveConnections")
            .map_err(map_error)?;
        let mut active = HashMap::new();
        for path in paths {
            let Ok(proxy) = self.proxy(path.as_str(), IFACE_ACTIVE) else {
                continue;
            };
            let profile = proxy.get_property::<OwnedObjectPath>("Connection");
            let state = proxy.get_property::<u32>("State");
            drop(proxy);
            if let (Ok(profile), Ok(state)) = (profile, state) {
                active.insert(profile.as_str().to_owned(), (path, state));
            }
        }
        Ok(active)
    }

    fn active_id(&self, active: &OwnedObjectPath) -> Option<String> {
        self.proxy(active.as_str(), IFACE_ACTIVE)
            .ok()?
            .get_property::<String>("Id")
            .ok()
    }

    /// Adds one radio's access points to `best`. The SSID the radio's active
    /// connection carries takes its state from that connection, even while no
    /// access point is active yet (preparing) or any more (failed).
    fn read_wifi(
        &self,
        device: &OwnedObjectPath,
        saved: &[Saved],
        active: &HashMap<String, (OwnedObjectPath, u32)>,
        best: &mut HashMap<Vec<u8>, WifiCandidate>,
    ) -> Result<(), NetworkError> {
        let wireless = self.proxy(device.as_str(), IFACE_WIRELESS)?;
        let device_active: OwnedObjectPath = self
            .proxy(device.as_str(), IFACE_DEVICE)?
            .get_property("ActiveConnection")
            .map_err(map_error)?;
        // The SSID this radio is joining or on, by its profile's settings.
        let carried: Option<(Vec<u8>, OwnedObjectPath, u32)> = active
            .iter()
            .find(|(_, (path, _))| *path == device_active)
            .and_then(|(profile, (path, state))| {
                let ssid = saved
                    .iter()
                    .find(|s| s.path.as_str() == profile)
                    .and_then(|s| s.ssid.clone())?;
                Some((ssid, path.clone(), *state))
            });
        let aps: Vec<OwnedObjectPath> = wireless
            .call("GetAllAccessPoints", &())
            .map_err(map_error)?;
        // A copy: the join followers must not wait on these D-Bus reads.
        let failed = self
            .failed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for ap in aps {
            let Ok(proxy) = self.proxy(ap.as_str(), IFACE_AP) else {
                continue;
            };
            let Ok(ssid) = proxy.get_property::<Vec<u8>>("Ssid") else {
                continue;
            };
            // A hidden network has no name to show or to join by.
            if ssid.is_empty() {
                continue;
            }
            let strength = proxy.get_property::<u8>("Strength").unwrap_or(0);
            let flags = proxy.get_property::<u32>("Flags").unwrap_or(0);
            let wpa = proxy.get_property::<u32>("WpaFlags").unwrap_or(0);
            let rsn = proxy.get_property::<u32>("RsnFlags").unwrap_or(0);
            drop(proxy);
            let profile = saved
                .iter()
                .find(|s| s.ssid.as_deref() == Some(ssid.as_slice()))
                .map(|s| s.path.clone());
            let link = carried.as_ref().filter(|(s, _, _)| *s == ssid);
            let mut row = ap_to_network(&ssid, strength, flags, wpa, rsn, profile.is_some(), false);
            row.state = wifi_row_state(link.map(|(_, _, s)| *s), failed.contains(&ssid));
            let route = Route::Wifi {
                device: device.clone(),
                ap,
                ssid: ssid.clone(),
                security: row.security,
                wep: wep_only(flags, wpa, rsn),
                saved: profile,
                active: link.map(|(_, path, _)| path.clone()),
            };
            // One row per SSID: the radio carrying it wins, then the strongest.
            let replace = best.get(&ssid).is_none_or(|kept| {
                (link.is_some(), row.signal) > (kept_carried(&kept.route), kept.row.signal)
            });
            if replace {
                best.insert(ssid, WifiCandidate { row, route });
            }
        }
        Ok(())
    }

    fn route(&self, id: &str) -> Result<Route, NetworkError> {
        self.routes
            .get(id)
            .cloned()
            .ok_or_else(|| NetworkError::NotFound(id.to_owned()))
    }

    fn activate(
        &self,
        connection: &str,
        device: &str,
        specific: &str,
    ) -> Result<OwnedObjectPath, NetworkError> {
        let connection = object_path(connection)?;
        let device = object_path(device)?;
        let specific = object_path(specific)?;
        self.manager()?
            .call::<_, _, OwnedObjectPath>("ActivateConnection", &(connection, device, specific))
            .map_err(map_error)
    }

    fn deactivate(&self, active: Option<OwnedObjectPath>) -> Result<(), NetworkError> {
        // Not active: nothing to leave.
        let Some(active) = active else {
            return Ok(());
        };
        self.manager()?
            .call::<_, _, ()>("DeactivateConnection", &(active,))
            .map_err(map_error)
    }

    fn delete(&self, profile: &OwnedObjectPath) -> Result<(), NetworkError> {
        self.proxy(profile.as_str(), IFACE_CONNECTION)?
            .call::<_, _, ()>("Delete", &())
            .map_err(map_error)
    }

    /// Joins a Wi-Fi network that has no saved profile. The profile starts
    /// volatile (`persist: volatile`): the join's follower saves it to disk
    /// once the network is reached and deletes it if the join fails, so a
    /// mistyped passphrase never leaves a saved profile behind. The
    /// passphrase lives in NetworkManager's profile, never in Cuprita.
    fn add_and_activate(
        &self,
        device: &OwnedObjectPath,
        ap: &OwnedObjectPath,
        ssid: &[u8],
        password: Option<&str>,
    ) -> Result<(), NetworkError> {
        let name = String::from_utf8_lossy(ssid).into_owned();
        let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        settings.insert(
            "connection",
            HashMap::from([
                ("id", Value::from(name.as_str())),
                ("type", Value::from("802-11-wireless")),
            ]),
        );
        settings.insert(
            "802-11-wireless",
            HashMap::from([
                ("ssid", Value::from(ssid)),
                ("mode", Value::from("infrastructure")),
            ]),
        );
        if let Some(password) = password {
            settings.insert(
                "802-11-wireless-security",
                HashMap::from([
                    ("key-mgmt", Value::from("wpa-psk")),
                    ("psk", Value::from(password)),
                ]),
            );
        }
        let options: HashMap<&str, Value<'_>> =
            HashMap::from([("persist", Value::from("volatile"))]);
        let (profile, active, _) = self
            .manager()?
            .call::<_, _, (
                OwnedObjectPath,
                OwnedObjectPath,
                HashMap<String, OwnedValue>,
            )>(
                "AddAndActivateConnection2",
                &(settings, device, ap, options),
            )
            .map_err(map_error)?;
        self.follow(active, Some(profile), ssid);
        Ok(())
    }

    /// Follows a Wi-Fi join to its outcome on a thread of its own.
    fn follow(&self, active: OwnedObjectPath, volatile: Option<OwnedObjectPath>, ssid: &[u8]) {
        join::follow(join::Join {
            connection: self.connection.clone(),
            active,
            volatile,
            ssid: ssid.to_vec(),
            failed: Arc::clone(&self.failed),
            notify: self.notify.clone(),
        });
    }
}

fn kept_carried(route: &Route) -> bool {
    matches!(
        route,
        Route::Wifi {
            active: Some(_),
            ..
        }
    )
}

fn proxy<'a>(
    connection: &Connection,
    path: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>, NetworkError> {
    zbus::blocking::proxy::Builder::new(connection)
        .destination(NM)
        .and_then(|b| b.path(path))
        .and_then(|b| b.interface(interface))
        .map(|b| b.cache_properties(CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(map_error)
}

fn object_path(path: &str) -> Result<OwnedObjectPath, NetworkError> {
    OwnedObjectPath::try_from(path).map_err(|e| NetworkError::Failed(e.to_string()))
}

impl Network for NmNetwork {
    fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError> {
        let manager = self.manager()?;
        let wifi_enabled: bool = manager.get_property("WirelessEnabled").map_err(map_error)?;
        let saved = self.saved_connections()?;
        let active = self.active_connections(&manager)?;
        let devices: Vec<OwnedObjectPath> = manager.call("GetDevices", &()).map_err(map_error)?;

        let mut rows = Vec::new();
        let mut routes = HashMap::new();
        let mut best = HashMap::new();
        let mut radio_error = None;
        for device in devices {
            let Ok(proxy) = self.proxy(device.as_str(), IFACE_DEVICE) else {
                continue;
            };
            let Ok(kind) = proxy.get_property::<u32>("DeviceType") else {
                continue;
            };
            match kind {
                DEVICE_ETHERNET => {
                    let state = proxy.get_property::<u32>("State").unwrap_or(0);
                    let interface = proxy
                        .get_property::<String>("Interface")
                        .unwrap_or_default();
                    let link = proxy
                        .get_property::<OwnedObjectPath>("ActiveConnection")
                        .ok()
                        .filter(|p| p.as_str() != NO_PATH);
                    let name = link
                        .as_ref()
                        .and_then(|p| self.active_id(p))
                        .unwrap_or_else(|| interface.clone());
                    // Known: a saved wired profile this device may use.
                    let known = saved.iter().any(|s| {
                        s.kind == "802-3-ethernet"
                            && (s.interface.is_empty() || s.interface == interface)
                    });
                    let id = format!("ethernet:{interface}");
                    routes.insert(
                        id.clone(),
                        Route::Ethernet {
                            device: device.clone(),
                            active: link,
                        },
                    );
                    rows.push(NetworkEntry {
                        id,
                        kind: NetworkKind::Ethernet,
                        name,
                        state: device_state_to_state(state),
                        signal: None,
                        security: Security::Open,
                        known,
                    });
                }
                DEVICE_WIFI if wifi_enabled => {
                    drop(proxy);
                    // One unreadable radio must not hide the wired link.
                    if let Err(error) = self.read_wifi(&device, &saved, &active, &mut best) {
                        eprintln!("Cuprita: the Wi-Fi radio {device} could not be read: {error}");
                        radio_error = Some(error);
                    }
                }
                _ => {}
            }
        }
        match radio_error {
            Some(error) if !self.radio_failed => {
                self.radio_failed = true;
                self.say(error.message_es());
            }
            Some(_) => {}
            None => self.radio_failed = false,
        }
        for candidate in best.into_values() {
            routes.insert(candidate.row.id.clone(), candidate.route);
            rows.push(candidate.row);
        }

        let mut seen = HashSet::new();
        for profile in saved
            .iter()
            .filter(|s| s.kind == "vpn" || s.kind == "wireguard")
        {
            if !seen.insert(profile.uuid.clone()) {
                continue;
            }
            let link = active.get(profile.path.as_str());
            let id = format!("vpn:{}", profile.uuid);
            routes.insert(
                id.clone(),
                Route::Vpn {
                    connection: profile.path.clone(),
                    active: link.map(|(path, _)| path.clone()),
                },
            );
            rows.push(NetworkEntry {
                id,
                kind: NetworkKind::Vpn,
                name: profile.id.clone(),
                state: link.map_or(NetworkState::Disconnected, |(_, s)| {
                    active_state_to_state(*s)
                }),
                signal: None,
                security: Security::Open,
                known: true,
            });
        }

        order_networks(&mut rows);
        self.routes = routes;
        Ok(NetworkSnapshot {
            wifi_enabled,
            airplane: self.airplane,
            networks: rows,
        })
    }

    fn set_wifi_enabled(&mut self, on: bool) -> Result<(), NetworkError> {
        self.manager()?
            .set_property("WirelessEnabled", on)
            .map_err(map_fdo)
    }

    /// Airplane mode is Cuprita's, kept in memory: here it turns the Wi-Fi
    /// radio off (and back on); the controller powers the Bluetooth adapter
    /// through its own trait.
    fn set_airplane(&mut self, on: bool) -> Result<(), NetworkError> {
        self.set_wifi_enabled(!on)?;
        self.airplane = on;
        Ok(())
    }

    fn connect(&mut self, id: &str, password: Option<&str>) -> Result<(), NetworkError> {
        match self.route(id)? {
            Route::Ethernet { device, .. } => {
                self.activate(NO_PATH, device.as_str(), NO_PATH).map(drop)
            }
            Route::Wifi { wep: true, .. } => Err(NetworkError::wep_unsupported()),
            Route::Wifi {
                device,
                ap,
                ssid,
                saved: Some(profile),
                ..
            } => {
                let active = self.activate(profile.as_str(), device.as_str(), ap.as_str())?;
                self.follow(active, None, &ssid);
                Ok(())
            }
            Route::Wifi {
                security: Security::Enterprise,
                ..
            } => Err(NetworkError::enterprise_unsupported()),
            Route::Wifi {
                device,
                ap,
                ssid,
                security: Security::Psk,
                ..
            } => match password {
                Some(password) => self.add_and_activate(&device, &ap, &ssid, Some(password)),
                None => Err(NetworkError::password_required()),
            },
            Route::Wifi {
                device, ap, ssid, ..
            } => self.add_and_activate(&device, &ap, &ssid, None),
            Route::Vpn { connection, .. } => self
                .activate(connection.as_str(), NO_PATH, NO_PATH)
                .map(drop),
        }
    }

    fn disconnect(&mut self, id: &str) -> Result<(), NetworkError> {
        match self.route(id)? {
            Route::Ethernet { active, .. }
            | Route::Wifi { active, .. }
            | Route::Vpn { active, .. } => self.deactivate(active),
        }
    }

    fn forget(&mut self, id: &str) -> Result<(), NetworkError> {
        match self.route(id)? {
            Route::Wifi {
                saved: Some(profile),
                ..
            }
            | Route::Vpn {
                connection: profile,
                ..
            } => self.delete(&profile),
            _ => Err(NetworkError::NotFound(id.to_owned())),
        }
    }

    fn set_vpn_active(&mut self, id: &str, on: bool) -> Result<(), NetworkError> {
        match self.route(id)? {
            Route::Vpn { connection, .. } if on => self
                .activate(connection.as_str(), NO_PATH, NO_PATH)
                .map(drop),
            Route::Vpn { active, .. } => self.deactivate(active),
            _ => Err(NetworkError::NotFound(id.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NM_802_11_AP_SEC_PAIR_CCMP | GROUP_CCMP | KEY_MGMT_PSK`.
    const RSN_WPA2_PSK: u32 = 0x8 | 0x80 | 0x100;
    const RSN_WPA2_EAP: u32 = 0x8 | 0x80 | KEY_MGMT_802_1X;

    #[test]
    fn a_wpa2_psk_access_point_is_psk() {
        let row = ap_to_network(b"Home", 72, AP_PRIVACY, 0, RSN_WPA2_PSK, true, false);
        assert_eq!(row.security, Security::Psk);
        assert_eq!(row.kind, NetworkKind::Wifi);
        assert_eq!(row.name, "Home");
        assert_eq!(row.signal, Some(72));
        assert!(row.known);
        assert_eq!(row.state, NetworkState::Disconnected);
        assert!(!wep_only(AP_PRIVACY, 0, RSN_WPA2_PSK));
    }

    #[test]
    fn an_802_1x_access_point_is_enterprise() {
        let row = ap_to_network(b"Campus", 50, AP_PRIVACY, 0, RSN_WPA2_EAP, false, false);
        assert_eq!(row.security, Security::Enterprise);
    }

    #[test]
    fn an_open_access_point_is_open() {
        let row = ap_to_network(b"Cafe", 20, 0, 0, 0, false, true);
        assert_eq!(row.security, Security::Open);
        assert_eq!(row.state, NetworkState::Connected);
        assert!(!wep_only(0, 0, 0));
    }

    #[test]
    fn a_wep_access_point_reads_protected_and_is_flagged() {
        let row = ap_to_network(b"Old", 30, AP_PRIVACY, 0, 0, false, false);
        assert_eq!(row.security, Security::Psk);
        assert!(wep_only(AP_PRIVACY, 0, 0));
    }

    #[test]
    fn ssid_bytes_decode_lossily_and_ids_keep_the_bytes() {
        let row = ap_to_network(&[0x43, 0xff, 0x61], 10, 0, 0, 0, false, false);
        assert_eq!(row.name, "C\u{fffd}a");
        assert_eq!(row.id, "wifi:43ff61");
        // Same text, different bytes: different rows.
        let other = ap_to_network(&[0x43, 0xfe, 0x61], 10, 0, 0, 0, false, false);
        assert_eq!(other.name, row.name);
        assert_ne!(other.id, row.id);
    }

    #[test]
    fn device_states_map() {
        assert_eq!(device_state_to_state(100), NetworkState::Connected);
        assert_eq!(device_state_to_state(40), NetworkState::Connecting);
        assert_eq!(device_state_to_state(90), NetworkState::Connecting);
        assert_eq!(device_state_to_state(120), NetworkState::Failed);
        assert_eq!(device_state_to_state(30), NetworkState::Disconnected);
    }

    #[test]
    fn wifi_rows_follow_the_active_connection() {
        assert_eq!(
            wifi_row_state(Some(ACTIVE_ACTIVATING), false),
            NetworkState::Connecting
        );
        assert_eq!(
            wifi_row_state(Some(ACTIVE_ACTIVATED), false),
            NetworkState::Connected
        );
        assert_eq!(
            wifi_row_state(Some(ACTIVE_DEACTIVATING), false),
            NetworkState::Disconnected
        );
        assert_eq!(
            wifi_row_state(Some(ACTIVE_DEACTIVATED), false),
            NetworkState::Disconnected
        );
        assert_eq!(wifi_row_state(None, false), NetworkState::Disconnected);
        // A failed join shows until a new one is under way or succeeds.
        assert_eq!(wifi_row_state(None, true), NetworkState::Failed);
        assert_eq!(
            wifi_row_state(Some(ACTIVE_DEACTIVATED), true),
            NetworkState::Failed
        );
        assert_eq!(
            wifi_row_state(Some(ACTIVE_ACTIVATING), true),
            NetworkState::Connecting
        );
    }

    #[test]
    fn error_names_map_to_the_section_errors() {
        assert_eq!(
            map_error_name("org.freedesktop.NetworkManager.PermissionDenied", None),
            NetworkError::Denied
        );
        assert_eq!(
            map_error_name("org.freedesktop.DBus.Error.ServiceUnknown", None),
            NetworkError::Unavailable
        );
        assert_eq!(
            map_error_name(
                "org.freedesktop.NetworkManager.UnknownConnection",
                Some("no such profile".to_owned())
            ),
            NetworkError::Failed("no such profile".to_owned())
        );
        assert_eq!(
            map_fdo(zbus::fdo::Error::AccessDenied("polkit".to_owned())),
            NetworkError::Denied
        );
    }
}
