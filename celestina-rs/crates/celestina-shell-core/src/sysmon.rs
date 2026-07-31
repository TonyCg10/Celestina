//! CPU and memory, as the kernel reports them and the panel shows them.
//!
//! Reading `/proc` is two lines of IO; deciding what those lines *mean* is the
//! part worth testing, so all of it lives here as functions over text. A
//! caller hands in what it read and gets back a value it can publish, or an
//! error naming what was unreadable — never a guess.
//!
//! CPU is a rate, not a reading: `/proc/stat` counts ticks since boot, so a
//! percentage only exists between two samples. That is why [`CpuSampler`] holds
//! the previous one and refuses to invent a number for the first.

use std::fmt;

/// Above this a value is worth noticing; above [`CRITICAL_PERCENT`] it is worth
/// interrupting for. These are the thresholds the author already lives with
/// (`cpuWarningThreshold` 80, `cpuCriticalThreshold` 90, and the same pair for
/// memory), so the panel's idea of "busy" does not change under them. The panel
/// maps these states to appearance; the numbers are policy and live here, not
/// in the theme.
pub const ELEVATED_PERCENT: u8 = 80;
pub const CRITICAL_PERCENT: u8 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Normal,
    Elevated,
    Critical,
}

impl Load {
    #[must_use]
    pub fn of(percent: u8) -> Self {
        if percent >= CRITICAL_PERCENT {
            Self::Critical
        } else if percent >= ELEVATED_PERCENT {
            Self::Elevated
        } else {
            Self::Normal
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Elevated => "elevated",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SysmonError {
    NoCpuLine,
    NotEnoughCpuFields,
    UnreadableNumber,
    MissingMemoryField(&'static str),
    /// Two samples that are not apart in time say nothing about a rate.
    NoElapsedTime,
}

impl fmt::Display for SysmonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCpuLine => write!(formatter, "/proc/stat has no aggregate cpu line"),
            Self::NotEnoughCpuFields => write!(formatter, "/proc/stat's cpu line is too short"),
            Self::UnreadableNumber => write!(formatter, "/proc carried an unreadable number"),
            Self::MissingMemoryField(field) => {
                write!(formatter, "/proc/meminfo has no {field}")
            }
            Self::NoElapsedTime => write!(formatter, "two cpu samples with no time between them"),
        }
    }
}

impl std::error::Error for SysmonError {}

/// One reading of `/proc/stat`'s aggregate line: how many ticks the machine
/// spent idle, and how many it spent at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTicks {
    pub idle: u64,
    pub total: u64,
}

/// Parses the aggregate `cpu` line of `/proc/stat`.
///
/// # Errors
///
/// Refuses a file with no aggregate line, a line too short to carry idle and
/// iowait, or any field that is not a number.
pub fn parse_cpu(stat: &str) -> Result<CpuTicks, SysmonError> {
    let line = stat
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or(SysmonError::NoCpuLine)?;

    let mut ticks = Vec::new();
    for field in line.split_whitespace().skip(1) {
        ticks.push(
            field
                .parse::<u64>()
                .map_err(|_| SysmonError::UnreadableNumber)?,
        );
    }
    // user, nice, system, idle, iowait — everything after is optional and still
    // counts toward the total, which is what keeps this correct on a kernel
    // that adds another column.
    if ticks.len() < 5 {
        return Err(SysmonError::NotEnoughCpuFields);
    }

    Ok(CpuTicks {
        idle: ticks[3] + ticks[4],
        total: ticks.iter().sum(),
    })
}

/// Turns successive `/proc/stat` readings into a busy percentage.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<CpuTicks>,
}

impl CpuSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The busy percentage since the previous sample, or `None` for the first
    /// one — a rate needs two readings, and the panel would rather show nothing
    /// than a number invented from one. Whole percent, because that is what a
    /// bar shows; the arithmetic stays in integers rather than borrowing a
    /// float's rounding for a number nobody reads that closely.
    ///
    /// # Errors
    ///
    /// Returns [`SysmonError::NoElapsedTime`] when no tick passed between the
    /// two samples, which would otherwise divide by zero.
    pub fn sample(&mut self, ticks: CpuTicks) -> Result<Option<u8>, SysmonError> {
        let Some(previous) = self.previous.replace(ticks) else {
            return Ok(None);
        };

        let total = ticks.total.saturating_sub(previous.total);
        if total == 0 {
            return Err(SysmonError::NoElapsedTime);
        }
        let idle = ticks.idle.saturating_sub(previous.idle).min(total);

        Ok(Some(percent_of(total.saturating_sub(idle), total)))
    }

    /// Forgets the previous reading, so the next sample starts a fresh rate
    /// rather than measuring across a gap the panel was not watching.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub used_kib: u64,
    pub total_kib: u64,
}

impl Memory {
    #[must_use]
    pub fn used_percent(&self) -> u8 {
        percent_of(self.used_kib, self.total_kib)
    }
}

/// `part` of `whole` as whole percent, saturating at 100 and answering 0 for a
/// whole of nothing. Integer arithmetic throughout: tick and kibibyte counts
/// are exact, and turning them into floats to divide would only add rounding.
fn percent_of(part: u64, whole: u64) -> u8 {
    if whole == 0 {
        return 0;
    }
    u8::try_from(part.min(whole).saturating_mul(100) / whole).unwrap_or(100)
}

/// Parses `/proc/meminfo`.
///
/// Used memory is total minus *available*, not minus free: the kernel's
/// `MemAvailable` already accounts for reclaimable cache, and reporting cache
/// as used is the classic way to tell someone their memory is full when it is
/// not.
///
/// # Errors
///
/// Refuses a file missing either field, or carrying one that is not a number.
pub fn parse_memory(meminfo: &str) -> Result<Memory, SysmonError> {
    let field = |name: &'static str| -> Result<u64, SysmonError> {
        let line = meminfo
            .lines()
            .find(|line| line.starts_with(name) && line[name.len()..].starts_with(':'))
            .ok_or(SysmonError::MissingMemoryField(name))?;
        line.split_whitespace()
            .nth(1)
            .ok_or(SysmonError::MissingMemoryField(name))?
            .parse::<u64>()
            .map_err(|_| SysmonError::UnreadableNumber)
    };

    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    Ok(Memory {
        used_kib: total.saturating_sub(available),
        total_kib: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  100 20 50 800 30 0 0 0 0 0\ncpu0 50 10 25 400 15 0 0 0 0 0\nintr 1\n";

    #[test]
    fn the_aggregate_cpu_line_counts_idle_and_iowait_as_idle() {
        let ticks = parse_cpu(STAT).expect("a readable stat");

        assert_eq!(ticks.idle, 830);
        assert_eq!(ticks.total, 1000);
    }

    #[test]
    fn an_unreadable_stat_is_refused_rather_than_guessed() {
        assert_eq!(parse_cpu("intr 1\n"), Err(SysmonError::NoCpuLine));
        assert_eq!(
            parse_cpu("cpu  1 2 3\n"),
            Err(SysmonError::NotEnoughCpuFields)
        );
        assert_eq!(
            parse_cpu("cpu  1 2 3 four 5\n"),
            Err(SysmonError::UnreadableNumber)
        );
    }

    #[test]
    fn the_first_sample_reports_nothing_because_a_rate_needs_two() {
        let mut sampler = CpuSampler::new();

        assert_eq!(
            sampler.sample(CpuTicks {
                idle: 830,
                total: 1000
            }),
            Ok(None)
        );
        // Half the new ticks were busy.
        assert_eq!(
            sampler.sample(CpuTicks {
                idle: 880,
                total: 1100
            }),
            Ok(Some(50))
        );
    }

    #[test]
    fn two_samples_with_no_time_between_them_report_no_rate() {
        let mut sampler = CpuSampler::new();
        let ticks = CpuTicks {
            idle: 830,
            total: 1000,
        };
        sampler.sample(ticks).expect("first sample");

        assert_eq!(sampler.sample(ticks), Err(SysmonError::NoElapsedTime));
    }

    #[test]
    fn a_reset_sampler_measures_from_the_next_reading_only() {
        let mut sampler = CpuSampler::new();
        sampler
            .sample(CpuTicks {
                idle: 830,
                total: 1000,
            })
            .expect("first sample");

        sampler.reset();
        assert_eq!(
            sampler.sample(CpuTicks {
                idle: 880,
                total: 1100
            }),
            Ok(None)
        );
    }

    #[test]
    fn used_memory_leaves_reclaimable_cache_out_of_it() {
        let memory = parse_memory(
            "MemTotal:       16000 kB\nMemFree:         1000 kB\nMemAvailable:    8000 kB\n",
        )
        .expect("readable meminfo");

        assert_eq!(memory.used_kib, 8000);
        assert_eq!(memory.total_kib, 16000);
        assert_eq!(memory.used_percent(), 50);
    }

    #[test]
    fn meminfo_without_the_fields_it_needs_is_refused() {
        assert_eq!(
            parse_memory("MemFree: 1000 kB\n"),
            Err(SysmonError::MissingMemoryField("MemTotal"))
        );
        assert_eq!(
            parse_memory("MemTotal: 16000 kB\n"),
            Err(SysmonError::MissingMemoryField("MemAvailable"))
        );
        // A prefix is not a field: `MemTotalSomething` must not answer for it.
        assert_eq!(
            parse_memory("MemTotalish: 1 kB\nMemAvailable: 1 kB\n"),
            Err(SysmonError::MissingMemoryField("MemTotal"))
        );
    }

    #[test]
    fn load_names_the_state_the_panel_paints() {
        assert_eq!(Load::of(0), Load::Normal);
        assert_eq!(Load::of(79), Load::Normal);
        assert_eq!(Load::of(ELEVATED_PERCENT), Load::Elevated);
        assert_eq!(Load::of(89), Load::Elevated);
        assert_eq!(Load::of(CRITICAL_PERCENT), Load::Critical);
        assert_eq!(Load::of(100).as_str(), "critical");
    }

    #[test]
    fn a_percentage_never_leaves_its_range() {
        // A machine whose counters moved backwards, and one with no memory at
        // all: neither may produce a number outside 0..=100.
        assert_eq!(percent_of(5_000, 1_000), 100);
        assert_eq!(percent_of(0, 0), 0);
        assert_eq!(percent_of(1, 3), 33);
    }
}
