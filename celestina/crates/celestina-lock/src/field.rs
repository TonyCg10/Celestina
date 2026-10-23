// language-contract: allow-non-english
// (the tests type international input: accents, symbols, emoji)
//! The password field and the secret that leaves it (design §4.5, M2).
//!
//! The field holds what was typed as UTF-8 bytes in a buffer whose capacity
//! (512) is reserved up front and never exceeded, so no reallocation can leave
//! a copy behind; every byte that leaves the field (backspace, escape, the
//! submitted secret, drop) is zeroed. `Debug` shows neither the bytes nor
//! their count, and nothing here implements `Display`.
//!
//! A key is appended only if the result stays within [`SECRET_MAX`] bytes, so
//! the lock can never build a secret the verifier refuses as oversized (T-K1).
#![forbid(unsafe_code)]

use std::fmt;

use celestina_lock_proto::SECRET_MAX;
use zeroize::{Zeroize, Zeroizing};

/// A secret on its way to the verifier: owned bytes, zeroed on drop, no
/// `Clone`, and a `Debug` that shows nothing.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    /// The bytes, exactly as typed.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// How many bytes the secret has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the secret has no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A secret from bytes the caller built, for the tests and the rehearsal
    /// probe. The bytes are moved in, not copied.
    #[must_use]
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

/// What has been typed so far.
pub struct Field {
    bytes: Zeroizing<Vec<u8>>,
    chars: usize,
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Field(..)")
    }
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}

impl Field {
    /// An empty field with its whole capacity reserved.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: Zeroizing::new(Vec::with_capacity(SECRET_MAX)),
            chars: 0,
        }
    }

    /// Whether nothing has been typed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// How many characters (not bytes) have been typed: what the view shows
    /// as bullets.
    #[must_use]
    pub fn chars(&self) -> usize {
        self.chars
    }

    /// Appends the text of one key. Returns whether it was taken.
    ///
    /// Refused: empty text, any control character (Enter's `\r`, Tab, Escape,
    /// a NUL: a control key has no business in a secret), and anything that
    /// would take the field past [`SECRET_MAX`] bytes.
    pub fn push(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        if self.bytes.len().saturating_add(text.len()) > SECRET_MAX {
            return false;
        }
        self.bytes.extend_from_slice(text.as_bytes());
        self.chars = self.chars.saturating_add(text.chars().count());
        true
    }

    /// Removes the last character.
    pub fn pop(&mut self) {
        // Continuation bytes are `10xxxxxx`; the character starts at the first
        // byte that is not one.
        while let Some(last) = self.bytes.last_mut() {
            let continuation = *last & 0xC0 == 0x80;
            *last = 0;
            self.bytes.pop();
            if !continuation {
                self.chars = self.chars.saturating_sub(1);
                return;
            }
        }
        self.chars = 0;
    }

    /// Forgets everything typed.
    pub fn clear(&mut self) {
        self.bytes.zeroize();
        self.chars = 0;
    }

    /// Hands the typed bytes over as a [`Secret`] and leaves an empty field
    /// with a fresh reserved buffer.
    pub fn take(&mut self) -> Secret {
        self.chars = 0;
        Secret(std::mem::replace(
            &mut self.bytes,
            Zeroizing::new(Vec::with_capacity(SECRET_MAX)),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_clear_and_take() {
        let mut field = Field::new();
        assert!(field.is_empty());
        assert!(field.push("a"));
        assert!(field.push("ñ"));
        assert!(field.push("€"));
        assert!(field.push("😀"));
        assert_eq!(field.chars(), 4);
        field.pop();
        assert_eq!(field.chars(), 3);
        field.pop();
        field.pop();
        assert_eq!(field.chars(), 1);
        let secret = field.take();
        assert_eq!(secret.as_bytes(), b"a");
        assert!(field.is_empty());
        assert_eq!(field.chars(), 0);
        // Pop on an empty field is a no-op.
        field.pop();
        assert_eq!(field.chars(), 0);
        assert!(field.push("x"));
        field.clear();
        assert!(field.is_empty());
    }

    #[test]
    fn control_characters_and_empty_text_are_not_taken() {
        let mut field = Field::new();
        for text in [
            "", "\r", "\n", "\t", "\u{1b}", "\0", "a\0", "\u{7f}", "\u{85}",
        ] {
            assert!(!field.push(text), "{text:?}");
        }
        assert!(field.is_empty());
        assert!(field.push(" "), "a space is a password character");
    }

    #[test]
    fn the_field_never_grows_past_512_bytes_or_reallocates() {
        let mut field = Field::new();
        let capacity = field.bytes.capacity();
        // 256 two-byte characters fill it exactly.
        for _ in 0..256 {
            assert!(field.push("é"));
        }
        assert_eq!(field.bytes.len(), 512);
        // The 513th byte, as one byte or as part of a wider character, is
        // refused.
        assert!(!field.push("a"));
        assert!(!field.push("é"));
        assert!(!field.push("€"));
        assert_eq!(field.bytes.len(), 512);
        assert_eq!(field.bytes.capacity(), capacity, "no reallocation");
        assert_eq!(field.chars(), 256);
        // Removing a character makes room for one of the same width only.
        field.pop();
        assert!(!field.push("€"));
        assert!(field.push("é"));
    }

    #[test]
    fn debug_shows_nothing_of_the_field_or_the_secret() {
        let mut field = Field::new();
        assert!(field.push("hunter2"));
        assert_eq!(format!("{field:?}"), "Field(..)");
        let secret = field.take();
        assert_eq!(format!("{secret:?}"), "Secret(..)");
    }
}
