//! The last minute of a value.
//!
//! The graph always shows a full minute: a ring that is not yet full answers
//! zeros for the seconds it has not lived, so the trace starts flat at the
//! left edge and grows to the right instead of stretching to fit.

/// How many one-second samples a graph shows. The one place this number lives.
pub const HISTORY_SAMPLES: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    samples: [f32; HISTORY_SAMPLES],
    next: usize,
    filled: usize,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    #[must_use]
    pub fn new() -> Self {
        Self {
            samples: [0.0; HISTORY_SAMPLES],
            next: 0,
            filled: 0,
        }
    }

    pub fn push(&mut self, value: f32) {
        self.samples[self.next] = value;
        self.next = (self.next + 1) % HISTORY_SAMPLES;
        self.filled = (self.filled + 1).min(HISTORY_SAMPLES);
    }

    /// Oldest first, always `HISTORY_SAMPLES` long.
    #[must_use]
    pub fn values(&self) -> Vec<f32> {
        let mut out = vec![0.0; HISTORY_SAMPLES];
        let start = HISTORY_SAMPLES - self.filled;
        for (offset, slot) in out[start..].iter_mut().enumerate() {
            let index = (self.next + HISTORY_SAMPLES - self.filled + offset) % HISTORY_SAMPLES;
            *slot = self.samples[index];
        }
        out
    }

    #[must_use]
    pub fn latest(&self) -> Option<f32> {
        (self.filled > 0).then(|| self.samples[(self.next + HISTORY_SAMPLES - 1) % HISTORY_SAMPLES])
    }

    /// The largest sample in the window, or 0 when empty.
    #[must_use]
    pub fn max(&self) -> f32 {
        self.values().into_iter().fold(0.0_f32, f32::max)
    }

    /// The window scaled to its own peak: 1.0 is the busiest second shown. A
    /// window with no positive sample answers zeros. This is how a throughput
    /// graph gets a shape without a ceiling nobody knows in advance.
    #[must_use]
    pub fn fractions(&self) -> Vec<f32> {
        let peak = self.max();
        let values = self.values();
        if peak <= 0.0 {
            return vec![0.0; values.len()];
        }
        values
            .into_iter()
            .map(|value| (value / peak).clamp(0.0, 1.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_ring_is_a_flat_minute() {
        let ring = Ring::new();
        assert_eq!(ring.values(), vec![0.0; HISTORY_SAMPLES]);
        assert_eq!(ring.latest(), None);
    }

    #[test]
    fn samples_arrive_at_the_right_edge_and_age_leftwards() {
        let mut ring = Ring::new();
        ring.push(0.5);
        ring.push(1.0);
        let values = ring.values();
        assert_eq!(values.len(), HISTORY_SAMPLES);
        assert_eq!(values[HISTORY_SAMPLES - 2], 0.5);
        assert_eq!(values[HISTORY_SAMPLES - 1], 1.0);
        assert_eq!(values[0], 0.0);
        assert_eq!(ring.latest(), Some(1.0));
    }

    #[test]
    fn a_full_ring_forgets_the_oldest_sample() {
        let mut ring = Ring::new();
        for index in 0..=HISTORY_SAMPLES {
            ring.push(index as f32);
        }
        let values = ring.values();
        assert_eq!(values[0], 1.0);
        assert_eq!(values[HISTORY_SAMPLES - 1], HISTORY_SAMPLES as f32);
    }

    #[test]
    fn fractions_scale_a_series_by_its_own_peak() {
        let mut ring = Ring::new();
        ring.push(50.0);
        ring.push(200.0);
        let fractions = ring.fractions();
        assert_eq!(fractions.len(), HISTORY_SAMPLES);
        assert_eq!(ring.max(), 200.0);
        assert_eq!(fractions[HISTORY_SAMPLES - 2], 0.25);
        assert_eq!(fractions[HISTORY_SAMPLES - 1], 1.0);
        assert_eq!(fractions[0], 0.0);
    }

    #[test]
    fn a_flat_or_empty_series_has_no_peak_to_scale_by() {
        let mut ring = Ring::new();
        assert_eq!(ring.max(), 0.0);
        assert_eq!(ring.fractions(), vec![0.0; HISTORY_SAMPLES]);
        ring.push(0.0);
        ring.push(0.0);
        assert_eq!(ring.fractions(), vec![0.0; HISTORY_SAMPLES]);
    }
}
