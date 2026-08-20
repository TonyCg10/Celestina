//! What the picture is actually doing, as numbers rather than as an impression.
//!
//! The roadmap has kept presentation-timing work shut for one reason: judder
//! that cannot be reproduced cannot be fixed, and "it stutters sometimes" is
//! not a measurement. This module is the missing half — it turns the backend's
//! counters into a rate, a verdict and a report a person can capture while the
//! thing is actually stuttering, on their own machine and their own display.
//!
//! Two facts about the backend's numbers decide the whole shape:
//!
//! - **Dropped and delayed are cumulative counters, not rates.** Reporting the
//!   total makes a long evening of watching look catastrophic next to a short
//!   clip. What matters is how many were dropped *per minute*, which is a
//!   difference between two samples divided by the time between them.
//! - **A counter can go backwards.** The backend resets them when a file is
//!   reloaded, and a difference taken across that reset would be negative and
//!   read as an impossibly good result. A drop is treated as a fresh start.
//!
//! Nothing here reads a clock. Each sample carries how far into the capture it
//! was taken, so the same recording produces the same summary every time it is
//! folded.

use std::collections::VecDeque;
use std::time::Duration;

/// How many samples one capture keeps. At the host's one-per-second cadence
/// this is ten minutes, which is far longer than anyone watches a stutter
/// before reaching for the report — and it is a fixed cost rather than a
/// recording that grows for as long as a film runs.
pub const CAPACITY: usize = 600;

/// One reading of the backend's counters.
///
/// Every field is optional because every one of them is: `display-fps` and
/// `vsync-jitter` stay unknown until the host reports when frames actually
/// reach the screen, and a backend with no video output has none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PacingSample {
    /// How far into the capture this was taken.
    pub at: Duration,
    pub dropped: Option<i64>,
    pub delayed: Option<i64>,
    pub display_fps: Option<f64>,
    pub vsync_jitter: Option<f64>,
}

/// What a capture amounts to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PacingSummary {
    /// Frames the decoder never presented, per minute.
    pub dropped_per_minute: f64,
    /// Frames presented late, per minute.
    pub delayed_per_minute: f64,
    /// The display's own refresh rate, as last reported.
    pub display_fps: Option<f64>,
    /// The worst jitter seen, in seconds. The worst and not the mean: judder is
    /// noticed at its peaks, and an average hides a spike inside a good minute.
    pub worst_jitter: Option<f64>,
    /// How long the capture covers.
    pub span: Duration,
    /// How many readings it is built from.
    pub samples: usize,
}

/// What the summary means, with the thresholds written down rather than left to
/// whoever reads the numbers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Verdict {
    /// Not enough yet to say anything. Two samples are the minimum, because one
    /// counter reading is not a rate.
    #[default]
    TooEarly,
    /// Nothing measurable is going wrong.
    Smooth,
    /// Frames are being presented late. This is what is usually *seen* as
    /// judder on a desktop that is otherwise keeping up.
    Delayed,
    /// Frames are not arriving at all. Worse than late, and usually a decoder
    /// or a disk rather than a compositor.
    Dropping,
}

impl Verdict {
    /// Above this many dropped frames a minute, something is being lost. One or
    /// two around a seek is ordinary; a steady stream is not.
    pub const DROPPED_PER_MINUTE: f64 = 6.0;
    /// Above this many late frames a minute, the picture is visibly uneven.
    pub const DELAYED_PER_MINUTE: f64 = 30.0;

    /// The word for a summary.
    #[must_use]
    pub fn of(summary: &PacingSummary) -> Self {
        if summary.samples < 2 {
            return Self::TooEarly;
        }
        if summary.dropped_per_minute > Self::DROPPED_PER_MINUTE {
            return Self::Dropping;
        }
        if summary.delayed_per_minute > Self::DELAYED_PER_MINUTE {
            return Self::Delayed;
        }
        Self::Smooth
    }
}

/// A bounded recording of what the backend reported while something played.
#[derive(Clone, Debug, Default)]
pub struct PacingCapture {
    samples: VecDeque<PacingSample>,
}

impl PacingCapture {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one reading, dropping the oldest once the capture is full.
    pub fn push(&mut self, sample: PacingSample) {
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Everything recorded, oldest first — for a report that lists the readings
    /// rather than only their conclusion.
    pub fn samples(&self) -> impl Iterator<Item = &PacingSample> {
        self.samples.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Folds the recording into rates.
    ///
    /// Counters are read as differences between consecutive samples, and a
    /// difference that comes out negative is treated as a reset and contributes
    /// nothing: the backend restarts its counters when a file reloads, and a
    /// negative rate would read as a picture that repaired itself.
    #[must_use]
    pub fn summary(&self) -> PacingSummary {
        let mut dropped = 0i64;
        let mut delayed = 0i64;
        let mut worst_jitter: Option<f64> = None;
        let mut display_fps = None;

        for sample in &self.samples {
            if let Some(jitter) = sample.vsync_jitter.filter(|value| value.is_finite()) {
                worst_jitter = Some(worst_jitter.map_or(jitter, |worst: f64| worst.max(jitter)));
            }
            if let Some(fps) = sample.display_fps.filter(|value| value.is_finite()) {
                display_fps = Some(fps);
            }
        }
        for pair in self.samples.as_slices().0.windows(2) {
            dropped += rise(pair[0].dropped, pair[1].dropped);
            delayed += rise(pair[0].delayed, pair[1].delayed);
        }
        // A ring buffer is two slices once it has wrapped, and `windows` cannot
        // see across the seam. The pair that straddles it is added by hand
        // rather than by copying the whole recording to make it contiguous.
        let (front, back) = self.samples.as_slices();
        if let (Some(last), Some(first)) = (front.last(), back.first()) {
            dropped += rise(last.dropped, first.dropped);
            delayed += rise(last.delayed, first.delayed);
        }
        for pair in back.windows(2) {
            dropped += rise(pair[0].dropped, pair[1].dropped);
            delayed += rise(pair[0].delayed, pair[1].delayed);
        }

        let span = match (self.samples.front(), self.samples.back()) {
            (Some(first), Some(last)) => last.at.saturating_sub(first.at),
            _ => Duration::ZERO,
        };
        let minutes = span.as_secs_f64() / 60.0;
        let per_minute = |count: i64| {
            if minutes > 0.0 {
                count as f64 / minutes
            } else {
                0.0
            }
        };

        PacingSummary {
            dropped_per_minute: per_minute(dropped),
            delayed_per_minute: per_minute(delayed),
            display_fps,
            worst_jitter,
            span,
            samples: self.samples.len(),
        }
    }

    /// The summary's verdict.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        Verdict::of(&self.summary())
    }
}

/// How much a counter rose between two readings, or nothing when it did not
/// rise or was not reported.
fn rise(before: Option<i64>, after: Option<i64>) -> i64 {
    match (before, after) {
        (Some(before), Some(after)) if after >= before => after - before,
        // A counter that went backwards is a reset, not a repair.
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{PacingCapture, PacingSample, Verdict, CAPACITY};
    use std::time::Duration;

    fn sample(second: u64, dropped: i64, delayed: i64) -> PacingSample {
        PacingSample {
            at: Duration::from_secs(second),
            dropped: Some(dropped),
            delayed: Some(delayed),
            display_fps: Some(59.95),
            vsync_jitter: Some(0.001),
        }
    }

    #[test]
    fn counters_become_rates_rather_than_totals() {
        let mut capture = PacingCapture::new();
        // Six dropped over thirty seconds is twelve a minute, whatever the
        // counter happened to start at.
        capture.push(sample(0, 1_000, 2_000));
        capture.push(sample(30, 1_006, 2_015));

        let summary = capture.summary();
        assert!((summary.dropped_per_minute - 12.0).abs() < 0.001);
        assert!((summary.delayed_per_minute - 30.0).abs() < 0.001);
        assert_eq!(summary.span, Duration::from_secs(30));
        assert_eq!(summary.samples, 2);
    }

    #[test]
    fn a_counter_that_restarts_is_a_reset_and_not_a_repair() {
        let mut capture = PacingCapture::new();
        capture.push(sample(0, 500, 500));
        // The file reloaded and the backend started again from zero.
        capture.push(sample(30, 0, 0));
        capture.push(sample(60, 3, 0));

        let summary = capture.summary();
        assert!(
            summary.dropped_per_minute >= 0.0,
            "a negative rate would read as a picture that repaired itself"
        );
        assert!((summary.dropped_per_minute - 3.0).abs() < 0.001);
    }

    #[test]
    fn one_reading_is_not_a_rate() {
        let mut capture = PacingCapture::new();
        capture.push(sample(0, 0, 0));
        assert_eq!(capture.verdict(), Verdict::TooEarly);
        assert_eq!(capture.summary().dropped_per_minute, 0.0);
    }

    #[test]
    fn the_verdict_names_late_frames_and_lost_ones_differently() {
        let mut smooth = PacingCapture::new();
        smooth.push(sample(0, 0, 0));
        smooth.push(sample(60, 2, 10));
        assert_eq!(smooth.verdict(), Verdict::Smooth);

        let mut late = PacingCapture::new();
        late.push(sample(0, 0, 0));
        late.push(sample(60, 0, 90));
        assert_eq!(late.verdict(), Verdict::Delayed);

        let mut losing = PacingCapture::new();
        losing.push(sample(0, 0, 0));
        losing.push(sample(60, 40, 200));
        assert_eq!(
            losing.verdict(),
            Verdict::Dropping,
            "losing frames outranks presenting them late"
        );
    }

    #[test]
    fn the_worst_jitter_survives_a_good_minute_around_it() {
        let mut capture = PacingCapture::new();
        capture.push(PacingSample {
            vsync_jitter: Some(0.0005),
            ..sample(0, 0, 0)
        });
        capture.push(PacingSample {
            vsync_jitter: Some(0.042),
            ..sample(30, 0, 0)
        });
        capture.push(PacingSample {
            vsync_jitter: Some(0.0004),
            ..sample(60, 0, 0)
        });

        let summary = capture.summary();
        assert_eq!(summary.worst_jitter, Some(0.042));
        assert_eq!(summary.display_fps, Some(59.95));
    }

    #[test]
    fn a_capture_is_bounded_and_keeps_counting_across_the_wrap() {
        let mut capture = PacingCapture::new();
        for second in 0..(CAPACITY as u64 + 100) {
            capture.push(sample(second, second as i64, 0));
        }
        assert_eq!(capture.len(), CAPACITY);

        let summary = capture.summary();
        // One dropped frame per second, whichever samples survived the wrap.
        assert!(
            (summary.dropped_per_minute - 60.0).abs() < 0.001,
            "the pair straddling the ring's seam was lost: {summary:?}"
        );
    }

    #[test]
    fn a_backend_that_reports_nothing_summarises_to_nothing_rather_than_to_zero() {
        let mut capture = PacingCapture::new();
        for second in [0, 30] {
            capture.push(PacingSample {
                at: Duration::from_secs(second),
                ..PacingSample::default()
            });
        }
        let summary = capture.summary();
        assert_eq!(summary.display_fps, None);
        assert_eq!(summary.worst_jitter, None);
        assert_eq!(summary.dropped_per_minute, 0.0);
        assert_eq!(capture.verdict(), Verdict::Smooth);
    }

    #[test]
    fn clearing_starts_a_new_capture() {
        let mut capture = PacingCapture::new();
        capture.push(sample(0, 0, 0));
        capture.clear();
        assert!(capture.is_empty());
        assert_eq!(capture.verdict(), Verdict::TooEarly);
    }
}
