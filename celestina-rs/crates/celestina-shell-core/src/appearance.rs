//! What this session looks like, in the terms the rest of the desktop asks in.
//!
//! Applications do not read `CelestinaTheme`. They ask the desktop's settings
//! portal whether the session is dark and what its accent is, and if nobody
//! answers they guess — which is how a session ends up with one dark shell and
//! a handful of bright white dialogs.
//!
//! So the shell answers, from the same sealed values it paints with. The
//! conversions here are the whole of it: the portal speaks in an enumeration
//! and in floating-point components, the theme speaks in hex, and neither
//! should have to know about the other.

use crate::niri_colours::{is_literal_colour, SEALED};

/// The portal's `color-scheme` values. Named rather than bare numbers because
/// `1` meaning dark and `2` meaning light is not something to rediscover at a
/// call site.
pub const SCHEME_NO_PREFERENCE: u32 = 0;
pub const SCHEME_PREFER_DARK: u32 = 1;
pub const SCHEME_PREFER_LIGHT: u32 = 2;

/// The namespace the appearance keys live under.
pub const NAMESPACE: &str = "org.freedesktop.appearance";

/// What this session is. Celestina's sealed palette is a dark one and there is
/// no light scheme to switch to, so this is a statement rather than a setting:
/// answering "no preference" would invite an application to pick the bright
/// variant it has, beside a shell that has none.
#[must_use]
pub fn colour_scheme() -> u32 {
    SCHEME_PREFER_DARK
}

/// Reads `#rrggbb` or `#rrggbbaa` into components between 0 and 1.
///
/// Alpha is dropped: the portal's accent is a colour, not a composite, and an
/// application blending it against its own background needs the colour rather
/// than this shell's transparency.
#[must_use]
pub fn components(hex: &str) -> Option<(f64, f64, f64)> {
    if !is_literal_colour(hex) {
        return None;
    }
    let digits = hex.strip_prefix('#')?;
    let channel = |index: usize| -> Option<f64> {
        let start = index * 2;
        let value = u8::from_str_radix(digits.get(start..start + 2)?, 16).ok()?;
        Some(f64::from(value) / 255.0)
    };

    Some((channel(0)?, channel(1)?, channel(2)?))
}

/// The session's accent, as the portal wants it.
///
/// Returns `None` only if the sealed accent stopped being a literal colour,
/// which the sealed-colour guard already refuses — so a caller that gets
/// nothing here should answer "no value" rather than invent one.
#[must_use]
pub fn accent() -> Option<(f64, f64, f64)> {
    SEALED
        .iter()
        .find(|colour| colour.token == "accent")
        .and_then(|colour| components(colour.value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_says_it_is_dark_rather_than_having_no_preference() {
        // "No preference" would let an application pick its bright variant
        // beside a shell that has no bright variant at all.
        assert_eq!(colour_scheme(), SCHEME_PREFER_DARK);
        assert_ne!(colour_scheme(), SCHEME_NO_PREFERENCE);
    }

    #[test]
    fn components_are_read_from_the_hex_the_theme_declares() {
        let (red, green, blue) = components("#3e91ff").expect("the accent");
        assert!((red - 62.0 / 255.0).abs() < f64::EPSILON);
        assert!((green - 145.0 / 255.0).abs() < f64::EPSILON);
        assert!((blue - 255.0 / 255.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_ends_of_the_range_land_exactly_on_zero_and_one() {
        assert_eq!(components("#000000"), Some((0.0, 0.0, 0.0)));
        assert_eq!(components("#ffffff"), Some((1.0, 1.0, 1.0)));
    }

    #[test]
    fn alpha_is_dropped_rather_than_folded_into_the_colour() {
        // The same colour at two transparencies is the same accent: an
        // application blends it itself.
        assert_eq!(components("#3e91ff00"), components("#3e91ff"));
    }

    #[test]
    fn anything_that_is_not_a_colour_has_no_components() {
        assert_eq!(components("accent"), None);
        assert_eq!(components("#fff"), None);
        assert_eq!(components(""), None);
    }

    #[test]
    fn the_accent_answered_is_the_sealed_one() {
        assert_eq!(accent(), components("#3e91ff"));
    }
}
