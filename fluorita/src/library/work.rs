//! What the library does off the GUI thread.
//!
//! The scan, the tag pass, the artwork pass and the watch all live here because
//! they share one shape: they run on an owned thread, they are bounded, and the
//! only thing they hand back is a finished snapshot through the queue. The Qt
//! half in `library.rs` never blocks on any of it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use celestina_core::CancellationToken;
use cxx_qt_lib::QString;
use fluorita_core::{Catalogue, MediaKind, SourceScope, SourceSet, XdgMediaDirs};
use fluorita_engine::backend::ArtworkJob;
use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};
use fluorita_engine::{catalogue_store, source_store, LibraryChange, LibraryWatcher, ScanLimits};

use crate::folders::{self, FolderChoice};

use super::copy;
use super::project::project;
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

/// Asks the desktop for a folder and reports the answer through the queue.
///
/// The whole exchange happens here because it blocks for as long as the person
/// takes to decide. Only the answer crosses back, as two strings: the chosen
/// path, empty when nothing was chosen, and a notice, empty when there is
/// nothing to say. A dismissed dialog is therefore silent, and a desktop that
/// could not be asked says why.
pub(super) fn run_folder_choice(qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>) {
    let (path, notice) = match folders::choose(copy::CHOOSE_FOLDER) {
        FolderChoice::Chosen(path) => (path.to_string_lossy().into_owned(), String::new()),
        FolderChoice::Cancelled => (String::new(), String::new()),
        FolderChoice::Unavailable(reason) => (
            String::new(),
            format!("{}: {reason}", copy::CHOOSER_UNAVAILABLE),
        ),
    };
    let _ = qt_thread.queue(move |library| {
        library.folder_chosen(QString::from(&path), QString::from(&notice));
    });
}

/// Moves one item to the desktop Trash and reports the outcome.
///
/// On the same filesystem this is an atomic rename; from another mount it is a
/// real copy-verify-remove, which is why it never runs on the GUI thread. The
/// operation owns the freedesktop rules — reserving the info file, rolling back
/// a failure — and this function only carries the answer back.
pub(super) fn run_trash(path: &Path, qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>) {
    let cancellation = CancellationToken::new();
    let mut progress = |_progress| {};
    let notice = match siderita_ops::trash(path, &cancellation, &mut progress) {
        Ok(_) => String::new(),
        // The reason matters: "permission denied" and "it is already gone" call
        // for different things from the person reading it.
        Err(error) => format!("{}: {error}", copy::TRASH_FAILED),
    };
    let moved = path.to_string_lossy().into_owned();
    let _ = qt_thread.queue(move |library| {
        library.item_trashed(QString::from(&moved), QString::from(&notice));
    });
}

/// Reads the catalogue and the configuration, walks the roots, then watches.
///
/// `configured` is `Some` when the user just changed the configuration; that
/// set is stored before anything is walked, so the choice survives even a scan
/// that fails. It is `None` on launch, when the stored configuration — or the
/// first-run seed — is what to use.
pub(super) fn run_scan(
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    configured: Option<SourceSet>,
    scope: SourceScope,
    cancellation: &CancellationToken,
) {
    let store = catalogue_store::default_path();
    let source_store_path = source_store::default_path();

    // `store_now` is the whole point of persisting: a set that only lives in
    // this process would hand out fresh handles next launch, and the catalogue
    // on disk keys every record by one. A seed is therefore written down too —
    // it is the configuration until the user changes it, and one media
    // directory appearing or disappearing would otherwise shift every handle
    // under the stored records.
    let (sources, store_now) = match configured {
        Some(sources) => (sources, true),
        None => match source_store_path.as_deref() {
            Some(path) => {
                let loaded = source_store::load(path, &media_directories());
                if loaded.skipped > 0 {
                    eprintln!(
                        "fluorita: {} stored folder entries could not be read",
                        loaded.skipped
                    );
                }
                (loaded.sources, loaded.seeded)
            }
            None => (SourceSet::seeded_from(&media_directories()), false),
        },
    };
    // Stored before anything is walked. A scan that fails afterwards costs a
    // scan; a choice lost because the scan failed would look like the button
    // did nothing.
    if store_now {
        if let Some(path) = source_store_path.as_deref() {
            if let Err(error) = source_store::save(path, &sources) {
                eprintln!("fluorita: could not store the configured folders: {error}");
            }
        }
    }

    // What was known last time, on screen before anything is walked — minus
    // anything belonging to a root that is no longer configured.
    let mut catalogue = match store.as_deref().map(catalogue_store::load) {
        Some(Ok(outcome)) => outcome.catalogue,
        _ => Catalogue::new(),
    };
    catalogue.retain_configured(&sources);
    if !catalogue.is_empty() {
        let stored = project(&catalogue, &sources, scope, false, "stored");
        let _ = qt_thread.queue(move |library| library.apply(stored));
    }

    if sources.is_empty() {
        return publish_failure(qt_thread, &sources, scope, copy::NO_SOURCES);
    }

    let Ok(worker) = EngineWorker::start() else {
        return publish_failure(qt_thread, &sources, scope, copy::SCANNER_UNAVAILABLE);
    };
    if worker
        .submit(Job::Scan {
            generation: celestina_core::Generation::INITIAL,
            sources: Box::new(sources.clone()),
            limits: ScanLimits::conservative(),
        })
        .is_err()
    {
        return publish_failure(qt_thread, &sources, scope, copy::SCANNER_UNAVAILABLE);
    }

    let Some(JobOutcome::Scanned { result, .. }) = worker.poll(SCAN_TIMEOUT) else {
        return publish_failure(qt_thread, &sources, scope, copy::SCAN_TIMED_OUT);
    };
    let Ok(outcome) = result else {
        return publish_failure(qt_thread, &sources, scope, copy::SCAN_FAILED);
    };

    let truncated = outcome.truncated;
    let complete = outcome.is_complete();
    let reached = outcome.reached.clone();
    catalogue.absorb(outcome.records, complete);
    // A file the walk did not find under a root that answered is deleted, not
    // merely absent, and the library stops showing it. Only a complete pass may
    // conclude this, and only for the roots that actually answered — a drive
    // that is not plugged in keeps everything it holds.
    if complete {
        catalogue.forget_vanished(&reached);
    }
    // A root that is no longer configured keeps no records: the scan cannot
    // refresh what it does not walk, and a stale entry would read as a file
    // that went missing.
    catalogue.retain_configured(&sources);

    // Tags are the expensive part, and the only reason this catalogue is worth
    // storing: what is read here is not read again unless the file changes.
    let learned = learn_tags(&worker, &mut catalogue);

    // Best effort: a catalogue that could not be written is a slower next
    // launch, not a broken library, so it must not fail the scan on screen.
    if let Some(path) = store.as_deref() {
        let _ = catalogue_store::save(path, &catalogue);
    }

    let refreshed = project(&catalogue, &sources, scope, truncated, "ready");
    let _ = qt_thread.queue(move |library| library.apply(refreshed));
    let _ = learned;

    // From here the library keeps itself up to date without walking again.
    watch_library(
        &sources,
        scope,
        catalogue,
        store.as_deref(),
        qt_thread,
        cancellation,
    );
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
    scope: SourceScope,
    mut catalogue: Catalogue,
    store: Option<&Path>,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    cancellation: &CancellationToken,
) {
    let Ok(watcher) = LibraryWatcher::start(sources) else {
        // No watcher is a library that is merely not live; the scan it already
        // published stands.
        return;
    };
    for root in watcher.unwatched() {
        // Counted in the log rather than hidden: a root nobody watches looks
        // like a scan that forgot it.
        eprintln!("fluorita: could not watch {}", root.display());
    }

    // The host stops this loop by cancelling, which is what lets a second scan
    // — after the user maps or unmaps a folder — join this thread instead of
    // waiting on a watch that would otherwise run until the window closes.
    while !cancellation.is_cancelled() {
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
                    // The watcher saw this exact file go, in a root it is
                    // watching right now, so the root plainly answers. That is
                    // the same evidence a completed scan gives, so the record
                    // goes rather than lingering as a permanently missing row.
                    let id = catalogue
                        .find_by_path(&path)
                        .map(|record| record.id().clone());
                    if let Some(id) = id {
                        changed |= catalogue.forget(&id).is_some();
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
                    let reached = outcome.reached.clone();
                    catalogue.absorb(outcome.records, complete);
                    if complete {
                        catalogue.forget_vanished(&reached);
                    }
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
        let refreshed = project(&catalogue, sources, scope, false, "ready");
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

/// Reports a failure without losing the sidebar: the roots are still
/// configured, and a window that dropped them would leave no way to add or
/// remove one after a scan went wrong.
pub(super) fn publish_failure(
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    sources: &SourceSet,
    scope: SourceScope,
    message: &str,
) {
    let mut snapshot = project(&Catalogue::new(), sources, scope, false, "error");
    snapshot.summary = message.to_owned();
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
