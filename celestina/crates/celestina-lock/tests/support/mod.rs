//! Shared by the integration tests that copy an executable and run the copy.

use std::io;
use std::process::{Child, Command};
use std::time::Duration;

/// How often a busy executable is tried again, and the pause between tries.
const TRIES: u32 = 100;
const PAUSE: Duration = Duration::from_millis(10);

/// Spawns `command`, trying again while the kernel answers "text file busy".
///
/// The tests of one binary run on threads, and each copies an executable and
/// runs the copy. A `fork` on one thread inherits the descriptor another
/// thread holds open for writing its own copy, until that forked child execs;
/// an exec of the copy inside that window fails with `ETXTBSY` although its
/// writer has long closed it (seen on the GitHub runner on 2026-10-24,
/// `tests/signals.rs`). The window is another child's fork-to-exec, so a
/// short wait always clears it, and any other error is returned at once.
pub fn spawn_fresh(command: &mut Command) -> io::Result<Child> {
    let mut tried = 0;
    loop {
        match command.spawn() {
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && tried < TRIES => {
                tried += 1;
                std::thread::sleep(PAUSE);
            }
            other => return other,
        }
    }
}
