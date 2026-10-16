//! What reaches the event loop from the shell's workers.
//!
//! Every provider runs on its own thread and publishes typed values through
//! the loop's `celestina_wayland::Sender<Message>`; the loop is the only place
//! that touches a surface (design §5.1). A variant arrives with the piece that
//! owns its type: the command verbs came with the command surface
//! (`command.rs`); notifications come with CEL-1-F, the tray with CEL-1-G and
//! the desktop index with CEL-1-E. The launcher's close is the one message the
//! app sends itself (see [`Message::LauncherClosed`]).

use celestina_core::desktop_entry::DesktopEntry;
use celestina_wayland::{OutputId, SurfaceId};

use crate::command::Verb;
use crate::launcher::Switch;
use crate::providers::niri::{NiriModel, NiriRequest};

/// One value a worker hands to the event loop.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// The compositor's reduced state, sent whole whenever it changed (at most
    /// once per coalescing window), and empty when the connection is lost.
    Niri(NiriModel),
    /// The suite's appearance file changed: reduced motion and text scale.
    Appearance(celestina_settings::Appearance),
    /// A compositor request was refused because the request queue was full or
    /// its worker had stopped; the loop shows the failure instead of queueing.
    NiriRequestRefused(NiriRequest),
    /// A verb of `org.celestina.Shell1`, from the bus worker.
    Command(Verb),
    /// The session's applications the launcher offers, from the desktop
    /// index's worker, whenever they changed.
    DesktopIndex(Vec<DesktopEntry>),
    /// The launcher on `surface` of `output` closed during an input call,
    /// which cannot destroy a surface; the app posts this to itself and
    /// destroys it here, then runs the `switch` that was chosen, if any
    /// (opening an Island page needs the Island's surface, which an input
    /// call cannot reach either).
    LauncherClosed {
        surface: SurfaceId,
        output: OutputId,
        switch: Option<Switch>,
    },
}
