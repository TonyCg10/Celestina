//! What the library does off the GUI thread.
//!
//! The scan, the tag pass, the poster pass and the watch all live here because
//! they share one shape: they run on an owned thread, they are bounded, and the
//! only thing they hand back is a finished snapshot through the queue. The Qt
//! half in `library.rs` never blocks on any of it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use celestina_core::CancellationToken;
use cxx_qt_lib::QString;
use fluorita_core::{
    Catalogue, MediaId, MediaKind, MediaRecord, MediaSource, SourceScope, SourceSet, XdgMediaDirs,
};
use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};
use fluorita_engine::{
    catalogue_store, source_store, ArtworkAttempt, ArtworkPass, EngineError, LibraryChange,
    LibraryWatcher, ScanLimits,
};

use crate::folders::{self, FolderChoice};
use celestina_core::pathkey;

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

/// Tag reads per watch batch. A retagged track, or an album copied in, is
/// read as it lands, so Music sorts it under what its tags now say; a batch
/// that brings more than this finishes on the next change or the next launch,
/// and the watch goes back to listening within seconds rather than minutes.
pub(super) const MAX_PROBES_PER_BATCH: usize = 32;

/// A probe that takes longer than this is a file that will not answer.
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Posters and covers asked for per background pass. Between two passes the
/// grid is projected again, so a large library fills in by batches instead of
/// all at once at the end.
pub(super) const MAX_ARTWORK_PER_PASS: usize = 200;

/// Extracting one frame is seconds of work at most. The pass waits twice this
/// for an answer before it gives the item up.
pub(super) const ARTWORK_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the watch waits before checking that its host is still there.
pub(super) const WATCH_POLL: Duration = Duration::from_millis(500);

/// The longest this thread waits for the engine without looking at its token.
///
/// The engine worker's own `poll` blocks for the whole budget it is given, so a
/// scan waited 180 s and a tag probe 15 s before cancellation could even be
/// read. The host joins this thread from the GUI, which is why adding or
/// removing a folder mid-scan froze the interface for minutes.
pub(super) const CANCEL_POLL: Duration = Duration::from_millis(100);

/// How a bounded wait for the engine ended.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Waited<T> {
    Finished(T),
    /// The host asked this work to stop. Nothing is published for it: whatever
    /// asked for the cancellation owns what happens next.
    Cancelled,
    TimedOut,
}

/// Waits for one finished job while staying answerable to cancellation.
///
/// Generic over the wait so the rule — check the token, then wait a slice, then
/// check again, never past the budget — can be exercised without an engine.
pub(super) fn await_outcome<T, F>(
    cancellation: &CancellationToken,
    budget: Duration,
    chunk: Duration,
    mut wait: F,
) -> Waited<T>
where
    F: FnMut(Duration) -> Option<T>,
{
    let deadline = std::time::Instant::now() + budget;
    loop {
        if cancellation.is_cancelled() {
            return Waited::Cancelled;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return Waited::TimedOut;
        }
        let slice = chunk.min(deadline - now);
        if let Some(outcome) = wait(slice) {
            return Waited::Finished(outcome);
        }
    }
}

/// The same wait, against the real engine worker, cancelling its current job.
fn await_job(
    worker: &EngineWorker,
    cancellation: &CancellationToken,
    budget: Duration,
) -> Waited<JobOutcome> {
    let waited = await_outcome(cancellation, budget, CANCEL_POLL, |slice| {
        worker.poll(slice)
    });
    if matches!(waited, Waited::Cancelled) {
        // Telling the engine as well: the token this thread watches is not the
        // one the job inside the worker holds.
        worker.cancel_current();
    }
    waited
}

/// The video and audio items the background poster pass has already asked the
/// backend about this session, by path and modification time.
///
/// A clip that gives no frame and a track with no cover stay "pending" by the
/// cache's rule for ever; without this the pass would ask about them again
/// after every scan and every watched change, and — since the per-pass cap is
/// taken after this filter — never reach the items behind them. A file that
/// changes gets a new modification time, and so another try.
pub(super) type Tried = std::sync::Mutex<std::collections::HashSet<(PathBuf, SystemTime)>>;

/// Produces the missing video posters and audio covers, in the background.
///
/// Started by the host after a scan settles and after the watch folds in new
/// items — the author's decision of 2026-10-07, which replaced the explicit
/// "Generar miniaturas" button. Images never come through here: the shared
/// thumbnail provider reads them with Qt's image reader.
///
/// Bounded the way the button's pass was: one backend job at a time, each
/// within [`ARTWORK_TIMEOUT`], at most [`MAX_ARTWORK_PER_PASS`] per pass. The
/// pass repeats until nothing is pending, publishing between two passes so
/// posters appear as they are made. An item that fails or does not answer ends
/// that item, not the batch. Cancellation — a rescan, a closing window — is
/// read between items and in slices while one runs.
///
/// The engine is started only once something is pending, so a library with
/// nothing to produce never starts the backend at all.
pub(super) fn run_posters(
    catalogue: &Catalogue,
    tried: &Tried,
    cancellation: &CancellationToken,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
) {
    run_poster_passes(catalogue, tried, cancellation, qt_thread);
    let _ = qt_thread.queue(|library| library.posters_finished());
}

fn run_poster_passes(
    catalogue: &Catalogue,
    tried: &Tried,
    cancellation: &CancellationToken,
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
) {
    let Some(cache_root) = thumbnail_cache_root() else {
        return;
    };
    let untried = |record: &MediaRecord| {
        !lock_tried(tried).contains(&(record.path().to_path_buf(), record.identity().modified))
    };
    let mut pending = fluorita_engine::pending_artwork_where(
        catalogue,
        &cache_root,
        MAX_ARTWORK_PER_PASS,
        untried,
    );
    if pending.is_empty() || cancellation.is_cancelled() {
        return;
    }
    let Ok(worker) = EngineWorker::start() else {
        return;
    };
    let mut pass = ArtworkPass::new(&worker);
    while !pending.is_empty() {
        let mut stopped = false;
        let produced = pass.run(
            pending,
            &cache_root,
            ARTWORK_TIMEOUT,
            cancellation,
            |item, attempt| match attempt {
                // Produced is no longer pending by the cache's own rule.
                ArtworkAttempt::Produced => {}
                ArtworkAttempt::Failed | ArtworkAttempt::Unanswered => {
                    lock_tried(tried).insert((item.source.clone(), item.source_mtime));
                }
                ArtworkAttempt::Cancelled | ArtworkAttempt::Stopped => stopped = true,
            },
        );
        if produced > 0
            && qt_thread
                .queue(|library| library.posters_produced())
                .is_err()
        {
            return;
        }
        if stopped || cancellation.is_cancelled() {
            return;
        }
        pending = fluorita_engine::pending_artwork_where(
            catalogue,
            &cache_root,
            MAX_ARTWORK_PER_PASS,
            untried,
        );
    }
}

/// Only a set insert or lookup ever runs under this lock, so a poisoned one
/// still holds consistent data and is used as it is.
fn lock_tried(
    tried: &Tried,
) -> std::sync::MutexGuard<'_, std::collections::HashSet<(PathBuf, SystemTime)>> {
    tried
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Asks the desktop for a folder and reports the answer through the queue.
///
/// The whole exchange happens here because it blocks for as long as the person
/// takes to decide. Only the answer crosses back, as two strings: the chosen
/// folder's path key, empty when nothing was chosen, and a notice, empty when
/// there is nothing to say. A dismissed dialog is therefore silent, and a
/// desktop that could not be asked says why.
///
/// The key rather than the path: the portal returns raw bytes and this crosses
/// a `QString`, so a folder whose name is not UTF-8 would otherwise be mapped
/// under its lossy spelling and scanned as a root that does not exist.
///
/// `cancellation` is the host's: a window closing while the dialog is open
/// withdraws the request and returns within one receive slice, so the host
/// can join this thread instead of waiting on the person.
pub(super) fn run_folder_choice(
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    cancellation: &CancellationToken,
) {
    let (key, notice) = match folders::choose(copy::CHOOSE_FOLDER, cancellation) {
        FolderChoice::Chosen(path) => (pathkey::encode(&path), String::new()),
        FolderChoice::Cancelled => (String::new(), String::new()),
        FolderChoice::Unavailable(reason) => (
            String::new(),
            format!("{}: {reason}", copy::CHOOSER_UNAVAILABLE),
        ),
    };
    let _ = qt_thread.queue(move |library| {
        library.folder_chosen(QString::from(&key), QString::from(&notice));
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
    // The recipe that reopens an edited copy dies with the copy; asked for
    // before the move, while the path still names the file it describes.
    let identity = crate::recipes::identity_at(path);
    let notice = match siderita_ops::trash(path, &cancellation, &mut progress) {
        // A move into a Trash on another filesystem is a copy, and a file
        // that changed while it was being copied stays where it was rather
        // than being deleted half-read. The catalogue must not forget a file
        // that is still there, and the person must hear that it is.
        Ok(_) if std::fs::symlink_metadata(path).is_ok() => copy::TRASH_LEFT_BEHIND.to_owned(),
        Ok(_) => {
            if let Some(identity) = identity {
                crate::recipes::forget(&identity);
            }
            String::new()
        }
        // The reason matters: "permission denied" and "it is already gone" call
        // for different things from the person reading it.
        Err(error) => format!("{}: {error}", copy::TRASH_FAILED),
    };
    // The key the host started this worker from, handed back so it can forget
    // exactly the record that moved rather than one that merely looks like it.
    let moved = pathkey::encode(path);
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
    let loaded = store.as_deref().map(catalogue_store::load);
    let (mut catalogue, store) = match loaded {
        Some(Ok(outcome)) => {
            if let Some(aside) = &outcome.set_aside {
                eprintln!(
                    "fluorita: the stored catalogue could not be read and was kept at {}",
                    aside.display()
                );
            }
            (outcome.catalogue, store)
        }
        // A catalogue that could not be read *or* set aside is still at its
        // name, so this run never saves over it: the library works from the
        // scan, and the next launch tries again.
        Some(Err(error)) => {
            eprintln!("fluorita: the stored catalogue is left untouched: {error}");
            (Catalogue::new(), None)
        }
        None => (Catalogue::new(), None),
    };
    catalogue.retain_configured(&sources);
    if !catalogue.is_empty() {
        let stored = project(&shared(&catalogue), &sources, scope, false, "stored");
        let _ = qt_thread.queue(move |library| library.apply(stored));
    }

    if sources.is_empty() {
        return publish_failure(qt_thread, &catalogue, &sources, scope, copy::NO_SOURCES);
    }

    let Ok(worker) = EngineWorker::start() else {
        return publish_failure(
            qt_thread,
            &catalogue,
            &sources,
            scope,
            copy::SCANNER_UNAVAILABLE,
        );
    };
    if worker
        .submit(Job::Scan {
            generation: celestina_core::Generation::INITIAL,
            sources: Box::new(sources.clone()),
            limits: ScanLimits::conservative(),
        })
        .is_err()
    {
        return publish_failure(
            qt_thread,
            &catalogue,
            &sources,
            scope,
            copy::SCANNER_UNAVAILABLE,
        );
    }

    let scanned = match await_job(&worker, cancellation, SCAN_TIMEOUT) {
        Waited::Finished(outcome) => outcome,
        // A cancelled scan publishes nothing: the host either replaced this
        // configuration or is going away, and both own what comes next.
        Waited::Cancelled => return,
        Waited::TimedOut => {
            return publish_failure(qt_thread, &catalogue, &sources, scope, copy::SCAN_TIMED_OUT)
        }
    };
    let JobOutcome::Scanned { result, .. } = scanned else {
        return publish_failure(qt_thread, &catalogue, &sources, scope, copy::SCAN_FAILED);
    };
    let Ok(outcome) = result else {
        return publish_failure(qt_thread, &catalogue, &sources, scope, copy::SCAN_FAILED);
    };

    let truncated = outcome.truncated;
    fold_scan(&mut catalogue, outcome);
    // A root that is no longer configured keeps no records: the scan cannot
    // refresh what it does not walk, and a stale entry would read as a file
    // that went missing.
    catalogue.retain_configured(&sources);

    // Tags are the expensive part, and the only reason this catalogue is worth
    // storing: what is read here is not read again unless the file changes.
    let pending = untagged_audio(&catalogue, |_| true, MAX_PROBES_PER_RUN);
    learn_tags(&worker, &mut catalogue, cancellation, pending);
    if cancellation.is_cancelled() {
        return;
    }

    // Best effort: a catalogue that could not be written is a slower next
    // launch, not a broken library, so it must not fail the scan on screen.
    if let Some(path) = store.as_deref() {
        let _ = catalogue_store::save(path, &catalogue);
    }

    let refreshed = project(&shared(&catalogue), &sources, scope, truncated, "ready");
    let _ = qt_thread.queue(move |library| library.apply(refreshed));

    // From here the library keeps itself up to date without walking again.
    watch_library(
        &sources,
        catalogue,
        store.as_deref(),
        &worker,
        qt_thread,
        cancellation,
    );
}

/// Folds one finished pass into the catalogue.
///
/// A file the walk did not find is marked missing only where the pass may
/// judge it, and forgotten only where that root also answered — so a deleted
/// file disappears, a drive that is not plugged in keeps everything it holds,
/// and a subtree the depth bound kept the walk out of is left exactly as it
/// was, in that root and no other.
fn fold_scan(catalogue: &mut Catalogue, outcome: fluorita_engine::ScanOutcome) {
    catalogue.absorb(outcome.records, &outcome.coverage);
    catalogue.forget_vanished(&outcome.coverage);
}

/// The catalogue as a publication hands it out: one copy, on this thread,
/// shared by every projection and verb that reads it afterwards.
fn shared(catalogue: &Catalogue) -> Arc<Catalogue> {
    Arc::new(catalogue.clone())
}

/// Folds changes in as they happen, until the host goes away.
///
/// This is the whole point of watching: a file dropped into a watched folder
/// appears without anyone asking, and one that goes away says so — without
/// re-walking roots that did not change. A burst, a folder moved in, or a
/// watcher that lost events asks for a walk instead, because folding in what
/// you did not see is guessing. Audio that arrived or changed is probed here
/// too, a bounded handful per batch: a retagged track must sort under what its
/// tags now say, not under "unknown artist" until the next launch.
pub(super) fn watch_library(
    sources: &SourceSet,
    mut catalogue: Catalogue,
    store: Option<&Path>,
    worker: &EngineWorker,
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

        let walk = || fluorita_engine::scan(sources, ScanLimits::conservative(), cancellation);
        let Ok(folded) = fold_batch(&mut catalogue, sources, batch, || watcher.drain(), walk)
        else {
            return;
        };
        let Folded {
            mut changed,
            touched,
            rewalked,
        } = folded;

        // A walk may have brought in a whole folder of music; otherwise only
        // what this batch touched is read again.
        let pending = if rewalked {
            untagged_audio(&catalogue, |_| true, MAX_PROBES_PER_BATCH)
        } else {
            untagged_audio(
                &catalogue,
                |record| touched.iter().any(|path| path == record.path()),
                MAX_PROBES_PER_BATCH,
            )
        };
        changed |= learn_tags(worker, &mut catalogue, cancellation, pending) > 0;
        if cancellation.is_cancelled() {
            return;
        }

        if !changed {
            continue;
        }
        if let Some(path) = store {
            let _ = catalogue_store::save(path, &catalogue);
        }
        // Projected by the host under the scope selected *then*, on its own
        // worker: the scan that started this watch captured a scope when it
        // began, and a folder the user has selected since would otherwise be
        // overwritten by the previous one's content the next time a file
        // moved. The copy is made here, off the GUI thread, once per batch.
        let published = shared(&catalogue);
        let configured = sources.clone();
        if qt_thread
            .queue(move |library| library.catalogue_changed(published, configured))
            .is_err()
        {
            return;
        }
    }
}

/// What folding one batch changed.
pub(super) struct Folded {
    pub(super) changed: bool,
    /// Paths the batch said appeared or changed, whose tags may need reading.
    pub(super) touched: Vec<PathBuf>,
    /// Whether a walk ran, which may have brought in untagged audio anywhere.
    pub(super) rewalked: bool,
}

/// A walk was cancelled; the host is going away or asked for another one.
#[derive(Debug)]
pub(super) struct Cancelled;

/// Folds one batch of watch changes into the catalogue.
///
/// A resync walks, through `walk`, after taking the batches already queued
/// behind it from `drain`: the walk answers their resyncs. The individual
/// changes among them are applied after the walk, each checked against the
/// disk again, because a walk does not judge every file — a ceiling or a
/// deadline stops it, the depth bound and unreadable folders keep it out, a
/// root may not answer — and dropping them would leave a deleted file on
/// screen and never add a new one.
pub(super) fn fold_batch(
    catalogue: &mut Catalogue,
    sources: &SourceSet,
    batch: Vec<LibraryChange>,
    mut drain: impl FnMut() -> Vec<Vec<LibraryChange>>,
    mut walk: impl FnMut() -> fluorita_engine::EngineResult<fluorita_engine::ScanOutcome>,
) -> Result<Folded, Cancelled> {
    let mut folded = Folded {
        changed: false,
        touched: Vec::new(),
        rewalked: false,
    };
    for change in batch {
        match change {
            LibraryChange::Touched(path) => {
                folded.changed |= absorb_one(catalogue, sources, &path);
                folded.touched.push(path);
            }
            LibraryChange::Removed(path) => {
                // The watcher saw this exact file go, in a root it is
                // watching right now, so the root plainly answers. That is
                // the same evidence a completed scan gives, so the record
                // goes rather than lingering as a permanently missing row.
                folded.changed |= forget_at(catalogue, &path);
            }
            LibraryChange::RemovedTree(path) => {
                // A folder left with everything in it, on the same evidence
                // as a single file going.
                folded.changed |= catalogue.forget_under(&path) > 0;
            }
            LibraryChange::Resync(_) => {
                // A run of these queued while a copy went on would each walk
                // the whole library; the walk about to happen answers every
                // one of them.
                let deferred: Vec<LibraryChange> = drain()
                    .into_iter()
                    .flatten()
                    .filter(|queued| !matches!(queued, LibraryChange::Resync(_)))
                    .collect();
                // Under the host's token: this walk can take as long as the
                // launch scan, and the host joins this thread from the GUI
                // when a folder is added or removed or the window closes.
                match walk() {
                    Ok(outcome) => {
                        fold_scan(catalogue, outcome);
                        folded.changed = true;
                        folded.rewalked = true;
                    }
                    Err(EngineError::Cancelled) => return Err(Cancelled),
                    // A walk that failed changes nothing it did not see; the
                    // next change asks again.
                    Err(_) => {}
                }
                for queued in deferred {
                    reapply(catalogue, sources, queued, &mut folded);
                }
            }
        }
    }
    Ok(folded)
}

/// Applies a change that waited behind a walk, checked against the disk as
/// it is now: a file that is back is not forgotten, and one that is gone is
/// not added.
fn reapply(
    catalogue: &mut Catalogue,
    sources: &SourceSet,
    change: LibraryChange,
    folded: &mut Folded,
) {
    let gone = |path: &Path| std::fs::symlink_metadata(path).is_err();
    match change {
        LibraryChange::Touched(path) => {
            // `absorb_one` stats the file itself and does nothing for one
            // that is not there.
            folded.changed |= absorb_one(catalogue, sources, &path);
            folded.touched.push(path);
        }
        LibraryChange::Removed(path) if gone(&path) => {
            folded.changed |= forget_at(catalogue, &path);
        }
        LibraryChange::RemovedTree(path) if gone(&path) => {
            folded.changed |= catalogue.forget_under(&path) > 0;
        }
        LibraryChange::Removed(_) | LibraryChange::RemovedTree(_) | LibraryChange::Resync(_) => {}
    }
}

/// Forgets the record whose file lived at `path`. Returns whether one went.
fn forget_at(catalogue: &mut Catalogue, path: &Path) -> bool {
    let id = catalogue
        .find_by_path(path)
        .map(|record| record.id().clone());
    id.is_some_and(|id| catalogue.forget(&id).is_some())
}

/// Stats one changed path and folds it in. Returns whether anything moved.
pub(super) fn absorb_one(catalogue: &mut Catalogue, sources: &SourceSet, path: &Path) -> bool {
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
        match sources_owner(sources, path, kind) {
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

    // Only this file is judged, and nothing else may be concluded to have
    // disappeared — except a record at this very path under another identity,
    // which is the file this one replaced.
    let summary = catalogue.absorb_changed(record);
    summary.added + summary.replaced > 0
}

/// The configured root that owns a changed file, decided by the configuration
/// itself rather than by what happens to sit near it in the catalogue.
///
/// Guessing from a neighbouring record fell back to the first record in the
/// whole catalogue whenever the parent directory matched nothing — so a file
/// created in a subfolder nobody had scanned yet was filed under an unrelated
/// root. Roots cannot nest, so there is exactly one right answer or none.
pub(super) fn sources_owner(
    sources: &SourceSet,
    path: &Path,
    kind: MediaKind,
) -> Option<fluorita_core::SourceId> {
    sources.owner_of(path, kind).map(MediaSource::id)
}

/// Audio the catalogue has never probed, among the records `wanted` accepts,
/// at most `cap` of them.
///
/// Only audio, and only what has no duration yet: a video's tags are not what
/// Gallery shows, and a track that was probed before keeps what it learned
/// because its size and mtime say the bytes are the same.
pub(super) fn untagged_audio(
    catalogue: &Catalogue,
    wanted: impl Fn(&MediaRecord) -> bool,
    cap: usize,
) -> Vec<(PathBuf, MediaId)> {
    catalogue
        .records()
        .filter(|record| record.kind() == MediaKind::Audio)
        .filter(|record| record.is_available() && record.metadata().duration.is_none())
        .filter(|record| wanted(record))
        .take(cap)
        .map(|record| (record.path().to_path_buf(), record.id().clone()))
        .collect()
}

/// Reads tags for `pending`, returning how many were learned.
pub(super) fn learn_tags(
    worker: &EngineWorker,
    catalogue: &mut Catalogue,
    cancellation: &CancellationToken,
    pending: Vec<(PathBuf, MediaId)>,
) -> usize {
    let mut learned = 0;
    for (path, id) in pending {
        // Five hundred probes of up to fifteen seconds each is minutes of work
        // the host must be able to interrupt between two of them, not only
        // after the last.
        if cancellation.is_cancelled() {
            break;
        }
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
        let Waited::Finished(JobOutcome::Probed { result, .. }) =
            await_job(worker, cancellation, PROBE_TIMEOUT)
        else {
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

/// Reports a failure without losing the sidebar or the library.
///
/// The roots stay configured, or there would be no way to add or remove one
/// after a scan went wrong — and the catalogue that was already on screen stays
/// with them. Projecting an empty one emptied a stored library the user was
/// looking at, which reads as data loss for what is only a walk that failed.
pub(super) fn publish_failure(
    qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>,
    catalogue: &Catalogue,
    sources: &SourceSet,
    scope: SourceScope,
    message: &str,
) {
    let mut snapshot = project(&shared(catalogue), sources, scope, false, "error");
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
pub(crate) fn media_directories() -> XdgMediaDirs {
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

#[cfg(test)]
mod tests {
    use super::{await_outcome, fold_batch, sources_owner, Waited, CANCEL_POLL};
    use celestina_core::CancellationToken;
    use fluorita_core::{KindSet, MediaKind, SourceSet};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn two_roots() -> SourceSet {
        let mut sources = SourceSet::new();
        sources
            .add(PathBuf::from("/mnt/pictures"), KindSet::all())
            .expect("an absolute root the set has never seen");
        sources
            .add(PathBuf::from("/mnt/music"), KindSet::all())
            .expect("a second root that does not nest in the first");
        sources
    }

    #[test]
    fn a_new_file_is_filed_under_the_root_that_contains_it() {
        let sources = two_roots();
        let owner = sources_owner(
            &sources,
            Path::new("/mnt/music/2026/track.flac"),
            MediaKind::Audio,
        );

        let expected = sources
            .owner_of(Path::new("/mnt/music"), MediaKind::Audio)
            .map(fluorita_core::MediaSource::id);
        assert_eq!(owner, expected);
    }

    #[test]
    fn a_file_under_no_configured_root_has_no_owner() {
        // The guess this replaced answered with the catalogue's first record,
        // so a file nobody configured landed under an unrelated folder.
        assert_eq!(
            sources_owner(&two_roots(), Path::new("/tmp/loose.png"), MediaKind::Image),
            None
        );
    }

    #[test]
    fn a_finished_job_is_reported_as_it_arrives() {
        let cancellation = CancellationToken::new();
        let mut calls = 0;
        let waited = await_outcome(
            &cancellation,
            Duration::from_secs(30),
            CANCEL_POLL,
            |_slice| {
                calls += 1;
                (calls == 3).then_some("scanned")
            },
        );

        assert_eq!(waited, Waited::Finished("scanned"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn a_cancelled_wait_returns_without_spending_the_budget() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut calls = 0;
        // A 180 s budget, and not one wait: this is the difference between a
        // folder change that answers at once and an interface frozen for
        // minutes, because the host joins this thread from the GUI.
        let waited = await_outcome::<&str, _>(
            &cancellation,
            Duration::from_secs(180),
            CANCEL_POLL,
            |_slice| {
                calls += 1;
                None
            },
        );

        assert_eq!(waited, Waited::Cancelled);
        assert_eq!(calls, 0);
    }

    #[test]
    fn a_silent_engine_gives_the_budget_back_in_slices() {
        let cancellation = CancellationToken::new();
        let mut slices = Vec::new();
        let waited = await_outcome::<&str, _>(
            &cancellation,
            Duration::from_millis(250),
            Duration::from_millis(100),
            |slice| {
                slices.push(slice);
                None
            },
        );

        assert_eq!(waited, Waited::TimedOut);
        assert!(slices.len() >= 2, "the wait was not split at all");
        assert!(
            slices
                .iter()
                .all(|slice| *slice <= Duration::from_millis(100)),
            "a slice outran the chunk, so the token would go unread that long"
        );
    }

    #[test]
    fn changes_queued_behind_a_walk_still_apply_where_the_walk_cannot_judge() {
        use fluorita_core::{Catalogue, MediaId, MediaRecord, SourceIdentity};
        use fluorita_engine::{LibraryChange, ResyncReason, ScanLimits};

        let scratch =
            std::env::temp_dir().join(format!("fluorita-fold-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let (crowded, quiet) = (scratch.join("crowded"), scratch.join("quiet"));
        std::fs::create_dir_all(&crowded).expect("first root");
        std::fs::create_dir_all(&quiet).expect("second root");
        for index in 0..4 {
            std::fs::write(crowded.join(format!("{index}.png")), b"").expect("fixture");
        }
        let arrived = quiet.join("arrived.png");
        std::fs::write(&arrived, b"").expect("fixture");
        let deleted = quiet.join("deleted.png");

        let mut sources = SourceSet::new();
        sources
            .add(crowded.clone(), KindSet::all())
            .expect("an absolute root");
        let second = sources
            .add(quiet.clone(), KindSet::all())
            .expect("a second root");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(MediaRecord::new(
            MediaId::filesystem(1, 1),
            second,
            deleted.clone(),
            MediaKind::Image,
            SourceIdentity::new(0, std::time::UNIX_EPOCH),
        ));

        // The file ceiling stops the walk in the first root, so it judges
        // nothing in the second: only the queued changes can say that one
        // file there went and another arrived.
        let queued = vec![vec![
            LibraryChange::Removed(deleted.clone()),
            LibraryChange::Touched(arrived.clone()),
        ]];
        let mut drained = Some(queued);
        let folded = fold_batch(
            &mut catalogue,
            &sources,
            vec![LibraryChange::Resync(ResyncReason::Burst)],
            || drained.take().unwrap_or_default(),
            || {
                fluorita_engine::scan(
                    &sources,
                    ScanLimits {
                        max_files: 2,
                        ..ScanLimits::conservative()
                    },
                    &CancellationToken::new(),
                )
            },
        )
        .expect("not cancelled");

        assert!(folded.changed);
        assert!(
            catalogue.find_by_path(&deleted).is_none(),
            "the deleted file stayed"
        );
        assert!(
            catalogue.find_by_path(&arrived).is_some(),
            "the new file never came"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_queued_removal_of_a_file_that_is_back_is_not_applied() {
        use fluorita_core::{Catalogue, MediaId, MediaRecord, SourceIdentity};
        use fluorita_engine::{LibraryChange, ResyncReason, ScanOutcome};

        let root = std::env::temp_dir().join(format!("fluorita-fold-back-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");
        let back = root.join("back.png");
        std::fs::write(&back, b"").expect("fixture");
        let mut sources = SourceSet::new();
        let only = sources.add(root.clone(), KindSet::all()).expect("a root");
        let mut catalogue = Catalogue::new();
        catalogue.upsert(MediaRecord::new(
            MediaId::filesystem(1, 1),
            only,
            back.clone(),
            MediaKind::Image,
            SourceIdentity::new(0, std::time::UNIX_EPOCH),
        ));

        let mut drained = Some(vec![vec![LibraryChange::Removed(back.clone())]]);
        fold_batch(
            &mut catalogue,
            &sources,
            vec![LibraryChange::Resync(ResyncReason::Burst)],
            || drained.take().unwrap_or_default(),
            // A walk that failed: nothing it did not see changes.
            || Err::<ScanOutcome, _>(fluorita_engine::EngineError::WorkerStopped),
        )
        .expect("not cancelled");

        assert!(catalogue.find_by_path(&back).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
