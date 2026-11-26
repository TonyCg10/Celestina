//! The clocks of a capsule sheet: the rect's flights (open, switch, close, a
//! resize at rest) and the body's opacities. Pure: every method takes the
//! instant, nothing reads a clock.

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Easing, Motion, EXIT, ONE_UI};

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
    /// The capsule when the flight began: an open or a switch starts from
    /// `from` moved by how far the capsule's edges have moved since, so a
    /// capsule that slides under the flight carries the sheet with it.
    capsule: Rect,
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
        capsule: Rect,
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
            capsule,
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
            capsule,
            now,
            theme.motion,
            theme.overshoot,
            reduced,
        )
    }

    /// The body is replaced while the sheet stays: the rect flies from
    /// `shown` to the new target. `capsule` is the capsule's extent now.
    pub fn switch(&mut self, shown: Rect, capsule: Rect, now: Instant) {
        let reduced = self.reduced;
        *self = SheetMotion::flight(
            Kind::Switch,
            shown,
            capsule,
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

    /// Where the flight starts at `now`: `from`, its edges moved as far as
    /// the capsule's (`capsule` is the capsule's extent now) since the flight
    /// began, so a capsule that shrinks or grows under an open carries the
    /// sheet with it and the two stay one shape. A close starts where it was.
    fn start_at(&self, capsule: Rect) -> Rect {
        if self.kind == Kind::Close {
            return self.from;
        }
        let left = self.from.x + (capsule.x - self.capsule.x);
        let right = self.from.x
            + self.from.w
            + ((capsule.x + capsule.w) - (self.capsule.x + self.capsule.w));
        Rect {
            x: left,
            w: (right - left).max(0.0),
            ..self.from
        }
    }

    /// The rect at `now`. `capsule` is the capsule's extent now and `to` the
    /// live target (the capsule while closing). The OutBack overshoot is cut
    /// to `surface`.
    pub fn rect(&self, now: Instant, capsule: Rect, to: Rect, surface: (f32, f32)) -> Rect {
        let rect = if self.reduced {
            to
        } else if self.flying(now) {
            lerp_rect(
                self.start_at(capsule),
                to,
                self.easing.eval(self.fraction(now)),
            )
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
        let got = m.rect(ms(t0, 175.0), capsule(), tall, SURFACE);
        assert!(got.y + got.h <= SURFACE.1, "{got:?}");
        assert!(got.x >= 0.0 && got.x + got.w <= SURFACE.0);
        let nan = m.rect(
            ms(t0, 175.0),
            capsule(),
            r(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
            SURFACE,
        );
        assert!([nan.x, nan.y, nan.w, nan.h].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn the_open_flight_starts_at_the_capsule_and_overshoots_its_target() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert_eq!(m.rect(t0, capsule(), target(), SURFACE), capsule());
        let mid = m.rect(ms(t0, 175.0), capsule(), target(), SURFACE);
        assert!(mid.w > target().w, "{mid:?}");
        assert_eq!(
            m.rect(ms(t0, 350.0), capsule(), target(), SURFACE),
            target()
        );
        assert_eq!(
            m.rect(ms(t0, 900.0), capsule(), target(), SURFACE),
            target()
        );
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
        assert_eq!(m.rect(done, capsule(), capsule(), SURFACE), target());
        assert_eq!(
            m.rect(ms(done, 150.0), capsule(), capsule(), SURFACE),
            capsule()
        );
        assert!(m.wants_frame(ms(done, 100.0)));
        assert!(!m.wants_frame(ms(done, 150.0)));
    }

    #[test]
    fn a_switch_fades_the_old_body_over_the_first_30_percent() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let next = r(900.0, 8.0, 320.0, 200.0);
        m.switch(target(), capsule(), done);
        assert!(!m.is_closing());
        assert!(near(m.leaving_opacity(done), 1.0));
        assert!(near(m.leaving_opacity(ms(done, 52.5)), 0.5));
        assert!(near(m.leaving_opacity(ms(done, 105.0)), 0.0));
        assert!(near(m.body_opacity(done), 0.0));
        assert!(near(m.body_opacity(ms(done, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(done, 350.0)), 1.0));
        assert_eq!(m.rect(done, capsule(), next, SURFACE), target());
        assert_eq!(m.rect(ms(done, 350.0), capsule(), next, SURFACE), next);
        assert!(near(m.leaving_opacity(ms(done, 350.0)), 0.0));
    }

    #[test]
    fn a_resize_at_rest_morphs_on_the_normal_motion() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let taller = r(930.0, 8.0, 240.0, 376.0);
        m.resize(target(), done);
        assert_eq!(m.rect(done, capsule(), taller, SURFACE), target());
        let mid = m.rect(ms(done, 100.0), capsule(), taller, SURFACE);
        let k = ONE_UI.eval(0.5);
        assert!(near(mid.h, 276.0 + 100.0 * k), "{mid:?}");
        assert_eq!(m.rect(ms(done, 200.0), capsule(), taller, SURFACE), taller);
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
        m.resize(m.rect(during, capsule(), target(), SURFACE), during);
        assert!(m.wants_frame(ms(t0, 349.0)));
        let taller = r(930.0, 8.0, 240.0, 376.0);
        assert_eq!(m.rect(ms(t0, 350.0), capsule(), taller, SURFACE), taller);
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }

    #[test]
    fn reduced_motion_jumps_the_rect_and_still_fades_the_body() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(capsule(), &Theme::dark(), t0, true);
        assert_eq!(m.rect(t0, capsule(), target(), SURFACE), target());
        assert!(near(m.body_opacity(t0), 0.0));
        assert!(near(m.body_opacity(ms(t0, 175.0)), 0.2308));
        assert!(near(m.body_opacity(ms(t0, 350.0)), 1.0));
        let done = ms(t0, 400.0);
        m.resize(target(), done);
        let taller = r(930.0, 8.0, 240.0, 376.0);
        assert_eq!(m.rect(done, capsule(), taller, SURFACE), taller);
        m.close(taller, done);
        assert_eq!(m.rect(done, capsule(), capsule(), SURFACE), capsule());
        assert!(m.closed(done));
    }

    #[test]
    fn an_open_follows_a_capsule_that_slides_under_it() {
        let t0 = Instant::now();
        let m = opened(t0);
        // The capsule loses 38 px at its right end; a menu narrower than
        // the capsule is the capsule, so the target is that rect.
        let narrow = r(1000.0, 8.0, 62.0, 40.0);
        let to = r(1000.0, 8.0, 62.0, 140.0);
        for t in [0.0, 60.0, 175.0, 349.0] {
            let got = m.rect(ms(t0, t), narrow, to, SURFACE);
            assert!(near(got.x, 1000.0) && near(got.w, 62.0), "{t}: {got:?}");
        }
        // A capsule that moves while it does is followed on both edges.
        let sliding = r(1010.0, 8.0, 50.0, 40.0);
        let to = r(1010.0, 8.0, 50.0, 140.0);
        let got = m.rect(t0, sliding, to, SURFACE);
        assert_eq!((got.x, got.w), (1010.0, 50.0));
        // A close starts where it was, whatever the capsule does.
        let mut closing = opened(t0);
        let done = ms(t0, 400.0);
        closing.close(target(), done);
        assert_eq!(closing.rect(done, narrow, narrow, SURFACE), target());
    }

    #[test]
    fn a_switch_follows_the_capsule_from_where_the_sheet_was() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 400.0);
        let shown = r(1000.0, 8.0, 100.0, 140.0);
        m.switch(shown, capsule(), done);
        let moved = r(1000.0, 8.0, 70.0, 40.0);
        let got = m.rect(done, moved, moved, SURFACE);
        assert_eq!((got.x, got.w, got.h), (1000.0, 70.0, 140.0));
    }

    #[test]
    fn nothing_wants_a_frame_once_every_clock_has_settled() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(m.wants_frame(t0));
        assert!(!m.wants_frame(ms(t0, 350.0)));
    }
}
