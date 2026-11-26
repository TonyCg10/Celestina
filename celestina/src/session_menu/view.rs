//! The session menu's views: the list of rows and the list with one
//! dangerous row replaced by its confirmation card, and the sizes both take.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::{JustifyContent, LengthPercentageAuto};

use super::{Mode, SessionAction, MENU_W};
use crate::{
    copy,
    island::view::{button_filled, centred, text_style, BUTTON_H},
};

/// A row's icon, logical px.
const ICON: f32 = 18.0;
/// The divider's thickness, logical px.
const DIVIDER_H: f32 = 1.0;

/// The menu card.
#[must_use]
pub fn menu_key() -> Key {
    Key::named("session-menu")
}

/// The divider after Suspender (with its margins).
#[must_use]
pub fn divider_key() -> Key {
    Key::named("session-divider")
}

/// The confirmation card.
#[must_use]
pub fn confirm_key() -> Key {
    Key::named("session-confirm")
}

/// The confirmation card's question.
#[must_use]
pub fn ask_key() -> Key {
    Key::named("session-ask")
}

/// The confirmation card's Cancelar button.
#[must_use]
pub fn cancel_key() -> Key {
    Key::named("session-cancel")
}

/// The confirmation card's action button.
#[must_use]
pub fn do_key() -> Key {
    Key::named("session-do")
}

/// The width the question wraps at.
#[must_use]
pub fn question_width(theme: &Theme) -> f32 {
    MENU_W - 2.0 * theme.spacing.sm - 2.0 * theme.spacing.md
}

/// The list's size: five rows, the divider (with its margins) and the
/// body's padding (`xs` on top, `sm` below and at the sides).
#[must_use]
pub fn list_size(theme: &Theme) -> (f32, f32) {
    (
        MENU_W,
        5.0 * theme.row_height
            + DIVIDER_H
            + 2.0 * theme.spacing.xs
            + theme.spacing.xs
            + theme.spacing.sm,
    )
}

/// The confirmation card's height for a question `question_h` tall.
#[must_use]
pub fn card_height(theme: &Theme, question_h: f32) -> f32 {
    2.0 * theme.spacing.md + question_h.max(question_min_h(theme)) + theme.spacing.sm + BUTTON_H
}

/// The least height the question takes: enough that the card's buttons start
/// below the row the card replaced, so a second click of a double-click on
/// that row lands on the question, never on the action.
#[must_use]
pub fn question_min_h(theme: &Theme) -> f32 {
    theme.row_height - theme.spacing.md - theme.spacing.sm
}

/// The menu's size with a confirmation card for a question `question_h`
/// tall: the list with one row given over to the card.
#[must_use]
pub fn confirm_size(theme: &Theme, question_h: f32) -> (f32, f32) {
    let (w, h) = list_size(theme);
    (w, h - theme.row_height + card_height(theme, question_h))
}

/// The menu in `mode`; `question_h` is the wrapped question's height.
#[must_use]
pub fn view(theme: &Theme, mode: Mode, question_h: f32) -> View {
    let (w, h) = match mode {
        Mode::List => list_size(theme),
        Mode::Confirm(_) => confirm_size(theme, question_h),
    };
    let mut card = view::col(menu_key()).size(w, h).padding_sides(
        theme.spacing.xs,
        theme.spacing.sm,
        theme.spacing.sm,
    );
    for action in SessionAction::ROWS {
        if mode == Mode::Confirm(action) {
            card = card.child(confirm(theme, action));
        } else {
            card = card.child(row(theme, action));
        }
        if action == SessionAction::Suspend {
            card = card.child(divider(theme));
        }
    }
    card
}

fn row(theme: &Theme, action: SessionAction) -> View {
    centred(view::row(action.key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_button)
        .interactive()
        .focusable()
        .child(view::icon(Key::named("session-icon"), action.icon(), ICON).tint(Role::TextHi))
        .child(
            view::text(
                Key::named("session-label"),
                action.label(),
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
            )
            .grow(),
        )
}

fn divider(theme: &Theme) -> View {
    view::col(divider_key())
        .padding_xy(0.0, theme.spacing.xs)
        .child(
            view::row(Key::named("session-divider-line"))
                .height(DIVIDER_H)
                .fill(Role::Divider),
        )
}

fn confirm(theme: &Theme, action: SessionAction) -> View {
    let ask = action.ask().unwrap_or_default();
    let mut actions = view::row(Key::named("session-actions")).gap(theme.spacing.xs);
    actions.style.justify_content = JustifyContent::FLEX_END;
    let actions = actions
        .child(button_filled(
            theme,
            cancel_key(),
            "session-cancel-label",
            copy::SESSION_CANCEL,
            Role::ControlFill,
            Role::TextHi,
        ))
        .child(button_filled(
            theme,
            do_key(),
            "session-do-label",
            action.label(),
            Role::Danger,
            Role::DangerInk,
        ));
    let mut question = view::text(
        ask_key(),
        ask,
        text_style(
            theme.type_scale.body,
            Weight::Regular,
            false,
            Role::DangerFillInk,
        ),
    )
    .max_width(question_width(theme));
    question.style.min_size.height = LengthPercentageAuto::length(question_min_h(theme));
    view::col(confirm_key())
        .fill(Role::DangerFill)
        .stroke(Role::DangerBorder)
        .radius(theme.radius_button)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm)
        .child(question)
        .child(actions)
}
