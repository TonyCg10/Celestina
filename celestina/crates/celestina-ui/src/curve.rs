//! Cubic-bezier easing curves and the motion ladder (DESIGN.md §2, §5.5).
#![forbid(unsafe_code)]

use std::time::Duration;

use celestina_paint::MotionMs;

/// CSS-style cubic bezier: control points (x1, y1, x2, y2) between (0, 0)
/// and (1, 1). `eval` maps a time fraction (x) to progress (y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier(pub f32, pub f32, pub f32, pub f32);

/// The One UI standard curve: enter and move (the default curve).
pub const ONE_UI: CubicBezier = CubicBezier(0.22, 0.25, 0.0, 1.0);
/// The style's expressive SineInOut80 variant.
pub const SINE_IN_OUT_80: CubicBezier = CubicBezier(0.33, 0.0, 0.2, 1.0);
/// The style's expressive SineInOut90 variant.
pub const SINE_IN_OUT_90: CubicBezier = CubicBezier(0.33, 0.0, 0.1, 1.0);
/// Exit: ease-in cubic.
pub const EXIT: CubicBezier = CubicBezier(0.32, 0.0, 0.67, 0.0);
/// Opacity-only changes.
pub const LINEAR: CubicBezier = CubicBezier(0.0, 0.0, 1.0, 1.0);

impl CubicBezier {
    /// Progress at time fraction `x`, clamped to 0..=1; NaN counts as the
    /// end, so a broken clock settles instead of freezing mid-flight.
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 || x.is_nan() {
            return 1.0;
        }
        let t = self.solve_t(x);
        axis(t, self.1, self.3).clamp(0.0, 1.0)
    }

    /// Parameter `t` with x(t) = `x`: a few Newton steps from `t = x`,
    /// then bisection when Newton does not converge.
    fn solve_t(&self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..8 {
            let dx = axis(t, self.0, self.2) - x;
            if dx.abs() < 1e-6 {
                return t;
            }
            let slope = axis_derivative(t, self.0, self.2);
            if slope.abs() < 1e-6 {
                break;
            }
            t -= dx / slope;
            if !(0.0..=1.0).contains(&t) {
                break;
            }
        }
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.0;
            if axis(mid, self.0, self.2) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    }
}

/// One coordinate of the curve at parameter `t`, control values `a`, `b`.
fn axis(t: f32, a: f32, b: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
}

fn axis_derivative(t: f32, a: f32, b: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

/// The duration ladder of the style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Fast,
    Normal,
    Slow,
    Exit,
}

impl Motion {
    /// The step's duration on `ladder` (the theme's `MotionMs`), capped
    /// by its ceiling.
    pub fn duration(self, ladder: &MotionMs) -> Duration {
        let ms = match self {
            Motion::Fast => ladder.fast,
            Motion::Normal => ladder.normal,
            Motion::Slow => ladder.slow,
            Motion::Exit => ladder.exit,
        };
        Duration::from_millis(u64::from(ms.min(ladder.ceiling)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const ALL: [CubicBezier; 5] = [ONE_UI, SINE_IN_OUT_80, SINE_IN_OUT_90, EXIT, LINEAR];

    /// Reference y(x) by bisection on t, independent of the implementation.
    fn reference(c: CubicBezier, x: f32) -> f32 {
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if axis(mid, c.0, c.2) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        axis((lo + hi) / 2.0, c.1, c.3)
    }

    #[test]
    fn endpoints_are_exact() {
        for c in ALL {
            assert_eq!(c.eval(0.0), 0.0);
            assert_eq!(c.eval(1.0), 1.0);
        }
    }

    #[test]
    fn matches_bisection_reference() {
        for c in ALL {
            for x in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
                let want = reference(c, x);
                let got = c.eval(x);
                assert!((got - want).abs() <= 1e-3, "{c:?} at {x}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn one_ui_eases_out_and_exit_eases_in() {
        assert!(ONE_UI.eval(0.5) > 0.8, "{}", ONE_UI.eval(0.5));
        assert!(EXIT.eval(0.5) < 0.3, "{}", EXIT.eval(0.5));
        assert!((LINEAR.eval(0.3) - 0.3).abs() < 1e-4);
    }

    #[test]
    fn durations_are_capped_by_the_ceiling() {
        let ladder = MotionMs {
            fast: 100,
            normal: 200,
            slow: 900,
            exit: 150,
            ceiling: 500,
        };
        assert_eq!(Motion::Slow.duration(&ladder), Duration::from_millis(500));
        assert_eq!(Motion::Fast.duration(&ladder), Duration::from_millis(100));
    }

    #[test]
    fn the_curves_are_the_style_s() {
        assert_eq!(ONE_UI, CubicBezier(0.22, 0.25, 0.0, 1.0));
        assert_eq!(SINE_IN_OUT_80, CubicBezier(0.33, 0.0, 0.2, 1.0));
        assert_eq!(SINE_IN_OUT_90, CubicBezier(0.33, 0.0, 0.1, 1.0));
    }

    #[test]
    fn output_is_monotonic_and_clamped() {
        for c in ALL {
            let mut prev = 0.0;
            for i in 0..=200 {
                let y = c.eval(i as f32 / 200.0);
                assert!(y >= prev - 1e-6, "{c:?} dips at {i}");
                assert!((0.0..=1.0).contains(&y));
                prev = y;
            }
        }
        assert_eq!(ONE_UI.eval(-1.0), 0.0);
        assert_eq!(ONE_UI.eval(2.0), 1.0);
        assert_eq!(ONE_UI.eval(f32::NAN), 1.0);
    }

    #[test]
    fn motion_durations_are_the_theme_ladder() {
        let ladder = celestina_paint::Theme::dark().motion;
        assert_eq!(Motion::Fast.duration(&ladder), Duration::from_millis(100));
        assert_eq!(Motion::Normal.duration(&ladder), Duration::from_millis(200));
        assert_eq!(Motion::Slow.duration(&ladder), Duration::from_millis(350));
        assert_eq!(Motion::Exit.duration(&ladder), Duration::from_millis(150));
    }
}
