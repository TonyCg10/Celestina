//! Input events the runtime delivers to a shell, in logical pixels of the
//! surface that received them.
#![forbid(unsafe_code)]

use std::fmt;

use smithay_client_toolkit::seat::{
    keyboard::{KeyEvent, Modifiers as SctkModifiers},
    pointer::PointerEventKind,
};

use crate::ids::{CloseReason, PopupId};
#[cfg(feature = "gl")]
use crate::{geometry::Rect, ids::SurfaceId};

/// The modifier keys held while a key event happened, from xkb's effective
/// state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
    /// Caps Lock is latched (xkb's `Lock` modifier), so a person typing a
    /// password can be told before the attempt fails.
    pub caps_lock: bool,
}

impl From<&SctkModifiers> for Modifiers {
    fn from(modifiers: &SctkModifiers) -> Self {
        Self {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            logo: modifiers.logo,
            caps_lock: modifiers.caps_lock,
        }
    }
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
///
/// `Debug` is written by hand: a key event is what a person types, and in the
/// session lock that is a password, so the keysym and the text never reach a
/// log line however the event is formatted (`?ev`, `{ev:?}`, `{ev:#?}`).
#[derive(Clone, PartialEq)]
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
    /// The modifier state changed with no key event behind it (the
    /// compositor reports it when the keyboard enters a surface, and when a
    /// lock key latches). Delivered to the surface holding the keyboard focus
    /// by the lock profile only.
    Modifiers(Modifiers),
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

/// Stands for a value `InputEvent`'s `Debug` must not show.
struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Debug for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PointerMotion { x, y } => f
                .debug_struct("PointerMotion")
                .field("x", x)
                .field("y", y)
                .finish(),
            Self::PointerPress {
                x,
                y,
                button,
                serial,
            } => f
                .debug_struct("PointerPress")
                .field("x", x)
                .field("y", y)
                .field("button", button)
                .field("serial", serial)
                .finish(),
            Self::PointerRelease {
                x,
                y,
                button,
                serial,
            } => f
                .debug_struct("PointerRelease")
                .field("x", x)
                .field("y", y)
                .field("button", button)
                .field("serial", serial)
                .finish(),
            Self::PointerLeave => f.write_str("PointerLeave"),
            Self::Axis {
                x,
                y,
                horizontal,
                vertical,
            } => f
                .debug_struct("Axis")
                .field("x", x)
                .field("y", y)
                .field("horizontal", horizontal)
                .field("vertical", vertical)
                .finish(),
            // The keysym and the text are what was typed: never shown, not
            // even whether the key produced text.
            Self::Key {
                modifiers,
                pressed,
                serial,
                ..
            } => f
                .debug_struct("Key")
                .field("keysym", &Redacted)
                .field("utf8", &Redacted)
                .field("modifiers", modifiers)
                .field("pressed", pressed)
                .field("serial", serial)
                .finish(),
            Self::Modifiers(modifiers) => f.debug_tuple("Modifiers").field(modifiers).finish(),
            Self::FocusEnter => f.write_str("FocusEnter"),
            Self::FocusLost => f.write_str("FocusLost"),
            Self::PopupClosed { id, reason } => f
                .debug_struct("PopupClosed")
                .field("id", id)
                .field("reason", reason)
                .finish(),
            Self::Preedit { text, cursor } => f
                .debug_struct("Preedit")
                .field("text", text)
                .field("cursor", cursor)
                .finish(),
            Self::CommitText(text) => f.debug_tuple("CommitText").field(text).finish(),
            Self::Paste(text) => f.debug_tuple("Paste").field(text).finish(),
        }
    }
}

/// The key event of a keyboard handler, in the toolkit's own type.
pub(crate) fn key_input(
    event: KeyEvent,
    modifiers: Modifiers,
    pressed: bool,
    serial: u32,
) -> InputEvent {
    InputEvent::Key {
        keysym: event.keysym.raw(),
        utf8: event.utf8,
        modifiers,
        pressed,
        serial,
    }
}

/// The input event of a pointer event at `position` (logical px of its
/// surface); `None` for an axis event that carries no movement (source, stop
/// and discrete-only events).
pub(crate) fn pointer_input(kind: &PointerEventKind, position: (f64, f64)) -> Option<InputEvent> {
    let (x, y) = (position.0 as f32, position.1 as f32);
    Some(match *kind {
        PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
            InputEvent::PointerMotion { x, y }
        }
        PointerEventKind::Press { button, serial, .. } => InputEvent::PointerPress {
            x,
            y,
            button,
            serial,
        },
        PointerEventKind::Release { button, serial, .. } => InputEvent::PointerRelease {
            x,
            y,
            button,
            serial,
        },
        PointerEventKind::Leave { .. } => InputEvent::PointerLeave,
        PointerEventKind::Axis {
            horizontal,
            vertical,
            ..
        } => {
            if horizontal.absolute == 0.0 && vertical.absolute == 0.0 {
                return None;
            }
            InputEvent::Axis {
                x,
                y,
                horizontal: horizontal.absolute as f32,
                vertical: vertical.absolute as f32,
            }
        }
    })
}

/// What a shell asked of the seat through a `SurfaceCtl`, applied by the
/// runtime right after the shell call returns.
#[cfg(feature = "gl")]
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
#[cfg(feature = "gl")]
pub(crate) type SeatRequests = Vec<(SurfaceId, SeatRequest)>;

#[cfg(test)]
mod tests {
    use super::*;

    fn key(keysym: u32, utf8: Option<&str>) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: utf8.map(str::to_owned),
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 7,
        }
    }

    #[test]
    fn t_w3_key_debug_shows_no_keysym_or_text() {
        // Distinctive typed values, so a hit cannot be a coincidence with a
        // field name or a number of the event.
        let keysym: u32 = 0x1234_abcd;
        let typed = "Zq9!Xw";
        let ev = key(keysym, Some(typed));
        for shown in [
            format!("{ev:?}"),
            format!("{ev:#?}"),
            // The way the runtimes log it: a `tracing` field with `?`.
            format!("input for a surface that is gone: {ev:?}"),
        ] {
            assert!(shown.contains("Key"), "still names the event: {shown}");
            for secret in [
                typed,
                "Zq9",
                "Xw",
                &keysym.to_string(),
                &format!("{keysym:x}"),
                &format!("{keysym:X}"),
                &format!("{keysym:#x}"),
            ] {
                assert!(
                    !shown.contains(secret),
                    "{secret:?} must not appear in {shown}"
                );
            }
        }
    }

    #[test]
    fn key_debug_does_not_even_show_whether_the_key_produced_text() {
        let with_text = format!("{:?}", key(1, Some("a")));
        let without = format!("{:?}", key(2, None));
        assert_eq!(with_text, without);
        assert!(with_text.contains("serial: 7") && with_text.contains("pressed: true"));
    }

    #[test]
    fn the_other_events_keep_the_derived_debug_format() {
        assert_eq!(
            format!("{:?}", InputEvent::PointerMotion { x: 1.0, y: 2.5 }),
            "PointerMotion { x: 1.0, y: 2.5 }"
        );
        assert_eq!(format!("{:?}", InputEvent::FocusEnter), "FocusEnter");
        assert_eq!(format!("{:?}", InputEvent::PointerLeave), "PointerLeave");
        assert_eq!(
            format!("{:?}", InputEvent::Paste("x".to_owned())),
            "Paste(\"x\")"
        );
        assert_eq!(
            format!(
                "{:?}",
                InputEvent::PopupClosed {
                    id: PopupId(3),
                    reason: CloseReason::Dismissed
                }
            ),
            "PopupClosed { id: PopupId(3), reason: Dismissed }"
        );
    }

    #[test]
    fn caps_lock_is_reported_from_the_xkb_modifier_state() {
        // sctk fills `caps_lock` from xkb's effective `Lock` modifier; the
        // toolkit carries it over with the others.
        let locked = SctkModifiers {
            caps_lock: true,
            ..SctkModifiers::default()
        };
        let mapped = Modifiers::from(&locked);
        assert!(mapped.caps_lock);
        assert!(!mapped.shift && !mapped.ctrl && !mapped.alt && !mapped.logo);
        assert!(!Modifiers::from(&SctkModifiers::default()).caps_lock);
        let all = SctkModifiers {
            ctrl: true,
            alt: true,
            shift: true,
            caps_lock: true,
            logo: true,
            num_lock: true,
        };
        assert_eq!(
            Modifiers::from(&all),
            Modifiers {
                ctrl: true,
                alt: true,
                shift: true,
                logo: true,
                caps_lock: true
            }
        );
    }
}
