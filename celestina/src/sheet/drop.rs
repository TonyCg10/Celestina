//! The menu drop (CEL-D-013, drop D1): a capsule's piece hangs a drop from
//! its bottom edge on a thread, the thread pinches off near the bulb, and
//! the round bulb opens into the window; closing runs it backwards, the
//! window gathering into the bulb, which rises, meets the cone the capsule
//! reaches down and is absorbed. The drop is one continuous liquid contour
//! (`liquid::contour`), not a rounded rect with a neck. Pure frames of one
//! clock `t` in 0..=1: the window's rect and radius, the liquid, and the
//! content's opacity and blur. Ports of the approved preview's
//! `dropOpenV4` and `dropCloseV4` for `d1`. No painter, no GL, no clock of
//! its own.

use celestina_paint::{Rect, Seg, ShapePath};
use celestina_ui::{CubicBezier, Easing, ONE_UI, SINE_IN_OUT_80};

use super::geometry::CAPSULE_BOTTOM;
use super::liquid::{self, Bulb, Liquid};

/// The open's length, ms (the caller caps it by the motion ceiling).
pub const OPEN_MS: u64 = 460;
/// The close's length, ms.
pub const CLOSE_MS: u64 = 400;
/// Where in the close the pill and the piece's opacity go back.
pub const MERGE_AT: f32 = 0.8;
/// The end of the fall: where `CEL-D-016` holds `t` until its layout arrives.
pub const OPEN_HOLD: f32 = 0.52;

/// The close's rise: ease in.
const RISE: CubicBezier = CubicBezier(0.4, 0.0, 0.9, 0.6);

/// The bulb's radius, px.
const BULB_R: f32 = 12.0;
/// How far the bulb falls from the edge before it opens, px.
const FALL: f32 = 16.0;
/// The content's blur while it is hidden, px.
const BLUR: f32 = 12.0;

/// End of the swell and of the fall, in the open's `t`.
const SWELL_END: f32 = 0.28;
/// Share of the fall in which the thread thins, then pinches.
const THIN: f32 = 0.55;
const PINCH: f32 = 0.78;
/// How much the bulb stretches while it falls.
const STRETCH: f32 = 0.10;
/// Where the thread's waist sits between the edge and the bulb's top.
const THREAD: f32 = 0.72;
/// The close's gather ends here.
const GATHER_END: f32 = 0.42;
/// The fillet of the open's liquid is 7 px, the close's 6.
const OPEN_FILLET: f32 = 7.0;
const CLOSE_FILLET: f32 = 6.0;

/// Where the drop hangs from: the live piece's centre and width. D1 reads
/// only the centre: the bulb is the same whatever the piece's width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    pub cx: f32,
    pub w: f32,
}

impl Source {
    pub fn of(pill: Rect) -> Source {
        Source {
            cx: pill.x + pill.w / 2.0,
            w: pill.w,
        }
    }
}

/// One frame of a drop: the window or the bulb, the liquid and the content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropFrame {
    /// The window's rect; in a liquid-only frame the bulb's box, for clips
    /// and travels.
    pub rect: Rect,
    pub radius: f32,
    /// Whether `rect` is painted as a rounded rect.
    pub drawn: bool,
    /// The liquid under the capsule's edge, if any.
    pub liquid: Option<Liquid>,
    /// The glass's opacity; the drop never fades it.
    pub glass: f32,
    pub content: f32,
    pub blur: f32,
}

impl DropFrame {
    /// The window at rest: no liquid, opaque, sharp.
    pub fn window(rect: Rect, radius: f32) -> DropFrame {
        DropFrame {
            rect,
            radius,
            drawn: true,
            liquid: None,
            glass: 1.0,
            content: 1.0,
            blur: 0.0,
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn unit(v: f32) -> f32 {
    if v.is_nan() {
        1.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

fn clean(src: Source) -> Source {
    let f = |v: f32| if v.is_finite() { v } else { 0.0 };
    Source {
        cx: f(src.cx),
        w: f(src.w).max(0.0),
    }
}

fn clean_rect(r: Rect) -> Rect {
    if [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) {
        r
    } else {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }
}

fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
    Rect {
        x: lerp(a.x, b.x, k),
        y: lerp(a.y, b.y, k),
        w: lerp(a.w, b.w, k),
        h: lerp(a.h, b.h, k),
    }
}

/// The box of an ellipse bulb centred on `cx`.
fn bulb_box(cx: f32, b: Bulb) -> Rect {
    Rect {
        x: cx - b.rx,
        y: b.cy - b.ry,
        w: 2.0 * b.rx,
        h: 2.0 * b.ry,
    }
}

/// A liquid-only frame: the bulb's box carried in `rect`, nothing drawn as a
/// rounded rect, the content hidden.
fn liquid_frame(cx: f32, liquid: Liquid, bulb: Bulb) -> DropFrame {
    DropFrame {
        rect: bulb_box(cx, bulb),
        radius: bulb.rx.min(bulb.ry),
        drawn: false,
        liquid: Some(liquid),
        glass: 1.0,
        content: 0.0,
        blur: BLUR,
    }
}

/// The open at clock `t` (0..=1; NaN counts as 1) towards the window `to`,
/// from the piece `src`. `overshoot` is the theme's (1.15) and `radius` the
/// window's corner radius at rest (`theme.radius_lg`); a non-finite or
/// negative `radius` counts as 0.
pub fn open_frame(t: f32, to: Rect, src: Source, overshoot: f32, radius: f32) -> DropFrame {
    let t = unit(t);
    let to = clean_rect(to);
    let cx = clean(src).cx;
    let radius = if radius.is_finite() {
        radius.max(0.0)
    } else {
        0.0
    };
    let overshoot = if overshoot.is_finite() {
        overshoot
    } else {
        0.0
    };
    if t >= 1.0 {
        return DropFrame::window(to, radius);
    }
    let y0 = CAPSULE_BOTTOM;
    let cy_end = y0 + FALL + BULB_R;
    if t < SWELL_END {
        // The swell: a bulb grows on the edge.
        let u = ONE_UI.eval(t / SWELL_END);
        let r = lerp(3.0, BULB_R, u);
        let cy = y0 + lerp(-0.5, 0.95, u) * r;
        let bulb = Bulb { cy, rx: r, ry: r };
        let liquid = Liquid {
            cx,
            y0,
            a: lerp(r, 8.0, u),
            m: lerp(r, 6.5, u),
            yw: y0 + (cy - r - y0) * 0.5,
            f: OPEN_FILLET,
            bulb: Some(bulb),
            edge: None,
        };
        return liquid_frame(cx, liquid, bulb);
    }
    if t < OPEN_HOLD {
        // The fall: the bulb hangs, stretches a thread and pinches it off.
        let u = SINE_IN_OUT_80.eval((t - SWELL_END) / (OPEN_HOLD - SWELL_END));
        let cy = lerp(y0 + 0.95 * BULB_R, cy_end, u);
        let el = STRETCH * (std::f32::consts::PI * (u / PINCH * 0.9).clamp(0.0, 1.0)).sin();
        let ry = BULB_R * (1.0 + el);
        let rx = BULB_R * (1.0 - el * 0.5);
        let yw = y0 + (cy - ry - y0).max(0.0) * THREAD;
        let m = if u < THIN {
            lerp(6.5, 1.4, ONE_UI.eval(u / THIN))
        } else if u < PINCH {
            lerp(1.4, 0.0, (u - THIN) / (PINCH - THIN))
        } else {
            -lerp(0.0, 10.0, (u - PINCH) / (1.0 - PINCH))
        };
        let a = if u < PINCH {
            lerp(8.0, 4.0, u / PINCH)
        } else {
            lerp(4.0, 1.5, (u - PINCH) / (1.0 - PINCH))
        };
        let bulb = Bulb { cy, rx, ry };
        let liquid = Liquid {
            cx,
            y0,
            a,
            m,
            yw,
            f: OPEN_FILLET,
            bulb: Some(bulb),
            edge: None,
        };
        return liquid_frame(cx, liquid, bulb);
    }
    // The open: the round bulb lands on the window, the thread's stump
    // retracts into the edge.
    let u = unit((t - OPEN_HOLD) / (1.0 - OPEN_HOLD));
    let k = Easing::OutBack(overshoot).eval(u);
    let bulb_at_rest = Rect {
        x: cx - BULB_R,
        y: cy_end - BULB_R,
        w: 2.0 * BULB_R,
        h: 2.0 * BULB_R,
    };
    let a = lerp(1.5, 0.0, (u / 0.2).clamp(0.0, 1.0));
    let liquid = (a > 0.05).then_some(Liquid {
        cx,
        y0,
        a,
        m: -10.0 - 40.0 * u,
        yw: y0 + FALL * THREAD,
        f: OPEN_FILLET,
        bulb: None,
        edge: None,
    });
    DropFrame {
        rect: lerp_rect(bulb_at_rest, to, k),
        radius: lerp(BULB_R, radius, k.clamp(0.0, 1.0)),
        drawn: true,
        liquid,
        glass: 1.0,
        content: unit((t - 0.6) / 0.34),
        blur: BLUR * (1.0 - ONE_UI.eval(u)),
    }
}

/// The close at clock `t` (0..=1; NaN counts as 1) from the window `from` as
/// it was shown, at its corner radius `from_radius` (a non-finite or negative
/// one counts as 0), back into the live piece `src`.
pub fn close_frame(t: f32, from: Rect, from_radius: f32, src: Source) -> DropFrame {
    let t = unit(t);
    let from = clean_rect(from);
    let from_radius = if from_radius.is_finite() {
        from_radius.max(0.0)
    } else {
        0.0
    };
    let cx = clean(src).cx;
    let y0 = CAPSULE_BOTTOM;
    let cy_start = y0 + FALL + BULB_R;
    if t < GATHER_END {
        // The gather: the window shrinks into the round bulb.
        let u = ONE_UI.eval(t / GATHER_END);
        let bulb_box = Rect {
            x: cx - BULB_R,
            y: cy_start - BULB_R,
            w: 2.0 * BULB_R,
            h: 2.0 * BULB_R,
        };
        return DropFrame {
            rect: lerp_rect(from, bulb_box, u),
            radius: lerp(from_radius, BULB_R, u),
            drawn: true,
            liquid: None,
            glass: 1.0,
            content: 1.0 - unit(t / 0.28),
            blur: BLUR * unit(t / 0.3).powf(0.7),
        };
    }
    // The rise: the bulb climbs, the capsule reaches down a cone, they meet.
    let u = RISE.eval((t - GATHER_END) / (1.0 - GATHER_END));
    let cy = lerp(cy_start, y0 - BULB_R * 0.7, u);
    let r = lerp(
        BULB_R,
        BULB_R * 0.5,
        ((y0 + BULB_R - cy) / (2.0 * BULB_R)).clamp(0.0, 1.0),
    );
    let g = cy - r - y0;
    let chord = if (cy - y0).abs() < r {
        (r * r - (cy - y0) * (cy - y0)).sqrt()
    } else {
        0.0
    };
    let (mut a, m);
    if g > 2.0 {
        a = lerp(0.0, 4.0, ((12.0 - g) / 9.0).clamp(0.0, 1.0));
        m = -(g - 2.0) * 0.9;
    } else {
        let s = ((2.0 - g) / 5.0).clamp(0.0, 1.0);
        a = lerp(4.0, r * 0.9, s).max(chord * 0.95);
        m = lerp(0.0, r * 0.75, s);
    }
    a *= 1.0 - ((t - 0.92) / 0.08).clamp(0.0, 1.0);
    let bulb = Bulb { cy, rx: r, ry: r };
    let liquid = Liquid {
        cx,
        y0,
        a,
        m,
        yw: y0 + g.max(0.0) * 0.5,
        f: CLOSE_FILLET,
        bulb: Some(bulb),
        edge: None,
    };
    liquid_frame(cx, liquid, bulb)
}

/// The number of contours of `path`.
fn contours(path: &ShapePath) -> usize {
    path.segs()
        .iter()
        .filter(|s| matches!(s, Seg::Move(..)))
        .count()
}

/// The one path of a capsule's glass and its windows: the capsule's rounded
/// rect (when it is part of the shape), then per frame its rounded rect when
/// it is drawn and its liquid contour, all hanging from the capsule's bottom
/// edge. The sources are kept for the caller's pairing; the liquid already
/// carries its own centre. With a capsule, each liquid is held to the
/// capsule's flat bottom (a piece near a rounded end hangs no top out past
/// the curve) and starts 2 px above the edge, inside the capsule's glass;
/// without one it starts at the edge, so it does not overlap the capsule
/// glass the bar's tree paints.
///
/// `inset` is the device inset in logical px (callers pass `0.5 / dpr`).
/// femtovg 0.27 (`path/cache.rs`, `expand_fill`, 537-594) gives a path of
/// exactly one convex contour half a fringe (the fill inset by `woff`) and
/// every other path a full fringe outward, so the edge of a joined path sits
/// half a device px further out than the same rect through the tree. Every
/// part is built `inset` px inside to cancel that, except when the path is
/// exactly one rounded rect (no capsule, one drawn frame, no liquid run): a
/// concave contour, such as a liquid alone, gets the full fringe. A
/// non-finite or negative `inset` counts as 0.
pub fn shape(
    capsule: Option<(Rect, f32)>,
    frames: &[(DropFrame, Source)],
    inset: f32,
) -> ShapePath {
    let inset = if inset.is_finite() {
        inset.max(0.0)
    } else {
        0.0
    };
    // The capsule's flat bottom, and the room the liquid has above its edge.
    let edge = capsule.map(|(rect, radius)| {
        let r = radius.min(rect.w / 2.0).min(rect.h / 2.0).max(0.0);
        (rect.x + r, rect.x + rect.w - r)
    });
    let margin = if capsule.is_some() {
        liquid::TOP_MARGIN
    } else {
        0.0
    };
    let build = |d: f32, with_liquid: bool| {
        let mut path = ShapePath::new();
        let rrect = |path: &mut ShapePath, r: Rect, radius: f32| {
            let r = Rect {
                x: r.x + d,
                y: r.y + d,
                w: r.w - 2.0 * d,
                h: r.h - 2.0 * d,
            };
            path.rounded_rect(r, (radius - d).max(0.0));
        };
        if let Some((rect, radius)) = capsule {
            rrect(&mut path, rect, radius);
        }
        for (frame, _) in frames {
            if frame.drawn {
                rrect(&mut path, frame.rect, frame.radius);
            }
            if let (true, Some(l)) = (with_liquid, &frame.liquid) {
                liquid::contour(&mut path, &Liquid { edge, ..*l }, d, margin);
            }
        }
        path
    };
    let plain = build(0.0, true);
    let lone_rect = capsule.is_none()
        && matches!(frames, [(f, _)] if f.drawn)
        && contours(&plain) == 1
        && contours(&build(0.0, false)) == 1;
    if inset == 0.0 || lone_rect {
        return plain;
    }
    build(inset, true)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    const OVERSHOOT: f32 = 1.15;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn rect_is(r: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
        near(r.x, x) && near(r.y, y) && near(r.w, w) && near(r.h, h)
    }

    fn open(t: f32) -> DropFrame {
        open_frame(t, R, SRC, OVERSHOOT, 20.0)
    }

    fn close(t: f32) -> DropFrame {
        close_frame(t, R, 20.0, SRC)
    }

    fn lq(f: &DropFrame) -> Liquid {
        f.liquid.expect("a liquid in this frame")
    }

    fn bulb_is(b: Option<Bulb>, cy: f32, rx: f32, ry: f32) -> bool {
        b.is_some_and(|b| near(b.cy, cy) && near(b.rx, rx) && near(b.ry, ry))
    }

    fn subpaths(f: &DropFrame) -> usize {
        shape(None, &[(*f, SRC)], 0.0).flatten(8).len()
    }

    #[test]
    fn the_drop_swells_from_a_3_px_bulb_on_the_edge() {
        let f = open(0.0);
        let l = lq(&f);
        assert!(near(l.a, 3.0) && near(l.m, 3.0) && near(l.f, 7.0));
        assert!(bulb_is(l.bulb, 46.5, 3.0, 3.0), "{:?}", l.bulb);
        assert!(!f.drawn);
        assert!(near(f.content, 0.0) && near(f.blur, 12.0) && near(f.glass, 1.0));
    }

    #[test]
    fn the_bulb_hangs_round_at_the_end_of_the_swell() {
        let l = lq(&open(0.28));
        assert!(bulb_is(l.bulb, 59.4, 12.0, 12.0), "{:?}", l.bulb);
        assert!(near(l.a, 8.0) && near(l.m, 6.5));
    }

    #[test]
    fn the_thread_thins_then_pinches_into_two_pieces() {
        let ms: Vec<(f32, f32)> = (2801..5200)
            .map(|i| i as f32 / 10_000.0)
            .map(|t| (t, lq(&open(t)).m))
            .collect();
        assert!(ms.iter().any(|(_, m)| *m > 0.0 && *m <= 1.5));
        let (t, _) = ms
            .iter()
            .copied()
            .find(|(_, m)| *m < 0.0)
            .expect("the thread pinches");
        assert_eq!(subpaths(&open(t)), 2, "t {t}");
        assert_eq!(subpaths(&open(0.3)), 1);
    }

    #[test]
    fn the_open_starts_from_the_round_bulb_and_lands_on_the_window() {
        let f = open(0.52);
        assert!(f.drawn);
        assert!(rect_is(f.rect, 1788.0, 64.0, 24.0, 24.0), "{:?}", f.rect);
        assert!(near(f.radius, 12.0));
        let l = lq(&f);
        assert!(l.bulb.is_none() && near(l.a, 1.5));
        assert!((1..48).any(|i| open(0.52 + i as f32 * 0.01).rect.w > 320.0));
        let end = open(1.0);
        assert_eq!(end.rect, R);
        assert!(near(end.radius, 20.0) && near(end.content, 1.0) && near(end.blur, 0.0));
        assert!(end.liquid.is_none() && end.drawn);
    }

    #[test]
    fn the_open_content_fades_in_from_60_percent() {
        assert!(near(open(0.6).content, 0.0));
        assert!(near(open(0.77).content, 0.5));
        assert!(near(open(0.94).content, 1.0));
        assert!(near(open(0.52).blur, 12.0));
        assert!(near(open(1.0).blur, 0.0));
        assert!(near(open(0.3).content, 0.0) && near(open(0.3).blur, 12.0));
    }

    #[test]
    fn the_close_gathers_into_a_round_bulb_16_px_under_the_edge() {
        let f = close(0.42);
        assert!(!f.drawn);
        let l = lq(&f);
        assert!(bulb_is(l.bulb, 76.0, 12.0, 12.0), "{:?}", l.bulb);
        assert!(near(l.f, 6.0));
        let g = close(0.30);
        assert!(g.drawn && g.liquid.is_none());
        assert!(g.rect.w < R.w && g.rect.w > 24.0, "{:?}", g.rect);
        assert!(g.rect.x > R.x && g.rect.x < 1788.0);
        assert!(g.rect.y > 56.0 && g.rect.y < 64.0);
        let start = close(0.0);
        assert_eq!(start.rect, R);
        assert!(near(start.content, 1.0) && near(start.blur, 0.0));
        assert!(near(close(0.28).content, 0.0) && near(close(0.3).blur, 12.0));
    }

    #[test]
    fn the_capsule_reaches_down_and_the_bulb_merges() {
        let gap = |f: &DropFrame| {
            let b = lq(f).bulb.expect("bulb");
            b.cy - b.ry - CAPSULE_BOTTOM
        };
        let frames: Vec<DropFrame> = (4200..10_000).map(|i| close(i as f32 / 10_000.0)).collect();
        let reaching = frames
            .iter()
            .find(|f| gap(f) < 12.0 && gap(f) > 2.0)
            .expect("a frame with the capsule reaching down");
        assert!(lq(reaching).a > 0.0 && lq(reaching).m < 0.0);
        let merged = frames
            .iter()
            .find(|f| gap(f) < 2.0)
            .expect("a frame where the bulb has merged");
        assert!(lq(merged).m > 0.0);
        assert_eq!(subpaths(merged), 1);
    }

    #[test]
    fn the_capsule_absorbs_the_drop() {
        let l = lq(&close(1.0));
        for i in 1..=40 {
            let y = CAPSULE_BOTTOM + i as f32 * 0.5;
            assert!(liquid::half_width(&l, y) <= 0.05, "y {y}");
        }
    }

    #[test]
    fn the_close_follows_the_live_piece() {
        let moved = Source {
            cx: 1700.0,
            w: 28.0,
        };
        for t in [0.5, 0.7, 0.9] {
            let a = close_frame(t, R, 20.0, SRC);
            let b = close_frame(t, R, 20.0, moved);
            assert!(near(a.rect.x - b.rect.x, 100.0), "t {t}");
            assert!(near(lq(&a).cx - lq(&b).cx, 100.0));
        }
    }

    #[test]
    fn a_degenerate_piece_never_yields_non_finite_shapes() {
        let sources = [
            Source { cx: 1800.0, w: 0.0 },
            Source {
                cx: 1800.0,
                w: f32::NAN,
            },
            Source {
                cx: f32::NAN,
                w: 28.0,
            },
            Source {
                cx: f32::INFINITY,
                w: f32::NAN,
            },
        ];
        let times = [0.0, 0.1, 0.28, 0.4, 0.52, 0.8, 1.0, f32::NAN, -1.0, 2.0];
        for src in sources {
            for t in times {
                for f in [
                    open(t),
                    open_frame(t, R, src, OVERSHOOT, 20.0),
                    close_frame(t, R, 20.0, src),
                    close_frame(t, Rect { x: f32::NAN, ..R }, 20.0, src),
                ] {
                    let nums = [
                        f.rect.x, f.rect.y, f.rect.w, f.rect.h, f.radius, f.glass, f.content,
                        f.blur,
                    ];
                    assert!(nums.iter().all(|v| v.is_finite()), "{src:?} t {t}: {f:?}");
                    let path = shape(None, &[(f, src)], 0.0);
                    if let Some(b) = path.bounds() {
                        assert!([b.x, b.y, b.w, b.h].iter().all(|v| v.is_finite()));
                    }
                }
            }
        }
        let f = open_frame(0.5, R, SRC, f32::NAN, 20.0);
        assert!(f.rect.w.is_finite());
    }

    #[test]
    fn shape_unites_the_capsule_and_the_drop_in_one_path() {
        let capsule = Rect {
            x: 1700.0,
            y: 8.0,
            w: 200.0,
            h: 40.0,
        };
        let f = open(0.34);
        let l = lq(&f);
        let b = l.bulb.expect("bulb");
        let p = shape(Some((capsule, 20.0)), &[(f, SRC)], 0.0);
        assert!(p.contains(1800.0, 28.0));
        assert!(p.contains(1800.0, (CAPSULE_BOTTOM + l.yw) / 2.0));
        assert!(p.contains(1800.0, b.cy));
        assert!(!p.contains(1850.0, b.cy));
        // The bulb is not painted on the capsule's side of the thread.
        assert!(!p.contains(1800.0 + 20.0, CAPSULE_BOTTOM + 3.0));
        // The capsule and the liquid are two sub-paths of one fill.
        assert_eq!(p.flatten(8).len(), 2);
    }

    #[test]
    fn the_joined_path_is_inset_by_half_a_device_pixel() {
        let capsule = Rect {
            x: 1700.0,
            y: 8.0,
            w: 200.0,
            h: 40.0,
        };
        let f = open(0.34);
        let p = shape(Some((capsule, 20.0)), &[(f, SRC)], 0.25);
        assert_eq!(
            p.segs().first(),
            Some(&Seg::Move(1700.0 + 0.25 + 19.75, 8.25))
        );
        assert!(!p.contains(1700.1, 28.0));
        assert!(p.contains(1700.5, 28.0));
        assert!(p.contains(1800.0, 28.0));
        // The liquid is narrower by the same inset.
        let wide = shape(Some((capsule, 20.0)), &[(f, SRC)], 0.0);
        let b = lq(&f).bulb.expect("bulb");
        let (x0, x1) = (1800.0 - b.rx, 1800.0 + b.rx);
        assert!(wide.contains(x1 - 0.1, b.cy) && !p.contains(x1 - 0.1, b.cy));
        assert!(wide.contains(x0 + 0.1, b.cy) && !p.contains(x0 + 0.1, b.cy));
        // A lone window is one contour: not inset.
        let win = DropFrame::window(
            Rect {
                x: 1700.0,
                y: 56.0,
                w: 200.0,
                h: 100.0,
            },
            20.0,
        );
        let alone = shape(None, &[(win, SRC)], 0.25);
        assert_eq!(alone.segs().first(), Some(&Seg::Move(1720.0, 56.0)));
        // Bad insets count as 0.
        for bad in [-1.0, f32::NAN] {
            assert_eq!(
                shape(Some((capsule, 20.0)), &[(f, SRC)], bad),
                wide,
                "inset {bad}"
            );
        }
    }

    const CAPSULE: Rect = Rect {
        x: 1700.0,
        y: 8.0,
        w: 200.0,
        h: 40.0,
    };

    #[test]
    fn a_liquid_without_a_capsule_is_inset_like_any_concave_contour() {
        // Only a path of exactly one rounded rect keeps its edge; a liquid
        // alone (one concave contour) gets femtovg's full fringe, so it is
        // built inside by the inset too.
        let f = open(0.34);
        assert!(!f.drawn);
        let b = lq(&f).bulb.expect("bulb");
        let x1 = 1800.0 + b.rx;
        let wide = shape(None, &[(f, SRC)], 0.0);
        let inset = shape(None, &[(f, SRC)], 0.25);
        assert_eq!(subpaths(&f), 1, "premise: one contour");
        assert!(wide.contains(x1 - 0.1, b.cy) && !inset.contains(x1 - 0.1, b.cy));
        // A lone drawn window, and a drawn window whose liquid adds nothing,
        // are one rounded rect: not inset.
        let win = DropFrame::window(R, 20.0);
        assert_eq!(
            shape(None, &[(win, SRC)], 0.25).segs().first(),
            Some(&Seg::Move(1520.0, 56.0))
        );
        let with_nothing = DropFrame {
            liquid: Some(Liquid {
                a: 0.0,
                m: -10.0,
                bulb: None,
                ..lq(&open(0.52))
            }),
            ..win
        };
        assert_eq!(
            shape(None, &[(with_nothing, SRC)], 0.25).segs().first(),
            Some(&Seg::Move(1520.0, 56.0))
        );
        // Two frames are never one rounded rect.
        let two = shape(None, &[(win, SRC), (win, SRC)], 0.25);
        assert_eq!(two.segs().first(), Some(&Seg::Move(1520.0, 56.25)));
    }

    #[test]
    fn the_liquid_stays_inside_a_rounded_capsule_end_when_joined() {
        // A piece 28 px from the capsule's right end: the contour's part at
        // or above the edge lies inside the capsule's rounded rect.
        let src = Source {
            cx: CAPSULE.x + CAPSULE.w - 28.0,
            w: 24.0,
        };
        let grown = Rect {
            x: CAPSULE.x - 0.02,
            y: CAPSULE.y - 0.02,
            w: CAPSULE.w + 0.04,
            h: CAPSULE.h + 0.04,
        };
        let rr = ShapePath::rounded(grown, 20.02);
        // Strictly above the edge: a bulb hanging under the curve's foot
        // touches the edge line without rising past the capsule.
        const ABOVE: f32 = 0.25;
        let mut seen = 0;
        for i in 0..=100 {
            for f in [
                open_frame(i as f32 / 100.0, R, src, OVERSHOOT, 20.0),
                close_frame(i as f32 / 100.0, R, 20.0, src),
            ] {
                let p = shape(Some((CAPSULE, 20.0)), &[(f, src)], 0.0);
                for poly in p.flatten(8).iter().skip(1) {
                    for (x, y) in poly.iter().filter(|(_, y)| *y <= CAPSULE_BOTTOM - ABOVE) {
                        assert!(rr.contains(*x, *y), "t {i}%: ({x}, {y}) pokes out");
                        seen += 1;
                    }
                }
            }
        }
        assert!(seen > 500, "{seen}");
    }

    #[test]
    fn a_joined_liquid_starts_2_px_above_the_edge_and_a_loose_one_at_it() {
        let f = open(0.2);
        let top = |p: &ShapePath, n: usize| {
            p.flatten(8)
                .get(n)
                .map(|poly| poly.iter().map(|(_, y)| *y).fold(f32::MAX, f32::min))
        };
        let joined = shape(Some((CAPSULE, 20.0)), &[(f, SRC)], 0.0);
        let top_joined = top(&joined, 1).expect("the liquid");
        assert!(
            (top_joined - (CAPSULE_BOTTOM - 2.0)).abs() < 1e-3,
            "{top_joined}"
        );
        let loose = shape(None, &[(f, SRC)], 0.0);
        let top_loose = top(&loose, 0).expect("the liquid");
        assert!((top_loose - CAPSULE_BOTTOM).abs() < 1e-3, "{top_loose}");
    }

    #[test]
    fn the_window_radius_is_the_themes_not_a_constant() {
        for radius in [16.0, 24.0] {
            let end = open_frame(1.0, R, SRC, OVERSHOOT, radius);
            assert!(near(end.radius, radius));
            // The open lands on it.
            let late = open_frame(0.99, R, SRC, OVERSHOOT, radius);
            assert!((late.radius - radius).abs() < 1.0, "{}", late.radius);
            assert!(near(
                open_frame(0.52, R, SRC, OVERSHOOT, radius).radius,
                12.0
            ));
        }
        for bad in [f32::NAN, -4.0] {
            assert_eq!(open_frame(1.0, R, SRC, OVERSHOOT, bad).radius, 0.0);
            assert_eq!(close_frame(0.0, R, bad, SRC).radius, 0.0);
        }
    }

    #[test]
    fn the_close_gathers_from_the_radius_it_was_shown_at() {
        // From a window mid-open at radius 13.5, not from 20.
        let start = close_frame(0.0, R, 13.5, SRC);
        assert!(near(start.radius, 13.5));
        let mid = close_frame(0.2, R, 13.5, SRC);
        assert!(mid.radius < 13.5 && mid.radius > 12.0, "{}", mid.radius);
        // The gather ends on the bulb's radius whatever it started from.
        assert!(near(close_frame(0.42, R, 13.5, SRC).radius, 12.0));
    }

    /// (t, drawn, rect, radius, content, blur, liquid: ([a, m, yw, f], bulb)),
    /// printed by running the preview's JS with node.
    type Row = (
        f32,
        bool,
        [f32; 4],
        f32,
        f32,
        f32,
        Option<([f32; 4], Option<[f32; 3]>)>,
    );

    fn matches_row(f: &DropFrame, row: &Row) -> Result<(), String> {
        let (t, drawn, rc, radius, content, blur, liquid) = row;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let ok = f.drawn == *drawn
            && close(f.rect.x, rc[0])
            && close(f.rect.y, rc[1])
            && close(f.rect.w, rc[2])
            && close(f.rect.h, rc[3])
            && close(f.radius, *radius)
            && close(f.content, *content)
            && close(f.blur, *blur);
        let live = match (f.liquid, liquid) {
            (None, None) => true,
            (Some(l), Some((n, bulb))) => {
                close(l.a, n[0])
                    && close(l.m, n[1])
                    && close(l.yw, n[2])
                    && close(l.f, n[3])
                    && match (l.bulb, bulb) {
                        (None, None) => true,
                        (Some(b), Some(e)) => {
                            close(b.cy, e[0]) && close(b.rx, e[1]) && close(b.ry, e[2])
                        }
                        _ => false,
                    }
            }
            _ => false,
        };
        if ok && live {
            Ok(())
        } else {
            Err(format!("t {t}: {f:?} against {row:?}"))
        }
    }

    #[expect(
        clippy::excessive_precision,
        reason = "the preview's numbers as node printed them"
    )]
    const OPEN_ROWS: &[Row] = &[
        (
            0.08,
            false,
            [1790.44244, 43.76118, 19.11512, 19.11512],
            9.55756,
            0.0,
            12.0,
            Some((
                [8.42269, 7.32977, 45.88059, 7.0],
                Some([53.31874, 9.55756, 9.55756]),
            )),
        ),
        (
            0.18,
            false,
            [1788.37658, 46.71363, 23.24685, 23.24685],
            11.62342,
            0.0,
            12.0,
            Some((
                [8.15161, 6.71437, 47.35681, 7.0],
                Some([58.33705, 11.62342, 11.62342]),
            )),
        ),
        (
            0.28,
            false,
            [1788.0, 47.4, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some(([8.0, 6.5, 48.0, 7.0], Some([59.4, 12.0, 12.0]))),
        ),
        (
            0.34,
            false,
            [1788.54577, 51.53974, 22.90847, 26.18306],
            11.45423,
            0.0,
            12.0,
            Some((
                [6.38391, 1.72692, 50.54861, 7.0],
                Some([64.63127, 11.45423, 13.09153]),
            )),
        ),
        (
            0.39,
            false,
            [1788.26495, 59.16229, 23.4701, 25.05981],
            11.73505,
            0.0,
            12.0,
            Some((
                [4.2026, 0.24047, 56.03685, 7.0],
                Some([71.6922, 11.73505, 12.5299]),
            )),
        ),
        (
            0.43,
            false,
            [1788.0, 62.32276, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [2.64817, -5.40734, 58.31239, 7.0],
                Some([74.32276, 12.0, 12.0]),
            )),
        ),
        (
            0.46,
            false,
            [1788.0, 63.34157, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [1.95073, -8.19708, 59.04593, 7.0],
                Some([75.34157, 12.0, 12.0]),
            )),
        ),
        (
            0.49,
            false,
            [1788.0, 63.85189, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [1.60139, -9.59444, 59.41336, 7.0],
                Some([75.85189, 12.0, 12.0]),
            )),
        ),
        (
            0.52,
            true,
            [1788.0, 64.0, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some(([1.5, -10.0, 59.52, 7.0], None)),
        ),
        (
            0.6,
            true,
            [1628.33333, 59.56481, 188.10185, 121.57407],
            16.43519,
            0.0,
            6.18147,
            Some(([0.25, -16.66667, 59.52, 7.0], None)),
        ),
    ];

    #[expect(
        clippy::excessive_precision,
        reason = "the preview's numbers as node printed them"
    )]
    const CLOSE_ROWS: &[Row] = &[
        (
            0.3,
            true,
            [1780.76451, 63.79901, 31.43647, 28.42169],
            12.20099,
            0.0,
            12.0,
            None,
        ),
        (
            0.45,
            false,
            [1788.0, 63.88291, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [0.0, -12.49462, 55.94145, 6.0],
                Some([75.88291, 12.0, 12.0]),
            )),
        ),
        (
            0.55,
            false,
            [1788.0, 62.02495, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [0.0, -10.82245, 55.01247, 6.0],
                Some([74.02495, 12.0, 12.0]),
            )),
        ),
        (
            0.63,
            false,
            [1788.0, 59.17458, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [0.36685, -8.25712, 53.58729, 6.0],
                Some([71.17458, 12.0, 12.0]),
            )),
        ),
        (
            0.7,
            false,
            [1788.0, 55.79976, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [1.86677, -5.21979, 51.89988, 6.0],
                Some([67.79976, 12.0, 12.0]),
            )),
        ),
        (
            0.76,
            false,
            [1788.0, 52.25965, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [3.44015, -2.03369, 50.12983, 6.0],
                Some([64.25965, 12.0, 12.0]),
            )),
        ),
        (
            0.82,
            false,
            [1788.0, 48.07101, 24.0, 24.0],
            12.0,
            0.0,
            12.0,
            Some((
                [6.62343, 3.47218, 48.0355, 6.0],
                Some([60.07101, 12.0, 12.0]),
            )),
        ),
        (
            0.88,
            false,
            [1789.22669, 44.31994, 21.54663, 21.54663],
            10.77331,
            0.0,
            12.0,
            Some((
                [9.69598, 8.07999, 48.0, 6.0],
                Some([55.09326, 10.77331, 10.77331]),
            )),
        ),
        (
            0.94,
            false,
            [1790.7652, 39.70439, 18.46959, 18.46959],
            9.2348,
            0.0,
            12.0,
            Some((
                [6.54568, 6.9261, 48.0, 6.0],
                Some([48.93918, 9.2348, 9.2348]),
            )),
        ),
        (
            0.99,
            false,
            [1792.56042, 34.31874, 14.87916, 14.87916],
            7.43958,
            0.0,
            12.0,
            Some((
                [0.83695, 5.57969, 48.0, 6.0],
                Some([41.75832, 7.43958, 7.43958]),
            )),
        ),
    ];

    #[test]
    fn the_open_matches_the_preview_at_its_frames() {
        for row in OPEN_ROWS {
            matches_row(&open(row.0), row).unwrap();
        }
        for row in CLOSE_ROWS {
            matches_row(&close(row.0), row).unwrap();
        }
    }
}
