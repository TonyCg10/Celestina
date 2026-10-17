//! How often the bar shows a new reading (CEL-2-K, design §4).
//!
//! The system monitor publishes every two seconds; a bar that redrew for
//! each sample would break "rest is one frame a minute" (ADR 0015). A
//! [`Cadence`] keeps the value the bar shows and copies the newest reading
//! into it at most once per period of monotonic time. A reading that appears
//! or is withdrawn is shown at once: a meter that vanished is not a value to
//! hold back. The page, while open, reads the newest reading instead.

use std::time::{Duration, Instant};

/// The bar's period for the system monitor.
pub const SYSMON_BAR_PERIOD: Duration = Duration::from_secs(10);

/// What offering a reading to a [`Cadence`] means for the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The bar already shows it.
    Same,
    /// The bar shows something new: it is drawn again.
    Changed,
    /// The bar is held back: it changes after this wait.
    Pending(Duration),
}

/// What a new reading asks of one Island's surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// No frame: nothing it shows changes.
    Nothing,
    /// A frame now.
    Now,
    /// A frame after this wait, when the held-back reading is due.
    After(Duration),
}

/// The value one bar shows, refreshed at most once per period.
#[derive(Clone, Debug)]
pub struct Cadence<T> {
    shown: Option<T>,
    copied_at: Option<Instant>,
    period: Duration,
}

impl<T: Clone + PartialEq> Cadence<T> {
    #[must_use]
    pub fn new(period: Duration) -> Self {
        Self {
            shown: None,
            copied_at: None,
            period,
        }
    }

    /// The value the bar shows.
    #[must_use]
    pub fn shown(&self) -> Option<&T> {
        self.shown.as_ref()
    }

    /// What [`Cadence::offer`] would do with `newest` at `now`, without doing
    /// it.
    #[must_use]
    pub fn peek(&self, newest: Option<&T>, now: Instant) -> Offer {
        if newest == self.shown.as_ref() {
            return Offer::Same;
        }
        if self.shown.is_none() || newest.is_none() {
            return Offer::Changed;
        }
        match self.copied_at {
            Some(at) if now.saturating_duration_since(at) < self.period => {
                Offer::Pending((at + self.period).saturating_duration_since(now))
            }
            _ => Offer::Changed,
        }
    }

    /// Offers the newest reading at `now`: copied when [`Cadence::peek`] says
    /// [`Offer::Changed`].
    pub fn offer(&mut self, newest: Option<&T>, now: Instant) -> Offer {
        let offer = self.peek(newest, now);
        if offer == Offer::Changed {
            self.shown = newest.cloned();
            self.copied_at = Some(now);
        }
        offer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn a_reading_every_two_seconds_reaches_the_bar_once_per_ten() {
        let start = Instant::now();
        let mut bar = Cadence::new(SYSMON_BAR_PERIOD);
        // The first reading shows at once.
        assert_eq!(bar.offer(Some(&0u8), start), Offer::Changed);
        let mut changes = 0;
        let mut copied = vec![];
        // A different reading every two seconds for a minute.
        for n in 1..=30u8 {
            let now = start + 2 * S * u32::from(n);
            match bar.offer(Some(&n), now) {
                Offer::Changed => {
                    changes += 1;
                    copied.push(n);
                }
                Offer::Pending(wait) => assert!(wait <= SYSMON_BAR_PERIOD, "{wait:?}"),
                Offer::Same => panic!("every reading differs"),
            }
        }
        assert_eq!(changes, 6, "{copied:?}");
        assert_eq!(copied, [5, 10, 15, 20, 25, 30]);
        assert_eq!(bar.shown(), Some(&30));
    }

    #[test]
    fn a_held_back_reading_says_when_it_is_due_and_peek_changes_nothing() {
        let start = Instant::now();
        let mut bar = Cadence::new(SYSMON_BAR_PERIOD);
        bar.offer(Some(&1u8), start);
        assert_eq!(bar.peek(Some(&2), start + 3 * S), Offer::Pending(7 * S));
        assert_eq!(bar.peek(Some(&2), start + 3 * S), Offer::Pending(7 * S));
        assert_eq!(bar.shown(), Some(&1));
        // The same value is nothing to wait for.
        assert_eq!(bar.peek(Some(&1), start + 3 * S), Offer::Same);
        // At the boundary it is shown.
        assert_eq!(bar.offer(Some(&2), start + 10 * S), Offer::Changed);
        assert_eq!(bar.shown(), Some(&2));
    }

    #[test]
    fn appearing_and_withdrawing_show_at_once() {
        let start = Instant::now();
        let mut bar = Cadence::new(SYSMON_BAR_PERIOD);
        assert_eq!(bar.offer(None::<&u8>, start), Offer::Same);
        assert_eq!(bar.offer(Some(&1), start), Offer::Changed);
        assert_eq!(bar.offer(None, start + S), Offer::Changed);
        assert_eq!(bar.shown(), None);
        assert_eq!(bar.offer(Some(&4), start + 2 * S), Offer::Changed);
        assert_eq!(bar.shown(), Some(&4));
    }
}
