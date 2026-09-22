//! CPU, as `/proc/stat` and `cpufreq` report it.
//!
//! CPU is a rate, not a reading: `/proc/stat` counts ticks since boot, so a
//! percentage only exists between two samples. [`CpuSampler`] holds the
//! previous reading and refuses to invent a number for the first.

use std::fmt;

use crate::ratio::percent_of;

/// One reading of a `cpu` line: ticks spent idle, and ticks spent at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTicks {
    pub idle: u64,
    pub total: u64,
}

/// The aggregate line and every `cpuN` line, in kernel order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuStat {
    pub aggregate: CpuTicks,
    pub cores: Vec<CpuTicks>,
}

/// Busy percentages between two readings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuSample {
    pub aggregate_percent: u8,
    pub core_percents: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CpuError {
    NoAggregateLine,
    TooFewFields {
        line: String,
    },
    UnreadableNumber {
        line: String,
    },
    /// Two samples that are not apart in time say nothing about a rate.
    NoElapsedTime,
    /// The kernel hot-plugged a core between two readings; the rate restarts.
    CoreCountChanged {
        before: usize,
        after: usize,
    },
}

impl fmt::Display for CpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAggregateLine => write!(formatter, "/proc/stat has no aggregate cpu line"),
            Self::TooFewFields { line } => {
                write!(formatter, "/proc/stat line is too short: {line}")
            }
            Self::UnreadableNumber { line } => {
                write!(formatter, "/proc/stat carries an unreadable number: {line}")
            }
            Self::NoElapsedTime => write!(formatter, "two cpu samples with no time between them"),
            Self::CoreCountChanged { before, after } => {
                write!(
                    formatter,
                    "core count changed from {before} to {after} between samples"
                )
            }
        }
    }
}

impl std::error::Error for CpuError {}

/// Parses the aggregate `cpu` line and every `cpuN` line of `/proc/stat`.
///
/// # Errors
///
/// Refuses a file with no aggregate line, a line too short to carry idle and
/// iowait, or any field that is not a number.
pub fn parse_stat(stat: &str) -> Result<CpuStat, CpuError> {
    let mut aggregate = None;
    let mut cores = Vec::new();
    for line in stat.lines() {
        if let Some(rest) = line.strip_prefix("cpu") {
            let is_aggregate = rest.starts_with(' ');
            let is_core = rest.starts_with(|c: char| c.is_ascii_digit());
            if !is_aggregate && !is_core {
                continue;
            }
            let ticks = parse_ticks(line)?;
            if is_aggregate {
                aggregate = Some(ticks);
            } else {
                cores.push(ticks);
            }
        }
    }
    let aggregate = aggregate.ok_or(CpuError::NoAggregateLine)?;
    Ok(CpuStat { aggregate, cores })
}

/// user, nice, system, idle, iowait — everything after is optional and still
/// counts toward the total, which keeps this correct on a kernel that adds a
/// column.
fn parse_ticks(line: &str) -> Result<CpuTicks, CpuError> {
    let mut ticks = Vec::with_capacity(10);
    for field in line.split_whitespace().skip(1) {
        let value = field
            .parse::<u64>()
            .map_err(|_| CpuError::UnreadableNumber {
                line: line.to_owned(),
            })?;
        ticks.push(value);
    }
    if ticks.len() < 5 {
        return Err(CpuError::TooFewFields {
            line: line.to_owned(),
        });
    }
    Ok(CpuTicks {
        idle: ticks[3].saturating_add(ticks[4]),
        total: ticks
            .iter()
            .fold(0u64, |sum, tick| sum.saturating_add(*tick)),
    })
}

fn busy_percent(previous: CpuTicks, current: CpuTicks) -> Option<u8> {
    let total = current.total.saturating_sub(previous.total);
    if total == 0 {
        return None;
    }
    let idle = current.idle.saturating_sub(previous.idle).min(total);
    Some(percent_of(total - idle, total))
}

/// Turns successive `/proc/stat` readings into busy percentages.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<CpuStat>,
}

impl CpuSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The busy percentages since the previous sample, or `None` for the
    /// first one.
    ///
    /// # Errors
    ///
    /// [`CpuError::NoElapsedTime`] when no aggregate tick passed;
    /// [`CpuError::CoreCountChanged`] when the core list changed length, in
    /// which case the new reading becomes the baseline.
    pub fn sample(&mut self, stat: &CpuStat) -> Result<Option<CpuSample>, CpuError> {
        let Some(previous) = self.previous.replace(stat.clone()) else {
            return Ok(None);
        };
        if previous.cores.len() != stat.cores.len() {
            return Err(CpuError::CoreCountChanged {
                before: previous.cores.len(),
                after: stat.cores.len(),
            });
        }
        let aggregate_percent =
            busy_percent(previous.aggregate, stat.aggregate).ok_or(CpuError::NoElapsedTime)?;
        // A core with no ticks between samples is a core that did nothing.
        let core_percents = previous
            .cores
            .iter()
            .zip(&stat.cores)
            .map(|(before, after)| busy_percent(*before, *after).unwrap_or(0))
            .collect();
        Ok(Some(CpuSample {
            aggregate_percent,
            core_percents,
        }))
    }

    /// Forgets the previous reading, so the next sample starts a fresh rate.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

/// Parses a `cpufreq/scaling_cur_freq` file: one integer in kHz.
///
/// # Errors
///
/// [`CpuError::UnreadableNumber`] when the text is not one integer.
pub fn parse_frequency_khz(text: &str) -> Result<u64, CpuError> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| CpuError::UnreadableNumber {
            line: text.trim().to_owned(),
        })
}

/// The first `model name` of `/proc/cpuinfo`, trimmed; `None` when absent.
#[must_use]
pub fn parse_model(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name"))
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, model)| model.trim().to_owned())
        .filter(|model| !model.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  100 20 50 800 30 0 0 0 0 0\ncpu0 50 10 25 400 15 0 0 0 0 0\ncpu1 50 10 25 400 15 0 0 0 0 0\nintr 1\nctxt 2\n";

    fn stat(aggregate: (u64, u64), cores: &[(u64, u64)]) -> CpuStat {
        CpuStat {
            aggregate: CpuTicks {
                idle: aggregate.0,
                total: aggregate.1,
            },
            cores: cores
                .iter()
                .map(|&(idle, total)| CpuTicks { idle, total })
                .collect(),
        }
    }

    #[test]
    fn the_aggregate_line_counts_idle_and_iowait_as_idle() {
        let parsed = parse_stat(STAT).expect("a readable stat");
        assert_eq!(
            parsed.aggregate,
            CpuTicks {
                idle: 830,
                total: 1000
            }
        );
        assert_eq!(parsed.cores.len(), 2);
        assert_eq!(
            parsed.cores[1],
            CpuTicks {
                idle: 415,
                total: 500
            }
        );
    }

    #[test]
    fn an_unreadable_stat_is_refused_rather_than_guessed() {
        assert_eq!(parse_stat("intr 1\n"), Err(CpuError::NoAggregateLine));
        assert_eq!(
            parse_stat("cpu  1 2 3\n"),
            Err(CpuError::TooFewFields {
                line: "cpu  1 2 3".to_owned()
            })
        );
        assert_eq!(
            parse_stat("cpu  1 2 3 four 5\n"),
            Err(CpuError::UnreadableNumber {
                line: "cpu  1 2 3 four 5".to_owned()
            })
        );
        // A broken core line is refused too; the aggregate does not excuse it.
        assert!(matches!(
            parse_stat("cpu  1 2 3 4 5\ncpu0 1 2\n"),
            Err(CpuError::TooFewFields { .. })
        ));
    }

    #[test]
    fn the_first_sample_reports_nothing_because_a_rate_needs_two() {
        let mut sampler = CpuSampler::new();
        assert_eq!(
            sampler.sample(&stat((830, 1000), &[(415, 500), (415, 500)])),
            Ok(None)
        );
        // Half the new aggregate ticks were busy; core 0 was fully busy, core 1 idle.
        let sample = sampler
            .sample(&stat((880, 1100), &[(415, 550), (465, 550)]))
            .expect("second sample")
            .expect("a rate");
        assert_eq!(sample.aggregate_percent, 50);
        assert_eq!(sample.core_percents, vec![100, 0]);
    }

    #[test]
    fn two_samples_with_no_time_between_them_report_no_rate() {
        let mut sampler = CpuSampler::new();
        let reading = stat((830, 1000), &[(415, 500)]);
        sampler.sample(&reading).expect("first sample");
        assert_eq!(sampler.sample(&reading), Err(CpuError::NoElapsedTime));
    }

    #[test]
    fn a_changed_core_count_restarts_the_rate_from_the_new_reading() {
        let mut sampler = CpuSampler::new();
        sampler
            .sample(&stat((830, 1000), &[(415, 500)]))
            .expect("first");
        assert_eq!(
            sampler.sample(&stat((880, 1100), &[(415, 550), (465, 550)])),
            Err(CpuError::CoreCountChanged {
                before: 1,
                after: 2
            })
        );
        // The next reading measures against the two-core one, not the old.
        let sample = sampler
            .sample(&stat((930, 1200), &[(415, 600), (515, 600)]))
            .expect("third")
            .expect("a rate");
        assert_eq!(sample.core_percents, vec![100, 0]);
    }

    #[test]
    fn counters_that_go_backwards_saturate_instead_of_wrapping() {
        let mut sampler = CpuSampler::new();
        sampler.sample(&stat((830, 1000), &[])).expect("first");
        // idle moved backwards; busy cannot exceed the total.
        let sample = sampler
            .sample(&stat((800, 1100), &[]))
            .expect("second")
            .expect("rate");
        assert_eq!(sample.aggregate_percent, 100);
    }

    #[test]
    fn a_reset_sampler_measures_from_the_next_reading_only() {
        let mut sampler = CpuSampler::new();
        sampler.sample(&stat((830, 1000), &[])).expect("first");
        sampler.reset();
        assert_eq!(sampler.sample(&stat((880, 1100), &[])), Ok(None));
    }

    #[test]
    fn frequency_is_one_integer_in_khz() {
        assert_eq!(parse_frequency_khz("4658104\n"), Ok(4_658_104));
        assert!(matches!(
            parse_frequency_khz("fast\n"),
            Err(CpuError::UnreadableNumber { .. })
        ));
        assert!(matches!(
            parse_frequency_khz(""),
            Err(CpuError::UnreadableNumber { .. })
        ));
    }

    #[test]
    fn the_model_is_the_first_model_name_trimmed() {
        let cpuinfo = "processor\t: 0\nmodel name\t: AMD Ryzen 7 9800X3D 8-Core Processor\nprocessor\t: 1\nmodel name\t: other\n";
        assert_eq!(
            parse_model(cpuinfo).as_deref(),
            Some("AMD Ryzen 7 9800X3D 8-Core Processor")
        );
        assert_eq!(parse_model("processor : 0\n"), None);
    }
}
