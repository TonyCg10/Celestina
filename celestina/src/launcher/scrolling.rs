//! The arithmetic of the launcher's scrolled list: which offset reveals a
//! row, which rows to build and where the scroll bar sits. Pure, in logical
//! pixels. Non-finite inputs count as zero and results are always finite
//! and clamped.

use std::ops::Range;
use std::time::Duration;

/// The shortest the scroll bar's thumb gets.
pub const THUMB_MIN: f32 = 24.0;
/// How long the scroll bar stays after the offset last changed.
pub const THUMB_LINGER: Duration = Duration::from_millis(800);

/// A finite, non-negative length: NaN, infinities and negatives count as 0.
fn len(x: f32) -> f32 {
    if x.is_finite() {
        x.max(0.0)
    } else {
        0.0
    }
}

/// The new offset that shows `[top, top + height)` with the least change.
/// A row taller than the viewport shows its top.
pub fn reveal(offset: f32, top: f32, height: f32, viewport: f32) -> f32 {
    let (offset, top, height, viewport) = (len(offset), len(top), len(height), len(viewport));
    if height >= viewport || top < offset {
        top
    } else if top + height > offset + viewport {
        top + height - viewport
    } else {
        offset
    }
}

/// The items to build: those in view plus `margin` on each side, clamped to
/// `0..count`.
pub fn built(offset: f32, item: f32, viewport: f32, count: usize, margin: usize) -> Range<usize> {
    let (offset, item, viewport) = (len(offset), len(item), len(viewport));
    if item <= 0.0 {
        return 0..0;
    }
    // `as usize` saturates, so huge values stay in range after the clamp.
    let first = (offset / item).floor() as usize;
    let end = ((offset + viewport) / item).ceil() as usize;
    first.saturating_sub(margin).min(count)..end.saturating_add(margin).min(count)
}

/// The bar's y and height, `None` when `content <= viewport`.
pub fn thumb(offset: f32, content: f32, viewport: f32) -> Option<(f32, f32)> {
    let (offset, content, viewport) = (len(offset), len(content), len(viewport));
    if viewport <= 0.0 || content <= viewport {
        return None;
    }
    let height = (viewport * viewport / content).max(THUMB_MIN).min(viewport);
    let progress = (offset / (content - viewport)).clamp(0.0, 1.0);
    Some((progress * (viewport - height), height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_scrolls_down_to_show_the_bottom() {
        assert_eq!(reveal(0.0, 400.0, 40.0, 320.0), 120.0);
    }

    #[test]
    fn reveal_scrolls_up_to_show_the_top() {
        assert_eq!(reveal(200.0, 40.0, 40.0, 320.0), 40.0);
    }

    #[test]
    fn reveal_leaves_a_visible_row_alone() {
        assert_eq!(reveal(100.0, 120.0, 40.0, 320.0), 100.0);
        assert_eq!(reveal(100.0, 100.0, 40.0, 320.0), 100.0);
        assert_eq!(reveal(100.0, 380.0, 40.0, 320.0), 100.0);
    }

    #[test]
    fn built_adds_the_margin_around_the_visible_rows() {
        // Rows 2..11 are in view; four more each side, clamped at the top.
        assert_eq!(built(85.0, 40.0, 320.0, 100, 4), 0..15);
        assert_eq!(built(400.0, 40.0, 320.0, 100, 4), 6..22);
        assert_eq!(built(0.0, 40.0, 320.0, 30, 4), 0..12);
    }

    #[test]
    fn built_is_clamped_at_both_ends() {
        assert_eq!(built(0.0, 40.0, 320.0, 5, 4), 0..5);
        assert_eq!(built(4000.0, 40.0, 320.0, 100, 4), 96..100);
        assert_eq!(built(0.0, 40.0, 320.0, 0, 4), 0..0);
    }

    #[test]
    fn thumb_is_proportional() {
        assert_eq!(thumb(0.0, 640.0, 320.0), Some((0.0, 160.0)));
        assert_eq!(thumb(320.0, 640.0, 320.0), Some((160.0, 160.0)));
        assert_eq!(thumb(160.0, 640.0, 320.0), Some((80.0, 160.0)));
    }

    #[test]
    fn thumb_is_never_shorter_than_the_minimum() {
        let (y, h) = thumb(0.0, 100_000.0, 320.0).unwrap();
        assert_eq!((y, h), (0.0, THUMB_MIN));
        let (y, h) = thumb(100_000.0 - 320.0, 100_000.0, 320.0).unwrap();
        assert_eq!(h, THUMB_MIN);
        assert_eq!(y, 320.0 - THUMB_MIN);
    }

    #[test]
    fn thumb_is_absent_when_nothing_scrolls() {
        assert_eq!(thumb(0.0, 320.0, 320.0), None);
        assert_eq!(thumb(0.0, 100.0, 320.0), None);
    }

    #[test]
    fn bad_inputs_give_finite_clamped_results() {
        let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -50.0];
        for &a in &bad {
            for &b in &bad {
                assert!(reveal(a, b, a, b).is_finite());
                assert!(reveal(0.0, a, b, 320.0).is_finite());
                let r = built(a, b, a, 10, 4);
                assert!(r.start <= r.end && r.end <= 10, "{r:?}");
                if let Some((y, h)) = thumb(a, b, a) {
                    assert!(y.is_finite() && h.is_finite() && y >= 0.0 && h >= 0.0);
                }
                if let Some((y, h)) = thumb(a, 640.0, 320.0) {
                    assert!(y >= 0.0 && y + h <= 320.0);
                }
            }
        }
        assert_eq!(reveal(f32::NAN, 400.0, 40.0, 320.0), 120.0);
        assert!(reveal(-10.0, 0.0, 40.0, 320.0) >= 0.0);
        assert_eq!(built(f32::NAN, 40.0, 320.0, 100, 4), 0..12);
        assert_eq!(built(0.0, f32::NAN, 320.0, 100, 4), 0..0);
        let (y, _) = thumb(-30.0, 640.0, 320.0).unwrap();
        assert_eq!(y, 0.0);
        let (y, h) = thumb(9999.0, 640.0, 320.0).unwrap();
        assert_eq!(y + h, 320.0);
    }
}
