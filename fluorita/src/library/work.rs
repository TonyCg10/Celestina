//! What the library does off the GUI thread.
//!
//! The scan, the tag pass, the artwork pass and the watch all live here because
//! they share one shape: they run on an owned thread, they are bounded, and the
//! only thing they hand back is a finished snapshot through the queue. The Qt
//! half in `library.rs` never blocks on any of it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use celestina_core::CancellationToken;
use fluorita_core::{Catalogue, MediaKind, SourceSet, XdgMediaDirs};
use fluorita_engine::backend::ArtworkJob;
use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};
use fluorita_engine::{catalogue_store, LibraryChange, LibraryWatcher, ScanLimits};

use super::project::{project, LibrarySnapshot};
use super::qobject;

/// How long the worker waits for the scan before checking whether the host is
/// still there. A scan of a large library can legitimately take a while.
pub(super) const SCAN_TIMEOUT: Duration = Duration::from_secs(180);

/// Tag reads per launch. Each costs a backend probe of tens of milliseconds,
/// so a first run over a large music library is bounded and simply finishes
/// the rest next time — which is what the stored catalogue makes possible.
pub(super) const MAX_PROBES_PER_RUN: usize = 500;

/// A probe that takes longer than this is a file that will not answer.
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Posters and covers produced per explicit pass.
pub(super) const MAX_ARTWORK_PER_PASS: usize = 200;

/// Extracting one frame is seconds of work at most.
pub(super) const ARTWORK_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the watch waits before checking that its host is still there.
pub(super) const WATCH_POLL: Duration = Duration::from_millis(500);

pub(super) fn run_artwork(
    catalogue: &Catalogue,
    cancellation: &CancellationToken,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
) {
    let Some(cache_root) = thumbnail_cache_root() else {
        let _ = qt_thread.queue(move |library| library.artwork_finished(0));
        return;
    };
    let pending = fluorita_engine::pending_artwork(catalogue, &cache_root, MAX_ARTWORK_PER_PASS);
    let total = i32::try_from(pending.len()).unwrap_or(i32::MAX);

    let Ok(worker) = fluorita_engine::worker::EngineWorker::start() else {
        let _ = qt_thread.queue(move |library| library.artwork_finished(0));
        return;
    };

    let mut produced = 0;
    for (index, item) in pending.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            break;
        }
        let job = ArtworkJob {
            source: item.source,
            cache_root: cache_root.clone(),
            origin: item.origin,
            source_mtime: item.source_mtime,
            // Two passes must never stage into the same temporary name.
            uniquifier: index as u64 + 1,
            deadline: ARTWORK_TIMEOUT,
            cancellation: cancellation.clone(),
        };
        if worker
            .submit(Job::Artwork {
                generation: celestina_core::Generation::INITIAL,
                job: Box::new(job),
            })
            .is_err()
        {
            break;
        }
        let Some(JobOutcome::Artwork { result, .. }) = worker.poll(ARTWORK_TIMEOUT * 2) else {
            break;
        };
        // A file that will not give up a frame is a normal outcome — a broken
        // clip, an audio file with no cover — and the grid keeps its glyph.
        if result.is_ok() {
            produced += 1;
        }
        let done = i32::try_from(index + 1).unwrap_or(i32::MAX);
        let _ = qt_thread.queue(move |library| library.artwork_progress(done, total));
    }

    let _ = qt_thread.queue(move |library| library.artwork_finished(produced));
}

/// Turns row-major records into the four index-aligned lists QML binds to.
pub(super) fn run_scan(qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>) {
    let store = catalogue_store::default_path();

    // What was known last time, on screen before anything is walked.
    let mut catalogue = match store.as_deref().map(catalogue_store::load) {
        Some(Ok(outcome)) => outcome.catalogue,
        _ => Catalogue::new(),
    };
    if !catalogue.is_empty() {
        let stored = project(&catalogue, false, "guardada");
        let _ = qt_thread.queue(move |library| library.apply(stored));
    }

    let sources = SourceSet::seeded_from(&media_directories());
    let sources_for_watch = SourceSet::seeded_from(&media_directories());
    if sources.is_empty() {
        return publish_failure(qt_thread, "No hay carpetas de medios que explorar");
    }

    let Ok(worker) = EngineWorker::start() else {
        return publish_failure(qt_thread, "No se pudo iniciar el explorador");
    };
    if worker
        .submit(Job::Scan {
            generation: celestina_core::Generation::INITIAL,
            sources: Box::new(sources),
            limits: ScanLimits::conservative(),
        })
        .is_err()
    {
        return publish_failure(qt_thread, "No se pudo iniciar el explorador");
    }

    let Some(JobOutcome::Scanned { result, .. }) = worker.poll(SCAN_TIMEOUT) else {
        return publish_failure(qt_thread, "La exploración no terminó a tiempo");
    };
    let Ok(outcome) = result else {
        return publish_failure(qt_thread, "No se pudo explorar la biblioteca");
    };

    let truncated = outcome.truncated;
    let complete = outcome.is_complete();
    catalogue.absorb(outcome.records, complete);

    // Tags are the expensive part, and the only reason this catalogue is worth
    // storing: what is read here is not read again unless the file changes.
    let learned = learn_tags(&worker, &mut catalogue);

    // Best effort: a catalogue that could not be written is a slower next
    // launch, not a broken library, so it must not fail the scan on screen.
    if let Some(path) = store.as_deref() {
        let _ = catalogue_store::save(path, &catalogue);
    }

    let refreshed = project(&catalogue, truncated, "lista");
    let _ = qt_thread.queue(move |library| library.apply(refreshed));
    let _ = learned;

    // From here the library keeps itself up to date without walking again.
    watch_library(&sources_for_watch, catalogue, store.as_deref(), qt_thread);
}

/// Folds changes in as they happen, until the host goes away.
///
/// This is the whole point of watching: a file dropped into a watched folder
/// appears without anyone asking, and one that goes away says so — without
/// re-walking roots that did not change. A burst, or a watcher that lost
/// events, asks for a full scan instead, because folding in what you did not
/// see is guessing.
pub(super) fn watch_library(
    sources: &SourceSet,
    mut catalogue: Catalogue,
    store: Option<&Path>,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
) {
    let Ok(watcher) = LibraryWatcher::start(sources) else {
        // No watcher is a library that is merely not live; the scan it already
        // published stands.
        return;
    };
    for root in watcher.unwatched() {
        // Counted in the log rather than hidden: a root nobody watches looks
        // like a scan that forgot it.
        eprintln!("fluorita: no se pudo vigilar {}", root.display());
    }

    loop {
        let Some(batch) = watcher.poll(WATCH_POLL) else {
            // The host is gone when the queue stops accepting work; a probe
            // send is how that is noticed without a second channel.
            if qt_thread.queue(|_| {}).is_err() {
                return;
            }
            continue;
        };

        let mut changed = false;
        for change in batch {
            match change {
                LibraryChange::Touched(path) => changed |= absorb_one(&mut catalogue, &path),
                LibraryChange::Removed(path) => {
                    let id = catalogue
                        .find_by_path(&path)
                        .map(|record| record.id().clone());
                    if let Some(id) = id {
                        changed |= catalogue.mark_missing(&id);
                    }
                }
                LibraryChange::Resync(_) => {
                    let Ok(outcome) = fluorita_engine::scan(
                        sources,
                        ScanLimits::conservative(),
                        &celestina_core::CancellationToken::new(),
                    ) else {
                        continue;
                    };
                    let complete = outcome.is_complete();
                    catalogue.absorb(outcome.records, complete);
                    changed = true;
                }
            }
        }

        if !changed {
            continue;
        }
        if let Some(path) = store {
            let _ = catalogue_store::save(path, &catalogue);
        }
        let refreshed = project(&catalogue, false, "lista");
        if qt_thread
            .queue(move |library| library.apply(refreshed))
            .is_err()
        {
            return;
        }
    }
}

/// Stats one changed path and folds it in. Returns whether anything moved.
pub(super) fn absorb_one(catalogue: &mut Catalogue, path: &Path) -> bool {
    let Some(kind) = MediaKind::classify_path(path) else {
        return false;
    };
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    let id = {
        use std::os::unix::fs::MetadataExt;
        fluorita_core::MediaId::filesystem(metadata.dev(), metadata.ino())
    };
    #[cfg(not(unix))]
    let id = fluorita_core::MediaId::from_path(path);

    let record = fluorita_core::MediaRecord::new(
        id,
        // The root that owns it; a file under no configured root is not ours.
        match sources_owner(catalogue, path) {
            Some(source) => source,
            None => return false,
        },
        path.to_path_buf(),
        kind,
        fluorita_core::SourceIdentity::new(
            metadata.len(),
            metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        ),
    );

    // `complete: false` is the whole difference from a scan: only this file is
    // judged, and nothing else may be concluded to have disappeared.
    let summary = catalogue.absorb([record], false);
    summary.added + summary.replaced > 0
}

/// The source a changed file belongs to, borrowed from whatever the catalogue
/// already knows: an incremental update has no `SourceSet` at hand, and a new
/// file lands beside ones that do.
pub(super) fn sources_owner(catalogue: &Catalogue, path: &Path) -> Option<fluorita_core::SourceId> {
    let parent = path.parent()?;
    catalogue
        .records()
        .find(|record| record.path().parent() == Some(parent))
        .map(fluorita_core::MediaRecord::source)
        .or_else(|| {
            catalogue
                .records()
                .next()
                .map(fluorita_core::MediaRecord::source)
        })
}

/// Reads tags for audio the catalogue has never probed.
///
/// Only audio, and only what has no duration yet: a video's tags are not what
/// Gallery shows, and a track that was probed before keeps what it learned
/// because its size and mtime say the bytes are the same.
pub(super) fn learn_tags(worker: &EngineWorker, catalogue: &mut Catalogue) -> usize {
    let pending: Vec<(PathBuf, fluorita_core::MediaId)> = catalogue
        .records()
        .filter(|record| record.kind() == MediaKind::Audio)
        .filter(|record| record.is_available() && record.metadata().duration.is_none())
        .take(MAX_PROBES_PER_RUN)
        .map(|record| (record.path().to_path_buf(), record.id().clone()))
        .collect();

    let mut learned = 0;
    for (path, id) in pending {
        if worker
            .submit(Job::Probe {
                generation: celestina_core::Generation::INITIAL,
                path: path.clone(),
                budget: fluorita_engine::ProbeBudget::conservative(),
            })
            .is_err()
        {
            break;
        }
        let Some(JobOutcome::Probed { result, .. }) = worker.poll(PROBE_TIMEOUT) else {
            break;
        };
        // A file that will not answer is not an error: it keeps the name-based
        // title it already had, and the next launch may try again.
        let Ok(report) = result else { continue };
        let Some(record) = catalogue.get(&id).cloned() else {
            continue;
        };
        catalogue.upsert(record.with_metadata(report.metadata));
        learned += 1;
    }
    learned
}

pub(super) fn publish_failure(
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    message: &str,
) {
    let snapshot = LibrarySnapshot {
        state: "error",
        summary: message.to_owned(),
        ..LibrarySnapshot::default()
    };
    let _ = qt_thread.queue(move |library| library.apply(snapshot));
}

pub(crate) fn thumbnail_cache_root() -> Option<PathBuf> {
    celestina_core::xdg::cache_home().map(|cache| cache.join("thumbnails"))
}

/// The XDG media directories, as they exist on this machine.
///
/// A directory that is not there is simply not configured — seeding must never
/// fail a first run, and a library that invented folders would be worse than an
/// empty one.
pub(super) fn media_directories() -> XdgMediaDirs {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let existing = |names: &[&str]| -> Option<PathBuf> {
        let home = home.as_ref()?;
        names
            .iter()
            .map(|name| home.join(name))
            .find(|candidate| candidate.is_dir())
    };

    XdgMediaDirs {
        pictures: existing(&["Imágenes", "Pictures"]),
        videos: existing(&["Vídeos", "Videos"]),
        music: existing(&["Música", "Music"]),
    }
}
