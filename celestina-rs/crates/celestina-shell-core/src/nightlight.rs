//! The fixed night-light whitepoint and its bounded gamma transition.
//!
//! The shell offers one explicit 2700 K state rather than a schedule.  This
//! module owns the pure part of that promise: the exact whitepoint Celestina's
//! former `wlsunset` provider used, a monotonic transition with quiet endpoints,
//! and the native 16-bit ramps the Wayland adapter transports.  It deliberately
//! knows nothing about Wayland objects, file descriptors, outputs, or threads.

use std::time::Duration;

/// The complete neutral-to-warm or warm-to-neutral transition.
pub const TRANSITION_DURATION: Duration = Duration::from_millis(300);
/// Nineteen samples include both endpoints and leave eighteen 60 Hz intervals.
const TRANSITION_FRAME_COUNT: usize = 19;
/// Far above real DRM gamma sizes while keeping a hostile compositor bounded.
const MAX_GAMMA_RAMP_SIZE: u32 = 1 << 20;

/// Per-channel whitepoint multipliers in normalized sRGB space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Whitepoint {
    red: f64,
    green: f64,
    blue: f64,
}

impl Whitepoint {
    /// An identity gamma table: no colour-temperature adjustment.
    pub const NEUTRAL: Self = Self {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
    };

    /// The fixed 2700 K endpoint used by Celestina's night-light switch.
    #[must_use]
    pub fn warm_2700k() -> Self {
        // This is `calc_whitepoint(2700)` specialized from wlsunset's
        // `color.c`: its Illuminant D and Planckian loci are blended by the
        // same cosine factor, converted with the same XYZ-to-sRGB matrix and
        // 1/2.2 transfer, then normalized by the largest channel. Keeping the
        // calculation here avoids replacing the former effect with a generic
        // RGB approximation that merely looks close.
        const TEMPERATURE: f64 = 2700.0;

        let daylight_x = 0.244_063 + 0.099_11e3 / TEMPERATURE + 2.967_8e6 / TEMPERATURE.powi(2)
            - 4.607_0e9 / TEMPERATURE.powi(3);
        let daylight_y = -3.0 * daylight_x.powi(2) + 2.870 * daylight_x - 0.275;

        let planckian_x = -0.266_123_9e9 / TEMPERATURE.powi(3)
            - 0.234_358_9e6 / TEMPERATURE.powi(2)
            + 0.877_695_6e3 / TEMPERATURE
            + 0.179_910;
        let planckian_y = -0.954_947_6 * planckian_x.powi(3) - 1.374_185_93 * planckian_x.powi(2)
            + 2.091_370_15 * planckian_x
            - 0.167_488_67;

        let blend = ((std::f64::consts::PI * ((4000.0 - TEMPERATURE) / 1500.0)).cos() + 1.0) / 2.0;
        let x = daylight_x * blend + planckian_x * (1.0 - blend);
        let y = daylight_y * blend + planckian_y * (1.0 - blend);
        let z = 1.0 - x - y;

        let red = (3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z)
            .clamp(0.0, 1.0)
            .powf(1.0 / 2.2);
        let green = (-0.969_266_0 * x + 1.876_010_8 * y + 0.041_556_0 * z)
            .clamp(0.0, 1.0)
            .powf(1.0 / 2.2);
        let blue = (0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z)
            .clamp(0.0, 1.0)
            .powf(1.0 / 2.2);
        let maximum = red.max(green).max(blue);

        Self {
            red: red / maximum,
            green: green / maximum,
            blue: blue / maximum,
        }
    }

    fn interpolate(self, target: Self, progress: f64) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        // Smoothstep is monotonic but has zero slope at both ends. The first
        // and last physical colour changes therefore do not arrive as another
        // smaller flash around an otherwise gradual transition.
        let eased = progress * progress * (3.0 - 2.0 * progress);
        Self {
            red: self.red + (target.red - self.red) * eased,
            green: self.green + (target.green - self.green) * eased,
            blue: self.blue + (target.blue - self.blue) * eased,
        }
    }

    #[cfg(test)]
    fn channels(self) -> [f64; 3] {
        [self.red, self.green, self.blue]
    }
}

/// One sample in a complete night-light transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionFrame {
    /// When this sample is due relative to the transition's start.
    pub offset: Duration,
    /// The whitepoint to apply at that instant.
    pub whitepoint: Whitepoint,
}

/// Returns all samples for one bounded transition, including both endpoints.
#[must_use]
pub fn transition(from: Whitepoint, to: Whitepoint) -> Vec<TransitionFrame> {
    let last = TRANSITION_FRAME_COUNT - 1;
    (0..TRANSITION_FRAME_COUNT)
        .map(|index| {
            let index_u32 = u32::try_from(index).unwrap_or(u32::MAX);
            let last_u32 = u32::try_from(last).unwrap_or(u32::MAX);
            let progress = f64::from(index_u32) / f64::from(last_u32);
            let whitepoint = if index == 0 {
                from
            } else if index == last {
                to
            } else {
                from.interpolate(to, progress)
            };
            let nanos =
                TRANSITION_DURATION.as_nanos() * u128::from(index_u32) / u128::from(last_u32);
            TransitionFrame {
                offset: Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX)),
                whitepoint,
            }
        })
        .collect()
}

/// Builds red, green, then blue native-value ramps for one output.
///
/// `None` rejects sizes that cannot describe a ramp or whose three channels
/// cannot fit in memory. The Wayland adapter serializes the returned values in
/// native byte order, as the wlroots protocol requires.
#[must_use]
pub fn gamma_ramp(size: u32, whitepoint: Whitepoint) -> Option<Vec<u16>> {
    if !(2..=MAX_GAMMA_RAMP_SIZE).contains(&size) {
        return None;
    }
    let channel_length = usize::try_from(size).ok()?;
    let length = channel_length.checked_mul(3)?;
    let mut ramp = Vec::with_capacity(length);
    let denominator = f64::from(size - 1);

    for gain in [whitepoint.red, whitepoint.green, whitepoint.blue] {
        for index in 0..size {
            // This is wlsunset's gamma=1 table exactly: conversion to u16
            // truncates the non-negative value rather than rounding it.
            let position = f64::from(index) / denominator;
            let sample = f64::from(u16::MAX) * position * gain;
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the normalized inputs prove every sample is finite and within u16"
            )]
            ramp.push(sample as u16);
        }
    }
    Some(ramp)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-12;

    #[test]
    fn warm_endpoint_matches_wlsunset_2700_kelvin() {
        let actual = Whitepoint::warm_2700k().channels();
        let expected = [1.0, 0.672_610_352_350_144_1, 0.351_897_448_798_771_6];

        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= EPSILON);
        }
    }

    #[test]
    fn both_directions_are_bounded_monotonic_and_keep_their_endpoints() {
        for (from, to, direction) in [
            (Whitepoint::NEUTRAL, Whitepoint::warm_2700k(), -1.0),
            (Whitepoint::warm_2700k(), Whitepoint::NEUTRAL, 1.0),
        ] {
            let frames = transition(from, to);
            assert_eq!(frames.len(), TRANSITION_FRAME_COUNT);
            assert_eq!(frames.first().map(|frame| frame.whitepoint), Some(from));
            assert_eq!(frames.last().map(|frame| frame.whitepoint), Some(to));
            assert_eq!(
                frames.first().map(|frame| frame.offset),
                Some(Duration::ZERO)
            );
            assert_eq!(
                frames.last().map(|frame| frame.offset),
                Some(TRANSITION_DURATION)
            );

            for pair in frames.windows(2) {
                assert!(pair[0].offset < pair[1].offset);
                let before = pair[0].whitepoint.channels();
                let after = pair[1].whitepoint.channels();
                assert!((after[0] - before[0]).abs() <= EPSILON);
                assert!((after[1] - before[1]) * direction >= -EPSILON);
                assert!((after[2] - before[2]) * direction >= -EPSILON);
            }
        }
    }

    #[test]
    fn identity_ramp_has_three_complete_linear_channels() {
        assert_eq!(
            gamma_ramp(4, Whitepoint::NEUTRAL),
            Some(vec![
                0, 21_845, 43_690, 65_535, 0, 21_845, 43_690, 65_535, 0, 21_845, 43_690, 65_535,
            ])
        );
    }

    #[test]
    fn warm_ramp_has_expected_length_endpoints_and_order() {
        let size = 256_u32;
        let ramp = gamma_ramp(size, Whitepoint::warm_2700k()).expect("size is valid");
        let size = usize::try_from(size).expect("the fixture size fits usize");
        assert_eq!(ramp.len(), size * 3);
        assert_eq!([ramp[0], ramp[size], ramp[size * 2]], [0, 0, 0]);
        assert_eq!(
            [ramp[size - 1], ramp[size * 2 - 1], ramp[size * 3 - 1]],
            [65_535, 44_079, 23_061]
        );
        for channel in ramp.chunks_exact(size) {
            assert!(channel.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }

    #[test]
    fn unusable_or_overflowing_sizes_are_refused() {
        assert_eq!(gamma_ramp(0, Whitepoint::NEUTRAL), None);
        assert_eq!(gamma_ramp(1, Whitepoint::NEUTRAL), None);
        assert!(gamma_ramp(u32::MAX, Whitepoint::NEUTRAL).is_none());
    }
}
