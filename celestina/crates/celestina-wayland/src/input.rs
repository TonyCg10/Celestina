//! Input events the runtime delivers to a shell, in logical pixels of the
//! surface that received them.
#![forbid(unsafe_code)]

use crate::{
    geometry::Rect,
    popup::{CloseReason, PopupId},
    runtime::SurfaceId,
};

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
    /// The input method's composition in progress, to show at the cursor in
    /// place of the previous one, with the caret at byte `cursor` of `text`
    /// (a character boundary); empty text ends the composition. Only while
    /// the surface asked for text input (`SurfaceCtl::set_text_input_enabled`).
    Preedit {
        text: String,
        cursor: usize,
    },
    /// The input method's final text, replacing the composition (if any).
    CommitText(String),
    /// The clipboard's text, read for `SurfaceCtl::request_paste`.
    Paste(String),
}

/// What a shell asked of the seat through a `SurfaceCtl`, applied by the
/// runtime right after the shell call returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SeatRequest {
    /// IME text input on (or off) while the surface has the text-input focus.
    TextInput(bool),
    /// The caret's rectangle, in logical px of the surface.
    CursorRect(Rect),
    /// Read the clipboard's text and deliver it as `InputEvent::Paste`.
    Paste,
}

/// The seat requests recorded during shell calls, in order.
pub(crate) type SeatRequests = Vec<(SurfaceId, SeatRequest)>;
