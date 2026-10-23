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
//! - [`audio`], [`brightness`] — what the session's sound and a monitor's
//!   backlight are at, read from `wpctl`, `pactl` and `ddcutil` text, and the
//!   step arithmetic for changing them.
//! - [`osd`] — when the on-screen display shows, what, and for how long.
//! - [`bounded`] — the one bound on text from another process.
//! - [`coalesce`] — how often a changing provider may reach the event loop.
//! - [`connectivity`] — what the network, Bluetooth, battery and power-profile
//!   pieces know, the closed list of actions their pages may ask for, and how
//!   each is checked and confirmed.
//! - [`notifications`] — identity, replacement, expiry, actions, capabilities
//!   and how much history is kept and shown.
//! - [`launcher`] — which application a person means by part of a name.
//! - [`workspace_groups`] — which monitor a workspace belongs to once that
//!   monitor is gone.
//! - [`workspace_map`] — what a workspace holds, as shares of its columns.
//! - [`settings`] — the shell's own durable choice (do-not-disturb) and the
//!   rule that a choice is published only once it is written.
//! - [`calendar`] — which day falls where in a month.
//! - [`clipboard`] — what the clipboard history may keep, for how long, and
//!   the text-only form it is persisted in.
//! - [`idle`] — what the shell does when the person stops using the session:
//!   screen off, then lock and suspend, failing closed without a locker.
//! - [`media`] — which MPRIS player is shown, the pin over it, and the bounds
//!   on what a player says.
//! - [`nightlight`] — the fixed night-light whitepoint, its bounded transition
//!   and the gamma ramps a compositor takes.
//! - [`melibea`] — Melibea's socket protocol and the ordered projection of
//!   minimized windows.
//! - [`phone`] — which device is the phone and what the bar shows of it.
//! - [`session`] — the two-state switch and the session requests a person
//!   cannot take back.
//! - [`wallpaper`] — which image an output shows, how it is laid on the
//!   output, and the next or previous image of a folder.
//! - [`weather`] — the one request the shell makes, how its answer is read, and
//!   when a reading stops counting.
//! - [`glance`] — load levels and today's date for the bar's readings.
//! - [`lock`] — when the shell's own session lock is started, what its
//!   confirmations mean, and the only way to mint a proof that the session is
//!   covered before it suspends.

#![forbid(unsafe_code)]

pub mod audio;
pub mod bounded;
pub mod brightness;
pub mod calendar;
pub mod clipboard;
pub mod coalesce;
pub mod connectivity;
pub mod glance;
pub mod idle;
pub mod keybinds;
pub mod launcher;
pub mod lock;
pub mod media;
pub mod melibea;
pub mod nightlight;
pub mod notifications;
pub mod osd;
pub mod phone;
pub mod session;
pub mod settings;
pub mod thumbnail;
pub mod wallpaper;
pub mod weather;
pub mod workspace_groups;
pub mod workspace_map;
