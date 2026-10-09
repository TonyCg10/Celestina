//! Grafita's side of the suite's activation interface.
//!
//! The claim, the hand-off and the served `org.celestina.Application1` object
//! belong to `celestina_core::activation`; this adapter only turns its
//! requests into Qt work. `Open` opens each path in a tab of the running
//! window; `Activate` raises it. A drop from another application
//! (`openDropped`) takes the same road: each local file becomes an `Open`, and
//! a URI that names no local file is counted for the window's notice.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
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

        /// A drop left this many entries that are not local files unopened.
        #[qsignal]
        fn drop_ignored(self: Pin<&mut GrafitaActivation>, count: i32);

        /// A `text/uri-list` dropped on the window: each local file opens in
        /// a tab exactly as an `Open` would, the rest are reported.
        #[qinvokable]
        fn open_dropped(self: Pin<&mut GrafitaActivation>, uris: &QStringList);

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

    pub fn open_dropped(mut self: Pin<&mut Self>, uris: &QStringList) {
        // Decoding is string work, no filesystem: the probe and the read
        // happen in the tab's own session worker, as for any `Open`.
        let uris: Vec<String> = uris.iter().map(QString::to_string).collect();
        let actions = dropped_actions(&uris);
        for action in actions {
            match action {
                Action::Open(path) => self.as_mut().open_requested(QString::from(path.as_str())),
                Action::Ignored(count) => self.as_mut().drop_ignored(count),
                Action::Raise => self.as_mut().raise_requested(),
            }
        }
    }
}

/// The actions a drop comes to: the shared `Open` for its local files, then
/// one notice for whatever was not a local file.
fn dropped_actions(uris: &[String]) -> Vec<Action> {
    let drop = crate::url::dropped(uris);
    let mut actions = open_actions(drop.local);
    if drop.ignored > 0 {
        eprintln!(
            "grafita: {} dropped item(s) are not local files, ignored",
            drop.ignored
        );
        actions.push(Action::Ignored(
            i32::try_from(drop.ignored).unwrap_or(i32::MAX),
        ));
    }
    actions
}

/// One tab per path, in the form `openPath` reads back unchanged.
fn open_actions(paths: Vec<PathBuf>) -> Vec<Action> {
    paths
        .into_iter()
        .filter_map(|path| {
            // A name that is not UTF-8 travels as its `file://` URI, which
            // `openPath` reads back byte for byte.
            let argument = crate::url::qml_argument(&path);
            if argument.is_none() {
                eprintln!("grafita: cannot open {} in a tab", path.display());
            }
            argument.map(Action::Open)
        })
        .collect()
}

/// What the window is asked to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Raise,
    Open(String),
    /// A drop held this many entries that are not local files.
    Ignored(i32),
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
                    Action::Ignored(count) => activation.drop_ignored(count),
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
        for action in open_actions(paths) {
            self.0.deliver(action);
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
    use super::{dropped_actions, Action, Forward, Sink};
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

    #[test]
    fn a_drop_opens_each_local_file_and_reports_the_rest() {
        let uris = [
            "file:///tmp/a%20b".to_owned(),
            "smb://x/y".to_owned(),
            "file:///tmp/%FF.txt".to_owned(),
        ];
        assert_eq!(
            dropped_actions(&uris),
            vec![
                Action::Open("/tmp/a b".to_owned()),
                // Not UTF-8: it crosses as its canonical URI, byte-exact.
                Action::Open("file:///tmp/%FF.txt".to_owned()),
                Action::Ignored(1),
            ]
        );
        assert_eq!(
            dropped_actions(&["https://x/y".to_owned()]),
            vec![Action::Ignored(1)]
        );
    }
}
