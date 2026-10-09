//! The Bluetooth section's adapter switches, device commands and pairing
//! agent. The device list is `DeviceModel`, fed through `DEVICES`.
//!
//! Airplane mode is the network controller's switch, reported here through
//! `set_airplane` whenever the network side's flag changes (a toggle, or a
//! snapshot that already reads airplane at launch). Turning it on powers the
//! adapter off on this section's worker; while it is on, switching the
//! adapter on is refused with a notice and the page shows the switch
//! disabled. Turning airplane mode off does not power the adapter back on:
//! the person switches Bluetooth on here.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;

use cuprita_core::agent::{Agent, AgentAnswer, AgentRequest};
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::error::{airplane_mode_message, BluetoothError};
use cuprita_core::model::BluetoothSnapshot;

use super::notice::NoticeKind;
use super::{Hub, Report, Worker};
use crate::{agent, backend};

/// The trait object the worker drives.
type Backend = dyn Bluetooth + 'static;

/// The device snapshots `DeviceModel` subscribes to.
pub static DEVICES: Hub<BluetoothSnapshot> = Hub::new();

/// Whether airplane mode is on, as the network side last reported it.
static AIRPLANE_ON: AtomicBool = AtomicBool::new(false);
/// This section's worker, for the network controller's airplane switch.
static AIRPLANE: Mutex<Option<Worker<Backend>>> = Mutex::new(None);
/// This section's controller, told when airplane mode changes.
static CONTROLLER: Mutex<Option<CxxQtThread<qobject::BluetoothController>>> = Mutex::new(None);

/// Powers the adapter off for airplane mode. A machine without BlueZ has no
/// adapter to power off: that is logged, not shown.
fn power_off(backend: &mut Backend) -> Result<(), String> {
    match backend.set_powered(false) {
        Err(BluetoothError::Unavailable) => {
            eprintln!("Cuprita: airplane mode: no Bluetooth adapter to power off");
            Ok(())
        }
        other => other.map_err(|e| e.message_es()),
    }
}

/// Switches the adapter, unless airplane mode is on and the switch is to on:
/// then nothing happens (the controller already said why). Checked on the
/// worker as well, so a command queued before airplane mode went on cannot
/// power the adapter back on after it.
fn power_job(on: bool) -> impl FnOnce(&mut Backend) -> Result<(), BluetoothError> + Send {
    move |backend: &mut Backend| {
        if on && AIRPLANE_ON.load(Ordering::SeqCst) {
            Ok(())
        } else {
            backend.set_powered(on)
        }
    }
}

/// The network side reports airplane mode: on powers the adapter off.
/// `false` when that power-off could not be queued (no Bluetooth worker).
pub fn set_airplane(on: bool) -> bool {
    let was = AIRPLANE_ON.swap(on, Ordering::SeqCst);
    if was != on {
        if let Some(qt) = CONTROLLER
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let _ = qt.queue(move |controller: Pin<&mut qobject::BluetoothController>| {
                controller.set_airplane_value(on);
            });
        }
    }
    if !on || was {
        return true;
    }
    AIRPLANE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|worker| worker.run_quiet(power_off))
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, powered, READ, NOTIFY)]
        #[qproperty(bool, discovering, READ, NOTIFY)]
        #[qproperty(bool, busy, READ, NOTIFY)]
        // False until the first snapshot arrives; true from then on.
        #[qproperty(bool, loaded, READ, NOTIFY)]
        /// Airplane mode is on: the adapter stays off.
        #[qproperty(bool, airplane, READ, NOTIFY)]
        type BluetoothController = super::BluetoothControllerRust;

        #[qsignal]
        fn notice(self: Pin<&mut BluetoothController>, kind: QString, text: QString);

        /// The pairing agent asks the person: `kind` is `pin`, `confirm` or
        /// `display`; `passkey` is 0 for a PIN request.
        #[qsignal]
        fn agent_request(
            self: Pin<&mut BluetoothController>,
            kind: QString,
            device: QString,
            passkey: u32,
        );

        /// The waiting agent request is over (BlueZ withdrew it, or the
        /// pairing that showed a passkey finished): the dialog closes.
        #[qsignal]
        fn agent_closed(self: Pin<&mut BluetoothController>);

        #[qinvokable]
        fn set_powered(self: Pin<&mut BluetoothController>, on: bool);
        #[qinvokable]
        fn set_discovering(self: Pin<&mut BluetoothController>, on: bool);
        #[qinvokable]
        fn pair(self: Pin<&mut BluetoothController>, address: &QString);
        #[qinvokable]
        fn connect(self: Pin<&mut BluetoothController>, address: &QString);
        #[qinvokable]
        fn disconnect(self: Pin<&mut BluetoothController>, address: &QString);
        #[qinvokable]
        fn forget(self: Pin<&mut BluetoothController>, address: &QString);
        /// Answers the waiting agent request of `kind`: a `pin` with the PIN
        /// as `value`, a `confirm` with `yes` or `no`, any with `cancel`.
        #[qinvokable]
        fn answer_agent(self: Pin<&mut BluetoothController>, kind: &QString, value: &QString);
    }

    impl cxx_qt::Threading for BluetoothController {}
    impl cxx_qt::Initialize for BluetoothController {}
}

#[derive(Default)]
pub struct BluetoothControllerRust {
    powered: bool,
    discovering: bool,
    busy: bool,
    loaded: bool,
    pending: u32,
    airplane: bool,
    /// One entry per queued command, in order: the address of a `pair`, or
    /// `None`. The worker reports `Done` in the same order.
    jobs: VecDeque<Option<String>>,
    /// The latest snapshot's devices, to find a shown passkey's device.
    devices: Option<Arc<BluetoothSnapshot>>,
    agent: Agent,
    /// Where answers go back to the BlueZ agent.
    answers: Option<Sender<AgentAnswer>>,
    worker: Option<Worker<Backend>>,
}

impl cxx_qt::Initialize for qobject::BluetoothController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        // No poll: the backend's watcher asks for every re-read.
        let worker = Worker::start(
            move |refresh| {
                backend::bluetooth(move || {
                    refresh.request();
                })
            },
            None,
            |backend: &mut Backend| backend.snapshot().map_err(|e| e.message_es()),
            move |report| {
                let _ = qt.queue(move |controller: Pin<&mut qobject::BluetoothController>| {
                    controller.apply(report);
                });
            },
        );
        AIRPLANE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone_from(&worker);
        *CONTROLLER.lock().unwrap_or_else(PoisonError::into_inner) = Some(self.qt_thread());
        let airplane = AIRPLANE_ON.load(Ordering::SeqCst);
        self.as_mut().set_airplane_value(airplane);
        self.as_mut().rust_mut().worker = worker;
        let answers = agent::start(self.qt_thread());
        self.as_mut().rust_mut().answers = Some(answers);
    }
}

impl qobject::BluetoothController {
    fn apply(mut self: Pin<&mut Self>, report: Report<BluetoothSnapshot>) {
        match report {
            Report::Snapshot(snapshot) => {
                if self.rust().powered != snapshot.powered {
                    self.as_mut().rust_mut().powered = snapshot.powered;
                    self.as_mut().powered_changed();
                }
                if self.rust().discovering != snapshot.discovering {
                    self.as_mut().rust_mut().discovering = snapshot.discovering;
                    self.as_mut().discovering_changed();
                }
                // A passkey the device showed is over once it is paired.
                let paired = match self.rust().agent.pending() {
                    Some(AgentRequest::DisplayPasskey { device, .. }) => snapshot
                        .devices
                        .iter()
                        .any(|d| d.paired && d.name == *device),
                    _ => false,
                };
                if paired {
                    self.as_mut().close_agent();
                }
                self.as_mut().rust_mut().devices = Some(Arc::clone(&snapshot));
                DEVICES.publish(Arc::clone(&snapshot));
                if !self.rust().loaded {
                    // The models queued their copy of this snapshot just now,
                    // on this same thread: flipping `loaded` from a call queued
                    // after theirs means no frame sees it true with lists
                    // still empty.
                    let queued = self.qt_thread().queue(
                        |mut controller: Pin<&mut qobject::BluetoothController>| {
                            if !controller.rust().loaded {
                                controller.as_mut().rust_mut().loaded = true;
                                controller.as_mut().loaded_changed();
                            }
                        },
                    );
                    if queued.is_err() {
                        eprintln!("Cuprita: the loaded flag could not be queued");
                    }
                }
            }
            Report::Failed(text) => self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(text.as_str()),
            ),
            Report::Done => {
                let pending = self.rust().pending.saturating_sub(1);
                self.as_mut().rust_mut().pending = pending;
                self.as_mut().set_busy(pending > 0);
                // A shown passkey has no answer: it is over when the pairing
                // that asked for it returns.
                let finished = self.as_mut().rust_mut().jobs.pop_front().flatten();
                if finished.is_some()
                    && matches!(
                        self.rust().agent.pending(),
                        Some(AgentRequest::DisplayPasskey { .. })
                    )
                {
                    self.as_mut().close_agent();
                }
            }
        }
    }

    fn close_agent(mut self: Pin<&mut Self>) {
        let _ = self.as_mut().rust_mut().agent.request(AgentRequest::Cancel);
        self.agent_closed();
    }

    fn set_airplane_value(mut self: Pin<&mut Self>, on: bool) {
        if self.rust().airplane != on {
            self.as_mut().rust_mut().airplane = on;
            self.airplane_changed();
        }
    }

    /// The address of the device whose passkey is shown: the one being
    /// paired from here, else the snapshot's device of that name.
    fn shown_address(&self, device: &str) -> Option<String> {
        if let Some(address) = self.rust().jobs.iter().flatten().next() {
            return Some(address.clone());
        }
        self.rust()
            .devices
            .as_ref()?
            .devices
            .iter()
            .find(|d| d.name == device)
            .map(|d| d.address.clone())
    }

    fn set_busy(mut self: Pin<&mut Self>, busy: bool) {
        if self.rust().busy != busy {
            self.as_mut().rust_mut().busy = busy;
            self.busy_changed();
        }
    }

    fn dispatch(
        self: Pin<&mut Self>,
        job: impl FnOnce(&mut Backend) -> Result<(), BluetoothError> + Send + 'static,
    ) {
        self.dispatch_tagged(None, job);
    }

    /// Queues a command; `pairing` names the device a `pair` pairs.
    fn dispatch_tagged(
        mut self: Pin<&mut Self>,
        pairing: Option<String>,
        job: impl FnOnce(&mut Backend) -> Result<(), BluetoothError> + Send + 'static,
    ) {
        let queued = self
            .rust()
            .worker
            .as_ref()
            .is_some_and(|w| w.run(move |b| job(b).map_err(|e| e.message_es())));
        if queued {
            self.as_mut().rust_mut().jobs.push_back(pairing);
            let pending = self.rust().pending + 1;
            self.as_mut().rust_mut().pending = pending;
            self.as_mut().set_busy(true);
        } else {
            self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(BluetoothError::Unavailable.message_es().as_str()),
            );
        }
    }

    pub fn set_powered(self: Pin<&mut Self>, on: bool) {
        if on && AIRPLANE_ON.load(Ordering::SeqCst) {
            self.notice(
                QString::from(NoticeKind::Info.token()),
                QString::from(airplane_mode_message().as_str()),
            );
            return;
        }
        self.dispatch(power_job(on));
    }

    pub fn set_discovering(self: Pin<&mut Self>, on: bool) {
        self.dispatch(move |b| b.set_discovering(on));
    }

    pub fn pair(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        let tag = Some(address.clone());
        self.dispatch_tagged(tag, move |b| b.pair(&address));
    }

    pub fn connect(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        self.dispatch(move |b| b.connect(&address));
    }

    pub fn disconnect(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        self.dispatch(move |b| b.disconnect(&address));
    }

    pub fn forget(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        self.dispatch(move |b| b.forget(&address));
    }

    /// Hands an agent request to the page; a second one while the first
    /// waits is dropped (BlueZ asks one thing per pairing), and `Cancel`
    /// closes the dialog.
    pub fn raise_agent_request(mut self: Pin<&mut Self>, request: AgentRequest) {
        let shown = request
            .presentation()
            .map(|(kind, device, passkey)| (QString::from(kind), QString::from(device), passkey));
        if self.as_mut().rust_mut().agent.request(request).is_err() {
            return;
        }
        match shown {
            Some((kind, device, passkey)) => self.agent_request(kind, device, passkey),
            None => self.agent_closed(),
        }
    }

    pub fn answer_agent(mut self: Pin<&mut Self>, kind: &QString, value: &QString) {
        let answer = AgentAnswer::from_tokens(&kind.to_string(), &value.to_string());
        // Sent on to BlueZ and dropped: the code is never kept.
        let Some((request, answer)) = self.as_mut().rust_mut().agent.answer(answer) else {
            return;
        };
        match (request, answer) {
            // A shown passkey waits on nothing in the agent: cancelling it
            // abandons the pairing itself.
            (AgentRequest::DisplayPasskey { device, .. }, AgentAnswer::Cancelled) => {
                match self.shown_address(&device) {
                    Some(address) => backend::cancel_pairing(address),
                    None => eprintln!("Cuprita: no device «{device}» to stop pairing with"),
                }
            }
            (_, answer) => {
                if let Some(answers) = &self.rust().answers {
                    let _ = answers.send(answer);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use cuprita_core::fake::FakeBluetooth;

    use super::*;

    /// Airplane mode reaches the adapter through the Bluetooth worker, over
    /// the fake: the snapshot after it reads powered off, and switching the
    /// adapter on while it lasts leaves it off.
    #[test]
    fn airplane_mode_powers_the_adapter_off_and_keeps_it_off() {
        let (tx, rx) = mpsc::channel();
        let worker = Worker::start(
            |_refresh| (Box::new(FakeBluetooth::scripted()) as Box<Backend>, None),
            None,
            |backend: &mut Backend| backend.snapshot().map_err(|e| e.message_es()),
            move |report| {
                if let Report::Snapshot(snapshot) = report {
                    let _ = tx.send(snapshot.powered);
                }
            },
        );
        *AIRPLANE.lock().unwrap() = worker;
        // The first read: on.
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(true));
        assert!(set_airplane(true));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(false));
        let switch = AIRPLANE.lock().unwrap().clone().unwrap();
        assert!(switch.run(|b: &mut Backend| power_job(true)(b).map_err(|e| e.message_es())));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(false));
        // Airplane mode off: the adapter stays off until switched on.
        assert!(set_airplane(false));
        assert!(switch.run(|b: &mut Backend| power_job(true)(b).map_err(|e| e.message_es())));
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(true));
        *AIRPLANE.lock().unwrap() = None;
    }
}
