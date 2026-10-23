//! The lock never locks a session by accident (round 5, ADR 0022 amendment A4).
//!
//! On 2026-10-22 a bare `celestina-lock --rehearsal`, run with the
//! `WAYLAND_DISPLAY` of the author's desktop in its environment, connected to
//! the real compositor and locked the real session. The connection guard of
//! `celestina-wayland` now sits in front of the connection: a launch is
//! accepted only from the shell (marker), in a verified nest, or over a
//! socket handed in by a harness whose other end is the harness itself (its
//! parent) or a verified nest (round 6, finding I-A).
//!
//! Every test runs the real binary against a **test** compositor: a Python
//! process of its own that listens on a socket in a scratch runtime directory
//! and counts the bytes it receives. The environment of every child is built
//! from nothing (`env_clear`), so nothing of the desktop session reaches it.
//! No child is ever matched by name; each is killed by its pid.

use std::fs;
use std::os::fd::AsRawFd as _;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The test compositor: listens at argv[1], writes `<accepted> <bytes>` to
/// argv[2] after every event. argv[3] (optional) is the nest marker. It ends
/// itself with `SIGALRM` after a minute, so a test that dies early leaves no
/// orphan behind (round 6, m-e).
const LISTENER: &str = r#"
import os, signal, socket, sys, time
signal.alarm(60)
path, out = sys.argv[1], sys.argv[2]
server = socket.socket(socket.AF_UNIX)
server.bind(path)
server.listen(4)
server.settimeout(0.05)
accepted = 0
received = 0
conns = []
def report():
    with open(out + ".tmp", "w") as f:
        f.write("%d %d\n" % (accepted, received))
    os.replace(out + ".tmp", out)
report()
while True:
    try:
        conn, _ = server.accept()
        conn.settimeout(0.05)
        conns.append(conn)
        accepted += 1
        report()
    except OSError:
        pass
    for conn in list(conns):
        try:
            data = conn.recv(65536)
            if data:
                received += len(data)
                report()
            else:
                conns.remove(conn)
        except OSError:
            pass
"#;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Kills the child by its pid on every exit path of the test. The listener is
/// started directly (not under `timeout`), so the pid killed here is the
/// python process itself and nothing is left orphaned (round 6, m-e).
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A scratch runtime directory with the lock and a verifier beside it, and a
/// test compositor listening in it.
struct Rig {
    dir: Scratch,
    lock: PathBuf,
    report: PathBuf,
    display: String,
    _listener: Guard,
}

impl Rig {
    /// `nest`: whether the listener's own command line carries the nest
    /// marker, as the niri of `dev-session.sh` does.
    fn new(name: &str, nest: bool) -> Self {
        let dir = Scratch(std::env::temp_dir().join(format!(
            "celestina-lock-guard-{name}-{}",
            std::process::id()
        )));
        fs::create_dir_all(&dir.0).expect("scratch dir");
        let lock = dir.0.join("celestina-lock");
        fs::copy(env!("CARGO_BIN_EXE_celestina-lock"), &lock).expect("copy the lock");
        fs::copy("/usr/bin/true", dir.0.join("celestina-lock-verify")).expect("copy a verifier");
        let display = "wayland-guard-test".to_owned();
        let report = dir.0.join("report");
        let mut command = Command::new("/usr/bin/python3");
        command
            .args(["-I", "-c", LISTENER])
            .arg(dir.0.join(&display))
            .arg(&report);
        if nest {
            command.arg("celestina-dev-session.kdl");
        }
        let listener = command
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the test compositor");
        let rig = Self {
            dir,
            lock,
            report,
            display,
            _listener: Guard(listener),
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !(rig.dir.0.join(&rig.display).exists() && rig.report.exists()) {
            assert!(
                Instant::now() < deadline,
                "the test compositor never came up"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        rig
    }

    /// `(connections accepted, bytes received)` so far.
    fn seen(&self) -> (u64, u64) {
        let text = fs::read_to_string(&self.report).unwrap_or_default();
        let mut words = text
            .split_whitespace()
            .map(|w| w.parse::<u64>().unwrap_or(0));
        (words.next().unwrap_or(0), words.next().unwrap_or(0))
    }

    /// A connection to the test compositor, as a harness would hold one.
    fn connect(&self) -> UnixStream {
        UnixStream::connect(self.dir.0.join(&self.display)).expect("connect to the test compositor")
    }

    /// Spawns the lock with `WAYLAND_SOCKET=value` and an environment built
    /// from nothing. The descriptor the value names must be open and
    /// inheritable in the caller.
    fn spawn_handed(&self, rehearsal: bool, value: &str, display: bool) -> Guard {
        let mut command = Command::new(&self.lock);
        if rehearsal {
            command.arg("--rehearsal");
        }
        command.env_clear().env("WAYLAND_SOCKET", value);
        if display {
            // A display that would be accepted if it were consulted: it must
            // not be.
            command
                .env("WAYLAND_DISPLAY", &self.display)
                .env("XDG_RUNTIME_DIR", &self.dir.0);
        }
        Guard(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn the lock"),
        )
    }

    /// Spawns the lock with an environment built from nothing.
    fn spawn(&self, rehearsal: bool, marker: bool) -> Guard {
        let mut command = Command::new(&self.lock);
        if rehearsal {
            command.arg("--rehearsal");
        }
        command
            .env_clear()
            .env("WAYLAND_DISPLAY", &self.display)
            .env("XDG_RUNTIME_DIR", &self.dir.0);
        if marker {
            command.env("CELESTINA_LOCK_LAUNCHER", "shell");
        }
        Guard(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn the lock"),
        )
    }
}

/// The exit status once the lock ends by itself within `within`, else `None`.
fn exit_within(lock: &mut Guard, within: Duration) -> Option<i32> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(status) = lock.0.try_wait().expect("wait") {
            return status.code();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn stderr_of(mut lock: Guard) -> String {
    use std::io::Read as _;
    let mut text = String::new();
    if let Some(mut pipe) = lock.0.stderr.take() {
        let _ = pipe.read_to_string(&mut text);
    }
    text
}

fn assert_refused(rig: &Rig, rehearsal: bool, marker: bool) {
    let mut lock = rig.spawn(rehearsal, marker);
    assert_eq!(
        exit_within(&mut lock, Duration::from_secs(8)),
        Some(15),
        "the lock refuses with exit 15 (setup failed)"
    );
    let message = stderr_of(lock);
    assert!(
        message.contains("lock-rehearsal.sh"),
        "the refusal says how to rehearse: {message}"
    );
    // Give a late byte the time to arrive; none may.
    std::thread::sleep(Duration::from_millis(300));
    let (_, bytes) = rig.seen();
    assert_eq!(bytes, 0, "not one byte reached the compositor");
}

fn wait_for_traffic(rig: &Rig) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (_, bytes) = rig.seen();
        if bytes > 0 || Instant::now() >= deadline {
            return bytes;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_bare_run_against_a_compositor_that_is_not_a_nest_is_refused_without_a_byte() {
    let rig = Rig::new("bare", false);
    assert_refused(&rig, false, false);
}

#[test]
fn rehearsal_is_refused_outside_a_nest_even_with_the_shell_marker() {
    let rig = Rig::new("rehearsal", false);
    assert_refused(&rig, true, true);
    assert_refused(&rig, true, false);
}

#[test]
fn the_shell_marker_lets_the_production_lock_connect_and_speak() {
    let rig = Rig::new("shell", false);
    let lock = rig.spawn(false, true);
    let bytes = wait_for_traffic(&rig);
    drop(lock);
    assert!(bytes > 0, "the guard does not break the production launch");
}

#[test]
fn a_verified_nest_is_accepted_for_rehearsal_and_receives_wayland_traffic() {
    let rig = Rig::new("nest", true);
    let lock = rig.spawn(true, false);
    let bytes = wait_for_traffic(&rig);
    drop(lock);
    assert!(bytes > 0, "the lock speaks Wayland to the verified nest");
}

#[test]
fn a_run_without_the_marker_is_accepted_in_a_verified_nest_too() {
    let rig = Rig::new("nest-bare", true);
    let lock = rig.spawn(false, false);
    let bytes = wait_for_traffic(&rig);
    drop(lock);
    assert!(bytes > 0);
}

/// The descriptor the child is handed: a duplicate that survives `exec`
/// (`dup` clears close-on-exec), as `signals.rs` does.
fn inheritable(stream: &UnixStream) -> std::os::fd::OwnedFd {
    rustix::io::dup(stream).expect("dup")
}

/// Round 6, finding I-A: `WAYLAND_SOCKET` was accepted whatever it was
/// connected to, so any socket (the real compositor's, from a harness that
/// opened it) skipped the nest requirement.
#[test]
fn a_handed_in_socket_to_a_compositor_that_is_not_a_nest_is_refused_without_a_byte() {
    let rig = Rig::new("handed-foreign", false);
    let stream = rig.connect();
    let fd = inheritable(&stream);
    let mut lock = rig.spawn_handed(true, &fd.as_raw_fd().to_string(), false);
    assert_eq!(
        exit_within(&mut lock, Duration::from_secs(8)),
        Some(15),
        "refused with exit 15"
    );
    let message = stderr_of(lock);
    assert!(message.contains("lock-rehearsal.sh"), "{message}");
    std::thread::sleep(Duration::from_millis(300));
    let (accepted, bytes) = rig.seen();
    assert_eq!(accepted, 1, "only the harness's own connection");
    assert_eq!(bytes, 0, "not one byte reached the compositor");
}

/// The production lock is held to the same rule (the real shell hands in no
/// socket).
#[test]
fn a_handed_in_socket_to_a_foreign_peer_is_refused_for_the_production_launch_too() {
    let rig = Rig::new("handed-foreign-prod", false);
    let stream = rig.connect();
    let fd = inheritable(&stream);
    let mut lock = rig.spawn_handed(false, &fd.as_raw_fd().to_string(), false);
    assert_eq!(exit_within(&mut lock, Duration::from_secs(8)), Some(15));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(rig.seen().1, 0);
}

#[test]
fn a_handed_in_socket_to_a_verified_nest_is_accepted() {
    let rig = Rig::new("handed-nest", true);
    let stream = rig.connect();
    let fd = inheritable(&stream);
    let lock = rig.spawn_handed(true, &fd.as_raw_fd().to_string(), false);
    let bytes = wait_for_traffic(&rig);
    drop(lock);
    assert!(bytes > 0, "a nest reached through a handed-in socket works");
}

/// A value that is not a descriptor number of 3 or more is refused as a
/// launch (exit 15, like every refusal), and nothing is connected: the
/// display the environment also names is never consulted.
#[test]
fn a_wayland_socket_that_is_not_a_descriptor_number_is_refused_and_nothing_is_connected() {
    let rig = Rig::new("handed-bad", true);
    for value in ["0", "1", "2", "x", "", " 5", "-1", "5 6"] {
        let mut lock = rig.spawn_handed(true, value, true);
        assert_eq!(
            exit_within(&mut lock, Duration::from_secs(8)),
            Some(15),
            "WAYLAND_SOCKET={value:?}"
        );
        let message = stderr_of(lock);
        assert!(message.contains("WAYLAND_SOCKET"), "{value:?}: {message}");
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(rig.seen(), (0, 0), "nothing was connected, nothing sent");
}
