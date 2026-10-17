//! The bar's blur and input regions: the union of the painted silhouettes,
//! cut by the same [`silhouette_slices`] the Island's own region uses, so a
//! region never covers a pixel no capsule paints and no painted pixel is left
//! without one (spec §6.1).

use celestina_paint::{silhouette_slices, Rect};

use crate::island::state::SILHOUETTE_STEP;

/// A painted rounded rect: its rect and its corner radius.
pub type Shape = (Rect, f32);

/// The bands covering every shape: each capsule of a zone, and the Island
/// (the capsule closed, the page open or mid-growth), in the order given.
/// An empty rect contributes nothing.
#[must_use]
pub fn union(shapes: &[Shape]) -> Vec<Rect> {
    shapes
        .iter()
        .flat_map(|&(rect, radius)| silhouette_slices(rect, radius, SILHOUETTE_STEP))
        .collect()
}

/// The input region of a surface `surface` big: the union of the silhouettes
/// at rest, the whole surface while a page is open (a click outside closes
/// it, ADR 0007).
#[must_use]
pub fn input_region(shapes: &[Shape], page_open: bool, surface: (f32, f32)) -> Vec<Rect> {
    if page_open {
        vec![Rect::whole(surface)]
    } else {
        union(shapes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    /// Whether any band covers the pixel centre `(x + 0.5, y + 0.5)`.
    fn covered(bands: &[Rect], x: i32, y: i32) -> bool {
        bands
            .iter()
            .any(|b| b.contains(x as f32 + 0.5, y as f32 + 0.5))
    }

    /// Whether the pixel centre lies inside the rounded rect.
    fn painted(shape: Shape, x: i32, y: i32) -> bool {
        let (rect, radius) = shape;
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        if !rect.contains(px, py) {
            return false;
        }
        let rad = radius.min(rect.w / 2.0).min(rect.h / 2.0);
        let cx = px.clamp(rect.x + rad, rect.x + rect.w - rad);
        let cy = py.clamp(rect.y + rad, rect.y + rect.h - rad);
        (px - cx).powi(2) + (py - cy).powi(2) <= rad * rad
    }

    fn check(shapes: &[Shape], size: (i32, i32)) {
        let bands = union(shapes);
        for y in 0..size.1 {
            for x in 0..size.0 {
                let inside = shapes.iter().any(|&s| painted(s, x, y));
                // The slicing overshoots by at most one band, outward.
                if inside {
                    assert!(covered(&bands, x, y), "painted ({x},{y}) has no region");
                }
                if covered(&bands, x, y) {
                    let near = shapes.iter().any(|&(rect, _)| {
                        let grown = Rect {
                            x: rect.x - SILHOUETTE_STEP,
                            y: rect.y - SILHOUETTE_STEP,
                            w: rect.w + 2.0 * SILHOUETTE_STEP,
                            h: rect.h + 2.0 * SILHOUETTE_STEP,
                        };
                        grown.contains(x as f32 + 0.5, y as f32 + 0.5)
                    });
                    assert!(near, "region over ({x},{y}) far from any capsule");
                }
            }
        }
    }

    #[test]
    fn the_union_covers_every_painted_capsule_and_the_closed_island() {
        let shapes = [
            (r(16.0, 8.0, 120.0, 40.0), 20.0),
            (r(300.0, 8.0, 140.0, 40.0), 20.0),
            (r(600.0, 8.0, 90.0, 40.0), 20.0),
        ];
        check(&shapes, (720, 60));
        // Between two capsules there is no region at all.
        let bands = union(&shapes);
        assert!(!covered(&bands, 200, 28));
        assert!(!covered(&bands, 550, 28));
    }

    #[test]
    fn the_union_follows_an_open_island() {
        let shapes = [
            (r(16.0, 8.0, 120.0, 40.0), 20.0),
            (r(150.0, 8.0, 420.0, 300.0), 20.0),
            (r(700.0, 8.0, 90.0, 40.0), 20.0),
        ];
        check(&shapes, (820, 320));
    }

    #[test]
    fn the_corners_of_a_capsule_are_not_in_its_region() {
        let bands = union(&[(r(10.0, 8.0, 100.0, 40.0), 20.0)]);
        assert!(!covered(&bands, 10, 8));
        assert!(covered(&bands, 60, 28));
    }

    #[test]
    fn an_empty_rect_contributes_nothing() {
        assert!(union(&[(r(0.0, 0.0, 0.0, 40.0), 20.0)]).is_empty());
        assert!(union(&[]).is_empty());
    }

    #[test]
    fn a_page_open_takes_the_whole_surface_and_rest_takes_the_union() {
        let shapes = [(r(16.0, 8.0, 120.0, 40.0), 20.0)];
        let surface = (1920.0, 560.0);
        assert_eq!(input_region(&shapes, true, surface), [Rect::whole(surface)]);
        assert_eq!(input_region(&shapes, false, surface), union(&shapes));
        assert!(input_region(&[], false, surface).is_empty());
    }
}
