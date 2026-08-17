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
//! - [`command`] — the host's typed, bounded requests, the rejections that
//!   answer the ones a helper cannot serve, and what became of the ones it
//!   accepted.
//! - [`connectivity`] — what the network and Bluetooth indicators may be asked
//!   to do, which identities may be acted on, and what the machine must show
//!   before a request counts as done.
//! - [`pending`] — requests already carried out and still waiting to be
//!   confirmed by a later observation.
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
//! - [`nightlight`] — the fixed warm whitepoint and the bounded monotonic
//!   gamma transition used to reach and leave it.
//! - [`weather`] — the one thing that leaves the machine, and how little goes
//!   with it.
//! - [`calendar`] — which day falls where in a month, computed rather than
//!   fetched.
//! - [`handover`] — what the old shell still does for this session, and what it
//!   would take to stop needing it.
//! - [`inventory`] — what a listing tool answered, the difference between a
//!   tool that is absent and one that is merely slow, and what a panel may
//!   show while it is not answering.
//! - [`appearance`] — what this session looks like, in the terms the settings
//!   portal asks in.
//! - [`niri_colours`] — the compositor's own colours, generated from the same
//!   sealed tokens the panel paints with so the two cannot drift.
//! - [`wallpaper`] — which image belongs on which screen, and what an output
//!   with none shows instead.
//! - [`settings`] — the only state this shell owns rather than reads, and the
//!   rule that a choice is published only once it is durable.
//! - [`session`] — what a key binding may ask the session to become: the typed
//!   verbs, the bounds their options must satisfy, and what a step leaves a
//!   level at.
//! - [`sysmon`] — what `/proc` says about CPU and memory, and what counts as a
//!   load worth noticing.
//! - [`workspace_groups`] — which monitor a workspace belongs to once that
//!   monitor is off and the compositor has stopped saying so, and the rule that
//!   keeps a displaced observation from overwriting the answer.
//! - [`workspace_map`] — what a workspace holds, folded into the columns and
//!   rows it really has, as shares rather than pixels.
//! - [`runtime`] — the aggregate those three add up to: which providers a
//!   helper carries, what they last said, and when the host is told.
//!
//! Nothing here knows Qt, QML or any particular provider. Time arrives as a
//! millisecond stamp and IO as a `Write`, so every rule above is testable
//! without a process, a socket or a clock.

pub mod appearance;
pub mod audio;
pub mod bluetooth;
pub mod brightness;
pub mod calendar;
pub mod clipboard;
pub mod coalesce;
pub mod command;
pub mod connectivity;
pub mod diagnostics;
pub mod handover;
pub mod inventory;
pub mod journal;
pub mod launcher;
pub mod lines;
pub mod media;
pub mod network;
pub mod nightlight;
pub mod niri_colours;
pub mod notifications;
pub mod pending;
pub mod power;
pub mod runtime;
pub mod session;
pub mod settings;
pub mod snapshot;
pub mod sysmon;
pub mod wallpaper;
pub mod weather;
pub mod workspace_groups;
pub mod workspace_map;

/// Truncates hostile or accidental text to a bounded prefix.
///
/// The limit counts UTF-16 code units, because that is what the Qt host counts
/// when it revalidates the same field. Counting Unicode scalars here instead
/// would let text made of astral-plane characters — emoji — pass this bound and
/// fail the host's, and the host rejects the whole frame rather than one field,
/// so every provider's reading would freeze over one long title. Characters are
/// taken whole, so neither a multi-byte boundary nor a surrogate pair is ever
/// split, and the result can therefore be one unit shorter than the limit.
#[must_use]
pub fn bounded(text: &str, limit: usize) -> String {
    let mut kept = String::new();
    let mut units = 0;
    for character in text.chars() {
        units += character.len_utf16();
        if units > limit {
            break;
        }
        kept.push(character);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::bounded;

    #[test]
    fn bounding_counts_characters_not_bytes() {
        assert_eq!(bounded("niñez", 3), "niñ");
        assert_eq!(bounded("short", 50), "short");
    }

    #[test]
    fn bounding_counts_the_units_the_host_counts() {
        // Every one of these is a single character and two UTF-16 code units,
        // so a limit of four admits two of them and no more.
        let astral = "😀😀😀";
        assert_eq!(bounded(astral, 4), "😀😀");
        assert_eq!(bounded(astral, 6), astral);
        // A limit that would land inside a surrogate pair keeps the character
        // out rather than splitting it.
        assert_eq!(bounded(astral, 5), "😀😀");
        assert_eq!(bounded(astral, 1), "");
    }
}
