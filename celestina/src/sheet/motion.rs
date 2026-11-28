//! The clocks of a capsule menu window: the drop that opens it and folds it
//! back (the frames come from [`super::drop`]), a resize at rest, the travel
//! to another menu's shape on the same capsule, and the travel's two
//! contents (the one left fades and blurs out, the new one fades in
//! sharpening). Pure: every method takes the instant, nothing reads a clock.

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Easing, Motion, LINEAR, ONE_UI};

use super::drop::{self, DropFrame, Source, CLOSE_MS, MERGE_AT, OPEN_MS};

/// Which flight the window is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    /// The window on the same capsule goes to another menu's shape.
    Travel,
}

/// The travel's duration.
pub const TRAVEL_MS: u64 = 300;
/// The fraction of a travel by which the content left has gone.
const TRAVEL_OLD_END: f32 = 0.3;
/// The blur the content left reaches, and the new one starts from, in
/// logical px.
const TRAVEL_BLUR_PX: f32 = 8.0;
/// The fade of an open or a close under reduced motion, ms.
const REDUCED_MS: u64 = 150;
/// The fraction of a close at which the piece the window was open on gives
/// back its pill and its opacity: the drop's merge into the capsule
/// ([`SheetMotion::merged`]). Under reduced motion the close's first frame.
pub const PIECE_RESTORE_AT: f32 = MERGE_AT;

/// A content as it was shown: its opacity and its blur in logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub alpha: f32,
    pub blur: f32,
}

/// The window's frames over time.
#[derive(Clone, Debug)]
pub struct SheetMotion {
    kind: Kind,
    /// Where a travel starts, or a close: the window as it was shown.
    from: Rect,
    /// The corner radius it was shown at there (the window's own, or less
    /// while a drop is still opening it).
    from_radius: f32,
    /// A close: the glass it fades from (whole unless it cut a reduced-motion
    /// open short).
    from_glass: f32,
    /// The window's corner radius at rest (the theme's large one).
    radius: f32,
    start: Instant,
    duration: Duration,
    easing: Easing,
    /// A morph at rest: where it started and when.
    resize: Option<(Rect, Instant)>,
    ladder: MotionMs,
    overshoot: f32,
    reduced: bool,
    /// A travel: the content being left, as it was shown at the start. A
    /// close: the content it fades from, whole unless it cut an open or a
    /// travel short.
    old: Shown,
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

pub(crate) fn lerp_rect(a: Rect, b: Rect, k: f32) -> Rect {
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
        radius: f32,
        now: Instant,
        ladder: MotionMs,
        overshoot: f32,
        reduced: bool,
    ) -> SheetMotion {
        let ceiling = u64::from(ladder.ceiling);
        let capped = |ms: u64| Duration::from_millis(ms.min(ceiling));
        let (duration, easing) = match kind {
            Kind::Travel => (
                if reduced {
                    Duration::ZERO
                } else {
                    capped(TRAVEL_MS)
                },
                Easing::Bezier(ONE_UI),
            ),
            Kind::Close => (
                capped(if reduced { REDUCED_MS } else { CLOSE_MS }),
                Easing::Bezier(LINEAR),
            ),
            Kind::Open => (
                capped(if reduced { REDUCED_MS } else { OPEN_MS }),
                Easing::Bezier(LINEAR),
            ),
        };
        SheetMotion {
            kind,
            from,
            from_radius: radius,
            from_glass: 1.0,
            radius,
            start: now,
            duration,
            easing,
            resize: None,
            ladder,
            overshoot,
            reduced,
            old: Shown {
                alpha: 1.0,
                blur: 0.0,
            },
        }
    }

    /// A window dropping out of `pill` (the open piece's rect in the bar):
    /// the drop of [`drop::open_frame`], 460 ms, or a 150 ms fade under
    /// reduced motion.
    pub fn open(pill: Rect, theme: &Theme, now: Instant, reduced: bool) -> SheetMotion {
        SheetMotion::flight(
            Kind::Open,
            pill,
            theme.radius_lg,
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

    /// The window closes from `shown` (the rect as painted now, at the
    /// corner radius `shown_radius`) into the drop under its piece. The
    /// piece's live pill is read by every frame ([`SheetMotion::frame`]), not
    /// here. A close that cuts an open or a travel short starts from the
    /// radius, the content and (under reduced motion) the glass it shows at
    /// that frame, so nothing pops back to whole.
    pub fn close(&mut self, shown: Rect, shown_radius: f32, _pill: Rect, now: Instant) {
        if self.kind == Kind::Close {
            return;
        }
        let content = self.content_shown(now);
        let glass = self
            .clock_frame(now, WHERE_NOT, Source { cx: 0.0, w: 0.0 })
            .glass;
        *self = SheetMotion::flight(
            Kind::Close,
            shown,
            self.radius,
            now,
            self.ladder,
            self.overshoot,
            self.reduced,
        );
        self.from_radius = finite(shown_radius);
        self.from_glass = glass;
        self.old = content;
    }

    /// Takes in the reduced-motion setting at the moment of a switch (the
    /// flight that follows reads it).
    pub fn set_reduced(&mut self, reduced: bool) {
        self.reduced = reduced;
    }

    /// The window goes to another menu's shape on the same capsule: from
    /// `shown` (the rect as painted now, at the corner radius
    /// `shown_radius`) to the target, the radius going to the theme's large
    /// one, with a whole glass. `old` is the content being left as it is
    /// shown now. Starts from any flight, a close included; under reduced
    /// motion the swap is at once.
    pub fn travel(
        &mut self,
        shown: Rect,
        shown_radius: f32,
        old: Shown,
        theme: &Theme,
        now: Instant,
    ) {
        let reduced = self.reduced;
        *self = SheetMotion::flight(
            Kind::Travel,
            shown,
            theme.radius_lg,
            now,
            theme.motion,
            theme.overshoot,
            reduced,
        );
        self.from_radius = finite(shown_radius);
        self.old = old;
    }

    /// The open body as painted at `now`: its opacity and its blur.
    #[must_use]
    pub fn content_shown(&self, now: Instant) -> Shown {
        let frame = self.clock_frame(now, WHERE_NOT, Source { cx: 0.0, w: 0.0 });
        Shown {
            alpha: frame.content,
            blur: frame.blur,
        }
    }

    /// The new content's own blur over a travel, in logical px:
    /// `8 * (1 - clamp((t - 0.2) / 0.6))`. None for every other flight and
    /// under reduced motion.
    #[must_use]
    pub fn content_blur_px(&self, now: Instant) -> f32 {
        if self.kind != Kind::Travel || self.reduced {
            return 0.0;
        }
        TRAVEL_BLUR_PX * (1.0 - ((self.fraction(now) - 0.2) / 0.6).clamp(0.0, 1.0))
    }

    /// The content left by a travel, while it still shows: opacity
    /// `a0 * (1 - t / 0.3)`, blur `max(b0, 8 * t / 0.3)`. `None` once it has
    /// gone (`t >= 0.3`) and for every other flight.
    #[must_use]
    pub fn old_content(&self, now: Instant) -> Option<Shown> {
        if self.kind != Kind::Travel {
            return None;
        }
        let k = self.fraction(now) / TRAVEL_OLD_END;
        (k < 1.0).then(|| Shown {
            alpha: self.old.alpha * (1.0 - k),
            blur: self.old.blur.max(TRAVEL_BLUR_PX * k),
        })
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

    /// The window at rest on `to`, morphing there from a resize.
    fn at_rest(&self, now: Instant, to: Rect) -> DropFrame {
        let rect = match self.resize {
            Some((from, start)) => match self.resize_fraction(start, now) {
                Some(x) => lerp_rect(from, to, ONE_UI.eval(x)),
                None => to,
            },
            None => to,
        };
        DropFrame::window(rect, self.radius)
    }

    /// The frame of the clocks alone, the rect not yet cut to the surface.
    fn clock_frame(&self, now: Instant, to: Rect, src: Source) -> DropFrame {
        let t = self.fraction(now);
        match self.kind {
            Kind::Open if self.reduced => {
                let fade = LINEAR.eval(t);
                DropFrame {
                    glass: fade,
                    content: fade,
                    ..DropFrame::window(to, self.radius)
                }
            }
            Kind::Open if self.flying(now) => {
                drop::open_frame(t, to, src, self.overshoot, self.radius)
            }
            Kind::Open => self.at_rest(now, to),
            Kind::Close if self.reduced => {
                // From the glass and the content shown when it started: a
                // close that cuts the open's fade short never flashes whole.
                let fade = 1.0 - LINEAR.eval(t);
                DropFrame {
                    glass: self.from_glass * fade,
                    content: self.old.alpha * fade,
                    ..DropFrame::window(self.from, self.from_radius)
                }
            }
            Kind::Close => {
                let mut frame = drop::close_frame(t, self.from, self.from_radius, src);
                frame.content *= self.old.alpha;
                frame.blur = frame.blur.max(self.old.blur);
                frame
            }
            Kind::Travel => {
                let mut frame = if self.flying(now) {
                    let k = self.easing.eval(t);
                    DropFrame::window(
                        lerp_rect(self.from, to, k),
                        lerp(self.from_radius, self.radius, k.clamp(0.0, 1.0)),
                    )
                } else {
                    self.at_rest(now, to)
                };
                frame.content = ((t - 0.2) / 0.5).clamp(0.0, 1.0);
                frame.blur = self.content_blur_px(now);
                frame
            }
        }
    }

    /// The window at `now`. `target` is the live window rect it grows to (or
    /// travels to, or rests on); `pill` is the live rect of the piece it
    /// hangs from, which an open and a close read every frame; `surface`
    /// cuts the rect (the OutBack overshoot never paints past the edges).
    /// A frame whose `drawn` is false is only liquid: its rect is the bulb's
    /// box.
    #[must_use]
    pub fn frame(&self, now: Instant, target: Rect, pill: Rect, surface: (f32, f32)) -> DropFrame {
        let mut frame = self.clock_frame(now, target, Source::of(pill));
        frame.rect = within(frame.rect, surface);
        frame
    }

    /// The rect of [`SheetMotion::frame`].
    #[must_use]
    pub fn rect(&self, now: Instant, target: Rect, pill: Rect, surface: (f32, f32)) -> Rect {
        self.frame(now, target, pill, surface).rect
    }

    /// Whether a close has reached [`MERGE_AT`] of its way: the piece's pill
    /// and opacity go back then. Under reduced motion at the close's start.
    #[must_use]
    pub fn merged(&self, now: Instant) -> bool {
        if self.kind != Kind::Close {
            return false;
        }
        if self.reduced {
            return true;
        }
        let at = (self.duration.as_micros() as f32 * MERGE_AT).round();
        self.elapsed(now).as_micros() as f32 >= at
    }

    /// Where the flight started: the window as it was shown, for a close.
    #[must_use]
    pub fn start_rect(&self) -> Rect {
        self.from
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

    /// How far the close has run, 0..=1 of its time; `None` while the window
    /// is not closing.
    #[must_use]
    pub fn close_fraction(&self, now: Instant) -> Option<f32> {
        (self.kind == Kind::Close).then(|| self.fraction(now))
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

/// The rect `content_shown` passes: the content never reads it.
const WHERE_NOT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 0.0,
    h: 0.0,
};

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

    /// Two frames are the same drop, up to the rounding of the clock.
    fn near_frame(a: DropFrame, b: DropFrame) -> bool {
        near_rect(a.rect, b.rect)
            && near(a.radius, b.radius)
            && a.drawn == b.drawn
            && near(a.glass, b.glass)
            && near(a.content, b.content)
            && near(a.blur, b.blur)
            && match (a.liquid, b.liquid) {
                (None, None) => true,
                (Some(x), Some(y)) => near(x.a, y.a) && near(x.m, y.m) && near(x.yw, y.yw),
                _ => false,
            }
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

    fn frame_of(m: &SheetMotion, at: Instant) -> DropFrame {
        m.frame(at, target(), pill(), SURFACE)
    }

    /// An open that closes 500 ms in, from the window at rest.
    fn closing(t0: Instant) -> (SheetMotion, Instant) {
        let mut m = opened(t0);
        let done = ms(t0, 500.0);
        m.close(target(), 20.0, pill(), done);
        (m, done)
    }

    #[test]
    fn opening_is_drop_d1_over_460_ms() {
        let t0 = Instant::now();
        let m = opened(t0);
        let over = Theme::dark().overshoot;
        for n in [0.0f32, 129.0, 239.0, 300.0, 460.0] {
            let want = drop::open_frame(n / 460.0, target(), Source::of(pill()), over, 20.0);
            assert!(near_frame(frame_of(&m, ms(t0, n)), want), "at {n} ms");
        }
        assert_eq!(frame_of(&m, t0).liquid.map(|l| l.a), Some(3.0));
        assert_eq!(
            frame_of(&m, ms(t0, 460.0)),
            DropFrame::window(target(), 20.0)
        );
        assert_eq!(
            frame_of(&m, ms(t0, 900.0)),
            DropFrame::window(target(), 20.0)
        );
        assert!(m.wants_frame(ms(t0, 459.0)));
        assert!(!m.wants_frame(ms(t0, 460.0)));
    }

    #[test]
    fn closing_is_drop_d1_over_400_ms_from_the_shown_rect() {
        let t0 = Instant::now();
        let (m, done) = closing(t0);
        assert!(m.is_closing());
        assert!(!m.closed(ms(done, 399.0)));
        assert!(m.closed(ms(done, 400.0)));
        for n in [0.0f32, 100.0, 168.0, 300.0, 399.0] {
            let want = drop::close_frame(n / 400.0, target(), 20.0, Source::of(pill()));
            assert!(near_frame(frame_of(&m, ms(done, n)), want), "at {n} ms");
        }
        assert_eq!(m.start_rect(), target());
        assert!(!m.wants_frame(ms(done, 400.0)));
        assert!(m.wants_frame(ms(done, 399.0)));
    }

    #[test]
    fn the_close_reads_the_live_pill_every_frame() {
        let t0 = Instant::now();
        let (m, done) = closing(t0);
        let moved = r(pill().x + 100.0, pill().y, pill().w, pill().h);
        let at = ms(done, 300.0);
        let a = m.frame(at, target(), pill(), SURFACE);
        let b = m.frame(at, target(), moved, SURFACE);
        assert!(near(b.rect.x - a.rect.x, 100.0), "{a:?} {b:?}");
        let want = drop::close_frame(0.75, target(), 20.0, Source::of(moved));
        assert!(near_frame(b, want));
    }

    #[test]
    fn merged_turns_true_at_320_ms_of_the_close() {
        let t0 = Instant::now();
        let (m, done) = closing(t0);
        assert!(!m.merged(done));
        assert!(!m.merged(ms(done, 319.0)));
        assert!(m.merged(ms(done, 320.0)));
        assert!(m.merged(ms(done, 900.0)));
        assert!(!opened(t0).merged(ms(t0, 900.0)));
    }

    #[test]
    fn reduced_motion_fades_in_place_over_150_ms() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(pill(), &Theme::dark(), t0, true);
        let f = frame_of(&m, t0);
        assert_eq!(f.rect, target());
        assert!(f.drawn && f.liquid.is_none());
        assert!(near(f.glass, 0.0) && near(f.content, 0.0) && f.blur == 0.0);
        let f = frame_of(&m, ms(t0, 75.0));
        assert!(near(f.glass, 0.5) && near(f.content, 0.5), "{f:?}");
        assert_eq!(f.rect, target());
        assert_eq!(
            frame_of(&m, ms(t0, 150.0)),
            DropFrame::window(target(), 20.0)
        );
        assert!(m.wants_frame(ms(t0, 149.0)));
        assert!(!m.wants_frame(ms(t0, 150.0)));
        // The close fades out where the window stands.
        let at = ms(t0, 400.0);
        let taller = r(930.0, 56.0, 240.0, 336.0);
        m.close(taller, 20.0, pill(), at);
        assert!(m.merged(at), "the pill goes back at once");
        let f = m.frame(at, target(), pill(), SURFACE);
        assert_eq!(f.rect, taller);
        assert!(near(f.glass, 1.0) && near(f.content, 1.0));
        let f = m.frame(ms(at, 75.0), target(), pill(), SURFACE);
        assert!(near(f.glass, 0.5) && near(f.content, 0.5) && f.liquid.is_none());
        assert!(!m.closed(ms(at, 149.0)));
        assert!(m.closed(ms(at, 150.0)));
    }

    #[test]
    fn the_glass_never_fades_outside_reduced_motion() {
        let t0 = Instant::now();
        let m = opened(t0);
        for n in 0..=460 {
            assert_eq!(frame_of(&m, ms(t0, n as f32)).glass, 1.0, "open at {n}");
        }
        let (m, done) = closing(t0);
        for n in 0..=400 {
            assert_eq!(frame_of(&m, ms(done, n as f32)).glass, 1.0, "close at {n}");
        }
    }

    #[test]
    fn the_durations_follow_the_ladder_ceiling() {
        let t0 = Instant::now();
        let mut theme = Theme::dark();
        theme.motion.ceiling = 300;
        let mut m = SheetMotion::open(pill(), &theme, t0, false);
        assert!(m.wants_frame(ms(t0, 299.0)));
        assert!(!m.wants_frame(ms(t0, 300.0)));
        assert_eq!(
            m.frame(ms(t0, 300.0), target(), pill(), SURFACE),
            DropFrame::window(target(), 20.0)
        );
        let at = ms(t0, 500.0);
        m.close(target(), 20.0, pill(), at);
        assert!(!m.closed(ms(at, 299.0)));
        assert!(m.closed(ms(at, 300.0)));
        // The merge moment scales with it: 80 % of 300 ms.
        assert!(!m.merged(ms(at, 239.0)));
        assert!(m.merged(ms(at, 240.0)));
    }

    #[test]
    fn the_overshoot_never_leaves_the_surface() {
        let t0 = Instant::now();
        let m = opened(t0);
        let tall = r(1500.0, 56.0, 420.0, 496.0);
        for n in (0..=460).step_by(5) {
            let got = m.rect(ms(t0, n as f32), tall, pill(), SURFACE);
            assert!(got.y + got.h <= SURFACE.1 && got.y >= 0.0, "{got:?}");
            assert!(got.x >= 0.0 && got.x + got.w <= SURFACE.0, "{got:?}");
        }
        let nan = m.rect(
            ms(t0, 300.0),
            r(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
            pill(),
            SURFACE,
        );
        assert!([nan.x, nan.y, nan.w, nan.h].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn nothing_wants_a_frame_once_every_clock_has_settled() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert!(m.wants_frame(t0));
        assert!(!m.wants_frame(ms(t0, 460.0)));
        let (m, done) = closing(t0);
        assert!(m.wants_frame(done));
        assert!(!m.wants_frame(ms(done, 400.0)));
    }

    #[test]
    fn the_close_fraction_runs_over_the_close_only() {
        let t0 = Instant::now();
        let m = opened(t0);
        assert_eq!(m.close_fraction(t0), None);
        assert_eq!(m.close_fraction(ms(t0, 900.0)), None);
        let (m, done) = closing(t0);
        assert_eq!(m.close_fraction(done), Some(0.0));
        assert!(near(m.close_fraction(ms(done, 200.0)).unwrap_or(-1.0), 0.5));
        assert_eq!(m.close_fraction(ms(done, 400.0)), Some(1.0));
        assert_eq!(m.close_fraction(ms(done, 900.0)), Some(1.0));
    }

    #[test]
    fn closing_in_mid_open_starts_from_the_shown_rect_and_content() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let mid = ms(t0, 100.0);
        let shown = m.rect(mid, target(), pill(), SURFACE);
        let content = m.content_shown(mid);
        m.close(shown, 20.0, pill(), mid);
        assert_eq!(m.start_rect(), shown);
        assert_eq!(m.content_shown(mid), content, "no pop of the content");
    }

    #[test]
    fn a_resize_at_rest_morphs_on_the_normal_motion() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let done = ms(t0, 500.0);
        let taller = r(930.0, 56.0, 240.0, 336.0);
        m.resize(target(), done);
        assert_eq!(m.rect(done, taller, pill(), SURFACE), target());
        let mid = m.rect(ms(done, 100.0), taller, pill(), SURFACE);
        let k = ONE_UI.eval(0.5);
        assert!(near(mid.h, 236.0 + 100.0 * k), "{mid:?}");
        assert_eq!(m.rect(ms(done, 200.0), taller, pill(), SURFACE), taller);
        assert!(m.wants_frame(ms(done, 100.0)));
        assert!(!m.wants_frame(ms(done, 200.0)));
        // The body does not fade for a resize.
        assert!(near(m.content_shown(ms(done, 10.0)).alpha, 1.0));
        assert!(frame_of(&m, ms(done, 10.0)).drawn);
    }

    #[test]
    fn a_resize_during_a_flight_is_carried_by_the_flight() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let during = ms(t0, 100.0);
        m.resize(m.rect(during, target(), pill(), SURFACE), during);
        assert!(m.wants_frame(ms(t0, 459.0)));
        let taller = r(930.0, 56.0, 240.0, 336.0);
        assert_eq!(m.rect(ms(t0, 460.0), taller, pill(), SURFACE), taller);
        assert!(!m.wants_frame(ms(t0, 460.0)));
    }

    fn travel_to() -> Rect {
        r(1400.0, 56.0, 360.0, 300.0)
    }

    const FRESH: Shown = Shown {
        alpha: 1.0,
        blur: 0.0,
    };

    /// An open window at rest that travels at `t0 + 500 ms`.
    fn travelling(t0: Instant) -> (SheetMotion, Instant) {
        let mut m = opened(t0);
        let at = ms(t0, 500.0);
        m.travel(target(), 20.0, FRESH, &Theme::dark(), at);
        (m, at)
    }

    fn travel_frame(m: &SheetMotion, at: Instant) -> DropFrame {
        m.frame(at, travel_to(), pill(), SURFACE)
    }

    #[test]
    fn a_travel_morphs_the_rect_on_one_ui_in_300_ms() {
        let t0 = Instant::now();
        let (m, at) = travelling(t0);
        let to = travel_to();
        assert!(near_rect(travel_frame(&m, at).rect, target()));
        assert!(near_rect(
            travel_frame(&m, ms(at, 150.0)).rect,
            lerp_rect(target(), to, ONE_UI.eval(0.5))
        ));
        assert!(near_rect(travel_frame(&m, ms(at, 300.0)).rect, to));
        assert_eq!(travel_frame(&m, ms(at, 900.0)).rect, to);
        for n in [0.0, 75.0, 150.0, 299.0, 300.0] {
            let f = travel_frame(&m, ms(at, n));
            assert_eq!(f.radius, 20.0, "radius at {n}");
            assert_eq!(f.glass, 1.0, "glass at {n}");
            assert!(f.drawn && f.liquid.is_none(), "no drop at {n}");
        }
        assert!(m.wants_frame(ms(at, 299.0)));
        assert!(!m.wants_frame(ms(at, 300.0)));
        assert!(!m.is_closing());
    }

    #[test]
    fn the_old_content_fades_and_blurs_out_by_90_ms() {
        let t0 = Instant::now();
        let (m, at) = travelling(t0);
        let old = |n: f32| m.old_content(ms(at, n));
        let s = old(0.0).unwrap_or(Shown {
            alpha: -1.0,
            blur: -1.0,
        });
        assert!(near(s.alpha, 1.0) && near(s.blur, 0.0), "{s:?}");
        let s = old(45.0).unwrap_or(Shown {
            alpha: -1.0,
            blur: -1.0,
        });
        assert!(near(s.alpha, 0.5) && near(s.blur, 4.0), "{s:?}");
        assert!(old(89.0).is_some());
        assert!(old(90.0).is_none());
        assert!(old(900.0).is_none());
        // An open at rest has none.
        assert!(opened(t0).old_content(t0).is_none());
    }

    #[test]
    fn the_new_content_fades_in_from_60_ms_and_is_sharp_at_240_ms() {
        let t0 = Instant::now();
        let (m, at) = travelling(t0);
        for (n, a) in [
            (0.0, 0.0),
            (60.0, 0.0),
            (90.0, 0.2),
            (150.0, 0.6),
            (210.0, 1.0),
        ] {
            let f = travel_frame(&m, ms(at, n));
            assert!(near(f.content, a), "opacity at {n}");
        }
        for (n, b) in [
            (0.0, 8.0),
            (60.0, 8.0),
            (90.0, 6.667),
            (150.0, 4.0),
            (210.0, 1.333),
            (240.0, 0.0),
            (300.0, 0.0),
        ] {
            assert!(
                (m.content_blur_px(ms(at, n)) - b).abs() < 2e-3,
                "blur at {n}"
            );
            assert!((travel_frame(&m, ms(at, n)).blur - b).abs() < 2e-3);
        }
        assert_eq!(opened(t0).content_blur_px(t0), 0.0);
    }

    /// A time in the open where the window is drawn and its corners are
    /// still opening (a radius under 19).
    fn mid_open(t0: Instant) -> (SheetMotion, Instant, DropFrame) {
        let m = opened(t0);
        let mid = ms(t0, 270.0);
        let shown = frame_of(&m, mid);
        assert!(shown.drawn && shown.radius < 19.0, "premise: {shown:?}");
        (m, mid, shown)
    }

    #[test]
    fn a_travel_during_the_open_starts_from_the_rect_and_radius_shown() {
        let t0 = Instant::now();
        let (mut m, mid, shown) = mid_open(t0);
        m.travel(shown.rect, shown.radius, FRESH, &Theme::dark(), mid);
        let first = travel_frame(&m, mid);
        assert!(near_rect(first.rect, shown.rect));
        assert!(near(first.radius, shown.radius), "no pop: {first:?}");
        // The radius goes to the theme's large one on the travel's own curve.
        let halfway = travel_frame(&m, ms(mid, 150.0));
        let k = ONE_UI.eval(0.5);
        assert!(
            near(halfway.radius, lerp(shown.radius, 20.0, k)),
            "{halfway:?}"
        );
        assert!(near_rect(
            travel_frame(&m, ms(mid, 300.0)).rect,
            travel_to()
        ));
        assert_eq!(travel_frame(&m, ms(mid, 300.0)).radius, 20.0);
        assert_eq!(travel_frame(&m, ms(mid, 900.0)).radius, 20.0);
    }

    #[test]
    fn closing_in_mid_open_starts_from_the_shown_radius() {
        let t0 = Instant::now();
        let (mut m, mid, shown) = mid_open(t0);
        m.close(shown.rect, shown.radius, pill(), mid);
        let first = frame_of(&m, mid);
        assert!(near(first.radius, shown.radius), "no pop to 20: {first:?}");
        assert!(near_rect(first.rect, shown.rect));
        // The gather runs from it to the bulb's 12, never through 20.
        for n in (0..=168).step_by(8) {
            let f = frame_of(&m, ms(mid, n as f32));
            assert!(
                f.radius <= shown.radius + 1e-3 && f.radius >= 12.0 - 1e-3,
                "+{n}: {f:?}"
            );
        }
        // A close from rest still gathers from the window's own radius.
        let (rest, done) = closing(t0);
        assert!(near(frame_of(&rest, done).radius, 20.0));
    }

    #[test]
    fn a_close_that_cuts_a_reduced_open_short_fades_from_what_is_shown() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(pill(), &Theme::dark(), t0, true);
        let at = ms(t0, 75.0);
        let shown = frame_of(&m, at);
        assert!(
            near(shown.glass, 0.5) && near(shown.content, 0.5),
            "{shown:?}"
        );
        m.close(shown.rect, shown.radius, pill(), at);
        // The next frame is what was shown, never whole.
        let f = frame_of(&m, at);
        assert!(near(f.glass, 0.5) && near(f.content, 0.5), "{f:?}");
        let f = frame_of(&m, ms(at, 75.0));
        assert!(near(f.glass, 0.25) && near(f.content, 0.25), "{f:?}");
        for n in 0..=150 {
            let f = frame_of(&m, ms(at, n as f32));
            assert!(
                f.glass <= 0.5 + 1e-3 && f.content <= 0.5 + 1e-3,
                "+{n}: {f:?}"
            );
        }
        let f = frame_of(&m, ms(at, 150.0));
        assert!(near(f.glass, 0.0) && near(f.content, 0.0));
    }

    #[test]
    fn a_travel_during_a_close_turns_the_window_back() {
        let t0 = Instant::now();
        let (mut m, done) = closing(t0);
        let at = ms(done, 200.0);
        let shown = m.rect(at, target(), pill(), SURFACE);
        let old = m.content_shown(at);
        m.travel(shown, 20.0, old, &Theme::dark(), at);
        let o = m.old_content(at).unwrap_or(Shown {
            alpha: -1.0,
            blur: -1.0,
        });
        assert!(near(o.alpha, old.alpha) && near(o.blur, old.blur), "{o:?}");
        assert!(!m.is_closing());
        assert!(!m.closed(ms(at, 900.0)));
        assert!(near_rect(travel_frame(&m, at).rect, shown));
        assert_eq!(travel_frame(&m, ms(at, 300.0)).glass, 1.0);
    }

    #[test]
    fn a_close_during_a_travel_fades_from_the_content_shown() {
        let t0 = Instant::now();
        let (mut m, at) = travelling(t0);
        // +150 ms: the new content shows 0.6 at 4 px.
        let mid = ms(at, 150.0);
        let shown = travel_frame(&m, mid).rect;
        m.close(shown, 20.0, pill(), mid);
        assert!(m.is_closing());
        let s = m.content_shown(mid);
        assert!(near(s.alpha, 0.6), "{s:?}");
        assert!(near(s.blur, 4.0), "{s:?}");
        // It fades on the close's own curve, from that start.
        let f = frame_of(&m, ms(mid, 112.0));
        assert!(near(f.content, 0.0), "gone by 28 % of the close: {f:?}");
        assert!(frame_of(&m, ms(mid, 400.0)).blur >= 4.0);
        // From rest a close still starts whole and unblurred.
        let mut rest = opened(t0);
        rest.close(target(), 20.0, pill(), at);
        let s = rest.content_shown(at);
        assert!(near(s.alpha, 1.0) && near(s.blur, 0.0));
    }

    #[test]
    fn the_old_content_keeps_a_blur_it_already_had() {
        let t0 = Instant::now();
        let mut m = opened(t0);
        let at = ms(t0, 500.0);
        let old = Shown {
            alpha: 0.4,
            blur: 10.0,
        };
        m.travel(target(), 20.0, old, &Theme::dark(), at);
        let s = m.old_content(at).unwrap_or(Shown {
            alpha: -1.0,
            blur: -1.0,
        });
        assert!(near(s.alpha, 0.4) && near(s.blur, 10.0), "{s:?}");
        let s = m.old_content(ms(at, 45.0)).unwrap_or(Shown {
            alpha: -1.0,
            blur: -1.0,
        });
        assert!(near(s.alpha, 0.2) && near(s.blur, 10.0), "{s:?}");
    }

    #[test]
    fn a_resize_during_a_travel_is_carried_by_the_travel() {
        let t0 = Instant::now();
        let (mut m, at) = travelling(t0);
        let during = ms(at, 100.0);
        m.resize(travel_frame(&m, during).rect, during);
        assert!(m.wants_frame(ms(at, 299.0)));
        let taller = r(1400.0, 56.0, 360.0, 400.0);
        assert!(near_rect(
            m.rect(ms(at, 300.0), taller, pill(), SURFACE),
            taller
        ));
        assert!(!m.wants_frame(ms(at, 300.0)));
        assert!(!m.wants_frame(ms(at, 500.0)));
    }

    #[test]
    fn reduced_motion_swaps_at_once() {
        let t0 = Instant::now();
        let mut m = SheetMotion::open(pill(), &Theme::dark(), t0, true);
        let at = ms(t0, 400.0);
        m.travel(target(), 20.0, FRESH, &Theme::dark(), at);
        let f = travel_frame(&m, at);
        assert_eq!(f, DropFrame::window(travel_to(), 20.0));
        assert_eq!(m.content_blur_px(at), 0.0);
        assert!(m.old_content(at).is_none());
        assert!(!m.wants_frame(at));
    }
}
