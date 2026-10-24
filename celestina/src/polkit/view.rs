//! The prompt's views (design section 5.2): a dimmed whole-output surface and,
//! on one of them, a Haze card centred on it. Pure functions of what the card
//! may show.
//!
//! What the card says comes from exactly two places: polkit's own words (the
//! message, the action id and the account, from
//! [`celestina_shell::polkit::Request::display`], after the display
//! sanitiser; and PAM's lines, decoded and sanitised) and the shell's labels
//! in `copy.rs`. The card composes no description of the action itself (design
//! N-18). The icon is the shell's own glyph, never the `icon_name` polkit
//! sent (N-20).
//!
//! The answer field shows **no per-character glyphs** (design S-4): empty, or,
//! once anything is typed, a row of [`FIELD_DOTS`] dots whose number does not
//! depend on the length of the answer. Colours, radii and spacing are the
//! theme's tokens.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, SecretFieldState, View};
use taffy::{AlignItems, JustifyContent};

use crate::{
    copy,
    island::view::{button_filled, centred, text_style},
};

/// The glyph on the card: the shell's own semantic icon, the closest the
/// suite's icon set has to a lock (`celestina-style/icons` has no
/// `lock.svg` yet).
pub const LOCK_GLYPH: &str = "key";
/// The card's width, logical px.
pub const CARD_W: f32 = 440.0;
/// The glyph's size.
const GLYPH_SIZE: f32 = 28.0;
/// The dots a non-empty answer shows, whatever its length.
pub const FIELD_DOTS: usize = 12;
/// One dot's diameter.
const DOT: f32 = 8.0;

/// Which part of the card the keyboard acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The answer field: characters edit, Enter sends.
    Field,
    /// The authenticate control: Enter or Space sends.
    Authenticate,
    /// The cancel control: Enter or Space gives the request up.
    Cancel,
}

impl Focus {
    /// The next part in Tab order, wrapping.
    #[must_use]
    pub fn next(self) -> Focus {
        match self {
            Focus::Field => Focus::Authenticate,
            Focus::Authenticate => Focus::Cancel,
            Focus::Cancel => Focus::Field,
        }
    }

    /// The previous part in Tab order, wrapping.
    #[must_use]
    pub fn previous(self) -> Focus {
        match self {
            Focus::Field => Focus::Cancel,
            Focus::Authenticate => Focus::Field,
            Focus::Cancel => Focus::Authenticate,
        }
    }
}

/// Where the conversation with the helper stands, as the card shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// PAM has asked nothing yet.
    Preparing,
    /// PAM asked: the card takes the answer.
    Asking,
    /// The answer is with the helper.
    Checking,
}

/// Everything the card shows, borrowed from the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardModel<'a> {
    /// Polkit's message, sanitised.
    pub message: &'a str,
    /// The action id, sanitised.
    pub action_id: &'a str,
    /// The account being asked for.
    pub account: &'a str,
    /// PAM's prompt, once it asked (decoded and sanitised).
    pub pam_prompt: Option<&'a str>,
    /// PAM's last words, an error or an information line.
    pub pam_line: Option<&'a str>,
    /// The line after a wrong answer.
    pub failure: Option<&'a str>,
    /// Whether anything is typed.
    pub field: SecretFieldState,
    /// Where the keyboard acts.
    pub focus: Focus,
    /// Where the conversation stands.
    pub phase: Phase,
}

/// The surface: the root of the view, filled with the scrim.
#[must_use]
pub fn surface_key() -> Key {
    Key::named("polkit-surface")
}

/// The card.
#[must_use]
pub fn card_key() -> Key {
    Key::named("polkit-card")
}

/// The answer field.
#[must_use]
pub fn field_key() -> Key {
    Key::named("polkit-field")
}

/// The authenticate control.
#[must_use]
pub fn authenticate_key() -> Key {
    Key::named("polkit-authenticate")
}

/// The cancel control.
#[must_use]
pub fn cancel_key() -> Key {
    Key::named("polkit-cancel")
}

/// The node that carries `focus`.
#[must_use]
pub fn focus_key(focus: Focus) -> Key {
    match focus {
        Focus::Field => field_key(),
        Focus::Authenticate => authenticate_key(),
        Focus::Cancel => cancel_key(),
    }
}

/// The accessible name and state of the answer field: its state says only
/// whether something is typed, never what or how much (design S-4, S-5).
#[must_use]
pub fn field_accessible(state: SecretFieldState) -> (&'static str, &'static str) {
    let state = match state {
        SecretFieldState::Empty => copy::POLKIT_FIELD_EMPTY,
        SecretFieldState::HasContent => copy::POLKIT_FIELD_HAS_CONTENT,
    };
    (copy::POLKIT_FIELD_NAME, state)
}

fn surface(size: (f32, f32)) -> View {
    let mut root = view::col(surface_key())
        .size(size.0, size.1)
        .fill(Role::Scrim);
    root.style.align_items = AlignItems::CENTER;
    root.style.justify_content = JustifyContent::CENTER;
    root
}

/// A surface that only dims: the other outputs of a prompt, which take no
/// keyboard.
#[must_use]
pub fn dim(size: (f32, f32)) -> View {
    surface(size)
}

/// The card's surface: the dim with the card centred on it.
#[must_use]
pub fn card_surface(theme: &Theme, size: (f32, f32), model: &CardModel<'_>) -> View {
    surface(size).child(card(theme, model))
}

fn label(theme: &Theme, key: &'static str, text: &str) -> View {
    view::text(
        Key::named(key),
        text,
        text_style(
            theme.type_scale.caption,
            Weight::Regular,
            false,
            Role::TextLo,
        ),
    )
}

fn wrapped(theme: &Theme, key: &'static str, text: &str, size: f32, role: Role) -> View {
    view::text(
        Key::named(key),
        text,
        text_style(size, Weight::Regular, false, role),
    )
    .max_width(CARD_W - 2.0 * theme.spacing.lg)
}

fn dots(theme: &Theme) -> View {
    centred(view::row(Key::named("polkit-dots")))
        .gap(theme.spacing.xs)
        .children((0..FIELD_DOTS).map(|i| {
            view::row(Key::indexed("polkit-dot", i as u64))
                .size(DOT, DOT)
                .radius(DOT / 2.0)
                .fill(Role::TextHi)
        }))
}

fn field(theme: &Theme, state: SecretFieldState) -> View {
    let mut field = centred(view::row(field_key()))
        .height(theme.top_bar)
        .radius(theme.top_bar / 2.0)
        .fill(Role::ControlFill)
        .padding_xy(theme.spacing.lg, 0.0)
        .interactive();
    if state == SecretFieldState::HasContent {
        field = field.child(dots(theme));
    }
    field
}

/// The card for `model` in `theme`.
#[must_use]
pub fn card(theme: &Theme, model: &CardModel<'_>) -> View {
    let spacing = theme.spacing;
    let mut column = view::col(card_key())
        .haze(theme.radius_lg)
        .width(CARD_W)
        .padding(spacing.lg)
        .gap(spacing.md);
    column.style.align_items = AlignItems::STRETCH;
    let header = centred(view::col(Key::named("polkit-header")))
        .gap(spacing.sm)
        .child(view::icon(Key::named("polkit-glyph"), LOCK_GLYPH, GLYPH_SIZE).tint(Role::Accent))
        .child(view::text(
            Key::named("polkit-title"),
            copy::POLKIT_TITLE,
            text_style(
                theme.type_scale.title,
                Weight::Semibold,
                false,
                Role::TextHi,
            ),
        ));
    let facts = view::col(Key::named("polkit-facts"))
        .gap(spacing.xs)
        .child(label(
            theme,
            "polkit-action-label",
            copy::POLKIT_ACTION_LABEL,
        ))
        .child(wrapped(
            theme,
            "polkit-action",
            model.action_id,
            theme.type_scale.row_secondary,
            Role::TextHi,
        ))
        .child(label(
            theme,
            "polkit-account-label",
            copy::POLKIT_ACCOUNT_LABEL,
        ))
        .child(wrapped(
            theme,
            "polkit-account",
            model.account,
            theme.type_scale.row_secondary,
            Role::TextHi,
        ));
    let prompt = match model.phase {
        Phase::Preparing => copy::POLKIT_PREPARING,
        Phase::Asking => model.pam_prompt.unwrap_or_default(),
        Phase::Checking => copy::POLKIT_CHECKING,
    };
    let mut column = column
        .child(header)
        .child(wrapped(
            theme,
            "polkit-message",
            model.message,
            theme.type_scale.body,
            Role::TextHi,
        ))
        .child(facts)
        .child(wrapped(
            theme,
            "polkit-prompt",
            prompt,
            theme.type_scale.body,
            Role::TextLo,
        ))
        .child(field(theme, model.field));
    if let Some(line) = model.pam_line {
        column = column.child(wrapped(
            theme,
            "polkit-pam-line",
            line,
            theme.type_scale.row_secondary,
            Role::TextLo,
        ));
    }
    if let Some(line) = model.failure {
        column = column.child(wrapped(
            theme,
            "polkit-failure",
            line,
            theme.type_scale.row_secondary,
            Role::Danger,
        ));
    }
    let mut buttons = centred(view::row(Key::named("polkit-buttons"))).gap(spacing.sm);
    buttons.style.justify_content = JustifyContent::FLEX_END;
    let ready = model.phase == Phase::Asking && model.field == SecretFieldState::HasContent;
    let (fill, ink) = if ready {
        (Role::Accent, Role::AccentInk)
    } else {
        (Role::ControlFill, Role::TextFaint)
    };
    column.child(
        buttons
            .child(button_filled(
                theme,
                cancel_key(),
                "polkit-cancel-label",
                copy::POLKIT_CANCEL,
                Role::ControlFill,
                Role::TextHi,
            ))
            .child(button_filled(
                theme,
                authenticate_key(),
                "polkit-authenticate-label",
                copy::POLKIT_AUTHENTICATE,
                fill,
                ink,
            )),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::TextStyle;
    use celestina_ui::{Tree, ViewKind};

    use super::*;

    fn model<'a>(field: SecretFieldState) -> CardModel<'a> {
        CardModel {
            message: "Run a program",
            action_id: "org.example.Run_Thing",
            account: "toni",
            pam_prompt: Some("Password:"),
            pam_line: None,
            failure: None,
            field,
            focus: Focus::Field,
            phase: Phase::Asking,
        }
    }

    fn texts(view: &View) -> Vec<String> {
        let mut out = Vec::new();
        collect(view, &mut out);
        out
    }

    fn collect(view: &View, out: &mut Vec<String>) {
        if let ViewKind::Text { text, .. } = &view.kind {
            out.push(text.clone());
        }
        for child in &view.children {
            collect(child, out);
        }
    }

    fn fake_measure(s: &str, _: &TextStyle, max: Option<f32>) -> (f32, f32) {
        let width = 7.0 * s.chars().count() as f32;
        match max {
            Some(max) if width > max => (max, 16.0 * (width / max).ceil()),
            _ => (width, 16.0),
        }
    }

    #[test]
    fn the_card_text_is_polkits_and_the_shells_labels_and_nothing_else() {
        let theme = Theme::dark();
        let view = card_surface(&theme, (1280.0, 800.0), &model(SecretFieldState::Empty));
        let mut expected = vec![
            copy::POLKIT_TITLE,
            "Run a program",
            copy::POLKIT_ACTION_LABEL,
            "org.example.Run_Thing",
            copy::POLKIT_ACCOUNT_LABEL,
            "toni",
            "Password:",
            copy::POLKIT_CANCEL,
            copy::POLKIT_AUTHENTICATE,
        ];
        let mut found = texts(&view);
        expected.sort_unstable();
        found.sort();
        assert_eq!(found, expected);
    }

    #[test]
    fn the_card_shows_no_per_character_glyphs() {
        let theme = Theme::dark();
        let empty = card_surface(&theme, (1280.0, 800.0), &model(SecretFieldState::Empty));
        let typed = card_surface(
            &theme,
            (1280.0, 800.0),
            &model(SecretFieldState::HasContent),
        );
        let dots_of = |view: &View| {
            let mut count = 0;
            count_dots(view, &mut count);
            count
        };
        assert_eq!(dots_of(&empty), 0, "nothing typed: no dots");
        assert_eq!(dots_of(&typed), FIELD_DOTS, "something typed: a fixed row");
        // The view is a function of the field's two states, so one character
        // and five hundred show the same row and the same text.
        assert_eq!(texts(&empty).len(), texts(&typed).len());
        assert_eq!(
            card(&theme, &model(SecretFieldState::HasContent)),
            card(&theme, &model(SecretFieldState::HasContent))
        );
    }

    fn count_dots(view: &View, count: &mut usize) {
        if view.key == Key::indexed("polkit-dot", 0)
            || (0..FIELD_DOTS as u64).any(|i| view.key == Key::indexed("polkit-dot", i))
        {
            *count += 1;
        }
        for child in &view.children {
            count_dots(child, count);
        }
    }

    #[test]
    fn the_answer_field_names_only_whether_it_has_content() {
        assert_eq!(
            field_accessible(SecretFieldState::Empty),
            (copy::POLKIT_FIELD_NAME, copy::POLKIT_FIELD_EMPTY)
        );
        assert_eq!(
            field_accessible(SecretFieldState::HasContent),
            (copy::POLKIT_FIELD_NAME, copy::POLKIT_FIELD_HAS_CONTENT)
        );
    }

    #[test]
    fn the_icon_is_the_shells_own_glyph() {
        let theme = Theme::dark();
        let view = card(&theme, &model(SecretFieldState::Empty));
        fn glyph(view: &View) -> Option<&View> {
            if let ViewKind::Icon { .. } = view.kind {
                return Some(view);
            }
            view.children.iter().find_map(glyph)
        }
        let found = glyph(&view);
        assert!(found.is_some_and(|v| matches!(
            &v.kind,
            ViewKind::Icon { key: celestina_paint::IconKey::Semantic(name), .. } if *name == LOCK_GLYPH
        )));
    }

    #[test]
    fn the_card_is_centred_inside_the_dim_and_the_dim_alone_has_no_card() {
        let theme = Theme::dark();
        let size = (1280.0, 800.0);
        let mut tree = Tree::new(false, &theme);
        tree.apply(
            card_surface(&theme, size, &model(SecretFieldState::Empty)),
            Instant::now(),
        );
        tree.layout_measured(size, fake_measure);
        let root = tree.node_by_key(surface_key()).map(|id| tree.rect(id));
        assert_eq!(root.map(|r| (r.w, r.h)), Some(size));
        let card = tree.node_by_key(card_key()).map(|id| tree.rect(id));
        let Some(card) = card else {
            panic!("the card is laid out");
        };
        assert_eq!(card.w, CARD_W);
        assert!(
            ((card.x + card.w / 2.0) - size.0 / 2.0).abs() <= 1.0,
            "centred across"
        );
        assert!(
            ((card.y + card.h / 2.0) - size.1 / 2.0).abs() <= 1.0,
            "centred down"
        );

        let mut tree = Tree::new(false, &theme);
        tree.apply(dim(size), Instant::now());
        tree.layout_measured(size, fake_measure);
        assert!(tree.node_by_key(card_key()).is_none());
        assert!(tree.node_by_key(surface_key()).is_some());
    }

    #[test]
    fn tab_order_wraps_both_ways() {
        let mut focus = Focus::Field;
        for expected in [Focus::Authenticate, Focus::Cancel, Focus::Field] {
            focus = focus.next();
            assert_eq!(focus, expected);
        }
        for expected in [Focus::Cancel, Focus::Authenticate, Focus::Field] {
            focus = focus.previous();
            assert_eq!(focus, expected);
        }
    }

    #[test]
    fn the_failure_and_pam_lines_appear_only_when_there_are_some() {
        let theme = Theme::dark();
        let mut m = model(SecretFieldState::Empty);
        assert!(!texts(&card(&theme, &m)).iter().any(|t| t == "Try again"));
        m.failure = Some("Try again");
        m.pam_line = Some("Account locked soon");
        let shown = texts(&card(&theme, &m));
        assert!(shown.iter().any(|t| t == "Try again"));
        assert!(shown.iter().any(|t| t == "Account locked soon"));
    }

    #[test]
    fn the_prompt_line_follows_the_phase() {
        let theme = Theme::dark();
        let mut m = model(SecretFieldState::Empty);
        m.phase = Phase::Preparing;
        assert!(texts(&card(&theme, &m)).contains(&copy::POLKIT_PREPARING.to_owned()));
        m.phase = Phase::Checking;
        let shown = texts(&card(&theme, &m));
        assert!(shown.contains(&copy::POLKIT_CHECKING.to_owned()));
        assert!(!shown.contains(&"Password:".to_owned()));
    }
}
