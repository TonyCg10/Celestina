//! The thread that reads the machine, and what it hands the window.
//!
//! One thread, one second, one immutable [`Snapshot`]. Every source is read
//! and parsed here, off the Qt thread; the snapshot crosses to Qt by value and
//! is applied whole, so the window never shows one second's CPU beside another
//! second's memory. A source that cannot be read leaves its section
//! [`Section::Unavailable`] with the reason while the others keep publishing.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hematita_core::cpu::{self, CpuSampler};
use hematita_core::disk::{self, SECTOR_BYTES};
use hematita_core::gpu::{self, AmdgpuFiles, GpuReading};
use hematita_core::memory::{self, Memory};
use hematita_core::network;
use hematita_core::rate::NamedCounters;

/// A number nobody stares at, and rare enough that the monitor is not a reason
/// the machine is busy. The one place the cadence lives.
pub const INTERVAL: Duration = Duration::from_secs(1);

const STAT_PATH: &str = "/proc/stat";
const MEMINFO_PATH: &str = "/proc/meminfo";
const CPUINFO_PATH: &str = "/proc/cpuinfo";
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
const DISKSTATS_PATH: &str = "/proc/diskstats";
const NET_DEV_PATH: &str = "/proc/net/dev";
const BLOCK_ROOT: &str = "/sys/block";
const NET_ROOT: &str = "/sys/class/net";
const DRM_ROOT: &str = "/sys/class/drm";

/// Why a section could not be read. The window composes the sentence; this
/// is data, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasonKind {
    Unreadable,
    Malformed,
    NoRate,
}

impl ReasonKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
            Self::NoRate => "no-rate",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
    pub kind: ReasonKind,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum Section<T> {
    Available(T),
    Unavailable(Reason),
}

#[derive(Clone, Debug)]
pub struct CpuReading {
    pub aggregate_percent: u8,
    pub core_percents: Vec<u8>,
    pub frequency_mhz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskInfo {
    pub name: String,
    pub model: String,
    pub size_bytes: u64,
    pub rotational: bool,
}

#[derive(Clone, Debug)]
pub struct DiskSection {
    pub info: DiskInfo,
    /// `[read, write]` bytes per second; `None` until the second reading.
    pub rate: Section<Option<[f64; 2]>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInfo {
    pub name: String,
    pub wireless: bool,
    pub up: bool,
    pub speed_mbit: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct InterfaceSection {
    pub info: InterfaceInfo,
    /// `[rx, tx]` bytes per second; `None` until the second reading.
    pub rate: Section<Option<[f64; 2]>>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub generation: u64,
    pub cpu: Section<Option<CpuReading>>,
    pub memory: Section<Memory>,
    /// `None` when the machine has no `amdgpu` card: absence, not failure.
    pub gpu: Option<Section<GpuReading>>,
    pub disks: Vec<DiskSection>,
    pub interfaces: Vec<InterfaceSection>,
    /// Carried by the first snapshot only.
    pub identity: Option<Identity>,
}

/// Facts that do not change while the machine is up, read once.
#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub cpu_model: String,
    pub cpu_cores: usize,
    /// `vendor:device` of the AMD card, empty without one.
    pub gpu_id: String,
}

fn read(path: &Path) -> Result<String, Reason> {
    std::fs::read_to_string(path).map_err(|_| Reason {
        kind: ReasonKind::Unreadable,
        path: path.display().to_string(),
    })
}

fn malformed(path: &Path) -> Reason {
    Reason {
        kind: ReasonKind::Malformed,
        path: path.display().to_string(),
    }
}

/// The `amdgpu` card's device directory, if any: the first `cardN` whose
/// `device/driver` link ends in `amdgpu`.
fn amdgpu_device() -> Option<PathBuf> {
    let entries = std::fs::read_dir(DRM_ROOT).ok()?;
    let mut cards: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("card") && !name.contains('-'))
        })
        .collect();
    cards.sort();
    cards.into_iter().find_map(|card| {
        let device = card.join("device");
        let target = std::fs::read_link(device.join("driver")).ok()?;
        gpu::is_amdgpu_driver_link(&target.to_string_lossy()).then_some(device)
    })
}

/// Reads the identity once, tolerating absence.
#[must_use]
pub fn read_identity() -> Identity {
    let cpu_model = read(Path::new(CPUINFO_PATH))
        .ok()
        .and_then(|text| cpu::parse_model(&text))
        .unwrap_or_default();
    let cpu_cores = read(Path::new(STAT_PATH))
        .ok()
        .and_then(|text| cpu::parse_stat(&text).ok())
        .map_or(0, |stat| stat.cores.len());
    let gpu_id = amdgpu_device()
        .and_then(|device| {
            let vendor = read(&device.join("vendor")).ok()?;
            let id = read(&device.join("device")).ok()?;
            Some(format!(
                "{}:{}",
                vendor.trim().trim_start_matches("0x"),
                id.trim().trim_start_matches("0x")
            ))
        })
        .unwrap_or_default();
    Identity {
        cpu_model,
        cpu_cores,
        gpu_id,
    }
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
    let mut disk_counters = NamedCounters::<2>::new();
    let mut interface_counters = NamedCounters::<2>::new();
    let gpu_device = amdgpu_device();
    let mut generation = 0u64;
    let mut last = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        generation += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;
        let identity = (generation == 1).then(read_identity);
        publish(Snapshot {
            generation,
            cpu: sample_cpu(&mut cpu_sampler),
            memory: sample_memory(),
            gpu: sample_gpu(gpu_device.as_deref()),
            disks: sample_disks(&mut disk_counters, elapsed),
            interfaces: sample_interfaces(&mut interface_counters, elapsed),
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
    let path = Path::new(STAT_PATH);
    let stat = match read(path) {
        Ok(text) => match cpu::parse_stat(&text) {
            Ok(stat) => stat,
            Err(_) => {
                sampler.reset();
                return Section::Unavailable(malformed(path));
            }
        },
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
        Err(_) => {
            return Section::Unavailable(Reason {
                kind: ReasonKind::NoRate,
                path: STAT_PATH.to_owned(),
            })
        }
    };
    // Frequency is optional: a VM or a locked governor has no such file.
    let frequency_mhz = read(Path::new(FREQUENCY_PATH))
        .ok()
        .and_then(|text| cpu::parse_frequency_khz(&text).ok())
        .and_then(|khz| u32::try_from(khz / 1000).ok());
    Section::Available(sample.map(|sample| CpuReading {
        aggregate_percent: sample.aggregate_percent,
        core_percents: sample.core_percents,
        frequency_mhz,
    }))
}

fn sample_memory() -> Section<Memory> {
    let path = Path::new(MEMINFO_PATH);
    match read(path) {
        Ok(text) => memory::parse_meminfo(&text)
            .map(Section::Available)
            .unwrap_or_else(|_| Section::Unavailable(malformed(path))),
        Err(reason) => Section::Unavailable(reason),
    }
}

/// The eight `amdgpu` sysfs files the reading needs, in contract order.
const AMDGPU_FILES: [&str; 8] = [
    "gpu_busy_percent",
    "mem_busy_percent",
    "mem_info_vram_used",
    "mem_info_vram_total",
    "mem_info_gtt_used",
    "mem_info_gtt_total",
    "pp_dpm_sclk",
    "pp_dpm_mclk",
];

fn sample_gpu(device: Option<&Path>) -> Option<Section<GpuReading>> {
    let device = device?;
    let texts: Result<Vec<String>, Reason> = AMDGPU_FILES
        .iter()
        .map(|name| read(&device.join(name)))
        .collect();
    let texts = match texts {
        Ok(texts) => texts,
        Err(reason) => return Some(Section::Unavailable(reason)),
    };
    let files = AmdgpuFiles {
        busy_percent: &texts[0],
        memory_busy_percent: &texts[1],
        vram_used: &texts[2],
        vram_total: &texts[3],
        gtt_used: &texts[4],
        gtt_total: &texts[5],
        sclk: &texts[6],
        mclk: &texts[7],
    };
    Some(match gpu::parse_amdgpu(&files) {
        Ok(reading) => Section::Available(reading),
        Err(_) => Section::Unavailable(malformed(&device.join(AMDGPU_FILES[0]))),
    })
}

/// Every whole disk with its counters, as `[read, write]` bytes.
fn read_disks() -> Result<Vec<(String, [u64; 2])>, Reason> {
    let path = Path::new(DISKSTATS_PATH);
    let text = read(path)?;
    let stats = disk::parse_diskstats(&text).map_err(|_| malformed(path))?;
    Ok(stats
        .into_iter()
        .map(|stat| {
            (
                stat.name,
                [
                    stat.read_sectors.saturating_mul(SECTOR_BYTES),
                    stat.write_sectors.saturating_mul(SECTOR_BYTES),
                ],
            )
        })
        .collect())
}

fn disk_info(name: &str) -> DiskInfo {
    let root = Path::new(BLOCK_ROOT).join(name);
    let model = read(&root.join("device/model"))
        .map(|text| disk::clean_model(&text))
        .unwrap_or_default();
    let size_bytes = read(&root.join("size"))
        .ok()
        .and_then(|text| disk::parse_size_sectors(&text).ok())
        .map_or(0, |sectors| sectors.saturating_mul(SECTOR_BYTES));
    let rotational = read(&root.join("queue/rotational"))
        .ok()
        .and_then(|text| disk::parse_flag(&text).ok())
        .unwrap_or(false);
    DiskInfo {
        name: name.to_owned(),
        model,
        size_bytes,
        rotational,
    }
}

fn sample_disks(counters: &mut NamedCounters<2>, elapsed: Duration) -> Vec<DiskSection> {
    let readings = match read_disks() {
        Ok(readings) => readings,
        Err(reason) => {
            counters.reset();
            // One unreadable file is every disk unreadable; the list keeps
            // the disks it can still name from sysfs so the rows do not vanish.
            return enumerate_block_devices()
                .into_iter()
                .map(|name| DiskSection {
                    info: disk_info(&name),
                    rate: Section::Unavailable(reason.clone()),
                })
                .collect();
        }
    };
    let rates = counters.sample(&readings, elapsed);
    readings
        .iter()
        .map(|(name, _)| DiskSection {
            info: disk_info(name),
            rate: Section::Available(
                rates
                    .iter()
                    .find(|(rated, _)| rated == name)
                    .map(|(_, rate)| *rate),
            ),
        })
        .collect()
}

fn enumerate_block_devices() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(BLOCK_ROOT)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| disk::is_whole_device(name))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn interface_info(name: &str) -> InterfaceInfo {
    let root = Path::new(NET_ROOT).join(name);
    InterfaceInfo {
        name: name.to_owned(),
        wireless: root.join("wireless").is_dir(),
        up: read(&root.join("operstate")).is_ok_and(|text| network::parse_operstate(&text)),
        speed_mbit: read(&root.join("speed"))
            .ok()
            .and_then(|text| network::parse_speed_mbit(&text)),
    }
}

/// Interfaces whose link type is Ethernet (which Wi-Fi also reports).
fn is_shown_interface(name: &str) -> bool {
    read(&Path::new(NET_ROOT).join(name).join("type"))
        .ok()
        .and_then(|text| network::parse_link_type(&text).ok())
        .is_some_and(|kind| kind == network::ARPHRD_ETHER)
}

fn sample_interfaces(counters: &mut NamedCounters<2>, elapsed: Duration) -> Vec<InterfaceSection> {
    let path = Path::new(NET_DEV_PATH);
    let readings: Vec<(String, [u64; 2])> = match read(path)
        .and_then(|text| network::parse_net_dev(&text).map_err(|_| malformed(path)))
    {
        Ok(stats) => stats
            .into_iter()
            .filter(|stat| is_shown_interface(&stat.name))
            .map(|stat| (stat.name, [stat.rx_bytes, stat.tx_bytes]))
            .collect(),
        Err(reason) => {
            counters.reset();
            return Vec::from([InterfaceSection {
                info: InterfaceInfo {
                    name: String::new(),
                    wireless: false,
                    up: false,
                    speed_mbit: None,
                },
                rate: Section::Unavailable(reason),
            }]);
        }
    };
    let rates = counters.sample(&readings, elapsed);
    readings
        .iter()
        .map(|(name, _)| InterfaceSection {
            info: interface_info(name),
            rate: Section::Available(
                rates
                    .iter()
                    .find(|(rated, _)| rated == name)
                    .map(|(_, rate)| *rate),
            ),
        })
        .collect()
}
