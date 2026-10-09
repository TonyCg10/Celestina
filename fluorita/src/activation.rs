//! What Fluorita was asked to open, before anything is opened, and how a
//! second launch reaches the first.
//!
//! The single instance is the suite's shared claim-first hand-off,
//! `celestina_core::activation`, on `org.celestina.Fluorita` and on a bus
//! connection of its own, beside MPRIS's. This file's adapter only turns its
//! requests into Qt work: `Open` plays the first playable file (the same path
//! a command-line argument takes) and ignores the rest, as Fluorita has no
//! play queue; a folder that is a configured library source becomes the
//! selected source; anything else, and `Activate`, raises the window.
//!
//! A drop from another application (`openDropped`) is decided on the same
//! worker by the same rule, with two differences a drop calls for: a folder
//! that is not a source is offered to the source chooser, opened at it, and a
//! URI that names no local file is counted for the window's notice.
//!
//! The desktop hands over either a path or a `file://` URI, and either may be
//! bytes that are not valid UTF-8. The raw `PathBuf` is what would eventually
//! reach the engine; the lossy string beside it exists only to put something on
//! screen and must never be turned back into a path.
//!
//! Classification here is deliberately free: `fluorita-core` decides an item's
//! kind from its name alone, so the scaffold can say *what* it was handed
//! without starting a decoder. That is the same contract the library relies on
//! while browsing.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner, Request};
use celestina_core::file_uri::{self, FileUriError};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use fluorita_core::MediaKind;

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
        type FluoritaActivation = super::FluoritaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut FluoritaActivation>);

        /// Another launch handed a playable file: its path key, the label a
        /// person reads, and its kind as the window words it.
        #[qsignal]
        fn open_requested(
            self: Pin<&mut FluoritaActivation>,
            key: QString,
            name: QString,
            kind: QString,
        );

        /// Another launch handed a folder that is a configured source.
        #[qsignal]
        fn source_requested(self: Pin<&mut FluoritaActivation>, source: i32);

        /// A dropped folder that is not a source: offer it to the source
        /// chooser, opened at its path key.
        #[qsignal]
        fn folder_offered(self: Pin<&mut FluoritaActivation>, key: QString);

        /// A drop left this many entries that are not local files unopened.
        #[qsignal]
        fn drop_ignored(self: Pin<&mut FluoritaActivation>, count: i32);

        /// A `text/uri-list` dropped on the window, decided on the worker.
        #[qinvokable]
        fn open_dropped(self: Pin<&mut FluoritaActivation>, uris: &QStringList);

        /// Attaches this object to the claim `main` made, once.
        #[qinvokable]
        fn start(self: Pin<&mut FluoritaActivation>);
    }

    impl cxx_qt::Threading for FluoritaActivation {}
}

#[derive(Default)]
pub struct FluoritaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::FluoritaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();
/// The worker's queue, for drops; set when the worker starts.
static DROPS: OnceLock<std::sync::mpsc::Sender<Job>> = OnceLock::new();

impl qobject::FluoritaActivation {
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

    pub fn open_dropped(self: Pin<&mut Self>, uris: &QStringList) {
        let uris: Vec<String> = uris.iter().map(QString::to_string).collect();
        if let Some(jobs) = DROPS.get() {
            let _ = jobs.send(Job::Dropped(uris));
        }
    }
}

/// What the window is asked to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Raise,
    Play {
        key: String,
        name: String,
        kind: &'static str,
    },
    Source(i32),
    /// A dropped folder to offer to the source chooser, as its path key.
    Offer(String),
    /// A drop held this many entries that are not local files.
    Ignored(i32),
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
                move |activation: Pin<&mut qobject::FluoritaActivation>| match action {
                    Action::Raise => activation.raise_requested(),
                    Action::Play { key, name, kind } => activation.open_requested(
                        QString::from(key.as_str()),
                        QString::from(name.as_str()),
                        QString::from(kind),
                    ),
                    Action::Source(source) => activation.source_requested(source),
                    Action::Offer(key) => activation.folder_offered(QString::from(key.as_str())),
                    Action::Ignored(count) => activation.drop_ignored(count),
                },
            );
        }
    }
}

/// What one handed path is, as far as `Open` cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handed {
    Folder,
    File,
}

/// The facts `Open` needs, asked off the Qt thread: whether a path is a
/// folder, and which configured source a folder is.
trait World: Clone + Send + 'static {
    fn kind(&self, path: &Path) -> Handed;
    fn source_of(&self, folder: &Path) -> Option<i32>;
}

#[derive(Clone)]
struct Disk;

impl World for Disk {
    fn kind(&self, path: &Path) -> Handed {
        if std::fs::metadata(path).is_ok_and(|meta| meta.is_dir()) {
            Handed::Folder
        } else {
            Handed::File
        }
    }

    fn source_of(&self, folder: &Path) -> Option<i32> {
        crate::library::configured_source(folder)
    }
}

/// The one action an `Open` comes to: the first playable file, else the
/// first folder that is a source, else a raise. What is left over is said.
/// With `offer`, a folder that is not a source is a candidate too: it goes
/// to the source chooser.
fn decide(paths: &[PathBuf], world: &impl World, offer: bool) -> Action {
    let mut chosen = None;
    for path in paths {
        let candidate = match world.kind(path) {
            Handed::File if MediaKind::classify_path(path).is_some() => {
                let media = RequestedMedia {
                    path: Some(path.clone()),
                    label: display_label(path),
                    kind: MediaKind::classify_path(path),
                };
                Some(Action::Play {
                    key: celestina_core::pathkey::encode(path),
                    name: media.label.clone(),
                    kind: media.kind_label(),
                })
            }
            Handed::File => {
                eprintln!("fluorita: {} is not media, not opened", path.display());
                None
            }
            Handed::Folder => {
                let source = world.source_of(path).map(Action::Source);
                if source.is_none() && offer {
                    Some(Action::Offer(celestina_core::pathkey::encode(path)))
                } else if source.is_none() {
                    eprintln!(
                        "fluorita: {} is not a library folder, not opened",
                        path.display()
                    );
                    None
                } else {
                    source
                }
            }
        };
        match (&chosen, candidate) {
            (None, Some(action)) => chosen = Some(action),
            (Some(_), Some(_)) => {
                eprintln!(
                    "fluorita: {} ignored, there is no play queue",
                    path.display()
                );
            }
            (_, None) => {}
        }
    }
    chosen.unwrap_or(Action::Raise)
}

/// What the worker is fed: the shared requests, and the window's drops.
enum Job {
    Shared(Request),
    Dropped(Vec<String>),
}

/// What a drop comes to: one action for its local entries, as `Open` would
/// decide them (plus the folder offer), and a notice for the rest.
fn decide_dropped(uris: &[String], world: &impl World) -> Vec<Action> {
    let mut local = Vec::new();
    let mut ignored = 0_usize;
    for uri in uris {
        match file_uri::to_path(uri) {
            Ok(path) => local.push(path),
            Err(_) => ignored += 1,
        }
    }
    let mut actions = Vec::new();
    if !local.is_empty() {
        actions.push(decide(&local, world, true));
    }
    if ignored > 0 {
        eprintln!("fluorita: {ignored} dropped item(s) are not local files, ignored");
        actions.push(Action::Ignored(i32::try_from(ignored).unwrap_or(i32::MAX)));
    }
    actions
}

/// Turns the shared requests into Fluorita's actions on one long-lived
/// worker fed in arrival order: the filesystem and the stored sources are
/// asked there, so a stalled mount holds neither the Qt thread nor the bus
/// thread, and requests keep the order they arrived in.
struct Forward {
    jobs: std::sync::mpsc::Sender<Job>,
}

impl Forward {
    fn start<S: Sink, W: World>(sink: S, world: W) -> Self {
        let (jobs, queue) = std::sync::mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("fluorita-activation".to_owned())
            .spawn(move || {
                for job in queue {
                    match job {
                        Job::Shared(Request::Activate) => sink.deliver(Action::Raise),
                        Job::Shared(Request::Open(paths)) => {
                            sink.deliver(decide(&paths, &world, false));
                        }
                        Job::Dropped(uris) => {
                            for action in decide_dropped(&uris, &world) {
                                sink.deliver(action);
                            }
                        }
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("fluorita: no activation worker, other launches go unanswered: {error}");
        }
        Self { jobs }
    }
}

impl Activatable for Forward {
    fn activate(&self) {
        let _ = self.jobs.send(Job::Shared(Request::Activate));
    }

    fn open(&self, paths: Vec<PathBuf>) {
        let _ = self.jobs.send(Job::Shared(Request::Open(paths)));
    }
}

/// Claims Fluorita's name before any window exists. A running window that
/// takes this launch ends the process here (status 0); otherwise the owner is
/// kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    let served = Forward::start(QtSink, Disk);
    let _ = DROPS.set(served.jobs.clone());
    if let Some(owner) =
        activation::claim_or_exit(activation::FLUORITA, Box::new(served), argv_paths)
    {
        let _ = OWNER.set(owner);
    }
}

/// The item named on the command line, if any.
#[derive(Clone, Debug, Default)]
pub struct RequestedMedia {
    /// The real path, byte-exact. `None` when Fluorita was launched with no
    /// argument, which is the ordinary "open the library" case.
    pub path: Option<PathBuf>,
    /// A label for the window. Lossy on purpose, and never reopened.
    pub label: String,
    pub kind: Option<MediaKind>,
}

impl RequestedMedia {
    /// The Spanish label the window shows for the classified kind. An
    /// unclassified file says so plainly rather than being guessed at.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            Some(MediaKind::Image) => "imagen",
            Some(MediaKind::Video) => "vídeo",
            Some(MediaKind::Audio) => "audio",
            None if self.path.is_some() => "tipo no reconocido",
            None => "",
        }
    }
}

/// Reads the first argument that names something to open.
///
/// Options are skipped rather than treated as filenames, and only the first
/// item is taken: this window opens one item, so silently swallowing the rest
/// would be worse than showing the one it took.
#[must_use]
pub fn requested_media() -> RequestedMedia {
    std::env::args_os()
        .skip(1)
        .find(|argument| !is_option(argument))
        .map_or_else(RequestedMedia::default, |argument| describe(&argument))
}

fn is_option(argument: &OsString) -> bool {
    argument
        .to_str()
        .is_some_and(|text| text.starts_with('-') && text != "-")
}

fn describe(argument: &OsString) -> RequestedMedia {
    // Resolved once, here: everything downstream — the engine's source handle,
    // the `file://` URL a still is shown through — refuses a relative path
    // rather than guess a working directory.
    let path = local_path(argument).map(|path| std::fs::canonicalize(&path).unwrap_or(path));
    let label = path
        .as_deref()
        .map(display_label)
        .unwrap_or_else(|| argument.to_string_lossy().into_owned());
    let kind = path.as_deref().and_then(MediaKind::classify_path);
    RequestedMedia { path, label, kind }
}

/// Turns one argument into a local path: a `file://` URI is read by the
/// suite's one strict parser, anything else is taken as a path as-is.
///
/// A URI with a host other than the local machine is refused, and so is one
/// that is malformed — an escape that is not one, a query, a NUL: Fluorita's
/// library is local, and quietly reading either as a local path would open
/// the wrong file.
#[must_use]
pub fn local_path(argument: &OsString) -> Option<PathBuf> {
    // Not text, so not a URI: the argument is the path, bytes and all.
    let Some(text) = argument.to_str() else {
        return Some(PathBuf::from(argument));
    };
    match file_uri::to_path(text) {
        Ok(path) => Some(path),
        // Not a URI: the argument is the path.
        Err(FileUriError::NotFileScheme) => Some(PathBuf::from(argument)),
        Err(_) => None,
    }
}

/// A human-readable name for the window title. Lossy by design, and bounded
/// and stripped like every name a file claims.
fn display_label(path: &Path) -> String {
    fluorita_core::displayed_name(path)
}

#[cfg(test)]
mod tests {
    use super::{
        decide, decide_dropped, describe, local_path, Action, Forward, Handed, RequestedMedia,
        Sink, World,
    };
    use celestina_core::activation::Activatable;
    use fluorita_core::MediaKind;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Duration;

    /// Folders end in `/d`; `/d/fotos` is source 3.
    #[derive(Clone)]
    struct Fake;

    impl World for Fake {
        fn kind(&self, path: &Path) -> Handed {
            if path.extension().is_none() {
                Handed::Folder
            } else {
                Handed::File
            }
        }

        fn source_of(&self, folder: &Path) -> Option<i32> {
            (folder == Path::new("/d/fotos")).then_some(3)
        }
    }

    #[derive(Clone)]
    struct Recorder(mpsc::Sender<Action>);

    impl Sink for Recorder {
        fn deliver(&self, action: Action) {
            let _ = self.0.send(action);
        }
    }

    #[test]
    fn an_open_plays_the_first_media_file_and_ignores_the_rest() {
        let (sender, seen) = mpsc::channel();
        let forward = Forward::start(Recorder(sender), Fake);
        forward.open(vec![
            PathBuf::from("/m/notas.txt"),
            PathBuf::from("/m/a b.mkv"),
            PathBuf::from("/m/c.mp3"),
        ]);
        forward.activate();
        let next = || {
            seen.recv_timeout(Duration::from_secs(5))
                .expect("an action")
        };
        assert_eq!(
            [next(), next()],
            [
                Action::Play {
                    key: "/m/a%20b.mkv".to_owned(),
                    name: "a b.mkv".to_owned(),
                    kind: describe(&OsString::from("/m/a b.mkv")).kind_label(),
                },
                Action::Raise,
            ]
        );
    }

    #[test]
    fn a_folder_selects_its_source_or_only_raises() {
        assert_eq!(
            decide(&[PathBuf::from("/d/fotos")], &Fake, false),
            Action::Source(3)
        );
        assert_eq!(
            decide(&[PathBuf::from("/d/otra")], &Fake, false),
            Action::Raise
        );
    }

    #[test]
    fn a_plain_path_is_taken_as_it_is() {
        assert_eq!(
            local_path(&OsString::from("/home/toni/Vídeos/clip.mp4")),
            Some(PathBuf::from("/home/toni/Vídeos/clip.mp4"))
        );
    }

    #[test]
    fn a_file_uri_is_decoded_with_the_suite_codec() {
        assert_eq!(
            local_path(&OsString::from("file:///home/toni/a%20b.mp4")),
            Some(PathBuf::from("/home/toni/a b.mp4"))
        );
        assert_eq!(
            local_path(&OsString::from("file://localhost/home/toni/x.mp3")),
            Some(PathBuf::from("/home/toni/x.mp3"))
        );
    }

    #[test]
    fn a_remote_authority_is_refused_rather_than_read_as_local() {
        assert_eq!(
            local_path(&OsString::from("file://otherhost/etc/x.mp4")),
            None
        );
    }

    #[test]
    fn a_malformed_uri_is_refused_rather_than_guessed_at() {
        // The folder chooser already refused these; the command line now
        // reads a URI the same way.
        assert_eq!(local_path(&OsString::from("file:///m/half%2")), None);
        assert_eq!(local_path(&OsString::from("file:///m/x.mp4?t=3")), None);
        assert_eq!(local_path(&OsString::from("file:///m/a%00b.mp4")), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_argument_keeps_its_bytes() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let argument = OsString::from_vec(b"/home/toni/mal-\xFF.mp4".to_vec());
        let path = local_path(&argument).expect("a path");

        assert_eq!(path.as_os_str().as_bytes(), b"/home/toni/mal-\xFF.mp4");
        // The label may be lossy; the path must not be.
        assert!(describe(&argument).label.contains('\u{FFFD}'));
    }

    #[test]
    fn the_kind_label_never_guesses() {
        let video = describe(&OsString::from("/home/toni/clip.mkv"));
        assert_eq!(video.kind, Some(MediaKind::Video));
        assert_eq!(video.kind_label(), "vídeo");
        assert_eq!(video.label, "clip.mkv");

        let unknown = describe(&OsString::from("/home/toni/notas.txt"));
        assert_eq!(unknown.kind, None);
        assert_eq!(unknown.kind_label(), "tipo no reconocido");

        assert_eq!(RequestedMedia::default().kind_label(), "");
    }

    #[test]
    fn a_drop_decides_like_open_and_offers_other_folders() {
        let uris = |list: &[&str]| list.iter().map(|uri| (*uri).to_owned()).collect::<Vec<_>>();
        // The first media file plays; a remote URI is only reported.
        assert_eq!(
            decide_dropped(
                &uris(&["smb://x/y", "file:///m/a%20b.mkv", "file:///m/c.png"]),
                &Fake
            ),
            vec![
                Action::Play {
                    key: "/m/a%20b.mkv".to_owned(),
                    name: "a b.mkv".to_owned(),
                    kind: describe(&OsString::from("/m/a b.mkv")).kind_label(),
                },
                Action::Ignored(1),
            ]
        );
        // A dropped image opens in the viewer through the same action.
        assert!(matches!(
            decide_dropped(&uris(&["file:///m/c.png"]), &Fake).as_slice(),
            [Action::Play { kind: "imagen", .. }]
        ));
        assert_eq!(
            decide_dropped(&uris(&["file:///d/fotos"]), &Fake),
            vec![Action::Source(3)]
        );
        assert_eq!(
            decide_dropped(&uris(&["file:///d/otra"]), &Fake),
            vec![Action::Offer("/d/otra".to_owned())]
        );
        assert_eq!(
            decide_dropped(&uris(&["https://x/y"]), &Fake),
            vec![Action::Ignored(1)]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dropped_name_that_is_not_utf8_keeps_its_bytes() {
        let actions = decide_dropped(&["file:///d/%FF".to_owned()], &Fake);
        let [Action::Offer(key)] = actions.as_slice() else {
            panic!("an offer, got {actions:?}");
        };
        let path = celestina_core::pathkey::decode(key).expect("a path key");
        assert_eq!(
            std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()),
            b"/d/\xFF"
        );
    }
}
