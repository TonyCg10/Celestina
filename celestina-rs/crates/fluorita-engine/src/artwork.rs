//! Producing the static PNG the rest of the desktop already reads.
//!
//! Siderita shows video and audio thumbnails it cannot generate; this is the
//! producer that fills them in. The key, the size and the validity rule are
//! `fluorita-core`'s frozen contract — nothing here re-derives them.
//!
//! Two rules make the write safe: the backend renders into a private directory
//! **inside the cache root** so the final step is a rename on the same
//! filesystem, and the entry is only renamed into place once the complete PNG
//! exists. A reader therefore sees either the previous entry or the new one,
//! never half of either.
//!
//! The backend's PNG carries no text, so before it is published it is given
//! the two keys the freedesktop spec requires — `Thumb::URI` and
//! `Thumb::MTime` — and an entry is judged by the spec's rule, the recorded
//! mtime against the source's, rather than by the cache file's own date.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use fluorita_core::{ArtworkOrigin, ArtworkPublication};

use crate::backend::ArtworkJob;
use crate::error::{EngineError, EngineResult};
use crate::instance::{wait_for_load, Instance, LoadOutcome};
use crate::source::SourceHandle;

/// The freedesktop "large" box: 256 px on the longest side, aspect preserved.
const SCALE_FILTER: &str =
    "lavfi=[scale=w=256:h=256:force_original_aspect_ratio=decrease:flags=bicubic]";

/// A quarter in is far enough past titles and fades to be representative, and
/// cheap because the backend seeks there instead of decoding up to it.
const POSTER_POSITION: &str = "25%";

/// The most of a rendered frame that is read back to be keyed. A 256-pixel
/// PNG is a few hundred kilobytes at worst; anything past this is not the
/// frame this module asked for.
const MAX_RENDERED_BYTES: u64 = 8 * 1024 * 1024;

/// How much of a cache entry is read to find its keys. They sit ahead of the
/// image data, a few hundred bytes in for every writer that follows the spec.
const KEYS_PREFIX_BYTES: u64 = 4 * 1024;

pub fn publish(job: &ArtworkJob) -> EngineResult<PathBuf> {
    if job.cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    if matches!(job.origin, ArtworkOrigin::ImageDownscale) {
        return Err(EngineError::UnusableSource {
            path: job.source.clone(),
            reason: "an image thumbnail must not start the media backend",
        });
    }

    let plan = ArtworkPublication::prepare(
        &job.cache_root,
        &job.source,
        job.source_mtime,
        job.uniquifier,
    )
    .ok_or_else(|| EngineError::UnusableSource {
        path: job.source.clone(),
        reason: "the source has no canonical file URI",
    })?;

    let staging = staging_directory(&plan, job.uniquifier)?;
    let outcome = render(job, &staging);
    let rendered = match outcome {
        Ok(path) => path,
        Err(error) => {
            remove_directory(&staging);
            return Err(error);
        }
    };

    let published = install(&plan, &rendered);
    remove_directory(&staging);
    published.map(|()| plan.final_path)
}

/// Renders exactly one frame into `staging` and returns the file it wrote.
fn render(job: &ArtworkJob, staging: &Path) -> EngineResult<PathBuf> {
    let handle = SourceHandle::open(&job.source)?;
    let start = match job.origin {
        // An embedded cover is a single attached picture: seeking into it would
        // land past the only frame there is.
        ArtworkOrigin::EmbeddedCover => "0",
        _ => POSTER_POSITION,
    };
    let outdir = staging
        .to_str()
        .ok_or_else(|| EngineError::UnusableSource {
            path: staging.to_path_buf(),
            reason: "the cache path is not valid UTF-8",
        })?;

    let instance = Instance::new(&[
        ("vo", "image"),
        ("vo-image-format", "png"),
        ("vo-image-outdir", outdir),
        ("ao", "null"),
        ("audio", "no"),
        ("frames", "1"),
        ("start", start),
        ("vf", SCALE_FILTER),
        // One frame needs no hardware context, and asking for one per
        // catalogued file would cost more than it saves.
        ("hwdec", "no"),
    ])?;
    let client = instance.client()?;
    instance.load(&handle)?;

    match wait_for_load(&client, job.deadline, &job.cancellation, &job.source)? {
        LoadOutcome::Loaded => {}
        LoadOutcome::Ended(reason) => {
            return Err(EngineError::Undecodable {
                path: job.source.clone(),
                detail: format!("nothing to extract ({reason:?})"),
            })
        }
    }
    wait_for_frame(&instance, &client, job)?;

    first_png(staging).ok_or_else(|| EngineError::Undecodable {
        path: job.source.clone(),
        detail: "the backend produced no frame".to_owned(),
    })
}

/// Waits for the single frame to be written: the backend reports end-of-file
/// once it has rendered it.
fn wait_for_frame(
    instance: &Instance,
    client: &libmpv2::Mpv,
    job: &ArtworkJob,
) -> EngineResult<()> {
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
                    operation: "wait for the extracted frame",
                    source,
                })
            }
            _ => {}
        }
        // `idle-active` means the backend finished the file and has nothing
        // queued: with `frames=1` that is the frame being on disk.
        if instance.optional_bool("idle-active").unwrap_or(false) {
            return Ok(());
        }
    }
    Err(EngineError::TimedOut {
        operation: "extract artwork",
        after: job.deadline,
    })
}

/// Keys the rendered frame and moves it onto the cache entry: write the keyed
/// PNG to the entry's unique sibling, owner-only from its first byte, then
/// rename.
fn install(plan: &ArtworkPublication, rendered: &Path) -> EngineResult<()> {
    fs::create_dir_all(plan.parent_directory()).map_err(|source| EngineError::Io {
        operation: "create the cache directory",
        path: plan.parent_directory().to_path_buf(),
        source,
    })?;
    let keyed = keyed_frame(plan, rendered)?;
    // The sibling's name is this producer's own; one left by a pass that died
    // half-way is debris, not someone else's file.
    let _ = fs::remove_file(&plan.temporary_path);

    // A thumbnail can disclose the content of a private file, so it is
    // owner-only before it holds anything, let alone before it becomes
    // visible under its final name.
    let written = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(plan.mode);
        }
        let mut file = options.open(&plan.temporary_path)?;
        file.write_all(&keyed)
    })();
    if let Err(source) = written {
        let _ = fs::remove_file(&plan.temporary_path);
        return Err(EngineError::Io {
            operation: "stage the thumbnail",
            path: plan.temporary_path.clone(),
            source,
        });
    }

    fs::rename(&plan.temporary_path, &plan.final_path).map_err(|source| {
        let _ = fs::remove_file(&plan.temporary_path);
        EngineError::Io {
            operation: "publish the thumbnail",
            path: plan.final_path.clone(),
            source,
        }
    })
}

/// The rendered frame with the spec keys in it, read within a bound.
fn keyed_frame(plan: &ArtworkPublication, rendered: &Path) -> EngineResult<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(rendered)
        .and_then(|file| file.take(MAX_RENDERED_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|source| EngineError::Io {
            operation: "read the rendered thumbnail",
            path: rendered.to_path_buf(),
            source,
        })?;
    if bytes.len() as u64 > MAX_RENDERED_BYTES {
        return Err(EngineError::OverBudget {
            what: "the rendered thumbnail",
            limit: MAX_RENDERED_BYTES,
            actual: bytes.len() as u64,
        });
    }
    fluorita_core::with_thumbnail_keys(&bytes, &plan.thumb_uri, plan.thumb_mtime_seconds)
        .ok_or_else(|| EngineError::Undecodable {
            path: rendered.to_path_buf(),
            detail: "the backend's frame is not a PNG".to_owned(),
        })
}

/// The keys a cache entry records, `None` when there is no entry. An entry
/// that cannot be read, or is not a PNG, records nothing and is produced
/// again.
fn recorded_keys(entry: &Path) -> Option<fluorita_core::ThumbnailKeys> {
    let file = fs::File::open(entry).ok()?;
    let mut prefix = Vec::new();
    if file
        .take(KEYS_PREFIX_BYTES)
        .read_to_end(&mut prefix)
        .is_err()
    {
        return Some(fluorita_core::ThumbnailKeys::default());
    }
    Some(fluorita_core::thumbnail_keys(&prefix).unwrap_or_default())
}

/// A private directory beside the entry, so the final step is a same-filesystem
/// rename rather than a copy that could be read half-written.
fn staging_directory(plan: &ArtworkPublication, uniquifier: u64) -> EngineResult<PathBuf> {
    let directory = plan
        .parent_directory()
        .join(format!(".fluorita-staging-{uniquifier:x}"));
    if directory.exists() {
        remove_directory(&directory);
    }
    fs::create_dir_all(&directory).map_err(|source| EngineError::Io {
        operation: "create the staging directory",
        path: directory.clone(),
        source,
    })?;
    Ok(directory)
}

fn remove_directory(path: &Path) {
    // Cleaning up is best effort by design: failing to remove a temporary
    // directory must not fail a thumbnail that was published correctly.
    let _ = fs::remove_dir_all(path);
}

fn first_png(directory: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
        .collect();
    entries.sort();
    entries.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::{first_png, publish, SCALE_FILTER};
    use crate::backend::ArtworkJob;
    use celestina_core::CancellationToken;
    use fluorita_core::ArtworkOrigin;
    use std::time::{Duration, SystemTime};

    #[test]
    fn the_scale_filter_keeps_the_aspect_inside_the_large_box() {
        assert!(SCALE_FILTER.contains("force_original_aspect_ratio=decrease"));
        assert!(SCALE_FILTER.contains("w=256:h=256"));
    }

    #[test]
    fn an_image_is_refused_so_the_media_backend_never_starts_for_one() {
        let job = ArtworkJob {
            source: std::path::PathBuf::from("/home/toni/Imágenes/foto.png"),
            cache_root: std::env::temp_dir(),
            origin: ArtworkOrigin::ImageDownscale,
            source_mtime: SystemTime::UNIX_EPOCH,
            uniquifier: 1,
            deadline: Duration::from_secs(5),
            cancellation: CancellationToken::new(),
        };

        let error = publish(&job).expect_err("images belong to the toolkit");
        assert!(error.to_string().contains("image thumbnail"));
    }

    #[test]
    fn a_cancelled_job_never_touches_the_cache() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let job = ArtworkJob {
            source: std::path::PathBuf::from("/home/toni/Vídeos/clip.mp4"),
            cache_root: std::env::temp_dir(),
            origin: ArtworkOrigin::VideoPoster,
            source_mtime: SystemTime::UNIX_EPOCH,
            uniquifier: 2,
            deadline: Duration::from_secs(5),
            cancellation,
        };

        assert!(matches!(
            publish(&job),
            Err(crate::error::EngineError::Cancelled)
        ));
    }

    #[test]
    fn an_empty_staging_directory_yields_no_frame() {
        let directory = std::env::temp_dir().join("fluorita-engine-empty-staging");
        std::fs::create_dir_all(&directory).expect("temporary directory");

        assert_eq!(first_png(&directory), None);

        std::fs::remove_dir_all(&directory).ok();
    }
}

/// One item that has no usable thumbnail yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingArtwork {
    pub media: fluorita_core::MediaId,
    pub source: PathBuf,
    pub origin: ArtworkOrigin,
    pub source_mtime: std::time::SystemTime,
}

/// Which catalogued items would need a thumbnail produced.
///
/// This never decodes: it reads the first few kilobytes of an entry, where the
/// spec's keys are, and judges them with the core's validity rule. Deciding
/// *what* to generate must never cost a decode, or the decision would be as
/// expensive as the work — and it runs on a worker, never on the GUI thread.
///
/// Images are excluded on purpose. Qt's image reader produces those, through
/// the shared thumbnail provider in `fluorita-qt`; routing them through the
/// media backend is the cost the suite's contract keeps out.
#[must_use]
pub fn pending(
    catalogue: &fluorita_core::Catalogue,
    cache_root: &Path,
    limit: usize,
) -> Vec<PendingArtwork> {
    pending_where(catalogue, cache_root, limit, |_| true)
}

/// [`pending`], among the records `wanted` accepts.
///
/// The filter runs before the limit, so a background pass that remembers what
/// it already tried — a broken clip, a track with no cover — reaches the next
/// items instead of being handed the same failures every time.
#[must_use]
pub fn pending_where(
    catalogue: &fluorita_core::Catalogue,
    cache_root: &Path,
    limit: usize,
    wanted: impl Fn(&fluorita_core::MediaRecord) -> bool,
) -> Vec<PendingArtwork> {
    catalogue
        .records()
        .filter(|record| record.is_available())
        .filter(|record| wanted(record))
        .filter(|record| {
            matches!(
                record.kind(),
                fluorita_core::MediaKind::Video | fluorita_core::MediaKind::Audio
            )
        })
        .filter(|record| {
            let Some(entry) = fluorita_core::large_thumbnail_path(cache_root, record.path()) else {
                return false;
            };
            let cached = recorded_keys(&entry);
            fluorita_core::ArtworkValidity::evaluate(record.identity().modified, cached.as_ref())
                .needs_generation()
        })
        .take(limit)
        .map(|record| PendingArtwork {
            media: record.id().clone(),
            source: record.path().to_path_buf(),
            origin: record.kind().artwork_origin(),
            source_mtime: record.identity().modified,
        })
        .collect()
}

#[cfg(test)]
mod pending_tests {
    use super::pending;
    use fluorita_core::{
        ArtworkOrigin, Catalogue, MediaId, MediaKind, MediaRecord, SourceId, SourceIdentity,
    };
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("fluorita-pending-tests/{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch");
        directory
    }

    fn record(inode: u64, path: &str, kind: MediaKind, seconds: u64) -> MediaRecord {
        MediaRecord::new(
            MediaId::filesystem(66, inode),
            SourceId::from_value(0),
            PathBuf::from(path),
            kind,
            SourceIdentity::new(1_024, SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
        )
    }

    /// The smallest PNG a decoder accepts.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd,
        0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x9f, 0x01, 0x8c, 0xff, 0x33, 0x30, 0x00, 0x00, 0x1f, 0xee, 0x03, 0xfd,
        0x35, 0x1b, 0x00, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60,
        0x82,
    ];

    /// Writes a cache entry for `source` that records `recorded` as the
    /// source's mtime, with the cache file itself stamped `stamped`.
    fn cache_entry(cache_root: &Path, source: &str, recorded: i64, stamped: u64) {
        let entry =
            fluorita_core::large_thumbnail_path(cache_root, Path::new(source)).expect("cache path");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("cache dir");
        let uri = fluorita_core::file_uri(Path::new(source)).expect("a uri");
        let keyed =
            fluorita_core::with_thumbnail_keys(TINY_PNG, &uri, Some(recorded)).expect("a PNG");
        std::fs::write(&entry, keyed).expect("entry");
        let stamp = std::fs::FileTimes::new()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(stamped));
        let file = std::fs::File::options()
            .write(true)
            .open(&entry)
            .expect("open entry");
        file.set_times(stamp).expect("stamp");
    }

    #[test]
    fn only_video_and_audio_are_ever_pending() {
        let cache_root = scratch("kinds");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/clip.mkv", MediaKind::Video, 100));
        catalogue.upsert(record(2, "/m/song.flac", MediaKind::Audio, 100));
        catalogue.upsert(record(3, "/m/foto.png", MediaKind::Image, 100));

        let pending = pending(&catalogue, &cache_root, 100);

        assert_eq!(pending.len(), 2, "una imagen no pasa por el motor");
        assert!(pending
            .iter()
            .any(|item| item.origin == ArtworkOrigin::VideoPoster));
        assert!(pending
            .iter()
            .any(|item| item.origin == ArtworkOrigin::EmbeddedCover));
    }

    #[test]
    fn an_entry_that_already_exists_and_is_current_is_not_pending() {
        let cache_root = scratch("fresh");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/clip.mkv", MediaKind::Video, 100));
        cache_entry(&cache_root, "/m/clip.mkv", 100, 200);

        assert!(pending(&catalogue, &cache_root, 100).is_empty());
    }

    #[test]
    fn an_entry_recorded_for_another_mtime_is_pending_even_when_newer() {
        // The source was restored from a backup with an older mtime: the
        // cache file is newer than it and still shows the other bytes.
        let cache_root = scratch("restored");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/clip.mkv", MediaKind::Video, 100));
        cache_entry(&cache_root, "/m/clip.mkv", 150, 200);

        assert_eq!(pending(&catalogue, &cache_root, 100).len(), 1);
    }

    #[test]
    fn an_entry_with_no_spec_keys_is_produced_again() {
        let cache_root = scratch("unkeyed");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/clip.mkv", MediaKind::Video, 100));
        let entry = fluorita_core::large_thumbnail_path(&cache_root, Path::new("/m/clip.mkv"))
            .expect("cache path");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("cache dir");
        std::fs::write(&entry, TINY_PNG).expect("entry");

        assert_eq!(pending(&catalogue, &cache_root, 100).len(), 1);
    }

    #[test]
    fn an_entry_older_than_its_source_is_pending_again() {
        let cache_root = scratch("stale");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(record(1, "/m/clip.mkv", MediaKind::Video, 300));
        cache_entry(&cache_root, "/m/clip.mkv", 100, 100);

        assert_eq!(pending(&catalogue, &cache_root, 100).len(), 1);
    }

    #[test]
    fn a_missing_file_is_not_worth_generating_for() {
        let cache_root = scratch("missing");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(
            record(1, "/mnt/externo/clip.mkv", MediaKind::Video, 100)
                .with_availability(fluorita_core::Availability::Missing),
        );

        assert!(pending(&catalogue, &cache_root, 100).is_empty());
    }

    #[test]
    fn the_limit_is_respected() {
        let cache_root = scratch("limit");
        let mut catalogue = Catalogue::new();
        for inode in 1..=10 {
            catalogue.upsert(record(
                inode,
                &format!("/m/clip{inode}.mkv"),
                MediaKind::Video,
                100,
            ));
        }

        assert_eq!(pending(&catalogue, &cache_root, 3).len(), 3);
    }

    #[test]
    fn what_was_already_tried_is_filtered_before_the_limit() {
        // A background pass remembers the items that gave nothing; if the
        // limit were taken first, three broken clips would be the whole of
        // every later pass and the rest would never get a poster.
        let cache_root = scratch("tried");
        let mut catalogue = Catalogue::new();
        for inode in 1..=6 {
            catalogue.upsert(record(
                inode,
                &format!("/m/clip{inode}.mkv"),
                MediaKind::Video,
                100,
            ));
        }
        let tried = |record: &MediaRecord| matches!(record.id().filesystem_parts(), Some((_, inode)) if inode <= 3);

        let next = super::pending_where(&catalogue, &cache_root, 3, |record| !tried(record));

        assert_eq!(next.len(), 3);
        assert!(next
            .iter()
            .all(|item| !matches!(item.media.filesystem_parts(), Some((_, inode)) if inode <= 3)));
    }
}
