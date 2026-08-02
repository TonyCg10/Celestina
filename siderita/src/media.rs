//! Siderita's adapter for the embedded Fluorita player.
//!
//! Everything true about media — what a kind is, what a session may claim,
//! which requests are even meaningful — lives in `fluorita-core`; everything
//! that decodes lives in `fluorita-engine`. This file is the Qt half and
//! nothing else, and it is deliberately smaller than Fluorita's own: a file
//! manager peeks at one item, it does not host a library.
//!
//! Three rules it exists to keep:
//!
//! - **Browsing costs nothing.** Nothing here runs until `Space` asks for a
//!   preview of a real media file. Listing a folder, drawing thumbnails from
//!   the shared cache and stepping the selection never construct a session, so
//!   the media backend is not in the picture at all until it is wanted.
//! - **One session at a time.** Stepping to the next entry closes the previous
//!   one before opening anything, so two files can never be decoding at once.
//! - **Confirmed state only.** A click is a request; the transport moves when
//!   the backend reports it, through `fluorita-core`'s own playback model.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use fluorita_core::{MediaKind, PlaybackRequest, PlaybackSession, PlaybackState, ReportOutcome};
use fluorita_engine::backend::{MediaEngine, SessionRequest};
use fluorita_engine::MpvEngine;

/// How long the worker waits for a backend report before reading its inbox.
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
        // The shared render seam (`fluorita-qt`): the Qt Quick surface libmpv
        // draws into, which CXX-Qt cannot express and which Fluorita's own
        // window uses too.
        include!("fluorita/mpvvideoitem.h");

        #[rust_name = "register_video_item"]
        fn register_fluorita_video_item(engine: Pin<&mut QQmlApplicationEngine>);
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // active     — a session is open and the modal should show the player
        // name/kind  — what is playing, for the label and the icon
        // state      — the backend's own word: abriendo/reproduciendo/pausado/
        //              terminado/error
        // timed      — whether a transport means anything for this item
        // pending    — a request is in flight and not yet confirmed
        // errorText  — why playback failed; empty when fine
        #[qobject]
        #[qml_element]
        #[qproperty(bool, active)]
        #[qproperty(QString, name)]
        #[qproperty(QString, kind)]
        #[qproperty(QString, state)]
        #[qproperty(bool, timed)]
        #[qproperty(bool, pending)]
        #[qproperty(i32, position_ms)]
        #[qproperty(i32, duration_ms)]
        #[qproperty(i32, volume_percent)]
        #[qproperty(QString, error_text)]
        /// The backend handle the video surface renders from, or zero. Clearing
        /// it is what stops the surface before anything it renders from is
        /// destroyed.
        #[qproperty(u64, render_handle)]
        type SideritaPlayer = super::SideritaPlayerRust;

        /// This path is not something the embedded player can show. The caller
        /// falls back to the ordinary quick-look card, with `reason` as the
        /// sentence to put on it.
        #[qsignal]
        fn preview_declined(self: Pin<&mut SideritaPlayer>, path: QString, reason: QString);

        /// Opens `path` in the embedded player when it is media this surface
        /// can honestly play. Anything else answers [`preview_declined`].
        #[qinvokable]
        fn request_preview(self: Pin<&mut SideritaPlayer>, path: &QString);

        /// Opens `path` in the standalone Fluorita application.
        ///
        /// Reports whether the launcher could be started at all; a missing
        /// binary is a truthful failure the caller falls back from, not a
        /// silent no-op.
        #[qinvokable]
        fn launch_standalone(self: Pin<&mut SideritaPlayer>, path: &QString) -> bool;

        /// Whether `path` is media this app would hand to Fluorita at all.
        /// Decided from the name alone, so it costs nothing.
        #[qinvokable]
        fn is_media(self: Pin<&mut SideritaPlayer>, path: &QString) -> bool;

        /// The surface built its render context: the backend may load now.
        /// With this output there is no video until a context exists, and a
        /// file loaded before then ends immediately with "nothing to play".
        #[qinvokable]
        fn surface_ready(self: Pin<&mut SideritaPlayer>);

        /// The surface released its context, so the session may be dropped —
        /// and a preview that was waiting for this may start.
        #[qinvokable]
        fn surface_released(self: Pin<&mut SideritaPlayer>);

        #[qinvokable]
        fn toggle(self: Pin<&mut SideritaPlayer>);

        #[qinvokable]
        fn seek(self: Pin<&mut SideritaPlayer>, milliseconds: i32);

        #[qinvokable]
        fn set_volume(self: Pin<&mut SideritaPlayer>, percent: i32);

        /// Closes the session and releases the backend. Idempotent: the modal
        /// calls it on every dismissal and on every selection step.
        #[qinvokable]
        fn close(self: Pin<&mut SideritaPlayer>);
    }

    impl cxx_qt::Threading for SideritaPlayer {}
}

/// What the worker is told to do.
enum Command {
    /// The surface is ready; load and play.
    Start,
    Transport(PlaybackRequest),
    Stop,
}

/// What one accepted report changes on screen.
struct Snapshot {
    state: PlaybackState,
    position: Duration,
    duration: Option<Duration>,
    volume: Option<f64>,
    pending: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct SideritaPlayerRust {
    active: bool,
    name: QString,
    kind: QString,
    state: QString,
    timed: bool,
    pending: bool,
    position_ms: i32,
    duration_ms: i32,
    volume_percent: i32,
    error_text: QString,
    render_handle: u64,

    /// A close is waiting for the surface to confirm it stopped rendering.
    closing: bool,
    /// What to open once that confirmation arrives. This is what keeps "one
    /// session at a time" true even though a video's teardown is asynchronous:
    /// stepping the selection queues the next item instead of racing it.
    pending_request: Option<PathBuf>,

    commands: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl qobject::SideritaPlayer {
    pub fn request_preview(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        // Whatever was playing goes first: one session, always. A video's
        // surface answers from the render thread, so if the close is still in
        // flight this request waits for it instead of starting beside it.
        self.as_mut().close();
        if self.rust().closing {
            self.as_mut().rust_mut().pending_request = Some(path);
            return;
        }
        self.as_mut().open(path);
    }

    fn open(mut self: core::pin::Pin<&mut Self>, path: PathBuf) {
        let Some(kind) = MediaKind::classify_path(&path) else {
            self.decline(&path, "Sin vista previa");
            return;
        };
        match kind {
            // Siderita already draws stills itself, with the toolkit and no
            // decoder — routing them here would be the cost the suite's
            // contract keeps out of a file manager.
            MediaKind::Image => {
                self.decline(&path, "La imagen se muestra aquí mismo");
                return;
            }
            // Video and audio both play here now; only the surface differs.
            MediaKind::Video | MediaKind::Audio => {}
        }

        let display = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        self.as_mut().set_name(QString::from(&display));
        self.as_mut()
            .set_kind(QString::from(if kind == MediaKind::Video {
                "vídeo"
            } else {
                "audio"
            }));
        self.as_mut().set_state(QString::from("abriendo"));
        self.as_mut().set_error_text(QString::default());
        self.as_mut().set_position_ms(0);
        self.as_mut().set_duration_ms(0);
        self.as_mut().set_volume_percent(100);
        self.as_mut().set_pending(false);
        self.as_mut().set_timed(kind.capabilities().timed);
        self.as_mut().set_active(true);

        let (sender, receiver) = mpsc::channel::<Command>();
        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("siderita-player".to_owned())
            .spawn(move || run_session(&path, kind, &receiver, &qt_thread));

        match worker {
            Ok(handle) => {
                self.as_mut().rust_mut().commands = Some(sender);
                self.as_mut().rust_mut().worker = Some(handle);
            }
            Err(_) => {
                self.as_mut().set_state(QString::from("error"));
                self.as_mut()
                    .set_error_text(QString::from("No se pudo iniciar la reproducción"));
            }
        }
    }

    pub fn launch_standalone(self: core::pin::Pin<&mut Self>, path: &QString) -> bool {
        let path = PathBuf::from(path.to_string());
        crate::controller::shell::spawn_detached("fluorita", &path).is_ok()
    }

    pub fn is_media(self: core::pin::Pin<&mut Self>, path: &QString) -> bool {
        MediaKind::classify_path(&PathBuf::from(path.to_string())).is_some()
    }

    pub fn toggle(self: core::pin::Pin<&mut Self>) {
        // Which way it goes is the backend's business; the transport asks for
        // the opposite of what the backend last confirmed.
        let playing = self.state() == &QString::from("reproduciendo");
        let request = if playing {
            PlaybackRequest::Pause
        } else {
            PlaybackRequest::Play
        };
        self.send(request);
    }

    pub fn seek(self: core::pin::Pin<&mut Self>, milliseconds: i32) {
        let target = Duration::from_millis(milliseconds.max(0).unsigned_abs().into());
        self.send(PlaybackRequest::Seek(target));
    }

    pub fn set_volume(self: core::pin::Pin<&mut Self>, percent: i32) {
        let level = f64::from(percent.clamp(0, 100)) / 100.0;
        self.send(PlaybackRequest::SetVolume(level));
    }

    /// The handle goes first: the surface must stop rendering before anything
    /// it renders from can be destroyed.
    pub fn close(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().worker.is_none() {
            self.as_mut().set_active(false);
            return;
        }
        if *self.render_handle() != 0 {
            self.as_mut().set_render_handle(0);
            self.as_mut().rust_mut().closing = true;
            // The surface answers from the render thread; `surface_released`
            // finishes the teardown.
            return;
        }
        self.as_mut().stop_worker();
    }

    pub fn surface_ready(mut self: core::pin::Pin<&mut Self>) {
        if let Some(commands) = self.as_mut().rust_mut().commands.as_ref() {
            let _ = commands.send(Command::Start);
        }
    }

    pub fn surface_released(mut self: core::pin::Pin<&mut Self>) {
        if !self.rust().closing {
            return;
        }
        self.as_mut().rust_mut().closing = false;
        self.as_mut().stop_worker();
        if let Some(path) = self.as_mut().rust_mut().pending_request.take() {
            self.open(path);
        }
    }

    /// Stops the worker and joins it. Synchronous by design: past this point no
    /// decoder of the previous item is alive.
    fn stop_worker(mut self: core::pin::Pin<&mut Self>) {
        if let Some(commands) = self.as_mut().rust_mut().commands.take() {
            let _ = commands.send(Command::Stop);
        }
        if let Some(handle) = self.as_mut().rust_mut().worker.take() {
            let _ = handle.join();
        }
        self.as_mut().set_active(false);
        self.as_mut().set_pending(false);
        self.as_mut().set_render_handle(0);
        self.as_mut().set_state(QString::default());
    }

    fn decline(mut self: core::pin::Pin<&mut Self>, path: &std::path::Path, reason: &str) {
        self.as_mut().set_active(false);
        let path = QString::from(path.to_string_lossy().as_ref());
        self.preview_declined(path, QString::from(reason));
    }

    fn send(mut self: core::pin::Pin<&mut Self>, request: PlaybackRequest) {
        if let Some(commands) = self.as_mut().rust_mut().commands.as_ref() {
            if commands.send(Command::Transport(request)).is_ok() {
                self.as_mut().set_pending(true);
            }
        }
    }

    /// Applies one accepted report. Runs on the GUI thread, through the queue.
    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: &Snapshot) {
        self.as_mut()
            .set_state(QString::from(state_label(snapshot.state)));
        self.as_mut()
            .set_position_ms(milliseconds(snapshot.position));
        if let Some(duration) = snapshot.duration {
            self.as_mut().set_duration_ms(milliseconds(duration));
        }
        if let Some(volume) = snapshot.volume {
            self.as_mut()
                .set_volume_percent((volume * 100.0).round() as i32);
        }
        self.as_mut().set_pending(snapshot.pending);
        if let Some(error) = &snapshot.error {
            self.as_mut().set_error_text(QString::from(error));
        }
    }
}

impl Drop for SideritaPlayerRust {
    fn drop(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(Command::Stop);
        }
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

/// The Spanish the modal shows for a confirmed state.
const fn state_label(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Idle => "",
        PlaybackState::Opening => "abriendo",
        PlaybackState::Playing => "reproduciendo",
        PlaybackState::Paused => "pausado",
        PlaybackState::Ended => "terminado",
        PlaybackState::Failed => "error",
    }
}

fn milliseconds(duration: Duration) -> i32 {
    i32::try_from(duration.as_millis()).unwrap_or(i32::MAX)
}

/// The session, on its own thread. Nothing here runs on the GUI thread.
fn run_session(
    path: &std::path::Path,
    kind: MediaKind,
    commands: &mpsc::Receiver<Command>,
    qt_thread: &cxx_qt::CxxQtThread<qobject::SideritaPlayer>,
) {
    let mut truth = PlaybackSession::new();
    let media = fluorita_core::MediaId::from_path(path);
    let Ok(generation) = truth.select(media, kind) else {
        publish_failure(qt_thread, "no se pudo iniciar la sesión");
        return;
    };

    let mut request = SessionRequest::new(path.to_path_buf(), generation);
    // Only a moving picture needs a surface; audio would pay for a render
    // context it never draws into.
    let presenting = kind.capabilities().has_video && kind != MediaKind::Image;
    if presenting {
        request = request.embedded_video();
    }

    let mut session = match MpvEngine::new().open_session(request) {
        Ok(session) => session,
        Err(error) => {
            publish_failure(qt_thread, &error.user_message());
            return;
        }
    };

    // Audio has nothing to wait for. Video waits for the surface's render
    // context, which arrives as `Command::Start`.
    if !presenting {
        if let Err(error) = session.start() {
            publish_failure(qt_thread, &error.user_message());
            return;
        }
    }

    if let Some(handle) = session.render_handle() {
        let address = handle.value();
        let _ = qt_thread.queue(move |mut player| player.as_mut().set_render_handle(address));
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
                    position: truth.position().unwrap_or(Duration::ZERO),
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

fn publish_failure(qt_thread: &cxx_qt::CxxQtThread<qobject::SideritaPlayer>, message: &str) {
    let message = message.to_owned();
    let _ = qt_thread.queue(move |mut player| {
        player.as_mut().set_state(QString::from("error"));
        player.as_mut().set_error_text(QString::from(&message));
    });
}
