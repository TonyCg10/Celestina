//! The Qt half of the library.
//!
//! What the library *is* — the roots, the catalogue, the Gallery and Music
//! projections — lives in `fluorita-core`; walking the disk lives in
//! `fluorita-engine`. This file moves the result to QML under the same rules
//! the player follows:
//!
//! - **The GUI thread never walks a directory.** The scan runs on the engine's
//!   worker and arrives through the queue.
//! - **Browsing starts no decoder.** Thumbnails are read from the shared
//!   freedesktop cache if something else already produced them; a missing one
//!   stays missing rather than starting the media backend for a grid.
//! - **Producing the missing ones is a decision, not a side effect.** It does
//!   start the backend — that is what generating a poster *is* — so it happens
//!   only when the user asks for it, bounded and cancellable, never on launch.
//! - **A truncated scan says so.** Reconciliation may only conclude that a file
//!   disappeared from a pass that actually finished.
//! - **What was learned is not learned again.** The catalogue is read from disk
//!   before the walk and published straight away, so the window opens on the
//!   library it had; the walk then refreshes it and only files whose bytes
//!   actually changed lose their extracted metadata.
//!
//! Rows travel as parallel `QStringList`s rather than a native model. That is a
//! measured choice, not a shortcut: the author's library is 94 items found in
//! 251 µs, and CXX-Qt 0.9 cannot override `QAbstractListModel`'s virtuals from
//! Rust, so a native model would mean a second hand-written C++ model beside
//! Siderita's. If a real library ever reaches a few thousand items — the point
//! where rebuilding these lists on every change starts to show — that is the
//! moment to write it, with the numbers in hand.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::Duration;

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use fluorita_core::{
    gallery, Catalogue, GalleryFilter, GalleryOrder, MediaKind, MusicLibrary, SourceSet,
    XdgMediaDirs,
};
use fluorita_engine::backend::ArtworkJob;
use fluorita_engine::worker::{EngineWorker, Job, JobOutcome};
use fluorita_engine::{catalogue_store, ScanLimits};

/// How long the worker waits for the scan before checking whether the host is
/// still there. A scan of a large library can legitimately take a while.
const SCAN_TIMEOUT: Duration = Duration::from_secs(180);

/// Tag reads per launch. Each costs a backend probe of tens of milliseconds, so
/// a first run over a large music library is bounded and simply finishes the
/// rest next time — which is exactly what the stored catalogue makes possible.
const MAX_PROBES_PER_RUN: usize = 500;

/// A probe that takes longer than this is a file that will not answer.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Posters and covers produced per explicit pass. A large library finishes
/// across several passes rather than holding the backend open indefinitely.
const MAX_ARTWORK_PER_PASS: usize = 200;

/// Extracting one frame is seconds of work at most; longer means a file that
/// will not give one up.
const ARTWORK_TIMEOUT: Duration = Duration::from_secs(30);

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
        /// The library surface QML binds to.
        #[qobject]
        #[qml_element]
        /// `vacía` before a scan, `explorando` while one runs, `lista` when it
        /// finished, `error` when it could not.
        #[qproperty(QString, state)]
        /// A sentence for the header: what was found, or why nothing was.
        #[qproperty(QString, summary)]
        /// True when a bound was reached, so the grid is not the whole library.
        #[qproperty(bool, truncated)]
        #[qproperty(i32, image_count)]
        #[qproperty(i32, video_count)]
        #[qproperty(i32, track_count)]
        /// How many items would need a thumbnail produced. Zero hides the
        /// whole idea from the interface, because there is nothing to do.
        #[qproperty(i32, artwork_pending)]
        /// `parada`, `generando` or `cancelando`.
        #[qproperty(QString, artwork_state)]
        /// Produced so far in the running pass, and what it set out to do.
        #[qproperty(i32, artwork_done)]
        #[qproperty(i32, artwork_total)]
        /// Bumped once, after every list of a scan is in place. QML rebuilds
        /// its rows from this and never from the lists directly: publishing
        /// four lists one by one makes the bindings re-run between them, with
        /// half the columns still holding the previous scan.
        #[qproperty(i32, revision)]
        /// Gallery rows, index-aligned: absolute path, display name, kind and
        /// the cached thumbnail URL (empty when nothing produced one).
        #[qproperty(QStringList, gallery_paths)]
        #[qproperty(QStringList, gallery_names)]
        #[qproperty(QStringList, gallery_kinds)]
        #[qproperty(QStringList, gallery_thumbnails)]
        /// Music rows, index-aligned and already in projection order, so a
        /// `ListView` can section on the artist without sorting anything.
        #[qproperty(QStringList, music_paths)]
        #[qproperty(QStringList, music_titles)]
        #[qproperty(QStringList, music_artists)]
        #[qproperty(QStringList, music_albums)]
        type FluoritaLibrary = super::LibraryRust;

        /// Walks the configured roots. Safe to call again: a scan in flight is
        /// cancelled and replaced.
        #[qinvokable]
        fn scan(self: Pin<&mut FluoritaLibrary>);

        /// Stops a scan in progress and joins its thread.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaLibrary>);

        /// Produces the thumbnails the shared cache is missing, for video and
        /// audio only. This is the one thing here that starts the media
        /// backend, which is why nothing but an explicit request calls it.
        #[qinvokable]
        fn generate_artwork(self: Pin<&mut FluoritaLibrary>);

        /// Asks the running pass to stop at the next item.
        #[qinvokable]
        fn cancel_artwork(self: Pin<&mut FluoritaLibrary>);
    }

    impl cxx_qt::Threading for FluoritaLibrary {}
}

pub struct LibraryRust {
    state: QString,
    summary: QString,
    truncated: bool,
    image_count: i32,
    video_count: i32,
    track_count: i32,
    artwork_pending: i32,
    artwork_state: QString,
    artwork_done: i32,
    artwork_total: i32,
    revision: i32,

    gallery_paths: QStringList,
    gallery_names: QStringList,
    gallery_kinds: QStringList,
    gallery_thumbnails: QStringList,

    music_paths: QStringList,
    music_titles: QStringList,
    music_artists: QStringList,
    music_albums: QStringList,

    worker: Option<JoinHandle<()>>,
    /// The catalogue as last published, so an artwork pass knows what to look
    /// at without walking anything again.
    catalogue: Catalogue,
    artwork_worker: Option<JoinHandle<()>>,
    artwork_cancellation: CancellationToken,
}

// Written out rather than derived: a default `QString` is empty, and an empty
// state is a state the interface has to guess at. Both of these are read by a
// binding before anything has run.
impl Default for LibraryRust {
    fn default() -> Self {
        Self {
            state: QString::from("vacía"),
            summary: QString::default(),
            truncated: false,
            image_count: 0,
            video_count: 0,
            track_count: 0,
            artwork_pending: 0,
            artwork_state: QString::from("parada"),
            artwork_done: 0,
            artwork_total: 0,
            revision: 0,
            gallery_paths: QStringList::default(),
            gallery_names: QStringList::default(),
            gallery_kinds: QStringList::default(),
            gallery_thumbnails: QStringList::default(),
            music_paths: QStringList::default(),
            music_titles: QStringList::default(),
            music_artists: QStringList::default(),
            music_albums: QStringList::default(),
            worker: None,
            catalogue: Catalogue::new(),
            artwork_worker: None,
            artwork_cancellation: CancellationToken::new(),
        }
    }
}

/// Everything one publication produces, already shaped for QML.
#[derive(Default)]
struct LibrarySnapshot {
    /// `guardada` while showing what was stored and the walk is still running,
    /// `lista` once the walk has been folded in, `error` when it could not be.
    state: &'static str,
    summary: String,
    truncated: bool,
    image_count: i32,
    video_count: i32,
    track_count: i32,
    gallery: Vec<[String; 4]>,
    music: Vec<[String; 4]>,
    /// What the projection was made from, kept so an explicit artwork pass has
    /// something to work on without re-walking the disk.
    catalogue: Catalogue,
    /// How many items the shared cache has no usable thumbnail for.
    artwork_pending: i32,
}

impl qobject::FluoritaLibrary {
    pub fn scan(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().close();
        self.as_mut().set_state(QString::from("explorando"));
        self.as_mut()
            .set_summary(QString::from("Explorando tus carpetas…"));

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-library".to_owned())
            .spawn(move || {
                run_scan(&qt_thread);
            });

        match worker {
            Ok(handle) => self.as_mut().rust_mut().worker = Some(handle),
            Err(_) => {
                self.as_mut().set_state(QString::from("error"));
                self.as_mut()
                    .set_summary(QString::from("No se pudo iniciar la exploración"));
            }
        }
    }

    pub fn close(mut self: core::pin::Pin<&mut Self>) {
        // Dropping the worker's own `EngineWorker` cancels the scan, so joining
        // cannot wait on a walk that would otherwise finish the whole disk.
        if let Some(handle) = self.as_mut().rust_mut().worker.take() {
            let _ = handle.join();
        }
    }

    /// Starts the explicit artwork pass.
    pub fn generate_artwork(mut self: core::pin::Pin<&mut Self>) {
        if self.artwork_state() == &QString::from("generando") {
            return;
        }
        let catalogue = self.rust().catalogue.clone();
        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().artwork_cancellation = cancellation.clone();
        self.as_mut().set_artwork_state(QString::from("generando"));
        self.as_mut().set_artwork_done(0);

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-artwork".to_owned())
            .spawn(move || run_artwork(&catalogue, &cancellation, &qt_thread));
        match worker {
            Ok(handle) => self.as_mut().rust_mut().artwork_worker = Some(handle),
            Err(_) => self.as_mut().set_artwork_state(QString::from("parada")),
        }
    }

    pub fn cancel_artwork(mut self: core::pin::Pin<&mut Self>) {
        self.rust().artwork_cancellation.cancel();
        self.as_mut().set_artwork_state(QString::from("cancelando"));
    }

    /// One produced thumbnail, reported from the pass.
    fn artwork_progress(mut self: core::pin::Pin<&mut Self>, done: i32, total: i32) {
        self.as_mut().set_artwork_done(done);
        self.as_mut().set_artwork_total(total);
    }

    /// The pass finished: refresh the grid so the new thumbnails appear.
    fn artwork_finished(mut self: core::pin::Pin<&mut Self>, produced: i32) {
        self.as_mut().set_artwork_state(QString::from("parada"));
        if let Some(handle) = self.as_mut().rust_mut().artwork_worker.take() {
            let _ = handle.join();
        }
        if produced > 0 {
            let catalogue = self.rust().catalogue.clone();
            let refreshed = project(&catalogue, *self.truncated(), "lista");
            self.apply(refreshed);
        }
    }

    /// Publishes a finished scan. Runs on the GUI thread, through the queue.
    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: LibrarySnapshot) {
        self.as_mut().set_summary(QString::from(&snapshot.summary));
        self.as_mut().set_truncated(snapshot.truncated);
        self.as_mut().set_image_count(snapshot.image_count);
        self.as_mut().set_video_count(snapshot.video_count);
        self.as_mut().set_track_count(snapshot.track_count);

        let gallery = columns(&snapshot.gallery);
        self.as_mut().set_gallery_paths(gallery[0].clone());
        self.as_mut().set_gallery_names(gallery[1].clone());
        self.as_mut().set_gallery_kinds(gallery[2].clone());
        self.as_mut().set_gallery_thumbnails(gallery[3].clone());

        let music = columns(&snapshot.music);
        self.as_mut().set_music_paths(music[0].clone());
        self.as_mut().set_music_titles(music[1].clone());
        self.as_mut().set_music_artists(music[2].clone());
        self.as_mut().set_music_albums(music[3].clone());

        self.as_mut().set_artwork_pending(snapshot.artwork_pending);
        self.as_mut().rust_mut().catalogue = snapshot.catalogue;
        self.as_mut().set_state(QString::from(snapshot.state));
        // Last, and only once: this is what QML watches.
        let next = self.revision().wrapping_add(1);
        self.as_mut().set_revision(next);
    }
}

impl Drop for LibraryRust {
    fn drop(&mut self) {
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        // Cancel before joining: an artwork pass would otherwise hold the
        // window open for as long as the backend needs for the current file.
        self.artwork_cancellation.cancel();
        if let Some(handle) = self.artwork_worker.take() {
            let _ = handle.join();
        }
    }
}

/// The explicit pass, on its own thread: produce what the cache is missing.
fn run_artwork(
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
fn columns(rows: &[[String; 4]]) -> [QStringList; 4] {
    let mut lists = [
        QStringList::default(),
        QStringList::default(),
        QStringList::default(),
        QStringList::default(),
    ];
    for row in rows {
        for (column, value) in row.iter().enumerate() {
            lists[column].append(QString::from(value));
        }
    }
    lists
}

/// The whole thing, on the worker thread: read what is stored, show it, walk,
/// fold the walk in, write it back.
fn run_scan(qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>) {
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
}

/// Reads tags for audio the catalogue has never probed.
///
/// Only audio, and only what has no duration yet: a video's tags are not what
/// Gallery shows, and a track that was probed before keeps what it learned
/// because its size and mtime say the bytes are the same.
fn learn_tags(worker: &EngineWorker, catalogue: &mut Catalogue) -> usize {
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

fn publish_failure(qt_thread: &cxx_qt::CxxQtThread<qobject::FluoritaLibrary>, message: &str) {
    let snapshot = LibrarySnapshot {
        state: "error",
        summary: message.to_owned(),
        ..LibrarySnapshot::default()
    };
    let _ = qt_thread.queue(move |library| library.apply(snapshot));
}

/// Projects the catalogue into the two surfaces.
fn project(catalogue: &Catalogue, truncated: bool, state: &'static str) -> LibrarySnapshot {
    let cache_root = thumbnail_cache_root();
    let items = gallery(catalogue, GalleryFilter::All, GalleryOrder::NewestFirst);

    let image_count = items
        .iter()
        .filter(|item| item.kind == MediaKind::Image)
        .count();
    let video_count = items.len() - image_count;

    let gallery_rows: Vec<[String; 4]> = items
        .iter()
        .map(|item| {
            [
                item.path.to_string_lossy().into_owned(),
                item.display_name.clone(),
                kind_label(item.kind).to_owned(),
                cached_thumbnail(cache_root.as_deref(), &item.path),
            ]
        })
        .collect();

    let music = MusicLibrary::project(catalogue);
    let music_rows: Vec<[String; 4]> = music
        .artists
        .iter()
        .flat_map(|artist| {
            artist.albums.iter().flat_map(move |album| {
                album.tracks.iter().map(move |track| {
                    [
                        track.path.to_string_lossy().into_owned(),
                        track.display_name.clone(),
                        artist.name.clone().unwrap_or_else(unknown_artist),
                        album.title.clone().unwrap_or_else(unknown_album),
                    ]
                })
            })
        })
        .collect();

    let artwork_pending = cache_root.as_deref().map_or(0, |root| {
        i32::try_from(fluorita_engine::pending_artwork(catalogue, root, MAX_ARTWORK_PER_PASS).len())
            .unwrap_or(i32::MAX)
    });

    LibrarySnapshot {
        state,
        catalogue: catalogue.clone(),
        artwork_pending,
        summary: summarize(image_count, video_count, music_rows.len(), truncated),
        truncated,
        image_count: i32::try_from(image_count).unwrap_or(i32::MAX),
        video_count: i32::try_from(video_count).unwrap_or(i32::MAX),
        track_count: i32::try_from(music_rows.len()).unwrap_or(i32::MAX),
        gallery: gallery_rows,
        music: music_rows,
    }
}

fn unknown_artist() -> String {
    "Sin artista".to_owned()
}

fn unknown_album() -> String {
    "Sin álbum".to_owned()
}

const fn kind_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "imagen",
        MediaKind::Video => "vídeo",
        MediaKind::Audio => "audio",
    }
}

/// The shared thumbnail entry for a file, but **only if it already exists**.
///
/// Browsing never produces artwork: that would start the media backend for
/// every card in a grid, which is exactly the cost the suite's contract keeps
/// out of normal browsing. A missing thumbnail is an empty string and the
/// delegate shows a themed glyph instead.
fn cached_thumbnail(cache_root: Option<&Path>, source: &Path) -> String {
    let Some(root) = cache_root else {
        return String::new();
    };
    let Some(entry) = fluorita_core::large_thumbnail_path(root, source) else {
        return String::new();
    };
    if !entry.is_file() {
        return String::new();
    }
    fluorita_core::file_uri(&entry).unwrap_or_default()
}

fn thumbnail_cache_root() -> Option<PathBuf> {
    celestina_core::xdg::cache_home().map(|cache| cache.join("thumbnails"))
}

/// The XDG media directories, as they exist on this machine.
///
/// A directory that is not there is simply not configured — seeding must never
/// fail a first run, and a library that invented folders would be worse than an
/// empty one.
fn media_directories() -> XdgMediaDirs {
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

/// What the header says. Counts are what the scan actually saw, and a
/// truncated pass says so instead of reading like a complete inventory.
fn summarize(images: usize, videos: usize, tracks: usize, truncated: bool) -> String {
    if images == 0 && videos == 0 && tracks == 0 {
        return "No hay medios en tus carpetas".to_owned();
    }
    let mut parts: Vec<String> = Vec::new();
    if images > 0 {
        parts.push(format!("{images} {}", plural(images, "imagen", "imágenes")));
    }
    if videos > 0 {
        parts.push(format!("{videos} {}", plural(videos, "vídeo", "vídeos")));
    }
    if tracks > 0 {
        parts.push(format!("{tracks} {}", plural(tracks, "pista", "pistas")));
    }
    let counted = parts.join(" · ");
    if truncated {
        format!("{counted} (exploración incompleta: se alcanzó un límite)")
    } else {
        counted
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 { one } else { many }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{cached_thumbnail, kind_label, summarize};
    use fluorita_core::MediaKind;
    use std::path::Path;

    #[test]
    fn the_summary_counts_what_was_found() {
        assert_eq!(summarize(86, 8, 0, false), "86 imágenes · 8 vídeos");
        assert_eq!(summarize(1, 1, 1, false), "1 imagen · 1 vídeo · 1 pista");
        assert_eq!(summarize(0, 0, 0, false), "No hay medios en tus carpetas");
    }

    #[test]
    fn a_truncated_scan_never_reads_like_a_complete_inventory() {
        let summary = summarize(50_000, 0, 0, true);
        assert!(summary.contains("incompleta"));
        assert!(summary.contains("50000 imágenes"));
    }

    #[test]
    fn a_missing_thumbnail_is_empty_rather_than_generated() {
        // Nothing produced this entry, so browsing must show a glyph — not
        // start a decoder to make one.
        let root = std::env::temp_dir().join("fluorita-library-tests");
        std::fs::create_dir_all(&root).expect("scratch");
        assert_eq!(
            cached_thumbnail(Some(&root), Path::new("/home/toni/Vídeos/clip.mkv")),
            ""
        );
        assert_eq!(cached_thumbnail(None, Path::new("/home/toni/x.png")), "");
    }

    #[test]
    fn an_existing_thumbnail_is_offered_as_a_url() {
        let root = std::env::temp_dir().join("fluorita-library-tests/cache");
        let source = Path::new("/home/toni/Vídeos/clip con espacio.mkv");
        let entry = fluorita_core::large_thumbnail_path(&root, source).expect("cache path");
        std::fs::create_dir_all(entry.parent().expect("parent")).expect("cache dir");
        std::fs::write(&entry, b"fake png").expect("entry");

        let url = cached_thumbnail(Some(&root), source);

        assert!(url.starts_with("file://"), "unexpected url: {url}");
        assert!(url.ends_with(".png"));
        std::fs::remove_file(&entry).ok();
    }

    #[test]
    fn kind_labels_are_the_spanish_the_interface_shows() {
        assert_eq!(kind_label(MediaKind::Image), "imagen");
        assert_eq!(kind_label(MediaKind::Video), "vídeo");
        assert_eq!(kind_label(MediaKind::Audio), "audio");
    }
}
