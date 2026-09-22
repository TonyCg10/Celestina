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
}
