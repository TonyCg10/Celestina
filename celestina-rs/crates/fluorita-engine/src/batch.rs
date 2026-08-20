//! Running one small change over a selection.
//!
//! This owns no rule about *what* may be applied — `fluorita-core`'s `batch`
//! does — and no way of writing a file of its own: each item goes through the
//! same [`edit::save`](crate::edit::save) or
//! [`metadata::write`](crate::metadata::write) a single item would. A batch
//! that had its own writer would be a second place for the ordering rules to be
//! got wrong, on the path that touches forty files instead of one.
//!
//! Three properties matter more here than anywhere else in the engine, because
//! a mistake is multiplied by the size of the selection:
//!
//! - **It stops when asked.** Cancellation is checked before every item, and
//!   the run reports that it stopped rather than pretending it finished.
//! - **One failure is not the end.** A file that refuses is counted and the run
//!   continues; forty photographs are not abandoned because one is unreadable.
//! - **It never guesses.** An item that does not admit the operation is skipped
//!   by the matrices that own that answer, and counted as skipped rather than
//!   as done.

use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use fluorita_core::{
    BatchOperation, BatchProgress, Canvas, EditCapabilities, EditDocument, EditLimits, ItemOutcome,
    MediaKind, PrivateFact, SaveChoice, TagChange, Transform,
};

use crate::edit::{Bin, Rasteriser, SaveRequest};
use crate::metadata::MetadataRequest;

/// One run over a selection.
pub struct BatchRequest<'a> {
    /// The files, in the order the person selected them.
    pub items: &'a [PathBuf],
    pub operation: BatchOperation,
    pub choice: SaveChoice,
    /// The word a copy's name is marked with — product copy, owned by the host.
    pub copy_marker: &'a str,
    /// The pixel ceiling this host refuses to work above, so a batch is bounded
    /// by exactly the number a single edit is.
    pub max_canvas_pixels: u64,
}

/// Runs `request`, reporting after every item.
///
/// `measure` answers a picture's dimensions — the host owns that seam because
/// measuring is the toolkit's job, and the engine must not grow a second image
/// reader to run a batch. `report` is called with the tally after each item, so
/// a surface can show progress without polling.
///
/// Never fails as a whole: a run that could do nothing still returns its tally,
/// because "what happened to my forty files" is the answer, not an error.
pub fn run(
    request: &BatchRequest<'_>,
    rasteriser: &dyn Rasteriser,
    bin: &dyn Bin,
    measure: &dyn Fn(&Path) -> Option<(u32, u32)>,
    report: &mut dyn FnMut(BatchProgress),
    cancellation: &CancellationToken,
) -> BatchProgress {
    let mut progress = BatchProgress::of(request.items.len());
    report(progress);

    for item in request.items {
        if cancellation.is_cancelled() {
            progress.cancelled = true;
            break;
        }
        let outcome = apply(request, item, rasteriser, bin, measure, cancellation);
        progress.record(outcome);
        report(progress);
    }
    progress
}

fn apply(
    request: &BatchRequest<'_>,
    item: &Path,
    rasteriser: &dyn Rasteriser,
    bin: &dyn Bin,
    measure: &dyn Fn(&Path) -> Option<(u32, u32)>,
    cancellation: &CancellationToken,
) -> ItemOutcome {
    let Some(kind) = MediaKind::classify_path(item) else {
        return ItemOutcome::Skipped;
    };
    if !request.operation.admits(kind, item) {
        return ItemOutcome::Skipped;
    }

    match request.operation {
        BatchOperation::Forget => forget(request, item, bin, cancellation),
        BatchOperation::Turn { .. } | BatchOperation::Mirror { .. } => {
            turn(request, item, kind, rasteriser, bin, measure, cancellation)
        }
    }
}

fn turn(
    request: &BatchRequest<'_>,
    item: &Path,
    kind: MediaKind,
    rasteriser: &dyn Rasteriser,
    bin: &dyn Bin,
    measure: &dyn Fn(&Path) -> Option<(u32, u32)>,
    cancellation: &CancellationToken,
) -> ItemOutcome {
    let Some((width, height)) = measure(item) else {
        return ItemOutcome::Failed;
    };
    let Some(canvas) = Canvas::new(width, height) else {
        return ItemOutcome::Failed;
    };
    if canvas.pixels() > request.max_canvas_pixels {
        return ItemOutcome::Failed;
    }

    let capabilities = EditCapabilities::of(kind, item);
    let Some(format) = capabilities.output_format() else {
        return ItemOutcome::Skipped;
    };
    let mut document = EditDocument::new(canvas, EditLimits::new(request.max_canvas_pixels));
    let transform = match request.operation {
        BatchOperation::Turn { clockwise } => Transform::Rotate(if clockwise {
            fluorita_core::Quarter::Clockwise
        } else {
            fluorita_core::Quarter::CounterClockwise
        }),
        BatchOperation::Mirror { horizontal } => Transform::Flip(if horizontal {
            fluorita_core::Axis::Horizontal
        } else {
            fluorita_core::Axis::Vertical
        }),
        BatchOperation::Forget => return ItemOutcome::Skipped,
    };
    if document.transform(transform, &capabilities).is_err() {
        return ItemOutcome::Failed;
    }

    let composition = document.composition();
    let save = SaveRequest {
        source: item,
        composition: &composition,
        orientation: document.orientation_only(),
        format,
        choice: request.choice,
        copy_marker: request.copy_marker,
    };
    match crate::edit::save(&save, rasteriser, bin, cancellation) {
        Ok(_) => ItemOutcome::Done,
        Err(_) => ItemOutcome::Failed,
    }
}

fn forget(
    request: &BatchRequest<'_>,
    item: &Path,
    bin: &dyn Bin,
    cancellation: &CancellationToken,
) -> ItemOutcome {
    // What the file carries decides what is asked for: a picture with no EXIF
    // is skipped rather than rewritten to remove nothing, which would move its
    // modification time and make the catalogue re-probe it for no reason.
    let Ok(bytes) = std::fs::read(item) else {
        return ItemOutcome::Failed;
    };
    let carried = crate::metadata::private_facts(&bytes);
    if carried.is_empty() {
        return ItemOutcome::Skipped;
    }

    let tags = TagChange::new();
    let strip: Vec<PrivateFact> = carried;
    let write = MetadataRequest {
        source: item,
        tags: &tags,
        strip: &strip,
        cover: None,
        choice: request.choice,
        copy_marker: request.copy_marker,
    };
    match crate::metadata::write(&write, bin, cancellation) {
        Ok(_) => ItemOutcome::Done,
        Err(_) => ItemOutcome::Failed,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;
    use fluorita_core::{BatchOperation, BatchProgress, Composition, OutputFormat, SaveChoice};
    use siderita_ops::OpError;

    use super::{run, BatchRequest};
    use crate::edit::{Bin, RasterFailure, Rasteriser};

    struct FakeRasteriser {
        calls: RefCell<usize>,
    }

    impl Rasteriser for FakeRasteriser {
        fn render(
            &self,
            _source: &Path,
            _composition: &Composition,
            _format: OutputFormat,
            _quality: Option<u8>,
        ) -> Result<Vec<u8>, RasterFailure> {
            *self.calls.borrow_mut() += 1;
            Ok(b"rendered".to_vec())
        }
    }

    struct FakeBin;

    impl Bin for FakeBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            std::fs::remove_file(path).map_err(|error| OpError::io(path, &error))?;
            Ok(PathBuf::from("/trash").join(path.file_name().unwrap_or_default()))
        }
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-batch-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("test directory");
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).expect("test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request<'a>(items: &'a [PathBuf], operation: BatchOperation) -> BatchRequest<'a> {
        BatchRequest {
            items,
            operation,
            choice: SaveChoice::Copy,
            copy_marker: "editado",
            max_canvas_pixels: 100_000_000,
        }
    }

    fn measured(_path: &Path) -> Option<(u32, u32)> {
        Some((40, 30))
    }

    #[test]
    fn a_mixed_selection_acts_on_what_admits_the_operation_and_skips_the_rest() {
        let directory = TestDir::new("mixed");
        let items = vec![
            directory.file("uno.png", b"a picture"),
            directory.file("clip.mkv", b"a film"),
            directory.file("dos.png", b"another picture"),
            directory.file("notas.txt", b"not media at all"),
        ];
        let rasteriser = FakeRasteriser {
            calls: RefCell::new(0),
        };
        let mut seen: Vec<BatchProgress> = Vec::new();

        let progress = run(
            &request(&items, BatchOperation::Turn { clockwise: true }),
            &rasteriser,
            &FakeBin,
            &measured,
            &mut |progress| seen.push(progress),
            &CancellationToken::new(),
        );

        assert_eq!(progress.total, 4);
        assert_eq!(progress.done, 2, "both pictures were turned");
        assert_eq!(progress.skipped, 2, "the film and the text were skipped");
        assert_eq!(progress.failed, 0);
        assert!(progress.finished());
        assert!(!progress.cancelled);
        assert_eq!(*rasteriser.calls.borrow(), 2);
        assert_eq!(
            seen.len(),
            5,
            "the tally is reported once before the run and once per item"
        );
        assert!(directory.0.join("uno (editado).png").exists());
    }

    #[test]
    fn a_run_stops_when_it_is_asked_to_and_says_it_stopped() {
        let directory = TestDir::new("cancel");
        let items = vec![
            directory.file("uno.png", b"a picture"),
            directory.file("dos.png", b"another picture"),
        ];
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let progress = run(
            &request(&items, BatchOperation::Turn { clockwise: true }),
            &FakeRasteriser {
                calls: RefCell::new(0),
            },
            &FakeBin,
            &measured,
            &mut |_| {},
            &cancellation,
        );

        assert!(progress.cancelled);
        assert_eq!(progress.settled(), 0);
        assert!(!progress.changed_anything());
        assert!(!directory.0.join("uno (editado).png").exists());
    }

    #[test]
    fn one_unreadable_item_does_not_end_the_run() {
        let directory = TestDir::new("failure");
        let items = vec![
            directory.file("roto.png", b"a picture"),
            directory.file("bueno.png", b"a picture"),
        ];
        let measure = |path: &Path| {
            if path.file_name().is_some_and(|name| name == "roto.png") {
                None
            } else {
                Some((40u32, 30u32))
            }
        };

        let progress = run(
            &request(&items, BatchOperation::Mirror { horizontal: true }),
            &FakeRasteriser {
                calls: RefCell::new(0),
            },
            &FakeBin,
            &measure,
            &mut |_| {},
            &CancellationToken::new(),
        );

        assert_eq!(progress.failed, 1);
        assert_eq!(progress.done, 1);
        assert!(progress.finished());
        assert!(directory.0.join("bueno (editado).png").exists());
    }

    #[test]
    fn forgetting_skips_a_picture_that_carries_nothing() {
        let directory = TestDir::new("forget");
        // A JPEG with no EXIF: it admits the operation by format and has
        // nothing to remove, which is a skip and not a rewrite.
        let items = vec![directory.file("foto.jpg", &[0xFF, 0xD8, 0xFF, 0xDA, 1, 2, 0xFF, 0xD9])];

        let progress = run(
            &request(&items, BatchOperation::Forget),
            &FakeRasteriser {
                calls: RefCell::new(0),
            },
            &FakeBin,
            &measured,
            &mut |_| {},
            &CancellationToken::new(),
        );

        assert_eq!(progress.skipped, 1);
        assert_eq!(progress.done, 0);
        assert!(
            !directory.0.join("foto (editado).jpg").exists(),
            "a rewrite that would remove nothing must not happen"
        );
    }
}
