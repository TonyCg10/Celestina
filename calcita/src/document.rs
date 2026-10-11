//! One window's reading state: which document, which page of how many, and
//! the zoom. QtPdf renders and reports; this object decides what the page
//! field and the zoom controls mean (`calcita_core`'s grammar and ladder).
//! It also reads the document's annotation model on a worker
//! (`calcita_core::annot`), which editing (CAL-2-B) builds on.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::mpsc;

use calcita_core::annot::{self, Annotation};
use calcita_core::pagelabel::GoTo;
use calcita_core::search::{self, Direction, SearchRequest};
use calcita_core::zoom::{self, ZoomMode};
use celestina_core::{file_uri, pathkey};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};

use crate::controller::display_name;

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
        type CalcitaDocument = super::CalcitaDocumentRust;

        /// Sets the document this window shows (a pathkey).
        fn set_document_key(self: Pin<&mut CalcitaDocument>, key: QString);

        /// The view should scroll to `page` (1-based).
        #[qsignal]
        fn page_requested(self: Pin<&mut CalcitaDocument>, page: i32);

        /// Something to tell the person: `kind` is `error` or `info`.
        #[qsignal]
        fn notice(self: Pin<&mut CalcitaDocument>, kind: QString, text: QString);

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
    path: PathBuf,
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
/// time, the newest asked for, handing each result to `publish`. It ends
/// when its sender is dropped. `None` when the thread did not start.
fn start_model_reader(
    publish: impl Fn(u64, Vec<Annotation>, i32) + Send + 'static,
) -> Option<mpsc::Sender<ModelRequest>> {
    let (sender, receiver) = mpsc::channel::<ModelRequest>();
    let spawned = std::thread::Builder::new()
        .name("calcita-annotations".to_owned())
        .spawn(move || {
            while let Ok(first) = receiver.recv() {
                let request = newest(first, &receiver);
                let (annotations, count) = match annot::read_annotations(&request.path) {
                    Ok(annotations) => {
                        let count = i32::try_from(annotations.len()).unwrap_or(i32::MAX);
                        (annotations, count)
                    }
                    Err(error) => {
                        eprintln!(
                            "calcita: annotations of {}: {error}",
                            request.path.display()
                        );
                        (Vec::new(), -1)
                    }
                };
                publish(request.generation, annotations, count);
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
            zoom: ZoomMode::FitWidth,
            annotations: Vec::new(),
            model_generation: 0,
            model_reader: None,
        }
    }
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
        self.read_model(path);
    }

    /// Reads the annotation model of `path` on this window's reader (`lopdf`
    /// reads the whole file) and publishes it on the Qt thread, unless
    /// another document came in the meantime. The reader takes one request
    /// at a time and only the newest of those waiting, so a document
    /// replaced before its read started is never read.
    fn read_model(mut self: Pin<&mut Self>, path: Option<PathBuf>) {
        let generation = self.rust().model_generation.wrapping_add(1);
        self.as_mut().rust_mut().model_generation = generation;
        self.as_mut().apply_model(generation, Vec::new(), -1);
        let Some(path) = path else {
            return;
        };
        let request = ModelRequest { generation, path };
        let request = match self.rust().model_reader.as_ref() {
            Some(reader) => match reader.send(request) {
                Ok(()) => return,
                Err(mpsc::SendError(request)) => request,
            },
            None => request,
        };
        let qt = self.qt_thread();
        let reader = start_model_reader(move |generation, annotations, count| {
            let _ = qt.queue(move |document: Pin<&mut qobject::CalcitaDocument>| {
                document.apply_model(generation, annotations, count);
            });
        });
        if let Some(reader) = reader {
            if reader.send(request).is_ok() {
                self.as_mut().rust_mut().model_reader = Some(reader);
            }
        }
    }

    fn apply_model(
        mut self: Pin<&mut Self>,
        generation: u64,
        annotations: Vec<Annotation>,
        count: i32,
    ) {
        if self.rust().model_generation != generation {
            return;
        }
        self.as_mut().rust_mut().annotations = annotations;
        if self.rust().annotation_count != count {
            self.as_mut().rust_mut().annotation_count = count;
            self.as_mut().annotation_count_changed();
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
        accepts_scale, count_on_page, mode_name, newest, start_model_reader, step_hit,
        zoom_word_for, ModelRequest,
    };
    use calcita_core::annot::{self, Annotation};
    use calcita_core::zoom::ZoomMode;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

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

    fn request(generation: u64) -> ModelRequest {
        ModelRequest {
            generation,
            path: PathBuf::from(format!("/nowhere/{generation}.pdf")),
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
        let reader = start_model_reader(move |generation, _annotations, count| {
            let _ = published.send((generation, count));
        })
        .expect("the reader starts");
        reader
            .send(ModelRequest {
                generation: 1,
                path: fixtures.join("annotated.pdf"),
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
    }
}
