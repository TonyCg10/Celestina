//! The clocks of a capsule menu window: the rect's flights (grow from the
//! pill, fold into it, a resize at rest), its corner radius and the body's
//! opacity. Pure: every method takes the instant, nothing reads a clock.

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Easing, Motion, EXIT, ONE_UI};

/// Which flight the rect is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
}

/// The window's rect, radius and body opacity over time.
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

pub(super) fn lerp(a: f32, b: f32, k: f32) -> f32 {
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
        let (step, easing) = match kind {
            Kind::Close => (Motion::Exit, Easing::Bezier(EXIT)),
            Kind::Open => (Motion::Slow, Easing::OutBack(overshoot)),
        };
        SheetMotion {
            kind,
            from,
            radius,
            start: now,
            duration: step.duration(&ladder),
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

    /// The window folds into `pill` (its piece's live slot; only its height
    /// counts here, for the end radius), from `shown`.
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

    /// The corner radius at `now`: from the pill's (half its height) to the
    /// theme's large radius while it grows, and back while it folds. The
    /// OutBack overshoot never takes it past either end.
    pub fn radius(&self, now: Instant) -> f32 {
        let (from, to) = self.radius;
        let k = if self.reduced || !self.flying(now) {
            1.0
        } else {
            self.easing.eval(self.fraction(now))
        };
        let r = from + (to - from) * k;
        r.clamp(from.min(to), from.max(to))
    }

    /// The body's opacity: fades in over the last 65 % of an open, out over
    /// the first 60 % of a close (the window being left in a switch folds, so
    /// it fades this way too).
    pub fn body_opacity(&self, now: Instant) -> f32 {
        let t = self.fraction(now);
        match self.kind {
            Kind::Open => ((t - 0.35) / 0.65).clamp(0.0, 1.0),
            Kind::Close => (1.0 - t / 0.6).clamp(0.0, 1.0),
        }
    }

    /// The glass's opacity: whole while the window grows or stays, and
    /// fading over the last 15 % of a fold, so the pill it folds into gives
    /// way to the piece that comes back to the slot.
    pub fn glass_opacity(&self, now: Instant) -> f32 {
        match self.kind {
            Kind::Open => 1.0,
            Kind::Close if self.reduced => 0.0,
            Kind::Close => ((1.0 - self.fraction(now)) / 0.15).clamp(0.0, 1.0),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glass_fades_over_the_last_15_percent_of_a_fold() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let pill = Rect {
            x: 100.0,
            y: 14.0,
            w: 28.0,
            h: 28.0,
        };
        let mut m = SheetMotion::open(pill, &theme, t0, false);
        assert_eq!(m.glass_opacity(t0), 1.0);
        let shown = Rect {
            x: 60.0,
            y: 56.0,
            w: 300.0,
            h: 200.0,
        };
        m.close(shown, pill, t0);
        let exit = Motion::Exit.duration(&theme.motion);
        let at = |f: f32| t0 + exit.mul_f32(f);
        assert_eq!(m.glass_opacity(at(0.5)), 1.0);
        assert_eq!(m.glass_opacity(at(0.85)), 1.0);
        assert!((m.glass_opacity(at(0.925)) - 0.5).abs() < 0.02);
        assert_eq!(m.glass_opacity(at(1.0)), 0.0);
    }

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

    #[test]
    fn closing_folds_into_the_live_slot_in_150_ms() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let slot = r(1000.0, 14.0, 28.0, 28.0);
        m.close(target(), slot, done);
        assert!(m.is_closing());
        assert!(!m.closed(ms(done, 149.0)));
        assert!(m.closed(ms(done, 150.0)));
        assert_eq!(m.rect(done, slot, SURFACE), target());
        assert_eq!(m.rect(ms(done, 150.0), slot, SURFACE), slot);
        // The end is live: a slot that moved is where it lands.
        let moved = r(1010.0, 14.0, 40.0, 28.0);
        assert_eq!(m.rect(ms(done, 150.0), moved, SURFACE), moved);
        assert!(m.wants_frame(ms(done, 100.0)));
        assert!(!m.wants_frame(ms(done, 150.0)));
        // The radius folds from the window's to the pill's.
        assert!(near(m.radius(done), Theme::dark().radius_lg));
        assert!(near(m.radius(ms(done, 150.0)), 14.0));
        // The body fades out over the first 60 %.
        assert!(near(m.body_opacity(done), 1.0));
        assert!(near(m.body_opacity(ms(done, 45.0)), 0.5));
        assert!(near(m.body_opacity(ms(done, 90.0)), 0.0));
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
        assert_eq!(m.rect(done, slot, SURFACE), slot);
        assert!(near(m.radius(done), 14.0));
        assert!(m.closed(done));
        assert!(near(m.body_opacity(ms(done, 45.0)), 0.5), "it still fades");
    }

    #[test]
    fn nothing_wants_a_frame_once_every_clock_has_settled() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(m.wants_frame(t0));
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }
}
