//! The liquid contour of the menu drop (CEL-D-013, drop D1). Under the
//! capsule's bottom edge `y0` the shape is a half-width profile `w(y)`,
//! mirrored around the piece's centre: the edge fillet, a thread (`a` at the
//! edge, the waist `m` at `yw`, then widening into the bulb) and the bulb (an
//! ellipse), joined by smooth maxima so every join is a liquid fillet and the
//! outline has no step. A negative waist splits the thread into two cones
//! that retract: the pinch-off. Ports of the approved preview's `smax`,
//! `profileW` and `liquidPath`. Every sub-path is closed and clockwise on
//! screen (y down), so a non-zero fill unites them with the rounded rects
//! they touch. These are free numbers: no constant of any one drop lives
//! here.

use celestina_paint::ShapePath;

/// How far above the edge a joined contour starts, px: it overlaps the
/// capsule's own glass in the same path.
pub const TOP_MARGIN: f32 = 2.0;
/// The sampling step down the profile, px.
const STEP: f32 = 0.5;
/// Samples along the bulb's ellipse (plus the first).
const BULB_SAMPLES: usize = 64;
/// A run whose half-width stays under this is empty, px.
const MIN_WIDTH: f32 = 0.05;
/// Smoothing of the edge/thread join and of the thread/bulb join, px.
const K1: f32 = 3.0;
const K2: f32 = 4.0;
/// How steeply the edge fillet's flank narrows, px per px.
const FLANK: f32 = 6.0;
/// No contour is longer than this, px (a guard against absurd input).
const MAX_SPAN: f32 = 8192.0;

/// The drop's bulb: an ellipse centred on the piece, `cy` down from the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bulb {
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

/// The numbers of one contour: the piece's centre `cx`, the capsule's edge
/// `y0`, the thread's half-width `a` at the edge, its waist `m` at `yw`, the
/// edge fillet's radius `f` and the bulb, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Liquid {
    pub cx: f32,
    pub y0: f32,
    pub a: f32,
    pub m: f32,
    pub yw: f32,
    pub f: f32,
    pub bulb: Option<Bulb>,
    /// The x-range of the capsule's flat bottom the liquid hangs from, if it
    /// hangs from one. Near a rounded end the root (the thread's `a` and the
    /// fillet) narrows on both sides alike to the room left on that flat
    /// bottom, so nothing pokes past the curve and the outline stays smooth.
    pub edge: Option<(f32, f32)>,
}

impl Liquid {
    fn finite(&self) -> bool {
        let base = [self.cx, self.y0, self.a, self.m, self.yw, self.f];
        base.iter().all(|v| v.is_finite())
            && self
                .bulb
                .is_none_or(|b| [b.cy, b.rx, b.ry].iter().all(|v| v.is_finite()))
            && self
                .edge
                .is_none_or(|(lo, hi)| lo.is_finite() && hi.is_finite())
    }
}

/// A smooth maximum of `a` and `b`: their maximum away from each other,
/// rounded within `k` of each other. Under 0.01 it is the plain maximum.
pub fn smax(a: f32, b: f32, k: f32) -> f32 {
    if k < 0.01 {
        return a.max(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.max(b) + h * h * k / 4.0
}

fn unit(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// The contour's half-width at depth `y` (`profileW`); 0 or negative where
/// there is no liquid. Non-finite input gives 0.
pub fn half_width(l: &Liquid, y: f32) -> f32 {
    if !l.finite() || !y.is_finite() {
        return 0.0;
    }
    // Near a capsule's rounded end the root narrows, on both sides alike, so
    // the attachment and its fillet end where the capsule's flat bottom ends.
    let room = l.edge.map_or(f32::INFINITY, |(lo, hi)| {
        (hi - l.cx).min(l.cx - lo).max(0.0)
    });
    // Up to half of that room goes to the fillet, so the root keeps a smooth
    // flare instead of a straight neck with corners.
    let (y0, m, yw) = (l.y0, l.m, l.yw);
    let f0 = l.f.min(l.a.max(0.0)).min(room * 0.5);
    let a = l.a.min(room - f0);
    let aa = a.max(0.0);
    let f = f0.min(aa);
    // The edge fillet and its flank.
    let e = if aa < 0.05 {
        -1e9
    } else if y <= y0 {
        aa + f
    } else if y < y0 + f {
        let d = y - (y0 + f);
        aa + f - (f * f - d * d).max(0.0).sqrt()
    } else {
        aa - FLANK * (y - y0 - f)
    };
    // The thread: from `a` at the edge to the waist, then out to the bulb.
    let th = if y <= yw {
        let s = ((yw - y) / (yw - y0).max(0.5)).min(1.0);
        m + (a - m) * s * s
    } else if let Some(b) = l.bulb.filter(|b| y < b.cy) {
        let s = (y - yw) / (b.cy - yw).max(0.5);
        m + (b.rx * 0.9 - m) * s * s
    } else {
        -1e9
    };
    let mut w = smax(e, th, K1);
    if let Some(b) = l.bulb {
        let d = (y - b.cy) / b.ry;
        let bw = if d.abs() < 1.0 {
            b.rx * (1.0 - d * d).sqrt()
        } else {
            0.0
        };
        let kb = K2 * unit((b.cy - y) / (0.5 * b.ry)) * unit((y - b.cy + b.ry) / (0.3 * b.ry));
        w = smax(w, bw, kb);
    }
    w
}

/// The contour's half-widths at depth `y`, right and left of the centre,
/// each `inset` px narrower. The profile is symmetric: an `edge` narrows its
/// root in [`half_width`] instead of cutting one side.
fn sides(l: &Liquid, y: f32, inset: f32) -> (f32, f32) {
    let w = half_width(l, y) - inset;
    (w, w)
}

/// Adds the liquid as closed clockwise polygons, one per run where the
/// half-widths are over 0.05 px (`liquidPath`): the right side down, the left
/// side up. Every half-width is `inset` px narrower (a non-finite or negative
/// inset counts as 0), and the contour starts `margin` px above the edge (a
/// non-finite or negative margin counts as 0: 2 where the capsule's own glass
/// is in the same path, so they overlap; 0 where the tree paints it).
/// Non-finite input adds nothing.
pub fn contour(path: &mut ShapePath, l: &Liquid, inset: f32, margin: f32) {
    if !l.finite() {
        return;
    }
    let inset = if inset.is_finite() {
        inset.max(0.0)
    } else {
        0.0
    };
    let margin = if margin.is_finite() {
        margin.max(0.0)
    } else {
        0.0
    };
    let top = l.y0 - margin;
    let mut bot = l.y0 + l.f.max(2.0);
    bot = match l.bulb {
        Some(b) => bot.max(b.cy + b.ry),
        None => bot.max(l.yw + 1.0),
    };
    if bot - top > MAX_SPAN {
        return;
    }
    let mut ys: Vec<f32> = Vec::new();
    let mut i = 0u32;
    loop {
        let y = top + STEP * i as f32;
        if y > bot {
            break;
        }
        ys.push(y);
        i += 1;
    }
    if let Some(b) = l.bulb {
        for i in 0..=BULB_SAMPLES {
            let a = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::PI * i as f32 / BULB_SAMPLES as f32;
            ys.push(b.cy + b.ry * a.sin());
        }
    }
    ys.push(bot);
    ys.sort_by(f32::total_cmp);

    let mut runs: Vec<Vec<(f32, f32, f32)>> = Vec::new();
    let mut open = false;
    for y in ys {
        if y < top || y > bot {
            continue;
        }
        let (wr, wl) = sides(l, y, inset);
        if wr + wl > 2.0 * MIN_WIDTH {
            if !open {
                runs.push(Vec::new());
                open = true;
            }
            if let Some(run) = runs.last_mut() {
                run.push((y, wr, wl));
            }
        } else {
            open = false;
        }
    }
    for run in runs.iter().filter(|r| r.len() >= 2) {
        for (n, &(y, wr, _)) in run.iter().enumerate() {
            if n == 0 {
                path.move_to(l.cx + wr, y);
            } else {
                path.line_to(l.cx + wr, y);
            }
        }
        for &(y, _, wl) in run.iter().rev() {
            path.line_to(l.cx - wl, y);
        }
        path.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::drop::{close_frame, open_frame, Source};
    use crate::sheet::geometry::CAPSULE_BOTTOM;
    use celestina_paint::Rect;

    const SRC: Source = Source {
        cx: 1800.0,
        w: 28.0,
    };
    const R: Rect = Rect {
        x: 1500.0,
        y: 56.0,
        w: 320.0,
        h: 200.0,
    };

    fn area(path: &ShapePath) -> f32 {
        path.flatten(8)
            .iter()
            .map(|poly| {
                (0..poly.len())
                    .map(|i| {
                        let (x0, y0) = poly[i];
                        let (x1, y1) = poly[(i + 1) % poly.len()];
                        (x0 * y1 - x1 * y0) / 2.0
                    })
                    .sum::<f32>()
            })
            .sum()
    }

    fn pendant() -> Liquid {
        Liquid {
            cx: 1800.0,
            y0: 48.0,
            a: 8.0,
            m: 6.5,
            yw: 48.0,
            f: 7.0,
            bulb: Some(Bulb {
                cy: 59.4,
                rx: 12.0,
                ry: 12.0,
            }),
            edge: None,
        }
    }

    fn pinched() -> Liquid {
        Liquid {
            m: -8.0,
            yw: 59.0,
            bulb: Some(Bulb {
                cy: 75.0,
                rx: 12.0,
                ry: 12.0,
            }),
            ..pendant()
        }
    }

    fn made(l: &Liquid) -> ShapePath {
        let mut p = ShapePath::new();
        contour(&mut p, l, 0.0, TOP_MARGIN);
        p
    }

    #[test]
    fn smax_is_max_away_from_k_and_rounds_within_it() {
        assert_eq!(smax(5.0, 0.0, 3.0), 5.0);
        assert_eq!(smax(1.0, 1.0, 4.0), 2.0);
        assert_eq!(smax(1.0, 3.0, 0.0), 3.0);
    }

    #[test]
    fn a_contour_is_closed_clockwise_polygons_one_per_run() {
        let one = made(&pendant());
        assert_eq!(one.flatten(8).len(), 1);
        assert!(area(&one) > 0.0);
        let two = made(&pinched());
        assert_eq!(two.flatten(8).len(), 2);
        assert!(area(&two) > 0.0);
    }

    #[test]
    fn the_liquid_contour_narrows_by_the_inset() {
        let l = Liquid {
            bulb: Some(Bulb {
                cy: CAPSULE_BOTTOM + 30.0,
                rx: 12.0,
                ry: 12.0,
            }),
            ..pendant()
        };
        let cy = CAPSULE_BOTTOM + 30.0;
        let inset = 0.25;
        let mut p = ShapePath::new();
        contour(&mut p, &l, inset, TOP_MARGIN);
        let reach = p
            .flatten(8)
            .iter()
            .flatten()
            .filter(|(_, y)| (y - cy).abs() < 0.01)
            .map(|(x, _)| x - l.cx)
            .fold(f32::MIN, f32::max);
        assert!(
            (reach - (half_width(&l, cy) - inset)).abs() < 0.01,
            "{reach}"
        );
        // A bad inset counts as 0.
        for bad in [-1.0, f32::NAN, f32::INFINITY] {
            let mut q = ShapePath::new();
            contour(&mut q, &l, bad, TOP_MARGIN);
            assert_eq!(q, made(&l), "inset {bad}");
        }
    }

    /// The capsule the liquid hangs from in the edge tests: 200 wide, its
    /// bottom at the edge `y0` (48), the radius 20.
    const CAPSULE: Rect = Rect {
        x: 1700.0,
        y: 8.0,
        w: 200.0,
        h: 40.0,
    };

    fn flat(capsule: Rect, radius: f32) -> (f32, f32) {
        (capsule.x + radius, capsule.x + capsule.w - radius)
    }

    /// Every frame of an open and a close, hung from `cx` under `capsule`.
    fn frames_at(cx: f32, capsule: Rect) -> Vec<(f32, Liquid)> {
        let src = Source { cx, w: 28.0 };
        let mut out = Vec::new();
        for i in 0..52 {
            let t = i as f32 * 0.01;
            if let Some(l) = open_frame(t, R, src, 1.15, 20.0).liquid {
                out.push((
                    t,
                    Liquid {
                        edge: Some(flat(capsule, 20.0)),
                        ..l
                    },
                ));
            }
        }
        for i in 42..=100 {
            let t = i as f32 * 0.01;
            if let Some(l) = close_frame(t, R, 20.0, src).liquid {
                out.push((
                    t,
                    Liquid {
                        edge: Some(flat(capsule, 20.0)),
                        ..l
                    },
                ));
            }
        }
        out
    }

    /// The capsule grown by a hair, so a vertex on its edge counts inside.
    fn rounded(capsule: Rect) -> ShapePath {
        let g = 0.02;
        ShapePath::rounded(
            Rect {
                x: capsule.x - g,
                y: capsule.y - g,
                w: capsule.w + 2.0 * g,
                h: capsule.h + 2.0 * g,
            },
            20.0 + g,
        )
    }

    fn assert_inside_at_the_edge(cx: f32, capsule: Rect) {
        let rr = rounded(capsule);
        let frames = frames_at(cx, capsule);
        assert!(frames.len() > 80);
        for (t, l) in frames {
            let mut p = ShapePath::new();
            contour(&mut p, &l, 0.0, TOP_MARGIN);
            for (x, y) in p.flatten(8).iter().flatten() {
                // Strictly above the edge: a bulb hanging under the curve's
                // foot touches the edge line without rising past the capsule.
                if *y <= l.y0 - 0.25 {
                    assert!(rr.contains(*x, *y), "cx {cx} t {t}: ({x}, {y}) pokes out");
                }
            }
        }
    }

    #[test]
    fn a_piece_near_a_rounded_end_keeps_the_liquids_top_inside_the_capsule() {
        // The piece's centre is 28 px from the capsule's right end, and from
        // its left one.
        assert_inside_at_the_edge(CAPSULE.x + CAPSULE.w - 28.0, CAPSULE);
        assert_inside_at_the_edge(CAPSULE.x + 28.0, CAPSULE);
    }

    #[test]
    fn a_capsule_holding_one_icon_keeps_the_liquids_top_inside_it() {
        let one = Rect {
            x: 1772.0,
            y: 8.0,
            w: 56.0,
            h: 40.0,
        };
        assert_inside_at_the_edge(1800.0, one);
        // The drop still hangs: the bulb is there.
        let (_, l) = frames_at(1800.0, one)
            .into_iter()
            .find(|(t, l)| *t > 0.3 && l.bulb.is_some())
            .expect("a frame with a bulb");
        let mut p = ShapePath::new();
        contour(&mut p, &l, 0.0, TOP_MARGIN);
        let b = l.bulb.expect("bulb");
        assert!(p.contains(1800.0, b.cy));
    }

    #[test]
    fn a_piece_in_mid_capsule_has_exactly_the_contour_it_had_without_an_edge() {
        for (t, l) in frames_at(1800.0, CAPSULE) {
            let mut with = ShapePath::new();
            contour(&mut with, &l, 0.0, TOP_MARGIN);
            let mut without = ShapePath::new();
            contour(&mut without, &Liquid { edge: None, ..l }, 0.0, TOP_MARGIN);
            assert_eq!(with, without, "t {t}");
        }
    }

    #[test]
    fn the_edge_limit_has_no_step() {
        // Near the end each side's half-width is continuous in y: the
        // allowance grows with depth, it never jumps.
        let cx = CAPSULE.x + CAPSULE.w - 28.0;
        let mut checked = 0;
        for (t, l) in frames_at(cx, CAPSULE) {
            let mut bot = l.y0 + l.f.max(2.0);
            bot = bot.max(l.bulb.map_or(l.yw + 1.0, |b| b.cy + b.ry));
            let mut y = l.y0 + 1.0;
            while y <= bot {
                let (r0, l0) = sides(&l, y, 0.0);
                let (r1, l1) = sides(&l, y + 0.05, 0.0);
                for (a, b) in [(r0, r1), (l0, l1)] {
                    if a.min(b) > 1.0 {
                        assert!((b - a).abs() <= 0.6, "t {t} y {y}: {a} -> {b}");
                        checked += 1;
                    }
                }
                y += 0.05;
            }
        }
        assert!(checked > 1000);
    }

    #[test]
    fn the_top_margin_is_the_callers() {
        let top = |margin: f32| {
            let mut p = ShapePath::new();
            contour(&mut p, &pendant(), 0.0, margin);
            p.bounds().map(|b| b.y)
        };
        assert_eq!(top(2.0), Some(46.0));
        assert_eq!(top(0.0), Some(48.0));
        for bad in [-3.0, f32::NAN] {
            assert_eq!(top(bad), Some(48.0), "margin {bad}");
        }
    }

    #[test]
    fn the_half_width_has_no_step_anywhere() {
        let mut lives = Vec::new();
        for i in 0..52 {
            let t = i as f32 * 0.01;
            if let Some(l) = open_frame(t, R, SRC, 1.15, 20.0).liquid {
                lives.push((t, l));
            }
        }
        for i in 42..=100 {
            let t = i as f32 * 0.01;
            if let Some(l) = close_frame(t, R, 20.0, SRC).liquid {
                lives.push((t, l));
            }
        }
        assert!(lives.len() > 80);
        for (t, l) in lives {
            let mut bot = l.y0 + l.f.max(2.0);
            bot = bot.max(l.bulb.map_or(l.yw + 1.0, |b| b.cy + b.ry));
            // Above the edge the profile is hidden under the capsule and
            // the preview ramps it in 0.5 px; the fillet then leaves the edge
            // on a vertical tangent (a quarter circle). Both are skipped: the
            // first pixel under the edge is that tangent, not a step.
            let mut y = l.y0 + 1.0;
            while y <= bot {
                let (w0, w1) = (half_width(&l, y), half_width(&l, y + 0.05));
                // The round ends of the bulb and of a cone are vertical
                // tangents too: under 1 px wide they are skipped.
                if w0.min(w1) > 1.0 {
                    assert!((w1 - w0).abs() <= 0.6, "t {t} y {y}: {w0} -> {w1}");
                }
                y += 0.05;
            }
        }
    }

    #[test]
    fn nothing_is_drawn_above_a_bulb_that_has_not_reached_the_edge() {
        let l = close_frame(0.55, R, 20.0, SRC)
            .liquid
            .expect("a liquid at 0.55");
        assert!(half_width(&l, CAPSULE_BOTTOM + 0.5) <= 0.05);
    }

    #[test]
    fn non_finite_inputs_add_nothing() {
        for bad in [
            Liquid {
                cx: f32::NAN,
                ..pendant()
            },
            Liquid {
                m: f32::INFINITY,
                ..pendant()
            },
            Liquid {
                bulb: Some(Bulb {
                    cy: f32::NAN,
                    rx: 1.0,
                    ry: 1.0,
                }),
                ..pendant()
            },
        ] {
            assert!(made(&bad).is_empty());
            assert_eq!(half_width(&bad, 50.0), 0.0);
        }
        assert_eq!(half_width(&pendant(), f32::NAN), 0.0);
    }

    #[test]
    fn near_a_rounded_end_the_root_narrows_on_both_sides_alike_with_no_corner() {
        // The default layout's last piece: its centre 29 px from the capsule's
        // end, 9 px inside the flat bottom's end.
        let capsule = Rect {
            x: 1500.0,
            y: 8.0,
            w: 400.0,
            h: 40.0,
        };
        let cx = capsule.x + capsule.w - 29.0;
        for (t, l) in frames_at(cx, capsule) {
            let mut p = ShapePath::new();
            contour(&mut p, &l, 0.0, TOP_MARGIN);
            for poly in p.flatten(1) {
                let (lo, hi) = poly
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
                assert!(
                    ((cx - lo) - (hi - cx)).abs() < 0.01,
                    "t {t}: lopsided {lo}..{hi} around {cx}"
                );
            }
            // From 1 px under the edge: the fillet leaves the edge on a vertical
            // tangent, as in the approved preview.
            let mut y = l.y0 + 1.0;
            while y < l.y0 + 12.0 {
                let (w0, w1) = (half_width(&l, y), half_width(&l, y + 0.05));
                if w0 > 1.0 && w1 > 1.0 {
                    assert!((w1 - w0).abs() <= 0.6, "t {t}: step at {y}: {w0} -> {w1}");
                }
                y += 0.05;
            }
        }
    }
}
