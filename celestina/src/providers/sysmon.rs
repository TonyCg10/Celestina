//! The system monitor worker (CEL-2-K): CPU, memory and temperature for the
//! `sysmon` piece, sampled off the event loop's thread.
//!
//! One named thread behind the [`Worker`] guard reads `/proc/stat`,
//! `/proc/meminfo` and `/sys/class/hwmon` every [`INTERVAL`], parses them
//! with `hematita-core` (the monorepo's one owner of that parsing) and
//! publishes a [`SysReading`] as [`Message::Sysmon`] only when it differs
//! from the last one sent. CPU is a rate, so the first sample of a run
//! publishes nothing. When the counters cannot be read the worker publishes
//! `None` once: a machine whose counters vanished is not at 0 %.
//!
//! Every read is bounded (a size cap per file, a cap on chips and on files
//! per chip) and an unreadable chip is skipped. The three reads sit behind
//! [`ProcSource`], so tests use a fixture instead of the real `/proc`.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_wayland::Sender;
use hematita_core::cpu::{parse_stat, CpuSampler};
use hematita_core::memory::parse_meminfo;
use hematita_core::sensors::{discover, ChannelKind, ChipListing};

use super::Worker;
use crate::message::Message;

/// How often the counters are read.
pub const INTERVAL: Duration = Duration::from_secs(2);
/// How often the wait between samples looks at the shutdown flag.
const POLL: Duration = Duration::from_millis(100);
/// How many samples each history keeps, oldest first.
pub const HISTORY_LEN: usize = 60;
/// The most `/proc/stat` bytes read (the aggregate line comes first).
const MAX_STAT_BYTES: u64 = 256 * 1024;
/// The most `/proc/meminfo` bytes read.
const MAX_MEMINFO_BYTES: u64 = 64 * 1024;
/// The most bytes read from one hwmon file; they hold one short value.
const MAX_HWMON_FILE_BYTES: u64 = 4096;
/// The most hwmon chips looked at.
const MAX_CHIPS: usize = 64;
/// The most files read from one chip.
const MAX_CHIP_FILES: usize = 64;
/// The chips whose temperature stands for the processor.
const CPU_CHIPS: [&str; 4] = ["k10temp", "coretemp", "zenpower", "cpu_thermal"];
/// Temperatures outside this range (in °C) are a driver's sentinel.
const PLAUSIBLE_C: std::ops::RangeInclusive<f64> = -100.0..=200.0;

/// What the monitor knows at one instant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SysReading {
    /// Busy percentage of all cores since the previous sample.
    pub cpu: u8,
    /// Used memory percentage (total minus available).
    pub ram: u8,
    pub ram_used_kib: u64,
    pub ram_total_kib: u64,
    /// The hottest processor temperature in whole °C, when a chip reports one.
    pub temp_c: Option<i16>,
    /// The last [`HISTORY_LEN`] CPU percentages, oldest first.
    pub cpu_history: Vec<u8>,
    /// The last [`HISTORY_LEN`] memory percentages, oldest first.
    pub ram_history: Vec<u8>,
}

/// The three reads a sample needs.
pub trait ProcSource: Send + 'static {
    /// The text of `/proc/stat`.
    ///
    /// # Errors
    ///
    /// The file cannot be read.
    fn read_stat(&mut self) -> io::Result<String>;

    /// The text of `/proc/meminfo`.
    ///
    /// # Errors
    ///
    /// The file cannot be read.
    fn read_meminfo(&mut self) -> io::Result<String>;

    /// The hwmon chips that could be read; an unreadable chip is left out.
    fn hwmon(&mut self) -> Vec<ChipListing>;
}

/// The kernel's own files.
#[derive(Debug)]
pub struct System {
    proc_root: PathBuf,
    hwmon_root: PathBuf,
}

impl System {
    #[must_use]
    pub fn new() -> Self {
        Self::at("/proc", "/sys/class/hwmon")
    }

    fn at(proc_root: impl Into<PathBuf>, hwmon_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
            hwmon_root: hwmon_root.into(),
        }
    }
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads at most `cap` bytes of `path` as text.
fn read_capped(path: &Path, cap: u64) -> io::Result<String> {
    let mut text = String::new();
    File::open(path)?.take(cap).read_to_string(&mut text)?;
    Ok(text)
}

impl ProcSource for System {
    fn read_stat(&mut self) -> io::Result<String> {
        read_capped(&self.proc_root.join("stat"), MAX_STAT_BYTES)
    }

    fn read_meminfo(&mut self) -> io::Result<String> {
        read_capped(&self.proc_root.join("meminfo"), MAX_MEMINFO_BYTES)
    }

    fn hwmon(&mut self) -> Vec<ChipListing> {
        let Ok(entries) = fs::read_dir(&self.hwmon_root) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        names.truncate(MAX_CHIPS);
        names
            .into_iter()
            .filter_map(|key| read_chip(&self.hwmon_root.join(&key), key))
            .collect()
    }
}

/// One chip: its `name`, and its temperature files (the only kind this
/// monitor uses). `None` when the chip has no readable name or directory.
fn read_chip(dir: &Path, key: String) -> Option<ChipListing> {
    let name = read_capped(&dir.join("name"), MAX_HWMON_FILE_BYTES)
        .ok()?
        .trim()
        .to_owned();
    // Only the processor's chips are worth reading further.
    if !CPU_CHIPS.contains(&name.as_str()) {
        return Some(ChipListing {
            key,
            name,
            files: Vec::new(),
        });
    }
    let mut files = Vec::new();
    let mut wanted: Vec<String> = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|file| file.starts_with("temp"))
        .collect();
    wanted.sort();
    wanted.truncate(MAX_CHIP_FILES);
    for file in wanted {
        if let Ok(contents) = read_capped(&dir.join(&file), MAX_HWMON_FILE_BYTES) {
            files.push((file, contents));
        }
    }
    Some(ChipListing { key, name, files })
}

/// The hottest temperature channel of the processor's chips, in whole °C.
#[must_use]
pub fn cpu_temperature(chips: &[ChipListing]) -> Option<i16> {
    chips
        .iter()
        .filter(|chip| CPU_CHIPS.contains(&chip.name.as_str()))
        .flat_map(|chip| discover(chip).channels)
        .filter(|channel| channel.kind == ChannelKind::Temperature)
        .map(|channel| channel.value)
        .filter(|value| PLAUSIBLE_C.contains(value))
        .fold(None, |hottest: Option<f64>, value| {
            Some(hottest.map_or(value, |best| best.max(value)))
        })
        // Within ±200, so the cast cannot truncate.
        .map(|value| value.round() as i16)
}

/// The sampler's state between ticks; no thread, no clock.
#[derive(Debug, Default)]
pub struct Sampler {
    cpu: CpuSampler,
    cpu_history: VecDeque<u8>,
    ram_history: VecDeque<u8>,
    /// What the loop was last told: `None` before the first publish.
    sent: Option<Option<SysReading>>,
}

impl Sampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes one sample and returns what to publish, if anything: `Some(Some)`
    /// a changed reading, `Some(None)` the withdrawal (once), `None` nothing.
    pub fn step(&mut self, source: &mut impl ProcSource) -> Option<Option<SysReading>> {
        match self.read(source) {
            Ok(Some(reading)) => {
                if self.sent.as_ref() == Some(&Some(reading.clone())) {
                    return None;
                }
                self.sent = Some(Some(reading.clone()));
                Some(Some(reading))
            }
            // No rate yet, or a tick with no elapsed time: nothing to say.
            Ok(None) => None,
            Err(why) => {
                tracing::debug!(%why, "sysmon: the counters cannot be read");
                self.cpu.reset();
                self.cpu_history.clear();
                self.ram_history.clear();
                if self.sent == Some(None) {
                    return None;
                }
                self.sent = Some(None);
                Some(None)
            }
        }
    }

    fn read(&mut self, source: &mut impl ProcSource) -> Result<Option<SysReading>, String> {
        let stat = parse_stat(&source.read_stat().map_err(|err| err.to_string())?)
            .map_err(|err| err.to_string())?;
        let memory = parse_meminfo(&source.read_meminfo().map_err(|err| err.to_string())?)
            .map_err(|err| err.to_string())?;
        let Ok(Some(sample)) = self.cpu.sample(&stat) else {
            // The first sample, a tick with no elapsed time, or a changed
            // core count (the sampler restarts from this reading).
            return Ok(None);
        };
        let cpu = sample.aggregate_percent;
        let ram = memory.used_percent();
        push_bounded(&mut self.cpu_history, cpu);
        push_bounded(&mut self.ram_history, ram);
        Ok(Some(SysReading {
            cpu,
            ram,
            ram_used_kib: memory.used_kib,
            ram_total_kib: memory.total_kib,
            temp_c: cpu_temperature(&source.hwmon()),
            cpu_history: self.cpu_history.iter().copied().collect(),
            ram_history: self.ram_history.iter().copied().collect(),
        }))
    }
}

fn push_bounded(history: &mut VecDeque<u8>, value: u8) {
    if history.len() == HISTORY_LEN {
        history.pop_front();
    }
    history.push_back(value);
}

/// Starts the monitor, publishing into the loop through `sender`.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(sender: Sender<Message>, shutdown: &Arc<AtomicBool>) -> io::Result<Worker> {
    spawn_with(System::new(), shutdown, move |reading| {
        sender.send(Message::Sysmon(reading)).is_ok()
    })
}

/// [`spawn`] with its source and its sink given; `publish` returns whether
/// the value was accepted (false ends the worker).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_with(
    source: impl ProcSource,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(Option<SysReading>) -> bool + Send + 'static,
) -> io::Result<Worker> {
    spawn_every(INTERVAL, source, shutdown, publish)
}

fn spawn_every(
    interval: Duration,
    source: impl ProcSource,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(Option<SysReading>) -> bool + Send + 'static,
) -> io::Result<Worker> {
    let flag = Arc::clone(shutdown);
    Worker::spawn("sysmon", shutdown, move || {
        run(source, &flag, interval, publish);
    })
}

fn run(
    mut source: impl ProcSource,
    shutdown: &AtomicBool,
    interval: Duration,
    mut publish: impl FnMut(Option<SysReading>) -> bool,
) {
    let mut sampler = Sampler::new();
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        if let Some(reading) = sampler.step(&mut source) {
            if !publish(reading) {
                tracing::debug!("sysmon: the loop is gone; the worker stops");
                return;
            }
        }
        let wake = Instant::now() + interval;
        while Instant::now() < wake {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(POLL.min(wake.saturating_duration_since(Instant::now())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// A scripted source: `stat` ticks advance by `step` each read.
    struct Fixture {
        busy: u64,
        total: u64,
        step_busy: u64,
        step_total: u64,
        mem_available: u64,
        broken: bool,
        chips: Vec<ChipListing>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                busy: 0,
                total: 1000,
                step_busy: 25,
                step_total: 100,
                mem_available: 8000,
                broken: false,
                chips: Vec::new(),
            }
        }
    }

    impl ProcSource for Fixture {
        fn read_stat(&mut self) -> io::Result<String> {
            if self.broken {
                return Err(io::Error::other("gone"));
            }
            self.busy += self.step_busy;
            self.total += self.step_total;
            let idle = self.total - self.busy;
            Ok(format!("cpu  {} 0 0 {idle} 0 0 0 0 0 0\n", self.busy))
        }

        fn read_meminfo(&mut self) -> io::Result<String> {
            if self.broken {
                return Err(io::Error::other("gone"));
            }
            Ok(format!(
                "MemTotal: 16000 kB\nMemAvailable: {} kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n",
                self.mem_available
            ))
        }

        fn hwmon(&mut self) -> Vec<ChipListing> {
            self.chips.clone()
        }
    }

    fn chip(name: &str, files: &[(&str, &str)]) -> ChipListing {
        ChipListing {
            key: "hwmon0".to_owned(),
            name: name.to_owned(),
            files: files
                .iter()
                .map(|(n, c)| ((*n).to_owned(), (*c).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn the_first_sample_publishes_nothing_and_the_second_does() {
        let mut source = Fixture::new();
        let mut sampler = Sampler::new();
        assert_eq!(sampler.step(&mut source), None);
        let reading = sampler
            .step(&mut source)
            .expect("a publish")
            .expect("a reading");
        assert_eq!(reading.cpu, 25);
        assert_eq!(reading.ram, 50);
        assert_eq!((reading.ram_used_kib, reading.ram_total_kib), (8000, 16000));
        assert_eq!(reading.cpu_history, vec![25]);
        assert_eq!(reading.temp_c, None);
    }

    #[test]
    fn an_unchanged_reading_is_not_published_again() {
        let mut source = Fixture::new();
        let mut sampler = Sampler::new();
        // Fill the history so a constant load stops changing the reading.
        let mut published = 0;
        for _ in 0..(HISTORY_LEN + 5) {
            if sampler.step(&mut source).is_some() {
                published += 1;
            }
        }
        assert_eq!(
            published, HISTORY_LEN,
            "one per sample until the window is full"
        );
        assert_eq!(sampler.step(&mut source), None);
    }

    #[test]
    fn a_failure_withdraws_once_and_the_rate_restarts() {
        let mut source = Fixture::new();
        let mut sampler = Sampler::new();
        sampler.step(&mut source);
        assert!(matches!(sampler.step(&mut source), Some(Some(_))));
        source.broken = true;
        assert_eq!(sampler.step(&mut source), Some(None));
        assert_eq!(sampler.step(&mut source), None);
        source.broken = false;
        // The sampler was reset: the first sample after recovery has no rate.
        assert_eq!(sampler.step(&mut source), None);
        let back = sampler
            .step(&mut source)
            .expect("publish")
            .expect("reading");
        assert_eq!(back.cpu_history.len(), 1, "history restarted");
    }

    #[test]
    fn the_history_keeps_the_last_sixty_samples_oldest_first() {
        let mut source = Fixture::new();
        let mut sampler = Sampler::new();
        sampler.step(&mut source);
        let mut last = None;
        for tick in 0..70u64 {
            source.step_busy = tick.min(100);
            if let Some(Some(reading)) = sampler.step(&mut source) {
                last = Some(reading);
            }
        }
        let reading = last.expect("readings");
        assert_eq!(reading.cpu_history.len(), HISTORY_LEN);
        assert_eq!(reading.cpu_history.first(), Some(&10));
        assert_eq!(reading.cpu_history.last(), Some(&69));
    }

    #[test]
    fn the_hottest_processor_channel_is_chosen_and_other_chips_ignored() {
        let chips = [
            chip("nvme", &[("temp1_input", "99000\n")]),
            chip(
                "k10temp",
                &[
                    ("temp1_input", "45500\n"),
                    ("temp3_input", "61000\n"),
                    ("temp2_input", "-9999999\n"),
                ],
            ),
            chip("coretemp", &[("temp1_input", "52000\n")]),
        ];
        assert_eq!(cpu_temperature(&chips), Some(61));
        assert_eq!(
            cpu_temperature(&[chip("nvme", &[("temp1_input", "50000\n")])]),
            None
        );
        assert_eq!(cpu_temperature(&[]), None);
    }

    #[test]
    fn the_real_source_reads_a_fixture_tree_and_skips_unreadable_chips() {
        let root = std::env::temp_dir().join(format!("celestina-sysmon-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let hwmon = root.join("hwmon");
        fs::create_dir_all(hwmon.join("hwmon0")).expect("dir");
        fs::create_dir_all(hwmon.join("hwmon1")).expect("dir");
        fs::write(hwmon.join("hwmon0/name"), "k10temp\n").expect("write");
        fs::write(hwmon.join("hwmon0/temp1_input"), "70250\n").expect("write");
        // hwmon1 has no `name`: it is skipped.
        fs::write(root.join("stat"), "cpu  1 2 3 4 5\n").expect("write");
        fs::write(root.join("meminfo"), "MemTotal: 1 kB\n").expect("write");
        let mut system = System::at(&root, &hwmon);
        assert_eq!(system.read_stat().expect("stat"), "cpu  1 2 3 4 5\n");
        assert!(system
            .read_meminfo()
            .expect("meminfo")
            .starts_with("MemTotal"));
        let chips = system.hwmon();
        assert_eq!(chips.len(), 1);
        assert_eq!(cpu_temperature(&chips), Some(70));
        let missing = System::at(root.join("none"), root.join("none")).hwmon();
        assert!(missing.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_worker_publishes_and_joins_promptly() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (sent, received) = mpsc::channel();
        let worker = spawn_every(
            Duration::from_millis(10),
            Fixture::new(),
            &shutdown,
            move |r| sent.send(r).is_ok(),
        )
        .expect("the thread starts");
        let first = received
            .recv_timeout(Duration::from_secs(5))
            .expect("a reading");
        assert!(first.is_some());
        let started = Instant::now();
        worker.join();
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_long_interval_still_stops_within_the_poll() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = spawn_with(Fixture::new(), &shutdown, |_| true).expect("starts");
        thread::sleep(Duration::from_millis(50));
        let started = Instant::now();
        worker.join();
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
