//! Bounded, deterministically terminated child processes.
//!
//! The daemon shells out to small desktop tools — `playerctl`, `wl-paste`,
//! `wl-copy`, `sshfs`, `fusermount3` — from threads that also pump a phone
//! link. A tool that never answers must never be able to stop that pump, so
//! every spawn here has an absolute deadline and a cancellation flag, and every
//! exit path kills the whole process *group*: a child that forked before
//! answering would otherwise keep the inherited pipe open and outlive its
//! parent's deadline.
//!
//! This is the one owner of that discipline. Callers choose the program, the
//! arguments and the budget; they do not each re-derive the waiting, the
//! draining and the reaping.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use rustix::pipe::{pipe_with, PipeFlags};
use rustix::process::{kill_process_group, Pid, Signal};

/// The most output any of these tools may produce before it is treated as a
/// tool that will not stop talking.
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const OUTPUT_READ_CHUNK: usize = 8 * 1024;

pub(crate) fn command_output_from(
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
    let (status, output) = wait_with_output(
        &mut child,
        group,
        &mut output_reader,
        deadline,
        stopping,
        GroupPolicy::Terminate,
    )?;
    status.success().then_some(output)
}

pub(crate) fn output_pipe() -> io::Result<(File, Stdio)> {
    let (read_end, write_end) = pipe_with(PipeFlags::CLOEXEC)?;
    let flags = fcntl_getfl(&read_end)?;
    fcntl_setfl(&read_end, flags | OFlags::NONBLOCK)?;
    Ok((File::from(read_end), Stdio::from(write_end)))
}

pub(crate) fn spawn_grouped(
    program: &str,
    args: &[&str],
    stdout: Stdio,
) -> io::Result<(Child, Pid)> {
    spawn_grouped_env(program, args, stdout, &[])
}

/// [`spawn_grouped`], with extra environment variables for the child.
///
/// A daemon inherits the environment it was *started* with, and a user service
/// can start before the compositor publishes the session's display variables.
/// A child that needs them therefore cannot rely on this process's environment
/// and must be given the values resolved at spawn time.
pub(crate) fn spawn_grouped_env(
    program: &str,
    args: &[&str],
    stdout: Stdio,
    env: &[(String, String)],
) -> io::Result<(Child, Pid)> {
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null())
        .process_group(0);
    let child = command.spawn()?;
    let group = Pid::from_child(&child);
    Ok((child, group))
}

/// What to do with the rest of the process group once the command itself has
/// exited.
///
/// [`GroupPolicy::Terminate`] is right for a tool whose descendants only ever
/// inherit the pipe by accident. It is wrong for `wl-copy` and `sshfs`, which
/// deliberately fork a background process — the one that owns the selection,
/// or the mount — and exit: killing their group would undo the very work the
/// command just reported as done.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupPolicy {
    Terminate,
    KeepBackgroundChildren,
}

pub(crate) fn wait_with_output(
    child: &mut Child,
    group: Pid,
    reader: &mut File,
    deadline: Instant,
    stopping: &AtomicBool,
    on_exit: GroupPolicy,
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
                if on_exit == GroupPolicy::Terminate {
                    terminate_group(group);
                }
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

pub(crate) fn drain_available(reader: &mut File, output: &mut Vec<u8>) -> io::Result<()> {
    let mut chunk = [0_u8; OUTPUT_READ_CHUNK];
    loop {
        let remaining = MAX_OUTPUT_BYTES.saturating_sub(output.len());
        let read_limit = (remaining + 1).min(chunk.len());
        match reader.read(&mut chunk[..read_limit]) {
            Ok(0) => return Ok(()),
            Ok(read) if read > remaining => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the command exceeded the bounded capture size",
                ));
            }
            Ok(read) => output.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn wait_bounded(
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

pub(crate) fn terminate_group(group: Pid) {
    let _ = kill_process_group(group, Signal::KILL);
}

pub(crate) fn terminate_group_and_reap(child: &mut Child, group: Pid) {
    terminate_group(group);
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn poll_delay(deadline: Instant) -> Duration {
    deadline
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(10))
}

pub(crate) fn cancelled(stopping: &AtomicBool, deadline: Instant) -> bool {
    stopping.load(Ordering::Acquire) || Instant::now() >= deadline
}

/// What a bounded command that reads stdin ended up doing. The captured bytes
/// are its diagnostics, kept whether it succeeded or not so a caller can say
/// *why* rather than only *that* it failed.
pub(crate) struct InputOutcome {
    pub(crate) succeeded: bool,
    pub(crate) captured: Vec<u8>,
}

impl InputOutcome {
    /// The command's own explanation, trimmed, or `fallback` when it gave none
    /// — which is the case when the deadline, not the command, ended it.
    pub(crate) fn reason(&self, fallback: &str) -> String {
        let captured = String::from_utf8_lossy(&self.captured);
        let trimmed = captured.trim();
        if trimmed.is_empty() {
            fallback.to_owned()
        } else {
            trimmed.to_owned()
        }
    }
}

/// Run a command that reads one input on stdin, under the same deadline and
/// group teardown, capturing its diagnostics.
///
/// The write is non-blocking and deadline-checked because a child that never
/// reads would otherwise fill the pipe buffer and block the calling thread —
/// which is a link pump — with no bound at all.
pub(crate) fn run_with_input(
    program: &str,
    args: &[&str],
    input: &[u8],
    deadline: Instant,
    stopping: &AtomicBool,
) -> InputOutcome {
    let failed = || InputOutcome {
        succeeded: false,
        captured: Vec::new(),
    };
    if cancelled(stopping, deadline) {
        return failed();
    }
    let Ok((mut capture_reader, capture)) = output_pipe() else {
        return failed();
    };
    let Ok((mut child, group)) = spawn_grouped_with_input(program, args, capture) else {
        return failed();
    };
    let Some(stdin) = child.stdin.take() else {
        terminate_group_and_reap(&mut child, group);
        return failed();
    };
    if write_all_bounded(stdin, input, deadline, stopping).is_err() {
        terminate_group_and_reap(&mut child, group);
        return failed();
    }
    match wait_with_output(
        &mut child,
        group,
        &mut capture_reader,
        deadline,
        stopping,
        // Both callers here — `wl-copy` and `sshfs` — hand their real work to
        // a background child and exit, so the group outlives the command.
        GroupPolicy::KeepBackgroundChildren,
    ) {
        Some((status, captured)) => InputOutcome {
            succeeded: status.success(),
            captured,
        },
        None => failed(),
    }
}

fn spawn_grouped_with_input(
    program: &str,
    args: &[&str],
    capture: Stdio,
) -> io::Result<(Child, Pid)> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(capture)
        .process_group(0);
    let child = command.spawn()?;
    let group = Pid::from_child(&child);
    Ok((child, group))
}

/// Write every byte, or give up at the deadline. Closing the handle on return
/// is what tells the child its input ended.
fn write_all_bounded(
    stdin: std::process::ChildStdin,
    input: &[u8],
    deadline: Instant,
    stopping: &AtomicBool,
) -> io::Result<()> {
    let flags = fcntl_getfl(&stdin)?;
    fcntl_setfl(&stdin, flags | OFlags::NONBLOCK)?;
    let mut stdin = File::from(std::os::fd::OwnedFd::from(stdin));
    let mut written = 0;
    while written < input.len() {
        if cancelled(stopping, deadline) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the command did not read its input in time",
            ));
        }
        match stdin.write(&input[written..]) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(poll_delay(deadline));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::run_with_input;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn marker_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "magnetita-subprocess-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn input_reaches_the_command_and_its_diagnostics_come_back() {
        let stopping = AtomicBool::new(false);
        let marker = marker_path("input");
        let outcome = run_with_input(
            "sh",
            &[
                "-c",
                "cat > \"$1\"; printf note >&2",
                "sh",
                &marker.to_string_lossy(),
            ],
            b"the-secret\n",
            Instant::now() + Duration::from_secs(2),
            &stopping,
        );

        assert!(outcome.succeeded);
        assert_eq!(outcome.reason("none"), "note");
        assert_eq!(fs::read(&marker).unwrap(), b"the-secret\n");
        let _ = fs::remove_file(marker);
    }

    #[test]
    fn a_command_that_never_reads_its_input_ends_at_the_deadline() {
        let stopping = AtomicBool::new(false);
        let started = Instant::now();
        // Never reads stdin, never exits: the shape of an sshfs pointed at an
        // address that accepts the connection and then says nothing.
        let outcome = run_with_input(
            "sh",
            &["-c", "sleep 30"],
            b"the-secret\n",
            Instant::now() + Duration::from_millis(200),
            &stopping,
        );

        assert!(!outcome.succeeded);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the command must not outlive its budget, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_background_child_survives_a_successful_command() {
        let stopping = AtomicBool::new(false);
        let marker = marker_path("background");
        let script = format!(
            "cat >/dev/null; (sleep 1; : > \"{}\") & exit 0",
            marker.to_string_lossy()
        );
        // wl-copy and sshfs both work this way; terminating the group on exit
        // would drop the selection or the mount the command just established.
        let outcome = run_with_input(
            "sh",
            &["-c", &script],
            b"value\n",
            Instant::now() + Duration::from_secs(2),
            &stopping,
        );

        assert!(outcome.succeeded);
        std::thread::sleep(Duration::from_millis(1_400));
        let survived = marker.exists();
        let _ = fs::remove_file(marker);
        assert!(survived, "the backgrounded child must outlive the command");
    }

    #[test]
    fn an_already_cancelled_run_spawns_nothing() {
        let stopping = AtomicBool::new(true);
        let outcome = run_with_input(
            "sh",
            &["-c", "exit 0"],
            b"",
            Instant::now() + Duration::from_secs(2),
            &stopping,
        );
        assert!(!outcome.succeeded);
    }
}
