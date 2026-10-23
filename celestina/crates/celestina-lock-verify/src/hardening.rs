//! What the verifier does first, before it reads a byte of the secret (design
//! §4.2 "Not traced at start", §4.11, TM-12): not dumpable, no core file, and
//! no tracer attached.
//!
//! `PR_SET_DUMPABLE 0` refuses a later `ptrace` attach and makes
//! `/proc/<pid>/mem` unreadable to the user, but it cannot detach a tracer that
//! attached in the instant before the call, so `TracerPid` is read right after
//! and a non-zero value ends the process. No secret exists yet at that point.
//!
//! `PR_SET_NO_NEW_PRIVS` is **never** set here or anywhere in this crate:
//! `pam_unix` runs the setuid `unix_chkpwd`, and that bit would make every
//! attempt `Unavailable` (design §4.11, §6 item 20).
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Read;

use celestina_lock_proto::{inherited_descriptors, tracer_pid, Verdict};
use rustix::process::{set_dumpable_behavior, setrlimit, DumpableBehavior, Resource, Rlimit};

/// The longest part of `/proc/self/status` read: the file is a few kilobytes.
const STATUS_MAX: u64 = 64 * 1024;

/// Closes every descriptor the verifier was not handed on purpose (round 4,
/// finding I-α): all of them from 3 up. The lock gives it nothing but stdin
/// (the secret), stdout and stderr; a copy of the logind delay inhibitor the
/// shell held while it started the lock must not live as long as an attempt.
/// The lock closes its own accidental descriptors the same way and the
/// decision of which are accidents lives in `celestina-lock-proto`; this
/// binary has no `WAYLAND_SOCKET` to keep.
///
/// A function of its own, called first by `main`, because the tests call
/// [`apply`] in the test process and must not close its descriptors.
///
/// # Errors
///
/// [`Verdict::Unavailable`] when `/proc/self/fd` cannot be listed: not
/// knowing what is open is not a pass.
pub fn close_inherited_descriptors() -> Result<(), Verdict> {
    let names: Vec<String> = std::fs::read_dir("/proc/self/fd")
        .and_then(|dir| {
            dir.map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect()
        })
        .map_err(|_| Verdict::Unavailable)?;
    for number in inherited_descriptors(names.iter().map(String::as_str), &[]) {
        // The listing's own descriptor is in `names` and already closed.
        let _ = nix::unistd::close(number);
    }
    Ok(())
}

/// Hardening, then the tracer check, as the verdict the process exits with if
/// either fails: `Unavailable` when a syscall refused, `BadRequest` (6) when a
/// tracer is attached or `/proc/self/status` cannot be read to tell.
///
/// # Errors
///
/// The verdict to exit with.
pub fn apply() -> Result<(), Verdict> {
    set_dumpable_behavior(DumpableBehavior::NotDumpable).map_err(|_| Verdict::Unavailable)?;
    setrlimit(
        Resource::Core,
        Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| Verdict::Unavailable)?;
    check_not_traced()
}

/// Read `/proc/self/status` (bounded) and refuse a non-zero `TracerPid`.
///
/// # Errors
///
/// [`Verdict::BadRequest`] when a tracer is attached, and also when the file
/// cannot be read or has no `TracerPid` line: not knowing is not a pass.
pub fn check_not_traced() -> Result<(), Verdict> {
    let mut text = String::new();
    File::open("/proc/self/status")
        .and_then(|file| file.take(STATUS_MAX).read_to_string(&mut text))
        .map_err(|_| Verdict::BadRequest)?;
    check_tracer(&text)
}

/// The tracer decision over the text of `/proc/<pid>/status`.
///
/// # Errors
///
/// [`Verdict::BadRequest`] unless a `TracerPid:` line says `0`.
pub fn check_tracer(status: &str) -> Result<(), Verdict> {
    match tracer_pid(status) {
        Some(0) => Ok(()),
        _ => Err(Verdict::BadRequest),
    }
}

/// The value of the `NoNewPrivs:` line, for the tests that assert the bit is
/// left as it was found.
#[cfg(test)]
fn no_new_privs(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("NoNewPrivs:"))
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::process::{dumpable_behavior, getrlimit};
    use std::sync::Mutex;

    /// Hardening is process-wide: the tests that apply it and restore the
    /// dumpable bit take turns.
    static PROCESS_STATE: Mutex<()> = Mutex::new(());

    const FIXTURE_UNTRACED: &str =
        "Name:\tcelestina-lock-verify\nState:\tR (running)\nTgid:\t4242\nPid:\t4242\nPPid:\t1\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\n";

    #[test]
    fn t_h1_not_dumpable_and_core_limit_zero_after_hardening() {
        let _turn = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        // This test process is hardened for the rest of its life: harmless
        // (no core files, no attach) and what the binary does at start.
        apply().expect("hardening");
        assert_eq!(dumpable_behavior(), Ok(DumpableBehavior::NotDumpable));
        let core = getrlimit(Resource::Core);
        assert_eq!(core.current, Some(0));
        assert_eq!(core.maximum, Some(0));
        // The hardening leaves the process dumpable again for the tests that
        // read /proc/self/mem-like files of their own.
        set_dumpable_behavior(DumpableBehavior::Dumpable).expect("restore");
    }

    #[test]
    fn t_h2_non_zero_tracer_pid_exits_6() {
        assert_eq!(check_tracer(FIXTURE_UNTRACED), Ok(()));
        let traced = FIXTURE_UNTRACED.replace("TracerPid:\t0", "TracerPid:\t31337");
        assert_eq!(check_tracer(&traced), Err(Verdict::BadRequest));
        assert_eq!(Verdict::BadRequest.exit_code(), 6);
        // A status without the line, or with a garbled one, is not a pass.
        for status in [
            "",
            "Name:\tx\n",
            "TracerPid:\n",
            "TracerPid:\tabc\n",
            "TracerPid:\t-1\n",
            "TracerPid:\t0x0\n",
        ] {
            assert_eq!(check_tracer(status), Err(Verdict::BadRequest), "{status:?}");
        }
        // Only the line named TracerPid counts, not a value in another field.
        assert_eq!(
            check_tracer("Name:\tTracerPid:\t0\nPPid:\t0\n"),
            Err(Verdict::BadRequest)
        );
    }

    #[test]
    fn the_real_status_of_this_process_says_untraced_or_names_the_tracer() {
        let text = std::fs::read_to_string("/proc/self/status").expect("status");
        assert!(tracer_pid(&text).is_some());
    }

    #[test]
    fn no_new_privs_is_never_set() {
        let _turn = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        let read = || {
            let text = std::fs::read_to_string("/proc/self/status").expect("status");
            no_new_privs(&text).expect("NoNewPrivs line")
        };
        let before = read();
        apply().expect("hardening");
        assert_eq!(read(), before, "hardening changed NoNewPrivs");
        set_dumpable_behavior(DumpableBehavior::Dumpable).expect("restore");
    }
}
