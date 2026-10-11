// language-contract: product-copy
//! The recording worker: one named thread that owns the recorder
//! (`recorder.rs`: the portal, `gst-launch-1.0` and `ffmpeg`, or the fake
//! under `SELENITA_FAKE=1`) and walks the crate's state machine, so the Qt
//! thread never waits on the portal, a child or a file.
//!
//! A recording runs here: the portal is asked for a monitor (the person
//! chooses in its dialog), the pipeline is started into a hidden segment in
//! the videos folder's `Grabaciones` (`names::RECORDINGS_FOLDER`), the child
//! is watched (an early exit is a failure) together with the stop file
//! `selenita --stop` touches when no instance answers on the bus and with
//! the segment's size (a source that never streams leaves the muxer nothing
//! to write: past [`SIGNAL_DEADLINE`] with an empty segment the recording
//! fails with a notice instead of hanging the stop, SEL-1-E), and the stop
//! interrupts the child, waits for the muxer to finish, publishes the file
//! under its name without replacing anything and hands the entry to the
//! capture worker, which owns the history. A child that ends on its own with
//! status 0 (the source sent EOS) has its file published too; any other
//! early exit is a failure. At start the recorder is probed for the muxer,
//! so a missing `gst-plugins-good` is said before anything is recorded.
//!
//! A focused start (`selenita --record-focused`, SEL-4-A) first asks niri
//! for the focused output and leaves a one-shot pick request for the
//! portal's chooser (`selenita_core::pick`), so `selenita --pick-output`
//! answers it without the dialog; when niri cannot say, the start goes on
//! without one and the dialog asks. Whatever start it is, a request nobody
//! consumed is removed before and after the portal is asked, so a plain
//! start always shows the dialog. A discard (the recording bar's
//! «Descartar») kills the child and removes every segment: nothing is
//! published, nothing joins the history and no preview shows.
//!
//! A pause (SEL-4-B) finishes the current segment as a stop does and keeps
//! the portal's session; a resume starts a new child on the same node, on a
//! fresh remote, into the next segment (`selenita_core::segments`). A
//! segment the child wrote nothing into (a pause or a stop right after a
//! resume, before the first buffer) is killed at once and dropped: such a
//! child never reaches the EOS an interrupt asks for. The stop, from
//! recording or paused, says `stopping` before it finishes the last
//! segment, then publishes one segment as it is and joins two or more with
//! `ffmpeg` into one file; when the join (or the joined file's publish)
//! fails each segment is published on its own, and a file that cannot be
//! published at all is left where it is and named in the notice, so nothing
//! recorded is lost. When the current segment fails (a resume that cannot
//! reopen the session, a child that dies, a source that stops streaming)
//! the segments already finished are published and the notice says so. The
//! clock counts recorded time only: a resume reports a start that far in
//! the past. A «Descartar» asked while the stop finishes or joins is
//! honoured once that step returns ([`Signals`]), and a toggle asked while
//! a stop ran is dropped rather than starting another recording.
//!
//! When the window quits ([`quit`], after the event loop ends) a recording
//! under way or paused is stopped the same way, and every entry published
//! from then on (the quit's own, a child that ended in the same turn, a
//! join that finished after the window closed) is written to the history
//! file directly, the Qt thread being gone; a portal dialog still open is
//! closed. The wait is bounded by [`QUIT_DEADLINE`]; past it the process
//! ends and whatever was not published stays hidden.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use celestina_core::{atomic_file, xdg};
use selenita_core::names::recordings_dir_in;
use selenita_core::pick;
use selenita_core::record::{self, take_stop_request, Event, Pipeline, State, VideoEncoder};
use selenita_core::runner::{Exit, RunError};
use selenita_core::segments::{self, concat_list, part_name, segment_file, JOIN_MAX};
use selenita_core::{capture_file_name, videos_dir, Entry, EntryKind, History};

use crate::capture::publish;
use crate::portal::PortalError;
use crate::recorder::{make, Probe, Recorder};

/// How long the muxer gets to finish the file after the interrupt.
pub const STOP_DEADLINE: Duration = Duration::from_secs(10);
/// What the quit adds to the finish and the longest join: the publish (or
/// the segments' publish one by one) and the history file's write.
const PUBLISH_MARGIN: Duration = Duration::from_secs(15);
/// How long the quitting window waits for the worker: the last segment's
/// finish, the longest join (`segments::JOIN_MAX`) and the publish.
pub const QUIT_DEADLINE: Duration = STOP_DEADLINE
    .saturating_add(JOIN_MAX)
    .saturating_add(PUBLISH_MARGIN);
/// How long after the child started the segment may stay empty: the muxer
/// writes its header with the first frames, so an empty file past this
/// means the portal's node never streamed (measured on 2026-10-10: a node
/// that exists but sends nothing holds `gst-launch-1.0 -e` past any stop,
/// and the kill at [`STOP_DEADLINE`] lost the recording in silence).
pub const SIGNAL_DEADLINE: Duration = Duration::from_secs(8);
/// How often the child and the stop file are looked at while recording or
/// paused.
const POLL: Duration = Duration::from_millis(250);

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
    /// A discarded recording's hidden file could not be removed.
    Discard(String),
    /// The segments could not be joined (or the joined file published);
    /// they were published one by one.
    Join(String),
    /// The resume could not start the next segment; what was recorded
    /// before the pause was published.
    Resume(String),
    /// The current segment failed after a resume; what was recorded before
    /// the last pause was published.
    Partial(Box<RecordError>),
    /// The segment a pause finished was lost; the recording stays paused
    /// with the earlier ones.
    SegmentLost(Box<RecordError>),
    /// Recorded files that could not be published: left where they are.
    Unpublished {
        cause: Box<RecordError>,
        kept: Vec<PathBuf>,
    },
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
            Self::Discard(_) => "No se ha podido borrar la grabación descartada.".to_owned(),
            Self::Join(_) => {
                "No se han podido unir las partes de la grabación: se han guardado por separado."
                    .to_owned()
            }
            Self::Resume(_) => {
                "No se ha podido reanudar la grabación: se ha guardado lo grabado hasta la pausa."
                    .to_owned()
            }
            Self::Partial(_) => {
                "La grabación se ha interrumpido: se ha guardado lo grabado hasta la última pausa."
                    .to_owned()
            }
            Self::SegmentLost(_) => {
                "Se ha perdido el último tramo de la grabación; lo anterior se conserva.".to_owned()
            }
            Self::Unpublished { kept, .. } => {
                let places: Vec<String> = kept
                    .iter()
                    .map(|path| format!("«{}»", path.display()))
                    .collect();
                format!(
                    "No se ha podido guardar la grabación: sigue en {}.",
                    places.join(", ")
                )
            }
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
            Self::Discard(detail) => {
                write!(formatter, "cannot remove the discarded file: {detail}")
            }
            Self::Join(detail) => write!(formatter, "cannot join the segments: {detail}"),
            Self::Resume(detail) => write!(formatter, "cannot resume: {detail}"),
            Self::Partial(cause) => write!(formatter, "the segment after a pause failed: {cause}"),
            Self::SegmentLost(cause) => write!(formatter, "the paused segment was lost: {cause}"),
            Self::Unpublished { cause, kept } => write!(
                formatter,
                "left unpublished ({cause}): {}",
                kept.iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => write!(formatter, "{other:?}"),
        }
    }
}

/// One recording as the controller asked for it.
#[derive(Clone, Debug)]
pub struct Request {
    /// The system's sound too, for every segment.
    pub audio: bool,
    /// Product copy from the Qt seam: the file name's stem (the recording word).
    pub stem: String,
    /// The monitor niri has focused, without the output dialog
    /// (`--record-focused`).
    pub focused: bool,
}

/// What the worker reports to the Qt thread.
#[derive(Debug)]
pub enum Report {
    /// What the host lacks for recording, if anything.
    Missing(Option<&'static str>),
    /// The recording moved to this state.
    State(State),
    /// The clock's origin: the moment the recording would have begun had it
    /// never paused (now less the time recorded so far).
    Started(SystemTime),
    /// The recording paused with this much recorded.
    Paused(Duration),
    /// A file is published: its entry, for the history. A failed join
    /// publishes one per segment.
    Finished(Entry),
    /// The recording was discarded: its files are gone, nothing to show.
    Discarded,
    /// The recording failed, in Spanish.
    Failed(String),
}

enum Job {
    /// Start, or stop what records; asked at this instant.
    Toggle(Request, Instant),
    Stop,
    Pause,
    Resume,
    /// Stop and delete the files.
    Discard,
    /// The window quits: finish what records, then end; the sender hears
    /// when it is done.
    Quit(Sender<()>),
}

/// What the Qt thread tells the worker without waiting for its turn: the
/// worker runs one job at a time, and a stop's finish and join take
/// seconds.
#[derive(Debug, Default)]
pub struct Signals {
    /// «Descartar» was asked: a stop under way drops everything instead of
    /// publishing.
    discard: AtomicBool,
    /// The window has quit: entries go to the history file directly.
    quitting: AtomicBool,
}

/// Where a recording's files go, where the stop file is, where a focused
/// start leaves its pick request, where a join writes its list and which
/// history file takes the entries published after the window quit.
pub struct Places {
    /// The recordings folder inside the videos folder; made at start.
    pub recordings: PathBuf,
    pub stop_file: PathBuf,
    /// The runtime folder's pick request; `None` without a runtime folder
    /// (the chooser reads only that one, so no other place would do).
    pub pick_request: Option<PathBuf>,
    /// The private runtime folder, where a join's concat list goes; `None`
    /// without one (the segments are then published one by one).
    pub runtime: Option<PathBuf>,
    /// The history file (`History::default_file`).
    pub history: Option<PathBuf>,
}

impl Places {
    /// `Grabaciones` in the [`videos_folder`], the runtime folder's stop
    /// file and pick request, the history file. Blocking.
    pub(crate) fn resolve() -> Self {
        let runtime = xdg::runtime_dir().ok();
        let pick_request = runtime.as_deref().map(pick::request_file);
        let stop_runtime = runtime.clone().unwrap_or_else(std::env::temp_dir);
        Self {
            recordings: recordings_dir_in(&videos_folder()),
            stop_file: record::stop_file(&stop_runtime),
            pick_request,
            runtime,
            history: History::default_file(),
        }
    }
}

/// The videos folder (else the home, else the system temporary folder),
/// where the recordings folder is. Blocking. The capture worker reads it
/// too, for `Adopt`.
pub(crate) fn videos_folder() -> PathBuf {
    videos_dir()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir)
}

static NEXT_HIDDEN: AtomicU64 = AtomicU64::new(1);

fn hidden_file(folder: &Path) -> PathBuf {
    folder.join(format!(
        ".selenita-{}-{}.mp4",
        std::process::id(),
        NEXT_HIDDEN.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A recording under way or paused.
struct Active {
    /// The hidden file a join writes; the segments sit beside it.
    hidden: PathBuf,
    started: SystemTime,
    stem: String,
    /// Every segment's pipeline; its `out` is the latest segment.
    pipeline: Pipeline,
    /// The segments finished so far, in order.
    done: Vec<PathBuf>,
    /// The segment the child writes; `None` while paused.
    current: Option<PathBuf>,
    /// When the current segment began.
    segment_started: SystemTime,
    /// The time recorded in the finished segments.
    recorded: Duration,
}

impl Active {
    /// Every file of this recording that may exist: the segments and the
    /// join's output.
    fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.done
            .iter()
            .chain(self.current.iter())
            .chain(std::iter::once(&self.hidden))
    }

    /// Removes every file of this recording.
    fn remove_all(&self) {
        for file in self.files() {
            let _ = remove_file(file);
        }
    }
}

/// Removes `path`; a file that is not there is gone already.
fn remove_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn size_of(path: &Path) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

/// How the current segment ended.
enum Closed {
    /// No child ran (paused, or nothing started).
    Nothing,
    /// Finished with something in it: kept.
    Kept,
    /// Nothing was written to it (empty after its EOS, or killed after the
    /// grace without one): the file dropped, nothing lost.
    Empty,
    /// The child did not finish it: dropped, and why.
    Lost(RecordError),
}

/// What a stop published, and what the window must still be told (a
/// failed join, a segment lost after a pause, a file left unpublished).
#[derive(Debug)]
pub struct Finished {
    pub entries: Vec<Entry>,
    pub notice: Option<RecordError>,
}

impl Finished {
    /// The notice as an error the worker says.
    fn into_result(self) -> Result<(), RecordError> {
        self.notice.map_or(Ok(()), Err)
    }
}

/// What [`publish_segments`] did.
struct Published {
    entries: Vec<Entry>,
    notice: Option<RecordError>,
    /// A discard asked during the join won: everything was removed.
    discarded: bool,
}

/// The worker's state between jobs: the machine's state and the recording
/// it is about, with the choices made at start.
pub struct Session {
    state: State,
    active: Option<Active>,
    encoder: VideoEncoder,
    /// How long a segment may stay empty after its child started.
    signal_deadline: Duration,
    signals: Arc<Signals>,
    /// When the last stop or discard returned: a toggle asked before it was
    /// asked of that recording, not for a new one.
    stopped_at: Option<Instant>,
}

/// What the worker does after a turn.
#[derive(Debug, Eq, PartialEq)]
enum Flow {
    Go,
    End,
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
            signals: Arc::default(),
            stopped_at: None,
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

    /// A «Descartar» was asked since the last look; taken once.
    fn discard_asked(&self) -> bool {
        self.signals.discard.swap(false, Ordering::SeqCst)
    }

    /// Hands a published entry on: to the window, or once it has quit to
    /// the history file directly.
    fn deliver(&self, entry: Entry, places: &Places, report: &mut dyn FnMut(Report)) {
        if self.signals.quitting.load(Ordering::SeqCst) {
            record_in_history(places.history.clone(), entry);
        } else {
            report(Report::Finished(entry));
        }
    }

    /// While it records: whether the current segment has anything yet.
    /// Past the deadline with an empty segment the portal's node never
    /// streamed, so the child is killed and the recording ends: with
    /// [`RecordError::NoSignal`] when nothing came before, else with the
    /// segments already finished published ([`RecordError::Partial`]).
    /// `None` otherwise (nothing to say, or too early to tell).
    pub fn watch_signal(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Option<RecordError> {
        if self.state != State::Recording {
            return None;
        }
        let active = self.active.as_mut()?;
        let current = active.current.as_ref()?;
        let waited = SystemTime::now()
            .duration_since(active.segment_started)
            .unwrap_or_default();
        if waited < self.signal_deadline || size_of(current) > 0 {
            return None;
        }
        recorder.abandon();
        if let Some(current) = active.current.take() {
            let _ = remove_file(&current);
        }
        self.wrap_up(recorder, places, report, Some(RecordError::NoSignal))
            .map_or_else(Some, |finished| finished.notice)
    }

    /// Starts a recording: the portal, then the child into the first
    /// segment.
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
        // A discard asked of an earlier recording does not reach this one.
        self.signals.discard.store(false, Ordering::SeqCst);
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
        let pick = places.pick_request.as_deref();
        match pick {
            // A request nobody consumed must not answer this start's chooser.
            Some(file) => {
                remove_unconsumed(file);
                if request.focused {
                    aim(recorder, file);
                }
            }
            None if request.focused => eprintln!(
                "selenita: recording: no runtime folder for a pick request; the chooser's dialog asks"
            ),
            None => {}
        }
        let prepared = recorder.prepare(pick);
        // The chooser consumed it, or never ran (a backend without
        // Selenita's chooser): either way it goes now.
        if let Some(file) = pick {
            remove_unconsumed(file);
        }
        let prepared = prepared?;
        std::fs::create_dir_all(&places.recordings)
            .map_err(|error| RecordError::Write(error.to_string()))?;
        let hidden = hidden_file(&places.recordings);
        let segment = segment_file(&hidden, 1);
        let pipeline = Pipeline {
            source: prepared.source,
            encoder: self.encoder,
            audio: request.audio,
            out: segment.clone(),
        };
        if let Err(error) = recorder.start(&pipeline, prepared.fd) {
            let _ = std::fs::remove_file(&segment);
            return Err(error);
        }
        let now = SystemTime::now();
        self.active = Some(Active {
            hidden,
            started: now,
            stem: request.stem.clone(),
            pipeline,
            done: Vec::new(),
            current: Some(segment),
            segment_started: now,
            recorded: Duration::ZERO,
        });
        Ok(())
    }

    /// Ends the current segment: the child is told to finish it (SIGINT,
    /// EOS), and a segment finished with something in it is kept with its
    /// time. One still empty once the child finished, or once the
    /// recorder's grace went by without its EOS (a child that never
    /// received a buffer, killed then rather than at [`STOP_DEADLINE`]), is
    /// dropped: nothing was recorded in it.
    fn close_segment(&mut self, recorder: &mut dyn Recorder) -> Closed {
        let Some(active) = self.active.as_mut() else {
            return Closed::Nothing;
        };
        let Some(segment) = active.current.take() else {
            return Closed::Nothing;
        };
        let ran = SystemTime::now()
            .duration_since(active.segment_started)
            .unwrap_or_default();
        // Always asked to finish first: the size before the EOS says
        // nothing (the muxer may write only then). The recorder kills a
        // child that has not reached its EOS within the grace with its file
        // still empty (`NoSignal`): it never received a buffer.
        let closed = match (recorder.finish(&segment), size_of(&segment)) {
            (Ok(()) | Err(RecordError::NoSignal), 0) => Closed::Empty,
            (Ok(()), _) => Closed::Kept,
            (Err(error), _) => Closed::Lost(error),
        };
        if matches!(closed, Closed::Kept) {
            active.recorded += ran;
            active.done.push(segment);
        } else {
            let _ = remove_file(&segment);
        }
        closed
    }

    /// Pauses: the current segment is finished and kept, the portal's
    /// session stays open. A segment with nothing in it is dropped and the
    /// recording pauses all the same; one the child failed is dropped too
    /// when earlier segments exist (the recording pauses with them, and
    /// [`RecordError::SegmentLost`] says so).
    ///
    /// # Errors
    ///
    /// [`RecordError::NotRecording`] unless it records;
    /// [`RecordError::SegmentLost`] as above; with nothing before a failed
    /// segment the recording fails with the child's error.
    pub fn pause(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Result<(), RecordError> {
        if self.state != State::Recording {
            return Err(RecordError::NotRecording);
        }
        let notice = match self.close_segment(recorder) {
            Closed::Nothing | Closed::Kept | Closed::Empty => None,
            Closed::Lost(cause) if self.has_segments() => {
                eprintln!("selenita: recording: the paused segment was lost: {cause}");
                Some(RecordError::SegmentLost(Box::new(cause)))
            }
            Closed::Lost(cause) => {
                return self
                    .wrap_up(recorder, places, report, Some(cause))
                    .and_then(Finished::into_result);
            }
        };
        // The time comes before the state, as the origin does: the clock
        // reads it the moment it sees `Paused`.
        let recorded = self
            .active
            .as_ref()
            .map_or(Duration::ZERO, |active| active.recorded);
        report(Report::Paused(recorded));
        self.step(Event::Pause, report);
        notice.map_or(Ok(()), Err)
    }

    fn has_segments(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.done.is_empty())
    }

    /// Resumes: a new child on the same node, on a fresh remote, into the
    /// next segment; the clock's origin moves back by the time recorded.
    ///
    /// # Errors
    ///
    /// [`RecordError::NotRecording`] unless it is paused. When the next
    /// segment cannot start (the portal's session is gone) the recording
    /// ends with what was recorded before the pause published
    /// ([`RecordError::Resume`]).
    pub fn resume(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Result<(), RecordError> {
        if self.state != State::Paused {
            return Err(RecordError::NotRecording);
        }
        let Some(active) = self.active.as_mut() else {
            return Err(RecordError::NotRecording);
        };
        let part = u32::try_from(active.done.len() + 1).unwrap_or(u32::MAX);
        let segment = segment_file(&active.hidden, part);
        let mut pipeline = active.pipeline.clone();
        pipeline.out = segment.clone();
        let started = recorder
            .reopen()
            .and_then(|fd| recorder.start(&pipeline, fd));
        match started {
            Ok(()) => {
                let now = SystemTime::now();
                active.pipeline = pipeline;
                active.current = Some(segment);
                active.segment_started = now;
                let origin = now.checked_sub(active.recorded).unwrap_or(now);
                report(Report::Started(origin));
                self.step(Event::Resume, report);
                Ok(())
            }
            Err(error) => {
                let _ = remove_file(&segment);
                let cause = RecordError::Resume(error.to_string());
                self.wrap_up(recorder, places, report, Some(cause))
                    .and_then(Finished::into_result)
            }
        }
    }

    /// Stops the recording, recording or paused, and publishes it: one
    /// segment as it is, more joined into one file (or, when the join
    /// fails, each on its own). The state is `stopping` from the start, so
    /// the window shows the finish and the join as such.
    ///
    /// # Errors
    ///
    /// [`RecordError`] when nothing records, nothing was recorded, or no
    /// file could be published.
    pub fn stop(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Result<Finished, RecordError> {
        if !self.state.is_under_way() {
            return Err(RecordError::NotRecording);
        }
        self.step(Event::Stop, report);
        let cause = match self.close_segment(recorder) {
            Closed::Nothing | Closed::Kept => None,
            // Nothing before it either: the node never streamed.
            Closed::Empty if !self.has_segments() => Some(RecordError::NoSignal),
            Closed::Empty => None,
            Closed::Lost(cause) => Some(cause),
        };
        self.wrap_up(recorder, places, report, cause)
    }

    /// Ends a recording whose current segment, if any, is already dealt
    /// with: the session is released and the finished segments published,
    /// unless a «Descartar» was asked meanwhile, which removes them all.
    /// `cause` is why the current segment was lost, if it was: with no
    /// segment left the recording fails with it, else it becomes the
    /// notice ([`RecordError::Partial`] unless it already says more).
    fn wrap_up(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
        cause: Option<RecordError>,
    ) -> Result<Finished, RecordError> {
        if self.state.is_under_way() {
            self.step(Event::Stop, report);
        }
        recorder.release();
        let outcome = self.publish_or_drop(recorder, places, report, cause);
        self.stopped_at = Some(Instant::now());
        outcome
    }

    fn publish_or_drop(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
        cause: Option<RecordError>,
    ) -> Result<Finished, RecordError> {
        let Some(active) = self.active.take() else {
            self.fail(report);
            return Err(cause.unwrap_or(RecordError::NotRecording));
        };
        if self.discard_asked() {
            active.remove_all();
            return Ok(self.discarded(report));
        }
        if active.done.is_empty() {
            active.remove_all();
            self.fail(report);
            return Err(cause.unwrap_or(RecordError::NoFile));
        }
        let published = publish_segments(recorder, places, &active, &|| self.discard_asked());
        if published.discarded {
            return Ok(self.discarded(report));
        }
        if published.entries.is_empty() {
            self.fail(report);
            return Err(published.notice.unwrap_or(RecordError::NoFile));
        }
        self.step(Event::Stopped, report);
        for entry in &published.entries {
            self.deliver(entry.clone(), places, report);
        }
        let notice = match (published.notice, cause) {
            (Some(unpublished @ RecordError::Unpublished { .. }), _) => Some(unpublished),
            (_, Some(cause @ (RecordError::Resume(_) | RecordError::Partial(_)))) => Some(cause),
            (_, Some(cause)) => Some(RecordError::Partial(Box::new(cause))),
            (joined, None) => joined,
        };
        Ok(Finished {
            entries: published.entries,
            notice,
        })
    }

    /// The end of a discarded stop: idle, nothing published.
    fn discarded(&mut self, report: &mut dyn FnMut(Report)) -> Finished {
        self.step(Event::Stopped, report);
        report(Report::Discarded);
        Finished {
            entries: Vec::new(),
            notice: None,
        }
    }

    /// The child ended on its own while recording. With status 0 the source
    /// sent EOS and the muxer finished the segment, which is kept; any
    /// other exit loses it. Either way the recording ends there.
    ///
    /// # Errors
    ///
    /// [`RecordError::Exited`] for an unsuccessful exit with nothing before
    /// it, or the publish's error.
    pub fn ended(
        &mut self,
        exit: Exit,
        places: &Places,
        recorder: &mut dyn Recorder,
        report: &mut dyn FnMut(Report),
    ) -> Result<Finished, RecordError> {
        let mut cause = None;
        if let Some(active) = self.active.as_mut() {
            if let Some(segment) = active.current.take() {
                if exit.code == Some(0) && size_of(&segment) > 0 {
                    active.recorded += SystemTime::now()
                        .duration_since(active.segment_started)
                        .unwrap_or_default();
                    active.done.push(segment);
                } else {
                    let _ = remove_file(&segment);
                    cause = Some(if exit.code == Some(0) {
                        RecordError::NoFile
                    } else {
                        RecordError::Exited(exit)
                    });
                }
            }
        }
        self.wrap_up(recorder, places, report, cause)
    }

    /// Stops the recording without keeping it: the child is killed (its
    /// segment is going, so there is nothing for the muxer to finish), the
    /// session released and every segment removed. Nothing is published
    /// and no entry made.
    ///
    /// # Errors
    ///
    /// [`RecordError::NotRecording`] when nothing records or is paused, and
    /// [`RecordError::Discard`] when a file could not be removed; the
    /// session is idle either way.
    pub fn discard(
        &mut self,
        recorder: &mut dyn Recorder,
        report: &mut dyn FnMut(Report),
    ) -> Result<(), RecordError> {
        self.signals.discard.store(false, Ordering::SeqCst);
        if !self.state.is_under_way() {
            return Err(RecordError::NotRecording);
        }
        self.step(Event::Stop, report);
        recorder.abandon();
        let mut kept = Vec::new();
        if let Some(active) = self.active.take() {
            for file in active.files() {
                if let Err(error) = remove_file(file) {
                    kept.push(format!("{}: {error}", file.display()));
                }
            }
        }
        self.stopped_at = Some(Instant::now());
        if kept.is_empty() {
            self.step(Event::Stopped, report);
            report(Report::Discarded);
            Ok(())
        } else {
            self.fail(report);
            Err(RecordError::Discard(kept.join("; ")))
        }
    }

    /// The window quits: a recording under way or paused is stopped and
    /// published (its entries go to the history file, the window being
    /// gone, and are returned); whatever else runs is killed and its files
    /// removed.
    pub fn quit(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &Places,
        report: &mut dyn FnMut(Report),
    ) -> Vec<Entry> {
        self.signals.quitting.store(true, Ordering::SeqCst);
        let entries = if self.state.is_under_way() {
            match self.stop(recorder, places, report) {
                Ok(finished) => {
                    if let Some(notice) = &finished.notice {
                        eprintln!("selenita: recording at quit: {notice}");
                    }
                    finished.entries
                }
                Err(error) => {
                    eprintln!("selenita: recording at quit: {error}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        self.abandon(recorder, report);
        entries
    }

    /// Kills whatever runs, removes its files and returns to idle.
    fn abandon(&mut self, recorder: &mut dyn Recorder, report: &mut dyn FnMut(Report)) {
        recorder.abandon();
        if self.state.is_busy() {
            self.fail(report);
        } else if let Some(active) = self.active.take() {
            active.remove_all();
        }
    }

    /// Whatever failed: back to idle, the hidden files gone.
    fn fail(&mut self, report: &mut dyn FnMut(Report)) {
        if let Some(active) = self.active.take() {
            active.remove_all();
        }
        self.step(Event::Failure, report);
        self.step(Event::Acknowledged, report);
    }

    /// One turn of the worker: first the child (while it records), then
    /// `job`, or the stop file when no job came. A discard wins over a
    /// child that ended on its own in the same turn (nothing is published);
    /// any other job asked something of the recording that has just ended
    /// and is dropped, except a quit. A toggle asked while the last stop
    /// ran is dropped too: it was a second press on the stop.
    fn turn(
        &mut self,
        recorder: &mut dyn Recorder,
        places: &mut Places,
        missing: Option<&'static str>,
        job: Option<Job>,
        tell: &mut dyn FnMut(Report),
    ) -> Flow {
        if matches!(job, Some(Job::Quit(_))) {
            // From here every entry goes to the history file.
            self.signals.quitting.store(true, Ordering::SeqCst);
        }
        let discarding = matches!(job, Some(Job::Discard));
        if self.state == State::Recording && !discarding {
            let outcome = match recorder.exited() {
                Some(exit) => Some(
                    self.ended(exit, places, recorder, tell)
                        .and_then(Finished::into_result),
                ),
                None => self.watch_signal(recorder, places, tell).map(Err),
            };
            if let Some(outcome) = outcome {
                say(outcome, tell);
                if !matches!(job, Some(Job::Quit(_))) {
                    return Flow::Go;
                }
            }
        }
        let job = match job {
            Some(job) => job,
            None if self.state.is_under_way() && take_stop_request(&places.stop_file) => Job::Stop,
            None => return Flow::Go,
        };
        let under_way = self.state.is_under_way();
        let outcome = match job {
            Job::Toggle(..) | Job::Stop if under_way => self
                .stop(recorder, places, tell)
                .and_then(Finished::into_result),
            Job::Toggle(_, asked) if self.stopped_at.is_some_and(|stopped| asked < stopped) => {
                eprintln!(
                    "selenita: recording: a toggle asked while the last one stopped is dropped"
                );
                Ok(())
            }
            Job::Toggle(request, _) => match missing {
                Some(element) => Err(RecordError::Missing(element.to_owned())),
                None => {
                    // Re-read, so a folder chosen since is honoured.
                    *places = Places::resolve();
                    self.start(recorder, places, &request, tell)
                }
            },
            // A stop, pause, resume or discard with nothing to act on
            // (`selenita --stop` pressed twice, a late click) is not worth
            // a notice.
            Job::Stop => Ok(()),
            Job::Pause if self.state == State::Recording => self.pause(recorder, places, tell),
            Job::Resume if self.state == State::Paused => self.resume(recorder, places, tell),
            Job::Pause | Job::Resume => Ok(()),
            Job::Discard if under_way => self.discard(recorder, tell),
            Job::Discard => {
                // Asked of a recording that has ended since.
                self.signals.discard.store(false, Ordering::SeqCst);
                Ok(())
            }
            Job::Quit(done) => {
                self.quit(recorder, places, tell);
                let _ = done.send(());
                return Flow::End;
            }
        };
        say(outcome, tell);
        Flow::Go
    }
}

/// Says a turn's failure, or its notice, in the window.
fn say(outcome: Result<(), RecordError>, tell: &mut dyn FnMut(Report)) {
    if let Err(error) = outcome {
        eprintln!("selenita: recording: {error}");
        tell(Report::Failed(error.message_es()));
    }
}

/// Publishes a recording's finished segments: one as it is; more joined
/// into the hidden file and published as one. When the join or the joined
/// file's publish fails, each segment is published on its own as
/// `<name> (n).mp4`, with [`RecordError::Join`] as the notice. A file that
/// cannot be published is never removed: it stays where it is and
/// [`RecordError::Unpublished`] names it. A «Descartar» asked during the
/// join (`discard_asked`) removes everything instead.
fn publish_segments(
    recorder: &mut dyn Recorder,
    places: &Places,
    active: &Active,
    discard_asked: &dyn Fn() -> bool,
) -> Published {
    let name = capture_file_name(&active.stem, active.started, "mp4");
    let publish_one = |hidden: &Path, name: &str| -> Result<Entry, RecordError> {
        let size = size_of(hidden);
        if size == 0 {
            return Err(RecordError::NoFile);
        }
        publish(hidden, &places.recordings, name)
            .map(|path| Entry {
                path,
                kind: EntryKind::Recording,
                taken_at: active.started,
                size,
            })
            .map_err(|error| RecordError::Write(error.to_string()))
    };
    let kept_notice = |cause: RecordError, kept: Vec<PathBuf>| RecordError::Unpublished {
        cause: Box::new(cause),
        kept,
    };
    if let [only] = active.done.as_slice() {
        return match publish_one(only, &name) {
            Ok(entry) => Published {
                entries: vec![entry],
                notice: None,
                discarded: false,
            },
            Err(error) => {
                eprintln!(
                    "selenita: recording: {error}; the segment stays at {}",
                    only.display()
                );
                Published {
                    entries: Vec::new(),
                    notice: Some(kept_notice(error, vec![only.clone()])),
                    discarded: false,
                }
            }
        };
    }
    let joined = join(recorder, places, active);
    if discard_asked() {
        active.remove_all();
        return Published {
            entries: Vec::new(),
            notice: None,
            discarded: true,
        };
    }
    let joined = joined.and_then(|()| publish_one(&active.hidden, &name));
    match joined {
        Ok(entry) => {
            for segment in &active.done {
                let _ = remove_file(segment);
            }
            Published {
                entries: vec![entry],
                notice: None,
                discarded: false,
            }
        }
        Err(error) => {
            eprintln!("selenita: recording: {error}; publishing the segments one by one");
            let _ = remove_file(&active.hidden);
            let mut entries = Vec::new();
            let mut kept = Vec::new();
            let mut last = None;
            for (index, segment) in active.done.iter().enumerate() {
                let part = u32::try_from(index + 1).unwrap_or(u32::MAX);
                match publish_one(segment, &part_name(&name, part)) {
                    Ok(entry) => entries.push(entry),
                    Err(failure) => {
                        eprintln!(
                            "selenita: recording: segment {part}: {failure}; it stays at {}",
                            segment.display()
                        );
                        if size_of(segment) > 0 {
                            kept.push(segment.clone());
                        }
                        last = Some(failure);
                    }
                }
            }
            let joined = RecordError::Join(error.to_string());
            let notice = match last {
                Some(failure) if !kept.is_empty() => kept_notice(failure, kept),
                _ => joined,
            };
            Published {
                entries,
                notice: Some(notice),
                discarded: false,
            }
        }
    }
}

/// Writes the concat list in the private runtime folder and has the
/// recorder join the segments into the hidden file; the list goes after.
fn join(recorder: &mut dyn Recorder, places: &Places, active: &Active) -> Result<(), RecordError> {
    let fail = |detail: String| RecordError::Join(detail);
    let runtime = places
        .runtime
        .as_deref()
        .ok_or_else(|| fail("no runtime folder for the list".to_owned()))?;
    let list = concat_list(&active.done).map_err(|error| fail(error.to_string()))?;
    let tag = active
        .hidden
        .file_stem()
        .map(|stem| stem.to_string_lossy().trim_start_matches('.').to_owned())
        .unwrap_or_default();
    let file = segments::list_file(runtime, &tag);
    if let Some(folder) = file.parent() {
        xdg::ensure_private_dir(folder).map_err(|error| fail(error.to_string()))?;
    }
    atomic_file::replace_private(&file, &list)
        .map(|_| ())
        .map_err(|error| fail(error.to_string()))?;
    let joined = recorder.join(&file, &active.done, &active.hidden);
    let _ = remove_file(&file);
    joined?;
    if size_of(&active.hidden) == 0 {
        return Err(fail("the join wrote nothing".to_owned()));
    }
    Ok(())
}

/// Asks the recorder's niri for the focused output and leaves the pick
/// request for it in `file`; when either fails the start goes on without
/// one (the chooser's dialog asks), which is logged.
fn aim(recorder: &mut dyn Recorder, file: &Path) {
    let output = match recorder.focused_output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "selenita: recording: niri did not name the focused output ({error}); the chooser's dialog asks"
            );
            return;
        }
    };
    match pick::write_request(file, &output, SystemTime::now()) {
        Ok(()) => eprintln!("selenita: recording: the chooser is asked for {output}"),
        Err(error) => eprintln!(
            "selenita: recording: no pick request for {output} ({error}); the chooser's dialog asks"
        ),
    }
}

/// Removes a pick request nobody consumed, saying so.
fn remove_unconsumed(file: &Path) {
    match pick::clear_request(file) {
        Ok(true) => eprintln!("selenita: recording: an unconsumed pick request was removed"),
        Ok(false) => {}
        Err(error) => eprintln!("selenita: recording: {error}"),
    }
}

/// The entry joins the history file directly: once the window has quit the
/// Qt thread and the capture worker's queue are no longer there to carry
/// it. The history keeps one row per path, so an entry written twice is one.
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
    signals: Arc<Signals>,
}

/// The worker's inbox and signals for [`quit`], set when it starts.
static QUIT: OnceLock<(Sender<Job>, Arc<Signals>)> = OnceLock::new();

/// The window quit: tells the worker that entries now go to the history
/// file, asks it to finish a recording under way (or closes the portal
/// dialog it waits on) and waits up to `deadline` for it. Blocking; called
/// once the event loop has ended.
pub fn quit(deadline: Duration) {
    let Some((jobs, signals)) = QUIT.get() else {
        return;
    };
    signals.quitting.store(true, Ordering::SeqCst);
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
    /// while it records or is paused. `report` runs on the worker and must
    /// queue to the Qt thread itself.
    pub fn start(fake: bool, report: impl Fn(Report) + Send + 'static) -> Option<Self> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let signals = Arc::new(Signals::default());
        let _ = QUIT.set((jobs.clone(), Arc::clone(&signals)));
        let shared = Arc::clone(&signals);
        std::thread::Builder::new()
            .name("selenita-record".to_owned())
            .spawn(move || {
                let mut recorder = make(fake);
                let probe = recorder.probe();
                report(Report::Missing(probe.missing()));
                let mut session = Session::new(probe);
                session.signals = shared;
                let mut tell = |what: Report| report(what);
                let mut places = Places::resolve();
                loop {
                    let job = if session.state().is_under_way() {
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
                    let flow = session.turn(
                        recorder.as_mut(),
                        &mut places,
                        probe.missing(),
                        job,
                        &mut tell,
                    );
                    if flow == Flow::End {
                        break;
                    }
                }
                session.quit(recorder.as_mut(), &places, &mut tell);
            })
            .ok()
            .map(|_| Self { jobs, signals })
    }

    fn send(&self, job: Job) -> Result<(), RecordError> {
        self.jobs.send(job).map_err(|_| RecordError::WorkerGone)
    }

    /// Starts a recording, or stops the one under way (or paused).
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn toggle(&self, request: Request) -> Result<(), RecordError> {
        self.send(Job::Toggle(request, Instant::now()))
    }

    /// Stops the recording under way or paused; nothing happens when none
    /// is.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn stop(&self) -> Result<(), RecordError> {
        self.send(Job::Stop)
    }

    /// Pauses the recording under way; nothing happens otherwise.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn pause(&self) -> Result<(), RecordError> {
        self.send(Job::Pause)
    }

    /// Resumes the paused recording; nothing happens otherwise.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn resume(&self) -> Result<(), RecordError> {
        self.send(Job::Resume)
    }

    /// Stops the recording under way or paused and deletes its files;
    /// nothing happens when none is. A stop already finishing or joining
    /// sees the signal at once and drops everything instead of publishing.
    ///
    /// # Errors
    ///
    /// [`RecordError::WorkerGone`] when the thread is gone.
    pub fn discard(&self) -> Result<(), RecordError> {
        self.signals.discard.store(true, Ordering::SeqCst);
        self.send(Job::Discard)
    }
}

#[cfg(test)]
mod tests {
    use super::{Places, RecordError, Report, Request, Session};
    use crate::recorder::{make, Fake, Probe, Recorder, FAKE_NODE};
    use selenita_core::names::recordings_dir_in;
    use selenita_core::niri::NiriError;
    use selenita_core::record::{Pipeline, State};
    use selenita_core::runner::Exit;
    use selenita_core::EntryKind;
    use std::os::fd::OwnedFd;
    use std::path::{Path, PathBuf};
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
                pick_request: Some(self.pick_request()),
                runtime: Some(self.0.join("run")),
                history: Some(self.0.join("history")),
            }
        }

        /// The runtime folder's pick request, `run/selenita/pick-output`.
        fn pick_request(&self) -> PathBuf {
            selenita_core::pick::request_file(&self.0.join("run"))
        }

        /// The recordings folder, `videos/Grabaciones`, spelled out so the
        /// name itself is under test.
        fn recordings(&self) -> PathBuf {
            self.0.join("videos").join("Grabaciones")
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
            focused: false,
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
            .expect("stopped")
            .entries
            .remove(0);
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
        assert!(
            !scratch.0.join("videos").join("Recordings").exists(),
            "the old recordings folder is never made"
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
        assert_eq!(
            fake.calls,
            ["prepare", "start"],
            "a plain start asks niri nothing"
        );
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
        assert_eq!(fake.calls, ["prepare", "start", "finish", "release"]);
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
            fn focused_output(&mut self) -> Result<String, NiriError> {
                Err(NiriError::NoSocket)
            }
            fn prepare(
                &mut self,
                _pick: Option<&Path>,
            ) -> Result<crate::recorder::Prepared, RecordError> {
                Ok(crate::recorder::Prepared {
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
            fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
                panic!("the stop would hang: the watchdog must act first")
            }
            fn abandon(&mut self) {}
            fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
                Ok(None)
            }
            fn release(&mut self) {}
            fn join(
                &mut self,
                _list: &Path,
                _segments: &[PathBuf],
                _out: &Path,
            ) -> Result<(), RecordError> {
                Ok(())
            }
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
            session.watch_signal(&mut silent, &scratch.places(), &mut |r| heard.push(r)),
            Some(RecordError::NoSignal)
        ));
        assert_eq!(session.state(), State::Idle);
        // The stop is asked, then nothing is there to publish.
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
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
        assert!(session
            .watch_signal(&mut silent, &scratch.places(), &mut |_| {})
            .is_none());
        // The fake writes at start, so even a zero deadline never fires.
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe()).with_signal_deadline(Duration::ZERO);
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |_| {})
            .expect("started");
        assert!(session
            .watch_signal(&mut fake, &scratch.places(), &mut |_| {})
            .is_none());
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
                .expect("stopped")
                .entries
                .remove(0);
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
        fn focused_output(&mut self) -> Result<String, NiriError> {
            Err(NiriError::NoSocket)
        }
        fn prepare(
            &mut self,
            _pick: Option<&Path>,
        ) -> Result<crate::recorder::Prepared, RecordError> {
            if self.cancel {
                return Err(RecordError::Portal(crate::portal::PortalError::Cancelled));
            }
            Ok(crate::recorder::Prepared {
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
        fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
            Err(RecordError::Exited(Exit {
                code: Some(1),
                stderr: "boom".to_owned(),
            }))
        }
        fn abandon(&mut self) {}
        fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
            Ok(None)
        }
        fn release(&mut self) {}
        fn join(
            &mut self,
            _list: &Path,
            _segments: &[PathBuf],
            _out: &Path,
        ) -> Result<(), RecordError> {
            Ok(())
        }
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
            session.ended(exit, &scratch.places(), &mut broken, &mut |r| heard.push(r)),
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
        fn focused_output(&mut self) -> Result<String, NiriError> {
            Err(NiriError::NoSocket)
        }
        fn prepare(
            &mut self,
            _pick: Option<&Path>,
        ) -> Result<crate::recorder::Prepared, RecordError> {
            Ok(crate::recorder::Prepared {
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
        fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
            panic!("the child is gone")
        }
        fn abandon(&mut self) {}
        fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
            Ok(None)
        }
        fn release(&mut self) {}
        fn join(
            &mut self,
            _list: &Path,
            _segments: &[PathBuf],
            _out: &Path,
        ) -> Result<(), RecordError> {
            Ok(())
        }
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
            .ended(exit, &scratch.places(), &mut finishing, &mut |r| {
                heard.push(r)
            })
            .expect("published")
            .entries
            .remove(0);
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
            .pop()
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
            .is_empty());
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

    fn focused(audio: bool) -> Request {
        Request {
            focused: true,
            ..request(audio)
        }
    }

    /// «Descartar»: the child is killed rather than finished, the hidden
    /// file goes, nothing is published and the session is idle again, with
    /// a report of its own and no `Finished`.
    #[test]
    fn a_discard_removes_the_hidden_file_and_publishes_nothing() {
        let scratch = Scratch::new("discard");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |r| {
                heard.push(r)
            })
            .expect("started");
        let hidden = fake.started[0].out.clone();
        assert!(hidden.exists());
        session
            .discard(&mut fake, &mut |r| heard.push(r))
            .expect("discarded");
        assert_eq!(session.state(), State::Idle);
        assert!(session.active.is_none());
        assert_eq!(
            fake.calls,
            ["prepare", "start", "abandon"],
            "killed, not stopped"
        );
        assert!(!hidden.exists(), "the hidden file is gone");
        assert!(std::fs::read_dir(scratch.recordings())
            .expect("folder")
            .next()
            .is_none());
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle
            ]
        );
        assert!(heard.iter().any(|r| matches!(r, Report::Discarded)));
        assert!(!heard.iter().any(|r| matches!(r, Report::Finished(_))));
        // Nothing left to discard.
        assert!(matches!(
            session.discard(&mut fake, &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        // The next recording starts and publishes as usual.
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |_| {})
            .expect("started again");
        session
            .stop(&mut fake, &scratch.places(), &mut |_| {})
            .expect("published");
    }

    /// A hidden file that cannot be removed fails the discard with its own
    /// notice; the session is idle all the same.
    #[test]
    fn a_discard_that_cannot_remove_the_file_says_so() {
        /// A child whose output is a folder with something in it, which
        /// `remove_file` refuses.
        struct Stubborn;
        impl Recorder for Stubborn {
            fn probe(&mut self) -> Probe {
                Fake::default().probe()
            }
            fn focused_output(&mut self) -> Result<String, NiriError> {
                Err(NiriError::NoSocket)
            }
            fn prepare(
                &mut self,
                _pick: Option<&Path>,
            ) -> Result<crate::recorder::Prepared, RecordError> {
                Ok(crate::recorder::Prepared {
                    source: selenita_core::record::Source { node: 7, fd: None },
                    fd: None,
                })
            }
            fn start(
                &mut self,
                pipeline: &Pipeline,
                _fd: Option<OwnedFd>,
            ) -> Result<(), RecordError> {
                std::fs::create_dir_all(pipeline.out.join("inside"))
                    .map_err(|e| RecordError::Write(e.to_string()))
            }
            fn exited(&mut self) -> Option<Exit> {
                None
            }
            fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
                panic!("a discard never stops")
            }
            fn abandon(&mut self) {}
            fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
                Ok(None)
            }
            fn release(&mut self) {}
            fn join(
                &mut self,
                _list: &Path,
                _segments: &[PathBuf],
                _out: &Path,
            ) -> Result<(), RecordError> {
                Ok(())
            }
        }
        let scratch = Scratch::new("stubborn");
        let mut stubborn = Stubborn;
        let mut session = Session::new(stubborn.probe());
        let mut heard = Vec::new();
        session
            .start(
                &mut stubborn,
                &scratch.places(),
                &request(false),
                &mut |r| heard.push(r),
            )
            .expect("started");
        let error = session
            .discard(&mut stubborn, &mut |r| heard.push(r))
            .expect_err("the folder stays");
        assert!(matches!(error, RecordError::Discard(_)), "{error}");
        assert_eq!(
            error.message_es(),
            "No se ha podido borrar la grabación descartada."
        );
        assert_eq!(session.state(), State::Idle);
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Failed,
                State::Idle
            ]
        );
        assert!(!heard.iter().any(|r| matches!(r, Report::Discarded)));
    }

    /// A focused start asks niri first and leaves the request the chooser
    /// takes, so the portal casts that output without its dialog; the
    /// request is gone once the portal answered.
    #[test]
    fn a_focused_start_leaves_the_request_for_the_chooser() {
        let scratch = Scratch::new("focused");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &scratch.places(), &focused(false), &mut |_| {})
            .expect("started");
        assert_eq!(fake.calls, ["focused_output", "prepare", "start"]);
        assert_eq!(fake.chosen.as_deref(), Some("FAKE-1"));
        assert!(!scratch.pick_request().exists(), "consumed");
        assert_eq!(session.state(), State::Recording);
        session
            .stop(&mut fake, &scratch.places(), &mut |_| {})
            .expect("stopped");
    }

    /// When niri cannot name the focused output the start goes on without a
    /// request: the chooser's dialog asks, as for a plain start.
    #[test]
    fn without_niri_a_focused_start_leaves_no_request() {
        let scratch = Scratch::new("no-niri");
        let mut fake = Fake::default();
        fake.focused = None;
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &scratch.places(), &focused(false), &mut |_| {})
            .expect("started all the same");
        assert_eq!(fake.calls, ["focused_output", "prepare", "start"]);
        assert_eq!(fake.chosen, None, "no request reached the chooser");
        assert!(!scratch.pick_request().exists());
        assert_eq!(session.state(), State::Recording);
        session
            .stop(&mut fake, &scratch.places(), &mut |_| {})
            .expect("stopped");
        // Without a runtime folder there is nowhere to leave one either.
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let places = Places {
            pick_request: None,
            ..scratch.places()
        };
        session
            .start(&mut fake, &places, &focused(false), &mut |_| {})
            .expect("started");
        assert_eq!(fake.calls, ["prepare", "start"]);
        assert_eq!(fake.chosen, None);
    }

    /// A plain start never meets a request: one left over (fresh, from a
    /// start whose chooser never ran) is removed before the portal is
    /// asked, and one the portal's chooser never consumed is removed after.
    #[test]
    fn a_plain_start_shows_the_dialog_whatever_was_left_over() {
        let scratch = Scratch::new("leftover");
        selenita_core::pick::write_request(
            &scratch.pick_request(),
            "DP-9",
            std::time::SystemTime::now(),
        )
        .expect("left over");
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &scratch.places(), &request(false), &mut |_| {})
            .expect("started");
        assert_eq!(fake.chosen, None, "the leftover never answered");
        assert!(!scratch.pick_request().exists());
        session
            .stop(&mut fake, &scratch.places(), &mut |_| {})
            .expect("stopped");

        /// A portal whose chooser is not Selenita's: it never reads the
        /// request, which is there while it is asked.
        struct Deaf {
            saw: bool,
            pick: PathBuf,
        }
        impl Recorder for Deaf {
            fn probe(&mut self) -> Probe {
                Fake::default().probe()
            }
            fn focused_output(&mut self) -> Result<String, NiriError> {
                Ok("DP-3".to_owned())
            }
            fn prepare(
                &mut self,
                pick: Option<&Path>,
            ) -> Result<crate::recorder::Prepared, RecordError> {
                assert_eq!(pick, Some(self.pick.as_path()));
                self.saw = self.pick.exists();
                Ok(crate::recorder::Prepared {
                    source: selenita_core::record::Source { node: 7, fd: None },
                    fd: None,
                })
            }
            fn start(
                &mut self,
                pipeline: &Pipeline,
                _fd: Option<OwnedFd>,
            ) -> Result<(), RecordError> {
                std::fs::write(&pipeline.out, b"x").map_err(|e| RecordError::Write(e.to_string()))
            }
            fn exited(&mut self) -> Option<Exit> {
                None
            }
            fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
                Ok(())
            }
            fn abandon(&mut self) {}
            fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
                Ok(None)
            }
            fn release(&mut self) {}
            fn join(
                &mut self,
                _list: &Path,
                _segments: &[PathBuf],
                _out: &Path,
            ) -> Result<(), RecordError> {
                Ok(())
            }
        }
        let mut deaf = Deaf {
            saw: false,
            pick: scratch.pick_request(),
        };
        let mut session = Session::new(deaf.probe());
        session
            .start(&mut deaf, &scratch.places(), &focused(false), &mut |_| {})
            .expect("started");
        assert!(deaf.saw, "the request was there while the portal was asked");
        assert!(!scratch.pick_request().exists(), "and gone after");
    }

    /// The files in `folder`, sorted.
    fn names_in(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
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

    fn finished(reports: &[Report]) -> Vec<selenita_core::Entry> {
        reports
            .iter()
            .filter_map(|report| match report {
                Report::Finished(entry) => Some(entry.clone()),
                _ => None,
            })
            .collect()
    }

    /// A paused recording with one segment: the pause finishes the child
    /// and keeps the session, the stop from paused publishes that segment
    /// as it is (no join) and only then releases the session.
    #[test]
    fn a_pause_then_a_stop_publishes_the_one_segment_without_a_join() {
        let scratch = Scratch::new("pause-one");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |r| heard.push(r))
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |r| heard.push(r))
            .expect("paused");
        assert_eq!(session.state(), State::Paused);
        assert_eq!(
            fake.calls,
            ["prepare", "start", "finish"],
            "the session stays"
        );
        assert!(heard.iter().any(|r| matches!(r, Report::Paused(_))));
        let done = session
            .stop(&mut fake, &places, &mut |r| heard.push(r))
            .expect("stopped");
        assert!(done.notice.is_none());
        assert_eq!(done.entries.len(), 1);
        assert_eq!(fake.calls, ["prepare", "start", "finish", "release"]);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished"
        );
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Paused,
                State::Stopping,
                State::Idle
            ]
        );
        let names = names_in(&scratch.recordings());
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(!names[0].starts_with('.'));
    }

    /// Two segments: the resume reopens the session's remote (no portal
    /// dialog), the next child writes the second segment, the stop from
    /// paused joins both into one file, one entry and nothing hidden left,
    /// the list included. The clock's origin moves back by the time
    /// recorded.
    #[test]
    fn two_segments_are_joined_into_one_recording() {
        let scratch = Scratch::new("pause-two");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(true), &mut |r| heard.push(r))
            .expect("started");
        std::thread::sleep(Duration::from_millis(20));
        session
            .pause(&mut fake, &places, &mut |r| heard.push(r))
            .expect("paused");
        std::thread::sleep(Duration::from_millis(30));
        let resumed_at = std::time::SystemTime::now();
        session
            .resume(&mut fake, &places, &mut |r| heard.push(r))
            .expect("resumed");
        assert_eq!(session.state(), State::Recording);
        session
            .pause(&mut fake, &places, &mut |r| heard.push(r))
            .expect("paused again");
        let done = session
            .stop(&mut fake, &places, &mut |r| heard.push(r))
            .expect("stopped");
        assert!(done.notice.is_none());
        assert_eq!(
            fake.calls,
            ["prepare", "start", "finish", "reopen", "start", "finish", "release", "join"]
        );
        assert_eq!(done.entries.len(), 1);
        let entry = &done.entries[0];
        assert_eq!(
            std::fs::read(&entry.path).expect("file"),
            b"fake mp4 finishedfake mp4 finished"
        );
        assert_eq!(entry.size, 34);
        assert_eq!(finished(&heard), done.entries);
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
        assert!(
            names_in(&scratch.0.join("run").join("selenita"))
                .iter()
                .all(|name| !name.starts_with("join-")),
            "the list is gone"
        );
        // Both segments came from the same pipeline, sound included, each
        // into a file of its own beside the hidden one.
        assert_eq!(fake.started.len(), 2);
        assert!(fake.started.iter().all(|pipeline| pipeline.audio));
        assert_eq!(fake.started[0].source, fake.started[1].source);
        assert!(fake.started[0]
            .out
            .to_string_lossy()
            .ends_with("-part1.mp4"));
        assert!(fake.started[1]
            .out
            .to_string_lossy()
            .ends_with("-part2.mp4"));
        // The second origin is the resume less what was recorded before it.
        let origins: Vec<_> = heard
            .iter()
            .filter_map(|r| match r {
                Report::Started(when) => Some(*when),
                _ => None,
            })
            .collect();
        assert_eq!(origins.len(), 2);
        assert!(origins[1] < resumed_at, "moved back by the time recorded");
        assert!(origins[1] > origins[0], "the pause is not counted");
        let paused: Vec<_> = heard
            .iter()
            .filter_map(|r| match r {
                Report::Paused(recorded) => Some(*recorded),
                _ => None,
            })
            .collect();
        assert_eq!(paused.len(), 2);
        assert!(paused[0] >= Duration::from_millis(20));
        assert!(paused[1] >= paused[0]);
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Paused,
                State::Recording,
                State::Paused,
                State::Stopping,
                State::Idle
            ]
        );
    }

    /// Three segments, the stop from recording: the last child is finished
    /// first, then the three are joined in order.
    #[test]
    fn three_segments_stopped_while_recording_are_joined_in_order() {
        let scratch = Scratch::new("pause-three");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        for _ in 0..2 {
            session
                .pause(&mut fake, &places, &mut |_| {})
                .expect("paused");
            session
                .resume(&mut fake, &places, &mut |_| {})
                .expect("resumed");
        }
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("stopped");
        assert_eq!(done.entries.len(), 1);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished".repeat(3)
        );
        assert_eq!(fake.started.len(), 3);
        assert!(fake.started[2]
            .out
            .to_string_lossy()
            .ends_with("-part3.mp4"));
        assert_eq!(fake.calls.last(), Some(&"join"));
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
    }

    /// «Descartar» while paused: every segment goes, nothing is published.
    #[test]
    fn a_discard_while_paused_removes_every_segment() {
        let scratch = Scratch::new("pause-discard");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |r| heard.push(r))
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        assert_eq!(
            names_in(&scratch.recordings()).len(),
            2,
            "two hidden segments"
        );
        session
            .discard(&mut fake, &mut |r| heard.push(r))
            .expect("discarded");
        assert_eq!(session.state(), State::Idle);
        assert!(names_in(&scratch.recordings()).is_empty());
        assert!(heard.iter().any(|r| matches!(r, Report::Discarded)));
        assert!(finished(&heard).is_empty());
        assert_eq!(fake.calls.last(), Some(&"abandon"));
    }

    /// A quit while paused finishes and publishes like a quit while
    /// recording: the two segments joined, one entry for the history.
    #[test]
    fn a_quit_while_paused_publishes_the_recording() {
        let scratch = Scratch::new("pause-quit");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        let entries = session.quit(&mut fake, &places, &mut |_| {});
        assert_eq!(entries.len(), 1);
        assert_eq!(
            std::fs::read(&entries[0].path).expect("file"),
            b"fake mp4 finished".repeat(2)
        );
        assert_eq!(session.state(), State::Idle);
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
    }

    /// A resume whose session is gone: the recording ends with what was
    /// recorded before the pause published, and says so.
    #[test]
    fn a_failed_resume_publishes_what_was_recorded() {
        let scratch = Scratch::new("pause-resume-fails");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |r| heard.push(r))
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |r| heard.push(r))
            .expect("paused");
        fake.reopen_fails = true;
        let error = session
            .resume(&mut fake, &places, &mut |r| heard.push(r))
            .expect_err("the notice");
        assert!(matches!(error, RecordError::Resume(_)), "{error}");
        assert!(error.message_es().contains("pausa"));
        assert_eq!(session.state(), State::Idle);
        let entries = finished(&heard);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            std::fs::read(&entries[0].path).expect("file"),
            b"fake mp4 finished"
        );
        assert_eq!(
            names_in(&scratch.recordings()).len(),
            1,
            "nothing hidden left"
        );
        assert_eq!(fake.started.len(), 1, "no second child");
        assert_eq!(
            states(&heard)[2..],
            [State::Paused, State::Stopping, State::Idle]
        );
    }

    /// A segment lost after a resume (its child died): the segments before
    /// it are published and the notice says the rest was lost.
    #[test]
    fn a_child_that_dies_after_a_resume_keeps_the_earlier_segments() {
        let scratch = Scratch::new("pause-dies");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let exit = Exit {
            code: Some(1),
            stderr: "node gone".to_owned(),
        };
        let done = session
            .ended(exit, &places, &mut fake, &mut |r| heard.push(r))
            .expect("published");
        assert!(matches!(done.notice, Some(RecordError::Partial(_))));
        assert_eq!(done.entries.len(), 1);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished"
        );
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
        assert_eq!(session.state(), State::Idle);
    }

    /// A join that fails loses nothing: each segment is published on its
    /// own as `<name> (n).mp4`, each with its entry, and the notice says so.
    #[test]
    fn a_failed_join_publishes_each_segment_on_its_own() {
        let scratch = Scratch::new("pause-join-fails");
        let places = scratch.places();
        let mut fake = Fake::default();
        fake.join_fails = true;
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let done = session
            .stop(&mut fake, &places, &mut |r| heard.push(r))
            .expect("published");
        let notice = done.notice.expect("a notice");
        assert!(matches!(notice, RecordError::Join(_)), "{notice}");
        assert!(notice.message_es().contains("por separado"));
        assert_eq!(done.entries.len(), 2);
        assert_eq!(finished(&heard).len(), 2);
        let names = names_in(&scratch.recordings());
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names[0].starts_with("Recording ") && names[0].ends_with(" (1).mp4"));
        assert!(names[1].ends_with(" (2).mp4"));
        for entry in &done.entries {
            assert_eq!(
                std::fs::read(&entry.path).expect("file"),
                b"fake mp4 finished"
            );
        }
        assert_eq!(session.state(), State::Idle);
    }

    /// Without a runtime folder there is nowhere private for the list: the
    /// segments are published one by one, as for a failed join.
    #[test]
    fn without_a_runtime_folder_the_segments_are_published_one_by_one() {
        let scratch = Scratch::new("pause-no-runtime");
        let places = Places {
            runtime: None,
            ..scratch.places()
        };
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("published");
        assert!(matches!(done.notice, Some(RecordError::Join(_))));
        assert_eq!(done.entries.len(), 2);
        assert!(!fake.calls.contains(&"join"));
    }

    /// A pause only while recording, a resume only while paused.
    #[test]
    fn a_pause_and_a_resume_are_refused_out_of_turn() {
        let scratch = Scratch::new("pause-refused");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        assert!(matches!(
            session.pause(&mut fake, &places, &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        assert!(matches!(
            session.resume(&mut fake, &places, &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        assert!(matches!(
            session.resume(&mut fake, &places, &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        assert!(matches!(
            session.pause(&mut fake, &places, &mut |_| {}),
            Err(RecordError::NotRecording)
        ));
        assert!(matches!(
            session.start(&mut fake, &places, &request(false), &mut |_| {}),
            Err(RecordError::Busy)
        ));
        // Out of turn through the worker: nothing to say.
        let mut heard = Vec::new();
        let mut places = places;
        assert_eq!(
            session.turn(
                &mut fake,
                &mut places,
                None,
                Some(super::Job::Pause),
                &mut |r| { heard.push(r) }
            ),
            super::Flow::Go
        );
        assert!(heard.is_empty());
        assert_eq!(session.state(), State::Paused);
    }

    /// The stop file (`selenita --stop` with no instance on the bus) is
    /// taken while paused too, and stops the recording.
    #[test]
    fn the_stop_file_stops_a_paused_recording() {
        let scratch = Scratch::new("pause-stop-file");
        let mut places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        selenita_core::record::request_stop(&places.stop_file).expect("touched");
        session.turn(&mut fake, &mut places, None, None, &mut |r| heard.push(r));
        assert_eq!(session.state(), State::Idle);
        assert_eq!(finished(&heard).len(), 1);
    }

    /// The child ended on its own in the same turn as a queued
    /// «Descartar»: the discard wins, nothing is published and the file
    /// goes. Without the discard the same child's file is published; a
    /// stop queued with it is dropped, as it asked for what just happened.
    #[test]
    fn a_discard_wins_over_a_child_that_ended_in_the_same_turn() {
        let scratch = Scratch::new("discard-wins");
        let mut places = scratch.places();
        let mut finishing = Finishing;
        let mut session = Session::new(finishing.probe());
        let mut heard = Vec::new();
        session
            .start(&mut finishing, &places, &request(false), &mut |_| {})
            .expect("started");
        let flow = session.turn(
            &mut finishing,
            &mut places,
            None,
            Some(super::Job::Discard),
            &mut |r| heard.push(r),
        );
        assert_eq!(flow, super::Flow::Go);
        assert!(heard.iter().any(|r| matches!(r, Report::Discarded)));
        assert!(finished(&heard).is_empty(), "nothing published");
        assert!(
            names_in(&scratch.recordings()).is_empty(),
            "the file is gone"
        );
        assert_eq!(session.state(), State::Idle);

        let mut heard = Vec::new();
        session
            .start(&mut finishing, &places, &request(false), &mut |_| {})
            .expect("started");
        session.turn(
            &mut finishing,
            &mut places,
            None,
            Some(super::Job::Stop),
            &mut |r| heard.push(r),
        );
        assert_eq!(
            finished(&heard).len(),
            1,
            "the ended child is published once"
        );
        assert!(!heard.iter().any(|r| matches!(r, Report::Failed(_))));
        assert_eq!(session.state(), State::Idle);
    }

    /// The history file's lines (one entry each).
    fn history_lines(scratch: &Scratch) -> usize {
        std::fs::read_to_string(scratch.0.join("history"))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    /// Round 1, item 1: the stop says `stopping` before it finishes the last
    /// segment, so the window shows the finish (and the join) as such.
    #[test]
    fn the_stop_says_stopping_before_the_last_segment_finishes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let scratch = Scratch::new("stopping-first");
        let places = scratch.places();
        let mut fake = Fake::default();
        let said = Arc::new(AtomicBool::new(false));
        let said_at_finish = Arc::new(AtomicBool::new(false));
        let (seen, noted) = (Arc::clone(&said), Arc::clone(&said_at_finish));
        fake.on_finish = Some(Box::new(move || {
            noted.store(seen.load(Ordering::SeqCst), Ordering::SeqCst);
        }));
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |r| heard.push(r))
            .expect("started");
        session
            .stop(&mut fake, &places, &mut |r| {
                if matches!(r, Report::State(State::Stopping)) {
                    said.store(true, Ordering::SeqCst);
                }
                heard.push(r);
            })
            .expect("stopped");
        assert!(
            said_at_finish.load(Ordering::SeqCst),
            "stopping was said before the finish"
        );
        assert_eq!(
            states(&heard),
            [
                State::Preparing,
                State::Recording,
                State::Stopping,
                State::Idle
            ],
            "stopping is said once"
        );
    }

    /// A «Descartar» that arrives while the stop finishes the last segment
    /// (the worker busy, the signal set) wins: nothing is published.
    #[test]
    fn a_discard_asked_during_the_finish_drops_everything() {
        let scratch = Scratch::new("discard-finish");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .signals
            .discard
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let done = session
            .stop(&mut fake, &places, &mut |r| heard.push(r))
            .expect("stopped");
        assert!(done.entries.is_empty() && done.notice.is_none());
        assert!(heard.iter().any(|r| matches!(r, Report::Discarded)));
        assert!(finished(&heard).is_empty());
        assert!(names_in(&scratch.recordings()).is_empty());
        assert_eq!(session.state(), State::Idle);
    }

    /// … and during the join of two segments: the joined file and both
    /// segments go, nothing is published.
    #[test]
    fn a_discard_asked_during_the_join_drops_everything() {
        let scratch = Scratch::new("discard-join");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let signals = std::sync::Arc::clone(&session.signals);
        fake.on_join = Some(Box::new(move || {
            signals
                .discard
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }));
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let done = session
            .stop(&mut fake, &places, &mut |r| heard.push(r))
            .expect("stopped");
        assert_eq!(fake.calls.last(), Some(&"join"), "the join ran");
        assert!(done.entries.is_empty());
        assert!(heard.iter().any(|r| matches!(r, Report::Discarded)));
        assert!(names_in(&scratch.recordings()).is_empty());
        assert_eq!(session.state(), State::Idle);
    }

    /// A second press on the stop (a toggle asked while the stop ran)
    /// never starts another recording; a toggle asked afterwards does.
    #[test]
    fn a_toggle_asked_while_the_stop_ran_is_dropped() {
        let scratch = Scratch::new("toggle-stopping");
        let mut places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        let pressed_during = std::time::Instant::now();
        let mut heard = Vec::new();
        session.turn(
            &mut fake,
            &mut places,
            None,
            Some(super::Job::Stop),
            &mut |r| heard.push(r),
        );
        assert_eq!(session.state(), State::Idle);
        // `missing` makes a start that is not dropped say so, so the test
        // never records into the person's own folder.
        let mut after = Vec::new();
        session.turn(
            &mut fake,
            &mut places,
            Some("mp4mux"),
            Some(super::Job::Toggle(request(false), pressed_during)),
            &mut |r| after.push(r),
        );
        assert!(after.is_empty(), "dropped without a word: {after:?}");
        assert_eq!(session.state(), State::Idle);
        session.turn(
            &mut fake,
            &mut places,
            Some("mp4mux"),
            Some(super::Job::Toggle(
                request(false),
                std::time::Instant::now(),
            )),
            &mut |r| after.push(r),
        );
        assert!(
            after.iter().any(|r| matches!(r, Report::Failed(_))),
            "a later toggle is a start"
        );
    }

    /// Round 2: a child that never reaches its EOS (its node never sent a
    /// buffer) is asked to finish, killed once the grace went by with its
    /// file still empty, and its segment dropped: a pause right after a
    /// resume stays paused with the earlier segment, which the next stop
    /// publishes; a stop right after a resume does the same.
    #[test]
    fn a_child_that_never_reached_its_eos_is_killed_after_the_grace_and_dropped() {
        let scratch = Scratch::new("pause-empty");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        fake.never_exits = true;
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let mut heard = Vec::new();
        session
            .pause(&mut fake, &places, &mut |r| heard.push(r))
            .expect("paused again");
        assert_eq!(session.state(), State::Paused);
        assert_eq!(
            fake.calls,
            ["prepare", "start", "finish", "reopen", "start", "finish", "kill"],
            "asked to finish first, killed after the grace"
        );
        assert_eq!(
            names_in(&scratch.recordings()).len(),
            1,
            "the empty segment went"
        );
        assert!(heard.iter().any(|r| matches!(r, Report::Paused(_))));
        // Resumed and stopped before the first buffer again.
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("published");
        assert!(done.notice.is_none(), "nothing was lost");
        assert_eq!(done.entries.len(), 1);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished"
        );
        assert!(!fake.calls.contains(&"join"));
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
    }

    /// The same as the first segment: the stop fails as a node that never
    /// streamed, with no file left.
    #[test]
    fn a_first_segment_that_never_reached_its_eos_fails_as_no_signal() {
        let scratch = Scratch::new("stop-never");
        let places = scratch.places();
        let mut fake = Fake::default();
        fake.never_exits = true;
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        let error = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect_err("nothing recorded");
        assert!(matches!(error, RecordError::NoSignal), "{error}");
        assert_eq!(session.state(), State::Idle);
        assert!(names_in(&scratch.recordings()).is_empty());
    }

    /// Round 2: a segment the muxer writes only at its EOS (empty on disk
    /// until the finish, as a quiet screen's first seconds can be) is kept
    /// by a pause and by a stop, a short first segment included: the size
    /// before the finish never decides.
    #[test]
    fn a_segment_written_only_at_its_eos_is_kept_by_a_pause_and_a_stop() {
        let scratch = Scratch::new("eos-only");
        let places = scratch.places();
        let mut fake = Fake::default();
        fake.writes_at_eos = true;
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        assert_eq!(session.state(), State::Paused);
        assert!(!fake.calls.contains(&"kill"));
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("published");
        assert!(done.notice.is_none());
        assert_eq!(done.entries.len(), 1);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished".repeat(2),
            "both segments kept and joined"
        );

        // A short first segment stopped at once: published, as before.
        let scratch = Scratch::new("eos-only-short");
        let places = scratch.places();
        let mut fake = Fake::default();
        fake.writes_at_eos = true;
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("published");
        assert_eq!(done.entries.len(), 1);
        assert_eq!(
            std::fs::read(&done.entries[0].path).expect("file"),
            b"fake mp4 finished"
        );
    }

    /// A pause whose child fails to finish its segment, with earlier
    /// segments: that segment is dropped, the recording stays paused and
    /// the notice says a stretch was lost.
    #[test]
    fn a_pause_whose_child_fails_stays_paused_with_the_earlier_segments() {
        let scratch = Scratch::new("pause-lost");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        fake.finish_fails = true;
        let error = session
            .pause(&mut fake, &places, &mut |_| {})
            .expect_err("a notice");
        assert!(matches!(error, RecordError::SegmentLost(_)), "{error}");
        assert_eq!(session.state(), State::Paused);
        assert_eq!(names_in(&scratch.recordings()).len(), 1);
        fake.finish_fails = false;
        let done = session
            .stop(&mut fake, &places, &mut |_| {})
            .expect("published");
        assert_eq!(done.entries.len(), 1);
    }

    /// Round 1, item 3: once the window has quit, entries go to the history
    /// file: the quit's own (a paused recording joined), one from a child
    /// that ended on its own in the quit's turn, and one from a stop whose
    /// join finished after the window closed.
    #[test]
    fn entries_after_the_quit_reach_the_history_file() {
        let scratch = Scratch::new("quit-history");
        let mut places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        let mut heard = Vec::new();
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        let (done, finished_signal) = std::sync::mpsc::channel();
        let flow = session.turn(
            &mut fake,
            &mut places,
            None,
            Some(super::Job::Quit(done)),
            &mut |r| heard.push(r),
        );
        assert_eq!(flow, super::Flow::End);
        assert!(finished_signal.try_recv().is_ok());
        assert!(finished(&heard).is_empty(), "nothing told to a gone window");
        assert_eq!(history_lines(&scratch), 1);

        // A child that ended on its own in the quit's turn.
        let scratch = Scratch::new("quit-history-ended");
        let mut places = scratch.places();
        let mut finishing = Finishing;
        let mut session = Session::new(finishing.probe());
        session
            .start(&mut finishing, &places, &request(false), &mut |_| {})
            .expect("started");
        let (done, _finished) = std::sync::mpsc::channel();
        session.turn(
            &mut finishing,
            &mut places,
            None,
            Some(super::Job::Quit(done)),
            &mut |_| {},
        );
        assert_eq!(history_lines(&scratch), 1, "the ended child's entry");

        // A stop under way when the window closed (`record::quit` sets the
        // signal before its job arrives).
        let scratch = Scratch::new("quit-history-stop");
        let mut places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .signals
            .quitting
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let mut told = Vec::new();
        session.turn(
            &mut fake,
            &mut places,
            None,
            Some(super::Job::Stop),
            &mut |r| told.push(r),
        );
        assert!(finished(&told).is_empty());
        assert_eq!(history_lines(&scratch), 1);
    }

    /// Round 1, item 4: files that cannot be published (here the folder
    /// refuses every new name, so the join's publish and each segment's
    /// fail) are never removed: they stay where they are and the notice
    /// names them.
    #[test]
    fn files_that_cannot_be_published_stay_and_are_named() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("unpublished");
        let places = scratch.places();
        let mut fake = Fake::default();
        let mut session = Session::new(fake.probe());
        session
            .start(&mut fake, &places, &request(false), &mut |_| {})
            .expect("started");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        session
            .resume(&mut fake, &places, &mut |_| {})
            .expect("resumed");
        session
            .pause(&mut fake, &places, &mut |_| {})
            .expect("paused");
        let folder = scratch.recordings();
        let mode = |bits| std::fs::Permissions::from_mode(bits);
        std::fs::set_permissions(&folder, mode(0o500)).expect("read-only");
        let result = session.stop(&mut fake, &places, &mut |_| {});
        std::fs::set_permissions(&folder, mode(0o700)).expect("writable");
        let error = result.expect_err("nothing published");
        let RecordError::Unpublished { kept, .. } = &error else {
            panic!("expected the kept files, got {error}");
        };
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|path| path.exists()), "{kept:?}");
        assert!(error.message_es().contains("sigue en"));
        assert_eq!(session.state(), State::Idle);
    }

    #[test]
    fn every_error_has_spanish_words() {
        for error in [
            RecordError::Busy,
            RecordError::NotRecording,
            RecordError::NoFile,
            RecordError::NoSignal,
            RecordError::Missing("mp4mux".to_owned()),
            RecordError::Discard("/videos/.selenita-1-1.mp4: busy".to_owned()),
            RecordError::Join("ffmpeg: exit 1".to_owned()),
            RecordError::Resume("the session is gone".to_owned()),
            RecordError::Partial(Box::new(RecordError::NoSignal)),
            RecordError::SegmentLost(Box::new(RecordError::NoFile)),
            RecordError::Unpublished {
                cause: Box::new(RecordError::NoFile),
                kept: vec![PathBuf::from("/videos/.selenita-1-1-part1.mp4")],
            },
            RecordError::Portal(crate::portal::PortalError::Cancelled),
        ] {
            assert!(!error.message_es().is_empty());
        }
        assert!(RecordError::Missing("mp4mux".to_owned())
            .message_es()
            .contains("«mp4mux»"));
    }
}
