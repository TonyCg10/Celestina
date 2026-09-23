//! The thread that reads the machine, and what it hands the window.
//!
//! One thread, one second, one immutable [`Snapshot`]. Every source is read
//! and parsed here, off the Qt thread; the snapshot crosses to Qt by value and
//! is applied whole, so the window never shows one second's CPU beside another
//! second's memory. A source that cannot be read leaves its section
//! [`Section::Unavailable`] with the reason while the others keep publishing.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hematita_core::cpu::{self, CpuSampler};
use hematita_core::disk::{self, SECTOR_BYTES};
use hematita_core::gpu::{self, AmdgpuFiles, GpuReading};
use hematita_core::memory::{self, Memory};
use hematita_core::network;
use hematita_core::passwd;
use hematita_core::process::{self, ProcessSampler};
use hematita_core::rate::NamedCounters;
use hematita_core::sensors::{self, Chip, ChipListing};
use hematita_core::services::{Scope, Unit};

/// A number nobody stares at, and rare enough that the monitor is not a reason
/// the machine is busy. The one place the cadence lives.
pub const INTERVAL: Duration = Duration::from_secs(1);

/// Processes are read every second tick: two thousand PIDs are two thousand
/// directories, and a table nobody reads at a glance does not need the
/// cadence a graph does.
pub const PROCESS_TICKS: u64 = 2;

/// Units change rarely; every fifth tick is live enough for a page and cheap
/// enough for two bus round-trips carrying a few hundred rows.
pub const SERVICE_TICKS: u64 = 5;

/// A chip's labels and limits are read once and then re-read every thirtieth
/// tick: a driver that gains a channel while the window is up — a card that
/// binds, a module that loads — is shown within half a minute instead of never.
pub const SENSOR_FACTS_TICKS: u64 = 30;

const STAT_PATH: &str = "/proc/stat";
const MEMINFO_PATH: &str = "/proc/meminfo";
const CPUINFO_PATH: &str = "/proc/cpuinfo";
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
const DISKSTATS_PATH: &str = "/proc/diskstats";
const NET_DEV_PATH: &str = "/proc/net/dev";
const BLOCK_ROOT: &str = "/sys/block";
const NET_ROOT: &str = "/sys/class/net";
const DRM_ROOT: &str = "/sys/class/drm";
const PROC_ROOT: &str = "/proc";
const PASSWD_PATH: &str = "/etc/passwd";
const HWMON_ROOT: &str = "/sys/class/hwmon";
const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const SYSTEMD_OBJECT: &str = "/org/freedesktop/systemd1";
const SYSTEMD_MANAGER: &str = "org.freedesktop.systemd1.Manager";
/// Not a path: what the page names when a bus, rather than a file, is what
/// could not be read.
const SYSTEM_BUS: &str = "system bus";
const SESSION_BUS: &str = "session bus";

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
pub struct ProcessReading {
    pub pid: u32,
    /// Ticks after boot when the process started. With the PID this is the
    /// process's identity across PID reuse, and it is what the signal path
    /// re-checks before it acts.
    pub start_ticks: u64,
    pub name: String,
    pub uid: u32,
    /// `None` until the second reading of this PID.
    pub cpu_percent: Option<f32>,
    pub memory_kib: u64,
    /// `[read, write]` bytes per second; `None` for another user's process
    /// (the kernel refuses `io`) or before the second reading.
    pub io_rate: Option<[f64; 2]>,
    pub application: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProcessSnapshot {
    pub own_uid: u32,
    pub readings: Vec<ProcessReading>,
    /// Read once when the thread starts and shared by every snapshot: a user
    /// created during a session is rare, and shows as a number.
    pub users: Arc<HashMap<u32, String>>,
}

/// What does not change while a process lives, read once per (pid, start).
#[derive(Clone, Debug)]
struct ProcessFacts {
    start_ticks: u64,
    name: String,
    uid: u32,
    application: Option<String>,
}

struct ProcessState {
    sampler: ProcessSampler,
    io: NamedCounters<2>,
    facts: HashMap<u32, ProcessFacts>,
    users: Arc<HashMap<u32, String>>,
    own_uid: u32,
    clock_ticks: u64,
}

impl ProcessState {
    fn new() -> Self {
        let users = read(Path::new(PASSWD_PATH))
            .map(|text| passwd::parse(&text))
            .unwrap_or_default();
        Self {
            sampler: ProcessSampler::new(),
            io: NamedCounters::new(),
            facts: HashMap::new(),
            users: Arc::new(users),
            own_uid: rustix::process::getuid().as_raw(),
            clock_ticks: rustix::param::clock_ticks_per_second(),
        }
    }
}

/// The two buses' unit lists. Each bus is its own section: the system bus
/// answering while the session bus does not is a page with half its rows and
/// one sentence, not a failure.
#[derive(Clone, Debug)]
pub struct ServiceSnapshot {
    pub system: Section<Vec<Unit>>,
    pub user: Section<Vec<Unit>>,
}

#[derive(Clone, Debug)]
pub struct SensorSnapshot {
    pub chips: Vec<Chip>,
}

/// What a chip directory holds that does not change: its name, which files
/// exist, its labels and limits. Read once per directory; only the `_input`
/// and `_average` files are read again each tick.
struct ChipFacts {
    name: String,
    /// Every recognised channel file except the value files.
    statics: Vec<(String, String)>,
    /// The value files, read each tick.
    value_files: Vec<String>,
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
    /// `None` on the ticks that did not read processes.
    pub processes: Option<Section<ProcessSnapshot>>,
    pub sensors: Section<SensorSnapshot>,
    /// `None` on the ticks that did not ask the buses.
    pub services: Option<ServiceSnapshot>,
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

type Subscriber = Box<dyn Fn(&Snapshot) + Send + 'static>;

/// The one sampling thread and everyone listening to it. Two hub objects read
/// the same machine, and one thread reading `/proc` is the whole point: the
/// singleton is what keeps a second window-side object from spawning a second
/// reader.
struct Hub {
    subscribers: Mutex<Vec<Subscriber>>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

static HUB: OnceLock<Hub> = OnceLock::new();

fn hub() -> &'static Hub {
    HUB.get_or_init(|| Hub {
        subscribers: Mutex::new(Vec::new()),
        stop: Arc::new(AtomicBool::new(false)),
        handle: Mutex::new(None),
    })
}

/// Registers a subscriber and starts the thread if it is not running.
///
/// # Errors
///
/// The OS refused to create the thread, or one of the hub's locks was
/// poisoned — in which case the subscriber is not registered and the caller
/// is told, rather than being left listening to nothing.
pub fn subscribe(callback: impl Fn(&Snapshot) + Send + 'static) -> std::io::Result<()> {
    let shared = hub();
    // The `handle` lock is taken first and held across the registration, so a
    // `subscribe` racing a [`stop`] either registers before that `stop` drains
    // the list — and is dropped with it — or waits and registers on a hub that
    // is already stopped and armed again. Registering outside this lock could
    // have put a subscriber into a list `stop` was about to clear while the
    // caller believed it was listening.
    let mut handle = shared
        .handle
        .lock()
        .map_err(|_| std::io::Error::other("sampler lock poisoned"))?;
    shared
        .subscribers
        .lock()
        .map_err(|_| std::io::Error::other("sampler subscribers lock poisoned"))?
        .push(Box::new(callback));
    if handle.is_none() {
        let stop = Arc::clone(&shared.stop);
        *handle = Some(
            thread::Builder::new()
                .name("hematita-sampler".to_owned())
                .spawn(move || {
                    run(&stop, &|snapshot| {
                        if let Ok(subscribers) = hub().subscribers.lock() {
                            for subscriber in subscribers.iter() {
                                subscriber(&snapshot);
                            }
                        }
                    });
                })?,
        );
    }
    Ok(())
}

/// Asks the thread to stop, waits for it, and leaves the hub able to start
/// again: the subscribers of the run that just ended are dropped, and the
/// flag is lowered so a later [`subscribe`] spawns a thread that lives.
///
/// The thread checks the flag every hundred milliseconds, so this waits that
/// long at worst; a join that returns an error means it panicked, and there is
/// nothing left to do about that during shutdown. A second call is a no-op.
///
/// Because [`subscribe`] now registers inside the `handle` lock this holds,
/// the guarantee is total: no subscriber can be registered between the drain
/// below and the lowering of the flag, so the hub a later `subscribe` finds is
/// either running or stopped-and-empty, never a drained hub holding a
/// subscriber that will never be called.
pub fn stop() {
    let shared = hub();
    shared.stop.store(true, Ordering::Relaxed);
    // The `handle` lock is held across the whole sequence, so a `subscribe`
    // racing this either registers before the flag is read or waits and finds
    // a hub that is stopped, drained and armed again — never one that is
    // half-way between.
    if let Ok(mut handle) = shared.handle.lock() {
        if let Some(handle) = handle.take() {
            let _ = handle.join();
        }
        // The thread is gone, so nothing will call these again; keeping them
        // would hold every closure's captured `CxxQtThread` for the life of
        // the process.
        if let Ok(mut subscribers) = shared.subscribers.lock() {
            subscribers.clear();
        }
        shared.stop.store(false, Ordering::Relaxed);
    }
}

fn run(stop: &AtomicBool, publish: &dyn Fn(Snapshot)) {
    let mut cpu_sampler = CpuSampler::new();
    let mut disk_counters = NamedCounters::<2>::new();
    let mut interface_counters = NamedCounters::<2>::new();
    // Facts that sysfs will not change while the name exists, read once per
    // name instead of once per second. A name that leaves takes its entry.
    let mut disk_facts: HashMap<String, DiskInfo> = HashMap::new();
    // Per interface: whether its link type is shown, and whether it is
    // wireless. `operstate` and `speed` do change, so they stay per tick.
    let mut interface_facts: HashMap<String, (bool, bool)> = HashMap::new();
    let gpu_device = amdgpu_device();
    let mut processes = ProcessState::new();
    // Processes are read every other tick, so their rates are measured
    // against the previous *process* read rather than the previous tick.
    let mut last_process = Instant::now();
    // The divisor of a process's CPU share: this second's core count, which
    // the CPU reading answers. One core until it has.
    let mut core_count = 1usize;
    let mut sensor_facts: HashMap<String, ChipFacts> = HashMap::new();
    // The two bus connections, opened on the first service tick and kept: a
    // bus that could not be opened is retried on the next service tick, and a
    // connection that lives is not re-established every five seconds.
    let mut system_bus: Option<zbus::blocking::Connection> = None;
    let mut session_bus: Option<zbus::blocking::Connection> = None;
    let mut generation = 0u64;
    let mut last = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        generation += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;
        let identity = (generation == 1).then(read_identity);
        let cpu = sample_cpu(&mut cpu_sampler);
        if let Section::Available(Some(reading)) = &cpu {
            if !reading.core_percents.is_empty() {
                core_count = reading.core_percents.len();
            }
        }
        let process_section = if generation % PROCESS_TICKS == 0 {
            let now = Instant::now();
            let since = now.duration_since(last_process);
            last_process = now;
            Some(sample_processes(&mut processes, since, core_count))
        } else {
            None
        };
        // The first tick asks too, so the page fills at once instead of after
        // five seconds of nothing.
        let service_section = if generation == 1 || generation % SERVICE_TICKS == 0 {
            Some(sample_services(&mut system_bus, &mut session_bus))
        } else {
            None
        };
        publish(Snapshot {
            generation,
            cpu,
            memory: sample_memory(),
            gpu: sample_gpu(gpu_device.as_deref()),
            disks: sample_disks(&mut disk_counters, &mut disk_facts, elapsed),
            interfaces: sample_interfaces(&mut interface_counters, &mut interface_facts, elapsed),
            processes: process_section,
            sensors: sample_sensors(&mut sensor_facts, generation),
            services: service_section,
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

fn chip_facts(dir: &Path) -> Option<ChipFacts> {
    let name = read(&dir.join("name")).ok()?.trim().to_owned();
    let mut statics = Vec::new();
    let mut value_files = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        let Ok(file) = entry.file_name().into_string() else {
            continue;
        };
        let Some((_, _, attribute)) = sensors::parse_channel_file(&file) else {
            continue;
        };
        match attribute {
            "input" | "average" => value_files.push(file),
            "label" | "max" | "crit" | "cap" => {
                if let Ok(contents) = read(&entry.path()) {
                    statics.push((file, contents));
                }
            }
            _ => {}
        }
    }
    value_files.sort();
    Some(ChipFacts {
        name,
        statics,
        value_files,
    })
}

/// One `ListUnits` row: name, description, load state, active state, sub
/// state, the unit it follows, its object path, and the job triple. Ten
/// fields, `a(ssssssouso)`, exactly as systemd declares them.
type UnitRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    zbus::zvariant::OwnedObjectPath,
    u32,
    String,
    zbus::zvariant::OwnedObjectPath,
);

fn bus_unavailable(label: &str) -> Reason {
    Reason {
        kind: ReasonKind::Unreadable,
        path: label.to_owned(),
    }
}

/// Every unit one manager has loaded. A bus that answers something other than
/// the declared shape is `Malformed`, which is a different sentence from a bus
/// that is not there.
fn list_units(
    connection: &zbus::blocking::Connection,
    scope: Scope,
    bus_label: &str,
) -> Section<Vec<Unit>> {
    let proxy = match zbus::blocking::Proxy::new(
        connection,
        SYSTEMD_SERVICE,
        SYSTEMD_OBJECT,
        SYSTEMD_MANAGER,
    ) {
        Ok(proxy) => proxy,
        Err(_) => return Section::Unavailable(bus_unavailable(bus_label)),
    };
    match proxy.call::<_, _, Vec<UnitRow>>("ListUnits", &()) {
        Ok(rows) => Section::Available(
            rows.into_iter()
                .map(|(name, description, _load, active, sub, ..)| Unit {
                    name,
                    description,
                    scope,
                    active,
                    sub,
                })
                .collect(),
        ),
        Err(_) => Section::Unavailable(Reason {
            kind: ReasonKind::Malformed,
            path: bus_label.to_owned(),
        }),
    }
}

/// Both managers' unit lists, on this thread. This is the only place the
/// sampler talks to a bus, and it does it every [`SERVICE_TICKS`] ticks.
fn sample_services(
    system: &mut Option<zbus::blocking::Connection>,
    user: &mut Option<zbus::blocking::Connection>,
) -> ServiceSnapshot {
    if system.is_none() {
        *system = zbus::blocking::Connection::system().ok();
    }
    if user.is_none() {
        *user = zbus::blocking::Connection::session().ok();
    }
    ServiceSnapshot {
        system: system.as_ref().map_or_else(
            || Section::Unavailable(bus_unavailable(SYSTEM_BUS)),
            |connection| list_units(connection, Scope::System, SYSTEM_BUS),
        ),
        user: user.as_ref().map_or_else(
            || Section::Unavailable(bus_unavailable(SESSION_BUS)),
            |connection| list_units(connection, Scope::User, SESSION_BUS),
        ),
    }
}

/// Every `hwmonN` directory as a chip. The labels and limits are read once per
/// directory; the value files and the chip's `name` are read each tick, which
/// is what a live reading is. A chip whose value file vanishes keeps its other
/// channels; a chip directory that vanishes drops out of `facts` and the list.
///
/// The `hwmonN` index is not an identity: a device that goes away frees its
/// index and the next device to bind can be given it. So the cache is keyed on
/// the index but validated by the chip's own `name` every tick — one small
/// read per chip — and a name that changed throws the cached labels and limits
/// away rather than letting a new device inherit the old one's `crit`.
fn sample_sensors(
    facts: &mut HashMap<String, ChipFacts>,
    generation: u64,
) -> Section<SensorSnapshot> {
    // Every thirtieth tick the cached labels and limits are thrown away and
    // read again, so a chip that gained a channel is enumerated rather than
    // frozen at whatever it published when the window opened.
    let re_enumerate = generation % SENSOR_FACTS_TICKS == 0;
    let root = Path::new(HWMON_ROOT);
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            return Section::Unavailable(Reason {
                kind: ReasonKind::Unreadable,
                path: HWMON_ROOT.to_owned(),
            })
        }
    };
    let mut keys: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .into_string()
                .ok()
                .map(|key| (key, entry.path()))
        })
        .filter(|(key, _)| key.starts_with("hwmon"))
        .collect();
    keys.sort();
    facts.retain(|key, _| keys.iter().any(|(k, _)| k == key));
    let mut chips = Vec::new();
    for (key, dir) in keys {
        // The one fact that says whether the cache is still about this device.
        let name = read(&dir.join("name")).map(|text| text.trim().to_owned());
        if let (Ok(name), Some(cached)) = (&name, facts.get(&key)) {
            if &cached.name != name {
                facts.remove(&key);
            }
        }
        if re_enumerate {
            facts.remove(&key);
        }
        let Some(chip_facts) = (match facts.get(&key) {
            Some(existing) => Some(existing),
            None => chip_facts(&dir).and_then(|f| {
                facts.insert(key.clone(), f);
                facts.get(&key)
            }),
        }) else {
            continue;
        };
        let mut files = chip_facts.statics.clone();
        for value_file in &chip_facts.value_files {
            if let Ok(contents) = read(&dir.join(value_file)) {
                files.push((value_file.clone(), contents));
            }
        }
        chips.push(sensors::discover(&ChipListing {
            key: key.clone(),
            name: chip_facts.name.clone(),
            files,
        }));
    }
    Section::Available(SensorSnapshot { chips })
}

/// Every process the kernel lists, with its CPU share of the whole machine,
/// its resident size and its disk rates. `status` is read exactly once per
/// PID per tick: the facts come from it the first time a PID is seen, and the
/// resident size from it every time.
fn sample_processes(
    state: &mut ProcessState,
    elapsed: Duration,
    cores: usize,
) -> Section<ProcessSnapshot> {
    let root = Path::new(PROC_ROOT);
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            state.sampler.reset();
            state.io.reset();
            state.facts.clear();
            return Section::Unavailable(Reason {
                kind: ReasonKind::Unreadable,
                path: PROC_ROOT.to_owned(),
            });
        }
    };
    let mut ticks = Vec::new();
    let mut io_readings = Vec::new();
    let mut partial = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = entry.path();
        // A process may vanish between readdir and read: skip it silently.
        let Ok(stat_text) = read(&dir.join("stat")) else {
            continue;
        };
        let Ok(stat) = process::parse_stat(&stat_text) else {
            continue;
        };
        let Ok(status_text) = read(&dir.join("status")) else {
            continue;
        };
        let Ok(status) = process::parse_status(&status_text) else {
            continue;
        };
        let facts = match state.facts.get(&pid) {
            Some(facts) if facts.start_ticks == stat.start_ticks => facts.clone(),
            _ => {
                let cmdline = std::fs::read(dir.join("cmdline"))
                    .map(|bytes| process::parse_cmdline(&bytes))
                    .unwrap_or_default();
                let application = read(&dir.join("cgroup"))
                    .ok()
                    .and_then(|text| process::parse_cgroup(&text))
                    .map(|scope| scope.desktop_id);
                let facts = ProcessFacts {
                    start_ticks: stat.start_ticks,
                    name: display_name(&stat.comm, &cmdline),
                    uid: status.uid,
                    application,
                };
                state.facts.insert(pid, facts.clone());
                facts
            }
        };
        // Resident size changes every tick; it is the one status field the
        // cache cannot answer.
        let memory_kib = status.rss_kib.unwrap_or(0);
        if facts.uid == state.own_uid {
            if let Some(io) = read(&dir.join("io"))
                .ok()
                .and_then(|text| process::parse_io(&text).ok())
            {
                io_readings.push((
                    io_key(pid, stat.start_ticks),
                    [io.read_bytes, io.write_bytes],
                ));
            }
        }
        ticks.push((pid, stat.start_ticks, stat.cpu_ticks));
        partial.push((pid, stat.start_ticks, facts, memory_kib));
    }
    let live: HashSet<u32> = partial.iter().map(|(pid, _, _, _)| *pid).collect();
    state.facts.retain(|pid, _| live.contains(pid));
    let cpu: HashMap<u32, f32> = state
        .sampler
        .sample(&ticks, elapsed, state.clock_ticks, cores)
        .into_iter()
        .collect();
    let io: HashMap<String, [f64; 2]> =
        state.io.sample(&io_readings, elapsed).into_iter().collect();
    let readings = partial
        .into_iter()
        .map(|(pid, start_ticks, facts, memory_kib)| ProcessReading {
            pid,
            start_ticks,
            name: facts.name,
            uid: facts.uid,
            cpu_percent: cpu.get(&pid).copied(),
            memory_kib,
            io_rate: io.get(&io_key(pid, start_ticks)).copied(),
            application: facts.application,
        })
        .collect();
    Section::Available(ProcessSnapshot {
        own_uid: state.own_uid,
        readings,
        users: Arc::clone(&state.users),
    })
}

/// The name an IO counter is remembered under. A PID alone is not an
/// identity — the kernel reuses them — so the process's start time is part of
/// the key and a recycled PID starts its rates over instead of inheriting the
/// counters of whatever held that number before it.
fn io_key(pid: u32, start_ticks: u64) -> String {
    format!("{pid}:{start_ticks}")
}

/// The name shown for a process: the first word of its command line when it
/// has one (the binary's basename), else the kernel's `comm`.
fn display_name(comm: &str, cmdline: &[String]) -> String {
    cmdline
        .first()
        .and_then(|argument| argument.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .map_or_else(|| comm.to_owned(), str::to_owned)
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
        Err(cpu::CpuError::NoElapsedTime) => {
            return Section::Unavailable(Reason {
                kind: ReasonKind::NoRate,
                path: STAT_PATH.to_owned(),
            })
        }
        Err(_) => return Section::Unavailable(malformed(path)),
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
    // An array rather than a vector, so the eight reads below are eight
    // bindings and no index can be out of range by construction.
    let texts: Result<Vec<String>, Reason> = AMDGPU_FILES
        .iter()
        .map(|name| read(&device.join(name)))
        .collect();
    let texts = match texts {
        Ok(texts) => texts,
        Err(reason) => return Some(Section::Unavailable(reason)),
    };
    // An eight-file read that did not yield eight strings is a malformed
    // read, not an absent card: the section says so rather than vanishing.
    let Ok(texts) = <[String; 8]>::try_from(texts) else {
        return Some(Section::Unavailable(malformed(
            &device.join(AMDGPU_FILES[0]),
        )));
    };
    let [busy, mem_busy, vram_used, vram_total, gtt_used, gtt_total, sclk, mclk] = &texts;
    let files = AmdgpuFiles {
        busy_percent: busy,
        memory_busy_percent: mem_busy,
        vram_used,
        vram_total,
        gtt_used,
        gtt_total,
        sclk,
        mclk,
    };
    Some(match gpu::parse_amdgpu(&files) {
        Ok(reading) => Section::Available(reading),
        // Name the file that was not a number, not the first one read.
        Err(gpu::GpuError::UnreadableNumber { file, .. }) => {
            Section::Unavailable(malformed(&device.join(file)))
        }
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

fn sample_disks(
    counters: &mut NamedCounters<2>,
    facts: &mut HashMap<String, DiskInfo>,
    elapsed: Duration,
) -> Vec<DiskSection> {
    let readings = match read_disks() {
        Ok(readings) => readings,
        Err(reason) => {
            counters.reset();
            // One unreadable file is every disk unreadable; the list keeps
            // the disks it can still name from sysfs so the rows do not vanish.
            let names = enumerate_block_devices();
            let sections: Vec<DiskSection> = names
                .iter()
                .map(|name| DiskSection {
                    info: facts
                        .entry(name.clone())
                        .or_insert_with(|| disk_info(name))
                        .clone(),
                    rate: Section::Unavailable(reason.clone()),
                })
                .collect();
            facts.retain(|name, _| names.iter().any(|known| known == name));
            return sections;
        }
    };
    // By name, so the list keeps one order whether it came from
    // `/proc/diskstats` or from the sysfs fallback a failure falls back to.
    let mut readings = readings;
    readings.sort_by(|(left, _), (right, _)| left.cmp(right));
    let rates = counters.sample(&readings, elapsed);
    let sections: Vec<DiskSection> = readings
        .iter()
        .map(|(name, _)| DiskSection {
            info: facts
                .entry(name.clone())
                .or_insert_with(|| disk_info(name))
                .clone(),
            rate: Section::Available(
                rates
                    .iter()
                    .find(|(rated, _)| rated == name)
                    .map(|(_, rate)| *rate),
            ),
        })
        .collect();
    facts.retain(|name, _| readings.iter().any(|(known, _)| known == name));
    sections
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

fn interface_info(name: &str, wireless: bool) -> InterfaceInfo {
    let root = Path::new(NET_ROOT).join(name);
    InterfaceInfo {
        name: name.to_owned(),
        wireless,
        // Read every tick: a cable pulled out changes both of these.
        up: read(&root.join("operstate")).is_ok_and(|text| network::parse_operstate(&text)),
        speed_mbit: read(&root.join("speed"))
            .ok()
            .and_then(|text| network::parse_speed_mbit(&text)),
    }
}

/// `(shown, wireless)` for one interface: the two facts sysfs fixes when the
/// interface appears, cached by name.
fn interface_traits(name: &str, facts: &mut HashMap<String, (bool, bool)>) -> (bool, bool) {
    *facts.entry(name.to_owned()).or_insert_with(|| {
        (
            is_shown_interface(name),
            Path::new(NET_ROOT).join(name).join("wireless").is_dir(),
        )
    })
}

/// Interfaces whose link type is Ethernet (which Wi-Fi also reports).
fn is_shown_interface(name: &str) -> bool {
    read(&Path::new(NET_ROOT).join(name).join("type"))
        .ok()
        .and_then(|text| network::parse_link_type(&text).ok())
        .is_some_and(|kind| kind == network::ARPHRD_ETHER)
}

fn sample_interfaces(
    counters: &mut NamedCounters<2>,
    facts: &mut HashMap<String, (bool, bool)>,
    elapsed: Duration,
) -> Vec<InterfaceSection> {
    let path = Path::new(NET_DEV_PATH);
    let stats = match read(path)
        .and_then(|text| network::parse_net_dev(&text).map_err(|_| malformed(path)))
    {
        Ok(stats) => stats,
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
    // Every name the kernel listed keeps its cached verdict, shown or not:
    // an interface that is not shown is one this list never asks sysfs about
    // again while it exists.
    facts.retain(|name, _| stats.iter().any(|stat| &stat.name == name));
    let mut readings: Vec<(String, [u64; 2])> = stats
        .into_iter()
        .filter(|stat| interface_traits(&stat.name, facts).0)
        .map(|stat| (stat.name, [stat.rx_bytes, stat.tx_bytes]))
        .collect();
    // By name, for the same reason the disks are: the row order is the
    // machine's inventory, not the order a kernel file happens to list.
    readings.sort_by(|(left, _), (right, _)| left.cmp(right));
    let rates = counters.sample(&readings, elapsed);
    let sections: Vec<InterfaceSection> = readings
        .iter()
        .map(|(name, _)| InterfaceSection {
            info: interface_info(name, interface_traits(name, facts).1),
            rate: Section::Available(
                rates
                    .iter()
                    .find(|(rated, _)| rated == name)
                    .map(|(_, rate)| *rate),
            ),
        })
        .collect();
    sections
}
