//! The progressive blur of a row cut by a window: the pure weights and
//! gradient stops. The row is drawn sharp and blurred at three strengths;
//! across the band at a cut edge the four levels are mixed by these weights
//! (level 0 is the sharp row), and the whole row also fades towards the
//! edge. The GPU side lives in `Painter::cut_row`.
#![forbid(unsafe_code)]

/// The blur sigmas of levels 1 to 3, in logical px; level 0 is the sharp row.
pub const SOFT_SIGMAS: [f32; 3] = [1.5, 3.0, 6.0];

/// The widest band at a cut edge, in logical px.
pub const SOFT_BAND_MAX: f32 = 96.0;

/// The alpha the row keeps at the cut edge itself.
pub const SOFT_EDGE_ALPHA: f32 = 0.4;

/// Which sides of the window cut the row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuts {
    pub left: bool,
    pub right: bool,
}

/// The band's width for a window `window_w` wide: 96 px or two fifths of the
/// window, whichever is smaller; 0 for an empty or NaN window.
#[must_use]
pub fn band_width(window_w: f32) -> f32 {
    if window_w.is_finite() && window_w > 0.0 {
        SOFT_BAND_MAX.min(window_w * 2.0 / 5.0)
    } else {
        0.0
    }
}

/// The weight of `level` (0 sharp, 1 to 3 blurred) at `t`, the distance
/// fraction into the band: 0 where the band begins, 1 at the cut edge. Each
/// level peaks at `t = level / 3` and the four always sum to 1.
#[must_use]
pub fn level_weight(level: usize, t: f32) -> f32 {
    (1.0 - (3.0 * t - level as f32).abs()).max(0.0)
}

/// The alpha of the whole row at `t`: 1 where the band begins, 0.4 at the cut.
#[must_use]
pub fn edge_alpha(t: f32) -> f32 {
    1.0 - (1.0 - SOFT_EDGE_ALPHA) * t
}

/// The gradient stops `(offset, alpha)` that mask `level` across a window
/// `window_w` wide, offsets 0..=1 from its left edge. A cut side ramps over
/// `band` px; a side with no cut keeps the sharp level at full alpha and the
/// others at nothing, all the way to the window's edge.
#[must_use]
pub fn level_stops(level: usize, window_w: f32, band: f32, cuts: Cuts) -> Vec<(f32, f32)> {
    let uncut = if level == 0 { 1.0 } else { 0.0 };
    let b = if window_w.is_finite() && window_w > 0.0 && band.is_finite() {
        (band / window_w).clamp(0.0, 0.5)
    } else {
        0.0
    };
    let at = |t: f32| level_weight(level, t) * edge_alpha(t);
    let mut stops = Vec::with_capacity(8);
    if cuts.left && b > 0.0 {
        for t in [1.0, 2.0 / 3.0, 1.0 / 3.0, 0.0] {
            stops.push(((1.0 - t) * b, at(t)));
        }
    } else {
        stops.push((0.0, uncut));
    }
    if cuts.right && b > 0.0 {
        for t in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] {
            stops.push((1.0 - b + t * b, at(t)));
        }
    } else {
        stops.push((1.0, uncut));
    }
    stops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_is_96_or_two_fifths_of_the_window() {
        assert_eq!(band_width(320.0), 96.0);
        assert_eq!(band_width(200.0), 80.0);
        assert_eq!(band_width(0.0), 0.0);
        assert_eq!(band_width(-5.0), 0.0);
        assert_eq!(band_width(f32::NAN), 0.0);
    }

    #[test]
    fn the_four_levels_always_sum_to_one() {
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let sum: f32 = (0..4).map(|k| level_weight(k, t)).sum();
            assert!((sum - 1.0).abs() < 1e-5, "t {t}: {sum}");
        }
    }

    #[test]
    fn each_level_peaks_at_its_third() {
        for k in 0..4 {
            assert!((level_weight(k, k as f32 / 3.0) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_edge_fades_to_forty_percent() {
        assert_eq!(edge_alpha(0.0), 1.0);
        assert!((edge_alpha(1.0) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn level_stops_on_two_cuts_match_the_spike() {
        // A 320 px window: the band is 96 px, 0.3 of it.
        let band = band_width(320.0);
        let b = band / 320.0;
        let stops = level_stops(
            1,
            320.0,
            band,
            Cuts {
                left: true,
                right: true,
            },
        );
        assert_eq!(stops.len(), 8);
        let want = [
            (0.0, 0.0 * 0.4),
            (b / 3.0, level_weight(1, 2.0 / 3.0) * edge_alpha(2.0 / 3.0)),
            (2.0 * b / 3.0, 1.0 * edge_alpha(1.0 / 3.0)),
            (b, level_weight(1, 0.0) * 1.0),
            (1.0 - b, 0.0),
            (1.0 - b + b / 3.0, 1.0 * edge_alpha(1.0 / 3.0)),
            (
                1.0 - b + 2.0 * b / 3.0,
                level_weight(1, 2.0 / 3.0) * edge_alpha(2.0 / 3.0),
            ),
            (1.0, 0.0),
        ];
        for (got, want) in stops.iter().zip(want) {
            assert!((got.0 - want.0).abs() < 1e-5, "{got:?} vs {want:?}");
            assert!((got.1 - want.1).abs() < 1e-5, "{got:?} vs {want:?}");
        }
        // The sharp level is whole where the band begins and gone at the cut.
        let sharp = level_stops(
            0,
            320.0,
            band,
            Cuts {
                left: true,
                right: true,
            },
        );
        assert!(
            (sharp[0].1 - 0.0).abs() < 1e-6,
            "level 0 has no weight at the edge"
        );
        assert!(
            (sharp[3].1 - 1.0).abs() < 1e-6,
            "level 0 is whole at the band's start"
        );
    }

    #[test]
    fn an_uncut_side_keeps_the_sharp_row_at_full_alpha() {
        let band = band_width(320.0);
        let cuts = Cuts {
            left: false,
            right: true,
        };
        let sharp = level_stops(0, 320.0, band, cuts);
        assert_eq!(sharp[0], (0.0, 1.0));
        for level in 1..4 {
            assert_eq!(level_stops(level, 320.0, band, cuts)[0], (0.0, 0.0));
        }
        let cuts = Cuts {
            left: true,
            right: false,
        };
        assert_eq!(
            *level_stops(0, 320.0, band, cuts).last().unwrap(),
            (1.0, 1.0)
        );
        assert_eq!(
            *level_stops(2, 320.0, band, cuts).last().unwrap(),
            (1.0, 0.0)
        );
        // No cut at all: one flat ramp.
        assert_eq!(
            level_stops(0, 320.0, band, Cuts::default()),
            vec![(0.0, 1.0), (1.0, 1.0)]
        );
    }

    #[test]
    fn stops_are_sorted_and_finite_for_any_window() {
        for w in [f32::NAN, -1.0, 0.0, 1.0, 50.0, 320.0] {
            for k in 0..4 {
                let stops = level_stops(
                    k,
                    w,
                    band_width(w),
                    Cuts {
                        left: true,
                        right: true,
                    },
                );
                assert!(stops.windows(2).all(|p| p[0].0 <= p[1].0), "{w} {k}");
                assert!(stops.iter().all(|s| s.0.is_finite() && s.1.is_finite()));
            }
        }
    }
}
