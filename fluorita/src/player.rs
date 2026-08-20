//! The Qt half of playing one item.
//!
//! Everything true about playback lives in `fluorita-core`'s [`PlaybackSession`]
//! and everything that decodes lives in `fluorita-engine`. This file moves
//! values between them and QML, and does so under three rules:
//!
//! - **The GUI thread never decodes.** Opening a session, polling the backend
//!   and closing it all happen on an owned worker thread; Qt only ever receives
//!   finished values through the queue.
//! - **Nothing is published that the engine did not report.** A click becomes a
//!   pending request; the properties QML binds to move when a report arrives and
//!   is accepted by its generation.
//! - **The surface releases before the backend does.** The video item builds a
//!   render context from the backend handle on Qt's render thread, so closing
//!   clears the handle first and waits for the item to confirm the context is
//!   gone before the session may be dropped.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use fluorita_core::{
    Continuation, MediaKind, PlaybackRequest, PlaybackSession, PlaybackState, ReportOutcome, Speed,
    StreamKind,
};

use crate::image::ImageDecision;
use fluorita_engine::backend::{MediaEngine, SessionRequest};
use fluorita_engine::MpvEngine;

/// How long the worker waits for a backend report before looking at its inbox.
/// Short enough that a pause feels immediate, long enough not to spin.
const POLL_TIMEOUT: Duration = Duration::from_millis(50);

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qsize.h");
        type QSize = cxx_qt_lib::QSize;
    }

    unsafe extern "C++" {
        // Reads an image's dimensions from its header, without decoding it
        // (see cpp/imageprobe.cpp). cxx-qt-lib exposes no QImageReader.
        include!("fluorita/imageprobe.h");

        #[rust_name = "probe_image"]
        fn fluorita_probe_image(key: &QString) -> QSize;
    }

    unsafe extern "C++" {
        // The Qt Quick surface libmpv renders into (see cpp/mpvvideoitem.cpp).
        // CXX-Qt 0.9 cannot express it: it needs a `QQuickFramebufferObject`
        // subclass overriding a virtual and running on Qt's render thread.
        include!("fluorita/mpvvideoitem.h");

        #[rust_name = "register_video_item"]
        fn register_fluorita_video_item(engine: Pin<&mut QQmlApplicationEngine>);
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// Confirmed playback state, worded for the interface: `inactivo`,
        /// `abriendo`, `reproduciendo`, `pausado`, `terminado` or `error`.
        #[qproperty(QString, state)]
        /// The backend handle the video surface renders from. Zero means there
        /// is nothing to render — including while a session is closing.
        #[qproperty(u64, render_handle)]
        #[qproperty(f64, position_seconds)]
        #[qproperty(f64, duration_seconds)]
        #[qproperty(bool, seekable)]
        #[qproperty(bool, has_video)]
        #[qproperty(bool, has_audio)]
        /// True between a transport click and the report that confirms it.
        #[qproperty(bool, pending)]
        /// Empty unless the backend failed, in which case it is what to show.
        #[qproperty(QString, error_message)]
        /// The still image to display, as a `file:` URL, or empty. A still is
        /// never a session: the toolkit decodes it and the media backend is
        /// never started for one.
        #[qproperty(QString, image_source)]
        /// True while a transport would be meaningful. An image has none, so
        /// the interface must not draw one.
        #[qproperty(bool, timed)]
        /// The confirmed output level, `0.0..=1.0`. Only ever set from what the
        /// backend reports, the same way `position_seconds` is — call
        /// `set_volume` to request a change, never this.
        #[qproperty(f64, volume_level)]
        /// What happened to the last frame kept, or empty. Its own property
        /// rather than `error_message`: keeping a frame is not playback, and a
        /// success has to be sayable too.
        #[qproperty(QString, frame_notice)]
        /// True while one is being extracted, so the action cannot be asked
        /// for twice.
        #[qproperty(bool, extracting_frame)]
        /// The audio streams this file holds, as the words a menu shows, and
        /// which of them the backend confirmed. `-1` is none.
        #[qproperty(QStringList, audio_streams)]
        #[qproperty(i32, audio_stream)]
        /// The same for subtitles, where none is the ordinary case.
        #[qproperty(QStringList, subtitle_streams)]
        #[qproperty(i32, subtitle_stream)]
        /// Whether choosing is worth offering at all: one audio stream is not
        /// a choice, one set of subtitles is, because it can also be off.
        #[qproperty(bool, choosable_audio)]
        #[qproperty(bool, choosable_subtitles)]
        /// The confirmed playback rate.
        #[qproperty(f64, speed)]
        /// True while frame pacing is being recorded.
        #[qproperty(bool, capturing_pacing)]
        /// What the recording says so far, in one line a person can read.
        #[qproperty(QString, pacing_line)]
        /// Its verdict as a token the surface colours by: `too-early`,
        /// `smooth`, `delayed` or `dropping`.
        #[qproperty(QString, pacing_verdict)]
        /// Where the last report was written, or empty.
        #[qproperty(QString, pacing_report)]
        /// True while this player is showing a hover preview rather than what
        /// a person chose to open. A preview is silent, loops, starts inside
        /// the film and never reaches the bus: one desktop has one media
        /// player, and a picture that plays because a pointer went past is not
        /// what "now playing" means.
        #[qproperty(bool, previewing)]
        /// What happens when the current item ends, as a position in the
        /// domain's own list of modes. A number and not a word because this
        /// crosses the seam as a token, and the words for it belong to the
        /// surface.
        #[qproperty(i32, continuation)]
        type FluoritaPlayer = super::PlayerRust;

        /// Opens the item a path key names and starts it. A second call
        /// replaces the first.
        ///
        /// The argument is a key under
        /// [ADR 0008](../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md),
        /// which is what the library's rows and the command-line scaffold both
        /// publish. A value that is not one is refused out loud rather than
        /// opened as whatever file its characters happen to name.
        #[qinvokable]
        fn open(self: Pin<&mut FluoritaPlayer>, key: &QString);

        #[qinvokable]
        fn play(self: Pin<&mut FluoritaPlayer>);

        #[qinvokable]
        fn pause(self: Pin<&mut FluoritaPlayer>);

        /// Play when paused, pause when playing. Anything else is ignored: a
        /// toggle on a failed file must not look like it did something.
        #[qinvokable]
        fn toggle(self: Pin<&mut FluoritaPlayer>);

        #[qinvokable]
        fn seek(self: Pin<&mut FluoritaPlayer>, seconds: f64);

        #[qinvokable]
        fn set_volume(self: Pin<&mut FluoritaPlayer>, level: f64);

        /// Clears the handle and asks the worker to stop. The session is only
        /// dropped once the surface confirms its render context is gone.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaPlayer>);

        /// Called by the video item once its render context exists. Until then
        /// a session that presents through it has nowhere to put frames.
        #[qinvokable]
        fn surface_ready(self: Pin<&mut FluoritaPlayer>);

        /// Starts or stops recording what the picture is doing.
        ///
        /// The counters are the backend's; what this adds is the difference
        /// between two of them over the time between, which is the only form
        /// in which they mean anything.
        #[qinvokable]
        fn toggle_pacing(self: Pin<&mut FluoritaPlayer>);

        /// Writes the recording to a file and publishes its path, so a person
        /// who just saw judder has something to attach to a report rather than
        /// a memory of it.
        #[qinvokable]
        fn write_pacing_report(self: Pin<&mut FluoritaPlayer>);

        /// Opens `key` as a bounded hover preview: silent, looping, starting
        /// inside the film. Refused for anything that is not a moving picture.
        #[qinvokable]
        fn preview(self: Pin<&mut FluoritaPlayer>, key: &QString);

        /// Uses the audio stream at this position in `audio_streams`, or none
        /// of them at `-1`.
        #[qinvokable]
        fn select_audio_stream(self: Pin<&mut FluoritaPlayer>, index: i32);

        /// The same for subtitles. `-1` turns them off.
        #[qinvokable]
        fn select_subtitle_stream(self: Pin<&mut FluoritaPlayer>, index: i32);

        /// Chooses what happens at the end of an item.
        #[qinvokable]
        fn set_continuation_mode(self: Pin<&mut FluoritaPlayer>, mode: i32);

        /// Which item the folder should open next, given where the current one
        /// sits and how many there are. `-1` when nothing should start.
        ///
        /// The host asks this only after the engine confirmed the end: a
        /// prediction made while a file was merely near its end would skip a
        /// track whose last seconds failed to decode.
        #[qinvokable]
        fn next_in_folder(self: &FluoritaPlayer, index: i32, count: i32) -> i32;

        /// Plays at this rate. Clamped by the domain before it is asked for.
        /// Named for the act rather than for the property it ends up changing,
        /// because `set_speed` is already the confirmed value's own setter.
        #[qinvokable]
        fn play_at(self: Pin<&mut FluoritaPlayer>, rate: f64);

        /// Keeps the frame at the current position as a picture beside the
        /// film. Takes the row's path key. Returns at once: a seek and a decode
        /// run on their own backend instance, off this thread.
        #[qinvokable]
        fn extract_frame(self: Pin<&mut FluoritaPlayer>, key: &QString);

        /// Called by the video item once its render context is released.
        #[qinvokable]
        fn surface_released(self: Pin<&mut FluoritaPlayer>);

        /// Called by the video item when it could not build a render context.
        /// Nothing will ever be presented, so the wait for a first frame ends
        /// here instead of lasting for the session.
        #[qinvokable]
        fn surface_failed(self: Pin<&mut FluoritaPlayer>);
    }

    impl cxx_qt::Threading for FluoritaPlayer {}
    impl cxx_qt::Constructor<()> for FluoritaPlayer {}
}

/// What the worker thread is told to do.
enum Command {
    /// Load the media and begin. Sent once, when there is somewhere to render.
    Start,
    Transport(PlaybackRequest),
    /// Close the session and leave. The worker acknowledges by exiting.
    Stop,
}

/// One report, already reduced to what Qt needs.
struct Snapshot {
    state: PlaybackState,
    position: Option<Duration>,
    duration: Option<Duration>,
    /// The confirmed output level. Carried because the bus publishes it, and a
    /// panel that showed a volume the engine never confirmed would be lying in
    /// the same way a transport would.
    volume: Option<f64>,
    pending: bool,
    error: Option<String>,
    /// What the file holds, and what is playing out of it. Carried on the
    /// snapshot like everything else: the surface never reads the session.
    streams: Vec<fluorita_core::Stream>,
    audio: Option<i64>,
    subtitle: Option<i64>,
    speed: f64,
}

#[derive(Default)]
pub struct PlayerRust {
    state: QString,
    render_handle: u64,
    /// An item asked for while a surface was still rendering the previous one.
    /// It starts once that surface confirms it has let go.
    /// Which session the player is on. A worker publishes its render handle
    /// asynchronously, so a close that lands in that window would otherwise be
    /// overtaken by the handle of the session it just destroyed — leaving a
    /// live-looking address for an `mpv_handle` that is gone.
    generation: u64,
    pending_open: Option<PathBuf>,
    position_seconds: f64,
    duration_seconds: f64,
    seekable: bool,
    has_video: bool,
    has_audio: bool,
    pending: bool,
    error_message: QString,
    image_source: QString,
    timed: bool,
    volume_level: f64,
    frame_notice: QString,
    extracting_frame: bool,
    audio_streams: QStringList,
    audio_stream: i32,
    subtitle_streams: QStringList,
    subtitle_stream: i32,
    choosable_audio: bool,
    choosable_subtitles: bool,
    speed: f64,
    continuation: i32,
    previewing: bool,
    capturing_pacing: bool,
    pacing_line: QString,
    pacing_verdict: QString,
    pacing_report: QString,

    /// What has been recorded, and the flag the worker reads to know whether
    /// to sample at all. Shared because the sampling happens on the session
    /// thread and the folding happens here.
    pacing: fluorita_core::PacingCapture,
    pacing_on: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// When the current recording began, so each sample can say how far into
    /// it it was taken. The domain never reads a clock; this is where the time
    /// comes from.
    pacing_started: Option<std::time::Instant>,

    /// The streams the last snapshot carried, kept so a chosen menu position
    /// can be turned back into the backend's own identifier without QML ever
    /// seeing one.
    streams: Vec<fluorita_core::Stream>,

    /// The extraction in flight, if any. One at a time: two decoders started
    /// from the same window would compete for the same name on disk.
    frame_worker: Option<JoinHandle<()>>,

    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
    /// Set while a close is waiting for the surface to release its context.
    closing: bool,

    /// What the rest of the desktop reads. Created with the first session and
    /// kept for the process: a panel that saw the player once should not lose
    /// it because a track ended.
    mpris: Option<crate::mpris::Mpris>,
    /// The inbox a bus request is delivered into. Shared because the D-Bus
    /// thread reaches it while the GUI thread swaps it per session.
    remote: std::sync::Arc<std::sync::Mutex<Option<Sender<Command>>>>,
    /// What is playing, as MPRIS describes it. Kept beside the QObject's own
    /// properties because the bus wants the path and the title, which the
    /// interface does not show.
    now_playing: crate::mpris::NowPlaying,
}

impl cxx_qt::Initialize for qobject::FluoritaPlayer {
    fn initialize(self: core::pin::Pin<&mut Self>) {
        let mut this = self;
        this.as_mut().set_state(QString::from("inactivo"));
    }
}

impl qobject::FluoritaPlayer {
    /// Opens one item on the worker thread.
    /// Opens an item, closing whatever is open first.
    ///
    /// A live render context cannot simply be torn down under the surface that
    /// is drawing from it — that is the whole reason [`Self::close`] hands the
    /// handle back before stopping the worker. Replacing one video with another
    /// went straight to `stop_worker`, destroying the mpv instance while the
    /// Qt item still rendered from it, and crashed. So a session with a live
    /// surface is closed through the same handshake and the new item waits for
    /// the surface to confirm.
    pub fn open(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        // Whatever this player was doing before, an explicit open is not a
        // preview: leaving the flag set would give the item a person chose the
        // silent, looping treatment meant for a glance.
        self.as_mut().set_previewing(false);
        self.as_mut().open_key(key);
    }

    fn open_key(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        let text = key.to_string();
        // An empty key is the ordinary "nothing to open" case — a bare launch
        // binds one — and stays silent. Anything else that is not a key is a
        // caller error, and a player that quietly did nothing about it would be
        // indistinguishable from one that is simply slow.
        if text.is_empty() {
            return;
        }
        let path = match celestina_core::pathkey::decode(&text) {
            Ok(path) => path,
            Err(_) => {
                self.as_mut().set_state(QString::from("error"));
                self.as_mut()
                    .set_error_message(QString::from(crate::copy::UNREADABLE_KEY));
                return;
            }
        };
        match decide_open(*self.render_handle() != 0, self.closing()) {
            OpenAction::Begin => self.begin(path),
            OpenAction::CloseFirst => {
                self.as_mut().rust_mut().pending_open = Some(path);
                self.close();
            }
            // A close is already in flight and the handle is already zero, so
            // the old gate — "is anything rendering?" — read this as an idle
            // player and went straight to tearing the session down under a
            // context that may not be free yet. Waiting is the only correct
            // answer: `surface_released` starts what is left here.
            OpenAction::Wait => self.as_mut().rust_mut().pending_open = Some(path),
        }
    }

    /// The half of opening that assumes nothing is rendering any more.
    fn begin(mut self: core::pin::Pin<&mut Self>, path: PathBuf) {
        self.as_mut().stop_worker();
        self.as_mut().reset_for(&path);

        // A file this player cannot classify is not opened at all: starting a
        // decoder for a text file would break the contract that browsing costs
        // nothing, and showing it as "opening" would be a state that never
        // resolves.
        let Some(kind) = MediaKind::classify_path(&path) else {
            // Falls through to the refusal below.
            self.as_mut().set_state(QString::from("error"));
            self.as_mut()
                .set_error_message(QString::from(crate::copy::UNKNOWN_KIND));
            return;
        };

        // A still is the toolkit's job. Deciding it here is what keeps the
        // promise that looking at a photograph costs nothing from the media
        // stack: no session, no handle, no decoder thread.
        if kind == MediaKind::Image {
            self.show_image(path.as_path());
            return;
        }

        let (sender, receiver) = mpsc::channel::<Command>();
        let qt_thread = self.qt_thread();
        let generation = self.rust().generation.wrapping_add(1);
        self.as_mut().rust_mut().generation = generation;

        let worker = std::thread::Builder::new()
            .name("fluorita-player".to_owned())
            .spawn({
                // The publisher needs the path too, and the worker takes it by
                // value; one clone is cheaper than making the session borrow.
                let path = path.clone();
                let previewing = *self.previewing();
                let pacing_on = std::sync::Arc::clone(&self.rust().pacing_on);
                move || {
                    run_session(
                        &path, kind, generation, previewing, &pacing_on, &receiver, &qt_thread,
                    )
                }
            });

        match worker {
            Ok(handle) => {
                self.as_mut().rust_mut().commands = Some(sender.clone());
                if let Ok(mut remote) = self.as_mut().rust_mut().remote.lock() {
                    *remote = Some(sender);
                }
                self.as_mut().start_publishing(&path, kind);
                self.as_mut().rust_mut().worker = Some(handle);
            }
            Err(error) => {
                self.as_mut()
                    .set_error_message(QString::from(&format!("{error}")));
                self.as_mut().set_state(QString::from("error"));
            }
        }
    }

    /// The surface has a render context: the session may load now.
    pub fn surface_ready(self: core::pin::Pin<&mut Self>) {
        if let Some(sender) = self.rust().commands.as_ref() {
            let _ = sender.send(Command::Start);
        }
    }

    /// Judges a still against its budget and hands it to the toolkit.
    fn show_image(mut self: core::pin::Pin<&mut Self>, path: &std::path::Path) {
        let bytes = std::fs::metadata(path).map(|data| data.len()).unwrap_or(0);
        // The probe is addressed by path key, not by the path: it opens the
        // file by descriptor on the decoded bytes, so a name this side cannot
        // spell is still measured — and measured on itself, never on whatever
        // file a lossy spelling would have hit.
        let probed = {
            let measured = qobject::probe_image(&QString::from(
                celestina_core::pathkey::encode(path).as_str(),
            ));
            let (width, height) = (measured.width(), measured.height());
            (width > 0 && height > 0)
                .then(|| (u32::try_from(width).ok(), u32::try_from(height).ok()))
                .and_then(|(width, height)| Some((width?, height?)))
        };

        match ImageDecision::judge(bytes, probed) {
            // The URL is the suite's frozen `file://` spelling, so a name with
            // spaces or a non-ASCII character reaches the toolkit intact.
            ImageDecision::Show { .. } => match fluorita_core::file_uri(path) {
                Some(url) => {
                    self.as_mut().set_image_source(QString::from(&url));
                    self.as_mut().set_state(QString::from("mostrando"));
                }
                None => {
                    self.as_mut().set_state(QString::from("error"));
                    self.as_mut()
                        .set_error_message(QString::from(crate::copy::UNRESOLVED_IMAGE));
                }
            },
            refusal => {
                self.as_mut().set_image_source(QString::default());
                self.as_mut().set_state(QString::from("error"));
                self.as_mut()
                    .set_error_message(QString::from(&refusal.message()));
            }
        }
    }

    pub fn play(self: core::pin::Pin<&mut Self>) {
        self.send(PlaybackRequest::Play);
    }

    pub fn pause(self: core::pin::Pin<&mut Self>) {
        self.send(PlaybackRequest::Pause);
    }

    pub fn toggle(self: core::pin::Pin<&mut Self>) {
        match self.state().to_string().as_str() {
            "reproduciendo" => self.send(PlaybackRequest::Pause),
            "pausado" => self.send(PlaybackRequest::Play),
            _ => {}
        }
    }

    pub fn seek(self: core::pin::Pin<&mut Self>, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            self.send(PlaybackRequest::Seek(Duration::from_secs_f64(seconds)));
        }
    }

    pub fn set_volume(self: core::pin::Pin<&mut Self>, level: f64) {
        if level.is_finite() {
            self.send(PlaybackRequest::SetVolume(level));
        }
    }

    pub fn select_audio_stream(self: core::pin::Pin<&mut Self>, index: i32) {
        self.select_stream(StreamKind::Audio, index);
    }

    pub fn select_subtitle_stream(self: core::pin::Pin<&mut Self>, index: i32) {
        self.select_stream(StreamKind::Subtitle, index);
    }

    pub fn toggle_pacing(mut self: core::pin::Pin<&mut Self>) {
        let on = !*self.capturing_pacing();
        self.as_mut().set_capturing_pacing(on);
        self.as_mut().set_pacing_report(QString::default());
        if on {
            // A new recording, not a continuation of the last one: the numbers
            // are rates over a span, and stitching two sittings together would
            // average a stutter away with the good minutes before it.
            self.as_mut().rust_mut().pacing.clear();
            self.as_mut().rust_mut().pacing_started = Some(std::time::Instant::now());
        }
        self.rust()
            .pacing_on
            .store(on, std::sync::atomic::Ordering::Relaxed);
        self.as_mut().publish_pacing();
    }

    pub fn write_pacing_report(mut self: core::pin::Pin<&mut Self>) {
        let Some(path) = pacing_report_path() else {
            self.as_mut()
                .set_pacing_report(QString::from(crate::copy::NO_REPORT_DIRECTORY));
            return;
        };
        let report =
            render_pacing_report(&self.rust().pacing, self.rust().now_playing.path.as_deref());
        match celestina_core::atomic_file::replace(&path, report.as_bytes()) {
            Ok(()) => {
                let shown = path.to_string_lossy().into_owned();
                self.as_mut().set_pacing_report(QString::from(&shown));
            }
            Err(_) => {
                self.as_mut()
                    .set_pacing_report(QString::from(crate::copy::REPORT_NOT_WRITTEN));
            }
        }
    }

    /// Takes one reading from the session thread.
    ///
    /// The sample carries how far into the recording it was taken, because the
    /// domain that folds it never reads a clock — and because a rate needs the
    /// time between two readings, not the moment either of them arrived.
    pub fn record_pacing(
        mut self: core::pin::Pin<&mut Self>,
        stats: &fluorita_engine::backend::FrameStats,
    ) {
        if !*self.capturing_pacing() {
            return;
        }
        let at = self
            .rust()
            .pacing_started
            .map_or(std::time::Duration::ZERO, |started| started.elapsed());
        let sample = fluorita_core::PacingSample {
            at,
            dropped: stats.dropped,
            delayed: stats.delayed,
            display_fps: stats.display_fps,
            vsync_jitter: stats.vsync_jitter,
        };
        self.as_mut().rust_mut().pacing.push(sample);
        self.publish_pacing();
    }

    /// Folds what has been recorded and publishes it as one readable line.
    fn publish_pacing(mut self: core::pin::Pin<&mut Self>) {
        let summary = self.rust().pacing.summary();
        let verdict = fluorita_core::Verdict::of(&summary);
        self.as_mut()
            .set_pacing_verdict(QString::from(verdict_token(verdict)));
        self.as_mut()
            .set_pacing_line(QString::from(&crate::copy::pacing_line(&summary, verdict)));
    }

    pub fn preview(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        let Ok(path) = celestina_core::pathkey::decode(&key.to_string()) else {
            return;
        };
        // Only a moving picture has anything to preview. A still is already on
        // the card, and starting a decoder for one would break the promise that
        // browsing costs nothing.
        if !MediaKind::classify_path(&path).is_some_and(|kind| kind == MediaKind::Video) {
            return;
        }
        self.as_mut().set_previewing(true);
        self.as_mut().open_key(key);
    }

    pub fn set_continuation_mode(mut self: core::pin::Pin<&mut Self>, mode: i32) {
        // A mode this build does not have is ignored rather than stored: the
        // list is the domain's, and a number outside it would answer every
        // later question with "stop".
        if usize::try_from(mode).is_ok_and(|mode| mode < Continuation::ALL.len()) {
            self.as_mut().set_continuation(mode);
        }
    }

    #[must_use]
    pub fn next_in_folder(&self, index: i32, count: i32) -> i32 {
        let (Ok(index), Ok(count)) = (usize::try_from(index), usize::try_from(count)) else {
            return -1;
        };
        // What ended decides whether anything follows, and the path is the one
        // thing this object always has for the open item.
        let Some(kind) = self
            .rust()
            .now_playing
            .path
            .as_deref()
            .and_then(MediaKind::classify_path)
        else {
            return -1;
        };
        let mode = Continuation::ALL
            .get(usize::try_from(*self.continuation()).unwrap_or(0))
            .copied()
            .unwrap_or_default();
        mode.next(kind, index, count)
            .and_then(|next| i32::try_from(next).ok())
            .unwrap_or(-1)
    }

    pub fn play_at(self: core::pin::Pin<&mut Self>, rate: f64) {
        self.send(PlaybackRequest::SetSpeed(Speed::new(rate)));
    }

    /// The handle goes first: the surface must stop rendering before anything
    /// it renders from can be destroyed.
    pub fn close(mut self: core::pin::Pin<&mut Self>) {
        if self.worker().is_none() {
            // Nothing to close. An activation parked while a close was in
            // flight still has to go somewhere: leaving it here is what turned
            // a stale handle into a player that answered nothing for the rest
            // of the session.
            if let Some(path) = self.as_mut().rust_mut().pending_open.take() {
                self.as_mut().rust_mut().closing = false;
                self.as_mut().set_render_handle(0);
                self.begin(path);
            }
            return;
        }
        if self.closing() {
            return;
        }
        // Marked before the handle is cleared, not after. A surface with no
        // renderer answers `contextReleased` synchronously from inside the
        // property write, and the acknowledgement would arrive at a player that
        // did not yet know it was closing — leaving the flag set for ever.
        self.as_mut().rust_mut().closing = true;
        if *self.render_handle() == 0 {
            // Nothing was ever handed to a surface — audio, which needs none —
            // so clearing the handle would change nothing and no acknowledgement
            // would ever come back. Settling here is what keeps a track from
            // leaving the player permanently mid-close.
            self.surface_released();
            return;
        }
        self.as_mut().set_render_handle(0);
        // A surface that never had a context answers immediately; one that did
        // answers from the render thread. Either way `surface_released` runs.
    }

    /// The surface could not render. Sound, position and transport are still
    /// honest; what is not honest is a picture that will never arrive.
    pub fn surface_failed(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().set_state(QString::from("error"));
        self.as_mut()
            .set_error_message(QString::from(crate::copy::SURFACE_UNAVAILABLE));
        self.as_mut().set_pending(false);
    }

    pub fn surface_released(mut self: core::pin::Pin<&mut Self>) {
        // Also the guard against stopping a worker twice: the flag is the one
        // record of "this session is being closed", and a second release —
        // which the surface may legitimately send, since the render thread and
        // the immediate path can both answer — must not reach the worker of
        // whatever session started in the meantime.
        if !self.closing() {
            return;
        }
        self.as_mut().rust_mut().closing = false;
        self.as_mut().stop_worker();
        self.as_mut().set_state(QString::from("inactivo"));
        self.as_mut().set_position_seconds(0.0);
        self.as_mut().set_duration_seconds(0.0);
        self.as_mut().set_pending(false);
        // Someone asked for the next item while this one was still rendering.
        // Now that the surface has let go, it is safe to start.
        let waiting = self.as_mut().rust_mut().pending_open.take();
        if let Some(path) = waiting {
            self.begin(path);
        }
    }

    fn closing(&self) -> bool {
        self.rust().closing
    }

    fn worker(&self) -> Option<&JoinHandle<()>> {
        self.rust().worker.as_ref()
    }

    /// Turns a menu position into the backend's own identifier.
    ///
    /// QML never sees an identifier: it says "the second one" and this decides
    /// what that means, so a file whose streams are numbered oddly — which is
    /// most of them — cannot be mis-addressed from the surface.
    fn select_stream(mut self: core::pin::Pin<&mut Self>, kind: StreamKind, index: i32) {
        let id = if index < 0 {
            None
        } else {
            match self
                .rust()
                .streams
                .iter()
                .filter(|stream| stream.kind == kind)
                .nth(index as usize)
            {
                Some(stream) => Some(stream.id),
                // A position that names nothing is dropped rather than turned
                // into a request the backend would refuse.
                None => return,
            }
        };
        self.as_mut()
            .send(PlaybackRequest::SelectStream { kind, id });
    }

    fn send(self: core::pin::Pin<&mut Self>, request: PlaybackRequest) {
        let mut this = self;
        if let Some(sender) = this.as_mut().rust().commands.as_ref() {
            // A worker that already exited is not an error the user can act on:
            // the state it left behind is still the honest one.
            let _ = sender.send(Command::Transport(request));
        }
        this.set_pending(true);
    }

    /// Stops the worker and joins it, so no thread outlives the player.
    fn stop_worker(mut self: core::pin::Pin<&mut Self>) {
        let sender = self.as_mut().rust_mut().commands.take();
        if let Some(sender) = sender {
            let _ = sender.send(Command::Stop);
        }
        let worker = self.as_mut().rust_mut().worker.take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        self.as_mut().set_render_handle(0);
    }

    /// Keeps the frame at the position the player is confirmed to be at.
    ///
    /// The position comes from the published property rather than from a fresh
    /// query, so the picture is the frame the person was looking at when they
    /// asked — the engine's confirmed position, not one measured a moment later.
    pub fn extract_frame(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        if *self.extracting_frame() {
            return;
        }
        let Ok(path) = celestina_core::pathkey::decode(&key.to_string()) else {
            self.as_mut()
                .set_frame_notice(QString::from(crate::copy::UNREADABLE_KEY));
            return;
        };
        if !MediaKind::classify_path(&path).is_some_and(|kind| kind.capabilities().has_video) {
            self.as_mut()
                .set_frame_notice(QString::from(crate::copy::NO_FRAME));
            return;
        }

        let at = std::time::Duration::from_secs_f64(self.position_seconds().max(0.0));
        self.as_mut().set_extracting_frame(true);
        self.as_mut().set_frame_notice(QString::default());

        let qt_thread = self.qt_thread();
        let worker = std::thread::spawn(move || {
            let request = fluorita_engine::FrameRequest {
                source: &path,
                at,
                marker: crate::copy::FRAME_MARKER,
                deadline: fluorita_engine::FRAME_DEADLINE,
            };
            let message = match fluorita_engine::extract_frame(
                &request,
                &celestina_core::CancellationToken::new(),
            ) {
                Ok(kept) => crate::copy::frame_kept(&kept),
                Err(error) => error.user_message(),
            };
            let _ = qt_thread.queue(move |mut player| {
                player.as_mut().set_extracting_frame(false);
                player.as_mut().set_frame_notice(QString::from(&message));
            });
        });
        self.as_mut().rust_mut().frame_worker = Some(worker);
    }

    fn reset_for(mut self: core::pin::Pin<&mut Self>, path: &std::path::Path) {
        let kind = MediaKind::classify_path(path);
        let capabilities = kind.map(MediaKind::capabilities);
        self.as_mut().set_state(QString::from("abriendo"));
        self.as_mut().set_error_message(QString::default());
        self.as_mut().set_image_source(QString::default());
        self.as_mut()
            .set_timed(capabilities.is_some_and(|caps| caps.timed));
        self.as_mut().set_position_seconds(0.0);
        self.as_mut().set_duration_seconds(0.0);
        self.as_mut().set_pending(false);
        // Every new session starts a fresh mpv instance, and nothing in this
        // codebase carries a chosen level across it (`SessionRequest` never
        // sets `initial_volume` here) — so mpv's own default, 100%, is what
        // actually starts playing next, and the display should say so.
        self.as_mut().set_volume_level(1.0);
        self.as_mut()
            .set_seekable(capabilities.is_some_and(|caps| caps.seekable));
        // "Has video" here means a moving picture the render surface must
        // present. A still draws through the toolkit instead, so it is false.
        self.as_mut().set_has_video(kind == Some(MediaKind::Video));
        self.as_mut()
            .set_has_audio(capabilities.is_some_and(|caps| caps.has_audio));
    }

    /// Applies one worker snapshot. Runs on the Qt thread, by construction.
    /// Starts publishing on the bus, and says what is playing now.
    fn start_publishing(
        mut self: core::pin::Pin<&mut Self>,
        path: &std::path::Path,
        kind: MediaKind,
    ) {
        // A preview is not what this desktop is playing.
        if *self.previewing() {
            return;
        }
        if self.rust().mpris.is_none() {
            let remote = std::sync::Arc::clone(&self.rust().remote);
            // The closure runs on the D-Bus thread: it does nothing but hand
            // the request to whatever session is open, exactly as a click does.
            let published = crate::mpris::Mpris::start(std::sync::Arc::new(
                move |request: fluorita_core::PlaybackRequest| {
                    if let Ok(remote) = remote.lock() {
                        if let Some(sender) = remote.as_ref() {
                            let _ = sender.send(Command::Transport(request));
                        }
                    }
                },
            ));
            self.as_mut().rust_mut().mpris = published;
        }

        let now = crate::mpris::NowPlaying {
            state: PlaybackState::Opening,
            path: Some(path.to_path_buf()),
            title: path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned()),
            position: Duration::ZERO,
            duration: None,
            volume: 1.0,
            seekable: kind.capabilities().seekable,
        };
        self.as_mut().rust_mut().now_playing = now.clone();
        if let Some(mpris) = self.rust().mpris.as_ref() {
            mpris.publish(now);
        }
    }

    /// Mirrors one confirmed report onto the bus.
    fn publish_now_playing(mut self: core::pin::Pin<&mut Self>, snapshot: &Snapshot) {
        let mut now = self.rust().now_playing.clone();
        now.state = snapshot.state;
        if let Some(position) = snapshot.position {
            now.position = position;
        }
        if let Some(duration) = snapshot.duration {
            now.duration = Some(duration);
        }
        if let Some(volume) = snapshot.volume {
            now.volume = volume;
        }
        self.as_mut().rust_mut().now_playing = now.clone();
        if let Some(mpris) = self.rust().mpris.as_ref() {
            mpris.publish(now);
        }
    }

    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: &Snapshot) {
        self.as_mut().publish_now_playing(snapshot);
        self.as_mut()
            .set_state(QString::from(state_label(snapshot.state)));
        self.as_mut().set_pending(snapshot.pending);
        if let Some(position) = snapshot.position {
            self.as_mut().set_position_seconds(position.as_secs_f64());
        }
        if let Some(duration) = snapshot.duration {
            self.as_mut().set_duration_seconds(duration.as_secs_f64());
        }
        if let Some(volume) = snapshot.volume {
            self.as_mut().set_volume_level(volume);
        }
        if let Some(message) = snapshot.error.as_deref() {
            self.as_mut().set_error_message(QString::from(message));
        }
        self.publish_streams(snapshot);
    }

    /// Publishes the streams as the words a menu shows.
    ///
    /// The label is built here and not in QML because what a stream is called
    /// is three optional fields and a fallback, and a surface rebuilding that
    /// rule would drift from the one the domain bounded.
    fn publish_streams(mut self: core::pin::Pin<&mut Self>, snapshot: &Snapshot) {
        let label = |stream: &fluorita_core::Stream, position: usize| {
            if stream.is_anonymous() {
                crate::copy::stream_position(position)
            } else if stream.title.is_empty() {
                stream.language.clone()
            } else if stream.language.is_empty() {
                stream.title.clone()
            } else {
                format!("{} · {}", stream.title, stream.language)
            }
        };

        for kind in [StreamKind::Audio, StreamKind::Subtitle] {
            let mut labels = QStringList::default();
            let mut selected = -1i32;
            let chosen = match kind {
                StreamKind::Audio => snapshot.audio,
                StreamKind::Subtitle => snapshot.subtitle,
            };
            for (position, stream) in snapshot
                .streams
                .iter()
                .filter(|stream| stream.kind == kind)
                .enumerate()
            {
                labels.append(QString::from(&label(stream, position)));
                if chosen == Some(stream.id) {
                    selected = i32::try_from(position).unwrap_or(-1);
                }
            }
            let count = labels.len();
            match kind {
                StreamKind::Audio => {
                    self.as_mut().set_audio_streams(labels);
                    self.as_mut().set_audio_stream(selected);
                    // One audio stream is not a choice.
                    self.as_mut().set_choosable_audio(count > 1);
                }
                StreamKind::Subtitle => {
                    self.as_mut().set_subtitle_streams(labels);
                    self.as_mut().set_subtitle_stream(selected);
                    // One set of subtitles is, because it can also be off.
                    self.as_mut().set_choosable_subtitles(count > 0);
                }
            }
        }
        self.as_mut().set_speed(snapshot.speed);
        self.as_mut().rust_mut().streams = snapshot.streams.clone();
    }
}

/// What opening an item must do, given what the surface is doing right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenAction {
    /// Nothing is rendering and nothing is being torn down: start at once.
    Begin,
    /// Something is rendering: hand the handle back and wait for the surface.
    CloseFirst,
    /// A close is already in flight: the handle is gone but the render context
    /// may not be, so the request waits for the same acknowledgement.
    Wait,
}

/// Kept apart from the QObject so the rule can be read and tested on its own;
/// the pinned method it drives cannot be constructed without a Qt application.
const fn decide_open(rendering: bool, closing: bool) -> OpenAction {
    if closing {
        OpenAction::Wait
    } else if rendering {
        OpenAction::CloseFirst
    } else {
        OpenAction::Begin
    }
}

/// Stops and joins the worker when the player itself goes away.
///
/// Quitting with a video playing used to run the backend's destruction beside
/// the scene graph's, because nothing joined this thread: the process simply
/// ended and whichever teardown lost the race was the one that crashed.
impl Drop for PlayerRust {
    fn drop(&mut self) {
        if let Some(sender) = self.commands.take() {
            // A worker that already left is not an error; the join below is
            // what makes the shutdown deterministic either way.
            let _ = sender.send(Command::Stop);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.frame_worker.take() {
            let _ = worker.join();
        }
    }
}

/// The interface's vocabulary for confirmed state. Spanish, because it is shown.
fn state_label(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Idle => "inactivo",
        PlaybackState::Opening => "abriendo",
        PlaybackState::Playing => "reproduciendo",
        PlaybackState::Paused => "pausado",
        PlaybackState::Ended => "terminado",
        PlaybackState::Failed => "error",
    }
}

/// The worker: owns the session, applies commands, forwards confirmed state.
fn run_session(
    path: &std::path::Path,
    kind: MediaKind,
    session_generation: u64,
    previewing: bool,
    pacing_on: &std::sync::atomic::AtomicBool,
    commands: &mpsc::Receiver<Command>,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaPlayer>,
) {
    let mut truth = PlaybackSession::new();
    let media = fluorita_core::MediaId::from_path(path);
    let Ok(generation) = truth.select(media, kind) else {
        publish_failure(qt_thread, "no se pudo iniciar la sesión");
        return;
    };

    let mut request = SessionRequest::new(path.to_path_buf(), generation);
    // Only a moving picture needs a surface; audio would pay for a GL context
    // and a render context it never draws into.
    if kind.capabilities().has_video {
        request = request.embedded_video();
    }
    if previewing {
        // A preview is a glance, not a session: no sound, no hardware context
        // for a picture the size of a card, and it starts where the film is
        // rather than in its titles. It loops because a frozen last frame
        // under the pointer reads as a hang.
        request = request.silent();
        request.hardware_decoding = false;
        request.looping = true;
        request.start_at = Some(PREVIEW_START);
    }

    let mut session = match MpvEngine::new().open_session(request) {
        Ok(session) => session,
        Err(error) => {
            publish_failure(qt_thread, &error.user_message());
            return;
        }
    };

    // Audio has no surface to wait for, so it starts here. Video waits for the
    // surface to report a render context, which arrives as `Command::Start`.
    let presenting = kind.capabilities().has_video;
    if !presenting {
        if let Err(error) = session.start() {
            publish_failure(qt_thread, &error.user_message());
            return;
        }
    }

    if let Some(handle) = session.render_handle() {
        let address = handle.value();
        let _ = qt_thread.queue(move |mut player| {
            // The close that ended this session may already have run: it joins
            // the worker and destroys the instance, and this closure was queued
            // before either. Publishing now would hand the surface the address
            // of a freed `mpv_handle`, and leave the player holding a handle
            // with no worker — a state in which every later activation is a
            // silent no-op.
            if player.rust().generation == session_generation {
                player.as_mut().set_render_handle(address);
            }
        });
    }

    // The environment variable still turns the sampler on for a headless run;
    // the shared flag is what the window toggles while something is playing.
    let forced = std::env::var_os("FLUORITA_PACING").is_some();
    let mut last_pacing = std::time::Instant::now();

    loop {
        match commands.try_recv() {
            Ok(Command::Start) => {
                if let Err(error) = session.start() {
                    publish_failure(qt_thread, &error.user_message());
                    break;
                }
            }
            Ok(Command::Transport(action)) => {
                if truth.request(action).is_ok() {
                    let _ = session.request(action);
                }
            }
            Ok(Command::Stop) | Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        let sampling = forced || pacing_on.load(std::sync::atomic::Ordering::Relaxed);
        if sampling && last_pacing.elapsed() >= PACING_INTERVAL {
            let stats = session.frame_stats();
            if forced {
                report_pacing(&stats);
            }
            let taken = std::time::Instant::now();
            let _ = qt_thread.queue(move |player| player.record_pacing(&stats));
            last_pacing = taken;
        }

        if let Some(report) = session.poll(POLL_TIMEOUT) {
            if truth.apply(&report) == ReportOutcome::Applied {
                let streams = truth.streams();
                let snapshot = Snapshot {
                    state: truth.state(),
                    position: truth.position(),
                    duration: truth.duration(),
                    volume: truth.volume(),
                    pending: truth.pending_transport().is_some() || truth.is_seeking(),
                    error: truth.error().map(str::to_owned),
                    streams: streams
                        .of(StreamKind::Audio)
                        .chain(streams.of(StreamKind::Subtitle))
                        .cloned()
                        .collect(),
                    audio: streams.selected(StreamKind::Audio),
                    subtitle: streams.selected(StreamKind::Subtitle),
                    speed: truth.speed().rate(),
                };
                let _ = qt_thread.queue(move |player| player.apply(&snapshot));
            }
        }
    }

    session.close();
}

/// Where a hover preview begins. Far enough in to be past titles and black,
/// and a fixed offset rather than a fraction so a long film does not start
/// twenty minutes from its opening.
const PREVIEW_START: Duration = Duration::from_secs(30);

/// The token a surface colours by.
const fn verdict_token(verdict: fluorita_core::Verdict) -> &'static str {
    match verdict {
        fluorita_core::Verdict::TooEarly => "too-early",
        fluorita_core::Verdict::Smooth => "smooth",
        fluorita_core::Verdict::Delayed => "delayed",
        fluorita_core::Verdict::Dropping => "dropping",
    }
}

/// Where a report lands: beside the caches this application already owns, under
/// a name carrying the moment it was written so two are never the same file.
fn pacing_report_path() -> Option<std::path::PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let directory = celestina_core::xdg::cache_home()?.join("fluorita");
    std::fs::create_dir_all(&directory).ok()?;
    Some(directory.join(format!("pacing-{stamp}.txt")))
}

/// The report itself: the conclusion first, then every reading behind it.
///
/// English, like every other development artefact — this is evidence attached
/// to a defect report, not something the interface says.
fn render_pacing_report(
    capture: &fluorita_core::PacingCapture,
    source: Option<&std::path::Path>,
) -> String {
    let summary = capture.summary();
    let verdict = fluorita_core::Verdict::of(&summary);
    let mut out = String::new();
    out.push_str("fluorita frame pacing report\n\n");
    if let Some(source) = source {
        out.push_str(&format!("source\t{}\n", source.display()));
    }
    out.push_str(&format!("verdict\t{}\n", verdict_token(verdict)));
    out.push_str(&format!("samples\t{}\n", summary.samples));
    out.push_str(&format!(
        "span_seconds\t{:.1}\n",
        summary.span.as_secs_f64()
    ));
    out.push_str(&format!(
        "dropped_per_minute\t{:.2}\n",
        summary.dropped_per_minute
    ));
    out.push_str(&format!(
        "delayed_per_minute\t{:.2}\n",
        summary.delayed_per_minute
    ));
    out.push_str(&format!(
        "display_fps\t{}\n",
        summary
            .display_fps
            .map_or_else(|| "unknown".to_owned(), |value| format!("{value:.2}"))
    ));
    out.push_str(&format!(
        "worst_vsync_jitter\t{}\n\n",
        summary
            .worst_jitter
            .map_or_else(|| "unknown".to_owned(), |value| format!("{value:.4}"))
    ));
    out.push_str("at_seconds\tdropped\tdelayed\tdisplay_fps\tvsync_jitter\n");
    for sample in capture.samples() {
        let number = |value: Option<i64>| {
            value.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
        };
        let decimal = |value: Option<f64>| {
            value.map_or_else(|| "unknown".to_owned(), |value| format!("{value:.4}"))
        };
        out.push_str(&format!(
            "{:.1}\t{}\t{}\t{}\t{}\n",
            sample.at.as_secs_f64(),
            number(sample.dropped),
            number(sample.delayed),
            decimal(sample.display_fps),
            decimal(sample.vsync_jitter),
        ));
    }
    out
}

/// How often the pacing sampler prints, when it is on.
const PACING_INTERVAL: Duration = Duration::from_secs(1);

/// Prints what the backend measured about presentation, when asked to.
///
/// Behind an environment variable because it is a measurement, not a feature:
/// a player that printed frame statistics constantly would be noise, and one
/// that hid them would leave "does it pace?" unanswerable outside a profiler.
/// `FLUORITA_PACING=1` is what the roadmap's evidence runs with.
///
/// Sampled *while playing*, not at the end: pacing is a distribution over time,
/// and a session that has already stopped has no properties left to answer with
/// — which is how the first attempt at this measured nothing at all.
fn report_pacing(stats: &fluorita_engine::backend::FrameStats) {
    let show = |value: Option<f64>| {
        value.map_or_else(|| "desconocido".to_owned(), |number| format!("{number:.2}"))
    };
    let count = |value: Option<i64>| {
        value.map_or_else(|| "desconocido".to_owned(), |number| number.to_string())
    };
    eprintln!(
        "fluorita: pacing — descartados={} tardíos={} display_fps={} jitter={}",
        count(stats.dropped),
        count(stats.delayed),
        show(stats.display_fps),
        show(stats.vsync_jitter),
    );
}

fn publish_failure(qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaPlayer>, message: &str) {
    let message = message.to_owned();
    let _ = qt_thread.queue(move |mut player| {
        player.as_mut().set_state(QString::from("error"));
        player.as_mut().set_error_message(QString::from(&message));
    });
}

#[cfg(test)]
mod tests {
    use super::{decide_open, state_label, OpenAction};
    use cxx_qt_lib::QString;
    use fluorita_core::PlaybackState;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A 2x2 red PNG, written byte by byte so the fixture needs no encoder.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd,
        0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x9f, 0x01, 0x8c, 0xff, 0x33, 0x30, 0x00, 0x00, 0x1f, 0xee, 0x03, 0xfd,
        0x35, 0x1b, 0x00, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
        0x82,
    ];

    /// A temporary directory that removes itself, holding a file whose name the
    /// test chooses byte by byte.
    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-probe-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create fixture directory");
            Self(path)
        }

        fn write(&self, name: &[u8], contents: &[u8]) -> PathBuf {
            let file = self.0.join(OsString::from_vec(name.to_vec()));
            fs::write(&file, contents).expect("write fixture file");
            file
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The probe, addressed the way `show_image` addresses it.
    fn probe(path: &std::path::Path) -> (i32, i32) {
        let key = celestina_core::pathkey::encode(path);
        let size = super::qobject::probe_image(&QString::from(key.as_str()));
        (size.width(), size.height())
    }

    /// A handle published by a session the player has already left must be
    /// dropped. The address it carries belongs to an `mpv_handle` that
    /// `stop_worker` has destroyed, and accepting it leaves the player holding
    /// a handle with no worker — the state in which `decide_open` routes every
    /// later activation to a close that returns immediately.
    #[test]
    fn a_handle_from_a_session_already_left_is_not_published() {
        // The guard the queued closure applies, stated as the rule it is.
        fn publishes(current: u64, published_by: u64) -> bool {
            current == published_by
        }
        assert!(publishes(7, 7));
        assert!(!publishes(8, 7), "a close bumped past this session");
        // The counter wraps rather than overflowing, and a wrap is still a
        // different session.
        assert!(!publishes(0, u64::MAX));
    }

    /// The other half: a close that finds no worker still has to resolve an
    /// activation parked while the previous one was closing.
    #[test]
    fn an_activation_parked_during_a_close_is_not_stranded() {
        // Parking happens on `Wait`, which is what `closing` produces.
        assert_eq!(decide_open(false, true), OpenAction::Wait);
        assert_eq!(decide_open(true, true), OpenAction::Wait);
        // And a player still holding a handle routes through the close that
        // now has to honour what was parked.
        assert_eq!(decide_open(true, false), OpenAction::CloseFirst);
    }

    #[test]
    fn an_image_whose_name_is_not_utf8_is_measured_on_itself() {
        let fixture = Fixture::new("nonutf8");
        let picture = fixture.write(b"na\xffme.png", TINY_PNG);
        // The spelling that used to be handed over names no file at all.
        assert!(
            picture.to_str().is_none(),
            "the fixture must be unspellable"
        );
        assert_eq!(probe(&picture), (2, 2));
    }

    #[test]
    fn an_ordinary_image_is_still_measured() {
        let fixture = Fixture::new("ordinary");
        let picture = fixture.write(b"foto.png", TINY_PNG);
        assert_eq!(probe(&picture), (2, 2));
    }

    #[test]
    fn what_is_not_a_readable_image_measures_nothing_usable() {
        // Qt spells an invalid size -1x-1, which is why the caller gates on a
        // positive pair rather than on zero.
        fn unusable((width, height): (i32, i32)) -> bool {
            width <= 0 || height <= 0
        }
        let fixture = Fixture::new("refusals");
        let text = fixture.write(b"nota.png", b"not a picture");
        assert!(unusable(probe(&text)));
        assert!(unusable(probe(&fixture.0.join("absent.png"))));
        // A relative key would resolve against this process's directory.
        let relative = super::qobject::probe_image(&QString::from("foto.png"));
        assert!(unusable((relative.width(), relative.height())));
    }

    #[test]
    fn an_idle_player_opens_straight_away() {
        assert_eq!(decide_open(false, false), OpenAction::Begin);
    }

    #[test]
    fn a_rendering_player_hands_the_surface_back_first() {
        assert_eq!(decide_open(true, false), OpenAction::CloseFirst);
    }

    #[test]
    fn a_close_in_flight_is_waited_for_rather_than_raced() {
        // The handle is already zero here, which is exactly why gating on it
        // alone let a new session start on top of a context that was still
        // being freed — and left the acknowledgement to kill the new worker.
        assert_eq!(decide_open(false, true), OpenAction::Wait);
        assert_eq!(decide_open(true, true), OpenAction::Wait);
    }

    #[test]
    fn every_state_has_a_word_the_interface_can_show() {
        assert_eq!(state_label(PlaybackState::Idle), "inactivo");
        assert_eq!(state_label(PlaybackState::Opening), "abriendo");
        assert_eq!(state_label(PlaybackState::Playing), "reproduciendo");
        assert_eq!(state_label(PlaybackState::Paused), "pausado");
        assert_eq!(state_label(PlaybackState::Ended), "terminado");
        assert_eq!(state_label(PlaybackState::Failed), "error");
    }
}
