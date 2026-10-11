//! The Qt half of trimming one video.
//!
//! What a trim *is* — the span, the encoder rule, the child's argument list —
//! lives in `fluorita_core::trim`; running the child and landing its file is
//! [`run`], on a worker this object owns. This file moves values between
//! them and QML under the editor's rules:
//!
//! - **The GUI thread never encodes, waits or touches a file.** A save starts
//!   a worker and returns; progress and the outcome arrive through the
//!   queue.
//! - **The span is checked by the domain.** QML moves the handles; whether
//!   the span is one a trim can save, and whether it is the whole film (and
//!   so saves nothing), is [`Span`]'s answer, published as `edited`. The
//!   shortest span, and the handles' step, is one frame of the film's own
//!   rate, which `ffprobe` gives on a worker when the trim opens.
//! - **What the result leaves out is said before it is saved.** A trim
//!   writes MP4 with the main video and one audio stream; a film in another
//!   container, or with more streams than that, gets a notice
//!   (`container_notice`), as the picture editor says a format change.
//! - **The two outcomes are the picture editor's** (ADR 0009, `PRV-1`):
//!   «Guardar ambas» lands a copy beside the original and hands it to
//!   Selenita's adoption; «Guardar solo la editada» sends the original to the
//!   Trash, through the same seam, once the result exists. The original is
//!   never written to, and a failure leaves it as it was.
//! - **A path is a key, not text** (ADR 0008).

use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;

use celestina_core::{pathkey, CancellationToken};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use fluorita_core::trim::{SourceFacts, Span, TrimError, MIN_SPAN};
use fluorita_core::{MediaKind, SaveChoice};
use fluorita_engine::DesktopTrash;

mod copy;
mod run;

use run::{choose_encoder, probe_source, run, Failure, Seams, Tool, TrimJob};

/// Whether a film is already an MP4 by its name: a trim always writes one.
fn is_mp4(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        /// The trim of one video, for the edit window.
        #[qobject]
        #[qml_element]
        /// The video's path key, set by `open`.
        #[qproperty(QString, key)]
        /// The film's length as the player confirmed it, in seconds; `0`
        /// until it did.
        #[qproperty(f64, length_seconds)]
        /// The span kept, in seconds of the film.
        #[qproperty(f64, trim_start)]
        #[qproperty(f64, trim_end)]
        /// One frame of the film, in seconds: the shortest span a trim
        /// accepts and the handles' step. A fallback until the film is
        /// described.
        #[qproperty(f64, minimum_seconds)]
        /// Set when the result will not be the original's container or will
        /// leave streams out; empty when it keeps everything it can hold.
        #[qproperty(QString, container_notice)]
        /// True when the span is one a trim can save and is not the whole
        /// film: a trim with its handles at the ends saves nothing.
        #[qproperty(bool, edited)]
        /// True while the child runs or its file lands.
        #[qproperty(bool, saving)]
        /// The share of the span written, `0.0..=1.0`, while saving.
        #[qproperty(f64, progress)]
        /// What the last save said, or why it was refused; empty otherwise.
        #[qproperty(QString, notice)]
        /// The file the last save wrote, as its path key and as a `file://`
        /// URL for a drag, until the next open. Empty before any save.
        #[qproperty(QString, saved_key)]
        #[qproperty(QString, saved_url)]
        type FluoritaTrim = super::TrimRust;

        /// Starts over on this video, by its path key.
        #[qinvokable]
        fn open(self: Pin<&mut FluoritaTrim>, key: &QString);

        /// The length the player confirmed. The first one sets the span to
        /// the whole film; a later one keeps the span inside it.
        #[qinvokable]
        fn set_length(self: Pin<&mut FluoritaTrim>, seconds: f64);

        /// Moves the handles, in seconds; each is kept inside the film.
        #[qinvokable]
        fn set_span(self: Pin<&mut FluoritaTrim>, start: f64, end: f64);

        /// Writes the span. `replace` sends the original to the Trash once the
        /// result exists; otherwise a copy lands beside it. Returns at once.
        #[qinvokable]
        fn save_trim(self: Pin<&mut FluoritaTrim>, replace: bool);

        /// Stops a save in flight: the child is killed and its file removed.
        #[qinvokable]
        fn cancel(self: Pin<&mut FluoritaTrim>);

        /// Stops whatever runs and forgets the video.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaTrim>);
    }

    impl cxx_qt::Threading for FluoritaTrim {}
    impl cxx_qt::Initialize for FluoritaTrim {}
}

#[derive(Default)]
pub struct TrimRust {
    key: QString,
    length_seconds: f64,
    trim_start: f64,
    trim_end: f64,
    minimum_seconds: f64,
    container_notice: QString,
    edited: bool,
    saving: bool,
    progress: f64,
    notice: QString,
    saved_key: QString,
    saved_url: QString,

    worker: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
    /// The film's description in flight, its own cancellation, and which
    /// open it answers: an answer for a film since closed is dropped.
    prober: Option<JoinHandle<()>>,
    probe_cancellation: CancellationToken,
    opening: u64,
}

impl Drop for TrimRust {
    fn drop(&mut self) {
        // The child is killed within one poll and its file removed; the join
        // waits for that, so no thread or child outlives the window.
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.probe_cancellation.cancel();
        if let Some(prober) = self.prober.take() {
            let _ = prober.join();
        }
    }
}

impl cxx_qt::Initialize for qobject::FluoritaTrim {
    fn initialize(self: std::pin::Pin<&mut Self>) {
        self.set_minimum_seconds(MIN_SPAN.as_secs_f64());
    }
}

/// A number of seconds from QML as a duration; anything that is not one is
/// zero, which no span accepts as an end.
fn duration(seconds: f64) -> Duration {
    fluorita_core::duration_from_seconds(seconds).unwrap_or(Duration::ZERO)
}

impl qobject::FluoritaTrim {
    pub fn open(mut self: std::pin::Pin<&mut Self>, key: &QString) {
        self.as_mut().close();
        self.as_mut().set_key(key.clone());
        let Ok(source) = pathkey::decode(&key.to_string()) else {
            return;
        };
        // The container is known from the name, at once; what the film holds
        // is asked of `ffprobe` on a worker, and refines the notice.
        let container_changes = !is_mp4(&source);
        self.as_mut()
            .set_container_notice(QString::from(&copy::notice(container_changes, 0)));
        let generation = self.rust().opening;
        let cancellation = self.rust().probe_cancellation.clone();
        let qt_thread = self.qt_thread();
        let prober = std::thread::Builder::new()
            .name("fluorita-trim-probe".to_owned())
            .spawn(move || {
                let facts = match probe_source(&Tool::probe(), &source, &cancellation) {
                    Ok(facts) => facts,
                    Err(error) => {
                        // The save says what is missing; the trim goes on
                        // with the fallback frame.
                        eprintln!("fluorita: the film could not be described: {error}");
                        return;
                    }
                };
                let _ = qt_thread.queue(move |trim| trim.described(generation, facts));
            });
        match prober {
            Ok(handle) => self.as_mut().rust_mut().prober = Some(handle),
            Err(error) => eprintln!("fluorita: could not describe the film: {error}"),
        }
    }

    /// The film's description landed. Runs on the GUI thread, through the
    /// queue.
    fn described(mut self: std::pin::Pin<&mut Self>, generation: u64, facts: SourceFacts) {
        if generation != self.rust().opening {
            return;
        }
        if let Some(prober) = self.as_mut().rust_mut().prober.take() {
            let _ = prober.join();
        }
        let container_changes = pathkey::decode(&self.key().to_string())
            .map(|source| !is_mp4(&source))
            .unwrap_or(false);
        self.as_mut()
            .set_container_notice(QString::from(&copy::notice(
                container_changes,
                facts.left_out(),
            )));
        if let Some(frame) = facts.frame() {
            self.as_mut().set_minimum_seconds(frame.as_secs_f64());
            let (start, end) = (*self.trim_start(), *self.trim_end());
            self.set_span(start, end);
        }
    }

    pub fn set_length(mut self: std::pin::Pin<&mut Self>, seconds: f64) {
        if *self.saving() || !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        let first = *self.length_seconds() <= 0.0;
        self.as_mut().set_length_seconds(seconds);
        let (start, end) = if first {
            (0.0, seconds)
        } else {
            (*self.trim_start(), *self.trim_end())
        };
        self.set_span(start, end);
    }

    pub fn set_span(mut self: std::pin::Pin<&mut Self>, start: f64, end: f64) {
        if *self.saving() {
            return;
        }
        let length = *self.length_seconds();
        let within = |value: f64| {
            if value.is_finite() {
                value.clamp(0.0, length.max(0.0))
            } else {
                0.0
            }
        };
        self.as_mut().set_trim_start(within(start));
        self.as_mut().set_trim_end(within(end));
        let edited = self
            .span()
            .is_ok_and(|span| !span.is_whole(duration(length)));
        self.as_mut().set_edited(edited);
    }

    pub fn save_trim(mut self: std::pin::Pin<&mut Self>, replace: bool) {
        if *self.saving() {
            return;
        }
        let Ok(source) = pathkey::decode(&self.key().to_string()) else {
            self.as_mut().refuse(&TrimError::NotAVideo);
            return;
        };
        if MediaKind::classify_path(&source) != Some(MediaKind::Video) {
            self.as_mut().refuse(&TrimError::NotAVideo);
            return;
        }
        let span = match self.span() {
            Ok(span) if span.is_whole(duration(*self.length_seconds())) => {
                self.as_mut().refuse(&TrimError::Unchanged);
                return;
            }
            Ok(span) => span,
            Err(error) => {
                self.as_mut().refuse(&error);
                return;
            }
        };
        let choice = if replace {
            SaveChoice::Replace
        } else {
            SaveChoice::Copy
        };
        self.as_mut().start(source, span, choice);
    }

    pub fn cancel(self: std::pin::Pin<&mut Self>) {
        // The worker notices within one poll, kills the child and reports the
        // cancellation through the queue; nothing waits here.
        self.rust().cancellation.cancel();
    }

    pub fn close(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().stop_worker();
        self.as_mut().stop_prober();
        self.as_mut().set_minimum_seconds(MIN_SPAN.as_secs_f64());
        self.as_mut().set_container_notice(QString::default());
        self.as_mut().set_key(QString::default());
        self.as_mut().set_length_seconds(0.0);
        self.as_mut().set_trim_start(0.0);
        self.as_mut().set_trim_end(0.0);
        self.as_mut().set_edited(false);
        self.as_mut().set_saving(false);
        self.as_mut().set_progress(0.0);
        self.as_mut().set_notice(QString::default());
        self.as_mut().set_saved_key(QString::default());
        self.as_mut().set_saved_url(QString::default());
    }

    /// The span the handles hold, as the domain judges it, at the film's
    /// own frame.
    fn span(&self) -> Result<Span, TrimError> {
        let frame = fluorita_core::duration_from_seconds(*self.minimum_seconds())
            .ok()
            .filter(|frame| !frame.is_zero())
            .unwrap_or(MIN_SPAN);
        Span::framed(
            duration(*self.trim_start()),
            duration(*self.trim_end()),
            duration(*self.length_seconds()),
            frame,
        )
    }

    /// Cancels and joins a description in flight; its answer, if queued, is
    /// dropped by the generation.
    fn stop_prober(mut self: std::pin::Pin<&mut Self>) {
        self.rust().probe_cancellation.cancel();
        if let Some(prober) = self.as_mut().rust_mut().prober.take() {
            let _ = prober.join();
        }
        let mut trim = self.as_mut().rust_mut();
        trim.probe_cancellation = CancellationToken::new();
        trim.opening = trim.opening.wrapping_add(1);
    }

    fn refuse(mut self: std::pin::Pin<&mut Self>, error: &TrimError) {
        self.as_mut().set_notice(QString::from(&error.message_es()));
    }

    /// Joins a worker that has finished, or cancels and joins one that has
    /// not: the child is killed within one poll.
    fn stop_worker(mut self: std::pin::Pin<&mut Self>) {
        self.rust().cancellation.cancel();
        if let Some(worker) = self.as_mut().rust_mut().worker.take() {
            let _ = worker.join();
        }
        self.as_mut().rust_mut().cancellation = CancellationToken::new();
    }

    fn start(mut self: std::pin::Pin<&mut Self>, source: PathBuf, span: Span, choice: SaveChoice) {
        self.as_mut().stop_worker();
        self.as_mut().set_saving(true);
        self.as_mut().set_progress(0.0);
        self.as_mut().set_notice(QString::default());

        let cancellation = self.rust().cancellation.clone();
        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-trim".to_owned())
            .spawn(move || {
                let tool = Tool::system();
                let reporter = qt_thread.clone();
                let on_progress = move |share: f64| {
                    let _ = reporter.queue(move |trim| trim.set_progress(share));
                };
                let outcome = choose_encoder(&tool)
                    .map_err(Failure::from)
                    .and_then(|encoder| {
                        let job = TrimJob {
                            source,
                            span,
                            encoder,
                            choice,
                        };
                        let seams = Seams {
                            bin: &DesktopTrash,
                            adopt: &crate::adopt::request,
                        };
                        run(&job, &tool, &seams, &cancellation, &on_progress)
                    });
                let (message, written) = match &outcome {
                    Ok(landed) => (
                        copy::saved(landed),
                        Some((
                            pathkey::encode(&landed.written),
                            celestina_core::file_uri::from_path(&landed.written)
                                .unwrap_or_default(),
                        )),
                    ),
                    Err(failure) => {
                        eprintln!("fluorita: the trim did not land: {}", copy::detail(failure));
                        (copy::failure(failure), None)
                    }
                };
                let _ = qt_thread.queue(move |mut trim| {
                    trim.as_mut().set_saving(false);
                    trim.as_mut().set_notice(QString::from(&message));
                    if let Some((key, url)) = written {
                        trim.as_mut().set_progress(1.0);
                        trim.as_mut().set_saved_key(QString::from(&key));
                        trim.as_mut().set_saved_url(QString::from(&url));
                    } else {
                        trim.as_mut().set_progress(0.0);
                    }
                });
            });
        match worker {
            Ok(handle) => self.as_mut().rust_mut().worker = Some(handle),
            Err(error) => {
                eprintln!("fluorita: could not start the trim: {error}");
                self.as_mut().set_saving(false);
                self.as_mut()
                    .set_notice(QString::from(crate::copy::WORKER_NOT_STARTED));
            }
        }
    }
}
