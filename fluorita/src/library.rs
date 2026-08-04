//! The Qt half of the library.
//!
//! What the library *is* — the roots, the catalogue, the Gallery and Music
//! projections — lives in `fluorita-core`; walking the disk and storing the
//! configuration live in `fluorita-engine`. This file moves the result to QML
//! under the same rules the player follows:
//!
//! - **The GUI thread never walks a directory.** The scan runs on the engine's
//!   worker and arrives through the queue, and so does the folder chooser: a
//!   portal request lasts as long as the person takes to decide.
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
//! The library is navigated by configured root, so the sidebar rows and the
//! selected scope are published beside the content. Selecting a folder is a
//! re-projection of the catalogue this object already holds, not a new walk;
//! adding or removing one changes the stored configuration and re-enters the
//! single scan path.
//!
//! Rows travel as parallel `QStringList`s rather than a native model. That is a
//! measured choice, not a shortcut: the author's library is 94 items found in
//! 251 µs, and CXX-Qt 0.9 cannot override `QAbstractListModel`'s virtuals from
//! Rust, so a native model would mean a second hand-written C++ model beside
//! Siderita's. If a real library ever reaches a few thousand items — the point
//! where rebuilding these lists on every change starts to show — that is the
//! moment to write it, with the numbers in hand.

use std::thread::JoinHandle;

use celestina_core::CancellationToken;

mod copy;
mod detail;
mod project;
mod work;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use project::{project, LibrarySnapshot};
use work::{run_artwork, run_folder_choice, run_scan, run_trash};

use fluorita_core::{Catalogue, SourceId, SourceScope, SourceSet};

/// What `selectedSource` holds when nothing is selected: every configured root
/// at once. A real [`SourceId`] is a `u32`, so no handle can collide with it.
const EVERY_SOURCE: i32 = -1;

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
        /// `empty` before a scan, `scanning` while one runs, `stored` while the
        /// last known library is on screen, `ready` when the walk finished,
        /// `error` when it could not.
        #[qproperty(QString, state)]
        /// A sentence for the header: what the selected scope holds, or why it
        /// holds nothing.
        #[qproperty(QString, summary)]
        /// True when a bound was reached, so the grid is not the whole library.
        #[qproperty(bool, truncated)]
        #[qproperty(i32, image_count)]
        #[qproperty(i32, video_count)]
        #[qproperty(i32, track_count)]
        /// How many items would need a thumbnail produced. Zero hides the
        /// whole idea from the interface, because there is nothing to do.
        #[qproperty(i32, artwork_pending)]
        /// `idle`, `generating` or `cancelling`.
        #[qproperty(QString, artwork_state)]
        /// Produced so far in the running pass, and what it set out to do.
        #[qproperty(i32, artwork_done)]
        #[qproperty(i32, artwork_total)]
        /// Bumped once, after every list of a publication is in place. QML
        /// rebuilds its rows from this and never from the lists directly:
        /// publishing several lists one by one makes the bindings re-run
        /// between them, with half the columns still holding the previous
        /// publication.
        #[qproperty(i32, revision)]
        /// The sidebar, index-aligned: the root's handle as text, the label to
        /// show and the root itself. Configuration order, which is the order
        /// the user built.
        #[qproperty(QStringList, source_ids)]
        #[qproperty(QStringList, source_names)]
        #[qproperty(QStringList, source_paths)]
        /// The selected root's handle, or `-1` for every root at once. The
        /// content below is always exactly this scope.
        #[qproperty(i32, selected_source)]
        /// True while the desktop's folder chooser is open. The button says so
        /// rather than looking dead for as long as the person takes to decide.
        #[qproperty(bool, choosing_folder)]
        /// Why the last folder request could not be answered, or empty. A
        /// cancelled dialog is not a failure and leaves this empty.
        #[qproperty(QString, folder_notice)]
        /// The properties panel: open, and the item it describes. Every field
        /// is already a display string, and filling them opens no file.
        #[qproperty(bool, detail_open)]
        #[qproperty(QString, detail_name)]
        #[qproperty(QString, detail_path)]
        #[qproperty(QString, detail_kind)]
        #[qproperty(QString, detail_size)]
        #[qproperty(QString, detail_modified)]
        #[qproperty(QString, detail_duration)]
        #[qproperty(QString, detail_folder)]
        /// Set when the described file is not where the catalogue saw it.
        #[qproperty(QString, detail_notice)]
        /// What happened to the last item action, or empty. A successful trash
        /// says so too: a row vanishing with no word for it reads as a crash.
        #[qproperty(QString, item_notice)]
        /// Gallery rows, index-aligned: absolute path, display name, kind and
        /// the cached thumbnail URL (empty when nothing produced one).
        #[qproperty(QStringList, gallery_paths)]
        #[qproperty(QStringList, gallery_names)]
        #[qproperty(QStringList, gallery_kinds)]
        #[qproperty(QStringList, gallery_thumbnails)]
        /// `1` while the file is where the catalogue last saw it, `0` once a
        /// scan or the watch found it gone. A missing item stays in the grid —
        /// a disconnected drive is not data loss — but it must say so.
        #[qproperty(QStringList, gallery_available)]
        /// Music rows, index-aligned and already in projection order, so a
        /// `ListView` can section on the artist without sorting anything.
        #[qproperty(QStringList, music_paths)]
        #[qproperty(QStringList, music_titles)]
        #[qproperty(QStringList, music_artists)]
        #[qproperty(QStringList, music_albums)]
        #[qproperty(QStringList, music_available)]
        /// The cached cover for each track, empty when nothing produced one.
        #[qproperty(QStringList, music_thumbnails)]
        type FluoritaLibrary = super::LibraryRust;

        /// Walks the configured roots. Safe to call again: a scan in flight is
        /// cancelled and replaced.
        #[qinvokable]
        fn scan(self: Pin<&mut FluoritaLibrary>);

        /// Stops a scan and its watch, and joins the thread.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaLibrary>);

        /// Shows one configured root, or every root when given `-1`. This only
        /// re-projects the catalogue already in hand; nothing is walked.
        #[qinvokable]
        fn select_source(self: Pin<&mut FluoritaLibrary>, source: i32);

        /// Asks the desktop for a folder to map. Returns at once: the request
        /// is answered on a worker, and the result arrives through the queue.
        #[qinvokable]
        fn add_folder(self: Pin<&mut FluoritaLibrary>);

        /// Stops reading a root. Its catalogue entries go with it; not one of
        /// its files is touched.
        #[qinvokable]
        fn remove_folder(self: Pin<&mut FluoritaLibrary>, source: i32);

        /// Fills the properties panel for one item and opens it. Reads only
        /// what the catalogue already knows; it starts no decoder.
        #[qinvokable]
        fn describe_item(self: Pin<&mut FluoritaLibrary>, path: &QString);

        /// Closes the properties panel.
        #[qinvokable]
        fn close_detail(self: Pin<&mut FluoritaLibrary>);

        /// Sends one item to the desktop Trash. Returns at once: the move can
        /// be a real cross-filesystem copy, so it runs on a worker and the
        /// result arrives through the queue.
        #[qinvokable]
        fn trash_item(self: Pin<&mut FluoritaLibrary>, path: &QString);

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

    source_ids: QStringList,
    source_names: QStringList,
    source_paths: QStringList,
    selected_source: i32,
    choosing_folder: bool,
    folder_notice: QString,

    detail_open: bool,
    detail_name: QString,
    detail_path: QString,
    detail_kind: QString,
    detail_size: QString,
    detail_modified: QString,
    detail_duration: QString,
    detail_folder: QString,
    detail_notice: QString,
    item_notice: QString,

    gallery_paths: QStringList,
    gallery_names: QStringList,
    gallery_kinds: QStringList,
    gallery_thumbnails: QStringList,
    gallery_available: QStringList,

    music_paths: QStringList,
    music_titles: QStringList,
    music_artists: QStringList,
    music_albums: QStringList,
    music_available: QStringList,
    music_thumbnails: QStringList,

    worker: Option<JoinHandle<()>>,
    /// Cancels the scan and, above all, the watch loop that follows it. Without
    /// it a second scan would join a thread that only ever returns when this
    /// object dies, which is a deadlock on the GUI thread.
    cancellation: CancellationToken,
    /// The catalogue as last published, so an artwork pass and a change of
    /// selection both work from it without walking anything again.
    catalogue: Catalogue,
    /// The configuration as last published, so an add or a remove is applied to
    /// what the user is looking at.
    configured: SourceSet,
    /// The trash move in flight, if any. One at a time: two answers racing to
    /// change the same catalogue would publish whichever finished last.
    trash_worker: Option<JoinHandle<()>>,
    /// The folder chooser in flight, if any. One at a time: a second dialog
    /// would let two answers race to configure the same library.
    folder_worker: Option<JoinHandle<()>>,
    artwork_worker: Option<JoinHandle<()>>,
    artwork_cancellation: CancellationToken,
}

// Written out rather than derived: a default `QString` is empty, and an empty
// state is a state the interface has to guess at. Both of these are read by a
// binding before anything has run.
impl Default for LibraryRust {
    fn default() -> Self {
        Self {
            state: QString::from("empty"),
            summary: QString::default(),
            truncated: false,
            image_count: 0,
            video_count: 0,
            track_count: 0,
            artwork_pending: 0,
            artwork_state: QString::from("idle"),
            artwork_done: 0,
            artwork_total: 0,
            revision: 0,
            source_ids: QStringList::default(),
            source_names: QStringList::default(),
            source_paths: QStringList::default(),
            selected_source: EVERY_SOURCE,
            choosing_folder: false,
            folder_notice: QString::default(),
            detail_open: false,
            detail_name: QString::default(),
            detail_path: QString::default(),
            detail_kind: QString::default(),
            detail_size: QString::default(),
            detail_modified: QString::default(),
            detail_duration: QString::default(),
            detail_folder: QString::default(),
            detail_notice: QString::default(),
            item_notice: QString::default(),
            gallery_paths: QStringList::default(),
            gallery_names: QStringList::default(),
            gallery_kinds: QStringList::default(),
            gallery_thumbnails: QStringList::default(),
            gallery_available: QStringList::default(),
            music_paths: QStringList::default(),
            music_titles: QStringList::default(),
            music_artists: QStringList::default(),
            music_albums: QStringList::default(),
            music_available: QStringList::default(),
            music_thumbnails: QStringList::default(),
            worker: None,
            cancellation: CancellationToken::new(),
            catalogue: Catalogue::new(),
            configured: SourceSet::new(),
            folder_worker: None,
            trash_worker: None,
            artwork_worker: None,
            artwork_cancellation: CancellationToken::new(),
        }
    }
}

impl qobject::FluoritaLibrary {
    pub fn scan(self: core::pin::Pin<&mut Self>) {
        self.start_scan(None);
    }

    /// Starts a walk, optionally under a configuration the user just changed.
    ///
    /// `configured` is `None` on launch, when the stored configuration is what
    /// the worker should read; it is `Some` after an add or a remove, and the
    /// worker then stores that set before walking it.
    fn start_scan(mut self: core::pin::Pin<&mut Self>, configured: Option<SourceSet>) {
        self.as_mut().close();
        self.as_mut().set_state(QString::from("scanning"));
        self.as_mut().set_summary(QString::from(copy::SCANNING));

        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().cancellation = cancellation.clone();
        let scope = self.rust().scope();

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-library".to_owned())
            .spawn(move || {
                run_scan(&qt_thread, configured, scope, &cancellation);
            });

        match worker {
            Ok(handle) => self.as_mut().rust_mut().worker = Some(handle),
            Err(_) => {
                self.as_mut().set_state(QString::from("error"));
                self.as_mut()
                    .set_summary(QString::from(copy::SCAN_NOT_STARTED));
            }
        }
    }

    pub fn close(mut self: core::pin::Pin<&mut Self>) {
        // Cancel before joining. Dropping the worker's own `EngineWorker`
        // cancels a walk in progress, but the watch that follows it runs until
        // it is told to stop; joining first would wait forever.
        self.rust().cancellation.cancel();
        if let Some(handle) = self.as_mut().rust_mut().worker.take() {
            let _ = handle.join();
        }
    }

    pub fn select_source(mut self: core::pin::Pin<&mut Self>, source: i32) {
        if *self.selected_source() == source {
            return;
        }
        self.as_mut().set_selected_source(source);
        // Everything needed is already here: this is the same catalogue, read
        // through a different scope. Walking again to change folders would make
        // navigation cost what a scan costs.
        let catalogue = self.rust().catalogue.clone();
        let configured = self.rust().configured.clone();
        let scope = self.rust().scope();
        let truncated = *self.truncated();
        let state = if catalogue.is_empty() {
            "empty"
        } else {
            "ready"
        };
        let snapshot = project(&catalogue, &configured, scope, truncated, state);
        self.apply(snapshot);
    }

    pub fn add_folder(mut self: core::pin::Pin<&mut Self>) {
        if *self.choosing_folder() {
            return;
        }
        self.as_mut().set_choosing_folder(true);
        self.as_mut().set_folder_notice(QString::default());

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-folder".to_owned())
            .spawn(move || run_folder_choice(&qt_thread));
        match worker {
            Ok(handle) => self.as_mut().rust_mut().folder_worker = Some(handle),
            Err(_) => {
                self.as_mut().set_choosing_folder(false);
                self.as_mut()
                    .set_folder_notice(QString::from(copy::CHOOSER_UNAVAILABLE));
            }
        }
    }

    pub fn remove_folder(mut self: core::pin::Pin<&mut Self>, source: i32) {
        let Ok(value) = u32::try_from(source) else {
            return;
        };
        let handle = SourceId::from_value(value);
        let mut configured = self.rust().configured.clone();
        if !configured.remove(handle) {
            return;
        }
        // The records go with the root — the rescan applies that rule against
        // the whole configuration, so there is one answer to "does this record
        // still belong". Not one file is touched.
        // A selection that named the removed root would scope to nothing.
        if *self.selected_source() == source {
            self.as_mut().set_selected_source(EVERY_SOURCE);
        }
        self.start_scan(Some(configured));
    }

    pub fn describe_item(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        let wanted = std::path::PathBuf::from(path.to_string());
        let Some(record) = self.rust().catalogue.find_by_path(&wanted).cloned() else {
            // The row named a file the catalogue no longer holds — a scan just
            // forgot it, or the panel was opened on a stale grid. Saying so
            // beats opening a panel full of blanks.
            self.as_mut()
                .set_item_notice(QString::from(copy::ITEM_GONE));
            return;
        };
        let detail = detail::describe(&record, &self.rust().configured);
        self.as_mut().set_detail_name(QString::from(&detail.name));
        self.as_mut().set_detail_path(QString::from(&detail.path));
        self.as_mut().set_detail_kind(QString::from(&detail.kind));
        self.as_mut().set_detail_size(QString::from(&detail.size));
        self.as_mut()
            .set_detail_modified(QString::from(&detail.modified));
        self.as_mut()
            .set_detail_duration(QString::from(&detail.duration));
        self.as_mut()
            .set_detail_folder(QString::from(&detail.folder));
        self.as_mut()
            .set_detail_notice(QString::from(&detail.notice));
        self.as_mut().set_item_notice(QString::default());
        self.as_mut().set_detail_open(true);
    }

    pub fn close_detail(mut self: core::pin::Pin<&mut Self>) {
        self.as_mut().set_detail_open(false);
    }

    pub fn trash_item(mut self: core::pin::Pin<&mut Self>, path: &QString) {
        if self.rust().trash_worker.is_some() {
            return;
        }
        let wanted = std::path::PathBuf::from(path.to_string());
        if self.rust().catalogue.find_by_path(&wanted).is_none() {
            self.as_mut()
                .set_item_notice(QString::from(copy::ITEM_GONE));
            return;
        }
        self.as_mut().set_item_notice(QString::default());

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-trash".to_owned())
            .spawn(move || run_trash(&wanted, &qt_thread));
        match worker {
            Ok(handle) => self.as_mut().rust_mut().trash_worker = Some(handle),
            Err(_) => self
                .as_mut()
                .set_item_notice(QString::from(copy::TRASH_NOT_STARTED)),
        }
    }

    /// The trash move finished. Runs on the GUI thread, through the queue.
    ///
    /// The record goes only when the engine confirms the file actually moved.
    /// Dropping it on request would show the item gone while it was still on
    /// disk, which is exactly the "requested is not confirmed" mistake the
    /// suite's contract exists to prevent.
    fn item_trashed(mut self: core::pin::Pin<&mut Self>, path: QString, notice: QString) {
        if let Some(handle) = self.as_mut().rust_mut().trash_worker.take() {
            let _ = handle.join();
        }
        self.as_mut().set_item_notice(notice.clone());
        if !notice.to_string().is_empty() {
            return;
        }
        let moved = std::path::PathBuf::from(path.to_string());
        let id = self
            .rust()
            .catalogue
            .find_by_path(&moved)
            .map(|record| record.id().clone());
        if let Some(id) = id {
            self.as_mut().rust_mut().catalogue.forget(&id);
        }
        // The panel may be describing the very item that just left.
        if self.detail_path() == &path {
            self.as_mut().set_detail_open(false);
        }
        let catalogue = self.rust().catalogue.clone();
        let configured = self.rust().configured.clone();
        let scope = self.rust().scope();
        let refreshed = project(&catalogue, &configured, scope, *self.truncated(), "ready");
        self.apply(refreshed);
    }

    /// Starts the explicit artwork pass.
    pub fn generate_artwork(mut self: core::pin::Pin<&mut Self>) {
        if self.artwork_state() == &QString::from("generating") {
            return;
        }
        let catalogue = self.rust().catalogue.clone();
        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().artwork_cancellation = cancellation.clone();
        self.as_mut().set_artwork_state(QString::from("generating"));
        self.as_mut().set_artwork_done(0);

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-artwork".to_owned())
            .spawn(move || run_artwork(&catalogue, &cancellation, &qt_thread));
        match worker {
            Ok(handle) => self.as_mut().rust_mut().artwork_worker = Some(handle),
            Err(_) => self.as_mut().set_artwork_state(QString::from("idle")),
        }
    }

    pub fn cancel_artwork(mut self: core::pin::Pin<&mut Self>) {
        self.rust().artwork_cancellation.cancel();
        self.as_mut().set_artwork_state(QString::from("cancelling"));
    }

    /// The folder chooser answered. Runs on the GUI thread, through the queue.
    ///
    /// An empty path means the dialog was dismissed, which is not a failure and
    /// says nothing. A refused root — relative, nested inside a configured one,
    /// already mapped — is the domain's decision, and it is reported rather
    /// than swallowed, because the folder visibly did not appear.
    fn folder_chosen(mut self: core::pin::Pin<&mut Self>, path: QString, notice: QString) {
        self.as_mut().set_choosing_folder(false);
        if let Some(handle) = self.as_mut().rust_mut().folder_worker.take() {
            let _ = handle.join();
        }
        self.as_mut().set_folder_notice(notice);

        let chosen = path.to_string();
        if chosen.is_empty() {
            return;
        }
        let mut configured = self.rust().configured.clone();
        // Everything supported inside it: the user chose this folder for its
        // contents, and a kind filter they were never asked about would hide
        // files that are plainly there.
        match configured.add(
            std::path::PathBuf::from(&chosen),
            fluorita_core::KindSet::all(),
        ) {
            Ok(added) => {
                self.as_mut()
                    .set_selected_source(i32::try_from(added.value()).unwrap_or(EVERY_SOURCE));
                self.start_scan(Some(configured));
            }
            Err(rejection) => self.set_folder_notice(QString::from(copy::rejected(rejection))),
        }
    }

    /// One produced thumbnail, reported from the pass.
    fn artwork_progress(mut self: core::pin::Pin<&mut Self>, done: i32, total: i32) {
        self.as_mut().set_artwork_done(done);
        self.as_mut().set_artwork_total(total);
    }

    /// The pass finished: refresh the grid so the new thumbnails appear.
    fn artwork_finished(mut self: core::pin::Pin<&mut Self>, produced: i32) {
        self.as_mut().set_artwork_state(QString::from("idle"));
        if let Some(handle) = self.as_mut().rust_mut().artwork_worker.take() {
            let _ = handle.join();
        }
        if produced > 0 {
            let catalogue = self.rust().catalogue.clone();
            let configured = self.rust().configured.clone();
            let scope = self.rust().scope();
            let refreshed = project(&catalogue, &configured, scope, *self.truncated(), "ready");
            self.apply(refreshed);
        }
    }

    /// Publishes a finished projection. Runs on the GUI thread.
    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: LibrarySnapshot) {
        self.as_mut().set_summary(QString::from(&snapshot.summary));
        self.as_mut().set_truncated(snapshot.truncated);
        self.as_mut().set_image_count(snapshot.image_count);
        self.as_mut().set_video_count(snapshot.video_count);
        self.as_mut().set_track_count(snapshot.track_count);

        let sources = columns(&snapshot.sources);
        self.as_mut().set_source_ids(sources[0].clone());
        self.as_mut().set_source_names(sources[1].clone());
        self.as_mut().set_source_paths(sources[2].clone());

        let gallery = columns(&snapshot.gallery);
        self.as_mut().set_gallery_paths(gallery[0].clone());
        self.as_mut().set_gallery_names(gallery[1].clone());
        self.as_mut().set_gallery_kinds(gallery[2].clone());
        self.as_mut().set_gallery_thumbnails(gallery[3].clone());
        self.as_mut().set_gallery_available(gallery[4].clone());

        let music = columns(&snapshot.music);
        self.as_mut().set_music_paths(music[0].clone());
        self.as_mut().set_music_titles(music[1].clone());
        self.as_mut().set_music_artists(music[2].clone());
        self.as_mut().set_music_albums(music[3].clone());
        self.as_mut().set_music_available(music[4].clone());
        self.as_mut().set_music_thumbnails(music[5].clone());

        self.as_mut().set_artwork_pending(snapshot.artwork_pending);
        self.as_mut().rust_mut().catalogue = snapshot.catalogue;
        self.as_mut().rust_mut().configured = snapshot.configured;
        // A selection whose root is gone would scope the content to nothing
        // while the sidebar shows no row selected.
        let selected = *self.selected_source();
        if selected != EVERY_SOURCE
            && u32::try_from(selected)
                .ok()
                .and_then(|value| self.rust().configured.get(SourceId::from_value(value)))
                .is_none()
        {
            self.as_mut().set_selected_source(EVERY_SOURCE);
        }
        self.as_mut().set_state(QString::from(snapshot.state));
        // Last, and only once: this is what QML watches.
        let next = self.revision().wrapping_add(1);
        self.as_mut().set_revision(next);
    }
}

impl LibraryRust {
    /// The scope the content is projected under. `EVERY_SOURCE` is negative,
    /// so the conversion failing is exactly the "everything" case.
    fn scope(&self) -> SourceScope {
        u32::try_from(self.selected_source)
            .ok()
            .map_or(SourceScope::All, |value| {
                SourceScope::One(SourceId::from_value(value))
            })
    }
}

impl Drop for LibraryRust {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        // A folder chooser can outlive the window: the person may still have
        // the dialog open. Joining it is the only way its thread cannot report
        // into an object that is going away.
        if let Some(handle) = self.folder_worker.take() {
            let _ = handle.join();
        }
        // A cross-filesystem trash move is a real copy. Joining it is what
        // stops a half-moved file from being left behind by a closing window.
        if let Some(handle) = self.trash_worker.take() {
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

/// Turns row-major records into the index-aligned lists QML binds to.
fn columns<const N: usize>(rows: &[[String; N]]) -> [QStringList; N] {
    let mut lists = std::array::from_fn(|_| QStringList::default());
    for row in rows {
        for (column, value) in row.iter().enumerate() {
            lists[column].append(QString::from(value));
        }
    }
    lists
}
