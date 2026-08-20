//! The Qt half of applying one small change to a selection.
//!
//! Everything about *what* a batch may be lives in `fluorita-core`'s `batch`,
//! and every file it writes goes through the engine's ordinary single-item
//! paths. This object exists for the two things neither of those can do: it
//! runs the work off the GUI thread, and it turns a tally into the sentence a
//! person reads at the end.
//!
//! The tally is published as it moves rather than at the end. A run over forty
//! photographs that showed nothing until it finished would be indistinguishable
//! from one that hung.

use std::path::PathBuf;
use std::thread::JoinHandle;

use celestina_core::{pathkey, CancellationToken};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use fluorita_core::{BatchOperation, BatchProgress, SaveChoice};
use fluorita_engine::{BatchRequest, DesktopTrash};

use crate::image;
use crate::rasteriser::ToolkitRasteriser;

mod copy;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        /// The batch surface QML binds to.
        #[qobject]
        #[qml_element]
        /// True while a run is under way. Every verb is refused meanwhile, and
        /// the only thing offered is stopping it.
        #[qproperty(bool, running)]
        /// The tally, as it moves.
        #[qproperty(i32, total)]
        #[qproperty(i32, done)]
        #[qproperty(i32, skipped)]
        #[qproperty(i32, failed)]
        /// What the finished run amounted to, or empty. A run that wrote
        /// nothing says so rather than reporting a success.
        #[qproperty(QString, notice)]
        type FluoritaBatch = super::BatchRust;

        /// Applies one operation to every key given.
        ///
        /// `operation` is one of `turn-right`, `turn-left`, `mirror-h`,
        /// `mirror-v` or `forget`. `replace` writes in place and sends each
        /// original to the Trash; otherwise a copy lands beside each one.
        #[qinvokable]
        fn run(
            self: Pin<&mut FluoritaBatch>,
            keys: &QStringList,
            operation: &QString,
            replace: bool,
        );

        /// Asks the run to stop at the next item. What it already wrote stays
        /// written: those files were finished, and undoing them would be a
        /// second batch nobody asked for.
        #[qinvokable]
        fn cancel(self: Pin<&mut FluoritaBatch>);

        /// Whether a selection has anything this operation can act on, so the
        /// action is not offered for a selection it would skip entirely.
        #[qinvokable]
        fn admits(self: &FluoritaBatch, keys: &QStringList, operation: &QString) -> bool;
    }

    impl cxx_qt::Threading for FluoritaBatch {}
}

pub struct BatchRust {
    running: bool,
    total: i32,
    done: i32,
    skipped: i32,
    failed: i32,
    notice: QString,

    worker: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl Default for BatchRust {
    fn default() -> Self {
        Self {
            running: false,
            total: 0,
            done: 0,
            skipped: 0,
            failed: 0,
            notice: QString::default(),
            worker: None,
            cancellation: CancellationToken::new(),
        }
    }
}

impl Drop for BatchRust {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl qobject::FluoritaBatch {
    pub fn run(
        mut self: std::pin::Pin<&mut Self>,
        keys: &QStringList,
        operation: &QString,
        replace: bool,
    ) {
        if *self.running() {
            return;
        }
        let Some(operation) = parse_operation(operation) else {
            return;
        };
        let items = decode(keys);
        if items.is_empty() {
            self.as_mut()
                .set_notice(QString::from(copy::NOTHING_CHOSEN));
            return;
        }

        self.as_mut().set_running(true);
        self.as_mut().set_notice(QString::default());
        self.as_mut()
            .set_total(i32::try_from(items.len()).unwrap_or(i32::MAX));
        self.as_mut().set_done(0);
        self.as_mut().set_skipped(0);
        self.as_mut().set_failed(0);

        let choice = if replace {
            SaveChoice::Replace
        } else {
            SaveChoice::Copy
        };
        // A fresh token per run: the previous one may have been cancelled, and
        // a run that started already cancelled would do nothing and say it was
        // stopped.
        self.as_mut().rust_mut().cancellation = CancellationToken::new();
        let cancellation = self.rust().cancellation.clone();
        let qt_thread = self.qt_thread();
        let reporting = self.qt_thread();

        let worker = std::thread::spawn(move || {
            let request = BatchRequest {
                items: &items,
                operation,
                choice,
                copy_marker: copy::COPY_MARKER,
                max_canvas_pixels: image::MAX_PIXELS,
            };
            let mut report = |progress: BatchProgress| {
                let _ = reporting.queue(move |mut batch| {
                    batch
                        .as_mut()
                        .set_done(i32::try_from(progress.done).unwrap_or(i32::MAX));
                    batch
                        .as_mut()
                        .set_skipped(i32::try_from(progress.skipped).unwrap_or(i32::MAX));
                    batch
                        .as_mut()
                        .set_failed(i32::try_from(progress.failed).unwrap_or(i32::MAX));
                });
            };
            let progress = fluorita_engine::run_batch(
                &request,
                &ToolkitRasteriser,
                &DesktopTrash,
                &measure,
                &mut report,
                &cancellation,
            );
            let message = copy::finished(progress);
            let _ = qt_thread.queue(move |mut batch| {
                batch.as_mut().set_running(false);
                batch.as_mut().set_notice(QString::from(&message));
            });
        });
        self.as_mut().rust_mut().worker = Some(worker);
    }

    pub fn cancel(self: std::pin::Pin<&mut Self>) {
        self.rust().cancellation.cancel();
    }

    #[must_use]
    pub fn admits(&self, keys: &QStringList, operation: &QString) -> bool {
        let Some(operation) = parse_operation(operation) else {
            return false;
        };
        decode(keys).iter().any(|path| {
            fluorita_core::MediaKind::classify_path(path)
                .is_some_and(|kind| operation.admits(kind, path))
        })
    }
}

/// Measures a picture with the same probe the viewer uses, so a batch judges a
/// file exactly as opening it would.
fn measure(path: &std::path::Path) -> Option<(u32, u32)> {
    let key = QString::from(&pathkey::encode(path));
    let measured = crate::player::qobject::probe_image(&key);
    (measured.width() > 0 && measured.height() > 0).then(|| {
        (
            u32::try_from(measured.width()).unwrap_or(u32::MAX),
            u32::try_from(measured.height()).unwrap_or(u32::MAX),
        )
    })
}

fn decode(keys: &QStringList) -> Vec<PathBuf> {
    let mut items = Vec::new();
    for index in 0..keys.len() {
        if let Some(key) = keys.get(index) {
            // A value that is not a path key names no file, so it is dropped
            // rather than resolved to whatever its characters spell.
            if let Ok(path) = pathkey::decode(&key.to_string()) {
                items.push(path);
            }
        }
    }
    items
}

fn parse_operation(value: &QString) -> Option<BatchOperation> {
    match value.to_string().as_str() {
        "turn-right" => Some(BatchOperation::Turn { clockwise: true }),
        "turn-left" => Some(BatchOperation::Turn { clockwise: false }),
        "mirror-h" => Some(BatchOperation::Mirror { horizontal: true }),
        "mirror-v" => Some(BatchOperation::Mirror { horizontal: false }),
        "forget" => Some(BatchOperation::Forget),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_operation;
    use fluorita_core::BatchOperation;

    #[test]
    fn every_operation_the_domain_offers_has_a_name_the_surface_can_send() {
        for operation in BatchOperation::ALL {
            let name = match operation {
                BatchOperation::Turn { clockwise: true } => "turn-right",
                BatchOperation::Turn { clockwise: false } => "turn-left",
                BatchOperation::Mirror { horizontal: true } => "mirror-h",
                BatchOperation::Mirror { horizontal: false } => "mirror-v",
                BatchOperation::Forget => "forget",
            };
            assert_eq!(parse_operation(&name.into()), Some(operation));
        }
    }

    #[test]
    fn a_name_the_domain_does_not_know_is_refused_rather_than_guessed_at() {
        assert_eq!(parse_operation(&"".into()), None);
        assert_eq!(parse_operation(&"turn".into()), None);
        assert_eq!(parse_operation(&"crop".into()), None);
    }
}
