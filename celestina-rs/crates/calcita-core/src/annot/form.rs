// language-contract: product-copy
//! A document's form (its AcroForm): the fields read with every widget
//! they show on, the copy of the document QtPdf is given to draw them, and
//! what filling a text field or a checkbox writes.
//!
//! - **Reading.** The field tree (`/Fields`, `/Kids`) is walked once, with
//!   a depth bound and a visited set; a field's fully qualified name joins
//!   the partial names (`/T`) of its ancestors with `.`, and `/FT`, `/Ff`,
//!   `/V`, `/DA`, `/Q` and `/MaxLen` are inherited from its ancestors (and
//!   `/DA` and `/Q` from the AcroForm itself) as ISO 32000-1, 12.7.3.1 asks.
//!   A kid with a `/T` is a field of its own; one without is a widget of
//!   its parent, so one field may show on several widgets, on several
//!   pages.
//! - **Showing.** QtPdf draws no widget annotation (`/Widget`), with or
//!   without its annotations flag (the CAL-2 spike). The page provider is
//!   therefore given a copy ([`stamped_view`]) in which each widget's
//!   current appearance (`/AP /N`, or the state `/AS` names) is drawn into
//!   its page's content, mapped onto its `/Rect` as ISO 32000-1, 12.5.5
//!   maps an appearance. The copy lives in memory only.
//! - **Filling.** Calcita fills text fields and checkboxes. A text field
//!   takes `/V` and a new appearance on each widget, in Helvetica in
//!   WinAnsi at the `/DA` size (0 fitted to the widget), aligned as `/Q`
//!   says, within `/MaxLen`, in cells for a comb field and on several lines
//!   for a multiline one; a checkbox takes `/V` and `/AS`, the on-state
//!   named by its widgets' own `/AP /N` (never assumed `Yes`). Radio
//!   buttons, choice lists, push buttons, signatures and read-only fields
//!   are shown and never written ([`ReadOnly`]).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream};

use super::appearance::{
    advance, components, hex_string, number, text_lines, win_ansi, Appearance, Resources,
    FIRST_BASELINE, FONT_RESOURCE, LINE_HEIGHT, TEXT_PADDING,
};
use super::coords::PageBox;
use super::model::{Annotation, FieldKind, Rect, Rgb};
use super::read::{
    color_of, inherited, load_for_edit, load_within, model_of, name, numbers, page_annotations,
    pages, rect_of, resolve, resolve_dictionary, text, AnnotError, DefaultAppearance, Loaded,
    MAX_DEPTH, MAX_INFLATED_BYTES,
};

/// `/Ff` bits (ISO 32000-1, tables 221, 226 and 228).
const FIELD_READ_ONLY: i64 = 1;
const FIELD_MULTILINE: i64 = 1 << 12;
const FIELD_PASSWORD: i64 = 1 << 13;
const FIELD_RADIO: i64 = 1 << 15;
const FIELD_PUSHBUTTON: i64 = 1 << 16;
const FIELD_FILE_SELECT: i64 = 1 << 20;
const FIELD_COMB: i64 = 1 << 24;

/// Annotation flags a widget is not shown with (ISO 32000-1, table 165).
const ANNOT_HIDDEN: i64 = 1 << 1;
const ANNOT_NO_VIEW: i64 = 1 << 5;
/// What keeps a widget off the page: hidden or `NoView`, the same rule for
/// stamping and for the controls. Invisible (bit 1) applies only to
/// annotation types a viewer does not know (table 165), so a widget marked
/// so is shown, as QtPdf and poppler show other annotations marked so.
const NOT_SHOWN: i64 = ANNOT_HIDDEN | ANNOT_NO_VIEW;

/// The largest size a text field's automatic font size takes, in points.
const AUTO_SIZE_MAX: f64 = 12.0;
/// The smallest: below it the text is clipped instead.
const AUTO_SIZE_MIN: f64 = 4.0;
/// Helvetica's ascender and descender, in font sizes (Adobe's AFM).
const ASCENT: f64 = 0.718;
const DESCENT: f64 = 0.207;

/// A field's fully qualified name: the partial names of it and its
/// ancestors joined with `.`. Shown to the person; two fields may share
/// one (or have none), so a field is found by its [`FieldId`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FieldName(pub String);

/// How a field is found again in its file: its own object, else its place
/// in the form's walk (such a field is never filled: there is nowhere to
/// write its `/V`).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FieldId {
    Object { number: u32, generation: u16 },
    Walk(u32),
}

/// What a field is filled with.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// A text field's text.
    Text(String),
    /// Whether a checkbox is checked.
    Checked(bool),
}

/// Where a text field's text sits across its widget (`/Q`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// How a text field's text is laid out: the field's, which a widget's own
/// `/DA` and `/Q` override ([`TextFormat::of`]).
#[derive(Clone, Debug, PartialEq)]
pub struct TextFormat {
    /// The font size `/DA` names; `None` for 0 or none, fitted to the
    /// widget.
    pub size: Option<f64>,
    /// The fill colour `/DA` names; black without one.
    pub color: Rgb,
    pub align: Align,
    /// `/MaxLen`, the most characters the field holds.
    pub max_len: Option<usize>,
    pub multiline: bool,
    /// A comb field: `max_len` cells across, one character each.
    pub comb: bool,
}

impl TextFormat {
    /// The format `widget` lays the text out with: its own `/DA` (size and
    /// colour) and `/Q` when it carries them, else the field's.
    #[must_use]
    pub fn of(&self, widget: &Widget) -> Self {
        let mut format = self.clone();
        if let Some(da) = &widget.da {
            let parsed = DefaultAppearance::parse(da);
            format.size = parsed.size;
            format.color = parsed.color.unwrap_or(Rgb::BLACK);
        }
        if let Some(q) = widget.q {
            format.align = align_of(q);
        }
        format
    }
}

/// `/Q` as an alignment: 1 centred, 2 right, anything else left.
fn align_of(q: i64) -> Align {
    match q {
        1 => Align::Center,
        2 => Align::Right,
        _ => Align::Left,
    }
}

/// Why Calcita shows a field without filling it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadOnly {
    Radio,
    Choice,
    Signature,
    PushButton,
    /// The form marks it read-only (`/Ff` bit 1).
    Locked,
    /// A password or file field, a checkbox whose widgets name no single
    /// on-state, a widget without an object of its own, or an unknown type.
    Unsupported,
}

impl ReadOnly {
    /// What the window says when the field is tapped, in Spanish.
    #[must_use]
    pub fn message_es(self) -> &'static str {
        match self {
            Self::Radio => "Calcita aún no rellena botones de opción: se muestran como están.",
            Self::Choice => "Calcita aún no rellena listas de opciones: se muestran como están.",
            Self::Signature => "Las firmas no se pueden rellenar en Calcita.",
            Self::PushButton => "Este botón no hace nada en Calcita.",
            Self::Locked => "Este campo es de solo lectura.",
            Self::Unsupported => "Calcita no puede rellenar este campo.",
        }
    }
}

/// What Calcita may do with a field.
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Text(TextFormat),
    /// A checkbox, and the name of its on-state.
    Checkbox {
        on: String,
    },
    /// Shown only.
    No(ReadOnly),
}

/// One widget a field shows on.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    /// 0-based; `None` when no page lists it.
    pub page: Option<u32>,
    pub rect: Rect,
    /// `/MK /BG`.
    pub background: Option<Rgb>,
    /// `/MK /BC`.
    pub border: Option<Rgb>,
    /// `/BS /W`, else the third number of `/Border`, else 1.
    pub border_width: f64,
    /// A button widget's on-state: the name in its `/AP /N` other than
    /// `Off`.
    pub on_state: Option<String>,
    /// Shown: its `/F` is neither hidden nor `NoView`. A widget
    /// not shown is neither stamped nor given a control.
    pub shown: bool,
    /// Its own `/DA` and `/Q`, a kid's when it differs from its field's.
    pub(crate) da: Option<String>,
    pub(crate) q: Option<i64>,
    pub(crate) object: Option<ObjectId>,
}

/// One terminal field of the form.
#[derive(Clone, Debug, PartialEq)]
pub struct FormField {
    pub id: FieldId,
    pub name: FieldName,
    /// `/TU`, the name a person reads, when the form gives one.
    pub label: Option<String>,
    pub kind: FieldKind,
    /// A text field's text, a button's state name; `None` without `/V`.
    pub value: Option<String>,
    pub fill: Fill,
    /// In `/Kids` order; at least one.
    pub widgets: Vec<Widget>,
    /// The field's own object, where `/V` is written.
    pub(crate) node: Option<ObjectId>,
}

impl FormField {
    /// The value as filling speaks of it; `None` for a field Calcita does
    /// not fill.
    #[must_use]
    pub fn current(&self) -> Option<FieldValue> {
        match &self.fill {
            Fill::Text(_) => Some(FieldValue::Text(self.value.clone().unwrap_or_default())),
            Fill::Checkbox { on } => Some(FieldValue::Checked(
                self.value.as_deref() == Some(on.as_str()),
            )),
            Fill::No(_) => None,
        }
    }

    /// Takes `value` (already checked with [`check_fill`]).
    pub(crate) fn set(&mut self, value: &FieldValue) {
        self.value = match (&self.fill, value) {
            (Fill::Checkbox { on }, FieldValue::Checked(true)) => Some(on.clone()),
            (Fill::Checkbox { .. }, FieldValue::Checked(false)) => Some("Off".to_owned()),
            (_, FieldValue::Text(text)) => Some(text.clone()),
            (_, FieldValue::Checked(_)) => return,
        };
    }
}

/// Whether `field` may take `value`.
///
/// # Errors
///
/// [`AnnotError::ReadOnlyField`] for a field Calcita only shows,
/// [`AnnotError::NotFillable`] for a value of the other kind or a line
/// break in a single-line field ([`folded`] takes it out first),
/// [`AnnotError::Unencodable`] for text
/// Helvetica in WinAnsi cannot show, [`AnnotError::TooLong`] past `/MaxLen`.
pub fn check_fill(field: &FormField, value: &FieldValue) -> Result<(), AnnotError> {
    match (&field.fill, value) {
        (Fill::No(why), _) => Err(AnnotError::ReadOnlyField(*why)),
        (Fill::Text(format), FieldValue::Text(text)) => {
            if !format.multiline && text.contains(['\n', '\r']) {
                return Err(AnnotError::NotFillable);
            }
            win_ansi(text)?;
            match format.max_len {
                Some(most) if text.chars().count() > most => Err(AnnotError::TooLong(most)),
                _ => Ok(()),
            }
        }
        (Fill::Checkbox { .. }, FieldValue::Checked(_)) => Ok(()),
        _ => Err(AnnotError::NotFillable),
    }
}

/// `value` as `field` can hold it: in a single-line text field, each line
/// break (CR LF, CR or LF) and tab becomes a space, so a value another
/// program wrote with one can be edited there; any other value as it is.
#[must_use]
pub fn folded(field: &FormField, value: FieldValue) -> FieldValue {
    match (&field.fill, value) {
        (Fill::Text(format), FieldValue::Text(text)) if !format.multiline => {
            FieldValue::Text(text.replace("\r\n", " ").replace(['\r', '\n', '\t'], " "))
        }
        (_, value) => value,
    }
}

/// What the window shows of a document, from one read of its file: its
/// annotations, page boxes and form fields, and the copy QtPdf draws its
/// widgets from.
#[derive(Debug, Default)]
pub struct Viewed {
    pub annotations: Vec<Annotation>,
    pub boxes: Vec<PageBox>,
    pub fields: Vec<FormField>,
    /// The document with every widget's appearance stamped into its page
    /// ([`stamped_view`]); `None` when no widget has one, or when the
    /// document's cross-reference table was rebuilt by scanning, so no
    /// update could point back at it.
    pub stamped: Option<Vec<u8>>,
}

/// Reads [`Viewed`] from the document at `path`.
///
/// # Errors
///
/// As [`super::read_annotations`]; a failed stamping costs only the copy.
pub fn view(path: &Path) -> Result<Viewed, AnnotError> {
    let loaded = load_within(path, MAX_INFLATED_BYTES)?;
    let (annotations, boxes) = model_of(&loaded.document);
    let fields = fields_of(&loaded.document);
    let plans = stamp_plans(&loaded.document, &HashSet::new());
    let stamped = if plans.is_empty() || loaded.document.xref_start == 0 {
        None
    } else {
        stamped(loaded, &plans).ok()
    };
    Ok(Viewed {
        annotations,
        boxes,
        fields,
        stamped,
    })
}

/// The bytes of `original` with every shown widget's current appearance
/// drawn into its page's content, as one incremental update kept in
/// memory: what the page provider renders, since QtPdf draws no widget.
/// Without a widget to stamp, the original's bytes.
///
/// # Errors
///
/// The editing reader's [`AnnotError`] (unreadable, too large, encrypted,
/// unparseable, damaged), or [`AnnotError::Write`].
pub fn stamped_view(original: &Path) -> Result<Vec<u8>, AnnotError> {
    let loaded = load_for_edit(original)?;
    let plans = stamp_plans(&loaded.document, &HashSet::new());
    if plans.is_empty() {
        return Ok(loaded.bytes);
    }
    stamped(loaded, &plans)
}

fn stamped(loaded: Loaded, plans: &[PagePlan]) -> Result<Vec<u8>, AnnotError> {
    let mut document = IncrementalDocument::create_from(loaded.bytes, loaded.document);
    stamp(&mut document, plans)?;
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).map_err(AnnotError::Write)?;
    Ok(bytes)
}

/// The widgets of `fields` Calcita fills, by object: while editing they
/// are drawn as controls, so the copy leaves them out.
pub(crate) fn fillable_widgets(fields: &[FormField]) -> HashSet<ObjectId> {
    fields
        .iter()
        .filter(|field| !matches!(field.fill, Fill::No(_)))
        .flat_map(|field| field.widgets.iter().filter_map(|widget| widget.object))
        .collect()
}

/// One page's widgets to stamp: each appearance's object and the matrix
/// that maps it onto its widget's rectangle.
#[derive(Clone, Debug)]
pub(crate) struct PagePlan {
    page: ObjectId,
    placed: Vec<(ObjectId, [f64; 6])>,
}

/// What stamping draws, page by page: every widget a page lists that is
/// shown (not hidden, not `NoView`) and has a normal appearance, except
/// those in `skip`.
pub(crate) fn stamp_plans(document: &Document, skip: &HashSet<ObjectId>) -> Vec<PagePlan> {
    pages(document)
        .into_iter()
        .filter_map(|(_, page)| {
            let placed: Vec<(ObjectId, [f64; 6])> = page_annotations(document, page)
                .into_iter()
                .filter(|(_, object, _)| !object.is_some_and(|object| skip.contains(&object)))
                .filter_map(|(_, _, widget)| placement(document, widget))
                .collect();
            (!placed.is_empty()).then_some(PagePlan { page, placed })
        })
        .collect()
}

/// The appearance a shown widget is drawn with and where.
fn placement(document: &Document, widget: &Dictionary) -> Option<(ObjectId, [f64; 6])> {
    if name(document, widget, b"Subtype").as_deref() != Some("Widget") {
        return None;
    }
    let flags = integer(document, widget, b"F").unwrap_or(0);
    if flags & NOT_SHOWN != 0 {
        return None;
    }
    let rect = rect_of(document, widget.get(b"Rect").ok()?)?;
    let appearance = normal_appearance(document, widget)?;
    let form = document.get_object(appearance).ok()?.as_stream().ok()?;
    let bbox = rect_of(document, form.dict.get(b"BBox").ok()?)?;
    let matrix = form
        .dict
        .get(b"Matrix")
        .ok()
        .map(|matrix| numbers(document, matrix))
        .and_then(|values| <[f64; 6]>::try_from(values).ok())
        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fit(bbox, matrix, rect).map(|placed| (appearance, placed))
}

/// The matrix that maps an appearance onto its rectangle (ISO 32000-1,
/// 12.5.5): its `/BBox` carried by its `/Matrix`, then the bounds of that
/// scaled and moved onto `rect`. Drawing the form applies its own matrix,
/// so this is the one the page adds. `None` for a box without area.
fn fit(bbox: Rect, matrix: [f64; 6], rect: Rect) -> Option<[f64; 6]> {
    let [a, b, c, d, e, f] = matrix;
    let corners = [
        (bbox.x0, bbox.y0),
        (bbox.x1, bbox.y0),
        (bbox.x0, bbox.y1),
        (bbox.x1, bbox.y1),
    ]
    .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in corners {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (width, height) = (x1 - x0, y1 - y0);
    let usable = |value: f64| value.is_finite() && value > 0.0;
    if !usable(width) || !usable(height) {
        return None;
    }
    let sx = rect.width() / width;
    let sy = rect.height() / height;
    Some([sx, 0.0, 0.0, sy, rect.x0 - x0 * sx, rect.y0 - y0 * sy])
}

/// A widget's normal appearance: `/AP /N` itself, or the state of it
/// `/AS` names.
fn normal_appearance(document: &Document, widget: &Dictionary) -> Option<ObjectId> {
    let appearances = resolve_dictionary(document, widget.get(b"AP").ok()?)?;
    let normal = appearances.get(b"N").ok()?;
    match document.dereference(normal).ok()? {
        (Some(id), Object::Stream(_)) => Some(id),
        (_, Object::Dictionary(states)) => {
            let state = name(document, widget, b"AS")?;
            let chosen = states.get(state.as_bytes()).ok()?;
            match document.dereference(chosen).ok()? {
                (Some(id), Object::Stream(_)) => Some(id),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Draws each plan into its page in `document`'s update: the page's
/// content is wrapped in `q`…`Q` (so a state it leaves set cannot reach
/// the widgets), then each appearance is drawn as a form XObject under a
/// name the page's resources do not use yet. The page's resources, even
/// inherited ones, are written into the page itself.
pub(crate) fn stamp(
    document: &mut IncrementalDocument,
    plans: &[PagePlan],
) -> Result<(), AnnotError> {
    for plan in plans {
        let previous = document.get_prev_documents();
        let current = if document.new_document.has_object(plan.page) {
            &document.new_document
        } else {
            previous
        };
        let page = current
            .get_dictionary(plan.page)
            .map_err(AnnotError::Unparseable)?;
        let contents: Vec<Object> = match page.get(b"Contents") {
            Ok(Object::Reference(id)) => match previous.get_object(*id) {
                Ok(Object::Array(items)) => items.clone(),
                _ => vec![Object::Reference(*id)],
            },
            Ok(Object::Array(items)) => items.clone(),
            _ => Vec::new(),
        };
        let mut resources = page
            .get(b"Resources")
            .ok()
            .or_else(|| inherited(previous, plan.page, b"Resources"))
            .and_then(|resources| resolve_dictionary(previous, resources))
            .cloned()
            .unwrap_or_default();
        let mut xobjects = resources
            .get(b"XObject")
            .ok()
            .and_then(|xobjects| resolve_dictionary(previous, xobjects))
            .cloned()
            .unwrap_or_default();
        let mut operators = String::from("Q\n");
        let mut counter = 0_usize;
        for (appearance, [a, b, c, d, e, f]) in &plan.placed {
            let label = loop {
                counter += 1;
                let label = format!("CalcitaWidget{counter}");
                if !xobjects.has(label.as_bytes()) {
                    break label;
                }
            };
            xobjects.set(label.as_str(), Object::Reference(*appearance));
            operators.push_str(&format!(
                "q {} {} {} {} {} {} cm /{label} Do Q\n",
                number(*a),
                number(*b),
                number(*c),
                number(*d),
                number(*e),
                number(*f)
            ));
        }
        resources.set("XObject", xobjects);
        let open = document
            .new_document
            .add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
        let close = document
            .new_document
            .add_object(Stream::new(Dictionary::new(), operators.into_bytes()));
        let mut wrapped = vec![Object::Reference(open)];
        wrapped.extend(contents);
        wrapped.push(Object::Reference(close));
        document
            .opt_clone_object_to_new_document(plan.page)
            .map_err(AnnotError::Unparseable)?;
        let page = document
            .new_document
            .get_dictionary_mut(plan.page)
            .map_err(AnnotError::Unparseable)?;
        page.set("Contents", Object::Array(wrapped));
        page.set("Resources", resources);
    }
    Ok(())
}

/// A number in `dictionary`, references followed.
fn integer(document: &Document, dictionary: &Dictionary, key: &[u8]) -> Option<i64> {
    resolve(document, dictionary.get(key).ok()?).as_i64().ok()
}

/// A text field's text laid out in one widget: the font size, and each run
/// of WinAnsi bytes with the point its baseline starts at.
#[derive(Clone, Debug, PartialEq)]
struct Layout {
    size: f64,
    runs: Vec<(f64, f64, Vec<u8>)>,
}

/// The space inside a widget's border, which the text keeps clear of by
/// [`TEXT_PADDING`] more.
fn border_inset(widget: &Widget) -> f64 {
    if widget.border.is_some() {
        widget.border_width.max(0.0)
    } else {
        0.0
    }
}

/// A count of lines, cells or characters as a number, exact far past any
/// a widget holds.
fn count(items: usize) -> f64 {
    f64::from(u32::try_from(items).unwrap_or(u32::MAX))
}

fn width_of(bytes: &[u8], size: f64) -> f64 {
    f64::from(advance(bytes)) * size / 1000.0
}

/// Lays `text` out in `widget` as `format` asks.
fn layout(widget: &Widget, format: &TextFormat, text: &str) -> Result<Layout, AnnotError> {
    let format = &format.of(widget);
    let rect = widget.rect;
    let inset = border_inset(widget);
    let room_x = (rect.width() - 2.0 * (inset + TEXT_PADDING)).max(0.0);
    let room_y = (rect.height() - 2.0 * (inset + TEXT_PADDING)).max(0.0);
    let start_x = rect.x0 + inset + TEXT_PADDING;
    let aligned = |line_width: f64| match format.align {
        Align::Left => start_x,
        Align::Center => rect.x0 + (rect.width() - line_width) / 2.0,
        Align::Right => rect.x1 - inset - TEXT_PADDING - line_width,
    };
    if format.multiline {
        let lines_at = |size: f64| text_lines(text, rect.width() - 2.0 * inset, size);
        let size = match format.size {
            Some(size) => size,
            None => {
                let mut size = AUTO_SIZE_MAX;
                while size > AUTO_SIZE_MIN
                    && count(lines_at(size)?.len()) * LINE_HEIGHT * size > room_y
                {
                    size -= 0.5;
                }
                size
            }
        };
        let top = rect.y1 - inset - TEXT_PADDING - FIRST_BASELINE * size;
        let runs = lines_at(size)?
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let y = top - count(index) * LINE_HEIGHT * size;
                (aligned(width_of(&line, size)), y, line)
            })
            .collect();
        return Ok(Layout { size, runs });
    }
    let encoded = win_ansi(text)?;
    let fitted_height = (room_y / (ASCENT + DESCENT + 0.2)).min(AUTO_SIZE_MAX);
    let size = match (format.size, format.comb, format.max_len) {
        (Some(size), _, _) => size,
        (None, true, _) => fitted_height.max(AUTO_SIZE_MIN),
        (None, false, _) => {
            let wide = width_of(&encoded, 1.0);
            let fitted_width = if wide > 0.0 { room_x / wide } else { f64::MAX };
            fitted_height.min(fitted_width).max(AUTO_SIZE_MIN)
        }
    };
    let baseline = rect.y0 + (rect.height() - (ASCENT + DESCENT) * size) / 2.0 + DESCENT * size;
    let runs = match (format.comb, format.max_len) {
        (true, Some(cells)) if cells > 0 => {
            let cell = rect.width() / count(cells);
            encoded
                .iter()
                .enumerate()
                .map(|(index, byte)| {
                    let glyph = vec![*byte];
                    let x = rect.x0 + cell * (count(index) + 0.5) - width_of(&glyph, size) / 2.0;
                    (x, baseline, glyph)
                })
                .collect()
        }
        _ => vec![(aligned(width_of(&encoded, size)), baseline, encoded)],
    };
    Ok(Layout { size, runs })
}

/// The font size `text` is shown at in `widget`: `format`'s, or the one
/// fitted to the widget when the form leaves it to the viewer (the largest,
/// for text Helvetica in WinAnsi cannot show, which is never written).
#[must_use]
pub fn font_size(widget: &Widget, format: &TextFormat, text: &str) -> f64 {
    layout(widget, format, text).map_or(format.of(widget).size.unwrap_or(AUTO_SIZE_MAX), |layout| {
        layout.size
    })
}

/// A text field widget's appearance showing `text`: its background and
/// border as `/MK` names them, then the text, marked as the variable text
/// (`/Tx BMC`) and clipped inside the border. The `/BBox` is the widget's
/// rectangle in page space, so it maps onto it unchanged.
///
/// # Errors
///
/// As [`win_ansi`].
pub fn text_field_ap(
    widget: &Widget,
    format: &TextFormat,
    text: &str,
) -> Result<Appearance, AnnotError> {
    let rect = widget.rect;
    let inset = border_inset(widget);
    let layout = layout(widget, format, text)?;
    let format = format.of(widget);
    let mut content = String::new();
    if let Some(background) = widget.background {
        content.push_str(&format!(
            "{} rg {} {} {} {} re f ",
            components(background),
            number(rect.x0),
            number(rect.y0),
            number(rect.width()),
            number(rect.height())
        ));
    }
    if let (Some(border), true) = (widget.border, inset > 0.0) {
        content.push_str(&format!(
            "{} RG {} w {} {} {} {} re S ",
            components(border),
            number(inset),
            number(rect.x0 + inset / 2.0),
            number(rect.y0 + inset / 2.0),
            number(rect.width() - inset),
            number(rect.height() - inset)
        ));
    }
    content.push_str(&format!(
        "/Tx BMC q {} {} {} {} re W n BT /{FONT_RESOURCE} {} Tf {} rg",
        number(rect.x0 + inset),
        number(rect.y0 + inset),
        number((rect.width() - 2.0 * inset).max(0.0)),
        number((rect.height() - 2.0 * inset).max(0.0)),
        number(layout.size),
        components(format.color)
    ));
    for (x, y, run) in &layout.runs {
        content.push_str(&format!(
            " 1 0 0 1 {} {} Tm {} Tj",
            number(*x),
            number(*y),
            hex_string(run)
        ));
    }
    content.push_str(" ET Q EMC");
    Ok(Appearance {
        bbox: rect,
        content: content.into_bytes(),
        resources: Resources::Helvetica,
    })
}

/// Every terminal field of `document`'s form, in the form's order; empty
/// without a form.
pub(crate) fn fields_of(document: &Document) -> Vec<FormField> {
    let Some(form) = document
        .catalog()
        .ok()
        .and_then(|catalog| catalog.get(b"AcroForm").ok())
        .and_then(|form| resolve_dictionary(document, form))
    else {
        return Vec::new();
    };
    let Some(roots) = form
        .get(b"Fields")
        .ok()
        .and_then(|fields| resolve(document, fields).as_array().ok())
    else {
        return Vec::new();
    };
    let start = Inherited {
        da: text(document, form, b"DA"),
        q: integer(document, form, b"Q"),
        ..Inherited::default()
    };
    let mut walk = FieldWalk {
        document,
        widget_pages: widget_pages(document),
        visited: HashSet::new(),
        fields: Vec::new(),
    };
    for root in roots {
        walk.visit(root, &start, 0);
    }
    walk.fields
}

/// The page of each widget a page lists in `/Annots`.
fn widget_pages(document: &Document) -> HashMap<ObjectId, u32> {
    let mut found = HashMap::new();
    for (page, page_id) in pages(document) {
        for (_, id, _) in page_annotations(document, page_id) {
            if let Some(id) = id {
                found.entry(id).or_insert(page);
            }
        }
    }
    found
}

/// What a field inherits from its ancestors (ISO 32000-1, 12.7.3.1).
#[derive(Clone, Default)]
struct Inherited {
    name: String,
    field_type: Option<String>,
    flags: i64,
    value: Option<Object>,
    da: Option<String>,
    q: Option<i64>,
    max_len: Option<i64>,
}

struct FieldWalk<'a> {
    document: &'a Document,
    widget_pages: HashMap<ObjectId, u32>,
    visited: HashSet<ObjectId>,
    fields: Vec<FormField>,
}

impl FieldWalk<'_> {
    fn visit(&mut self, node: &Object, parent: &Inherited, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let id = node.as_reference().ok();
        if let Some(id) = id {
            if !self.visited.insert(id) {
                return;
            }
        }
        let document = self.document;
        let Some(dictionary) = resolve_dictionary(document, node) else {
            return;
        };
        let mut own = parent.clone();
        if let Some(partial) = text(document, dictionary, b"T") {
            own.name = if own.name.is_empty() {
                partial
            } else {
                format!("{}.{partial}", own.name)
            };
        }
        if let Some(field_type) = name(document, dictionary, b"FT") {
            own.field_type = Some(field_type);
        }
        if let Some(flags) = integer(document, dictionary, b"Ff") {
            own.flags = flags;
        }
        if let Ok(value) = dictionary.get(b"V") {
            own.value = Some(resolve(document, value).clone());
        }
        if let Some(da) = text(document, dictionary, b"DA") {
            own.da = Some(da);
        }
        if let Some(q) = integer(document, dictionary, b"Q") {
            own.q = Some(q);
        }
        if let Some(max_len) = integer(document, dictionary, b"MaxLen") {
            own.max_len = Some(max_len);
        }
        let kids: Vec<&Object> = dictionary
            .get(b"Kids")
            .ok()
            .and_then(|kids| resolve(document, kids).as_array().ok())
            .map(|kids| kids.iter().collect())
            .unwrap_or_default();
        // A kid with a `/T` is a field of its own; one without is a widget of
        // this field. A kid without a `/T` but with kids of its own is a
        // field node too (its kids keep this field's name).
        let (fields, widgets): (Vec<&Object>, Vec<&Object>) = kids.into_iter().partition(|kid| {
            resolve_dictionary(document, kid).is_some_and(|kid| kid.has(b"T") || kid.has(b"Kids"))
        });
        for kid in fields {
            self.visit(kid, &own, depth + 1);
        }
        let widgets: Vec<(Option<ObjectId>, &Dictionary)> = if widgets.is_empty() {
            if dictionary.has(b"Rect") {
                vec![(id, dictionary)]
            } else {
                Vec::new()
            }
        } else {
            widgets
                .into_iter()
                .filter_map(|widget| {
                    resolve_dictionary(document, widget)
                        .map(|dict| (widget.as_reference().ok(), dict))
                })
                .collect()
        };
        if widgets.is_empty() {
            return;
        }
        let field = self.field(&own, id, dictionary, &widgets);
        self.fields.push(field);
    }

    fn widget(&self, id: Option<ObjectId>, widget: &Dictionary, node: Option<ObjectId>) -> Widget {
        let document = self.document;
        let page = id
            .and_then(|id| self.widget_pages.get(&id).copied())
            .or_else(|| {
                let target = widget.get(b"P").ok()?.as_reference().ok()?;
                pages(document)
                    .into_iter()
                    .find_map(|(page, id)| (id == target).then_some(page))
            });
        let characteristics = widget
            .get(b"MK")
            .ok()
            .and_then(|mk| resolve_dictionary(document, mk));
        let mk_color = |key: &[u8]| {
            characteristics
                .and_then(|mk| mk.get(key).ok())
                .and_then(|color| color_of(&numbers(document, color)))
        };
        let border_width = widget
            .get(b"BS")
            .ok()
            .and_then(|style| resolve_dictionary(document, style))
            .and_then(|style| style.get(b"W").ok())
            .and_then(|width| resolve(document, width).as_float().ok().map(f64::from))
            .or_else(|| {
                widget
                    .get(b"Border")
                    .ok()
                    .and_then(|border| numbers(document, border).get(2).copied())
            })
            .filter(|width| width.is_finite() && *width >= 0.0)
            .unwrap_or(1.0);
        let on_state = widget
            .get(b"AP")
            .ok()
            .and_then(|ap| resolve_dictionary(document, ap))
            .and_then(|ap| ap.get(b"N").ok())
            .and_then(|normal| resolve_dictionary(document, normal))
            .and_then(|states| {
                states
                    .iter()
                    .map(|(state, _)| String::from_utf8_lossy(state).into_owned())
                    .find(|state| state != "Off")
            });
        // A widget that is its field's own object has its field's `/DA`.
        let own = id.is_none() || id != node;
        Widget {
            page,
            shown: integer(document, widget, b"F").unwrap_or(0) & NOT_SHOWN == 0,
            da: own.then(|| text(document, widget, b"DA")).flatten(),
            q: own.then(|| integer(document, widget, b"Q")).flatten(),
            rect: widget
                .get(b"Rect")
                .ok()
                .and_then(|rect| rect_of(document, rect))
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
            background: mk_color(b"BG"),
            border: mk_color(b"BC"),
            border_width,
            on_state,
            object: id,
        }
    }

    fn field(
        &self,
        own: &Inherited,
        node: Option<ObjectId>,
        dictionary: &Dictionary,
        widgets: &[(Option<ObjectId>, &Dictionary)],
    ) -> FormField {
        let document = self.document;
        let kind = match own.field_type.as_deref() {
            Some("Tx") => FieldKind::Text,
            Some("Btn") if own.flags & (FIELD_RADIO | FIELD_PUSHBUTTON) == 0 => FieldKind::Checkbox,
            _ => FieldKind::Other,
        };
        let first = widgets.first().map(|(_, widget)| *widget);
        let value = own
            .value
            .as_ref()
            .and_then(|value| match value {
                Object::Name(state) => Some(String::from_utf8_lossy(state).into_owned()),
                other => other.as_str().ok().map(super::read::decode_text),
            })
            .or_else(|| {
                (kind == FieldKind::Checkbox)
                    .then(|| first.and_then(|widget| name(document, widget, b"AS")))
                    .flatten()
            });
        let widgets: Vec<Widget> = widgets
            .iter()
            .map(|(id, widget)| self.widget(*id, widget, node))
            .collect();
        let writable = node.is_some() && widgets.iter().all(|widget| widget.object.is_some());
        let fill = match own.field_type.as_deref() {
            _ if own.flags & FIELD_READ_ONLY != 0 => Fill::No(ReadOnly::Locked),
            Some("Tx") if own.flags & (FIELD_PASSWORD | FIELD_FILE_SELECT) != 0 || !writable => {
                Fill::No(ReadOnly::Unsupported)
            }
            Some("Tx") => {
                // The field's own (or inherited) `/DA` and `/Q`; a kid
                // widget's own override them ([`TextFormat::of`]).
                let da = own
                    .da
                    .as_deref()
                    .map(DefaultAppearance::parse)
                    .unwrap_or_default();
                let max_len = own
                    .max_len
                    .filter(|most| *most > 0)
                    .and_then(|most| usize::try_from(most).ok());
                let multiline = own.flags & FIELD_MULTILINE != 0;
                Fill::Text(TextFormat {
                    size: da.size,
                    color: da.color.unwrap_or(Rgb::BLACK),
                    align: own.q.map_or(Align::Left, align_of),
                    max_len,
                    multiline,
                    comb: own.flags & FIELD_COMB != 0 && max_len.is_some() && !multiline,
                })
            }
            Some("Btn") if own.flags & FIELD_RADIO != 0 => Fill::No(ReadOnly::Radio),
            Some("Btn") if own.flags & FIELD_PUSHBUTTON != 0 => Fill::No(ReadOnly::PushButton),
            Some("Btn") => {
                let mut states = widgets.iter().map(|widget| widget.on_state.clone());
                match states.next().flatten() {
                    Some(on) if writable && states.all(|state| state.as_deref() == Some(&on)) => {
                        Fill::Checkbox { on }
                    }
                    _ => Fill::No(ReadOnly::Unsupported),
                }
            }
            Some("Ch") => Fill::No(ReadOnly::Choice),
            Some("Sig") => Fill::No(ReadOnly::Signature),
            _ => Fill::No(ReadOnly::Unsupported),
        };
        let id = node.map_or_else(
            || FieldId::Walk(u32::try_from(self.fields.len()).unwrap_or(u32::MAX)),
            |(number, generation)| FieldId::Object { number, generation },
        );
        FormField {
            id,
            name: FieldName(own.name.clone()),
            label: text(document, dictionary, b"TU"),
            kind,
            value,
            fill,
            widgets,
            node,
        }
    }
}
