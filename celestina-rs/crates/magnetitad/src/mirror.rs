//! The resident wireless mirror — one press, and thereafter no press at all.
//!
//! [`mirror_discovery`](crate::mirror_discovery) says where the phone is;
//! [`magnetita_core::mirror`] decides what should happen; this module is the
//! part that actually runs `adb` and `scrcpy` and keeps running while the
//! author is not looking.
//!
//! It is one owned thread. It polls discovery, feeds what changed into the pure
//! [`MirrorLink`], and carries out the single [`MirrorAction`] the link asks
//! for. Because Android re-randomises the ADB port on every toggle of Wireless
//! debugging, the standing intent to mirror is honoured against whatever
//! endpoint is advertised *now* — which is what makes the second mirror, and
//! every one after it, need no input.
//!
//! **The scrcpy process is owned by pid.** The author's script reached for
//! `pkill -9 scrcpy`, which would kill an unrelated scrcpy they had open. This
//! keeps the [`Child`] it spawned and terminates that process group and no
//! other. It is also the one long-lived child here: `adb` calls are bounded by
//! [`subprocess`], but scrcpy is meant to outlive its spawn and live until the
//! window closes, so it is polled for exit rather than waited on.

use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use magnetita_core::mirror::{
    AdbService, MirrorAction, MirrorEndpoint, MirrorError, MirrorEvent, MirrorLink, MirrorState,
    FIXED_PORT,
};
use magnetita_core::mirror_options::MirrorOptions;

use crate::mirror_discovery::{self, Advertisement};
use crate::runtime::log;
use rustix::process::Pid;

use crate::subprocess;

/// How often the LAN is asked what is advertised. Fast enough that toggling
/// Wireless debugging feels immediate, slow enough to be nothing next to the
/// `playerctl` polling this daemon already does.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Budget for one `adb` call. `adb connect` against an unreachable host is the
/// slow case and still answers well inside this.
const ADB_BUDGET: Duration = Duration::from_secs(10);

/// How long `adb` is given to move the device from `offline` to `device` after
/// a successful connect. The author's script needed six one-second tries.
const READY_TRIES: u32 = 8;

/// What the app can ask of the mirror.
#[derive(Clone, Debug)]
pub(crate) enum MirrorCommand {
    /// Mirror now, and keep mirroring across the phone's port changes.
    Start,
    /// Stop, and stop reconnecting.
    Stop,
    /// Pair with this code, which the phone is showing right now.
    Pair(String),
    /// Change one mirror option, by its contract name.
    SetOption(String, String),
}

/// What the app renders. A confirmed snapshot, never an optimistic one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MirrorSnapshot {
    pub(crate) state: &'static str,
    pub(crate) can_pair: bool,
    pub(crate) reason: &'static str,
    /// The mirror options, as the contract's `(key, value)` pairs. Published
    /// with the state so the settings surface never shows a value from one
    /// moment beside a state from another.
    pub(crate) options: Vec<(String, String)>,
}

/// The shared, always-current snapshot plus the channel into the worker.
#[derive(Clone)]
pub(crate) struct Mirror {
    snapshot: Arc<Mutex<MirrorSnapshot>>,
    commands: Sender<MirrorCommand>,
}

impl Mirror {
    pub(crate) fn snapshot(&self) -> MirrorSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn send(&self, command: MirrorCommand) -> Result<(), &'static str> {
        self.commands
            .send(command)
            .map_err(|_| "the mirror worker is not running")
    }
}

/// The worker's owned lifetime. Dropping the daemon sets `stopping` and joins,
/// so no mirror survives the service that started it.
pub(crate) struct MirrorWorker {
    stopping: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MirrorWorker {
    pub(crate) fn stop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MirrorWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts the mirror worker and returns the handle the D-Bus interface holds.
///
/// `options_path` is the mirror's own settings file, a sibling of the plugin
/// settings rather than a key inside them: the plugin flags are a published
/// contract the app and the shell already read, and the mirror is not a plugin.
pub(crate) fn start(options_path: PathBuf, endpoint_path: PathBuf) -> (Mirror, MirrorWorker) {
    let (commands, inbox) = mpsc::channel();
    let options = load_options(&options_path);
    let snapshot = Arc::new(Mutex::new(snapshot_of(&MirrorLink::new(), &options)));
    let stopping = Arc::new(AtomicBool::new(false));

    let worker_snapshot = Arc::clone(&snapshot);
    let worker_stopping = Arc::clone(&stopping);
    let handle = thread::spawn(move || {
        Session::new(
            worker_snapshot,
            worker_stopping,
            options,
            options_path,
            endpoint_path,
        )
        .run(&inbox);
    });

    (
        Mirror { snapshot, commands },
        MirrorWorker {
            stopping,
            handle: Some(handle),
        },
    )
}

/// Everything the worker thread owns. Nothing here is shared, so the link and
/// the child process have exactly one owner.
struct Session {
    link: MirrorLink,
    published: Arc<Mutex<MirrorSnapshot>>,
    stopping: Arc<AtomicBool>,
    /// The scrcpy this session started, and the only one it may ever kill.
    mirror: Option<(Child, Pid)>,
    seen_connect: Option<Advertisement>,
    seen_pairing: Option<Advertisement>,
    options: MirrorOptions,
    options_path: PathBuf,
    /// Where the last reached fixed-port endpoint persists, so a daemon
    /// restart does not lose the one way in that needs no discovery.
    endpoint_path: PathBuf,
    /// The session's display variables, resolved fresh before each spawn.
    display_env: Vec<(String, String)>,
}

impl Session {
    fn new(
        published: Arc<Mutex<MirrorSnapshot>>,
        stopping: Arc<AtomicBool>,
        options: MirrorOptions,
        options_path: PathBuf,
        endpoint_path: PathBuf,
    ) -> Self {
        Self {
            link: match load_endpoint(&endpoint_path) {
                Some(remembered) => MirrorLink::with_remembered(remembered),
                None => MirrorLink::new(),
            },
            published,
            stopping,
            mirror: None,
            seen_connect: None,
            seen_pairing: None,
            options,
            options_path,
            endpoint_path,
            display_env: Vec::new(),
        }
    }

    fn run(&mut self, inbox: &Receiver<MirrorCommand>) {
        if !tool_available("adb") || !tool_available("scrcpy") {
            log(
                "mirror",
                "adb or scrcpy is not installed; the mirror is off",
            );
            self.apply(MirrorEvent::ToolMissing);
        }

        while !self.stopping.load(Ordering::Acquire) {
            match inbox.recv_timeout(POLL_INTERVAL) {
                Ok(command) => self.command(command),
                Err(RecvTimeoutError::Timeout) => {}
                // The app and the daemon are gone; take the mirror with us.
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            self.poll_discovery();
            self.poll_mirror_exit();
        }
        self.kill_mirror();
    }

    fn command(&mut self, command: MirrorCommand) {
        let event = match command {
            MirrorCommand::Start => MirrorEvent::MirrorRequested,
            MirrorCommand::Stop => MirrorEvent::StopRequested,
            MirrorCommand::Pair(code) => MirrorEvent::CodeEntered { code },
            MirrorCommand::SetOption(key, value) => {
                self.set_option(&key, &value);
                return;
            }
        };
        self.apply(event);
    }

    /// Applies and persists one option. A running mirror is left alone: scrcpy
    /// cannot change its resolution mid-stream, and silently restarting it
    /// under the author would be worse than the setting taking effect next
    /// time. The state line says as much.
    fn set_option(&mut self, key: &str, value: &str) {
        if !self.options.set(key, value) {
            log("mirror", &format!("refused option {key}={value}"));
            return;
        }
        if let Err(error) = save_options(&self.options, &self.options_path) {
            log("mirror", &format!("could not persist options: {error}"));
        }
        log("mirror", &format!("option {key}={value}"));
        self.publish();
    }

    /// Asks the LAN what is advertised and turns the difference into events.
    /// The *endpoint* is compared, not merely presence: a phone whose port
    /// changed while we were not looking is a new endpoint, not the old one.
    fn poll_discovery(&mut self) {
        for service in [AdbService::Connect, AdbService::Pairing] {
            let found = mirror_discovery::browse(service, &self.stopping)
                .into_iter()
                .next();
            let seen = match service {
                AdbService::Connect => &mut self.seen_connect,
                AdbService::Pairing => &mut self.seen_pairing,
            };
            if *seen == found {
                continue;
            }
            let previous = seen.take();
            *seen = found.clone();
            if previous.is_some() && found.is_none() {
                log("mirror", &format!("{} went away", service.service_type()));
                self.apply(MirrorEvent::ServiceLost { service });
            } else if let Some(found) = found {
                log(
                    "mirror",
                    &format!("{} at {}", service.service_type(), found.endpoint.serial()),
                );
                self.apply(MirrorEvent::ServiceFound {
                    service,
                    endpoint: found.endpoint,
                });
            }
        }
    }

    /// Notices the author closing the scrcpy window, without blocking on it.
    fn poll_mirror_exit(&mut self) {
        let Some((child, _)) = self.mirror.as_mut() else {
            return;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                self.mirror = None;
                let failed = !status.success();
                log(
                    "mirror",
                    if failed {
                        "scrcpy ended unexpectedly; the mirror will return when the phone does"
                    } else {
                        "scrcpy window closed"
                    },
                );
                self.apply(MirrorEvent::MirrorExited { failed });
            }
            Ok(None) => {}
            Err(_) => {
                self.mirror = None;
                self.apply(MirrorEvent::MirrorExited { failed: true });
            }
        }
    }

    /// Advances the pure link and carries out what it asks for, following the
    /// chain until it asks for nothing — a successful pair leads straight into
    /// a connect, and a connect straight into the mirror, with no press between.
    fn apply(&mut self, event: MirrorEvent) {
        let mut next = Some(event);
        while let Some(event) = next.take() {
            let action = self.link.handle(event);
            self.publish();
            next = self.carry_out(action);
        }
    }

    fn carry_out(&mut self, action: MirrorAction) -> Option<MirrorEvent> {
        match action {
            MirrorAction::None => None,
            MirrorAction::Pair { endpoint, code } => Some(MirrorEvent::PairFinished {
                paired: self.pair(endpoint, &code),
            }),
            MirrorAction::Connect { endpoint } => {
                let reached = self.connect(endpoint);
                log(
                    "mirror",
                    &match reached {
                        Some(reached) => format!("connected to {}", reached.serial()),
                        None => format!("could not reach {}", endpoint.serial()),
                    },
                );
                if let Some(reached) = reached {
                    self.remember(reached);
                }
                Some(MirrorEvent::ConnectFinished { endpoint: reached })
            }
            MirrorAction::StartMirror { serial } => {
                // Resolved here, not at boot: this daemon can start before the
                // compositor publishes the session's display, and a scrcpy with
                // nowhere to draw dies at once — which read as "the mirror
                // failed", over and over, instead of "there is no session".
                self.display_env = session_display_env();
                if self.display_env.is_empty() {
                    log("mirror", "no graphical session to open the mirror on");
                    return Some(MirrorEvent::DisplayMissing);
                }
                match self.start_mirror(&serial) {
                    Some(pid) => {
                        log("mirror", &format!("scrcpy {pid} mirroring {serial}"));
                        Some(MirrorEvent::MirrorStarted { pid })
                    }
                    None => {
                        log("mirror", "scrcpy could not start");
                        Some(MirrorEvent::MirrorExited { failed: true })
                    }
                }
            }
            MirrorAction::StopMirror => {
                self.kill_mirror();
                None
            }
        }
    }

    fn pair(&self, endpoint: MirrorEndpoint, code: &str) -> bool {
        // The code is six digits from the phone's screen. It is still passed as
        // its own argument, never through a shell.
        let serial = endpoint.serial();
        let Some(output) = self.adb(&["pair", &serial, code]) else {
            return false;
        };
        String::from_utf8_lossy(&output).contains("Successfully paired")
    }

    /// Reaches the phone, and returns the endpoint that actually answered.
    ///
    /// That is not always the one dialled. After a discovered endpoint comes
    /// up, this pins the device to [`FIXED_PORT`] so the *next* mirror needs no
    /// discovery at all — the port `adb tcpip` opens keeps answering when
    /// Android has turned wireless debugging off and taken the advertisement
    /// with it.
    fn connect(&self, endpoint: MirrorEndpoint) -> Option<MirrorEndpoint> {
        let reached = self.dial(endpoint)?;
        if reached.port == FIXED_PORT {
            return Some(reached);
        }
        // Best-effort: a phone that refuses to be pinned still mirrors on the
        // endpoint that already works, so a failure here is not the mirror's.
        Some(self.pin_fixed_port(reached).unwrap_or(reached))
    }

    /// One `adb connect`, waited until adb calls the device ready.
    fn dial(&self, endpoint: MirrorEndpoint) -> Option<MirrorEndpoint> {
        let serial = endpoint.serial();
        let output = self.adb(&["connect", &serial])?;
        if !String::from_utf8_lossy(&output).contains("connected") {
            return None;
        }
        // `connected` only means the socket opened. The author's script learned
        // the hard way that the device can sit `offline` afterwards, so the
        // mirror does not start until adb calls it a device.
        for _ in 0..READY_TRIES {
            if self.stopping.load(Ordering::Acquire) {
                return None;
            }
            if self.device_ready(&serial) {
                return Some(endpoint);
            }
            thread::sleep(Duration::from_millis(500));
        }
        None
    }

    /// Moves the device onto [`FIXED_PORT`] and reconnects there.
    ///
    /// `adb tcpip` restarts `adbd`, which drops the very connection that asked
    /// for it — so this is a disconnect and a fresh dial, not a live
    /// reconfiguration.
    fn pin_fixed_port(&self, reached: MirrorEndpoint) -> Option<MirrorEndpoint> {
        let serial = reached.serial();
        let port = FIXED_PORT.to_string();
        self.adb(&["-s", &serial, "tcpip", &port])?;
        let _ = self.adb(&["disconnect", &serial]);
        thread::sleep(Duration::from_secs(1));

        let pinned = self.dial(MirrorEndpoint {
            host: reached.host,
            port: FIXED_PORT,
        })?;
        log(
            "mirror",
            &format!("pinned {} — discovery is no longer needed", pinned.serial()),
        );
        Some(pinned)
    }

    fn device_ready(&self, serial: &str) -> bool {
        let Some(output) = self.adb(&["devices"]) else {
            return false;
        };
        String::from_utf8_lossy(&output).lines().any(|line| {
            let mut fields = line.split_whitespace();
            fields.next() == Some(serial) && fields.next() == Some("device")
        })
    }

    fn adb(&self, args: &[&str]) -> Option<Vec<u8>> {
        subprocess::command_output_from("adb", args, Instant::now() + ADB_BUDGET, &self.stopping)
    }

    /// Starts scrcpy and keeps its child. The arguments come from the author's
    /// stored options, built by the one owner of that translation.
    fn start_mirror(&mut self, serial: &str) -> Option<u32> {
        self.kill_mirror();
        let args = self.options.scrcpy_args(serial);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let (child, group) =
            subprocess::spawn_grouped_env("scrcpy", &args, Stdio::null(), &self.display_env)
                .ok()?;
        let pid = child.id();
        self.mirror = Some((child, group));
        Some(pid)
    }

    /// Kills only the scrcpy this session spawned.
    fn kill_mirror(&mut self) {
        if let Some((mut child, group)) = self.mirror.take() {
            subprocess::terminate_group_and_reap(&mut child, group);
        }
    }

    /// Persists a reached fixed-port endpoint. Only the fixed port is worth
    /// keeping: a discovered one is stale by the next toggle.
    fn remember(&self, reached: MirrorEndpoint) {
        if reached.port != FIXED_PORT {
            return;
        }
        if let Err(error) = save_endpoint(&reached, &self.endpoint_path) {
            log(
                "mirror",
                &format!("could not persist the endpoint: {error}"),
            );
        }
    }

    fn publish(&self) {
        let snapshot = snapshot_of(&self.link, &self.options);
        let mut published = self
            .published
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *published = snapshot;
    }
}

/// Whether a program exists on `PATH`, so "scrcpy is not installed" is a stated
/// reason rather than a mirror that silently never appears.
fn tool_available(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

fn snapshot_of(link: &MirrorLink, options: &MirrorOptions) -> MirrorSnapshot {
    let state = link.state();
    MirrorSnapshot {
        state: state_label(state),
        can_pair: link.can_pair(),
        reason: match state {
            MirrorState::Failed(reason) => reason_label(reason),
            _ => "",
        },
        options: options
            .to_pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    }
}

/// Reads the remembered fixed-port endpoint, or `None`.
///
/// Validated through [`MirrorEndpoint::parse`] like anything else that becomes
/// a subprocess argument. This file is ours, but it is on disk and hand
/// editable, and the parse costs nothing.
fn load_endpoint(path: &Path) -> Option<MirrorEndpoint> {
    let text = std::fs::read_to_string(path).ok()?;
    let (host, port) = text.trim().rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let endpoint = MirrorEndpoint::parse(host, port.parse().ok()?).ok()?;
    (endpoint.port == FIXED_PORT).then_some(endpoint)
}

fn save_endpoint(endpoint: &MirrorEndpoint, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    celestina_core::atomic_file::replace(path, endpoint.serial().as_bytes())
}

/// Loads the stored options, falling back to defaults for a missing or corrupt
/// file: losing a preference is not worth refusing to mirror.
fn load_options(path: &Path) -> MirrorOptions {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => MirrorOptions::default(),
    }
}

fn save_options(options: &MirrorOptions, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    celestina_core::atomic_file::replace(path, text.as_bytes())
}

/// The contract's state names. Language-neutral, like the packet types: the
/// app owns the wording the author reads.
fn state_label(state: MirrorState) -> &'static str {
    match state {
        MirrorState::Idle => "idle",
        MirrorState::Available => "available",
        MirrorState::Pairing => "pairing",
        MirrorState::Connecting => "connecting",
        MirrorState::Connected => "connected",
        MirrorState::Mirroring => "mirroring",
        MirrorState::Failed(_) => "failed",
    }
}

/// The session's display variables, for a child that must open a window.
///
/// This process's own environment comes first, but a user service started at
/// login may simply not have them: the compositor pushes `WAYLAND_DISPLAY` and
/// `DISPLAY` into the systemd user manager's environment *after* it starts, and
/// only processes launched afterwards inherit them. So the manager is asked
/// too, which is where the compositor publishes the authoritative values.
///
/// Empty means there is no graphical session to draw on, which is a state to
/// report rather than a failure to retry.
fn session_display_env() -> Vec<(String, String)> {
    let mut resolved = Vec::new();
    for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
        if let Some(value) = std::env::var_os(key).and_then(|value| value.into_string().ok()) {
            if !value.is_empty() {
                resolved.push((key.to_owned(), value));
            }
        }
    }
    if !resolved.is_empty() {
        return resolved;
    }
    systemd_user_environment()
        .into_iter()
        .filter(|(key, _)| key == "WAYLAND_DISPLAY" || key == "DISPLAY")
        .collect()
}

/// The systemd user manager's environment block, or empty if it cannot be
/// read. Best-effort by design: an unreachable session bus just means the
/// mirror stays windowless, not that the daemon should fail.
fn systemd_user_environment() -> Vec<(String, String)> {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return Vec::new();
    };
    let Ok(proxy) = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    ) else {
        return Vec::new();
    };
    let Ok(block) = proxy.get_property::<Vec<String>>("Environment") else {
        return Vec::new();
    };
    block
        .into_iter()
        .filter_map(|entry| {
            entry
                .split_once('=')
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn reason_label(reason: MirrorError) -> &'static str {
    match reason {
        MirrorError::BadAddress => "bad-address",
        MirrorError::BadPort => "bad-port",
        MirrorError::BadServiceName => "bad-service-name",
        MirrorError::NotAdvertised => "not-advertised",
        MirrorError::PairRejected => "pair-rejected",
        MirrorError::ConnectFailed => "connect-failed",
        MirrorError::DeviceOffline => "device-offline",
        MirrorError::MirrorFailed => "mirror-failed",
        MirrorError::ToolMissing => "tool-missing",
        MirrorError::NoDisplay => "no-display",
    }
}

/// Where the mirror contract is served. A sibling of `org.celestina.Devices1`
/// on the same bus name, not an extension of it: mirroring is Android
/// debugging, not KDE Connect, and the shell and Siderita consume that contract
/// without ever wanting this one.
pub(crate) const OBJECT_PATH: &str = "/org/celestina/Mirror1";

/// The interface name.
pub(crate) const INTERFACE: &str = "org.celestina.Mirror1";

/// `org.celestina.Mirror1` — the app's whole view of the mirror.
pub(crate) struct MirrorInterface {
    mirror: Mirror,
}

impl MirrorInterface {
    pub(crate) fn new(mirror: Mirror) -> Self {
        Self { mirror }
    }
}

#[zbus::interface(name = "org.celestina.Mirror1")]
impl MirrorInterface {
    /// Start mirroring, and keep mirroring across the phone's port changes.
    /// Answers immediately: the work is the worker's, and the app reflects only
    /// what [`State`](Self::state) then confirms.
    fn start(&self) -> zbus::fdo::Result<()> {
        self.mirror
            .send(MirrorCommand::Start)
            .map_err(|reason| zbus::fdo::Error::Failed(reason.to_owned()))
    }

    /// Stop mirroring and stop reconnecting. Never touches a scrcpy this daemon
    /// did not start.
    fn stop(&self) -> zbus::fdo::Result<()> {
        self.mirror
            .send(MirrorCommand::Stop)
            .map_err(|reason| zbus::fdo::Error::Failed(reason.to_owned()))
    }

    /// Pair with the code the phone is showing. Only meaningful while
    /// [`CanPair`](Self::can_pair) is true — that is, while a pairing screen is
    /// actually open, which is the only moment a code exists.
    fn pair(&self, code: String) -> zbus::fdo::Result<()> {
        self.mirror
            .send(MirrorCommand::Pair(code))
            .map_err(|reason| zbus::fdo::Error::Failed(reason.to_owned()))
    }

    /// One of `idle`, `available`, `pairing`, `connecting`, `connected`,
    /// `mirroring`, `failed`.
    #[zbus(property)]
    fn state(&self) -> String {
        self.mirror.snapshot().state.to_owned()
    }

    /// True while the phone is showing a pairing screen.
    #[zbus(property)]
    fn can_pair(&self) -> bool {
        self.mirror.snapshot().can_pair
    }

    /// Why the last attempt failed, empty when it did not. Language-neutral;
    /// the app owns the wording.
    #[zbus(property)]
    fn reason(&self) -> String {
        self.mirror.snapshot().reason.to_owned()
    }

    /// The mirror options as `key -> value`, both from the closed contract
    /// vocabulary. Keys: `resolution`, `rate`, `quality`, `audio`, `screenOff`,
    /// `stayAwake`.
    #[zbus(property)]
    fn options(&self) -> std::collections::HashMap<String, String> {
        self.mirror.snapshot().options.into_iter().collect()
    }

    /// Change one option. A value outside the contract is refused and the
    /// stored setting is left as it was. Takes effect on the next mirror:
    /// scrcpy cannot be reconfigured mid-stream.
    fn set_option(&self, key: String, value: String) -> zbus::fdo::Result<()> {
        self.mirror
            .send(MirrorCommand::SetOption(key, value))
            .map_err(|reason| zbus::fdo::Error::Failed(reason.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_and_reason_has_a_contract_name() {
        // The app switches on these strings; an unnamed one would render blank.
        for state in [
            MirrorState::Idle,
            MirrorState::Available,
            MirrorState::Pairing,
            MirrorState::Connecting,
            MirrorState::Connected,
            MirrorState::Mirroring,
            MirrorState::Failed(MirrorError::NotAdvertised),
        ] {
            assert!(!state_label(state).is_empty());
        }
        for reason in [
            MirrorError::BadAddress,
            MirrorError::BadPort,
            MirrorError::BadServiceName,
            MirrorError::NotAdvertised,
            MirrorError::PairRejected,
            MirrorError::ConnectFailed,
            MirrorError::DeviceOffline,
            MirrorError::MirrorFailed,
            MirrorError::ToolMissing,
            MirrorError::NoDisplay,
        ] {
            assert!(!reason_label(reason).is_empty());
        }
    }

    #[test]
    fn a_fresh_link_reports_idle_with_no_reason() {
        let snapshot = snapshot_of(&MirrorLink::new(), &MirrorOptions::default());
        assert_eq!(snapshot.state, "idle");
        assert!(!snapshot.can_pair);
        assert_eq!(snapshot.reason, "");
    }

    #[test]
    fn a_failure_carries_its_reason_into_the_snapshot() {
        let mut link = MirrorLink::new();
        link.handle(MirrorEvent::MirrorRequested);
        let snapshot = snapshot_of(&link, &MirrorOptions::default());
        assert_eq!(snapshot.state, "failed");
        assert_eq!(snapshot.reason, "not-advertised");
    }

    #[test]
    fn the_worker_starts_and_joins_deterministically() {
        let (mirror, mut worker) = start(
            PathBuf::from("/nonexistent/mirror.json"),
            PathBuf::from("/nonexistent/mirror-endpoint"),
        );
        assert_eq!(mirror.snapshot().state, "idle");
        worker.stop();
        // A second stop is a no-op, so Drop after an explicit stop is safe.
        worker.stop();
    }

    #[test]
    fn a_stopped_worker_refuses_commands_instead_of_panicking() {
        let (mirror, mut worker) = start(
            PathBuf::from("/nonexistent/mirror.json"),
            PathBuf::from("/nonexistent/mirror-endpoint"),
        );
        worker.stop();
        // The worker is joined; the channel may or may not have been dropped
        // yet, but neither outcome may panic.
        let _ = mirror.send(MirrorCommand::Stop);
    }

    #[test]
    fn a_remembered_endpoint_round_trips_through_its_file() {
        let dir = std::env::temp_dir().join(format!("magnetita-mirror-{}", std::process::id()));
        let path = dir.join("mirror-endpoint");
        let endpoint = MirrorEndpoint::parse("10.0.0.190", u32::from(FIXED_PORT)).unwrap();
        save_endpoint(&endpoint, &path).expect("save");
        assert_eq!(load_endpoint(&path), Some(endpoint));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_discovery_port_is_refused_on_load() {
        // The file is ours, but it is on disk: a hand-edited discovery port is
        // exactly the stale cache this design refuses, so it is not honoured.
        let dir = std::env::temp_dir().join(format!("magnetita-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("mirror-endpoint");
        std::fs::write(&path, b"10.0.0.190:37059").expect("write");
        assert_eq!(load_endpoint(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_or_malformed_endpoint_file_is_simply_no_memory() {
        let dir = std::env::temp_dir().join(format!("magnetita-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(load_endpoint(&dir.join("absent")), None);
        for junk in [
            "",
            "nonsense",
            "10.0.0.190",
            "phone.local:5555",
            "10.0.0.190:0",
        ] {
            let path = dir.join("junk");
            std::fs::write(&path, junk.as_bytes()).expect("write");
            assert_eq!(load_endpoint(&path), None, "{junk:?} was honoured");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tool_that_cannot_exist_is_not_reported_as_available() {
        assert!(!tool_available("magnetita-no-such-tool"));
    }
}
