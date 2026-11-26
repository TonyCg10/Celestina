//! The clocks of a capsule sheet: the rect's flights (open, switch, close, a
//! resize at rest), the body's opacities and the row's indicator pill and
//! spotlight. Pure: every method takes the instant, nothing reads a clock.

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Animated, Easing, Motion, Reduced, EXIT, LINEAR, ONE_UI};

/// The spotlight appears this long after the pill.
const SPOT_DELAY: Duration = Duration::from_millis(140);
/// The spotlight's fade out.
const SPOT_OUT: Duration = Duration::from_millis(120);

/// Which flight the rect is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Switch,
    Close,
}

/// The sheet's rect and its body's opacities over time.
#[derive(Clone, Debug)]
pub struct SheetMotion {
    kind: Kind,
    from: Rect,
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

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
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
        now: Instant,
        ladder: MotionMs,
        overshoot: f32,
        reduced: bool,
    ) -> SheetMotion {
        let (step, easing) = match kind {
            Kind::Close => (Motion::Exit, Easing::Bezier(EXIT)),
            Kind::Open | Kind::Switch => (Motion::Slow, Easing::OutBack(overshoot)),
        };
        SheetMotion {
            kind,
            from,
            start: now,
            duration: step.duration(&ladder),
            easing,
            resize: None,
            ladder,
            overshoot,
            reduced,
        }
    }

    /// A sheet opening from `capsule`.
    pub fn open(capsule: Rect, theme: &Theme, now: Instant, reduced: bool) -> SheetMotion {
        SheetMotion::flight(
            Kind::Open,
            capsule,
            now,
            theme.motion,
            theme.overshoot,
            reduced,
        )
    }

    /// The body is replaced while the sheet stays: the rect flies from
    /// `shown` to the new target.
    pub fn switch(&mut self, shown: Rect, now: Instant) {
        let reduced = self.reduced;
        *self = SheetMotion::flight(
            Kind::Switch,
            shown,
            now,
            self.ladder,
            self.overshoot,
            reduced,
        );
    }

    /// The body changed size while at rest: the rect morphs from `shown` to
    /// the new target. During a flight the flight carries the new target, so
    /// this does nothing; under reduced motion the rect jumps.
    pub fn resize(&mut self, shown: Rect, now: Instant) {
        if self.reduced || self.kind == Kind::Close || self.flying(now) {
            return;
        }
        self.resize = Some((shown, now));
    }

    /// The sheet goes back to its capsule.
    pub fn close(&mut self, shown: Rect, now: Instant) {
        if self.kind == Kind::Close {
            return;
        }
        let reduced = self.reduced;
        *self = SheetMotion::flight(
            Kind::Close,
            shown,
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

    /// The rect at `now`. `to` is the live target (the capsule while
    /// closing). The OutBack overshoot is cut to `surface`.
    pub fn rect(&self, now: Instant, to: Rect, surface: (f32, f32)) -> Rect {
        let rect = if self.reduced {
            to
        } else if self.flying(now) {
            lerp_rect(self.from, to, self.easing.eval(self.fraction(now)))
        } else if let Some((from, start)) = self.resize.filter(|_| self.kind != Kind::Close) {
            match self.resize_fraction(start, now) {
                Some(x) => lerp_rect(from, to, ONE_UI.eval(x)),
                None => to,
            }
        } else {
            to
        };
        within(rect, surface)
    }

    /// The body's opacity: fades in over the last 65 % of an open or a
    /// switch, out over the first 60 % of a close.
    pub fn body_opacity(&self, now: Instant) -> f32 {
        let t = self.fraction(now);
        match self.kind {
            Kind::Open | Kind::Switch => ((t - 0.35) / 0.65).clamp(0.0, 1.0),
            Kind::Close => (1.0 - t / 0.6).clamp(0.0, 1.0),
        }
    }

    /// The body being left in a switch: fades out over the first 30 %.
    pub fn leaving_opacity(&self, now: Instant) -> f32 {
        match self.kind {
            Kind::Switch => (1.0 - self.fraction(now) / 0.3).clamp(0.0, 1.0),
            Kind::Open | Kind::Close => 0.0,
        }
    }

    pub fn is_closing(&self) -> bool {
        self.kind == Kind::Close
    }

    /// The close flight has ended (at once under reduced motion).
    pub fn closed(&self, now: Instant) -> bool {
        self.kind == Kind::Close && (self.reduced || !self.flying(now))
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

/// What the indicator was shown with.
#[derive(Clone, Copy, Debug)]
struct Params {
    ladder: MotionMs,
    overshoot: f32,
    reduced: bool,
}

/// The pill under the open piece and the spotlight over the body.
#[derive(Clone, Debug)]
pub struct Indicator {
    x: Animated,
    w: Animated,
    opacity: Animated,
    spot: Animated,
    y: f32,
    h: f32,
    visible: bool,
    params: Option<Params>,
}

impl Default for Indicator {
    fn default() -> Indicator {
        Indicator {
            x: Animated::new(0.0),
            w: Animated::new(0.0),
            opacity: Animated::new(0.0),
            spot: Animated::new(0.0),
            y: 0.0,
            h: 0.0,
            visible: false,
            params: None,
        }
    }
}

impl Indicator {
    /// The pill appears under `pill` (or slides there when already shown);
    /// the spotlight follows 140 ms later (at once under reduced motion).
    pub fn show(&mut self, pill: Rect, theme: &Theme, now: Instant, reduced: bool) {
        let jump = !self.visible && self.opacity.value(now) <= 0.0;
        self.params = Some(Params {
            ladder: theme.motion,
            overshoot: theme.overshoot,
            reduced,
        });
        self.y = finite(pill.y);
        self.h = finite(pill.h).max(0.0);
        if jump {
            self.x.set(pill.x);
            self.w.set(pill.w);
            self.opacity.set(0.0);
            self.spot.set(0.0);
        } else {
            self.slide_to(pill, now);
        }
        self.visible = true;
        self.opacity.animate_to(
            1.0,
            Motion::Fast.duration(&theme.motion),
            LINEAR,
            now,
            Reduced::Opacity,
        );
        let delay = if reduced { Duration::ZERO } else { SPOT_DELAY };
        self.spot.animate_to(
            1.0,
            Motion::Normal.duration(&theme.motion),
            ONE_UI,
            now + delay,
            Reduced::Opacity,
        );
    }

    fn slide_to(&mut self, pill: Rect, now: Instant) {
        let Some(p) = self.params else { return };
        let duration = Motion::Slow.duration(&p.ladder);
        let reduced = Reduced::spatial(p.reduced);
        self.x
            .animate_to(pill.x, duration, Easing::OutBack(p.overshoot), now, reduced);
        self.w
            .animate_to(pill.w, duration, Easing::OutBack(p.overshoot), now, reduced);
    }

    /// The pill moves to `pill` on the open spring.
    pub fn slide(&mut self, pill: Rect, now: Instant) {
        if !self.visible {
            return;
        }
        self.y = finite(pill.y);
        self.h = finite(pill.h).max(0.0);
        self.slide_to(pill, now);
    }

    /// The pill fades out in 150 ms, the spotlight in 120 ms.
    pub fn hide(&mut self, now: Instant) {
        let Some(p) = self.params else { return };
        self.visible = false;
        self.opacity.animate_to(
            0.0,
            Motion::Exit.duration(&p.ladder),
            EXIT,
            now,
            Reduced::Opacity,
        );
        self.spot
            .animate_to(0.0, SPOT_OUT, EXIT, now, Reduced::Opacity);
    }

    /// The pill's rect and alpha, or `None` once it is gone.
    pub fn pill(&self, now: Instant) -> Option<(Rect, f32)> {
        self.params?;
        let alpha = self.opacity.value(now);
        if !self.visible && alpha <= 0.0 {
            return None;
        }
        let rect = Rect {
            x: self.x.value(now),
            y: self.y,
            w: self.w.value(now).max(0.0),
            h: self.h,
        };
        Some((rect, alpha))
    }

    pub fn spot_opacity(&self, now: Instant) -> f32 {
        self.spot.value(now)
    }

    pub fn wants_frame(&self, now: Instant) -> bool {
        [&self.x, &self.w, &self.opacity, &self.spot]
            .iter()
            .any(|a| a.is_animating(now))
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

    fn capsule() -> Rect {
        r(1000.0, 8.0, 100.0, 40.0)
    }

    fn target() -> Rect {
        r(930.0, 8.0, 240.0, 276.0)
    }

    fn opened(t0: Instant) -> SheetMotion {
        SheetMotion::open(capsule(), &Theme::dark(), t0, false)
    }

    #[test]
    fn the_overshoot_never_leaves_the_surface() {
        let t0 = Instant::now();
        let m = opened(t0);
        let tall = r(930.0, 8.0, 320.0, 544.0);
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
    fn the_open_flight_starts_at_the_capsule_and_overshoots_its_target() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert_eq!(m.rect(t0, target(), SURFACE), capsule());
        let mid = m.rect(ms(t0, 175.0), target(), SURFACE);
        assert!(mid.w > target().w, "{mid:?}");
        assert_eq!(m.rect(ms(t0, 350.0), target(), SURFACE), target());
        assert_eq!(m.rect(ms(t0, 900.0), target(), SURFACE), target());
    }

    #[test]
    fn the_body_fades_in_over_the_last_65_percent() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(near(m.body_opacity(t0), 0.0));
        assert!(near(m.body_opacity(ms(t0, 122.5)), 0.0));
        assert!(near(m.body_opacity(ms(t0, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(t0, 350.0)), 1.0));
        assert!(near(m.leaving_opacity(ms(t0, 100.0)), 0.0));
    }

    #[test]
    fn closing_fades_the_body_out_over_the_first_60_percent() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        m.close(target(), done);
        assert!(m.is_closing());
        assert!(near(m.body_opacity(done), 1.0));
        assert!(near(m.body_opacity(ms(done, 45.0)), 0.5));
        assert!(near(m.body_opacity(ms(done, 90.0)), 0.0));
        assert!(!m.closed(ms(done, 149.0)));
        assert!(m.closed(ms(done, 150.0)));
        assert_eq!(m.rect(done, capsule(), SURFACE), target());
        assert_eq!(m.rect(ms(done, 150.0), capsule(), SURFACE), capsule());
        assert!(m.wants_frame(ms(done, 100.0)));
        assert!(!m.wants_frame(ms(done, 150.0)));
    }

    #[test]
    fn a_switch_fades_the_old_body_over_the_first_30_percent() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let next = r(900.0, 8.0, 320.0, 200.0);
        m.switch(target(), done);
        assert!(!m.is_closing());
        assert!(near(m.leaving_opacity(done), 1.0));
        assert!(near(m.leaving_opacity(ms(done, 52.5)), 0.5));
        assert!(near(m.leaving_opacity(ms(done, 105.0)), 0.0));
        assert!(near(m.body_opacity(done), 0.0));
        assert!(near(m.body_opacity(ms(done, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(done, 350.0)), 1.0));
        assert_eq!(m.rect(done, next, SURFACE), target());
        assert_eq!(m.rect(ms(done, 350.0), next, SURFACE), next);
        assert!(near(m.leaving_opacity(ms(done, 350.0)), 0.0));
    }

    #[test]
    fn a_resize_at_rest_morphs_on_the_normal_motion() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let taller = r(930.0, 8.0, 240.0, 376.0);
        m.resize(target(), done);
        assert_eq!(m.rect(done, taller, SURFACE), target());
        let mid = m.rect(ms(done, 100.0), taller, SURFACE);
        let k = ONE_UI.eval(0.5);
        assert!(near(mid.h, 276.0 + 100.0 * k), "{mid:?}");
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
        let taller = r(930.0, 8.0, 240.0, 376.0);
        assert_eq!(m.rect(ms(t0, 350.0), taller, SURFACE), taller);
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }

    #[test]
    fn reduced_motion_jumps_the_rect_and_still_fades_the_body() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(capsule(), &Theme::dark(), t0, true);
        assert_eq!(m.rect(t0, target(), SURFACE), target());
        assert!(near(m.body_opacity(t0), 0.0));
        assert!(near(m.body_opacity(ms(t0, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(t0, 350.0)), 1.0));
        let done = ms(t0, 400.0);
        m.resize(target(), done);
        let taller = r(930.0, 8.0, 240.0, 376.0);
        assert_eq!(m.rect(done, taller, SURFACE), taller);
        m.close(taller, done);
        assert_eq!(m.rect(done, capsule(), SURFACE), capsule());
        assert!(m.closed(done));
    }

    #[test]
    fn the_spotlight_waits_140_ms_then_fades_in_on_one_ui() {
        let t0 = Instant::now();
        let theme = Theme::dark();
        let mut ind = Indicator::default();
        ind.show(r(1010.0, 8.0, 80.0, 40.0), &theme, t0, false);
        assert!(near(ind.spot_opacity(t0), 0.0));
        assert!(near(ind.spot_opacity(ms(t0, 140.0)), 0.0));
        assert!(near(ind.spot_opacity(ms(t0, 240.0)), ONE_UI.eval(0.5)));
        assert!((ind.spot_opacity(ms(t0, 240.0)) - 0.9047).abs() < 1e-3);
        assert!(near(ind.spot_opacity(ms(t0, 340.0)), 1.0));
        // Pill fades in over 100 ms, linear.
        let (_, a) = ind.pill(ms(t0, 50.0)).unwrap();
        assert!(near(a, 0.5));
        // Reduced motion: no delay.
        let mut quick = Indicator::default();
        quick.show(r(1010.0, 8.0, 80.0, 40.0), &theme, t0, true);
        assert!(quick.spot_opacity(ms(t0, 100.0)) > 0.0);
        assert!(near(quick.spot_opacity(ms(t0, 100.0)), ONE_UI.eval(0.5)));
    }

    #[test]
    fn the_pill_slides_with_the_open_spring() {
        let t0 = Instant::now();
        let theme = Theme::dark();
        let mut ind = Indicator::default();
        let a = r(1010.0, 8.0, 80.0, 40.0);
        let b = r(1200.0, 8.0, 120.0, 40.0);
        ind.show(a, &theme, t0, false);
        let done = ms(t0, 400.0);
        assert_eq!(ind.pill(done).unwrap().0, a);
        ind.slide(b, done);
        let (mid, _) = ind.pill(ms(done, 175.0)).unwrap();
        let k = Easing::OutBack(theme.overshoot).eval(0.5);
        assert!(near(mid.x, 1010.0 + 190.0 * k), "{mid:?}");
        assert!(near(mid.w, 80.0 + 40.0 * k));
        assert_eq!(ind.pill(ms(done, 350.0)).unwrap().0, b);
        // Hiding fades the pill out in 150 ms and the spotlight in 120 ms.
        let later = ms(done, 500.0);
        ind.hide(later);
        assert!(ind.pill(ms(later, 75.0)).unwrap().1 < 1.0);
        assert!(ind.pill(ms(later, 150.0)).is_none());
        assert!(near(ind.spot_opacity(ms(later, 120.0)), 0.0));
        let mut reduced = Indicator::default();
        reduced.show(a, &theme, t0, true);
        reduced.slide(b, done);
        assert_eq!(reduced.pill(done).unwrap().0, b);
    }

    #[test]
    fn nothing_wants_a_frame_once_every_clock_has_settled() {
        let t0 = Instant::now();
        let theme = Theme::dark();
        let m = opened(t0);
        assert!(m.wants_frame(t0));
        assert!(!m.wants_frame(ms(t0, 350.0)));
        let mut ind = Indicator::default();
        assert!(!ind.wants_frame(t0));
        ind.show(r(1010.0, 8.0, 80.0, 40.0), &theme, t0, false);
        assert!(ind.wants_frame(ms(t0, 100.0)));
        assert!(!ind.wants_frame(ms(t0, 341.0)));
        ind.hide(ms(t0, 400.0));
        assert!(ind.wants_frame(ms(t0, 450.0)));
        assert!(!ind.wants_frame(ms(t0, 551.0)));
    }
}
