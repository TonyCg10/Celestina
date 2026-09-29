//! The thread that reads the machine, and what it hands the window.
//!
//! One thread, one second, one immutable [`Snapshot`]. Every source is read
//! and parsed here, off the Qt thread; the snapshot crosses to Qt by value and
//! is applied whole, so the window never shows one second's CPU beside another
//! second's memory. A source that cannot be read leaves its section
//! [`Section::Unavailable`] with the reason while the others keep publishing.
//! Every kernel file is read bounded, through [`crate::kernel_text`].

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use celestina_core::desktop_entry;
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

use crate::kernel_text::{self, TextError, ARGV0_LIMIT};

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

/// A process's name and application are read again once every fifteenth
/// process read (half a minute), each process on its own read so the
/// re-reads spread instead of landing together: a launcher that moves a
/// running process into its application's scope is shown within that time.
/// An `exec`, which changes the kernel's `comm`, is caught on the next read.
pub const FACTS_REFRESH_READS: u64 = 15;

/// The account table is read again at most once every thirtieth process
/// read (a minute), and only when a listed process's uid has no name in it:
/// a user created during the session gets a name without the table being
/// read every tick.
pub const USERS_REFRESH_READS: u64 = 30;

/// How long [`stop`] waits for the sampling thread before leaving it to end
/// on its own. A tick checks the stop flag between its sections and sleeps
/// in 100 ms slices, so this is reached only when one read is itself slow.
pub const STOP_WAIT: Duration = Duration::from_millis(500);

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
/// How long the listing waits for a manager's reply. The sampler is one
/// thread for the whole window, so a bus that stops answering must not be
/// able to stop the clock: two seconds is far longer than `ListUnits` takes
/// on a machine with a thousand units and far shorter than a tick nobody
/// would notice missing. A listing that times out drops its connection and
/// the next service tick opens a new one.
const LISTING_TIMEOUT: Duration = Duration::from_secs(2);

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
    /// Read when the thread starts and shared by every snapshot; read again,
    /// at most every [`USERS_REFRESH_READS`] process reads, when a listed
    /// uid has no name. Until then such a uid shows as a number.
    pub users: Arc<HashMap<u32, String>>,
    /// Per desktop id a listed process belongs to, the name and icon its
    /// `.desktop` entry gives; an id without a readable entry is absent.
    pub applications: Arc<HashMap<String, Application>>,
}

/// What an application's `.desktop` entry says it is called and looks like.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub icon: String,
}

/// What is read about a process besides its counters, kept per (pid, start)
/// and read again when its `comm` changes or its turn to refresh comes.
#[derive(Clone, Debug)]
struct ProcessFacts {
    start_ticks: u64,
    /// The kernel's `comm` when the facts were read; an `exec` changes it.
    comm: String,
    name: String,
    application: Option<String>,
}

struct ProcessState {
    sampler: ProcessSampler,
    /// Keyed by `(pid, start_ticks)`: a recycled pid starts its rates over.
    io: NamedCounters<2, (u32, u64)>,
    facts: HashMap<u32, ProcessFacts>,
    /// Desktop id to its entry, looked up on this thread once while any
    /// process of that application is listed; `None` is a remembered absence.
    applications: HashMap<String, Option<Application>>,
    users: Arc<HashMap<u32, String>>,
    own_uid: u32,
    clock_ticks: u64,
    /// Process reads so far, which staggers the facts' refreshes.
    reads: u64,
    /// The process read at which the account table was last read.
    users_read: u64,
}

impl ProcessState {
    fn new() -> Self {
        let users = read_users().unwrap_or_default();
        Self {
            sampler: ProcessSampler::new(),
            io: NamedCounters::new(),
            facts: HashMap::new(),
            applications: HashMap::new(),
            users: Arc::new(users),
            own_uid: rustix::process::getuid().as_raw(),
            clock_ticks: rustix::param::clock_ticks_per_second(),
            reads: 0,
            users_read: 0,
        }
    }

    /// Reads the account table again when a listed uid has no name and the
    /// last read is [`USERS_REFRESH_READS`] process reads old; a table that
    /// cannot be read keeps the names already known.
    fn refresh_users<'a>(&mut self, uids: impl IntoIterator<Item = &'a u32>) {
        let unknown = uids.into_iter().any(|uid| !self.users.contains_key(uid));
        if !users_due(unknown, self.reads, self.users_read) {
            return;
        }
        self.users_read = self.reads;
        if let Ok(users) = read_users() {
            self.users = Arc::new(users);
        }
    }
}

fn read_users() -> Result<HashMap<u32, String>, Reason> {
    read(Path::new(PASSWD_PATH)).map(|text| passwd::parse(&text))
}

/// Whether the account table is read again on process read `reads`, the
/// last read of it having been on read `last`.
fn users_due(unknown: bool, reads: u64, last: u64) -> bool {
    unknown && reads.wrapping_sub(last) >= USERS_REFRESH_READS
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

/// A kernel text file, bounded by [`kernel_text::TEXT_LIMIT`]. A file over
/// the limit is `Malformed` — it is not the file this reads — and any other
/// failure `Unreadable`.
pub(crate) fn read(path: &Path) -> Result<String, Reason> {
    kernel_text::read_text(path).map_err(|error| Reason {
        kind: reason_of_text_error(&error),
        path: path.display().to_string(),
    })
}

fn reason_of_text_error(error: &TextError) -> ReasonKind {
    if error.is_too_large() {
        ReasonKind::Malformed
    } else {
        ReasonKind::Unreadable
    }
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
    run: Mutex<Option<Run>>,
}

/// One sampling thread: its own stop flag, so a thread [`stop`] stopped
/// waiting for can never be revived by a later [`subscribe`], and the
/// channel whose sender it drops when it ends.
struct Run {
    stop: Arc<AtomicBool>,
    ended: mpsc::Receiver<()>,
    handle: JoinHandle<()>,
}

/// How [`stop`] parted with a sampling thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stopped {
    /// The thread ended within the wait and was joined.
    Joined,
    /// The thread was still inside a slow read; it was left to end on its
    /// own, and it publishes nothing more.
    Detached,
}

static HUB: OnceLock<Hub> = OnceLock::new();

fn hub() -> &'static Hub {
    HUB.get_or_init(|| Hub {
        subscribers: Mutex::new(Vec::new()),
        run: Mutex::new(None),
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
    // The `run` lock is taken first and held across the registration, so a
    // `subscribe` racing a [`stop`] either registers before that `stop` drains
    // the list — and is dropped with it — or waits and registers on a hub that
    // is already stopped. Registering outside this lock could have put a
    // subscriber into a list `stop` was about to clear while the caller
    // believed it was listening.
    let mut running = shared
        .run
        .lock()
        .map_err(|_| std::io::Error::other("sampler lock poisoned"))?;
    shared
        .subscribers
        .lock()
        .map_err(|_| std::io::Error::other("sampler subscribers lock poisoned"))?
        .push(Box::new(callback));
    if running.is_none() {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let (ended_sender, ended) = mpsc::channel::<()>();
        let handle = thread::Builder::new()
            .name("hematita-sampler".to_owned())
            .spawn(move || {
                // Dropped when this thread ends, however it ends.
                let _ended = ended_sender;
                run(&flag, &|snapshot| {
                    if let Ok(subscribers) = hub().subscribers.lock() {
                        // Read under the lock `stop` drains through, after it
                        // raised the flag: a stopped run never reaches the
                        // subscribers of the next one.
                        if flag.load(Ordering::Acquire) {
                            return;
                        }
                        for subscriber in subscribers.iter() {
                            subscriber(&snapshot);
                        }
                    }
                });
            })?;
        *running = Some(Run {
            stop,
            ended,
            handle,
        });
    }
    Ok(())
}

/// Asks the thread to stop, waits for it at most [`STOP_WAIT`], and leaves
/// the hub able to start again: the subscribers of the run that just ended
/// are dropped, and a later [`subscribe`] spawns a new thread with a flag of
/// its own. A second call is a no-op.
///
/// This runs on the Qt thread when the window goes away, so it must not
/// wait for a read that does not answer: the thread checks its flag between
/// the sections of a tick and every hundred milliseconds while it sleeps,
/// but one service listing can take its two-second timeout and opening a bus
/// has none. A thread still inside such a read when the wait ends is left to
/// finish it; its flag is raised, so it publishes nothing and then ends.
pub fn stop() {
    let shared = hub();
    let Ok(mut running) = shared.run.lock() else {
        return;
    };
    let Some(current) = running.take() else {
        return;
    };
    current.stop.store(true, Ordering::Release);
    // The thread is done publishing, or will publish nothing more, so
    // nothing will call these again; keeping them would hold every
    // closure's captured `CxxQtThread` for the life of the process.
    if let Ok(mut subscribers) = shared.subscribers.lock() {
        subscribers.clear();
    }
    let _ = finish(current, STOP_WAIT);
}

/// Waits at most `limit` for `run` to end; joins it when it did, and drops
/// its handle (detaching it) when it did not.
fn finish(run: Run, limit: Duration) -> Stopped {
    match run.ended.recv_timeout(limit) {
        Err(RecvTimeoutError::Timeout) => Stopped::Detached,
        // Nothing is ever sent: the sender dropping is the thread ending.
        Ok(()) | Err(RecvTimeoutError::Disconnected) => {
            let _ = run.handle.join();
            Stopped::Joined
        }
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
    while !stop.load(Ordering::Acquire) {
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
        // A stop asked for during the slower sections ends the tick here
        // rather than after the buses.
        if stop.load(Ordering::Acquire) {
            break;
        }
        // The first tick asks too, so the page fills at once instead of after
        // five seconds of nothing.
        let service_section = if generation == 1 || generation % SERVICE_TICKS == 0 {
            Some(sample_services(&mut system_bus, &mut session_bus, stop))
        } else {
            None
        };
        if stop.load(Ordering::Acquire) {
            break;
        }
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
        while slept < INTERVAL && !stop.load(Ordering::Acquire) {
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

/// A listing that did not happen: why, and whether the connection it was
/// asked over is worth keeping.
#[derive(Debug, PartialEq, Eq)]
struct ListFailure {
    reason: Reason,
    keep_connection: bool,
}

/// What a failed `ListUnits` says about the manager and the connection.
///
/// - A reply that arrived but is not `a(ssssssouso)` (`Variant`,
///   `InvalidReply`) is `Malformed`: the manager answered, so the
///   connection is alive and stays.
/// - A D-Bus error reply (`MethodError`) — no manager on that bus
///   (`ServiceUnknown`), a refusal (`AccessDenied`) — leaves the listing
///   unavailable; the bus itself carried that answer, so the connection
///   stays too.
/// - Anything else — a socket that went away, a timeout — leaves this
///   connection unfit to reuse, and the next service tick opens another.
fn failure_of(error: &zbus::Error, bus_label: &str) -> ListFailure {
    match error {
        zbus::Error::Variant(_) | zbus::Error::InvalidReply => ListFailure {
            reason: Reason {
                kind: ReasonKind::Malformed,
                path: bus_label.to_owned(),
            },
            keep_connection: true,
        },
        zbus::Error::MethodError(..) => ListFailure {
            reason: bus_unavailable(bus_label),
            keep_connection: true,
        },
        _ => ListFailure {
            reason: bus_unavailable(bus_label),
            keep_connection: false,
        },
    }
}

/// Every unit one manager has loaded. A manager that answers something other
/// than the declared shape is `Malformed`, which is a different sentence from
/// a bus that is not there.
fn list_units(
    connection: &zbus::blocking::Connection,
    scope: Scope,
    bus_label: &str,
) -> Result<Vec<Unit>, ListFailure> {
    let proxy = match zbus::blocking::Proxy::new(
        connection,
        SYSTEMD_SERVICE,
        SYSTEMD_OBJECT,
        SYSTEMD_MANAGER,
    ) {
        Ok(proxy) => proxy,
        Err(_) => {
            return Err(ListFailure {
                reason: bus_unavailable(bus_label),
                keep_connection: false,
            })
        }
    };
    match proxy.call::<_, _, Vec<UnitRow>>("ListUnits", &()) {
        Ok(rows) => Ok(rows
            .into_iter()
            .map(|(name, description, _load, active, sub, ..)| Unit {
                name,
                description,
                scope,
                active,
                sub,
            })
            .collect()),
        Err(error) => Err(failure_of(&error, bus_label)),
    }
}

/// One bus's section, opening the connection if there is none and dropping it
/// if this listing proved it dead — so a bus that goes away is reopened on the
/// next service tick instead of failing for the rest of the session. The
/// connection carries [`LISTING_TIMEOUT`], so a manager that stops answering
/// costs one tick's listing rather than the sampler thread.
fn section_of_bus(
    slot: &mut Option<zbus::blocking::Connection>,
    open: fn() -> zbus::Result<zbus::blocking::connection::Builder<'static>>,
    scope: Scope,
    bus_label: &str,
) -> Section<Vec<Unit>> {
    if slot.is_none() {
        *slot = open()
            .and_then(|builder| builder.method_timeout(LISTING_TIMEOUT).build())
            .ok();
    }
    let Some(connection) = slot.as_ref() else {
        return Section::Unavailable(bus_unavailable(bus_label));
    };
    match list_units(connection, scope, bus_label) {
        Ok(units) => Section::Available(units),
        Err(failure) => {
            if !failure.keep_connection {
                *slot = None;
            }
            Section::Unavailable(failure.reason)
        }
    }
}

/// Both managers' unit lists, on this thread. This is the only place the
/// sampler talks to a bus, and it does it every [`SERVICE_TICKS`] ticks. A
/// stop asked for after the system bus answered skips the session bus; the
/// snapshot is not published then anyway.
fn sample_services(
    system: &mut Option<zbus::blocking::Connection>,
    user: &mut Option<zbus::blocking::Connection>,
    stop: &AtomicBool,
) -> ServiceSnapshot {
    let system = section_of_bus(
        system,
        zbus::blocking::connection::Builder::system,
        Scope::System,
        SYSTEM_BUS,
    );
    let user = if stop.load(Ordering::Acquire) {
        Section::Unavailable(bus_unavailable(SESSION_BUS))
    } else {
        section_of_bus(
            user,
            zbus::blocking::connection::Builder::session,
            Scope::User,
            SESSION_BUS,
        )
    };
    ServiceSnapshot { system, user }
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
/// PID per tick, and its owner and resident size are taken from it every
/// time: a `setuid` or a `setuid` binary's `exec` shows at once. The name
/// and the application come from the cached facts, read again when the
/// kernel's `comm` changes (an `exec`) or the process's refresh turn comes.
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
            state.applications.clear();
            return Section::Unavailable(Reason {
                kind: ReasonKind::Unreadable,
                path: PROC_ROOT.to_owned(),
            });
        }
    };
    state.reads = state.reads.wrapping_add(1);
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
            Some(facts) if facts_current(facts, &stat, pid, state.reads) => facts.clone(),
            _ => {
                let facts = read_facts(&dir, &stat);
                state.facts.insert(pid, facts.clone());
                facts
            }
        };
        // Resident size and owner change while a process lives; they are
        // what the cache cannot answer.
        let memory_kib = status.rss_kib.unwrap_or(0);
        if status.uid == state.own_uid {
            if let Some(io) = read(&dir.join("io"))
                .ok()
                .and_then(|text| process::parse_io(&text).ok())
            {
                io_readings.push(((pid, stat.start_ticks), [io.read_bytes, io.write_bytes]));
            }
        }
        ticks.push((pid, stat.start_ticks, stat.cpu_ticks));
        partial.push((pid, stat.start_ticks, status.uid, facts, memory_kib));
    }
    state.refresh_users(partial.iter().map(|(_, _, uid, ..)| uid));
    let live: HashSet<u32> = partial.iter().map(|(pid, ..)| *pid).collect();
    state.facts.retain(|pid, _| live.contains(pid));
    let applications = listed_applications(
        &mut state.applications,
        partial
            .iter()
            .filter_map(|(.., facts, _)| facts.application.as_deref()),
    );
    let cpu: HashMap<u32, f32> = state
        .sampler
        .sample(&ticks, elapsed, state.clock_ticks, cores)
        .into_iter()
        .collect();
    let io: HashMap<(u32, u64), [f64; 2]> =
        state.io.sample(&io_readings, elapsed).into_iter().collect();
    let readings = partial
        .into_iter()
        .map(
            |(pid, start_ticks, uid, facts, memory_kib)| ProcessReading {
                pid,
                start_ticks,
                name: facts.name,
                uid,
                cpu_percent: cpu.get(&pid).copied(),
                memory_kib,
                io_rate: io.get(&(pid, start_ticks)).copied(),
                application: facts.application,
            },
        )
        .collect();
    Section::Available(ProcessSnapshot {
        own_uid: state.own_uid,
        readings,
        users: Arc::clone(&state.users),
        applications: Arc::new(applications),
    })
}

/// Whether the cached facts still describe the process `stat` reads now:
/// the same start (not a recycled pid), the same `comm` (no `exec` since),
/// and not this process's turn to refresh on read number `reads`.
fn facts_current(facts: &ProcessFacts, stat: &process::ProcessStat, pid: u32, reads: u64) -> bool {
    let turn = reads.wrapping_add(u64::from(pid)) % FACTS_REFRESH_READS == 0;
    facts.start_ticks == stat.start_ticks && facts.comm == stat.comm && !turn
}

/// A process's name, from the start of its command line, and the desktop
/// application its cgroup places it in.
fn read_facts(dir: &Path, stat: &process::ProcessStat) -> ProcessFacts {
    let cmdline = kernel_text::read_prefix(&dir.join("cmdline"), ARGV0_LIMIT)
        .map(|bytes| process::parse_cmdline(&bytes))
        .unwrap_or_default();
    let application = read(&dir.join("cgroup"))
        .ok()
        .and_then(|text| process::parse_cgroup(&text))
        .map(|scope| scope.desktop_id);
    ProcessFacts {
        start_ticks: stat.start_ticks,
        comm: stat.comm.clone(),
        name: display_name(&stat.comm, &cmdline),
        application,
    }
}

/// The entries of the applications `listed` names, each looked up once while
/// any of its processes is listed: `known` keeps the answers (an absence
/// included) for those ids and forgets the others, so an application that
/// comes back is looked up again.
fn listed_applications<'a>(
    known: &mut HashMap<String, Option<Application>>,
    listed: impl Iterator<Item = &'a str>,
) -> HashMap<String, Application> {
    let live: HashSet<&str> = listed.collect();
    known.retain(|id, _| live.contains(id.as_str()));
    let mut found = HashMap::new();
    for id in live {
        let entry = known
            .entry(id.to_owned())
            .or_insert_with(|| application_entry(id));
        if let Some(application) = entry {
            found.insert(id.to_owned(), application.clone());
        }
    }
    found
}

/// The name and icon of desktop id `id` (without `.desktop`), read bounded
/// through the suite's one desktop-entry lookup, under its one shadowing
/// rule. An entry without a name is no answer.
fn application_entry(id: &str) -> Option<Application> {
    let dirs = desktop_entry::application_search_dirs();
    let entry = desktop_entry::find(&dirs, &format!("{id}.desktop"))?;
    (!entry.name.is_empty()).then_some(Application {
        name: entry.name,
        icon: entry.icon,
    })
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

/// The model of the block device `name` (`nvme0n1`, `sda`), read the way the
/// disk section reads it, or `None` when the device reports none. The storage
/// section names a mounted disk with it.
pub fn disk_model_of(name: &str) -> Option<String> {
    let model = disk_info(name).model;
    (!model.is_empty()).then_some(model)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn method_error(name: &str) -> zbus::Error {
        let message = zbus::message::Message::method_call(SYSTEMD_OBJECT, "ListUnits")
            .expect("a well-formed path and member")
            .build(&())
            .expect("a message with an empty body");
        zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(name).expect("a well-formed error name"),
            None,
            message,
        )
    }

    fn stat(pid: u32, comm: &str, start_ticks: u64) -> process::ProcessStat {
        process::ProcessStat {
            pid,
            comm: comm.to_owned(),
            state: 'S',
            ppid: 1,
            cpu_ticks: 0,
            start_ticks,
        }
    }

    fn facts(comm: &str, start_ticks: u64) -> ProcessFacts {
        ProcessFacts {
            start_ticks,
            comm: comm.to_owned(),
            name: comm.to_owned(),
            application: None,
        }
    }

    #[test]
    fn a_reply_of_the_wrong_shape_is_malformed_and_keeps_the_connection() {
        for error in [
            zbus::Error::Variant(zbus::zvariant::Error::IncorrectType),
            zbus::Error::InvalidReply,
        ] {
            let failure = failure_of(&error, SYSTEM_BUS);
            assert_eq!(failure.reason.kind, ReasonKind::Malformed, "{error:?}");
            assert!(failure.keep_connection);
        }
    }

    #[test]
    fn an_error_reply_is_an_unavailable_manager_on_a_live_bus() {
        for name in [
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "org.freedesktop.DBus.Error.AccessDenied",
        ] {
            let failure = failure_of(&method_error(name), SESSION_BUS);
            assert_eq!(
                failure,
                ListFailure {
                    reason: bus_unavailable(SESSION_BUS),
                    keep_connection: true,
                },
                "{name}"
            );
        }
    }

    #[test]
    fn a_broken_or_silent_bus_drops_the_connection() {
        let timeout = zbus::Error::from(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out",
        ));
        let failure = failure_of(&timeout, SYSTEM_BUS);
        assert_eq!(failure.reason.kind, ReasonKind::Unreadable);
        assert!(!failure.keep_connection);
    }

    #[test]
    fn facts_are_read_again_after_an_exec_or_a_recycled_pid() {
        let cached = facts("bash", 100);
        // A read that is not this process's refresh turn.
        let reads = 1;
        assert!(facts_current(&cached, &stat(41, "bash", 100), 41, reads));
        assert!(
            !facts_current(&cached, &stat(41, "sudo", 100), 41, reads),
            "exec"
        );
        assert!(
            !facts_current(&cached, &stat(41, "bash", 900), 41, reads),
            "recycled"
        );
    }

    #[test]
    fn each_process_refreshes_once_per_period_on_its_own_read() {
        let cached = facts("bash", 100);
        for pid in [1_u32, 2, 41, 4_194_303] {
            let turns = (0..FACTS_REFRESH_READS)
                .filter(|reads| !facts_current(&cached, &stat(pid, "bash", 100), pid, *reads))
                .count();
            assert_eq!(turns, 1, "pid {pid}");
        }
        // Neighbouring pids take their turns on different reads.
        let turn_of = |pid: u32| {
            (0..FACTS_REFRESH_READS)
                .find(|reads| !facts_current(&cached, &stat(pid, "bash", 100), pid, *reads))
        };
        assert_ne!(turn_of(41), turn_of(42));
    }

    #[test]
    fn the_account_table_is_read_again_only_for_an_unknown_uid_and_not_often() {
        assert!(!users_due(false, 1000, 0), "every uid has a name");
        assert!(!users_due(true, 29, 0), "read at the start, too soon");
        assert!(users_due(true, 30, 0));
        assert!(!users_due(true, 45, 30), "read a moment ago");
        assert!(users_due(true, 60, 30));
    }

    #[test]
    fn an_application_is_looked_up_once_while_it_is_listed() {
        let mut known: HashMap<String, Option<Application>> = HashMap::new();
        known.insert(
            "gone".to_owned(),
            Some(Application {
                name: "Gone".to_owned(),
                icon: String::new(),
            }),
        );
        known.insert(
            "kept".to_owned(),
            Some(Application {
                name: "Kept".to_owned(),
                icon: "kept".to_owned(),
            }),
        );
        let found = listed_applications(
            &mut known,
            ["kept", "kept", "hematita-test-no-such-application"].into_iter(),
        );
        assert!(!known.contains_key("gone"), "no process lists it any more");
        assert_eq!(
            known.get("hematita-test-no-such-application"),
            Some(&None),
            "an absence is remembered"
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found.get("kept").map(|app| app.icon.as_str()), Some("kept"));
    }

    #[test]
    fn an_oversized_kernel_file_is_malformed_and_a_missing_one_unreadable() {
        let refused = TextError::File(celestina_core::atomic_file::ReadError::TooLarge {
            path: PathBuf::from("/proc/cpuinfo"),
            limit: 4,
        });
        assert_eq!(reason_of_text_error(&refused), ReasonKind::Malformed);
        let missing = TextError::Missing {
            path: PathBuf::from("/proc/1/io"),
        };
        assert_eq!(reason_of_text_error(&missing), ReasonKind::Unreadable);
        let reason = read(Path::new("/proc/hematita-no-such-file")).expect_err("missing");
        assert_eq!(reason.kind, ReasonKind::Unreadable);
    }

    #[test]
    fn a_thread_that_ends_in_time_is_joined() {
        let (ended_sender, ended) = mpsc::channel::<()>();
        let handle = thread::spawn(move || drop(ended_sender));
        let run = Run {
            stop: Arc::new(AtomicBool::new(true)),
            ended,
            handle,
        };
        assert_eq!(finish(run, Duration::from_secs(5)), Stopped::Joined);
    }

    #[test]
    fn a_thread_stuck_in_a_read_is_left_behind_instead_of_waited_for() {
        let (ended_sender, ended) = mpsc::channel::<()>();
        let (release, parked) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            let _ended = ended_sender;
            // Stands for a bus that does not answer.
            let _ = parked.recv();
        });
        let run = Run {
            stop: Arc::new(AtomicBool::new(true)),
            ended,
            handle,
        };
        let started = Instant::now();
        assert_eq!(finish(run, Duration::from_millis(50)), Stopped::Detached);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the wait is bounded"
        );
        let _ = release.send(());
    }
}
