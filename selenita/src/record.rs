// language-contract: product-copy
//! The recording worker: one named thread that owns the recorder (the portal
//! and `gst-launch-1.0`, or the fake under `SELENITA_FAKE=1`) and walks the
//! crate's state machine, so the Qt thread never waits on the portal, the
//! child or a file.
//!
//! A recording runs here: the portal is asked for a monitor (the person
//! chooses in its dialog), the pipeline is started into a hidden file in
//! the videos folder's `Recordings`, the child is watched (an early exit is a failure)
//! together with the stop file `selenita --stop` touches when no instance
//! answers on the bus and with the hidden file's size (a source that never
//! streams leaves the muxer nothing to write: past [`SIGNAL_DEADLINE`] with
//! an empty file the recording fails with a notice instead of hanging the
//! stop, SEL-1-E), and the stop interrupts the child, waits for the
//! muxer to finish, publishes the file under its name without replacing
//! anything and hands the entry to the capture worker, which owns the
//! history. A child that ends on its own with status 0 (the source sent
//! EOS) has its file published too; any other early exit is a failure. At
//! start the recorder is probed for the muxer, so a missing
//! `gst-plugins-good` is said before anything is recorded.
//!
//! When the window quits ([`quit`], after the event loop ends) a recording
//! under way is stopped the same way and its entry written to the history
//! file directly, the Qt thread being gone; a portal dialog still open is
//! closed. The wait is bounded by [`STOP_DEADLINE`]; past it the child is
//! killed and its hidden file removed.

use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use celestina_core::xdg;
use selenita_core::names::recordings_dir_in;
use selenita_core::record::{
    self, inspect_argv, take_stop_request, Event, Pipeline, Source, State, VideoEncoder, INSPECTOR,
    LAUNCHER, MUXER,
};
use selenita_core::runner::{self, Exit, RunError, Running};
use selenita_core::{capture_file_name, tools, videos_dir, Entry, EntryKind, History};

use crate::capture::publish;
use crate::portal::{self, PortalError};

/// How long the muxer gets to finish the file after the interrupt, and how
/// long the quitting window waits for the worker.
pub const STOP_DEADLINE: Duration = Duration::from_secs(10);
/// How long `gst-inspect-1.0` may take per element.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);
/// How long after the child started the hidden file may stay empty: the
/// muxer writes its header with the first frames, so an empty file past this
/// means the portal's node never streamed (measured on 2026-10-10: a node
/// that exists but sends nothing holds `gst-launch-1.0 -e` past any stop,
/// and the kill at [`STOP_DEADLINE`] lost the recording in silence).
pub const SIGNAL_DEADLINE: Duration = Duration::from_secs(8);
/// How often the child and the stop file are looked at while recording.
const POLL: Duration = Duration::from_millis(250);
/// A render node VA-API can use.
const RENDER_NODE: &str = "/dev/dri/renderD128";

/// Why a recording failed.
#[derive(Debug)]
pub enum RecordError {
    Portal(PortalError),
    Tool(RunError),
    /// The child exited before the stop: its exit and last stderr line.
    Exited(Exit),
    /// The child ended and left no file.
    NoFile,
    /// The child ran but the portal's node sent no frame within
    /// [`SIGNAL_DEADLINE`]: nothing was ever written.
    NoSignal,
    Write(String),
    /// `mp4mux` (or the launcher) is not installed.
    Missing(String),
    /// A recording is already under way.
    Busy,
    /// Nothing is recording.
    NotRecording,
    /// The worker thread is gone.
    WorkerGone,
}

impl RecordError {
    /// What the window says, in Spanish.
    #[must_use]
    pub fn message_es(&self) -> String {
        match self {
            Self::Portal(PortalError::Cancelled) => "Grabación cancelada.".to_owned(),
            Self::Portal(PortalError::Timeout(_)) => {
                "El portal de pantalla no ha respondido a tiempo.".to_owned()
            }
            Self::Portal(_) => "El portal de pantalla no ha dado ninguna pantalla.".to_owned(),
            Self::Tool(RunError::Missing(program)) | Self::Missing(program) => {
                let name = Path::new(program).file_name().map_or_else(
                    || program.clone(),
                    |name| name.to_string_lossy().into_owned(),
                );
                format!("Falta «{name}» (gst-plugins-good): instálalo para grabar.")
            }
            Self::Tool(_) => "No se ha podido iniciar la grabación.".to_owned(),
            Self::Exited(_) => "La grabación se ha interrumpido.".to_owned(),
            Self::NoFile => "La grabación no ha producido ningún archivo.".to_owned(),
            Self::NoSignal => {
                "El portal no ha enviado ninguna imagen: la grabación se ha cancelado.".to_owned()
            }
            Self::Write(_) => "No se ha podido guardar la grabación.".to_owned(),
            Self::Busy => "Ya hay una grabación en curso.".to_owned(),
            Self::NotRecording => "No hay ninguna grabación en curso.".to_owned(),
            Self::WorkerGone => "La grabación no está disponible.".to_owned(),
        }
    }
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Portal(error) => write!(formatter, "{error}"),
            Self::Tool(error) => write!(formatter, "{error}"),
            Self::Exited(exit) => write!(formatter, "exited {:?}: {}", exit.code, exit.stderr),
            Self::Write(detail) | Self::Missing(detail) => write!(formatter, "{self:?}: {detail}"),
            other => write!(formatter, "{other:?}"),
        }
    }
}

/// What the recorder found installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Probe {
    /// `gst-inspect-1.0`, which the other answers come from.
    pub inspector: bool,
    pub launcher: bool,
    pub muxer: bool,
    /// A VA-API render node exists.
    pub render_node: bool,
    /// `vah264enc` is installed.
    pub va_element: bool,
}

impl Probe {
    /// The first thing a recording needs and the host lacks.
    #[must_use]
    pub fn missing(self) -> Option<&'static str> {
        if !self.inspector {
            Some(INSPECTOR)
        } else if !self.launcher {
            Some(LAUNCHER)
        } else if !self.muxer {
            Some(MUXER)
        } else {
            None
        }
    }
}

/// The source the recorder prepared, and whatever keeps it open.
pub struct Prepared {
    pub source: Source,
    pub fd: Option<OwnedFd>,
}

/// The session as the recording worker sees it. Every method blocks.
pub trait Recorder: Send {
    /// What is installed; run once at start.
    fn probe(&mut self) -> Probe;
    /// Asks for the monitor (the person may be choosing).
    fn prepare(&mut self) -> Result<Prepared, RecordError>;
    /// Starts the pipeline into `pipeline.out`, handing `fd` to the child.
    fn start(&mut self, pipeline: &Pipeline, fd: Option<OwnedFd>) -> Result<(), RecordError>;
    /// `Some` once the child exited on its own.
    fn exited(&mut self) -> Option<Exit>;
    /// Tells the child to finish the file and waits for it; the session is
    /// released either way.
    fn stop(&mut self) -> Result<(), RecordError>;
    /// Kills whatever runs and releases the session.
    fn abandon(&mut self);
}

/// The recorder `fake` asks for.
pub fn make(fake: bool) -> Box<dyn Recorder> {
    if fake {
        Box::new(Fake::default())
    } else {
        Box::new(Real {
            tools_dir: std::env::var_os("SELENITA_TOOLS_DIR")
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
            session: None,
            child: None,
        })
    }
}

struct Real {
    tools_dir: Option<PathBuf>,
    session: Option<portal::Session>,
    child: Option<Running>,
}

impl Real {
    fn has_element(&self, element: &str) -> bool {
        let argv = tools::resolve(inspect_argv(element), self.tools_dir.as_deref());
        runner::run(&argv, None, PROBE_DEADLINE).is_ok()
    }

    fn release(&mut self) {
        // Closing the session tells the compositor to stop casting.
        self.session = None;
    }
}

impl Recorder for Real {
    fn probe(&mut self) -> Probe {
        let installed = |argv: Vec<std::ffi::OsString>| {
            let argv = tools::resolve(argv, self.tools_dir.as_deref());
            !matches!(
                runner::run(&argv, None, PROBE_DEADLINE),
                Err(RunError::Missing(_))
            )
        };
        let inspector = installed(inspect_argv(MUXER));
        let launcher = installed(vec![LAUNCHER.into(), "--version".into()]);
        Probe {
            inspector,
            launcher,
            muxer: inspector && self.has_element(MUXER),
            render_node: Path::new(RENDER_NODE).exists(),
            va_element: inspector && self.has_element(VideoEncoder::VaH264.element()),
        }
    }

    fn prepare(&mut self) -> Result<Prepared, RecordError> {
        let (session, stream) = portal::open_monitor().map_err(RecordError::Portal)?;
        self.session = Some(session);
        // The one line a live diagnosis needs: the node the `Start` response
        // named and whether the remote came with it.
        eprintln!(
            "selenita: recording: portal stream node {}, remote fd {}",
            stream.node,
            if stream.fd.is_some() { "yes" } else { "no" }
        );
        let fd_number = stream.fd.as_ref().map(|_| 0);
        Ok(Prepared {
            source: Source {
                node: stream.node,
                fd: fd_number,
            },
            fd: stream.fd,
        })
    }

    fn start(&mut self, pipeline: &Pipeline, fd: Option<OwnedFd>) -> Result<(), RecordError> {
        let argv = tools::resolve(pipeline.launch_argv(), self.tools_dir.as_deref());
        match runner::start(&argv, fd) {
            Ok(child) => {
                self.child = Some(child);
                Ok(())
            }
            Err(error) => {
                self.release();
                Err(RecordError::Tool(error))
            }
        }
    }

    fn exited(&mut self) -> Option<Exit> {
        let exit = self.child.as_mut()?.poll()?;
        self.child = None;
        self.release();
        Some(exit)
    }

    fn stop(&mut self) -> Result<(), RecordError> {
        let Some(mut child) = self.child.take() else {
            self.release();
            return Err(RecordError::NotRecording);
        };
        let interrupted = child.interrupt();
        let (on_its_own, exit) = child.wait_until(STOP_DEADLINE);
        self.release();
        interrupted.map_err(RecordError::Tool)?;
        if on_its_own && exit.code == Some(0) {
            Ok(())
        } else {
            Err(RecordError::Exited(exit))
        }
    }

    fn abandon(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
        self.release();
    }
}

/// `SELENITA_FAKE=1`: no portal, no child. A start writes a few bytes to the
/// file and the stop a few more, so the recording lands in the history like
/// a real one; nothing leaves the process but a line on stderr.
#[derive(Default)]
pub struct Fake {
    out: Option<PathBuf>,
    pub started: Vec<Pipeline>,
    /// Every call, in order: `prepare`, `start`, `stop`, `abandon`.
    pub calls: Vec<&'static str>,
}

/// The node the fake portal answers with.
pub const FAKE_NODE: u32 = 1;

impl Recorder for Fake {
    fn probe(&mut self) -> Probe {
        Probe {
            inspector: true,
            launcher: true,
            muxer: true,
            render_node: false,
            va_element: false,
        }
    }

    fn prepare(&mut self) -> Result<Prepared, RecordError> {
        self.calls.push("prepare");
        Ok(Prepared {
            source: Source {
                node: FAKE_NODE,
                fd: None,
            },
            fd: None,
        })
    }

    fn start(&mut self, pipeline: &Pipeline, _fd: Option<OwnedFd>) -> Result<(), RecordError> {
        self.calls.push("start");
        eprintln!("selenita-fake: record to {}", pipeline.out.display());
        std::fs::write(&pipeline.out, b"fake mp4 ")
            .map_err(|error| RecordError::Write(error.to_string()))?;
        self.out = Some(pipeline.out.clone());
        self.started.push(pipeline.clone());
        Ok(())
    }

    fn exited(&mut self) -> Option<Exit> {
        None
    }

    fn stop(&mut self) -> Result<(), RecordError> {
        self.calls.push("stop");
        let out = self.out.take().ok_or(RecordError::NotRecording)?;
        let mut bytes =
            std::fs::read(&out).map_err(|error| RecordError::Write(error.to_string()))?;
        bytes.extend_from_slice(b"finished");
        std::fs::write(&out, bytes).map_err(|error| RecordError::Write(error.to_string()))
    }

    fn abandon(&mut self) {
        self.calls.push("abandon");
        self.out = None;
    }
}

/// One recording as the controller asked for it.
#[derive(Clone, Debug)]
pub struct Request {
    /// The system's sound too.
    pub audio: bool,
    /// Product copy from the Qt seam: the file name's stem (the recording word).
    pub stem: String,
}

/// What the worker reports to the Qt thread.
#[derive(Debug)]
pub enum Report {
    /// What the host lacks for recording, if anything.
    Missing(Option<&'static str>),
    /// The recording moved to this state.
    State(State),
    /// The child runs since this moment.
    Started(SystemTime),
    /// The file is published: its entry, for the history.
    Finished(Entry),
    /// The recording failed, in Spanish.
    Failed(String),
}

enum Job {
    Toggle(Request),
    Stop,
    /// The window quits: finish what records, then end; the sender hears
    /// when it is done.
    Quit(Sender<()>),
}

/// Where a recording's files go and where the stop file is.
pub struct Places {
    /// The recordings folder inside the videos folder; made at start.
    pub recordings: PathBuf,
    pub stop_file: PathBuf,
}

impl Places {
    /// `Recordings` in the videos folder (else in the home), and the runtime
    /// folder's stop file. Blocking.
    fn resolve() -> Self {
        let videos = videos_dir()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(std::env::temp_dir);
        let runtime = xdg::runtime_dir().unwrap_or_else(|_| std::env::temp_dir());
        Self {
            recordings: recordings_dir_in(&videos),
            stop_file: record::stop_file(&runtime),
        }
    }
}

static NEXT_HIDDEN: AtomicU64 = AtomicU64::new(1);

fn hidden_file(folder: &Path) -> PathBuf {
    folder.join(format!(
        ".selenita-{}-{}.mp4",
        std::process::id(),
        NEXT_HIDDEN.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A recording under way.
struct Active {
    hidden: PathBuf,
    started: SystemTime,
    stem: String,
}

/// The worker's state between jobs: the machine's state and the recording
/// it is about, with the choices made at start.
pub struct Session {
    state: State,
    active: Option<Active>,
    encoder: VideoEncoder,
    /// How long the hidden file may stay empty after the start.
    signal_deadline: Duration,
}

impl Session {
    /// Idle, with the encoder `probe` allows.
    #[must_use]
    pub fn new(probe: Probe) -> Self {
        Self {
            state: State::Idle,
            active: None,
            encoder: VideoEncoder::choose(probe.render_node, probe.va_element),
            signal_deadline: SIGNAL_DEADLINE,
        }
    }

    /// The same session with another [`SIGNAL_DEADLINE`] (the tests use
    /// zero).
    #[cfg(test)]
    #[must_use]
    pub fn with_signal_deadline(mut self, deadline: Duration) -> Self {
        self.signal_deadline = deadline;
        self
    }

    /// While it records: whether the child has written anything yet. Past
    /// the deadline with an empty file the portal's node never streamed, so
    /// the child is killed and the recording fails with [`RecordError::
    /// NoSignal`]; `None` otherwise (nothing to say, or too early to tell).
    pub fn watch_signal(
        &mut self,
        recorder: &mut dyn Recorder,
        report: &mut dyn FnMut(Report),
    ) -> Option<RecordError> {
        let active = self.active.as_ref()?;
        if self.state != State::Recording {
            return None;
        }
        let waited = SystemTime::now()
            .duration_since(active.started)
            .unwrap_or_default();
        if waited < self.signal_deadline {
            return None;
        }
        let written = std::fs::metadata(&active.hidden)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if written > 0 {
            return None;
        }
        recorder.abandon();
        self.fail(report);
        Some(RecordError::NoSignal)
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    fn step(&mut self, event: Event, report: &mut dyn FnMut(Report)) {
        match self.state.next(event) {
            Ok(state) => {
                self.state = state;
                report(Report::State(state));
            }
            Err(error) => eprintln!("selenita: recording: {error}"),
        }
    }

    /// Starts a recording: the portal, then the child into a hidden file.
    ///
    /// # Errors
    ///
    /// [`RecordError`] for a recording already under way, the portal or the
    /// start; the session is idle again then.
    pub fn start(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        request: &Request,
        report: &mut dyn FnMut(Report),
    ) -> Result<(), RecordError> {
        if self.state.is_busy() {
            return Err(RecordError::Busy);
        }
        self.step(Event::Start, report);
        // A stale stop request from before this recording must not end it.
        let _ = take_stop_request(&places.stop_file);
        match self.prepare_and_start(recorder, places, request) {
            Ok(()) => {
                // The start comes before the state: the card's clock reads
                // the new origin the moment it sees `Recording`, never the
                // previous recording's.
                if let Some(active) = &self.active {
                    report(Report::Started(active.started));
                }
                self.step(Event::Prepared, report);
                Ok(())
            }
            Err(error) => {
                recorder.abandon();
                self.fail(report);
                Err(error)
            }
        }
    }

    fn prepare_and_start(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        request: &Request,
    ) -> Result<(), RecordError> {
        let prepared = recorder.prepare()?;
        std::fs::create_dir_all(&places.recordings)
            .map_err(|error| RecordError::Write(error.to_string()))?;
        let hidden = hidden_file(&places.recordings);
        let pipeline = Pipeline {
            source: prepared.source,
            encoder: self.encoder,
            audio: request.audio,
            out: hidden.clone(),
        };
        if let Err(error) = recorder.start(&pipeline, prepared.fd) {
            let _ = std::fs::remove_file(&hidden);
            return Err(error);
        }
        self.active = Some(Active {
            hidden,
            started: SystemTime::now(),
            stem: request.stem.clone(),
        });
        Ok(())
    }

    /// Stops the recording and publishes its file; the entry for the
    /// history.
    ///
    /// # Errors
    ///
    /// [`RecordError`] when nothing records, the child did not finish
    /// cleanly or the file could not be published; the hidden file is
    /// removed then.
    pub fn stop(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Result<Entry, RecordError> {
        if self.state != State::Recording {
            return Err(RecordError::NotRecording);
        }
        self.step(Event::Stop, report);
        let stopped = recorder.stop();
        let result = stopped.and_then(|()| self.publish(places));
        match &result {
            Ok(entry) => {
                self.step(Event::Stopped, report);
                report(Report::Finished(entry.clone()));
            }
            Err(_) => self.fail(report),
        }
        result
    }

    /// The child ended on its own. With status 0 the source sent EOS and
    /// the muxer finished the file, which is published; anything else is a
    /// failure and the file goes.
    ///
    /// # Errors
    ///
    /// [`RecordError::Exited`] for an unsuccessful exit, or the publish's.
    pub fn ended(
        &mut self,
        exit: Exit,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Result<Entry, RecordError> {
        if exit.code != Some(0) {
            self.fail(report);
            return Err(RecordError::Exited(exit));
        }
        self.step(Event::Stop, report);
        let result = self.publish(places);
        match &result {
            Ok(entry) => {
                self.step(Event::Stopped, report);
                report(Report::Finished(entry.clone()));
            }
            Err(_) => self.fail(report),
        }
        result
    }

    /// The window quits: a recording under way is stopped and published;
    /// whatever else runs is killed and its file removed.
    pub fn quit(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Option<Entry> {
        let entry = if self.state == State::Recording {
            match self.stop(recorder, places, report) {
                Ok(entry) => Some(entry),
                Err(error) => {
                    eprintln!("selenita: recording at quit: {error}");
                    None
                }
            }
        } else {
            None
        };
        self.abandon(recorder, report);
        entry
    }

    /// Kills whatever runs, removes its file and returns to idle.
    fn abandon(&mut self, recorder: &mut dyn Recorder, report: &mut dyn FnMut(Report)) {
        recorder.abandon();
        if self.state.is_busy() {
            self.fail(report);
        } else if let Some(active) = self.active.take() {
            let _ = std::fs::remove_file(active.hidden);
        }
    }

    fn publish(&mut self, places: &Places) -> Result<Entry, RecordError> {
        let active = self.active.take().ok_or(RecordError::NotRecording)?;
        let size = std::fs::metadata(&active.hidden)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if size == 0 {
            let _ = std::fs::remove_file(&active.hidden);
            return Err(RecordError::NoFile);
        }
        let name = capture_file_name(&active.stem, active.started, "mp4");
        let path = publish(&active.hidden, &places.recordings, &name).map_err(|error| {
            let _ = std::fs::remove_file(&active.hidden);
            RecordError::Write(error.to_string())
        })?;
        Ok(Entry {
            path,
            kind: EntryKind::Recording,
            taken_at: active.started,
            size,
        })
    }

    /// Whatever failed: back to idle, the hidden file gone.
    fn fail(&mut self, report: &mut dyn FnMut(Report)) {
        if let Some(active) = self.active.take() {
            let _ = std::fs::remove_file(active.hidden);
        }
        self.step(Event::Failure, report);
        self.step(Event::Acknowledged, report);
    }
}

/// The entry joins the history file directly: at quit the Qt thread and the
/// capture worker's queue are no longer there to carry it.
fn record_in_history(file: Option<PathBuf>, entry: Entry) {
    let Some(file) = file else {
        return;
    };
    let mut history = History::load(file.clone()).unwrap_or_else(|error| {
        eprintln!("selenita: {error}");
        History::at(file)
    });
    history.push(entry);
    if let Err(error) = history.save() {
        eprintln!("selenita: {error}");
    }
}

/// The sending half of the recording worker.
pub struct Worker {
    jobs: Sender<Job>,
}

/// The worker's inbox for [`quit`], set when it starts.
static QUIT: OnceLock<Sender<Job>> = OnceLock::new();

/// The window quit: asks the worker to finish a recording under way (or
/// closes the portal dialog it waits on) and waits up to `deadline` for it.
/// Blocking; called once the event loop has ended.
pub fn quit(deadline: Duration) {
    let Some(jobs) = QUIT.get() else {
        return;
    };
    let (done, finished) = mpsc::channel();
    if jobs.send(Job::Quit(done)).is_err() {
        return;
    }
    crate::portal::cancel_pending();
    if finished.recv_timeout(deadline).is_err() {
        eprintln!("selenita: the recording did not finish before the quit deadline");
    }
}

impl Worker {
    /// Starts the thread: it builds the recorder, probes it and reports what
    /// is missing, then waits for jobs, watching the child and the stop file
    /// while it records. `report` runs on the worker and must queue to the
    /// Qt thread itself.
    pub fn start(fake: bool, report: impl Fn(Report) + Send + 'static) -> Option<Self> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let _ = QUIT.set(jobs.clone());
        std::thread::Builder::new()
            .name("selenita-record".to_owned())
            .spawn(move || {
                let mut recorder = make(fake);
                let probe = recorder.probe();
                report(Report::Missing(probe.missing()));
                let mut session = Session::new(probe);
                let mut tell = |what: Report| report(what);
                // Re-read when a recording starts, so a folder chosen since
                // is honoured; the stop file is looked at between polls.
                let mut places = Places::resolve();
                loop {
                    let job = if session.state() == State::Recording {
                        match inbox.recv_timeout(POLL) {
                            Ok(job) => Some(job),
                            Err(RecvTimeoutError::Timeout) => None,
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match inbox.recv() {
                            Ok(job) => Some(job),
                            Err(_) => break,
                        }
                    };
                    if session.state() == State::Recording {
                        if let Some(exit) = recorder.exited() {
                            if let Err(error) = session.ended(exit, &places, &mut tell) {
                                eprintln!("selenita: recording: {error}");
                                tell(Report::Failed(error.message_es()));
                            }
                            continue;
                        }
                        if let Some(error) = session.watch_signal(recorder.as_mut(), &mut tell) {
                            eprintln!("selenita: recording: {error}");
                            tell(Report::Failed(error.message_es()));
                            continue;
                        }
                    }
                    let job = match job {
                        Some(job) => job,
                        None if take_stop_request(&places.stop_file) => Job::Stop,
                        None => continue,
                    };
                    let outcome = match job {
                        Job::Toggle(_) if session.state() == State::Recording => session
                            .stop(recorder.as_mut(), &places, &mut tell)
                            .map(|_| ()),
                        Job::Toggle(request) => match probe.missing() {
                            Some(element) => Err(RecordError::Missing(element.to_owned())),
                            None => {
                                places = Places::resolve();
                                session.start(recorder.as_mut(), &places, &request, &mut tell)
                            }
                        },
                        // A stop with nothing recording (`selenita --stop`
                        // pressed twice) is not worth a notice.
                        Job::Stop if session.state() != State::Recording => Ok(()),
                        Job::Stop => session
                            .stop(recorder.as_mut(), &places, &mut tell)
                            .map(|_| ()),
                        Job::Quit(done) => {
                            if let Some(entry) = session.quit(recorder.as_mut(), &places, &mut tell)
                            {
                                record_in_history(History::default_file(), entry);
                            }
                            let _ = done.send(());
                            break;
                        }
                    };
                    if let Err(error) = outcome {
                        eprintln!("selenita: recording: {error}");
                        tell(Report::Failed(error.message_es()));
                    }
                }
                session.quit(recorder.as_mut(), &places, &mut tell);
            })
            .ok()
            .map(|_| Self { jobs })
    }

    /// Starts a recording, or stops the one under way.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn toggle(&self, request: Request) -> Result<(), RecordError> {
        self.jobs
            .send(Job::Toggle(request))
            .map_err(|_| RecordError::WorkerGone)
    }

    /// Stops the recording under way; nothing happens when none is.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn stop(&self) -> Result<(), RecordError> {
        self.jobs
            .send(Job::Stop)
            .map_err(|_| RecordError::WorkerGone)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        make, Fake, Places, Probe, RecordError, Recorder, Report, Request, Session, FAKE_NODE,
    };
    use selenita_core::names::recordings_dir_in;
    use selenita_core::record::{Pipeline, State};
    use selenita_core::runner::Exit;
    use selenita_core::EntryKind;
    use std::os::fd::OwnedFd;
    use std::path::PathBuf;
    use std::time::Duration;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("selenita-record-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }

        fn places(&self) -> Places {
            Places {
                recordings: recordings_dir_in(&self.0.join("videos")),
                stop_file: self.0.join("run").join("selenita").join("stop"),
            }
        }

        /// The recordings folder, `videos/Recordings`.
        fn recordings(&self) -> PathBuf {
            self.0.join("videos").join("Recordings")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request(audio: bool) -> Request {
        Request {
            audio,
            stem: "Recording".to_owned(),
        }
    }

    fn states(reports: &[Report]) -> Vec<State> {
        reports
            .iter()
            .filter_map(|report| match report {
                Report::State(state) => Some(*state),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_recording_walks_the_states_and_lands_in_the_recordings_folder() {
        let scratch = Scratch::new("walk");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &scratch.places(), &request(true), &mut |r| {
                heard.push(r)
            })
            .expect("started");
        assert_eq!(session.state(), State::Recording);
        assert!(fake.started[0].audio);
        // The origin arrives before the state the card's clock starts on.
        let started_at = heard
            .iter()
            .position(|r| matches!(r, Report::Started(_)))
            .expect("a start");
        let recording_at = heard
            .iter()
            .position(|r| matches!(r, Report::State(State::Recording)))
            .expect("the recording state");
        assert!(started_at < recording_at, "{heard:?}");
        let entry = session
            .stop(&mut fake, &scratch.places(), &mut |r| heard.push(r))
            .expect("stopped");
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle
            ]
        );
        assert_eq!(entry.kind, EntryKind::Recording);
        assert_eq!(entry.path.parent(), Some(scratch.recordings().as_path()));
        assert!(
            !scratch
                .0
                .join("videos")
                .join(entry.path.file_name().unwrap())
                .exists(),
            "nothing lands in the videos root"
        );
        let name = entry
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with("Recording ") && name.ends_with(".mp4"),
            "{name}"
        );
        assert_eq!(
            std::fs::read(&entry.path).expect("file"),
            b"fake mp4 finished"
        );
        assert_eq!(entry.size, 17);
        assert!(heard
            .iter()
            .any(|r| matches!(r, Report::Finished(e) if e == &entry)));
        let hidden: Vec<_> = std::fs::read_dir(scratch.recordings())
            .expect("folder")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(hidden.is_empty(), "the hidden file is published");
    }

    /// The child is spawned only once the portal has answered, and on the
    /// node that answer named: `prepare` (the whole portal exchange, `Start`
    /// and the remote included) returns before `start` is called, and the
    /// pipeline's source is the prepared node. Pinned after the live run of
    /// 2026-10-10 asked whether the order had moved (it had not).
    #[test]
    fn the_child_is_spawned_after_the_portal_answered_and_on_its_node() {
        let scratch = Scratch::new("order");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |r| {
                heard.push(r)
            })
            .expect("started");
        assert_eq!(fake.calls, ["prepare", "start"]);
        assert_eq!(fake.started[0].source.node, FAKE_NODE);
        assert_eq!(fake.started[0].source.fd, None, "no remote: no fd= token");
        let argv: Vec<String> = fake.started[0]
            .launch_argv()
            .iter()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        assert!(argv.contains(&format!("path={FAKE_NODE}")), "{argv:?}");
        // Nothing was reported before the portal answered: the first report
        // is the preparing state, then the origin, then recording.
        assert!(matches!(heard[0], Report::State(State::Preparing)));
        session
            .stop(&mut fake, &scratch.places(), &mut |r| heard.push(r))
            .expect("stopped");
        assert_eq!(fake.calls, ["prepare", "start", "stop"]);
    }

    /// A child whose source never streams writes nothing. Past the signal
    /// deadline the recording fails with its own notice, the child is
    /// killed and the empty file goes; a child that did write is left alone.
    #[test]
    fn a_source_that_never_streams_fails_the_recording_in_time() {
        struct Silent;
        impl Recorder for Silent {
            fn probe(&mut self) -> Probe {
                Fake::default().probe()
            }
            fn prepare(&mut self) -> Result<super::Prepared, RecordError> {
                Ok(super::Prepared {
                    source: selenita_core::record::Source { node: 57, fd: None },
                    fd: None,
                })
            }
            fn start(
                &mut self,
                pipeline: &Pipeline,
                _fd: Option<OwnedFd>,
            ) -> Result<(), RecordError> {
                // The muxer opened the file and wrote nothing.
                std::fs::write(&pipeline.out, b"").map_err(|e| RecordError::Write(e.to_string()))
            }
            fn exited(&mut self) -> Option<Exit> {
                None
            }
            fn stop(&mut self) -> Result<(), RecordError> {
                panic!("the stop would hang: the watchdog must act first")
            }
            fn abandon(&mut self) {}
        }
        let scratch = Scratch::new("silent");
        let mut silent = Silent;
        let mut session = Session::new(silent.probe()).with_signal_deadline(Duration::ZERO);
        let mut heard = Vec::new();
        session
            .start(&mut silent, &scratch.places(), &request(false), &mut |r| {
                heard.push(r)
            })
            .expect("started");
        assert!(matches!(
            session.watch_signal(&mut silent, &mut |r| heard.push(r)),
            Some(RecordError::NoSignal)
        ));
        assert_eq!(session.state(), State::Idle);
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Failed,
                State::Idle
            ]
        );
        assert!(std::fs::read_dir(scratch.recordings())
            .unwrap()
            .next()
            .is_none());
        assert!(!RecordError::NoSignal.message_es().is_empty());
        // Idle: nothing to watch.
        assert!(session.watch_signal(&mut silent, &mut |_| {}).is_none());
        // The fake writes at start, so even a zero deadline never fires.
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe()).with_signal_deadline(Duration::ZERO);
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |_| {})
            .expect("started");
        assert!(session.watch_signal(&mut fake, &mut |_| {}).is_none());
        assert_eq!(session.state(), State::Recording);
        session
            .stop(&mut fake, &scratch.places(), &mut |_| {})
            .expect("stopped");
    }

    /// Two recordings in a row: both publish, the second with an origin of
    /// its own reported before its state, nothing of the first left behind.
    #[test]
    fn a_second_recording_in_a_row_publishes_with_its_own_origin() {
        let scratch = Scratch::new("twice");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        let mut origins = Vec::new();
        for _ in 0..2 {
            session
                .start(&mut fake, &scratch.places(), &request(false), &mut |r| {
                    heard.push(r)
                })
                .expect("started");
            assert_eq!(session.state(), State::Recording);
            std::thread::sleep(std::time::Duration::from_millis(5));
            let entry = session
                .stop(&mut fake, &scratch.places(), &mut |r| heard.push(r))
                .expect("stopped");
            assert_eq!(session.state(), State::Idle);
            assert!(session.active.is_none());
            origins.push(entry.taken_at);
        }
        assert!(origins[1] > origins[0], "the second origin is later");
        let starts: Vec<_> = heard
            .iter()
            .filter_map(|r| match r {
                Report::Started(when) => Some(*when),
                _ => None,
            })
            .collect();
        assert_eq!(starts, origins);
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle,
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle
            ]
        );
        assert_eq!(fake.started.len(), 2);
        assert_ne!(fake.started[0].out, fake.started[1].out);
        let published: Vec<_> = std::fs::read_dir(scratch.recordings())
            .expect("folder")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(published.len(), 2, "{published:?}");
        assert!(published.iter().all(|name| !name.starts_with('.')));
    }

    #[test]
    fn a_second_start_is_busy_and_a_stop_without_a_recording_is_refused() {
        let scratch = Scratch::new("busy");
        let mut fake = make(true);
        let mut session = Session::new(fake.probe());
        assert!(matches!(
            session.stop(fake.as_mut(), &scratch.places(), &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        session
            .start(
                fake.as_mut(),
                &scratch.places(),
                &request(false),
                &mut |_| {},
            )
            .expect("started");
        assert!(matches!(
            session.start(
                fake.as_mut(),
                &scratch.places(),
                &request(false),
                &mut |_| {}
            ),
            Err(RecordError::Busy)
        ));
        session
            .stop(fake.as_mut(), &scratch.places(), &mut |_| {})
            .expect("stopped");
        assert_eq!(session.state(), State::Idle);
    }

    /// A recorder whose portal is cancelled, or whose child dies.
    struct Broken {
        cancel: bool,
    }

    impl Recorder for Broken {
        fn probe(&mut self) -> Probe {
            Probe {
                inspector: true,
                launcher: true,
                muxer: true,
                render_node: true,
                va_element: true,
            }
        }
        fn prepare(&mut self) -> Result<super::Prepared, RecordError> {
            if self.cancel {
                return Err(RecordError::Portal(crate::portal::PortalError::Cancelled));
            }
            Ok(super::Prepared {
                source: selenita_core::record::Source { node: 7, fd: None },
                fd: None,
            })
        }
        fn start(&mut self, pipeline: &Pipeline, _fd: Option<OwnedFd>) -> Result<(), RecordError> {
            std::fs::write(&pipeline.out, b"x").map_err(|e| RecordError::Write(e.to_string()))
        }
        fn exited(&mut self) -> Option<Exit> {
            Some(Exit {
                code: Some(1),
                stderr: "boom".to_owned(),
            })
        }
        fn stop(&mut self) -> Result<(), RecordError> {
            Err(RecordError::Exited(Exit {
                code: Some(1),
                stderr: "boom".to_owned(),
            }))
        }
        fn abandon(&mut self) {}
    }

    #[test]
    fn a_cancelled_portal_leaves_the_session_idle_without_a_file() {
        let scratch = Scratch::new("cancel");
        let mut session = Session::new(Probe {
            inspector: true,
            launcher: true,
            muxer: true,
            render_node: true,
            va_element: true,
        });
        let mut heard = Vec::new();
        let result = session.start(
            &mut Broken { cancel: true },
            &scratch.places(),
            &request(false),
            &mut |r| heard.push(r),
        );
        assert!(matches!(
            result,
            Err(RecordError::Portal(crate::portal::PortalError::Cancelled))
        ));
        assert_eq!(
            states(&heard),
            [State::Preparing, State::Failed, State::Idle]
        );
        assert!(
            !scratch.recordings().exists()
                || std::fs::read_dir(scratch.recordings())
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn a_child_that_dies_fails_the_recording_and_removes_its_file() {
        let scratch = Scratch::new("dies");
        let mut broken = Broken { cancel: false };
        let mut session = Session::new(broken.probe());
        let mut heard = Vec::new();
        session
            .start(&mut broken, &scratch.places(), &request(false), &mut |r| {
                heard.push(r)
            })
            .expect("started");
        let exit = broken.exited().expect("exited");
        assert!(matches!(
            session.ended(exit, &scratch.places(), &mut |r| heard.push(r)),
            Err(RecordError::Exited(_))
        ));
        assert_eq!(session.state(), State::Idle);
        assert!(std::fs::read_dir(scratch.recordings())
            .unwrap()
            .next()
            .is_none());
        // A stop that finds the child gone is a failure too, and leaves no file.
        session
            .start(&mut broken, &scratch.places(), &request(false), &mut |_| {})
            .expect("started again");
        assert!(matches!(
            session.stop(&mut broken, &scratch.places(), &mut |_| {}),
            Err(RecordError::Exited(_))
        ));
        assert!(std::fs::read_dir(scratch.recordings())
            .unwrap()
            .next()
            .is_none());
    }

    /// A child whose source sent EOS: it exits 0 on its own.
    struct Finishing;

    impl Recorder for Finishing {
        fn probe(&mut self) -> Probe {
            Fake::default().probe()
        }
        fn prepare(&mut self) -> Result<super::Prepared, RecordError> {
            Ok(super::Prepared {
                source: selenita_core::record::Source { node: 7, fd: None },
                fd: None,
            })
        }
        fn start(&mut self, pipeline: &Pipeline, _fd: Option<OwnedFd>) -> Result<(), RecordError> {
            std::fs::write(&pipeline.out, b"eos").map_err(|e| RecordError::Write(e.to_string()))
        }
        fn exited(&mut self) -> Option<Exit> {
            Some(Exit {
                code: Some(0),
                stderr: String::new(),
            })
        }
        fn stop(&mut self) -> Result<(), RecordError> {
            panic!("the child is gone")
        }
        fn abandon(&mut self) {}
    }

    #[test]
    fn a_child_that_ends_with_eos_has_its_file_published() {
        let scratch = Scratch::new("eos");
        let mut finishing = Finishing;
        let mut session = Session::new(finishing.probe());
        let mut heard = Vec::new();
        session
            .start(
                &mut finishing,
                &scratch.places(),
                &request(false),
                &mut |r| heard.push(r),
            )
            .expect("started");
        let exit = finishing.exited().expect("exited");
        let entry = session
            .ended(exit, &scratch.places(), &mut |r| heard.push(r))
            .expect("published");
        assert_eq!(std::fs::read(&entry.path).expect("file"), b"eos");
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle
            ]
        );
        assert!(heard
            .iter()
            .any(|r| matches!(r, Report::Finished(e) if e == &entry)));
    }

    #[test]
    fn quitting_while_recording_publishes_the_file_and_writes_the_history() {
        let scratch = Scratch::new("quit");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |_| {})
            .expect("started");
        let entry = session
            .quit(&mut fake, &scratch.places(), &mut |_| {})
            .expect("published at quit");
        assert_eq!(session.state(), State::Idle);
        assert_eq!(
            std::fs::read(&entry.path).expect("file"),
            b"fake mp4 finished"
        );
        let file = scratch.0.join("history");
        super::record_in_history(Some(file.clone()), entry.clone());
        let saved = selenita_core::History::load(file).expect("history");
        assert_eq!(saved.entries().len(), 1);
        assert_eq!(saved.entries()[0].path, entry.path);
        assert_eq!(saved.entries()[0].kind, EntryKind::Recording);
        // Nothing recording: a quit publishes nothing and leaves no file.
        assert!(session
            .quit(&mut fake, &scratch.places(), &mut |_| {})
            .is_none());
        let files: Vec<_> = std::fs::read_dir(scratch.recordings())
            .expect("folder")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn the_encoder_follows_the_probe() {
        let va = Session::new(Probe {
            inspector: true,
            launcher: true,
            muxer: true,
            render_node: true,
            va_element: true,
        });
        let no_node = Session::new(Probe {
            inspector: true,
            launcher: true,
            muxer: true,
            render_node: false,
            va_element: true,
        });
        assert_eq!(no_node.encoder, selenita_core::record::VideoEncoder::X264);
        assert_eq!(va.encoder, selenita_core::record::VideoEncoder::VaH264);
        let software = Session::new(Fake::default().probe());
        assert_eq!(software.encoder, selenita_core::record::VideoEncoder::X264);
        assert_eq!(
            Probe {
                inspector: false,
                launcher: false,
                muxer: false,
                render_node: false,
                va_element: false
            }
            .missing(),
            Some("gst-inspect-1.0")
        );
        assert_eq!(
            Probe {
                inspector: true,
                launcher: false,
                muxer: false,
                render_node: false,
                va_element: false
            }
            .missing(),
            Some("gst-launch-1.0")
        );
        assert_eq!(
            Probe {
                inspector: true,
                launcher: true,
                muxer: false,
                render_node: false,
                va_element: false
            }
            .missing(),
            Some("mp4mux")
        );
    }

    #[test]
    fn every_error_has_spanish_words() {
        for error in [
            RecordError::Busy,
            RecordError::NotRecording,
            RecordError::NoFile,
            RecordError::NoSignal,
            RecordError::Missing("mp4mux".to_owned()),
            RecordError::Portal(crate::portal::PortalError::Cancelled),
        ] {
            assert!(!error.message_es().is_empty());
        }
        assert!(RecordError::Missing("mp4mux".to_owned())
            .message_es()
            .contains("«mp4mux»"));
    }
}
