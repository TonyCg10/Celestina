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

use std::thread::JoinHandle;

use celestina_core::CancellationToken;

mod project;
mod work;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use project::{project, LibrarySnapshot};
use work::{run_artwork, run_scan};

use fluorita_core::Catalogue;

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
    gallery_available: QStringList,

    music_paths: QStringList,
    music_titles: QStringList,
    music_artists: QStringList,
    music_albums: QStringList,
    music_available: QStringList,

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
            gallery_available: QStringList::default(),
            music_paths: QStringList::default(),
            music_titles: QStringList::default(),
            music_artists: QStringList::default(),
            music_albums: QStringList::default(),
            music_available: QStringList::default(),
            worker: None,
            catalogue: Catalogue::new(),
            artwork_worker: None,
            artwork_cancellation: CancellationToken::new(),
        }
    }
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
        self.as_mut().set_gallery_available(gallery[4].clone());

        let music = columns(&snapshot.music);
        self.as_mut().set_music_paths(music[0].clone());
        self.as_mut().set_music_titles(music[1].clone());
        self.as_mut().set_music_artists(music[2].clone());
        self.as_mut().set_music_albums(music[3].clone());
        self.as_mut().set_music_available(music[4].clone());

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

/// Turns row-major records into the index-aligned lists QML binds to.
fn columns(rows: &[[String; 5]]) -> [QStringList; 5] {
    let mut lists = [
        QStringList::default(),
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
