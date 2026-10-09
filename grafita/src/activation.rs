//! Grafita's side of the suite's activation interface.
//!
//! The claim, the hand-off and the served `org.celestina.Application1` object
//! belong to `celestina_core::activation`; this adapter only turns its
//! requests into Qt work. `Open` opens each path in a tab of the running
//! window; `Activate` raises it.

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
        type GrafitaActivation = super::GrafitaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut GrafitaActivation>);

        /// Another launch handed this window a document to open in a tab, in
        /// the form `openPath` reads back unchanged.
        #[qsignal]
        fn open_requested(self: Pin<&mut GrafitaActivation>, path: QString);

        /// Attaches this object to the claim `main` made, once: what arrived
        /// since, and every later request, reaches the window as a signal.
        #[qinvokable]
        fn start(self: Pin<&mut GrafitaActivation>);
    }

    impl cxx_qt::Threading for GrafitaActivation {}
}

#[derive(Default)]
pub struct GrafitaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::GrafitaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::GrafitaActivation {
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
        // `attach` runs only after `start` stored the thread; a failed queue
        // means the window is closing and nobody is left to answer.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(
                move |activation: Pin<&mut qobject::GrafitaActivation>| match action {
                    Action::Raise => activation.raise_requested(),
                    Action::Open(path) => activation.open_requested(QString::from(path.as_str())),
                },
            );
        }
    }
}

/// Turns the shared requests into Grafita's actions.
struct Forward<S>(S);

impl<S: Sink> Activatable for Forward<S> {
    fn activate(&self) {
        self.0.deliver(Action::Raise);
    }

    fn open(&self, paths: Vec<PathBuf>) {
        for path in paths {
            // A name that is not UTF-8 travels as its `file://` URI, which
            // `openPath` reads back byte for byte.
            match crate::url::qml_argument(&path) {
                Some(argument) => self.0.deliver(Action::Open(argument)),
                None => eprintln!("grafita: cannot open {} in a tab", path.display()),
            }
        }
    }
}

/// Claims Grafita's name before any window exists. A running window that
/// takes this launch ends the process here (status 0); otherwise the owner is
/// kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    if let Some(owner) =
        activation::claim_or_exit(activation::GRAFITA, Box::new(Forward(QtSink)), argv_paths)
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
    fn an_open_reaches_the_window_as_one_tab_per_document() {
        let recorder = Recorder::default();
        let forward = Forward(recorder.clone());
        forward.open(vec![
            PathBuf::from("/tmp/a.txt"),
            PathBuf::from("/tmp/b.rs"),
        ]);
        forward.activate();
        assert_eq!(
            *recorder.0.lock().expect("recorder"),
            vec![
                Action::Open("/tmp/a.txt".to_owned()),
                Action::Open("/tmp/b.rs".to_owned()),
                Action::Raise,
            ]
        );
    }
}
