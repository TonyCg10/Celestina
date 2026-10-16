//! The theme the painter and the UI tree draw with: one colour per semantic
//! role plus the shape, density, type and motion scalars.
//!
//! CEL-1-B carries the dark scheme as local constants: the spike's
//! `tokens.rs` extended with every role, the values taken from
//! `celestina-style/DESIGN.md` §2/§4/§5 and the scheme of
//! `celestina-style/CelestinaTheme.qml` where DESIGN.md names no number.
//! CEL-1-C replaces `Theme::dark` by `From<&celestina_tokens::Scheme>`.
#![forbid(unsafe_code)]

use crate::color::Color;

/// A semantic colour role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Canvas,
    Card,
    TextHi,
    TextLo,
    TextFaint,
    Accent,
    AccentInk,
    Divider,
    ControlFill,
    ControlHover,
    ControlPressed,
    FocusRing,
    GlassTint,
    GlassFallback,
    Danger,
}

impl Role {
    /// Every role, in declaration order.
    pub const ALL: [Role; 15] = [
        Role::Canvas,
        Role::Card,
        Role::TextHi,
        Role::TextLo,
        Role::TextFaint,
        Role::Accent,
        Role::AccentInk,
        Role::Divider,
        Role::ControlFill,
        Role::ControlHover,
        Role::ControlPressed,
        Role::FocusRing,
        Role::GlassTint,
        Role::GlassFallback,
        Role::Danger,
    ];
}

/// The spacing ladder in logical px (DESIGN.md §5.1, `CelestinaTheme.space*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
    pub xxxl: f32,
}

/// The nine type roles in logical px (DESIGN.md §5.4); only these scale with
/// the host's text scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    pub mini: f32,
    pub caption: f32,
    pub row_secondary: f32,
    pub body: f32,
    pub row_title: f32,
    pub title: f32,
    pub header_collapsed: f32,
    pub header_expanded: f32,
    pub display: f32,
}

impl TypeScale {
    /// Every role at `round(base × factor)`, as `CelestinaTheme.font*` does.
    fn scaled(&self, factor: f64) -> TypeScale {
        let s = |base: f32| (f64::from(base) * factor).round() as f32;
        TypeScale {
            mini: s(self.mini),
            caption: s(self.caption),
            row_secondary: s(self.row_secondary),
            body: s(self.body),
            row_title: s(self.row_title),
            title: s(self.title),
            header_collapsed: s(self.header_collapsed),
            header_expanded: s(self.header_expanded),
            display: s(self.display),
        }
    }
}

/// The duration ladder in milliseconds (DESIGN.md §5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionMs {
    pub fast: u32,
    pub normal: u32,
    pub slow: u32,
    pub exit: u32,
    /// The hard ceiling no transition exceeds.
    pub ceiling: u32,
}

/// Colours per role and the shape, density, type and motion scalars.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub canvas: Color,
    pub card: Color,
    pub text_hi: Color,
    pub text_lo: Color,
    pub text_faint: Color,
    pub accent: Color,
    pub accent_ink: Color,
    pub divider: Color,
    pub control_fill: Color,
    pub control_hover: Color,
    pub control_pressed: Color,
    pub focus_ring: Color,
    pub glass_tint: Color,
    pub glass_fallback: Color,
    pub danger: Color,
    /// Haze's grain alpha, drawn under the tint.
    pub grain_alpha: f32,
    pub radius_lg: f32,
    pub radius_md: f32,
    pub radius_sm: f32,
    pub radius_button: f32,
    pub row_height: f32,
    pub top_bar: f32,
    pub spacing: Spacing,
    /// The type ladder at the current text scale.
    pub type_scale: TypeScale,
    pub motion: MotionMs,
    /// The press recoil scale.
    pub press_scale: f32,
    /// The type ladder at text scale 1, so `scaled` never compounds.
    type_base: TypeScale,
}

/// One row of the dark scheme: a role, its `#rrggbb` colour and its alpha in
/// `0.0..=1.0`. The white washes are the QML scheme's `#AAffffff` bytes written
/// as `byte / 255`.
const DARK: &[(Role, &str, f32)] = &[
    // DESIGN.md §5.2 L1: the black a Haze-tinted bar shows over the canvas.
    (Role::Card, "#0b0c10", 1.0),
    (Role::Canvas, "#050608", 1.0),
    (Role::TextHi, "#f7f8fc", 1.0),
    (Role::TextLo, "#9ba3af", 1.0),
    (Role::TextFaint, "#78818e", 1.0),
    (Role::Accent, "#3e91ff", 1.0),
    (Role::AccentInk, "#050608", 1.0),
    (Role::Divider, "#ffffff", 22.0 / 255.0),
    (Role::ControlFill, "#ffffff", 26.0 / 255.0),
    (Role::ControlHover, "#ffffff", 41.0 / 255.0),
    // The QML scheme has no pressed plate; one more hover step.
    (Role::ControlPressed, "#ffffff", 56.0 / 255.0),
    // The accent mixed 0.24 towards `accentLift` (#fcfcff).
    (Role::FocusRing, "#6cabff", 1.0),
    // Haze's tint: the canvas at 0.70 (`#b3050608`, spec §5.3).
    (Role::GlassTint, "#050608", 0.70),
    // What Haze paints when it cannot blur: the opaque canvas
    // (`CelestinaTheme.glassFallback: canvas`).
    (Role::GlassFallback, "#050608", 1.0),
    (Role::Danger, "#ff746d", 1.0),
];

/// `#rrggbb` at `alpha` as a colour. It runs at compile time, so a malformed
/// `DARK` row fails the build.
const fn parse_row(hex: &str, alpha: f32) -> Color {
    let b = hex.as_bytes();
    assert!(b.len() == 7 && b[0] == b'#', "a colour is #rrggbb");
    Color(
        hex_pair(b[1], b[2]),
        hex_pair(b[3], b[4]),
        hex_pair(b[5], b[6]),
        alpha,
    )
}

const fn hex_pair(high: u8, low: u8) -> u8 {
    hex_digit(high) * 16 + hex_digit(low)
}

const fn hex_digit(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        b'A'..=b'F' => digit - b'A' + 10,
        _ => panic!("a colour digit is 0-9, a-f or A-F"),
    }
}

/// `DARK` laid out by role and indexed by `Role as usize`. A role with no row,
/// or with two, fails the build.
const fn dark_palette() -> [Color; 15] {
    let mut rows: [Option<Color>; 15] = [None; 15];
    let mut i = 0;
    while i < DARK.len() {
        let (role, hex, alpha) = DARK[i];
        let at = role as usize;
        assert!(rows[at].is_none(), "a role has two rows in DARK");
        rows[at] = Some(parse_row(hex, alpha));
        i += 1;
    }
    let mut out = [Color(0, 0, 0, 0.0); 15];
    let mut at = 0;
    while at < 15 {
        out[at] = match rows[at] {
            Some(colour) => colour,
            None => panic!("a role has no row in DARK"),
        };
        at += 1;
    }
    out
}

/// The dark scheme's numeric scalars.
struct Scalars {
    /// Haze's grain alpha, drawn under the tint.
    grain_alpha: f32,
    radius_lg: f32,
    radius_md: f32,
    radius_sm: f32,
    radius_button: f32,
    row_height: f32,
    top_bar: f32,
    /// The spacing ladder in logical px (DESIGN.md §5.1, `CelestinaTheme.space*`).
    spacing: Spacing,
    /// The type ladder at text scale 1 (DESIGN.md §5.4).
    type_base: TypeScale,
    /// The duration ladder in milliseconds (DESIGN.md §5.5).
    motion: MotionMs,
    /// The press recoil scale.
    press_scale: f32,
}

const SCALARS: Scalars = Scalars {
    grain_alpha: 0.15,
    radius_lg: 20.0,
    radius_md: 12.0,
    radius_sm: 8.0,
    radius_button: 10.0,
    row_height: 40.0,
    top_bar: 48.0,
    spacing: Spacing {
        xs: 4.0,
        sm: 8.0,
        md: 12.0,
        lg: 16.0,
        xl: 20.0,
        xxl: 24.0,
        xxxl: 32.0,
    },
    type_base: TypeScale {
        mini: 10.0,
        caption: 11.0,
        row_secondary: 12.0,
        body: 13.0,
        row_title: 15.0,
        title: 17.0,
        header_collapsed: 20.0,
        header_expanded: 30.0,
        display: 34.0,
    },
    motion: MotionMs {
        fast: 100,
        normal: 200,
        slow: 350,
        exit: 150,
        ceiling: 500,
    },
    press_scale: 0.96,
};

impl Theme {
    /// The dark scheme, the only one Celestina ships.
    pub fn dark() -> Theme {
        const PALETTE: [Color; 15] = dark_palette();
        let colour = |role: Role| PALETTE[role as usize];
        Theme {
            canvas: colour(Role::Canvas),
            card: colour(Role::Card),
            text_hi: colour(Role::TextHi),
            text_lo: colour(Role::TextLo),
            text_faint: colour(Role::TextFaint),
            accent: colour(Role::Accent),
            accent_ink: colour(Role::AccentInk),
            divider: colour(Role::Divider),
            control_fill: colour(Role::ControlFill),
            control_hover: colour(Role::ControlHover),
            control_pressed: colour(Role::ControlPressed),
            focus_ring: colour(Role::FocusRing),
            glass_tint: colour(Role::GlassTint),
            glass_fallback: colour(Role::GlassFallback),
            danger: colour(Role::Danger),
            grain_alpha: SCALARS.grain_alpha,
            radius_lg: SCALARS.radius_lg,
            radius_md: SCALARS.radius_md,
            radius_sm: SCALARS.radius_sm,
            radius_button: SCALARS.radius_button,
            row_height: SCALARS.row_height,
            top_bar: SCALARS.top_bar,
            spacing: SCALARS.spacing,
            type_scale: SCALARS.type_base,
            motion: SCALARS.motion,
            press_scale: SCALARS.press_scale,
            type_base: SCALARS.type_base,
        }
    }

    /// The colour of `role`.
    pub fn color(&self, role: Role) -> Color {
        match role {
            Role::Canvas => self.canvas,
            Role::Card => self.card,
            Role::TextHi => self.text_hi,
            Role::TextLo => self.text_lo,
            Role::TextFaint => self.text_faint,
            Role::Accent => self.accent,
            Role::AccentInk => self.accent_ink,
            Role::Divider => self.divider,
            Role::ControlFill => self.control_fill,
            Role::ControlHover => self.control_hover,
            Role::ControlPressed => self.control_pressed,
            Role::FocusRing => self.focus_ring,
            Role::GlassTint => self.glass_tint,
            Role::GlassFallback => self.glass_fallback,
            Role::Danger => self.danger,
        }
    }

    /// This theme at host text scale `text_scale` (DESIGN.md §5.4): every
    /// type role is `round(base × text_scale)` and nothing else changes. A
    /// factor that is not a finite positive number keeps the base ladder.
    pub fn scaled(&self, text_scale: f64) -> Theme {
        let factor = if text_scale.is_finite() && text_scale > 0.0 {
            text_scale
        } else {
            1.0
        };
        Theme {
            type_scale: self.type_base.scaled(factor),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        match Color::hex(s) {
            Ok(c) => c,
            Err(err) => panic!("{s}: {err}"),
        }
    }

    #[test]
    fn dark_matches_design_md() {
        let t = Theme::dark();
        assert_eq!(t.card, hex("#0b0c10"));
        assert_eq!(t.color(Role::Card), hex("#0b0c10"));
        assert_eq!(t.glass_tint, t.canvas.with_alpha(0.70));
        assert_eq!(t.color(Role::GlassTint), hex("#050608").with_alpha(0.70));
        assert_eq!(
            t.color(Role::GlassFallback),
            hex("#050608"),
            "opaque canvas"
        );
        assert_eq!(t.grain_alpha, 0.15);
        assert_eq!(t.color(Role::Accent), hex("#3e91ff"));
        assert_eq!(t.color(Role::Canvas), hex("#050608"));
        assert_eq!(t.color(Role::AccentInk), hex("#050608"));
        assert_eq!(t.color(Role::TextHi), hex("#f7f8fc"));
        assert_eq!(t.color(Role::TextLo), hex("#9ba3af"));
        assert_eq!(
            (t.radius_lg, t.radius_md, t.radius_sm, t.radius_button),
            (20.0, 12.0, 8.0, 10.0)
        );
        assert_eq!((t.row_height, t.top_bar), (40.0, 48.0));
        assert_eq!(
            (t.motion.fast, t.motion.normal, t.motion.slow, t.motion.exit),
            (100, 200, 350, 150)
        );
        assert_eq!(t.motion.ceiling, 500);
        assert_eq!(t.press_scale, 0.96);
    }

    #[test]
    fn every_role_has_a_colour() {
        let t = Theme::dark();
        for role in Role::ALL {
            let c = t.color(role);
            assert!((0.0..=1.0).contains(&c.3), "{role:?} alpha {}", c.3);
        }
        assert_eq!(Role::ALL.len(), 15);
    }

    #[test]
    fn dark_table_rows_parse() {
        for &(role, text, alpha) in DARK {
            let runtime = hex(text).with_alpha(alpha);
            assert_eq!(parse_row(text, alpha), runtime, "{role:?}");
            assert!((0.0..=1.0).contains(&alpha), "{role:?} alpha {alpha}");
        }
    }

    #[test]
    fn dark_table_covers_every_role() {
        for role in Role::ALL {
            let rows = DARK.iter().filter(|(r, _, _)| *r == role).count();
            assert_eq!(rows, 1, "{role:?} has {rows} rows in DARK");
        }
        assert_eq!(DARK.len(), Role::ALL.len());
    }

    #[test]
    fn scaled_multiplies_only_the_type_ladder() {
        let base = Theme::dark();
        let large = base.scaled(1.3);
        let b = base.type_scale;
        let l = large.type_scale;
        let pairs = [
            (b.mini, l.mini),
            (b.caption, l.caption),
            (b.row_secondary, l.row_secondary),
            (b.body, l.body),
            (b.row_title, l.row_title),
            (b.title, l.title),
            (b.header_collapsed, l.header_collapsed),
            (b.header_expanded, l.header_expanded),
            (b.display, l.display),
        ];
        for (base_size, scaled) in pairs {
            assert_eq!(scaled, (base_size * 1.3).round(), "{base_size}");
        }
        assert_eq!(l.body, 17.0, "13 × 1.3 = 16.9 rounds to 17");
        let mut rest = large.clone();
        rest.type_scale = base.type_scale;
        assert_eq!(rest, base, "nothing but the type ladder changed");
    }

    #[test]
    fn scaled_never_compounds() {
        let base = Theme::dark();
        assert_eq!(base.scaled(1.3).scaled(1.3), base.scaled(1.3));
        assert_eq!(base.scaled(1.3).scaled(1.0), base);
    }

    #[test]
    fn scaled_ignores_a_nonsense_factor() {
        let base = Theme::dark();
        for f in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(base.scaled(f), base, "{f}");
        }
    }
}
