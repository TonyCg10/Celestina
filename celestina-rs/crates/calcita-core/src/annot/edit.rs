//! Edits to a document's annotations and form fields, with undo and redo.
//!
//! An edit names what changes and nothing about the file: a new annotation,
//! a move, a colour, a text box's text, a deletion, a field's value. The [`EditSet`] keeps
//! them in order; applied to the [`Base`] (the annotations as the file has
//! them, and its page boxes) they give the annotations as the person sees
//! them while editing, and [`EditSet::resolve`] tells the writer what to
//! add, change and remove. Every rule about what may change lives here:
//! only the three kinds Calcita draws, of any author; a text box's text and
//! colour only when it carries no rich text; text only in what Helvetica in
//! WinAnsi can show; a text box always as tall as its lines; only text
//! fields and checkboxes filled ([`super::form::check_fill`]).
//!
//! Geometry arrives from the page as it is shown and goes through
//! [`super::coords`] once, in the constructors below.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::appearance::{self, fit_text_rect, win_ansi};
use super::coords::{to_pdf, to_pdf_rect, PageBox};
use super::form::{check_fill, fields_of, folded, FieldId, FieldValue, FormField};
use super::model::{AnnotId, Annotation, Kind, Point, Quad, Rect, Rgb};
use super::read::{load_for_edit, model_of, AnnotError};

/// How far, in points, a text box's width or height may differ and still
/// be its size: what the round trip through the displayed page and QML's
/// three decimals leaves.
const SIZE_TOLERANCE: f64 = 0.01;

/// The most points one stroke keeps; a longer one is cut there.
pub const MAX_STROKE_POINTS: usize = 10_000;

/// One change to the annotations.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// A new annotation, with a fresh [`AnnotId::Name`] ([`new_id`]).
    Add(Annotation),
    /// The annotation now fills this rectangle (PDF user space): a text box
    /// keeps its text and is laid out again; a highlight's quadrilaterals
    /// and a drawing's strokes are carried and scaled with it.
    Move(AnnotId, Rect),
    Recolor(AnnotId, Rgb),
    /// A text box's new text.
    SetText(AnnotId, String),
    Delete(AnnotId),
    /// A form field's new value: a text field's text or a checkbox's state.
    /// The field is named by its id: two fields may share a name.
    Fill(FieldId, FieldValue),
}

/// What edits apply to: the document's annotations as read, in page and
/// `/Annots` order, each page's box, and its form's fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    pub annotations: Vec<Annotation>,
    pub boxes: Vec<PageBox>,
    pub fields: Vec<FormField>,
}

impl Base {
    /// The annotations, page boxes and form fields of the document at
    /// `path`, from one read.
    ///
    /// # Errors
    ///
    /// As [`super::read_annotations`].
    pub fn read(path: &Path) -> Result<Self, AnnotError> {
        let loaded = load_for_edit(path)?;
        let (annotations, boxes) = model_of(&loaded.document);
        let fields = fields_of(&loaded.document);
        Ok(Self {
            annotations,
            boxes,
            fields,
        })
    }

    /// The box of the 0-based `page`.
    #[must_use]
    pub fn page_box(&self, page: u32) -> Option<PageBox> {
        usize::try_from(page)
            .ok()
            .and_then(|page| self.boxes.get(page))
            .copied()
    }

    /// The field `id` names.
    #[must_use]
    pub fn field(&self, id: &FieldId) -> Option<&FormField> {
        self.fields.iter().find(|field| &field.id == id)
    }

    /// The ids of the annotations Calcita may edit (its three kinds).
    #[must_use]
    pub fn editable_ids(&self) -> Vec<AnnotId> {
        self.annotations
            .iter()
            .filter(|annotation| is_editable(annotation))
            .map(|annotation| annotation.id.clone())
            .collect()
    }
}

/// What the writer does to the file: the annotations to add, the existing
/// ones to rewrite (as they must end), the existing ones to remove, and the
/// fields whose value changed (with their new value).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    pub added: Vec<Annotation>,
    pub changed: Vec<Annotation>,
    pub deleted: Vec<AnnotId>,
    pub filled: Vec<FormField>,
}

impl Resolved {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.deleted.is_empty()
            && self.filled.is_empty()
    }
}

/// The edits made so far, in order, and those undone since the last new one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditSet {
    applied: Vec<Edit>,
    undone: Vec<Edit>,
}

/// Whether Calcita draws and edits `annotation` (a text box, a highlight
/// or a drawing, whoever wrote it).
#[must_use]
pub fn is_editable(annotation: &Annotation) -> bool {
    !matches!(annotation.kind, Kind::Other { .. })
}

impl EditSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `edit` after checking it against the annotations as they are
    /// now; a new edit forgets what was undone.
    ///
    /// # Errors
    ///
    /// [`AnnotError::NotEditable`] when it names no annotation that may
    /// change that way, [`AnnotError::Unencodable`] for text Helvetica in
    /// WinAnsi cannot show. Nothing is added then.
    ///
    /// A fill that changes a single-line text field takes its value
    /// [`folded`] (line breaks and tabs as spaces); one that gives the field
    /// the value it shows is kept as it is, so it rewrites nothing.
    pub fn push(&mut self, base: &Base, edit: Edit) -> Result<(), AnnotError> {
        let edit = match edit {
            Edit::Fill(id, value) => {
                let shown = self.fields(base).into_iter().find(|field| field.id == id);
                let value = match shown {
                    Some(field) if field.current().as_ref() != Some(&value) => {
                        folded(&field, value)
                    }
                    _ => value,
                };
                Edit::Fill(id, value)
            }
            other => other,
        };
        let mut shown = self.apply(base);
        apply_one(&mut shown, base, &edit)?;
        self.applied.push(edit);
        self.undone.clear();
        Ok(())
    }

    /// Takes back the last edit; false when there is none.
    pub fn undo(&mut self) -> bool {
        match self.applied.pop() {
            Some(edit) => {
                self.undone.push(edit);
                true
            }
            None => false,
        }
    }

    /// Makes the last undone edit again; false when there is none.
    pub fn redo(&mut self) -> bool {
        match self.undone.pop() {
            Some(edit) => {
                self.applied.push(edit);
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.applied.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// No edit is in effect (some may wait to be redone).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty()
    }

    /// The edits in effect, in order.
    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        &self.applied
    }

    /// The annotations as the person sees them: the base with every edit in
    /// effect applied in order. An edit that no longer applies (the base
    /// changed under it) is skipped.
    #[must_use]
    pub fn apply(&self, base: &Base) -> Vec<Annotation> {
        let mut shown = base.annotations.clone();
        for edit in &self.applied {
            let _ = apply_one(&mut shown, base, edit);
        }
        shown
    }

    /// The form's fields as the person sees them: the base's with every
    /// fill in effect applied in order (the last one of a field wins).
    #[must_use]
    pub fn fields(&self, base: &Base) -> Vec<FormField> {
        let mut shown = base.fields.clone();
        for edit in &self.applied {
            if let Edit::Fill(id, value) = edit {
                let accepted = base.field(id).is_some_and(|field| accepts(field, value));
                if let Some(field) = shown.iter_mut().find(|field| &field.id == id) {
                    if accepted {
                        field.set(value);
                    }
                }
            }
        }
        shown
    }

    /// What the writer must do so the file holds [`EditSet::apply`]'s
    /// annotations and [`EditSet::fields`]' values: new annotations to add,
    /// edited ones as they end, removed ones, filled fields. An annotation
    /// added and then deleted, or edited back to what it was, and a field
    /// filled back with its value, cost nothing.
    #[must_use]
    pub fn resolve(&self, base: &Base) -> Resolved {
        let shown = self.apply(base);
        let shown_ids: HashSet<&AnnotId> = shown.iter().map(|annotation| &annotation.id).collect();
        let mut resolved = Resolved::default();
        for annotation in &shown {
            match base
                .annotations
                .iter()
                .find(|original| original.id == annotation.id)
            {
                None => resolved.added.push(annotation.clone()),
                Some(original) if original != annotation => {
                    resolved.changed.push(annotation.clone());
                }
                Some(_) => {}
            }
        }
        resolved.deleted = base
            .annotations
            .iter()
            .filter(|original| !shown_ids.contains(&original.id))
            .map(|original| original.id.clone())
            .collect();
        resolved.filled = self
            .fields(base)
            .into_iter()
            .zip(&base.fields)
            .filter(|(shown, original)| shown.current() != original.current())
            .map(|(shown, _)| shown)
            .collect();
        resolved
    }
}

/// Applies `edit` to `shown`, or says why it cannot be.
fn apply_one(shown: &mut Vec<Annotation>, base: &Base, edit: &Edit) -> Result<(), AnnotError> {
    let editable = |shown: &[Annotation], id: &AnnotId| {
        shown
            .iter()
            .position(|annotation| &annotation.id == id && is_editable(annotation))
            .ok_or(AnnotError::NotEditable)
    };
    match edit {
        Edit::Add(annotation) => {
            if !is_editable(annotation) || shown.iter().any(|other| other.id == annotation.id) {
                return Err(AnnotError::NotEditable);
            }
            let mut added = annotation.clone();
            if let Kind::Text {
                contents, size_pt, ..
            } = &added.kind
            {
                let page = base.page_box(added.page).ok_or(AnnotError::NotEditable)?;
                added.rect = fit_text_rect(page, added.rect, contents, *size_pt)?;
            }
            shown.push(added);
        }
        Edit::Move(id, rect) => {
            let index = editable(shown, id)?;
            let original = base.annotations.iter().find(|original| &original.id == id);
            let relaid = lays_out_again(original, &shown[index]);
            let annotation = &mut shown[index];
            let rect = Rect::new(rect.x0, rect.y0, rect.x1, rect.y1);
            let from = annotation.rect;
            match &mut annotation.kind {
                // A box Calcita lays out (its own, or one whose text, colour
                // or size changed) grows to its lines; any other only moves,
                // at its own size, keeping its own appearance, never
                // re-encoded: a new size would stretch that appearance.
                Kind::Text {
                    contents, size_pt, ..
                } if relaid => {
                    let page = base
                        .page_box(annotation.page)
                        .ok_or(AnnotError::NotEditable)?;
                    annotation.rect = fit_text_rect(page, rect, contents, *size_pt)?;
                    return Ok(());
                }
                Kind::Text { .. } => {
                    let resized = (rect.width() - from.width()).abs() > SIZE_TOLERANCE
                        || (rect.height() - from.height()).abs() > SIZE_TOLERANCE;
                    if resized {
                        return Err(AnnotError::NotEditable);
                    }
                    annotation.rect = Rect {
                        x0: rect.x0,
                        y0: rect.y0,
                        x1: rect.x0 + from.width(),
                        y1: rect.y0 + from.height(),
                    };
                    return Ok(());
                }
                Kind::Highlight { quads } => {
                    for quad in quads.iter_mut() {
                        for point in &mut quad.points {
                            *point = carried(*point, from, rect);
                        }
                    }
                }
                Kind::Ink { strokes, .. } => {
                    for point in strokes.iter_mut().flatten() {
                        *point = carried(*point, from, rect);
                    }
                }
                Kind::Other { .. } => return Err(AnnotError::NotEditable),
            }
            annotation.rect = rect;
        }
        Edit::Recolor(id, color) => {
            let index = editable(shown, id)?;
            match &shown[index].kind {
                Kind::Text { rich: true, .. } => return Err(AnnotError::NotEditable),
                // A recoloured box is laid out again in Helvetica: refused
                // now when its text cannot be, so no save fails on it.
                Kind::Text { contents, .. } => {
                    win_ansi(contents)?;
                }
                _ => {}
            }
            shown[index].color = *color;
        }
        Edit::SetText(id, text) => {
            let index = editable(shown, id)?;
            let page = base
                .page_box(shown[index].page)
                .ok_or(AnnotError::NotEditable)?;
            let annotation = &mut shown[index];
            let Kind::Text {
                contents,
                size_pt,
                rich: false,
            } = &mut annotation.kind
            else {
                return Err(AnnotError::NotEditable);
            };
            annotation.rect = fit_text_rect(page, annotation.rect, text, *size_pt)?;
            text.clone_into(contents);
        }
        Edit::Delete(id) => {
            let index = editable(shown, id)?;
            shown.remove(index);
        }
        // A value changes no annotation; it is checked against its field.
        Edit::Fill(id, value) => {
            let field = base.field(id).ok_or(AnnotError::NotFillable)?;
            if !accepts_own(field, value) {
                check_fill(field, value)?;
            }
        }
    }
    Ok(())
}

/// Whether `field` (as the file has it) takes `value`: its own value
/// always, else what [`check_fill`] allows.
fn accepts(field: &FormField, value: &FieldValue) -> bool {
    accepts_own(field, value) || check_fill(field, value).is_ok()
}

/// `value` is the one the file gives `field`, which is never refused (a
/// single-line field another program filled with a line break included).
fn accepts_own(field: &FormField, value: &FieldValue) -> bool {
    field.current().as_ref() == Some(value)
}

/// Whether the writer lays `edited` out again (a new appearance from its
/// model) rather than keeping the one its file has: always for an
/// annotation Calcita adds (`original` is `None`) and for a changed
/// highlight or drawing; for a text box when its text, font size, colour
/// or rectangle's size changed (a box reaches a new size only once it is
/// laid out: a move of one that is not keeps its size), so a box only
/// moved keeps its program's appearance, `/DA`, `/Contents` and `/RC`.
#[must_use]
pub fn lays_out_again(original: Option<&Annotation>, edited: &Annotation) -> bool {
    match (original, &edited.kind) {
        (None, _) => true,
        (Some(original), Kind::Text { .. }) => {
            original.kind != edited.kind
                || original.color != edited.color
                || (original.rect.width() - edited.rect.width()).abs() > SIZE_TOLERANCE
                || (original.rect.height() - edited.rect.height()).abs() > SIZE_TOLERANCE
        }
        (Some(original), _) => original != edited,
    }
}

/// `point` of a shape that filled `from`, once the shape fills `to`.
fn carried(point: Point, from: Rect, to: Rect) -> Point {
    let scale = |span_to: f64, span_from: f64| {
        if span_from > 0.0 {
            span_to / span_from
        } else {
            1.0
        }
    };
    Point {
        x: to.x0 + (point.x - from.x0) * scale(to.width(), from.width()),
        y: to.y0 + (point.y - from.y0) * scale(to.height(), from.height()),
    }
}

/// A fresh id for an annotation Calcita adds: its `/NM`, unique enough to
/// never meet another in one file (the time, the process and a counter).
#[must_use]
pub fn new_id() -> AnnotId {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    AnnotId::Name(format!(
        "calcita-{nanos:x}-{:x}-{count}",
        std::process::id()
    ))
}

/// A new text box on the 0-based `page`, whose top-left corner and width are
/// those of `view` (in the page's displayed points); it is as tall as its
/// lines once added.
#[must_use]
pub fn text_box(
    page_box: PageBox,
    page: u32,
    view: Rect,
    text: &str,
    size_pt: f64,
    color: Rgb,
) -> Annotation {
    Annotation {
        id: new_id(),
        page,
        kind: Kind::Text {
            contents: text.to_owned(),
            size_pt,
            rich: false,
        },
        rect: to_pdf_rect(page_box, view),
        color,
        author: None,
    }
}

/// A new highlight on the 0-based `page` over `boxes` (QtPdf's selection
/// boxes, in the page's displayed points): one quadrilateral each, its
/// upper side the box's top as the page is shown. `None` without a box.
#[must_use]
pub fn highlight(page_box: PageBox, page: u32, boxes: &[Rect], color: Rgb) -> Option<Annotation> {
    let quads: Vec<Quad> = boxes
        .iter()
        .filter(|view| view.width() > 0.0 && view.height() > 0.0)
        .map(|view| {
            let corner = |x, y| to_pdf(page_box, Point { x, y });
            Quad {
                points: [
                    corner(view.x0, view.y0),
                    corner(view.x1, view.y0),
                    corner(view.x0, view.y1),
                    corner(view.x1, view.y1),
                ],
            }
        })
        .collect();
    let rect = appearance::highlight_rect(&quads)?;
    Some(Annotation {
        id: new_id(),
        page,
        kind: Kind::Highlight { quads },
        rect,
        color,
        author: None,
    })
}

/// A new drawing on the 0-based `page`: one stroke through `points` (the
/// page's displayed points), `width` points wide; points closer than a
/// tenth of a point to the last one kept are dropped, and the stroke is
/// cut at [`MAX_STROKE_POINTS`]. `None` without a point.
#[must_use]
pub fn ink(
    page_box: PageBox,
    page: u32,
    points: &[Point],
    width: f64,
    color: Rgb,
) -> Option<Annotation> {
    let mut stroke: Vec<Point> = Vec::new();
    for point in points
        .iter()
        .filter(|point| point.x.is_finite() && point.y.is_finite())
    {
        let pdf = to_pdf(page_box, *point);
        let near = stroke
            .last()
            .is_some_and(|last| (last.x - pdf.x).hypot(last.y - pdf.y) < 0.1);
        if !near {
            stroke.push(pdf);
        }
        if stroke.len() == MAX_STROKE_POINTS {
            break;
        }
    }
    let strokes = vec![stroke];
    let rect = appearance::ink_rect(&strokes, width)?;
    Some(Annotation {
        id: new_id(),
        page,
        kind: Kind::Ink { strokes, width },
        rect,
        color,
        author: None,
    })
}
