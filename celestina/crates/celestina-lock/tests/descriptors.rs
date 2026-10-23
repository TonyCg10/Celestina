//! The real lock closes every descriptor it was not handed on purpose (round 4,
//! finding I-α).
//!
//! The shell can hold an inheritable copy of the logind delay inhibitor while
//! it starts the lock (the bus library receives it without close-on-exec), and
//! the lock would then keep the inhibitor for its whole life. The test hands
//! the binary a pipe's write end that is not close-on-exec, as the leak does,
//! and drops its own copy: the read end sees end-of-file as soon as the lock
//! has closed its inherited one, while the lock is still alive. The Wayland
//! socket it was given on purpose must survive. Nothing here touches a
//! compositor, a session or a bus; the child is killed by its pid at the end.

use std::fs;
use std::io::{PipeReader, Read};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rustix::event::{poll, PollFd, PollFlags};
use rustix::io::{fcntl_setfd, FdFlags};

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Whether the pipe's write ends are all closed.
fn at_eof(reader: &PipeReader) -> bool {
    let mut fds = [PollFd::new(reader, PollFlags::IN)];
    let zero = rustix::time::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(poll(&mut fds, Some(&zero)), Ok(n) if n > 0)
        && fds[0].revents().contains(PollFlags::HUP)
}

#[test]
fn the_lock_closes_what_it_inherited_by_accident_and_keeps_the_wayland_socket() {
    let dir = Scratch(
        std::env::temp_dir().join(format!("celestina-lock-descriptors-{}", std::process::id())),
    );
    fs::create_dir_all(&dir.0).expect("scratch dir");
    let lock = dir.0.join("celestina-lock");
    fs::copy(env!("CARGO_BIN_EXE_celestina-lock"), &lock).expect("copy the lock");
    fs::copy("/usr/bin/true", dir.0.join("celestina-lock-verify")).expect("copy a verifier");

    // The socket the lock is handed on purpose, and the accident: a write end
    // that survives exec, with the test's read end watching it.
    let (mut ours, theirs) = UnixStream::pair().expect("socketpair");
    let socket = rustix::io::dup(&theirs).expect("dup");
    let (reader, writer) = std::io::pipe().expect("a pipe");
    fcntl_setfd(&writer, FdFlags::empty()).expect("inheritable");
    let socket_number = socket.as_raw_fd();
    let leaked_number = writer.as_raw_fd();
    assert_ne!(socket_number, leaked_number);

    let child = Command::new(&lock)
        .arg("--rehearsal")
        .env_clear()
        .env("WAYLAND_SOCKET", socket_number.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the lock");
    let mut child = Guard(child);
    drop(theirs);
    drop(socket);
    drop(writer);

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !at_eof(&reader) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        at_eof(&reader),
        "the lock still holds the descriptor it inherited by accident"
    );
    assert!(
        child.0.try_wait().expect("wait").is_none(),
        "the lock ended before it could be observed"
    );
    // The lock is non-dumpable, so its `/proc/<pid>/fd` is not ours to read:
    // the socket it was handed is seen from the other end. Closed, the peer
    // reads end-of-file once what the lock wrote is consumed; open, it only
    // ever blocks.
    std::thread::sleep(Duration::from_millis(300));
    ours.set_nonblocking(true).expect("nonblocking");
    let mut buffer = [0u8; 4096];
    loop {
        match ours.read(&mut buffer) {
            Ok(0) => panic!("the Wayland socket it was handed on purpose was closed"),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => panic!("reading the peer of the Wayland socket: {err}"),
        }
    }
    drop(ours);
}
