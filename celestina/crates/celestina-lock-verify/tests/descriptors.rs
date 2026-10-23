//! The real verifier closes every descriptor it was not handed on purpose
//! (round 4, finding I-α): see the lock's `tests/descriptors.rs`.
//!
//! The verifier is started on a valid request whose stdin never ends, so it
//! stays alive reading the secret; the test hands it a pipe's write end that
//! survives exec and drops its own copy.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use rustix::io::{fcntl_setfd, FdFlags};

const BIN: &str = env!("CARGO_BIN_EXE_celestina-lock-verify");

struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_verifier_closes_what_it_inherited_by_accident() {
    let (mut reader, writer) = std::io::pipe().expect("a pipe");
    fcntl_setfd(&writer, FdFlags::empty()).expect("inheritable");
    let mut child = Guard(
        Command::new(BIN)
            .args(["--protocol", "1", "--service", "celestina-lock-rehearsal"])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the verifier"),
    );
    drop(writer);

    // End-of-file arrives when every write end is closed. The thread ends
    // with the pipe, whatever the outcome: the guard kills the child.
    let (seen, eof) = mpsc::channel();
    std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        let _ = seen.send(matches!(reader.read(&mut byte), Ok(0)));
    });
    assert!(
        eof.recv_timeout(Duration::from_secs(10)) == Ok(true),
        "the verifier still holds the descriptor it inherited by accident"
    );
    assert!(
        child.0.try_wait().expect("wait").is_none(),
        "the verifier ended before it could be observed"
    );
}
