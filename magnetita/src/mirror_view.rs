//! The mirror as a window of this application: the daemon's `Mirror1`
//! streams the phone's picture into a FIFO, this object decodes it with the
//! suite's engine (libmpv, as Fluorita and Siderita do) into the shared
//! `MpvVideo` surface, and the window's pointer, wheel and keys go back to
//! the daemon as the wire's touches, swipes and keys. One worker thread
//! carries the D-Bus calls so no touch waits on the GUI thread, and one
//! watcher polls the daemon's link state so the window opens and closes
//! with the phone.

use core::pin::Pin;
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::lifecycle::{Guard, Owned};

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
        // draws into, the same one Fluorita's window and Siderita's preview use.
        include!("fluorita/mpvvideoitem.h");

        #[rust_name = "register_video_item"]
        fn register_fluorita_video_item(engine: Pin<&mut QQmlApplicationEngine>);
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        // streaming    — the daemon says the phone is streaming; the window shows
        // renderHandle — the engine instance the surface renders from (0: none)
        // pictureWidth/Height — the streamed picture, the pixels touches use
        // error        — why the picture cannot show, in the person's words
        #[qproperty(bool, streaming)]
        #[qproperty(u64, render_handle)]
        #[qproperty(i32, picture_width)]
        #[qproperty(i32, picture_height)]
        #[qproperty(QString, error)]
        type MirrorView = super::MirrorViewRust;

        /// Starts watching the daemon; the window follows its link state.
        #[qinvokable]
        fn start(self: Pin<&mut MirrorView>);

        /// The surface's render context exists: the engine may load.
        #[qinvokable]
        fn surface_ready(self: Pin<&mut MirrorView>);

        /// The surface's render context is gone: the engine may go.
        #[qinvokable]
        fn surface_released(self: Pin<&mut MirrorView>);

        /// The person closed the window: stop the link mirror.
        #[qinvokable]
        fn close_requested(self: Pin<&mut MirrorView>);

        /// A finger on the picture: `phase` 0 down, 1 move, 2 up, `x`/`y` in
        /// picture pixels.
        #[qinvokable]
        fn touch(self: Pin<&mut MirrorView>, phase: i32, x: i32, y: i32);

        /// A wheel tick at a picture point: `direction` 1 down, -1 up.
        #[qinvokable]
        fn wheel(self: Pin<&mut MirrorView>, direction: i32, x: i32, y: i32);

        /// A key of the window, Qt's key code, pressed or released.
        #[qinvokable]
        fn key(self: Pin<&mut MirrorView>, qt_key: i32, pressed: bool);

        /// `Back`, `Home` or `Recents`.
        #[qinvokable]
        fn global(self: Pin<&mut MirrorView>, action: QString);
    }

    impl cxx_qt::Threading for MirrorView {}
}

/// What the input worker sends the daemon, in order.
enum Outbound {
    Touch { action: u8, x: u16, y: u16 },
    Key { keycode: u16, pressed: bool },
    Global(String),
    Stop,
}

#[derive(Default)]
pub struct MirrorViewRust {
    streaming: bool,
    render_handle: u64,
    picture_width: i32,
    picture_height: i32,
    error: QString,
    /// The engine decoding the FIFO; alive from the daemon's `streaming` to
    /// the surface's release.
    engine: Option<fluorita_engine::instance::Instance>,
    /// The FIFO to load once the surface has its context.
    pending_video: Option<String>,
    /// The FIFO the engine is on, so a replaced one is noticed.
    current_video: Option<String>,
    /// A stream to open once the current engine has gone.
    reopen: Option<String>,
    /// A close waits for the surface to let the context go.
    closing: bool,
    input: Option<Sender<Outbound>>,
    input_worker: Option<JoinHandle<()>>,
    owned: Owned,
}

impl Drop for MirrorViewRust {
    fn drop(&mut self) {
        self.owned.close();
        self.input.take();
        if let Some(worker) = self.input_worker.take() {
            let _ = worker.join();
        }
    }
}

/// How often the daemon's link state is read.
const WATCH_INTERVAL: Duration = Duration::from_millis(400);

/// libmpv the way the mirror wants it: the raw stream from the FIFO, the
/// least delay the demuxer and decoder allow, no audio, no timing of its
/// own since every frame is late the moment it arrives.
fn engine_options(codec: &str) -> Vec<(&'static str, String)> {
    vec![
        ("vo", "libmpv".into()),
        ("hwdec", "auto-safe".into()),
        ("ao", "null".into()),
        ("untimed", "yes".into()),
        ("cache", "no".into()),
        ("demuxer", "lavf".into()),
        ("demuxer-lavf-format", codec.into()),
        ("demuxer-lavf-o", "fflags=+nobuffer,flags=+low_delay".into()),
        ("demuxer-lavf-analyzeduration", "1".into()),
        ("demuxer-readahead-secs", "0".into()),
        ("demuxer-thread", "no".into()),
        ("vd-lavc-threads", "1".into()),
        ("vd-lavc-o", "flags=+low_delay".into()),
        ("video-latency-hacks", "yes".into()),
        ("framedrop", "vo".into()),
        ("container-fps-override", "60".into()),
        ("keep-open", "yes".into()),
        // The picture fills the surface whatever its shape: the window is
        // the compositor's to size, and a band would only hide the phone.
        ("keepaspect", "no".into()),
    ]
}

impl qobject::MirrorView {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().input.is_some() {
            return;
        }
        // The input worker: every touch is one D-Bus call, off the GUI thread.
        let (tx, rx) = channel::<Outbound>();
        let worker = std::thread::spawn(move || {
            while let Ok(op) = rx.recv() {
                let result = match op {
                    Outbound::Touch { action, x, y } => {
                        crate::devices::mirror_link_touch(action, x, y)
                    }
                    Outbound::Key { keycode, pressed } => {
                        crate::devices::mirror_link_key(keycode, pressed)
                    }
                    Outbound::Global(action) => crate::devices::mirror_link_global(&action),
                    Outbound::Stop => crate::devices::mirror_stop(),
                };
                if let Err(error) = result {
                    eprintln!("magnetita: mirror input: {error}");
                }
            }
        });
        {
            let state = self.as_mut().rust_mut().get_mut();
            state.input = Some(tx);
            state.input_worker = Some(worker);
        }
        // The watcher: the daemon's link state, on its own thread, applied on
        // the GUI thread while this object is alive.
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard: Guard| {
            let mut last = String::new();
            while guard.open() {
                let now = crate::devices::mirror_link().unwrap_or_default();
                if now != last {
                    last = now.clone();
                    let _ = qt.queue(move |view: Pin<&mut qobject::MirrorView>| {
                        view.apply_link(now);
                    });
                }
                std::thread::sleep(WATCH_INTERVAL);
            }
        });
    }

    /// The daemon's link state, `state|video|width height`, applied whole.
    fn apply_link(mut self: Pin<&mut Self>, link: String) {
        let mut parts = link.split('|');
        let state = parts.next().unwrap_or("");
        let video = parts.next().unwrap_or("").to_owned();
        let picture = parts.next().unwrap_or("");
        let mut dims = picture
            .split_whitespace()
            .filter_map(|n| n.parse::<i32>().ok());
        let (width, height) = (dims.next().unwrap_or(0), dims.next().unwrap_or(0));
        match state {
            "streaming" if !video.is_empty() => {
                self.as_mut().set_picture_width(width);
                self.as_mut().set_picture_height(height);
                let replaced = self.rust().current_video.as_deref() != Some(video.as_str());
                if self.rust().engine.is_some() && replaced {
                    // The daemon started a new stream (a rotation, a second
                    // start): let this engine go, then open the new FIFO.
                    self.as_mut().rust_mut().get_mut().reopen = Some(video);
                    self.as_mut().close();
                } else if self.rust().engine.is_none() && !self.rust().closing {
                    self.as_mut().open(video);
                }
                self.as_mut().set_streaming(true);
            }
            "failed" => {
                self.as_mut()
                    .set_error(QString::from("El m\u{f3}vil no pudo empezar el espejo"));
                self.as_mut().close();
            }
            _ => {
                self.as_mut().set_error(QString::default());
                self.as_mut().close();
            }
        }
    }

    /// The engine over the FIFO; the load waits for the surface's context.
    fn open(mut self: Pin<&mut Self>, video: String) {
        let codec = if video.ends_with(".h264") {
            "h264"
        } else {
            "hevc"
        };
        let options = engine_options(codec);
        let borrowed: Vec<(&str, &str)> = options
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        match fluorita_engine::instance::Instance::new(&borrowed) {
            Ok(engine) => {
                let handle = engine.render_handle().value();
                self.as_mut().watch_engine(&engine);
                let state = self.as_mut().rust_mut().get_mut();
                state.engine = Some(engine);
                state.pending_video = Some(video.clone());
                state.current_video = Some(video);
                self.as_mut().set_error(QString::default());
                self.as_mut().set_render_handle(handle);
            }
            Err(error) => {
                eprintln!("magnetita: mirror engine: {error}");
                self.as_mut()
                    .set_error(QString::from("No se pudo abrir el decodificador"));
            }
        }
    }

    pub fn surface_ready(mut self: Pin<&mut Self>) {
        let video = self.as_mut().rust_mut().get_mut().pending_video.take();
        if let (Some(video), Some(engine)) = (video, self.rust().engine.as_ref()) {
            if let Err(error) = engine.command("loadfile", &[&video, "replace"]) {
                eprintln!("magnetita: mirror load: {error}");
            }
        }
    }

    pub fn surface_released(mut self: Pin<&mut Self>) {
        if !self.rust().closing {
            return;
        }
        let reopen = {
            let state = self.as_mut().rust_mut().get_mut();
            state.closing = false;
            state.engine = None;
            state.current_video = None;
            state.reopen.take()
        };
        if let Some(video) = reopen {
            self.as_mut().open(video);
        }
    }

    /// The engine's own events, so the window can say why there is no
    /// picture: the stream ending, or failing to load.
    fn watch_engine(mut self: Pin<&mut Self>, engine: &fluorita_engine::instance::Instance) {
        let Ok(client) = engine.client() else {
            return;
        };
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard: Guard| {
            use libmpv2::events::Event;
            while guard.open() {
                let message = match client.wait_event(0.25) {
                    Some(Ok(Event::FileLoaded)) => Some(String::new()),
                    Some(Ok(Event::EndFile(reason))) => {
                        Some(format!("El v\u{ed}deo termin\u{f3}: {reason:?}"))
                    }
                    Some(Ok(Event::Shutdown)) | Some(Err(_)) => break,
                    _ => None,
                };
                if let Some(message) = message {
                    let _ = qt.queue(move |mut view: Pin<&mut qobject::MirrorView>| {
                        view.as_mut().set_error(QString::from(&message));
                    });
                }
            }
        });
    }

    /// Lets the surface go first, then the engine: the render API frees its
    /// context on the render thread before the instance may be destroyed.
    fn close(mut self: Pin<&mut Self>) {
        self.as_mut().set_streaming(false);
        if self.rust().engine.is_none() {
            return;
        }
        if *self.render_handle() != 0 {
            self.as_mut().rust_mut().get_mut().closing = true;
            self.as_mut().set_render_handle(0);
        } else {
            let state = self.as_mut().rust_mut().get_mut();
            state.engine = None;
            state.current_video = None;
        }
    }

    pub fn close_requested(mut self: Pin<&mut Self>) {
        self.as_mut().send(Outbound::Stop);
        self.as_mut().close();
    }

    fn send(self: Pin<&mut Self>, op: Outbound) {
        if let Some(input) = &self.rust().input {
            let _ = input.send(op);
        }
    }

    pub fn touch(mut self: Pin<&mut Self>, phase: i32, x: i32, y: i32) {
        let (width, height) = (*self.picture_width(), *self.picture_height());
        if width <= 0 || height <= 0 {
            return;
        }
        let action = match phase {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        let px = x.clamp(0, width - 1) as u16;
        let py = y.clamp(0, height - 1) as u16;
        self.as_mut().send(Outbound::Touch {
            action,
            x: px,
            y: py,
        });
    }

    /// A wheel tick is a short swipe: a fifth of the picture, in six steps,
    /// the finger moving against the wheel as a hand would.
    pub fn wheel(mut self: Pin<&mut Self>, direction: i32, x: i32, y: i32) {
        let (width, height) = (*self.picture_width(), *self.picture_height());
        if width <= 0 || height <= 0 {
            return;
        }
        let px = x.clamp(0, width - 1) as u16;
        let travel = i64::from(height) / 5;
        let dir = i64::from(direction.signum());
        let at =
            |i: i64| (i64::from(y) - dir * travel * i / 6).clamp(0, i64::from(height) - 1) as u16;
        self.as_mut().send(Outbound::Touch {
            action: 0,
            x: px,
            y: at(0),
        });
        for i in 1..=6 {
            self.as_mut().send(Outbound::Touch {
                action: 1,
                x: px,
                y: at(i),
            });
        }
        self.as_mut().send(Outbound::Touch {
            action: 2,
            x: px,
            y: at(6),
        });
    }

    pub fn key(mut self: Pin<&mut Self>, qt_key: i32, pressed: bool) {
        if let Some(keycode) = crate::projection::android_keycode(qt_key) {
            self.as_mut().send(Outbound::Key { keycode, pressed });
        }
    }

    pub fn global(mut self: Pin<&mut Self>, action: QString) {
        let action = action.to_string();
        if matches!(action.as_str(), "Back" | "Home" | "Recents") {
            self.as_mut().send(Outbound::Global(action));
        }
    }
}
