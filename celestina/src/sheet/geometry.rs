//! The pure rect rules of a capsule sheet: where it lands, where its body
//! sits under the row, what a press hits. Every clamp is panic-free
//! (`f32::clamp` panics when min > max) and every result is finite.

use celestina_paint::Rect;

use crate::island::state::{CAPSULE_H, PAGE_MARGIN, TOP_GAP};

/// The space a sheet keeps from the surface's sides, logical px.
pub const SIDE_MARGIN: f32 = 16.0;
/// The height of the row (the capsule's own row) at the top of a sheet.
pub const ROW_H: f32 = CAPSULE_H;

/// What a press lands on while a sheet is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// Inside the row window.
    Row,
    /// Inside the viewport below the row.
    Body,
    Outside,
}

/// `v` when finite, else 0.
fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// `v` limited to `lo..=hi`, `hi` lifted to `lo` first: never panics.
fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi.max(lo))
}

fn sane(r: Rect) -> Rect {
    Rect {
        x: finite(r.x),
        y: finite(r.y),
        w: finite(r.w).max(0.0),
        h: finite(r.h).max(0.0),
    }
}

/// The tallest a sheet gets on `surface`: the room under the top gap, less
/// the bottom margin (544 on a 560 surface).
pub fn max_height(surface: (f32, f32)) -> f32 {
    (finite(surface.1) - TOP_GAP - PAGE_MARGIN).max(0.0)
}

/// Where the sheet lands: `body` is the body box size, `piece` the slot of
/// the opening piece (its centre is what counts) and `capsule` the capsule's
/// extent without that piece. The sheet is never narrower than the capsule,
/// so every other piece of the row stays visible: a menu at least as wide as
/// the capsule is centred on the slot and held so that the capsule stays
/// inside it; a narrower one takes the capsule's width and place (its body
/// box sits under the row, see [`body_box`]). The side margins hold when the
/// surface has room for them.
pub fn target(capsule: Rect, piece: Rect, body: (f32, f32), surface: (f32, f32)) -> Rect {
    let capsule = sane(capsule);
    let piece = sane(piece);
    let menu_w = finite(body.0).max(0.0);
    let surface_w = finite(surface.0).max(0.0);
    let left = capsule.x;
    let right = capsule.x + capsule.w;
    let centre = piece.x + piece.w / 2.0;
    let (x, w) = if menu_w >= capsule.w {
        (clamp(centre - menu_w / 2.0, right - menu_w, left), menu_w)
    } else {
        (left, capsule.w)
    };
    let x = clamp(x, SIDE_MARGIN, surface_w - SIDE_MARGIN - w);
    let h = (ROW_H + finite(body.1).max(0.0)).min(max_height(surface));
    Rect {
        x,
        y: TOP_GAP,
        w,
        h,
    }
}

/// Where the body box sits: under the row of `sheet`, `body_w` wide, centred
/// on the slot `piece` and held inside the sheet. A body as wide as the sheet
/// fills it.
pub fn body_box(sheet: Rect, piece: Rect, body_w: f32) -> Rect {
    let s = sane(sheet);
    let piece = sane(piece);
    let w = finite(body_w).max(0.0);
    let centre = piece.x + piece.w / 2.0;
    Rect {
        x: clamp(centre - w / 2.0, s.x, s.x + s.w - w),
        y: s.y + ROW_H,
        w,
        h: (s.h - ROW_H).max(0.0),
    }
}

/// The row's window: the sheet's top 40 px.
pub fn row_window(sheet: Rect) -> Rect {
    let s = sane(sheet);
    Rect {
        x: s.x,
        y: s.y,
        w: s.w,
        h: ROW_H,
    }
}

/// The body's viewport: the sheet under its row.
pub fn viewport(sheet: Rect) -> Rect {
    let s = sane(sheet);
    Rect {
        x: s.x,
        y: s.y + ROW_H,
        w: s.w,
        h: (s.h - ROW_H).max(0.0),
    }
}

/// What the point lands on: the row window (the sheet's top 40 px, which
/// holds the capsule's row and always contains its capsule), the body
/// viewport, or neither. Everything else is `Outside`.
pub fn hit(sheet: Rect, x: f32, y: f32) -> Hit {
    if row_window(sheet).contains(x, y) {
        Hit::Row
    } else if viewport(sheet).contains(x, y) {
        Hit::Body
    } else {
        Hit::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    const BAR: (f32, f32) = (1920.0, 560.0);

    #[test]
    fn a_wider_menu_grows_from_the_capsule_and_keeps_its_row() {
        let capsule = r(1000.0, 8.0, 100.0, 40.0);
        let piece = r(1040.0, 8.0, 20.0, 40.0);
        let t = target(capsule, piece, (240.0, 236.0), BAR);
        assert_eq!(t, r(930.0, 8.0, 240.0, 276.0));
        assert!(t.x <= capsule.x && t.x + t.w >= capsule.x + capsule.w);
    }

    #[test]
    fn a_wide_menu_at_the_start_zone_is_held_by_the_margin() {
        let capsule = r(16.0, 8.0, 120.0, 40.0);
        let piece = r(30.0, 8.0, 20.0, 40.0);
        let t = target(capsule, piece, (240.0, 100.0), BAR);
        assert_eq!(t.x, 16.0);
    }

    #[test]
    fn a_menu_narrower_than_its_capsule_takes_the_capsule_width() {
        let capsule = r(1000.0, 8.0, 400.0, 40.0);
        let slot = r(1300.0, 8.0, 0.0, 40.0);
        let t = target(capsule, slot, (240.0, 100.0), BAR);
        assert_eq!((t.x, t.w), (1000.0, 400.0));
        // Wherever the slot is, the sheet stays on the capsule.
        for x in [1000.0, 1100.0, 1399.0] {
            let t = target(capsule, r(x, 8.0, 0.0, 40.0), (240.0, 100.0), BAR);
            assert_eq!((t.x, t.w), (1000.0, 400.0));
        }
        // The body box sits under the slot, held inside the sheet.
        let sheet = r(1000.0, 8.0, 400.0, 140.0);
        assert_eq!(
            body_box(sheet, r(1250.0, 8.0, 0.0, 40.0), 240.0),
            r(1130.0, 48.0, 240.0, 100.0)
        );
        // Past the end it is held at the sheet's right.
        assert_eq!(body_box(sheet, slot, 240.0).x, 1160.0);
        assert_eq!(body_box(sheet, r(1010.0, 8.0, 0.0, 40.0), 240.0).x, 1000.0);
        assert_eq!(body_box(sheet, r(1390.0, 8.0, 0.0, 40.0), 240.0).x, 1160.0);
        let full = body_box(sheet, slot, 400.0);
        assert_eq!((full.x, full.w), (1000.0, 400.0));
    }

    #[test]
    fn a_wider_menu_still_contains_the_capsule() {
        let capsule = r(1300.0, 8.0, 604.0, 40.0);
        let t = target(capsule, r(1590.0, 8.0, 0.0, 40.0), (700.0, 100.0), BAR);
        assert_eq!(t.w, 700.0);
        assert!(t.x <= capsule.x && t.x + t.w >= capsule.x + capsule.w);
        let at_the_end = target(capsule, r(1900.0, 8.0, 0.0, 40.0), (700.0, 100.0), BAR);
        assert_eq!(at_the_end.x + at_the_end.w, 1904.0);
        // Equal widths: the menu is the capsule.
        let same = target(capsule, r(1590.0, 8.0, 0.0, 40.0), (604.0, 100.0), BAR);
        assert_eq!((same.x, same.w), (1300.0, 604.0));
    }

    #[test]
    fn the_side_margins_still_clamp() {
        // A wide menu at the end of the surface.
        let end = r(1700.0, 8.0, 200.0, 40.0);
        let t = target(end, r(1890.0, 8.0, 0.0, 40.0), (320.0, 100.0), BAR);
        assert_eq!(t.x + t.w, 1904.0);
        // A narrow one over a capsule that sits inside the margin.
        let edge = r(5.0, 8.0, 400.0, 40.0);
        let t = target(edge, r(100.0, 8.0, 0.0, 40.0), (240.0, 100.0), BAR);
        assert_eq!((t.x, t.w), (16.0, 400.0));
    }

    #[test]
    fn a_tall_body_is_capped_at_544() {
        let capsule = r(1000.0, 8.0, 100.0, 40.0);
        let t = target(capsule, capsule, (320.0, 505.0), BAR);
        assert_eq!(t.h, 544.0);
        assert_eq!(max_height(BAR), 544.0);
    }

    #[test]
    fn the_target_never_panics_and_keeps_the_margins_when_it_can() {
        let capsule = r(100.0, 8.0, 80.0, 40.0);
        let t = target(capsule, capsule, (420.0, 100.0), (300.0, 560.0));
        assert_eq!(t.x, 16.0);
        let nan = f32::NAN;
        let t = target(capsule, r(nan, nan, nan, nan), (320.0, 100.0), BAR);
        assert!([t.x, t.y, t.w, t.h].iter().all(|v| v.is_finite()));
        let t = target(r(nan, nan, nan, nan), capsule, (nan, nan), (nan, nan));
        assert!([t.x, t.y, t.w, t.h].iter().all(|v| v.is_finite()));
        let t = target(capsule, capsule, (320.0, 100.0), (0.0, 0.0));
        assert!([t.x, t.y, t.w, t.h].iter().all(|v| v.is_finite()));
        // Room for the margins: they hold.
        let t = target(
            r(16.0, 8.0, 1888.0, 40.0),
            r(18.0, 8.0, 10.0, 40.0),
            (320.0, 100.0),
            BAR,
        );
        assert_eq!(t.x, 16.0);
        let t = target(
            r(16.0, 8.0, 1888.0, 40.0),
            r(1890.0, 8.0, 10.0, 40.0),
            (320.0, 100.0),
            BAR,
        );
        assert_eq!(t.x + t.w, 1904.0);
    }

    #[test]
    fn hit_tells_row_body_and_outside_apart() {
        let sheet = r(930.0, 8.0, 240.0, 276.0);
        assert_eq!(hit(sheet, 1000.0, 20.0), Hit::Row);
        assert_eq!(hit(sheet, 1000.0, 100.0), Hit::Body);
        assert_eq!(hit(sheet, 100.0, 20.0), Hit::Outside);
        assert_eq!(hit(sheet, 1000.0, 400.0), Hit::Outside);
        assert_eq!(row_window(sheet), r(930.0, 8.0, 240.0, 40.0));
        assert_eq!(viewport(sheet), r(930.0, 48.0, 240.0, 236.0));
        assert_eq!(viewport(r(0.0, 8.0, 10.0, 20.0)).h, 0.0);
    }
}
