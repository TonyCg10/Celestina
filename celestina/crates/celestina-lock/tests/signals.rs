//! The real binary ignores `SIGTERM`, `SIGINT`, `SIGHUP`, `SIGQUIT`, `SIGABRT`, `SIGALRM`, `SIGUSR1` and `SIGUSR2` (design §4.2, §7
//! F27, finding C1): the signals must be blocked in **every** thread of the
//! process, because a thread created before the mask is set inherits none and
//! would let the default action kill the lock.
//!
//! The binary runs against a socketpair as `WAYLAND_SOCKET`. The other end is
//! a minimal hand-written Wayland server (round 5, B-2): it advertises the
//! globals the lock binds and answers the two `sync` round trips, so the
//! lock's startup **completes** (the status writer thread is created, after
//! the signal mask, exactly where the finding C1 lived) and then it never
//! answers the lock request. A server that answered nothing would leave the
//! process in its first round trip, single-threaded, and the multi-thread
//! case this test exists for would not be exercised. Nothing here touches a
//! compositor, a session or a bus. The child is killed by its pid at the end
//! (never by name).

use std::fs;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// `SIGHUP`, `SIGINT` and `SIGTERM` as bits of a `SigBlk` mask (signal n is
/// bit n-1).
const HUP: u64 = 1 << 0;
const INT: u64 = 1 << 1;
const TERM: u64 = 1 << 14;
/// `SIGQUIT` (3), `SIGABRT` (6), `SIGUSR1` (10), `SIGUSR2` (12) and `SIGALRM`
/// (14) join them (review B, Info-2). `SIGKILL` and `SIGSTOP` cannot be
/// caught or blocked: the residual X2 of F27.
const QUIT: u64 = 1 << 2;
const ABRT: u64 = 1 << 5;
const USR1: u64 = 1 << 9;
const USR2: u64 = 1 << 11;
const ALRM: u64 = 1 << 13;
/// `SIGPIPE` (13): the Rust runtime ignores it as it starts.
const PIPE: u64 = 1 << 12;
const ALL: u64 = HUP | INT | TERM | QUIT | ABRT | USR1 | USR2 | ALRM;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Kills the child by pid on every exit path of the test.
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn sig_blk(path: &std::path::Path) -> Option<u64> {
    let text = fs::read_to_string(path).ok()?;
    let line = text.lines().find(|l| l.starts_with("SigBlk:"))?;
    u64::from_str_radix(line.trim_start_matches("SigBlk:").trim(), 16).ok()
}

/// A hexadecimal mask line of `/proc/<pid>/status`, such as `SigIgn:`.
fn sig_field(pid: u32, field: &str) -> Option<u64> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = text.lines().find(|l| l.starts_with(field))?;
    u64::from_str_radix(line.trim_start_matches(field).trim(), 16).ok()
}

fn masks(pid: u32) -> Vec<(String, u64)> {
    let Ok(dir) = fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            sig_blk(&entry.path().join("status")).map(|mask| (name, mask))
        })
        .collect()
}

/// `wl_display` is object 1: request 0 `sync(new_id)`, request 1
/// `get_registry(new_id)`; `wl_registry` event 0 is `global`, `wl_callback`
/// event 0 is `done`.
fn string_arg(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let length = u32::try_from(text.len() + 1).expect("short");
    bytes.extend_from_slice(&length.to_ne_bytes());
    bytes.extend_from_slice(text.as_bytes());
    bytes.push(0);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    bytes
}

fn event(object: u32, opcode: u16, body: &[u8]) -> Vec<u8> {
    let size = u32::try_from(8 + body.len()).expect("short");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&object.to_ne_bytes());
    bytes.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
    bytes.extend_from_slice(body);
    bytes
}

/// The globals the lock profile binds, with versions inside the ranges it asks
/// for. The lock manager is the one that matters: without it the lock exits
/// before it starts any worker.
const GLOBALS: &[(&str, u32)] = &[
    ("wl_compositor", 6),
    ("wl_shm", 1),
    ("wl_seat", 7),
    ("wl_output", 4),
    ("ext_session_lock_manager_v1", 1),
];

/// Serves `stream` until it closes: answers `get_registry` with the globals and
/// every `sync` with `done`; ignores everything else, including the lock
/// request.
fn serve(mut stream: UnixStream) {
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => pending.extend_from_slice(&chunk[..n]),
        }
        while pending.len() >= 8 {
            let word = u32::from_ne_bytes([pending[4], pending[5], pending[6], pending[7]]);
            let size = (word >> 16) as usize;
            if size < 8 || pending.len() < size {
                break;
            }
            let object = u32::from_ne_bytes([pending[0], pending[1], pending[2], pending[3]]);
            let opcode = (word & 0xffff) as u16;
            let arg = |at: usize| {
                pending
                    .get(8 + at..12 + at)
                    .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            };
            let mut reply = Vec::new();
            if object == 1 && opcode == 1 {
                if let Some(registry) = arg(0) {
                    for (index, (interface, version)) in GLOBALS.iter().enumerate() {
                        let mut body = u32::try_from(index + 1)
                            .expect("few")
                            .to_ne_bytes()
                            .to_vec();
                        body.extend(string_arg(interface));
                        body.extend_from_slice(&version.to_ne_bytes());
                        reply.extend(event(registry, 0, &body));
                    }
                }
            } else if object == 1 && opcode == 0 {
                if let Some(callback) = arg(0) {
                    reply.extend(event(callback, 0, &0u32.to_ne_bytes()));
                }
            }
            pending.drain(..size);
            if !reply.is_empty() && stream.write_all(&reply).is_err() {
                return;
            }
        }
    }
}

fn all_block(masks: &[(String, u64)]) -> bool {
    !masks.is_empty() && masks.iter().all(|(_, m)| m & ALL == ALL)
}

/// A sealed memory file holding a bundle with one 4x4 block, built with the
/// shared codec: the stdin the shell hands the lock when it has a picture.
fn one_block_bundle() -> std::os::fd::OwnedFd {
    use celestina_lock_proto::{encode, Bundle, OutputBlock};
    use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, MemfdFlags, SealFlags};
    let pixels = std::iter::repeat_n([250u8, 250, 250, 255], 16)
        .flatten()
        .collect();
    let block = OutputBlock::new("DP-1", 4, 4, pixels).expect("block");
    let bytes = encode(&Bundle {
        prior_failures: 0,
        offset_known: false,
        utc_offset_s: 0,
        outputs: vec![block],
    });
    let fd = memfd_create("bundle", MemfdFlags::ALLOW_SEALING).expect("memfd");
    ftruncate(&fd, u64::try_from(bytes.len()).expect("len")).expect("truncate");
    let mut written = 0;
    while written < bytes.len() {
        written += rustix::io::pwrite(
            &fd,
            &bytes[written..],
            u64::try_from(written).expect("offset"),
        )
        .expect("pwrite");
    }
    fcntl_add_seals(
        &fd,
        SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
    )
    .expect("seal");
    fd
}

/// The `State:` letter of the process (`Z` is a zombie: `/proc/<pid>` still
/// exists then, which is why a bare `metadata` check missed finding C1 of
/// review A).
fn state(pid: u32) -> Option<char> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let line = text.lines().find(|l| l.starts_with("State:"))?;
    line.trim_start_matches("State:").trim().chars().next()
}

/// A sealed bundle of the eight largest-allowed blocks: a large read that
/// keeps the process busy before the event loop exists (round B7c, B-2). Each
/// block is about 1.6 MiB, the whole file stays under the 16 MiB cap.
fn eight_block_bundle() -> std::os::fd::OwnedFd {
    use celestina_lock_proto::{encode, Bundle, OutputBlock};
    use rustix::fs::{fcntl_add_seals, ftruncate, memfd_create, MemfdFlags, SealFlags};
    let outputs = (0..8)
        .map(|n| {
            let pixels = std::iter::repeat_n([250u8, 250, 250, 255], 640 * 640)
                .flatten()
                .collect();
            OutputBlock::new(format!("DP-{n}"), 640, 640, pixels).expect("block")
        })
        .collect();
    let bytes = encode(&Bundle {
        prior_failures: 0,
        offset_known: false,
        utc_offset_s: 0,
        outputs,
    });
    let fd = memfd_create("bundle", MemfdFlags::ALLOW_SEALING).expect("memfd");
    ftruncate(&fd, u64::try_from(bytes.len()).expect("len")).expect("truncate");
    let mut written = 0;
    while written < bytes.len() {
        written += rustix::io::pwrite(
            &fd,
            &bytes[written..],
            u64::try_from(written).expect("offset"),
        )
        .expect("pwrite");
    }
    fcntl_add_seals(
        &fd,
        SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
    )
    .expect("seal");
    fd
}

/// Runs the real binary with `stdin` and `min_threads` expected, checks the
/// mask of every thread, sends the eight signals and checks the process is
/// alive and not a zombie.
fn the_signals_do_not_touch_it(
    stdin: Option<std::os::fd::OwnedFd>,
    min_threads: usize,
    early_term: bool,
) {
    static CASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = Scratch(std::env::temp_dir().join(format!(
        "celestina-lock-signals-{}-{}",
        std::process::id(),
        CASE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    fs::create_dir_all(&dir.0).expect("scratch dir");
    let lock = dir.0.join("celestina-lock");
    fs::copy(env!("CARGO_BIN_EXE_celestina-lock"), &lock).expect("copy the lock");
    // The verifier beside the lock: any executable the process can pin.
    fs::copy("/usr/bin/true", dir.0.join("celestina-lock-verify")).expect("copy a verifier");

    let (ours, theirs) = UnixStream::pair().expect("socketpair");
    // The child's end must survive exec: the duplicate is not close-on-exec.
    let inherited = rustix::io::dup(&theirs).expect("dup");
    let number = inherited.as_raw_fd();

    let server = std::thread::spawn({
        let stream = ours.try_clone().expect("clone the server end");
        move || serve(stream)
    });
    let child = Command::new(&lock)
        .arg("--rehearsal")
        .env_clear()
        .env("WAYLAND_SOCKET", number.to_string())
        .stdin(stdin.map_or_else(Stdio::null, Stdio::from))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the lock");
    let pid = child.id();
    let child = Guard(child);
    drop(theirs);
    drop(inherited);
    if early_term {
        // Round B7c, B-2: a `SIGTERM` by pid as soon as the Rust runtime of
        // the lock is up (it ignores `SIGPIPE`: the bit shows in `SigIgn`;
        // before that the image is still being loaded and nothing the lock
        // wrote has run), over and over until the mask is there: the window
        // in which the default disposition applied while the bundle was read.
        let start = Instant::now();
        let mut sent = 0;
        while start.elapsed() < Duration::from_secs(10) {
            if !matches!(state(pid), Some(letter) if letter != 'Z' && letter != 'X') {
                break;
            }
            if sig_field(pid, "SigIgn:").is_some_and(|m| m & PIPE != 0) {
                let status = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status()
                    .expect("kill(1)");
                if !status.success() {
                    break;
                }
                sent += 1;
                if masks(pid).iter().any(|(_, m)| m & TERM != 0) && sent > 1 {
                    break;
                }
            }
        }
        assert!(sent > 0, "the process was never seen running");
    }

    // Wait until startup is done: the writer thread exists (two threads at
    // least) and every thread has the mask. The process then waits for an
    // answer to its lock request that never comes.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !(masks(pid).len() >= min_threads && all_block(&masks(pid)))
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = masks(pid);
    assert!(
        before.len() >= min_threads,
        "startup completed and every worker exists, so the multi-thread case is exercised: {before:x?}"
    );
    assert!(
        all_block(&before),
        "every thread blocks HUP, INT, TERM, QUIT, ABRT, ALRM, USR1 and USR2: {before:x?}"
    );

    for signal in ["TERM", "INT", "HUP", "QUIT", "ABRT", "ALRM", "USR1", "USR2"] {
        let status = Command::new("kill")
            .args([&format!("-{signal}"), &pid.to_string()])
            .status()
            .expect("kill(1)");
        assert!(status.success(), "{signal}");
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        fs::metadata(format!("/proc/{pid}")).is_ok(),
        "the lock survived eight signals"
    );
    assert!(
        matches!(state(pid), Some(letter) if letter != 'Z' && letter != 'X'),
        "the lock is running, not a zombie: {:?}",
        state(pid)
    );
    let after = masks(pid);
    assert!(
        after.len() >= min_threads && all_block(&after),
        "{after:x?}"
    );
    drop(child);
    drop(ours);
    server.join().expect("the test server ended");
}

#[test]
fn the_ignored_signals_are_blocked_in_every_thread_and_the_process_survives() {
    the_signals_do_not_touch_it(None, 2, false);
}

/// Finding C1 of review A: with a picture on stdin the `backdrop` worker
/// exists too, and it must be created after the mask like the others.
#[test]
fn the_ignored_signals_are_blocked_in_the_backdrop_worker_too() {
    the_signals_do_not_touch_it(Some(one_block_bundle()), 3, false);
}

/// Round B7c, finding B-2: the process read its bundle with the default
/// disposition (0.5 to 3.8 ms measured), and a `SIGTERM` in that window killed
/// it before `requested`, which the shell reads as `ExitBeforeRequest` and
/// never retries. The mask is now the first instruction of `launch::start`.
/// The test sends `SIGTERM` from the moment `/proc/<pid>` exists, with the
/// largest bundle the codec accepts, and the lock must go on to the end of its
/// startup (the two round trips are served, the writer thread exists).
#[test]
fn a_sigterm_right_after_the_spawn_does_not_kill_the_lock() {
    for _ in 0..5 {
        the_signals_do_not_touch_it(Some(eight_block_bundle()), 3, true);
    }
}
