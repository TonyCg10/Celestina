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
/// The page size a page gets unless it asks for another, logical px.
pub const PAGE_SIZE: (f32, f32) = (420.0, 300.0);
/// The space a page keeps from the surface's sides and bottom edge, logical
/// px (the top keeps [`TOP_GAP`]).
pub const PAGE_MARGIN: f32 = 8.0;
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
    /// The horizontal offset of the page content in page widths: −1 or +1
    /// right after a switch, 0 at rest. A clock of its own: a switch is a
    /// gesture of its own, separate from the growth.
    slide: Animated,
    /// What the shown page asks for, logical px.
    request: (f32, f32),
    /// 0..1 from the page size shown when `request` last changed while open
    /// ([`Self::resize_from`]) to [`Self::page_size`]: a size change is a
    /// gesture of its own, so it has a clock of its own.
    resize: Animated,
    resize_from: (f32, f32),
    /// The surface's logical size, which bounds the page.
    surface: (f32, f32),
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
            slide: Animated::new(0.0),
            request: PAGE_SIZE,
            resize: Animated::new(1.0),
            resize_from: PAGE_SIZE,
            surface: (f32::INFINITY, f32::INFINITY),
            ladder: Theme::dark().motion,
        }
    }

    /// Rounded up to an even number of px so the centred capsule sits on
    /// whole pixels at rest.
    pub fn set_capsule_w(&mut self, w: f32) {
        self.capsule_w = even_width(w);
    }

    /// The surface's logical size, which bounds [`Self::page_size`].
    pub fn set_surface(&mut self, size: (f32, f32)) {
        self.surface = size;
    }

    /// The size the shown page asks for; a non-finite or non-positive
    /// request is ignored and the last one stays. While open, a changed
    /// request grows or shrinks the shown rect to it on the `Normal` motion
    /// from the size it has at `now` (one step under `reduced`); while
    /// closed, or while the growth is still in flight, the size just becomes
    /// the growth's target: one clock per gesture, so the open's own clock
    /// carries it.
    pub fn request_page_size(&mut self, size: (f32, f32), now: Instant, reduced: bool) {
        let valid = |v: f32| v.is_finite() && v > 0.0;
        if !valid(size.0) || !valid(size.1) || size == self.request {
            return;
        }
        let shown = self.shown_page_size(now);
        self.request = size;
        if !self.is_open() || self.progress.is_animating(now) {
            self.resize.set(1.0);
            return;
        }
        self.resize_from = shown;
        self.resize.set(0.0);
        self.resize.animate_to(
            1.0,
            Motion::Normal.duration(&self.ladder),
            ONE_UI,
            now,
            Reduced::spatial(reduced),
        );
    }

    /// The largest page the surface allows: what [`Self::page_size`] clamps
    /// a request to.
    #[must_use]
    pub fn max_page_size(&self) -> (f32, f32) {
        (
            (self.surface.0 - 2.0 * PAGE_MARGIN).max(0.0),
            (self.surface.1 - TOP_GAP - PAGE_MARGIN).max(0.0),
        )
    }

    /// The open page's final size: its request, but never past the surface
    /// minus the top gap and the page margins.
    #[must_use]
    pub fn page_size(&self) -> (f32, f32) {
        let (max_w, max_h) = self.max_page_size();
        (self.request.0.min(max_w), self.request.1.min(max_h))
    }

    /// The page size at `now`: [`Self::page_size`], or on its way there
    /// after the request changed.
    fn shown_page_size(&self, now: Instant) -> (f32, f32) {
        let t = self.resize.value(now);
        let (w, h) = self.page_size();
        (
            lerp(self.resize_from.0, w, t),
            lerp(self.resize_from.1, h, t),
        )
    }

    /// Shows `page` in an open Island, sliding the content horizontally on
    /// the `Normal` motion: from −1 to 0 when `page` is right of the shown
    /// one in the tab order, from +1 to 0 when it is left (a single step
    /// under reduced motion). Switching to the shown page does nothing;
    /// while closed it only records the page a toggle reopens.
    pub fn switch(&mut self, page: Page, now: Instant, reduced: bool) {
        if page == self.page {
            return;
        }
        let from = if tab_index(page) > tab_index(self.page) {
            -1.0
        } else {
            1.0
        };
        self.page = page;
        if !self.is_open() {
            return;
        }
        // A switch during a slide continues from where the content is.
        let start = if self.slide.is_animating(now) {
            self.slide.value(now)
        } else {
            from
        };
        self.slide.set(start);
        self.slide.animate_to(
            0.0,
            Motion::Normal.duration(&self.ladder),
            ONE_UI,
            now,
            Reduced::spatial(reduced),
        );
    }

    /// The page content's horizontal offset at `now`, in page widths
    /// (−1..1; 0 at rest).
    #[must_use]
    pub fn slide(&self, now: Instant) -> f32 {
        self.slide.value(now)
    }

    /// Grows into `page` (350 ms on the One UI curve; one step under
    /// reduced motion). While open, it only switches the page; from closed
    /// it starts with the content at rest, whatever slide was left over.
    pub fn open(&mut self, page: Page, now: Instant, reduced: bool) {
        if self.is_open() {
            self.switch(page, now, reduced);
            return;
        }
        self.page = page;
        self.slide.set(0.0);
        // A resize left in flight ends here: the growth is the only clock.
        self.resize.set(1.0);
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
        let (page_w, page_h) = self.shown_page_size(now);
        let w = lerp(self.capsule_w, page_w, p);
        Rect {
            x: (surface_w - w) / 2.0,
            y: TOP_GAP,
            w,
            h: lerp(CAPSULE_H, page_h, p),
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

    /// Whether the growth, a page slide or a resize is in flight at `now`.
    #[must_use]
    pub fn wants_frame(&self, now: Instant) -> bool {
        self.progress.is_animating(now)
            || self.slide.is_animating(now)
            || self.resize.is_animating(now)
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

/// `page`'s place in the tab row, left to right.
fn tab_index(page: Page) -> usize {
    Page::ALL.iter().position(|p| *p == page).unwrap_or(0)
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
        assert!(
            !s.progress.is_animating(ms(t0, 400)),
            "switching does not regrow"
        );
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
    fn switching_pages_slides_content_horizontally() {
        let t0 = Instant::now();
        let normal = Motion::Normal.duration(&Theme::dark().motion);
        let half = t0 + normal / 2;
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, true);
        assert_eq!(s.slide(t0), 0.0, "opening does not slide");
        // Moving right: the new content comes in from the left edge's side.
        s.switch(Page::Tray, t0, false);
        assert_eq!(s.page(), Some(Page::Tray));
        assert!(close_to(s.slide(t0), -1.0));
        assert!(s.wants_frame(t0));
        let mid = s.slide(half);
        assert!(mid > -1.0 && mid < 0.0, "{mid}");
        assert_eq!(s.slide(t0 + normal), 0.0);
        assert!(!s.wants_frame(t0 + normal));
        // Moving left: from +1 to 0.
        let t1 = t0 + normal;
        s.switch(Page::Notifications, t1, false);
        assert!(close_to(s.slide(t1), 1.0));
        let mid = s.slide(t1 + normal / 2);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(s.slide(t1 + normal), 0.0);
    }

    #[test]
    fn a_switch_during_a_slide_continues_from_where_the_content_is() {
        let t0 = Instant::now();
        let normal = Motion::Normal.duration(&Theme::dark().motion);
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, true);
        s.switch(Page::Tray, t0, false);
        let t1 = t0 + normal / 2;
        let before = s.slide(t1);
        assert!(before > -1.0 && before < 0.0, "{before}");
        // Back to the left mid-flight: no snap to +1.
        s.switch(Page::Activity, t1, false);
        assert!(close_to(s.slide(t1), before), "{}", s.slide(t1));
        assert_eq!(s.slide(t1 + normal), 0.0);
    }

    #[test]
    fn opening_from_closed_starts_with_the_content_at_rest() {
        let t0 = Instant::now();
        let normal = Motion::Normal.duration(&Theme::dark().motion);
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, true);
        s.switch(Page::Tray, t0, false);
        s.close(t0, true);
        assert!(close_to(s.slide(t0), -1.0), "left over by the close");
        let t1 = t0 + normal;
        s.open(Page::Tray, t1, false);
        assert_eq!(s.slide(t1), 0.0);
        assert_eq!(s.slide(t1 + normal), 0.0);
    }

    #[test]
    fn switching_to_the_same_page_is_a_no_op() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Notifications, t0, true);
        s.switch(Page::Notifications, t0, false);
        assert_eq!(s.slide(t0), 0.0);
        assert!(!s.wants_frame(t0));
        assert_eq!(s.page(), Some(Page::Notifications));
    }

    #[test]
    fn reduced_motion_switches_pages_without_a_slide() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, true);
        s.switch(Page::Tray, t0, true);
        assert_eq!(s.page(), Some(Page::Tray));
        assert_eq!(s.slide(t0), 0.0);
        assert!(!s.wants_frame(t0));
    }

    #[test]
    fn page_size_grows_to_the_pages_request_but_not_past_the_surface() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.set_surface((SURFACE_W, 560.0));
        assert_eq!(s.page_size(), PAGE_SIZE, "the default request");
        s.request_page_size((500.0, 400.0), t0, false);
        assert_eq!(s.page_size(), (500.0, 400.0));
        s.request_page_size((2000.0, 2000.0), t0, false);
        assert_eq!(
            s.page_size(),
            (SURFACE_W - 2.0 * PAGE_MARGIN, 560.0 - TOP_GAP - PAGE_MARGIN)
        );
        // A nonsense request keeps the last one.
        s.request_page_size((f32::NAN, 10.0), t0, false);
        s.request_page_size((-1.0, 10.0), t0, false);
        assert_eq!(
            s.page_size(),
            (SURFACE_W - 2.0 * PAGE_MARGIN, 560.0 - TOP_GAP - PAGE_MARGIN)
        );
        // The open rect and its silhouette follow the grown page.
        s.request_page_size((500.0, 400.0), t0, false);
        s.open(Page::Activity, t0, true);
        let r = s.rect(t0, SURFACE_W);
        assert_eq!((r.x, r.y, r.w, r.h), (130.0, 8.0, 500.0, 400.0));
        let slices = s.silhouette(t0, SURFACE_W, Theme::dark().radius_lg);
        let bottom = slices.iter().map(|b| b.y + b.h).fold(0.0, f32::max);
        assert!(close_to(bottom, 408.0), "{bottom}");
    }

    #[test]
    fn a_request_during_the_growth_rides_the_growth_clock() {
        let t0 = Instant::now();
        let slow = Motion::Slow.duration(&Theme::dark().motion);
        let mut s = IslandState::new(CAPSULE_W);
        s.set_surface((SURFACE_W, 560.0));
        s.open(Page::Activity, t0, false);
        // The next frame's views ask for more than the default page.
        s.request_page_size((500.0, 400.0), ms(t0, 16), false);
        for t in [16, 100, 175, 250, 349] {
            let now = ms(t0, t);
            let p = s.progress(now);
            let r = s.rect(now, SURFACE_W);
            assert!(
                close_to(r.w, lerp(CAPSULE_W, 500.0, p)),
                "w at {t} ms: {r:?}"
            );
            assert!(
                close_to(r.h, lerp(CAPSULE_H, 400.0, p)),
                "h at {t} ms: {r:?}"
            );
            assert!(!s.resize.is_animating(now), "one clock at {t} ms");
        }
        let end = s.rect(t0 + slow, SURFACE_W);
        assert_eq!((end.w, end.h), (500.0, 400.0));
    }

    #[test]
    fn a_new_request_while_open_animates_the_rect_instead_of_jumping() {
        let t0 = Instant::now();
        let normal = Motion::Normal.duration(&Theme::dark().motion);
        let mut s = IslandState::new(CAPSULE_W);
        s.set_surface((SURFACE_W, 560.0));
        s.open(Page::Activity, t0, true);
        let before = s.rect(t0, SURFACE_W);
        assert_eq!((before.w, before.h), PAGE_SIZE);
        s.request_page_size((500.0, 400.0), t0, false);
        assert_eq!(s.rect(t0, SURFACE_W), before, "no jump at the request");
        assert!(s.wants_frame(t0));
        let mid = s.rect(t0 + normal / 2, SURFACE_W);
        assert!(mid.w > 420.0 && mid.w < 500.0, "{mid:?}");
        assert!(mid.h > 300.0 && mid.h < 400.0, "{mid:?}");
        assert!(close_to(mid.x + mid.w / 2.0, SURFACE_W / 2.0), "centred");
        let end = s.rect(t0 + normal, SURFACE_W);
        assert_eq!((end.w, end.h), (500.0, 400.0));
        assert!(!s.wants_frame(t0 + normal));
        // The same request again restarts nothing.
        s.request_page_size((500.0, 400.0), t0 + normal, false);
        assert!(!s.wants_frame(t0 + normal));
        // A retarget mid-flight starts from the size shown then.
        s.request_page_size((420.0, 300.0), t0 + normal, false);
        let shown = s.rect(t0 + normal + normal / 2, SURFACE_W);
        s.request_page_size((460.0, 350.0), t0 + normal + normal / 2, false);
        assert_eq!(s.rect(t0 + normal + normal / 2, SURFACE_W), shown);
        // Reduced motion and a closed Island change the size in one step.
        s.request_page_size((300.0, 200.0), t0, true);
        assert_eq!(s.rect(t0, SURFACE_W).w, 300.0);
        s.close(t0, true);
        s.request_page_size((410.0, 310.0), t0, false);
        s.open(Page::Activity, t0, true);
        assert_eq!(s.rect(t0, SURFACE_W).w, 410.0);
    }

    #[test]
    fn opening_while_open_switches_with_a_slide() {
        let t0 = Instant::now();
        let mut s = IslandState::new(CAPSULE_W);
        s.open(Page::Activity, t0, true);
        s.open(Page::Tray, t0, false);
        assert!(close_to(s.slide(t0), -1.0));
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
