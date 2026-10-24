//! The theme the painter and the UI tree draw with: one colour per semantic
//! role plus the shape, density, type and motion scalars.
//!
//! The theme is built from `celestina_tokens::DARK` and the crate's scalar
//! modules (`radius`, `spacing`, `type_scale`, `motion`, `haze`, `control`,
//! `opacity`), which are generated from `celestina-style/tokens.toml`; no
//! value lives here.
#![forbid(unsafe_code)]

use celestina_tokens::{control, haze, motion, opacity, radius, spacing, type_scale, Rgba, Scheme};

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
    /// The dim under a modal layer (the scheme's `scrim`).
    Scrim,
}

impl Role {
    /// Every role, in declaration order.
    pub const ALL: [Role; 16] = [
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
        Role::Scrim,
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
    /// The dim a modal layer lays over the output.
    pub scrim: Color,
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
    /// A linear track's height (slider, progress).
    pub linear_track_height: f32,
    /// A slider handle's diameter.
    pub slider_handle_size: f32,
    /// The alpha a disabled control is drawn at.
    pub disabled_opacity: f32,
    /// The type ladder at text scale 1, so `scaled` never compounds.
    type_base: TypeScale,
}

/// A token colour as the painter's colour: the same bytes and alpha.
fn color_of(c: Rgba) -> Color {
    Color(c.r, c.g, c.b, c.a)
}

impl From<&Scheme> for Theme {
    /// The theme of `scheme`; the scalars are the crate's, the same for every
    /// scheme.
    fn from(scheme: &Scheme) -> Theme {
        let type_base = TypeScale {
            mini: type_scale::MINI,
            caption: type_scale::CAPTION,
            row_secondary: type_scale::ROW_SECONDARY,
            body: type_scale::BODY,
            row_title: type_scale::ROW_TITLE,
            title: type_scale::TITLE,
            header_collapsed: type_scale::HEADER_COLLAPSED,
            header_expanded: type_scale::HEADER_EXPANDED,
            display: type_scale::DISPLAY,
        };
        Theme {
            canvas: color_of(scheme.canvas),
            card: color_of(scheme.card),
            text_hi: color_of(scheme.text),
            text_lo: color_of(scheme.text_muted),
            text_faint: color_of(scheme.text_faint),
            accent: color_of(scheme.accent),
            accent_ink: color_of(scheme.accent_ink),
            divider: color_of(scheme.divider),
            control_fill: color_of(scheme.control_fill),
            control_hover: color_of(scheme.control_hover),
            control_pressed: color_of(scheme.control_pressed),
            focus_ring: color_of(scheme.focus_ring),
            glass_tint: color_of(scheme.glass_tint),
            // What Haze paints when it cannot blur: the opaque canvas. The
            // QML theme defines `glassFallback: canvas`, a relation and not a
            // role of the scheme, so it follows the canvas here too.
            glass_fallback: color_of(scheme.canvas),
            danger: color_of(scheme.danger),
            scrim: color_of(scheme.scrim),
            grain_alpha: haze::GRAIN,
            radius_lg: radius::LG,
            radius_md: radius::MD,
            radius_sm: radius::SM,
            radius_button: radius::BUTTON,
            row_height: spacing::ROW_HEIGHT,
            top_bar: spacing::TOP_BAR_HEIGHT,
            spacing: Spacing {
                xs: spacing::XS,
                sm: spacing::SM,
                md: spacing::MD,
                lg: spacing::LG,
                xl: spacing::XL,
                xxl: spacing::XXL,
                xxxl: spacing::XXXL,
            },
            type_scale: type_base,
            motion: MotionMs {
                fast: motion::FAST_MS,
                normal: motion::NORMAL_MS,
                slow: motion::SLOW_MS,
                exit: motion::EXIT_MS,
                ceiling: motion::CEILING_MS,
            },
            press_scale: motion::PRESS_SCALE,
            linear_track_height: control::LINEAR_TRACK_HEIGHT,
            slider_handle_size: control::SLIDER_HANDLE_SIZE,
            disabled_opacity: opacity::DISABLED,
            type_base,
        }
    }
}

impl Theme {
    /// The dark scheme, the only one Celestina ships until `celestina-settings`
    /// carries a `scheme` key.
    pub fn dark() -> Theme {
        Theme::from(&celestina_tokens::DARK)
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
            Role::Scrim => self.scrim,
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
        assert_eq!(
            (
                t.linear_track_height,
                t.slider_handle_size,
                t.disabled_opacity
            ),
            (4.0, 15.0, 0.5)
        );
    }

    #[test]
    fn every_role_has_a_colour() {
        let t = Theme::dark();
        for role in Role::ALL {
            let c = t.color(role);
            assert!((0.0..=1.0).contains(&c.3), "{role:?} alpha {}", c.3);
        }
        assert_eq!(Role::ALL.len(), 16);
    }

    #[test]
    fn theme_from_tokens_matches_the_generated_dark_scheme() {
        use celestina_tokens::{motion, DARK};
        let t = Theme::from(&DARK);
        assert_eq!(t.color(Role::Accent), hex("#3e91ff"));
        assert_eq!(t.color(Role::Card), hex("#0b0c10"));
        assert_eq!(t.glass_tint, t.canvas.with_alpha(0.70));
        assert_eq!(t.grain_alpha, 0.15);
        assert_eq!(
            (t.motion.fast, t.motion.normal, t.motion.slow, t.motion.exit),
            (100, 200, 350, 150)
        );
        assert_eq!(t.motion.ceiling, 500);
        assert_eq!(
            (t.motion.fast, t.motion.ceiling),
            (motion::FAST_MS, motion::CEILING_MS)
        );
        // Every role reads the matching field of the scheme, so a swapped
        // assignment fails here and not only in the spot checks above.
        for role in Role::ALL {
            let field = match role {
                Role::Canvas => DARK.canvas,
                Role::Card => DARK.card,
                Role::TextHi => DARK.text,
                Role::TextLo => DARK.text_muted,
                Role::TextFaint => DARK.text_faint,
                Role::Accent => DARK.accent,
                Role::AccentInk => DARK.accent_ink,
                Role::Divider => DARK.divider,
                Role::ControlFill => DARK.control_fill,
                Role::ControlHover => DARK.control_hover,
                Role::ControlPressed => DARK.control_pressed,
                Role::FocusRing => DARK.focus_ring,
                Role::GlassTint => DARK.glass_tint,
                // `CelestinaTheme.glassFallback: canvas`, a relation.
                Role::GlassFallback => DARK.canvas,
                Role::Danger => DARK.danger,
                Role::Scrim => DARK.scrim,
            };
            assert_eq!(
                t.color(role),
                Color(field.r, field.g, field.b, field.a),
                "{role:?}"
            );
        }
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
