//! Running the session's own tools, and holding the runtime while doing it.
//!
//! Every provider here reads the desktop through a program the session already
//! has installed. What they share is not what those programs say but how they
//! are run: bounded, killed if they outstay their welcome, and never able to
//! block the helper by hanging.

use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::runtime::ProviderRuntime;

/// A tool that cannot answer in this long is a tool the panel does without.
/// It is generous for a process spawn and short enough that a stuck player or
/// a wedged adapter never becomes a stuck provider.
const TOOL_TIMEOUT: Duration = Duration::from_millis(750);

/// A poisoned runtime lock still owns usable state; recovering it keeps the
/// panel fed instead of taking the helper down with the thread that panicked —
/// the documented mutex pattern of this suite.
pub fn lock_runtime(runtime: &Mutex<ProviderRuntime>) -> MutexGuard<'_, ProviderRuntime> {
    match runtime.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Runs a short-lived tool and returns its output, killing it if it outstays
/// its deadline: a hung tool must not take its provider down with it.
pub fn run_bounded(program: &str, args: &[&str]) -> Option<String> {
    run_bounded_with(program, args, TOOL_TIMEOUT)
}

/// The same, for a tool whose own pace is the reason it is slow. DDC is a
/// physical conversation with a monitor and takes about a second; giving it the
/// panel's timeout would mean never reading a brightness at all.
pub fn run_bounded_with(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs an external tool the session already uses, detached from this helper's
/// pipes. A short-lived thread waits on it so a launched tool never lingers as
/// a zombie, and the helper never blocks on one.
pub fn launch(program: &str) -> Result<(), String> {
    let child = Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start {program}: {error}"))?;

    thread::Builder::new()
        .name("reaper".to_owned())
        .spawn(move || {
            let mut child = child;
            let _ = child.wait();
        })
        .map_err(|error| format!("cannot wait on {program}: {error}"))?;
    Ok(())
}
