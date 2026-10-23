//! `celestina-lock`: the session lock (CEL-2-H, design
//! `docs/superpowers/specs/2026-10-17-celestina-cel-2-h-session-lock-design.md`).
//!
//! An `ext-session-lock-v1` client that covers every output, asks for the
//! password, and is the only thing that can end the lock, after the PAM
//! verifier child says yes. The pieces, from the inside out:
//!
//! * [`machine`] — the state machine of design §4.5, pure and tested without a
//!   compositor.
//! * [`verify`] — one attempt: the pinned verifier, its deadline, and the only
//!   place that mints the [`verify::Authenticated`] token.
//! * [`unlock`] — the single call that ends the lock, and it needs the token.
//! * [`driver`] — pumps the machine and carries out its actions through a
//!   [`driver::Host`].
//! * [`app`] — the adapter over the toolkit's lock profile (Wayland, timers,
//!   the status pipe) and the process's startup ([`launch`]).
//! * [`view`], [`paint`], [`copy`] — the CPU painter (design §4.6.1): the
//!   prompt over `tiny-skia` and `cosmic-text`, one embedded font, and the
//!   Spanish copy.
//! * [`field`], [`report`], [`wake`], [`hardening`], [`state_file`],
//!   [`backdrop`], [`present`] — the secret field, the status lines, the
//!   deadline timer, the first things the process does, the author's pid file,
//!   the bundle reader and the seam the painter plugs into.
//!
//! The library exists so the rehearsal probe (`unlock_probe`, behind the
//! `fake-verifier` feature) runs the very same core as the shipped binary. No
//! module here has `unsafe`.
#![forbid(unsafe_code)]

pub mod app;
pub mod backdrop;
pub mod copy;
pub mod driver;
pub mod field;
pub mod hardening;
pub mod launch;
pub mod machine;
pub mod paint;
pub mod present;
pub mod report;
pub mod state_file;
pub mod unlock;
pub mod verify;
pub mod view;
pub mod wake;
