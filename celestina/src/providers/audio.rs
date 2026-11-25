//! The audio worker (CEL-2-C): the default output and input levels, read and
//! written through `wpctl` and observed through `pactl subscribe`, all off the
//! event loop's thread.
//!
//! One named thread behind the [`Worker`] guard takes [`AudioCommand`]s from a
//! bounded queue ([`COMMAND_QUEUE`]; the loop gets a logged refusal beyond it
//! and never waits) and publishes [`AudioEvent`]s as [`Message::Audio`].
//!
//! Observing: a `pactl subscribe` child (served by `pipewire-pulse`) says
//! *that* something changed; `wpctl get-volume` says *what*. Events are
//! settled ([`Timing::settle`], 30 ms by default): a wheel burst is one
//! reading. Child lines are bounded ([`MAX_EVENT_LINE_BYTES`]) and parsed by
//! the pure `parse_pactl_event`; a hostile line is no event. If the child
//! dies the worker polls both devices every [`Timing::poll`] and respawns it
//! with a growing backoff (1 s to 30 s); the child is killed and reaped when
//! the worker stops.
//!
//! A device with no reading publishes `level: None`, never 0 %. Every process
//! sits behind [`Environment`] so tests use fakes: no test runs `wpctl` or
//! `pactl`.
//!
//! The graph (CEL-I-004, design section 4): while a consumer holds the gate
//! ([`AudioCommand::Graph`]) the worker also reads `wpctl status` and each
//! listed stream's level, and publishes an [`AudioGraph`] as
//! [`Message::AudioGraph`] when it differs from the last one published. Its
//! events settle apart from the levels' ([`Timing::graph_settle`]), and a
//! `sink-input` event never moves the default levels. The three graph writes
//! are checked against the last published graph: an id the popup never listed
//! is refused and no process runs.

use std::io::{self, BufRead, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use celestina_shell::audio::{
    parse_pactl_event, parse_wpctl_volume, stepped, AudioEvent as Observed, AudioGraph, AudioLevel,
    Direction, GraphDevice, GraphStream, CEILING_PERCENT, MAX_EVENT_LINE_BYTES, MAX_GRAPH_STREAMS,
    STEP_PERCENT,
};
use celestina_wayland::Sender;
use cuprita_core::model::EndpointKind;
use cuprita_core::wpctl_status::{parse_status, Status};

use super::Worker;
use crate::message::Message;

/// Commands that may wait for the worker; the loop gets a refusal beyond it.
pub const COMMAND_QUEUE: usize = 16;
/// How long one `wpctl` call may run before it is killed.
pub const WPCTL_TIMEOUT: Duration = Duration::from_secs(2);
/// The most output bytes of one `wpctl` call that are read.
const MAX_WPCTL_OUTPUT: u64 = 4096;
/// The most output bytes of `wpctl status` that are read; what comes after is
/// cut and the text read so far is parsed.
pub const MAX_STATUS_BYTES: u64 = 64 * 1024;
/// The most observer lines handled per loop turn, so commands never starve.
const LINES_PER_TURN: usize = 64;
/// Lines the reader thread may hold before it waits for the worker.
const LINE_BACKLOG: usize = 64;

/// Which of the session's two default devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Device {
    /// The default sink (speakers, headphones).
    Output,
    /// The default source (microphone).
    Input,
}

impl Device {
    const ALL: [Device; 2] = [Device::Output, Device::Input];

    fn index(self) -> usize {
        match self {
            Device::Output => 0,
            Device::Input => 1,
        }
    }

    /// The `wpctl` name of the default node.
    fn node(self) -> &'static str {
        match self {
            Device::Output => "@DEFAULT_AUDIO_SINK@",
            Device::Input => "@DEFAULT_AUDIO_SOURCE@",
        }
    }
}

/// What the loop asks of the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCommand {
    /// Read both devices and publish both readings, changed or not (a popup
    /// opening, a duty turning on).
    ReadBoth,
    /// Set the level to `percent` (at most 100, as the keys' `-l 1.0`).
    SetVolume { device: Device, percent: u8 },
    /// Move the level one step; nothing happens without a reading.
    Step {
        device: Device,
        direction: Direction,
    },
    /// Flip the mute.
    ToggleMute { device: Device },
    /// The graph's consumer gate: `true` reads the graph now and keeps it
    /// current, `false` stops reading it.
    Graph(bool),
    /// Make the listed device `id` the default one.
    SetDefault { id: u32 },
    /// Set a listed stream's level to `percent` (at most 100).
    SetStreamVolume { id: u32, percent: u8 },
    /// Flip a listed stream's mute.
    ToggleStreamMute { id: u32 },
}

/// A device's level, as the worker read it. `None` is "no reading" (no such
/// device, a tool that failed), never 0 %.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioEvent {
    pub device: Device,
    pub level: Option<AudioLevel>,
}

/// What the worker publishes: a default device's level, or the graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Published {
    Level(AudioEvent),
    Graph(AudioGraph),
}

/// The worker's timings; tests shrink them.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// A burst of events is read once, this long after its last event.
    pub settle: Duration,
    /// The longest a burst may be postponed.
    pub settle_cap: Duration,
    /// How often both devices are read while the observer is down.
    pub poll: Duration,
    /// The first wait before respawning the observer.
    pub backoff_initial: Duration,
    /// The longest wait between respawns.
    pub backoff_max: Duration,
    /// How long one loop turn waits for news.
    pub tick: Duration,
    /// A burst of events or writes is followed by one graph read, this long
    /// after the last one (never postponed past four times this).
    pub graph_settle: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            settle: Duration::from_millis(30),
            settle_cap: Duration::from_millis(150),
            poll: Duration::from_secs(2),
            backoff_initial: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            tick: Duration::from_millis(25),
            graph_settle: Duration::from_millis(250),
        }
    }
}

/// The wait after `current` failed (or ended): doubled, capped at `max`.
#[must_use]
pub fn next_backoff(current: Duration, max: Duration) -> Duration {
    current.saturating_mul(2).min(max)
}

/// One read of the observer child.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObserverRead {
    /// A whole line, without its newline.
    Line(String),
    /// Nothing within the wait.
    Pending,
    /// The child ended.
    Closed,
}

/// The running `pactl subscribe` child, or a fake of it.
pub trait Observer: Send {
    /// The next line, waiting at most `wait`.
    fn next(&mut self, wait: Duration) -> ObserverRead;
    /// Kills and reaps the child. Called once before the observer is dropped.
    fn stop(&mut self);
}

/// The worker's effects: the two tools and the child.
pub trait Environment: Send + 'static {
    /// The device's level, or `None` when `wpctl` has no usable reading.
    fn read(&mut self, device: Device) -> Option<AudioLevel>;

    /// Sets the level to `percent` (already at most 100).
    ///
    /// # Errors
    ///
    /// The tool could not be run or refused.
    fn set_volume(&mut self, device: Device, percent: u8) -> io::Result<()>;

    /// Flips the device's mute.
    ///
    /// # Errors
    ///
    /// The tool could not be run or refused.
    fn toggle_mute(&mut self, device: Device) -> io::Result<()>;

    /// The text of `wpctl status`, at most [`MAX_STATUS_BYTES`] of it, or
    /// `None` when the tool failed.
    fn status(&mut self) -> Option<String>;

    /// The level of node `id`, or `None` when `wpctl` has no usable reading.
    fn read_node(&mut self, id: u32) -> Option<AudioLevel>;

    /// Makes node `id` the default of its kind.
    ///
    /// # Errors
    ///
    /// The tool could not be run or refused.
    fn set_default(&mut self, id: u32) -> io::Result<()>;

    /// Sets node `id`'s level to `percent` (already at most 100).
    ///
    /// # Errors
    ///
    /// The tool could not be run or refused.
    fn set_node_volume(&mut self, id: u32, percent: u8) -> io::Result<()>;

    /// Flips node `id`'s mute.
    ///
    /// # Errors
    ///
    /// The tool could not be run or refused.
    fn toggle_node_mute(&mut self, id: u32) -> io::Result<()>;

    /// Starts the `pactl subscribe` child.
    ///
    /// # Errors
    ///
    /// The child could not be started.
    fn spawn_observer(&mut self) -> io::Result<Box<dyn Observer>>;
}

/// `wpctl get-volume` arguments.
#[must_use]
pub fn read_args(device: Device) -> Vec<String> {
    vec!["get-volume".to_owned(), device.node().to_owned()]
}

/// `wpctl set-volume` arguments: the exact fraction, no floats, limit 1.0.
#[must_use]
pub fn set_volume_args(device: Device, percent: u8) -> Vec<String> {
    volume_args(device.node().to_owned(), percent)
}

fn volume_args(node: String, percent: u8) -> Vec<String> {
    let percent = percent.min(CEILING_PERCENT);
    vec![
        "set-volume".to_owned(),
        node,
        format!("{}.{:02}", percent / 100, percent % 100),
        "-l".to_owned(),
        "1.0".to_owned(),
    ]
}

/// `wpctl status` arguments.
#[must_use]
pub fn status_args() -> Vec<String> {
    vec!["status".to_owned()]
}

/// `wpctl get-volume <id>` arguments.
#[must_use]
pub fn node_read_args(id: u32) -> Vec<String> {
    vec!["get-volume".to_owned(), id.to_string()]
}

/// `wpctl set-default <id>` arguments.
#[must_use]
pub fn set_default_args(id: u32) -> Vec<String> {
    vec!["set-default".to_owned(), id.to_string()]
}

/// `wpctl set-volume <id> <fraction> -l 1.0` arguments, as [`set_volume_args`].
#[must_use]
pub fn set_node_volume_args(id: u32, percent: u8) -> Vec<String> {
    volume_args(id.to_string(), percent)
}

/// `wpctl set-mute <id> toggle` arguments.
#[must_use]
pub fn toggle_node_mute_args(id: u32) -> Vec<String> {
    vec!["set-mute".to_owned(), id.to_string(), "toggle".to_owned()]
}

/// `wpctl set-mute ... toggle` arguments.
#[must_use]
pub fn toggle_mute_args(device: Device) -> Vec<String> {
    vec![
        "set-mute".to_owned(),
        device.node().to_owned(),
        "toggle".to_owned(),
    ]
}

/// The real tools.
#[derive(Debug, Default)]
pub struct System;

impl System {
    /// Runs `wpctl` with a deadline; returns its first bytes of output.
    fn wpctl(args: &[String]) -> io::Result<String> {
        let mut child = Self::spawn_program("wpctl", args)?;
        let deadline = Instant::now() + WPCTL_TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "wpctl timed out"));
            }
            thread::sleep(Duration::from_millis(5));
        };
        let mut text = String::new();
        if let Some(stdout) = child.stdout.take() {
            let mut bytes = Vec::new();
            stdout.take(MAX_WPCTL_OUTPUT).read_to_end(&mut bytes)?;
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        if status.success() {
            Ok(text)
        } else {
            Err(io::Error::other(format!("wpctl exited with {status}")))
        }
    }

    fn spawn_program(program: &str, args: &[String]) -> io::Result<Child> {
        Command::new(program)
            .args(args)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    }

    /// Runs `wpctl` with [`WPCTL_TIMEOUT`], reading at most `cap` bytes.
    fn wpctl_capped(args: &[String], cap: u64) -> io::Result<String> {
        Self::capped("wpctl", args, cap, WPCTL_TIMEOUT)
    }

    /// Runs `program` with a deadline and reads at most `cap` bytes of its
    /// output *while it runs* (a long output must not fill the pipe and
    /// stall the child). Output past the cap is cut: the child is killed and
    /// the text read so far is returned. Every path kills and reaps.
    fn capped(program: &str, args: &[String], cap: u64, timeout: Duration) -> io::Result<String> {
        let mut child = Self::spawn_program(program, args)?;
        let Some(stdout) = child.stdout.take() else {
            reap(&mut child);
            return Err(io::Error::other("the program has no stdout"));
        };
        let reader = thread::Builder::new()
            .name("audio-wpctl-status".to_owned())
            .spawn(move || {
                let mut bytes = Vec::new();
                stdout.take(cap).read_to_end(&mut bytes).map(|_| bytes)
            });
        let reader = match reader {
            Ok(reader) => reader,
            Err(err) => {
                reap(&mut child);
                return Err(err);
            }
        };
        let deadline = Instant::now() + timeout;
        while !reader.is_finished() {
            if Instant::now() >= deadline {
                // Killing the child closes the pipe, which frees the reader.
                reap(&mut child);
                let _ = reader.join();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{program} timed out"),
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        let bytes = match reader.join() {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(err)) => {
                reap(&mut child);
                return Err(err);
            }
            Err(_) => {
                reap(&mut child);
                return Err(io::Error::other(format!("the {program} reader panicked")));
            }
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if u64::try_from(bytes.len()).is_ok_and(|read| read >= cap) {
            tracing::debug!(cap, program, "audio: output cut at the cap");
            reap(&mut child);
            return Ok(text);
        }
        // The output ended, so the child is on its way out.
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(text),
                Ok(Some(status)) => {
                    return Err(io::Error::other(format!("{program} exited with {status}")))
                }
                Ok(None) if Instant::now() < deadline => {}
                Ok(None) => {
                    reap(&mut child);
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("{program} timed out"),
                    ));
                }
                Err(err) => {
                    reap(&mut child);
                    return Err(err);
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Kills and reaps `child`; both may fail when it is already gone.
fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl Environment for System {
    fn read(&mut self, device: Device) -> Option<AudioLevel> {
        match Self::wpctl(&read_args(device)) {
            Ok(text) => parse_wpctl_volume(&text),
            Err(err) => {
                tracing::debug!(%err, ?device, "audio: no reading");
                None
            }
        }
    }

    fn set_volume(&mut self, device: Device, percent: u8) -> io::Result<()> {
        Self::wpctl(&set_volume_args(device, percent)).map(drop)
    }

    fn toggle_mute(&mut self, device: Device) -> io::Result<()> {
        Self::wpctl(&toggle_mute_args(device)).map(drop)
    }

    fn status(&mut self) -> Option<String> {
        match Self::wpctl_capped(&status_args(), MAX_STATUS_BYTES) {
            Ok(text) => Some(text),
            Err(err) => {
                tracing::debug!(%err, "audio: no status");
                None
            }
        }
    }

    fn read_node(&mut self, id: u32) -> Option<AudioLevel> {
        match Self::wpctl(&node_read_args(id)) {
            Ok(text) => parse_wpctl_volume(&text),
            Err(err) => {
                tracing::debug!(%err, id, "audio: no node reading");
                None
            }
        }
    }

    fn set_default(&mut self, id: u32) -> io::Result<()> {
        Self::wpctl(&set_default_args(id)).map(drop)
    }

    fn set_node_volume(&mut self, id: u32, percent: u8) -> io::Result<()> {
        Self::wpctl(&set_node_volume_args(id, percent)).map(drop)
    }

    fn toggle_node_mute(&mut self, id: u32) -> io::Result<()> {
        Self::wpctl(&toggle_node_mute_args(id)).map(drop)
    }

    fn spawn_observer(&mut self) -> io::Result<Box<dyn Observer>> {
        let mut child = Command::new("pactl")
            .arg("subscribe")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("pactl has no stdout"));
        };
        let (lines, received) = mpsc::sync_channel(LINE_BACKLOG);
        let reader = thread::Builder::new()
            .name("audio-pactl-lines".to_owned())
            .spawn(move || {
                pump_lines(io::BufReader::new(stdout), |line| lines.send(line).is_ok());
            });
        match reader {
            Ok(reader) => Ok(Box::new(PactlChild {
                child: Some(child),
                received: Some(received),
                reader: Some(reader),
            })),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(err)
            }
        }
    }
}

/// Splits `reader` into lines of at most [`MAX_EVENT_LINE_BYTES`] and hands
/// each to `emit`, which returns whether to go on. A longer line is skipped to
/// its end, whole, without being kept.
pub fn pump_lines(mut reader: impl BufRead, mut emit: impl FnMut(String) -> bool) {
    let mut line: Vec<u8> = Vec::new();
    let mut skipping = false;
    loop {
        let (consumed, ended) = {
            let buffer = match reader.fill_buf() {
                Ok([]) | Err(_) => return,
                Ok(buffer) => buffer,
            };
            match buffer.iter().position(|byte| *byte == b'\n') {
                Some(at) => {
                    if !skipping {
                        line.extend_from_slice(&buffer[..at]);
                    }
                    (at + 1, true)
                }
                None => {
                    if !skipping {
                        line.extend_from_slice(buffer);
                    }
                    (buffer.len(), false)
                }
            }
        };
        reader.consume(consumed);
        if line.len() > MAX_EVENT_LINE_BYTES {
            skipping = true;
            line.clear();
        }
        if ended {
            if !skipping && !emit(String::from_utf8_lossy(&line).into_owned()) {
                return;
            }
            skipping = false;
            line.clear();
        }
    }
}

/// The real `pactl subscribe` child and the thread that reads its lines.
struct PactlChild {
    child: Option<Child>,
    received: Option<Receiver<String>>,
    reader: Option<JoinHandle<()>>,
}

impl Observer for PactlChild {
    fn next(&mut self, wait: Duration) -> ObserverRead {
        let Some(received) = &self.received else {
            return ObserverRead::Closed;
        };
        match received.recv_timeout(wait) {
            Ok(line) => ObserverRead::Line(line),
            Err(RecvTimeoutError::Timeout) => ObserverRead::Pending,
            Err(RecvTimeoutError::Disconnected) => ObserverRead::Closed,
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Dropping the receiver frees a reader blocked on a full backlog; the
        // dead child's closed pipe ends one blocked on a read.
        self.received = None;
        if let Some(reader) = self.reader.take() {
            if reader.join().is_err() {
                tracing::error!("audio: the pactl reader panicked");
            }
        }
    }
}

impl Drop for PactlChild {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The loop's handle on the worker: the bounded command queue.
#[derive(Debug, Clone)]
pub struct AudioProvider {
    commands: SyncSender<AudioCommand>,
}

impl AudioProvider {
    /// Queues a command for the worker.
    ///
    /// # Errors
    ///
    /// The command back, logged, when [`COMMAND_QUEUE`] commands already wait
    /// or the worker has stopped. It never blocks.
    pub fn request(&self, command: AudioCommand) -> Result<(), AudioCommand> {
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(refused)) => {
                tracing::warn!(?refused, "audio: the command queue is full; refused");
                Err(refused)
            }
            Err(TrySendError::Disconnected(refused)) => {
                tracing::warn!(?refused, "audio: the worker has stopped; refused");
                Err(refused)
            }
        }
    }
}

/// A handle whose commands land in the returned queue, for the tests of the
/// handle's users.
#[cfg(test)]
pub(crate) fn test_handle() -> (AudioProvider, std::sync::mpsc::Receiver<AudioCommand>) {
    let (commands, queue) = std::sync::mpsc::sync_channel(COMMAND_QUEUE);
    (AudioProvider { commands }, queue)
}

/// Starts the worker on the real system, publishing into `sender`.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(AudioProvider, Worker)> {
    spawn_with(System, Timing::default(), shutdown, move |published| {
        let message = match published {
            Published::Level(event) => Message::Audio(event),
            Published::Graph(graph) => Message::AudioGraph(graph),
        };
        sender.send(message).is_ok()
    })
}

/// [`spawn`] with its effects, timings and sink given; `publish` returns
/// whether the message was accepted (false ends the worker).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_with(
    environment: impl Environment,
    timing: Timing,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(Published) -> bool + Send + 'static,
) -> io::Result<(AudioProvider, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("audio", shutdown, move || {
        run(environment, timing, &queue, &flag, publish);
    })?;
    Ok((AudioProvider { commands }, worker))
}

/// The observer's state: running, or down with the next respawn due.
struct Watch {
    observer: Option<Box<dyn Observer>>,
    /// When the next respawn is attempted while down.
    respawn_at: Instant,
    backoff: Duration,
    /// When both devices are next read while down.
    poll_at: Instant,
}

/// Events seen and not yet read: which devices, and the settle deadline.
#[derive(Default)]
struct Pending {
    dirty: [bool; 2],
    first: Option<Instant>,
    due: Option<Instant>,
}

/// A graph read waiting for its burst to settle.
#[derive(Clone, Copy)]
struct GraphDue {
    first: Instant,
    due: Instant,
}

struct Engine<E, P> {
    environment: E,
    publish: P,
    timing: Timing,
    /// The last published reading per device (outer `None`: never published).
    last: [Option<Option<AudioLevel>>; 2],
    /// Whether a consumer holds the graph gate.
    graph_on: bool,
    /// The last graph published; writes are checked against it. `None`
    /// before the first one and after the gate closes: every write is refused.
    graph: Option<AudioGraph>,
    graph_due: Option<GraphDue>,
    /// Whether the last read had no usable status (logged on the way in).
    status_unusable: bool,
}

impl<E: Environment, P: FnMut(Published) -> bool> Engine<E, P> {
    /// Marks the graph for a read after [`Timing::graph_settle`], if the gate
    /// is open.
    fn note_graph(&mut self, now: Instant) {
        if !self.graph_on {
            return;
        }
        let first = self.graph_due.map_or(now, |pending| pending.first);
        let settle = self.timing.graph_settle;
        let due = (now + settle).min(first + settle.saturating_mul(4));
        self.graph_due = Some(GraphDue { first, due });
    }

    fn graph_ready(&self) -> bool {
        self.graph_due
            .is_some_and(|pending| Instant::now() >= pending.due)
    }

    /// Reads the graph and publishes it when it differs from the last one
    /// published (or always when `force`). False: the loop is gone.
    fn refresh_graph(&mut self, force: bool) -> bool {
        self.graph_due = None;
        let text = self.environment.status();
        let status = text.as_deref().map(parse_status);
        let unusable = status
            .as_ref()
            .is_none_or(|status| status.endpoints.is_empty());
        if unusable && !self.status_unusable {
            // Once on the way in, not on every re-read while it lasts.
            tracing::warn!(
                failed = text.is_none(),
                "audio: wpctl status gave no devices; the graph is empty"
            );
        }
        self.status_unusable = unusable;
        let graph = match status {
            Some(status) => {
                let environment = &mut self.environment;
                graph_of(&status, |id| environment.read_node(id))
            }
            None => AudioGraph::default(),
        };
        if !force && self.graph.as_ref() == Some(&graph) {
            return true;
        }
        self.graph = Some(graph.clone());
        (self.publish)(Published::Graph(graph))
    }

    /// Runs `write` for node `id` when the last published graph lists it
    /// (`listed`), then marks the graph for a read; otherwise logs the
    /// refusal and runs nothing.
    fn graph_write(
        &mut self,
        what: &str,
        id: u32,
        listed: fn(&AudioGraph, u32) -> bool,
        write: impl FnOnce(&mut E) -> io::Result<()>,
    ) {
        if !self.graph.as_ref().is_some_and(|graph| listed(graph, id)) {
            tracing::debug!(id, what, "audio: refused, the id is not in the graph");
            return;
        }
        if let Err(err) = write(&mut self.environment) {
            tracing::warn!(%err, id, what, "audio: the graph write failed");
        }
        self.note_graph(Instant::now());
    }

    /// Reads `device` and publishes it when it differs from the last
    /// publication (or always when `force`). False: the loop is gone.
    fn refresh(&mut self, device: Device, force: bool) -> bool {
        let level = self.environment.read(device);
        let slot = &mut self.last[device.index()];
        if !force && *slot == Some(level) {
            return true;
        }
        *slot = Some(level);
        (self.publish)(Published::Level(AudioEvent { device, level }))
    }

    fn command(&mut self, command: AudioCommand) -> bool {
        match command {
            AudioCommand::ReadBoth => {
                for device in Device::ALL {
                    if !self.refresh(device, true) {
                        return false;
                    }
                }
                true
            }
            AudioCommand::SetVolume { device, percent } => {
                let percent = percent.min(CEILING_PERCENT);
                if let Err(err) = self.environment.set_volume(device, percent) {
                    tracing::warn!(%err, ?device, "audio: cannot set the volume");
                }
                self.refresh(device, false)
            }
            AudioCommand::Step { device, direction } => {
                let Some(current) = self.environment.read(device) else {
                    tracing::debug!(?device, "audio: no reading to step from");
                    return self.refresh(device, false);
                };
                let target = stepped(current.percent, direction, STEP_PERCENT);
                if target != current.percent {
                    if let Err(err) = self.environment.set_volume(device, target) {
                        tracing::warn!(%err, ?device, "audio: cannot step the volume");
                    }
                }
                self.refresh(device, false)
            }
            AudioCommand::ToggleMute { device } => {
                if let Err(err) = self.environment.toggle_mute(device) {
                    tracing::warn!(%err, ?device, "audio: cannot toggle the mute");
                }
                self.refresh(device, false)
            }
            AudioCommand::Graph(on) => {
                let opening = on && !self.graph_on;
                self.graph_on = on;
                if on {
                    // A consumer that just arrived has no graph yet.
                    self.refresh_graph(opening)
                } else {
                    self.graph = None;
                    self.graph_due = None;
                    true
                }
            }
            AudioCommand::SetDefault { id } => {
                self.graph_write("set-default", id, AudioGraph::has_device, |env| {
                    env.set_default(id)
                });
                true
            }
            AudioCommand::SetStreamVolume { id, percent } => {
                let percent = percent.min(CEILING_PERCENT);
                self.graph_write("stream-volume", id, AudioGraph::has_stream, |env| {
                    env.set_node_volume(id, percent)
                });
                true
            }
            AudioCommand::ToggleStreamMute { id } => {
                self.graph_write("stream-mute", id, AudioGraph::has_stream, |env| {
                    env.toggle_node_mute(id)
                });
                true
            }
        }
    }
}

/// The graph `status` describes: outputs and inputs by their description,
/// and the playback streams whose endpoint is a listed output. Each stream's
/// level is `read` (a process: only the streams that are kept are asked).
/// Rows past the caps are dropped and the drop is logged.
#[must_use]
pub fn graph_of(status: &Status, mut read: impl FnMut(u32) -> Option<AudioLevel>) -> AudioGraph {
    let devices = |kind: EndpointKind| -> Vec<GraphDevice> {
        status
            .endpoints
            .iter()
            .filter(|endpoint| endpoint.kind == kind)
            .map(|endpoint| GraphDevice {
                id: endpoint.id,
                name: endpoint.description.clone(),
                default: endpoint.default,
            })
            .collect()
    };
    let outputs = devices(EndpointKind::Sink);
    let inputs = devices(EndpointKind::Source);
    // Endpoint 0 is "unknown" for the parser, never a device.
    let on_outputs: Vec<_> = status
        .streams
        .iter()
        .filter(|stream| {
            stream.endpoint != 0 && outputs.iter().any(|output| output.id == stream.endpoint)
        })
        .collect();
    let streams = on_outputs
        .iter()
        .take(MAX_GRAPH_STREAMS)
        .map(|stream| GraphStream {
            id: stream.id,
            name: stream.app_name.clone(),
            level: read(stream.id),
            playing: status.playing.contains(&stream.id),
        })
        .collect();
    let listed = outputs.len() + inputs.len() + on_outputs.len();
    let graph = AudioGraph {
        outputs,
        inputs,
        streams,
    }
    .bounded();
    let kept = graph.outputs.len() + graph.inputs.len() + graph.streams.len();
    if kept < listed {
        tracing::debug!(dropped = listed - kept, "audio: rows past the caps dropped");
    }
    graph
}

fn mark(pending: &mut Pending, event: Observed, now: Instant, timing: &Timing) {
    match event {
        Observed::Sink => pending.dirty[Device::Output.index()] = true,
        Observed::Source => pending.dirty[Device::Input.index()] = true,
        Observed::Server => pending.dirty = [true, true],
        // A stream moves the graph only (see `Engine::note_graph`), never a
        // default device's level.
        Observed::Stream => return,
    }
    let first = *pending.first.get_or_insert(now);
    pending.due = Some((now + timing.settle).min(first + timing.settle_cap));
}

fn run(
    environment: impl Environment,
    timing: Timing,
    queue: &Receiver<AudioCommand>,
    shutdown: &AtomicBool,
    publish: impl FnMut(Published) -> bool,
) {
    let mut worker = Engine {
        environment,
        publish,
        timing,
        last: [None, None],
        graph_on: false,
        graph: None,
        graph_due: None,
        status_unusable: false,
    };
    let now = Instant::now();
    let mut watch = Watch {
        observer: None,
        respawn_at: now,
        backoff: timing.backoff_initial,
        poll_at: now + timing.poll,
    };
    let mut pending = Pending::default();

    // The baseline: the first reading of both devices.
    if worker.command(AudioCommand::ReadBoth) {
        turns(&mut worker, &mut watch, &mut pending, queue, shutdown);
    }
    if let Some(mut observer) = watch.observer.take() {
        observer.stop();
    }
}

fn turns<E: Environment, P: FnMut(Published) -> bool>(
    worker: &mut Engine<E, P>,
    watch: &mut Watch,
    pending: &mut Pending,
    queue: &Receiver<AudioCommand>,
    shutdown: &AtomicBool,
) {
    let timing = worker.timing;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }

        // Commands first: a key press never waits behind the observer.
        for _ in 0..COMMAND_QUEUE {
            match queue.try_recv() {
                Ok(command) => {
                    if !worker.command(command) {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        let now = Instant::now();
        if let Some(observer) = watch.observer.as_mut() {
            let mut wait = timing.tick;
            for _ in 0..LINES_PER_TURN {
                match observer.next(wait) {
                    ObserverRead::Line(line) => {
                        // A line from the child proves it works again.
                        watch.backoff = timing.backoff_initial;
                        if let Some(event) = parse_pactl_event(&line) {
                            let seen = Instant::now();
                            mark(pending, event, seen, &timing);
                            worker.note_graph(seen);
                        }
                        wait = Duration::ZERO;
                    }
                    ObserverRead::Pending => break,
                    ObserverRead::Closed => {
                        tracing::warn!("audio: pactl subscribe ended; polling until it returns");
                        if let Some(mut dead) = watch.observer.take() {
                            dead.stop();
                        }
                        let now = Instant::now();
                        watch.respawn_at = now + watch.backoff;
                        watch.backoff = next_backoff(watch.backoff, timing.backoff_max);
                        watch.poll_at = now + timing.poll;
                        break;
                    }
                }
            }
        } else {
            thread::sleep(timing.tick);
            if now >= watch.respawn_at {
                match worker.environment.spawn_observer() {
                    Ok(observer) => {
                        watch.observer = Some(observer);
                        // Whatever changed while it was down.
                        pending.dirty = [true, true];
                        pending.first.get_or_insert(now);
                        pending.due = Some(now);
                        worker.note_graph(now);
                    }
                    Err(err) => {
                        tracing::warn!(%err, "audio: cannot start pactl subscribe");
                        watch.respawn_at = Instant::now() + watch.backoff;
                        watch.backoff = next_backoff(watch.backoff, timing.backoff_max);
                    }
                }
            }
            if watch.observer.is_none() && Instant::now() >= watch.poll_at {
                watch.poll_at = Instant::now() + timing.poll;
                for device in Device::ALL {
                    if !worker.refresh(device, false) {
                        return;
                    }
                }
                if worker.graph_on && !worker.refresh_graph(false) {
                    return;
                }
            }
        }

        // A settled burst is one reading per device it touched.
        if pending.due.is_some_and(|due| Instant::now() >= due) {
            let dirty = pending.dirty;
            *pending = Pending::default();
            for device in Device::ALL {
                if dirty[device.index()] && !worker.refresh(device, false) {
                    return;
                }
            }
        }

        // A settled graph burst is one graph read, apart from the levels.
        if worker.graph_ready() && !worker.refresh_graph(false) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::audio::MAX_GRAPH_DEVICES;
    use cuprita_core::model::{AudioEndpoint, AudioStream};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn level(percent: u8) -> Option<AudioLevel> {
        Some(AudioLevel {
            percent,
            muted: false,
        })
    }

    fn fast() -> Timing {
        Timing {
            settle: Duration::from_millis(20),
            settle_cap: Duration::from_millis(100),
            poll: Duration::from_millis(30),
            backoff_initial: Duration::from_millis(10),
            backoff_max: Duration::from_millis(40),
            tick: Duration::from_millis(3),
            graph_settle: Duration::from_millis(40),
        }
    }

    #[derive(Default)]
    struct Fake {
        levels: [Option<AudioLevel>; 2],
        reads: [usize; 2],
        /// The scripted `wpctl status` text; `None` is a failed call.
        status: Option<String>,
        status_reads: usize,
        /// Per-node levels for `read_node`, and the ids it was asked for.
        nodes: Vec<(u32, AudioLevel)>,
        node_reads: Vec<u32>,
        writes: Vec<String>,
        spawns: Vec<Instant>,
        /// One entry per spawn attempt; empty means "fail".
        spawn_ok: VecDeque<bool>,
        feed: Option<mpsc::Sender<ObserverRead>>,
        stops: usize,
    }

    #[derive(Clone, Default)]
    struct FakeEnv(Arc<Mutex<Fake>>);

    impl FakeEnv {
        fn with<R>(&self, apply: impl FnOnce(&mut Fake) -> R) -> R {
            apply(&mut self.0.lock().expect("the fake is not poisoned"))
        }
        fn line(&self, text: &str) {
            self.with(|fake| {
                if let Some(feed) = &fake.feed {
                    let _ = feed.send(ObserverRead::Line(text.to_owned()));
                }
            });
        }
        /// The child dies: the feed is dropped.
        fn kill_child(&self) {
            self.with(|fake| fake.feed = None);
        }
    }

    struct FakeObserver {
        env: FakeEnv,
        lines: Receiver<ObserverRead>,
    }

    impl Observer for FakeObserver {
        fn next(&mut self, wait: Duration) -> ObserverRead {
            match self.lines.recv_timeout(wait) {
                Ok(read) => read,
                Err(RecvTimeoutError::Timeout) => ObserverRead::Pending,
                Err(RecvTimeoutError::Disconnected) => ObserverRead::Closed,
            }
        }
        fn stop(&mut self) {
            self.env.with(|fake| fake.stops += 1);
        }
    }

    impl Environment for FakeEnv {
        fn read(&mut self, device: Device) -> Option<AudioLevel> {
            self.with(|fake| {
                fake.reads[device.index()] += 1;
                fake.levels[device.index()]
            })
        }
        fn set_volume(&mut self, device: Device, percent: u8) -> io::Result<()> {
            self.with(|fake| {
                fake.writes.push(format!("set {device:?} {percent}"));
                let muted = fake.levels[device.index()].is_some_and(|l| l.muted);
                fake.levels[device.index()] = Some(AudioLevel { percent, muted });
            });
            Ok(())
        }
        fn toggle_mute(&mut self, device: Device) -> io::Result<()> {
            self.with(|fake| {
                fake.writes.push(format!("mute {device:?}"));
                if let Some(l) = &mut fake.levels[device.index()] {
                    l.muted = !l.muted;
                }
            });
            Ok(())
        }
        fn status(&mut self) -> Option<String> {
            self.with(|fake| {
                fake.status_reads += 1;
                fake.status.clone()
            })
        }
        fn read_node(&mut self, id: u32) -> Option<AudioLevel> {
            self.with(|fake| {
                fake.node_reads.push(id);
                fake.nodes
                    .iter()
                    .find(|(node, _)| *node == id)
                    .map(|(_, l)| *l)
            })
        }
        fn set_default(&mut self, id: u32) -> io::Result<()> {
            self.with(|fake| fake.writes.push(format!("default {id}")));
            Ok(())
        }
        fn set_node_volume(&mut self, id: u32, percent: u8) -> io::Result<()> {
            self.with(|fake| fake.writes.push(format!("node-volume {id} {percent}")));
            Ok(())
        }
        fn toggle_node_mute(&mut self, id: u32) -> io::Result<()> {
            self.with(|fake| fake.writes.push(format!("node-mute {id}")));
            Ok(())
        }
        fn spawn_observer(&mut self) -> io::Result<Box<dyn Observer>> {
            let env = self.clone();
            self.with(|fake| {
                fake.spawns.push(Instant::now());
                if fake.spawn_ok.pop_front().unwrap_or(false) {
                    let (feed, lines) = mpsc::channel();
                    fake.feed = Some(feed);
                    Ok(Box::new(FakeObserver { env, lines }) as Box<dyn Observer>)
                } else {
                    Err(io::Error::other("no pactl"))
                }
            })
        }
    }

    type Events = Arc<Mutex<Vec<AudioEvent>>>;
    type Graphs = Arc<Mutex<Vec<AudioGraph>>>;

    fn start(env: &FakeEnv, shutdown: &Arc<AtomicBool>) -> (AudioProvider, Worker, Events) {
        let (provider, worker, events, _graphs) = start_graphs(env, shutdown);
        (provider, worker, events)
    }

    fn start_graphs(
        env: &FakeEnv,
        shutdown: &Arc<AtomicBool>,
    ) -> (AudioProvider, Worker, Events, Graphs) {
        let events: Events = Arc::default();
        let graphs: Graphs = Arc::default();
        let (levels, shapes) = (Arc::clone(&events), Arc::clone(&graphs));
        let (provider, worker) = spawn_with(env.clone(), fast(), shutdown, move |published| {
            match published {
                Published::Level(event) => levels.lock().expect("events").push(event),
                Published::Graph(graph) => shapes.lock().expect("graphs").push(graph),
            }
            true
        })
        .expect("the worker starts");
        (provider, worker, events, graphs)
    }

    fn graphs_of(graphs: &Graphs) -> Vec<AudioGraph> {
        graphs.lock().expect("graphs").clone()
    }

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "timed out: {what}"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn snapshot(events: &Events) -> Vec<AudioEvent> {
        events.lock().expect("events").clone()
    }

    #[test]
    fn the_arguments_are_exact_fractions_with_the_limit() {
        assert_eq!(
            set_volume_args(Device::Output, 55),
            ["set-volume", "@DEFAULT_AUDIO_SINK@", "0.55", "-l", "1.0"]
        );
        assert_eq!(set_volume_args(Device::Input, 5)[2], "0.05");
        assert_eq!(set_volume_args(Device::Output, 100)[2], "1.00");
        assert_eq!(set_volume_args(Device::Output, 250)[2], "1.00");
        assert_eq!(set_volume_args(Device::Output, 0)[2], "0.00");
        assert_eq!(
            read_args(Device::Input),
            ["get-volume", "@DEFAULT_AUDIO_SOURCE@"]
        );
        assert_eq!(
            toggle_mute_args(Device::Output),
            ["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]
        );
    }

    #[test]
    fn the_first_reading_is_published_for_both_devices() {
        let env = FakeEnv::default();
        env.with(|f| f.levels = [level(40), level(70)]);
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, events) = start(&env, &shutdown);
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        worker.join();
        let events = snapshot(&events);
        assert_eq!(
            events[0],
            AudioEvent {
                device: Device::Output,
                level: level(40)
            }
        );
        assert_eq!(
            events[1],
            AudioEvent {
                device: Device::Input,
                level: level(70)
            }
        );
    }

    #[test]
    fn a_burst_of_events_is_one_reading() {
        let env = FakeEnv::default();
        env.with(|f| {
            f.levels = [level(40), level(70)];
            f.spawn_ok.push_back(true);
        });
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, events) = start(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        // The respawn catch-up reads once more; let it finish first.
        thread::sleep(Duration::from_millis(80));
        let before = env.with(|f| f.reads);

        env.with(|f| f.levels[0] = level(45));
        for _ in 0..10 {
            env.line("Event 'change' on sink #51");
        }
        wait_until("the new level", || {
            snapshot(&events)
                .last()
                .is_some_and(|e| e.level == level(45))
        });
        thread::sleep(Duration::from_millis(80));
        let after = env.with(|f| f.reads);
        worker.join();

        assert_eq!(after[0] - before[0], 1, "ten events, one output reading");
        assert_eq!(after[1], before[1], "the input was not touched");
    }

    #[test]
    fn a_full_queue_refuses_and_never_blocks() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let provider = AudioProvider { commands };
        for _ in 0..COMMAND_QUEUE {
            assert_eq!(provider.request(AudioCommand::ReadBoth), Ok(()));
        }
        let started = Instant::now();
        let refused = AudioCommand::ToggleMute {
            device: Device::Output,
        };
        assert_eq!(provider.request(refused), Err(refused));
        assert!(started.elapsed() < Duration::from_millis(500));
        drop(queue);
        assert_eq!(
            provider.request(refused),
            Err(refused),
            "a stopped worker refuses too"
        );
    }

    #[test]
    fn the_backoff_doubles_up_to_its_cap() {
        let max = Duration::from_secs(30);
        let mut wait = Duration::from_secs(1);
        let mut seen = Vec::new();
        for _ in 0..7 {
            seen.push(wait.as_secs());
            wait = next_backoff(wait, max);
        }
        assert_eq!(seen, [1, 2, 4, 8, 16, 30, 30]);
    }

    #[test]
    fn a_missing_observer_is_retried_with_a_growing_wait() {
        let env = FakeEnv::default();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, _events) = start(&env, &shutdown);
        wait_until("five attempts", || env.with(|f| f.spawns.len() >= 5));
        worker.join();
        let spawns = env.with(|f| f.spawns.clone());
        let gaps: Vec<Duration> = spawns.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps[2] > gaps[0], "{gaps:?}");
        assert!(gaps[2] >= Duration::from_millis(35), "{gaps:?}");
    }

    #[test]
    fn a_dead_child_is_polled_and_then_respawned() {
        let env = FakeEnv::default();
        env.with(|f| {
            f.levels = [level(40), level(70)];
            f.spawn_ok = VecDeque::from([true, false, true]);
        });
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, events) = start(&env, &shutdown);
        wait_until("the first child", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);

        env.kill_child();
        // No line arrives, yet the change is seen by the poll.
        env.with(|f| f.levels[0] = level(10));
        wait_until("the polled reading", || {
            snapshot(&events)
                .last()
                .is_some_and(|e| e.level == level(10))
        });
        assert_eq!(env.with(|f| f.stops), 1, "the dead child was reaped");

        wait_until("the respawn", || env.with(|f| f.feed.is_some()));
        env.with(|f| f.levels[1] = level(20));
        env.line("Event 'change' on source #3");
        wait_until("the new child works", || {
            snapshot(&events)
                .last()
                .is_some_and(|e| e.level == level(20))
        });
        worker.join();
    }

    #[test]
    fn stopping_joins_and_reaps_the_child() {
        let env = FakeEnv::default();
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, _events) = start(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        let started = Instant::now();
        worker.join();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(shutdown.load(Ordering::Acquire));
        assert_eq!(env.with(|f| f.stops), 1);
    }

    #[test]
    fn a_hostile_line_is_ignored() {
        let env = FakeEnv::default();
        env.with(|f| {
            f.levels = [level(40), level(70)];
            f.spawn_ok.push_back(true);
        });
        let shutdown = Arc::new(AtomicBool::new(false));
        let (_provider, worker, events) = start(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        thread::sleep(Duration::from_millis(80));
        let before = env.with(|f| f.reads);

        for line in [
            "Event 'change' on client #5",
            "garbage \u{0} \u{1b}[2J",
            "Event 'change' on sink #-1",
            "",
        ] {
            env.line(line);
        }
        env.line(&format!(
            "Event 'change' on sink #5{}",
            " ".repeat(MAX_EVENT_LINE_BYTES)
        ));
        thread::sleep(Duration::from_millis(120));
        let after = env.with(|f| f.reads);
        worker.join();

        assert_eq!(after, before, "no hostile line caused a reading");
    }

    #[test]
    fn no_reading_is_none_never_zero_percent() {
        let env = FakeEnv::default();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events) = start(&env, &shutdown);
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(
            provider.request(AudioCommand::Step {
                device: Device::Output,
                direction: Direction::Up
            }),
            Ok(())
        );
        assert_eq!(provider.request(AudioCommand::ReadBoth), Ok(()));
        wait_until("the forced reading", || snapshot(&events).len() >= 4);
        worker.join();
        assert!(snapshot(&events).iter().all(|event| event.level.is_none()));
        assert!(
            env.with(|f| f.writes.is_empty()),
            "nothing was written blind"
        );
    }

    #[test]
    fn commands_write_exact_steps_and_report_what_was_read_back() {
        let env = FakeEnv::default();
        env.with(|f| f.levels = [level(50), level(70)]);
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events) = start(&env, &shutdown);
        wait_until("the baseline", || snapshot(&events).len() >= 2);

        for command in [
            AudioCommand::Step {
                device: Device::Output,
                direction: Direction::Up,
            },
            AudioCommand::SetVolume {
                device: Device::Input,
                percent: 30,
            },
            AudioCommand::ToggleMute {
                device: Device::Output,
            },
        ] {
            assert_eq!(provider.request(command), Ok(()));
        }
        wait_until("three read-backs", || snapshot(&events).len() >= 5);
        worker.join();

        assert_eq!(
            env.with(|f| f.writes.clone()),
            ["set Output 55", "set Input 30", "mute Output",]
        );
        let events = snapshot(&events);
        assert_eq!(events[2].level, level(55));
        assert_eq!(events[3].level, level(30));
        assert_eq!(
            events[4].level,
            Some(AudioLevel {
                percent: 55,
                muted: true
            })
        );
    }

    #[test]
    fn lines_are_bounded_and_split() {
        let mut input = Vec::new();
        input.extend_from_slice(b"one\n");
        input.extend_from_slice(&vec![b'x'; MAX_EVENT_LINE_BYTES + 10]);
        input.extend_from_slice(b"\ntwo\npartial");
        let mut seen = Vec::new();
        pump_lines(io::Cursor::new(input), |line| {
            seen.push(line);
            true
        });
        assert_eq!(seen, ["one", "two"]);
    }

    #[test]
    fn a_line_exactly_at_the_bound_is_kept_and_emit_can_stop_the_pump() {
        let mut input = vec![b'y'; MAX_EVENT_LINE_BYTES];
        input.extend_from_slice(b"\nnext\n");
        let mut seen = Vec::new();
        pump_lines(io::Cursor::new(input), |line| {
            seen.push(line.len());
            false
        });
        assert_eq!(seen, [MAX_EVENT_LINE_BYTES]);
    }

    /// The shape of the author's `wpctl status`: six sinks, two sources and
    /// three streams, one of them still `[init]` and one a recorder.
    fn status_text() -> String {
        [
            "PipeWire 'pipewire-0' [1.6.9, toni@cachyos, cookie:1200239385]",
            " └─ Clients:",
            "        32. WirePlumber                         [1.6.9, toni@cachyos, pid:1751]",
            "",
            "Audio",
            " ├─ Devices:",
            " │      55. Navi 48 HDMI/DP Audio Controller    [alsa]",
            " │      57. USB2.0 Device                       [alsa]",
            " │  ",
            " ├─ Sinks:",
            " │  *   46. USB2.0 Device Analog Stereo       [vol: 0.00 MUTED]",
            " │      92. Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI) [vol: 0.75]",
            " │      93. Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI 2) [vol: 0.75]",
            " │      60. Ryzen HD Audio Controller Analog Stereo [vol: 0.40]",
            " │      61. Ryzen HD Audio Controller Digital Stereo [vol: 0.40]",
            " │     200. Bluetooth Headset                  [vol: 0.50]",
            " │  ",
            " ├─ Sources:",
            " │  *   34. Brio 100 Mono                       [vol: 0.00 MUTED]",
            " │      45. USB2.0 Device Mono                  [vol: 0.00 MUTED]",
            " │  ",
            " ├─ Filters:",
            " │  ",
            " └─ Streams:",
            "       116. speech-dispatcher-dummy                                     ",
            "            117. output_FL       > USB2.0 Device:playback_FL\t[init]",
            "            118. output_FR       > USB2.0 Device:playback_FR\t[init]",
            "       150. Zen                                                         ",
            "            151. output_FL       > Bluetooth Headset:playback_FL\t[active]",
            "            152. output_FR       > Bluetooth Headset:playback_FR\t[active]",
            "       137. Recorder                                                    ",
            "            138. input_FL        < Brio 100:capture_FL\t[active]",
            "",
            "Settings",
            " └─ Default Configured Devices:",
            "         0. Audio/Sink    bluez_output.5C_D3_3D_49_38_BD.1",
            "",
        ]
        .join("\n")
    }

    fn with_status(env: &FakeEnv) {
        env.with(|f| {
            f.status = Some(status_text());
            f.nodes = vec![
                (
                    116,
                    AudioLevel {
                        percent: 30,
                        muted: false,
                    },
                ),
                (
                    150,
                    AudioLevel {
                        percent: 80,
                        muted: true,
                    },
                ),
            ];
        });
    }

    /// Lets the worker drain its queue and any settle in flight.
    fn quiet() {
        thread::sleep(Duration::from_millis(200));
    }

    #[test]
    fn graph_of_keeps_playback_streams_only() {
        let status = parse_status(&status_text());
        let mut asked = Vec::new();
        let graph = graph_of(&status, |id| {
            asked.push(id);
            level(u8::try_from(id % 100).expect("fits"))
        });
        assert_eq!(graph.outputs.len(), 6);
        assert_eq!(graph.inputs.len(), 2);
        assert_eq!(graph.outputs[0].name, "USB2.0 Device Analog Stereo");
        assert!(graph.outputs[0].default && !graph.outputs[1].default);
        assert!(graph.inputs[0].default);
        let streams: Vec<(u32, &str)> = graph
            .streams
            .iter()
            .map(|s| (s.id, s.name.as_str()))
            .collect();
        assert_eq!(streams, [(116, "speech-dispatcher-dummy"), (150, "Zen")]);
        assert_eq!(asked, [116, 150], "the recorder is never read");
        // Zen has an `[active]` port; speech-dispatcher's is `[init]`.
        let playing: Vec<(u32, bool)> = graph.streams.iter().map(|s| (s.id, s.playing)).collect();
        assert_eq!(playing, [(116, false), (150, true)]);
        assert_eq!(graph.streams[0].level, level(16));
    }

    #[test]
    fn graph_of_bounds_what_it_reads_and_lists() {
        let mut status = Status::default();
        for id in 1..=12 {
            status.endpoints.push(AudioEndpoint {
                id,
                kind: EndpointKind::Sink,
                name: String::new(),
                description: format!("sink {id}"),
                volume: 1.0,
                muted: false,
                default: id == 12,
            });
            status.streams.push(AudioStream {
                id: 100 + id,
                app_name: format!("app {id}"),
                app_icon: String::new(),
                volume: 1.0,
                muted: false,
                endpoint: id,
            });
        }
        let mut asked = 0;
        let graph = graph_of(&status, |_| {
            asked += 1;
            None
        });
        assert_eq!(graph.outputs.len(), MAX_GRAPH_DEVICES);
        assert!(graph.outputs.iter().any(|d| d.id == 12 && d.default));
        assert_eq!(graph.streams.len(), MAX_GRAPH_STREAMS);
        assert_eq!(
            asked, MAX_GRAPH_STREAMS,
            "no process for a row that is dropped"
        );
    }

    #[test]
    fn the_new_arguments_are_exact() {
        assert_eq!(status_args(), ["status"]);
        assert_eq!(node_read_args(116), ["get-volume", "116"]);
        assert_eq!(set_default_args(46), ["set-default", "46"]);
        assert_eq!(
            set_node_volume_args(116, 55),
            ["set-volume", "116", "0.55", "-l", "1.0"]
        );
        assert_eq!(set_node_volume_args(116, 250)[2], "1.00");
        assert_eq!(set_node_volume_args(116, 5)[2], "0.05");
        assert_eq!(toggle_node_mute_args(116), ["set-mute", "116", "toggle"]);
    }

    #[test]
    fn the_graph_is_read_once_when_the_gate_opens_and_never_while_closed() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);

        // Closed: every event kind is ignored by the graph.
        for line in [
            "Event 'change' on sink #46",
            "Event 'change' on source #34",
            "Event 'change' on server",
            "Event 'new' on sink-input #116",
        ] {
            env.line(line);
        }
        quiet();
        assert_eq!(env.with(|f| f.status_reads), 0);
        assert!(graphs_of(&graphs).is_empty());

        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        quiet();
        assert_eq!(env.with(|f| f.status_reads), 1, "one read for the gate");
        assert_eq!(env.with(|f| f.node_reads.clone()), [116, 150]);
        let graph = graphs_of(&graphs).remove(0);
        assert_eq!(
            graph.streams[1].level.map(|l| (l.percent, l.muted)),
            Some((80, true))
        );

        // Closing stops it again.
        assert_eq!(provider.request(AudioCommand::Graph(false)), Ok(()));
        quiet();
        env.line("Event 'change' on sink #46");
        env.line("Event 'change' on sink-input #116");
        quiet();
        worker.join();
        assert_eq!(env.with(|f| f.status_reads), 1);
        assert_eq!(graphs_of(&graphs).len(), 1);
    }

    #[test]
    fn a_burst_of_stream_events_is_one_graph_read() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        quiet();
        let before = env.with(|f| f.status_reads);

        for _ in 0..10 {
            env.line("Event 'change' on sink-input #116");
        }
        env.with(|f| f.status = Some(status_text().replace("Zen", "Zen Browser")));
        wait_until("the new graph", || graphs_of(&graphs).len() >= 2);
        quiet();
        worker.join();
        assert_eq!(env.with(|f| f.status_reads) - before, 1);
    }

    #[test]
    fn stream_events_never_read_the_default_levels() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| {
            f.levels = [level(40), level(70)];
            f.spawn_ok.push_back(true);
        });
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        // The respawn catch-up reads once more; let it finish first.
        quiet();
        let before = env.with(|f| f.reads);

        for _ in 0..5 {
            env.line("Event 'change' on sink-input #116");
        }
        env.with(|f| f.status = Some(status_text().replace("Zen", "Zen Browser")));
        wait_until("the graph moved", || graphs_of(&graphs).len() >= 2);
        quiet();
        worker.join();
        assert_eq!(env.with(|f| f.reads), before);
    }

    #[test]
    fn a_write_to_an_unlisted_id_runs_nothing() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);

        // No graph published yet: every write is refused.
        for command in [
            AudioCommand::SetDefault { id: 46 },
            AudioCommand::SetStreamVolume {
                id: 116,
                percent: 20,
            },
            AudioCommand::ToggleStreamMute { id: 116 },
        ] {
            assert_eq!(provider.request(command), Ok(()));
        }
        quiet();
        assert!(env.with(|f| f.writes.is_empty()), "before any graph");
        assert_eq!(env.with(|f| f.status_reads), 0, "a refusal reads nothing");

        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        quiet();
        let reads = env.with(|f| f.status_reads);
        for command in [
            AudioCommand::SetDefault { id: 999 },
            // A stream is not a device, and a device is not a stream.
            AudioCommand::SetDefault { id: 116 },
            AudioCommand::SetStreamVolume {
                id: 999,
                percent: 20,
            },
            AudioCommand::SetStreamVolume {
                id: 46,
                percent: 20,
            },
            // The recorder is not listed.
            AudioCommand::ToggleStreamMute { id: 137 },
        ] {
            assert_eq!(provider.request(command), Ok(()));
        }
        quiet();
        worker.join();
        assert!(env.with(|f| f.writes.is_empty()));
        assert_eq!(
            env.with(|f| f.status_reads),
            reads,
            "a refusal re-reads nothing"
        );
    }

    #[test]
    fn set_default_and_stream_writes_use_exact_arguments() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        let before = env.with(|f| f.status_reads);

        for command in [
            AudioCommand::SetDefault { id: 92 },
            AudioCommand::SetStreamVolume {
                id: 150,
                percent: 55,
            },
            AudioCommand::SetStreamVolume {
                id: 150,
                percent: 250,
            },
            AudioCommand::ToggleStreamMute { id: 116 },
        ] {
            assert_eq!(provider.request(command), Ok(()));
        }
        env.with(|f| f.status = Some(status_text().replace("Zen", "Zen Browser")));
        wait_until("the read-back graph", || graphs_of(&graphs).len() >= 2);
        quiet();
        worker.join();
        assert_eq!(
            env.with(|f| f.writes.clone()),
            [
                "default 92",
                "node-volume 150 55",
                "node-volume 150 100",
                "node-mute 116"
            ]
        );
        assert_eq!(
            env.with(|f| f.status_reads) - before,
            1,
            "four writes, one read-back after the settle"
        );
    }

    #[test]
    fn an_unreadable_status_is_an_empty_graph() {
        let env = FakeEnv::default();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        worker.join();
        assert_eq!(graphs_of(&graphs), [AudioGraph::default()]);
    }

    #[test]
    fn an_unchanged_graph_is_not_published_twice() {
        let env = FakeEnv::default();
        with_status(&env);
        env.with(|f| f.spawn_ok.push_back(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the observer", || env.with(|f| f.feed.is_some()));
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());
        quiet();
        let before = env.with(|f| f.status_reads);

        env.line("Event 'change' on sink-input #116");
        env.line("Event 'change' on server");
        wait_until("the re-read", || env.with(|f| f.status_reads) > before);
        quiet();
        worker.join();
        assert_eq!(graphs_of(&graphs).len(), 1, "same graph, one message");
    }

    #[test]
    fn a_dead_child_polls_the_graph_too() {
        let env = FakeEnv::default();
        with_status(&env);
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker, events, graphs) = start_graphs(&env, &shutdown);
        wait_until("the baseline", || snapshot(&events).len() >= 2);
        assert_eq!(provider.request(AudioCommand::Graph(true)), Ok(()));
        wait_until("the graph", || !graphs_of(&graphs).is_empty());

        env.with(|f| f.status = Some(status_text().replace("Zen", "Zen Browser")));
        wait_until("the polled graph", || graphs_of(&graphs).len() >= 2);
        worker.join();
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["-c".to_owned(), script.to_owned()]
    }

    #[test]
    fn a_huge_output_is_cut_at_the_cap_without_hanging() {
        let started = Instant::now();
        let text = System::capped(
            "sh",
            &sh("head -c 10000000 /dev/zero"),
            64 * 1024,
            Duration::from_secs(3),
        )
        .expect("the cut text is returned");
        assert_eq!(text.len(), 64 * 1024);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_short_output_is_returned_whole() {
        let text = System::capped(
            "sh",
            &sh("printf 'one\\ntwo\\n'"),
            64 * 1024,
            Duration::from_secs(3),
        )
        .expect("short output");
        assert_eq!(text, "one\ntwo\n");
    }

    #[test]
    fn a_failing_program_is_an_error() {
        let result = System::capped("sh", &sh("echo hi; exit 3"), 1024, Duration::from_secs(3));
        assert!(result.is_err());
    }

    #[test]
    fn a_slow_program_times_out_and_is_reaped() {
        let started = Instant::now();
        let err = System::capped("sleep", &["5".to_owned()], 1024, Duration::from_millis(100))
            .expect_err("it times out");
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "killed, not awaited"
        );
    }

    #[test]
    fn a_missing_program_is_an_error() {
        assert!(
            System::capped("no-such-program-celestina", &[], 16, Duration::from_secs(1)).is_err()
        );
    }
}
