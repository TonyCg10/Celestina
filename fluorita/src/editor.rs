//! The Qt half of editing one picture.
//!
//! What an edit *is* lives in `fluorita-core`'s [`EditDocument`]; what it
//! *costs* — reading, drawing, writing, the Trash — lives in
//! `fluorita-engine`. This file moves values between them and QML under the
//! rules the rest of the application already follows:
//!
//! - **The GUI thread never encodes.** Composing a stack is arithmetic and
//!   happens here; rasterising and writing run on an owned worker and the
//!   result arrives through the queue.
//! - **Nothing is published that the engine did not confirm.** A save is a
//!   pending request until the bytes are on disk; the item is not renamed, the
//!   original is not reported as trashed and the recipe is not remembered
//!   before that.
//! - **Coordinates are image coordinates.** Everything crossing this seam is in
//!   pixels of the current canvas, so a stroke drawn on a scaled display lands
//!   where it was drawn. QML converts from its own view once, on the way in.
//! - **A path is a key, not text** (ADR 0008).
//!
//! The two save outcomes are the product's whole promise about editing, and
//! they are not symmetric: a copy keeps the base, so its recipe is remembered
//! and it reopens with its objects; a replacement removes the base, so the
//! result is flattened and its recipe is forgotten.

use std::path::PathBuf;
use std::thread::JoinHandle;

use celestina_core::{pathkey, CancellationToken};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use fluorita_core::{
    Annotation, Area, Axis, Canvas, EditCapabilities, EditClass, EditDocument, EditLimits,
    EditRejected, Ink, MediaKind, ObjectId, Point, Quarter, Redaction, SaveChoice, ShapeKind,
    Transform,
};
use fluorita_engine::{edit_store, DesktopTrash, SaveRequest, Saved};

use crate::image;
use crate::rasteriser::ToolkitRasteriser;

mod copy;

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
        /// The edit surface QML binds to.
        #[qobject]
        #[qml_element]
        /// True while a picture is open for editing.
        #[qproperty(bool, open)]
        /// The path key of the picture being edited. The preview renders this
        /// file and draws the objects over it.
        #[qproperty(QString, key)]
        /// The canvas the edit currently produces, in pixels. QML scales its
        /// view to this and converts every pointer position back into it.
        #[qproperty(i32, canvas_width)]
        #[qproperty(i32, canvas_height)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        /// True once anything has been done that a save would write.
        #[qproperty(bool, edited)]
        /// Whether saving would reorder the original's bytes rather than
        /// produce a new image. The interface must show these differently.
        #[qproperty(bool, lossless)]
        /// That same fact as the words a person reads.
        #[qproperty(QString, class_label)]
        /// Set when the result cannot be written in the original's format, so
        /// the name on disk will change. Empty when it will not.
        #[qproperty(QString, container_notice)]
        /// True while a save is in flight. Every verb is refused meanwhile.
        #[qproperty(bool, saving)]
        /// What happened to the last save, or empty.
        #[qproperty(QString, notice)]
        /// The picture on disk, in its own pixels, so a preview can place the
        /// visible part of it without measuring the file a second time.
        #[qproperty(i32, base_width)]
        #[qproperty(i32, base_height)]
        /// The picture on disk, as a `file://` URL for the toolkit. Empty when
        /// nothing is open, or when the name cannot be spelled as a URL.
        #[qproperty(QString, source_url)]
        /// What a preview has to draw: the part of that file still visible, as
        /// `x,y,width,height` in its own pixels, and the quarter turns and
        /// mirror to draw it with. Computed by the document, because a surface
        /// that worked it out again would be a second implementation of the
        /// mapping — and the one that disagrees is the one on screen.
        #[qproperty(QString, preview_source)]
        #[qproperty(i32, preview_quarters)]
        #[qproperty(bool, preview_mirrored)]
        /// The selected object's handle, or `0` for none.
        #[qproperty(i32, selected)]
        /// Bumped once after every object list is in place, for the same
        /// reason the library bumps its own: QML must rebuild from one signal
        /// rather than from half-published columns.
        #[qproperty(i32, revision)]
        /// The objects on the canvas, index-aligned and in drawing order.
        /// Handles as text, one of `text`/`stroke`/`line`/`shape`/`highlight`/
        /// `redact`, the geometry as comma-separated canvas pixels, the ink as
        /// `#rrggbbaa`, the stroke width, and what is left: the words for a
        /// text, `rect`/`ellipse` for a shape, `blur`/`pixelate` for a
        /// redaction.
        #[qproperty(QStringList, object_ids)]
        #[qproperty(QStringList, object_kinds)]
        #[qproperty(QStringList, object_geometry)]
        #[qproperty(QStringList, object_inks)]
        #[qproperty(QStringList, object_widths)]
        #[qproperty(QStringList, object_details)]
        type FluoritaEditor = super::EditorRust;

        /// Opens a picture for editing. Takes a row's path key; a value that is
        /// not one, an item that is not an editable image, and one past the
        /// viewing budget are all refused with a stated reason.
        #[qinvokable]
        fn open_item(self: Pin<&mut FluoritaEditor>, key: &QString);

        /// Whether this item can be edited at all, asked before anything is
        /// offered for it.
        ///
        /// The one owner of that answer is the capability matrix; a surface
        /// that worked it out from a file name would be a second one, and the
        /// two would disagree the day a format changed sides.
        #[qinvokable]
        fn admits(self: &FluoritaEditor, key: &QString) -> bool;

        /// Closes the editor, discarding whatever was not saved.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaEditor>);

        /// Turns the canvas a quarter turn, clockwise or not.
        #[qinvokable]
        fn rotate(self: Pin<&mut FluoritaEditor>, clockwise: bool);

        /// Mirrors the canvas across one axis.
        #[qinvokable]
        fn flip(self: Pin<&mut FluoritaEditor>, horizontal: bool);

        /// Keeps this area of the canvas and discards the rest. `area` is
        /// `x,y,width,height` in canvas pixels — the same spelling the object
        /// rows are published in, so the surface reads and writes one shape.
        #[qinvokable]
        fn crop(self: Pin<&mut FluoritaEditor>, area: &QString);

        /// Rescales the whole canvas.
        #[qinvokable]
        fn resize(self: Pin<&mut FluoritaEditor>, width: i32, height: i32);

        /// Places words on the canvas. `backdrop` may be empty for none.
        #[qinvokable]
        fn add_text(
            self: Pin<&mut FluoritaEditor>,
            area: &QString,
            size: f32,
            ink: &QString,
            backdrop: &QString,
            text: &QString,
        );

        /// Places a freehand stroke. `points` is `x,y;x,y;…` in canvas pixels.
        #[qinvokable]
        fn add_stroke(self: Pin<&mut FluoritaEditor>, points: &QString, width: f32, ink: &QString);

        /// Places a straight line, with an arrow head at the far end or not.
        /// `ends` is `x1,y1,x2,y2` in canvas pixels.
        #[qinvokable]
        fn add_line(
            self: Pin<&mut FluoritaEditor>,
            ends: &QString,
            width: f32,
            ink: &QString,
            arrow: bool,
        );

        /// Places a rectangle or an ellipse. `fill` may be empty for an
        /// outline.
        #[qinvokable]
        fn add_shape(
            self: Pin<&mut FluoritaEditor>,
            ellipse: bool,
            area: &QString,
            stroke: f32,
            ink: &QString,
            fill: &QString,
        );

        /// Places a translucent wash.
        #[qinvokable]
        fn add_highlight(self: Pin<&mut FluoritaEditor>, area: &QString, ink: &QString);

        /// Covers an area so what is under it cannot be recovered from the
        /// result.
        #[qinvokable]
        fn add_redaction(self: Pin<&mut FluoritaEditor>, area: &QString, blur: bool);

        /// Selects one object, or `0` to select none.
        #[qinvokable]
        fn select_object(self: Pin<&mut FluoritaEditor>, id: i32);

        /// Moves one object by a delta in canvas pixels.
        #[qinvokable]
        fn move_object(self: Pin<&mut FluoritaEditor>, id: i32, dx: f32, dy: f32);

        /// Resizes one object's box, given as `width,height` in canvas pixels.
        /// A stroke has no box and is left as it is.
        #[qinvokable]
        fn resize_object(self: Pin<&mut FluoritaEditor>, id: i32, size: &QString);

        /// Takes one object off the canvas.
        #[qinvokable]
        fn remove_object(self: Pin<&mut FluoritaEditor>, id: i32);

        #[qinvokable]
        fn undo(self: Pin<&mut FluoritaEditor>);

        #[qinvokable]
        fn redo(self: Pin<&mut FluoritaEditor>);

        /// Writes the edit. `replace` sends the original to the Trash and
        /// flattens the result; otherwise a copy lands beside it and stays
        /// reopenable. Returns at once: the work runs on a worker.
        #[qinvokable]
        fn save(self: Pin<&mut FluoritaEditor>, replace: bool);
    }

    impl cxx_qt::Threading for FluoritaEditor {}
}

pub struct EditorRust {
    open: bool,
    key: QString,
    canvas_width: i32,
    canvas_height: i32,
    can_undo: bool,
    can_redo: bool,
    edited: bool,
    lossless: bool,
    class_label: QString,
    container_notice: QString,
    saving: bool,
    notice: QString,
    base_width: i32,
    base_height: i32,
    source_url: QString,
    preview_source: QString,
    preview_quarters: i32,
    preview_mirrored: bool,
    selected: i32,
    revision: i32,

    object_ids: QStringList,
    object_kinds: QStringList,
    object_geometry: QStringList,
    object_inks: QStringList,
    object_widths: QStringList,
    object_details: QStringList,

    /// The picture being edited, byte-exact. Never published: the key is.
    source: Option<PathBuf>,
    document: Option<EditDocument>,
    capabilities: Option<EditCapabilities>,
    worker: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl Default for EditorRust {
    fn default() -> Self {
        Self {
            open: false,
            key: QString::default(),
            canvas_width: 0,
            canvas_height: 0,
            can_undo: false,
            can_redo: false,
            edited: false,
            lossless: false,
            class_label: QString::default(),
            container_notice: QString::default(),
            saving: false,
            notice: QString::default(),
            base_width: 0,
            base_height: 0,
            source_url: QString::default(),
            preview_source: QString::default(),
            preview_quarters: 0,
            preview_mirrored: false,
            selected: 0,
            revision: 0,
            object_ids: QStringList::default(),
            object_kinds: QStringList::default(),
            object_geometry: QStringList::default(),
            object_inks: QStringList::default(),
            object_widths: QStringList::default(),
            object_details: QStringList::default(),
            source: None,
            document: None,
            capabilities: None,
            worker: None,
            cancellation: CancellationToken::new(),
        }
    }
}

impl Drop for EditorRust {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl qobject::FluoritaEditor {
    pub fn open_item(mut self: std::pin::Pin<&mut Self>, key: &QString) {
        if *self.saving() {
            return;
        }
        let Ok(path) = pathkey::decode(&key.to_string()) else {
            self.as_mut().refuse(copy::UNREADABLE_KEY);
            return;
        };
        let Some(kind) = MediaKind::classify_path(&path) else {
            self.as_mut().refuse(copy::NOT_EDITABLE);
            return;
        };
        let capabilities = EditCapabilities::of(kind, &path);
        if !capabilities.is_editable() {
            self.as_mut().refuse(copy::NOT_EDITABLE);
            return;
        }

        // The same probe the viewer uses, so the canvas is the picture as it is
        // shown — orientation applied — and the budget is judged on what would
        // actually be allocated.
        let probed = crate::player::qobject::probe_image(key);
        let bytes = std::fs::metadata(&path).map(|it| it.len()).unwrap_or(0);
        let dimensions = (probed.width() > 0 && probed.height() > 0).then(|| {
            (
                u32::try_from(probed.width()).unwrap_or(u32::MAX),
                u32::try_from(probed.height()).unwrap_or(u32::MAX),
            )
        });
        let decision = image::ImageDecision::judge(bytes, dimensions);
        let Some(canvas) = (match decision {
            image::ImageDecision::Show { width, height } => Canvas::new(width, height),
            image::ImageDecision::Unreadable | image::ImageDecision::TooLarge { .. } => None,
        }) else {
            self.as_mut().refuse(&decision.message());
            return;
        };

        let document = EditDocument::new(canvas, EditLimits::new(image::MAX_PIXELS));
        {
            let mut editor = self.as_mut().rust_mut();
            editor.source = Some(path);
            editor.document = Some(document);
            editor.capabilities = Some(capabilities);
            editor.selected = 0;
        }
        let url = self
            .rust()
            .source
            .as_deref()
            .and_then(fluorita_core::file_uri)
            .unwrap_or_default();
        self.as_mut().set_source_url(QString::from(&url));
        self.as_mut().set_key(key.clone());
        self.as_mut().set_open(true);
        self.as_mut().set_notice(QString::default());
        self.publish();
    }

    #[must_use]
    pub fn admits(&self, key: &QString) -> bool {
        let Ok(path) = pathkey::decode(&key.to_string()) else {
            return false;
        };
        MediaKind::classify_path(&path)
            .is_some_and(|kind| EditCapabilities::of(kind, &path).is_editable())
    }

    pub fn close(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().cancel_worker();
        {
            let mut editor = self.as_mut().rust_mut();
            editor.source = None;
            editor.document = None;
            editor.capabilities = None;
            editor.selected = 0;
        }
        self.as_mut().set_open(false);
        self.as_mut().set_key(QString::default());
        self.as_mut().set_source_url(QString::default());
        self.as_mut().set_saving(false);
        self.publish();
    }

    pub fn rotate(mut self: std::pin::Pin<&mut Self>, clockwise: bool) {
        let quarter = if clockwise {
            Quarter::Clockwise
        } else {
            Quarter::CounterClockwise
        };
        self.as_mut().apply(Transform::Rotate(quarter));
    }

    pub fn flip(mut self: std::pin::Pin<&mut Self>, horizontal: bool) {
        let axis = if horizontal {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        self.as_mut().apply(Transform::Flip(axis));
    }

    pub fn crop(mut self: std::pin::Pin<&mut Self>, area: &QString) {
        let Some(area) = parse_area(area) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        self.as_mut().apply(Transform::Crop(area));
    }

    pub fn resize(mut self: std::pin::Pin<&mut Self>, width: i32, height: i32) {
        let Some(canvas) = Canvas::new(
            u32::try_from(width).unwrap_or(0),
            u32::try_from(height).unwrap_or(0),
        ) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::EmptyCanvas));
            return;
        };
        self.as_mut().apply(Transform::Resize(canvas));
    }

    pub fn add_text(
        mut self: std::pin::Pin<&mut Self>,
        area: &QString,
        size: f32,
        ink: &QString,
        backdrop: &QString,
        text: &QString,
    ) {
        let Some(area) = parse_area(area) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        let annotation = Annotation::Text {
            area,
            text: text.to_string(),
            size,
            ink: parse_ink(ink).unwrap_or(FALLBACK_INK),
            backdrop: parse_ink(backdrop),
            quarters: 0,
        };
        self.as_mut().place(annotation);
    }

    pub fn add_stroke(
        mut self: std::pin::Pin<&mut Self>,
        points: &QString,
        width: f32,
        ink: &QString,
    ) {
        let annotation = Annotation::Stroke {
            points: parse_points(&points.to_string()),
            width,
            ink: parse_ink(ink).unwrap_or(FALLBACK_INK),
        };
        self.as_mut().place(annotation);
    }

    pub fn add_line(
        mut self: std::pin::Pin<&mut Self>,
        ends: &QString,
        width: f32,
        ink: &QString,
        arrow: bool,
    ) {
        let Some([x1, y1, x2, y2]) = parse_numbers(ends) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        let annotation = Annotation::Line {
            from: Point::new(x1, y1),
            to: Point::new(x2, y2),
            width,
            ink: parse_ink(ink).unwrap_or(FALLBACK_INK),
            arrow,
        };
        self.as_mut().place(annotation);
    }

    pub fn add_shape(
        mut self: std::pin::Pin<&mut Self>,
        ellipse: bool,
        area: &QString,
        stroke: f32,
        ink: &QString,
        fill: &QString,
    ) {
        let Some(area) = parse_area(area) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        let annotation = Annotation::Shape {
            kind: if ellipse {
                ShapeKind::Ellipse
            } else {
                ShapeKind::Rectangle
            },
            area,
            width: stroke,
            ink: parse_ink(ink).unwrap_or(FALLBACK_INK),
            fill: parse_ink(fill),
        };
        self.as_mut().place(annotation);
    }

    pub fn add_highlight(mut self: std::pin::Pin<&mut Self>, area: &QString, ink: &QString) {
        let Some(area) = parse_area(area) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        let annotation = Annotation::Highlight {
            area,
            ink: parse_ink(ink).unwrap_or(FALLBACK_INK),
        };
        self.as_mut().place(annotation);
    }

    pub fn add_redaction(mut self: std::pin::Pin<&mut Self>, area: &QString, blur: bool) {
        let Some(area) = parse_area(area) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        let annotation = Annotation::Redact {
            area,
            style: if blur {
                Redaction::Blur
            } else {
                Redaction::Pixelate
            },
        };
        self.as_mut().place(annotation);
    }

    pub fn select_object(mut self: std::pin::Pin<&mut Self>, id: i32) {
        let exists = id == 0
            || self.rust().document.as_ref().is_some_and(|document| {
                document
                    .objects()
                    .iter()
                    .any(|(handle, _)| handle.value() == id as u64)
            });
        if exists {
            self.as_mut().set_selected(id);
        }
    }

    pub fn move_object(mut self: std::pin::Pin<&mut Self>, id: i32, dx: f32, dy: f32) {
        self.as_mut().rewrite(id, |annotation| {
            shift(annotation, dx, dy);
        });
    }

    pub fn resize_object(mut self: std::pin::Pin<&mut Self>, id: i32, size: &QString) {
        let Some([width, height]) = parse_numbers(size) else {
            self.as_mut()
                .refuse(copy::rejected(EditRejected::InvalidGeometry));
            return;
        };
        self.as_mut().rewrite(id, |annotation| {
            reshape(annotation, width, height);
        });
    }

    pub fn remove_object(mut self: std::pin::Pin<&mut Self>, id: i32) {
        if *self.saving() {
            return;
        }
        let outcome = self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .map(|document| document.remove(ObjectId::from_value(id.max(0) as u64)));
        match outcome {
            Some(Err(rejected)) => self.as_mut().refuse(copy::rejected(rejected)),
            _ => {
                if *self.selected() == id {
                    self.as_mut().set_selected(0);
                }
                self.publish();
            }
        }
    }

    pub fn undo(mut self: std::pin::Pin<&mut Self>) {
        if *self.saving() {
            return;
        }
        if self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .is_some_and(EditDocument::undo)
        {
            self.as_mut().set_selected(0);
            self.publish();
        }
    }

    pub fn redo(mut self: std::pin::Pin<&mut Self>) {
        if *self.saving() {
            return;
        }
        if self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .is_some_and(EditDocument::redo)
        {
            self.as_mut().set_selected(0);
            self.publish();
        }
    }

    pub fn save(mut self: std::pin::Pin<&mut Self>, replace: bool) {
        if *self.saving() || !*self.open() {
            return;
        }
        let (Some(source), Some(document), Some(capabilities)) = (
            self.rust().source.clone(),
            self.rust().document.clone(),
            self.rust().capabilities,
        ) else {
            return;
        };
        if !document.is_edited() {
            self.as_mut()
                .set_notice(QString::from(copy::NOTHING_TO_SAVE));
            return;
        }
        let Some(format) = capabilities.output_format() else {
            self.as_mut().refuse(copy::NOT_EDITABLE);
            return;
        };

        self.as_mut().cancel_worker();
        self.as_mut().set_saving(true);
        self.as_mut().set_notice(QString::default());

        let choice = if replace {
            SaveChoice::Replace
        } else {
            SaveChoice::Copy
        };
        let cancellation = self.rust().cancellation.clone();
        let qt_thread = self.qt_thread();
        let worker = std::thread::spawn(move || {
            let composition = document.composition();
            let request = SaveRequest {
                source: &source,
                composition: &composition,
                orientation: document.orientation_only(),
                format,
                choice,
                copy_marker: copy::COPY_MARKER,
            };
            let outcome = fluorita_engine::save_edit(
                &request,
                &ToolkitRasteriser,
                &DesktopTrash,
                &cancellation,
            );
            let remembered = match (&outcome, choice) {
                (Ok(saved), SaveChoice::Copy) => {
                    remember(&source, saved, document.base(), &composition)
                }
                _ => false,
            };
            let message = match &outcome {
                Ok(saved) => copy::saved(saved, remembered),
                Err(error) => copy::failure(error),
            };
            let landed = outcome.is_ok();
            let _ = qt_thread.queue(move |mut editor| {
                editor.as_mut().set_saving(false);
                editor.as_mut().set_notice(QString::from(&message));
                if landed {
                    // A replacement flattened the result and the copy is a
                    // different file: either way the document that produced it
                    // is no longer the document for what is now on disk.
                    editor.close();
                }
            });
        });
        self.as_mut().rust_mut().worker = Some(worker);
    }
}

/// Everything the object lists and the derived properties are rebuilt from.
impl qobject::FluoritaEditor {
    fn apply(mut self: std::pin::Pin<&mut Self>, transform: Transform) {
        if *self.saving() {
            return;
        }
        let Some(capabilities) = self.rust().capabilities else {
            return;
        };
        let outcome = self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .map(|document| document.transform(transform, &capabilities));
        match outcome {
            Some(Err(rejected)) => self.as_mut().refuse(copy::rejected(rejected)),
            Some(Ok(())) => {
                self.as_mut().set_selected(0);
                self.publish();
            }
            None => {}
        }
    }

    fn place(mut self: std::pin::Pin<&mut Self>, annotation: Annotation) {
        if *self.saving() {
            return;
        }
        let Some(capabilities) = self.rust().capabilities else {
            return;
        };
        let outcome = self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .map(|document| document.annotate(annotation, &capabilities));
        match outcome {
            Some(Err(rejected)) => self.as_mut().refuse(copy::rejected(rejected)),
            Some(Ok(id)) => {
                self.as_mut()
                    .set_selected(i32::try_from(id.value()).unwrap_or(0));
                self.publish();
            }
            None => {}
        }
    }

    fn rewrite(mut self: std::pin::Pin<&mut Self>, id: i32, change: impl FnOnce(&mut Annotation)) {
        if *self.saving() {
            return;
        }
        let handle = ObjectId::from_value(id.max(0) as u64);
        let Some(mut annotation) = self.rust().document.as_ref().and_then(|document| {
            document
                .objects()
                .iter()
                .find(|(candidate, _)| *candidate == handle)
                .map(|(_, annotation)| annotation.clone())
        }) else {
            return;
        };
        change(&mut annotation);
        let outcome = self
            .as_mut()
            .rust_mut()
            .document
            .as_mut()
            .map(|document| document.update(handle, annotation));
        match outcome {
            Some(Err(rejected)) => self.as_mut().refuse(copy::rejected(rejected)),
            _ => self.publish(),
        }
    }

    fn refuse(mut self: std::pin::Pin<&mut Self>, message: &str) {
        self.as_mut().set_notice(QString::from(message));
    }

    fn cancel_worker(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().cancellation.cancel();
        let worker = self.as_mut().rust_mut().worker.take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        self.as_mut().rust_mut().cancellation = CancellationToken::new();
    }

    fn publish(mut self: std::pin::Pin<&mut Self>) {
        let preview = self.rust().document.as_ref().map(EditDocument::preview);
        let (canvas, can_undo, can_redo, edited, class, objects) = match (
            self.rust().document.as_ref(),
            self.rust().capabilities.as_ref(),
        ) {
            (Some(document), Some(capabilities)) => (
                Some(document.canvas()),
                document.can_undo(),
                document.can_redo(),
                document.is_edited(),
                document.class(capabilities),
                document
                    .objects()
                    .iter()
                    .map(|(id, annotation)| (*id, annotation.clone()))
                    .collect::<Vec<_>>(),
            ),
            // Nothing open: the canvas is published as no size at all rather
            // than as a placeholder one, which is what a surface binds its
            // "there is nothing here" on.
            _ => (None, false, false, false, EditClass::Raster, Vec::new()),
        };

        let mut ids = QStringList::default();
        let mut kinds = QStringList::default();
        let mut geometry = QStringList::default();
        let mut inks = QStringList::default();
        let mut widths = QStringList::default();
        let mut details = QStringList::default();
        for (id, annotation) in &objects {
            let row = describe(annotation);
            ids.append(QString::from(&id.value().to_string()));
            kinds.append(QString::from(row.kind));
            geometry.append(QString::from(&row.geometry));
            inks.append(QString::from(&row.ink));
            widths.append(QString::from(&row.width));
            details.append(QString::from(&row.detail));
        }

        let open = *self.open();
        let container_notice = if open {
            self.rust()
                .capabilities
                .filter(|capabilities| capabilities.changes_container())
                .and_then(|capabilities| capabilities.output_format())
                .map(|format| copy::container_change(format.extension()))
                .unwrap_or_default()
        } else {
            String::new()
        };

        self.as_mut().set_canvas_width(canvas.map_or(0, |canvas| {
            i32::try_from(canvas.width()).unwrap_or(i32::MAX)
        }));
        self.as_mut().set_canvas_height(canvas.map_or(0, |canvas| {
            i32::try_from(canvas.height()).unwrap_or(i32::MAX)
        }));
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_can_redo(can_redo);
        self.as_mut().set_edited(edited);
        self.as_mut().set_lossless(class.preserves_original_bytes());
        self.as_mut()
            .set_class_label(QString::from(copy::class_label(class)));
        self.as_mut()
            .set_container_notice(QString::from(&container_notice));
        let (preview_source, quarters, mirrored) = preview.map_or_else(
            || (String::new(), 0, false),
            |preview| {
                (
                    write_area(preview.source),
                    i32::from(preview.orientation.quarters),
                    preview.orientation.mirrored,
                )
            },
        );
        self.as_mut()
            .set_preview_source(QString::from(&preview_source));
        self.as_mut().set_preview_quarters(quarters);
        self.as_mut().set_preview_mirrored(mirrored);
        let base = self.rust().document.as_ref().map(EditDocument::base);
        self.as_mut()
            .set_base_width(base.map_or(0, |base| i32::try_from(base.width()).unwrap_or(i32::MAX)));
        self.as_mut().set_base_height(
            base.map_or(0, |base| i32::try_from(base.height()).unwrap_or(i32::MAX)),
        );
        self.as_mut().set_object_ids(ids);
        self.as_mut().set_object_kinds(kinds);
        self.as_mut().set_object_geometry(geometry);
        self.as_mut().set_object_inks(inks);
        self.as_mut().set_object_widths(widths);
        self.as_mut().set_object_details(details);
        let revision = self.revision().wrapping_add(1);
        self.as_mut().set_revision(revision);
    }
}

/// Writes the recipe down, so the copy just written reopens with its objects.
///
/// Returns whether it was remembered. A failure is not a failed save — the
/// picture is on disk either way — so it is reported as "saved, but it will
/// reopen flat" rather than as an error.
fn remember(
    source: &std::path::Path,
    saved: &Saved,
    base: Canvas,
    composition: &fluorita_core::Composition,
) -> bool {
    let Some(store_path) = edit_store::default_path() else {
        return false;
    };
    let (Ok(base_metadata), Ok(result_metadata)) =
        (std::fs::metadata(source), std::fs::metadata(&saved.written))
    else {
        return false;
    };
    let Some(identity) = identity_of(&base_metadata) else {
        return false;
    };
    let Some(result_id) = media_id_of(&result_metadata) else {
        return false;
    };

    let mut store = edit_store::load(&store_path)
        .map(|loaded| loaded.store)
        .unwrap_or_default();
    store.remember(
        result_id,
        edit_store::StoredEdit {
            base: source.to_path_buf(),
            base_identity: identity,
            base_canvas: base,
            transforms: composition.transforms.clone(),
            objects: composition.objects.clone(),
        },
    );
    edit_store::save(&store_path, &store).is_ok()
}

fn identity_of(metadata: &std::fs::Metadata) -> Option<fluorita_core::SourceIdentity> {
    Some(fluorita_core::SourceIdentity::new(
        metadata.len(),
        metadata.modified().ok()?,
    ))
}

fn media_id_of(metadata: &std::fs::Metadata) -> Option<fluorita_core::MediaId> {
    use std::os::unix::fs::MetadataExt;
    Some(fluorita_core::MediaId::filesystem(
        metadata.dev(),
        metadata.ino(),
    ))
}

/// The ink used when a surface hands over a colour that is not one. Opaque
/// white reads on almost any photograph, and an invisible annotation would be
/// worse than a wrong one.
const FALLBACK_INK: Ink = Ink::new(255, 255, 255, 255);

fn parse_ink(value: &QString) -> Option<Ink> {
    let text = value.to_string();
    let digits = text.strip_prefix('#')?;
    let byte = |index: usize| u8::from_str_radix(digits.get(index..index + 2)?, 16).ok();
    match digits.len() {
        6 => Some(Ink::new(byte(0)?, byte(2)?, byte(4)?, 255)),
        8 => Some(Ink::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

fn write_ink(ink: Ink) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        ink.red, ink.green, ink.blue, ink.alpha
    )
}

/// `x,y,width,height` in canvas pixels, the same spelling the object rows are
/// published in. One shape crossing the seam in both directions rather than two.
fn parse_area(value: &QString) -> Option<Area> {
    let [x, y, width, height] = parse_numbers(value)?;
    (width > 0.0 && height > 0.0).then_some(Area::new(Point::new(x, y), width, height))
}

/// Exactly `N` finite numbers, or nothing. A surface that sends a short list or
/// a word is refused rather than having the difference filled in.
fn parse_numbers<const N: usize>(value: &QString) -> Option<[f32; N]> {
    let text = value.to_string();
    let mut values = [0.0f32; N];
    let mut fields = text.split(',');
    for slot in &mut values {
        let parsed: f32 = fields.next()?.trim().parse().ok()?;
        if !parsed.is_finite() {
            return None;
        }
        *slot = parsed;
    }
    fields.next().is_none().then_some(values)
}

fn parse_points(text: &str) -> Vec<Point> {
    text.split(';')
        .filter_map(|pair| {
            let (x, y) = pair.split_once(',')?;
            Some(Point::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

struct Row {
    kind: &'static str,
    geometry: String,
    ink: String,
    width: String,
    detail: String,
}

fn describe(annotation: &Annotation) -> Row {
    match annotation {
        Annotation::Text {
            area,
            text,
            size,
            ink,
            quarters,
            ..
        } => Row {
            kind: "text",
            geometry: write_area(*area),
            ink: write_ink(*ink),
            width: size.to_string(),
            detail: format!("{quarters}\u{1f}{text}"),
        },
        Annotation::Stroke { points, width, ink } => Row {
            kind: "stroke",
            geometry: points
                .iter()
                .map(|point| format!("{},{}", point.x, point.y))
                .collect::<Vec<_>>()
                .join(";"),
            ink: write_ink(*ink),
            width: width.to_string(),
            detail: String::new(),
        },
        Annotation::Line {
            from,
            to,
            width,
            ink,
            arrow,
        } => Row {
            kind: "line",
            geometry: format!("{},{},{},{}", from.x, from.y, to.x, to.y),
            ink: write_ink(*ink),
            width: width.to_string(),
            detail: if *arrow { "arrow" } else { "plain" }.to_owned(),
        },
        Annotation::Shape {
            kind,
            area,
            width,
            ink,
            fill,
        } => Row {
            kind: "shape",
            geometry: write_area(*area),
            ink: write_ink(*ink),
            width: width.to_string(),
            detail: format!(
                "{}\u{1f}{}",
                match kind {
                    ShapeKind::Rectangle => "rect",
                    ShapeKind::Ellipse => "ellipse",
                },
                fill.map(write_ink).unwrap_or_default()
            ),
        },
        Annotation::Highlight { area, ink } => Row {
            kind: "highlight",
            geometry: write_area(*area),
            ink: write_ink(*ink),
            width: String::new(),
            detail: String::new(),
        },
        Annotation::Redact { area, style } => Row {
            kind: "redact",
            geometry: write_area(*area),
            ink: String::new(),
            width: String::new(),
            detail: match style {
                Redaction::Pixelate => "pixelate",
                Redaction::Blur => "blur",
            }
            .to_owned(),
        },
    }
}

fn write_area(area: Area) -> String {
    format!(
        "{},{},{},{}",
        area.origin.x, area.origin.y, area.width, area.height
    )
}

fn shift(annotation: &mut Annotation, dx: f32, dy: f32) {
    match annotation {
        Annotation::Text { area, .. }
        | Annotation::Shape { area, .. }
        | Annotation::Highlight { area, .. }
        | Annotation::Redact { area, .. } => {
            area.origin = Point::new(area.origin.x + dx, area.origin.y + dy);
        }
        Annotation::Line { from, to, .. } => {
            *from = Point::new(from.x + dx, from.y + dy);
            *to = Point::new(to.x + dx, to.y + dy);
        }
        Annotation::Stroke { points, .. } => {
            for point in points.iter_mut() {
                *point = Point::new(point.x + dx, point.y + dy);
            }
        }
    }
}

fn reshape(annotation: &mut Annotation, width: f32, height: f32) {
    match annotation {
        Annotation::Text { area, .. }
        | Annotation::Shape { area, .. }
        | Annotation::Highlight { area, .. }
        | Annotation::Redact { area, .. } => {
            area.width = width;
            area.height = height;
        }
        // A line's shape is its two ends and a stroke's is its points; neither
        // has a box to stretch, so a request to stretch one is ignored rather
        // than approximated into something the person did not draw.
        Annotation::Line { .. } | Annotation::Stroke { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_ink, parse_points, reshape, shift, write_ink};
    use fluorita_core::{Annotation, Area, Ink, Point, Redaction};

    fn redaction() -> Annotation {
        Annotation::Redact {
            area: Area::new(Point::new(10.0, 20.0), 30.0, 40.0),
            style: Redaction::Pixelate,
        }
    }

    #[test]
    fn an_ink_survives_the_seam_in_both_directions() {
        let ink = Ink::new(0x12, 0x34, 0x56, 0x78);
        assert_eq!(parse_ink(&write_ink(ink).as_str().into()), Some(ink));
        assert_eq!(
            parse_ink(&"#ff8800".into()),
            Some(Ink::new(255, 136, 0, 255)),
            "a colour with no alpha is opaque"
        );
        assert_eq!(parse_ink(&"".into()), None, "no colour is not a colour");
        assert_eq!(parse_ink(&"ff8800".into()), None);
        assert_eq!(parse_ink(&"#zz8800".into()), None);
    }

    #[test]
    fn a_stroke_arrives_as_points_and_rubbish_is_dropped_rather_than_placed() {
        assert_eq!(
            parse_points("1,2;3.5,4.5"),
            vec![Point::new(1.0, 2.0), Point::new(3.5, 4.5)]
        );
        assert!(parse_points("nonsense").is_empty());
        assert_eq!(parse_points("1,2;broken;3,4").len(), 2);
    }

    #[test]
    fn moving_an_object_moves_every_part_of_it() {
        let mut stroke = Annotation::Stroke {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            width: 2.0,
            ink: Ink::new(0, 0, 0, 255),
        };
        shift(&mut stroke, 5.0, -5.0);
        match &stroke {
            Annotation::Stroke { points, .. } => {
                assert_eq!(points[0], Point::new(5.0, -5.0));
                assert_eq!(points[1], Point::new(15.0, 5.0));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn stretching_something_with_no_box_leaves_it_alone() {
        let mut line = Annotation::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 10.0),
            width: 2.0,
            ink: Ink::new(0, 0, 0, 255),
            arrow: true,
        };
        let before = line.clone();
        reshape(&mut line, 100.0, 100.0);
        assert_eq!(line, before);

        let mut boxed = redaction();
        reshape(&mut boxed, 100.0, 50.0);
        match &boxed {
            Annotation::Redact { area, .. } => {
                assert_eq!(area.width, 100.0);
                assert_eq!(area.height, 50.0);
                assert_eq!(area.origin, Point::new(10.0, 20.0));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
