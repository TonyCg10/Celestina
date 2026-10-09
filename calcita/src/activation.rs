//! Calcita's side of the suite's activation interface.
//!
//! `celestina_core::activation` owns the claim, the hand-off to a running
//! instance and the served `org.celestina.Application1` object; this adapter
//! only turns its requests into Qt signals: `Activate` raises the front
//! window, `Open` hands its paths, as pathkeys, to the window, which asks the
//! controller to open each one (one window per document). The paths this
//! launch was given on the command line arrive the same way once `start`
//! runs.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner};
use celestina_core::pathkey;
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
        type CalcitaActivation = super::CalcitaActivationRust;

        /// Another launch asked this window to come forward.
        #[qsignal]
        fn raise_requested(self: Pin<&mut CalcitaActivation>);

        /// Another launch, or this one's command line, asked to open these
        /// documents (pathkeys, in order).
        #[qsignal]
        fn open_requested(self: Pin<&mut CalcitaActivation>, keys: QStringList);

        /// Connects this object to the claim `main` made, once; requests that
        /// waited in the inbox are replayed then.
        #[qinvokable]
        fn start(self: Pin<&mut CalcitaActivation>);
    }

    impl cxx_qt::Threading for CalcitaActivation {}
}

#[derive(Default)]
pub struct CalcitaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::CalcitaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();
/// The documents this launch was started with, opened when `start` runs.
static ARGV: OnceLock<Vec<PathBuf>> = OnceLock::new();

impl qobject::CalcitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let _ = QT.set(self.qt_thread());
        if let Some(paths) = ARGV.get().filter(|paths| !paths.is_empty()) {
            self.as_mut().open_requested(keys_of(paths));
        }
        if let Some(owner) = OWNER.get() {
            owner.attach();
        }
    }
}

/// Where a request ends: the Qt thread in the binary, a tally under test.
trait Target: Send + 'static {
    fn raise(&self);
    fn open(&self, paths: Vec<PathBuf>);
}

fn keys_of(paths: &[PathBuf]) -> QStringList {
    paths
        .iter()
        .map(|path| QString::from(pathkey::encode(path).as_str()))
        .collect()
}

struct QtTarget;

impl Target for QtTarget {
    fn raise(&self) {
        // The queue fails only while the window is going away; nothing is
        // left to bring forward then.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(|activation: Pin<&mut qobject::CalcitaActivation>| {
                activation.raise_requested();
            });
        }
    }

    fn open(&self, paths: Vec<PathBuf>) {
        if let Some(qt) = QT.get() {
            let _ = qt.queue(move |activation: Pin<&mut qobject::CalcitaActivation>| {
                activation.open_requested(keys_of(&paths));
            });
        }
    }
}

struct Adapter<T>(T);

impl<T: Target> Activatable for Adapter<T> {
    fn activate(&self) {
        self.0.raise();
    }

    fn open(&self, paths: Vec<PathBuf>) {
        self.0.open(paths);
    }
}

/// Claims `org.celestina.Calcita` before any window exists. When a running
/// Calcita takes this launch the process ends here (status 0); otherwise the
/// owner is kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    let _ = ARGV.set(argv_paths.to_vec());
    if let Some(owner) =
        activation::claim_or_exit(activation::CALCITA, Box::new(Adapter(QtTarget)), argv_paths)
    {
        let _ = OWNER.set(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::{Adapter, Target};
    use celestina_core::activation::Activatable;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    /// The fake queue: every request the adapter would send to Qt, in order.
    #[derive(Clone, Default)]
    struct Queue(Arc<Mutex<Vec<String>>>);

    impl Queue {
        fn taken(&self) -> Vec<String> {
            self.0.lock().map(|queue| queue.clone()).unwrap_or_default()
        }
    }

    impl Target for Queue {
        fn raise(&self) {
            if let Ok(mut queue) = self.0.lock() {
                queue.push("raise".to_owned());
            }
        }

        fn open(&self, paths: Vec<PathBuf>) {
            if let Ok(mut queue) = self.0.lock() {
                for path in paths {
                    queue.push(format!("open {}", path.display()));
                }
            }
        }
    }

    #[test]
    fn activate_raises() {
        let queue = Queue::default();
        Adapter(queue.clone()).activate();
        assert_eq!(queue.taken(), vec!["raise"]);
    }

    #[test]
    fn open_hands_every_path_over_in_order() {
        let queue = Queue::default();
        let adapter = Adapter(queue.clone());
        adapter.open(vec![PathBuf::from("/tmp/informe.pdf")]);
        adapter.open(vec![
            PathBuf::from("/tmp/a.pdf"),
            PathBuf::from("/tmp/b.pdf"),
        ]);
        assert_eq!(
            queue.taken(),
            vec![
                "open /tmp/informe.pdf",
                "open /tmp/a.pdf",
                "open /tmp/b.pdf"
            ]
        );
    }

    #[test]
    fn the_keys_are_byte_exact() {
        use std::os::unix::ffi::OsStrExt;
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/a\xff.pdf"));
        let keys = super::keys_of(&[path]);
        assert_eq!(
            keys.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["/tmp/a%FF.pdf"]
        );
    }
}
