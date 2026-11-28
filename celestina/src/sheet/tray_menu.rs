//! A tray app's menu as the body of a capsule menu window (CEL-D-016, design
//! §5.6): the window of the bar icon the menu was asked from. The body opens
//! before the menu exists: it is *waiting*, 300 px wide and no tall, while
//! the window holds as a drop ([`super::motion::SheetMotion::hold`]) and the
//! app's dbusmenu answers. When the layout arrives the body builds a
//! [`MenuPane`] and the window opens to its size; a republished layout
//! replaces the pane's tree in place. The window's glass is the sheet's, so
//! the body draws no haze of its own.
//!
//! The body does not act. [`BodyOutcome::TrayMenu`] carries what the person
//! did and the Island sends it (the entry chosen, the submenu asked about);
//! one place in the app closes the item's menu once no window holds it any
//! more.

use std::time::Instant;

use celestina_paint::{Painter, Rect, Theme};
use celestina_ui::Tree;
use celestina_wayland::InputEvent;

use super::{body::BodyOutcome, Measure};
use crate::tray::{
    menu::{ShellEntry, MENU_W},
    pane::{MenuOutcome, MenuPane, PaneFrame},
    ItemId, MenuTree,
};

/// A bar icon's menu window body: waiting for its menu, then the pane.
pub struct TrayMenuBody {
    item: ItemId,
    /// The menu was asked from the keyboard: its first entry starts focused.
    keyboard: bool,
    /// The tallest the window gets: the pane scrolls past it.
    max_h: f32,
    /// `None` while the app has not answered.
    pane: Option<MenuPane>,
    /// While waiting: the height of the window this one took over on its
    /// capsule, kept so the window stays while the app is asked (0: none, the
    /// window holds as a drop).
    hold_h: f32,
}

impl std::fmt::Debug for TrayMenuBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayMenuBody")
            .field("item", &self.item)
            .field("waiting", &self.pane.is_none())
            .finish_non_exhaustive()
    }
}

impl TrayMenuBody {
    /// The body of `item`'s menu while the app has not answered: 300 wide and
    /// no tall. `max_h` is the tallest its window gets; `keyboard` asks for
    /// the first entry focused once the menu arrives.
    #[must_use]
    pub fn waiting(item: ItemId, keyboard: bool, max_h: f32) -> TrayMenuBody {
        TrayMenuBody {
            item,
            keyboard,
            max_h,
            pane: None,
            hold_h: 0.0,
        }
    }

    /// Keeps `h` (at most the window's tallest) as the waiting body's height:
    /// the window of the menu this one replaces stays at the height it has
    /// until the app answers.
    pub fn hold_height(&mut self, h: f32) {
        self.hold_h = if h.is_finite() {
            h.clamp(0.0, self.max_h)
        } else {
            0.0
        };
    }

    /// Whether the body waits and keeps a height for its window.
    #[must_use]
    pub fn holds_height(&self) -> bool {
        self.pane.is_none() && self.hold_h > 0.0
    }

    /// Takes in the app's menu: builds the pane the first time, and shows
    /// the republished `tree` (and `shell`'s entry) in the pane after that.
    /// Whether the look changed.
    pub fn arrive(
        &mut self,
        theme: &Theme,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        reduced: bool,
        now: Instant,
    ) -> bool {
        if let Some(pane) = self.pane.as_mut() {
            return pane.replace(tree, shell, now);
        }
        let mut pane = MenuPane::new(
            theme,
            tree,
            shell,
            PaneFrame::WINDOW,
            self.max_h,
            reduced,
            now,
        );
        if self.keyboard {
            pane.focus_first();
        }
        self.pane = Some(pane);
        true
    }

    /// The item whose menu this is.
    #[must_use]
    pub fn item(&self) -> &ItemId {
        &self.item
    }

    /// Whether the app has not answered yet.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.pane.is_none()
    }

    /// How many submenus are open below the root.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.pane.as_ref().map_or(0, MenuPane::depth)
    }

    /// The body's size: the pane's, or 300 wide and as tall as the window it
    /// replaced (none, with no window to keep) while waiting.
    pub(super) fn size(&self) -> (f32, f32) {
        self.pane
            .as_ref()
            .map_or((MENU_W, self.hold_h), MenuPane::size)
    }

    /// Whether the wheel is the pane's (it scrolls) rather than the sheet's.
    pub(super) fn takes_wheel(&self) -> bool {
        self.pane.as_ref().is_some_and(MenuPane::can_scroll)
    }

    /// The focused row's rect, in body coordinates.
    pub(super) fn focused_rect(&self) -> Option<Rect> {
        self.pane.as_ref().and_then(MenuPane::focused_rect)
    }

    /// Handles `ev` (already in body coordinates). The window's own
    /// outcomes (the keep and the close) are the sheet's; what the person
    /// chose is the Island's.
    pub(super) fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let Some(pane) = self.pane.as_mut() else {
            return BodyOutcome::Keep { changed: false };
        };
        match pane.input(ev, now) {
            MenuOutcome::Keep { changed } => BodyOutcome::Keep { changed },
            MenuOutcome::Close => BodyOutcome::Close,
            outcome => BodyOutcome::TrayMenu(outcome),
        }
    }

    /// Takes in a new theme and cuts the labels to the room each has with
    /// `measure`; whether the look changed.
    pub(super) fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        self.pane.as_mut().is_some_and(|pane| {
            let themed = pane.set_theme(theme, now);
            pane.fit_labels(measure, now) | themed
        })
    }

    pub(super) fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        if let Some(pane) = self.pane.as_mut() {
            pane.layout(lay);
        }
    }

    pub(super) fn paint(&mut self, painter: &mut Painter, now: Instant) {
        if let Some(pane) = self.pane.as_mut() {
            pane.paint(painter, now);
        }
    }

    /// The label the pane shows in row `index`, as it is built (tests).
    #[cfg(test)]
    fn pane_shown_label(&self, index: usize) -> Option<String> {
        self.pane.as_ref().and_then(|pane| pane.shown_label(index))
    }

    pub(super) fn tree(&self) -> Option<&Tree> {
        self.pane.as_ref().map(MenuPane::tree)
    }

    pub(super) fn is_animating(&self, now: Instant) -> bool {
        self.pane
            .as_ref()
            .is_some_and(|pane| pane.is_animating(now))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use celestina_paint::TextStyle;
    use celestina_wayland::Modifiers;

    use super::*;
    use crate::tray::MenuEntry;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn item() -> ItemId {
        ItemId {
            bus_name: ":1.7".to_owned(),
            path: "/StatusNotifierItem".to_owned(),
        }
    }

    fn entry(id: i32, label: &str) -> MenuEntry {
        MenuEntry {
            id,
            label: label.to_owned(),
            enabled: true,
            ..MenuEntry::default()
        }
    }

    /// `n` plain entries.
    fn menu(n: i32) -> MenuTree {
        MenuTree {
            id: 0,
            entries: (1..=n).map(|i| entry(i, &format!("Entry {i}"))).collect(),
        }
    }

    /// One entry that opens a submenu of one entry.
    fn nested() -> MenuTree {
        MenuTree {
            id: 0,
            entries: vec![MenuEntry {
                submenu: Some(MenuTree {
                    id: 1,
                    entries: vec![entry(11, "Inner")],
                }),
                ..entry(1, "More")
            }],
        }
    }

    fn lay(body: &mut TrayMenuBody) {
        body.layout(&mut |tree, size| tree.layout_measured(size, fake_measure));
    }

    fn arrived(tree: MenuTree, now: Instant) -> TrayMenuBody {
        let theme = Theme::dark();
        let mut body = TrayMenuBody::waiting(item(), false, 496.0);
        assert!(body.arrive(&theme, tree, None, false, now));
        lay(&mut body);
        body
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

    #[test]
    fn a_waiting_body_is_300_wide_and_arrives_at_its_menus_height() {
        let t0 = Instant::now();
        let mut body = TrayMenuBody::waiting(item(), false, 496.0);
        assert!(body.is_waiting());
        assert_eq!(body.size(), (300.0, 0.0));
        assert_eq!(body.depth(), 0);
        assert!(body.tree().is_none(), "nothing to focus or measure yet");
        assert!(!body.is_animating(t0));
        // Input before the menu does nothing.
        assert_eq!(
            body.input(&key(0xff54), t0),
            BodyOutcome::Keep { changed: false }
        );
        // Three entries: 8 px of padding over and under 3 rows of 40.
        let theme = Theme::dark();
        assert!(body.arrive(&theme, menu(3), None, false, t0));
        assert!(!body.is_waiting());
        assert_eq!(body.item(), &item());
        assert_eq!(body.size(), (300.0, 16.0 + 3.0 * theme.row_height));
        // A republish with one more entry grows it in place.
        assert!(body.arrive(&theme, menu(4), None, false, t0));
        assert_eq!(body.size(), (300.0, 16.0 + 4.0 * theme.row_height));
        // The same menu again changes nothing.
        assert!(!body.arrive(&theme, menu(4), None, false, t0));
    }

    #[test]
    fn a_menu_asked_from_the_keyboard_starts_with_its_first_entry_focused() {
        let t0 = Instant::now();
        let mut body = TrayMenuBody::waiting(item(), true, 496.0);
        assert!(body.arrive(&Theme::dark(), menu(2), None, false, t0));
        let tree = body.tree().expect("a tree");
        assert!(tree.highlight().focused.is_some());
        assert!(body.focused_rect().is_some());
        // The pointer's menu starts with no focus.
        let plain = arrived(menu(2), t0);
        assert!(plain
            .tree()
            .is_some_and(|t| t.highlight().focused.is_none()));
        assert_eq!(plain.focused_rect(), None);
    }

    #[test]
    fn a_body_taller_than_496_stays_496_and_takes_the_wheel() {
        let t0 = Instant::now();
        let mut body = arrived(menu(30), t0);
        assert_eq!(body.size(), (300.0, 496.0));
        assert!(body.takes_wheel());
        // The wheel is the pane's: it scrolls the rows.
        let wheel = InputEvent::Axis {
            x: 150.0,
            y: 100.0,
            horizontal: 0.0,
            vertical: 120.0,
        };
        assert_eq!(body.input(&wheel, t0), BodyOutcome::Keep { changed: true });
        assert!(body.is_animating(t0), "the scroll bar shows");
        assert!(!body.is_animating(t0 + Duration::from_secs(2)));
        // A short menu leaves the wheel to the sheet.
        let short = arrived(menu(2), t0);
        assert!(!short.takes_wheel());
    }

    #[test]
    fn escape_belongs_to_the_body_only_below_the_root() {
        let t0 = Instant::now();
        let mut body = arrived(nested(), t0);
        assert_eq!(body.depth(), 0);
        // At the root Escape is the window's: the pane asks to close.
        assert_eq!(body.input(&key(0xff1b), t0), BodyOutcome::Close);
        // Enter on the entry drills in (the item is asked about the submenu).
        body.input(&key(0xff09), t0);
        let outcome = body.input(&key(0xff0d), t0);
        lay(&mut body);
        assert_eq!(outcome, BodyOutcome::TrayMenu(MenuOutcome::Submenu(1)));
        assert_eq!(body.depth(), 1);
        // Below the root Escape goes back one level.
        let outcome = body.input(&key(0xff1b), t0);
        assert_eq!(outcome, BodyOutcome::TrayMenu(MenuOutcome::Back));
        assert_eq!(body.depth(), 0);
    }

    #[test]
    fn a_new_theme_rebuilds_the_pane_and_says_so_once() {
        let t0 = Instant::now();
        let mut body = arrived(menu(2), t0);
        let dark = Theme::dark();
        // The labels' cuts are taken in once, then nothing changes.
        body.sync(&dark, t0, &mut fake_measure);
        assert!(!body.sync(&dark, t0, &mut fake_measure));
        let mut other = Theme::dark();
        other.row_height += 4.0;
        assert!(body.sync(&other, t0, &mut fake_measure));
        assert!(!body.sync(&other, t0, &mut fake_measure));
        assert_eq!(body.size().1, 16.0 + 2.0 * other.row_height);
        // A waiting body has no theme to change.
        let mut waiting = TrayMenuBody::waiting(item(), false, 496.0);
        assert!(!waiting.sync(&other, t0, &mut fake_measure));
    }

    /// 14 px per char (wide letters and CJK ideographs), 16 px tall.
    fn wide_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (14.0 * s.chars().count() as f32, 16.0)
    }

    #[test]
    fn a_wide_label_in_the_window_ends_inside_its_row() {
        use celestina_ui::{Key, ELLIPSIS};
        let theme = Theme::dark();
        let t0 = Instant::now();
        let wide = |c: char| c.to_string().repeat(36);
        let entries = [('W', false), ('\u{4e2d}', true)]
            .iter()
            .enumerate()
            .map(|(i, (c, sub))| MenuEntry {
                submenu: sub.then(|| MenuTree {
                    id: 9,
                    entries: Vec::new(),
                }),
                ..entry(i as i32 + 1, &wide(*c))
            })
            .collect();
        let mut body = TrayMenuBody::waiting(item(), false, 496.0);
        body.arrive(&theme, MenuTree { id: 0, entries }, None, false, t0);
        assert!(
            body.sync(&theme, t0, &mut wide_measure),
            "the labels were cut"
        );
        assert!(!body.sync(&theme, t0, &mut wide_measure), "and only once");
        body.layout(&mut |tree, size| tree.layout_measured(size, wide_measure));
        let Some(tree) = body.tree() else {
            panic!("no pane");
        };
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let gap = theme.spacing.md;
        for (i, chevron) in [(0usize, false), (1, true)] {
            let row = rect(crate::tray::menu::entry_key(i));
            let label = rect(Key::indexed("tray-menu-label", i as u64));
            let end = if chevron {
                rect(Key::indexed("tray-menu-more", i as u64)).x - gap
            } else {
                row.x + row.w - gap
            };
            let shown = body
                .pane_shown_label(i)
                .unwrap_or_else(|| panic!("no label {i}"));
            let text_w = 14.0 * shown.chars().count() as f32;
            assert!(text_w <= label.w + 0.01, "{shown}: {text_w} in {label:?}");
            assert!(label.x + text_w <= end + 0.01, "{shown} ends before {end}");
            assert!(shown.contains(ELLIPSIS), "{shown}");
        }
    }
}
