//! The window-wide controller, a QML singleton.
//!
//! It carries the appearance to the window (`appearanceReducedMotion` and
//! `appearanceTextScale`, handed to `CelestinaAppearance`), the smoke and
//! fake switches, the capture card's choices, the recording card's state,
//! the history and the outputs. Every capture and every history action is
//! queued to the capture worker (`capture.rs`) and every recording to the
//! recording worker (`record.rs`); their reports come back through the Qt
//! thread's queue. The singleton lives as long as the engine, so its
//! follower and its workers live until the process exits.

use std::pin::Pin;
use std::time::UNIX_EPOCH;

use celestina_core::file_uri;
use celestina_settings::Follower;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant};
use selenita_core::target::delay_from_seconds;
use selenita_core::tools::slurp_colour;
use selenita_core::{Entry, TargetKind};

use crate::appearance::{self, Values};
use crate::capture::{Action, CaptureError, Report, Request, Worker};
use crate::record;

/// slurp's colours when QML hands over something that is not a colour: the
/// theme's accent and a quarter-black veil.
const FALLBACK_ACCENT: &str = "#3e91ffff";
const FALLBACK_BACKGROUND: &str = "#00000040";

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(
            bool,
            appearance_reduced_motion,
            cxx_name = "appearanceReducedMotion",
            READ,
            NOTIFY
        )]
        #[qproperty(
            f64,
            appearance_text_scale,
            cxx_name = "appearanceTextScale",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, smoke_report, cxx_name = "smokeReport", READ, CONSTANT)]
        #[qproperty(bool, fake, READ, CONSTANT)]
        // This launch came from `--screenshot` with no Selenita running: the
        // window starts hidden, takes the capture and shows on the history.
        #[qproperty(bool, launch_capture, cxx_name = "launchCapture", READ, CONSTANT)]
        // The capture card's choices: `screen`, `window` or `region`; the
        // delay in seconds (0, 3, 5 or 10); the two destinations; the output
        // a screen capture takes (empty: every output).
        #[qproperty(QString, target, READ, WRITE, NOTIFY)]
        #[qproperty(i32, delay, READ, WRITE, NOTIFY)]
        #[qproperty(bool, to_clipboard, cxx_name = "toClipboard", READ, WRITE, NOTIFY)]
        #[qproperty(bool, to_file, cxx_name = "toFile", READ, WRITE, NOTIFY)]
        #[qproperty(QString, screen_output, cxx_name = "screenOutput", READ, WRITE, NOTIFY)]
        // Product copy from QML (`qsTr`): the file name's stem and the
        // folder inside the pictures folder.
        #[qproperty(QString, file_stem, cxx_name = "fileStem", READ, WRITE, NOTIFY)]
        #[qproperty(QString, folder_name, cxx_name = "folderName", READ, WRITE, NOTIFY)]
        // The enabled outputs' names, left to right.
        #[qproperty(QVariant, outputs, READ, NOTIFY)]
        // The history, the latest first: `{ id, name, url, kind, size,
        // takenAt }` with `takenAt` in milliseconds since the epoch.
        #[qproperty(QVariant, history, READ, NOTIFY)]
        // A capture is under way.
        #[qproperty(bool, busy, READ, NOTIFY)]
        // The recording: `idle`, `preparing` (the portal's dialog),
        // `recording` or `stopping`; the system's sound switch; when the
        // recording under way began (milliseconds since the epoch); the
        // last recording's history id and file name; what the host lacks
        // for recording (an element name, empty when nothing).
        #[qproperty(QString, recording_state, cxx_name = "recordingState", READ, NOTIFY)]
        #[qproperty(bool, with_audio, cxx_name = "withAudio", READ, WRITE, NOTIFY)]
        #[qproperty(
            f64,
            recording_started_at,
            cxx_name = "recordingStartedAt",
            READ,
            NOTIFY
        )]
        #[qproperty(QString, last_recording_id, cxx_name = "lastRecordingId", READ, NOTIFY)]
        #[qproperty(
            QString,
            last_recording_name,
            cxx_name = "lastRecordingName",
            READ,
            NOTIFY
        )]
        #[qproperty(QString, recorder_missing, cxx_name = "recorderMissing", READ, NOTIFY)]
        // Product copy from QML (`qsTr`): the recording file name's stem.
        #[qproperty(
            QString,
            recording_stem,
            cxx_name = "recordingStem",
            READ,
            WRITE,
            NOTIFY
        )]
        type SelenitaController = super::SelenitaControllerRust;

        /// A sentence for the window's notice pill; `kind` is `info` or
        /// `error`.
        #[qsignal]
        fn notice(self: Pin<&mut SelenitaController>, kind: QString, text: QString);
        /// Seconds left before the picture is taken; 0 when it is taken.
        #[qsignal]
        fn countdown(self: Pin<&mut SelenitaController>, seconds: i32);
        /// A capture finished; `entry_id` is its history row, empty when it
        /// went to the clipboard only.
        #[qsignal]
        fn captured(self: Pin<&mut SelenitaController>, entry_id: QString);
        /// A launch's capture is over: the window, hidden until now, shows
        /// on the history.
        #[qsignal]
        #[cxx_name = "showWindowRequested"]
        fn show_window_requested(self: Pin<&mut SelenitaController>);
        /// A recording was published; `entry_id` is its history row.
        #[qsignal]
        #[cxx_name = "recordingFinished"]
        fn recording_finished(self: Pin<&mut SelenitaController>, entry_id: QString);

        /// Takes a capture with the card's choices; `accent` and
        /// `background` are the theme's colours for the region selector.
        /// The window stays where it is: a capture may include it.
        #[qinvokable]
        fn capture(self: Pin<&mut SelenitaController>, accent: &QString, background: &QString);
        #[qinvokable]
        #[cxx_name = "openInFluorita"]
        fn open_in_fluorita(self: Pin<&mut SelenitaController>, id: &QString);
        #[qinvokable]
        fn copy(self: Pin<&mut SelenitaController>, id: &QString);
        #[qinvokable]
        #[cxx_name = "showInSiderita"]
        fn show_in_siderita(self: Pin<&mut SelenitaController>, id: &QString);
        /// Moves the entry's file to the trash and forgets it.
        #[qinvokable]
        #[cxx_name = "deleteEntry"]
        fn delete_entry(self: Pin<&mut SelenitaController>, id: &QString);
        /// Starts a recording with the card's choices, or stops the one
        /// under way.
        #[qinvokable]
        #[cxx_name = "toggleRecording"]
        fn toggle_recording(self: Pin<&mut SelenitaController>);
        /// Stops the recording under way; nothing happens when none is.
        #[qinvokable]
        #[cxx_name = "stopRecording"]
        fn stop_recording(self: Pin<&mut SelenitaController>);
    }

    impl cxx_qt::Threading for SelenitaController {}
    impl cxx_qt::Initialize for SelenitaController {}
}

/// `smokeReport` is `SELENITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to take one capture over the fakes and print what it shows. `fake`
/// is `SELENITA_FAKE=1`: the capture worker drives the fake backend
/// (`backend.rs`) instead of niri, `grim`, `slurp` and `wl-copy`.
pub struct SelenitaControllerRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    smoke_report: bool,
    fake: bool,
    target: QString,
    delay: i32,
    to_clipboard: bool,
    to_file: bool,
    screen_output: QString,
    file_stem: QString,
    folder_name: QString,
    outputs: QVariant,
    history: QVariant,
    busy: bool,
    recording_state: QString,
    with_audio: bool,
    recording_started_at: f64,
    last_recording_id: QString,
    last_recording_name: QString,
    recorder_missing: QString,
    recording_stem: QString,
    launch_capture: bool,
    /// The window has not shown yet and shows when the launch's capture
    /// ends.
    reveal: bool,
    worker: Option<Worker>,
    recorder: Option<record::Worker>,
    /// Held for the singleton's life; dropping it stops the follower.
    follower: Option<Follower>,
}

impl Default for SelenitaControllerRust {
    fn default() -> Self {
        let initial = appearance::initial();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale,
            smoke_report: std::env::var_os("SELENITA_SMOKE_REPORT").is_some(),
            fake: fake_requested(std::env::var_os("SELENITA_FAKE").as_deref()),
            target: QString::from(TargetKind::Screen.as_str()),
            delay: 0,
            to_clipboard: true,
            to_file: true,
            screen_output: QString::default(),
            file_stem: QString::default(),
            folder_name: QString::default(),
            // Empty lists, not invalid variants: QML reads `.length`.
            outputs: string_list(&[]),
            history: history_rows(&[]),
            busy: false,
            recording_state: QString::from(record_state_word(selenita_core::record::State::Idle)),
            with_audio: false,
            recording_started_at: 0.0,
            last_recording_id: QString::default(),
            last_recording_name: QString::default(),
            recorder_missing: QString::default(),
            recording_stem: QString::default(),
            launch_capture: crate::activation::capture_waiting(),
            reveal: crate::activation::capture_waiting(),
            worker: None,
            recorder: None,
            follower: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::SelenitaController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = appearance::follow(move |values| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::SelenitaController>| {
                controller.apply_appearance(values);
            });
        });
        self.as_mut().rust_mut().follower = Some(follower);

        let qt = self.qt_thread();
        let worker = Worker::start(self.rust().fake, move |report| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::SelenitaController>| {
                controller.apply(report);
            });
        });
        self.as_mut().rust_mut().worker = worker;

        let qt = self.qt_thread();
        let recorder = record::Worker::start(self.rust().fake, move |report| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::SelenitaController>| {
                controller.apply_recording(report);
            });
        });
        self.as_mut().rust_mut().recorder = recorder;
    }
}

impl qobject::SelenitaController {
    fn apply_appearance(mut self: Pin<&mut Self>, values: Values) {
        if self.rust().appearance_reduced_motion != values.reduced_motion {
            self.as_mut().rust_mut().appearance_reduced_motion = values.reduced_motion;
            self.as_mut().appearance_reduced_motion_changed();
        }
        if self.rust().appearance_text_scale != values.text_scale {
            self.as_mut().rust_mut().appearance_text_scale = values.text_scale;
            self.as_mut().appearance_text_scale_changed();
        }
    }

    fn apply(mut self: Pin<&mut Self>, report: Report) {
        match report {
            Report::History(entries) => {
                // The last recording's line and «Abrir en Fluorita» follow
                // the history: trashed (or gone), they go.
                let last = self.rust().last_recording_id.to_string();
                if !last_recording_listed(&last, &entries) {
                    self.as_mut().forget_last_recording();
                }
                self.as_mut().rust_mut().history = history_rows(&entries);
                self.as_mut().history_changed();
            }
            Report::Outputs(names) => {
                self.as_mut().rust_mut().outputs = string_list(&names);
                self.as_mut().outputs_changed();
            }
            Report::Countdown(left) => {
                self.as_mut().countdown(i32::try_from(left).unwrap_or(0));
            }
            Report::Captured(id) => {
                self.as_mut().end_capture();
                let id = id.unwrap_or_default();
                self.as_mut().captured(QString::from(id.as_str()));
            }
            Report::Cancelled => self.as_mut().end_capture(),
            Report::CaptureFailed(text) => {
                self.as_mut().end_capture();
                self.as_mut().say_error(&text);
            }
            Report::Failed(text) => self.as_mut().say_error(&text),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn apply_recording(mut self: Pin<&mut Self>, report: record::Report) {
        match report {
            record::Report::Missing(missing) => {
                let element = missing.unwrap_or_default();
                self.as_mut().rust_mut().recorder_missing = QString::from(element);
                self.as_mut().recorder_missing_changed();
                if let Some(element) = missing {
                    let error = record::RecordError::Missing(element.to_owned());
                    self.as_mut().say_error(&error.message_es());
                }
            }
            record::Report::State(state) => {
                // A failure is said by its own report; the card only sees
                // the states a recording rests in.
                if state == selenita_core::record::State::Failed {
                    return;
                }
                let word = QString::from(record_state_word(state));
                if self.rust().recording_state != word {
                    self.as_mut().rust_mut().recording_state = word;
                    self.as_mut().recording_state_changed();
                }
            }
            record::Report::Started(when) => {
                let millis = when
                    .duration_since(UNIX_EPOCH)
                    .map_or(0.0, |elapsed| elapsed.as_millis() as f64);
                self.as_mut().rust_mut().recording_started_at = millis;
                self.as_mut().recording_started_at_changed();
            }
            record::Report::Finished(entry) => {
                let id = entry.id();
                let name = entry
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.as_mut().rust_mut().last_recording_id = QString::from(id.as_str());
                self.as_mut().last_recording_id_changed();
                self.as_mut().rust_mut().last_recording_name = QString::from(name.as_str());
                self.as_mut().last_recording_name_changed();
                // The capture worker owns the history: the entry joins it
                // there and comes back as the next `History` report.
                let pushed = self
                    .rust()
                    .worker
                    .as_ref()
                    .ok_or(CaptureError::WorkerGone)
                    .and_then(|worker| worker.push(entry));
                if let Err(error) = pushed {
                    self.as_mut().say_error(&error.message_es());
                }
                self.as_mut().recording_finished(QString::from(id.as_str()));
            }
            record::Report::Failed(text) => self.as_mut().say_error(&text),
        }
    }

    fn say_error(self: Pin<&mut Self>, text: &str) {
        self.notice(QString::from("error"), QString::from(text));
    }

    fn set_busy(mut self: Pin<&mut Self>, busy: bool) {
        if self.rust().busy != busy {
            self.as_mut().rust_mut().busy = busy;
            self.as_mut().busy_changed();
        }
    }

    /// The capture is over, however it ended: a launch's window, hidden
    /// until its capture, shows now.
    fn end_capture(mut self: Pin<&mut Self>) {
        self.as_mut().set_busy(false);
        if std::mem::take(&mut self.as_mut().rust_mut().reveal) {
            self.as_mut().show_window_requested();
        }
    }

    /// The last recording's id and name go: its row left the history.
    fn forget_last_recording(mut self: Pin<&mut Self>) {
        if !self.rust().last_recording_id.is_empty() {
            self.as_mut().rust_mut().last_recording_id = QString::default();
            self.as_mut().last_recording_id_changed();
        }
        if !self.rust().last_recording_name.is_empty() {
            self.as_mut().rust_mut().last_recording_name = QString::default();
            self.as_mut().last_recording_name_changed();
        }
    }

    pub fn capture(mut self: Pin<&mut Self>, accent: &QString, background: &QString) {
        if self.rust().busy {
            self.say_error(&CaptureError::Busy.message_es());
            return;
        }
        let kind = self
            .rust()
            .target
            .to_string()
            .parse::<TargetKind>()
            .unwrap_or(TargetKind::Screen);
        let request = Request {
            kind,
            output: self.rust().screen_output.to_string(),
            delay: delay_from_seconds(i64::from(self.rust().delay)),
            to_clipboard: self.rust().to_clipboard,
            to_file: self.rust().to_file,
            accent: slurp_colour(&accent.to_string()).unwrap_or_else(|| FALLBACK_ACCENT.to_owned()),
            background: slurp_colour(&background.to_string())
                .unwrap_or_else(|| FALLBACK_BACKGROUND.to_owned()),
            stem: self.rust().file_stem.to_string(),
            folder: self.rust().folder_name.to_string(),
        };
        if !request.to_clipboard && !request.to_file && kind != TargetKind::Window {
            self.say_error(&CaptureError::NoDestination.message_es());
            return;
        }
        let queued = self
            .rust()
            .worker
            .as_ref()
            .ok_or(CaptureError::WorkerGone)
            .and_then(|worker| worker.capture(request));
        if let Err(error) = queued {
            self.say_error(&error.message_es());
            return;
        }
        self.as_mut().set_busy(true);
    }

    fn act(self: Pin<&mut Self>, action: Action, id: &QString) {
        let queued = self
            .rust()
            .worker
            .as_ref()
            .ok_or(CaptureError::WorkerGone)
            .and_then(|worker| worker.act(action, id.to_string()));
        if let Err(error) = queued {
            self.say_error(&error.message_es());
        }
    }

    pub fn open_in_fluorita(self: Pin<&mut Self>, id: &QString) {
        self.act(Action::OpenInFluorita, id);
    }

    pub fn copy(self: Pin<&mut Self>, id: &QString) {
        self.act(Action::Copy, id);
    }

    pub fn show_in_siderita(self: Pin<&mut Self>, id: &QString) {
        self.act(Action::ShowInSiderita, id);
    }

    pub fn delete_entry(self: Pin<&mut Self>, id: &QString) {
        self.act(Action::Delete, id);
    }

    pub fn toggle_recording(self: Pin<&mut Self>) {
        let request = record::Request {
            audio: self.rust().with_audio,
            stem: self.rust().recording_stem.to_string(),
        };
        let queued = self
            .rust()
            .recorder
            .as_ref()
            .ok_or(record::RecordError::WorkerGone)
            .and_then(|recorder| recorder.toggle(request));
        if let Err(error) = queued {
            self.say_error(&error.message_es());
        }
    }

    pub fn stop_recording(self: Pin<&mut Self>) {
        let queued = self
            .rust()
            .recorder
            .as_ref()
            .ok_or(record::RecordError::WorkerGone)
            .and_then(record::Worker::stop);
        if let Err(error) = queued {
            self.say_error(&error.message_es());
        }
    }
}

/// The word QML reads for a recording state.
fn record_state_word(state: selenita_core::record::State) -> &'static str {
    state.as_str()
}

/// Whether the last recording (`id`, empty when there is none) still has
/// its row in `entries`; an empty id has nothing to lose.
fn last_recording_listed(id: &str, entries: &[Entry]) -> bool {
    id.is_empty() || entries.iter().any(|entry| entry.id() == id)
}

fn string_list(items: &[String]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for item in items {
        list.append(QVariant::from(&QString::from(item.as_str())));
    }
    QVariant::from(&list)
}

/// Milliseconds since the epoch, as QML's `Date` takes them.
#[allow(clippy::cast_precision_loss)]
fn millis(entry: &Entry) -> f64 {
    entry
        .taken_at
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_millis() as f64)
}

#[allow(clippy::cast_precision_loss)]
fn history_rows(entries: &[Entry]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for entry in entries {
        let mut map = QMap::<QMapPair_QString_QVariant>::default();
        let text = |value: &str| QVariant::from(&QString::from(value));
        map.insert(QString::from("id"), text(&entry.id()));
        let name = entry
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        map.insert(QString::from("name"), text(&name));
        map.insert(
            QString::from("url"),
            text(&file_uri::from_path(&entry.path).unwrap_or_default()),
        );
        map.insert(QString::from("kind"), text(entry.kind.as_str()));
        map.insert(QString::from("size"), QVariant::from(&(entry.size as f64)));
        map.insert(QString::from("takenAt"), QVariant::from(&millis(entry)));
        list.append(QVariant::from(&map));
    }
    QVariant::from(&list)
}

/// Only the exact value `1` asks for the fakes, so a stray empty or `0` never
/// does.
fn fake_requested(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::{fake_requested, last_recording_listed};
    use selenita_core::{Entry, EntryKind};
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn only_one_asks_for_the_fakes() {
        assert!(fake_requested(Some(OsStr::new("1"))));
        assert!(!fake_requested(Some(OsStr::new("0"))));
        assert!(!fake_requested(Some(OsStr::new(""))));
        assert!(!fake_requested(None));
    }

    /// The last recording's line follows its history row: once the row is
    /// gone (trashed, or pruned at load), the card has nothing to open.
    #[test]
    fn the_last_recording_is_forgotten_with_its_row() {
        let clip = Entry {
            path: PathBuf::from("/videos/Recordings/Clip 1.mp4"),
            kind: EntryKind::Recording,
            taken_at: SystemTime::UNIX_EPOCH,
            size: 3,
        };
        let shot = Entry {
            path: PathBuf::from("/pictures/Capturas/Shot 1.png"),
            kind: EntryKind::Screenshot,
            taken_at: SystemTime::UNIX_EPOCH,
            size: 3,
        };
        assert!(last_recording_listed(
            &clip.id(),
            &[shot.clone(), clip.clone()]
        ));
        assert!(!last_recording_listed(&clip.id(), &[shot]));
        assert!(!last_recording_listed(&clip.id(), &[]));
        assert!(last_recording_listed("", &[]));
    }
}
