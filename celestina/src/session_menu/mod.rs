//! The session menu (CEL-I-006): an `xdg_popup` of the Island's surface,
//! hung below the Session piece. The lock and suspend rows close it with their
//! choice at once; the log-out, reboot and power-off rows swap their row for a
//! danger card that asks first, and the menu grows with its top edge fixed.
//!
//! Like the overflow menu it does not act: a choice is written into the slot
//! the app handed it and the popup closes; the app makes the call once the
//! Island's surface gets `PopupClosed`, so no popup grab is held while the
//! lock maps. Escape on the card goes back to the list, Escape on the list
//! and a press outside close with no choice. The placement is the level
//! popups' ([`crate::osd::popup::placement`]): bottom edge, bottom gravity, so
//! a resize leaves the top where it is.

mod view;

use std::{cell::Cell, rc::Rc, time::Instant};

use celestina_paint::{silhouette_slices, Rect, Role, Theme, Weight};
use celestina_shell::session::PowerAction;
use celestina_ui::{Interaction, Key, Tree, UiEvent};
use celestina_wayland::{FrameCx, InputEvent, PopupAction, PopupShell, SurfaceCtl};

pub use view::{
    ask_key, cancel_key, confirm_key, confirm_size, divider_key, do_key, list_size, menu_key,
    question_width, view,
};

use crate::{
    command::Verb,
    copy,
    island::{
        menu::{menu_events, SharedPainter},
        state::SILHOUETTE_STEP,
        view::text_style,
    },
};

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

/// Where a chosen action is left for the app to act on.
pub type SessionSlot = Rc<Cell<Option<SessionAction>>>;

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

/// Height of `text` wrapped at `width` with font size `size`.
type Fit = Box<dyn FnMut(&str, f32, f32) -> f32>;

/// The menu without its painter.
pub struct SessionCore {
    theme: Theme,
    mode: Mode,
    /// The wrapped question's height in the current mode.
    question_h: f32,
    asked: (f32, f32),
    fit: Fit,
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
    /// The list menu in `theme`; `fit` measures a wrapped question.
    #[must_use]
    pub fn new(
        theme: &Theme,
        reduced: bool,
        now: Instant,
        fit: impl FnMut(&str, f32, f32) -> f32 + 'static,
    ) -> SessionCore {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view(theme, Mode::List, 0.0), now);
        SessionCore {
            theme: theme.clone(),
            mode: Mode::List,
            question_h: 0.0,
            asked: list_size(theme),
            fit: Box::new(fit),
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

    /// The size to ask the compositor for, once, when the menu is no longer
    /// the size it was last asked for.
    pub fn resize_wanted(&mut self) -> Option<(f32, f32)> {
        let size = self.size();
        (size != self.asked).then(|| {
            self.asked = size;
            size
        })
    }

    fn show(&mut self, mode: Mode, now: Instant) {
        self.mode = mode;
        self.question_h = match mode.action().and_then(SessionAction::ask) {
            Some(ask) => (self.fit)(ask, self.theme.type_scale.body, question_width(&self.theme)),
            None => 0.0,
        };
        self.tree
            .apply(view(&self.theme, mode, self.question_h), now);
    }

    /// Rebuilds the view when the painter's theme changed; `fit` measures
    /// with the frame's own painter.
    fn set_theme(
        &mut self,
        theme: &Theme,
        now: Instant,
        mut fit: impl FnMut(&str, f32, f32) -> f32,
    ) {
        if *theme == self.theme {
            return;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        if let Some(ask) = self.mode.action().and_then(SessionAction::ask) {
            self.question_h = fit(ask, theme.type_scale.body, question_width(theme));
        }
        self.tree
            .apply(view(theme, self.mode, self.question_h), now);
    }

    /// Handles one event.
    pub fn input(&mut self, ev: &InputEvent, now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let events = menu_events(&mut self.input, &mut self.tree, ev, now);
        let mut changed = !events.is_empty();
        for event in &events {
            match event {
                UiEvent::Escape => match self.mode {
                    Mode::List => return Outcome::Close(None),
                    Mode::Confirm(action) => self.back(action, now),
                },
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    let keyboard = matches!(event, UiEvent::Activate(_));
                    if let Some(out) = self.chose(*key, keyboard, now) {
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
    fn back(&mut self, action: SessionAction, now: Instant) {
        self.show(Mode::List, now);
        self.input.focus_key(&mut self.tree, action.key());
    }

    fn chose(&mut self, key: Key, keyboard: bool, now: Instant) -> Option<Outcome> {
        if let Some(action) = SessionAction::of_key(key) {
            if action.needs_confirm() {
                self.show(Mode::Confirm(action), now);
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
            self.show(Mode::List, now);
            if keyboard {
                self.input.focus_key(&mut self.tree, action.key());
            }
            None
        } else {
            None
        }
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

/// The menu as a popup shell.
pub struct SessionMenu {
    core: SessionCore,
    painter: SharedPainter,
    choice: SessionSlot,
}

impl std::fmt::Debug for SessionMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionMenu")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl SessionMenu {
    /// The menu in `theme`, drawn with `painter`; a chosen action is left in
    /// `choice`.
    #[must_use]
    pub fn new(
        painter: SharedPainter,
        theme: &Theme,
        reduced: bool,
        choice: SessionSlot,
    ) -> SessionMenu {
        let measuring = Rc::clone(&painter);
        let fit = move |text: &str, size: f32, width: f32| match measuring.try_borrow_mut() {
            Ok(mut painter) => {
                let style = text_style(size, Weight::Regular, false, Role::DangerFillInk);
                painter.text().measure(text, &style, Some(width), 1.0).1
            }
            // Never reached in practice (inputs run between frames): one
            // line is the honest lower bound.
            Err(_) => size,
        };
        SessionMenu {
            core: SessionCore::new(theme, reduced, Instant::now(), fit),
            painter,
            choice,
        }
    }

    /// The size the menu opens at.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        self.core.size()
    }
}

impl PopupShell for SessionMenu {
    fn frame(&mut self, cx: &mut FrameCx<'_>) {
        let Ok(mut painter) = self.painter.try_borrow_mut() else {
            tracing::warn!("session menu: the painter is in use; frame skipped");
            cx.ctl.request_redraw();
            return;
        };
        let now = cx.now;
        let theme = painter.theme().clone();
        self.core.set_theme(&theme, now, |text, size, width| {
            let style = text_style(size, Weight::Regular, false, Role::DangerFillInk);
            painter.text().measure(text, &style, Some(width), 1.0).1
        });
        if let Some(size) = self.core.resize_wanted() {
            cx.ctl.resize_popup(size);
        }
        let radius = theme.radius_md;
        let dpr = cx.scale as f32;
        let tree = &mut self.core.tree;
        tree.layout(cx.size, painter.text(), dpr);
        if let Some(fault) = tree.take_fault() {
            tracing::warn!(%fault, "session menu: layout fault");
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
                tracing::info!(?choice, "session menu: closed from the menu");
                self.choice.set(choice);
                PopupAction::Close
            }
            Outcome::Keep { changed } => {
                if let Some(size) = self.core.resize_wanted() {
                    tracing::debug!(?size, "session menu: a confirmation card came or went");
                    ctl.resize_popup(size);
                }
                if changed {
                    ctl.request_redraw();
                }
                PopupAction::Keep
            }
        }
    }

    fn dismissed(&mut self) {
        tracing::info!("session menu: dismissed by the compositor");
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_ui::{ViewKind, BTN_LEFT};
    use celestina_wayland::{AnchorEdge, Gravity, Modifiers};

    use super::*;

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
        let mut core = SessionCore::new(&theme, false, Instant::now(), fit);
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
        let out = core.input(&key(keysym), Instant::now());
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
        core.input(&down, now);
        let out = core.input(&up, now);
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
        assert_eq!(ys[0], theme.spacing.sm);
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
    fn a_dangerous_row_asks_first_and_grows_keeping_its_top() {
        let theme = Theme::dark();
        let mut c = core();
        let list = c.size();
        assert_eq!(c.resize_wanted(), None);
        let out = click(&mut c, SessionAction::Reboot.key());
        assert_eq!(out, Outcome::Keep { changed: true });
        let grown = c.size();
        assert!(grown.1 > list.1 && grown.0 == list.0);
        assert_eq!(c.resize_wanted(), Some(grown));
        assert_eq!(c.resize_wanted(), None, "asked once");
        // The card took the row's place.
        let card = rect_of(&c, confirm_key());
        assert_eq!(
            card.y,
            rect_of(&c, SessionAction::LogOut.key()).y + theme.row_height
        );
        assert_eq!(card.h, view::card_height(&theme, 16.0));
        // The compositor keeps the top edge where it was.
        let p = crate::osd::popup::placement(
            &theme,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
        );
        assert_eq!(
            (p.anchor_edge, p.gravity),
            (AnchorEdge::Bottom, Gravity::Bottom)
        );
        assert_eq!(p.offset, (0, theme.spacing.sm as i32));
        // The way back shrinks it again.
        click(&mut c, cancel_key());
        assert_eq!(c.resize_wanted(), Some(list));
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
    fn escape_in_the_confirm_card_cancels_and_escape_in_the_list_closes() {
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
