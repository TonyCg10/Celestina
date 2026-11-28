//! The pure rect rules of a capsule menu window: where it lands under its
//! capsule, what a press hits. Every clamp is panic-free (`f32::clamp` panics
//! when min > max) and every result is finite.

use celestina_paint::Rect;

use crate::island::state::{CAPSULE_H, PAGE_MARGIN, TOP_GAP};

/// The space a window keeps from the surface's sides, logical px.
pub const SIDE_MARGIN: f32 = 16.0;
/// How far a window reaches past its piece, towards the capsule's outer end,
/// logical px (CEL-D-017).
pub const PIECE_LEAD: f32 = SIDE_MARGIN;
/// The gap between the bar's capsules and a window, logical px.
const WINDOW_GAP: f32 = 8.0;
/// The capsules' bottom edge, where a drop hangs from and the bar's glass ends
/// (48).
pub const CAPSULE_BOTTOM: f32 = TOP_GAP + CAPSULE_H;
/// The window's top edge: the capsules' top, their height and the gap (56).
pub const WINDOW_TOP: f32 = TOP_GAP + CAPSULE_H + WINDOW_GAP;

/// Which end of its capsule a window opens towards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The window ends just after its piece, within the capsule's left edge.
    Start,
    /// The window starts just before its piece, within the capsule's right
    /// edge.
    End,
    /// The window centred on the capsule.
    Centre,
}

impl Edge {
    /// The end of the capsule the window of bar zone `zone` opens towards: the
    /// start zone (0) its left end, the end zone (3) its right end, the centre
    /// zones (1, 2) none.
    #[must_use]
    pub fn of_zone(zone: usize) -> Edge {
        match zone {
            0 => Edge::Start,
            3 => Edge::End,
            _ => Edge::Centre,
        }
    }
}

/// What a press lands on while a window is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// Inside the window, under the bar's band.
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

/// The tallest a window gets on `surface`: the room under [`WINDOW_TOP`],
/// less the bottom margin (496 on a 560 surface).
pub fn max_height(surface: (f32, f32)) -> f32 {
    (finite(surface.1) - WINDOW_TOP - PAGE_MARGIN).max(0.0)
}

/// Where the window lands: `body` is the body's size, `capsule` the
/// capsule's extent as painted now, `edge` the end of it the window opens
/// towards and `piece` the rect of the piece it opens from. An end window
/// starts [`PIECE_LEAD`] before its piece, a start window ends [`PIECE_LEAD`]
/// after it, each moved inward only as far as it must to stay within its
/// capsule; the centre ignores the piece. The window's top is [`WINDOW_TOP`];
/// a window wider than its capsule grows inward from the capsule's outer
/// edge, and the side margins hold when the surface has room for them.
pub fn window(
    capsule: Rect,
    edge: Edge,
    piece: Rect,
    body: (f32, f32),
    surface: (f32, f32),
) -> Rect {
    let capsule = sane(capsule);
    let surface_w = finite(surface.0).max(0.0);
    // A window never leaves the surface. Wider than its room, it gives up
    // its side margins first (its body keeps its width and nothing is cut);
    // only a window wider than the whole surface narrows.
    let w = finite(body.0).max(0.0).min(surface_w);
    let margin = SIDE_MARGIN.min(((surface_w - w) / 2.0).max(0.0));
    let piece = sane(piece);
    let (inner, outer) = (capsule.x, capsule.x + capsule.w - w);
    let x = match edge {
        Edge::Start => (piece.x + piece.w + PIECE_LEAD - w).min(outer).max(inner),
        Edge::End => (piece.x - PIECE_LEAD).max(inner).min(outer),
        Edge::Centre => capsule.x + capsule.w / 2.0 - w / 2.0,
    };
    Rect {
        x: clamp(x, margin, surface_w - margin - w),
        y: WINDOW_TOP,
        w,
        h: finite(body.1).max(0.0).min(max_height(surface)),
    }
}

/// The pill a window grows from and closes towards: the piece's rect in the bar
/// (its corner radius is half its height).
pub fn pill(piece: Rect) -> Rect {
    sane(piece)
}

/// What the point lands on: the window, or neither. The bar's band (above
/// [`WINDOW_TOP`]) is never the window's, even while the window grows
/// through it.
pub fn hit(window: Rect, x: f32, y: f32) -> Hit {
    if y >= WINDOW_TOP && sane(window).contains(x, y) {
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

    /// A 28 px piece at `capsule`'s outer end for `edge`: where today's
    /// placement agrees with the piece's.
    fn outer(capsule: Rect, edge: Edge) -> Rect {
        let x = match edge {
            Edge::End => capsule.x + capsule.w - 28.0,
            _ => capsule.x,
        };
        r(x, 14.0, 28.0, 28.0)
    }

    #[test]
    fn an_end_window_starts_16_px_before_its_piece() {
        let capsule = r(1376.0, 8.0, 528.0, 40.0);
        let piece = r(1392.0, 14.0, 140.0, 28.0);
        let w = window(capsule, Edge::of_zone(3), piece, (360.0, 300.0), BAR);
        assert_eq!(w, r(1376.0, 56.0, 360.0, 300.0));
        assert_eq!(WINDOW_TOP, 56.0);
        assert_eq!(PIECE_LEAD, SIDE_MARGIN);
    }

    #[test]
    fn an_end_window_moves_inward_only_as_far_as_it_must() {
        let capsule = r(1376.0, 8.0, 528.0, 40.0);
        let piece = r(1700.0, 14.0, 28.0, 28.0);
        let w = window(capsule, Edge::End, piece, (420.0, 100.0), BAR);
        assert_eq!((w.x, w.x + w.w), (1484.0, 1904.0));
    }

    #[test]
    fn the_end_piece_keeps_todays_place() {
        let capsule = r(1376.0, 8.0, 528.0, 40.0);
        let piece = r(1872.0, 14.0, 28.0, 28.0);
        let w = window(capsule, Edge::End, piece, (240.0, 100.0), BAR);
        assert_eq!((w.x, w.x + w.w), (1664.0, 1904.0));
    }

    #[test]
    fn a_start_window_ends_16_px_after_its_piece() {
        let capsule = r(16.0, 8.0, 600.0, 40.0);
        let piece = r(400.0, 14.0, 28.0, 28.0);
        let w = window(capsule, Edge::of_zone(0), piece, (240.0, 200.0), BAR);
        assert_eq!((w.x, w.x + w.w), (204.0, 444.0));
    }

    #[test]
    fn a_start_window_moves_inward_only_as_far_as_it_must() {
        let capsule = r(16.0, 8.0, 600.0, 40.0);
        let piece = r(40.0, 14.0, 28.0, 28.0);
        let w = window(capsule, Edge::Start, piece, (240.0, 200.0), BAR);
        assert_eq!(w.x, 16.0);
    }

    #[test]
    fn a_centre_window_ignores_its_piece() {
        let capsule = r(800.0, 8.0, 300.0, 40.0);
        for zone in [1, 2] {
            for piece in [r(810.0, 14.0, 28.0, 28.0), r(1060.0, 14.0, 28.0, 28.0)] {
                let w = window(capsule, Edge::of_zone(zone), piece, (240.0, 200.0), BAR);
                assert_eq!((w.x, w.w), (830.0, 240.0));
                assert_eq!(w.x + w.w / 2.0, capsule.x + capsule.w / 2.0);
            }
        }
    }

    #[test]
    fn a_window_wider_than_its_capsule_grows_inward_as_today() {
        // End: the right edges agree, the window grows to the left.
        let end = r(1700.0, 8.0, 204.0, 40.0);
        let piece = outer(end, Edge::End);
        let w = window(end, Edge::End, piece, (420.0, 100.0), BAR);
        assert_eq!((w.x, w.x + w.w), (1484.0, 1904.0));
        // A piece in the middle of that capsule: still the right edges.
        let mid = r(1760.0, 14.0, 28.0, 28.0);
        let w = window(end, Edge::End, mid, (420.0, 100.0), BAR);
        assert_eq!((w.x, w.x + w.w), (1484.0, 1904.0));
        // A capsule that sits inside the margin: the margin holds.
        let tight = r(1890.0, 8.0, 100.0, 40.0);
        let w = window(
            tight,
            Edge::End,
            outer(tight, Edge::End),
            (320.0, 100.0),
            BAR,
        );
        assert_eq!(w.x + w.w, 1904.0);
        // Start: the left edges agree, the window grows to the right.
        let start = r(16.0, 8.0, 120.0, 40.0);
        let piece = outer(start, Edge::Start);
        let w = window(start, Edge::Start, piece, (420.0, 100.0), BAR);
        assert_eq!((w.x, w.x + w.w), (16.0, 436.0));
        let edge = r(5.0, 8.0, 120.0, 40.0);
        let w = window(
            edge,
            Edge::Start,
            outer(edge, Edge::Start),
            (420.0, 100.0),
            BAR,
        );
        assert_eq!(w.x, 16.0);
        // The far side of a start window on a narrow output stays inside.
        let narrow = (480.0, 560.0);
        let w = window(start, Edge::Start, piece, (420.0, 100.0), narrow);
        assert!(w.x >= 16.0 && w.x + w.w <= 480.0 - 16.0 + 0.01, "{w:?}");
        let far = r(300.0, 8.0, 160.0, 40.0);
        let w = window(
            far,
            Edge::Start,
            outer(far, Edge::Start),
            (420.0, 100.0),
            narrow,
        );
        assert_eq!((w.x, w.x + w.w), (44.0, 464.0));
    }

    #[test]
    fn a_non_finite_piece_stays_finite_and_inside_its_capsule() {
        let nan = f32::NAN;
        let end = r(1376.0, 8.0, 528.0, 40.0);
        let w = window(
            end,
            Edge::End,
            r(nan, 14.0, 28.0, 28.0),
            (360.0, 300.0),
            BAR,
        );
        assert!(
            w.x.is_finite() && w.x >= end.x && w.x + w.w <= 1904.0,
            "{w:?}"
        );
        let start = r(16.0, 8.0, 600.0, 40.0);
        let w = window(
            start,
            Edge::Start,
            r(nan, nan, nan, nan),
            (240.0, 200.0),
            BAR,
        );
        assert!(
            w.x.is_finite() && w.x >= start.x && w.x + w.w <= 616.0,
            "{w:?}"
        );
    }

    #[test]
    fn a_window_wider_than_its_room_gives_up_its_margins_before_it_narrows() {
        let capsule = r(300.0, 8.0, 100.0, 40.0);
        for edge in [Edge::Start, Edge::Centre, Edge::End] {
            let piece = outer(capsule, edge);
            // 420 on 440: the body keeps its width, the margins shrink to 10.
            let w = window(capsule, edge, piece, (420.0, 100.0), (440.0, 560.0));
            assert_eq!((w.x, w.w), (10.0, 420.0), "{edge:?}");
            // Wider than the whole surface: it narrows to the surface.
            let w = window(capsule, edge, piece, (500.0, 100.0), (440.0, 560.0));
            assert_eq!((w.x, w.w), (0.0, 440.0), "{edge:?}");
        }
    }

    #[test]
    fn the_window_is_capped_at_496_on_a_560_surface() {
        let capsule = r(1000.0, 8.0, 100.0, 40.0);
        let piece = outer(capsule, Edge::End);
        let w = window(capsule, Edge::End, piece, (320.0, 505.0), BAR);
        assert_eq!(w.h, 496.0);
        assert_eq!(max_height(BAR), 496.0);
        assert_eq!(
            window(capsule, Edge::End, piece, (320.0, 120.0), BAR).h,
            120.0
        );
    }

    #[test]
    fn the_pill_is_the_pieces_rect() {
        let piece = r(1040.0, 14.0, 60.0, 28.0);
        assert_eq!(pill(piece), piece);
        // Its radius is half its height.
        assert_eq!(pill(piece).h / 2.0, 14.0);
    }

    #[test]
    fn the_window_never_panics_and_stays_finite() {
        let nan = f32::NAN;
        let capsule = r(100.0, 8.0, 80.0, 40.0);
        let piece = outer(capsule, Edge::End);
        let cases = [
            window(capsule, Edge::End, piece, (420.0, 100.0), (300.0, 560.0)),
            window(
                r(nan, nan, nan, nan),
                Edge::Centre,
                r(nan, nan, nan, nan),
                (nan, nan),
                (nan, nan),
            ),
            window(capsule, Edge::Start, piece, (320.0, 100.0), (0.0, 0.0)),
        ];
        for w in cases {
            assert!([w.x, w.y, w.w, w.h].iter().all(|v| v.is_finite()), "{w:?}");
        }
        let narrow = window(capsule, Edge::End, piece, (420.0, 100.0), (300.0, 560.0));
        assert_eq!((narrow.x, narrow.w), (0.0, 300.0));
    }

    #[test]
    fn hit_tells_the_window_and_outside_apart() {
        let w = r(1544.0, 56.0, 360.0, 300.0);
        assert_eq!(hit(w, 1600.0, 100.0), Hit::Body);
        assert_eq!(hit(w, 100.0, 100.0), Hit::Outside);
        assert_eq!(hit(w, 1600.0, 400.0), Hit::Outside);
        // A window growing through the bar's band does not own the band.
        let growing = r(1544.0, 14.0, 360.0, 300.0);
        assert_eq!(hit(growing, 1600.0, 30.0), Hit::Outside);
        assert_eq!(hit(growing, 1600.0, 100.0), Hit::Body);
    }
}
