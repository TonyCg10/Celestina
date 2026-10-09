//! Siderita's side of the suite's activation interface.
//!
//! `celestina_core::activation` claims `org.celestina.Siderita`, serves
//! `org.celestina.Application1` and keeps early requests in its inbox; this
//! adapter only turns them into Qt work. `Open` gives each folder a tab of its
//! own and shows each file in a tab on its parent folder with the file
//! selected; `Activate` raises the window. `org.freedesktop.FileManager1`
//! (`dbus.rs`) and the portal (`portal.rs`) are separate names and stay as
//! they are.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner, Request};
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
        type SideritaActivation = super::SideritaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut SideritaActivation>);

        /// Another launch handed a folder (a path key) to open in a tab.
        #[qsignal]
        fn open_folder_requested(self: Pin<&mut SideritaActivation>, folder: QString);

        /// Another launch handed a file: open a tab on `folder` and select
        /// `item` there (both path keys).
        #[qsignal]
        fn reveal_requested(self: Pin<&mut SideritaActivation>, folder: QString, item: QString);

        /// Attaches this object to the claim `main` made, once.
        #[qinvokable]
        fn start(self: Pin<&mut SideritaActivation>);
    }

    impl cxx_qt::Threading for SideritaActivation {}
}

#[derive(Default)]
pub struct SideritaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::SideritaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::SideritaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let _ = QT.set(self.qt_thread());
        if let Some(owner) = OWNER.get() {
            // The replay only queues onto the adapter's worker, which asks
            // the filesystem; nothing here blocks the Qt thread.
            owner.attach();
        }
    }
}

/// What the window is asked to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Raise,
    OpenFolder(String),
    Reveal { folder: String, item: String },
}

/// Where actions go: the Qt thread in the application, a recorder in tests.
trait Sink: Clone + Send + 'static {
    fn deliver(&self, action: Action);
}

#[derive(Clone)]
struct QtSink;

impl Sink for QtSink {
    fn deliver(&self, action: Action) {
        // A failed queue means the window is closing: nobody is left to show.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(
                move |activation: Pin<&mut qobject::SideritaActivation>| match action {
                    Action::Raise => activation.raise_requested(),
                    Action::OpenFolder(folder) => {
                        activation.open_folder_requested(QString::from(folder.as_str()));
                    }
                    Action::Reveal { folder, item } => activation.reveal_requested(
                        QString::from(folder.as_str()),
                        QString::from(item.as_str()),
                    ),
                },
            );
        }
    }
}

/// How one handed path is shown: a folder (or a path that cannot be read,
/// which its tab then reports) gets its own tab; a file is selected in its
/// parent folder.
fn route(path: &Path, is_dir: bool) -> Action {
    let key = celestina_core::pathkey::encode(path);
    match path.parent() {
        Some(parent) if !is_dir => Action::Reveal {
            folder: celestina_core::pathkey::encode(parent),
            item: key,
        },
        _ => Action::OpenFolder(key),
    }
}

/// Turns the shared requests into Siderita's actions on one long-lived
/// worker fed in arrival order: whether a path is a folder is asked there, so
/// a stalled mount holds neither the Qt thread nor the bus thread, and an
/// `Activate` that follows an `Open` raises the window after its tabs exist.
struct Forward {
    jobs: std::sync::mpsc::Sender<Request>,
}

impl Forward {
    fn start<S: Sink>(sink: S) -> Self {
        let (jobs, queue) = std::sync::mpsc::channel::<Request>();
        let spawned = std::thread::Builder::new()
            .name("siderita-activation".to_owned())
            .spawn(move || {
                for job in queue {
                    match job {
                        Request::Activate => sink.deliver(Action::Raise),
                        Request::Open(paths) => {
                            for path in paths {
                                let is_dir =
                                    std::fs::metadata(&path).map_or(true, |meta| meta.is_dir());
                                sink.deliver(route(&path, is_dir));
                            }
                        }
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("siderita: no activation worker, other launches go unanswered: {error}");
        }
        Self { jobs }
    }
}

impl Activatable for Forward {
    fn activate(&self) {
        let _ = self.jobs.send(Request::Activate);
    }

    fn open(&self, paths: Vec<PathBuf>) {
        let _ = self.jobs.send(Request::Open(paths));
    }
}

/// The local paths named on the command line (`Exec=siderita %U`): flags
/// skipped, `file://` URIs decoded byte-exact, a remote URI ignored with a
/// notice.
#[must_use]
pub fn argv_paths() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .filter(|arg| !arg.as_encoded_bytes().starts_with(b"-"))
        .filter_map(|arg| match arg.to_str() {
            Some(text) if text.contains("://") => match celestina_core::file_uri::to_path(text) {
                Ok(path) => Some(path),
                Err(error) => {
                    eprintln!("siderita: ignoring {text}: {error}");
                    None
                }
            },
            _ => Some(PathBuf::from(arg)),
        })
        .collect()
}

/// Claims Siderita's name before any window exists. A running window that
/// takes this launch ends the process here (status 0); otherwise the owner is
/// kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    if let Some(owner) = activation::claim_or_exit(
        activation::SIDERITA,
        Box::new(Forward::start(QtSink)),
        argv_paths,
    ) {
        let _ = OWNER.set(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::{route, Action, Forward, Sink};
    use celestina_core::activation::Activatable;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Clone)]
    struct Recorder(mpsc::Sender<Action>);

    impl Sink for Recorder {
        fn deliver(&self, action: Action) {
            let _ = self.0.send(action);
        }
    }

    #[test]
    fn a_folder_gets_a_tab_and_a_file_is_selected_in_its_parent() {
        assert_eq!(
            route(Path::new("/srv/fotos"), true),
            Action::OpenFolder("/srv/fotos".to_owned())
        );
        assert_eq!(
            route(Path::new("/srv/a b.txt"), false),
            Action::Reveal {
                folder: "/srv".to_owned(),
                item: "/srv/a%20b.txt".to_owned()
            }
        );
    }

    #[test]
    fn requests_reach_the_window_in_arrival_order() {
        let (sender, seen) = mpsc::channel();
        let forward = Forward::start(Recorder(sender));
        let folder = std::env::temp_dir();
        forward.open(vec![folder.clone(), PathBuf::from("/no/such/place")]);
        forward.activate();
        forward.open(vec![PathBuf::from("/otra")]);
        let next = || {
            seen.recv_timeout(Duration::from_secs(5))
                .expect("an action")
        };
        assert_eq!(
            [next(), next(), next(), next()],
            [
                Action::OpenFolder(celestina_core::pathkey::encode(&folder)),
                Action::OpenFolder("/no/such/place".to_owned()),
                Action::Raise,
                Action::OpenFolder("/otra".to_owned()),
            ]
        );
    }
}
