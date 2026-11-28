//! A path of lines and cubics, and the horizontal bands that cover it: the
//! one shape a capsule's glass, its windows, the neck and the tips are drawn
//! as (CEL-D-013), and the blur region that follows it.
#![forbid(unsafe_code)]

use celestina_wayland::Rect;

/// Control-point factor of a quarter circle as one cubic; femtovg's
/// `rounded_rect` uses the same approximation.
const KAPPA: f32 = 0.552_284_7;

/// A straight side of a rounded rect under this length is not drawn, px.
const MIN_SIDE: f32 = 0.01;

/// Chords per cubic when the bands are cut.
const SLICE_CHORDS: usize = 16;

/// Chords per cubic for `contains`.
const CONTAINS_CHORDS: usize = 64;

/// Padding of a band's interval before it is rounded outward, in px.
const PAD: f32 = 0.5;

/// One drawing step of a [`ShapePath`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic {
        c1: (f32, f32),
        c2: (f32, f32),
        to: (f32, f32),
    },
    Close,
}

/// A set of sub-paths, each meant closed and clockwise on screen (y down),
/// filled non-zero so overlapping sub-paths are their union.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapePath {
    segs: Vec<Seg>,
}

impl ShapePath {
    pub fn new() -> ShapePath {
        ShapePath::default()
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.segs.push(Seg::Move(x, y));
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.segs.push(Seg::Line(x, y));
    }

    pub fn cubic_to(&mut self, c1: (f32, f32), c2: (f32, f32), to: (f32, f32)) {
        self.segs.push(Seg::Cubic { c1, c2, to });
    }

    pub fn close(&mut self) {
        self.segs.push(Seg::Close);
    }

    pub fn segs(&self) -> &[Seg] {
        &self.segs
    }

    pub fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    /// Adds every sub-path of `other`.
    pub fn append(&mut self, other: &ShapePath) {
        self.segs.extend_from_slice(&other.segs);
    }

    /// Adds a clockwise rounded rectangle starting at (`x + r`, `y`). The
    /// radius is clamped to `[0, min(w, h) / 2]`; a straight side under 0.01 px
    /// is skipped, the cubics meeting there. A rect of 0.3 px or less on
    /// a side, or a non-finite one, adds nothing.
    pub fn rounded_rect(&mut self, rect: Rect, radius: f32) {
        let Rect { x, y, w, h } = rect;
        let finite = [x, y, w, h, radius].iter().all(|v| v.is_finite());
        if !finite || w <= 0.3 || h <= 0.3 {
            return;
        }
        let r = radius.clamp(0.0, w.min(h) / 2.0);
        let k = r * (1.0 - KAPPA);
        let (right, bottom) = (x + w, y + h);
        // A straight side shorter than this is left out: femtovg's fringe
        // spikes at a zero-length join (a 1 px point at the equator of a
        // capsule or a circle).
        let wide = w - 2.0 * r >= MIN_SIDE;
        let tall = h - 2.0 * r >= MIN_SIDE;
        self.move_to(x + r, y);
        if wide {
            self.line_to(right - r, y);
        }
        self.cubic_to((right - k, y), (right, y + k), (right, y + r));
        if tall {
            self.line_to(right, bottom - r);
        }
        self.cubic_to(
            (right, bottom - k),
            (right - k, bottom),
            (right - r, bottom),
        );
        if wide {
            self.line_to(x + r, bottom);
        }
        self.cubic_to((x + k, bottom), (x, bottom - k), (x, bottom - r));
        if tall {
            self.line_to(x, y + r);
        }
        self.cubic_to((x, y + k), (x + k, y), (x + r, y));
        self.close();
    }

    /// A path of one rounded rectangle.
    pub fn rounded(rect: Rect, radius: f32) -> ShapePath {
        let mut path = ShapePath::new();
        path.rounded_rect(rect, radius);
        path
    }

    /// The hull of every point and control point, `None` when empty.
    pub fn bounds(&self) -> Option<Rect> {
        let mut hull: Option<(f32, f32, f32, f32)> = None;
        let mut add = |(x, y): (f32, f32)| {
            hull = Some(match hull {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        };
        for seg in &self.segs {
            match *seg {
                Seg::Move(x, y) | Seg::Line(x, y) => add((x, y)),
                Seg::Cubic { c1, c2, to } => {
                    add(c1);
                    add(c2);
                    add(to);
                }
                Seg::Close => {}
            }
        }
        hull.map(|(x0, y0, x1, y1)| Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }

    /// One polyline per sub-path, each cubic cut in `per_cubic` chords. The
    /// polyline is implicitly closed: the first point is not repeated.
    pub fn flatten(&self, per_cubic: usize) -> Vec<Vec<(f32, f32)>> {
        let n = per_cubic.max(1);
        let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
        let mut cur: Vec<(f32, f32)> = Vec::new();
        let mut flush = |cur: &mut Vec<(f32, f32)>| {
            if cur.len() >= 3 {
                out.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
        };
        for seg in &self.segs {
            match *seg {
                Seg::Move(x, y) => {
                    flush(&mut cur);
                    cur.push((x, y));
                }
                Seg::Line(x, y) => {
                    if !cur.is_empty() {
                        cur.push((x, y));
                    }
                }
                Seg::Cubic { c1, c2, to } => {
                    if let Some(&p0) = cur.last() {
                        for i in 1..=n {
                            let t = i as f32 / n as f32;
                            cur.push(cubic_at(p0, c1, c2, to, t));
                        }
                    }
                }
                Seg::Close => flush(&mut cur),
            }
        }
        flush(&mut cur);
        for poly in &mut out {
            if poly.len() > 1 && poly.first() == poly.last() {
                poly.pop();
            }
        }
        out
    }

    /// Whether the point is inside under the non-zero rule, on the path
    /// flattened at 64 chords per cubic.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.flatten(CONTAINS_CHORDS)
            .iter()
            .any(|poly| winding(poly, x, y) != 0)
    }
}

fn cubic_at(p0: (f32, f32), c1: (f32, f32), c2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p0.0 + b * c1.0 + c * c2.0 + d * p3.0,
        a * p0.1 + b * c1.1 + c * c2.1 + d * p3.1,
    )
}

/// Winding number of a closed polygon around a point.
fn winding(poly: &[(f32, f32)], x: f32, y: f32) -> i32 {
    let mut wn = 0;
    for (i, &(x0, y0)) in poly.iter().enumerate() {
        let (x1, y1) = poly[(i + 1) % poly.len()];
        let side = (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0);
        if y0 <= y {
            if y1 > y && side > 0.0 {
                wn += 1;
            }
        } else if y1 <= y && side < 0.0 {
            wn -= 1;
        }
    }
    wn
}

/// The band above: where its rects start in the output, where it ends, and
/// its (left, right) runs.
type PrevBand = (usize, f32, Vec<(f32, f32)>);

/// The horizontal bands, `step` px tall and pixel-aligned outward, that cover
/// every painted pixel of `path`. Each band holds one rect per run of its
/// sub-paths' x-extents (padded 0.5 px); a band equal to the one above it, and
/// touching it, is merged into it. An empty or non-finite path, or a `step`
/// of 0 or less, gives nothing.
pub fn slices_of_path(path: &ShapePath, step: f32) -> Vec<Rect> {
    if !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let polys = path.flatten(SLICE_CHORDS);
    if polys.is_empty()
        || polys
            .iter()
            .flatten()
            .any(|p| !p.0.is_finite() || !p.1.is_finite())
    {
        return Vec::new();
    }
    let spans: Vec<(f32, f32)> = polys
        .iter()
        .map(|poly| {
            poly.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p.1), hi.max(p.1))
            })
        })
        .collect();
    let top = spans.iter().fold(f32::MAX, |m, s| m.min(s.0)).floor();
    let bottom = spans.iter().fold(f32::MIN, |m, s| m.max(s.1)).ceil();

    let mut out: Vec<Rect> = Vec::new();
    let mut prev: Option<PrevBand> = None;
    let mut ya = top;
    while ya < bottom {
        let yb = (ya + step).min(bottom);
        let mut runs: Vec<(f32, f32)> = Vec::new();
        for (poly, &(pmin, pmax)) in polys.iter().zip(&spans) {
            if pmax <= ya || pmin >= yb {
                continue;
            }
            if let Some((lo, hi)) = extent_in_band(poly, ya, yb) {
                runs.push(((lo - PAD).floor(), (hi + PAD).ceil()));
            }
        }
        runs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for run in runs {
            match merged.last_mut() {
                Some(last) if run.0 <= last.1 => last.1 = last.1.max(run.1),
                _ => merged.push(run),
            }
        }
        match &prev {
            Some((start, end, last)) if *end == ya && *last == merged && !merged.is_empty() => {
                for rect in &mut out[*start..] {
                    rect.h += yb - ya;
                }
                prev = Some((*start, yb, merged));
            }
            _ if merged.is_empty() => prev = None,
            _ => {
                let start = out.len();
                out.extend(merged.iter().map(|&(l, r)| Rect {
                    x: l,
                    y: ya,
                    w: r - l,
                    h: yb - ya,
                }));
                prev = Some((start, yb, merged));
            }
        }
        ya = yb;
    }
    out
}

/// Min and max x of the polygon's part inside `ya..=yb`: its vertices there,
/// and where its edges cross the two lines.
fn extent_in_band(poly: &[(f32, f32)], ya: f32, yb: f32) -> Option<(f32, f32)> {
    let mut ext: Option<(f32, f32)> = None;
    let mut add = |x: f32| {
        ext = Some(match ext {
            None => (x, x),
            Some((lo, hi)) => (lo.min(x), hi.max(x)),
        });
    };
    for (i, &(x0, y0)) in poly.iter().enumerate() {
        let (x1, y1) = poly[(i + 1) % poly.len()];
        if (ya..=yb).contains(&y0) {
            add(x0);
        }
        for line in [ya, yb] {
            if (y0 - line) * (y1 - line) < 0.0 {
                add(x0 + (x1 - x0) * (line - y0) / (y1 - y0));
            }
        }
    }
    ext
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Shoelace area of a polyline; positive is clockwise with y down.
    fn area(poly: &[(f32, f32)]) -> f32 {
        let n = poly.len();
        (0..n)
            .map(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f32>()
            / 2.0
    }

    fn covered(slices: &[Rect], px: f32, py: f32) -> bool {
        slices.iter().any(|s| s.contains(px, py))
    }

    /// The neck of Task 1.2 between the capsule (y 48, half-width 17) and a
    /// drop's top (y 51, half-width 14), written out as segs.
    fn neck_segs() -> ShapePath {
        let (c, a, b, m, f, fb, o): (f32, f32, f32, f32, f32, f32, f32) =
            (1800.0, 17.0, 14.0, 13.6, 7.0, 7.0, 2.0);
        let (y0, y1): (f32, f32) = (48.0, 51.0);
        let (ym, k) = ((y0 + y1) / 2.0, (y1 - y0).max(1.0) / 2.0);
        let mut p = ShapePath::new();
        p.move_to(c - a - f, y0 - o);
        p.line_to(c + a + f, y0 - o);
        p.line_to(c + a + f, y0);
        p.cubic_to((c + a, y0), (c + m, ym - k / 2.0), (c + m, ym));
        p.cubic_to((c + m, ym + k / 2.0), (c + b, y1), (c + b + fb, y1));
        p.line_to(c + b + fb, y1 + o);
        p.line_to(c - b - fb, y1 + o);
        p.line_to(c - b - fb, y1);
        p.cubic_to((c - b, y1), (c - m, ym + k / 2.0), (c - m, ym));
        p.cubic_to((c - m, ym - k / 2.0), (c - a, y0), (c - a - f, y0));
        p.close();
        p
    }

    #[test]
    fn a_rounded_rect_is_one_clockwise_closed_contour() {
        let path = ShapePath::rounded(rect(10.0, 20.0, 100.0, 50.0), 12.0);
        assert_eq!(path.segs().first(), Some(&Seg::Move(22.0, 20.0)));
        assert_eq!(path.segs().last(), Some(&Seg::Close));
        let polys = path.flatten(8);
        assert_eq!(polys.len(), 1);
        assert!(area(&polys[0]) > 0.0);
    }

    #[test]
    fn a_rounded_rect_whose_height_is_twice_its_radius_has_no_zero_length_side() {
        for (r, name) in [
            (rect(10.0, 20.0, 200.0, 40.0), "h = 2r"),
            (rect(10.0, 20.0, 40.0, 100.0), "w = 2r"),
            (rect(10.0, 20.0, 24.0, 24.0), "circle"),
        ] {
            let radius = r.w.min(r.h) / 2.0;
            let path = ShapePath::rounded(r, radius);
            let mut prev = (0.0, 0.0);
            for seg in path.segs() {
                match *seg {
                    Seg::Move(x, y) => prev = (x, y),
                    Seg::Line(x, y) => {
                        let len = ((x - prev.0).powi(2) + (y - prev.1).powi(2)).sqrt();
                        assert!(len >= 0.01, "{name}: line of {len} to {x},{y}");
                        prev = (x, y);
                    }
                    Seg::Cubic { to, .. } => prev = to,
                    Seg::Close => {}
                }
            }
            assert_eq!(path.segs().last(), Some(&Seg::Close), "{name}");
            let polys = path.flatten(8);
            assert_eq!(polys.len(), 1, "{name}");
            assert!(area(&polys[0]) > 0.0, "{name}");
            assert!(path.contains(r.x + r.w / 2.0, r.y + r.h / 2.0), "{name}");
            assert!(path.contains(r.x + 0.5, r.y + r.h / 2.0), "{name}");
            assert!(!path.contains(r.x + 1.0, r.y + 1.0), "{name}");
            assert!(!path.contains(r.x + r.w - 1.0, r.y + r.h - 1.0), "{name}");
        }
    }

    #[test]
    fn a_rounded_rect_too_small_or_not_finite_adds_nothing() {
        assert!(ShapePath::rounded(rect(0.0, 0.0, 0.3, 40.0), 4.0).is_empty());
        assert!(ShapePath::rounded(rect(0.0, 0.0, 40.0, f32::NAN), 4.0).is_empty());
        assert!(ShapePath::rounded(rect(0.0, 0.0, 40.0, 40.0), f32::NAN).is_empty());
    }

    #[test]
    fn contains_follows_the_rounded_corners() {
        let path = ShapePath::rounded(rect(100.0, 100.0, 200.0, 80.0), 20.0);
        assert!(!path.contains(101.0, 101.0));
        assert!(path.contains(200.0, 140.0));
        assert!(path.contains(100.5, 140.0));
        assert!(!path.contains(99.0, 140.0));
        let b = path.bounds();
        assert_eq!(b, Some(rect(100.0, 100.0, 200.0, 80.0)));
    }

    #[test]
    fn overlapping_contours_are_their_union() {
        let mut path = ShapePath::rounded(rect(0.0, 0.0, 100.0, 60.0), 10.0);
        path.rounded_rect(rect(50.0, 20.0, 100.0, 60.0), 10.0);
        assert!(path.contains(75.0, 40.0));
        assert!(path.contains(10.0, 30.0));
        assert!(path.contains(140.0, 70.0));
        assert!(!path.contains(140.0, 5.0));
    }

    #[test]
    fn slices_of_a_rounded_rect_cover_like_silhouette_slices() {
        for (r, name) in [
            (rect(1500.0, 8.0, 400.0, 40.0), "capsule"),
            (rect(1500.0, 56.0, 320.0, 200.0), "window"),
        ] {
            let path = ShapePath::rounded(r, 20.0);
            let slices = slices_of_path(&path, 2.0);
            for py in (r.y.floor() as i32 - 2)..(r.y + r.h).ceil() as i32 + 2 {
                for px in (r.x.floor() as i32 - 2)..(r.x + r.w).ceil() as i32 + 2 {
                    let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                    if path.contains(cx, cy) {
                        assert!(covered(&slices, cx, cy), "{name}: pixel {px},{py} missed");
                    }
                }
            }
            assert!(
                slices.len() <= 2 * (20.0f32 / 2.0).ceil() as usize + 1,
                "{name}"
            );
        }
    }

    #[test]
    fn slices_never_miss_a_painted_pixel_and_overshoot_at_most_3_px() {
        let mut path = ShapePath::rounded(rect(1500.0, 8.0, 400.0, 40.0), 20.0);
        path.append(&neck_segs());
        path.rounded_rect(rect(1500.0, 56.0, 320.0, 200.0), 20.0);
        let slices = slices_of_path(&path, 2.0);
        let painted = |px: i32, py: i32| path.contains(px as f32 + 0.5, py as f32 + 0.5);
        for py in 0..270 {
            for px in 1490..1910 {
                let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                let slice_has = covered(&slices, cx, cy);
                if painted(px, py) {
                    assert!(slice_has, "pixel {px},{py} missed");
                } else if slice_has {
                    let near = (-3..=3).any(|dy| (-3..=3).any(|dx| painted(px + dx, py + dy)));
                    assert!(near, "pixel {px},{py} overshoots by more than 3 px");
                }
            }
        }
    }

    #[test]
    fn slices_cover_a_tip_and_a_detached_drop() {
        let mut path = ShapePath::rounded(rect(1500.0, 8.0, 400.0, 40.0), 20.0);
        // Tip: hanging, half-width 5, height 6, tab 4 above y 48.
        let (c, a, f, y, h) = (1800.0, 5.0, 4.0, 48.0, 6.0);
        let e = y + h;
        path.move_to(c - a - f, y - 2.0);
        path.line_to(c + a + f, y - 2.0);
        path.line_to(c + a + f, y);
        path.cubic_to((c + a, y), (c + 1.2, e - 0.4 * h), (c, e));
        path.cubic_to((c - 1.2, e - 0.4 * h), (c - a, y), (c - a - f, y));
        path.close();
        path.rounded_rect(rect(1783.0, 64.0, 34.0, 26.0), 13.0);
        let slices = slices_of_path(&path, 2.0);
        for py in 48..53 {
            for px in 1796..1805 {
                if path.contains(px as f32 + 0.5, py as f32 + 0.5) {
                    assert!(covered(&slices, px as f32 + 0.5, py as f32 + 0.5));
                }
            }
        }
        assert!(covered(&slices, 1800.5, 70.5));
        // Away from the tip, between the capsule and the drop, nothing.
        for py in 56..64 {
            for px in [1600, 1700, 1760, 1840, 1900] {
                assert!(
                    !covered(&slices, px as f32 + 0.5, py as f32 + 0.5),
                    "{px},{py}"
                );
            }
        }
    }

    #[test]
    fn equal_bands_merge_so_a_tall_window_is_few_rects() {
        let path = ShapePath::rounded(rect(1500.0, 56.0, 320.0, 496.0), 20.0);
        let slices = slices_of_path(&path, 2.0);
        assert!(slices.len() <= 21, "{} rects", slices.len());
        assert!(slices.iter().any(|s| s.h > 400.0));
    }

    #[test]
    fn an_empty_or_broken_path_has_no_slices() {
        assert!(slices_of_path(&ShapePath::new(), 2.0).is_empty());
        let mut broken = ShapePath::new();
        broken.move_to(0.0, 0.0);
        broken.line_to(f32::NAN, 10.0);
        broken.line_to(10.0, 10.0);
        broken.close();
        assert!(slices_of_path(&broken, 2.0).is_empty());
        let good = ShapePath::rounded(rect(0.0, 0.0, 40.0, 40.0), 10.0);
        assert!(slices_of_path(&good, 0.0).is_empty());
        assert!(slices_of_path(&good, -1.0).is_empty());
        assert!(slices_of_path(&good, f32::NAN).is_empty());
    }
}
