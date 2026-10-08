//! The Network section's commands and switches. The network list itself is
//! `NetworkModel`, fed from this controller's snapshots through `NETWORKS`.

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use cuprita_core::error::NetworkError;
use cuprita_core::model::NetworkSnapshot;
use cuprita_core::network::Network;

use super::notice::NoticeKind;
use super::{Hub, Report, Worker};
use crate::backend;

/// The trait object the worker drives.
type Backend = dyn Network + 'static;

/// The network snapshots `NetworkModel` subscribes to.
pub static NETWORKS: Hub<NetworkSnapshot> = Hub::new();

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
        #[qproperty(bool, wifi_enabled, READ, NOTIFY)]
        #[qproperty(bool, airplane, READ, NOTIFY)]
        #[qproperty(bool, busy, READ, NOTIFY)]
        type NetworkController = super::NetworkControllerRust;

        #[qsignal]
        fn notice(self: Pin<&mut NetworkController>, kind: QString, text: QString);

        #[qinvokable]
        fn set_wifi_enabled(self: Pin<&mut NetworkController>, on: bool);
        #[qinvokable]
        fn set_airplane(self: Pin<&mut NetworkController>, on: bool);
        /// An empty password means none: open and saved networks need none.
        #[qinvokable]
        fn connect(self: Pin<&mut NetworkController>, id: &QString, password: &QString);
        #[qinvokable]
        fn disconnect(self: Pin<&mut NetworkController>, id: &QString);
        #[qinvokable]
        fn forget(self: Pin<&mut NetworkController>, id: &QString);
        #[qinvokable]
        fn set_vpn_active(self: Pin<&mut NetworkController>, id: &QString, on: bool);
    }

    impl cxx_qt::Threading for NetworkController {}
    impl cxx_qt::Initialize for NetworkController {}
}

#[derive(Default)]
pub struct NetworkControllerRust {
    wifi_enabled: bool,
    airplane: bool,
    busy: bool,
    pending: u32,
    worker: Option<Worker<Backend>>,
}

impl cxx_qt::Initialize for qobject::NetworkController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let notices = self.qt_thread();
        // No poll: the backend's watcher asks for every re-read.
        let worker = Worker::start(
            move |refresh| {
                backend::network(
                    move || {
                        refresh.request();
                    },
                    move |text| {
                        let _ = notices.queue(
                            move |controller: Pin<&mut qobject::NetworkController>| {
                                controller.notice(
                                    QString::from(NoticeKind::Error.token()),
                                    QString::from(text.as_str()),
                                );
                            },
                        );
                    },
                )
            },
            None,
            |backend: &mut Backend| backend.snapshot().map_err(|e| e.message_es()),
            move |report| {
                let _ = qt.queue(move |controller: Pin<&mut qobject::NetworkController>| {
                    controller.apply(report);
                });
            },
        );
        self.as_mut().rust_mut().worker = worker;
    }
}

impl qobject::NetworkController {
    fn apply(mut self: Pin<&mut Self>, report: Report<NetworkSnapshot>) {
        match report {
            Report::Snapshot(snapshot) => {
                self.as_mut().set_wifi_enabled_value(snapshot.wifi_enabled);
                self.as_mut().set_airplane_value(snapshot.airplane);
                NETWORKS.publish(Arc::clone(&snapshot));
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

    fn set_wifi_enabled_value(mut self: Pin<&mut Self>, on: bool) {
        if self.rust().wifi_enabled != on {
            self.as_mut().rust_mut().wifi_enabled = on;
            self.wifi_enabled_changed();
        }
    }

    fn set_airplane_value(mut self: Pin<&mut Self>, on: bool) {
        if self.rust().airplane != on {
            self.as_mut().rust_mut().airplane = on;
            self.airplane_changed();
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
        job: impl FnOnce(&mut Backend) -> Result<(), NetworkError> + Send + 'static,
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
                QString::from(NetworkError::Unavailable.message_es().as_str()),
            );
        }
    }

    pub fn set_wifi_enabled(self: Pin<&mut Self>, on: bool) {
        self.dispatch(move |b| b.set_wifi_enabled(on));
    }

    pub fn set_airplane(self: Pin<&mut Self>, on: bool) {
        self.dispatch(move |b| b.set_airplane(on));
    }

    pub fn connect(self: Pin<&mut Self>, id: &QString, password: &QString) {
        let id = id.to_string();
        // Moved into the job and dropped with it: never stored.
        let password = Some(password.to_string()).filter(|p| !p.is_empty());
        self.dispatch(move |b| b.connect(&id, password.as_deref()));
    }

    pub fn disconnect(self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        self.dispatch(move |b| b.disconnect(&id));
    }

    pub fn forget(self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        self.dispatch(move |b| b.forget(&id));
    }

    pub fn set_vpn_active(self: Pin<&mut Self>, id: &QString, on: bool) {
        let id = id.to_string();
        self.dispatch(move |b| b.set_vpn_active(&id, on));
    }
}
