//! An `f32` property that animates between values, driven by frame time.
#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use crate::curve::{CubicBezier, LINEAR};

/// How one animation honours reduced motion, chosen by the caller for the
/// property it animates (DESIGN.md: reduced motion makes spatial and scale
/// motion instant; opacity fades stay).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reduced {
    /// Reduced motion is on and the property is spatial (position, size,
    /// scale): the animation becomes a single step to the target.
    Spatial,
    /// The animation runs in full: an opacity fade always, and any
    /// property while reduced motion is off.
    Opacity,
}

impl Reduced {
    /// The policy of a spatial property under the `reduced_motion` switch.
    pub fn spatial(reduced_motion: bool) -> Reduced {
        if reduced_motion {
            Reduced::Spatial
        } else {
            Reduced::Opacity
        }
    }
}

/// A property resting at a value or flying between two on a curve.
#[derive(Clone, Copy, Debug)]
pub struct Animated {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
    curve: CubicBezier,
}

impl Animated {
    /// A property resting at `v`.
    pub fn new(v: f32) -> Animated {
        Animated {
            from: v,
            to: v,
            start: Instant::now(),
            duration: Duration::ZERO,
            curve: LINEAR,
        }
    }

    /// Jumps to `v` without animating; a non-finite `v` is ignored.
    pub fn set(&mut self, v: f32) {
        if !v.is_finite() {
            return;
        }
        self.from = v;
        self.to = v;
        self.duration = Duration::ZERO;
    }

    /// Starts animating from the value at `now` to `v` over `duration`
    /// (`Motion::duration` on the theme's ladder). A retarget
    /// mid-flight continues from the current value, never from the old
    /// origin; asking for the current target again is a no-op, so a caller
    /// may ask every frame without restarting the flight. Under
    /// [`Reduced::Spatial`] the change is a single step. A non-finite `v`
    /// is ignored.
    pub fn animate_to(
        &mut self,
        v: f32,
        duration: Duration,
        curve: CubicBezier,
        now: Instant,
        reduced: Reduced,
    ) {
        if !v.is_finite() {
            return;
        }
        if reduced == Reduced::Spatial {
            self.set(v);
            return;
        }
        if v == self.to {
            return;
        }
        let current = self.value(now);
        if current == v {
            self.set(v);
            return;
        }
        self.from = current;
        self.to = v;
        self.start = now;
        self.duration = duration;
        self.curve = curve;
    }

    pub fn value(&self, now: Instant) -> f32 {
        if self.from == self.to || self.duration.is_zero() {
            return self.to;
        }
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= self.duration {
            return self.to;
        }
        let x = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.to - self.from) * self.curve.eval(x)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.from != self.to && now.saturating_duration_since(self.start) < self.duration
    }

    /// Ends any flight at its target.
    pub(crate) fn settle(&mut self) {
        self.set(self.to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Motion, LINEAR, ONE_UI};

    const FULL: Reduced = Reduced::Opacity;

    fn d(m: Motion) -> Duration {
        m.duration(&celestina_paint::Theme::dark().motion)
    }

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn new_and_set_rest_immediately() {
        let t0 = Instant::now();
        let mut a = Animated::new(3.0);
        assert_eq!(a.value(t0), 3.0);
        assert!(!a.is_animating(t0));
        a.set(5.0);
        assert_eq!(a.value(t0), 5.0);
        assert_eq!(a.target(), 5.0);
        assert!(!a.is_animating(t0));
    }

    #[test]
    fn animates_linearly_then_clamps() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(10.0, d(Motion::Fast), LINEAR, t0, FULL);
        assert_eq!(a.value(t0), 0.0);
        assert!((a.value(ms(t0, 25)) - 2.5).abs() < 1e-3);
        assert!((a.value(ms(t0, 50)) - 5.0).abs() < 1e-3);
        assert!(a.is_animating(ms(t0, 99)));
        assert_eq!(a.value(ms(t0, 100)), 10.0);
        assert!(!a.is_animating(ms(t0, 100)));
        assert_eq!(a.value(ms(t0, 5000)), 10.0);
    }

    #[test]
    fn retargets_from_the_current_value() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(10.0, d(Motion::Fast), LINEAR, t0, FULL);
        // Halfway (5.0) the target changes back to 0: continue from 5.
        let t1 = ms(t0, 50);
        a.animate_to(0.0, d(Motion::Fast), LINEAR, t1, FULL);
        assert!((a.value(t1) - 5.0).abs() < 1e-3);
        assert!((a.value(ms(t1, 50)) - 2.5).abs() < 1e-3);
        assert_eq!(a.value(ms(t1, 100)), 0.0);
        assert!(!a.is_animating(ms(t1, 100)));
    }

    #[test]
    fn retarget_to_the_current_value_is_at_rest() {
        let t0 = Instant::now();
        let mut a = Animated::new(1.0);
        a.animate_to(1.0, d(Motion::Slow), LINEAR, t0, FULL);
        assert!(!a.is_animating(t0));
        assert_eq!(a.value(ms(t0, 10)), 1.0);
    }

    #[test]
    fn retarget_to_the_same_target_does_not_restart() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(10.0, d(Motion::Fast), LINEAR, t0, FULL);
        // Re-asked every frame for the same target: the flight continues.
        a.animate_to(10.0, d(Motion::Fast), LINEAR, ms(t0, 50), FULL);
        assert!((a.value(ms(t0, 75)) - 7.5).abs() < 1e-3);
        assert!(!a.is_animating(ms(t0, 100)));
        // Settled at the target: still at rest.
        a.animate_to(10.0, d(Motion::Slow), LINEAR, ms(t0, 300), FULL);
        assert!(!a.is_animating(ms(t0, 300)));
        assert_eq!(a.value(ms(t0, 310)), 10.0);
    }

    #[test]
    fn set_cancels_a_running_animation() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(10.0, d(Motion::Slow), LINEAR, t0, FULL);
        a.set(7.0);
        assert_eq!(a.value(ms(t0, 100)), 7.0);
        assert!(!a.is_animating(ms(t0, 100)));
    }

    #[test]
    fn reduced_spatial_is_a_single_step() {
        let t0 = Instant::now();
        let mut a = Animated::new(1.0);
        a.animate_to(0.96, d(Motion::Fast), ONE_UI, t0, Reduced::Spatial);
        assert_eq!(a.value(t0), 0.96);
        assert_eq!(a.target(), 0.96);
        assert!(!a.is_animating(t0));
    }

    #[test]
    fn reduced_spatial_cancels_a_running_flight() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(10.0, d(Motion::Slow), ONE_UI, t0, FULL);
        a.animate_to(20.0, d(Motion::Slow), ONE_UI, ms(t0, 50), Reduced::Spatial);
        assert_eq!(a.value(ms(t0, 50)), 20.0);
        assert!(!a.is_animating(ms(t0, 50)));
    }

    #[test]
    fn reduced_opacity_keeps_fading() {
        let t0 = Instant::now();
        let mut a = Animated::new(0.0);
        a.animate_to(1.0, d(Motion::Normal), LINEAR, t0, Reduced::Opacity);
        assert_eq!(a.value(t0), 0.0);
        assert!(a.is_animating(ms(t0, 100)));
        assert!((a.value(ms(t0, 100)) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn spatial_policy_follows_the_reduced_motion_switch() {
        assert_eq!(Reduced::spatial(true), Reduced::Spatial);
        assert_eq!(Reduced::spatial(false), Reduced::Opacity);
    }

    #[test]
    fn non_finite_targets_are_ignored() {
        let t0 = Instant::now();
        let mut a = Animated::new(1.0);
        a.animate_to(f32::NAN, d(Motion::Fast), LINEAR, t0, FULL);
        a.set(f32::INFINITY);
        assert_eq!(a.value(t0), 1.0);
        assert!(!a.is_animating(t0));
    }
}
