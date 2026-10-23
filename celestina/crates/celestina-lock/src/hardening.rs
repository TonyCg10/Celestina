//! What the lock does first, before it reads a byte of anything (design §4.2
//! "Not traced at start", H-13, TM-12): not dumpable, no core file, and no
//! tracer attached.
//!
//! `PR_SET_DUMPABLE 0` refuses a later `ptrace` attach and makes
//! `/proc/<pid>/mem` and `environ` unreadable to the user, but it cannot
//! detach a tracer that attached in the instant before the call, so
//! `TracerPid` is read right after and a non-zero value ends the process
//! (exit 15, before `requested`). No secret exists yet at that point.
//!
//! The verifier does the same three steps (`celestina-lock-verify`'s
//! `hardening.rs`) and maps a failure to its own exit codes; the parse of
//! `TracerPid` is the one thing both share and lives in `celestina-lock-proto`.
//! The syscalls stay in each binary because that crate has no dependency to
//! make them with.
//!
//! `PR_SET_NO_NEW_PRIVS` is **never** set: the verifier runs PAM, and
//! `pam_unix` needs the setuid `unix_chkpwd` (design §6 item 20).
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Read;

use celestina_lock_proto::{inherited_descriptors, tracer_pid};
use rustix::process::{set_dumpable_behavior, setrlimit, DumpableBehavior, Resource, Rlimit};

/// The longest part of `/proc/self/status` read: the file is a few kilobytes.
const STATUS_MAX: u64 = 64 * 1024;

/// Why the lock must not start.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HardeningError {
    #[error("cannot make the process non-dumpable")]
    Dumpable,
    #[error("cannot set the core limit to zero")]
    CoreLimit,
    #[error("the process is being traced by pid {0}")]
    Traced(u32),
    #[error("cannot tell whether the process is traced")]
    TracerUnknown,
    #[error("cannot list the descriptors the process inherited")]
    Descriptors,
}

/// Closes every descriptor the lock was not handed on purpose (round 4,
/// finding I-α): all of them from 3 up, except `keep`, the descriptor
/// `WAYLAND_SOCKET` names (read, once, by `launch::start`: this module reads no
/// environment).
/// Descriptors 0 (the bundle), 1 and 2 are the shell's choice and stay.
///
/// This is the first thing the process does, before [`apply`]. The order does
/// not matter for safety (listing `/proc/self/fd` is allowed to a
/// non-dumpable process, and no secret exists yet), so it goes first because
/// that is when the shell's accidental copy of the logind delay inhibitor is
/// released soonest: while the lock holds it, logind waits `InhibitDelayMaxSec`
/// before every suspend. It is a function of its own, not a step of
/// [`apply`], because the tests call `apply` in the test process and must not
/// close its descriptors.
///
/// # Errors
///
/// [`HardeningError::Descriptors`] when `/proc/self/fd` cannot be listed: not
/// knowing what is open is not a pass, as for the tracer.
pub fn close_inherited_descriptors(keep: Option<i32>) -> Result<(), HardeningError> {
    let names: Vec<String> = std::fs::read_dir("/proc/self/fd")
        .and_then(|dir| {
            dir.map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect()
        })
        .map_err(|_| HardeningError::Descriptors)?;
    let keep: Vec<i32> = keep.into_iter().collect();
    for number in inherited_descriptors(names.iter().map(String::as_str), &keep) {
        // The listing's own descriptor is in `names` and already closed: its
        // `EBADF` is expected, and so is any other failure to close a
        // descriptor nobody here owns.
        let _ = nix::unistd::close(number);
    }
    Ok(())
}

/// Non-dumpable, no core file, then the tracer check.
///
/// # Errors
///
/// The first step that failed; the caller exits 15.
pub fn apply() -> Result<(), HardeningError> {
    set_dumpable_behavior(DumpableBehavior::NotDumpable).map_err(|_| HardeningError::Dumpable)?;
    setrlimit(
        Resource::Core,
        Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| HardeningError::CoreLimit)?;
    check_not_traced()
}

/// Reads `/proc/self/status` (bounded) and refuses a non-zero `TracerPid`.
///
/// # Errors
///
/// [`HardeningError::Traced`] with a tracer, [`HardeningError::TracerUnknown`]
/// when the file cannot be read or has no `TracerPid` line: not knowing is not
/// a pass.
pub fn check_not_traced() -> Result<(), HardeningError> {
    let mut text = String::new();
    File::open("/proc/self/status")
        .and_then(|file| file.take(STATUS_MAX).read_to_string(&mut text))
        .map_err(|_| HardeningError::TracerUnknown)?;
    check_tracer(&text)
}

/// The tracer decision over the text of `/proc/<pid>/status`.
///
/// # Errors
///
/// See [`check_not_traced`].
pub fn check_tracer(status: &str) -> Result<(), HardeningError> {
    match tracer_pid(status) {
        Some(0) => Ok(()),
        Some(pid) => Err(HardeningError::Traced(pid)),
        None => Err(HardeningError::TracerUnknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::process::{dumpable_behavior, getrlimit};
    use std::sync::Mutex;

    /// Hardening is process-wide: the tests that apply it take turns.
    static PROCESS_STATE: Mutex<()> = Mutex::new(());

    const UNTRACED: &str =
        "Name:\tcelestina-lock\nPid:\t4242\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\n";

    #[test]
    fn t_h1_not_dumpable_and_core_limit_zero_after_hardening() {
        let _turn = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        apply().expect("hardening");
        assert_eq!(dumpable_behavior(), Ok(DumpableBehavior::NotDumpable));
        let core = getrlimit(Resource::Core);
        assert_eq!(core.current, Some(0));
        assert_eq!(core.maximum, Some(0));
        // Restored so the rest of the test process can read its own files.
        set_dumpable_behavior(DumpableBehavior::Dumpable).expect("restore");
    }

    #[test]
    fn t_h2_non_zero_tracer_pid_exits_15_before_requested() {
        assert_eq!(check_tracer(UNTRACED), Ok(()));
        let traced = UNTRACED.replace("TracerPid:\t0", "TracerPid:\t31337");
        assert_eq!(check_tracer(&traced), Err(HardeningError::Traced(31337)));
        for status in ["", "Name:\tx\n", "TracerPid:\n", "TracerPid:\tabc\n"] {
            assert_eq!(
                check_tracer(status),
                Err(HardeningError::TracerUnknown),
                "{status:?}"
            );
        }
        // The exit that stands for it is the setup failure, before any
        // `requested` line exists.
        assert_eq!(celestina_lock_proto::LockExit::SetupFailed.code(), 15);
    }

    #[test]
    fn the_real_status_of_this_process_is_readable() {
        let text = std::fs::read_to_string("/proc/self/status").expect("status");
        assert!(tracer_pid(&text).is_some());
    }

    #[test]
    fn no_new_privs_is_left_as_found() {
        let _turn = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        let read = || {
            std::fs::read_to_string("/proc/self/status")
                .expect("status")
                .lines()
                .find_map(|line| line.strip_prefix("NoNewPrivs:"))
                .map(|value| value.trim().to_owned())
        };
        let before = read();
        apply().expect("hardening");
        assert_eq!(read(), before);
        set_dumpable_behavior(DumpableBehavior::Dumpable).expect("restore");
    }
}
