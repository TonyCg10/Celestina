//! The audio client: WirePlumber's own command line. `wpctl status` lists
//! the sinks, sources, streams and cards with the defaults marked;
//! `wpctl inspect` names them, `wpctl get-volume` reads a stream's volume and
//! `wpctl set-*` commands them. `wpctl` lists no card profiles, so those come
//! from `pw-cli enum-params`. The binding spike (CUP-1-E evidence) chose this
//! over the `pipewire` crate: a plain node `Props` write is not what
//! WirePlumber keeps for a device-backed sink.
//!
//! Every call spawns a short-lived process and blocks: call it on a worker.
//! The parsers are pure and tested over captured output.

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::audio::Audio;
use crate::error::AudioError;
use crate::model::{AudioEndpoint, AudioSnapshot, AudioStream, CardProfile};
use crate::volume::clamp_volume;

pub use crate::wpctl_status::{parse_status, parse_volume, Card, Status};

/// How often the watcher re-reads `wpctl status` when `pw-mon` cannot tell
/// it about changes, or PipeWire did not answer.
const POLL: Duration = Duration::from_secs(2);
/// How long the watcher lets a burst of `pw-mon` events settle.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// How long one tool call may take before the sound server counts as stalled.
const DEADLINE: Duration = Duration::from_secs(2);

/// Runs `program` with `args` in the C locale and returns its standard
/// output. A missing program means the sound server's tools are absent; an
/// object the tool cannot find is `NotFound`; a call still running after
/// `DEADLINE` is killed and reads as `Unavailable` (a stalled server).
fn run(program: &str, args: &[&str]) -> Result<String, AudioError> {
    run_within(program, args, DEADLINE)
}

fn run_within(program: &str, args: &[&str], deadline: Duration) -> Result<String, AudioError> {
    use std::io::Read;

    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AudioError::Unavailable,
            _ => logged(&format!("{program}: {e}")),
        })?;
    // Drained on threads of their own, so a full pipe never stalls the child.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AudioError::Unavailable);
            }
            Err(e) => return Err(logged(&format!("{program}: {e}"))),
        }
    };
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    if status.success() {
        return Ok(String::from_utf8_lossy(&stdout).into_owned());
    }
    Err(fault(String::from_utf8_lossy(&stderr).trim()))
}

/// A failure the tool described only in its own English words: the detail
/// goes to the log and the notice says a short Spanish sentence.
fn logged(detail: &str) -> AudioError {
    eprintln!("cuprita: PipeWire: {detail}");
    AudioError::service_failed()
}

/// A failed tool's message as an error: an object the tool cannot find is
/// `NotFound`, a sound server it cannot reach `Unavailable`, anything else
/// is logged (see `logged`).
fn fault(detail: &str) -> AudioError {
    // Warnings may come first; the tool's verdict is its last line.
    let detail = detail
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if let Some(rest) = detail.strip_prefix("Object '") {
        if let Some(id) = rest.split('\'').next() {
            if detail.ends_with("not found") {
                return AudioError::NotFound(id.to_owned());
            }
        }
    }
    if detail.contains("connect") {
        AudioError::Unavailable
    } else if detail.is_empty() {
        logged("the tool failed without a message")
    } else {
        logged(detail)
    }
}

/// The links `pw-link -lI` prints: for each port id, the node names of the
/// ports it is linked to (`node.name:port`).
#[must_use]
pub fn parse_links(text: &str) -> HashMap<u32, Vec<String>> {
    let mut links: HashMap<u32, Vec<String>> = HashMap::new();
    let mut port: Option<u32> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let link = trimmed.contains("|->") || trimmed.contains("|<-");
        if link {
            // `<link id>   |->   <peer port id> <node>:<port>`
            let Some(peer) = trimmed
                .split_once("|->")
                .or_else(|| trimmed.split_once("|<-"))
                .map(|(_, p)| p.trim())
            else {
                continue;
            };
            let name = peer.split_once(' ').map_or("", |(_, n)| n.trim());
            if let (Some(port), Some((node, _))) = (port, name.rsplit_once(':')) {
                links.entry(port).or_default().push(node.to_owned());
            }
        } else if let Some((id, _)) = trimmed.split_once(' ') {
            port = id.parse().ok();
        }
    }
    links
}

/// Each stream's endpoint from its ports' links: the first endpoint whose
/// `node.name` a port is linked to. Streams with no such link keep the
/// guess `parse_status` made from the port's nickname.
pub fn stream_endpoints(
    streams: &mut [AudioStream],
    stream_ports: &HashMap<u32, Vec<u32>>,
    endpoints: &[AudioEndpoint],
    links: &HashMap<u32, Vec<String>>,
) {
    for stream in streams {
        let peer = stream_ports
            .get(&stream.id)
            .into_iter()
            .flatten()
            .filter_map(|port| links.get(port))
            .flatten()
            .find_map(|node| {
                endpoints
                    .iter()
                    .find(|e| !e.name.is_empty() && e.name == *node)
            });
        if let Some(endpoint) = peer {
            stream.endpoint = endpoint.id;
        }
    }
}

/// The `key = "value"` pairs of `wpctl inspect`.
#[must_use]
pub fn parse_inspect(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start().trim_start_matches('*').trim_start();
            let (key, value) = line.split_once(" = ")?;
            Some((key.to_owned(), value.trim().trim_matches('"').to_owned()))
        })
        .collect()
}

/// One profile from `pw-cli enum-params`: its index, name and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    pub index: u32,
    /// `false` only when PipeWire says the profile is unavailable (`no`).
    pub available: bool,
    pub name: String,
    pub description: String,
}

/// The profiles `pw-cli enum-params <device> EnumProfile` (or `Profile`)
/// prints, one per `Object:`.
#[must_use]
pub fn parse_profiles(text: &str) -> Vec<ProfileEntry> {
    let mut profiles = Vec::new();
    let mut current: Option<ProfileEntry> = None;
    // The property whose value the next line carries.
    let mut key = "";
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("Object:") {
            profiles.extend(current.take());
            current = Some(ProfileEntry {
                index: 0,
                available: true,
                name: String::new(),
                description: String::new(),
            });
            key = "";
        } else if let Some(rest) = line.strip_prefix("Prop: key Spa:Pod:Object:Param:Profile:") {
            key = match rest.split_whitespace().next() {
                Some("index") => "index",
                Some("name") => "name",
                Some("description") => "description",
                Some("available") => "available",
                _ => "",
            };
        } else if let Some(profile) = current.as_mut() {
            match key {
                "index" => {
                    if let Some(n) = line.strip_prefix("Int ").and_then(|n| n.parse().ok()) {
                        profile.index = n;
                    }
                }
                "available" => {
                    if line.starts_with("Id ") {
                        profile.available = !line.contains("ParamAvailability:no)");
                    }
                }
                "name" | "description" => {
                    if let Some(s) = line.strip_prefix("String ") {
                        let s = s.trim_matches('"').to_owned();
                        if key == "name" {
                            profile.name = s;
                        } else {
                            profile.description = s;
                        }
                    }
                }
                _ => {}
            }
            key = "";
        }
    }
    profiles.extend(current);
    profiles
}

/// A card's choosable profiles, with the one in use marked. `off` is left
/// out (switching a card off is not a sound choice the page offers), and so
/// is a profile PipeWire marks unavailable, unless it is the one in use.
#[must_use]
pub fn card_profiles(
    card_id: u32,
    all: &[ProfileEntry],
    active: &[ProfileEntry],
) -> Vec<CardProfile> {
    let active = active.first().map(|p| p.name.as_str());
    all.iter()
        .filter(|p| p.name != "off" && (p.available || Some(p.name.as_str()) == active))
        .map(|p| CardProfile {
            card_id,
            id: p.name.clone(),
            description: p.description.clone(),
            active: Some(p.name.as_str()) == active,
        })
        .collect()
}

/// `wpctl`'s volume argument: a plain decimal, without a limit.
fn volume_arg(volume: f32) -> String {
    format!("{:.2}", clamp_volume(volume))
}

/// The audio client over `wpctl` and `pw-cli`.
#[derive(Debug, Default)]
pub struct WpctlAudio {
    _private: (),
}

impl WpctlAudio {
    /// Checks that `wpctl` runs and reaches the session's PipeWire.
    ///
    /// # Errors
    /// `Unavailable` without `wpctl` or a sound server.
    pub fn connect_session() -> Result<Self, AudioError> {
        run("wpctl", &["status"])?;
        Ok(Self { _private: () })
    }

    fn profiles_of(card_id: u32) -> Result<Vec<ProfileEntry>, AudioError> {
        Ok(parse_profiles(&run(
            "pw-cli",
            &["enum-params", &card_id.to_string(), "EnumProfile"],
        )?))
    }
}

impl Audio for WpctlAudio {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError> {
        let status = parse_status(&run("wpctl", &["status"])?);
        let mut endpoints = status.endpoints;
        for endpoint in &mut endpoints {
            // A node gone between the two calls keeps an empty name.
            if let Ok(text) = run("wpctl", &["inspect", &endpoint.id.to_string()]) {
                let props = parse_inspect(&text);
                if let Some(name) = props.get("node.name") {
                    endpoint.name.clone_from(name);
                }
            }
        }
        let mut streams = status.streams;
        for stream in &mut streams {
            let id = stream.id.to_string();
            if let Ok(text) = run("wpctl", &["inspect", &id]) {
                let props = parse_inspect(&text);
                if let Some(name) = props.get("application.name") {
                    stream.app_name.clone_from(name);
                }
                if let Some(icon) = props.get("application.icon-name") {
                    stream.app_icon.clone_from(icon);
                }
            }
            if let Some((volume, muted)) = run("wpctl", &["get-volume", &id])
                .ok()
                .and_then(|t| parse_volume(&t))
            {
                stream.volume = volume;
                stream.muted = muted;
            }
        }
        // Without `pw-link`, the nickname guess stands.
        if let Ok(text) = run("pw-link", &["-lI"]) {
            stream_endpoints(
                &mut streams,
                &status.stream_ports,
                &endpoints,
                &parse_links(&text),
            );
        }
        let mut profiles = Vec::new();
        for card in &status.cards {
            let id = card.id.to_string();
            let Ok(all) = run("pw-cli", &["enum-params", &id, "EnumProfile"]) else {
                continue;
            };
            let active = run("pw-cli", &["enum-params", &id, "Profile"]).unwrap_or_default();
            profiles.extend(card_profiles(
                card.id,
                &parse_profiles(&all),
                &parse_profiles(&active),
            ));
        }
        Ok(AudioSnapshot {
            endpoints,
            streams,
            profiles,
        })
    }

    fn set_default(&mut self, id: u32) -> Result<(), AudioError> {
        run("wpctl", &["set-default", &id.to_string()]).map(drop)
    }

    fn set_volume(&mut self, id: u32, volume: f32) -> Result<(), AudioError> {
        run(
            "wpctl",
            &["set-volume", &id.to_string(), &volume_arg(volume)],
        )
        .map(drop)
    }

    fn set_muted(&mut self, id: u32, muted: bool) -> Result<(), AudioError> {
        let state = if muted { "1" } else { "0" };
        run("wpctl", &["set-mute", &id.to_string(), state]).map(drop)
    }

    fn set_profile(&mut self, card_id: u32, profile: &str) -> Result<(), AudioError> {
        let index = Self::profiles_of(card_id)?
            .into_iter()
            .find(|p| p.name == profile)
            .map(|p| p.index)
            .ok_or_else(|| AudioError::NotFound(profile.to_owned()))?;
        run(
            "wpctl",
            &["set-profile", &card_id.to_string(), &index.to_string()],
        )
        .map(drop)
    }
}

/// Reads `pw-mon`'s event stream line by line and tells which lines finish
/// an event that can change a snapshot: a node, device or metadata object
/// added, changed or removed. Clients, ports and links come and go with
/// every `wpctl` call the client itself makes, so they do not count, or the
/// watcher would wake itself forever.
#[derive(Debug, Default)]
pub struct MonitorFilter {
    /// The event under way and its object id, until its type line arrives.
    pending: Option<(Event, Option<u32>)>,
    /// The nodes, devices and metadata objects seen, to judge a removal
    /// (`removed:` carries only the id).
    relevant: std::collections::HashSet<u32>,
    /// Whether the first dump (every object, `added:`) is over: until the
    /// first event that is not an addition, additions only fill `relevant`.
    primed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Added,
    Changed,
    Removed,
}

const RELEVANT_TYPES: &[&str] = &[
    "PipeWire:Interface:Node",
    "PipeWire:Interface:Device",
    "PipeWire:Interface:Metadata",
];

impl MonitorFilter {
    /// Feeds one line; `true` when it completes a relevant event.
    pub fn feed(&mut self, line: &str) -> bool {
        let line = line.trim();
        let event = match line {
            "added:" => Some(Event::Added),
            "changed:" => Some(Event::Changed),
            "removed:" => Some(Event::Removed),
            _ => None,
        };
        if let Some(event) = event {
            self.pending = Some((event, None));
            return false;
        }
        let Some((event, id)) = self.pending.as_mut() else {
            return false;
        };
        if let Some(n) = line.strip_prefix("id:") {
            *id = n.trim().parse().ok();
            if *event == Event::Removed {
                let (_, id) = self.pending.take().unwrap_or((Event::Removed, None));
                self.primed = true;
                return id.is_some_and(|id| self.relevant.remove(&id));
            }
            return false;
        }
        if let Some(kind) = line.strip_prefix("type:") {
            let (event, id) = self.pending.take().unwrap_or((Event::Changed, None));
            if !RELEVANT_TYPES.iter().any(|t| kind.trim().starts_with(t)) {
                return false;
            }
            if let Some(id) = id {
                self.relevant.insert(id);
            }
            if event == Event::Changed {
                self.primed = true;
            }
            return event == Event::Changed || self.primed;
        }
        false
    }
}

/// A running `pw-mon`, shared by its handle, its reader and `stop_monitors`.
type Monitor = Arc<Mutex<Option<Child>>>;

/// Every `pw-mon` a watcher started, so `stop_monitors` can end them when
/// the application exits (its worker threads, and the handles they hold,
/// are never dropped then).
static MONITORS: Mutex<Vec<Monitor>> = Mutex::new(Vec::new());

fn stop(monitor: &Monitor) {
    let mut slot = monitor.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(mut child) = slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Kills every `pw-mon` still running. Call it as the application exits.
pub fn stop_monitors() {
    let monitors = std::mem::take(&mut *MONITORS.lock().unwrap_or_else(PoisonError::into_inner));
    for monitor in &monitors {
        stop(monitor);
    }
}

/// Keeps the watcher alive. Dropping it stops the calls to `on_change` at
/// once, kills `pw-mon` and lets the poll thread end within one poll.
pub struct WatchHandle {
    stopped: Arc<AtomicBool>,
    monitor: Option<Monitor>,
}

impl WatchHandle {
    /// The process id of the watcher's `pw-mon`, while it runs.
    #[must_use]
    pub fn monitor_pid(&self) -> Option<u32> {
        let monitor = self.monitor.as_ref()?;
        let slot = monitor.lock().unwrap_or_else(PoisonError::into_inner);
        slot.as_ref().map(Child::id)
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(monitor) = self.monitor.take() {
            stop(&monitor);
            MONITORS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retain(|m| !Arc::ptr_eq(m, &monitor));
        }
    }
}

/// How long the watcher waits before it reads `wpctl status` again without
/// an event: never while `pw-mon` runs and the last read worked (its events
/// are the only wake-up), every 2 s while `pw-mon` is missing or ended, or
/// while PipeWire did not answer the last read.
fn next_wait(monitor_alive: bool, pipewire_present: bool) -> Option<Duration> {
    (!(monitor_alive && pipewire_present)).then_some(POLL)
}

/// Lets a burst of `pw-mon` events settle: waits until 300 ms after the
/// burst's first event, however many follow (the cap keeps a stream of
/// events from holding the read back).
fn settle(ticks: &mpsc::Receiver<()>) {
    let until = Instant::now() + DEBOUNCE;
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        if ticks.recv_timeout(left).is_err() {
            break;
        }
    }
}

/// Calls `on_change` from a thread of its own when the audio graph changes:
/// `wpctl status` is re-read on every burst of `pw-mon` events (a burst
/// settles for at most 300 ms, and only nodes, devices and metadata count,
/// see `MonitorFilter`); only when `pw-mon` is missing or ended, or PipeWire
/// did not answer, is it re-read every 2 s instead (see `next_wait`). A
/// change of the parsed status, or any such event (a stream's volume is not
/// in the status), calls.
///
/// # Errors
/// `Unavailable` without `wpctl`; `Failed` if a thread cannot start.
pub fn watch(on_change: impl Fn() + Send + 'static) -> Result<WatchHandle, AudioError> {
    // Compared parsed: the raw text names the `wpctl` client itself.
    let mut last = parse_status(&run("wpctl", &["status"])?);
    let stopped = Arc::new(AtomicBool::new(false));
    let (tick, ticks) = mpsc::channel::<()>();

    let mut monitor: Option<Monitor> = None;
    if let Ok(mut child) = Command::new("pw-mon")
        .arg("--no-colors")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        let out = child.stdout.take();
        let shared: Monitor = Arc::new(Mutex::new(Some(child)));
        MONITORS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::clone(&shared));
        let flag = Arc::clone(&stopped);
        let reader = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("cuprita-pw-mon".to_owned())
            .spawn(move || {
                use std::io::BufRead;
                if let Some(out) = out {
                    let mut filter = MonitorFilter::default();
                    for line in std::io::BufReader::new(out).lines() {
                        let Ok(line) = line else { break };
                        if flag.load(Ordering::Relaxed) {
                            break;
                        }
                        if filter.feed(&line) && tick.send(()).is_err() {
                            break;
                        }
                    }
                }
                stop(&reader);
            })
            .map_err(|e| AudioError::Failed(e.to_string()))?;
        monitor = Some(shared);
    }

    let flag = Arc::clone(&stopped);
    let mut monitor_alive = monitor.is_some();
    let mut pipewire_present = true;
    std::thread::Builder::new()
        .name("cuprita-audio-watch".to_owned())
        .spawn(move || loop {
            let woken = match next_wait(monitor_alive, pipewire_present) {
                // `pw-mon` reports every change: sleep until it speaks.
                None => match ticks.recv() {
                    Ok(()) => {
                        settle(&ticks);
                        true
                    }
                    Err(_) => {
                        // `pw-mon` ended: read once now, then poll.
                        monitor_alive = false;
                        false
                    }
                },
                Some(wait) if monitor_alive => match ticks.recv_timeout(wait) {
                    Ok(()) => {
                        settle(&ticks);
                        true
                    }
                    Err(RecvTimeoutError::Timeout) => false,
                    Err(RecvTimeoutError::Disconnected) => {
                        monitor_alive = false;
                        false
                    }
                },
                Some(wait) => {
                    std::thread::sleep(wait);
                    false
                }
            };
            if flag.load(Ordering::Relaxed) {
                return;
            }
            let Ok(now) = run("wpctl", &["status"]).map(|t| parse_status(&t)) else {
                pipewire_present = false;
                continue;
            };
            pipewire_present = true;
            if woken || now != last {
                last = now;
                on_change();
            }
        })
        .map_err(|e| AudioError::Failed(e.to_string()))?;

    Ok(WatchHandle { stopped, monitor })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_classify() {
        assert_eq!(
            fault("Object '9999' not found"),
            AudioError::NotFound("9999".to_owned())
        );
        assert_eq!(
            fault("Could not connect to PipeWire"),
            AudioError::Unavailable
        );
        // The tool's own English never reaches the notice.
        assert_eq!(fault("odd"), AudioError::service_failed());
        assert_eq!(fault(""), AudioError::service_failed());
        assert!(!fault("Invalid argument").message_es().contains("Invalid"));
    }

    #[test]
    fn the_watcher_polls_only_without_pw_mon_or_pipewire() {
        // `pw-mon` runs and PipeWire answers: only its events wake.
        assert_eq!(next_wait(true, true), None);
        // No `pw-mon` (missing or ended): poll.
        assert_eq!(next_wait(false, true), Some(POLL));
        // PipeWire did not answer: retry every 2 s, `pw-mon` or not.
        assert_eq!(next_wait(true, false), Some(POLL));
        assert_eq!(next_wait(false, false), Some(POLL));
    }

    fn events(filter: &mut MonitorFilter, text: &str) -> usize {
        text.lines().filter(|l| filter.feed(l)).count()
    }

    #[test]
    fn the_monitor_counts_nodes_not_clients() {
        let mut filter = MonitorFilter::default();
        // The first dump only learns the objects.
        let dump = "added:\n\tid: 46\n\ttype: PipeWire:Interface:Node (version 3)\n\
                    added:\n\tid: 90\n\ttype: PipeWire:Interface:Client (version 3)\n";
        assert_eq!(events(&mut filter, dump), 0);
        let client = "changed:\n\tid: 90\n\tpermissions: rwxm-\n\ttype: PipeWire:Interface:Client (version 3)\n";
        assert_eq!(events(&mut filter, client), 0);
        let node = "changed:\n\tid: 46\n\ttype: PipeWire:Interface:Node (version 3)\n";
        assert_eq!(events(&mut filter, node), 1);
        let new_stream = "added:\n\tid: 140\n\ttype: PipeWire:Interface:Node (version 3)\n";
        assert_eq!(events(&mut filter, new_stream), 1);
        assert_eq!(events(&mut filter, "removed:\n\tid: 90\n"), 0);
        assert_eq!(events(&mut filter, "removed:\n\tid: 140\n"), 1);
    }

    #[test]
    fn a_stalled_tool_is_killed_at_the_deadline() {
        let started = Instant::now();
        let result = run_within("sh", &["-c", "sleep 5"], Duration::from_millis(200));
        assert_eq!(result, Err(AudioError::Unavailable));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_failing_tool_reads_its_last_stderr_line() {
        let result = run_within(
            "sh",
            &[
                "-c",
                "echo 'W: noise' >&2; echo \"Object '7' not found\" >&2; exit 3",
            ],
            DEADLINE,
        );
        assert_eq!(result, Err(AudioError::NotFound("7".to_owned())));
        assert_eq!(
            run_within("sh", &["-c", "echo ok"], DEADLINE),
            Ok("ok\n".to_owned())
        );
        assert_eq!(
            run_within("no-such-tool-cuprita", &[], DEADLINE),
            Err(AudioError::Unavailable)
        );
    }

    #[test]
    fn a_dropped_watcher_kills_its_monitor() {
        // Needs a live session; without one there is nothing to watch.
        let Ok(handle) = watch(|| {}) else { return };
        let Some(pid) = handle.monitor_pid() else {
            return;
        };
        let proc_path = format!("/proc/{pid}");
        assert!(std::path::Path::new(&proc_path).exists());
        drop(handle);
        assert!(!std::path::Path::new(&proc_path).exists());
    }

    #[test]
    fn links_name_the_peer_nodes() {
        let text = "  73 alsa_output.usb:playback_FL\n 132   |<-  117 speech:output_FL\n\
                    117 speech:output_FL\n 132   |->   73 alsa_output.usb:playback_FL\n";
        let links = parse_links(text);
        assert_eq!(links.get(&117), Some(&vec!["alsa_output.usb".to_owned()]));
        assert_eq!(links.get(&73), Some(&vec!["speech".to_owned()]));
    }

    #[test]
    fn volume_arg_rounds() {
        assert_eq!(volume_arg(0.654), "0.65");
        assert_eq!(volume_arg(9.0), "1.50");
    }

    #[test]
    fn inspect_pairs() {
        let props = parse_inspect(
            "id 46, type Node\n  * node.name = \"alsa_out\"\n    media.class = \"Audio/Sink\"\n",
        );
        assert_eq!(props.get("node.name").map(String::as_str), Some("alsa_out"));
        assert_eq!(
            props.get("media.class").map(String::as_str),
            Some("Audio/Sink")
        );
    }
}
