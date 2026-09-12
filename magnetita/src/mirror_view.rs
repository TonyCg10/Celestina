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
        /// The mirror ended and every engine is gone: a mirror-only process
        /// may exit now, and not before, or libmpv aborts under a live render
        /// context.
        #[qproperty(bool, done)]
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

        /// The window is `width` by `height` and the picture's aspect says
        /// it should be `wanted` wide: asks the compositor for that width.
        #[qinvokable]
        fn fit(self: Pin<&mut MirrorView>, width: i32, height: i32, wanted: i32);
    }

    impl cxx_qt::Threading for MirrorView {}
}

/// The gap niri keeps around a tile, in logical pixels, as the author's
/// configuration sets it.
const NIRI_GAP: i32 = 12;

fn niri_action(args: &[&str]) {
    let _ = std::process::Command::new("niri")
        .args(["msg", "action"])
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// The logical width of `niri msg --json focused-output`.
fn niri_logical_width(json: &str) -> Option<i32> {
    let at = json.find("\"logical\":")?;
    let rest = &json[at..];
    let w = rest.find("\"width\":")?;
    rest[w + "\"width\":".len()..]
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// The id of this process's window whose title starts with `title`, in
/// `niri msg --json windows`; read without a JSON crate, the fields are
/// flat and named.
fn niri_window_id(json: &str, pid: u32, title: &str) -> Option<u64> {
    let mut at = 0;
    while let Some(rel) = json[at..].find("{\"id\":") {
        let start = at + rel;
        let end = json[start + 1..]
            .find("{\"id\":")
            .map(|r| start + 1 + r)
            .unwrap_or(json.len());
        let object = &json[start..end];
        at = end;
        let id: u64 = object
            .trim_start_matches("{\"id\":")
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()?;
        let mine = object.contains(&format!("\"pid\":{pid}"))
            && object.contains(&format!("\"title\":\"{title}"));
        if mine {
            return Some(id);
        }
    }
    None
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
    done: bool,
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
        // An engine still here at exit has a render context the render thread
        // owns; destroying it now is the abort libmpv promises. A leak at
        // exit costs nothing.
        if let Some(engine) = self.engine.take() {
            std::mem::forget(engine);
        }
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
/// least delay the demuxer and decoder allow, no audio. The core keeps its
/// own frame timing: the render surface presents on the host's schedule,
/// and the untimed and latency-hack modes left the output unconfigured
/// under it (the first frame never reached the surface).
fn engine_options(codec: &str) -> Vec<(&'static str, String)> {
    vec![
        ("vo", "libmpv".into()),
        // Software decoding on purpose: the picture is a phone's, which the
        // CPU decodes with time to spare, and a hardware surface the OpenGL
        // render cannot show is a black window with no error.
        ("hwdec", "no".into()),
        ("ao", "null".into()),
        ("cache", "no".into()),
        ("demuxer", "lavf".into()),
        ("demuxer-lavf-format", codec.into()),
        // Never `fflags=+nobuffer` here: on a FIFO, which cannot be read
        // twice, it makes lavf discard the packets it consumed while
        // probing (a second of them, the key frame included), and the
        // decoder then starts on a frame whose references it never saw.
        // Measured offline: 68 missing-reference errors with it, none
        // without. The probe is kept to the first unit instead.
        ("demuxer-lavf-o", "flags=+low_delay".into()),
        ("demuxer-lavf-analyzeduration", "0.02".into()),
        ("demuxer-lavf-probesize", "32".into()),
        ("demuxer-readahead-secs", "0".into()),
        ("vd-lavc-threads", "1".into()),
        ("vd-lavc-o", "flags=+low_delay".into()),
        // The stream carries no timestamps: timing by frame rate, as mpv
        // itself advises for it, and every frame shown as soon as it is
        // decoded rather than at a clock the stream cannot keep.
        ("container-fps-override", "60".into()),
        ("correct-pts", "no".into()),
        ("untimed", "yes".into()),
        ("keep-open", "yes".into()),
        // A still phone sends ten frames a second, not sixty: never pause to
        // fill a cache that a live stream cannot fill.
        ("cache-pause", "no".into()),
        ("cache-pause-initial", "no".into()),
        // The picture fills the surface whatever its shape: the window is
        // the compositor's to size, and a band would only hide the phone.
        ("keepaspect", "no".into()),
        // The engine's own log, complete, beside the FIFOs: the one place
        // that says what the demuxer and the decoder did with each unit.
        (
            "log-file",
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(format!(
                    "magnetita/mirror-window-{}.log",
                    std::process::id()
                ))
                .to_string_lossy()
                .into_owned(),
        ),
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
                    // start): let this engine go, then open the new FIFO. The
                    // mirror itself goes on, so `streaming` stays true.
                    self.as_mut().rust_mut().get_mut().reopen = Some(video);
                    self.as_mut().release_engine();
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
            eprintln!("magnetita: mirror: loading {video}");
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
        } else {
            self.as_mut().settle();
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
            let mut last_report = std::time::Instant::now();
            while guard.open() {
                if last_report.elapsed() > Duration::from_secs(5) {
                    last_report = std::time::Instant::now();
                    let time = client.get_property::<f64>("time-pos").unwrap_or(-1.0);
                    let width = client.get_property::<i64>("video-params/w").unwrap_or(-1);
                    let configured = client.get_property::<bool>("vo-configured").unwrap_or(false);
                    let idle = client.get_property::<bool>("core-idle").unwrap_or(false);
                    let waiting = client.get_property::<bool>("paused-for-cache").unwrap_or(false);
                    let cache = client.get_property::<f64>("demuxer-cache-duration").unwrap_or(-1.0);
                    let dropped = client.get_property::<i64>("frame-drop-count").unwrap_or(-1);
                    let paused = client.get_property::<bool>("pause").unwrap_or(false);
                    let eof = client.get_property::<bool>("eof-reached").unwrap_or(false);
                    let format = client.get_property::<String>("video-format").unwrap_or_default();
                    let seeking = client.get_property::<bool>("seeking").unwrap_or(false);
                    let vf_fps = client.get_property::<f64>("estimated-vf-fps").unwrap_or(-1.0);
                    let fps = client.get_property::<f64>("container-fps").unwrap_or(-1.0);
                    eprintln!(
                        "magnetita: mirror: time={time:.2} width={width} vo={configured} idle={idle} cache-wait={waiting} cache={cache:.2} dropped={dropped} pause={paused} eof={eof} format={format} seeking={seeking} vf-fps={vf_fps:.1} fps={fps:.1}"
                    );
                }
                let message = match client.wait_event(0.25) {
                    Some(Ok(Event::FileLoaded)) => {
                        eprintln!("magnetita: mirror: stream loaded");
                        Some(String::new())
                    }
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
            self.as_mut().set_done(true);
            return;
        }
        self.as_mut().release_engine();
    }

    /// Lets the engine go in the order the render seam needs: the handle
    /// first, the instance once the surface says its context is gone.
    fn release_engine(mut self: Pin<&mut Self>) {
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
            self.as_mut().settle();
        }
    }

    /// No engine remains: if the mirror is over too, the view is done.
    fn settle(mut self: Pin<&mut Self>) {
        if self.rust().engine.is_none() && !*self.streaming() && self.rust().reopen.is_none() {
            self.as_mut().set_done(true);
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

    /// On niri the layout owns the tile's size; what a window can ask is a
    /// column width and a window height, which `niri msg` grants. The
    /// picture's aspect decides: the width that fits the height, unless
    /// that overflows the output (a phone on its side), in which case the
    /// width is the output's and the height follows it. Elsewhere this
    /// does nothing.
    pub fn fit(self: Pin<&mut Self>, width: i32, height: i32, wanted: i32) {
        if (width - wanted).abs() <= 2 || wanted < 100 || height < 100 {
            return;
        }
        let pid = std::process::id();
        let aspect = f64::from(wanted) / f64::from(height);
        self.rust().owned.spawn(move |guard: Guard| {
            let Ok(out) = std::process::Command::new("niri")
                .args(["msg", "--json", "windows"])
                .stderr(std::process::Stdio::null())
                .output()
            else {
                return;
            };
            let text = String::from_utf8_lossy(&out.stdout);
            let Some(id) = niri_window_id(&text, pid, "Espejo") else {
                return;
            };
            // The output this window sits on: the widest a tile can be,
            // less niri's gaps either side.
            let output_width = std::process::Command::new("niri")
                .args(["msg", "--json", "focused-output"])
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .and_then(|out| niri_logical_width(&String::from_utf8_lossy(&out.stdout)))
                .map(|w| w - 2 * NIRI_GAP)
                .unwrap_or(i32::MAX);
            if !guard.open() {
                return;
            }
            let id = id.to_string();
            if wanted <= output_width {
                niri_action(&["set-window-width", "--id", &id, &wanted.to_string()]);
            } else {
                let fitted_height = (f64::from(output_width) / aspect).round() as i32;
                niri_action(&["set-window-width", "--id", &id, &output_width.to_string()]);
                niri_action(&["set-window-height", "--id", &id, &fitted_height.to_string()]);
            }
        });
    }

    pub fn global(mut self: Pin<&mut Self>, action: QString) {
        let action = action.to_string();
        if matches!(action.as_str(), "Back" | "Home" | "Recents") {
            self.as_mut().send(Outbound::Global(action));
        }
    }
}

#[cfg(test)]
mod niri {
    #[test]
    fn the_mirror_window_of_this_process_is_found_by_pid_and_title() {
        let json = r#"[{"id":7,"title":"Magnetita","app_id":"org.celestina.Magnetita","pid":42,"layout":{"window_size":[500,900]}},{"id":9,"title":"Espejo \u2014 Magnetita","app_id":"org.celestina.Magnetita","pid":42,"layout":{"window_size":[942,1010]}}]"#;
        assert_eq!(super::niri_window_id(json, 42, "Espejo"), Some(9));
        assert_eq!(super::niri_window_id(json, 42, "Magnetita"), Some(7));
        assert_eq!(super::niri_window_id(json, 43, "Espejo"), None);
    }

    #[test]
    fn the_focused_output_width_is_read() {
        let json = r#"{"name":"HDMI-A-1","logical":{"x":1920,"y":0,"width":2560,"height":1440,"scale":1.5}}"#;
        assert_eq!(super::niri_logical_width(json), Some(2560));
    }
}

#[cfg(test)]
mod probe {
    /// Feeds the synthetic sample through a FIFO into the engine with the
    /// window's options but no picture (`vo=null`), and reads what the core
    /// reports: the demuxer and decoder half of the window, without Qt.
    #[test]
    #[ignore]
    fn the_engine_decodes_the_fifo_without_a_window() {
        let sample = std::env::var("MIRROR_SAMPLE").expect("MIRROR_SAMPLE");
        let fifo =
            std::env::temp_dir().join(format!("magnetita-probe-{}.video", std::process::id()));
        let _ = std::fs::remove_file(&fifo);
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        let feed = fifo.clone();
        let sample_path = sample.clone();
        std::thread::spawn(move || {
            let bytes = std::fs::read(&sample_path).unwrap();
            let mut out = std::fs::OpenOptions::new().write(true).open(&feed).unwrap();
            use std::io::Write;
            for chunk in bytes.chunks(4096) {
                if out.write_all(chunk).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            // Keep the writer open so the stream looks live.
            std::thread::sleep(std::time::Duration::from_secs(6));
        });
        let mut options = super::engine_options("hevc");
        for (name, value) in options.iter_mut() {
            if *name == "vo" {
                *value = "null".into();
            }
        }
        let borrowed: Vec<(&str, &str)> = options.iter().map(|(n, v)| (*n, v.as_str())).collect();
        let engine = fluorita_engine::instance::Instance::new(&borrowed).unwrap();
        let client = engine.client().unwrap();
        engine
            .command("loadfile", &[fifo.to_str().unwrap(), "replace"])
            .unwrap();
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let time = client.get_property::<f64>("time-pos").unwrap_or(-1.0);
            let width = client.get_property::<i64>("video-params/w").unwrap_or(-1);
            let vo = client
                .get_property::<bool>("vo-configured")
                .unwrap_or(false);
            let idle = client.get_property::<bool>("core-idle").unwrap_or(false);
            let paused = client.get_property::<bool>("pause").unwrap_or(false);
            eprintln!("probe: time={time:.2} width={width} vo={vo} idle={idle} pause={paused}");
        }
        let _ = std::fs::remove_file(&fifo);
    }
}
