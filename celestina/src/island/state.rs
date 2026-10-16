//! The Island's pure state: closed (the capsule) or open on a page, the
//! geometry at an instant, the two opacities and the silhouette the blur and
//! input regions follow. No painter, no GL. Ported from the spike's
//! `src/island/state.rs`, with pages, reduced motion and the silhouette.

use std::time::Instant;

use celestina_paint::{silhouette_slices, MotionMs, Rect, Theme};
use celestina_ui::{Animated, Motion, Reduced, EXIT, ONE_UI};

use crate::command::Page;

/// Capsule height, logical px.
pub const CAPSULE_H: f32 = 40.0;
/// Gap between the surface top and the Island, logical px.
pub const TOP_GAP: f32 = 8.0;
/// The page size, logical px.
pub const PAGE_SIZE: (f32, f32) = (420.0, 300.0);
/// The tallest band of the silhouette (design §5.2: at most 2 logical px).
pub const SILHOUETTE_STEP: f32 = 2.0;
/// Capsule content fades out over the first 40 % of the progress, page
/// content fades in over the last 60 %.
const FADE_SPLIT: f32 = 0.4;

/// Closed or open, on which page, and the flight between the two.
#[derive(Clone, Debug)]
pub struct IslandState {
    /// 0 = capsule, 1 = page.
    progress: Animated,
    capsule_w: f32,
    /// The page shown while open, and the one a toggle reopens.
    page: Page,
    ladder: MotionMs,
}

impl IslandState {
    /// The capsule at rest, `capsule_w` wide.
    #[must_use]
    pub fn new(capsule_w: f32) -> Self {
        Self {
            progress: Animated::new(0.0),
            capsule_w: even_width(capsule_w),
            page: Page::Activity,
            ladder: Theme::dark().motion,
        }
    }

    /// Rounded up to an even number of px so the centred capsule sits on
    /// whole pixels at rest.
    pub fn set_capsule_w(&mut self, w: f32) {
        self.capsule_w = even_width(w);
    }

    /// Grows into `page` (350 ms on the One UI curve; one step under
    /// reduced motion). While open, it only switches the page.
    pub fn open(&mut self, page: Page, now: Instant, reduced: bool) {
        self.page = page;
        self.progress.animate_to(
            1.0,
            Motion::Slow.duration(&self.ladder),
            ONE_UI,
            now,
            Reduced::spatial(reduced),
        );
    }

    /// Shrinks back to the capsule on the shorter exit motion.
    pub fn close(&mut self, now: Instant, reduced: bool) {
        self.progress.animate_to(
            0.0,
            Motion::Exit.duration(&self.ladder),
            EXIT,
            now,
            Reduced::spatial(reduced),
        );
    }

    /// Closes an open Island, or opens the page it last showed.
    pub fn toggle(&mut self, now: Instant, reduced: bool) {
        if self.is_open() {
            self.close(now, reduced);
        } else {
            self.open(self.page, now, reduced);
        }
    }

    /// True once `open` was asked and until `close` is.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.progress.target() == 1.0
    }

    /// The open page; `None` while closed or closing.
    #[must_use]
    pub fn page(&self) -> Option<Page> {
        self.is_open().then_some(self.page)
    }

    /// The page the page view draws: the open one, or the one fading out.
    #[must_use]
    pub fn shown_page(&self) -> Page {
        self.page
    }

    #[must_use]
    pub fn progress(&self, now: Instant) -> f32 {
        self.progress.value(now)
    }

    /// The Island's rect at `now`, centred on a surface `surface_w` wide and
    /// glued to the same top edge.
    #[must_use]
    pub fn rect(&self, now: Instant, surface_w: f32) -> Rect {
        let p = self.progress(now);
        let w = lerp(self.capsule_w, PAGE_SIZE.0, p);
        Rect {
            x: (surface_w - w) / 2.0,
            y: TOP_GAP,
            w,
            h: lerp(CAPSULE_H, PAGE_SIZE.1, p),
        }
    }

    #[must_use]
    pub fn capsule_opacity(&self, now: Instant) -> f32 {
        capsule_opacity_at(self.progress(now))
    }

    #[must_use]
    pub fn page_opacity(&self, now: Instant) -> f32 {
        page_opacity_at(self.progress(now))
    }

    /// Whether the growth is in flight at `now`.
    #[must_use]
    pub fn wants_frame(&self, now: Instant) -> bool {
        self.progress.is_animating(now)
    }

    /// The rounded rect at `now` in bands of at most [`SILHOUETTE_STEP`]
    /// px: the blur region, and the input region at rest.
    #[must_use]
    pub fn silhouette(&self, now: Instant, surface_w: f32, radius: f32) -> Vec<Rect> {
        silhouette_slices(self.rect(now, surface_w), radius, SILHOUETTE_STEP)
    }

    /// The exclusive zone of the Island's surface: the capsule and its gap.
    #[must_use]
    pub fn exclusive_zone(&self) -> i32 {
        (CAPSULE_H + TOP_GAP) as i32
    }
}

fn even_width(w: f32) -> f32 {
    (w / 2.0).ceil() * 2.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn capsule_opacity_at(p: f32) -> f32 {
    (1.0 - p / FADE_SPLIT).clamp(0.0, 1.0)
}

fn page_opacity_at(p: f32) -> f32 {
    ((p - FADE_SPLIT) / (1.0 - FADE_SPLIT)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const SURFACE_W: f32 = 760.0;
    const CAPSULE_W: f32 = 200.0;

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    fn close_to(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn at_rest_it_is_the_capsule_centred_8px_below_the_top() {
        let t0 = Instant::now();
        let s = IslandState::new(CAPSULE_W);
        assert!(!s.is_open());
        assert_eq!(s.page(), None);
        let r = s.rect(t0, SURFACE_W);
        assert_eq!((r.x, r.y, r.w, r.h), (280.0, 8.0, 200.0, 40.0));
        assert_eq!(s.capsule_opacity(t0), 1.0);
        assert_eq!(s.page_opacity(t0), 0.0);
        assert!(!s.wants_frame(t0));
    }

    #[test]
    fn open_settles_on_the_420_by_300_page_with_the_same_top_edge() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        assert!(s.is_open());
        assert_eq!(s.page(), Some(Page::Activity));
        let end = ms(t0, 350);
        let r = s.rect(end, SURFACE_W);
        assert_eq!((r.x, r.y, r.w, r.h), (170.0, 8.0, 420.0, 300.0));
        assert_eq!(s.capsule_opacity(end), 0.0);
        assert_eq!(s.page_opacity(end), 1.0);
    }

    #[test]
    fn rect_lerps_between_the_two_geometries_with_the_progress() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        let t = ms(t0, 120);
        let p = s.progress(t);
        assert!(p > 0.0 && p < 1.0);
        let r = s.rect(t, SURFACE_W);
        assert!(close_to(r.w, 200.0 + (420.0 - 200.0) * p));
        assert!(close_to(r.h, 40.0 + (300.0 - 40.0) * p));
        // Always centred and glued to the same top edge.
        assert!(close_to(r.x + r.w / 2.0, SURFACE_W / 2.0));
        assert_eq!(r.y, 8.0);
    }

    #[test]
    fn opacity_windows_are_0_to_40_and_40_to_100_percent() {
        assert_eq!(capsule_opacity_at(0.0), 1.0);
        assert!(close_to(capsule_opacity_at(0.2), 0.5));
        assert_eq!(capsule_opacity_at(0.4), 0.0);
        assert_eq!(capsule_opacity_at(0.9), 0.0);
        assert_eq!(page_opacity_at(0.0), 0.0);
        assert_eq!(page_opacity_at(0.4), 0.0);
        assert!(close_to(page_opacity_at(0.7), 0.5));
        assert_eq!(page_opacity_at(1.0), 1.0);
    }

    #[test]
    fn opacities_follow_the_live_progress() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        let t = ms(t0, 60);
        let p = s.progress(t);
        assert!(close_to(s.capsule_opacity(t), capsule_opacity_at(p)));
        assert!(close_to(s.page_opacity(t), page_opacity_at(p)));
    }

    #[test]
    fn wants_frame_only_while_the_progress_moves() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        assert!(s.wants_frame(t0));
        assert!(s.wants_frame(ms(t0, 349)));
        assert!(!s.wants_frame(ms(t0, 350)));
        assert!(!s.wants_frame(ms(t0, 5000)));
        // Asking again at rest changes nothing.
        s.open(Page::Activity, ms(t0, 5000), false);
        assert!(!s.wants_frame(ms(t0, 5000)));
    }

    #[test]
    fn close_runs_the_shorter_exit_motion() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        let t1 = ms(t0, 400);
        s.close(t1, false);
        assert!(!s.is_open());
        assert_eq!(s.page(), None);
        assert_eq!(s.shown_page(), Page::Activity, "still drawn while it fades");
        assert!(s.wants_frame(ms(t1, 149)));
        assert!(!s.wants_frame(ms(t1, 150)));
        let r = s.rect(ms(t1, 150), SURFACE_W);
        assert_eq!((r.x, r.w, r.h), (280.0, 200.0, 40.0));
    }

    #[test]
    fn closing_during_an_open_retargets_from_the_current_progress() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        let t1 = ms(t0, 100);
        let before = s.rect(t1, SURFACE_W);
        let p = s.progress(t1);
        s.close(t1, false);
        let after = s.rect(t1, SURFACE_W);
        assert_eq!(before, after, "no jump at the retarget");
        assert!(close_to(s.progress(t1), p));
        assert!(s.progress(ms(t1, 50)) < p);
        // And it ends on the capsule after the exit motion.
        assert!(!s.wants_frame(ms(t1, 150)));
        assert_eq!(s.progress(ms(t1, 150)), 0.0);
    }

    #[test]
    fn a_new_capsule_width_applies_at_rest_and_is_even() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.set_capsule_w(201.3);
        let r = s.rect(t0, SURFACE_W);
        assert_eq!(r.w, 202.0);
        assert_eq!(r.x, 279.0);
    }

    #[test]
    fn toggle_reopens_the_last_page_and_open_switches_pages() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.toggle(t0, false);
        assert_eq!(s.page(), Some(Page::Activity));
        s.open(Page::Tray, ms(t0, 400), false);
        assert_eq!(s.page(), Some(Page::Tray));
        assert!(!s.wants_frame(ms(t0, 400)), "switching does not regrow");
        s.toggle(ms(t0, 500), false);
        assert_eq!(s.page(), None);
        s.toggle(ms(t0, 1000), false);
        assert_eq!(s.page(), Some(Page::Tray));
    }

    #[test]
    fn exclusive_zone_is_the_capsule_height_plus_its_margin() {
        assert_eq!(IslandState::new(CAPSULE_W).exclusive_zone(), 48);
    }

    /// Signed distance from a point to a rounded rect (negative inside).
    fn sdf(rect: Rect, radius: f32, px: f32, py: f32) -> f32 {
        let (hx, hy) = (rect.w / 2.0, rect.h / 2.0);
        let r = radius.clamp(0.0, hx.min(hy));
        let qx = (px - (rect.x + hx)).abs() - (hx - r);
        let qy = (py - (rect.y + hy)).abs() - (hy - r);
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
    }

    #[test]
    fn silhouette_follows_the_rect_every_frame() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, false);
        let radius = Theme::dark().radius_lg;
        for t in [0, 120, 350] {
            let now = ms(t0, t);
            let rect = s.rect(now, SURFACE_W);
            let slices = s.silhouette(now, SURFACE_W, radius);
            assert!(!slices.is_empty(), "t={t}");
            // Nothing beyond 2 px of the rect, and no band taller than 2 px
            // except the straight middle.
            for b in &slices {
                assert!(b.x >= rect.x - 2.0 && b.y >= rect.y - 2.0, "t={t} {b:?}");
                assert!(b.x + b.w <= rect.x + rect.w + 2.0, "t={t} {b:?}");
                assert!(b.y + b.h <= rect.y + rect.h + 2.0, "t={t} {b:?}");
            }
            // Every point of the rounded rect is in a band.
            let mut y = rect.y + 0.25;
            while y < rect.y + rect.h {
                let mut x = rect.x + 0.25;
                while x < rect.x + rect.w {
                    if sdf(rect, radius, x, y) < -0.01 {
                        assert!(slices.iter().any(|b| b.contains(x, y)), "t={t} ({x}, {y})");
                    }
                    x += 1.0;
                }
                y += 1.0;
            }
        }
    }

    #[test]
    fn reduced_motion_opens_in_one_step() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Notifications, t0, true);
        assert!(!s.wants_frame(t0));
        let r = s.rect(t0, SURFACE_W);
        assert_eq!((r.x, r.y, r.w, r.h), (170.0, 8.0, 420.0, 300.0));
        assert_eq!(s.page_opacity(t0), 1.0);
        s.close(t0, true);
        assert!(!s.wants_frame(t0));
        assert_eq!(s.rect(t0, SURFACE_W).w, 200.0);
    }
}
