//! Haze: the material plan (tint over grain, or the opaque fallback when the
//! compositor cannot blur), the grain texture, and the silhouette slices that
//! keep the compositor blur and input regions equal to the painted shape.
//! `silhouette_slices` comes from the spike's `src/paint/haze.rs`.
#![forbid(unsafe_code)]

use resvg::tiny_skia;

use crate::color::Color;
use crate::theme::{Role, Theme};
use crate::RgbaImage;
use celestina_wayland::Rect;

/// Side of the grain tile in texels; one texel per logical px.
pub(crate) const GRAIN_SIZE: u32 = 64;

/// Haze's noise texture, the PNG the style vendors for `GlassSurface`.
const GRAIN_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../celestina-style/icons/haze-noise.png"
));

/// What a Haze rect paints: with the compositor blur behind it, the grain at
/// `grain` under the `fill` tint (DESIGN.md §5.3: blur, grain, then tint);
/// without it, the opaque fallback alone (Review Focus 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HazePlan {
    pub fill: Color,
    pub grain: Option<f32>,
}

impl HazePlan {
    /// The plan for a surface whose compositor blur is (or is not) armed.
    pub fn for_(theme: &Theme, blur_available: bool) -> HazePlan {
        if blur_available {
            HazePlan {
                fill: theme.color(Role::GlassTint),
                grain: Some(theme.grain_alpha),
            }
        } else {
            HazePlan {
                fill: theme.color(Role::GlassFallback),
                grain: None,
            }
        }
    }
}

/// The grain could not be decoded (the PNG is compiled in, so this only
/// fails if the style's asset is corrupt).
#[derive(Debug, thiserror::Error)]
#[error("cannot decode the Haze grain texture")]
pub struct GrainError(#[source] Box<dyn std::error::Error + Send + Sync + 'static>);

/// The grain tile as premultiplied RGBA texels.
pub(crate) fn grain_texels() -> Result<RgbaImage, GrainError> {
    let pixmap = tiny_skia::Pixmap::decode_png(GRAIN_PNG).map_err(|e| GrainError(Box::new(e)))?;
    Ok(RgbaImage {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.take(),
    })
}

/// Horizontal slices of a rounded rect, each at most `step` tall, that cover
/// the shape. The slicing works on the pixel-aligned outward bounding rect
/// of `rect` (floor left/top, ceil right/bottom), and overshoot is at most
/// `step` measured against that snapped rect. The straight middle is one
/// slice; each corner contributes slices whose width follows the circle,
/// measured at the edge nearest the middle so the union never undershoots.
pub fn silhouette_slices(rect: Rect, radius: f32, step: f32) -> Vec<Rect> {
    if rect.w <= 0.0 || rect.h <= 0.0 || step <= 0.0 {
        return Vec::new();
    }
    let (left, top) = (rect.x.floor(), rect.y.floor());
    let (right, bottom) = ((rect.x + rect.w).ceil(), (rect.y + rect.h).ceil());
    let r = radius.clamp(0.0, ((right - left).min(bottom - top)) / 2.0);
    // Row boundaries, pixel-aligned outward. The corner regions end at the
    // floor/ceil of where the arcs end, so the straight slice absorbs the
    // last sub-pixel row of each arc.
    let y0 = top.floor();
    let y1 = (top + r).floor();
    let y2 = (bottom - r).ceil();
    let y3 = bottom.ceil();

    let slice = |inset: f32, ya: f32, yb: f32| Rect {
        x: (left + inset).floor(),
        y: ya,
        w: (right - inset).ceil() - (left + inset).floor(),
        h: yb - ya,
    };
    // Horizontal inset of the arc at depth `t` below the shape's edge.
    let inset = |t: f32| {
        if t >= r {
            0.0
        } else {
            let dy = r - t.max(0.0);
            r - (r * r - dy * dy).max(0.0).sqrt()
        }
    };

    let mut out = Vec::new();
    let mut ya = y0;
    while ya < y1 {
        let yb = (ya + step).min(y1);
        // The widest extent of a top band is at its lower edge.
        out.push(slice(inset(yb - top), ya, yb));
        ya = yb;
    }
    if y2 > y1 {
        out.push(slice(0.0, y1, y2));
    }
    let first_bottom = out.len();
    let mut yb = y3;
    while yb > y2.max(y1) {
        let ya = (yb - step).max(y2.max(y1));
        // The widest extent of a bottom band is at its upper edge.
        out.push(slice(inset(bottom - ya), ya, yb));
        yb = ya;
    }
    out[first_bottom..].reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed distance from a point to the rounded rect (negative inside).
    fn sdf(rect: Rect, radius: f32, px: f32, py: f32) -> f32 {
        let (hx, hy) = (rect.w / 2.0, rect.h / 2.0);
        let r = radius.clamp(0.0, hx.min(hy));
        let qx = (px - (rect.x + hx)).abs() - (hx - r);
        let qy = (py - (rect.y + hy)).abs() - (hy - r);
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
    }

    fn in_slices(slices: &[Rect], px: f32, py: f32) -> bool {
        slices
            .iter()
            .any(|s| px >= s.x && px <= s.x + s.w && py >= s.y && py <= s.y + s.h)
    }

    /// Every point inside the shape is inside a slice, and every point of a
    /// slice is within `max_over` px of the shape.
    fn check(rect: Rect, radius: f32, step: f32, max_over: f32) -> Vec<Rect> {
        let slices = silhouette_slices(rect, radius, step);
        let r = radius.clamp(0.0, rect.w.min(rect.h) / 2.0);
        let bound = 2 * (r / step).ceil() as usize + 1;
        assert!(
            slices.len() <= bound,
            "{} slices > bound {bound}",
            slices.len()
        );
        for s in &slices {
            // Only the straight middle slice (full width) may exceed `step`.
            let straight = s.w >= rect.w;
            assert!(straight || s.h <= step + 1e-4, "slice too tall: {s:?}");
            assert_eq!(s.y, s.y.floor(), "top not pixel aligned: {s:?}");
        }
        let n = 160;
        for i in 0..=n {
            for j in 0..=n {
                let px = rect.x + rect.w * i as f32 / n as f32;
                let py = rect.y + rect.h * j as f32 / n as f32;
                if sdf(rect, radius, px, py) <= 0.0 {
                    assert!(in_slices(&slices, px, py), "uncovered ({px}, {py})");
                }
            }
        }
        let mut worst = 0.0f32;
        for s in &slices {
            for (px, py) in [
                (s.x, s.y),
                (s.x + s.w, s.y),
                (s.x, s.y + s.h),
                (s.x + s.w, s.y + s.h),
            ] {
                worst = worst.max(sdf(rect, radius, px, py));
            }
        }
        assert!(worst <= max_over, "overshoot {worst} > {max_over}");
        slices
    }

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn radius_zero_is_one_rect() {
        let s = check(r(10.0, 4.0, 100.0, 30.0), 0.0, 2.0, 1.0);
        assert_eq!(s, vec![r(10.0, 4.0, 100.0, 30.0)]);
    }

    #[test]
    fn radius_zero_aligns_outward() {
        let s = silhouette_slices(r(10.4, 4.6, 100.0, 30.0), 0.0, 2.0);
        assert_eq!(s, vec![r(10.0, 4.0, 101.0, 31.0)]);
    }

    #[test]
    fn radius_8_covers() {
        check(r(0.0, 0.0, 120.0, 60.0), 8.0, 2.0, 2.0);
    }

    #[test]
    fn capsule_radius_20_covers_with_single_middle_free() {
        // 280x40 capsule: the corners meet, so there is no straight middle.
        let s = check(r(240.0, 0.0, 280.0, 40.0), 20.0, 2.0, 2.0);
        assert_eq!(s.len(), 20);
    }

    #[test]
    fn tall_rect_has_one_straight_middle_slice() {
        let s = check(r(0.0, 0.0, 200.0, 100.0), 20.0, 2.0, 2.0);
        // Exactly one slice is taller than `step`: the straight middle.
        let straight: Vec<_> = s.iter().filter(|s| s.h > 2.0).collect();
        assert_eq!(straight, vec![&r(0.0, 20.0, 200.0, 60.0)]);
        assert_eq!(s.iter().map(|s| s.h).sum::<f32>(), 100.0);
    }

    #[test]
    fn radius_clamps_to_half_the_short_side() {
        let a = silhouette_slices(r(0.0, 0.0, 100.0, 24.0), 20.0, 2.0);
        let b = silhouette_slices(r(0.0, 0.0, 100.0, 24.0), 12.0, 2.0);
        assert_eq!(a, b);
        check(r(0.0, 0.0, 100.0, 24.0), 20.0, 2.0, 2.0);
    }

    #[test]
    fn fractional_rect_still_covers() {
        let rect = r(100.3, 7.6, 280.0, 40.0);
        let slices = silhouette_slices(rect, 20.0, 2.0);
        // Overshoot is measured against the snapped outward rect.
        let snapped = r(100.0, 7.0, 281.0, 41.0);
        let worst = slices
            .iter()
            .flat_map(|s| {
                [
                    (s.x, s.y),
                    (s.x + s.w, s.y),
                    (s.x, s.y + s.h),
                    (s.x + s.w, s.y + s.h),
                ]
            })
            .map(|(px, py)| sdf(snapped, 20.0, px, py))
            .fold(0.0f32, f32::max);
        assert!(worst <= 2.0, "overshoot {worst}");
        // Still covers the original shape.
        let n = 160;
        for i in 0..=n {
            for j in 0..=n {
                let px = rect.x + rect.w * i as f32 / n as f32;
                let py = rect.y + rect.h * j as f32 / n as f32;
                if sdf(rect, 20.0, px, py) <= 0.0 {
                    assert!(in_slices(&slices, px, py), "uncovered ({px}, {py})");
                }
            }
        }
    }

    #[test]
    fn degenerate_rect_yields_nothing() {
        assert!(silhouette_slices(r(0.0, 0.0, 0.0, 10.0), 4.0, 2.0).is_empty());
    }

    #[test]
    fn grain_is_deterministic_noise() {
        let a = grain_texels().unwrap();
        assert_eq!(a, grain_texels().unwrap());
        assert_eq!((a.width, a.height), (GRAIN_SIZE, GRAIN_SIZE));
        assert_eq!(a.rgba.len(), (GRAIN_SIZE * GRAIN_SIZE * 4) as usize);
        let distinct: std::collections::HashSet<u8> =
            a.rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
        assert!(distinct.len() > 150, "{} alpha levels", distinct.len());
    }

    #[test]
    fn haze_without_blur_paints_the_opaque_fallback() {
        let theme = Theme::dark();
        assert_eq!(
            HazePlan::for_(&theme, true),
            HazePlan {
                fill: theme.color(Role::GlassTint),
                grain: Some(0.15),
            }
        );
        let fallback = HazePlan::for_(&theme, false);
        assert_eq!(
            fallback,
            HazePlan {
                fill: theme.color(Role::GlassFallback),
                grain: None,
            }
        );
        assert_eq!(fallback.fill.3, 1.0, "the fallback is opaque");
    }
}
