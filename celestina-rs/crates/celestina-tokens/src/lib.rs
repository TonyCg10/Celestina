//! The Celestina style tokens as plain Rust constants: schemes, radius,
//! motion, Haze and the type scale (design §5.7,
//! `docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`).
//! `generated.rs` is written by `celestina-style/scripts/gen-tokens.py` from
//! `celestina-style/tokens.toml`; edit the data file and run the generator.
#![forbid(unsafe_code)]

pub mod generated;
pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_finds_the_dark_scheme_only() {
        assert_eq!(Scheme::by_name("dark"), Some(&DARK));
        assert_eq!(Scheme::by_name("light"), None);
        assert_eq!(Scheme::by_name(""), None);
    }

    #[test]
    fn dark_keeps_the_shell_theme_bytes() {
        let opaque = |r, g, b| Rgba { r, g, b, a: 1.0 };
        assert_eq!(DARK.canvas, opaque(5, 6, 8));
        assert_eq!(DARK.card, opaque(11, 12, 16));
        assert_eq!(DARK.accent, opaque(62, 145, 255));
        assert_eq!(DARK.focus_ring, opaque(108, 171, 255));
        let white = |a: f32| Rgba {
            r: 255,
            g: 255,
            b: 255,
            a,
        };
        assert_eq!(DARK.divider, white(22.0 / 255.0));
        assert_eq!(DARK.control_fill, white(26.0 / 255.0));
        assert_eq!(DARK.control_hover, white(41.0 / 255.0));
        assert_eq!(DARK.control_pressed, white(56.0 / 255.0));
        assert_eq!(
            DARK.glass_tint,
            Rgba {
                a: 0.70,
                ..DARK.canvas
            }
        );
        assert_eq!(haze::GRAIN, 0.15);
        assert_eq!(motion::PRESS_SCALE, 0.96);
    }

    #[test]
    fn scalars_keep_their_units() {
        assert_eq!((radius::LG, radius::MD, radius::SM), (20.0, 12.0, 8.0));
        assert_eq!((motion::FAST_MS, motion::CEILING_MS), (100, 500));
        assert_eq!(type_scale::BODY, 13.0);
        assert_eq!(type_scale::scales::LARGER, 1.3);
        assert_eq!(type_scale::WEIGHT_TITLE, 600);
        assert_eq!(spacing::XXXL, 32.0);
        assert_eq!(motion::CURVE_STANDARD, [0.22, 0.25, 0.0, 1.0]);
    }
}
