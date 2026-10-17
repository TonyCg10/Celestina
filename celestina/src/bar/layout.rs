//! Zone geometry, in logical px and without GL: where the zone capsules and
//! the Island sit on an output `output_w` wide (spec §6.2, decision 3).
//!
//! The Island is centred on the output. The centre zone's other pieces sit
//! in capsules either side of it; `start` and `end` hug the output's edges.
//! A zone with nothing to show has no rect. When the zones do not fit, the
//! inner capsules keep their width and the outer ones give way, down to
//! nothing, so no capsule ever overlaps its neighbour or leaves the output.

use celestina_paint::{Rect, Theme};
use celestina_wayland::Anchoring;

use crate::island::state::{CAPSULE_H, TOP_GAP};

/// The measured content width of each capsule, in logical px. A zone with
/// nothing to show is `0.0`; so is any width that is not a positive finite
/// number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoneWidths {
    pub start: f32,
    pub centre_left: f32,
    /// The Island capsule's current width (it grows with its page).
    pub island: f32,
    pub centre_right: f32,
    pub end: f32,
}

/// Where each capsule sits. `None` is a zone that draws no capsule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub start: Option<Rect>,
    pub centre_left: Option<Rect>,
    pub island: Rect,
    pub centre_right: Option<Rect>,
    pub end: Option<Rect>,
}

impl Placement {
    /// Every placed capsule, left to right, the Island included.
    #[must_use]
    pub fn rects(&self) -> Vec<Rect> {
        [
            self.start,
            self.centre_left,
            Some(self.island),
            self.centre_right,
            self.end,
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// How the bar's layer surface is anchored. The wayland runtime maps this to
/// its own `Anchoring`; changing the bar's edge is this function's change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchors {
    pub top: bool,
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
}

/// The surface anchoring: the top edge, across the whole output.
#[must_use]
pub const fn anchors() -> Anchors {
    Anchors {
        top: true,
        left: true,
        right: true,
        bottom: false,
    }
}

/// The runtime's anchoring for [`anchors`]: the top edge across the output
/// is [`Anchoring::TopWide`].
#[must_use]
pub const fn anchoring() -> Anchoring {
    let a = anchors();
    match (a.top, a.left, a.right, a.bottom) {
        (true, false, false, false) => Anchoring::Top,
        (true, true, true, true) => Anchoring::Fill,
        (false, false, false, false) => Anchoring::Centred,
        // The runtime knows no other set of edges; the bar's is top-wide.
        _ => Anchoring::TopWide,
    }
}

/// The space between the output's edge and the outermost capsule.
#[must_use]
pub fn edge_margin(theme: &Theme) -> f32 {
    theme.spacing.lg
}

/// The least space between two neighbouring capsules.
#[must_use]
pub fn min_gap(theme: &Theme) -> f32 {
    theme.spacing.md
}

/// `w` if it is a positive finite number, else nothing.
fn sane(w: f32) -> f32 {
    if w.is_finite() && w > 0.0 {
        w
    } else {
        0.0
    }
}

/// The capsule's rect, or `None` when no width is left for it.
fn capsule(x: f32, w: f32) -> Option<Rect> {
    (w > 0.0).then_some(Rect {
        x,
        y: TOP_GAP,
        w,
        h: CAPSULE_H,
    })
}

/// Places the capsules on an output `output_w` logical px wide.
///
/// The Island keeps its width (up to the output's) and sits exactly in the
/// middle. The centre capsules come next to it, each a gap away; the outer
/// capsules take what is left between the margin and their inner neighbour.
#[must_use]
pub fn place(theme: &Theme, output_w: f32, widths: ZoneWidths) -> Placement {
    let output_w = sane(output_w);
    let margin = edge_margin(theme);
    let gap = min_gap(theme);

    let island_w = sane(widths.island).min(output_w);
    let island = Rect {
        x: (output_w - island_w) / 2.0,
        y: TOP_GAP,
        w: island_w,
        h: CAPSULE_H,
    };
    let island_right = island.x + island.w;

    // The left side, inner to outer.
    let centre_left = {
        let w = sane(widths.centre_left).min(island.x - gap - margin);
        capsule(island.x - gap - w.max(0.0), w.max(0.0))
    };
    let left_edge = centre_left.map_or(island.x, |r| r.x);
    let start = {
        let w = sane(widths.start).min(left_edge - gap - margin).max(0.0);
        capsule(margin, w)
    };

    // The right side, mirrored.
    let centre_right = {
        let x = island_right + gap;
        let w = sane(widths.centre_right)
            .min(output_w - margin - x)
            .max(0.0);
        capsule(x, w)
    };
    let right_edge = centre_right.map_or(island_right, |r| r.x + r.w);
    let end = {
        let w = sane(widths.end)
            .min(output_w - margin - (right_edge + gap))
            .max(0.0);
        capsule(output_w - margin - w, w)
    };

    Placement {
        start,
        centre_left,
        island,
        centre_right,
        end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::dark()
    }

    fn widths(start: f32, cl: f32, cr: f32, end: f32) -> ZoneWidths {
        ZoneWidths {
            start,
            centre_left: cl,
            island: 140.0,
            centre_right: cr,
            end,
        }
    }

    /// The output widths of 1920, 2560 and 3840 px at scale 1.0 and 1.5.
    fn outputs() -> Vec<f32> {
        let mut all = Vec::new();
        for physical in [1920.0_f32, 2560.0, 3840.0] {
            for scale in [1.0_f32, 1.5] {
                all.push(physical / scale);
            }
        }
        all
    }

    fn assert_island_centred(p: &Placement, w: f32) {
        let centre = p.island.x + p.island.w / 2.0;
        assert!((centre - w / 2.0).abs() < 1e-3, "{centre} vs {}", w / 2.0);
    }

    fn assert_clear(p: &Placement, w: f32, gap: f32) {
        let rects = p.rects();
        for r in &rects {
            assert!(r.x >= -1e-3 && r.x + r.w <= w + 1e-3, "{r:?} on {w}");
            assert_eq!((r.y, r.h), (TOP_GAP, CAPSULE_H));
        }
        for pair in rects.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(b.x - (a.x + a.w) >= gap - 1e-3, "{a:?} touches {b:?}");
        }
    }

    #[test]
    fn the_island_is_exactly_centred_at_every_output_size() {
        for w in outputs() {
            let p = place(&theme(), w, widths(200.0, 0.0, 0.0, 300.0));
            assert_island_centred(&p, w);
            assert_eq!(p.island.w, 140.0);
        }
    }

    #[test]
    fn the_outer_zones_hug_the_margins_and_clear_each_other() {
        let t = theme();
        for w in outputs() {
            let p = place(&t, w, widths(200.0, 90.0, 110.0, 300.0));
            let start = p.start.expect("start");
            let end = p.end.expect("end");
            assert_eq!(start.x, edge_margin(&t));
            assert!((end.x + end.w - (w - edge_margin(&t))).abs() < 1e-3);
            assert_eq!((start.w, end.w), (200.0, 300.0));
            assert_eq!(p.centre_left.map(|r| r.w), Some(90.0));
            assert_eq!(p.centre_right.map(|r| r.w), Some(110.0));
            assert_clear(&p, w, min_gap(&t));
            assert_island_centred(&p, w);
        }
    }

    #[test]
    fn a_zone_with_nothing_has_no_rect() {
        let p = place(&theme(), 1920.0, widths(0.0, 0.0, 0.0, 0.0));
        assert_eq!(
            (p.start, p.centre_left, p.centre_right, p.end),
            (None, None, None, None)
        );
        assert_eq!(p.rects(), [p.island]);
        for bad in [-5.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let p = place(&theme(), 1920.0, widths(bad, bad, bad, bad));
            assert_eq!(p.rects().len(), 1, "{bad}");
        }
    }

    #[test]
    fn outer_zones_give_way_when_the_output_is_too_narrow() {
        let t = theme();
        let w = 600.0;
        let p = place(&t, w, widths(900.0, 0.0, 0.0, 900.0));
        let start = p.start.expect("start is clamped, not dropped");
        let end = p.end.expect("end is clamped, not dropped");
        assert!(start.w < 900.0 && end.w < 900.0);
        // The free space either side of the Island, minus margin and gap.
        let free = p.island.x - edge_margin(&t) - min_gap(&t);
        assert!((start.w - free).abs() < 1e-3);
        assert_clear(&p, w, min_gap(&t));
        assert_island_centred(&p, w);
    }

    #[test]
    fn the_inner_capsules_keep_their_width_and_the_outer_ones_yield_first() {
        let t = theme();
        let w = 700.0;
        let p = place(&t, w, widths(500.0, 120.0, 120.0, 500.0));
        assert_eq!(p.centre_left.map(|r| r.w), Some(120.0));
        assert_eq!(p.centre_right.map(|r| r.w), Some(120.0));
        assert_clear(&p, w, min_gap(&t));
        // Squeezed further, the outer zones vanish before the inner ones shrink.
        let p = place(&t, 440.0, widths(500.0, 120.0, 120.0, 500.0));
        assert!(p.start.is_none() && p.end.is_none());
        assert_eq!(p.centre_left.map(|r| r.w), Some(120.0));
        assert_clear(&p, 440.0, min_gap(&t));
    }

    #[test]
    fn a_hostile_width_never_leaves_the_output() {
        let t = theme();
        for w in [0.0, 1.0, 100.0, 1920.0] {
            let p = place(
                &t,
                w,
                ZoneWidths {
                    start: 1e9,
                    centre_left: 1e9,
                    island: 1e9,
                    centre_right: 1e9,
                    end: 1e9,
                },
            );
            assert!(p.island.w <= w.max(0.0));
            assert_clear(&p, w, 0.0);
        }
    }

    #[test]
    fn the_bar_is_anchored_across_the_top() {
        assert_eq!(
            anchors(),
            Anchors {
                top: true,
                left: true,
                right: true,
                bottom: false
            }
        );
    }

    #[test]
    fn the_runtime_anchors_the_bar_top_wide() {
        assert_eq!(anchoring(), Anchoring::TopWide);
    }
}
