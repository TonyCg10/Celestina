//! Input events the runtime delivers to a shell, in logical pixels of the
//! surface that received them.
#![forbid(unsafe_code)]

use std::{fmt, time::Instant};

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
/// `Debug` is written by hand: a key press, the input method's text and the
/// clipboard's text can be a password (in the session lock a key event is
/// always what a person types), and the runtime logs an event it cannot
/// deliver, so the variants that can carry typed text print `<redacted>` in
/// their place, however the event is formatted (`?ev`, `{ev:?}`, `{ev:#?}`).
/// See the `Debug` impl below.
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
    /// The keyboard entered this surface, with the serial of the
    /// `wl_keyboard.enter` and the moment the runtime handled it (a
    /// monotonic clock). A key whose press carries an older serial began
    /// before the enter.
    FocusEnter {
        serial: u32,
        at: Instant,
    },
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

/// A stand-in that prints as `<redacted>` wherever typed text would be.
struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Whether a key press can name a typed character: its text has one that is
/// not a control character, or its keysym is outside the function, cursor,
/// modifier and function-key ranges (keypad digits and operators included in
/// "can name"). The keysym of a letter is the letter, so it is redacted with
/// the text; Return, Escape, Backspace, the arrows and the like stay
/// readable in a log.
fn names_a_character(keysym: u32, utf8: Option<&str>) -> bool {
    let text = utf8.is_some_and(|s| s.chars().any(|c| !c.is_control()));
    let function_key = (0xff00..=0xffff).contains(&keysym) && !(0xffaa..=0xffbd).contains(&keysym);
    text || !function_key
}

impl fmt::Debug for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputEvent::PointerMotion { x, y } => f
                .debug_struct("PointerMotion")
                .field("x", x)
                .field("y", y)
                .finish(),
            InputEvent::PointerPress {
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
            InputEvent::PointerRelease {
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
            InputEvent::PointerLeave => f.write_str("PointerLeave"),
            InputEvent::Axis {
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
            InputEvent::Key {
                keysym,
                utf8,
                modifiers,
                pressed,
                serial,
            } => {
                let mut key = f.debug_struct("Key");
                if names_a_character(*keysym, utf8.as_deref()) {
                    key.field("keysym", &Redacted);
                } else {
                    key.field("keysym", keysym);
                }
                // Not even whether the key produced text.
                let _ = utf8;
                key.field("utf8", &Redacted);
                key.field("modifiers", modifiers)
                    .field("pressed", pressed)
                    .field("serial", serial)
                    .finish()
            }
            InputEvent::Modifiers(modifiers) => {
                f.debug_tuple("Modifiers").field(modifiers).finish()
            }
            InputEvent::FocusEnter { serial, at } => f
                .debug_struct("FocusEnter")
                .field("serial", serial)
                .field("at", at)
                .finish(),
            InputEvent::FocusLost => f.write_str("FocusLost"),
            InputEvent::PopupClosed { id, reason } => f
                .debug_struct("PopupClosed")
                .field("id", id)
                .field("reason", reason)
                .finish(),
            // The cursor is a byte offset into the text: its length.
            InputEvent::Preedit { .. } => f
                .debug_struct("Preedit")
                .field("text", &Redacted)
                .field("cursor", &Redacted)
                .finish(),
            InputEvent::CommitText(_) => f.debug_tuple("CommitText").field(&Redacted).finish(),
            InputEvent::Paste(_) => f.debug_tuple("Paste").field(&Redacted).finish(),
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

#[cfg(feature = "gl")]
/// The surface a keyboard enter or leave names, as the runtime resolved it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusTarget {
    /// A layer surface: its shell is told.
    Layer(SurfaceId),
    /// An `xdg_popup`: the focus moves, the shell is not told.
    Popup(SurfaceId),
}

#[cfg(feature = "gl")]
impl FocusTarget {
    fn surface(self) -> SurfaceId {
        match self {
            FocusTarget::Layer(id) | FocusTarget::Popup(id) => id,
        }
    }
}

#[cfg(feature = "gl")]
/// Which surface holds the keyboard, and the events its enters and leaves
/// deliver. Pure: the handlers resolve the `wl_surface`, this decides.
#[derive(Debug, Default)]
pub(crate) struct KeyboardFocus {
    surface: Option<SurfaceId>,
}

#[cfg(feature = "gl")]
impl KeyboardFocus {
    /// The surface that holds the keyboard now.
    pub(crate) fn surface(&self) -> Option<SurfaceId> {
        self.surface
    }

    /// `wl_keyboard.enter`: the focus moves to `target` (to nothing for a
    /// surface that is not ours). A layer surface is told, with the enter's
    /// serial and the moment.
    pub(crate) fn enter(
        &mut self,
        target: Option<FocusTarget>,
        serial: u32,
        at: Instant,
    ) -> Option<InputEvent> {
        self.surface = target.map(FocusTarget::surface);
        match target {
            Some(FocusTarget::Layer(_)) => Some(InputEvent::FocusEnter { serial, at }),
            Some(FocusTarget::Popup(_)) | None => None,
        }
    }

    /// `wl_keyboard.leave`: a surface that did not hold the keyboard
    /// changes nothing and tells nobody. `popup_grab` says a popup of this
    /// layer surface is open, so the keyboard moved into it: the focus is
    /// gone but the page was not left.
    pub(crate) fn leave(&mut self, target: FocusTarget, popup_grab: bool) -> Option<InputEvent> {
        if self.surface != Some(target.surface()) {
            return None;
        }
        self.surface = None;
        match target {
            FocusTarget::Layer(_) if !popup_grab => Some(InputEvent::FocusLost),
            FocusTarget::Layer(_) | FocusTarget::Popup(_) => None,
        }
    }

    /// A surface was destroyed: it no longer holds the keyboard.
    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        if self.surface == Some(surface) {
            self.surface = None;
        }
    }
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
    #[cfg(feature = "gl")]
    use std::time::Duration;

    use super::*;

    /// What the runtime's log site (`tracing::debug!(?ev, ..)`) formats.
    fn logged(ev: &InputEvent) -> String {
        format!("{:?}", tracing::field::debug(ev))
    }

    fn key(keysym: u32, utf8: Option<&str>) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: utf8.map(str::to_owned),
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 4242,
        }
    }

    #[test]
    fn input_event_debug_redacts_typed_text() {
        // Every variant that can carry typed text, with text that must not
        // appear and the keysym of a typed letter ('h' = 0x68 = 104).
        let secret = "hunter2\u{e9}";
        let typed = [
            key(0x68, Some("h")),
            key(0x68, Some(secret)),
            // A key with no text still names a letter by its keysym.
            key(0x68, None),
            // Keypad digits are typed text too (KP_5).
            key(0xffb5, Some("5")),
            InputEvent::Preedit {
                text: secret.to_owned(),
                cursor: 7,
            },
            InputEvent::CommitText(secret.to_owned()),
            InputEvent::Paste(secret.to_owned()),
        ];
        for ev in &typed {
            for text in [format!("{ev:?}"), format!("{ev:#?}"), logged(ev)] {
                assert!(text.contains("<redacted>"), "{text}");
                for leaked in ["hunter2", "\u{e9}", "\"h\"", "104", "0x68", "68", "1000"] {
                    assert!(!text.contains(leaked), "{leaked:?} leaked in {text}");
                }
            }
        }
        // The preedit's cursor is the text's length: redacted too.
        let preedit = InputEvent::Preedit {
            text: "abcdef".to_owned(),
            cursor: 6,
        };
        assert!(!format!("{preedit:?}").contains('6'));
        // The key's other fields stay readable for debugging.
        let text = format!("{:?}", key(0x68, Some("h")));
        assert!(
            text.contains("4242") && text.contains("pressed: true"),
            "{text}"
        );
    }

    #[test]
    fn input_event_debug_keeps_the_events_that_carry_no_text_readable() {
        // Return, Escape and the arrows say what the person did, not what
        // they typed, and they are what a log reader needs.
        let text = format!("{:?}", key(0xff0d, Some("\r")));
        assert!(text.contains(&0xff0d.to_string()), "{text}");
        let text = format!("{:?}", key(0xff1b, None));
        assert!(text.contains(&0xff1b.to_string()), "{text}");
        assert_eq!(format!("{:?}", InputEvent::FocusLost), "FocusLost");
        assert_eq!(format!("{:?}", InputEvent::PointerLeave), "PointerLeave");
        let motion = format!("{:?}", InputEvent::PointerMotion { x: 1.5, y: 2.5 });
        assert!(motion.contains("1.5") && motion.contains("2.5"), "{motion}");
    }

    #[cfg(feature = "gl")]
    fn layer(n: u32) -> FocusTarget {
        FocusTarget::Layer(SurfaceId(n))
    }

    #[cfg(feature = "gl")]
    #[test]
    fn enter_then_leave_is_delivered_in_order() {
        let mut focus = KeyboardFocus::default();
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(30);
        assert_eq!(focus.surface(), None);

        // The enter carries its serial and moment.
        assert_eq!(
            focus.enter(Some(layer(1)), 77, t0),
            Some(InputEvent::FocusEnter { serial: 77, at: t0 })
        );
        assert_eq!(focus.surface(), Some(SurfaceId(1)));
        assert_eq!(focus.leave(layer(1), false), Some(InputEvent::FocusLost));
        assert_eq!(focus.surface(), None);

        // The keyboard moves from surface 1 to surface 2: leave, then enter.
        assert!(focus.enter(Some(layer(1)), 78, t0).is_some());
        assert_eq!(focus.leave(layer(1), false), Some(InputEvent::FocusLost));
        assert_eq!(
            focus.enter(Some(layer(2)), 79, t1),
            Some(InputEvent::FocusEnter { serial: 79, at: t1 })
        );
        assert_eq!(focus.surface(), Some(SurfaceId(2)));

        // Into a popup of surface 2 and out again: the page is not left.
        assert_eq!(focus.leave(layer(2), true), None);
        assert_eq!(focus.surface(), None);
        assert_eq!(
            focus.enter(Some(FocusTarget::Popup(SurfaceId(9))), 80, t1),
            None
        );
        assert_eq!(focus.surface(), Some(SurfaceId(9)));
        assert_eq!(focus.leave(FocusTarget::Popup(SurfaceId(9)), false), None);
        assert_eq!(
            focus.enter(Some(layer(2)), 81, t1),
            Some(InputEvent::FocusEnter { serial: 81, at: t1 })
        );
    }

    #[cfg(feature = "gl")]
    #[test]
    fn leave_of_an_unfocused_surface_is_ignored() {
        let mut focus = KeyboardFocus::default();
        let now = Instant::now();
        // Nothing focused: a stray leave tells nobody.
        assert_eq!(focus.leave(layer(1), false), None);
        // Surface 2 holds the keyboard; surface 1's late leave changes nothing.
        assert!(focus.enter(Some(layer(2)), 5, now).is_some());
        assert_eq!(focus.leave(layer(1), false), None);
        assert_eq!(focus.surface(), Some(SurfaceId(2)));
        // A surface that is not ours takes the keyboard: nobody is told.
        assert_eq!(focus.enter(None, 6, now), None);
        assert_eq!(focus.surface(), None);
        assert_eq!(focus.leave(layer(2), false), None);
        // A destroyed surface no longer holds it either, and a different
        // destroyed surface changes nothing.
        assert!(focus.enter(Some(layer(3)), 7, now).is_some());
        focus.forget(SurfaceId(4));
        assert_eq!(focus.surface(), Some(SurfaceId(3)));
        focus.forget(SurfaceId(3));
        assert_eq!(focus.surface(), None);
        assert_eq!(focus.leave(layer(3), false), None);
    }
}

#[cfg(test)]
mod lock_profile_tests {
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
    fn the_other_events_keep_a_readable_debug_format() {
        assert_eq!(
            format!("{:?}", InputEvent::PointerMotion { x: 1.0, y: 2.5 }),
            "PointerMotion { x: 1.0, y: 2.5 }"
        );
        assert_eq!(format!("{:?}", InputEvent::PointerLeave), "PointerLeave");
        // Typed text is redacted in the input method's and the clipboard's
        // events too (it can be a password).
        assert_eq!(
            format!("{:?}", InputEvent::Paste("x".to_owned())),
            "Paste(<redacted>)"
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
