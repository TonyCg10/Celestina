//! The session menu (CEL-I-006), a body of the Session capsule's sheet
//! (CEL-D-003). The lock and suspend rows choose at once; the log-out, reboot
//! and power-off rows swap their row for a danger card that asks first, and
//! the body grows (the sheet grows down with it).
//!
//! The body does not act: a choice comes out as [`Outcome::Close`] and the
//! Island posts it as one action; no popup grab is held, so the lock maps
//! over the closing sheet. Escape on the card goes back to the list, Escape
//! on the list closes the sheet. Nothing here stores a measure: the wrapped
//! question is measured with the caller's, in hand for the call.

mod view;

use std::time::Instant;

use celestina_paint::Theme;
use celestina_shell::session::PowerAction;
use celestina_ui::{Interaction, Key, Tree, UiEvent};
use celestina_wayland::InputEvent;

pub use view::{
    ask_key, cancel_key, confirm_key, confirm_size, divider_key, do_key, list_size, menu_key,
    question_width, view,
};

use crate::{command::Verb, copy, island::menu::menu_events};

/// The menu's width, logical px.
pub const MENU_W: f32 = 240.0;

/// What a row asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Lock,
    Suspend,
    LogOut,
    Reboot,
    PowerOff,
}

impl SessionAction {
    /// The rows, top to bottom.
    pub const ROWS: [SessionAction; 5] = [
        SessionAction::Lock,
        SessionAction::Suspend,
        SessionAction::LogOut,
        SessionAction::Reboot,
        SessionAction::PowerOff,
    ];

    /// Whether the row asks before it acts.
    #[must_use]
    pub fn needs_confirm(self) -> bool {
        matches!(
            self,
            SessionAction::LogOut | SessionAction::Reboot | SessionAction::PowerOff
        )
    }

    /// The command the app makes. Suspend is lock-and-suspend: the plain
    /// suspend verb is refused by design.
    #[must_use]
    pub fn verb(self) -> Verb {
        match self {
            SessionAction::Lock => Verb::Lock,
            SessionAction::Suspend => Verb::LockAndSuspend,
            SessionAction::LogOut => Verb::Session(PowerAction::LogOut),
            SessionAction::Reboot => Verb::Session(PowerAction::Reboot),
            SessionAction::PowerOff => Verb::Session(PowerAction::PowerOff),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SessionAction::Lock => "lock",
            SessionAction::Suspend => "moon",
            SessionAction::LogOut => "log-out",
            SessionAction::Reboot => "rotate-ccw",
            SessionAction::PowerOff => "power",
        }
    }

    fn label(self) -> &'static str {
        match self {
            SessionAction::Lock => copy::SESSION_LOCK,
            SessionAction::Suspend => copy::SESSION_SUSPEND,
            SessionAction::LogOut => copy::SESSION_LOG_OUT,
            SessionAction::Reboot => copy::SESSION_REBOOT,
            SessionAction::PowerOff => copy::SESSION_POWER_OFF,
        }
    }

    /// The question a dangerous row asks.
    fn ask(self) -> Option<&'static str> {
        match self {
            SessionAction::LogOut => Some(copy::SESSION_ASK_LOG_OUT),
            SessionAction::Reboot => Some(copy::SESSION_ASK_REBOOT),
            SessionAction::PowerOff => Some(copy::SESSION_ASK_POWER_OFF),
            SessionAction::Lock | SessionAction::Suspend => None,
        }
    }

    /// The row's node key.
    #[must_use]
    pub fn key(self) -> Key {
        Key::named(match self {
            SessionAction::Lock => "session-lock",
            SessionAction::Suspend => "session-suspend",
            SessionAction::LogOut => "session-log-out",
            SessionAction::Reboot => "session-reboot",
            SessionAction::PowerOff => "session-power-off",
        })
    }

    fn of_key(key: Key) -> Option<SessionAction> {
        SessionAction::ROWS.into_iter().find(|a| a.key() == key)
    }
}

/// What the menu shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    List,
    /// This dangerous row is a confirmation card.
    Confirm(SessionAction),
}

/// What the menu does with an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Stays open; `changed` when its look did (a frame is due).
    Keep { changed: bool },
    /// Closes, with the chosen action if one was chosen.
    Close(Option<SessionAction>),
}

/// Measures a wrapped question: `(text, font size, width)` to its height.
pub type Fit<'f> = dyn FnMut(&str, f32, f32) -> f32 + 'f;

/// The menu without its painter.
pub struct SessionCore {
    theme: Theme,
    mode: Mode,
    /// The wrapped question's height in the current mode.
    question_h: f32,
    tree: Tree,
    input: Interaction,
}

impl std::fmt::Debug for SessionCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCore")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl SessionCore {
    /// The list menu in `theme`.
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool, now: Instant) -> SessionCore {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view(theme, Mode::List, 0.0), now);
        SessionCore {
            theme: theme.clone(),
            mode: Mode::List,
            question_h: 0.0,
            tree,
            input: Interaction::new(),
        }
    }

    /// The size the menu has now, logical px.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        match self.mode {
            Mode::List => list_size(&self.theme),
            Mode::Confirm(_) => confirm_size(&self.theme, self.question_h),
        }
    }

    fn show(&mut self, mode: Mode, now: Instant, fit: &mut Fit<'_>) {
        self.mode = mode;
        self.question_h = match mode.action().and_then(SessionAction::ask) {
            Some(ask) => fit(ask, self.theme.type_scale.body, question_width(&self.theme)),
            None => 0.0,
        };
        self.tree
            .apply(view(&self.theme, mode, self.question_h), now);
    }

    /// Rebuilds the view when the theme changed; whether it did. `fit` is the
    /// caller's measure.
    pub fn set_theme(&mut self, theme: &Theme, now: Instant, fit: &mut Fit<'_>) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        if let Some(ask) = self.mode.action().and_then(SessionAction::ask) {
            self.question_h = fit(ask, theme.type_scale.body, question_width(theme));
        }
        self.tree
            .apply(view(theme, self.mode, self.question_h), now);
        true
    }

    /// Handles one event; `fit` is the caller's measure.
    pub fn input(&mut self, ev: &InputEvent, now: Instant, fit: &mut Fit<'_>) -> Outcome {
        let before = self.tree.highlight();
        let events = menu_events(&mut self.input, &mut self.tree, ev, now);
        let mut changed = !events.is_empty();
        for event in &events {
            match event {
                UiEvent::Escape => match self.mode {
                    Mode::List => return Outcome::Close(None),
                    Mode::Confirm(action) => self.back(action, now, fit),
                },
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    let keyboard = matches!(event, UiEvent::Activate(_));
                    if let Some(out) = self.chose(*key, keyboard, now, fit) {
                        return out;
                    }
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        changed |= self.tree.highlight() != before;
        Outcome::Keep { changed }
    }

    /// Back to the list; the keyboard returns to the row it left.
    fn back(&mut self, action: SessionAction, now: Instant, fit: &mut Fit<'_>) {
        self.show(Mode::List, now, fit);
        self.input.focus_key(&mut self.tree, action.key());
    }

    fn chose(
        &mut self,
        key: Key,
        keyboard: bool,
        now: Instant,
        fit: &mut Fit<'_>,
    ) -> Option<Outcome> {
        if let Some(action) = SessionAction::of_key(key) {
            if action.needs_confirm() {
                self.show(Mode::Confirm(action), now, fit);
                self.input.focus_key(&mut self.tree, cancel_key());
                return None;
            }
            return Some(Outcome::Close(Some(action)));
        }
        let Mode::Confirm(action) = self.mode else {
            return None;
        };
        if key == do_key() {
            Some(Outcome::Close(Some(action)))
        } else if key == cancel_key() {
            self.show(Mode::List, now, fit);
            if keyboard {
                self.input.focus_key(&mut self.tree, action.key());
            }
            None
        } else {
            None
        }
    }
}

impl SessionCore {
    /// Whether the confirmation card is shown: Escape then goes back to the
    /// list instead of closing.
    #[must_use]
    pub fn asking(&self) -> bool {
        matches!(self.mode, Mode::Confirm(_))
    }

    /// Lays the body out in its own size with `lay`.
    pub fn layout_in(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        let size = self.size();
        lay(&mut self.tree, size);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "session body: layout fault");
        }
    }

    /// Paints the body at its own coordinates.
    pub fn paint(&mut self, painter: &mut celestina_paint::Painter, now: Instant, hidden: &[Key]) {
        self.tree.paint_hiding(painter, now, hidden);
    }

    /// The menu's tree (the sheet reads its subtitle's place from it).
    #[must_use]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Whether the body's own animations (hover, focus) still run.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now)
    }

    /// The rect of the body's node `key` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn node_rect(&self, key: Key) -> Option<celestina_paint::Rect> {
        self.tree.node_by_key(key).map(|id| self.tree.rect(id))
    }
}

impl Mode {
    fn action(self) -> Option<SessionAction> {
        match self {
            Mode::List => None,
            Mode::Confirm(a) => Some(a),
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::{Rect, Role, TextStyle, Weight};
    use celestina_ui::{ViewKind, BTN_LEFT};
    use celestina_wayland::Modifiers;

    use super::*;
    use crate::island::view::text_style;

    const DOWN: u32 = 0xff54;
    const UP: u32 = 0xff52;
    const RETURN: u32 = 0xff0d;
    const ESCAPE: u32 = 0xff1b;

    /// 7 px per char, 16 px a line, wrapped at the width when given.
    fn fake_measure(s: &str, _: &TextStyle, max: Option<f32>) -> (f32, f32) {
        let w = 7.0 * s.chars().count() as f32;
        let lines = max.map_or(1.0, |m| (w / m).ceil().max(1.0));
        (w.min(max.unwrap_or(w)), 16.0 * lines)
    }

    fn fit(s: &str, _size: f32, width: f32) -> f32 {
        fake_measure(
            s,
            &text_style(13.0, Weight::Regular, false, Role::TextHi),
            Some(width),
        )
        .1
    }

    fn core() -> SessionCore {
        let theme = Theme::dark();
        let mut core = SessionCore::new(&theme, false, Instant::now());
        core.tree.layout_measured(core.size(), fake_measure);
        core
    }

    fn relayout(core: &mut SessionCore) {
        let size = core.size();
        core.tree.layout_measured(size, fake_measure);
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

    fn press(core: &mut SessionCore, keysym: u32) -> Outcome {
        let out = core.input(&key(keysym), Instant::now(), &mut fit);
        relayout(core);
        out
    }

    fn rect_of(core: &SessionCore, key: Key) -> Rect {
        match core.tree.node_by_key(key) {
            Some(id) => core.tree.rect(id),
            None => panic!("no node for {key:?}"),
        }
    }

    fn focused(core: &SessionCore) -> Option<Key> {
        core.tree.highlight().focused.map(|id| core.tree.key_of(id))
    }

    fn click(core: &mut SessionCore, key: Key) -> Outcome {
        let r = rect_of(core, key);
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let now = Instant::now();
        let down = InputEvent::PointerPress {
            x,
            y,
            button: BTN_LEFT,
            serial: 2,
        };
        let up = InputEvent::PointerRelease {
            x,
            y,
            button: BTN_LEFT,
            serial: 3,
        };
        core.input(&down, now, &mut fit);
        let out = core.input(&up, now, &mut fit);
        relayout(core);
        out
    }

    #[test]
    fn the_rows_are_lock_suspend_divider_log_out_reboot_power_off_in_order() {
        let theme = Theme::dark();
        let core = core();
        let size = core.size();
        assert_eq!(size, list_size(&theme));
        assert_eq!(size.0, 240.0);
        let card = rect_of(&core, menu_key());
        assert_eq!((card.w, card.h), size);
        let ys: Vec<f32> = SessionAction::ROWS
            .iter()
            .map(|a| {
                let r = rect_of(&core, a.key());
                assert_eq!(r.h, theme.row_height);
                r.y
            })
            .collect();
        let divider = rect_of(&core, divider_key());
        assert_eq!(divider.h, 1.0 + 2.0 * theme.spacing.xs);
        let subtitle = rect_of(&core, crate::sheet::subtitle::row_key());
        assert_eq!(
            (subtitle.y, subtitle.h),
            (theme.spacing.xs, crate::sheet::subtitle::SUBTITLE_H)
        );
        assert_eq!(ys[0], subtitle.y + subtitle.h, "the rows start under it");
        assert!(ys[0] < ys[1] && ys[1] < divider.y && divider.y < ys[2]);
        assert!(ys[2] < ys[3] && ys[3] < ys[4]);
        assert_eq!(ys[2], divider.y + divider.h);
        assert_eq!(
            ys[4] + theme.row_height + theme.spacing.sm,
            size.1,
            "the card ends under the last row"
        );
    }

    #[test]
    fn lock_and_suspend_close_at_once_with_their_choice() {
        for action in [SessionAction::Lock, SessionAction::Suspend] {
            let mut c = core();
            assert_eq!(click(&mut c, action.key()), Outcome::Close(Some(action)));
            let mut c = core();
            press(&mut c, DOWN);
            if action == SessionAction::Suspend {
                press(&mut c, DOWN);
            }
            assert_eq!(press(&mut c, RETURN), Outcome::Close(Some(action)));
        }
    }

    #[test]
    fn a_dangerous_row_asks_first_and_the_body_grows() {
        let theme = Theme::dark();
        let mut c = core();
        let list = c.size();
        let out = click(&mut c, SessionAction::Reboot.key());
        assert_eq!(out, Outcome::Keep { changed: true });
        let grown = c.size();
        assert!(grown.1 > list.1 && grown.0 == list.0);
        // The card took the row's place.
        let card = rect_of(&c, confirm_key());
        assert_eq!(
            card.y,
            rect_of(&c, SessionAction::LogOut.key()).y + theme.row_height
        );
        assert_eq!(card.h, view::card_height(&theme, 16.0));
        // The way back shrinks it again.
        click(&mut c, cancel_key());
        assert_eq!(c.size(), list);
    }

    #[test]
    fn a_double_click_on_a_dangerous_row_never_reaches_its_action() {
        let theme = Theme::dark();
        for action in [
            SessionAction::LogOut,
            SessionAction::Reboot,
            SessionAction::PowerOff,
        ] {
            let mut c = core();
            let row = rect_of(&c, action.key());
            click(&mut c, action.key());
            let card = rect_of(&c, confirm_key());
            assert_eq!(card.y, row.y, "{action:?}: the card takes the row's place");
            for key in [do_key(), cancel_key()] {
                let button = rect_of(&c, key);
                assert!(
                    button.y >= row.y + theme.row_height,
                    "{action:?}: {button:?} starts inside the row {row:?}"
                );
            }
        }
    }

    #[test]
    fn confirm_sends_the_action_and_cancel_returns_to_the_list() {
        for action in [
            SessionAction::LogOut,
            SessionAction::Reboot,
            SessionAction::PowerOff,
        ] {
            let mut c = core();
            click(&mut c, action.key());
            assert_eq!(click(&mut c, do_key()), Outcome::Close(Some(action)));
            let mut c = core();
            click(&mut c, action.key());
            assert_eq!(click(&mut c, cancel_key()), Outcome::Keep { changed: true });
            assert_eq!(c.mode, Mode::List);
            assert_eq!(c.size(), list_size(&Theme::dark()));
            assert_eq!(c.mode.action(), None);
            assert_eq!(
                click(&mut c, action.key()),
                Outcome::Keep { changed: true },
                "its row asks again"
            );
        }
    }

    #[test]
    fn escape_in_the_confirm_card_cancels_and_escape_in_the_list_closes_the_body() {
        let mut c = core();
        click(&mut c, SessionAction::PowerOff.key());
        assert_eq!(press(&mut c, ESCAPE), Outcome::Keep { changed: true });
        assert_eq!(c.mode, Mode::List);
        assert_eq!(focused(&c), Some(SessionAction::PowerOff.key()));
        assert_eq!(press(&mut c, ESCAPE), Outcome::Close(None));
    }

    #[test]
    fn the_keyboard_moves_through_the_rows_and_enter_chooses() {
        let mut c = core();
        assert_eq!(focused(&c), None);
        for action in SessionAction::ROWS {
            press(&mut c, DOWN);
            assert_eq!(focused(&c), Some(action.key()), "the divider is skipped");
        }
        press(&mut c, DOWN);
        assert_eq!(focused(&c), Some(SessionAction::Lock.key()), "wraps");
        press(&mut c, UP);
        assert_eq!(focused(&c), Some(SessionAction::PowerOff.key()));
        // Enter on a dangerous row asks, with the keyboard on Cancelar.
        assert_eq!(press(&mut c, RETURN), Outcome::Keep { changed: true });
        assert_eq!(c.mode, Mode::Confirm(SessionAction::PowerOff));
        assert_eq!(focused(&c), Some(cancel_key()));
        press(&mut c, DOWN);
        assert_eq!(focused(&c), Some(do_key()));
        assert_eq!(
            press(&mut c, RETURN),
            Outcome::Close(Some(SessionAction::PowerOff))
        );
    }

    #[test]
    fn each_action_is_the_verb_its_bus_word_names() {
        let parse = |words: &[&str]| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            Verb::parse(&words)
        };
        assert_eq!(parse(&["lock"]), Ok(SessionAction::Lock.verb()));
        assert_eq!(
            parse(&["lock", "suspend"]),
            Ok(SessionAction::Suspend.verb())
        );
        assert_eq!(SessionAction::Suspend.verb(), Verb::LockAndSuspend);
        assert_eq!(
            parse(&["session", "log-out"]),
            Ok(SessionAction::LogOut.verb())
        );
        assert_eq!(
            parse(&["session", "reboot"]),
            Ok(SessionAction::Reboot.verb())
        );
        assert_eq!(
            parse(&["session", "power-off"]),
            Ok(SessionAction::PowerOff.verb())
        );
        let asks: Vec<bool> = SessionAction::ROWS
            .iter()
            .map(|a| a.needs_confirm())
            .collect();
        assert_eq!(asks, [false, false, true, true, true]);
    }

    fn find(v: &celestina_ui::View, key: Key) -> Option<&celestina_ui::View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn the_confirm_card_uses_the_danger_roles() {
        let theme = Theme::dark();
        let v = view(&theme, Mode::Confirm(SessionAction::Reboot), 16.0);
        let card = find(&v, confirm_key()).expect("the card");
        assert_eq!(
            card.kind,
            ViewKind::Box {
                fill: Some(Role::DangerFill),
                stroke: Some(Role::DangerBorder),
                haze: false,
                radius: theme.radius_button
            }
        );
        let ask = find(card, ask_key()).expect("the question");
        let ViewKind::Text { text, style, .. } = &ask.kind else {
            panic!("a text");
        };
        assert_eq!(
            (text.as_str(), style.role),
            (copy::SESSION_ASK_REBOOT, Role::DangerFillInk)
        );
        let fill = |k| match find(card, k).map(|n| &n.kind) {
            Some(ViewKind::Box { fill, .. }) => *fill,
            other => panic!("{other:?}"),
        };
        assert_eq!(fill(cancel_key()), Some(Role::ControlFill));
        assert_eq!(fill(do_key()), Some(Role::Danger));
        let label = |k: Key| match &find(
            find(card, k).unwrap(),
            Key::named(if k == cancel_key() {
                "session-cancel-label"
            } else {
                "session-do-label"
            }),
        )
        .unwrap()
        .kind
        {
            ViewKind::Text { text, style, .. } => (text.clone(), style.role),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            label(cancel_key()),
            (copy::SESSION_CANCEL.to_owned(), Role::TextHi)
        );
        assert_eq!(
            label(do_key()),
            (copy::SESSION_REBOOT.to_owned(), Role::DangerInk)
        );
    }

    #[test]
    fn the_icons_it_names_exist_in_the_icon_set() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../celestina-style/icons");
        for action in SessionAction::ROWS {
            assert!(
                dir.join(format!("{}.svg", action.icon())).is_file(),
                "{}",
                action.icon()
            );
        }
        assert!(dir.join("power.svg").is_file(), "the piece's glyph");
    }
}
