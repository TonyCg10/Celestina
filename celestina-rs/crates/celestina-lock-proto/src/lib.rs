//! The session lock's shared vocabulary: verdicts, exit codes, the status-line
//! and bundle codecs, the backoff and the PAM service name (CEL-2-H, design
//! `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`
//! §4.13).
//!
//! Three programs speak this vocabulary: the shell (`celestina`), the lock
//! process (`celestina-lock`) and the PAM verifier (`celestina-lock-verify`).
//! It lives in a crate of its own, with no dependency at all, so that it can
//! be the one owner of every number both sides must agree on without pulling
//! anything into the binaries that can end a lock.
//!
//! - [`Verdict`] — what one authentication attempt came to, and the verifier's
//!   exit codes (§4.3 (c)).
//! - [`LockExit`] — the lock process's exit codes (§4.3 (b)).
//! - [`StatusLine`], [`parse_line`], [`render`], [`LineReader`] — the lines the
//!   lock prints on stdout, bounded and forward compatible (§4.3 (b)).
//! - [`Bundle`], [`encode`], [`decode`] — the backdrop bundle the shell hands
//!   the lock on stdin, format version 1 (§4.3 (a)).
//! - [`Backoff`] — the progressive delay after refusals (decision H-6).
//! - [`Service`], [`service_check`] — the closed list of PAM services and
//!   where their files are (§4.2, §4.11).
//! - [`tracer_pid`] — the one field of `/proc/<pid>/status` both binaries read
//!   to refuse to start under a tracer (§4.2, TM-12).
//! - [`inherited_descriptors`], [`wayland_fd`] — which descriptors a
//!   process was handed by accident and must close first (round 4, I-α).
//!
//! Everything here is pure: no file is opened, no clock read, no process
//! spawned. Input from another process is hostile; every decoder checks a
//! bound before it allocates for what the bound covers.

#![forbid(unsafe_code)]

mod backoff;
mod bundle;
mod descriptors;
mod exit;
mod lines;
mod proc_status;
mod service;
mod verdict;

use std::time::Duration;

pub use backoff::{Backoff, UNAVAILABLE_WAIT, WAIT_CAP};
pub use bundle::{
    decode, encode, BlockError, Bundle, Decoded, FileMeta, Ignored, OutputBlock, Status,
    HEADER_LEN, MAGIC, MAX_FILE_BYTES, MAX_OUTPUTS, NAME_MAX, SIDE_MAX,
};
pub use descriptors::{inherited_descriptors, wayland_fd, FIRST_INHERITED};
pub use exit::LockExit;
pub use lines::{parse_line, render, AttemptKind, LineError, LineReader, StatusLine, MAX_LINE};
pub use proc_status::tracer_pid;
pub use service::{service_check, Service, PAM_DIRECTORIES};
pub use verdict::Verdict;

/// The protocol version both sides speak: the lock's `hello` line, the
/// verifier's `--protocol` argument and the bundle's version field.
pub const PROTOCOL_VERSION: u16 = 1;

/// The longest secret the lock sends the verifier, in bytes (§4.11). The
/// lock never builds a longer one and the verifier refuses one.
pub const SECRET_MAX: usize = 512;

/// How long one attempt may take before the verifier is killed (decision 4).
pub const ATTEMPT_DEADLINE: Duration = Duration::from_secs(15);

/// How long the lock waits for a killed verifier to be reaped before it
/// leaves the child to the next attempt's spawn check (§4.10, M10).
pub const REAP_WAIT: Duration = Duration::from_secs(2);
