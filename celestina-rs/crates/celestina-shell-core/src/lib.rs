//! The shell's helper-side core: how a Celestina helper talks to its Qt host.
//!
//! Two helpers already speak this protocol — the Niri event-stream adapter and
//! the aggregate provider adapter that feeds the panel's widgets — and later
//! shell-owned, non-Qt services extend the second one rather than starting a
//! third runtime. What they share, and only what they share, lives here:
//!
//! - [`lines`] — bounded line framing for host input and the one serialized
//!   writer every frame leaves through, so two producers can never interleave
//!   a line.
//! - [`snapshot`] — the provider envelope: who published what, in which
//!   generation, with the bounds that keep a helper's output finite.
//! - [`command`] — the host's typed, bounded requests and the rejections that
//!   answer the ones a helper cannot serve.
//! - [`coalesce`] — how often a changing provider is allowed to reach the GUI
//!   thread.
//! - [`clipboard`] — what counts as clipboard history worth keeping.
//! - [`audio`] — what the session's audio device is set to, and whether it is
//!   silenced.
//! - [`brightness`] — what a monitor says about itself over DDC, and the three
//!   states a slow, optional conversation leaves a panel in.
//! - [`network`], [`bluetooth`], [`power`] — how the session is online, what is
//!   connected to it, and which power profile it is running.
//! - [`launcher`] — which application a person means when they type part of a
//!   name.
//! - [`notifications`] — what the session's applications are trying to say:
//!   identity, replacement, expiry, actions, capabilities and how much is kept.
//! - [`session`] — what a key binding may ask the session to become: the typed
//!   verbs, the bounds their options must satisfy, and what a step leaves a
//!   level at.
//! - [`sysmon`] — what `/proc` says about CPU and memory, and what counts as a
//!   load worth noticing.
//! - [`runtime`] — the aggregate those three add up to: which providers a
//!   helper carries, what they last said, and when the host is told.
//!
//! Nothing here knows Qt, QML or any particular provider. Time arrives as a
//! millisecond stamp and IO as a `Write`, so every rule above is testable
//! without a process, a socket or a clock.

pub mod audio;
pub mod bluetooth;
pub mod brightness;
pub mod clipboard;
pub mod coalesce;
pub mod command;
pub mod launcher;
pub mod lines;
pub mod network;
pub mod notifications;
pub mod power;
pub mod runtime;
pub mod session;
pub mod snapshot;
pub mod sysmon;

/// Truncates hostile or accidental text to a bounded prefix, counting
/// characters so a multi-byte boundary is never split.
#[must_use]
pub fn bounded(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::bounded;

    #[test]
    fn bounding_counts_characters_not_bytes() {
        assert_eq!(bounded("niñez", 3), "niñ");
        assert_eq!(bounded("short", 50), "short");
    }
}
