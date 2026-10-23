//! `celestina-lock-verify`: the session lock's PAM verifier (CEL-2-H, design
//! §4.3 (c), §4.11).
//!
//! One attempt per process. argv is exactly `--protocol 1 --service <name>`;
//! stdin is the secret (1 to 512 bytes, no NUL, exactly as typed, then EOF);
//! the exit status is the whole answer (see `celestina_lock_proto::Verdict`).
//! The process writes nothing to stdout or stderr and never opens a PAM session,
//! sets credentials or changes a password. It never sets `NO_NEW_PRIVS`.
//!
//! `unsafe` exists only in `pam.rs`. The crate root cannot say
//! `#![forbid(unsafe_code)]` (a `forbid` cannot be overridden by the allow in
//! `pam.rs`), so the workspace's `unsafe_code = "deny"` applies here and every
//! other module forbids it itself; `tests/source_guards.rs` scans the crate to
//! keep it so.

mod args;
mod consts;
mod conversation;
mod hardening;
mod outcome;
mod pam;
mod run;
mod secret;
mod session;

use std::fs::File;
use std::io::{self};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::process::ExitCode;

use celestina_lock_proto::Verdict;
use nix::unistd::{Uid, User};

use crate::run::Host;

/// The machine this process runs on.
struct RealHost;

impl Host for RealHost {
    fn uid(&self) -> u32 {
        // The real uid, not the effective one, and never `$USER`.
        rustix::process::getuid().as_raw()
    }

    fn user_name(&self, uid: u32) -> Option<String> {
        User::from_uid(Uid::from_raw(uid))
            .ok()
            .flatten()
            .map(|user| user.name)
    }

    fn existing(&self, candidates: &[PathBuf]) -> Vec<PathBuf> {
        candidates
            .iter()
            .filter(|path| std::fs::symlink_metadata(path).is_ok())
            .cloned()
            .collect()
    }
}

fn exit(verdict: Verdict) -> ExitCode {
    // `ExitCode::from` takes a `u8`; every code of the table fits.
    ExitCode::from(u8::try_from(verdict.exit_code()).unwrap_or(2))
}

fn main() -> ExitCode {
    // First, before anything else: whatever the lock handed over by accident
    // is closed (round 4, I-α; round 5, m-1 pins that this is the first
    // statement). Its result is acted on after the hook below, so a failure
    // ends the process as quietly as every other.
    let closed = hardening::close_inherited_descriptors();
    // Nothing is written anywhere: a panic ends the process with status 101,
    // which the lock reads as `Unavailable`, and prints nothing.
    std::panic::set_hook(Box::new(|_| {}));
    if let Err(verdict) = closed.and_then(|()| hardening::apply()) {
        return exit(verdict);
    }
    // stdin is read through a duplicate of its descriptor, unbuffered, so no
    // copy of the secret is left in `std`'s stdin buffer.
    let Ok(descriptor) = io::stdin().as_fd().try_clone_to_owned() else {
        return exit(Verdict::BadRequest);
    };
    let verdict = run::run(
        &RealHost,
        std::env::args_os().skip(1),
        File::from(descriptor),
        &mut pam::RealPam,
    );
    exit(verdict)
}
