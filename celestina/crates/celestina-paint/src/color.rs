//! Colours: sRGB bytes plus a straight alpha in 0..1, as the style's tokens
//! are written (`#rrggbb` and an opacity).
#![forbid(unsafe_code)]

/// An sRGB colour with straight (not premultiplied) alpha in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8, pub f32);

/// A colour literal that is not `#rrggbb`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{literal:?} is not a #rrggbb colour")]
pub struct ColorError {
    pub literal: String,
}

impl Color {
    /// Parses `#rrggbb` (the `#` is optional) as an opaque colour.
    pub fn hex(rgb: &str) -> Result<Color, ColorError> {
        let digits = rgb.strip_prefix('#').unwrap_or(rgb);
        let refuse = || ColorError {
            literal: rgb.to_owned(),
        };
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(refuse());
        }
        let channel = |at: usize| {
            digits
                .get(at..at + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(refuse)
        };
        Ok(Color(channel(0)?, channel(2)?, channel(4)?, 1.0))
    }

    /// The same colour at alpha `a`, clamped to `0.0..=1.0` (NaN is 0).
    pub fn with_alpha(self, a: f32) -> Color {
        Color(self.0, self.1, self.2, unit(a))
    }
}

/// Clamps to `0.0..=1.0`, mapping NaN to 0.
pub(crate) fn unit(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_with_and_without_the_hash() {
        assert_eq!(Color::hex("#0b0c10"), Ok(Color(0x0b, 0x0c, 0x10, 1.0)));
        assert_eq!(Color::hex("3E91FF"), Ok(Color(0x3e, 0x91, 0xff, 1.0)));
    }

    #[test]
    fn hex_refuses_anything_but_six_hex_digits() {
        for bad in [
            "",
            "#",
            "#0b0c1",
            "#0b0c100",
            "#0b0c1g",
            "#+b0c10",
            "#0b0c10\n",
            "\u{e9}0c10a",
        ] {
            assert!(Color::hex(bad).is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn with_alpha_replaces_only_the_alpha() {
        let c = Color(1, 2, 3, 1.0).with_alpha(0.7);
        assert_eq!(c, Color(1, 2, 3, 0.7));
        assert_eq!(c.with_alpha(2.0).3, 1.0, "clamped");
        assert_eq!(c.with_alpha(-1.0).3, 0.0, "clamped");
    }
}
