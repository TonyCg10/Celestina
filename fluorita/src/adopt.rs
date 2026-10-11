//! An edited copy of a capture joining Selenita's history (ADR 0012,
//! `PRV-1`).
//!
//! After «Guardar ambas» the copy is handed here, and this is the whole of
//! what happens to it:
//!
//! - a running Selenita is asked through its own interface,
//!   `Adopt(s key)`, and decides for itself whether the file is one of its
//!   captures;
//! - with no Selenita on the bus, the same rule is applied here and the row
//!   is appended through `selenita_core::history::History`, the file's one
//!   owner (load, push, private atomic replace).
//!
//! The rule is Selenita's own, `selenita_core::history::adopted`, the one
//! `Adopt` applies: a regular file directly inside the pictures folder's
//! captures folder (`selenita_core::names::CAPTURES_FOLDER`) with a `.png`
//! extension is a screenshot, one directly inside the videos folder's
//! recordings folder (`selenita_core::names::RECORDINGS_FOLDER`) with `.mp4`
//! is a recording, and anything else is left out without error. Both
//! folders are named by `selenita_core::history::FolderPaths::inside`; only
//! the pictures and videos folders are found here.
//!
//! Adoption never holds up a save. It runs on one long-lived worker fed
//! through a queue: the bus call waits for at most the hand-off timeout and
//! the history is file IO, neither of which belongs on the Qt thread or on
//! the save worker the editor joins. A failure is said on stderr and changes
//! nothing on disk. The usual way a copy gets here — «Guardar ambas» on the
//! window Selenita's preview opened, then closing it — is also the moment
//! the process ends, so `main` calls [`shutdown`] after the event loop: what
//! is queued is delivered, waiting a bounded time for a stuck bus.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use celestina_core::activation;
use selenita_core::history::{adopted, FolderPaths, History};
use selenita_core::names;

/// Selenita's own interface, which serves `Adopt`.
const ADOPT_INTERFACE: &str = "org.celestina.Selenita1";

/// How long the end of the process waits for the copies still queued: one
/// bus hand-off and a second for the connection and the history file.
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(activation::HAND_OFF_TIMEOUT.as_secs() + 1);

/// The adoption worker, started by the first copy and closed by
/// [`shutdown`]; after that, copies are no longer taken.
enum State {
    Idle,
    Running(Worker),
    Closed,
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

/// A poisoned lock still holds a usable worker.
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Hands a copy that just landed to the adoption worker. Never blocks and
/// never fails: without a worker the copy is only not adopted, which is said.
pub fn request(copy: &Path) {
    let mut state = state();
    if matches!(*state, State::Idle) {
        match Worker::start(adopt_and_report) {
            Ok(worker) => *state = State::Running(worker),
            Err(error) => {
                eprintln!("fluorita: no adoption worker, copies stay out of Selenita: {error}");
                *state = State::Closed;
            }
        }
    }
    match &*state {
        State::Running(worker) => worker.send(copy),
        State::Idle | State::Closed => eprintln!(
            "fluorita: {} was saved as the process ended and stays out of Selenita's history",
            copy.display()
        ),
    }
}

/// Delivers what is queued and ends the worker, waiting at most `wait`; a
/// copy still on a stuck bus after that is said and left to the exit.
pub fn shutdown(wait: Duration) {
    let previous = std::mem::replace(&mut *state(), State::Closed);
    if let State::Running(worker) = previous {
        if !worker.shutdown(wait) {
            eprintln!(
                "fluorita: Selenita did not answer in time; a copy may stay out of its history"
            );
        }
    }
}

/// The production delivery: the bus, the history file and the folders of
/// this session.
fn adopt_and_report(copy: &Path) {
    let outcome = adopt(
        copy,
        &Bus,
        History::default_file(),
        session_folders(
            names::pictures_dir().as_deref(),
            names::videos_dir().as_deref(),
        )
        .as_ref(),
    );
    if let Adopted::Failed(reason) = outcome {
        eprintln!(
            "fluorita: {} did not join Selenita's history: {reason}",
            copy.display()
        );
    }
}

/// One thread fed in arrival order, which says when it has ended.
struct Worker {
    copies: Sender<PathBuf>,
    /// Disconnects when the thread's loop ends: what a bounded join waits on.
    ended: Receiver<()>,
    thread: JoinHandle<()>,
}

impl Worker {
    fn start(deliver: impl Fn(&Path) + Send + 'static) -> std::io::Result<Self> {
        let (copies, queue) = std::sync::mpsc::channel::<PathBuf>();
        let (ending, ended) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("fluorita-adopt".to_owned())
            .spawn(move || {
                // Held for the loop's lifetime; dropping it is the signal.
                let _ending = ending;
                for copy in queue {
                    deliver(&copy);
                }
            })?;
        Ok(Self {
            copies,
            ended,
            thread,
        })
    }

    fn send(&self, copy: &Path) {
        let _ = self.copies.send(copy.to_path_buf());
    }

    /// Closes the queue and waits up to `wait` for what is in it. `true`
    /// when the thread ended and was joined.
    fn shutdown(self, wait: Duration) -> bool {
        let Self {
            copies,
            ended,
            thread,
        } = self;
        drop(copies);
        match ended.recv_timeout(wait) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => thread.join().is_ok(),
            Err(RecvTimeoutError::Timeout) => false,
        }
    }
}

/// Where Selenita keeps what it adopts, inside the session's `pictures`
/// and `videos` folders (as `selenita_core::names` finds them:
/// `XDG_PICTURES_DIR` and `XDG_VIDEOS_DIR`, else `user-dirs.dirs`), named
/// by `FolderPaths::inside`, the constructor Selenita's own `Adopt` uses.
/// `None` when neither folder is named.
///
/// `FolderPaths` holds both folders, so when the session names only one,
/// that one's adopting folder stands in for the other, which `adopted`
/// already accepts: the unnamed one adopts nothing of its own. This is not
/// Selenita's lookup there. Selenita falls back to `$HOME` (then the
/// temporary folder) for an unnamed folder and adopts inside it, where it
/// also writes; Fluorita does not guess, so a copy only that fallback would
/// keep joins the history through a running Selenita and is left out of the
/// offline append.
pub(crate) fn session_folders(
    pictures: Option<&Path>,
    videos: Option<&Path>,
) -> Option<FolderPaths> {
    let only = |folder: PathBuf| FolderPaths {
        captures: folder.clone(),
        recordings: folder,
    };
    match (pictures, videos) {
        (Some(pictures), Some(videos)) => Some(FolderPaths::inside(pictures, videos)),
        (Some(pictures), None) => Some(only(names::captures_dir_in(pictures))),
        (None, Some(videos)) => Some(only(names::recordings_dir_in(videos))),
        (None, None) => None,
    }
}

/// What asking Selenita came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    /// A running Selenita took the key.
    Taken,
    /// Nobody owns Selenita's name.
    NobodyRuns,
    /// The bus, or the question of who owns the name, failed: no Selenita
    /// can be reached, so none can be holding its history in memory either.
    Unreachable(String),
    /// Selenita runs and its `Adopt` failed or did not answer in time.
    Failed(String),
}

/// Whoever answers `Adopt`: the bus in the binary, a fake under test.
pub(crate) trait Selenita {
    fn adopt(&self, key: &str) -> Answer;
}

struct Bus;

impl Selenita for Bus {
    fn adopt(&self, key: &str) -> Answer {
        let unreachable = |error: &dyn std::fmt::Display| Answer::Unreachable(error.to_string());
        let connection = match zbus::blocking::connection::Builder::session()
            .map(|builder| builder.method_timeout(activation::HAND_OFF_TIMEOUT))
            .and_then(zbus::blocking::connection::Builder::build)
        {
            Ok(connection) => connection,
            Err(error) => return unreachable(&error),
        };
        let owned = zbus::blocking::fdo::DBusProxy::new(&connection)
            .map_err(|error| error.to_string())
            .and_then(|bus| {
                let name = zbus::names::BusName::try_from(activation::SELENITA.0)
                    .map_err(|error| error.to_string())?;
                bus.name_has_owner(name).map_err(|error| error.to_string())
            });
        match owned {
            Ok(true) => {}
            Ok(false) => return Answer::NobodyRuns,
            Err(error) => return unreachable(&error),
        }
        let path = activation::object_path(&activation::SELENITA);
        let called = zbus::blocking::Proxy::new(
            &connection,
            activation::SELENITA.0,
            path.as_str(),
            ADOPT_INTERFACE,
        )
        .and_then(|proxy| proxy.call::<_, _, ()>("Adopt", &(key,)));
        match called {
            Ok(()) => Answer::Taken,
            Err(error) => Answer::Failed(error.to_string()),
        }
    }
}

/// What became of one copy.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Adopted {
    /// A running Selenita was asked and answered.
    BySelenita,
    /// No Selenita ran: the row is in its history file.
    InHistory,
    /// Not a file Selenita's history keeps.
    Ignored,
    /// Said on stderr; nothing was written.
    Failed(String),
}

/// Adopts `copy`: through a running Selenita, else into `history_file`.
///
/// The file is written only when no Selenita can be running: nobody owns
/// its name, or the bus (or the question of who owns the name) cannot be
/// reached. When the `Adopt` call itself fails, Selenita runs, and writing
/// its file under it would race the copy it keeps in memory, so the failure
/// is only reported.
pub(crate) fn adopt(
    copy: &Path,
    selenita: &dyn Selenita,
    history_file: Option<PathBuf>,
    folders: Option<&FolderPaths>,
) -> Adopted {
    match selenita.adopt(&celestina_core::pathkey::encode(copy)) {
        Answer::Taken => Adopted::BySelenita,
        Answer::NobodyRuns | Answer::Unreachable(_) => append(copy, history_file, folders),
        Answer::Failed(reason) => Adopted::Failed(reason),
    }
}

/// Appends `copy` to the history in `history_file`, taken now, if it is one
/// the history keeps by Selenita's rule.
fn append(copy: &Path, history_file: Option<PathBuf>, folders: Option<&FolderPaths>) -> Adopted {
    let Some(entry) =
        folders.and_then(|folders| adopted(copy, folders.folders(), SystemTime::now()))
    else {
        return Adopted::Ignored;
    };
    let Some(file) = history_file else {
        return Adopted::Failed("no data home names Selenita's history".to_owned());
    };
    let mut history = match History::load(file) {
        Ok(history) => history,
        Err(error) => return Adopted::Failed(error.to_string()),
    };
    history.push(entry);
    match history.save() {
        Ok(()) => Adopted::InHistory,
        Err(error) => Adopted::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{adopt, session_folders, Adopted, Answer, Selenita, Worker};
    use selenita_core::history::FolderPaths;
    use selenita_core::names;
    use selenita_core::{Entry, EntryKind, History};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime};

    use crate::test_support::Scratch;

    /// A scratch session: its pictures and videos folders, and the folders
    /// Selenita keeps captures and recordings in, named by `selenita_core`.
    struct Session {
        scratch: Scratch,
    }

    impl Session {
        fn new(name: &str) -> Self {
            Self {
                scratch: Scratch::new(name),
            }
        }

        fn pictures(&self) -> PathBuf {
            self.scratch.path().join("Pictures")
        }

        fn videos(&self) -> PathBuf {
            self.scratch.path().join("Videos")
        }

        fn places(&self) -> Option<FolderPaths> {
            session_folders(Some(&self.pictures()), Some(&self.videos()))
        }

        fn captures(&self) -> PathBuf {
            names::captures_dir_in(&self.pictures())
        }

        fn recordings(&self) -> PathBuf {
            names::recordings_dir_in(&self.videos())
        }

        fn history_file(&self) -> PathBuf {
            self.scratch.path().join("data/selenita/history")
        }
    }

    /// Answers every `Adopt` the same way and keeps the keys it was asked.
    struct FakeSelenita {
        answer: Answer,
        asked: Mutex<Vec<String>>,
    }

    impl FakeSelenita {
        fn answering(answer: Answer) -> Self {
            Self {
                answer,
                asked: Mutex::new(Vec::new()),
            }
        }

        fn asked(&self) -> Vec<String> {
            self.asked.lock().expect("asked").clone()
        }
    }

    impl Selenita for FakeSelenita {
        fn adopt(&self, key: &str) -> Answer {
            self.asked.lock().expect("asked").push(key.to_owned());
            self.answer.clone()
        }
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
        std::fs::write(path, b"bytes").expect("a file");
    }

    /// The kind the offline append wrote for `copy`, or `None` when it was
    /// left out.
    fn appended_kind(session: &Session, copy: &Path) -> Option<EntryKind> {
        touch(copy);
        let file = session.history_file();
        let _ = std::fs::remove_file(&file);
        let adopted = adopt(
            copy,
            &FakeSelenita::answering(Answer::NobodyRuns),
            Some(file.clone()),
            session.places().as_ref(),
        );
        match adopted {
            Adopted::InHistory => {
                let history = History::load(file).expect("the history");
                assert_eq!(history.entries()[0].path, copy);
                Some(history.entries()[0].kind)
            }
            Adopted::Ignored => {
                assert!(!file.exists());
                None
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn the_folders_are_selenitas_own_names() {
        let session = Session::new("adopt-places");
        let folders = session.places().expect("two folders");

        assert_eq!(
            folders.captures,
            session.pictures().join(names::CAPTURES_FOLDER)
        );
        assert_eq!(
            folders.recordings,
            session.videos().join(names::RECORDINGS_FOLDER)
        );
    }

    #[test]
    fn selenitas_rule_decides_what_the_append_keeps() {
        let session = Session::new("adopt-kind");
        let captures = session.captures();
        let recordings = session.recordings();

        assert_eq!(
            appended_kind(&session, &captures.join("Captura (editado).png")),
            Some(EntryKind::Screenshot)
        );
        assert_eq!(
            appended_kind(&session, &recordings.join("Recording (editado).mp4")),
            Some(EntryKind::Recording)
        );
        assert_eq!(appended_kind(&session, &captures.join("foto.jpg")), None);
        assert_eq!(
            appended_kind(&session, &captures.join("dentro/x.png")),
            None
        );
        assert_eq!(
            appended_kind(&session, &session.scratch.path().join("x.png")),
            None
        );
    }

    #[test]
    fn a_session_that_names_neither_folder_keeps_nothing() {
        let session = Session::new("adopt-nowhere");
        let copy = session.captures().join("x.png");
        touch(&copy);
        let file = session.history_file();

        assert_eq!(
            adopt(
                &copy,
                &FakeSelenita::answering(Answer::NobodyRuns),
                Some(file.clone()),
                session_folders(None, None).as_ref()
            ),
            Adopted::Ignored
        );
        assert!(!file.exists());
    }

    #[test]
    fn without_a_pictures_folder_only_the_recordings_folder_adopts() {
        let session = Session::new("adopt-videos-only");
        let folders =
            session_folders(None, Some(&session.videos())).expect("the recordings folder");

        assert_eq!(folders.captures, session.recordings());
        assert_eq!(folders.recordings, session.recordings());
    }

    #[test]
    fn without_selenita_the_copy_joins_the_history_first() {
        let session = Session::new("adopt-history");
        let older = session.captures().join("Captura.png");
        touch(&older);
        let file = session.history_file();
        let mut history = History::at(file.clone());
        history.push(Entry {
            path: older.clone(),
            kind: EntryKind::Screenshot,
            taken_at: SystemTime::UNIX_EPOCH,
            size: 5,
        });
        history.save().expect("a history");
        let copy = session.captures().join("Captura (editado).png");
        touch(&copy);
        let selenita = FakeSelenita::answering(Answer::NobodyRuns);

        let adopted = adopt(
            &copy,
            &selenita,
            Some(file.clone()),
            session.places().as_ref(),
        );

        assert_eq!(adopted, Adopted::InHistory);
        assert_eq!(selenita.asked(), [celestina_core::pathkey::encode(&copy)]);
        let history = History::load(file).expect("the history");
        let paths: Vec<&PathBuf> = history.entries().iter().map(|entry| &entry.path).collect();
        assert_eq!(paths, [&copy, &older]);
        assert_eq!(history.entries()[0].kind, EntryKind::Screenshot);
        assert_eq!(history.entries()[0].size, 5);
    }

    #[test]
    fn a_session_without_a_bus_still_keeps_the_copy() {
        let session = Session::new("adopt-no-bus");
        let copy = session.recordings().join("Recording (editado).mp4");
        touch(&copy);
        let file = session.history_file();
        let selenita = FakeSelenita::answering(Answer::Unreachable("no session bus".to_owned()));

        assert_eq!(
            adopt(
                &copy,
                &selenita,
                Some(file.clone()),
                session.places().as_ref()
            ),
            Adopted::InHistory
        );
        let history = History::load(file).expect("the history");
        assert_eq!(history.entries()[0].kind, EntryKind::Recording);
    }

    #[test]
    fn with_selenita_running_it_decides_and_the_file_is_left_alone() {
        let session = Session::new("adopt-running");
        let copy = session.captures().join("Captura (editado).png");
        touch(&copy);
        let file = session.history_file();
        let selenita = FakeSelenita::answering(Answer::Taken);

        assert_eq!(
            adopt(
                &copy,
                &selenita,
                Some(file.clone()),
                session.places().as_ref()
            ),
            Adopted::BySelenita
        );
        assert_eq!(selenita.asked(), [celestina_core::pathkey::encode(&copy)]);
        assert!(!file.exists());
    }

    #[test]
    fn a_copy_selenita_does_not_keep_is_left_out_of_its_history() {
        let session = Session::new("adopt-elsewhere");
        let copy = session.scratch.path().join("Fotos/viaje (editado).png");
        touch(&copy);
        let file = session.history_file();

        assert_eq!(
            adopt(
                &copy,
                &FakeSelenita::answering(Answer::NobodyRuns),
                Some(file.clone()),
                session.places().as_ref()
            ),
            Adopted::Ignored
        );
        assert!(!file.exists());
    }

    #[test]
    fn a_running_selenita_that_refuses_is_reported_and_nothing_is_written() {
        let session = Session::new("adopt-refused");
        let copy = session.captures().join("Captura (editado).png");
        touch(&copy);
        let file = session.history_file();
        let selenita = FakeSelenita::answering(Answer::Failed(
            "org.freedesktop.DBus.Error.NoReply: no answer in time".to_owned(),
        ));

        assert!(matches!(
            adopt(
                &copy,
                &selenita,
                Some(file.clone()),
                session.places().as_ref()
            ),
            Adopted::Failed(_)
        ));
        assert!(!file.exists());
    }

    #[test]
    fn a_copy_queued_just_before_the_end_is_still_delivered() {
        let delivered = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let seen = Arc::clone(&delivered);
        // A hand-off that takes a while, as a bus call does.
        let worker = Worker::start(move |copy: &Path| {
            std::thread::sleep(Duration::from_millis(300));
            seen.lock().expect("delivered").push(copy.to_path_buf());
        })
        .expect("a worker");
        worker.send(Path::new("/p/a.png"));
        worker.send(Path::new("/p/b.png"));

        assert!(worker.shutdown(Duration::from_secs(3)));

        assert_eq!(
            *delivered.lock().expect("delivered"),
            [PathBuf::from("/p/a.png"), PathBuf::from("/p/b.png")]
        );
    }

    #[test]
    fn a_stuck_hand_off_holds_the_end_for_the_bound_only() {
        let worker =
            Worker::start(|_: &Path| std::thread::sleep(Duration::from_secs(5))).expect("a worker");
        worker.send(Path::new("/p/a.png"));
        let started = Instant::now();

        assert!(!worker.shutdown(Duration::from_millis(200)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
