// language-contract: product-copy
//! The capture worker: one named thread that owns the backend and the
//! history and runs every capture and every history action in order, so the
//! Qt thread never waits on a tool, a socket or a file.
//!
//! A capture runs its plan here: note the window a window capture means
//! (the latest focused one that is not Selenita) while Selenita is still on
//! screen, ask the window to step aside and, when the target may show the
//! output it was on, wait for it to go; count the delay down (one `Countdown` a second), resolve the target
//! (niri's outputs or focused window, or `slurp` for a region), take the
//! picture into a hidden file, publish it under its name in the captures
//! folder without replacing anything, put it on the clipboard when asked,
//! push it to the history and report it. Every failure is a [`CaptureError`]
//! whose `message_es` the window shows in its notice.
//!
//! The thread is detached and never joined, as Cuprita's workers: a tool
//! still running when the window closes is abandoned with the process.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, SystemTime};

use celestina_core::activation::SELENITA;
use celestina_core::{atomic_file, xdg};
use selenita_core::history::HistoryError;
use selenita_core::niri::{self, NiriError};
use selenita_core::runner::RunError;
use selenita_core::target::needs_settle;
use selenita_core::{capture_file_name, names, pictures_dir};
use selenita_core::{Entry, EntryKind, History, Target, TargetKind};

use crate::backend::Backend;

/// How long the compositor gets to take the hidden window off screen before
/// the picture is taken: an estimate VAL-SEL-SHOT confirms.
const HIDE_SETTLE: Duration = Duration::from_millis(350);

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
            Self::Write(detail) | Self::Open(detail) | Self::Show(detail) | Self::Trash(detail) => {
                write!(formatter, "{self:?}: {detail}")
            }
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
    /// The window is on screen and steps aside for this capture.
    pub hide: bool,
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
    /// Step aside now.
    Hide,
    /// Seconds left; the last is 0.
    Countdown(u32),
}

/// Runs one capture's plan. `sleep` waits (a test passes one that does not),
/// `progress` hears the hide and the seconds left.
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
    // Read while Selenita is still on screen and focused, so the window meant
    // is the one focused before it.
    let windows = if window_kind {
        backend.windows()?
    } else if request.hide {
        backend.windows().unwrap_or_default()
    } else {
        Vec::new()
    };
    let meant = if window_kind {
        Some(
            niri::pick_window(&windows, SELENITA.0)
                .cloned()
                .ok_or(CaptureError::Niri(NiriError::NoFocusedWindow))?,
        )
    } else {
        None
    };
    if request.hide {
        progress(Progress::Hide);
        let own = windows.iter().find(|window| window.app_id == SELENITA.0);
        let workspaces = if own.is_some() {
            backend.workspaces().unwrap_or_default()
        } else {
            Vec::new()
        };
        let own_output = own.and_then(|window| niri::output_of(window, &workspaces));
        let target_output = match request.kind {
            TargetKind::Screen => Some(request.output.as_str()),
            TargetKind::Window => meant
                .as_ref()
                .and_then(|window| niri::output_of(window, &workspaces)),
            TargetKind::Region => None,
        };
        if needs_settle(request.kind, target_output, own_output) {
            sleep(HIDE_SETTLE);
        }
    }
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
    /// The window steps aside now.
    HideWindow,
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
                                        Progress::Hide => Report::HideWindow,
                                        Progress::Countdown(left) => Report::Countdown(left),
                                    });
                                },
                            );
                            finish(outcome, &history, &report);
                        }
                        Job::Push(entry) => {
                            history.push(entry);
                            if let Err(error) = history.save() {
                                eprintln!("selenita: {error}");
                                report(Report::Failed(CaptureError::History(error).message_es()));
                            }
                            report(Report::History(history.entries().to_vec()));
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
    use super::{act, capture, publish, Action, CaptureError, Outcome, Places, Progress, Request};
    use crate::backend::{self, Backend, PIXEL_PNG};
    use selenita_core::niri::{Output, Window, Workspace};
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
            hide: false,
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

    #[test]
    fn the_delay_counts_down_after_the_window_settles() {
        let scratch = Scratch::new("delay");
        let mut fake = backend::make(true);
        let mut history = History::default();
        let slept = RefCell::new(Vec::new());
        let mut heard = Vec::new();
        let mut asked = request(TargetKind::Window);
        asked.delay = Duration::from_secs(3);
        asked.hide = true;
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
                Progress::Hide,
                Progress::Countdown(3),
                Progress::Countdown(2),
                Progress::Countdown(1),
                Progress::Countdown(0)
            ]
        );
        let slept = slept.into_inner();
        assert_eq!(slept.len(), 4);
        assert!(slept[0] < Duration::from_secs(1), "the settle comes first");
        assert!(slept[1..].iter().all(|d| *d == Duration::from_secs(1)));
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
        fn workspaces(&mut self) -> Result<Vec<Workspace>, CaptureError> {
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
        fn show_in_siderita(&mut self, _: &Path) -> Result<(), CaptureError> {
            Ok(())
        }
        fn trash(&mut self, _: &Path) -> Result<(), CaptureError> {
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
    fn the_settle_is_skipped_for_another_output() {
        let scratch = Scratch::new("settle");
        let mut fake = backend::Fake::default();
        let mut asked = request(TargetKind::Screen);
        asked.hide = true;
        asked.output = "OTHER-2".to_owned();
        let slept = RefCell::new(Vec::new());
        let mut heard = Vec::new();
        capture(
            &mut fake,
            &mut History::default(),
            &scratch.places(),
            &asked,
            &|duration| slept.borrow_mut().push(duration),
            &mut |step| heard.push(step),
        )
        .expect("captured");
        assert_eq!(heard, [Progress::Hide]);
        assert!(slept.into_inner().is_empty());
        asked.output = "FAKE-1".to_owned();
        let slept = RefCell::new(Vec::new());
        capture(
            &mut fake,
            &mut History::default(),
            &scratch.places(),
            &asked,
            &|duration| slept.borrow_mut().push(duration),
            &mut |_| {},
        )
        .expect("captured");
        assert_eq!(slept.into_inner().len(), 1, "Selenita's own output waits");
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

    #[test]
    fn every_error_has_spanish_words() {
        for error in [
            CaptureError::NoDestination,
            CaptureError::Busy,
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
