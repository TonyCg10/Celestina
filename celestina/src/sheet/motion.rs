//! The clocks of a capsule menu window: the rect's flights (grow from the
//! pill, a resize at rest, the close that blurs it out towards its piece),
//! its corner radius, the opacities of its glass and body and the close's
//! blur. Pure: every method takes the instant, nothing reads a clock.

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Easing, Motion, LINEAR, ONE_UI};

/// Which flight the rect is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
}

/// The close's duration.
const CLOSE_MS: u64 = 260;
/// The blur a close reaches, in logical px.
const CLOSE_BLUR_PX: f32 = 12.0;
/// How much of the way to the slot's pill the rect and radius go.
const CLOSE_SHRINK: f32 = 0.7;
/// A closing window's glass below this opacity leaves the compositor's blur
/// region (the backdrop blur cannot fade).
pub const BLUR_REGION_MIN_GLASS: f32 = 0.35;

/// The window's rect, radius and opacities over time.
#[derive(Clone, Debug)]
pub struct SheetMotion {
    kind: Kind,
    /// Where the flight starts: the pill (an open) or the window as it was
    /// shown (a close).
    from: Rect,
    /// The corner radius at the start and at the end of the flight.
    radius: (f32, f32),
    start: Instant,
    duration: Duration,
    easing: Easing,
    /// A morph at rest: where it started and when.
    resize: Option<(Rect, Instant)>,
    ladder: MotionMs,
    overshoot: f32,
    reduced: bool,
}

/// `v` when finite, else 0.
fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

pub(crate) fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

pub(super) fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
    Rect {
        x: lerp(a.x, b.x, k),
        y: lerp(a.y, b.y, k),
        w: lerp(a.w, b.w, k),
        h: lerp(a.h, b.h, k),
    }
}

/// `r` with non-finite parts zeroed, cut to the surface: the OutBack
/// overshoot never paints past the surface's edges.
fn within(r: Rect, surface: (f32, f32)) -> Rect {
    let (sw, sh) = (finite(surface.0).max(0.0), finite(surface.1).max(0.0));
    let x0 = finite(r.x).max(0.0).min(sw);
    let y0 = finite(r.y).max(0.0).min(sh);
    let x1 = (finite(r.x) + finite(r.w).max(0.0)).max(x0).min(sw);
    let y1 = (finite(r.y) + finite(r.h).max(0.0)).max(y0).min(sh);
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

impl SheetMotion {
    fn flight(
        kind: Kind,
        from: Rect,
        radius: (f32, f32),
        now: Instant,
        ladder: MotionMs,
        overshoot: f32,
        reduced: bool,
    ) -> SheetMotion {
        let (duration, easing) = match kind {
            Kind::Close => (
                Duration::from_millis(CLOSE_MS.min(u64::from(ladder.ceiling))),
                Easing::Bezier(LINEAR),
            ),
            Kind::Open => (Motion::Slow.duration(&ladder), Easing::OutBack(overshoot)),
        };
        SheetMotion {
            kind,
            from,
            radius,
            start: now,
            duration,
            easing,
            resize: None,
            ladder,
            overshoot,
            reduced,
        }
    }

    /// A window growing out of `pill` (the open piece's rect in the bar, its
    /// radius half its height) to the large radius of the theme.
    pub fn open(pill: Rect, theme: &Theme, now: Instant, reduced: bool) -> SheetMotion {
        SheetMotion::flight(
            Kind::Open,
            pill,
            (finite(pill.h) / 2.0, theme.radius_lg),
            now,
            theme.motion,
            theme.overshoot,
            reduced,
        )
    }

    /// The body changed size while the window stays: the rect morphs from
    /// `shown` to the new target. During a flight the flight carries the new
    /// target, so this does nothing; under reduced motion the rect jumps.
    pub fn resize(&mut self, shown: Rect, now: Instant) {
        if self.reduced || self.kind == Kind::Close || self.flying(now) {
            return;
        }
        self.resize = Some((shown, now));
    }

    /// The window closes towards `pill` (its piece's live slot; only its
    /// height counts here, for the end radius), from `shown`.
    pub fn close(&mut self, shown: Rect, pill: Rect, now: Instant) {
        if self.kind == Kind::Close {
            return;
        }
        let reduced = self.reduced;
        let radius = (self.radius(now), finite(pill.h) / 2.0);
        *self = SheetMotion::flight(
            Kind::Close,
            shown,
            radius,
            now,
            self.ladder,
            self.overshoot,
            reduced,
        );
    }

    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.start)
    }

    fn flying(&self, now: Instant) -> bool {
        self.elapsed(now) < self.duration
    }

    /// The flight's time fraction, 0..=1.
    fn fraction(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self.elapsed(now).as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    fn resize_fraction(&self, start: Instant, now: Instant) -> Option<f32> {
        let duration = Motion::Normal.duration(&self.ladder);
        let elapsed = now.saturating_duration_since(start);
        (elapsed < duration).then(|| elapsed.as_secs_f32() / duration.as_secs_f32())
    }

    /// The rect at `now`. `to` is the live end of the flight: the window's
    /// target while it grows, the piece's pill while it folds. The OutBack
    /// overshoot is cut to `surface`.
    pub fn rect(&self, now: Instant, to: Rect, surface: (f32, f32)) -> Rect {
        let rect = if self.kind == Kind::Close {
            let k = if self.reduced {
                0.0
            } else {
                self.close_factor(now)
            };
            lerp_rect(self.from, to, k)
        } else if self.reduced {
            to
        } else if self.flying(now) {
            lerp_rect(self.from, to, self.easing.eval(self.fraction(now)))
        } else if let Some((from, start)) = self.resize {
            match self.resize_fraction(start, now) {
                Some(x) => lerp_rect(from, to, ONE_UI.eval(x)),
                None => to,
            }
        } else {
            to
        };
        within(rect, surface)
    }

    /// The corner radius at `now`: from the pill's (half its height) to the
    /// theme's large radius while it grows, and back while it folds. The
    /// OutBack overshoot never takes it past either end.
    pub fn radius(&self, now: Instant) -> f32 {
        let (from, to) = self.radius;
        let k = if self.kind == Kind::Close {
            if self.reduced {
                0.0
            } else {
                self.close_factor(now)
            }
        } else if self.reduced || !self.flying(now) {
            1.0
        } else {
            self.easing.eval(self.fraction(now))
        };
        let r = from + (to - from) * k;
        r.clamp(from.min(to), from.max(to))
    }

    /// The close's way towards the slot's pill, 0..=0.7: the rect and the
    /// radius go this far.
    fn close_factor(&self, now: Instant) -> f32 {
        let t = self.fraction(now);
        CLOSE_SHRINK * (1.0 - (1.0 - t) * (1.0 - t))
    }

    /// The body's opacity: fades in over the last 65 % of an open, and out
    /// as `1 - (t / 0.8)^1.4` over a close (the window being left in a
    /// switch closes the same way).
    pub fn body_opacity(&self, now: Instant) -> f32 {
        let t = self.fraction(now);
        match self.kind {
            Kind::Open => ((t - 0.35) / 0.65).clamp(0.0, 1.0),
            Kind::Close => (1.0 - (t / 0.8).min(1.0).powf(1.4)).clamp(0.0, 1.0),
        }
    }

    /// The glass's opacity: whole while the window grows or stays, and
    /// `1 - t^1.6` over a close.
    pub fn glass_opacity(&self, now: Instant) -> f32 {
        match self.kind {
            Kind::Open => 1.0,
            Kind::Close => (1.0 - self.fraction(now).powf(1.6)).clamp(0.0, 1.0),
        }
    }

    /// The blur over glass and content together, in logical px: none while
    /// the window is open or at rest, `12 * min(1, t / 0.6)^0.7` over a
    /// close, none under reduced motion.
    #[must_use]
    pub fn blur_px(&self, now: Instant) -> f32 {
        if self.kind != Kind::Close || self.reduced {
            return 0.0;
        }
        CLOSE_BLUR_PX * (self.fraction(now) / 0.6).min(1.0).powf(0.7)
    }

    /// Whether the compositor's blur still covers the window: always but
    /// for a close whose glass has faded below [`BLUR_REGION_MIN_GLASS`].
    #[must_use]
    pub fn blur_region(&self, now: Instant) -> bool {
        self.kind != Kind::Close || self.glass_opacity(now) >= BLUR_REGION_MIN_GLASS
    }

    /// Where the flight started: the window as it was shown, for a close.
    #[must_use]
    pub fn start_rect(&self) -> Rect {
        self.from
    }

    /// The corner radius the flight started from: the window's, for a close.
    #[must_use]
    pub fn start_radius(&self) -> f32 {
        self.radius.0
    }

    /// The duration ladder the flights run on.
    #[must_use]
    pub fn ladder(&self) -> MotionMs {
        self.ladder
    }

    /// The open spring's overshoot.
    #[must_use]
    pub fn overshoot(&self) -> f32 {
        self.overshoot
    }

    /// Reduced motion: nothing flies.
    #[must_use]
    pub fn reduced(&self) -> bool {
        self.reduced
    }

    pub fn is_closing(&self) -> bool {
        self.kind == Kind::Close
    }

    /// The close flight has ended.
    pub fn closed(&self, now: Instant) -> bool {
        self.kind == Kind::Close && !self.flying(now)
    }

    /// Whether a clock is still running and the surface needs a frame.
    pub fn wants_frame(&self, now: Instant) -> bool {
        if self.flying(now) {
            return true;
        }
        self.kind != Kind::Close
            && self
                .resize
                .is_some_and(|(_, start)| self.resize_fraction(start, now).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: (f32, f32) = (1920.0, 560.0);

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn ms(t0: Instant, n: f32) -> Instant {
        t0 + Duration::from_micros((n * 1000.0).round() as u64)
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn near_rect(a: Rect, b: Rect) -> bool {
        near(a.x, b.x) && near(a.y, b.y) && near(a.w, b.w) && near(a.h, b.h)
    }

    /// The open piece's rect in the bar.
    fn pill() -> Rect {
        r(1040.0, 14.0, 60.0, 28.0)
    }

    fn target() -> Rect {
        r(930.0, 56.0, 240.0, 236.0)
    }

    fn opened(t0: Instant) -> SheetMotion {
        SheetMotion::open(pill(), &Theme::dark(), t0, false)
    }

    fn lerp_out_back(from: Rect, to: Rect, t: f32) -> Rect {
        let k = Easing::OutBack(Theme::dark().overshoot).eval(t);
        lerp_rect(from, to, k)
    }

    #[test]
    fn the_overshoot_never_leaves_the_surface() {
        let t0 = Instant::now();
        let m = opened(t0);
        let tall = r(1500.0, 56.0, 420.0, 496.0);
        let got = m.rect(ms(t0, 175.0), tall, SURFACE);
        assert!(got.y + got.h <= SURFACE.1, "{got:?}");
        assert!(got.x >= 0.0 && got.x + got.w <= SURFACE.0);
        let nan = m.rect(
            ms(t0, 175.0),
            r(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
            SURFACE,
        );
        assert!([nan.x, nan.y, nan.w, nan.h].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn opening_grows_from_the_pill_on_the_spring() {
        let t0 = Instant::now();
        let m = opened(t0);
        let at = |n: f32| m.rect(ms(t0, n), target(), (4000.0, 4000.0));
        assert_eq!(at(0.0), pill());
        assert!(near_rect(at(175.0), lerp_out_back(pill(), target(), 0.5)));
        assert!(at(175.0).w > target().w * 0.9, "the spring is ahead");
        assert_eq!(at(350.0), target());
        assert_eq!(at(900.0), target());
        // The radius goes from the pill's (half its height) to the theme's.
        let large = Theme::dark().radius_lg;
        assert!(near(m.radius(t0), 14.0));
        assert!(near(m.radius(ms(t0, 350.0)), large));
        assert!(near(m.radius(ms(t0, 900.0)), large));
        let k = Easing::OutBack(Theme::dark().overshoot).eval(0.5);
        assert!(near(
            m.radius(ms(t0, 175.0)),
            (14.0 + (large - 14.0) * k).min(large)
        ));
        assert!((0..=350).all(|n| m.radius(ms(t0, n as f32)) <= large + 1e-3));
    }

    #[test]
    fn the_body_fades_in_over_the_last_65_percent() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(near(m.body_opacity(t0), 0.0));
        assert!(near(m.body_opacity(ms(t0, 122.5)), 0.0));
        assert!(near(m.body_opacity(ms(t0, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(t0, 350.0)), 1.0));
    }

    fn closing(t0: Instant) -> (SheetMotion, Rect, Instant) {
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let slot = r(1000.0, 14.0, 28.0, 28.0);
        m.close(target(), slot, done);
        (m, slot, done)
    }

    #[test]
    fn closing_blurs_fades_and_shrinks_towards_the_slot() {
        let t0 = Instant::now();
        let (m, slot, done) = closing(t0);
        assert!(m.is_closing());
        assert!(!m.closed(ms(done, 259.0)));
        assert!(m.closed(ms(done, 260.0)));
        let large = Theme::dark().radius_lg;
        for f in [0.0f32, 0.3, 0.6, 1.0] {
            let at = ms(done, 260.0 * f);
            let k = 0.7 * (1.0 - (1.0 - f) * (1.0 - f));
            let blur = 12.0 * (f / 0.6).min(1.0).powf(0.7);
            let body = (1.0 - (f / 0.8).min(1.0).powf(1.4)).max(0.0);
            let glass = 1.0 - f.powf(1.6);
            assert!(near(m.blur_px(at), blur), "blur at {f}");
            assert!(near(m.body_opacity(at), body), "body at {f}");
            assert!(near(m.glass_opacity(at), glass), "glass at {f}");
            assert!(
                near_rect(m.rect(at, slot, SURFACE), lerp_rect(target(), slot, k)),
                "rect at {f}"
            );
            assert!(near(m.radius(at), lerp(large, 14.0, k)), "radius at {f}");
        }
        // Whole at the first frame, gone at the last.
        assert!(near(m.blur_px(done), 0.0));
        assert!(near(m.body_opacity(done), 1.0) && near(m.glass_opacity(done), 1.0));
        assert!(near(m.blur_px(ms(done, 156.0)), 12.0));
        assert!(near(m.blur_px(ms(done, 260.0)), 12.0));
        assert!(near(m.body_opacity(ms(done, 260.0)), 0.0));
        assert!(near(m.glass_opacity(ms(done, 260.0)), 0.0));
        // The end is live: a slot that moved is where it heads.
        let moved = r(1010.0, 14.0, 40.0, 28.0);
        let k = 0.7 * (1.0 - 0.5 * 0.5);
        assert!(near_rect(
            m.rect(ms(done, 130.0), moved, SURFACE),
            lerp_rect(target(), moved, k)
        ));
    }

    #[test]
    fn a_switch_closes_the_old_window_the_same_way() {
        // The window being left is closed by `Sheets::open` with the very
        // same call; its clocks are the close's.
        let t0 = Instant::now();
        let (m, slot, done) = closing(t0);
        let mut left = opened(t0);
        left.close(target(), slot, done);
        for n in [0.0, 78.0, 156.0, 208.0, 260.0] {
            let at = ms(done, n);
            assert_eq!(left.blur_px(at), m.blur_px(at));
            assert_eq!(left.glass_opacity(at), m.glass_opacity(at));
            assert_eq!(left.body_opacity(at), m.body_opacity(at));
            assert_eq!(left.rect(at, slot, SURFACE), m.rect(at, slot, SURFACE));
        }
    }

    #[test]
    fn the_blur_region_leaves_below_35_percent_glass() {
        let t0 = Instant::now();
        let (m, _, done) = closing(t0);
        // 1 - t^1.6 = 0.35 at t = 0.65^(1/1.6), about 0.764.
        assert!(m.blur_region(done));
        assert!(m.blur_region(ms(done, 260.0 * 0.76)));
        assert!(!m.blur_region(ms(done, 260.0 * 0.77)));
        assert!(!m.blur_region(ms(done, 260.0)));
        // An open window keeps it.
        assert!(opened(t0).blur_region(t0));
    }

    #[test]
    fn closing_in_mid_open_starts_from_the_shown_rect() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let mid = ms(t0, 100.0);
        let shown = m.rect(mid, target(), SURFACE);
        m.close(shown, pill(), mid);
        assert_eq!(m.rect(mid, pill(), SURFACE), shown);
        assert_eq!(m.start_rect(), shown);
    }

    #[test]
    fn no_frame_is_asked_after_a_close() {
        let t0 = Instant::now();
        let (m, slot, done) = closing(t0);
        assert!(m.wants_frame(ms(done, 259.0)));
        assert!(!m.wants_frame(ms(done, 260.0)));
        let end = lerp_rect(target(), slot, 0.7);
        assert!(near_rect(m.rect(ms(done, 900.0), slot, SURFACE), end));
    }

    #[test]
    fn closing_in_mid_open_starts_from_the_shown_radius() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let mid = ms(t0, 100.0);
        let before = m.radius(mid);
        m.close(m.rect(mid, target(), SURFACE), pill(), mid);
        assert!(near(m.radius(mid), before));
    }

    #[test]
    fn a_resize_at_rest_morphs_on_the_normal_motion() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let taller = r(930.0, 56.0, 240.0, 336.0);
        m.resize(target(), done);
        assert_eq!(m.rect(done, taller, SURFACE), target());
        let mid = m.rect(ms(done, 100.0), taller, SURFACE);
        let k = ONE_UI.eval(0.5);
        assert!(near(mid.h, 236.0 + 100.0 * k), "{mid:?}");
        assert_eq!(m.rect(ms(done, 200.0), taller, SURFACE), taller);
        assert!(m.wants_frame(ms(done, 100.0)));
        assert!(!m.wants_frame(ms(done, 200.0)));
        // The body does not fade for a resize.
        assert!(near(m.body_opacity(ms(done, 10.0)), 1.0));
    }

    #[test]
    fn a_resize_during_a_flight_is_carried_by_the_flight() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let during = ms(t0, 100.0);
        m.resize(m.rect(during, target(), SURFACE), during);
        assert!(m.wants_frame(ms(t0, 349.0)));
        let taller = r(930.0, 56.0, 240.0, 336.0);
        assert_eq!(m.rect(ms(t0, 350.0), taller, SURFACE), taller);
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }

    #[test]
    fn reduced_motion_jumps() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(pill(), &Theme::dark(), t0, true);
        assert_eq!(m.rect(t0, target(), SURFACE), target());
        assert!(near(m.radius(t0), Theme::dark().radius_lg));
        // The opacities still fade.
        assert!(near(m.body_opacity(t0), 0.0));
        assert!(near(m.body_opacity(ms(t0, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(t0, 350.0)), 1.0));
        let done = ms(t0, 400.0);
        m.resize(target(), done);
        let taller = r(930.0, 56.0, 240.0, 336.0);
        assert_eq!(m.rect(done, taller, SURFACE), taller);
        let slot = r(1000.0, 14.0, 28.0, 28.0);
        m.close(taller, slot, done);
        assert!(m.closed(ms(done, 260.0)));
        assert!(!m.closed(ms(done, 100.0)), "the opacities still fade");
    }

    #[test]
    fn reduced_motion_closes_by_fading_only() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(pill(), &Theme::dark(), t0, true);
        let done = ms(t0, 400.0);
        let slot = r(1000.0, 14.0, 28.0, 28.0);
        m.close(target(), slot, done);
        for n in [0.0, 78.0, 156.0, 260.0] {
            let at = ms(done, n);
            assert_eq!(m.blur_px(at), 0.0, "no blur");
            assert_eq!(m.rect(at, slot, SURFACE), target(), "no shrink");
            assert!(near(m.radius(at), Theme::dark().radius_lg));
        }
        assert!(near(
            m.glass_opacity(ms(done, 130.0)),
            1.0 - 0.5f32.powf(1.6)
        ));
        assert!(near(
            m.body_opacity(ms(done, 104.0)),
            1.0 - 0.5f32.powf(1.4)
        ));
        assert!(m.wants_frame(ms(done, 130.0)));
        assert!(!m.wants_frame(ms(done, 260.0)));
    }

    #[test]
    fn nothing_wants_a_frame_once_every_clock_has_settled() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(m.wants_frame(t0));
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }
}
