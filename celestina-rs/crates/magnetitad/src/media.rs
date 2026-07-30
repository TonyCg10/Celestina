//! Desktop media players, for the phone to see and drive — the
//! phone-drives-the-desktop half of `kdeconnect.mpris`.
//!
//! Shells out to `playerctl`, the standard MPRIS command-line tool, the same
//! best-effort way the daemon uses `wl-paste` for the clipboard: no playerctl
//! simply means the desktop advertises no players, never an error. Reading
//! `org.mpris.MediaPlayer2` off the bus directly would buy nothing one small,
//! already-packaged tool does not already do.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use magnetita_core::{MprisRequest, PlayerState};
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use rustix::pipe::{pipe_with, PipeFlags};
use rustix::process::{kill_process_group, Pid, Signal};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const OUTPUT_READ_CHUNK: usize = 8 * 1024;
const WORK_QUEUE: usize = 16;

pub enum Reply {
    Players(Vec<String>),
    State(PlayerState),
}

/// One bounded worker owns every `playerctl` subprocess. A slow or malicious
/// desktop player can delay this worker for at most [`COMMAND_TIMEOUT`], never
/// the phone's link pump, and request bursts cannot create unbounded threads.
pub struct Worker {
    requests: Option<SyncSender<MprisRequest>>,
    replies: Receiver<Reply>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Match the worker lifetime to the live plugin state. Deactivation drops and
/// cancels it synchronously; activation creates at most one owned thread.
pub fn set_active(worker: &mut Option<Worker>, active: bool) -> io::Result<()> {
    if active && worker.is_none() {
        *worker = Some(Worker::new()?);
    } else if !active {
        *worker = None;
    }
    Ok(())
}

impl Worker {
    pub fn new() -> io::Result<Self> {
        let (request_tx, request_rx) = mpsc::sync_channel::<MprisRequest>(WORK_QUEUE);
        let (reply_tx, reply_rx) = mpsc::sync_channel::<Reply>(WORK_QUEUE);
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        let thread = std::thread::Builder::new()
            .name("magnetita-mpris".to_owned())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    if worker_stopping.load(Ordering::Acquire) {
                        break;
                    }
                    run_request(request, &reply_tx, &worker_stopping);
                }
            })?;
        Ok(Self {
            requests: Some(request_tx),
            replies: reply_rx,
            stopping,
            thread: Some(thread),
        })
    }

    pub fn submit(&self, request: MprisRequest) {
        let Some(requests) = &self.requests else {
            return;
        };
        match requests.try_send(request) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn try_reply(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Cancel even the subprocess already running. Queued work belongs to
        // the closing or disabled link and is never drained during teardown.
        self.stopping.store(true, Ordering::Release);
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_request(request: MprisRequest, replies: &SyncSender<Reply>, stopping: &AtomicBool) {
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    if request.request_player_list {
        let _ = replies.try_send(Reply::Players(players(deadline, stopping)));
    }
    let Some(player) = request.player else {
        return;
    };
    let wants_state = request.action.is_some() || request.request_now_playing;
    if let Some(action) = request.action {
        control(&player, action.as_str(), deadline, stopping);
    }
    if let Some(volume) = request.set_volume {
        set_volume(&player, volume, deadline, stopping);
    }
    if wants_state && !cancelled(stopping, deadline) {
        if let Some(state) = state(&player, deadline, stopping) {
            let _ = replies.try_send(Reply::State(state));
        }
    }
}

/// The desktop's MPRIS players, in playerctl's order (most-recently-active
/// first). Empty when playerctl is absent or nothing is playing.
fn players(deadline: Instant, stopping: &AtomicBool) -> Vec<String> {
    let Some(output) = command_output(&["--list-all"], deadline, stopping) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One player's now-playing state, or `None` if playerctl cannot read it.
fn state(player: &str, deadline: Instant, stopping: &AtomicBool) -> Option<PlayerState> {
    // One metadata call yields every field, tab-separated, in a single spawn.
    let format = "{{title}}\t{{artist}}\t{{album}}\t{{mpris:length}}\t{{status}}\t{{volume}}";
    let output = command_output(
        &["--player", player, "metadata", "--format", format],
        deadline,
        stopping,
    )?;
    Some(parse_state(player, &String::from_utf8_lossy(&output)))
}

/// Run a KDE Connect transport verb on a desktop player. Best-effort; an unknown
/// verb is a no-op rather than an error.
fn control(player: &str, action: &str, deadline: Instant, stopping: &AtomicBool) {
    let subcommand = match action {
        "Play" => "play",
        "Pause" => "pause",
        "PlayPause" => "play-pause",
        "Stop" => "stop",
        "Next" => "next",
        "Previous" => "previous",
        _ => return,
    };
    command_status(&["--player", player, subcommand], deadline, stopping);
}

/// Set a desktop player's volume (0–100). Best-effort.
fn set_volume(player: &str, volume: i32, deadline: Instant, stopping: &AtomicBool) {
    let level = f64::from(volume.clamp(0, 100)) / 100.0;
    command_status(
        &["--player", player, "volume", &format!("{level:.2}")],
        deadline,
        stopping,
    );
}

fn command_output(args: &[&str], deadline: Instant, stopping: &AtomicBool) -> Option<Vec<u8>> {
    command_output_from("playerctl", args, deadline, stopping)
}

fn command_output_from(
    program: &str,
    args: &[&str],
    deadline: Instant,
    stopping: &AtomicBool,
) -> Option<Vec<u8>> {
    if cancelled(stopping, deadline) {
        return None;
    }
    let (mut output_reader, stdout) = output_pipe().ok()?;
    let (mut child, group) = spawn_grouped(program, args, stdout).ok()?;
    let (status, output) =
        wait_with_output(&mut child, group, &mut output_reader, deadline, stopping)?;
    status.success().then_some(output)
}

fn command_status(args: &[&str], deadline: Instant, stopping: &AtomicBool) {
    if cancelled(stopping, deadline) {
        return;
    }
    let Ok((mut child, group)) = spawn_grouped("playerctl", args, Stdio::null()) else {
        return;
    };
    let _ = wait_bounded(&mut child, group, deadline, stopping);
}

fn output_pipe() -> io::Result<(File, Stdio)> {
    let (read_end, write_end) = pipe_with(PipeFlags::CLOEXEC)?;
    let flags = fcntl_getfl(&read_end)?;
    fcntl_setfl(&read_end, flags | OFlags::NONBLOCK)?;
    Ok((File::from(read_end), Stdio::from(write_end)))
}

fn spawn_grouped(program: &str, args: &[&str], stdout: Stdio) -> io::Result<(Child, Pid)> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null())
        .process_group(0);
    let child = command.spawn()?;
    let group = Pid::from_child(&child);
    Ok((child, group))
}

fn wait_with_output(
    child: &mut Child,
    group: Pid,
    reader: &mut File,
    deadline: Instant,
    stopping: &AtomicBool,
) -> Option<(ExitStatus, Vec<u8>)> {
    let mut output = Vec::new();
    loop {
        if cancelled(stopping, deadline) {
            terminate_group_and_reap(child, group);
            return None;
        }
        if drain_available(reader, &mut output).is_err() {
            terminate_group_and_reap(child, group);
            return None;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                terminate_group(group);
                if drain_available(reader, &mut output).is_err() {
                    return None;
                }
                return Some((status, output));
            }
            Ok(None) => std::thread::sleep(poll_delay(deadline)),
            Err(_) => {
                terminate_group_and_reap(child, group);
                return None;
            }
        }
    }
}

fn drain_available(reader: &mut File, output: &mut Vec<u8>) -> io::Result<()> {
    let mut chunk = [0_u8; OUTPUT_READ_CHUNK];
    loop {
        let remaining = MAX_OUTPUT_BYTES.saturating_sub(output.len());
        let read_limit = (remaining + 1).min(chunk.len());
        match reader.read(&mut chunk[..read_limit]) {
            Ok(0) => return Ok(()),
            Ok(read) if read > remaining => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "playerctl output exceeded the bounded capture size",
                ));
            }
            Ok(read) => output.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn wait_bounded(
    child: &mut Child,
    group: Pid,
    deadline: Instant,
    stopping: &AtomicBool,
) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                terminate_group(group);
                return Some(status);
            }
            Ok(None) if !cancelled(stopping, deadline) => {
                std::thread::sleep(poll_delay(deadline));
            }
            Ok(None) => {
                terminate_group_and_reap(child, group);
                return None;
            }
            Err(_) => {
                terminate_group_and_reap(child, group);
                return None;
            }
        }
    }
}

fn terminate_group(group: Pid) {
    let _ = kill_process_group(group, Signal::KILL);
}

fn terminate_group_and_reap(child: &mut Child, group: Pid) {
    terminate_group(group);
    let _ = child.kill();
    let _ = child.wait();
}

fn poll_delay(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(10))
}

fn cancelled(stopping: &AtomicBool, deadline: Instant) -> bool {
    stopping.load(Ordering::Acquire) || Instant::now() >= deadline
}

/// Turn one playerctl metadata line into a [`PlayerState`]. Pure, so the field
/// unit conversions (µs → ms, 0–1 → 0–100) are testable without playerctl. We
/// do not report `pos`: it moves every tick and the phone's widget only needs it
/// for a seek bar we do not drive, so it stays unknown.
fn parse_state(player: &str, line: &str) -> PlayerState {
    let mut fields = line.trim_end_matches(['\r', '\n']).split('\t');
    let title = fields.next().unwrap_or_default().to_owned();
    let artist = fields.next().unwrap_or_default().to_owned();
    let album = fields.next().unwrap_or_default().to_owned();
    let length_us = fields
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1);
    let status = fields.next().unwrap_or_default().trim();
    let volume_unit: f64 = fields
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1.0);

    let now_playing = match (artist.is_empty(), title.is_empty()) {
        (_, true) => String::new(),
        (true, false) => title.clone(),
        (false, false) => format!("{artist} - {title}"),
    };

    PlayerState {
        player: player.to_owned(),
        title,
        artist,
        album,
        album_art_url: String::new(),
        is_playing: status.eq_ignore_ascii_case("Playing"),
        // playerctl controls generic players; report the transport as available
        // and let the player itself no-op what it cannot do.
        can_pause: true,
        can_play: true,
        can_go_next: true,
        can_go_previous: true,
        can_seek: false,
        length: if length_us >= 0 { length_us / 1000 } else { -1 },
        pos: -1,
        volume: if volume_unit >= 0.0 {
            (volume_unit.clamp(0.0, 1.0) * 100.0).round() as i32
        } else {
            -1
        },
        now_playing,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Stdio;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::{command_output_from, parse_state, spawn_grouped, wait_bounded, Worker};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn an_idle_worker_has_owned_joinable_lifetime() {
        let worker = Worker::new().unwrap();
        drop(worker);
    }

    #[test]
    fn completing_a_command_kills_descendants_that_inherited_stdout() {
        let stopping = AtomicBool::new(false);
        let marker = std::env::temp_dir().join(format!(
            "magnetita-media-descendant-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let marker_arg = marker.to_string_lossy().into_owned();
        let _ = fs::remove_file(&marker);
        let started = Instant::now();
        let output = command_output_from(
            "sh",
            &[
                "-c",
                "(sleep 1; : > \"$1\") & printf inherited",
                "sh",
                &marker_arg,
            ],
            Instant::now() + Duration::from_secs(2),
            &stopping,
        )
        .unwrap();

        assert_eq!(output, b"inherited");
        assert!(started.elapsed() < Duration::from_millis(750));
        std::thread::sleep(Duration::from_millis(1_100));
        let descendant_completed = marker.exists();
        let _ = fs::remove_file(marker);
        assert!(!descendant_completed);
    }

    #[test]
    fn output_above_the_memory_bound_terminates_the_command() {
        let stopping = AtomicBool::new(false);
        let started = Instant::now();
        let output = command_output_from(
            "sh",
            &[
                "-c",
                "i=0; while [ \"$i\" -lt 5000 ]; do printf 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef; i=$((i + 1)); done",
            ],
            Instant::now() + Duration::from_secs(2),
            &stopping,
        );

        assert!(output.is_none());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_timed_out_child_is_killed_and_reaped() {
        let stopping = AtomicBool::new(false);
        let (mut child, group) = spawn_grouped("sh", &["-c", "sleep 5"], Stdio::null()).unwrap();

        assert!(wait_bounded(
            &mut child,
            group,
            Instant::now() + Duration::from_millis(30),
            &stopping,
        )
        .is_none());
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn a_full_metadata_line_parses_with_unit_conversions() {
        // length in µs → ms; volume 0–1 → 0–100; status → is_playing.
        let line = "Song\tBand\tLP\t210000000\tPlaying\t0.8\n";
        let state = parse_state("Spotify", line);
        assert_eq!(state.player, "Spotify");
        assert_eq!(state.title, "Song");
        assert_eq!(state.artist, "Band");
        assert_eq!(state.album, "LP");
        assert!(state.is_playing);
        assert_eq!(state.length, 210_000); // 210 s in ms
        assert_eq!(state.volume, 80);
        assert_eq!(state.now_playing, "Band - Song");
    }

    #[test]
    fn a_paused_player_is_not_playing() {
        let state = parse_state("mpv", "T\tA\t\t-1\tPaused\t1.0");
        assert!(!state.is_playing);
        assert_eq!(state.length, -1);
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn a_track_without_an_artist_now_plays_just_the_title() {
        let state = parse_state("Firefox", "A tab\t\t\t-1\tPlaying\t-1");
        assert_eq!(state.now_playing, "A tab");
        assert_eq!(state.volume, -1); // unknown volume stays -1
    }

    #[test]
    fn player_volume_is_bounded_to_the_protocol_range() {
        let high = parse_state("mpv", "title\tartist\talbum\t0\tPlaying\t9.5");
        let negative = parse_state("mpv", "title\tartist\talbum\t0\tPlaying\t-0.5");
        assert_eq!(high.volume, 100);
        assert_eq!(negative.volume, -1);
    }
}
