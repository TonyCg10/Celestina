//! The one conversion between PDF user space and the displayed page.
//!
//! PDF user space has its origin at the bottom left of the page box and `y`
//! growing upwards; `/Rotate` turns the page clockwise when it is shown.
//! QtPdf (PDFium) speaks of a page in the displayed page's points: origin at
//! the top left of the page as shown, after the rotation, `y` growing
//! downwards. Selection boxes, link rectangles and search hits arrive in that
//! space, and an annotation is stored in the other one, so every crossing
//! goes through [`to_view`] and [`to_pdf`].
//!
//! The formula is the one the CAL-2 spike measured on QtPdf's renders for
//! `/Rotate` 0, 90, 180 and 270, with and without an offset box: from the
//! displayed point `(X, Y)` to the unrotated, top-left point `(u, v)`,
//!
//! | `/Rotate` | `(u, v)` |
//! |---|---|
//! | 0 | `(X, Y)` |
//! | 90 | `(Y, H − X)` |
//! | 180 | `(W − X, H − Y)` |
//! | 270 | `(W − Y, X)` |
//!
//! then `x = x0 + u` and `y = y0 + H − v`, where `(x0, y0, W, H)` is the page
//! box (the `/CropBox`, else the `/MediaBox`, as PDFium uses) before the
//! rotation.

use super::model::{Point, Rect};

/// How the page is turned when shown, clockwise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    /// The rotation `/Rotate` names, read as PDFium reads it: the whole
    /// number of quarter turns it holds, `(degrees / 90) % 4` with the
    /// division truncating, a negative count turned the other way round.
    /// ISO 32000-1 asks for a multiple of 90; PDFium shows 100 as 90 and 45
    /// upright, and so does this. A real `/Rotate` (`90.0`) is truncated to
    /// a whole number of degrees first ([`Rotation::from_number`]).
    #[must_use]
    pub fn from_degrees(degrees: i64) -> Self {
        match (degrees / 90).rem_euclid(4) {
            1 => Self::R90,
            2 => Self::R180,
            3 => Self::R270,
            _ => Self::R0,
        }
    }

    /// The rotation a real `/Rotate` names: truncated to whole degrees, as
    /// PDFium reads a number where it expects an integer, then as
    /// [`Rotation::from_degrees`]. Not a number reads as upright.
    #[must_use]
    pub fn from_number(degrees: f64) -> Self {
        // `as` saturates and maps NaN to 0, which reads as upright.
        Self::from_degrees(degrees.trunc() as i64)
    }

    /// Whether the page is shown on its side, so its width and height swap.
    #[must_use]
    pub fn is_quarter_turn(self) -> bool {
        matches!(self, Self::R90 | Self::R270)
    }
}

/// A page's box in PDF user space and its rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBox {
    /// The box's lower-left corner.
    pub x0: f64,
    pub y0: f64,
    /// The box's size before the rotation.
    pub w: f64,
    pub h: f64,
    pub rotate: Rotation,
}

impl PageBox {
    /// The page as shown, in points: width and height after the rotation.
    #[must_use]
    pub fn view_size(&self) -> (f64, f64) {
        if self.rotate.is_quarter_turn() {
            (self.h, self.w)
        } else {
            (self.w, self.h)
        }
    }
}

/// The displayed point (top-left origin, rotated) of the PDF user-space point
/// `point`.
#[must_use]
pub fn to_view(page: PageBox, point: Point) -> Point {
    let (w, h) = (page.w, page.h);
    let u = point.x - page.x0;
    let v = page.y0 + h - point.y;
    let (x, y) = match page.rotate {
        Rotation::R0 => (u, v),
        Rotation::R90 => (h - v, u),
        Rotation::R180 => (w - u, h - v),
        Rotation::R270 => (v, w - u),
    };
    Point { x, y }
}

/// The PDF user-space point of the displayed point `point`.
#[must_use]
pub fn to_pdf(page: PageBox, point: Point) -> Point {
    let (w, h) = (page.w, page.h);
    let (u, v) = match page.rotate {
        Rotation::R0 => (point.x, point.y),
        Rotation::R90 => (point.y, h - point.x),
        Rotation::R180 => (w - point.x, h - point.y),
        Rotation::R270 => (w - point.y, point.x),
    };
    Point {
        x: page.x0 + u,
        y: page.y0 + h - v,
    }
}

/// The displayed rectangle of a user-space rectangle, normalized (QtPdf's
/// rectangles are `normalized()` too: on a rotated page the corners swap).
#[must_use]
pub fn to_view_rect(page: PageBox, rect: Rect) -> Rect {
    let a = to_view(
        page,
        Point {
            x: rect.x0,
            y: rect.y0,
        },
    );
    let b = to_view(
        page,
        Point {
            x: rect.x1,
            y: rect.y1,
        },
    );
    Rect::new(a.x, a.y, b.x, b.y)
}

/// The user-space rectangle of a displayed rectangle, normalized.
#[must_use]
pub fn to_pdf_rect(page: PageBox, rect: Rect) -> Rect {
    let a = to_pdf(
        page,
        Point {
            x: rect.x0,
            y: rect.y0,
        },
    );
    let b = to_pdf(
        page,
        Point {
            x: rect.x1,
            y: rect.y1,
        },
    );
    Rect::new(a.x, a.y, b.x, b.y)
}
