//! The Bluetooth section's adapter switches, device commands and pairing
//! agent. The device list is `DeviceModel`, fed through `DEVICES`.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use cuprita_core::agent::{Agent, AgentAnswer, AgentRequest};
use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::error::BluetoothError;
use cuprita_core::model::BluetoothSnapshot;

use super::notice::NoticeKind;
use super::{Hub, Report, Worker};
use crate::backend;

/// The trait object the worker drives.
type Backend = dyn Bluetooth + 'static;

/// The device snapshots `DeviceModel` subscribes to.
pub static DEVICES: Hub<BluetoothSnapshot> = Hub::new();

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
        /// Answers the waiting agent request: `kind` is `pin` (with the PIN as
        /// `value`), `confirm`, `reject` or `cancel`.
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
    pending: u32,
    agent: Agent,
    worker: Option<Worker<Backend>>,
}

impl cxx_qt::Initialize for qobject::BluetoothController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let worker = Worker::spawn(
            backend::bluetooth(),
            |backend: &mut Backend| backend.snapshot().map_err(|e| e.message_es()),
            move |report| {
                let _ = qt.queue(move |controller: Pin<&mut qobject::BluetoothController>| {
                    controller.apply(report);
                });
            },
        );
        self.as_mut().rust_mut().worker = worker;
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
                DEVICES.publish(Arc::clone(&snapshot));
            }
            Report::Failed(text) => self.as_mut().notice(
                QString::from(NoticeKind::Error.token()),
                QString::from(text.as_str()),
            ),
            Report::Done => {
                let pending = self.rust().pending.saturating_sub(1);
                self.as_mut().rust_mut().pending = pending;
                self.as_mut().set_busy(pending > 0);
            }
        }
    }

    fn set_busy(mut self: Pin<&mut Self>, busy: bool) {
        if self.rust().busy != busy {
            self.as_mut().rust_mut().busy = busy;
            self.busy_changed();
        }
    }

    fn dispatch(
        mut self: Pin<&mut Self>,
        job: impl FnOnce(&mut Backend) -> Result<(), BluetoothError> + Send + 'static,
    ) {
        let queued = self
            .rust()
            .worker
            .as_ref()
            .is_some_and(|w| w.run(move |b| job(b).map_err(|e| e.message_es())));
        if queued {
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
        self.dispatch(move |b| b.set_powered(on));
    }

    pub fn set_discovering(self: Pin<&mut Self>, on: bool) {
        self.dispatch(move |b| b.set_discovering(on));
    }

    pub fn pair(self: Pin<&mut Self>, address: &QString) {
        let address = address.to_string();
        self.dispatch(move |b| b.pair(&address));
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

    /// Hands an agent request to the page. The BlueZ agent of CUP-1-D calls
    /// it; until then nothing asks.
    #[allow(dead_code)]
    pub fn raise_agent_request(mut self: Pin<&mut Self>, request: AgentRequest) {
        let (kind, device, passkey) = match &request {
            AgentRequest::Pin { device } => ("pin", device.clone(), 0),
            AgentRequest::Confirm { device, passkey } => ("confirm", device.clone(), *passkey),
            AgentRequest::DisplayPasskey { device, passkey } => {
                ("display", device.clone(), *passkey)
            }
        };
        if self.as_mut().rust_mut().agent.request(request).is_ok() {
            self.agent_request(QString::from(kind), QString::from(device.as_str()), passkey);
        }
    }

    pub fn answer_agent(mut self: Pin<&mut Self>, kind: &QString, value: &QString) {
        let answer = match kind.to_string().as_str() {
            "pin" => AgentAnswer::Pin(value.to_string()),
            "confirm" => AgentAnswer::Confirmed,
            "reject" => AgentAnswer::Rejected,
            _ => AgentAnswer::Cancelled,
        };
        // The answer goes to BlueZ with CUP-1-D's agent; the code itself is
        // never kept past this call.
        let _answered = self.as_mut().rust_mut().agent.answer(answer);
    }
}
