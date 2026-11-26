//! The spotlight under a sheet's pill: the preview's `.beam`, in logical px.
//! A cone and a fainter core, each a trapezoid with a vertical gradient,
//! blurred; the pure geometry and constants. The GPU side lives in
//! `Painter::spotlight`.
#![forbid(unsafe_code)]

use celestina_wayland::Rect;

/// The foot's width, in pill widths.
pub const SPOT_WIDTH: f32 = 2.5;
/// The cone's height; its top is at the pill's bottom edge.
pub const CONE_H: f32 = 116.0;
/// The cone's top edge, as fractions of the foot width.
pub const CONE_TOP: (f32, f32) = (0.3, 0.7);
/// The cone's white alpha along its height `(offset, alpha)`.
pub const CONE_STOPS: [(f32, f32); 3] = [(0.0, 0.13), (0.42, 0.04), (0.85, 0.0)];
pub const CONE_SIGMA: f32 = 7.0;
/// The core's span, as fractions of the foot width.
pub const CORE_SPAN: (f32, f32) = (0.34, 0.66);
pub const CORE_H: f32 = 50.0;
/// The core's top edge, as fractions of its own width.
pub const CORE_TOP: (f32, f32) = (0.1, 0.9);
pub const CORE_STOPS: [(f32, f32); 2] = [(0.0, 0.09), (1.0, 0.0)];
pub const CORE_SIGMA: f32 = 5.0;

/// The foot of the beam: `SPOT_WIDTH` pills wide, centred on the pill, its
/// top at the pill's bottom edge, `CONE_H` tall.
#[must_use]
pub fn foot(pill: Rect) -> Rect {
    let w = pill.w * SPOT_WIDTH;
    Rect {
        x: pill.x + pill.w / 2.0 - w / 2.0,
        y: pill.y + pill.h,
        w,
        h: CONE_H,
    }
}

/// Everything the blurred beam can touch: the foot inflated by three
/// cone sigmas on every side.
#[must_use]
pub fn spot_bounds(pill: Rect) -> Rect {
    let f = foot(pill);
    let m = 3.0 * CONE_SIGMA;
    Rect {
        x: f.x - m,
        y: f.y - m,
        w: f.w + 2.0 * m,
        h: f.h + 2.0 * m,
    }
}

/// The cone's four corners in the foot's own frame (origin at the foot's
/// top-left): top-left, top-right, bottom-right, bottom-left.
#[must_use]
pub fn cone_corners(foot: Rect) -> [(f32, f32); 4] {
    [
        (foot.w * CONE_TOP.0, 0.0),
        (foot.w * CONE_TOP.1, 0.0),
        (foot.w, CONE_H),
        (0.0, CONE_H),
    ]
}

/// The core's four corners in the foot's own frame: it spans `CORE_SPAN` of
/// the foot, is `CORE_H` tall and narrows to `CORE_TOP` of its width on top.
#[must_use]
pub fn core_corners(foot: Rect) -> [(f32, f32); 4] {
    let (x0, x1) = (foot.w * CORE_SPAN.0, foot.w * CORE_SPAN.1);
    let w = x1 - x0;
    [
        (x0 + w * CORE_TOP.0, 0.0),
        (x0 + w * CORE_TOP.1, 0.0),
        (x1, CORE_H),
        (x0, CORE_H),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const PILL: Rect = Rect {
        x: 100.0,
        y: 8.0,
        w: 40.0,
        h: 28.0,
    };

    #[test]
    fn the_spot_is_two_and_a_half_pills_wide_and_starts_at_the_pills_foot() {
        let f = foot(PILL);
        assert_eq!(f.w, 100.0);
        assert_eq!(f.y, 36.0, "the pill's bottom edge");
        assert_eq!(f.x + f.w / 2.0, 120.0, "centred on the pill");
        assert_eq!(f.h, CONE_H);
        let b = spot_bounds(PILL);
        assert_eq!(b.x, f.x - 21.0);
        assert_eq!(b.y, f.y - 21.0);
        assert_eq!(b.w, f.w + 42.0);
        assert_eq!(b.h, f.h + 42.0);
    }

    #[test]
    fn the_cone_narrows_to_its_top_and_the_core_sits_inside_it() {
        let f = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: CONE_H,
        };
        let cone = cone_corners(f);
        assert!((cone[0].0 - 30.0).abs() < 1e-4 && (cone[1].0 - 70.0).abs() < 1e-4);
        assert_eq!((cone[2], cone[3]), ((100.0, CONE_H), (0.0, CONE_H)));
        let core = core_corners(f);
        assert!((core[0].0 - 37.2).abs() < 1e-4 && (core[1].0 - 62.8).abs() < 1e-4);
        assert_eq!((core[2], core[3]), ((66.0, CORE_H), (34.0, CORE_H)));
        assert!(core.iter().all(|c| c.0 >= 0.0 && c.0 <= f.w));
    }
}
