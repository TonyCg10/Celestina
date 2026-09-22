//! Rates from counters that only ever grow.
//!
//! Disks and interfaces both report cumulative byte counters per name, so the
//! arithmetic that turns two readings into bytes per second is written once:
//! a name seen for the first time has no rate yet, a name that disappeared is
//! forgotten, and a counter that went backwards (a reset, a re-plug) yields
//! zero rather than a negative number or a wrap.

use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug)]
pub struct NamedCounters<const N: usize> {
    previous: HashMap<String, [u64; N]>,
}

impl<const N: usize> Default for NamedCounters<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> NamedCounters<N> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
        }
    }

    /// Per-second rates for every name present in both this reading and the
    /// previous one, in the order of `readings`. Names new to this reading
    /// are remembered but not rated; names missing from it are dropped.
    /// A zero `elapsed` rates nothing.
    pub fn sample(
        &mut self,
        readings: &[(String, [u64; N])],
        elapsed: Duration,
    ) -> Vec<(String, [f64; N])> {
        let seconds = elapsed.as_secs_f64();
        let mut rates = Vec::new();
        let mut current = HashMap::with_capacity(readings.len());
        for (name, values) in readings {
            if seconds > 0.0 {
                if let Some(previous) = self.previous.get(name) {
                    let mut rate = [0.0; N];
                    for (slot, (now, before)) in rate.iter_mut().zip(values.iter().zip(previous)) {
                        // A counter that went backwards is a reset, not a
                        // negative flow: saturate to zero.
                        *slot = now.saturating_sub(*before) as f64 / seconds;
                    }
                    rates.push((name.clone(), rate));
                }
            }
            current.insert(name.clone(), *values);
        }
        self.previous = current;
        rates
    }

    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, values: [u64; 2]) -> (String, [u64; 2]) {
        (name.to_owned(), values)
    }

    #[test]
    fn the_first_reading_of_a_name_has_no_rate() {
        let mut counters = NamedCounters::<2>::new();
        let rates = counters.sample(&[reading("sda", [1000, 2000])], Duration::from_secs(1));
        assert!(rates.is_empty());
    }

    #[test]
    fn the_second_reading_rates_the_difference_per_second() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [1000, 2000])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [3000, 2000])], Duration::from_secs(2));
        assert_eq!(rates, vec![("sda".to_owned(), [1000.0, 0.0])]);
    }

    #[test]
    fn a_name_that_appears_later_is_rated_from_its_own_second_reading() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        let rates = counters.sample(
            &[reading("sda", [10, 0]), reading("sdb", [5, 5])],
            Duration::from_secs(1),
        );
        assert_eq!(rates, vec![("sda".to_owned(), [10.0, 0.0])]);
        let rates = counters.sample(
            &[reading("sda", [20, 0]), reading("sdb", [15, 5])],
            Duration::from_secs(1),
        );
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[1], ("sdb".to_owned(), [10.0, 0.0]));
    }

    #[test]
    fn a_name_that_disappears_is_forgotten() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sdb", [0, 0])], Duration::from_secs(1));
        counters.sample(&[], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sdb", [100, 0])], Duration::from_secs(1));
        assert!(rates.is_empty(), "a re-plugged disk starts over");
    }

    #[test]
    fn a_counter_that_goes_backwards_rates_zero() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [1000, 0])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [10, 0])], Duration::from_secs(1));
        assert_eq!(rates, vec![("sda".to_owned(), [0.0, 0.0])]);
    }

    #[test]
    fn no_time_between_readings_rates_nothing() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [10, 0])], Duration::ZERO);
        assert!(rates.is_empty());
    }

    #[test]
    fn reset_forgets_everything() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        counters.reset();
        assert!(counters
            .sample(&[reading("sda", [10, 0])], Duration::from_secs(1))
            .is_empty());
    }
}
