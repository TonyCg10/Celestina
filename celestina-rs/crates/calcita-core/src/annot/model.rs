//! Calcita's view of a document's annotations and form fields.
//!
//! Three kinds are Calcita's own, whoever wrote them: a text box
//! (`/FreeText`), a highlight (`/Highlight`) and a freehand drawing
//! (`/Ink`). Every other annotation (links, form widgets, pop-ups, stamps…)
//! is `Other` and is only shown, never edited. Coordinates are PDF user
//! space; [`super::coords`] turns them into the displayed page's points.

/// A point in PDF user space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A rectangle in PDF user space, always normalized: `x0 <= x1`, `y0 <= y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    /// The rectangle with corners `(ax, ay)` and `(bx, by)`, in any order
    /// (a `/Rect` may name any two opposite corners).
    #[must_use]
    pub fn new(ax: f64, ay: f64, bx: f64, by: f64) -> Self {
        Self {
            x0: ax.min(bx),
            y0: ay.min(by),
            x1: ax.max(bx),
            y1: ay.max(by),
        }
    }

    #[must_use]
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    #[must_use]
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}

/// One quadrilateral of a highlight, its four corners in `/QuadPoints`
/// order (upper left, upper right, lower left, lower right as most programs
/// write them; the order is kept as read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub points: [Point; 4],
}

/// A colour, 8 bits per channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// What PDFium paints a highlight without `/C` in.
    pub const YELLOW: Self = Self {
        r: 255,
        g: 255,
        b: 0,
    };
}

/// How an annotation is found again in its file.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AnnotId {
    /// An indirect object: its number and generation, unique in the file.
    Object { number: u32, generation: u16 },
    /// A dictionary written straight into `/Annots` that carries `/NM`.
    Name(String),
    /// A dictionary written straight into `/Annots` without `/NM`: its
    /// place there, 0-based, on its 0-based page.
    Position { page: u32, index: u32 },
}

/// What an annotation is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A text box: its plain text (`/Contents`) and font size in points
    /// (from `/DA`; 12 when it names none).
    Text { contents: String, size_pt: f64 },
    /// A highlight over the quadrilaterals it covers.
    Highlight { quads: Vec<Quad> },
    /// A freehand drawing: each stroke a run of points.
    Ink { strokes: Vec<Vec<Point>> },
    /// Anything else, by its `/Subtype` (`Link`, `Widget`, `Popup`…).
    Other { subtype: String },
}

/// One annotation of a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: AnnotId,
    /// 0-based, as QtPdf counts pages.
    pub page: u32,
    pub kind: Kind,
    pub rect: Rect,
    /// A text box's text colour (from `/DA`, else `/C`); any other kind's
    /// `/C`. Without one: black, and yellow for a highlight.
    pub color: Rgb,
    /// `/T`, the author a program recorded.
    pub author: Option<String>,
}

/// What a form field holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    /// A text field (`/FT /Tx`).
    Text,
    /// A checkbox (`/FT /Btn`, neither a radio button nor a push button).
    Checkbox,
    /// Anything else: radio buttons, choice lists, push buttons, signatures.
    Other,
}

/// One form field, at its first widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The fully qualified name (`/T` of each ancestor, joined with `.`).
    pub name: String,
    pub kind: FieldKind,
    /// A text field's text, a checkbox's state name (`Yes`, `Off`…); `None`
    /// when the field has no `/V`.
    pub value: Option<String>,
    pub rect: Rect,
    /// 0-based; `None` when the widget is on no page of the document.
    pub page: Option<u32>,
}
