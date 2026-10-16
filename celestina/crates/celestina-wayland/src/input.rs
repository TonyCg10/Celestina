//! Input events the runtime delivers to a shell, in logical pixels of the
//! surface that received them.
#![forbid(unsafe_code)]

use crate::popup::{CloseReason, PopupId};

/// The modifier keys held while a key event happened, from xkb's effective
/// state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

/// The cursor a shell asks for while the pointer is over its surface, sent
/// through `wp_cursor_shape_v1` when the compositor offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    Pointer,
    Text,
}

/// One input event on one surface.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    PointerMotion {
        x: f32,
        y: f32,
    },
    PointerPress {
        x: f32,
        y: f32,
        button: u32,
        /// The serial of the press: the grab serial of a popup it opens.
        serial: u32,
    },
    PointerRelease {
        x: f32,
        y: f32,
        button: u32,
        serial: u32,
    },
    PointerLeave,
    /// A scroll, in logical pixels per event, at the pointer position.
    Axis {
        x: f32,
        y: f32,
        horizontal: f32,
        vertical: f32,
    },
    Key {
        keysym: u32,
        utf8: Option<String>,
        modifiers: Modifiers,
        pressed: bool,
        serial: u32,
    },
    /// The keyboard entered this surface.
    FocusEnter,
    /// The keyboard left this surface (not for a move into its own popup).
    FocusLost,
    /// Popup `id` of this surface is gone; `reason` says why (a
    /// compositor dismissal is a click outside the client).
    PopupClosed {
        id: PopupId,
        reason: CloseReason,
    },
}
