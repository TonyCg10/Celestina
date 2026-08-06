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
use cxx_qt_lib::QString;

use fluorita_core::{MediaKind, PlaybackRequest, PlaybackSession, PlaybackState, ReportOutcome};

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
}

#[derive(Default)]
pub struct PlayerRust {
    state: QString,
    render_handle: u64,
    /// An item asked for while a surface was still rendering the previous one.
    /// It starts once that surface confirms it has let go.
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

        let worker = std::thread::Builder::new()
            .name("fluorita-player".to_owned())
            .spawn({
                // The publisher needs the path too, and the worker takes it by
                // value; one clone is cheaper than making the session borrow.
                let path = path.clone();
                move || run_session(&path, kind, &receiver, &qt_thread)
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

    /// The handle goes first: the surface must stop rendering before anything
    /// it renders from can be destroyed.
    pub fn close(mut self: core::pin::Pin<&mut Self>) {
        if self.worker().is_none() || self.closing() {
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
        let _ = qt_thread.queue(move |player| {
            player.set_render_handle(address);
        });
    }

    let pacing_on = std::env::var_os("FLUORITA_PACING").is_some();
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

        if pacing_on && last_pacing.elapsed() >= PACING_INTERVAL {
            report_pacing(session.as_ref());
            last_pacing = std::time::Instant::now();
        }

        if let Some(report) = session.poll(POLL_TIMEOUT) {
            if truth.apply(&report) == ReportOutcome::Applied {
                let snapshot = Snapshot {
                    state: truth.state(),
                    position: truth.position(),
                    duration: truth.duration(),
                    volume: truth.volume(),
                    pending: truth.pending_transport().is_some() || truth.is_seeking(),
                    error: truth.error().map(str::to_owned),
                };
                let _ = qt_thread.queue(move |player| player.apply(&snapshot));
            }
        }
    }

    session.close();
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
fn report_pacing(session: &dyn fluorita_engine::backend::EngineSession) {
    let stats = session.frame_stats();
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
