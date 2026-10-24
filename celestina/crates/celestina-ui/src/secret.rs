//! The secret buffer and the field that fills it: what a password prompt
//! types into, as the polkit agent's design specifies in §6.5
//! (`docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`).
//!
//! [`Secret`] is a byte buffer of fixed capacity that never grows, so no
//! stale copy of a half-typed answer is left in a freed block; it wipes the
//! bytes of whatever leaves it and the whole buffer when it is dropped, and it
//! implements nothing that could print, copy, compare or serialise it.
//! [`SecretField`] is the view state of the field that edits one: it takes
//! character keys, Backspace, Enter and Escape and nothing else, and it shows
//! no per-character echo, only whether something was typed.
//!
//! Nothing here knows about polkit: the lock screen reuses both.
#![forbid(unsafe_code)]

use celestina_wayland::InputEvent;
use zeroize::{Zeroize, Zeroizing};

use crate::input::{XKB_KEY_ESCAPE, XKB_KEY_KP_ENTER, XKB_KEY_RETURN};

const XKB_KEY_BACKSPACE: u32 = 0xff08;

/// The most bytes of content a [`Secret`] holds: content plus the newline
/// that ends the answer line fits the 512-byte `fgets` of polkit's helper.
pub const MAX_SECRET_BYTES: usize = 510;

/// Why [`Secret::push`] kept a character out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecretRefused {
    /// A line feed, a carriage return or a NUL: the answer is one line and
    /// the helper's protocol ends it at the newline.
    #[error("the character cannot be part of a one-line answer")]
    Forbidden,
    /// The answer would pass `MAX_SECRET_BYTES`. It is never truncated.
    #[error("the answer is at its size limit")]
    Full,
}

/// A typed secret: no `Clone`, `Debug`, `Display`, `Serialize`,
/// `PartialEq` or `Deref`.
pub struct Secret {
    /// Always `MAX_SECRET_BYTES` long and never reallocated; the bytes past
    /// `len` are zero, so a dropped or half-used buffer holds no leftovers.
    buf: Zeroizing<Vec<u8>>,
    len: usize,
}

impl Secret {
    pub fn new() -> Secret {
        // The one allocation of the secret's life: `MAX_SECRET_BYTES` zeros
        // (the allocation is exactly that), only ever written within.
        Secret {
            buf: Zeroizing::new(vec![0; MAX_SECRET_BYTES]),
            len: 0,
        }
    }

    /// Appends `c`, or refuses it and changes nothing.
    pub fn push(&mut self, c: char) -> Result<(), SecretRefused> {
        if matches!(c, '\n' | '\r' | '\0') {
            return Err(SecretRefused::Forbidden);
        }
        let end = self.len + c.len_utf8();
        let mut encoded = [0u8; 4];
        let text = c.encode_utf8(&mut encoded);
        let stored = self
            .buf
            .get_mut(self.len..end)
            .map(|room| room.copy_from_slice(text.as_bytes()));
        // The stack copy of the character goes the way of the buffer's.
        encoded.zeroize();
        match stored {
            Some(()) => {
                self.len = end;
                Ok(())
            }
            None => Err(SecretRefused::Full),
        }
    }

    /// Removes the last character and wipes its bytes.
    pub fn pop(&mut self) {
        let Some(used) = self.buf.get_mut(..self.len) else {
            return;
        };
        // The content is valid UTF-8 by construction: the last character
        // starts at the last byte that is not a continuation byte.
        let start = used.iter().rposition(|b| b & 0xc0 != 0x80).unwrap_or(0);
        if let Some(removed) = used.get_mut(start..) {
            removed.zeroize();
        }
        self.len = start;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Wipes the used bytes and empties the buffer.
    pub fn clear(&mut self) {
        if let Some(used) = self.buf.get_mut(..self.len) {
            used.zeroize();
        }
        self.len = 0;
    }

    /// Writes the content and then the newline that ends the answer line, as
    /// two `write_all`s: no concatenated copy exists.
    pub fn write_line_to(&self, out: &mut impl std::io::Write) -> std::io::Result<()> {
        out.write_all(self.buf.get(..self.len).unwrap_or_default())?;
        out.write_all(b"\n")
    }

    /// Whether all of `text` still fits.
    pub(crate) fn has_room_for(&self, text: &str) -> bool {
        self.len + text.len() <= MAX_SECRET_BYTES
    }
}

impl Default for Secret {
    fn default() -> Self {
        Secret::new()
    }
}

/// What a [`SecretField`] shows of its secret: nothing else, so neither the
/// glyphs nor the length of the answer can be read off the screen or out of
/// an accessibility tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretFieldState {
    Empty,
    HasContent,
}

/// What a [`SecretField`] made of one event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretKey {
    /// Not a key the field takes; the card may use it (Tab, say).
    Ignored,
    /// The secret changed.
    Edited,
    /// A key the field takes, kept out of the secret (a newline, a NUL, the
    /// 511th byte).
    Refused,
    /// Enter on a non-empty field.
    Submit,
    /// Escape.
    Cancel,
}

/// The view state of a password field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SecretField {
    has_content: bool,
    focused: bool,
}

impl SecretField {
    pub fn new() -> SecretField {
        SecretField::default()
    }

    pub fn state(&self) -> SecretFieldState {
        if self.has_content {
            SecretFieldState::HasContent
        } else {
            SecretFieldState::Empty
        }
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Applies `ev` to `secret`. Only these edit it: a key press that types
    /// text (no Ctrl, Alt or Logo held) and Backspace. Enter on a non-empty
    /// field submits and Escape cancels, and the keyboard leaving wipes the
    /// secret. Everything else, paste, copy, cut, select-all, the input
    /// method's preedit and commit among it, does nothing.
    ///
    /// The event is taken by value so the text it carries, which can be the
    /// secret, is wiped here and not left to a plain drop.
    pub fn handle(&mut self, secret: &mut Secret, ev: InputEvent) -> SecretKey {
        let outcome = match ev {
            InputEvent::Key {
                keysym,
                utf8,
                modifiers,
                pressed: true,
                ..
            } => {
                let plain = !modifiers.ctrl && !modifiers.alt && !modifiers.logo;
                let outcome = match keysym {
                    XKB_KEY_ESCAPE => SecretKey::Cancel,
                    XKB_KEY_RETURN | XKB_KEY_KP_ENTER if plain && !secret.is_empty() => {
                        SecretKey::Submit
                    }
                    XKB_KEY_BACKSPACE if plain && !secret.is_empty() => {
                        secret.pop();
                        SecretKey::Edited
                    }
                    _ => match utf8.as_deref().filter(|_| plain) {
                        Some(text) => insert(secret, text),
                        None => SecretKey::Ignored,
                    },
                };
                wipe(utf8);
                outcome
            }
            InputEvent::Key { utf8, .. } => {
                wipe(utf8);
                SecretKey::Ignored
            }
            InputEvent::FocusEnter { .. } => {
                self.focused = true;
                SecretKey::Ignored
            }
            InputEvent::FocusLost => {
                // Keys typed before a loss are never kept.
                self.focused = false;
                secret.clear();
                SecretKey::Ignored
            }
            // The input method's text and the clipboard's: never accepted.
            InputEvent::Preedit { text, .. }
            | InputEvent::CommitText(text)
            | InputEvent::Paste(text) => {
                wipe(Some(text));
                SecretKey::Ignored
            }
            _ => SecretKey::Ignored,
        };
        self.has_content = !secret.is_empty();
        outcome
    }
}

/// Types `text`, all of it or none: a control character is not typed text
/// (Tab, a chord's control code), and a text that would pass the cap is
/// refused whole rather than cut.
fn insert(secret: &mut Secret, text: &str) -> SecretKey {
    if text.is_empty() || text.chars().any(char::is_control) {
        return SecretKey::Ignored;
    }
    if !secret.has_room_for(text) {
        return SecretKey::Refused;
    }
    for c in text.chars() {
        if secret.push(c).is_err() {
            return SecretKey::Refused;
        }
    }
    SecretKey::Edited
}

fn wipe(text: Option<String>) {
    if let Some(mut text) = text {
        text.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails to compile when `$ty` implements any of the traits. The two
    /// blanket impls make `some_item` ambiguous exactly when `$tr` holds
    /// (the trick of the `static_assertions` crate, written here so that the
    /// check needs no dev-dependency).
    macro_rules! assert_not_impl {
        ($ty:ty: $($tr:path),+ $(,)?) => {$({
            trait AmbiguousIfImpl<A> {
                fn some_item() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            impl<T: ?Sized + $tr> AmbiguousIfImpl<u8> for T {}
            let _ = <$ty as AmbiguousIfImpl<_>>::some_item;
        })+};
    }

    impl Secret {
        /// Every byte of the buffer, used or not.
        fn bytes(&self) -> &[u8] {
            &self.buf
        }
    }

    fn typed(text: &str) -> Secret {
        let mut secret = Secret::new();
        for c in text.chars() {
            assert_eq!(secret.push(c), Ok(()));
        }
        secret
    }

    #[test]
    fn secret_has_fixed_capacity_across_inserts() {
        let mut secret = Secret::new();
        let (capacity, address) = (secret.buf.capacity(), secret.buf.as_ptr());
        assert!(capacity >= MAX_SECRET_BYTES);
        for _ in 0..MAX_SECRET_BYTES {
            assert_eq!(secret.push('x'), Ok(()));
            assert_eq!(secret.buf.capacity(), capacity);
            assert_eq!(secret.buf.as_ptr(), address, "the buffer never moves");
        }
        // Past the cap, and through pop and clear: still the same block.
        assert_eq!(secret.push('x'), Err(SecretRefused::Full));
        secret.pop();
        secret.clear();
        assert_eq!(secret.buf.capacity(), capacity);
        assert_eq!(secret.buf.as_ptr(), address);
    }

    #[test]
    fn secret_pop_zeroes_the_removed_bytes() {
        // 'a', then a two-byte, a three-byte and a four-byte character.
        let mut secret = typed("a\u{e9}\u{20ac}\u{1f600}");
        assert!(secret.bytes()[10..].iter().all(|&b| b == 0));
        // Each pop zeroes the bytes of exactly the last character.
        for (new_end, last_kept) in [(6, 5), (3, 2), (1, 0)] {
            secret.pop();
            assert!(
                secret.bytes()[new_end..].iter().all(|&b| b == 0),
                "the tail past {new_end} is zero"
            );
            assert_ne!(secret.bytes()[last_kept], 0, "the rest is untouched");
        }
        assert_eq!(secret.bytes()[0], b'a');
        secret.pop();
        assert!(secret.is_empty());
        assert!(secret.bytes().iter().all(|&b| b == 0));
        // Pop on an empty secret is a no-op.
        secret.pop();
        assert!(secret.is_empty());
    }

    #[test]
    fn secret_clear_zeroes_the_used_length() {
        let mut secret = typed("hunter2 \u{e9}\u{1f600}");
        assert!(secret.bytes().iter().any(|&b| b != 0));
        secret.clear();
        assert!(secret.is_empty());
        assert!(secret.bytes().iter().all(|&b| b == 0));
        assert_eq!(secret.push('y'), Ok(()), "a cleared secret is reusable");
    }

    #[test]
    fn secret_is_not_clone_or_debug() {
        // `Serialize` cannot be named here (this crate has no serde); the
        // type has no derive and no impl, which this list and the review
        // check together.
        assert_not_impl!(
            Secret: Clone,
            std::fmt::Debug,
            std::fmt::Display,
            PartialEq,
            std::ops::Deref
        );
    }
}
