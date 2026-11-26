//! The pure rect rules of a capsule sheet: where it lands, which part of the
//! row its window shows, what a press hits. Every clamp is panic-free
//! (`f32::clamp` panics when min > max) and every result is finite.

use celestina_paint::soft_edge::Cuts;
use celestina_paint::Rect;

use crate::island::state::{CAPSULE_H, PAGE_MARGIN, TOP_GAP};

/// The space a sheet keeps from the surface's sides, logical px.
pub const SIDE_MARGIN: f32 = 16.0;
/// The height of the row (the capsule's own row) at the top of a sheet.
pub const ROW_H: f32 = CAPSULE_H;
/// How far the blurred row image reaches past the capsule: three sigmas of
/// the widest blur (6).
const ROW_IMAGE_PAD: f32 = 3.0 * 6.0;
/// The tolerance when deciding whether the window cuts the capsule.
const CUT_TOLERANCE: f32 = 0.5;

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

/// Where the sheet lands: `body` is the body box size, `piece` the opening
/// piece and `capsule` its capsule. The sheet is centred on the piece, then
/// held by the capsule's ends (a menu narrower than the capsule is a window
/// over it; a wider one keeps the capsule inside) and by the side margins
/// when the surface has room for them.
pub fn target(capsule: Rect, piece: Rect, body: (f32, f32), surface: (f32, f32)) -> Rect {
    let capsule = sane(capsule);
    let piece = sane(piece);
    let w = finite(body.0).max(0.0);
    let surface_w = finite(surface.0).max(0.0);
    let left = capsule.x;
    let right = capsule.x + capsule.w;
    let centre = piece.x + piece.w / 2.0;
    let wanted = centre - w / 2.0;
    let x = if w >= capsule.w {
        clamp(wanted, right - w, left)
    } else {
        clamp(wanted, left, right - w)
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

/// Which ends of the capsule's row the sheet's window cuts: a side is cut
/// only where the window's edge lies strictly inside the capsule (0.5 px
/// tolerance), never at the capsule's own end.
pub fn cuts(capsule: Rect, sheet: Rect) -> Cuts {
    let capsule = sane(capsule);
    let sheet = sane(sheet);
    let (cl, cr) = (capsule.x, capsule.x + capsule.w);
    let (sl, sr) = (sheet.x, sheet.x + sheet.w);
    Cuts {
        left: sl > cl + CUT_TOLERANCE && sl < cr,
        right: sr < cr - CUT_TOLERANCE && sr > cl,
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

/// The capsule inflated by 18 px (three sigmas of the widest blur) and cut
/// to the surface: the area the blurred row image covers.
pub fn row_image(capsule: Rect, surface: (f32, f32)) -> Rect {
    let c = sane(capsule);
    let (sw, sh) = (finite(surface.0).max(0.0), finite(surface.1).max(0.0));
    let x0 = clamp(c.x - ROW_IMAGE_PAD, 0.0, sw);
    let y0 = clamp(c.y - ROW_IMAGE_PAD, 0.0, sh);
    let x1 = clamp(c.x + c.w + ROW_IMAGE_PAD, x0, sw);
    let y1 = clamp(c.y + c.h + ROW_IMAGE_PAD, y0, sh);
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// What the point lands on: the row window, the body viewport, or neither.
/// Everything else is `Outside`, including the sheet's own capsule beyond the
/// window: its pieces are not drawn there, so a press on them closes the
/// sheet and never reaches the bar.
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
    fn a_narrower_menu_is_a_window_over_the_capsule() {
        let capsule = r(1300.0, 8.0, 604.0, 40.0);
        let t = target(capsule, r(1590.0, 8.0, 20.0, 40.0), (320.0, 100.0), BAR);
        assert_eq!(t.x, 1440.0);
        let t = target(capsule, r(1870.0, 8.0, 20.0, 40.0), (320.0, 100.0), BAR);
        assert_eq!(t.x, 1584.0);
        assert_eq!(t.x + t.w, 1904.0);
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
            r(0.0, 8.0, 1920.0, 40.0),
            r(2.0, 8.0, 10.0, 40.0),
            (320.0, 100.0),
            BAR,
        );
        assert_eq!(t.x, 16.0);
        let t = target(
            r(0.0, 8.0, 1920.0, 40.0),
            r(1910.0, 8.0, 10.0, 40.0),
            (320.0, 100.0),
            BAR,
        );
        assert_eq!(t.x + t.w, 1904.0);
    }

    #[test]
    fn cuts_appear_only_where_the_window_is_inside_the_capsule() {
        let capsule = r(1300.0, 8.0, 604.0, 40.0);
        let both = cuts(capsule, r(1440.0, 8.0, 320.0, 100.0));
        assert_eq!((both.left, both.right), (true, true));
        let flush_right = cuts(capsule, r(1584.0, 8.0, 320.0, 100.0));
        assert_eq!((flush_right.left, flush_right.right), (true, false));
        let flush_left = cuts(capsule, r(1300.0, 8.0, 320.0, 100.0));
        assert_eq!((flush_left.left, flush_left.right), (false, true));
        let wider = cuts(capsule, r(1200.0, 8.0, 800.0, 100.0));
        assert_eq!((wider.left, wider.right), (false, false));
        let within_tolerance = cuts(capsule, r(1300.4, 8.0, 603.2, 100.0));
        assert_eq!(
            (within_tolerance.left, within_tolerance.right),
            (false, false)
        );
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

    #[test]
    fn the_row_image_extends_eighteen_px_past_the_capsule_within_the_surface() {
        let img = row_image(r(1000.0, 8.0, 100.0, 40.0), BAR);
        assert_eq!(img, r(982.0, 0.0, 136.0, 66.0));
        let edge = row_image(r(0.0, 8.0, 1920.0, 40.0), BAR);
        assert_eq!((edge.x, edge.w), (0.0, 1920.0));
        let nan = row_image(r(f32::NAN, 8.0, 10.0, 40.0), BAR);
        assert!(nan.w.is_finite() && nan.h.is_finite());
    }
}
