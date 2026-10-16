//! Logical and pixel rectangles shared by regions, popups and the product.
#![forbid(unsafe_code)]

/// An axis-aligned rectangle in logical pixels of a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Whether the point lies inside: the origin edges count, the far edges
    /// do not, so adjacent rectangles never both claim a point.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    /// The rectangle covering a whole surface of `size` from the origin.
    pub fn whole(size: (f32, f32)) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: size.0,
            h: size.1,
        }
    }
}

/// An axis-aligned rectangle in integer surface-local pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Converts logical rectangles to pixel rectangles, rounding outwards so the
/// region always covers the drawn silhouette. Empty rectangles are dropped.
pub fn to_pixel_rects(rects: &[Rect]) -> Vec<PixelRect> {
    rects
        .iter()
        .filter(|r| r.w > 0.0 && r.h > 0.0)
        .map(|r| {
            let x0 = r.x.floor() as i32;
            let y0 = r.y.floor() as i32;
            let x1 = (r.x + r.w).ceil() as i32;
            let y1 = (r.y + r.h).ceil() as i32;
            PixelRect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn integer_rect_passes_through() {
        assert_eq!(
            to_pixel_rects(&[r(240.0, 0.0, 280.0, 40.0)]),
            vec![PixelRect {
                x: 240,
                y: 0,
                w: 280,
                h: 40
            }]
        );
    }

    #[test]
    fn fractional_rect_rounds_outwards() {
        // x: 10.4 → 10, right edge 289.6 → 290; y: 20.6 → 20, bottom 60.5 → 61.
        assert_eq!(
            to_pixel_rects(&[r(10.4, 20.6, 279.2, 39.9)]),
            vec![PixelRect {
                x: 10,
                y: 20,
                w: 280,
                h: 41
            }]
        );
    }

    #[test]
    fn negative_origin_rounds_outwards_too() {
        assert_eq!(
            to_pixel_rects(&[r(-0.5, -0.5, 1.0, 1.0)]),
            vec![PixelRect {
                x: -1,
                y: -1,
                w: 2,
                h: 2
            }]
        );
    }

    #[test]
    fn empty_rects_are_dropped() {
        assert_eq!(
            to_pixel_rects(&[r(5.0, 5.0, 0.0, 10.0), r(1.0, 1.0, 2.0, -1.0)]),
            vec![]
        );
    }

    #[test]
    fn contains_is_closed_at_the_origin_and_open_at_the_far_edges() {
        let rect = r(10.0, 20.0, 30.0, 40.0);
        assert!(rect.contains(10.0, 20.0));
        assert!(rect.contains(39.9, 59.9));
        assert!(!rect.contains(40.0, 30.0));
        assert!(!rect.contains(20.0, 60.0));
        assert!(!rect.contains(9.9, 30.0));
    }

    #[test]
    fn whole_covers_the_size_from_the_origin() {
        assert_eq!(Rect::whole((760.0, 560.0)), r(0.0, 0.0, 760.0, 560.0));
    }
}
