// language-contract: product-copy
//! One window's editing: the edit session, its undo and redo, and its save.
//!
//! What an edit *is* lives in `calcita_core::annot` (the edit set, the
//! writer, the one conversion between the displayed page and PDF space);
//! what saving *costs* lives in [`crate::saving`]. This object moves values
//! between them and QML:
//!
//! - **Beginning reads on a worker.** The document is read once for its
//!   annotations and page boxes, and once for the copy the page provider
//!   shows while editing (the editable annotations hidden, so QtPdf draws
//!   the rest and QML draws these); the copy is registered with the
//!   provider under a revision of its own before the window's pages move
//!   to it. An encrypted or unparseable document refuses with a notice.
//! - **Every coordinate crossing this seam is in the displayed page's
//!   points** (top-left origin, after `/Rotate`); `calcita_core` converts.
//! - **The annotations are published as one JSON array** (`annotations`),
//!   so QML rebuilds from one consistent value rather than from columns
//!   published one after another.
//! - **Saving runs on a worker** and lands through `crate::saving`; the
//!   window hears `saved` only once the bytes are on disk.

use std::path::PathBuf;
use std::pin::Pin;

use calcita_core::annot::appearance::{self, number};
use calcita_core::annot::coords::{to_pdf_rect, to_view, to_view_rect, PageBox};
use calcita_core::annot::edit::{self, is_editable, lays_out_again, Base, Edit, EditSet};
use calcita_core::annot::write::hidden_copy;
use calcita_core::annot::{AnnotError, AnnotId, Annotation, Kind, Point, Rect, Rgb};
use celestina_core::pathkey;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QPointF, QPolygonF, QString, QVariant};

use crate::pages;
use crate::saving::{self, DesktopTrash, SaveChoice, SaveError, Saved, Stamp};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
        include!("cxx-qt-lib/qpointf.h");
        type QPointF = cxx_qt_lib::QPointF;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, document_key, cxx_name = "documentKey", READ, WRITE = set_document_key, NOTIFY)]
        #[qproperty(bool, editing, READ, NOTIFY)]
        #[qproperty(bool, preparing, READ, NOTIFY)]
        #[qproperty(bool, saving, READ, NOTIFY)]
        #[qproperty(bool, edited, READ, NOTIFY)]
        #[qproperty(bool, can_undo, cxx_name = "canUndo", READ, NOTIFY)]
        #[qproperty(bool, can_redo, cxx_name = "canRedo", READ, NOTIFY)]
        #[qproperty(i32, revision, READ, NOTIFY)]
        #[qproperty(QString, annotations, READ, NOTIFY)]
        #[qproperty(f64, text_padding, cxx_name = "textPadding", READ, CONSTANT)]
        #[qproperty(f64, line_height, cxx_name = "lineHeight", READ, CONSTANT)]
        type CalcitaEditor = super::CalcitaEditorRust;

        /// Sets the document this editor edits (a pathkey); a session on
        /// another document ends.
        fn set_document_key(self: Pin<&mut CalcitaEditor>, key: QString);

        /// Something to tell the person: `kind` is `error` or `info`.
        #[qsignal]
        fn notice(self: Pin<&mut CalcitaEditor>, kind: QString, text: QString);

        /// A save landed: `key` holds the edited document now, and
        /// `replaced` says whether the original went to the Trash. The
        /// session has ended.
        #[qsignal]
        fn saved(self: Pin<&mut CalcitaEditor>, key: QString, replaced: bool);

        /// Starts editing: the document is read on a worker, and `editing`
        /// turns true once its copy is ready; a refusal is a notice.
        #[qinvokable]
        fn begin(self: Pin<&mut CalcitaEditor>);

        /// Ends editing and forgets every edit («Descartar», or leaving
        /// without changes).
        #[qinvokable]
        fn finish(self: Pin<&mut CalcitaEditor>);

        /// Adds a text box on the 0-based `page` whose top-left corner is
        /// `at` and whose width is `width` (displayed points); it is as tall
        /// as its lines. Gives its id, or "" when refused.
        #[qinvokable]
        fn add_text(
            self: Pin<&mut CalcitaEditor>,
            page: i32,
            at: QPointF,
            width: f64,
            text: &QString,
            color: &QString,
            size: f64,
        ) -> QString;

        /// Adds a highlight over `outlines`, QtPdf's selection geometry on
        /// that page (polygons in displayed points): one quadrilateral per
        /// outline's box. Gives its id or "".
        #[qinvokable]
        fn add_highlight(
            self: Pin<&mut CalcitaEditor>,
            page: i32,
            outlines: &QList_QVariant,
            color: &QString,
        ) -> QString;

        /// Adds a drawing through `points`, `x,y` in displayed points
        /// joined with spaces, `width` points wide. Gives its id or "".
        #[qinvokable]
        fn add_ink(
            self: Pin<&mut CalcitaEditor>,
            page: i32,
            points: &QString,
            color: &QString,
            width: f64,
        ) -> QString;

        /// The annotation `id` now fills this rectangle (displayed points).
        #[qinvokable]
        fn move_to(
            self: Pin<&mut CalcitaEditor>,
            id: &QString,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        ) -> bool;

        #[qinvokable]
        fn recolor(self: Pin<&mut CalcitaEditor>, id: &QString, color: &QString) -> bool;

        /// A text box's new text.
        #[qinvokable]
        fn set_text(self: Pin<&mut CalcitaEditor>, id: &QString, text: &QString) -> bool;

        #[qinvokable]
        fn remove(self: Pin<&mut CalcitaEditor>, id: &QString) -> bool;

        #[qinvokable]
        fn undo(self: Pin<&mut CalcitaEditor>);

        #[qinvokable]
        fn redo(self: Pin<&mut CalcitaEditor>);

        /// Saves on a worker: a copy beside the original («Guardar
        /// ambas»), or, with `replace`, the original to the Trash and the
        /// edited document under its name («Guardar solo la editada»).
        #[qinvokable]
        fn save(self: Pin<&mut CalcitaEditor>, replace: bool);
    }

    impl cxx_qt::Threading for CalcitaEditor {}
}

/// Said when there is nothing to save.
const NOTHING_TO_SAVE: &str = "No hay cambios que guardar.";
/// Said when the system refused a worker thread.
const WORKER_NOT_STARTED: &str = "No se pudo empezar; inténtalo de nuevo.";

/// The edit session of one document.
struct Session {
    path: PathBuf,
    /// The file's version when the session began; a save refuses another.
    stamp: Stamp,
    base: Base,
    edits: EditSet,
}

/// What beginning reads, or the message that says why it could not.
struct Prepared {
    stamp: Stamp,
    base: Base,
}

pub struct CalcitaEditorRust {
    document_key: QString,
    editing: bool,
    preparing: bool,
    saving: bool,
    edited: bool,
    can_undo: bool,
    can_redo: bool,
    /// The revision the window's pages render the copy at while editing;
    /// 0 otherwise.
    revision: i32,
    annotations: QString,
    /// A text box's layout, as the saved appearance lays it out: the
    /// padding around the text in points, and the line height in font
    /// sizes.
    text_padding: f64,
    line_height: f64,
    session: Option<Session>,
    /// Bumped when a session begins or ends, so a late worker's result for
    /// another session is dropped.
    generation: u64,
    /// The copy a beginning worker registers (key, revision) until its
    /// result reaches this object: a window destroyed between the worker's
    /// queue and the Qt thread's call takes it out in `Drop`.
    pending_copy: Option<(String, i32)>,
}

impl Default for CalcitaEditorRust {
    fn default() -> Self {
        Self {
            document_key: QString::default(),
            editing: false,
            preparing: false,
            saving: false,
            edited: false,
            can_undo: false,
            can_redo: false,
            revision: 0,
            annotations: QString::from("[]"),
            text_padding: appearance::TEXT_PADDING,
            line_height: appearance::LINE_HEIGHT,
            session: None,
            generation: 0,
            pending_copy: None,
        }
    }
}

impl Drop for CalcitaEditorRust {
    /// A window gone while editing, or while its edit was beginning, takes
    /// its copies out of the provider.
    fn drop(&mut self) {
        let shown = (self.revision != 0).then(|| (self.document_key.to_string(), self.revision));
        for (key, revision) in orphans(shown, self.pending_copy.take()) {
            pages::ffi::drop_copy(&QString::from(&key), revision);
        }
    }
}

/// The copies a window going away leaves registered: the one its pages
/// show and the one a beginning worker registers or has registered, each
/// once.
fn orphans(shown: Option<(String, i32)>, pending: Option<(String, i32)>) -> Vec<(String, i32)> {
    let mut copies: Vec<(String, i32)> = shown.into_iter().collect();
    if let Some(pending) = pending {
        if !copies.contains(&pending) {
            copies.push(pending);
        }
    }
    copies
}

/// Reads what a session needs and registers the copy the window's pages
/// show at `revision`. Runs on the beginning worker.
fn prepare(path: &std::path::Path, key: &str, revision: i32) -> Result<Prepared, String> {
    let read = |error: AnnotError| error.message_es();
    let stamp = Stamp::of(path).map_err(|error| read(AnnotError::Io(error)))?;
    let base = Base::read(path).map_err(read)?;
    let copy = hidden_copy(path, &base.editable_ids()).map_err(read)?;
    pages::ffi::show_copy(&QString::from(key), revision, &copy);
    Ok(Prepared { stamp, base })
}

/// After the beginning worker handed its result back: when the window was
/// already gone (`delivered` false) and the copy was registered, nobody
/// else will take it out of the provider, so the worker does (`forget`).
fn settle(delivered: bool, registered: bool, forget: impl FnOnce()) {
    if !delivered && registered {
        forget();
    }
}

/// The text an annotation id is published as, and looked up by.
fn id_text(id: &AnnotId) -> String {
    match id {
        AnnotId::Object { number, generation } => format!("o{number}.{generation}"),
        AnnotId::Name(name) => format!("n{name}"),
        AnnotId::Position { page, index } => format!("p{page}.{index}"),
    }
}

/// A colour QML spells `#rrggbb` (or `#aarrggbb`, whose alpha is dropped).
fn parse_color(text: &str) -> Option<Rgb> {
    let hex = text.strip_prefix('#')?;
    if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
        return None;
    }
    let rgb = &hex[hex.len() - 6..];
    let channel = |at: usize| u8::from_str_radix(&rgb[at..at + 2], 16).ok();
    Some(Rgb {
        r: channel(0)?,
        g: channel(2)?,
        b: channel(4)?,
    })
}

fn color_text(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// `x,y` pairs joined with spaces; `None` when one is not two finite
/// numbers.
fn parse_points(text: &str) -> Option<Vec<Point>> {
    text.split_ascii_whitespace()
        .map(|pair| {
            let (x, y) = pair.split_once(',')?;
            let x: f64 = x.parse().ok()?;
            let y: f64 = y.parse().ok()?;
            (x.is_finite() && y.is_finite()).then_some(Point { x, y })
        })
        .collect()
}

/// The box of each polygon of `outlines` (QtPdf's selection geometry);
/// what is not a polygon, or not finite, is skipped.
fn outline_boxes(outlines: &QList<QVariant>) -> Vec<Rect> {
    outlines
        .iter()
        .filter_map(|outline| outline.value::<QPolygonF>())
        .map(|outline| outline.bounding_rect())
        .map(|bounds| (bounds.x(), bounds.y(), bounds.width(), bounds.height()))
        .filter(|(x, y, width, height)| [x, y, width, height].iter().all(|v| v.is_finite()))
        .map(|(x, y, width, height)| Rect::new(x, y, x + width, y + height))
        .collect()
}

/// `text` as a JSON string.
fn json_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            control if u32::from(control) < 0x20 => {
                quoted.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// A JSON array of the points' displayed coordinates, flat.
fn json_points<'a>(page: PageBox, points: impl Iterator<Item = &'a Point>) -> String {
    let coordinates: Vec<String> = points
        .map(|point| to_view(page, *point))
        .flat_map(|shown| [number(shown.x), number(shown.y)])
        .collect();
    format!("[{}]", coordinates.join(","))
}

/// One annotation as QML reads it, in the displayed page's points: its id,
/// kind, page, bounds, colour, and what draws it (a text box's text, size,
/// whether it is rich and whether Calcita lays it out, `laidOut`, so only
/// such a box is resized: another program's keeps its own appearance and
/// size; a highlight's quadrilaterals as polygons, upper
/// left, upper right, lower right, lower left; a drawing's strokes and
/// width).
fn json_annotation(annotation: &Annotation, page: PageBox, laid_out: bool) -> String {
    let bounds = to_view_rect(page, annotation.rect);
    let (kind, details) = match &annotation.kind {
        Kind::Text {
            contents,
            size_pt,
            rich,
        } => (
            "text",
            format!(
                "\"text\":{},\"size\":{},\"rich\":{rich},\"laidOut\":{laid_out}",
                json_string(contents),
                number(*size_pt)
            ),
        ),
        Kind::Highlight { quads } => {
            let polygons: Vec<String> = quads
                .iter()
                .map(|quad| {
                    let [a, b, c, d] = &quad.points;
                    json_points(page, [a, b, d, c].into_iter())
                })
                .collect();
            ("highlight", format!("\"shape\":[{}]", polygons.join(",")))
        }
        Kind::Ink { strokes, width } => {
            let lines: Vec<String> = strokes
                .iter()
                .map(|stroke| json_points(page, stroke.iter()))
                .collect();
            (
                "ink",
                format!(
                    "\"shape\":[{}],\"size\":{}",
                    lines.join(","),
                    number(*width)
                ),
            )
        }
        Kind::Other { .. } => ("other", String::new()),
    };
    format!(
        "{{\"id\":{},\"kind\":\"{kind}\",\"page\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"color\":\"{}\",{details}}}",
        json_string(&id_text(&annotation.id)),
        annotation.page,
        number(bounds.x0),
        number(bounds.y0),
        number(bounds.width()),
        number(bounds.height()),
        color_text(annotation.color),
    )
}

/// The editable annotations as one JSON array, for QML.
fn json_model(shown: &[Annotation], base: &Base) -> String {
    let items: Vec<String> = shown
        .iter()
        .filter(|annotation| is_editable(annotation))
        .filter_map(|annotation| {
            let original = base
                .annotations
                .iter()
                .find(|original| original.id == annotation.id);
            base.page_box(annotation.page)
                .map(|page| json_annotation(annotation, page, lays_out_again(original, annotation)))
        })
        .collect();
    format!("[{}]", items.join(","))
}

macro_rules! set_property {
    ($object:expr, $field:ident, $changed:ident, $value:expr) => {{
        let value = $value;
        if $object.rust().$field != value {
            $object.as_mut().rust_mut().$field = value;
            $object.as_mut().$changed();
        }
    }};
}

impl qobject::CalcitaEditor {
    pub fn set_document_key(mut self: Pin<&mut Self>, key: QString) {
        if self.rust().document_key == key {
            return;
        }
        self.as_mut().finish();
        self.as_mut().rust_mut().document_key = key;
        self.as_mut().document_key_changed();
    }

    fn refuse(self: Pin<&mut Self>, message: &str) {
        self.notice(QString::from("error"), QString::from(message));
    }

    pub fn begin(mut self: Pin<&mut Self>) {
        if self.rust().editing || self.rust().preparing || self.rust().saving {
            return;
        }
        let key = self.rust().document_key.to_string();
        let Ok(path) = pathkey::decode(&key) else {
            self.refuse(&AnnotError::NotEditable.message_es());
            return;
        };
        let generation = self.rust().generation.wrapping_add(1);
        self.as_mut().rust_mut().generation = generation;
        set_property!(self, preparing, preparing_changed, true);
        let revision = pages::next_revision();
        self.as_mut().rust_mut().pending_copy = Some((key.clone(), revision));
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("calcita-edit-begin".to_owned())
            .spawn(move || {
                let prepared = prepare(&path, &key, revision);
                let registered = prepared.is_ok();
                let copy_key = QString::from(&key);
                let queued = qt.queue(move |editor: Pin<&mut qobject::CalcitaEditor>| {
                    editor.prepared(generation, &key, path, revision, prepared);
                });
                settle(queued.is_ok(), registered, || {
                    pages::ffi::drop_copy(&copy_key, revision);
                });
            });
        if let Err(error) = spawned {
            eprintln!("calcita: the edit worker did not start: {error}");
            set_property!(self, preparing, preparing_changed, false);
            self.refuse(WORKER_NOT_STARTED);
        }
    }

    fn prepared(
        mut self: Pin<&mut Self>,
        generation: u64,
        key: &str,
        path: PathBuf,
        revision: i32,
        prepared: Result<Prepared, String>,
    ) {
        if self
            .rust()
            .pending_copy
            .as_ref()
            .is_some_and(|(_, pending)| *pending == revision)
        {
            self.as_mut().rust_mut().pending_copy = None;
        }
        if self.rust().generation != generation {
            // A session that ended before it began: its copy goes too.
            pages::ffi::drop_copy(&QString::from(key), revision);
            return;
        }
        set_property!(self, preparing, preparing_changed, false);
        match prepared {
            Ok(Prepared { stamp, base }) => {
                self.as_mut().rust_mut().session = Some(Session {
                    path,
                    stamp,
                    base,
                    edits: EditSet::new(),
                });
                set_property!(self, revision, revision_changed, revision);
                set_property!(self, editing, editing_changed, true);
                self.publish();
            }
            Err(message) => self.refuse(&message),
        }
    }

    pub fn finish(mut self: Pin<&mut Self>) {
        let generation = self.rust().generation.wrapping_add(1);
        self.as_mut().rust_mut().generation = generation;
        self.as_mut().rust_mut().session = None;
        let revision = self.rust().revision;
        if revision != 0 {
            pages::ffi::drop_copy(&self.rust().document_key, revision);
        }
        set_property!(self, preparing, preparing_changed, false);
        set_property!(self, editing, editing_changed, false);
        set_property!(self, revision, revision_changed, 0);
        self.publish();
    }

    /// Publishes the annotations as they are now and what can be done.
    fn publish(mut self: Pin<&mut Self>) {
        let (model, edited, can_undo, can_redo) = match self.rust().session.as_ref() {
            Some(session) => (
                json_model(&session.edits.apply(&session.base), &session.base),
                !session.edits.is_empty(),
                session.edits.can_undo(),
                session.edits.can_redo(),
            ),
            None => ("[]".to_owned(), false, false, false),
        };
        set_property!(
            self,
            annotations,
            annotations_changed,
            QString::from(&model)
        );
        set_property!(self, edited, edited_changed, edited);
        set_property!(self, can_undo, can_undo_changed, can_undo);
        set_property!(self, can_redo, can_redo_changed, can_redo);
    }

    /// Pushes the edit `make` builds from the session; a refusal is a
    /// notice. Gives whether it was kept.
    fn push(mut self: Pin<&mut Self>, make: impl FnOnce(&Session) -> Option<Edit>) -> bool {
        if self.rust().saving {
            return false;
        }
        let outcome = {
            let mut rust = self.as_mut().rust_mut();
            let Some(session) = rust.session.as_mut() else {
                return false;
            };
            match make(session) {
                Some(edit) => session.edits.push(&session.base, edit),
                None => Err(AnnotError::NotEditable),
            }
        };
        match outcome {
            Ok(()) => {
                self.publish();
                true
            }
            Err(error) => {
                self.refuse(&error.message_es());
                false
            }
        }
    }

    /// The shown annotation `id` names.
    fn find(session: &Session, id: &str) -> Option<Annotation> {
        session
            .edits
            .apply(&session.base)
            .into_iter()
            .find(|annotation| id_text(&annotation.id) == id)
    }

    /// Pushes an edit that adds what `make` builds; gives its id or "".
    fn add(self: Pin<&mut Self>, make: impl FnOnce(&Session) -> Option<Annotation>) -> QString {
        let mut id = String::new();
        let kept = self.push(|session| {
            let annotation = make(session)?;
            id = id_text(&annotation.id);
            Some(Edit::Add(annotation))
        });
        if kept {
            QString::from(&id)
        } else {
            QString::default()
        }
    }

    pub fn add_text(
        self: Pin<&mut Self>,
        page: i32,
        at: QPointF,
        width: f64,
        text: &QString,
        color: &QString,
        size: f64,
    ) -> QString {
        let (x, y) = (at.x(), at.y());
        let (text, color) = (text.to_string(), parse_color(&color.to_string()));
        self.add(|session| {
            let page = u32::try_from(page).ok()?;
            let page_box = session.base.page_box(page)?;
            let sane = [x, y, width, size].iter().all(|value| value.is_finite());
            (sane && size > 0.0).then_some(())?;
            let view = Rect::new(x, y, x + width.max(0.0), y);
            Some(edit::text_box(page_box, page, view, &text, size, color?))
        })
    }

    pub fn add_highlight(
        self: Pin<&mut Self>,
        page: i32,
        outlines: &QList<QVariant>,
        color: &QString,
    ) -> QString {
        let (boxes, color) = (outline_boxes(outlines), parse_color(&color.to_string()));
        self.add(|session| {
            let page = u32::try_from(page).ok()?;
            edit::highlight(session.base.page_box(page)?, page, &boxes, color?)
        })
    }

    pub fn add_ink(
        self: Pin<&mut Self>,
        page: i32,
        points: &QString,
        color: &QString,
        width: f64,
    ) -> QString {
        let (points, color) = (
            parse_points(&points.to_string()),
            parse_color(&color.to_string()),
        );
        self.add(|session| {
            let page = u32::try_from(page).ok()?;
            (width.is_finite() && width > 0.0).then_some(())?;
            edit::ink(session.base.page_box(page)?, page, &points?, width, color?)
        })
    }

    pub fn move_to(
        self: Pin<&mut Self>,
        id: &QString,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let id = id.to_string();
        self.push(|session| {
            let annotation = Self::find(session, &id)?;
            let page = session.base.page_box(annotation.page)?;
            [x, y, width, height]
                .iter()
                .all(|value| value.is_finite())
                .then_some(())?;
            let view = Rect::new(x, y, x + width, y + height);
            Some(Edit::Move(annotation.id, to_pdf_rect(page, view)))
        })
    }

    pub fn recolor(self: Pin<&mut Self>, id: &QString, color: &QString) -> bool {
        let (id, color) = (id.to_string(), parse_color(&color.to_string()));
        self.push(|session| Some(Edit::Recolor(Self::find(session, &id)?.id, color?)))
    }

    pub fn set_text(self: Pin<&mut Self>, id: &QString, text: &QString) -> bool {
        let (id, text) = (id.to_string(), text.to_string());
        self.push(|session| Some(Edit::SetText(Self::find(session, &id)?.id, text)))
    }

    pub fn remove(self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        self.push(|session| Some(Edit::Delete(Self::find(session, &id)?.id)))
    }

    pub fn undo(mut self: Pin<&mut Self>) {
        if self.rust().saving {
            return;
        }
        let undone = self
            .as_mut()
            .rust_mut()
            .session
            .as_mut()
            .is_some_and(|session| session.edits.undo());
        if undone {
            self.publish();
        }
    }

    pub fn redo(mut self: Pin<&mut Self>) {
        if self.rust().saving {
            return;
        }
        let redone = self
            .as_mut()
            .rust_mut()
            .session
            .as_mut()
            .is_some_and(|session| session.edits.redo());
        if redone {
            self.publish();
        }
    }

    pub fn save(mut self: Pin<&mut Self>, replace: bool) {
        if self.rust().saving {
            return;
        }
        let job = self
            .rust()
            .session
            .as_ref()
            .map(|session| (session.path.clone(), session.stamp, session.edits.clone()));
        let Some((path, stamp, edits)) = job else {
            return;
        };
        if edits.is_empty() {
            self.notice(QString::from("info"), QString::from(NOTHING_TO_SAVE));
            return;
        }
        let choice = if replace {
            SaveChoice::EditedOnly
        } else {
            SaveChoice::Both
        };
        let generation = self.rust().generation;
        set_property!(self, saving, saving_changed, true);
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("calcita-edit-save".to_owned())
            .spawn(move || {
                let outcome = saving::save(&path, stamp, &edits, choice, &DesktopTrash);
                let _ = qt.queue(move |editor: Pin<&mut qobject::CalcitaEditor>| {
                    editor.landed(generation, outcome);
                });
            });
        if let Err(error) = spawned {
            eprintln!("calcita: the save worker did not start: {error}");
            set_property!(self, saving, saving_changed, false);
            self.refuse(WORKER_NOT_STARTED);
        }
    }

    fn landed(mut self: Pin<&mut Self>, generation: u64, outcome: Result<Saved, SaveError>) {
        set_property!(self, saving, saving_changed, false);
        let current = self.rust().generation == generation;
        match outcome {
            Ok(saved) => {
                self.as_mut()
                    .notice(QString::from("info"), QString::from(&saved.message_es()));
                if current {
                    self.as_mut().finish();
                }
                let key = pathkey::encode(&saved.written);
                self.saved(QString::from(&key), saved.replaced);
            }
            Err(error) => {
                eprintln!("calcita: the save failed: {error}");
                let original_gone = matches!(error, SaveError::NotPublished { .. });
                self.as_mut().refuse(&error.message_es());
                if current && original_gone {
                    // The original is in the Trash: the session's document
                    // is gone, and the edited one is where the notice says.
                    self.finish();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        color_text, id_text, json_model, json_string, orphans, outline_boxes, parse_color,
        parse_points, settle,
    };
    use calcita_core::annot::coords::{PageBox, Rotation};
    use calcita_core::annot::edit::Base;
    use calcita_core::annot::{read_annotations, AnnotId, Point, Rect, Rgb};
    use cxx_qt_lib::{QList, QPointF, QPolygonF, QRectF, QString, QVariant};
    use std::path::Path;

    #[test]
    fn ids_are_spelled_by_kind() {
        assert_eq!(
            id_text(&AnnotId::Object {
                number: 6,
                generation: 0
            }),
            "o6.0"
        );
        assert_eq!(
            id_text(&AnnotId::Name("calcita-1".to_owned())),
            "ncalcita-1"
        );
        assert_eq!(id_text(&AnnotId::Position { page: 2, index: 3 }), "p2.3");
    }

    #[test]
    fn colours_come_as_qml_spells_them() {
        assert_eq!(
            parse_color("#3e91ff"),
            Some(Rgb {
                r: 0x3e,
                g: 0x91,
                b: 0xff
            })
        );
        assert_eq!(
            parse_color("#803e91ff"),
            Some(Rgb {
                r: 0x3e,
                g: 0x91,
                b: 0xff
            })
        );
        assert_eq!(parse_color("3e91ff"), None);
        assert_eq!(parse_color("#3e91f"), None);
        assert_eq!(parse_color("#zz91ff"), None);
        assert_eq!(color_text(Rgb { r: 1, g: 2, b: 255 }), "#0102ff");
    }

    #[test]
    fn points_and_boxes_are_parsed_or_refused_whole() {
        assert_eq!(
            parse_points("1,2 3.5,4"),
            Some(vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.5, y: 4.0 }])
        );
        assert_eq!(parse_points("1,2 3"), None);
        assert_eq!(parse_points("1,NaN"), None);
    }

    #[test]
    fn each_selection_outline_gives_its_box() {
        let mut outlines = QList::<QVariant>::default();
        outlines.append(QVariant::from(&QPolygonF::from(&QRectF::new(
            1.0, 2.0, 10.0, 5.0,
        ))));
        outlines.append(QVariant::from(&QString::from("not a polygon")));
        let mut triangle = QPolygonF::default();
        for (x, y) in [(0.0, 0.0), (4.0, 1.0), (2.0, 3.0)] {
            triangle.append(QPointF::new(x, y));
        }
        outlines.append(QVariant::from(&triangle));
        assert_eq!(
            outline_boxes(&outlines),
            vec![
                Rect::new(1.0, 2.0, 11.0, 7.0),
                Rect::new(0.0, 0.0, 4.0, 3.0)
            ]
        );
    }

    #[test]
    fn a_copy_registered_for_a_window_already_gone_is_dropped() {
        let dropped = std::cell::Cell::new(0);
        settle(false, true, || dropped.set(dropped.get() + 1));
        assert_eq!(dropped.get(), 1, "nobody else will take it out");
        settle(true, true, || dropped.set(dropped.get() + 1));
        assert_eq!(dropped.get(), 1, "the window takes it when it ends");
        settle(false, false, || dropped.set(dropped.get() + 1));
        assert_eq!(dropped.get(), 1, "nothing was registered");
    }

    #[test]
    fn a_window_going_away_drops_the_copy_shown_and_the_one_beginning() {
        let copy = |revision| ("/doc.pdf".to_owned(), revision);
        assert_eq!(orphans(None, None), Vec::new());
        assert_eq!(orphans(Some(copy(3)), None), vec![copy(3)]);
        assert_eq!(
            orphans(None, Some(copy(4))),
            vec![copy(4)],
            "destroyed between the worker's queue and the Qt thread's call"
        );
        assert_eq!(
            orphans(Some(copy(3)), Some(copy(4))),
            vec![copy(3), copy(4)]
        );
        assert_eq!(orphans(Some(copy(4)), Some(copy(4))), vec![copy(4)], "once");
    }

    #[test]
    fn json_strings_escape_what_json_needs() {
        assert_eq!(
            json_string("a\"b\\c\nd\u{1}é"),
            "\"a\\\"b\\\\c\\nd\\u0001é\""
        );
    }

    #[test]
    fn the_model_is_published_in_displayed_points() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/annotated.pdf");
        let base = Base::read(&path).expect("readable");
        let model = json_model(&read_annotations(&path).expect("readable"), &base);
        // The FreeText at (300, 700)-(520, 760) on an A4 page.
        assert!(
            model.starts_with(
                "[{\"id\":\"o6.0\",\"kind\":\"text\",\"page\":0,\"x\":300,\"y\":82,\"w\":220,\"h\":60,\"color\":\"#000099\",\"text\":\"Hello Calcita\",\"size\":16,\"rich\":false,\"laidOut\":false}"
            ),
            "{model}"
        );
        assert!(model.contains("\"kind\":\"highlight\""), "{model}");
        assert!(model.contains("\"kind\":\"ink\""), "{model}");
        assert!(!model.contains("Link"), "only the editable kinds");
        let mut edits = calcita_core::annot::edit::EditSet::new();
        let added = calcita_core::annot::edit::text_box(
            base.boxes[0],
            0,
            Rect::new(72.0, 72.0, 272.0, 80.0),
            "nuevo",
            12.0,
            Rgb::BLACK,
        );
        edits
            .push(&base, calcita_core::annot::edit::Edit::Add(added))
            .expect("added");
        let model = json_model(&edits.apply(&base), &base);
        assert!(
            model.contains("\"text\":\"nuevo\",\"size\":12,\"rich\":false,\"laidOut\":true"),
            "Calcita lays out its own box: {model}"
        );
        let turned = Base {
            annotations: Vec::new(),
            boxes: vec![PageBox {
                x0: 0.0,
                y0: 0.0,
                w: 10.0,
                h: 10.0,
                rotate: Rotation::R90,
            }],
        };
        assert_eq!(json_model(&[], &turned), "[]");
    }
}
