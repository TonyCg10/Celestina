//! The pure domain of the Celestina shell (`celestina/`): the notifications
//! state machine, launcher ranking, the workspace model, coalescing and the
//! shell's durable settings, as plain Rust types the product consumes in
//! process (design §6.2,
//! `docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`). It
//! knows nothing about Wayland, GL or D-Bus.
//!
//! Every module was copied from the halted `celestina-shell-core` under the
//! root contract's salvage rule and adapted: no line protocol, no provider
//! envelope, no host commands. Each file's head names its source and commit.
//!
//! - [`bounded`] — the one bound on text from another process.
//! - [`coalesce`] — how often a changing provider may reach the event loop.
//! - [`notifications`] — identity, replacement, expiry, actions, capabilities
//!   and how much history is kept and shown.
//! - [`launcher`] — which application a person means by part of a name.
//! - [`workspace_groups`] — which monitor a workspace belongs to once that
//!   monitor is gone.
//! - [`workspace_map`] — what a workspace holds, as shares of its columns.
//! - [`settings`] — the shell's own durable choice (do-not-disturb) and the
//!   rule that a choice is published only once it is written.
//! - [`calendar`] — which day falls where in a month.
//! - [`idle`] — what the shell does when the person stops using the session:
//!   screen off, then lock and suspend, failing closed without a locker.
//! - [`nightlight`] — the fixed night-light whitepoint, its bounded transition
//!   and the gamma ramps a compositor takes.
//! - [`session`] — the two-state switch and the session requests a person
//!   cannot take back.
//! - [`wallpaper`] — which image an output shows, how it is laid on the
//!   output, and the next or previous image of a folder.
//! - [`weather`] — the one request the shell makes, how its answer is read, and
//!   when a reading stops counting.
//! - [`glance`] — load levels and today's date for the bar's readings.

#![forbid(unsafe_code)]

pub mod bounded;
pub mod calendar;
pub mod coalesce;
pub mod glance;
pub mod idle;
pub mod launcher;
pub mod nightlight;
pub mod notifications;
pub mod session;
pub mod settings;
pub mod wallpaper;
pub mod weather;
pub mod workspace_groups;
pub mod workspace_map;
