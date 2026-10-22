//! The real `NmNetwork` and `BluezBluetooth` clients of `cuprita-core` read
//! the fake system services (`crates/celestina-fake-system`) on a private bus,
//! through `DBUS_SYSTEM_BUS_ADDRESS`, and the fake UPower and
//! power-profiles-daemon answer the calls the power worker will make.
//!
//! No test reaches the real system bus: each one starts a `dbus-daemon` of its
//! own and points the variable at it. The variable is process-wide, so the
//! tests hold one lock while they run. Without `dbus-daemon` they say so and
//! pass.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::bluez::BluezBluetooth;
use cuprita_core::error::{BluetoothError, NetworkError};
use cuprita_core::model::{NetworkKind, NetworkState, Security};
use cuprita_core::network::Network;
use cuprita_core::nm::NmNetwork;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type as MessageType;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

use celestina_fake_system as fakes;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// A running fake system whose bus the clients find as the system bus.
struct Fixture {
    system: fakes::FakeSystem,
    _guard: MutexGuard<'static, ()>,
}

fn fixture(name: &str, services: fakes::Services) -> Option<Fixture> {
    let guard = ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    match fakes::FakeSystem::start(name, services) {
        Ok(system) => {
            std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", system.address());
            Some(Fixture {
                system,
                _guard: guard,
            })
        }
        Err(fakes::FakeError::DaemonMissing) => {
            eprintln!("dbus-daemon is not installed: the connectivity bus test is skipped");
            None
        }
        Err(error) => panic!("{error}"),
    }
}

fn connection(fixture: &Fixture) -> Connection {
    match zbus::blocking::connection::Builder::address(fixture.system.address())
        .and_then(zbus::blocking::connection::Builder::build)
    {
        Ok(connection) => connection,
        Err(error) => panic!("cannot connect to the fake bus: {error}"),
    }
}

fn proxy<'a>(
    connection: &Connection,
    name: &'a str,
    path: &'a str,
    interface: &'a str,
) -> Proxy<'a> {
    // No property cache: the tests read a value again after it changed.
    let built = zbus::blocking::proxy::Builder::new(connection)
        .destination(name)
        .and_then(|b| b.path(path))
        .and_then(|b| b.interface(interface))
        .map(|b| b.cache_properties(CacheProperties::No))
        .and_then(|b| b.build());
    match built {
        Ok(proxy) => proxy,
        Err(error) => panic!("cannot build a proxy to {name}: {error}"),
    }
}

#[test]
fn network_manager_is_read_through_the_real_client() {
    let Some(fx) = fixture("conn-nm-read", fakes::Services::all()) else {
        return;
    };
    assert_eq!(
        fx.system.serving(),
        vec![
            fakes::UPOWER_NAME,
            fakes::PPD_UPOWER_NAME,
            "org.freedesktop.NetworkManager",
            "org.bluez"
        ]
    );
    let mut client = NmNetwork::connect_system().expect("connect to the fake system bus");
    let snapshot = client.snapshot().expect("snapshot");
    assert!(snapshot.wifi_enabled);
    assert_eq!(snapshot.networks.len(), 2);

    let home = snapshot
        .networks
        .iter()
        .find(|n| n.name == fakes::NM_HOME_SSID)
        .expect("the home network");
    assert_eq!(home.kind, NetworkKind::Wifi);
    assert_eq!(home.state, NetworkState::Connected);
    assert_eq!(home.signal, Some(80));
    assert_eq!(home.security, Security::Psk);
    assert!(home.known);
    assert_eq!(home.address.as_deref(), Some(fakes::NM_HOME_ADDRESS));

    let open = snapshot
        .networks
        .iter()
        .find(|n| n.name == fakes::NM_OPEN_SSID)
        .expect("the open network");
    assert_eq!(open.state, NetworkState::Disconnected);
    assert_eq!(open.signal, Some(40));
    assert_eq!(open.security, Security::Open);
    assert!(!open.known);
    assert_eq!(open.address, None);

    // Reading calls nothing that changes a thing.
    assert_eq!(fx.system.calls().all(), Vec::<String>::new());
}

#[test]
fn network_manager_calls_are_recorded_and_what_was_not_called_is_provable() {
    let Some(fx) = fixture("conn-nm-calls", fakes::Services::all()) else {
        return;
    };
    let mut client = NmNetwork::connect_system().expect("connect");
    let snapshot = client.snapshot().expect("snapshot");
    let home = snapshot
        .networks
        .iter()
        .find(|n| n.name == fakes::NM_HOME_SSID)
        .expect("home")
        .id
        .clone();

    client.disconnect(&home).expect("leave the active link");
    assert_eq!(
        fx.system.calls().all(),
        vec![format!(
            "nm:DeactivateConnection({})",
            fakes::NM_ACTIVE_PATH
        )]
    );

    fx.system.calls().clear();
    client.connect(&home, None).expect("join the saved profile");
    assert_eq!(
        fx.system.calls().all(),
        vec![format!(
            "nm:ActivateConnection({},{},{})",
            fakes::NM_PROFILE_PATH,
            fakes::NM_DEVICE_PATH,
            fakes::NM_AP_HOME_PATH
        )]
    );
    assert_eq!(fx.system.calls().count("nm:AddAndActivateConnection2"), 0);
    assert_eq!(fx.system.calls().count("nm:Delete"), 0);

    fx.system.calls().clear();
    client
        .set_wifi_enabled(false)
        .expect("switch the radio off");
    assert_eq!(
        fx.system.calls().all(),
        vec!["nm:Set WirelessEnabled=false"]
    );
    assert!(!fx.system.wireless_enabled().expect("state"));
    let after = client.snapshot().expect("snapshot");
    assert!(!after.wifi_enabled);
    assert!(after.networks.is_empty());
}

#[test]
fn bluez_is_read_through_the_real_client() {
    let Some(fx) = fixture("conn-bluez-read", fakes::Services::all()) else {
        return;
    };
    let mut client = BluezBluetooth::connect_system().expect("connect");
    let snapshot = client.snapshot().expect("snapshot");
    assert!(snapshot.powered);
    assert!(!snapshot.discovering);
    assert_eq!(snapshot.devices.len(), 2);

    let headset = snapshot
        .devices
        .iter()
        .find(|d| d.address == fakes::BLUEZ_HEADSET_ADDRESS)
        .expect("headset");
    assert_eq!(headset.name, "Auriculares");
    assert!(headset.paired && headset.connected && headset.trusted);
    assert_eq!(headset.battery, Some(80));

    let keyboard = snapshot
        .devices
        .iter()
        .find(|d| d.address == fakes::BLUEZ_KEYBOARD_ADDRESS)
        .expect("keyboard");
    assert!(keyboard.paired && !keyboard.connected);
    assert_eq!(keyboard.battery, None);

    assert_eq!(fx.system.calls().all(), Vec::<String>::new());
}

#[test]
fn bluez_calls_are_recorded_and_pairing_is_never_called() {
    let Some(fx) = fixture("conn-bluez-calls", fakes::Services::all()) else {
        return;
    };
    let mut client = BluezBluetooth::connect_system().expect("connect");
    client.snapshot().expect("snapshot");

    client
        .connect(fakes::BLUEZ_KEYBOARD_ADDRESS)
        .expect("connect the keyboard");
    assert_eq!(
        fx.system.calls().all(),
        vec![format!("bluez:Connect({})", fakes::BLUEZ_KEYBOARD_PATH)]
    );
    let snapshot = client.snapshot().expect("snapshot");
    let keyboard = snapshot
        .devices
        .iter()
        .find(|d| d.address == fakes::BLUEZ_KEYBOARD_ADDRESS)
        .expect("keyboard");
    assert!(keyboard.connected);

    fx.system.calls().clear();
    client
        .disconnect(fakes::BLUEZ_HEADSET_ADDRESS)
        .expect("disconnect");
    client.set_powered(false).expect("power off");
    assert_eq!(
        fx.system.calls().all(),
        vec![
            format!("bluez:Disconnect({})", fakes::BLUEZ_HEADSET_PATH),
            "bluez:Set Powered=false".to_owned(),
        ]
    );
    for never in ["bluez:Pair", "bluez:RemoveDevice", "bluez:StartDiscovery"] {
        assert_eq!(fx.system.calls().count(never), 0, "{never}");
    }
    // An adapter that is off lists nothing.
    let off = client.snapshot().expect("snapshot");
    assert!(!off.powered);
    assert!(off.devices.is_empty());
}

#[test]
fn absent_services_read_as_unavailable() {
    let Some(_fx) = fixture("conn-absent", fakes::Services::none()) else {
        return;
    };
    let mut network = NmNetwork::connect_system().expect("the bus itself is there");
    assert_eq!(network.snapshot().unwrap_err(), NetworkError::Unavailable);
    let mut bluetooth = BluezBluetooth::connect_system().expect("the bus itself is there");
    assert_eq!(
        bluetooth.snapshot().unwrap_err(),
        BluetoothError::Unavailable
    );
}

fn read<T: TryFrom<OwnedValue>>(proxy: &Proxy<'_>, property: &str) -> T
where
    T::Error: Into<zbus::Error>,
{
    match proxy.get_property::<T>(property) {
        Ok(value) => value,
        Err(error) => panic!("cannot read {property}: {error}"),
    }
}

#[test]
fn upower_display_device_is_read_and_its_changes_are_published() {
    let Some(fx) = fixture("conn-upower", fakes::Services::all()) else {
        return;
    };
    let client = connection(&fx);
    let device = proxy(
        &client,
        fakes::UPOWER_NAME,
        fakes::UPOWER_DISPLAY_PATH,
        "org.freedesktop.UPower.Device",
    );
    assert!(read::<bool>(&device, "IsPresent"));
    assert_eq!(read::<u32>(&device, "State"), 2);
    assert!((read::<f64>(&device, "Percentage") - 64.0).abs() < f64::EPSILON);
    assert_eq!(read::<i64>(&device, "TimeToEmpty"), 7200);
    assert_eq!(read::<i64>(&device, "TimeToFull"), 0);

    let changes = changed_signals(&client, fakes::UPOWER_DISPLAY_PATH);
    fx.system.set_battery(55.0, 1).expect("change the battery");
    assert!(changes
        .recv_timeout(Duration::from_secs(5))
        .is_ok_and(|names| names.contains(&"Percentage".to_owned())));
    assert!((read::<f64>(&device, "Percentage") - 55.0).abs() < f64::EPSILON);
    assert_eq!(read::<u32>(&device, "State"), 1);
}

/// Forwards the property names of each `PropertiesChanged` on `path`.
fn changed_signals(connection: &Connection, path: &str) -> std::sync::mpsc::Receiver<Vec<String>> {
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface("org.freedesktop.DBus.Properties")
        .and_then(|b| b.member("PropertiesChanged"))
        .and_then(|b| b.path(path.to_owned()))
        .map(zbus::match_rule::Builder::build);
    let iterator =
        match rule.and_then(|rule| MessageIterator::for_match_rule(rule, connection, Some(16))) {
            Ok(iterator) => iterator,
            Err(error) => panic!("cannot subscribe: {error}"),
        };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for message in iterator {
            let Ok(message) = message else { break };
            let body = message.body();
            let Ok((_interface, changed, _invalidated)) = body.deserialize::<(
                String,
                std::collections::HashMap<String, OwnedValue>,
                Vec<String>,
            )>() else {
                continue;
            };
            if tx.send(changed.into_keys().collect()).is_err() {
                break;
            }
        }
    });
    rx
}

fn ppd_checks(name: &'static str, path: &'static str, interface: &'static str, record: &str) {
    let services = fakes::Services {
        ppd: if name == fakes::PPD_UPOWER_NAME {
            fakes::Ppd::Upower
        } else {
            fakes::Ppd::Hadess
        },
        ..fakes::Services::all()
    };
    let Some(fx) = fixture(&format!("conn-ppd-{record}"), services) else {
        return;
    };
    let client = connection(&fx);
    let daemon = proxy(&client, name, path, interface);
    assert_eq!(read::<String>(&daemon, "ActiveProfile"), "balanced");
    let profiles = read::<Vec<std::collections::HashMap<String, OwnedValue>>>(&daemon, "Profiles");
    let listed: Vec<String> = profiles
        .iter()
        .filter_map(|p| {
            p.get("Profile")
                .and_then(|v| String::try_from(v.clone()).ok())
        })
        .collect();
    assert_eq!(listed, fakes::PPD_PROFILES);
    assert_eq!(read::<String>(&daemon, "PerformanceDegraded"), "");

    // A set the daemon accepts is recorded and read back.
    assert!(daemon.set_property("ActiveProfile", "performance").is_ok());
    assert_eq!(
        fx.system.calls().all(),
        vec![format!("{record}:Set ActiveProfile=performance")]
    );
    assert_eq!(read::<String>(&daemon, "ActiveProfile"), "performance");

    // Another client's change is published.
    let changes = changed_signals(&client, path);
    fx.system.set_active_profile("power-saver").expect("change");
    assert!(changes
        .recv_timeout(Duration::from_secs(5))
        .is_ok_and(|names| names.contains(&"ActiveProfile".to_owned())));
}

#[test]
fn power_profiles_answer_under_the_upower_name() {
    ppd_checks(
        fakes::PPD_UPOWER_NAME,
        fakes::PPD_UPOWER_PATH,
        "org.freedesktop.UPower.PowerProfiles",
        "ppd",
    );
}

#[test]
fn power_profiles_answer_under_the_hadess_name() {
    ppd_checks(
        fakes::PPD_HADESS_NAME,
        fakes::PPD_HADESS_PATH,
        "net.hadess.PowerProfiles",
        "ppd-hadess",
    );
}

#[test]
fn both_power_profile_names_can_be_served_at_once() {
    let services = fakes::Services {
        ppd: fakes::Ppd::Both,
        ..fakes::Services::all()
    };
    let Some(fx) = fixture("conn-ppd-both", services) else {
        return;
    };
    let client = connection(&fx);
    let upower = proxy(
        &client,
        fakes::PPD_UPOWER_NAME,
        fakes::PPD_UPOWER_PATH,
        "org.freedesktop.UPower.PowerProfiles",
    );
    let hadess = proxy(
        &client,
        fakes::PPD_HADESS_NAME,
        fakes::PPD_HADESS_PATH,
        "net.hadess.PowerProfiles",
    );
    fx.system.set_active_profile("performance").expect("change");
    assert_eq!(read::<String>(&upower, "ActiveProfile"), "performance");
    assert_eq!(read::<String>(&hadess, "ActiveProfile"), "performance");
}

#[test]
fn only_the_selected_power_profile_name_is_served() {
    let services = fakes::Services {
        ppd: fakes::Ppd::Hadess,
        ..fakes::Services::all()
    };
    let Some(fx) = fixture("conn-ppd-select", services) else {
        return;
    };
    let client = connection(&fx);
    let absent = proxy(
        &client,
        fakes::PPD_UPOWER_NAME,
        fakes::PPD_UPOWER_PATH,
        "org.freedesktop.UPower.PowerProfiles",
    );
    assert!(absent.get_property::<String>("ActiveProfile").is_err());
    let present = proxy(
        &client,
        fakes::PPD_HADESS_NAME,
        fakes::PPD_HADESS_PATH,
        "net.hadess.PowerProfiles",
    );
    assert_eq!(read::<String>(&present, "ActiveProfile"), "balanced");
}

#[test]
fn a_profile_the_daemon_does_not_list_is_refused_but_recorded() {
    let Some(fx) = fixture("conn-ppd-refuse", fakes::Services::all()) else {
        return;
    };
    let client = connection(&fx);
    let daemon = proxy(
        &client,
        fakes::PPD_UPOWER_NAME,
        fakes::PPD_UPOWER_PATH,
        "org.freedesktop.UPower.PowerProfiles",
    );
    assert!(daemon.set_property("ActiveProfile", "turbo").is_err());
    assert_eq!(fx.system.calls().count("ppd:Set ActiveProfile=turbo"), 1);
    assert_eq!(read::<String>(&daemon, "ActiveProfile"), "balanced");
}
