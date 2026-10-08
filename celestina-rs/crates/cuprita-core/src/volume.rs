//! Volume as the person sees it: clamped, and as a whole percentage.

/// PipeWire allows boosting past 100 %; Cuprita stops at 150 %.
pub const VOLUME_MAX: f32 = 1.5;

/// Clamps to `0.0..=VOLUME_MAX`; a NaN is silence, not a crash.
#[must_use]
pub fn clamp_volume(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, VOLUME_MAX)
    }
}

/// The whole percentage shown beside a slider.
#[must_use]
pub fn percent(v: f32) -> u8 {
    // At most 150 after the clamp, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p = (clamp_volume(v) * 100.0).round() as u8;
    p
}
