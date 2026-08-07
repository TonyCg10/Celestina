//! Running the session's own tools, and holding the runtime while doing it.
//!
//! Every provider here reads the desktop through a program the session already
//! has installed. What they share is not what those programs say but how they
//! are run: bounded, killed if they outstay their welcome, and never able to
//! block the helper by hanging.

use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::inventory::Answer;
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
    probe_bounded(program, args).into_text()
}

/// The same, for a reading that cares *why* there was no output.
///
/// A program that is not installed and one that missed its deadline are the
/// same absence to a summary widget, which shows nothing either way. They are
/// not the same to a list: on a session without the tool an empty list is
/// permanently correct, and on a session with a slow one it is a lie that
/// erases what was already known.
pub fn probe_bounded(program: &str, args: &[&str]) -> Answer {
    probe_bounded_with_cancel(program, args, TOOL_TIMEOUT, None)
}

/// The same, for a tool whose own pace is the reason it is slow. DDC is a
/// physical conversation with a monitor and takes about a second; giving it the
/// panel's timeout would mean never reading a brightness at all.
pub fn run_bounded_with(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    run_bounded_with_cancel(program, args, timeout, None)
}

/// Runs a bounded tool owned by a cancellable provider. Shutdown kills and
/// reaps the direct child before the worker returns, so terminating the helper
/// cannot reparent a probe that was still talking to hardware.
pub fn run_bounded_with_cancel(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
) -> Option<String> {
    probe_bounded_with_cancel(program, args, timeout, cancelled).into_text()
}

/// The one implementation. Everything above is a narrowing of this answer for
/// a caller that does not need the distinction.
pub fn probe_bounded_with_cancel(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
) -> Answer {
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Answer::Unreadable;
    }

    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        // The only failure this can tell apart with confidence. A permission
        // error or an exhausted process table is a session that is unwell, not
        // one without the program.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Answer::Missing,
        Err(_) => return Answer::Unreadable,
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire))
                    || Instant::now() >= deadline
                {
                    stop_and_reap(&mut child);
                    return Answer::Unreadable;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                stop_and_reap(&mut child);
                return Answer::Unreadable;
            }
        }
    }

    let Ok(output) = child.wait_with_output() else {
        return Answer::Unreadable;
    };
    if !output.status.success() {
        return Answer::Unreadable;
    }
    Answer::Text(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn stop_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs an external tool the session already uses, detached from this helper's
/// pipes. A short-lived thread waits on it so a launched tool never lingers as
/// a zombie, and the helper never blocks on one.
pub fn launch(program: &str) -> Result<(), String> {
    launch_argv(&[program])
}

/// The same, for a launch that needs its own arguments — an application's
/// `Exec`, or a URL handed to `xdg-open`. `argv` is never run through a shell:
/// it is exactly the words execve receives, which is what keeps a `.desktop`
/// file's own text from being interpreted as anything more than a program name
/// and its arguments.
///
/// # Errors
///
/// Returns `Err` for an empty `argv`, or one whose program could not be
/// started at all; a launch is a request, and this is what a request that
/// could not even be made looks like.
pub fn launch_argv(argv: &[&str]) -> Result<(), String> {
    let [program, arguments @ ..] = argv else {
        return Err("nothing to launch".to_owned());
    };

    let child = Command::new(program)
        .args(arguments)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A program the session does not have. The whole point of the enum: this
    /// must not look like a tool that was merely slow, because a list built on
    /// it is permanently empty rather than temporarily unknown.
    #[test]
    fn a_program_that_is_not_installed_says_so_rather_than_timing_out() {
        let answer = probe_bounded("celestina-tool-that-does-not-exist", &[]);

        assert_eq!(answer, Answer::Missing);
        assert_eq!(answer.text(), None);
    }

    /// A tool that outstays its deadline. `sleep` is in coreutils, it touches
    /// nothing, and it is the one thing here that is guaranteed to be slow.
    #[test]
    fn a_program_that_outstays_its_deadline_is_unreadable() {
        let started = Instant::now();
        let answer = run_probe("sleep", &["30"], Duration::from_millis(80));

        assert_eq!(answer, Answer::Unreadable);
        // Killed at its deadline rather than waited out.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_program_that_fails_is_unreadable_rather_than_empty() {
        // `false` exits non-zero and prints nothing, which is exactly the
        // shape that must not be mistaken for "it answered with no rows".
        assert_eq!(probe_bounded("false", &[]), Answer::Unreadable);
    }

    /// The distinction that makes an empty inventory publishable: a tool that
    /// ran, succeeded and printed nothing really did answer.
    #[test]
    fn a_program_that_succeeds_with_no_output_answered_with_nothing() {
        let answer = probe_bounded("true", &[]);

        assert_eq!(answer, Answer::Text(String::new()));
        assert_eq!(answer.text(), Some(""));
    }

    /// A short-lived probe with its own deadline, so a timeout case does not
    /// have to wait out the shared one.
    fn run_probe(program: &str, args: &[&str], timeout: Duration) -> Answer {
        probe_bounded_with_cancel(program, args, timeout, None)
    }

    #[test]
    fn cancellation_stops_a_bounded_child_before_its_deadline() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = Arc::clone(&cancelled);
        let cancellation_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            request.store(true, Ordering::Release);
        });

        let started = Instant::now();
        // The child is killed and reaped before this returns, so nothing is
        // left behind for the helper to trip over on shutdown.
        let answer =
            probe_bounded_with_cancel("sleep", &["5"], Duration::from_secs(10), Some(&cancelled));

        // Cancelled, not absent: `sleep` is installed and was running.
        assert_eq!(answer, Answer::Unreadable);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(cancellation_thread.join().is_ok());

        // And a probe that is cancelled before it starts never spawns at all.
        assert_eq!(
            probe_bounded_with_cancel("sleep", &["5"], Duration::from_secs(10), Some(&cancelled)),
            Answer::Unreadable
        );
    }
}
