//! The overflow menu of the Island's page (design §5.5, Menus): an
//! `xdg_popup` of the Island's surface, hung below the overflow button, with
//! three rows: reopen the page, do not disturb (with its switch) and quit.
//! A Haze card with the medium radius, drawn with the process's one painter
//! on the popup's own EGL window.
//!
//! The menu does not act. Choosing a row writes the choice into the slot
//! the app handed it and closes the popup; the app reads the slot when the
//! Island's surface gets `PopupClosed` and makes the call itself. Escape, a
//! press on the Island and the compositor's `popup_done` close it with no
//! choice. Up and Down move the focus through the rows (Tab and Shift+Tab
//! too), Enter and Space choose the focused one; the focus ring is drawn
//! only for the keyboard's focus.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use celestina_paint::{silhouette_slices, Painter, Rect, Role, Theme, Weight};
use celestina_ui::{view, Interaction, Key, Tree, UiEvent, View};
use celestina_wayland::{
    AnchorEdge, FrameCx, Gravity, InputEvent, Placement, PopupAction, PopupShell, SurfaceCtl,
};

use super::{
    state::SILHOUETTE_STEP,
    view::{centred, switch, text_style},
};
use crate::copy;

/// The one painter of the process, shared by the app (the Islands) and the
/// open menu: a popup's frame gets no painter from the runtime. Frames run
/// one at a time on the loop, so a borrow never meets another.
pub type SharedPainter = Rc<RefCell<Painter>>;

/// Where a chosen row is left for the app to act on.
pub type ChoiceSlot = Rc<Cell<Option<MenuChoice>>>;

/// The menu's width, logical px: room for the longest row, the
/// do-not-disturb label beside its explanation.
pub const MENU_W: f32 = 280.0;

/// The gap between the overflow button and the menu, logical px.
const GAP_BELOW_BUTTON: i32 = 8;

/// What a row asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    /// Grow the open page again from the capsule.
    ReopenPage,
    /// Set do-not-disturb to this value (the switch's state flipped).
    DoNotDisturb(bool),
    /// End the shell.
    Quit,
}

/// What the menu shows, read by the app the instant it opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    /// The do-not-disturb setting in force.
    pub do_not_disturb: bool,
    /// Another program holds the notifications name: the row is disabled
    /// and says so.
    pub dnd_held: bool,
}

/// The menu card.
#[must_use]
pub fn menu_key() -> Key {
    Key::named("menu")
}

/// The reopen-the-page row.
#[must_use]
pub fn reopen_key() -> Key {
    Key::named("menu-reopen")
}

/// The do-not-disturb row.
#[must_use]
pub fn dnd_key() -> Key {
    Key::named("menu-dnd")
}

/// The quit row.
#[must_use]
pub fn quit_key() -> Key {
    Key::named("menu-quit")
}

/// The do-not-disturb row's switch.
#[must_use]
pub fn switch_key() -> Key {
    Key::named("menu-dnd-switch")
}

/// The popup's logical size with `theme`: three rows and the small
/// spacing all round.
#[must_use]
pub fn menu_size(theme: &Theme) -> (f32, f32) {
    (MENU_W, 3.0 * theme.row_height + 2.0 * theme.spacing.sm)
}

/// The menu hangs from the bottom-left corner of the overflow button `more`
/// (logical px of the Island's surface), growing down and right, a small
/// gap below it; the compositor slides and flips it to stay on the output.
#[must_use]
pub fn placement(more: Rect) -> Placement {
    Placement {
        anchor: more,
        anchor_edge: AnchorEdge::BottomLeft,
        gravity: Gravity::BottomRight,
        offset: (0, GAP_BELOW_BUTTON),
    }
}

/// What choosing the row `key` means in `state`; `None` for anything else
/// and for the disabled do-not-disturb row.
#[must_use]
pub fn choice_for(key: Key, state: MenuState) -> Option<MenuChoice> {
    if key == reopen_key() {
        Some(MenuChoice::ReopenPage)
    } else if key == dnd_key() && !state.dnd_held {
        Some(MenuChoice::DoNotDisturb(!state.do_not_disturb))
    } else if key == quit_key() {
        Some(MenuChoice::Quit)
    } else {
        None
    }
}

/// The menu card for `state`: a Haze card with the medium radius and three
/// rows of the row height, padded by the small spacing.
#[must_use]
pub fn view(theme: &Theme, state: MenuState) -> View {
    let (w, h) = menu_size(theme);
    let dnd = if state.dnd_held {
        row(theme, dnd_key(), copy::DO_NOT_DISTURB, Role::TextFaint).child(view::text(
            Key::named("menu-dnd-held"),
            copy::DO_NOT_DISTURB_HELD,
            text_style(
                theme.type_scale.caption,
                Weight::Regular,
                false,
                Role::TextFaint,
            ),
        ))
    } else {
        control(row(theme, dnd_key(), copy::DO_NOT_DISTURB, Role::TextHi)).child(switch(
            switch_key(),
            state.do_not_disturb,
            true,
        ))
    };
    view::col(menu_key())
        .haze(theme.radius_md)
        .size(w, h)
        .padding(theme.spacing.sm)
        .child(control(row(
            theme,
            reopen_key(),
            copy::MENU_REOPEN_PAGE,
            Role::TextHi,
        )))
        .child(dnd)
        .child(control(row(
            theme,
            quit_key(),
            copy::MENU_QUIT,
            Role::TextHi,
        )))
}

/// A row of the row height with its label; not yet a control.
fn row(theme: &Theme, key: Key, label: &str, role: Role) -> View {
    centred(view::row(key))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(
            view::text(
                Key::named("menu-label"),
                label,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .grow(),
        )
}

/// A row that takes the pointer and the keyboard.
fn control(row: View) -> View {
    row.interactive().focusable()
}

/// What the menu does with an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Stays open; `changed` when its look did (a frame is due).
    Keep { changed: bool },
    /// Closes, with the chosen row's meaning if one was chosen.
    Close(Option<MenuChoice>),
}

/// The menu without its painter: the tree, the pointer and keyboard state
/// and what it shows.
#[derive(Debug)]
pub struct MenuCore {
    state: MenuState,
    theme: Theme,
    tree: Tree,
    input: Interaction,
}

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_DOWN: u32 = 0xff54;

impl MenuCore {
    /// The menu for `state` in `theme`, under `reduced` motion.
    #[must_use]
    pub fn new(theme: &Theme, state: MenuState, reduced: bool, now: Instant) -> MenuCore {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view(theme, state), now);
        MenuCore {
            state,
            theme: theme.clone(),
            tree,
            input: Interaction::new(),
        }
    }

    /// Focuses the first row that takes the keyboard: a menu opened from
    /// the keyboard starts there, so Enter chooses at once.
    pub fn focus_first(&mut self) {
        self.input.key(
            &mut self.tree,
            &InputEvent::Key {
                keysym: XKB_KEY_TAB,
                utf8: None,
                modifiers: celestina_wayland::Modifiers::default(),
                pressed: true,
                serial: 0,
            },
        );
    }

    /// Rebuilds the view when the painter's theme changed.
    fn set_theme(&mut self, theme: &Theme, now: Instant) {
        if *theme != self.theme {
            self.theme = theme.clone();
            self.tree.set_theme(theme);
            self.tree.apply(view(theme, self.state), now);
        }
    }

    /// Handles one event: Up and Down are Shift+Tab and Tab through the
    /// rows; Escape closes; a click or Enter on a row chooses it.
    pub fn input(&mut self, ev: &InputEvent, now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
                utf8,
                modifiers,
                pressed,
                serial,
            } => {
                let tab = InputEvent::Key {
                    keysym: XKB_KEY_TAB,
                    utf8: utf8.clone(),
                    modifiers: celestina_wayland::Modifiers {
                        shift: *keysym == XKB_KEY_UP,
                        ..*modifiers
                    },
                    pressed: *pressed,
                    serial: *serial,
                };
                self.input.key(&mut self.tree, &tab)
            }
            InputEvent::Key { .. } | InputEvent::FocusLost => self.input.key(&mut self.tree, ev),
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        for event in &events {
            match event {
                UiEvent::Escape => return Outcome::Close(None),
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(choice) = choice_for(*key, self.state) {
                        return Outcome::Close(Some(choice));
                    }
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        Outcome::Keep {
            changed: !events.is_empty() || self.tree.highlight() != before,
        }
    }
}

/// The menu as a popup shell.
pub struct Menu {
    core: MenuCore,
    painter: SharedPainter,
    choice: ChoiceSlot,
}

impl std::fmt::Debug for Menu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menu")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl Menu {
    /// The menu for `state` in `theme`, drawn with `painter`; a chosen row
    /// is left in `choice`. Opened from the `keyboard`, its first enabled
    /// row starts focused.
    #[must_use]
    pub fn new(
        painter: SharedPainter,
        theme: &Theme,
        state: MenuState,
        reduced: bool,
        keyboard: bool,
        choice: ChoiceSlot,
    ) -> Menu {
        let mut core = MenuCore::new(theme, state, reduced, Instant::now());
        if keyboard {
            core.focus_first();
        }
        Menu {
            core,
            painter,
            choice,
        }
    }
}

impl PopupShell for Menu {
    fn frame(&mut self, cx: &mut FrameCx<'_>) {
        let Ok(mut painter) = self.painter.try_borrow_mut() else {
            tracing::warn!("menu: the painter is in use; frame skipped");
            cx.ctl.request_redraw();
            return;
        };
        let now = cx.now;
        // Cloned only when the painter's theme changed.
        self.core.set_theme(painter.theme(), now);
        let radius = painter.theme().radius_md;
        let dpr = cx.scale as f32;
        let tree = &mut self.core.tree;
        tree.layout(cx.size, painter.text(), dpr);
        if let Some(fault) = tree.take_fault() {
            tracing::warn!(%fault, "menu: layout fault");
        }
        painter.begin(cx.size, dpr, cx.blur_available);
        tree.paint(&mut painter, now);
        painter.end();
        let silhouette = silhouette_slices(Rect::whole(cx.size), radius, SILHOUETTE_STEP);
        cx.ctl.set_blur_region(&silhouette);
        cx.ctl.set_input_region(&silhouette);
        if tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) -> PopupAction {
        match self.core.input(&ev, Instant::now()) {
            Outcome::Close(choice) => {
                tracing::info!(?choice, "menu: closed from the menu");
                self.choice.set(choice);
                PopupAction::Close
            }
            Outcome::Keep { changed } => {
                if changed {
                    ctl.request_redraw();
                }
                PopupAction::Keep
            }
        }
    }

    fn dismissed(&mut self) {
        tracing::info!("menu: dismissed by the compositor");
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::{Rect, TextStyle, Theme};
    use celestina_ui::{Key, ViewKind};
    use celestina_wayland::{AnchorEdge, Gravity, InputEvent, Modifiers};

    use super::*;
    use crate::copy;

    const DOWN: u32 = 0xff54;
    const UP: u32 = 0xff52;
    const RETURN: u32 = 0xff0d;
    const ESCAPE: u32 = 0xff1b;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn key(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 1,
        }
    }

    fn core(state: MenuState) -> MenuCore {
        let theme = Theme::dark();
        let mut core = MenuCore::new(&theme, state, false, Instant::now());
        core.tree.layout_measured(menu_size(&theme), fake_measure);
        core
    }

    fn focused(core: &MenuCore) -> Option<Key> {
        core.tree.highlight().focused.map(|id| core.tree.key_of(id))
    }

    fn press(core: &mut MenuCore, keysym: u32) -> Outcome {
        core.input(&key(keysym), Instant::now())
    }

    fn rect_of(core: &MenuCore, key: Key) -> Rect {
        match core.tree.node_by_key(key) {
            Some(id) => core.tree.rect(id),
            None => panic!("no node for {key:?}"),
        }
    }

    #[test]
    fn menu_rows_fill_the_menu() {
        let theme = Theme::dark();
        let core = core(MenuState::default());
        let size = menu_size(&theme);
        assert_eq!(
            size,
            (MENU_W, 3.0 * theme.row_height + 2.0 * theme.spacing.sm)
        );
        assert_eq!(theme.row_height, 40.0);
        let menu = rect_of(&core, menu_key());
        assert_eq!((menu.x, menu.y, menu.w, menu.h), (0.0, 0.0, size.0, size.1));
        for (i, key) in [reopen_key(), dnd_key(), quit_key()]
            .into_iter()
            .enumerate()
        {
            let r = rect_of(&core, key);
            assert_eq!(
                (r.x, r.y, r.w, r.h),
                (
                    theme.spacing.sm,
                    theme.spacing.sm + i as f32 * theme.row_height,
                    size.0 - 2.0 * theme.spacing.sm,
                    theme.row_height
                ),
                "row {i}"
            );
        }
        let v = view(&theme, MenuState::default());
        assert!(
            matches!(v.kind, ViewKind::Box { haze: true, radius, .. } if radius == theme.radius_md),
            "a Haze card with the medium radius"
        );
    }

    #[test]
    fn the_menu_hangs_below_the_more_button() {
        let more = Rect {
            x: 480.0,
            y: 30.0,
            w: 56.0,
            h: 28.0,
        };
        let p = placement(more);
        assert_eq!(p.anchor, more);
        assert_eq!(p.anchor_edge, AnchorEdge::BottomLeft);
        assert_eq!(p.gravity, Gravity::BottomRight);
        assert_eq!(p.offset, (0, 8));
    }

    #[test]
    fn arrows_move_the_focus_through_the_rows_and_enter_chooses() {
        let mut core = core(MenuState::default());
        assert_eq!(focused(&core), None);
        assert_eq!(press(&mut core, DOWN), Outcome::Keep { changed: true });
        assert_eq!(focused(&core), Some(reopen_key()));
        press(&mut core, DOWN);
        assert_eq!(focused(&core), Some(dnd_key()));
        press(&mut core, DOWN);
        assert_eq!(focused(&core), Some(quit_key()));
        // Down wraps to the top; Up wraps to the bottom.
        press(&mut core, DOWN);
        assert_eq!(focused(&core), Some(reopen_key()));
        press(&mut core, UP);
        assert_eq!(focused(&core), Some(quit_key()));
        assert_eq!(
            press(&mut core, RETURN),
            Outcome::Close(Some(MenuChoice::Quit))
        );

        let mut core = self::core(MenuState::default());
        press(&mut core, UP);
        press(&mut core, UP);
        assert_eq!(focused(&core), Some(dnd_key()));
        assert_eq!(
            press(&mut core, RETURN),
            Outcome::Close(Some(MenuChoice::DoNotDisturb(true))),
            "the switch is off: choosing it turns it on"
        );
    }

    #[test]
    fn a_menu_opened_from_the_keyboard_focuses_its_first_enabled_row() {
        let mut free = core(MenuState::default());
        free.focus_first();
        assert_eq!(focused(&free), Some(reopen_key()));
        press(&mut free, DOWN);
        assert_eq!(focused(&free), Some(dnd_key()));

        let mut held = core(MenuState {
            do_not_disturb: false,
            dnd_held: true,
        });
        held.focus_first();
        assert_eq!(focused(&held), Some(reopen_key()));
        press(&mut held, DOWN);
        assert_eq!(
            focused(&held),
            Some(quit_key()),
            "the disabled row is skipped"
        );

        // Opened by the pointer: no row is focused until a key moves.
        assert_eq!(focused(&core(MenuState::default())), None);
    }

    #[test]
    fn escape_closes_the_menu_without_a_choice() {
        let mut core = core(MenuState::default());
        assert_eq!(press(&mut core, ESCAPE), Outcome::Close(None));
    }

    #[test]
    fn a_click_on_a_row_chooses_it() {
        let mut core = core(MenuState {
            do_not_disturb: true,
            dnd_held: false,
        });
        let r = rect_of(&core, dnd_key());
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let now = Instant::now();
        let down = InputEvent::PointerPress {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 2,
        };
        let up = InputEvent::PointerRelease {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 3,
        };
        assert!(matches!(core.input(&down, now), Outcome::Keep { .. }));
        assert_eq!(
            core.input(&up, now),
            Outcome::Close(Some(MenuChoice::DoNotDisturb(false))),
            "the switch is on: choosing it turns it off"
        );
    }

    fn text_of(v: &celestina_ui::View) -> Option<&str> {
        match &v.kind {
            ViewKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    #[test]
    fn do_not_disturb_is_disabled_while_another_program_holds_the_notifications() {
        let held = MenuState {
            do_not_disturb: false,
            dnd_held: true,
        };
        let v = view(&Theme::dark(), held);
        let row = &v.children[1];
        assert_eq!(row.key, dnd_key());
        assert!(!row.interactive && !row.focusable, "disabled");
        assert_eq!(text_of(&row.children[0]), Some(copy::DO_NOT_DISTURB));
        assert_eq!(
            text_of(&row.children[1]),
            Some(copy::DO_NOT_DISTURB_HELD),
            "it says why instead of showing the switch"
        );
        assert_eq!(choice_for(dnd_key(), held), None);

        // The keyboard skips it.
        let mut core = core(held);
        press(&mut core, DOWN);
        press(&mut core, DOWN);
        assert_eq!(focused(&core), Some(quit_key()));

        // Free, the row is a control with the switch.
        let v = view(&Theme::dark(), MenuState::default());
        let row = &v.children[1];
        assert!(row.interactive && row.focusable);
        assert_eq!(row.children[1].key, switch_key());
    }

    #[test]
    fn every_row_names_its_choice() {
        let free = MenuState::default();
        let v = view(&Theme::dark(), free);
        let labels: Vec<Option<&str>> = v
            .children
            .iter()
            .map(|row| text_of(&row.children[0]))
            .collect();
        assert_eq!(
            labels,
            [
                Some(copy::MENU_REOPEN_PAGE),
                Some(copy::DO_NOT_DISTURB),
                Some(copy::MENU_QUIT)
            ]
        );
        assert_eq!(choice_for(reopen_key(), free), Some(MenuChoice::ReopenPage));
        assert_eq!(choice_for(quit_key(), free), Some(MenuChoice::Quit));
        assert_eq!(choice_for(Key::named("other"), free), None);
    }
}
