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
//! - **A path crossing to QML is a key, not text.** Rows publish
//!   `celestina_core::percent`-encoded path bytes and every verb decodes them
//!   back; the name a person reads travels in its own column and never returns.
//!   That is [ADR 0008](../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md),
//!   and it is why a file whose name is not UTF-8 can now be described or
//!   trashed instead of reporting that it is no longer in the library.
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

use std::sync::Arc;
use std::thread::JoinHandle;

use celestina_core::CancellationToken;

use celestina_core::pathkey;

mod copy;
mod detail;
mod project;
mod work;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use project::{census, project_matching, LibrarySnapshot};
use work::{run_folder_choice, run_posters, run_scan, run_trash, Tried};

use fluorita_core::{Catalogue, Query, SourceId, SourceScope, SourceSet};

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
        /// Bumped once, after every list of a publication is in place. QML
        /// rebuilds its rows from this and never from the lists directly:
        /// publishing several lists one by one makes the bindings re-run
        /// between them, with half the columns still holding the previous
        /// publication.
        #[qproperty(i32, revision)]
        /// What is being searched for, as it was typed. Published so the box
        /// keeps its text across a re-projection, and so a header can say the
        /// library is showing a subset.
        #[qproperty(QString, query)]
        /// The sidebar, index-aligned: the root's handle as text, the label to
        /// show and where it is, as display text. Configuration order, which is
        /// the order the user built.
        #[qproperty(QStringList, source_ids)]
        /// How many items the query hid. Zero when nothing is being searched
        /// for; a surface uses it to say "nothing matches" rather than showing
        /// an empty grid that looks like a broken library.
        #[qproperty(i32, hidden_by_query)]
        #[qproperty(QStringList, source_names)]
        #[qproperty(QStringList, source_locations)]
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
        /// Where the item is, for a person to read. Lossy, like every label,
        /// and never given back to a verb — the menu acts on the row's key.
        #[qproperty(QString, detail_location)]
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
        /// Gallery rows, index-aligned: the item's path key, its display name,
        /// its kind and the cached thumbnail URL (empty when nothing produced
        /// one).
        ///
        /// The key is opaque ASCII under
        /// [ADR 0008](../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md):
        /// it is the only value `describe_item`, `trash_item` and the player's
        /// `open` accept, and it is not text to show. The name beside it is.
        #[qproperty(QStringList, gallery_keys)]
        #[qproperty(QStringList, gallery_names)]
        #[qproperty(QStringList, gallery_kinds)]
        #[qproperty(QStringList, gallery_thumbnails)]
        /// `1` while the file is where the catalogue last saw it, `0` once a
        /// scan or the watch found it gone. A missing item stays in the grid —
        /// a disconnected drive is not data loss — but it must say so.
        #[qproperty(QStringList, gallery_available)]
        /// Music rows, index-aligned and already in projection order, so a
        /// `ListView` can section on the artist without sorting anything. The
        /// first column is a path key, on the same terms as the gallery's.
        #[qproperty(QStringList, music_keys)]
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
        ///
        /// Takes the row's path key. A value that is not one is refused rather
        /// than turned into some other file's path.
        #[qinvokable]
        fn describe_item(self: Pin<&mut FluoritaLibrary>, key: &QString);

        /// Closes the properties panel.
        #[qinvokable]
        fn close_detail(self: Pin<&mut FluoritaLibrary>);

        /// Sends one item to the desktop Trash. Returns at once: the move can
        /// be a real cross-filesystem copy, so it runs on a worker and the
        /// result arrives through the queue. Takes the row's path key.
        #[qinvokable]
        fn trash_item(self: Pin<&mut FluoritaLibrary>, key: &QString);

        /// Shows only what matches, or everything again when the text is
        /// empty.
        #[qinvokable]
        fn search(self: Pin<&mut FluoritaLibrary>, text: &QString);
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
    revision: i32,
    query: QString,
    hidden_by_query: i32,

    source_ids: QStringList,
    source_names: QStringList,
    source_locations: QStringList,
    selected_source: i32,
    choosing_folder: bool,
    folder_notice: QString,

    detail_open: bool,
    detail_name: QString,
    detail_location: QString,
    detail_kind: QString,
    detail_size: QString,
    detail_modified: QString,
    detail_duration: QString,
    detail_folder: QString,
    detail_notice: QString,
    item_notice: QString,
    /// Which item the properties panel is about, byte-exact. Not published:
    /// `detail_location` is the lossy label a person reads, and comparing that
    /// would confuse two files whose names differ only in bytes no font shows.
    described: Option<std::path::PathBuf>,

    gallery_keys: QStringList,
    gallery_names: QStringList,
    gallery_kinds: QStringList,
    gallery_thumbnails: QStringList,
    gallery_available: QStringList,

    music_keys: QStringList,
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
    /// The catalogue as last published, so the poster pass and a change of
    /// selection both work from it without walking anything again. Shared
    /// rather than owned: the projection worker and the poster pass each take
    /// a handle, and cloning fifty thousand records to hand one out was part
    /// of the per-keystroke cost recorded as FLU-P1.
    catalogue: Arc<Catalogue>,
    /// The configuration as last published, so an add or a remove is applied to
    /// what the user is looking at.
    configured: SourceSet,
    /// The trash move in flight, if any. One at a time: two answers racing to
    /// change the same catalogue would publish whichever finished last.
    trash_worker: Option<JoinHandle<()>>,
    /// The projection in flight, if any. One at a time: a burst of keystrokes
    /// or clicks projects at most twice — the one running, then one over
    /// whatever is current when it lands — never once per event.
    projection: Option<JoinHandle<()>>,
    /// Bumped by everything a projection must reflect: a selection, a query,
    /// a catalogue. A result that carries an older ticket describes a library
    /// nobody is looking at any more, and is projected again instead.
    projection_ticket: u64,
    /// The folder chooser in flight, if any. One at a time: a second dialog
    /// would let two answers race to configure the same library.
    folder_worker: Option<JoinHandle<()>>,
    /// Withdraws the chooser's request when this object goes away, so the
    /// join in `Drop` waits for one receive slice rather than for a person
    /// who may have left the dialog open.
    folder_cancellation: CancellationToken,
    /// The background poster pass, if one is running. One at a time: it is
    /// the only work here that drives the media backend, one job at a time.
    artwork_worker: Option<JoinHandle<()>>,
    /// Stops the poster pass: a rescan replaces the catalogue it works from,
    /// and a closing window must not wait for the file being rendered.
    artwork_cancellation: CancellationToken,
    /// A pass was asked for while one was running; it starts again, over the
    /// catalogue as it is then, when the running one finishes.
    artwork_again: bool,
    /// What the poster pass has already asked the backend about this session,
    /// so a file that gives nothing is not asked again after every change.
    artwork_tried: Arc<Tried>,
}

// Written out rather than derived: a default `QString` is empty, and an empty
// state is a state the interface has to guess at. Both of these are read by a
// binding before anything has run.
impl Default for LibraryRust {
    fn default() -> Self {
        Self {
            state: QString::from("empty"),
            summary: QString::default(),
            query: QString::default(),
            hidden_by_query: 0,
            truncated: false,
            image_count: 0,
            video_count: 0,
            track_count: 0,
            revision: 0,
            source_ids: QStringList::default(),
            source_names: QStringList::default(),
            source_locations: QStringList::default(),
            selected_source: EVERY_SOURCE,
            choosing_folder: false,
            folder_notice: QString::default(),
            detail_open: false,
            detail_name: QString::default(),
            detail_location: QString::default(),
            detail_kind: QString::default(),
            detail_size: QString::default(),
            detail_modified: QString::default(),
            detail_duration: QString::default(),
            detail_folder: QString::default(),
            detail_notice: QString::default(),
            item_notice: QString::default(),
            described: None,
            gallery_keys: QStringList::default(),
            gallery_names: QStringList::default(),
            gallery_kinds: QStringList::default(),
            gallery_thumbnails: QStringList::default(),
            gallery_available: QStringList::default(),
            music_keys: QStringList::default(),
            music_titles: QStringList::default(),
            music_artists: QStringList::default(),
            music_albums: QStringList::default(),
            music_available: QStringList::default(),
            music_thumbnails: QStringList::default(),
            worker: None,
            cancellation: CancellationToken::new(),
            catalogue: Arc::new(Catalogue::new()),
            configured: SourceSet::new(),
            projection: None,
            projection_ticket: 0,
            folder_worker: None,
            folder_cancellation: CancellationToken::new(),
            trash_worker: None,
            artwork_worker: None,
            artwork_cancellation: CancellationToken::new(),
            artwork_again: false,
            artwork_tried: Arc::default(),
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
        // The poster pass works from the catalogue this walk replaces. It is
        // told to stop and not joined here — the backend may be in the middle
        // of a file — and the walk's own publication starts the next one.
        self.as_mut().cancel_posters();
        self.as_mut().set_state(QString::from("scanning"));
        self.as_mut().set_summary(QString::from(copy::SCANNING));
        // A projection still running describes the library as it was before
        // this walk — under the previous configuration and as "ready" — and
        // publishing it would overwrite "scanning" and drop a selection that
        // names the folder just added. Moving the ticket makes it stale; the
        // one that replaces it projects from what this object holds now.
        let ticket = self.rust().projection_ticket.wrapping_add(1);
        self.as_mut().rust_mut().projection_ticket = ticket;

        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().cancellation = cancellation.clone();
        let scope = self.rust().scope();

        let qt_thread = self.qt_thread();
        let walked = configured.clone();
        let worker = std::thread::Builder::new()
            .name("fluorita-library".to_owned())
            .spawn(move || {
                run_scan(&qt_thread, walked, scope, &cancellation);
            });

        match worker {
            Ok(handle) => {
                self.as_mut().rust_mut().worker = Some(handle);
                // The worker stores this set before it walks anything, so it
                // is already the configuration; a projection made meanwhile —
                // a query typed while the walk runs — keeps the selection
                // that names the new folder instead of resetting it.
                if let Some(configured) = configured {
                    self.as_mut().rust_mut().configured = configured;
                }
            }
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
        // navigation cost what a scan costs — and projecting here would make it
        // cost a `stat()` per row on the GUI thread.
        self.request_projection();
    }

    pub fn add_folder(mut self: core::pin::Pin<&mut Self>) {
        if *self.choosing_folder() {
            return;
        }
        self.as_mut().set_choosing_folder(true);
        self.as_mut().set_folder_notice(QString::default());

        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().folder_cancellation = cancellation.clone();
        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-folder".to_owned())
            .spawn(move || run_folder_choice(&qt_thread, &cancellation));
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

    pub fn describe_item(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        // A key that did not come from a published row is refused the same way
        // a row the catalogue has forgotten is: there is no item to describe,
        // and inventing a `PathBuf` from the characters would open a panel
        // about a different file.
        let Ok(wanted) = pathkey::decode(&key.to_string()) else {
            self.as_mut()
                .set_item_notice(QString::from(copy::ITEM_GONE));
            return;
        };
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
        self.as_mut()
            .set_detail_location(QString::from(&detail.location));
        // Which item the panel is about, byte-exact, so a trash that removes it
        // can close it. `detail_location` is display text and two different
        // files can spell the same one.
        self.as_mut().rust_mut().described = Some(wanted);
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

    pub fn trash_item(mut self: core::pin::Pin<&mut Self>, key: &QString) {
        if self.rust().trash_worker.is_some() {
            return;
        }
        // Refused before anything is moved. A key this process did not emit
        // names no item here, and guessing a path for it would send a file the
        // user never pointed at to the Trash.
        let Ok(wanted) = pathkey::decode(&key.to_string()) else {
            self.as_mut()
                .set_item_notice(QString::from(copy::ITEM_GONE));
            return;
        };
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
    ///
    /// `key` is the same key the worker was started from, carried back through
    /// the queue: the answer crosses as a `QString`, so it travels as a key for
    /// the same reason a published row does.
    fn item_trashed(mut self: core::pin::Pin<&mut Self>, key: QString, notice: QString) {
        if let Some(handle) = self.as_mut().rust_mut().trash_worker.take() {
            let _ = handle.join();
        }
        self.as_mut().set_item_notice(notice.clone());
        // Any notice means the file is still where it was — a failure, or a
        // move that left it in place — so the record stays with it.
        if !notice.to_string().is_empty() {
            return;
        }
        let Ok(moved) = pathkey::decode(&key.to_string()) else {
            return;
        };
        let id = self
            .rust()
            .catalogue
            .find_by_path(&moved)
            .map(|record| record.id().clone());
        if let Some(id) = id {
            // `make_mut` clones only while a worker still holds the previous
            // snapshot — the ordinary case mutates in place as before.
            Catalogue::forget(Arc::make_mut(&mut self.as_mut().rust_mut().catalogue), &id);
        }
        // The panel may be describing the very item that just left. Compared by
        // bytes, not by the label the panel is showing.
        if self.rust().described.as_deref() == Some(moved.as_path()) {
            self.as_mut().set_detail_open(false);
        }
        self.request_projection();
    }

    /// Shows only what matches `text`, or everything again when it is empty.
    ///
    /// Re-projects from the catalogue already in memory: no scan, no disk
    /// beyond the thumbnails the projection resolves, and those on a worker
    /// (FLU-P1, docs/evidence/2026-09-02-apps-performance-audit.md). The
    /// surface debounces the keystrokes; this does the filtering, because what
    /// matches is a rule the domain owns.
    pub fn search(mut self: core::pin::Pin<&mut Self>, text: &QString) {
        self.as_mut().set_query(text.clone());
        self.request_projection();
    }

    /// Asks for the content to be projected again from what this object holds
    /// now. Returns at once; the rows arrive through the queue.
    ///
    /// Every re-projection comes through here — a selection, a query, a trash,
    /// new artwork, a change the watch folded in — because every one of them
    /// resolves a thumbnail per row and counts pending artwork, which is a
    /// `stat()` per item and a frozen window on a large or remote library.
    fn request_projection(mut self: core::pin::Pin<&mut Self>) {
        let ticket = self.rust().projection_ticket.wrapping_add(1);
        self.as_mut().rust_mut().projection_ticket = ticket;
        // One in flight at a time. The one running finds its ticket stale when
        // it lands and starts again from whatever is current then, so the
        // newest request always wins and none waits behind a queue.
        if self.rust().projection.is_none() {
            self.start_projection();
        }
    }

    fn start_projection(mut self: core::pin::Pin<&mut Self>) {
        let ticket = self.rust().projection_ticket;
        let catalogue = Arc::clone(&self.rust().catalogue);
        let configured = self.rust().configured.clone();
        let scope = self.rust().scope();
        let truncated = *self.truncated();
        let state = self.rust().state_token();
        let text = self.query().to_string();
        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-projection".to_owned())
            .spawn(move || {
                let query = Query::new(&text);
                let matching =
                    project_matching(&catalogue, &configured, scope, truncated, state, &query);
                let total = census(&catalogue, scope);
                let _ = qt_thread.queue(move |library| {
                    library.finish_projection(ticket, matching, total);
                });
            });
        match worker {
            Ok(handle) => self.as_mut().rust_mut().projection = Some(handle),
            // The rows already on screen stay; only the new projection did not
            // run. The stored query and selection still say what was asked.
            Err(error) => eprintln!("fluorita: could not start a projection: {error}"),
        }
    }

    /// Publishes a finished projection on the Qt thread — unless something it
    /// had to reflect changed while it ran, in which case the current state is
    /// projected again instead.
    fn finish_projection(
        mut self: core::pin::Pin<&mut Self>,
        ticket: u64,
        matching: LibrarySnapshot,
        total: usize,
    ) {
        if let Some(handle) = self.as_mut().rust_mut().projection.take() {
            let _ = handle.join();
        }
        if ticket != self.rust().projection_ticket {
            // While a walk runs, its own publication is what comes next, and
            // it asks for a projection itself if a query or a selection needs
            // one; projecting the old catalogue again meanwhile would only
            // replace "scanning" with rows that are about to change.
            if self.rust().state_token() != "scanning" {
                self.start_projection();
            }
            return;
        }
        let shown = matching.gallery.len() + matching.music.len();
        self.as_mut()
            .set_hidden_by_query(i32::try_from(total.saturating_sub(shown)).unwrap_or(0));
        self.publish(matching);
    }

    /// The watch folded changes into the catalogue. Runs on the GUI thread,
    /// through the queue; the copy it carries was made on the watch thread.
    fn catalogue_changed(
        mut self: core::pin::Pin<&mut Self>,
        catalogue: Arc<Catalogue>,
        configured: SourceSet,
    ) {
        self.as_mut().rust_mut().catalogue = catalogue;
        self.as_mut().rust_mut().configured = configured;
        self.as_mut().request_projection();
        // New items may have arrived, and a video or a track among them gets
        // its poster the same way the scanned ones did.
        self.request_posters();
    }

    /// Asks for the background poster pass over the catalogue held now.
    ///
    /// Not a request the person makes: the scan's settled publication and the
    /// watch's changes call this (the author's decision of 2026-10-07). If a
    /// pass is already running, it runs again once that one finishes, so
    /// items that arrived meanwhile are not left out.
    fn request_posters(mut self: core::pin::Pin<&mut Self>) {
        if self.rust().artwork_worker.is_some() {
            self.as_mut().rust_mut().artwork_again = true;
            return;
        }
        self.as_mut().rust_mut().artwork_again = false;
        let catalogue = Arc::clone(&self.rust().catalogue);
        let tried = Arc::clone(&self.rust().artwork_tried);
        let cancellation = CancellationToken::new();
        self.as_mut().rust_mut().artwork_cancellation = cancellation.clone();

        let qt_thread = self.qt_thread();
        let worker = std::thread::Builder::new()
            .name("fluorita-artwork".to_owned())
            .spawn(move || run_posters(&catalogue, &tried, &cancellation, &qt_thread));
        match worker {
            Ok(handle) => self.as_mut().rust_mut().artwork_worker = Some(handle),
            // Posters are a nicety: the grid keeps its glyphs, and the next
            // scan or change asks again.
            Err(error) => eprintln!("fluorita: could not start the poster pass: {error}"),
        }
    }

    /// Stops the running poster pass at its next look at the token, without
    /// waiting for it. Its `posters_finished` still arrives and joins it.
    fn cancel_posters(mut self: core::pin::Pin<&mut Self>) {
        self.rust().artwork_cancellation.cancel();
        self.as_mut().rust_mut().artwork_again = false;
    }

    /// The folder chooser answered. Runs on the GUI thread, through the queue.
    ///
    /// An empty key means the dialog was dismissed, which is not a failure and
    /// says nothing. A refused root — relative, nested inside a configured one,
    /// already mapped — is the domain's decision, and it is reported rather
    /// than swallowed, because the folder visibly did not appear.
    ///
    /// The chosen folder crosses back as a key. The portal already hands over
    /// raw bytes, and a directory whose name is not UTF-8 was being mapped
    /// under its lossy spelling — a root the scan would then find nothing in.
    fn folder_chosen(mut self: core::pin::Pin<&mut Self>, key: QString, notice: QString) {
        self.as_mut().set_choosing_folder(false);
        if let Some(handle) = self.as_mut().rust_mut().folder_worker.take() {
            let _ = handle.join();
        }
        self.as_mut().set_folder_notice(notice);

        let Ok(chosen) = pathkey::decode(&key.to_string()) else {
            // Includes the dismissed dialog, which sends an empty key and has
            // already had its say above: there is nothing to add and nothing
            // more to report.
            return;
        };
        let mut configured = self.rust().configured.clone();
        // Everything supported inside it: the user chose this folder for its
        // contents, and a kind filter they were never asked about would hide
        // files that are plainly there.
        match configured.add(chosen, fluorita_core::KindSet::all()) {
            Ok(added) => {
                self.as_mut()
                    .set_selected_source(i32::try_from(added.value()).unwrap_or(EVERY_SOURCE));
                self.start_scan(Some(configured));
            }
            Err(rejection) => self.set_folder_notice(QString::from(copy::rejected(rejection))),
        }
    }

    /// A poster pass put new entries in the cache: project again so they
    /// appear. Runs on the GUI thread, through the queue.
    fn posters_produced(self: core::pin::Pin<&mut Self>) {
        self.request_projection();
    }

    /// The poster pass ended, finished or cancelled. Its thread has nothing
    /// left to do, so joining it here does not block.
    fn posters_finished(mut self: core::pin::Pin<&mut Self>) {
        if let Some(handle) = self.as_mut().rust_mut().artwork_worker.take() {
            let _ = handle.join();
        }
        if self.rust().artwork_again {
            self.request_posters();
        }
    }

    /// Publishes what the scan worker projected. Runs on the GUI thread.
    ///
    /// The scan projected unfiltered, under the scope selected when it began.
    /// If the person has typed a query or picked another folder since, or a
    /// projection over the previous catalogue is still running, the new
    /// catalogue is projected again under what is current now.
    fn apply(mut self: core::pin::Pin<&mut Self>, snapshot: LibrarySnapshot) {
        let reproject = !self.query().to_string().is_empty()
            || snapshot.scope != self.rust().scope()
            || self.rust().projection.is_some();
        // The walk has settled: what it found is the catalogue the posters
        // are made for. A stored or failed publication is not that moment.
        let settled = snapshot.state == "ready";
        self.as_mut().set_hidden_by_query(0);
        self.as_mut().publish(snapshot);
        if reproject {
            self.as_mut().request_projection();
        }
        if settled {
            self.request_posters();
        }
    }

    /// Puts one finished projection on screen. Runs on the GUI thread and
    /// touches no file.
    fn publish(mut self: core::pin::Pin<&mut Self>, snapshot: LibrarySnapshot) {
        self.as_mut().set_summary(QString::from(&snapshot.summary));
        self.as_mut().set_truncated(snapshot.truncated);
        self.as_mut().set_image_count(snapshot.image_count);
        self.as_mut().set_video_count(snapshot.video_count);
        self.as_mut().set_track_count(snapshot.track_count);

        let sources = columns(&snapshot.sources);
        self.as_mut().set_source_ids(sources[0].clone());
        self.as_mut().set_source_names(sources[1].clone());
        self.as_mut().set_source_locations(sources[2].clone());

        let gallery = columns(&snapshot.gallery);
        self.as_mut().set_gallery_keys(gallery[0].clone());
        self.as_mut().set_gallery_names(gallery[1].clone());
        self.as_mut().set_gallery_kinds(gallery[2].clone());
        self.as_mut().set_gallery_thumbnails(gallery[3].clone());
        self.as_mut().set_gallery_available(gallery[4].clone());

        let music = columns(&snapshot.music);
        self.as_mut().set_music_keys(music[0].clone());
        self.as_mut().set_music_titles(music[1].clone());
        self.as_mut().set_music_artists(music[2].clone());
        self.as_mut().set_music_albums(music[3].clone());
        self.as_mut().set_music_available(music[4].clone());
        self.as_mut().set_music_thumbnails(music[5].clone());

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
    /// The published state as the token a projection carries, so re-projecting
    /// while a scan is still running does not announce the library as ready.
    fn state_token(&self) -> &'static str {
        match self.state.to_string().as_str() {
            "scanning" => "scanning",
            "stored" => "stored",
            "error" => "error",
            "empty" => "empty",
            _ => "ready",
        }
    }

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
        // the dialog open. Cancelling withdraws the request from the desktop,
        // and the worker returns within one receive slice; joining it is what
        // stops its thread from reporting into an object that is going away.
        self.folder_cancellation.cancel();
        if let Some(handle) = self.folder_worker.take() {
            let _ = handle.join();
        }
        // A projection is bounded work with no token; it finishes and its
        // answer finds no one to deliver to.
        if let Some(handle) = self.projection.take() {
            let _ = handle.join();
        }
        // A cross-filesystem trash move is a real copy. Joining it is what
        // stops a half-moved file from being left behind by a closing window.
        if let Some(handle) = self.trash_worker.take() {
            let _ = handle.join();
        }
        // Cancel before joining: the poster pass would otherwise hold the
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

/// The handle of the configured source whose root is exactly `folder`, read
/// from the stored configuration (the first-run seed when none is stored).
/// Blocking: reads a file, so it runs off the Qt thread — another launch's
/// `Open` asks it on the activation worker.
pub(crate) fn configured_source(folder: &std::path::Path) -> Option<i32> {
    let sources = match fluorita_engine::source_store::default_path() {
        Some(path) => {
            fluorita_engine::source_store::load(&path, &work::media_directories()).sources
        }
        None => SourceSet::seeded_from(&work::media_directories()),
    };
    sources
        .sources()
        .iter()
        .find(|source| source.root() == folder)
        .and_then(|source| i32::try_from(source.id().value()).ok())
}
