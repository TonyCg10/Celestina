//! Fake system services on a private bus: UPower's display device,
//! power-profiles-daemon (under either of its two bus names), a minimal
//! NetworkManager and a minimal BlueZ, served in-process by `zbus` on a
//! `dbus-daemon` of the caller's own (the recipe of
//! `celestina/src/tray/test_bus.rs`, repeated here because that one is
//! crate-private).
//!
//! Test support, a crate of its own: `celestina` names it only under
//! `[dev-dependencies]` (unit tests, `tests/`, `examples/`), so the library
//! and the shipped binary never link it. It is deliberately not a feature of
//! `celestina`: a self dev-dependency with a feature changes the bytes that
//! `cargo test` leaves at `target/<profile>/celestina`.
//!
//! Nothing here can reach the real system or session bus. The daemon runs on
//! a configuration of its own, with an empty service directory unless a test
//! asks for activatable names (see [`FakeSystem::start_activatable`]; those
//! only touch a marker file), and a client reaches it only through the
//! address the caller passes (`FakeSystem::address`,
//! `DBUS_SYSTEM_BUS_ADDRESS`).
//!
//! Every method call and property write the fakes serve is recorded in
//! `Calls`, so a test asserts what was and was not called. The record names
//! the call and its object arguments, never a setting's contents (a passphrase
//! sent to `AddAndActivateConnection2` is not recorded). State is simulated
//! only where a test needs to observe it: BlueZ's `Powered` and `Connected`,
//! NetworkManager's `WirelessEnabled`, power-profiles-daemon's `ActiveProfile`
//! and UPower's display device through the `set_*` mutators.

use std::{
    collections::HashMap,
    fmt,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, PoisonError},
};

use zbus::{
    blocking::{connection::Builder, Connection},
    fdo,
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value},
};

/// The bus daemon's configuration: a bus listening at `LISTEN` with a service
/// directory of its own (empty unless a test lists activatable names) and a
/// permissive policy.
const BUS_CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>LISTEN</listen>
  <servicedir>SERVICES</servicedir>
  <keep_umask/>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

/// The executable an activatable fake name runs (an absolute path, as the bus
/// requires); a test that needs activation is skipped when it is missing.
pub const TOUCH: &str = "/usr/bin/touch";

/// Why the fakes could not start.
#[derive(Debug)]
pub enum FakeError {
    /// `dbus-daemon` is not installed: a test is skipped, not failed.
    DaemonMissing,
    /// Anything else, with its cause.
    Failed(String),
}

impl fmt::Display for FakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DaemonMissing => f.write_str("dbus-daemon is not installed"),
            Self::Failed(cause) => write!(f, "the fake system bus failed: {cause}"),
        }
    }
}

impl std::error::Error for FakeError {}

fn failed(error: impl fmt::Display) -> FakeError {
    FakeError::Failed(error.to_string())
}

/// A private bus daemon, killed and removed when dropped.
struct PrivateBus {
    child: Child,
    dir: PathBuf,
    address: String,
}

impl PrivateBus {
    fn start(name: &str, activatable: &[&str]) -> Result<Self, FakeError> {
        let dir = std::env::temp_dir().join(format!("celestina-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(failed)?;
        let config = dir.join("bus.conf");
        let listen = format!("unix:path={}", dir.join("bus").display());
        let services = dir.join("services");
        std::fs::create_dir_all(&services).map_err(failed)?;
        // An activatable name: a service file whose `Exec` only touches a
        // marker, so a test sees whether the bus was asked to start it.
        for well_known in activatable {
            let marker = dir.join(format!("activated-{well_known}"));
            std::fs::write(
                services.join(format!("{well_known}.service")),
                format!(
                    "[D-BUS Service]\nName={well_known}\nExec={TOUCH} {}\n",
                    marker.display()
                ),
            )
            .map_err(failed)?;
        }
        let config_text = BUS_CONFIG
            .replace("LISTEN", &listen)
            .replace("SERVICES", &services.display().to_string());
        std::fs::write(&config, config_text).map_err(failed)?;
        let spawned = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .arg("--nopidfile")
            .arg("--print-address=1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(if error.kind() == std::io::ErrorKind::NotFound {
                    FakeError::DaemonMissing
                } else {
                    failed(error)
                });
            }
        };
        let mut address = String::new();
        let read = child
            .stdout
            .take()
            .map(|out| BufReader::new(out).read_line(&mut address));
        if !matches!(read, Some(Ok(n)) if n > 0) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dir);
            return Err(failed("dbus-daemon printed no address"));
        }
        Ok(Self {
            child,
            dir,
            address: address.trim().to_owned(),
        })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The calls the fakes served, in order, as `service:Method(args)` lines
/// (`nm:ActivateConnection(/…/Settings/1,/…/Devices/1,/…/AccessPoint/2)`,
/// `bluez:Connect(/org/bluez/hci0/dev_…)`, `ppd:Set ActiveProfile=balanced`).
#[derive(Clone, Default)]
pub struct Calls(Arc<Mutex<Vec<String>>>);

impl Calls {
    fn record(&self, line: impl Into<String>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.into());
    }

    /// Every recorded call, oldest first.
    #[must_use]
    pub fn all(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Forgets the record (a test's setup reads are not its subject).
    pub fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// How many recorded calls start with `prefix`.
    #[must_use]
    pub fn count(&self, prefix: &str) -> usize {
        self.all().iter().filter(|c| c.starts_with(prefix)).count()
    }
}

/// An owned D-Bus value from a plain one.
fn owned<'a>(value: impl Into<Value<'a>>) -> OwnedValue {
    // Only a value carrying a file descriptor fails to convert; none here does.
    value
        .into()
        .try_to_owned()
        .unwrap_or_else(|_| OwnedValue::from(false))
}

fn path(text: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(text).unwrap_or_else(|_| OwnedObjectPath::from(ObjectPath::default()))
}

fn props<'a>(pairs: Vec<(&str, Value<'a>)>) -> HashMap<String, OwnedValue> {
    pairs
        .into_iter()
        .map(|(k, v)| (k.to_owned(), owned(v)))
        .collect()
}

// ---------------------------------------------------------------- UPower

/// The `DisplayDevice` of UPower: the machine's battery as one device.
pub const UPOWER_NAME: &str = "org.freedesktop.UPower";
pub const UPOWER_DISPLAY_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";

struct DisplayDevice {
    is_present: bool,
    state: u32,
    percentage: f64,
    time_to_empty: i64,
    time_to_full: i64,
}

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl DisplayDevice {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        self.is_present
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }

    #[zbus(property)]
    fn percentage(&self) -> f64 {
        self.percentage
    }

    #[zbus(property)]
    fn time_to_empty(&self) -> i64 {
        self.time_to_empty
    }

    #[zbus(property)]
    fn time_to_full(&self) -> i64 {
        self.time_to_full
    }
}

// ------------------------------------------------- power-profiles-daemon

/// Which bus name (and object path and interface) power-profiles-daemon is
/// served under. Current releases use the UPower-prefixed one, older ones
/// `net.hadess`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ppd {
    /// `org.freedesktop.UPower.PowerProfiles` at
    /// `/org/freedesktop/UPower/PowerProfiles`.
    Upower,
    /// `net.hadess.PowerProfiles` at `/net/hadess/PowerProfiles`.
    Hadess,
    /// Both names at once (a daemon during the transition between them).
    Both,
    /// No profile daemon.
    Absent,
}

pub const PPD_UPOWER_NAME: &str = "org.freedesktop.UPower.PowerProfiles";
pub const PPD_UPOWER_PATH: &str = "/org/freedesktop/UPower/PowerProfiles";
pub const PPD_HADESS_NAME: &str = "net.hadess.PowerProfiles";
pub const PPD_HADESS_PATH: &str = "/net/hadess/PowerProfiles";

/// The profiles the fake daemon lists, in its order.
pub const PPD_PROFILES: [&str; 3] = ["power-saver", "balanced", "performance"];

/// One power-profiles-daemon object. Both names share this body; only the
/// interface name differs, so the two structs come from one macro.
macro_rules! power_profiles {
    ($name:ident, $interface:literal, $record:literal) => {
        struct $name {
            calls: Calls,
            active: String,
            degraded: String,
        }

        #[zbus::interface(name = $interface)]
        impl $name {
            #[zbus(property)]
            fn active_profile(&self) -> String {
                self.active.clone()
            }

            #[zbus(property)]
            fn set_active_profile(&mut self, profile: String) -> zbus::Result<()> {
                self.calls
                    .record(format!("{}:Set ActiveProfile={profile}", $record));
                if !PPD_PROFILES.contains(&profile.as_str()) {
                    return Err(zbus::Error::from(fdo::Error::InvalidArgs(format!(
                        "no profile {profile}"
                    ))));
                }
                self.active = profile;
                Ok(())
            }

            #[zbus(property)]
            fn performance_degraded(&self) -> String {
                self.degraded.clone()
            }

            #[zbus(property)]
            fn profiles(&self) -> Vec<HashMap<String, OwnedValue>> {
                PPD_PROFILES
                    .iter()
                    .map(|p| {
                        props(vec![
                            ("Profile", Value::from(*p)),
                            ("Driver", Value::from("fake")),
                        ])
                    })
                    .collect()
            }
        }
    };
}

power_profiles!(PpdUpower, "org.freedesktop.UPower.PowerProfiles", "ppd");
power_profiles!(PpdHadess, "net.hadess.PowerProfiles", "ppd-hadess");

// ------------------------------------------------------- NetworkManager

const NM_NAME: &str = "org.freedesktop.NetworkManager";
pub const NM_PATH: &str = "/org/freedesktop/NetworkManager";
pub const NM_SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
pub const NM_PROFILE_PATH: &str = "/org/freedesktop/NetworkManager/Settings/1";
pub const NM_DEVICE_PATH: &str = "/org/freedesktop/NetworkManager/Devices/1";
pub const NM_AP_HOME_PATH: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
pub const NM_AP_OPEN_PATH: &str = "/org/freedesktop/NetworkManager/AccessPoint/2";
pub const NM_ACTIVE_PATH: &str = "/org/freedesktop/NetworkManager/ActiveConnection/1";
const NM_IP4_PATH: &str = "/org/freedesktop/NetworkManager/IP4Config/1";

/// The saved, active, WPA2 network and the open one the fake radio sees.
pub const NM_HOME_SSID: &str = "HomeNet";
pub const NM_OPEN_SSID: &str = "CafeOpen";
pub const NM_HOME_ADDRESS: &str = "192.168.1.20";

struct NmManager {
    calls: Calls,
    wireless_enabled: bool,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl NmManager {
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(NM_DEVICE_PATH)]
    }

    fn activate_connection(
        &self,
        connection: ObjectPath<'_>,
        device: ObjectPath<'_>,
        specific_object: ObjectPath<'_>,
    ) -> OwnedObjectPath {
        self.calls.record(format!(
            "nm:ActivateConnection({connection},{device},{specific_object})"
        ));
        path(NM_ACTIVE_PATH)
    }

    fn deactivate_connection(&self, active_connection: ObjectPath<'_>) {
        self.calls
            .record(format!("nm:DeactivateConnection({active_connection})"));
    }

    /// Recorded without its settings (they may hold a passphrase) and
    /// refused: the fake never creates a profile.
    #[zbus(name = "AddAndActivateConnection2")]
    fn add_and_activate_connection2(
        &self,
        _connection: HashMap<String, HashMap<String, OwnedValue>>,
        device: ObjectPath<'_>,
        specific_object: ObjectPath<'_>,
        _options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<(
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    )> {
        self.calls.record(format!(
            "nm:AddAndActivateConnection2({device},{specific_object})"
        ));
        Err(fdo::Error::NotSupported(
            "the fake NetworkManager creates no profile".to_owned(),
        ))
    }

    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.wireless_enabled
    }

    #[zbus(property)]
    fn set_wireless_enabled(&mut self, on: bool) {
        self.calls.record(format!("nm:Set WirelessEnabled={on}"));
        self.wireless_enabled = on;
    }

    #[zbus(property)]
    fn active_connections(&self) -> Vec<OwnedObjectPath> {
        vec![path(NM_ACTIVE_PATH)]
    }
}

struct NmSettings;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl NmSettings {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        vec![path(NM_PROFILE_PATH)]
    }
}

struct NmProfile {
    calls: Calls,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl NmProfile {
    /// The profile's settings, without a secret section.
    fn get_settings(&self) -> HashMap<String, HashMap<String, OwnedValue>> {
        HashMap::from([
            (
                "connection".to_owned(),
                props(vec![
                    ("id", Value::from(NM_HOME_SSID)),
                    ("type", Value::from("802-11-wireless")),
                    ("uuid", Value::from("0b1e57a5-0000-4000-8000-000000000001")),
                ]),
            ),
            (
                "802-11-wireless".to_owned(),
                props(vec![("ssid", Value::from(NM_HOME_SSID.as_bytes()))]),
            ),
        ])
    }

    fn delete(&self) {
        self.calls.record(format!("nm:Delete({NM_PROFILE_PATH})"));
    }
}

struct NmDevice;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl NmDevice {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2 // NM_DEVICE_TYPE_WIFI
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        100 // NM_DEVICE_STATE_ACTIVATED
    }

    #[zbus(property)]
    fn interface(&self) -> String {
        "wlan0".to_owned()
    }

    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        path(NM_ACTIVE_PATH)
    }

    #[zbus(property, name = "Ip4Config")]
    fn ip4_config(&self) -> OwnedObjectPath {
        path(NM_IP4_PATH)
    }
}

struct NmWireless;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl NmWireless {
    fn get_all_access_points(&self) -> Vec<OwnedObjectPath> {
        vec![path(NM_AP_HOME_PATH), path(NM_AP_OPEN_PATH)]
    }
}

struct NmAccessPoint {
    ssid: &'static str,
    strength: u8,
    wpa_flags: u32,
    rsn_flags: u32,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl NmAccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }

    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }

    #[zbus(property)]
    fn flags(&self) -> u32 {
        u32::from(self.rsn_flags != 0) // NM_802_11_AP_FLAGS_PRIVACY
    }

    #[zbus(property, name = "WpaFlags")]
    fn wpa_flags(&self) -> u32 {
        self.wpa_flags
    }

    #[zbus(property, name = "RsnFlags")]
    fn rsn_flags(&self) -> u32 {
        self.rsn_flags
    }
}

struct NmActive;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl NmActive {
    #[zbus(property)]
    fn connection(&self) -> OwnedObjectPath {
        path(NM_PROFILE_PATH)
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        2 // NM_ACTIVE_CONNECTION_STATE_ACTIVATED
    }

    #[zbus(property)]
    fn id(&self) -> String {
        NM_HOME_SSID.to_owned()
    }
}

struct NmIp4Config;

#[zbus::interface(name = "org.freedesktop.NetworkManager.IP4Config")]
impl NmIp4Config {
    #[zbus(property)]
    fn address_data(&self) -> Vec<HashMap<String, OwnedValue>> {
        vec![props(vec![
            ("address", Value::from(NM_HOME_ADDRESS)),
            ("prefix", Value::from(24_u32)),
        ])]
    }
}

// ----------------------------------------------------------------- BlueZ

const BLUEZ_NAME: &str = "org.bluez";
pub const BLUEZ_ADAPTER_PATH: &str = "/org/bluez/hci0";
/// A paired, connected headset with a battery (80 %).
pub const BLUEZ_HEADSET_ADDRESS: &str = "AA:BB:CC:00:00:01";
pub const BLUEZ_HEADSET_PATH: &str = "/org/bluez/hci0/dev_AA_BB_CC_00_00_01";
/// A paired keyboard, not connected, without a battery interface.
pub const BLUEZ_KEYBOARD_ADDRESS: &str = "AA:BB:CC:00:00:02";
pub const BLUEZ_KEYBOARD_PATH: &str = "/org/bluez/hci0/dev_AA_BB_CC_00_00_02";

struct BluezAdapter {
    calls: Calls,
    powered: bool,
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl BluezAdapter {
    fn start_discovery(&self) {
        self.calls.record("bluez:StartDiscovery");
    }

    fn stop_discovery(&self) {
        self.calls.record("bluez:StopDiscovery");
    }

    fn remove_device(&self, device: ObjectPath<'_>) {
        self.calls.record(format!("bluez:RemoveDevice({device})"));
    }

    #[zbus(property)]
    fn powered(&self) -> bool {
        self.powered
    }

    #[zbus(property)]
    fn set_powered(&mut self, on: bool) {
        self.calls.record(format!("bluez:Set Powered={on}"));
        self.powered = on;
    }

    #[zbus(property)]
    fn discovering(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn address(&self) -> String {
        "00:11:22:33:44:55".to_owned()
    }
}

struct BluezDevice {
    calls: Calls,
    path: &'static str,
    address: &'static str,
    alias: &'static str,
    icon: &'static str,
    connected: bool,
}

#[zbus::interface(name = "org.bluez.Device1")]
impl BluezDevice {
    async fn connect(
        &mut self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.calls.record(format!("bluez:Connect({})", self.path));
        self.connected = true;
        self.connected_changed(&emitter)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    async fn disconnect(
        &mut self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.calls
            .record(format!("bluez:Disconnect({})", self.path));
        self.connected = false;
        self.connected_changed(&emitter)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    fn pair(&self) {
        self.calls.record(format!("bluez:Pair({})", self.path));
    }

    fn cancel_pairing(&self) {
        self.calls
            .record(format!("bluez:CancelPairing({})", self.path));
    }

    #[zbus(property)]
    fn address(&self) -> String {
        self.address.to_owned()
    }

    #[zbus(property)]
    fn name(&self) -> String {
        self.alias.to_owned()
    }

    #[zbus(property)]
    fn alias(&self) -> String {
        self.alias.to_owned()
    }

    #[zbus(property)]
    fn icon(&self) -> String {
        self.icon.to_owned()
    }

    #[zbus(property)]
    fn paired(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn connected(&self) -> bool {
        self.connected
    }

    #[zbus(property)]
    fn trusted(&self) -> bool {
        true
    }
}

struct BluezBattery {
    percentage: u8,
}

#[zbus::interface(name = "org.bluez.Battery1")]
impl BluezBattery {
    #[zbus(property)]
    fn percentage(&self) -> u8 {
        self.percentage
    }
}

// ------------------------------------------------------------ the system

/// Which fake services to serve.
#[derive(Debug, Clone, Copy)]
pub struct Services {
    pub upower: bool,
    pub ppd: Ppd,
    pub network_manager: bool,
    pub bluez: bool,
}

impl Services {
    /// Everything, power-profiles-daemon under the current name.
    #[must_use]
    pub fn all() -> Self {
        Self {
            upower: true,
            ppd: Ppd::Upower,
            network_manager: true,
            bluez: true,
        }
    }

    /// The bus only: every name unowned (`ServiceUnknown`).
    #[must_use]
    pub fn none() -> Self {
        Self {
            upower: false,
            ppd: Ppd::Absent,
            network_manager: false,
            bluez: false,
        }
    }
}

fn serve<I: zbus::object_server::Interface>(
    connection: &Connection,
    at: &str,
    interface: I,
) -> Result<(), FakeError> {
    connection
        .object_server()
        .at(at, interface)
        .map(drop)
        .map_err(failed)
}

/// A private bus with the chosen fake services on it. Dropping it closes the
/// services and kills the daemon.
pub struct FakeSystem {
    // Connections first: they close before the daemon goes.
    upower: Option<Connection>,
    ppd_upower: Option<Connection>,
    ppd_hadess: Option<Connection>,
    nm: Option<Connection>,
    bluez: Option<Connection>,
    calls: Calls,
    bus: PrivateBus,
}

impl FakeSystem {
    /// Starts a bus named `name` (a directory under the temp directory, made
    /// unique by the process id) and serves `services` on it.
    pub fn start(name: &str, services: Services) -> Result<Self, FakeError> {
        Self::start_activatable(name, services, &[])
    }

    /// [`FakeSystem::start`] where each of `activatable` is also a bus name
    /// the daemon can start on demand: asking for one runs [`TOUCH`] on a
    /// marker file (the name never gets an owner), so
    /// [`FakeSystem::activated`] tells whether anything asked the bus to
    /// start it.
    pub fn start_activatable(
        name: &str,
        services: Services,
        activatable: &[&str],
    ) -> Result<Self, FakeError> {
        let bus = PrivateBus::start(name, activatable)?;
        let calls = Calls::default();
        let connect = |wanted: &[&str]| -> Result<Connection, FakeError> {
            let mut builder = Builder::address(bus.address.as_str()).map_err(failed)?;
            for well_known in wanted {
                builder = builder.name(*well_known).map_err(failed)?;
            }
            builder.build().map_err(failed)
        };

        let upower = if services.upower {
            let connection = connect(&[UPOWER_NAME])?;
            serve(
                &connection,
                UPOWER_DISPLAY_PATH,
                DisplayDevice {
                    is_present: true,
                    state: 2, // discharging
                    percentage: 64.0,
                    time_to_empty: 7200,
                    time_to_full: 0,
                },
            )?;
            Some(connection)
        } else {
            None
        };

        let ppd_upower = if matches!(services.ppd, Ppd::Upower | Ppd::Both) {
            let connection = connect(&[PPD_UPOWER_NAME])?;
            serve(
                &connection,
                PPD_UPOWER_PATH,
                PpdUpower {
                    calls: calls.clone(),
                    active: "balanced".to_owned(),
                    degraded: String::new(),
                },
            )?;
            Some(connection)
        } else {
            None
        };
        let ppd_hadess = if matches!(services.ppd, Ppd::Hadess | Ppd::Both) {
            let connection = connect(&[PPD_HADESS_NAME])?;
            serve(
                &connection,
                PPD_HADESS_PATH,
                PpdHadess {
                    calls: calls.clone(),
                    active: "balanced".to_owned(),
                    degraded: String::new(),
                },
            )?;
            Some(connection)
        } else {
            None
        };

        let nm = if services.network_manager {
            let connection = connect(&[NM_NAME])?;
            serve(
                &connection,
                NM_PATH,
                NmManager {
                    calls: calls.clone(),
                    wireless_enabled: true,
                },
            )?;
            serve(&connection, NM_SETTINGS_PATH, NmSettings)?;
            serve(
                &connection,
                NM_PROFILE_PATH,
                NmProfile {
                    calls: calls.clone(),
                },
            )?;
            serve(&connection, NM_DEVICE_PATH, NmDevice)?;
            serve(&connection, NM_DEVICE_PATH, NmWireless)?;
            serve(
                &connection,
                NM_AP_HOME_PATH,
                NmAccessPoint {
                    ssid: NM_HOME_SSID,
                    strength: 80,
                    wpa_flags: 0,
                    rsn_flags: 0x100, // NM_802_11_AP_SEC_KEY_MGMT_PSK
                },
            )?;
            serve(
                &connection,
                NM_AP_OPEN_PATH,
                NmAccessPoint {
                    ssid: NM_OPEN_SSID,
                    strength: 40,
                    wpa_flags: 0,
                    rsn_flags: 0,
                },
            )?;
            serve(&connection, NM_ACTIVE_PATH, NmActive)?;
            serve(&connection, NM_IP4_PATH, NmIp4Config)?;
            Some(connection)
        } else {
            None
        };

        let bluez = if services.bluez {
            let connection = connect(&[BLUEZ_NAME])?;
            serve(&connection, "/", zbus::fdo::ObjectManager)?;
            serve(
                &connection,
                BLUEZ_ADAPTER_PATH,
                BluezAdapter {
                    calls: calls.clone(),
                    powered: true,
                },
            )?;
            serve(
                &connection,
                BLUEZ_HEADSET_PATH,
                BluezDevice {
                    calls: calls.clone(),
                    path: BLUEZ_HEADSET_PATH,
                    address: BLUEZ_HEADSET_ADDRESS,
                    alias: "Auriculares",
                    icon: "audio-headset",
                    connected: true,
                },
            )?;
            serve(
                &connection,
                BLUEZ_HEADSET_PATH,
                BluezBattery { percentage: 80 },
            )?;
            serve(
                &connection,
                BLUEZ_KEYBOARD_PATH,
                BluezDevice {
                    calls: calls.clone(),
                    path: BLUEZ_KEYBOARD_PATH,
                    address: BLUEZ_KEYBOARD_ADDRESS,
                    alias: "Teclado",
                    icon: "input-keyboard",
                    connected: false,
                },
            )?;
            Some(connection)
        } else {
            None
        };

        Ok(Self {
            upower,
            ppd_upower,
            ppd_hadess,
            nm,
            bluez,
            calls,
            bus,
        })
    }

    /// The private bus's address: what `DBUS_SYSTEM_BUS_ADDRESS` is set to.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.bus.address
    }

    /// The activatable names the bus was asked to start so far (a marker
    /// file appears when the daemon runs the service's `Exec`).
    #[must_use]
    pub fn activated(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.bus.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .and_then(|file| file.strip_prefix("activated-"))
                    .map(str::to_owned)
            })
            .collect();
        names.sort();
        names
    }

    /// The well-known names being served, in a fixed order.
    #[must_use]
    pub fn serving(&self) -> Vec<&'static str> {
        [
            (UPOWER_NAME, self.upower.is_some()),
            (PPD_UPOWER_NAME, self.ppd_upower.is_some()),
            (PPD_HADESS_NAME, self.ppd_hadess.is_some()),
            (NM_NAME, self.nm.is_some()),
            (BLUEZ_NAME, self.bluez.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, served)| served.then_some(name))
        .collect()
    }

    /// The recorded calls.
    #[must_use]
    pub fn calls(&self) -> &Calls {
        &self.calls
    }

    /// Changes the display device and publishes `PropertiesChanged`.
    pub fn set_battery(&self, percentage: f64, state: u32) -> Result<(), FakeError> {
        let connection = self.upower.as_ref().ok_or_else(|| failed("no UPower"))?;
        let reference = connection
            .object_server()
            .interface::<_, DisplayDevice>(UPOWER_DISPLAY_PATH)
            .map_err(failed)?;
        let mut device = reference.get_mut();
        device.percentage = percentage;
        device.state = state;
        zbus::block_on(async {
            device
                .percentage_changed(reference.signal_emitter())
                .await?;
            device.state_changed(reference.signal_emitter()).await
        })
        .map_err(failed)
    }

    /// Changes `ActiveProfile` under every name being served and publishes
    /// `PropertiesChanged`, as another client's change would.
    pub fn set_active_profile(&self, profile: &str) -> Result<(), FakeError> {
        if let Some(connection) = &self.ppd_upower {
            let reference = connection
                .object_server()
                .interface::<_, PpdUpower>(PPD_UPOWER_PATH)
                .map_err(failed)?;
            let mut daemon = reference.get_mut();
            daemon.active = profile.to_owned();
            zbus::block_on(daemon.active_profile_changed(reference.signal_emitter()))
                .map_err(failed)?;
        }
        if let Some(connection) = &self.ppd_hadess {
            let reference = connection
                .object_server()
                .interface::<_, PpdHadess>(PPD_HADESS_PATH)
                .map_err(failed)?;
            let mut daemon = reference.get_mut();
            daemon.active = profile.to_owned();
            zbus::block_on(daemon.active_profile_changed(reference.signal_emitter()))
                .map_err(failed)?;
        }
        Ok(())
    }

    /// Whether NetworkManager's `WirelessEnabled` is on now.
    pub fn wireless_enabled(&self) -> Result<bool, FakeError> {
        let connection = self
            .nm
            .as_ref()
            .ok_or_else(|| failed("no NetworkManager"))?;
        let reference = connection
            .object_server()
            .interface::<_, NmManager>(NM_PATH)
            .map_err(failed)?;
        let enabled = reference.get().wireless_enabled;
        Ok(enabled)
    }
}
