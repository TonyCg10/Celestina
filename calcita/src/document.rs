//! One window's reading state: which document, which page of how many, and
//! the zoom. QtPdf renders and reports; this object decides what the page
//! field and the zoom controls mean (`calcita_core`'s grammar and ladder).
//! It also reads the document's annotation model and form on a worker
//! (`calcita_core::annot`), and names the version of the file the window
//! shows (`revision`, part of each page's address): a new one when the
//! document is set, when the window finds the file changed on disk
//! (`checkFile`, answered by the same worker) and after a save that
//! replaced it (`reload`), so no page the image cache holds from another
//! version is shown again.
//!
//! A document with a form is shown from a copy with its widgets stamped
//! into the pages (QtPdf draws no widget): the worker registers that copy
//! with the page provider under a revision of its own, and the window
//! moves to that revision when the read is published (until then its
//! pages show the file, without the widgets). The copy is dropped when the
//! window moves to another revision or goes.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex, PoisonError};

use calcita_core::annot::{form, Annotation};
use calcita_core::pagelabel::GoTo;
use calcita_core::search::{self, Direction, SearchRequest};
use calcita_core::zoom::{self, ZoomMode};
use celestina_core::{file_uri, pathkey};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};

use crate::controller::display_name;
use crate::editor::json_fields;
use crate::pages;
use crate::saving::Stamp;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, document_key, cxx_name = "documentKey", READ, WRITE = set_document_key, NOTIFY)]
        #[qproperty(QString, document_name, cxx_name = "documentName", READ, NOTIFY)]
        #[qproperty(QUrl, document_url, cxx_name = "documentUrl", READ, NOTIFY)]
        #[qproperty(i32, page, READ, NOTIFY)]
        #[qproperty(i32, page_count, cxx_name = "pageCount", READ, NOTIFY)]
        #[qproperty(QString, zoom_mode, cxx_name = "zoomMode", READ, NOTIFY)]
        #[qproperty(f64, zoom_factor, cxx_name = "zoomFactor", READ, NOTIFY)]
        #[qproperty(bool, loaded, READ, NOTIFY)]
        #[qproperty(i32, annotation_count, cxx_name = "annotationCount", READ, NOTIFY)]
        #[qproperty(i32, revision, READ, NOTIFY)]
        #[qproperty(QString, fields, READ, NOTIFY)]
        type CalcitaDocument = super::CalcitaDocumentRust;

        /// Sets the document this window shows (a pathkey).
        fn set_document_key(self: Pin<&mut CalcitaDocument>, key: QString);

        /// The view should scroll to `page` (1-based).
        #[qsignal]
        fn page_requested(self: Pin<&mut CalcitaDocument>, page: i32);

        /// Something to tell the person: `kind` is `error` or `info`.
        #[qsignal]
        fn notice(self: Pin<&mut CalcitaDocument>, kind: QString, text: QString);

        /// The file is a new version (changed on disk, or saved over): the
        /// window loads it again. `revision` is already the new one.
        #[qsignal]
        fn reload_requested(self: Pin<&mut CalcitaDocument>);

        /// Looks on the worker whether the file changed on disk since it was
        /// last read; when it did, the model is read again and
        /// `reloadRequested` follows.
        #[qinvokable]
        fn check_file(self: Pin<&mut CalcitaDocument>);

        /// The file was replaced by a save: a new revision, the model read
        /// again, and `reloadRequested`.
        #[qinvokable]
        fn reload(self: Pin<&mut CalcitaDocument>);

        /// Goes to the page `text` names (`12`, `+3`, `-2`, `inicio`, `fin`).
        #[qinvokable]
        #[cxx_name = "goTo"]
        fn go_to(self: Pin<&mut CalcitaDocument>, text: &QString) -> bool;

        #[qinvokable]
        fn zoom_in(self: Pin<&mut CalcitaDocument>);

        #[qinvokable]
        fn zoom_out(self: Pin<&mut CalcitaDocument>);

        #[qinvokable]
        fn fit_width(self: Pin<&mut CalcitaDocument>);

        #[qinvokable]
        fn fit_page(self: Pin<&mut CalcitaDocument>);

        /// A free zoom of `factor`, held inside the ladder's ends.
        #[qinvokable]
        fn set_zoom(self: Pin<&mut CalcitaDocument>, factor: f64);

        /// Restores a remembered zoom word; an unknown word keeps the zoom.
        #[qinvokable]
        fn restore_zoom(self: Pin<&mut CalcitaDocument>, word: &QString);

        /// The zoom word to remember (`width`, `page`, `free:<f>`).
        #[qinvokable]
        fn zoom_word(self: &CalcitaDocument) -> QString;

        /// QtPdf finished loading: `count` pages, or `-1` when it failed.
        #[qinvokable]
        fn report_loaded(self: Pin<&mut CalcitaDocument>, count: i32);

        /// The view now shows `page` (1-based).
        #[qinvokable]
        fn report_page(self: Pin<&mut CalcitaDocument>, page: i32);

        /// The view's scale after fitting, so zooming continues from it.
        #[qinvokable]
        fn report_scale(self: Pin<&mut CalcitaDocument>, factor: f64);

        /// What QtPdf searches for when the search field reads `text`: the
        /// text trimmed, or empty for no search.
        #[qinvokable]
        fn search_query(self: &CalcitaDocument, text: &QString) -> QString;

        /// The hit (0-based) a step from `current` (-1 for none) lands on
        /// among `count` hits, going round the ends; -1 when there are none.
        #[qinvokable]
        fn next_hit(self: &CalcitaDocument, current: i32, count: i32, forward: bool) -> i32;

        /// How many annotations the 0-based `page` carries, as last read;
        /// 0 until the model is read or when it could not be.
        #[qinvokable]
        fn annotations_on_page(self: &CalcitaDocument, page: i32) -> i32;
    }

    impl cxx_qt::Threading for CalcitaDocument {}
}

pub struct CalcitaDocumentRust {
    document_key: QString,
    document_name: QString,
    document_url: QUrl,
    page: i32,
    page_count: i32,
    zoom_mode: QString,
    zoom_factor: f64,
    loaded: bool,
    /// How many annotations the document carries; -1 until the model is
    /// read, and when it cannot be (encrypted, unparseable, too large).
    annotation_count: i32,
    /// The version of the file the pages show, in their address.
    revision: i32,
    /// The form's widgets as last read, one JSON array (`[]` without a
    /// form), for the window to place what a tap on a field does.
    fields: QString,
    /// The stamped copy registered for this window (its key and
    /// revision), while the pages show it.
    stamped: Option<(String, i32)>,
    /// The stamped copies the reader registered whose read has not reached
    /// this object yet: a window destroyed in between drops them.
    in_flight: InFlight,
    zoom: ZoomMode,
    /// The annotation model last read for this document.
    annotations: Vec<Annotation>,
    /// Bumped for every document, so a read of an earlier one is dropped.
    model_generation: u64,
    /// This window's model reader; it ends when the window's object goes.
    model_reader: Option<mpsc::Sender<ModelRequest>>,
}

/// One read the model reader is asked for.
struct ModelRequest {
    generation: u64,
    /// The document's path key, which names a stamped copy to the provider.
    key: String,
    path: PathBuf,
    /// Only when the file changed since the reader last read it.
    only_if_changed: bool,
}

/// What the model reader hands back for one read.
struct ModelRead {
    generation: u64,
    annotations: Vec<Annotation>,
    count: i32,
    /// The form's widgets, as `json_fields` gives them.
    fields: String,
    /// The stamped copy the reader registered for this read: its key and
    /// revision.
    stamped: Option<(String, i32)>,
    /// The read was a check, and the file had changed.
    changed: bool,
}

/// The newest of `first` and the requests already waiting behind it: the
/// older ones are skipped before they start.
fn newest(first: ModelRequest, waiting: &mpsc::Receiver<ModelRequest>) -> ModelRequest {
    let mut newest = first;
    while let Ok(newer) = waiting.try_recv() {
        newest = newer;
    }
    newest
}

/// Starts a window's model reader: one thread reading one document at a
/// time, the newest asked for, handing each result to `publish`. A check
/// (`only_if_changed`) reads only when the file's size or modification
/// time differs from the last read of the same path, and publishes nothing
/// otherwise. A document whose widgets can be stamped has its copy handed
/// to `register` (with its key), which gives the revision it is shown at.
/// It ends when its sender is dropped. `None` when the thread did not
/// start.
fn start_model_reader(
    register: impl Fn(&str, &[u8]) -> i32 + Send + 'static,
    publish: impl Fn(ModelRead) + Send + 'static,
) -> Option<mpsc::Sender<ModelRequest>> {
    let (sender, receiver) = mpsc::channel::<ModelRequest>();
    let spawned = std::thread::Builder::new()
        .name("calcita-annotations".to_owned())
        .spawn(move || {
            let mut last: Option<(PathBuf, Option<Stamp>)> = None;
            while let Ok(first) = receiver.recv() {
                let request = newest(first, &receiver);
                // Taken before the read, so a change during it is seen by
                // the next check.
                let stamp = Stamp::of(&request.path).ok();
                let known = last.as_ref().filter(|(path, _)| *path == request.path);
                if request.only_if_changed && known.is_some_and(|(_, seen)| *seen == stamp) {
                    continue;
                }
                // A check of a path never read (its first read overtaken)
                // is that first read, not a change.
                let changed = request.only_if_changed && known.is_some();
                last = Some((request.path.clone(), stamp));
                let read = match form::view(&request.path) {
                    Ok(viewed) => ModelRead {
                        generation: request.generation,
                        count: i32::try_from(viewed.annotations.len()).unwrap_or(i32::MAX),
                        annotations: viewed.annotations,
                        fields: json_fields(&viewed.fields, &viewed.boxes),
                        stamped: viewed
                            .stamped
                            .map(|copy| (request.key.clone(), register(&request.key, &copy))),
                        changed,
                    },
                    Err(error) => {
                        eprintln!(
                            "calcita: annotations of {}: {error}",
                            request.path.display()
                        );
                        ModelRead {
                            generation: request.generation,
                            annotations: Vec::new(),
                            count: -1,
                            fields: "[]".to_owned(),
                            stamped: None,
                            changed,
                        }
                    }
                };
                publish(read);
            }
        });
    match spawned {
        Ok(_) => Some(sender),
        Err(error) => {
            eprintln!("calcita: the annotation reader did not start: {error}");
            None
        }
    }
}

impl Default for CalcitaDocumentRust {
    fn default() -> Self {
        Self {
            document_key: QString::default(),
            document_name: QString::default(),
            document_url: QUrl::default(),
            page: 0,
            page_count: 0,
            zoom_mode: QString::from(mode_name(ZoomMode::FitWidth)),
            zoom_factor: 1.0,
            loaded: false,
            annotation_count: -1,
            revision: 0,
            fields: QString::from("[]"),
            stamped: None,
            in_flight: Arc::default(),
            zoom: ZoomMode::FitWidth,
            annotations: Vec::new(),
            model_generation: 0,
            model_reader: None,
        }
    }
}

/// The stamped copies registered and not yet applied, shared with the
/// reader.
type InFlight = Arc<Mutex<Vec<(String, i32)>>>;

/// Takes `copy` out of `in_flight` (it reached the window, or nobody).
fn landed(in_flight: &InFlight, copy: &(String, i32)) {
    in_flight
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|pending| pending != copy);
}

impl CalcitaDocumentRust {
    /// The stamped copies this window still holds in the provider: the one
    /// its pages show and those whose read is on its way, each taken.
    fn take_copies(&mut self) -> Vec<(String, i32)> {
        let pending = std::mem::take(
            &mut *self
                .in_flight
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        self.stamped.take().into_iter().chain(pending).collect()
    }
}

impl Drop for CalcitaDocumentRust {
    /// A window gone takes its stamped copies out of the provider.
    fn drop(&mut self) {
        for (key, revision) in self.take_copies() {
            pages::ffi::drop_copy(&QString::from(&key), revision);
        }
    }
}

/// Registers a stamped copy with the page provider under a new revision.
fn register_copy(key: &str, copy: &[u8]) -> i32 {
    let revision = pages::next_revision();
    pages::ffi::show_copy(&QString::from(key), revision, copy);
    revision
}

/// The `zoomMode` property's value: `fitWidth`, `fitPage` or `free`.
fn mode_name(mode: ZoomMode) -> &'static str {
    match mode {
        ZoomMode::FitWidth => "fitWidth",
        ZoomMode::FitPage => "fitPage",
        ZoomMode::Free(_) => "free",
    }
}

/// The zoom word to remember for `mode` while the view reads at `factor`: a
/// free zoom is remembered at the factor in use.
fn zoom_word_for(mode: ZoomMode, factor: f64) -> String {
    match mode {
        // The ladder is f32; the view's factor never needs more.
        #[allow(clippy::cast_possible_truncation)]
        ZoomMode::Free(_) => ZoomMode::Free(factor as f32).to_word(),
        fitted => fitted.to_word(),
    }
}

/// Whether a factor the view reports replaces `current`.
fn accepts_scale(current: f64, reported: f64) -> bool {
    reported.is_finite() && reported > 0.0 && (current - reported).abs() > 1e-6
}

impl qobject::CalcitaDocument {
    pub fn set_document_key(mut self: Pin<&mut Self>, key: QString) {
        if self.rust().document_key == key {
            return;
        }
        let path = pathkey::decode(&key.to_string()).ok();
        let name = path.as_deref().map(display_name).unwrap_or_default();
        let url = path
            .as_deref()
            .and_then(file_uri::from_path)
            .map(|uri| QUrl::from(&QString::from(uri.as_str())))
            .unwrap_or_default();
        self.as_mut().rust_mut().document_key = key;
        self.as_mut().rust_mut().document_name = QString::from(name.as_str());
        self.as_mut().rust_mut().document_url = url;
        self.as_mut().document_key_changed();
        self.as_mut().document_name_changed();
        self.as_mut().document_url_changed();
        self.as_mut().renew_revision();
        self.read_model(path, false);
    }

    fn renew_revision(self: Pin<&mut Self>) {
        self.show_revision(pages::next_revision(), None);
    }

    /// The pages move to `revision`, which shows `stamped` when it is the
    /// stamped copy's; the copy shown until now is dropped.
    fn show_revision(mut self: Pin<&mut Self>, revision: i32, stamped: Option<(String, i32)>) {
        if let Some((key, old)) = self.as_mut().rust_mut().stamped.take() {
            pages::ffi::drop_copy(&QString::from(&key), old);
        }
        self.as_mut().rust_mut().stamped = stamped;
        self.as_mut().rust_mut().revision = revision;
        self.revision_changed();
    }

    pub fn check_file(self: Pin<&mut Self>) {
        let path = pathkey::decode(&self.rust().document_key.to_string()).ok();
        if path.is_some() {
            self.read_model(path, true);
        }
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let path = pathkey::decode(&self.rust().document_key.to_string()).ok();
        self.as_mut().renew_revision();
        self.as_mut().read_model(path, false);
        self.reload_requested();
    }

    /// Reads the annotation model of `path` on this window's reader (`lopdf`
    /// reads the whole file) and publishes it on the Qt thread, unless
    /// another document came in the meantime. The reader takes one request
    /// at a time and only the newest of those waiting, so a document
    /// replaced before its read started is never read.
    ///
    /// A check (`only_if_changed`) keeps the model shown until the reader
    /// finds the file changed.
    fn read_model(mut self: Pin<&mut Self>, path: Option<PathBuf>, only_if_changed: bool) {
        // A check answers for the document already asked for, under its
        // generation.
        let generation = if only_if_changed {
            self.rust().model_generation
        } else {
            self.rust().model_generation.wrapping_add(1)
        };
        self.as_mut().rust_mut().model_generation = generation;
        if !only_if_changed {
            self.as_mut().apply_model(ModelRead {
                generation,
                annotations: Vec::new(),
                count: -1,
                fields: "[]".to_owned(),
                stamped: None,
                changed: false,
            });
        }
        let Some(path) = path else {
            return;
        };
        let request = ModelRequest {
            generation,
            key: self.rust().document_key.to_string(),
            path,
            only_if_changed,
        };
        let request = match self.rust().model_reader.as_ref() {
            Some(reader) => match reader.send(request) {
                Ok(()) => return,
                Err(mpsc::SendError(request)) => request,
            },
            None => request,
        };
        let qt = self.qt_thread();
        let registered = Arc::clone(&self.rust().in_flight);
        let unclaimed = Arc::clone(&self.rust().in_flight);
        let register = move |key: &str, copy: &[u8]| {
            let revision = register_copy(key, copy);
            registered
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((key.to_owned(), revision));
            revision
        };
        let reader = start_model_reader(register, move |read| {
            // A window gone before the read is queued leaves its copy to
            // the reader; one gone after takes it in `Drop`.
            let orphan = read.stamped.clone();
            let queued = qt.queue(move |document: Pin<&mut qobject::CalcitaDocument>| {
                document.apply_model(read);
            });
            if let (Err(_), Some(copy)) = (queued, orphan) {
                landed(&unclaimed, &copy);
                pages::ffi::drop_copy(&QString::from(&copy.0), copy.1);
            }
        });
        if let Some(reader) = reader {
            if reader.send(request).is_ok() {
                self.as_mut().rust_mut().model_reader = Some(reader);
            }
        }
    }

    fn apply_model(mut self: Pin<&mut Self>, read: ModelRead) {
        if let Some(copy) = &read.stamped {
            landed(&self.rust().in_flight, copy);
        }
        if self.rust().model_generation != read.generation {
            if let Some((key, revision)) = read.stamped {
                pages::ffi::drop_copy(&QString::from(&key), revision);
            }
            return;
        }
        self.as_mut().rust_mut().annotations = read.annotations;
        if self.rust().annotation_count != read.count {
            self.as_mut().rust_mut().annotation_count = read.count;
            self.as_mut().annotation_count_changed();
        }
        let fields = QString::from(&read.fields);
        if self.rust().fields != fields {
            self.as_mut().rust_mut().fields = fields;
            self.as_mut().fields_changed();
        }
        // The stamped copy has its own revision; a changed file without one
        // takes a new one.
        match read.stamped {
            Some((key, revision)) => self.as_mut().show_revision(revision, Some((key, revision))),
            None if read.changed => self.as_mut().renew_revision(),
            None => {}
        }
        if read.changed {
            self.reload_requested();
        }
    }

    fn set_page_value(mut self: Pin<&mut Self>, page: i32) {
        if self.rust().page != page {
            self.as_mut().rust_mut().page = page;
            self.as_mut().page_changed();
        }
    }

    fn apply_zoom(mut self: Pin<&mut Self>, mode: ZoomMode) {
        let previous = self.rust().zoom;
        self.as_mut().rust_mut().zoom = mode;
        if mode_name(previous) != mode_name(mode) {
            self.as_mut().rust_mut().zoom_mode = QString::from(mode_name(mode));
            self.as_mut().zoom_mode_changed();
        }
        if let ZoomMode::Free(factor) = mode {
            self.report_scale(f64::from(factor));
        }
    }

    fn current_factor(&self) -> f32 {
        // The ladder is f32; the view's factor never needs more.
        #[allow(clippy::cast_possible_truncation)]
        let factor = self.rust().zoom_factor as f32;
        factor
    }

    pub fn go_to(mut self: Pin<&mut Self>, text: &QString) -> bool {
        let current = u32::try_from(self.rust().page).unwrap_or(1);
        let count = u32::try_from(self.rust().page_count).unwrap_or(0);
        match GoTo::parse(&text.to_string(), current, count) {
            Ok(page) => {
                let page = i32::try_from(page).unwrap_or(1);
                self.as_mut().set_page_value(page);
                self.page_requested(page);
                true
            }
            Err(error) => {
                self.notice(QString::from("error"), QString::from(error.message_es()));
                false
            }
        }
    }

    pub fn zoom_in(self: Pin<&mut Self>) {
        let next = zoom::zoom_in(self.current_factor());
        self.apply_zoom(ZoomMode::Free(next));
    }

    pub fn zoom_out(self: Pin<&mut Self>) {
        let next = zoom::zoom_out(self.current_factor());
        self.apply_zoom(ZoomMode::Free(next));
    }

    pub fn fit_width(self: Pin<&mut Self>) {
        self.apply_zoom(ZoomMode::FitWidth);
    }

    pub fn fit_page(self: Pin<&mut Self>) {
        self.apply_zoom(ZoomMode::FitPage);
    }

    pub fn set_zoom(self: Pin<&mut Self>, factor: f64) {
        #[allow(clippy::cast_possible_truncation)]
        let factor = factor as f32;
        if factor.is_finite() && factor > 0.0 {
            self.apply_zoom(ZoomMode::Free(zoom::clamp(factor)));
        }
    }

    pub fn restore_zoom(self: Pin<&mut Self>, word: &QString) {
        if let Some(mode) = ZoomMode::parse(&word.to_string()) {
            self.apply_zoom(mode);
        }
    }

    pub fn zoom_word(&self) -> QString {
        QString::from(zoom_word_for(self.rust().zoom, self.rust().zoom_factor).as_str())
    }

    pub fn report_loaded(mut self: Pin<&mut Self>, count: i32) {
        let loaded = count >= 0;
        let count = count.max(0);
        if self.rust().page_count != count {
            self.as_mut().rust_mut().page_count = count;
            self.as_mut().page_count_changed();
        }
        self.as_mut().set_page_value(if count > 0 { 1 } else { 0 });
        if self.rust().loaded != loaded {
            self.as_mut().rust_mut().loaded = loaded;
            self.as_mut().loaded_changed();
        }
    }

    pub fn report_page(self: Pin<&mut Self>, page: i32) {
        let count = self.rust().page_count;
        if page >= 1 && page <= count {
            self.set_page_value(page);
        }
    }

    pub fn report_scale(mut self: Pin<&mut Self>, factor: f64) {
        if accepts_scale(self.rust().zoom_factor, factor) {
            self.as_mut().rust_mut().zoom_factor = factor;
            self.as_mut().zoom_factor_changed();
        }
    }
}

/// The hit a step lands on, in the property's `i32` terms (-1 for none).
fn step_hit(current: i32, count: i32, forward: bool) -> i32 {
    let direction = if forward {
        Direction::Forward
    } else {
        Direction::Backward
    };
    let current = usize::try_from(current).ok();
    let count = usize::try_from(count).unwrap_or(0);
    search::next_hit(current, count, direction, true)
        .and_then(|hit| i32::try_from(hit).ok())
        .unwrap_or(-1)
}

impl qobject::CalcitaDocument {
    pub fn search_query(&self, text: &QString) -> QString {
        SearchRequest::new(&text.to_string())
            .map(|request| QString::from(request.query.as_str()))
            .unwrap_or_default()
    }

    pub fn next_hit(&self, current: i32, count: i32, forward: bool) -> i32 {
        step_hit(current, count, forward)
    }

    pub fn annotations_on_page(&self, page: i32) -> i32 {
        count_on_page(&self.rust().annotations, page)
    }
}

/// How many of `annotations` lie on the 0-based `page`.
fn count_on_page(annotations: &[Annotation], page: i32) -> i32 {
    let Ok(page) = u32::try_from(page) else {
        return 0;
    };
    let count = annotations
        .iter()
        .filter(|annotation| annotation.page == page)
        .count();
    i32::try_from(count).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        accepts_scale, count_on_page, landed, mode_name, newest, start_model_reader, step_hit,
        zoom_word_for, CalcitaDocumentRust, ModelRequest,
    };
    use calcita_core::annot::form::ReadOnly;
    use calcita_core::annot::{self, Annotation};
    use calcita_core::zoom::ZoomMode;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_restored_word_names_the_mode_qml_sees() {
        let cases = [
            ("width", "fitWidth"),
            ("page", "fitPage"),
            ("free:1.5", "free"),
        ];
        for (word, name) in cases {
            let mode = ZoomMode::parse(word).expect("a known word");
            assert_eq!(mode_name(mode), name);
        }
        assert_eq!(
            ZoomMode::parse("zoom"),
            None,
            "an unknown word keeps the zoom"
        );
    }

    #[test]
    fn the_zoom_word_carries_the_factor_in_use() {
        assert_eq!(zoom_word_for(ZoomMode::FitWidth, 1.37), "width");
        assert_eq!(zoom_word_for(ZoomMode::FitPage, 0.8), "page");
        assert_eq!(zoom_word_for(ZoomMode::Free(1.0), 1.25), "free:1.25");
        let word = zoom_word_for(ZoomMode::Free(1.1), 1.1);
        assert_eq!(ZoomMode::parse(&word), Some(ZoomMode::Free(1.1)));
    }

    #[test]
    fn a_reported_scale_replaces_only_a_real_change() {
        assert!(accepts_scale(1.0, 1.46));
        assert!(!accepts_scale(1.0, 1.0));
        assert!(!accepts_scale(1.0, 0.0));
        assert!(!accepts_scale(1.0, f64::NAN));
    }

    #[test]
    fn a_step_speaks_the_properties_terms() {
        assert_eq!(step_hit(-1, 3, true), 0);
        assert_eq!(step_hit(-1, 3, false), 2);
        assert_eq!(step_hit(2, 3, true), 0);
        assert_eq!(step_hit(0, 0, true), -1);
        assert_eq!(step_hit(5, -1, false), -1);
    }

    fn fixture(name: &str) -> Vec<Annotation> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        annot::read_annotations(&path).expect("a readable fixture")
    }

    #[test]
    fn the_page_count_speaks_the_qml_page_index() {
        let annotated = fixture("annotated.pdf");
        assert_eq!(count_on_page(&annotated, 0), 4);
        assert_eq!(count_on_page(&annotated, 1), 0);
        assert_eq!(count_on_page(&annotated, -1), 0);
        let rotated = fixture("rotated.pdf");
        assert_eq!(
            (0..4)
                .map(|page| count_on_page(&rotated, page))
                .sum::<i32>(),
            4
        );
    }

    /// What the provider stand-in was handed: each copy's key and size.
    type Handed = Arc<Mutex<Vec<(String, usize)>>>;

    /// A provider stand-in: records what it is handed, by key, and gives
    /// revisions from 100.
    fn recorder() -> (Handed, impl Fn(&str, &[u8]) -> i32) {
        let handed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&handed);
        let register = move |key: &str, copy: &[u8]| {
            let mut seen = seen.lock().expect("unpoisoned");
            seen.push((key.to_owned(), copy.len()));
            99 + i32::try_from(seen.len()).expect("few")
        };
        (handed, register)
    }

    fn request(generation: u64) -> ModelRequest {
        ModelRequest {
            generation,
            key: format!("/nowhere/{generation}.pdf"),
            path: PathBuf::from(format!("/nowhere/{generation}.pdf")),
            only_if_changed: false,
        }
    }

    #[test]
    fn only_the_newest_waiting_read_starts() {
        let (sender, receiver) = mpsc::channel();
        for generation in 2..=4 {
            sender.send(request(generation)).expect("queued");
        }
        assert_eq!(newest(request(1), &receiver).generation, 4);
        assert_eq!(newest(request(5), &receiver).generation, 5, "none waiting");
    }

    #[test]
    fn the_reader_reads_one_document_at_a_time_and_publishes_each() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let (published, results) = mpsc::channel();
        let (handed, register) = recorder();
        let reader = start_model_reader(register, move |read| {
            let _ = published.send((read.generation, read.count));
        })
        .expect("the reader starts");
        reader
            .send(ModelRequest {
                generation: 1,
                key: "annotated".to_owned(),
                path: fixtures.join("annotated.pdf"),
                only_if_changed: false,
            })
            .expect("queued");
        let first = results.recv().expect("published");
        assert_eq!(first, (1, 4));
        reader.send(request(2)).expect("queued");
        assert_eq!(
            results.recv().expect("published"),
            (2, -1),
            "a missing file"
        );
        drop(reader);
        assert!(results.recv().is_err(), "the reader ends with its sender");
        assert!(
            handed.lock().expect("unpoisoned").is_empty(),
            "no form, no stamped copy"
        );
    }

    #[test]
    fn a_window_going_away_drops_its_copies_shown_and_on_their_way() {
        let mut document = CalcitaDocumentRust::default();
        let copy = |revision| ("/form.pdf".to_owned(), revision);
        document.stamped = Some(copy(3));
        document
            .in_flight
            .lock()
            .expect("unpoisoned")
            .extend([copy(4), copy(5)]);
        landed(&document.in_flight, &copy(4));
        assert_eq!(
            document.take_copies(),
            vec![copy(3), copy(5)],
            "the one shown and the one whose read never arrived, not the landed one"
        );
        assert!(document.take_copies().is_empty(), "each once");
    }

    #[test]
    fn a_form_is_read_with_its_widgets_and_its_stamped_copy() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let (published, results) = mpsc::channel();
        let (handed, register) = recorder();
        let reader = start_model_reader(register, move |read| {
            let _ = published.send((read.count, read.fields, read.stamped));
        })
        .expect("the reader starts");
        reader
            .send(ModelRequest {
                generation: 1,
                key: "form-key".to_owned(),
                path: fixtures.join("form.pdf"),
                only_if_changed: false,
            })
            .expect("queued");
        let (count, fields, stamped) = results.recv().expect("published");
        assert_eq!(count, 10, "every widget is an annotation");
        assert_eq!(stamped, Some(("form-key".to_owned(), 100)));
        let handed = handed.lock().expect("unpoisoned").clone();
        assert_eq!(handed.len(), 1);
        assert_eq!(handed[0].0, "form-key");
        let original = std::fs::metadata(fixtures.join("form.pdf"))
            .expect("the fixture")
            .len();
        assert!(
            u64::try_from(handed[0].1).expect("a size") > original,
            "the copy appends to the file"
        );
        assert!(
            fields.starts_with("[{\"field\":\"o8.0\",\"name\":\"name\",\"label\":\"Full name\",\"order\":0,\"widget\":0,\"page\":0,\"x\":72,\"y\":162,\"w\":300,\"h\":30,\"value\":\"\",\"kind\":\"text\",\"fillable\":true"),
            "{fields}"
        );
        assert!(
            fields.contains(
                "\"name\":\"copy\",\"label\":\"copy\",\"order\":5,\"widget\":1,\"page\":1"
            ),
            "{fields}"
        );
        let radio = crate::editor::json_string(ReadOnly::Radio.message_es());
        assert!(
            fields.contains(&radio),
            "the radio group's notice: {fields}"
        );
    }

    #[test]
    fn a_check_reads_again_only_when_the_file_changed() {
        let dir = std::env::temp_dir().join(format!("calcita-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("doc.pdf");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        std::fs::copy(fixtures.join("three-pages.pdf"), &path).expect("copied");
        let (published, results) = mpsc::channel();
        let (_, register) = recorder();
        let reader = start_model_reader(register, move |read| {
            let _ = published.send((read.generation, read.count, read.changed));
        })
        .expect("the reader starts");
        let ask = |generation, only_if_changed| ModelRequest {
            generation,
            key: "doc".to_owned(),
            path: path.clone(),
            only_if_changed,
        };
        reader.send(ask(1, false)).expect("queued");
        assert_eq!(results.recv().expect("published"), (1, 0, false));
        // An unchanged file: the check publishes nothing, so the read
        // queued after it is the next to publish.
        reader.send(ask(2, true)).expect("queued");
        reader.send(ask(3, false)).expect("queued");
        assert_eq!(results.recv().expect("published"), (3, 0, false));
        std::fs::copy(fixtures.join("annotated.pdf"), &path).expect("replaced outside");
        reader.send(ask(4, true)).expect("queued");
        assert_eq!(results.recv().expect("published"), (4, 4, true));
        reader.send(ask(5, true)).expect("queued");
        drop(reader);
        assert!(
            results.recv().is_err(),
            "an unchanged file publishes nothing"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
