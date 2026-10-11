// language-contract: product-copy
//! The capture worker: one named thread that owns the backend and the
//! history and runs every capture and every history action in order, so the
//! Qt thread never waits on a tool, a socket or a file.
//!
//! A capture runs its plan here: note the window a window capture means
//! (the latest focused one that is not Selenita), count the delay down (one
//! `Countdown` a second), resolve the target (niri's outputs or focused
//! window, or `slurp` for a region), take the picture into a hidden file,
//! publish it under its name in the captures folder without replacing
//! anything, put it on the clipboard when asked, push it to the history and
//! report it, then report its corner preview (`preview.rs`). A recording
//! the recording worker published joins the history here too, with its
//! preview and poster; the preview's click hands the file to Fluorita here;
//! and a file another application hands back (`Selenita1.Adopt`) joins the
//! history when it is one of Selenita's. Every failure is a
//! [`CaptureError`] whose `message_es` the window shows in its notice.
//! Selenita's window stays where it is: the person may want it in the
//! picture (SEL-1-D), and a window capture is the window focused before
//! Selenita in any case.
//!
//! The thread is detached and never joined, as Cuprita's workers: a tool
//! still running when the window closes is abandoned with the process.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, SystemTime};

use celestina_core::activation::SELENITA;
use celestina_core::{atomic_file, pathkey, xdg};
use selenita_core::history::{adopted, Folders, HistoryError};
use selenita_core::niri::{self, NiriError};
use selenita_core::runner::RunError;
use selenita_core::{capture_file_name, names, pictures_dir};
use selenita_core::{Entry, EntryKind, History, Target, TargetKind};

use crate::backend::Backend;
use crate::preview::{self, Preview};

/// How many numbered names a capture tries before giving up.
const NAME_ATTEMPTS: u32 = 1000;

/// Why a capture or a history action failed.
#[derive(Debug)]
pub enum CaptureError {
    Niri(NiriError),
    Tool(RunError),
    /// `slurp` printed something that is not a region.
    Selection,
    /// The tool succeeded and left no picture.
    NoPicture,
    Write(String),
    History(HistoryError),
    /// Neither the clipboard nor the folder was asked for.
    NoDestination,
    /// A capture is already under way.
    Busy,
    /// The history has no entry with that id.
    UnknownEntry,
    /// «Copiar» on a recording: only a picture goes to the clipboard.
    NotAPicture,
    Open(String),
    /// The preview's file could not be handed to Fluorita's editor.
    Edit(String),
    Show(String),
    Trash(String),
    /// The worker thread is gone.
    WorkerGone,
}

impl CaptureError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> String {
        match self {
            Self::Niri(NiriError::NoSocket) => {
                "Selenita necesita niri para capturar una ventana.".to_owned()
            }
            Self::Niri(NiriError::NoFocusedWindow) => "No hay ninguna ventana activa.".to_owned(),
            Self::Niri(_) => "niri no ha respondido.".to_owned(),
            Self::Tool(RunError::Missing(program)) => {
                let name = Path::new(program).file_name().map_or_else(
                    || program.clone(),
                    |name| name.to_string_lossy().into_owned(),
                );
                format!("Falta «{name}»: instálalo para capturar.")
            }
            Self::Tool(RunError::Deadline(_)) => {
                "La herramienta de captura no ha respondido a tiempo.".to_owned()
            }
            Self::Tool(_) => "No se ha podido hacer la captura.".to_owned(),
            Self::Selection => "La región elegida no es válida.".to_owned(),
            Self::NoPicture => "La captura no ha producido ninguna imagen.".to_owned(),
            Self::Write(_) => "No se ha podido guardar la captura.".to_owned(),
            Self::History(_) => "No se ha podido guardar el historial.".to_owned(),
            Self::NoDestination => {
                "Elige al menos un destino: el portapapeles o la carpeta.".to_owned()
            }
            Self::Busy => "Ya hay una captura en curso.".to_owned(),
            Self::UnknownEntry => "Esa captura ya no está en el historial.".to_owned(),
            Self::NotAPicture => "Una grabación no se copia al portapapeles.".to_owned(),
            Self::Open(_) => "No se ha podido abrir en Fluorita.".to_owned(),
            Self::Edit(_) => "No se ha podido abrir Fluorita.".to_owned(),
            Self::Show(_) => "No se ha podido mostrar en Siderita.".to_owned(),
            Self::Trash(_) => "No se ha podido mover a la papelera.".to_owned(),
            Self::WorkerGone => "La captura no está disponible.".to_owned(),
        }
    }
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Niri(error) => write!(formatter, "{error}"),
            Self::Tool(error) => write!(formatter, "{error}"),
            Self::History(error) => write!(formatter, "{error}"),
            Self::Write(detail)
            | Self::Open(detail)
            | Self::Edit(detail)
            | Self::Show(detail)
            | Self::Trash(detail) => write!(formatter, "{self:?}: {detail}"),
            other => write!(formatter, "{other:?}"),
        }
    }
}

/// One capture as the controller asked for it.
#[derive(Clone, Debug)]
pub struct Request {
    pub kind: TargetKind,
    /// The output for a screen capture; empty for every output.
    pub output: String,
    pub delay: Duration,
    pub to_clipboard: bool,
    pub to_file: bool,
    /// slurp's colours, `#RRGGBBAA`.
    pub accent: String,
    pub background: String,
    /// Product copy from the Qt seam: the file name's stem («Captura») and
    /// the folder inside the pictures folder («Capturas»).
    pub stem: String,
    pub folder: String,
}

/// What a finished capture came to.
#[derive(Debug)]
pub enum Outcome {
    /// The picture was taken: the history entry when it went to a file, and
    /// how the clipboard copy went when it was asked for.
    Captured {
        entry: Option<Entry>,
        clipboard: Result<(), CaptureError>,
    },
    /// The person cancelled the region.
    Cancelled,
}

/// Where a capture's files go: the pictures folder (the captures folder is
/// made inside it) and a private folder for a picture that only goes to the
/// clipboard.
pub struct Places {
    pub pictures: PathBuf,
    pub staging: PathBuf,
}

impl Places {
    /// The pictures folder (else the home), and the runtime folder's
    /// `selenita` (else the system temporary folder's). Blocking.
    fn resolve() -> Self {
        let pictures = pictures_dir()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(std::env::temp_dir);
        let staging = xdg::runtime_dir()
            .map(|dir| dir.join("selenita"))
            .unwrap_or_else(|_| std::env::temp_dir().join("selenita"));
        Self { pictures, staging }
    }
}

static NEXT_HIDDEN: AtomicU64 = AtomicU64::new(1);

fn hidden_file(folder: &Path) -> PathBuf {
    folder.join(format!(
        ".selenita-{}-{}.png",
        std::process::id(),
        NEXT_HIDDEN.fetch_add(1, Ordering::Relaxed)
    ))
}

/// What a capture tells the window while it runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    /// Seconds left; the last is 0.
    Countdown(u32),
}

/// Runs one capture's plan. `sleep` waits (a test passes one that does not),
/// `progress` hears the seconds left.
///
/// # Errors
///
/// The first step that failed; the hidden file is removed then.
pub fn capture(
    backend: &mut dyn Backend,
    history: &mut History,
    places: &Places,
    request: &Request,
    sleep: &dyn Fn(Duration),
    progress: &mut dyn FnMut(Progress),
) -> Result<Outcome, CaptureError> {
    let window_kind = request.kind == TargetKind::Window;
    // niri copies a window to the clipboard itself: that is a destination.
    if !request.to_clipboard && !request.to_file && !window_kind {
        return Err(CaptureError::NoDestination);
    }
    // Selenita is focused while the person presses its button, so the
    // window meant is the latest one focused before it.
    let meant = if window_kind {
        let windows = backend.windows()?;
        Some(
            niri::pick_window(&windows, SELENITA.0)
                .cloned()
                .ok_or(CaptureError::Niri(NiriError::NoFocusedWindow))?,
        )
    } else {
        None
    };
    let seconds = u32::try_from(request.delay.as_secs()).unwrap_or(0);
    for left in (1..=seconds).rev() {
        progress(Progress::Countdown(left));
        sleep(Duration::from_secs(1));
    }
    if seconds > 0 {
        progress(Progress::Countdown(0));
    }

    let target = match (request.kind, meant) {
        (TargetKind::Window, Some(window)) => Target::Window {
            id: window.id,
            geometry: window.geometry(),
            app_id: window.app_id,
            title: window.title,
        },
        (TargetKind::Region, _) => {
            match backend.select_region(&request.accent, &request.background)? {
                Some(geometry) => Target::Region { geometry },
                None => return Ok(Outcome::Cancelled),
            }
        }
        _ => Target::Screen {
            output: request.output.clone(),
        },
    };

    let folder = if request.to_file {
        places.pictures.join(&request.folder)
    } else {
        places.staging.clone()
    };
    std::fs::create_dir_all(&folder).map_err(|error| CaptureError::Write(error.to_string()))?;
    let hidden = hidden_file(&folder);
    let result = take(backend, history, request, &target, &folder, &hidden);
    let _ = std::fs::remove_file(&hidden);
    result
}

fn take(
    backend: &mut dyn Backend,
    history: &mut History,
    request: &Request,
    target: &Target,
    folder: &Path,
    hidden: &Path,
) -> Result<Outcome, CaptureError> {
    backend.grab(target, hidden)?;
    let png = std::fs::read(hidden).map_err(|_| CaptureError::NoPicture)?;
    if png.is_empty() {
        return Err(CaptureError::NoPicture);
    }
    let now = SystemTime::now();
    let entry = if request.to_file {
        let path = publish(
            hidden,
            folder,
            &capture_file_name(&request.stem, now, "png"),
        )?;
        let entry = Entry {
            path,
            kind: EntryKind::Screenshot,
            taken_at: now,
            size: png.len() as u64,
        };
        history.push(entry.clone());
        if let Err(error) = history.save() {
            // The picture is saved; only its line in the history may be
            // lost, so the capture still counts.
            eprintln!("selenita: {error}");
        }
        Some(entry)
    } else {
        None
    };
    // niri already put a window on the clipboard.
    let clipboard = if request.to_clipboard && !matches!(target, Target::Window { .. }) {
        backend.copy_png(&png)
    } else {
        Ok(())
    };
    Ok(Outcome::Captured { entry, clipboard })
}

/// Publishes `hidden` as `name` in `folder`, or as the first free numbered
/// name, never replacing a file. The recording worker publishes its file the
/// same way.
pub(crate) fn publish(hidden: &Path, folder: &Path, name: &str) -> Result<PathBuf, CaptureError> {
    for n in 1..=NAME_ATTEMPTS {
        let destination = folder.join(names::numbered(name, n));
        match atomic_file::publish_without_replacing(hidden, &destination) {
            Ok(()) => return Ok(destination),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(CaptureError::Write(error.to_string())),
        }
    }
    Err(CaptureError::Write(format!("no free name for {name}")))
}

/// What the worker reports to the Qt thread.
#[derive(Debug)]
pub enum Report {
    /// The history as it now is, the latest first.
    History(Vec<Entry>),
    /// The enabled outputs' names, left to right.
    Outputs(Vec<String>),
    /// Seconds left before the picture is taken.
    Countdown(u32),
    /// The capture is done: the entry's id when it went to a file.
    Captured(Option<String>),
    /// The person cancelled the region.
    Cancelled,
    /// The capture failed, in Spanish.
    CaptureFailed(String),
    /// A history action failed, in Spanish.
    Failed(String),
    /// A result was published: the corner preview shows it.
    Preview(Preview),
    /// The preview's file went to Fluorita.
    Edited,
    /// The preview's file (its key) could not be handed to Fluorita, in
    /// Spanish.
    EditFailed { key: String, message: String },
}

/// A history row action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    OpenInFluorita,
    Copy,
    ShowInSiderita,
    Delete,
}

enum Job {
    Capture(Request),
    Entry(Action, String),
    /// A recording finished elsewhere joins the history, which lives here.
    Push(Entry),
    /// The preview's file, by its key, goes to Fluorita's editor.
    Edit(String),
    /// Another application hands a file back (`Selenita1.Adopt`): its key
    /// and the captures folder's name inside the pictures folder.
    Adopt {
        key: String,
        folder: String,
    },
}

/// The sending half of the capture worker.
pub struct Worker {
    jobs: Sender<Job>,
}

impl Worker {
    /// Starts the thread: it builds the backend, loads the history, reads
    /// the outputs and reports both, then waits for jobs. `report` runs on
    /// the worker and must queue to the Qt thread itself.
    pub fn start(fake: bool, report: impl Fn(Report) + Send + 'static) -> Option<Self> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("selenita-capture".to_owned())
            .spawn(move || {
                let mut backend = crate::backend::make(fake);
                let mut history = load_history(&report);
                report(Report::History(history.entries().to_vec()));
                // Outside niri there are no outputs to choose: the screen
                // capture then takes every output, and nothing is said.
                let outputs = backend.outputs().unwrap_or_default();
                report(Report::Outputs(
                    outputs.into_iter().map(|output| output.name).collect(),
                ));
                // The last recording's poster, removed when a new preview
                // replaces it.
                let mut poster = None;
                for job in inbox {
                    match job {
                        Job::Capture(request) => {
                            let places = Places::resolve();
                            let outcome = capture(
                                backend.as_mut(),
                                &mut history,
                                &places,
                                &request,
                                &std::thread::sleep,
                                &mut |step| {
                                    report(match step {
                                        Progress::Countdown(left) => Report::Countdown(left),
                                    });
                                },
                            );
                            let saved = match &outcome {
                                Ok(Outcome::Captured {
                                    entry: Some(entry), ..
                                }) => Some(entry.clone()),
                                _ => None,
                            };
                            finish(outcome, &history, &report);
                            if let Some(entry) = saved {
                                // A picture needs no poster folder.
                                report(Report::Preview(preview::of(
                                    backend.as_mut(),
                                    &entry,
                                    None,
                                    &mut poster,
                                )));
                            }
                        }
                        Job::Push(entry) => {
                            history.push(entry.clone());
                            if let Err(error) = history.save() {
                                eprintln!("selenita: {error}");
                                report(Report::Failed(CaptureError::History(error).message_es()));
                            }
                            report(Report::History(history.entries().to_vec()));
                            let staging = preview::poster_folder();
                            report(Report::Preview(preview::of(
                                backend.as_mut(),
                                &entry,
                                staging.as_deref(),
                                &mut poster,
                            )));
                        }
                        Job::Edit(key) => match edit(backend.as_mut(), &key) {
                            Ok(()) => report(Report::Edited),
                            Err(error) => {
                                eprintln!("selenita: edit: {error}");
                                report(Report::EditFailed {
                                    key,
                                    message: error.message_es(),
                                });
                            }
                        },
                        Job::Adopt { key, folder } => {
                            let pictures = Places::resolve().pictures;
                            let recordings = crate::record::Places::resolve().recordings;
                            // Without the captures folder's name only the
                            // recordings folder adopts.
                            let captures = if folder.is_empty() {
                                recordings.clone()
                            } else {
                                pictures.join(&folder)
                            };
                            let folders = Folders {
                                captures: &captures,
                                recordings: &recordings,
                            };
                            if adopt(&mut history, folders, &key, SystemTime::now()) {
                                report(Report::History(history.entries().to_vec()));
                            }
                        }
                        Job::Entry(action, id) => {
                            match act(backend.as_mut(), &mut history, action, &id) {
                                Ok(true) => report(Report::History(history.entries().to_vec())),
                                Ok(false) => {}
                                Err(error) => {
                                    eprintln!("selenita: {action:?}: {error}");
                                    report(Report::Failed(error.message_es()));
                                    report(Report::History(history.entries().to_vec()));
                                }
                            }
                        }
                    }
                }
            })
            .ok()
            .map(|_| Self { jobs })
    }

    /// Queues a capture.
    ///
    /// # Errors
    ///
    /// [`CaptureError::WorkerGone`] when the thread is gone.
    pub fn capture(&self, request: Request) -> Result<(), CaptureError> {
        self.jobs
            .send(Job::Capture(request))
            .map_err(|_| CaptureError::WorkerGone)
    }

    /// Queues a row action on the entry `id`.
    ///
    /// # Errors
    ///
    /// [`CaptureError::WorkerGone`] when the thread is gone.
    pub fn act(&self, action: Action, id: String) -> Result<(), CaptureError> {
        self.jobs
            .send(Job::Entry(action, id))
            .map_err(|_| CaptureError::WorkerGone)
    }

    /// Queues a finished recording for the history.
    ///
    /// # Errors
    ///
    /// [`CaptureError::WorkerGone`] when the thread is gone.
    pub fn push(&self, entry: Entry) -> Result<(), CaptureError> {
        self.jobs
            .send(Job::Push(entry))
            .map_err(|_| CaptureError::WorkerGone)
    }

    /// Queues the hand-off of the file `key` names to Fluorita's editor.
    ///
    /// # Errors
    ///
    /// [`CaptureError::WorkerGone`] when the thread is gone.
    pub fn edit(&self, key: String) -> Result<(), CaptureError> {
        self.jobs
            .send(Job::Edit(key))
            .map_err(|_| CaptureError::WorkerGone)
    }

    /// Queues a file handed back by another application; `folder` is the
    /// captures folder's name.
    ///
    /// # Errors
    ///
    /// [`CaptureError::WorkerGone`] when the thread is gone.
    pub fn adopt(&self, key: String, folder: String) -> Result<(), CaptureError> {
        self.jobs
            .send(Job::Adopt { key, folder })
            .map_err(|_| CaptureError::WorkerGone)
    }
}

fn load_history(report: &impl Fn(Report)) -> History {
    let Some(file) = History::default_file() else {
        return History::default();
    };
    match History::load(file.clone()) {
        Ok(history) => {
            // Entries whose file vanished were dropped: write that down.
            let _ = history.save();
            history
        }
        Err(error) => {
            eprintln!("selenita: {error}");
            report(Report::Failed(CaptureError::History(error).message_es()));
            History::at(file)
        }
    }
}

fn finish(outcome: Result<Outcome, CaptureError>, history: &History, report: &impl Fn(Report)) {
    match outcome {
        Ok(Outcome::Captured { entry, clipboard }) => {
            if entry.is_some() {
                report(Report::History(history.entries().to_vec()));
            }
            if let Err(error) = clipboard {
                eprintln!("selenita: clipboard: {error}");
                report(Report::Failed(error.message_es()));
            }
            report(Report::Captured(entry.map(|entry| entry.id())));
        }
        Ok(Outcome::Cancelled) => report(Report::Cancelled),
        Err(error) => {
            eprintln!("selenita: capture: {error}");
            report(Report::CaptureFailed(error.message_es()));
        }
    }
}

/// Hands the file `key` names to Fluorita's editor, when it is still there.
fn edit(backend: &mut dyn Backend, key: &str) -> Result<(), CaptureError> {
    let path = pathkey::decode(key).map_err(|_| CaptureError::UnknownEntry)?;
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(CaptureError::UnknownEntry);
    }
    backend.edit_in_fluorita(&path)
}

/// `Selenita1.Adopt`: the file `key` names joins the history, taken `now`,
/// when it is a capture or a recording in Selenita's `folders`
/// ([`adopted`]); anything else is ignored. Whether it joined.
fn adopt(history: &mut History, folders: Folders<'_>, key: &str, now: SystemTime) -> bool {
    let Ok(path) = pathkey::decode(key) else {
        return false;
    };
    let Some(entry) = adopted(&path, folders, now) else {
        return false;
    };
    history.push(entry);
    if let Err(error) = history.save() {
        // The entry is listed; only its line in the file may be lost.
        eprintln!("selenita: {error}");
    }
    true
}

/// Runs a row action; whether the history changed.
fn act(
    backend: &mut dyn Backend,
    history: &mut History,
    action: Action,
    id: &str,
) -> Result<bool, CaptureError> {
    let (path, kind) = history
        .find(id)
        .map(|entry| (entry.path.clone(), entry.kind))
        .ok_or(CaptureError::UnknownEntry)?;
    if std::fs::symlink_metadata(&path).is_err() {
        // The file went away under the history: forget it and say so.
        history.remove(&path);
        let _ = history.save();
        return Err(CaptureError::UnknownEntry);
    }
    match action {
        Action::OpenInFluorita => backend.open_in_fluorita(&path).map(|()| false),
        Action::ShowInSiderita => backend.show_in_siderita(&path).map(|()| false),
        Action::Copy if kind == EntryKind::Recording => Err(CaptureError::NotAPicture),
        Action::Copy => {
            let png = std::fs::read(&path).map_err(|_| CaptureError::UnknownEntry)?;
            backend.copy_png(&png).map(|()| false)
        }
        Action::Delete => {
            backend.trash(&path)?;
            history.remove(&path);
            history.save().map_err(CaptureError::History)?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        act, adopt, capture, edit, publish, Action, CaptureError, Outcome, Places, Progress,
        Request,
    };
    use crate::backend::{self, Backend, PIXEL_PNG};
    use selenita_core::niri::{Output, Window};
    use selenita_core::{Geometry, History, Target, TargetKind};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("selenita-capture-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }

        fn places(&self) -> Places {
            Places {
                pictures: self.0.join("pictures"),
                staging: self.0.join("run"),
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request(kind: TargetKind) -> Request {
        Request {
            kind,
            output: String::new(),
            delay: Duration::ZERO,
            to_clipboard: true,
            to_file: true,
            accent: "#3e91ffff".to_owned(),
            background: "#00000040".to_owned(),
            stem: "Captura".to_owned(),
            folder: "Capturas".to_owned(),
        }
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn a_screen_capture_lands_in_the_folder_and_the_history() {
        let scratch = Scratch::new("screen");
        let mut fake = backend::make(true);
        let mut history = History::at(scratch.0.join("history"));
        let outcome = capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &request(TargetKind::Screen),
            &|_| {},
            &mut |_| {},
        )
        .expect("captured");
        let Outcome::Captured { entry, clipboard } = outcome else {
            panic!("not captured");
        };
        assert!(clipboard.is_ok());
        let entry = entry.expect("a file");
        let folder = scratch.0.join("pictures").join("Capturas");
        assert_eq!(entry.path.parent(), Some(folder.as_path()));
        let names = files(&folder);
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].starts_with("Captura ") && names[0].ends_with(".png"));
        assert_eq!(std::fs::read(&entry.path).expect("png"), PIXEL_PNG);
        assert_eq!(entry.size, PIXEL_PNG.len() as u64);
        assert_eq!(history.entries(), std::slice::from_ref(&entry));
        let saved = History::load(scratch.0.join("history")).expect("saved");
        // The file keeps whole seconds.
        assert_eq!(saved.entries().len(), 1);
        assert_eq!(saved.entries()[0].path, entry.path);
        assert_eq!(saved.entries()[0].size, entry.size);
    }

    /// The delay counts down and nothing else waits: the window never steps
    /// aside, so there is no hide and no settle before the seconds.
    #[test]
    fn the_delay_counts_down_and_the_window_is_never_asked_to_hide() {
        let scratch = Scratch::new("delay");
        let mut fake = backend::make(true);
        let mut history = History::default();
        let slept = RefCell::new(Vec::new());
        let mut heard = Vec::new();
        let mut asked = request(TargetKind::Window);
        asked.delay = Duration::from_secs(3);
        asked.to_file = false;
        capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &asked,
            &|duration| slept.borrow_mut().push(duration),
            &mut |step| heard.push(step),
        )
        .expect("captured");
        assert_eq!(
            heard,
            [
                Progress::Countdown(3),
                Progress::Countdown(2),
                Progress::Countdown(1),
                Progress::Countdown(0)
            ]
        );
        let slept = slept.into_inner();
        assert_eq!(slept, [Duration::from_secs(1); 3]);
    }

    /// A screen capture with no delay waits for nothing at all, whatever
    /// output it takes, Selenita's own included.
    #[test]
    fn a_screen_capture_of_selenitas_own_output_waits_for_nothing() {
        let scratch = Scratch::new("own-output");
        let mut fake = backend::Fake::default();
        let mut heard = Vec::new();
        for output in ["", "FAKE-1", "OTHER-2"] {
            let mut asked = request(TargetKind::Screen);
            asked.output = output.to_owned();
            asked.to_file = false;
            capture(
                &mut fake,
                &mut History::default(),
                &scratch.places(),
                &asked,
                &|_| panic!("nothing waits"),
                &mut |step| heard.push(step),
            )
            .expect("captured");
        }
        assert!(heard.is_empty(), "{heard:?}");
        assert_eq!(fake.grabbed.len(), 3);
    }

    #[test]
    fn a_clipboard_only_capture_leaves_no_file_and_no_entry() {
        let scratch = Scratch::new("clipboard");
        let mut fake = backend::make(true);
        let mut history = History::default();
        let mut asked = request(TargetKind::Region);
        asked.to_file = false;
        let outcome = capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &asked,
            &|_| {},
            &mut |_| {},
        )
        .expect("captured");
        assert!(matches!(outcome, Outcome::Captured { entry: None, .. }));
        assert!(history.entries().is_empty());
        assert!(files(&scratch.0.join("run")).is_empty());
        assert!(!scratch.0.join("pictures").exists());
    }

    #[test]
    fn no_destination_is_refused_before_anything_runs() {
        let scratch = Scratch::new("nowhere");
        let mut fake = backend::make(true);
        let mut asked = request(TargetKind::Screen);
        asked.to_file = false;
        asked.to_clipboard = false;
        let result = capture(
            fake.as_mut(),
            &mut History::default(),
            &scratch.places(),
            &asked,
            &|_| panic!("nothing waits"),
            &mut |_| {},
        );
        assert!(matches!(result, Err(CaptureError::NoDestination)));
    }

    /// Cancels every region and takes nothing.
    struct Cancelling;

    impl Backend for Cancelling {
        fn outputs(&mut self) -> Result<Vec<Output>, CaptureError> {
            Ok(Vec::new())
        }
        fn windows(&mut self) -> Result<Vec<Window>, CaptureError> {
            Ok(Vec::new())
        }
        fn select_region(&mut self, _: &str, _: &str) -> Result<Option<Geometry>, CaptureError> {
            Ok(None)
        }
        fn grab(&mut self, _: &Target, _: &Path) -> Result<(), CaptureError> {
            panic!("a cancelled region takes nothing")
        }
        fn copy_png(&mut self, _: &[u8]) -> Result<(), CaptureError> {
            Ok(())
        }
        fn open_in_fluorita(&mut self, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
        fn edit_in_fluorita(&mut self, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
        fn show_in_siderita(&mut self, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
        fn trash(&mut self, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
        fn poster(&mut self, _: &Path, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
    }

    #[test]
    fn a_cancelled_region_is_not_an_error() {
        let scratch = Scratch::new("cancel");
        let result = capture(
            &mut Cancelling,
            &mut History::default(),
            &scratch.places(),
            &request(TargetKind::Region),
            &|_| {},
            &mut |_| {},
        );
        assert!(matches!(result, Ok(Outcome::Cancelled)));
    }

    #[test]
    fn a_window_capture_takes_the_window_focused_before_selenita() {
        let scratch = Scratch::new("window");
        let mut fake = backend::Fake::default();
        let mut asked = request(TargetKind::Window);
        asked.to_clipboard = false;
        asked.to_file = false;
        let outcome = capture(
            &mut fake,
            &mut History::default(),
            &scratch.places(),
            &asked,
            &|_| {},
            &mut |_| {},
        )
        .expect("niri's clipboard copy is a destination");
        assert!(matches!(outcome, Outcome::Captured { entry: None, .. }));
        assert!(matches!(
            fake.grabbed.as_slice(),
            [Target::Window { id: 2, .. }]
        ));
    }

    #[test]
    fn a_taken_name_is_numbered_never_replaced() {
        let scratch = Scratch::new("publish");
        let name = "Captura 2026-10-09 14.32.05.png";
        std::fs::write(scratch.0.join(name), b"first").expect("first");
        let hidden = scratch.0.join(".hidden.png");
        std::fs::write(&hidden, b"second").expect("second");
        let published = publish(&hidden, &scratch.0, name).expect("published");
        assert_eq!(
            published,
            scratch.0.join("Captura 2026-10-09 14.32.05 (2).png")
        );
        assert_eq!(std::fs::read(scratch.0.join(name)).expect("kept"), b"first");
        assert!(!hidden.exists());
    }

    #[test]
    fn deleting_an_entry_removes_its_file_and_line() {
        let scratch = Scratch::new("delete");
        let mut fake = backend::make(true);
        let mut history = History::at(scratch.0.join("history"));
        let outcome = capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &request(TargetKind::Screen),
            &|_| {},
            &mut |_| {},
        )
        .expect("captured");
        let Outcome::Captured {
            entry: Some(entry), ..
        } = outcome
        else {
            panic!("a file");
        };
        assert_eq!(
            act(fake.as_mut(), &mut history, Action::Copy, &entry.id()).ok(),
            Some(false)
        );
        assert!(act(fake.as_mut(), &mut history, Action::Delete, &entry.id()).expect("deleted"));
        assert!(!entry.path.exists());
        assert!(history.entries().is_empty());
        assert!(matches!(
            act(
                fake.as_mut(),
                &mut history,
                Action::OpenInFluorita,
                &entry.id()
            ),
            Err(CaptureError::UnknownEntry)
        ));
    }

    /// Trashing one entry rewrites the file from the live list: the other
    /// entries' lines stay, so a trash never empties the history.
    #[test]
    fn trashing_one_entry_keeps_the_others_lines_in_the_file() {
        let scratch = Scratch::new("trash-keeps");
        let mut fake = backend::make(true);
        let file = scratch.0.join("history");
        let mut history = History::at(file.clone());
        let Ok(Outcome::Captured {
            entry: Some(shot), ..
        }) = capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &request(TargetKind::Screen),
            &|_| {},
            &mut |_| {},
        )
        else {
            panic!("a file");
        };
        // A recording handed over by the recording worker (`Job::Push`).
        let clip = scratch.0.join("clip.mp4");
        std::fs::write(&clip, b"mp4").expect("clip");
        let recording = selenita_core::Entry {
            path: clip,
            kind: selenita_core::EntryKind::Recording,
            taken_at: std::time::SystemTime::now(),
            size: 3,
        };
        history.push(recording.clone());
        history.save().expect("saved");
        assert_eq!(History::load(file.clone()).expect("two").entries().len(), 2);
        assert!(
            act(fake.as_mut(), &mut history, Action::Delete, &recording.id()).expect("trashed")
        );
        let left = History::load(file.clone()).expect("one");
        assert_eq!(left.entries().len(), 1);
        assert_eq!(left.entries()[0].path, shot.path);
        assert!(!std::fs::read_to_string(&file).expect("text").is_empty());
        // The last one trashed leaves an empty file: that is the whole list.
        assert!(act(fake.as_mut(), &mut history, Action::Delete, &shot.id()).expect("trashed"));
        assert!(History::load(file).expect("none").entries().is_empty());
    }

    #[test]
    fn an_entry_whose_file_vanished_is_forgotten_when_used() {
        let scratch = Scratch::new("vanished");
        let mut fake = backend::make(true);
        let mut history = History::at(scratch.0.join("history"));
        let Ok(Outcome::Captured {
            entry: Some(entry), ..
        }) = capture(
            fake.as_mut(),
            &mut history,
            &scratch.places(),
            &request(TargetKind::Screen),
            &|_| {},
            &mut |_| {},
        )
        else {
            panic!("a file");
        };
        std::fs::remove_file(&entry.path).expect("vanish");
        assert!(matches!(
            act(
                fake.as_mut(),
                &mut history,
                Action::ShowInSiderita,
                &entry.id()
            ),
            Err(CaptureError::UnknownEntry)
        ));
        assert!(history.entries().is_empty());
    }

    #[test]
    fn a_recording_is_not_copied_to_the_clipboard() {
        let scratch = Scratch::new("recording-copy");
        let mut fake = backend::make(true);
        let mut history = History::at(scratch.0.join("history"));
        let path = scratch.0.join("clip.mp4");
        std::fs::write(&path, b"mp4").expect("clip");
        let entry = selenita_core::Entry {
            path,
            kind: selenita_core::EntryKind::Recording,
            taken_at: std::time::SystemTime::now(),
            size: 3,
        };
        history.push(entry.clone());
        assert!(matches!(
            act(fake.as_mut(), &mut history, Action::Copy, &entry.id()),
            Err(CaptureError::NotAPicture)
        ));
        assert_eq!(
            act(
                fake.as_mut(),
                &mut history,
                Action::OpenInFluorita,
                &entry.id()
            )
            .ok(),
            Some(false)
        );
    }

    /// An edited copy beside a capture, or a trimmed recording, joins the
    /// history once, in the scratch home's folders; anything else does not.
    #[test]
    fn an_adopted_file_joins_the_history_once() {
        let scratch = Scratch::new("adopt");
        let captures = scratch.0.join("pictures").join("Capturas");
        let recordings = scratch.0.join("videos").join("Recordings");
        std::fs::create_dir_all(&captures).expect("captures");
        std::fs::create_dir_all(&recordings).expect("recordings");
        let folders = selenita_core::history::Folders {
            captures: &captures,
            recordings: &recordings,
        };
        let file = scratch.0.join("history");
        let mut history = History::at(file.clone());
        let now = std::time::SystemTime::now();
        let copy = captures.join("Captura 1 (2).png");
        std::fs::write(&copy, PIXEL_PNG).expect("copy");
        let key = celestina_core::pathkey::encode(&copy);
        assert!(adopt(&mut history, folders, &key, now));
        assert!(adopt(&mut history, folders, &key, now));
        assert_eq!(history.entries().len(), 1, "once");
        assert_eq!(history.entries()[0].path, copy);
        assert_eq!(
            History::load(file.clone()).expect("saved").entries().len(),
            1
        );

        let clip = recordings.join("Clip 1 (2).mp4");
        std::fs::write(&clip, b"mp4").expect("clip");
        assert!(adopt(
            &mut history,
            folders,
            &celestina_core::pathkey::encode(&clip),
            now
        ));
        assert_eq!(
            history.entries()[0].kind,
            selenita_core::EntryKind::Recording
        );

        let elsewhere = scratch.0.join("elsewhere.png");
        std::fs::write(&elsewhere, PIXEL_PNG).expect("elsewhere");
        assert!(!adopt(
            &mut history,
            folders,
            &celestina_core::pathkey::encode(&elsewhere),
            now
        ));
        assert!(!adopt(&mut history, folders, "not a key", now));
        assert_eq!(History::load(file).expect("saved").entries().len(), 2);
    }

    #[test]
    fn a_vanished_file_is_not_handed_to_fluorita() {
        let scratch = Scratch::new("edit");
        let mut fake = backend::make(true);
        let shot = scratch.0.join("Captura 1.png");
        std::fs::write(&shot, PIXEL_PNG).expect("shot");
        let key = celestina_core::pathkey::encode(&shot);
        assert!(edit(fake.as_mut(), &key).is_ok());
        std::fs::remove_file(&shot).expect("vanish");
        assert!(matches!(
            edit(fake.as_mut(), &key),
            Err(CaptureError::UnknownEntry)
        ));
        assert!(matches!(
            edit(fake.as_mut(), "not a key"),
            Err(CaptureError::UnknownEntry)
        ));
    }

    #[test]
    fn every_error_has_spanish_words() {
        for error in [
            CaptureError::NoDestination,
            CaptureError::Busy,
            CaptureError::Edit("UnknownMethod".to_owned()),
            CaptureError::Tool(selenita_core::runner::RunError::Missing(
                "/stubs/grim".to_owned(),
            )),
        ] {
            assert!(!error.message_es().is_empty());
        }
        assert!(CaptureError::Tool(selenita_core::runner::RunError::Missing(
            "/stubs/grim".to_owned()
        ))
        .message_es()
        .contains("«grim»"));
    }
}
