//! The thread that reads the machine, and what it hands the window.
//!
//! One thread, one second, one immutable [`Snapshot`]. Every source is read
//! and parsed here, off the Qt thread; the snapshot crosses to Qt by value and
//! is applied whole, so the window never shows one second's CPU beside another
//! second's memory. A source that cannot be read leaves its section
//! [`Section::Unavailable`] with the reason while the others keep publishing.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use hematita_core::cpu::{self, CpuSampler};
use hematita_core::memory::{self, Memory};

/// A number nobody stares at, and rare enough that the monitor is not a reason
/// the machine is busy. The one place the cadence lives.
pub const INTERVAL: Duration = Duration::from_secs(1);

const STAT_PATH: &str = "/proc/stat";
const MEMINFO_PATH: &str = "/proc/meminfo";
const CPUINFO_PATH: &str = "/proc/cpuinfo";
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";

#[derive(Clone, Debug)]
pub enum Section<T> {
    Available(T),
    Unavailable(String),
}

#[derive(Clone, Debug)]
pub struct CpuReading {
    pub aggregate_percent: u8,
    pub frequency_mhz: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub generation: u64,
    /// `None` on the first sample: a rate needs two readings.
    pub cpu: Section<Option<CpuReading>>,
    pub memory: Section<Memory>,
    /// Carried by the first snapshot only. The identity is read from `/proc`
    /// like everything else, so it is read on this thread and travels with the
    /// first sample rather than being read while the object is constructed on
    /// the Qt thread.
    pub identity: Option<Identity>,
}

/// Facts that do not change while the machine is up, read once.
#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub cpu_model: String,
    pub cpu_cores: usize,
}

/// Reads the identity once, tolerating absence.
#[must_use]
pub fn read_identity() -> Identity {
    let cpu_model = read(CPUINFO_PATH)
        .ok()
        .and_then(|text| cpu::parse_model(&text))
        .unwrap_or_default();
    let cpu_cores = read(STAT_PATH)
        .ok()
        .and_then(|text| cpu::parse_stat(&text).ok())
        .map_or(0, |stat| stat.cores.len());
    Identity {
        cpu_model,
        cpu_cores,
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(Path::new(path)).map_err(|error| format!("{path}: {error}"))
}

/// Owns the thread; dropping it asks the thread to stop and waits for it.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Sampler {
    /// Starts sampling; `publish` runs on the sampler thread with each
    /// snapshot and is expected to queue it onto the Qt thread.
    ///
    /// # Errors
    ///
    /// The OS refused to create the thread.
    pub fn spawn(publish: impl Fn(Snapshot) + Send + 'static) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name("hematita-sampler".to_owned())
            .spawn(move || run(&stop_flag, &publish))?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            // The thread checks the flag every hundred milliseconds, so this
            // waits that long at worst; a join that returns an error means it
            // panicked, and there is nothing left to do about that during
            // shutdown.
            let _ = handle.join();
        }
    }
}

fn run(stop: &AtomicBool, publish: &dyn Fn(Snapshot)) {
    let mut cpu_sampler = CpuSampler::new();
    let mut generation = 0u64;
    while !stop.load(Ordering::Relaxed) {
        generation += 1;
        let identity = if generation == 1 {
            Some(read_identity())
        } else {
            None
        };
        publish(Snapshot {
            generation,
            cpu: sample_cpu(&mut cpu_sampler),
            memory: sample_memory(),
            identity,
        });
        // Sleep in short slices so a close does not wait a whole interval.
        let mut slept = Duration::ZERO;
        while slept < INTERVAL && !stop.load(Ordering::Relaxed) {
            let slice = Duration::from_millis(100);
            thread::sleep(slice);
            slept += slice;
        }
    }
}

fn sample_cpu(sampler: &mut CpuSampler) -> Section<Option<CpuReading>> {
    let stat = match read(STAT_PATH)
        .and_then(|text| cpu::parse_stat(&text).map_err(|error| error.to_string()))
    {
        Ok(stat) => stat,
        Err(reason) => {
            // A machine whose counters went away is not a machine at 0 %.
            sampler.reset();
            return Section::Unavailable(reason);
        }
    };
    let sample = match sampler.sample(&stat) {
        Ok(sample) => sample,
        // A hot-plugged core restarts the rate; the next second answers.
        Err(cpu::CpuError::CoreCountChanged { .. }) => None,
        Err(error) => return Section::Unavailable(error.to_string()),
    };
    // Frequency is optional: a VM or a locked governor has no such file.
    let frequency_mhz = read(FREQUENCY_PATH)
        .ok()
        .and_then(|text| cpu::parse_frequency_khz(&text).ok())
        .and_then(|khz| u32::try_from(khz / 1000).ok());
    // The per-core percentages the sampler computes stay in the core crate
    // until H2 draws them: a field nobody reads is dead weight here.
    Section::Available(sample.map(|sample| CpuReading {
        aggregate_percent: sample.aggregate_percent,
        frequency_mhz,
    }))
}

fn sample_memory() -> Section<Memory> {
    match read(MEMINFO_PATH)
        .and_then(|text| memory::parse_meminfo(&text).map_err(|error| error.to_string()))
    {
        Ok(memory) => Section::Available(memory),
        Err(reason) => Section::Unavailable(reason),
    }
}
