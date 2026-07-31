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
        fn fluorita_probe_image(path: &QString) -> QSize;
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
        type FluoritaPlayer = super::PlayerRust;

        /// Opens `path` and starts it. A second call replaces the first.
        #[qinvokable]
        fn open(self: Pin<&mut FluoritaPlayer>, path: &QString);

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
    pending: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct PlayerRust {
    state: QString,
    render_handle: u64,
    position_seconds: f64,
    duration_seconds: f64,
    seekable: bool,
    has_video: bool,
    has_audio: bool,
    pending: bool,
    error_message: QString,
    image_source: QString,
    timed: bool,

    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
    /// Set while a close is waiting for the surface to release its context.
    closing: bool,
}

impl cxx_qt::Initialize for qobject::FluoritaPlayer {
    fn initialize(self: core::pin::Pin<&mut Self>) {
        let mut this = self;
        this.as_mut().set_state(QString::from("inactivo"));
    }
}

impl qobject::FluoritaPlayer {
    /// Opens one item on the worker thread.
    pub fn open(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        if path.as_os_str().is_empty() {
            return;
        }
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
                .set_error_message(QString::from("Fluorita no reconoce este tipo de archivo"));
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
            .spawn(move || run_session(&path, kind, &receiver, &qt_thread));

        match worker {
            Ok(handle) => {
                self.as_mut().rust_mut().commands = Some(sender);
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
        let probed = {
            let measured = qobject::probe_image(&QString::from(path.to_string_lossy().as_ref()));
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
                    self.as_mut().set_error_message(QString::from(
                        "No se pudo resolver la ruta de la imagen",
                    ));
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
        if self.worker().is_none() {
            return;
        }
        self.as_mut().set_render_handle(0);
        self.as_mut().rust_mut().closing = true;
        // A surface that never had a context answers immediately; one that did
        // answers from the render thread. Either way `surface_released` runs.
    }

    pub fn surface_released(mut self: core::pin::Pin<&mut Self>) {
        if !self.closing() {
            return;
        }
        self.as_mut().rust_mut().closing = false;
        self.as_mut().stop_worker();
        self.as_mut().set_state(QString::from("inactivo"));
        self.as_mut().set_position_seconds(0.0);
        self.as_mut().set_duration_seconds(0.0);
        self.as_mut().set_pending(false);
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
        self.as_mut()
            .set_seekable(capabilities.is_some_and(|caps| caps.seekable));
        // "Has video" here means a moving picture the render surface must
        // present. A still draws through the toolkit instead, so it is false.
        self.as_mut().set_has_video(kind == Some(MediaKind::Video));
        self.as_mut()
            .set_has_audio(capabilities.is_some_and(|caps| caps.has_audio));
    }

    /// Applies one worker snapshot. Runs on the Qt thread, by construction.
    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: &Snapshot) {
        self.as_mut()
            .set_state(QString::from(state_label(snapshot.state)));
        self.as_mut().set_pending(snapshot.pending);
        if let Some(position) = snapshot.position {
            self.as_mut().set_position_seconds(position.as_secs_f64());
        }
        if let Some(duration) = snapshot.duration {
            self.as_mut().set_duration_seconds(duration.as_secs_f64());
        }
        if let Some(message) = snapshot.error.as_deref() {
            self.as_mut().set_error_message(QString::from(message));
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

        if let Some(report) = session.poll(POLL_TIMEOUT) {
            if truth.apply(&report) == ReportOutcome::Applied {
                let snapshot = Snapshot {
                    state: truth.state(),
                    position: truth.position(),
                    duration: truth.duration(),
                    pending: truth.pending_transport().is_some() || truth.is_seeking(),
                    error: truth.error().map(str::to_owned),
                };
                let _ = qt_thread.queue(move |player| player.apply(&snapshot));
            }
        }
    }

    session.close();
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
    use super::state_label;
    use fluorita_core::PlaybackState;

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
