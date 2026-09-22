//! Memory and swap, as `/proc/meminfo` reports them.
//!
//! Used memory is total minus *available*, not minus free: `MemAvailable`
//! already accounts for reclaimable cache, and reporting cache as used is the
//! classic way to tell someone their memory is full when it is not.

use std::fmt;

use crate::ratio::percent_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub used_kib: u64,
    pub total_kib: u64,
    pub swap_used_kib: u64,
    pub swap_total_kib: u64,
}

impl Memory {
    #[must_use]
    pub fn used_percent(&self) -> u8 {
        percent_of(self.used_kib, self.total_kib)
    }

    #[must_use]
    pub fn swap_percent(&self) -> u8 {
        percent_of(self.swap_used_kib, self.swap_total_kib)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryError {
    MissingField(&'static str),
    UnreadableNumber { field: &'static str },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "/proc/meminfo has no {field}"),
            Self::UnreadableNumber { field } => {
                write!(formatter, "/proc/meminfo carries an unreadable {field}")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// Parses `/proc/meminfo`.
///
/// # Errors
///
/// Refuses a file missing `MemTotal`, `MemAvailable`, `SwapTotal` or
/// `SwapFree`, or carrying one that is not a number.
pub fn parse_meminfo(text: &str) -> Result<Memory, MemoryError> {
    let field = |name: &'static str| -> Result<u64, MemoryError> {
        let line = text
            .lines()
            .find(|line| {
                line.strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with(':'))
            })
            .ok_or(MemoryError::MissingField(name))?;
        line.split_whitespace()
            .nth(1)
            .ok_or(MemoryError::MissingField(name))?
            .parse::<u64>()
            .map_err(|_| MemoryError::UnreadableNumber { field: name })
    };

    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    let swap_total = field("SwapTotal")?;
    let swap_free = field("SwapFree")?;
    Ok(Memory {
        used_kib: total.saturating_sub(available),
        total_kib: total,
        swap_used_kib: swap_total.saturating_sub(swap_free),
        swap_total_kib: swap_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000 kB\nMemFree:         1000 kB\nMemAvailable:    8000 kB\nSwapTotal:       4000 kB\nSwapFree:        3000 kB\n";

    #[test]
    fn used_memory_leaves_reclaimable_cache_out_of_it() {
        let memory = parse_meminfo(MEMINFO).expect("readable meminfo");
        assert_eq!(memory.used_kib, 8000);
        assert_eq!(memory.total_kib, 16000);
        assert_eq!(memory.used_percent(), 50);
        assert_eq!(memory.swap_used_kib, 1000);
        assert_eq!(memory.swap_total_kib, 4000);
        assert_eq!(memory.swap_percent(), 25);
    }

    #[test]
    fn meminfo_without_the_fields_it_needs_is_refused() {
        assert_eq!(
            parse_meminfo("MemFree: 1000 kB\n"),
            Err(MemoryError::MissingField("MemTotal"))
        );
        assert_eq!(
            parse_meminfo("MemTotal: 16000 kB\n"),
            Err(MemoryError::MissingField("MemAvailable"))
        );
        // A prefix is not a field: `MemTotalish` must not answer for MemTotal.
        assert_eq!(
            parse_meminfo(
                "MemTotalish: 1 kB\nMemAvailable: 1 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n"
            ),
            Err(MemoryError::MissingField("MemTotal"))
        );
        assert_eq!(
            parse_meminfo(
                "MemTotal: lots kB\nMemAvailable: 1 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n"
            ),
            Err(MemoryError::UnreadableNumber { field: "MemTotal" })
        );
    }

    #[test]
    fn a_machine_without_swap_reports_zero_rather_than_dividing_by_it() {
        let memory =
            parse_meminfo("MemTotal: 10 kB\nMemAvailable: 5 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n")
                .expect("readable");
        assert_eq!(memory.swap_percent(), 0);
    }

    #[test]
    fn available_above_total_saturates_used_at_zero() {
        let memory = parse_meminfo(
            "MemTotal: 10 kB\nMemAvailable: 50 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n",
        )
        .expect("readable");
        assert_eq!(memory.used_kib, 0);
    }
}
