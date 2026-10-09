//! Hematita's side of the suite's activation interface.
//!
//! The claim-first hand-off that used to live here (HEM-H1-F) is now
//! `celestina_core::activation`, which claims `org.celestina.Hematita`, serves
//! `org.celestina.Application1` and keeps early requests in its inbox. This
//! adapter only turns those requests into Qt work: `Activate` raises the
//! window, `Open` browses the first folder in the storage section.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

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
        type HematitaActivation = super::HematitaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut HematitaActivation>);

        /// Another launch handed this window a folder to browse, as its path
        /// key.
        #[qsignal]
        fn open_requested(self: Pin<&mut HematitaActivation>, path: QString);

        /// Attaches this object to the claim `main` made, once: every request
        /// that arrived since, and every later one, reaches the Qt thread as
        /// a signal. Without a claim there is nothing to attach to.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaActivation>);
    }

    impl cxx_qt::Threading for HematitaActivation {}
}

#[derive(Default)]
pub struct HematitaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::HematitaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::HematitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let _ = QT.set(self.qt_thread());
        if let Some(owner) = OWNER.get() {
            owner.attach();
        }
    }
}

/// What the window is asked to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Raise,
    Open(String),
}

/// Where actions go: the Qt thread in the application, a recorder in tests.
trait Sink: Send + 'static {
    fn deliver(&self, action: Action);
}

struct QtSink;

impl Sink for QtSink {
    fn deliver(&self, action: Action) {
        // A failed queue means the window is closing: nobody is left to raise.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(
                move |activation: Pin<&mut qobject::HematitaActivation>| match action {
                    Action::Raise => activation.raise_requested(),
                    Action::Open(path) => activation.open_requested(QString::from(path.as_str())),
                },
            );
        }
    }
}

/// Turns the shared requests into Hematita's actions.
struct Forward<S>(S);

impl<S: Sink> Activatable for Forward<S> {
    fn activate(&self) {
        self.0.deliver(Action::Raise);
    }

    fn open(&self, paths: Vec<PathBuf>) {
        // One storage view: the first folder is browsed, the rest are not.
        // The folder crosses as its path key, byte-exact (ADR 0008).
        if paths.len() > 1 {
            eprintln!("hematita: browsing the first of {} folders", paths.len());
        }
        match paths.into_iter().next() {
            Some(path) => self
                .0
                .deliver(Action::Open(celestina_core::pathkey::encode(&path))),
            None => self.0.deliver(Action::Raise),
        }
    }
}

/// Claims Hematita's name before any window exists. A running window that
/// takes this launch ends the process here (status 0); otherwise the owner is
/// kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    if let Some(owner) =
        activation::claim_or_exit(activation::HEMATITA, Box::new(Forward(QtSink)), argv_paths)
    {
        let _ = OWNER.set(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Forward, Sink};
    use celestina_core::activation::Activatable;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<Action>>>);

    impl Sink for Recorder {
        fn deliver(&self, action: Action) {
            self.0.lock().expect("recorder").push(action);
        }
    }

    #[test]
    fn an_open_reaches_the_storage_view_with_the_first_folder() {
        let recorder = Recorder::default();
        let forward = Forward(recorder.clone());
        forward.open(vec![PathBuf::from("/srv/mis datos"), PathBuf::from("/tmp")]);
        forward.activate();
        assert_eq!(
            *recorder.0.lock().expect("recorder"),
            vec![Action::Open("/srv/mis%20datos".to_owned()), Action::Raise]
        );
    }
}
