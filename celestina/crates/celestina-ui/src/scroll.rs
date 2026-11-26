//! A vertical scroll offset: pure state, set from the content's extent each
//! frame and moved by the wheel.
#![forbid(unsafe_code)]

/// How far a body is scrolled, in logical px, within its extent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    offset: f32,
    max: f32,
}

impl Scroll {
    /// Sets the content and viewport heights: the most it scrolls is
    /// `(content - viewport).max(0)`, and the offset is pulled back inside
    /// it. A non-finite extent counts as no scroll.
    pub fn set_extent(&mut self, content: f32, viewport: f32) {
        let max = content - viewport;
        self.max = if max.is_finite() { max.max(0.0) } else { 0.0 };
        self.offset = self.offset.clamp(0.0, self.max);
    }

    /// Scrolls by `vertical` logical px (positive goes down the content).
    /// Whether the offset moved; a non-finite amount moves nothing.
    pub fn wheel(&mut self, vertical: f32) -> bool {
        if !vertical.is_finite() {
            return false;
        }
        let next = (self.offset + vertical).clamp(0.0, self.max);
        let moved = next != self.offset;
        self.offset = next;
        moved
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Whether the content is taller than the viewport.
    pub fn can_scroll(&self) -> bool {
        self.max > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scroll_offset_stays_within_its_content() {
        let mut s = Scroll::default();
        s.set_extent(500.0, 200.0);
        assert!(s.can_scroll());
        assert!(s.wheel(120.0));
        assert_eq!(s.offset(), 120.0);
        assert!(s.wheel(1000.0));
        assert_eq!(s.offset(), 300.0);
        assert!(!s.wheel(10.0), "already at the end");
        assert!(s.wheel(-1000.0));
        assert_eq!(s.offset(), 0.0);
        assert!(!s.wheel(-1.0));
    }

    #[test]
    fn a_body_that_fits_does_not_scroll() {
        let mut s = Scroll::default();
        s.set_extent(200.0, 200.0);
        assert!(!s.can_scroll());
        assert!(!s.wheel(50.0));
        assert_eq!(s.offset(), 0.0);
        s.set_extent(100.0, 200.0);
        assert!(!s.can_scroll());
        assert!(!s.wheel(50.0));
    }

    #[test]
    fn a_shrinking_content_pulls_the_offset_back() {
        let mut s = Scroll::default();
        s.set_extent(500.0, 200.0);
        s.wheel(300.0);
        s.set_extent(350.0, 200.0);
        assert_eq!(s.offset(), 150.0);
        s.set_extent(100.0, 200.0);
        assert_eq!(s.offset(), 0.0);
        s.set_extent(f32::NAN, 200.0);
        assert!(!s.can_scroll());
    }

    #[test]
    fn a_non_finite_wheel_moves_nothing() {
        let mut s = Scroll::default();
        s.set_extent(500.0, 200.0);
        s.wheel(40.0);
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(!s.wheel(v));
            assert_eq!(s.offset(), 40.0);
        }
    }
}
