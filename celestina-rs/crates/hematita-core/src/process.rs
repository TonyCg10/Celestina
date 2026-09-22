//! One process, as `/proc/PID` describes it.
//!
//! `stat` is the awkward one: the command name sits in parentheses and may
//! itself contain spaces and parentheses, so the line is split at the *last*
//! `)` and everything after it is fields. CPU is a rate between two readings
//! keyed by the process's start time, because PIDs are reused and a new
//! process wearing an old PID must not inherit the old one's ticks.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub ppid: u32,
    /// user + system ticks since the process started.
    pub cpu_ticks: u64,
    /// Ticks after boot when the process started: its identity across PID reuse.
    pub start_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessStatus {
    /// The real uid — the first of the four `Uid:` values.
    pub uid: u32,
    /// `VmRSS` in kibibytes; kernel threads have none.
    pub rss_kib: Option<u64>,
    pub threads: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIo {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// The desktop application a process was launched under, from its systemd
/// scope: `app-<launcher->id-<n>.scope` or `app-id@instance.service`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationScope {
    /// The `.desktop` basename without the suffix, e.g. `com.slack.Slack`.
    pub desktop_id: String,
    /// The whole unit name, e.g. `app-flatpak-com.slack.Slack-1877727323.scope`.
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessError {
    NoCommand { line: String },
    TooFewFields { line: String },
    UnreadableNumber { field: &'static str },
    MissingField(&'static str),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand { line } => {
                write!(formatter, "stat has no parenthesised command: {line}")
            }
            Self::TooFewFields { line } => write!(formatter, "stat line is too short: {line}"),
            Self::UnreadableNumber { field } => write!(formatter, "{field} is not a number"),
            Self::MissingField(field) => write!(formatter, "status has no {field}"),
        }
    }
}

impl std::error::Error for ProcessError {}

/// Parses `/proc/PID/stat`.
///
/// # Errors
///
/// No parenthesised command, too few fields after it, or a non-numeric field.
pub fn parse_stat(text: &str) -> Result<ProcessStat, ProcessError> {
    let line = text.trim_end();
    let open = line.find('(').ok_or_else(|| ProcessError::NoCommand {
        line: line.to_owned(),
    })?;
    let close = line.rfind(')').ok_or_else(|| ProcessError::NoCommand {
        line: line.to_owned(),
    })?;
    if close < open {
        return Err(ProcessError::NoCommand {
            line: line.to_owned(),
        });
    }
    let pid = line[..open]
        .trim()
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "pid" })?;
    let comm = line[open + 1..close].to_owned();
    let fields: Vec<&str> = line[close + 1..].split_whitespace().collect();
    if fields.len() < 20 {
        return Err(ProcessError::TooFewFields {
            line: line.to_owned(),
        });
    }
    let number = |index: usize, field: &'static str| -> Result<u64, ProcessError> {
        fields[index]
            .parse::<u64>()
            .map_err(|_| ProcessError::UnreadableNumber { field })
    };
    let state = fields[0].chars().next().unwrap_or('?');
    let ppid = u32::try_from(number(1, "ppid")?)
        .map_err(|_| ProcessError::UnreadableNumber { field: "ppid" })?;
    let utime = number(11, "utime")?;
    let stime = number(12, "stime")?;
    let start_ticks = number(19, "starttime")?;
    Ok(ProcessStat {
        pid,
        comm,
        state,
        ppid,
        cpu_ticks: utime.saturating_add(stime),
        start_ticks,
    })
}

/// Parses `/proc/PID/status` for the uid, resident size and thread count.
///
/// # Errors
///
/// Missing `Uid:` or `Threads:`, or a non-numeric value.
pub fn parse_status(text: &str) -> Result<ProcessStatus, ProcessError> {
    let field = |name: &'static str| -> Option<&str> {
        text.lines()
            .find_map(|line| {
                line.strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix(':'))
            })
            .map(str::trim)
    };
    let uid = field("Uid")
        .ok_or(ProcessError::MissingField("Uid"))?
        .split_whitespace()
        .next()
        .ok_or(ProcessError::MissingField("Uid"))?
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "Uid" })?;
    let threads = field("Threads")
        .ok_or(ProcessError::MissingField("Threads"))?
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "Threads" })?;
    let rss_kib = match field("VmRSS") {
        Some(value) => Some(
            value
                .split_whitespace()
                .next()
                .and_then(|digits| digits.parse::<u64>().ok())
                .ok_or(ProcessError::UnreadableNumber { field: "VmRSS" })?,
        ),
        None => None,
    };
    Ok(ProcessStatus {
        uid,
        rss_kib,
        threads,
    })
}

/// Splits `/proc/PID/cmdline` on NUL. Lossy on purpose: a command line is
/// shown, never executed.
#[must_use]
pub fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

/// Parses `/proc/PID/io` for the two byte counters that reach the disk.
///
/// # Errors
///
/// Missing `read_bytes:`/`write_bytes:` or a non-numeric value.
pub fn parse_io(text: &str) -> Result<ProcessIo, ProcessError> {
    let field = |name: &'static str| -> Result<u64, ProcessError> {
        text.lines()
            .find_map(|line| {
                line.strip_prefix(name)
                    .and_then(|rest| rest.strip_prefix(':'))
            })
            .ok_or(ProcessError::MissingField(name))?
            .trim()
            .parse::<u64>()
            .map_err(|_| ProcessError::UnreadableNumber { field: name })
    };
    Ok(ProcessIo {
        read_bytes: field("read_bytes")?,
        write_bytes: field("write_bytes")?,
    })
}

/// The application scope of `/proc/PID/cgroup`, if the process runs under
/// one. Recognised shapes:
/// `app-com.anthropic.Claude-893609.scope`, `app-flatpak-com.slack.Slack-1877727323.scope`,
/// `app-blueman@autostart.service`, `app-dbus-:1.3-org.freedesktop.portal@0.service`.
#[must_use]
pub fn parse_cgroup(text: &str) -> Option<ApplicationScope> {
    let path = text.lines().find_map(|line| line.strip_prefix("0::"))?;
    let unit = path.rsplit('/').find(|component| {
        component.starts_with("app-")
            && (component.ends_with(".scope") || component.ends_with(".service"))
    })?;
    let mut id = unit.strip_prefix("app-")?;
    let is_service = id.ends_with(".service");
    id = id
        .strip_suffix(".scope")
        .or_else(|| id.strip_suffix(".service"))?;
    if is_service {
        // `id@instance` and `dbus-:1.3-id@0`
        id = id.split('@').next()?;
        if let Some(rest) = id.strip_prefix("dbus-") {
            id = rest.split_once('-').map_or(rest, |(_, name)| name);
        }
    } else {
        // `[launcher-]id-<digits>`: the trailing number is the instance.
        if let Some((head, tail)) = id.rsplit_once('-') {
            if !tail.is_empty() && tail.bytes().all(|byte| byte.is_ascii_digit()) {
                id = head;
            }
        }
        // A launcher prefix is a plain word before a dotted or plain id:
        // `flatpak-com.slack.Slack`, `niri-kitty`. A reverse-DNS id has no
        // launcher when its first segment already contains a dot.
        if let Some((head, tail)) = id.split_once('-') {
            if !head.contains('.') && !tail.is_empty() {
                id = tail;
            }
        }
    }
    if id.is_empty() {
        return None;
    }
    Some(ApplicationScope {
        desktop_id: id.to_owned(),
        unit: unit.to_owned(),
    })
}

/// Turns successive per-PID tick readings into CPU percentages of the whole
/// machine (every core together, so the column sums to the CPU row).
#[derive(Debug, Default)]
pub struct ProcessSampler {
    previous: HashMap<u32, (u64, u64)>,
}

impl ProcessSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `readings` are `(pid, start_ticks, cpu_ticks)`. A PID seen before with
    /// the same start time is rated; one with another start time is a new
    /// process and starts over; PIDs absent from `readings` are forgotten.
    pub fn sample(
        &mut self,
        readings: &[(u32, u64, u64)],
        elapsed: Duration,
        clock_ticks: u64,
        cores: usize,
    ) -> Vec<(u32, f32)> {
        let capacity = elapsed.as_secs_f64() * clock_ticks as f64 * cores.max(1) as f64;
        let mut rates = Vec::new();
        let mut current = HashMap::with_capacity(readings.len());
        for &(pid, start, ticks) in readings {
            if capacity > 0.0 {
                if let Some(&(previous_start, previous_ticks)) = self.previous.get(&pid) {
                    if previous_start == start {
                        let delta = ticks.saturating_sub(previous_ticks) as f64;
                        let percent = (delta / capacity * 100.0).clamp(0.0, 100.0);
                        rates.push((pid, percent as f32));
                    }
                }
            }
            current.insert(pid, (start, ticks));
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

    const STAT: &str = "1967299 (zsh) S 908913 1967299 1967299 0 -1 4194304 283 887 0 0 5 7 0 0 12 -8 1 0 67467262 11894784 1108 18446744073709551615 0 0 0 0 0 0 2 4 134283265 1 0 0 17 3 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
    const STATUS: &str = "Name:\tzsh\nState:\tS (sleeping)\nUid:\t1000\t1000\t1000\t1000\nVmRSS:\t    4736 kB\nThreads:\t1\n";

    #[test]
    fn stat_splits_at_the_last_parenthesis_and_reads_the_fields_after_it() {
        let stat = parse_stat(STAT).expect("readable stat");
        assert_eq!(stat.pid, 1967299);
        assert_eq!(stat.comm, "zsh");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.ppid, 908913);
        assert_eq!(stat.cpu_ticks, 12);
        assert_eq!(stat.start_ticks, 67467262);
    }

    #[test]
    fn a_command_with_spaces_and_parentheses_does_not_break_the_split() {
        let stat =
            parse_stat("42 (Web Content (x)) R 1 1 1 0 -1 0 0 0 0 0 3 4 0 0 0 0 1 0 100 0 0 0\n")
                .expect("readable stat");
        assert_eq!(stat.comm, "Web Content (x)");
        assert_eq!(stat.cpu_ticks, 7);
        assert_eq!(stat.start_ticks, 100);
    }

    #[test]
    fn a_stat_without_a_command_or_with_too_few_fields_is_refused() {
        assert!(matches!(
            parse_stat("42 zsh S\n"),
            Err(ProcessError::NoCommand { .. })
        ));
        assert!(matches!(
            parse_stat("42 (zsh) S 1 2\n"),
            Err(ProcessError::TooFewFields { .. })
        ));
        assert!(matches!(
            parse_stat("42 (zsh) S x 1 1 0 -1 0 0 0 0 0 3 4 0 0 0 0 1 0 100 0 0 0\n"),
            Err(ProcessError::UnreadableNumber { field: "ppid" })
        ));
    }

    #[test]
    fn status_reads_the_real_uid_the_resident_size_and_the_threads() {
        let status = parse_status(STATUS).expect("readable status");
        assert_eq!(
            status,
            ProcessStatus {
                uid: 1000,
                rss_kib: Some(4736),
                threads: 1
            }
        );
        let kernel =
            parse_status("Name:\tkthreadd\nUid:\t0\t0\t0\t0\nThreads:\t1\n").expect("readable");
        assert_eq!(kernel.rss_kib, None);
        assert_eq!(
            parse_status("Name:\tx\n"),
            Err(ProcessError::MissingField("Uid"))
        );
    }

    #[test]
    fn cmdline_splits_on_nul_and_is_empty_for_a_kernel_thread() {
        assert_eq!(
            parse_cmdline(b"/usr/bin/zsh\0-c\0echo\0"),
            vec!["/usr/bin/zsh", "-c", "echo"]
        );
        assert_eq!(parse_cmdline(b""), Vec::<String>::new());
    }

    #[test]
    fn io_reads_the_two_disk_counters() {
        let io = parse_io("rchar: 1\nwchar: 2\nread_bytes: 4096\nwrite_bytes: 8192\n")
            .expect("readable io");
        assert_eq!(
            io,
            ProcessIo {
                read_bytes: 4096,
                write_bytes: 8192
            }
        );
        assert_eq!(
            parse_io("rchar: 1\n"),
            Err(ProcessError::MissingField("read_bytes"))
        );
    }

    #[test]
    fn cgroup_names_the_application_behind_every_scope_shape() {
        let prefix = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/";
        let scope = |unit: &str| parse_cgroup(&format!("{prefix}{unit}\n"));
        assert_eq!(
            scope("app-com.anthropic.Claude-893609.scope").map(|s| s.desktop_id),
            Some("com.anthropic.Claude".to_owned())
        );
        assert_eq!(
            scope("app-flatpak-com.slack.Slack-1877727323.scope").map(|s| s.desktop_id),
            Some("com.slack.Slack".to_owned())
        );
        assert_eq!(
            scope("app-blueman@autostart.service").map(|s| s.desktop_id),
            Some("blueman".to_owned())
        );
        assert_eq!(
            scope("app-dbus-:1.3-org.freedesktop.impl.portal.desktop.celestina@0.service")
                .map(|s| s.desktop_id),
            Some("org.freedesktop.impl.portal.desktop.celestina".to_owned())
        );
        assert_eq!(
            scope("app-niri-kitty-4242.scope").map(|s| s.desktop_id),
            Some("kitty".to_owned())
        );
        let unit = scope("app-com.anthropic.Claude-893609.scope").expect("a scope");
        assert_eq!(unit.unit, "app-com.anthropic.Claude-893609.scope");
        assert_eq!(
            parse_cgroup(
                "0::/user.slice/user-1000.slice/user@1000.service/session.slice/pipewire.service\n"
            ),
            None
        );
        assert_eq!(parse_cgroup("0::/system.slice/sshd.service\n"), None);
        assert_eq!(parse_cgroup(""), None);
    }

    #[test]
    fn a_reverse_dns_scope_without_an_instance_number_keeps_its_whole_id() {
        let prefix = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/";
        assert_eq!(
            parse_cgroup(&format!("{prefix}app-org.example.Tool.scope\n")).map(|s| s.desktop_id),
            Some("org.example.Tool".to_owned())
        );
        assert_eq!(
            parse_cgroup(&format!("{prefix}app-gnome-org.example.Tool-77.scope\n"))
                .map(|s| s.desktop_id),
            Some("org.example.Tool".to_owned())
        );
    }

    #[test]
    fn cpu_percent_is_the_share_of_the_whole_machine_between_two_readings() {
        let mut sampler = ProcessSampler::new();
        assert!(sampler
            .sample(&[(7, 100, 50)], Duration::from_secs(1), 100, 8)
            .is_empty());
        // 50 more ticks in one second at 100 ticks/s on 8 cores: 50 / 800.
        let rates = sampler.sample(&[(7, 100, 100)], Duration::from_secs(1), 100, 8);
        assert_eq!(rates, vec![(7, 6.25)]);
    }

    #[test]
    fn a_reused_pid_with_another_start_time_starts_over() {
        let mut sampler = ProcessSampler::new();
        sampler.sample(&[(7, 100, 50)], Duration::from_secs(1), 100, 1);
        assert!(sampler
            .sample(&[(7, 900, 10)], Duration::from_secs(1), 100, 1)
            .is_empty());
        let rates = sampler.sample(&[(7, 900, 60)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 50.0)]);
    }

    #[test]
    fn degenerate_inputs_rate_nothing_or_saturate() {
        let mut sampler = ProcessSampler::new();
        sampler.sample(&[(7, 1, 0)], Duration::from_secs(1), 100, 1);
        assert!(sampler
            .sample(&[(7, 1, 10)], Duration::ZERO, 100, 1)
            .is_empty());
        assert!(sampler
            .sample(&[(7, 1, 20)], Duration::from_secs(1), 0, 1)
            .is_empty());
        let rates = sampler.sample(&[(7, 1, 20_000)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 100.0)]);
        // Backwards ticks (should not happen) rate zero rather than wrapping.
        let rates = sampler.sample(&[(7, 1, 5)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 0.0)]);
    }
}
