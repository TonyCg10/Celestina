//! The Bluetooth provider: BlueZ read through Cuprita's client
//! (`cuprita_core::bluez::BluezBluetooth`) behind `Box<dyn Bluetooth>`,
//! adapted into the shell's bounded [`BtSection`] (CEL-2-F, decisions D2-D6,
//! D10).
//!
//! Driven by the lane loop in `super` ([`super::Provider`]). Of the client's
//! commands only four are ever called: `snapshot`, `set_powered`, `connect`
//! and `disconnect`, the last two only for a device the last inventory shows
//! paired ([`validate`] refuses the rest). `set_discovering`, `pair` and
//! `forget` have no step that reaches them, and no pairing agent is
//! registered.
//!
//! The client reports no adapter as `Unavailable`, so a section exists only
//! when an adapter does. It classifies a device by kind, not by its icon
//! name, so every input device reads [`BtIcon::Pointer`].
//!
//! [`validate`]: celestina_shell::connectivity::validate

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use celestina_shell::connectivity::{
    BtIcon, BtRow, BtSection, Expected, Failure, Inventory, Resolution, Section, Service, Step,
};
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::error::BluetoothError;
use cuprita_core::model::{BluetoothDevice, BluetoothSnapshot, DeviceKind};

use super::{ConnectivityEvent, Provider, WatchGuard};

/// Opens a client: production connects to the system bus.
pub type BluetoothOpener = Box<dyn FnMut() -> Result<Box<dyn Bluetooth>, BluetoothError> + Send>;
/// Starts a change watcher that raises the flag it is given on every
/// coalesced burst; `None` when it could not start.
pub type BluetoothWatcher = Box<dyn FnMut(Arc<AtomicBool>) -> Option<WatchGuard> + Send>;

/// The Bluetooth lane's provider.
pub struct BluetoothProvider {
    opener: BluetoothOpener,
    watcher: BluetoothWatcher,
    backend: Option<Box<dyn Bluetooth>>,
}

impl std::fmt::Debug for BluetoothProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BluetoothProvider")
            .field("open", &self.backend.is_some())
            .finish_non_exhaustive()
    }
}

impl BluetoothProvider {
    /// A provider over `opener` and `watcher`; tests pass fakes.
    #[must_use]
    pub fn new(opener: BluetoothOpener, watcher: BluetoothWatcher) -> Self {
        Self {
            opener,
            watcher,
            backend: None,
        }
    }

    /// The provider over the system bus: `BluezBluetooth::connect_system` and
    /// `cuprita_core::bluez::watch`.
    #[must_use]
    pub fn system() -> Self {
        Self::new(
            Box::new(|| {
                cuprita_core::bluez::BluezBluetooth::connect_system()
                    .map(|client| Box::new(client) as Box<dyn Bluetooth>)
            }),
            Box::new(|dirty: Arc<AtomicBool>| {
                match cuprita_core::bluez::watch(move || dirty.store(true, Ordering::Release)) {
                    Ok(handle) => Some(Box::new(handle) as WatchGuard),
                    Err(err) => {
                        tracing::warn!(
                            kind = error_kind(&err),
                            "bluetooth: no change watcher; re-reading periodically"
                        );
                        None
                    }
                }
            }),
        )
    }

    fn backend(&mut self) -> Result<&mut Box<dyn Bluetooth>, Failure> {
        self.backend.as_mut().ok_or(Failure::Unreachable)
    }
}

/// Cuprita's device kind as the shell's icon class.
fn icon_of(kind: DeviceKind) -> BtIcon {
    match kind {
        DeviceKind::Audio => BtIcon::Audio,
        DeviceKind::Input => BtIcon::Pointer,
        DeviceKind::Phone => BtIcon::Phone,
        DeviceKind::Computer => BtIcon::Computer,
        DeviceKind::Other => BtIcon::Other,
    }
}

/// One of Cuprita's devices as a shell row; `None` for an address outside the
/// grammar (the device is then not shown and cannot be named by a request).
fn row_of(device: &BluetoothDevice) -> Option<BtRow> {
    BtRow::new(
        &device.address,
        &device.name,
        icon_of(device.kind),
        device.paired,
        device.connected,
        device.trusted,
        device.battery,
    )
}

fn convert(snapshot: &BluetoothSnapshot) -> BtSection {
    BtSection::new(
        snapshot.powered,
        snapshot.discovering,
        snapshot.devices.iter().filter_map(row_of),
    )
}

/// A client error by kind; the detail is never shown or logged.
pub(super) fn failure_of(err: &BluetoothError) -> Failure {
    match err {
        BluetoothError::Unavailable => Failure::Unreachable,
        BluetoothError::Denied => Failure::Denied,
        BluetoothError::NotFound(_) => Failure::NotFound,
        BluetoothError::Busy => Failure::Busy,
        BluetoothError::Failed(_) => Failure::Other,
    }
}

/// A stable journal token for `err`, without its payload.
pub(super) fn error_kind(err: &BluetoothError) -> &'static str {
    match err {
        BluetoothError::Unavailable => "unavailable",
        BluetoothError::Denied => "denied",
        BluetoothError::NotFound(_) => "not-found",
        BluetoothError::Busy => "busy",
        BluetoothError::Failed(_) => "failed",
    }
}

impl Provider for BluetoothProvider {
    type State = BtSection;
    const WHAT: &'static str = "bluetooth";
    const SECTION: Section = Section::Bluetooth;

    fn open(&mut self) -> Result<(), Failure> {
        let backend = (self.opener)().map_err(|err| {
            tracing::debug!(kind = error_kind(&err), "bluetooth: cannot open the client");
            failure_of(&err)
        })?;
        self.backend = Some(backend);
        Ok(())
    }

    fn close(&mut self) {
        self.backend = None;
    }

    fn watch(&mut self, dirty: Arc<AtomicBool>) -> Option<WatchGuard> {
        (self.watcher)(dirty)
    }

    fn read(&mut self) -> Result<BtSection, Failure> {
        let snapshot = self.backend()?.snapshot().map_err(|err| {
            tracing::debug!(kind = error_kind(&err), "bluetooth: snapshot failed");
            failure_of(&err)
        })?;
        Ok(convert(&snapshot))
    }

    fn inventory(state: Option<&BtSection>) -> Inventory<'_> {
        Inventory {
            bluetooth: state,
            ..Inventory::default()
        }
    }

    fn perform(&mut self, step: &Step, _state: Option<&BtSection>) -> Result<(), Failure> {
        let result = match step {
            Step::SetBluetoothPowered(on) => self.backend()?.set_powered(*on),
            Step::ConnectDevice(address) => self.backend()?.connect(address.as_str()),
            Step::DisconnectDevice(address) => self.backend()?.disconnect(address.as_str()),
            other => {
                tracing::warn!(
                    step = super::step_kind(other),
                    "bluetooth: a step for another lane was dropped"
                );
                return Err(Failure::Other);
            }
        };
        result.map_err(|err| {
            tracing::info!(kind = error_kind(&err), "bluetooth: request failed");
            failure_of(&err)
        })
    }

    fn take_observed(&mut self) -> Vec<(Expected, Failure)> {
        Vec::new()
    }

    fn settled(&mut self, _resolution: &Resolution) {}

    fn event(state: Service<BtSection>) -> ConnectivityEvent {
        ConnectivityEvent::Bluetooth(state)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use celestina_shell::connectivity::BtAddress;
    use cuprita_core::fake::FakeBluetooth;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Every call the provider made into the client.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum BtCall {
        Snapshot,
        SetPowered(bool),
        SetDiscovering(bool),
        Pair(String),
        Connect(String),
        Disconnect(String),
        Forget(String),
    }

    /// What a test controls about the recording client.
    #[derive(Default)]
    pub(crate) struct BtScript {
        pub(crate) calls: Vec<BtCall>,
        /// Errors the next commands (not snapshots) return, in order.
        pub(crate) fail_next: VecDeque<BluetoothError>,
        /// While set, every call answers `Unavailable`.
        pub(crate) gone: bool,
    }

    pub(crate) type Script = Arc<Mutex<BtScript>>;

    /// `FakeBluetooth` behind a recorder, shared across reconnects.
    pub(crate) struct Recording {
        fake: Arc<Mutex<FakeBluetooth>>,
        script: Script,
    }

    impl Recording {
        fn note(&self, call: BtCall) -> Result<(), BluetoothError> {
            let mut script = self.script.lock().expect("script");
            script.calls.push(call.clone());
            if script.gone {
                return Err(BluetoothError::Unavailable);
            }
            if call != BtCall::Snapshot {
                if let Some(err) = script.fail_next.pop_front() {
                    return Err(err);
                }
            }
            Ok(())
        }

        fn fake(&self) -> std::sync::MutexGuard<'_, FakeBluetooth> {
            self.fake.lock().expect("fake")
        }
    }

    impl Bluetooth for Recording {
        fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError> {
            self.note(BtCall::Snapshot)?;
            self.fake().snapshot()
        }
        fn set_powered(&mut self, on: bool) -> Result<(), BluetoothError> {
            self.note(BtCall::SetPowered(on))?;
            self.fake().set_powered(on)
        }
        fn set_discovering(&mut self, on: bool) -> Result<(), BluetoothError> {
            self.note(BtCall::SetDiscovering(on))?;
            self.fake().set_discovering(on)
        }
        fn pair(&mut self, address: &str) -> Result<(), BluetoothError> {
            self.note(BtCall::Pair(address.to_owned()))?;
            self.fake().pair(address)
        }
        fn connect(&mut self, address: &str) -> Result<(), BluetoothError> {
            self.note(BtCall::Connect(address.to_owned()))?;
            self.fake().connect(address)
        }
        fn disconnect(&mut self, address: &str) -> Result<(), BluetoothError> {
            self.note(BtCall::Disconnect(address.to_owned()))?;
            self.fake().disconnect(address)
        }
        fn forget(&mut self, address: &str) -> Result<(), BluetoothError> {
            self.note(BtCall::Forget(address.to_owned()))?;
            self.fake().forget(address)
        }
    }

    /// A provider over a recording `FakeBluetooth::scripted()`.
    pub(crate) fn recording(script: &Script) -> BluetoothProvider {
        let fake = Arc::new(Mutex::new(FakeBluetooth::scripted()));
        let opener_script = Arc::clone(script);
        let opener: BluetoothOpener = Box::new(move || {
            if opener_script.lock().expect("script").gone {
                return Err(BluetoothError::Unavailable);
            }
            Ok(Box::new(Recording {
                fake: Arc::clone(&fake),
                script: Arc::clone(&opener_script),
            }) as Box<dyn Bluetooth>)
        });
        let watcher: BluetoothWatcher = Box::new(|_dirty| Some(Box::new(()) as WatchGuard));
        BluetoothProvider::new(opener, watcher)
    }

    fn opened(script: &Script) -> BluetoothProvider {
        let mut provider = recording(script);
        provider.open().expect("opens");
        provider
    }

    fn address(raw: &str) -> BtAddress {
        BtAddress::parse(raw).expect("address")
    }

    #[test]
    fn the_scripted_snapshot_becomes_a_section() {
        let script = Script::default();
        let mut provider = opened(&script);

        let section = provider.read().expect("reads");

        assert!(section.powered());
        assert!(!section.discovering());
        let headphones = section
            .row(&address("AA:BB:CC:00:00:01"))
            .expect("headphones");
        assert_eq!(headphones.icon(), BtIcon::Audio);
        assert!(headphones.paired());
        assert_eq!(headphones.battery(), Some(80));
        assert!(section.row(&address("AA:BB:CC:00:00:02")).is_some());
    }

    #[test]
    fn a_device_with_a_hostile_address_is_dropped() {
        let device = |address: &str| BluetoothDevice {
            address: address.to_owned(),
            name: "x\u{0}y".to_owned(),
            kind: DeviceKind::Input,
            paired: true,
            connected: false,
            trusted: false,
            battery: Some(200),
        };
        assert!(row_of(&device("not an address")).is_none());
        assert!(row_of(&device("")).is_none());
        let row = row_of(&device("aa:bb:cc:dd:ee:ff")).expect("kept");
        assert_eq!(row.address().as_str(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(row.battery(), Some(100));
        assert_eq!(row.icon(), BtIcon::Pointer);
        assert!(!row.alias().contains('\u{0}'));
    }

    #[test]
    fn only_power_connect_and_disconnect_reach_the_client() {
        let script = Script::default();
        let mut provider = opened(&script);
        let section = provider.read().expect("reads");
        let headphones = address("AA:BB:CC:00:00:01");

        provider
            .perform(&Step::ConnectDevice(headphones.clone()), Some(&section))
            .expect("connects");
        provider
            .perform(&Step::DisconnectDevice(headphones), Some(&section))
            .expect("disconnects");
        provider
            .perform(&Step::SetBluetoothPowered(false), Some(&section))
            .expect("powers off");
        assert_eq!(
            provider.perform(&Step::SetWifiEnabled(true), Some(&section)),
            Err(Failure::Other)
        );

        let calls = script.lock().expect("script").calls.clone();
        assert_eq!(
            calls,
            vec![
                BtCall::Snapshot,
                BtCall::Connect("AA:BB:CC:00:00:01".to_owned()),
                BtCall::Disconnect("AA:BB:CC:00:00:01".to_owned()),
                BtCall::SetPowered(false),
            ]
        );
    }

    #[test]
    fn client_errors_map_by_kind() {
        assert_eq!(
            failure_of(&BluetoothError::Unavailable),
            Failure::Unreachable
        );
        assert_eq!(failure_of(&BluetoothError::Denied), Failure::Denied);
        assert_eq!(
            failure_of(&BluetoothError::NotFound(String::new())),
            Failure::NotFound
        );
        assert_eq!(failure_of(&BluetoothError::Busy), Failure::Busy);
        assert_eq!(
            failure_of(&BluetoothError::pairing_cancelled()),
            Failure::Other
        );
    }
}
