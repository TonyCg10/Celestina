//! The content check's verdicts, gathered for publication.
//!
//! A small candidate group is verified in microseconds, so a check of five
//! hundred of them would otherwise hand a page five hundred verdicts, each
//! followed by a full republication, back to back. The check gathers them
//! here instead: the first verdict is published at once, and the ones after
//! it at most once per interval, in the order they were reached.

use std::time::{Duration, Instant};

use crate::usage::duplicates::Verified;

/// What the content check concluded about one candidate group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The sets proven byte-identical; none when no two copies matched.
    Sets(Vec<Verified>),
    /// A copy could not be read, or is no longer the file the scan
    /// recorded, so nothing was decided.
    Unreadable,
}

/// Verdicts waiting for their publication, each with the index of the
/// candidate group it answers.
#[derive(Debug)]
pub struct VerdictBatch {
    interval: Duration,
    last: Option<Instant>,
    pending: Vec<(usize, Verdict)>,
}

impl VerdictBatch {
    /// A batch published at most once per `interval`.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
            pending: Vec::new(),
        }
    }

    /// Gathers the verdict on candidate `index`.
    pub fn push(&mut self, index: usize, verdict: Verdict) {
        self.pending.push((index, verdict));
    }

    /// Whether nothing waits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Everything gathered, when a publication is due at `now`: the first
    /// one at once, then at most one per interval. `None` while nothing
    /// waits or the interval since the last publication has not passed.
    pub fn take_due(&mut self, now: Instant) -> Option<Vec<(usize, Verdict)>> {
        if self.pending.is_empty() {
            return None;
        }
        let due = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= self.interval);
        if !due {
            return None;
        }
        self.last = Some(now);
        Some(std::mem::take(&mut self.pending))
    }

    /// Everything still gathered, whatever the time: the check has ended.
    pub fn take_rest(&mut self) -> Vec<(usize, Verdict)> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::duplicates::Verified;
    use crate::usage::tree::NodeId;
    use std::time::{Duration, Instant};

    const INTERVAL: Duration = Duration::from_millis(250);

    fn sets(index: u32) -> Verdict {
        Verdict::Sets(vec![Verified {
            size: 8,
            nodes: vec![NodeId(index), NodeId(index + 1)],
        }])
    }

    #[test]
    fn the_first_verdict_is_published_at_once() {
        let start = Instant::now();
        let mut batch = VerdictBatch::new(INTERVAL);
        assert!(batch.take_due(start).is_none(), "nothing gathered yet");
        batch.push(0, sets(1));
        assert_eq!(batch.take_due(start), Some(vec![(0, sets(1))]));
        assert!(batch.is_empty());
    }

    #[test]
    fn a_storm_of_verdicts_becomes_one_publication_per_interval() {
        let start = Instant::now();
        let mut batch = VerdictBatch::new(INTERVAL);
        batch.push(0, sets(1));
        let mut publications = vec![batch.take_due(start).unwrap_or_default()];
        // Five hundred small groups verified within the next interval.
        for index in 1..500_u32 {
            let now = start + Duration::from_micros(u64::from(index) * 400);
            batch.push(index as usize, Verdict::Unreadable);
            if let Some(ready) = batch.take_due(now) {
                publications.push(ready);
            }
        }
        assert_eq!(publications.len(), 1, "the rest waits for the interval");
        let later = start + INTERVAL;
        let rest = batch.take_due(later).unwrap_or_default();
        assert_eq!(rest.len(), 499);
        assert_eq!(
            rest.first().map(|(index, _)| *index),
            Some(1),
            "in arrival order"
        );
        assert!(batch.take_due(later + INTERVAL).is_none(), "nothing left");
    }

    #[test]
    fn the_end_of_the_check_takes_whatever_is_left() {
        let start = Instant::now();
        let mut batch = VerdictBatch::new(INTERVAL);
        batch.push(3, sets(3));
        assert!(batch.take_due(start).is_some());
        batch.push(4, Verdict::Unreadable);
        assert!(batch.take_due(start).is_none());
        assert_eq!(batch.take_rest(), vec![(4, Verdict::Unreadable)]);
        assert!(batch.take_rest().is_empty());
    }
}
