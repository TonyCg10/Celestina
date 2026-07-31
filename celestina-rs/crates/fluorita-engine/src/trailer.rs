//! Bounded live previews.
//!
//! A trailer is not a thumbnail and must never become one: it lives in
//! Fluorita's own cache under its own extension, never under the freedesktop
//! `large/<key>.png` entry another application scans. `fluorita-core` keeps the
//! two apart by type; this module keeps them apart on disk.
//!
//! Everything about it is bounded on purpose. The budget caps duration, pixels
//! and bytes; the encode runs into a staging file that is only renamed once the
//! result has been **decoded back** and checked against that budget; and a
//! cancelled job leaves nothing behind. A grid of cards must never be able to
//! turn hovering into unbounded work.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fluorita_core::{cache_key, file_uri, TrailerBudget};

use crate::backend::{TrailerJob, TrailerOutcome};
use crate::error::{EngineError, EngineResult};
use crate::instance::{wait_for_load, Instance, LoadOutcome};
use crate::probe;
use crate::source::SourceHandle;

/// Where a trailer starts: the same quarter-in point the poster uses, so the
/// still and the moving preview show the same part of the film.
const TRAILER_POSITION: &str = "25%";

/// The directory inside Fluorita's cache. Never the freedesktop thumbnail root.
const TRAILER_DIRECTORY: &str = "trailers";
const TRAILER_EXTENSION: &str = "trailer";

pub fn produce(job: &TrailerJob) -> EngineResult<TrailerOutcome> {
    if job.cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }

    let destination = destination_for(&job.cache_root, &job.source).ok_or_else(|| {
        EngineError::UnusableSource {
            path: job.source.clone(),
            reason: "the source has no canonical file URI",
        }
    })?;
    let staging =
        destination.with_extension(format!("{TRAILER_EXTENSION}.tmp-{:x}", job.uniquifier));

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| EngineError::Io {
            operation: "create the trailer cache directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let encoded = encode(job, &staging);
    if let Err(error) = encoded {
        discard(&staging);
        return Err(error);
    }

    // Verify before publishing: a truncated or over-budget encode must not
    // become a cache entry that some host will happily play.
    let outcome = match verify(job, &staging) {
        Ok(outcome) => outcome,
        Err(error) => {
            discard(&staging);
            return Err(error);
        }
    };

    fs::rename(&staging, &destination).map_err(|source| {
        discard(&staging);
        EngineError::Io {
            operation: "publish the trailer",
            path: destination.clone(),
            source,
        }
    })?;

    Ok(TrailerOutcome {
        path: destination,
        ..outcome
    })
}

/// `<cache root>/trailers/<key>.trailer` — the same key the core computes, so
/// the two agree without either re-deriving the other's rule.
#[must_use]
pub fn destination_for(cache_root: &Path, source: &Path) -> Option<PathBuf> {
    let uri = file_uri(source)?;
    Some(
        cache_root
            .join(TRAILER_DIRECTORY)
            .join(format!("{}.{TRAILER_EXTENSION}", cache_key(&uri))),
    )
}

/// The largest 16:9 box whose area fits the budget's pixel cap.
///
/// Combined with `force_original_aspect_ratio=decrease`, an aspect-preserved
/// fit inside this box can never exceed the cap, whatever shape the source is.
#[must_use]
pub fn bounding_box(budget: TrailerBudget) -> (u32, u32) {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let width = ((budget.max_pixels as f64) * 16.0 / 9.0).sqrt() as u32;
    // Even dimensions: every common video encoder requires them.
    let width = (width / 2) * 2;
    let height = ((width * 9 / 16) / 2) * 2;
    (width.max(2), height.max(2))
}

fn encode(job: &TrailerJob, staging: &Path) -> EngineResult<()> {
    let handle = SourceHandle::open(&job.source)?;
    let output = staging
        .to_str()
        .ok_or_else(|| EngineError::UnusableSource {
            path: staging.to_path_buf(),
            reason: "the trailer cache path is not valid UTF-8",
        })?;
    let (width, height) = bounding_box(job.budget);

    // The instance is scoped: dropping it flushes and closes the muxer, so the
    // file is only inspected once the backend can no longer be writing to it.
    let instance = Instance::new(&[
        ("start", TRAILER_POSITION),
        (
            "length",
            &format!("{}", job.budget.max_duration.as_secs_f64()),
        ),
        // A preview is silent: it may start while something else is playing.
        ("audio", "no"),
        ("ao", "null"),
        (
            "vf",
            &format!("lavfi=[scale=w={width}:h={height}:force_original_aspect_ratio=decrease]"),
        ),
        ("o", output),
        ("of", "mp4"),
        ("ovc", "libx264"),
        ("ovcopts", "preset=veryfast,crf=28"),
        ("hwdec", "no"),
    ])?;
    let client = instance.client()?;
    instance.load(&handle)?;

    match wait_for_load(&client, job.deadline, &job.cancellation, &job.source)? {
        LoadOutcome::Loaded => {}
        LoadOutcome::Ended(reason) => {
            return Err(EngineError::Undecodable {
                path: job.source.clone(),
                detail: format!("nothing to preview ({reason:?})"),
            })
        }
    }
    wait_for_encode(&client, job)
}

fn wait_for_encode(client: &libmpv2::Mpv, job: &TrailerJob) -> EngineResult<()> {
    let started = std::time::Instant::now();
    while started.elapsed() < job.deadline {
        if job.cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        match client.wait_event(0.1) {
            Some(Ok(libmpv2::events::Event::EndFile(_) | libmpv2::events::Event::Shutdown)) => {
                return Ok(())
            }
            Some(Err(source)) => {
                return Err(EngineError::Backend {
                    operation: "wait for the trailer encode",
                    source,
                })
            }
            _ => {}
        }
    }
    Err(EngineError::TimedOut {
        operation: "encode trailer",
        after: job.deadline,
    })
}

/// Decodes the encode back and checks it against the budget it was made for.
fn verify(job: &TrailerJob, staging: &Path) -> EngineResult<TrailerOutcome> {
    let bytes = fs::metadata(staging)
        .map_err(|source| EngineError::Io {
            operation: "measure the trailer",
            path: staging.to_path_buf(),
            source,
        })?
        .len();
    if bytes > job.budget.max_bytes {
        return Err(EngineError::OverBudget {
            what: "trailer bytes",
            limit: job.budget.max_bytes,
            actual: bytes,
        });
    }

    let report = probe::probe(
        staging,
        crate::backend::ProbeBudget {
            deadline: job.deadline,
        },
        &job.cancellation,
    )?;
    if !report.has_moving_video() {
        return Err(EngineError::Undecodable {
            path: staging.to_path_buf(),
            detail: "the encoded trailer has no video".to_owned(),
        });
    }

    let duration = report.metadata.duration.unwrap_or_default();
    // A little slack: the encoder lands on a frame boundary, not on the exact
    // budget instant.
    if duration > job.budget.max_duration + Duration::from_millis(500) {
        return Err(EngineError::OverBudget {
            what: "trailer duration in milliseconds",
            limit: u64::try_from(job.budget.max_duration.as_millis()).unwrap_or(u64::MAX),
            actual: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        });
    }

    let pixels = u64::from(report.width.unwrap_or(0)) * u64::from(report.height.unwrap_or(0));
    if pixels > job.budget.max_pixels {
        return Err(EngineError::OverBudget {
            what: "trailer pixels",
            limit: job.budget.max_pixels,
            actual: pixels,
        });
    }

    Ok(TrailerOutcome {
        path: staging.to_path_buf(),
        bytes,
        duration,
        width: report.width.unwrap_or(0),
        height: report.height.unwrap_or(0),
    })
}

fn discard(staging: &Path) {
    // A cancelled or failed encode must not leave a partial file behind; that
    // it may already be gone is not an error.
    let _ = fs::remove_file(staging);
}

/// What one pruning pass removed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PruneSummary {
    pub removed: usize,
    pub freed_bytes: u64,
    pub remaining_bytes: u64,
}

/// Keeps the trailer cache bounded by removing the oldest entries first.
///
/// "Bounded" has to be enforced somewhere, or a cache of ephemeral previews
/// grows forever. Only `.trailer` files are ever considered, so nothing else in
/// the directory can be deleted by this.
pub fn prune_cache(cache_root: &Path, max_total_bytes: u64) -> EngineResult<PruneSummary> {
    let directory = cache_root.join(TRAILER_DIRECTORY);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        // No directory means no cache to bound.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PruneSummary::default())
        }
        Err(source) => {
            return Err(EngineError::Io {
                operation: "read the trailer cache",
                path: directory,
                source,
            })
        }
    };

    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == TRAILER_EXTENSION)
        })
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            Some((
                metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
                metadata.len(),
                path,
            ))
        })
        .collect();

    let mut total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    let mut summary = PruneSummary {
        remaining_bytes: total,
        ..PruneSummary::default()
    };
    if total <= max_total_bytes {
        return Ok(summary);
    }

    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in files {
        if total <= max_total_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
            summary.removed += 1;
            summary.freed_bytes += size;
        }
    }
    summary.remaining_bytes = total;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::{bounding_box, destination_for, produce, prune_cache, PruneSummary};
    use crate::backend::TrailerJob;
    use celestina_core::CancellationToken;
    use fluorita_core::TrailerBudget;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("fluorita-engine-trailer/{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch directory");
        directory
    }

    #[test]
    fn the_bounding_box_fits_inside_the_pixel_budget() {
        let (width, height) = bounding_box(TrailerBudget::conservative());

        assert_eq!((width, height), (1280, 720));
        assert!(u64::from(width) * u64::from(height) <= TrailerBudget::conservative().max_pixels);
        // Even dimensions: encoders reject odd ones.
        assert_eq!(width % 2, 0);
        assert_eq!(height % 2, 0);
    }

    #[test]
    fn a_tiny_budget_still_produces_a_usable_box() {
        let (width, height) = bounding_box(TrailerBudget {
            max_pixels: 4,
            ..TrailerBudget::conservative()
        });

        assert!(width >= 2 && height >= 2);
        assert_eq!(width % 2, 0);
    }

    #[test]
    fn a_trailer_never_lands_under_the_freedesktop_thumbnail_path() {
        let destination = destination_for(
            Path::new("/home/toni/.cache/fluorita"),
            Path::new("/home/toni/clip.mp4"),
        )
        .expect("absolute source");

        assert_eq!(
            destination,
            PathBuf::from(
                "/home/toni/.cache/fluorita/trailers/053a0fcc87f42f4b9e33ebc076783935.trailer"
            )
        );
        assert_ne!(
            destination.extension().and_then(|e| e.to_str()),
            Some("png")
        );
    }

    #[test]
    fn a_cancelled_trailer_never_starts_the_backend() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = produce(&TrailerJob {
            source: PathBuf::from("/home/toni/clip.mp4"),
            cache_root: std::env::temp_dir(),
            budget: TrailerBudget::conservative(),
            uniquifier: 1,
            deadline: Duration::from_secs(5),
            cancellation,
        })
        .expect_err("cancelled before starting");

        assert!(error.is_retryable());
    }

    #[test]
    fn pruning_removes_the_oldest_entries_until_the_cache_fits() {
        let root = scratch("prune");
        let directory = root.join("trailers");
        std::fs::create_dir_all(&directory).expect("cache directory");

        // Three entries of 1 KiB, written oldest first.
        for (index, name) in ["old", "middle", "new"].iter().enumerate() {
            let path = directory.join(format!("{name}.trailer"));
            std::fs::write(&path, vec![0_u8; 1024]).expect("entry");
            let stamp =
                std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + index as u64 * 10);
            let times = std::fs::FileTimes::new().set_modified(stamp);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|file| file.set_times(times))
                .expect("stamp the entry");
        }
        // Something that is not a trailer must survive pruning untouched.
        let bystander = directory.join("notes.txt");
        std::fs::write(&bystander, b"keep me").expect("bystander");

        let summary = prune_cache(&root, 2048).expect("prune");

        assert_eq!(summary.removed, 1);
        assert_eq!(summary.freed_bytes, 1024);
        assert_eq!(summary.remaining_bytes, 2048);
        assert!(!directory.join("old.trailer").exists());
        assert!(directory.join("middle.trailer").exists());
        assert!(directory.join("new.trailer").exists());
        assert!(bystander.exists(), "pruning must only touch trailers");
    }

    #[test]
    fn a_cache_already_within_its_bound_is_left_alone() {
        let root = scratch("prune-noop");
        std::fs::create_dir_all(root.join("trailers")).expect("cache directory");
        std::fs::write(root.join("trailers/a.trailer"), vec![0_u8; 512]).expect("entry");

        assert_eq!(
            prune_cache(&root, 4096).expect("prune"),
            PruneSummary {
                removed: 0,
                freed_bytes: 0,
                remaining_bytes: 512,
            }
        );
    }

    #[test]
    fn pruning_a_cache_that_does_not_exist_is_not_an_error() {
        let root = scratch("prune-missing");
        assert_eq!(
            prune_cache(&root, 1024).expect("prune"),
            PruneSummary::default()
        );
    }
}
