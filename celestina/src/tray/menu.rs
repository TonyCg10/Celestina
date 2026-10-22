//! The tray item's DBusMenu as popups (design §5.2, §5.5): each level of the
//! menu is a Haze card hung from the row that asked for it (the first) or
//! from the row of its parent's entry (a submenu, to the right, flipping at
//! the output's edge), every level an `xdg_popup` parented on the previous
//! one. The levels do not act: choosing an entry leaves `(item, entry id)`
//! in the slot the app handed them and closes the whole chain; the app sends
//! the `MenuEvent` once the root popup is gone.
//!
//! The chain shows one [`MenuModel`], shared by its levels: each level is
//! the subtree at its path of entry ids. When the item republishes its menu
//! the app replaces the model and asks the chain's popups to redraw; a
//! level then rebuilds its rows (a level whose path is gone shows nothing
//! until it is closed). Before a submenu opens, the level asks the host for
//! the item's `AboutToShow` through the sink the app handed it; a submenu
//! the item fills only then (its layout arrives empty) opens once the
//! republished model carries its entries.
//!
//! Keys go to the topmost level (the runtime routes them): Up and Down (Tab
//! and Shift+Tab) move the focus through the enabled entries, Enter and
//! Space choose, Right or Enter on a submenu entry opens it, Left and Escape
//! close this level only. A press outside the chain is the compositor's
//! `popup_done`, which dismisses every level.

use std::{cell::RefCell, rc::Rc, time::Instant};

use celestina_paint::{silhouette_slices, Rect, Role, Theme, Weight};
use celestina_ui::{view, Interaction, Key, Tree, UiEvent, View};
use celestina_wayland::{
    AnchorEdge, CloseReason, FrameCx, Gravity, InputEvent, Placement, PopupAction, PopupId,
    PopupShell, SurfaceCtl,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{ItemId, MenuTree, TrayCommand};
use crate::island::{
    menu::SharedPainter,
    state::SILHOUETTE_STEP,
    view::{centred, text_style},
};

/// Where a chosen entry is left for the app: the item and the `dbusmenu` id.
pub type ChoiceSlot = Rc<RefCell<Option<(ItemId, i32)>>>;

/// The menu tree an open chain shows, and a count that changes whenever
/// the app replaces it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuModel {
    /// The whole menu, from its root.
    pub tree: MenuTree,
    /// Grows with every replacement; a level compares it with the one it
    /// built its rows from.
    pub generation: u64,
}

impl MenuModel {
    /// Replaces the tree.
    pub fn replace(&mut self, tree: MenuTree) {
        self.tree = tree;
        self.generation += 1;
    }
}

/// The model shared by the chain's levels and the app.
pub type SharedMenu = Rc<RefCell<MenuModel>>;

/// Where a level asks the host for an item's `AboutToShow` before it opens
/// a submenu: the app's queue to the tray host.
pub type AskSink = Rc<dyn Fn(TrayCommand)>;

/// The popups a chain opened below its root, in order, so the app can
/// redraw every level when the model changes.
pub type Chain = Rc<RefCell<Vec<PopupId>>>;

/// What every level of one chain shares.
#[derive(Clone)]
pub struct MenuContext {
    /// The one painter.
    pub painter: SharedPainter,
    /// The item whose menu this is.
    pub item: ItemId,
    /// The menu shown.
    pub shared: SharedMenu,
    /// Where a chosen entry is left.
    pub choice: ChoiceSlot,
    /// Where `AboutToShow` is asked.
    pub ask: AskSink,
    /// The popups below the root.
    pub chain: Chain,
    /// Reduced motion.
    pub reduced: bool,
}

impl std::fmt::Debug for MenuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuContext")
            .field("item", &self.item)
            .field("chain", &self.chain)
            .finish_non_exhaustive()
    }
}

/// The level of `tree` at `path`: the submenu of the entry with each id in
/// turn, from the root. `None` when the path leads nowhere.
#[must_use]
pub fn level_at<'a>(tree: &'a MenuTree, path: &[i32]) -> Option<&'a MenuTree> {
    let mut level = tree;
    for id in path {
        level = level
            .entries
            .iter()
            .find(|e| e.id == *id)?
            .submenu
            .as_ref()?;
    }
    Some(level)
}

/// A menu level's width, logical px.
pub const MENU_W: f32 = 300.0;
/// The tallest a level gets, logical px; entries past it are not built.
pub const MAX_MENU_H: f32 = 640.0;
/// The longest label shown, in grapheme clusters, the ellipsis included.
pub const MAX_LABEL_GRAPHEMES: usize = 36;
/// The check mark's and the chevron's size.
const MARK: f32 = 16.0;

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_LEFT: u32 = 0xff51;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_RIGHT: u32 = 0xff53;
const XKB_KEY_DOWN: u32 = 0xff54;

/// The level's card.
#[must_use]
pub fn card_key() -> Key {
    Key::named("tray-menu")
}

/// The row of the entry at `index` of the level (separators included).
#[must_use]
pub fn entry_key(index: usize) -> Key {
    Key::indexed("tray-menu-entry", index as u64)
}

/// The height of a separator row.
fn separator_h(theme: &Theme) -> f32 {
    2.0 * theme.spacing.sm + 1.0
}

/// The height of the entry row `index` of `level`.
fn entry_h(theme: &Theme, level: &MenuTree, index: usize) -> f32 {
    if level.entries.get(index).is_some_and(|e| e.separator) {
        separator_h(theme)
    } else {
        theme.row_height
    }
}

/// How many entries of `level` are built in a card `height` tall: the
/// ones that fit under the padding, never more than [`MAX_MENU_H`] holds.
#[must_use]
pub fn drawn_len(theme: &Theme, level: &MenuTree, height: f32) -> usize {
    let room = height.min(MAX_MENU_H);
    let mut used = 2.0 * theme.spacing.sm;
    for index in 0..level.entries.len() {
        used += entry_h(theme, level, index);
        if used > room {
            return index;
        }
    }
    level.entries.len()
}

/// The popup's logical size for `level`: every entry up to [`MAX_MENU_H`].
#[must_use]
pub fn level_size(theme: &Theme, level: &MenuTree) -> (f32, f32) {
    let rows: f32 = (0..drawn_len(theme, level, MAX_MENU_H))
        .map(|i| entry_h(theme, level, i))
        .sum();
    (MENU_W, 2.0 * theme.spacing.sm + rows.max(theme.row_height))
}

/// Whether a popup of `size` must be resized to show a level that wants
/// `wanted`: the two differ once rounded up to whole pixels, as the
/// positioner rounds them.
#[must_use]
pub fn outgrown(size: (f32, f32), wanted: (f32, f32)) -> bool {
    let px = |v: f32| v.ceil() as i32;
    (px(size.0), px(size.1)) != (px(wanted.0), px(wanted.1))
}

/// The first level hangs below the page row `row` (logical px of the
/// Island's surface) by the theme's small spacing, growing down and right;
/// the compositor slides and flips it to stay on the output.
#[must_use]
pub fn placement(theme: &Theme, row: Rect) -> Placement {
    Placement {
        anchor: row,
        anchor_edge: AnchorEdge::BottomLeft,
        gravity: Gravity::BottomRight,
        offset: (0, theme.spacing.sm as i32),
    }
}

/// A submenu hangs from the right edge of its parent's entry `row`, its
/// first entry level with it (the card's padding lifted); the compositor
/// flips it to the left at the output's edge.
#[must_use]
pub fn submenu_placement(theme: &Theme, row: Rect) -> Placement {
    Placement {
        anchor: row,
        anchor_edge: AnchorEdge::TopRight,
        gravity: Gravity::BottomRight,
        offset: (0, -(theme.spacing.sm as i32)),
    }
}

/// `label` cut to [`MAX_LABEL_GRAPHEMES`] grapheme clusters with an ellipsis.
fn short(label: &str) -> String {
    let kept: Vec<&str> = label
        .graphemes(true)
        .take(MAX_LABEL_GRAPHEMES + 1)
        .collect();
    if kept.len() <= MAX_LABEL_GRAPHEMES {
        return label.to_owned();
    }
    let mut cut = kept[..MAX_LABEL_GRAPHEMES - 1].concat();
    cut.push('\u{2026}');
    cut
}

/// The card for `level`, `height` tall (the popup's height: the rows that
/// do not fit in it are not built, so none is clipped or focused unseen): a
/// Haze card with the medium radius; a separator is a line, an entry a row
/// with its check column, label and, for a submenu, a chevron. A disabled
/// entry is dimmed and takes neither pointer nor keyboard.
#[must_use]
pub fn level_view(theme: &Theme, level: &MenuTree, height: f32) -> View {
    view::col(card_key())
        .haze(theme.radius_md)
        .size(MENU_W, height)
        .padding(theme.spacing.sm)
        .children(
            level
                .entries
                .iter()
                .take(drawn_len(theme, level, height))
                .enumerate()
                .map(|(i, entry)| {
                    if entry.separator {
                        separator(theme, i)
                    } else {
                        entry_row(theme, i, entry)
                    }
                }),
        )
}

fn separator(theme: &Theme, index: usize) -> View {
    centred(view::row(entry_key(index)))
        .height(separator_h(theme))
        .padding_xy(theme.spacing.md, 0.0)
        .child(
            view::row(Key::indexed("tray-menu-line", index as u64))
                .height(1.0)
                .fill(Role::Divider)
                .grow(),
        )
}

fn entry_row(theme: &Theme, index: usize, entry: &super::MenuEntry) -> View {
    let role = if entry.enabled {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let at = index as u64;
    let mark = if entry.toggle == Some(true) {
        view::icon(Key::indexed("tray-menu-check", at), "check", MARK).tint(role)
    } else {
        view::row(Key::indexed("tray-menu-check", at)).size(MARK, MARK)
    };
    let mut row = centred(view::row(entry_key(index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(mark)
        .child(
            view::text(
                Key::indexed("tray-menu-label", at),
                &short(&entry.label),
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .grow(),
        );
    if entry.submenu.is_some() {
        row = row.child(
            view::icon(Key::indexed("tray-menu-more", at), "chevron-right", MARK).tint(role),
        );
    }
    if entry.enabled {
        row = row.interactive().focusable();
    }
    row
}

/// The index of the entry whose row is `key` in `level`.
#[must_use]
pub fn index_of(level: &MenuTree, key: Key) -> Option<usize> {
    (0..level.entries.len()).find(|&i| entry_key(i) == key)
}

/// What a level does with an input event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// Stays open; `changed` when its look did (a frame is due).
    Keep { changed: bool },
    /// Closes this level only (Escape, Left).
    Close,
    /// The entry with this `dbusmenu` id was chosen.
    Choose(i32),
    /// The submenu of entry `index`, whose row is at `row` in this level.
    Open { index: usize, row: Rect },
}

/// One level without its painter: the tree, the pointer and keyboard state
/// and the entries it shows.
#[derive(Debug)]
pub struct LevelCore {
    level: MenuTree,
    theme: Theme,
    tree: Tree,
    input: Interaction,
    /// The height the rows are built for: the popup's, once it has one.
    fit: f32,
    /// The size the runtime was asked for and has not given yet.
    asked: Option<(f32, f32)>,
}

impl LevelCore {
    /// The level `level` in `theme`, under `reduced` motion, built for the
    /// popup size the level wants.
    #[must_use]
    pub fn new(theme: &Theme, level: MenuTree, reduced: bool, now: Instant) -> LevelCore {
        let fit = level_size(theme, &level).1;
        let mut tree = Tree::new(reduced, theme);
        tree.apply(level_view(theme, &level, fit), now);
        LevelCore {
            level,
            theme: theme.clone(),
            tree,
            input: Interaction::new(),
            fit,
            asked: None,
        }
    }

    /// The popup size the level wants: every entry up to [`MAX_MENU_H`].
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        level_size(&self.theme, &self.level)
    }

    /// The size to ask the runtime for when the popup is `size` and the
    /// level wants another (a republished level grew or shrank): once per
    /// wanted size, so a compositor that gives nothing is not asked again
    /// until the level changes.
    pub fn resize_wanted(&mut self, size: (f32, f32)) -> Option<(f32, f32)> {
        let wanted = self.size();
        if !outgrown(size, wanted) {
            self.asked = None;
            return None;
        }
        if self.asked.is_some_and(|asked| !outgrown(asked, wanted)) {
            return None;
        }
        self.asked = Some(wanted);
        Some(wanted)
    }

    /// Builds the rows for a popup `height` tall: the ones past it are not
    /// built until the popup has the room.
    pub fn fit(&mut self, height: f32, now: Instant) {
        if !outgrown((MENU_W, self.fit), (MENU_W, height)) {
            return;
        }
        self.fit = height;
        self.tree
            .apply(level_view(&self.theme, &self.level, height), now);
    }

    /// Focuses the first enabled entry: a menu opened from the keyboard
    /// starts there, so Enter chooses at once.
    pub fn focus_first(&mut self) {
        self.key(XKB_KEY_TAB, false);
    }

    fn key(&mut self, keysym: u32, shift: bool) -> Vec<UiEvent> {
        self.input.key(
            &mut self.tree,
            &InputEvent::Key {
                keysym,
                utf8: None,
                modifiers: celestina_wayland::Modifiers {
                    shift,
                    ..celestina_wayland::Modifiers::default()
                },
                pressed: true,
                serial: 0,
            },
        )
    }

    /// Rebuilds the view when the painter's theme changed.
    fn set_theme(&mut self, theme: &Theme, now: Instant) {
        if *theme != self.theme {
            self.theme = theme.clone();
            self.tree.set_theme(theme);
            self.tree
                .apply(level_view(theme, &self.level, self.fit), now);
        }
    }

    /// Shows `level` instead: the rows are rebuilt in the room the popup
    /// has (the level asks for more or less through [`LevelCore::resize_wanted`])
    /// and the keyboard's focus stays on its row when that row is still
    /// there.
    pub fn replace_level(&mut self, level: MenuTree, now: Instant) {
        if level == self.level {
            return;
        }
        self.level = level;
        self.tree
            .apply(level_view(&self.theme, &self.level, self.fit), now);
    }

    /// The level shown.
    #[must_use]
    pub fn level(&self) -> &MenuTree {
        &self.level
    }

    /// The row of the entry at `index`, once laid out.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        self.tree
            .node_by_key(entry_key(index))
            .map(|id| self.tree.rect(id))
    }

    /// The focused entry, if the keyboard has one.
    fn focused_index(&self) -> Option<usize> {
        let id = self.tree.highlight().focused?;
        index_of(&self.level, self.tree.key_of(id))
    }

    /// What activating the entry at `index` means.
    fn activate(&self, index: usize) -> Outcome {
        let Some(entry) = self.level.entries.get(index) else {
            return Outcome::Keep { changed: false };
        };
        if !entry.enabled || entry.separator {
            return Outcome::Keep { changed: false };
        }
        if entry.submenu.is_some() {
            let row = self
                .tree
                .node_by_key(entry_key(index))
                .map_or_else(|| Rect::whole((0.0, 0.0)), |id| self.tree.rect(id));
            return Outcome::Open { index, row };
        }
        Outcome::Choose(entry.id)
    }

    /// Handles one event: Up and Down are Shift+Tab and Tab through the
    /// entries; Escape and Left close; Right opens a submenu; a click or
    /// Enter on an entry chooses or opens it.
    pub fn input(&mut self, ev: &InputEvent, now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key {
                keysym: XKB_KEY_LEFT,
                pressed: true,
                ..
            } => return Outcome::Close,
            InputEvent::Key {
                keysym: XKB_KEY_RIGHT,
                pressed: true,
                ..
            } => {
                return match self.focused_index() {
                    Some(index) if self.level.entries[index].submenu.is_some() => {
                        self.activate(index)
                    }
                    _ => Outcome::Keep { changed: false },
                };
            }
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
                pressed,
                modifiers,
                utf8,
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
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        for event in &events {
            match event {
                UiEvent::Escape => return Outcome::Close,
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(index) = index_of(&self.level, *key) {
                        let outcome = self.activate(index);
                        if outcome != (Outcome::Keep { changed: false }) {
                            return outcome;
                        }
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

/// What a level does when its child popup is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildClosed {
    /// The chain goes on: this level stays open.
    Stay,
    /// An entry was chosen below or the compositor dismissed the chain:
    /// this level closes too.
    CloseChain,
}

/// The pure rule of a chain: a level stays open when its child closes by
/// the child's own Escape or Left (or because it was replaced), and closes
/// when an entry was `chosen` or the compositor `Dismissed` the child.
#[must_use]
pub fn child_closed(chosen: bool, reason: CloseReason) -> ChildClosed {
    if chosen || reason == CloseReason::Dismissed {
        ChildClosed::CloseChain
    } else {
        ChildClosed::Stay
    }
}

/// One level as a popup shell: the subtree of the shared model at `path`.
pub struct TrayMenu {
    core: LevelCore,
    cx: MenuContext,
    /// The entry ids from the root to this level.
    path: Vec<i32>,
    /// The model generation the rows were built from.
    seen: u64,
    /// The submenu popup open from this level.
    child: Option<PopupId>,
    /// The entry whose submenu was asked for and arrived empty: it opens
    /// once the model carries its entries, from the `keyboard` or not.
    awaiting: Option<(i32, bool)>,
}

impl std::fmt::Debug for TrayMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayMenu")
            .field("core", &self.core)
            .field("path", &self.path)
            .field("child", &self.child)
            .field("awaiting", &self.awaiting)
            .finish_non_exhaustive()
    }
}

impl TrayMenu {
    /// The level at `path` of the chain `cx` describes, in `theme`. Opened
    /// from the `keyboard`, its first enabled entry starts focused.
    #[must_use]
    pub fn new(cx: &MenuContext, theme: &Theme, path: Vec<i32>, keyboard: bool) -> TrayMenu {
        let (level, seen) = {
            let model = cx.shared.borrow();
            (
                level_at(&model.tree, &path).cloned().unwrap_or_default(),
                model.generation,
            )
        };
        let mut core = LevelCore::new(theme, level, cx.reduced, Instant::now());
        if keyboard {
            core.focus_first();
        }
        TrayMenu {
            core,
            cx: cx.clone(),
            path,
            seen,
            child: None,
            awaiting: None,
        }
    }

    /// The level's popup size for the model as it is.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        self.core.size()
    }

    /// Asks the host for the entry's `AboutToShow`, then opens its submenu
    /// beside `row`, or waits for the republished model when the submenu is
    /// still empty.
    fn open_child(&mut self, index: usize, row: Rect, keyboard: bool, ctl: &mut SurfaceCtl<'_>) {
        let Some(entry) = self.core.level.entries.get(index) else {
            return;
        };
        let Some(sub) = entry.submenu.clone() else {
            return;
        };
        let id = entry.id;
        (self.cx.ask)(TrayCommand::AboutToShow(self.cx.item.clone(), id));
        if sub.entries.is_empty() {
            self.awaiting = Some((id, keyboard));
            return;
        }
        self.awaiting = None;
        let theme = self.core.theme.clone();
        let size = level_size(&theme, &sub);
        let mut path = self.path.clone();
        path.push(id);
        let child = TrayMenu::new(&self.cx, &theme, path, keyboard);
        let popup = ctl.open_popup(submenu_placement(&theme, row), size, Box::new(child));
        self.child = Some(popup);
        self.cx.chain.borrow_mut().push(popup);
    }

    /// Takes the model's changes: the rows follow the subtree at the path.
    fn follow_model(&mut self, now: Instant) {
        let model = self.cx.shared.borrow();
        if model.generation == self.seen {
            return;
        }
        self.seen = model.generation;
        let level = level_at(&model.tree, &self.path)
            .cloned()
            .unwrap_or_default();
        drop(model);
        self.core.replace_level(level, now);
    }

    /// Opens the submenu that was awaited, once it has entries.
    fn open_awaited(&mut self, ctl: &mut SurfaceCtl<'_>) {
        let Some((id, keyboard)) = self.awaiting else {
            return;
        };
        let Some(index) = self.core.level.entries.iter().position(|e| e.id == id) else {
            self.awaiting = None;
            return;
        };
        let filled = self.core.level.entries[index]
            .submenu
            .as_ref()
            .is_some_and(|sub| !sub.entries.is_empty());
        if !filled {
            return;
        }
        let Some(row) = self.core.row_rect(index) else {
            return;
        };
        self.awaiting = None;
        self.open_child_without_asking(index, row, keyboard, ctl);
    }

    /// Opens the submenu of entry `index` whose `AboutToShow` was already
    /// asked.
    fn open_child_without_asking(
        &mut self,
        index: usize,
        row: Rect,
        keyboard: bool,
        ctl: &mut SurfaceCtl<'_>,
    ) {
        let Some(entry) = self.core.level.entries.get(index) else {
            return;
        };
        let Some(sub) = entry.submenu.as_ref() else {
            return;
        };
        let theme = self.core.theme.clone();
        let size = level_size(&theme, sub);
        let mut path = self.path.clone();
        path.push(entry.id);
        let child = TrayMenu::new(&self.cx, &theme, path, keyboard);
        let popup = ctl.open_popup(submenu_placement(&theme, row), size, Box::new(child));
        self.child = Some(popup);
        self.cx.chain.borrow_mut().push(popup);
    }
}

impl PopupShell for TrayMenu {
    fn frame(&mut self, cx: &mut FrameCx<'_>) {
        let now = cx.now;
        self.follow_model(now);
        let painter = Rc::clone(&self.cx.painter);
        let Ok(mut painter) = painter.try_borrow_mut() else {
            tracing::warn!("tray menu: the painter is in use; frame skipped");
            cx.ctl.request_redraw();
            return;
        };
        self.core.set_theme(painter.theme(), now);
        // A republished level that outgrew its popup (or shrank) asks for
        // the room; until the configure brings it, the rows are built for
        // the size the popup has.
        if let Some(size) = self.core.resize_wanted(cx.size) {
            tracing::debug!(?size, have = ?cx.size, "tray menu: the level asks for its size");
            cx.ctl.resize_popup(size);
        }
        self.core.fit(cx.size.1, now);
        let radius = painter.theme().radius_md;
        let dpr = cx.scale as f32;
        let tree = &mut self.core.tree;
        tree.layout(cx.size, painter.text(), dpr);
        if let Some(fault) = tree.take_fault() {
            tracing::warn!(%fault, "tray menu: layout fault");
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
        drop(painter);
        self.open_awaited(&mut cx.ctl);
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) -> PopupAction {
        if let InputEvent::PopupClosed { id, reason } = ev {
            if self.child != Some(id) {
                return PopupAction::Keep;
            }
            self.child = None;
            self.cx.chain.borrow_mut().retain(|p| *p != id);
            let chosen = self.cx.choice.borrow().is_some();
            return match child_closed(chosen, reason) {
                ChildClosed::CloseChain => PopupAction::Close,
                ChildClosed::Stay => {
                    ctl.request_redraw();
                    PopupAction::Keep
                }
            };
        }
        let keyboard = matches!(ev, InputEvent::Key { .. });
        match self.core.input(&ev, Instant::now()) {
            Outcome::Close => PopupAction::Close,
            Outcome::Choose(entry) => {
                tracing::info!(entry, "tray menu: entry chosen");
                *self.cx.choice.borrow_mut() = Some((self.cx.item.clone(), entry));
                PopupAction::Close
            }
            Outcome::Open { index, row } => {
                self.open_child(index, row, keyboard, ctl);
                ctl.request_redraw();
                PopupAction::Keep
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
        tracing::info!("tray menu: dismissed by the compositor");
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_ui::ViewKind;
    use celestina_wayland::Modifiers;

    use super::*;
    use crate::tray::MenuEntry;

    const RETURN: u32 = 0xff0d;
    const ESCAPE: u32 = 0xff1b;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn entry(id: i32, label: &str) -> MenuEntry {
        MenuEntry {
            id,
            label: label.to_owned(),
            enabled: true,
            ..MenuEntry::default()
        }
    }

    fn separator_entry() -> MenuEntry {
        MenuEntry {
            separator: true,
            enabled: true,
            ..MenuEntry::default()
        }
    }

    /// Open, a separator, a checked toggle, an unchecked toggle, a disabled
    /// entry and a submenu with one entry.
    fn level() -> MenuTree {
        MenuTree {
            id: 0,
            entries: vec![
                entry(1, "Open"),
                separator_entry(),
                MenuEntry {
                    toggle: Some(true),
                    ..entry(2, "Mute")
                },
                MenuEntry {
                    toggle: Some(false),
                    ..entry(3, "Pause")
                },
                MenuEntry {
                    enabled: false,
                    ..entry(4, "Offline")
                },
                MenuEntry {
                    submenu: Some(MenuTree {
                        id: 5,
                        entries: vec![entry(51, "Inner")],
                    }),
                    ..entry(5, "More")
                },
            ],
        }
    }

    fn core_of(level: MenuTree) -> LevelCore {
        let theme = Theme::dark();
        let mut core = LevelCore::new(&theme, level, false, Instant::now());
        let size = core.size();
        core.tree.layout_measured(size, fake_measure);
        core
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

    fn press(core: &mut LevelCore, keysym: u32) -> Outcome {
        core.input(&key(keysym), Instant::now())
    }

    fn focused(core: &LevelCore) -> Option<Key> {
        core.tree.highlight().focused.map(|id| core.tree.key_of(id))
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn menu_rows_map_dbusmenu_entries_with_separators_toggles_and_disabled_state() {
        let theme = Theme::dark();
        let level = level();
        let v = level_view(&theme, &level, level_size(&theme, &level).1);
        assert!(
            matches!(v.kind, ViewKind::Box { haze: true, radius, .. } if radius == theme.radius_md),
            "a Haze card with the medium radius"
        );
        let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        assert_eq!(keys, (0..6).map(entry_key).collect::<Vec<_>>());
        let row = |i: usize| &v.children[i];
        // A plain entry is a control.
        assert!(row(0).interactive && row(0).focusable);
        // A separator is a line, not a control, and thinner than a row.
        assert!(!row(1).interactive && !row(1).focusable);
        assert_eq!(
            row(1).style.size.height,
            taffy::Dimension::length(separator_h(&theme))
        );
        assert!(find(row(1), Key::indexed("tray-menu-line", 1)).is_some());
        // Only a checked toggle shows the check mark.
        let check = |i: u64| find(&v, Key::indexed("tray-menu-check", i));
        assert!(matches!(
            check(2).map(|c| &c.kind),
            Some(ViewKind::Icon { .. })
        ));
        assert!(matches!(
            check(3).map(|c| &c.kind),
            Some(ViewKind::Box { .. })
        ));
        assert!(matches!(
            check(0).map(|c| &c.kind),
            Some(ViewKind::Box { .. })
        ));
        // A disabled entry is dimmed and takes neither pointer nor keyboard.
        assert!(!row(4).interactive && !row(4).focusable);
        let role_of = |i: u64| match find(&v, Key::indexed("tray-menu-label", i)).map(|c| &c.kind) {
            Some(ViewKind::Text { style, .. }) => Some(style.role),
            _ => None,
        };
        assert_eq!(role_of(0), Some(Role::TextHi));
        assert_eq!(role_of(4), Some(Role::TextFaint));
        // A submenu entry has the chevron, the others do not.
        assert!(find(&v, Key::indexed("tray-menu-more", 5)).is_some());
        assert!(find(&v, Key::indexed("tray-menu-more", 0)).is_none());
        assert!(row(5).interactive && row(5).focusable);
    }

    #[test]
    fn long_labels_are_cut_and_tall_menus_stop_at_the_height_limit() {
        assert_eq!(short("Open"), "Open");
        let long = "x".repeat(100);
        let cut = short(&long);
        assert_eq!(cut.graphemes(true).count(), MAX_LABEL_GRAPHEMES);
        assert!(cut.ends_with('\u{2026}'));

        let theme = Theme::dark();
        let tall = MenuTree {
            id: 0,
            entries: (0..64).map(|i| entry(i, "row")).collect(),
        };
        let shown = drawn_len(&theme, &tall, MAX_MENU_H);
        assert!(shown > 0 && shown < 64);
        let (w, h) = level_size(&theme, &tall);
        assert_eq!(w, MENU_W);
        assert!(h <= MAX_MENU_H);
        assert_eq!(level_view(&theme, &tall, h).children.len(), shown);
        assert_eq!(
            drawn_len(&theme, &tall, 2.0 * MAX_MENU_H),
            shown,
            "no popup taller than the limit"
        );
        // A short menu is as tall as its rows plus the padding.
        let (_, h) = level_size(&theme, &level());
        assert_eq!(
            h,
            2.0 * theme.spacing.sm + 5.0 * theme.row_height + separator_h(&theme)
        );
    }

    #[test]
    fn placements_hang_the_first_level_below_and_a_submenu_to_the_right() {
        let theme = Theme::dark();
        let row = Rect {
            x: 10.0,
            y: 100.0,
            w: 300.0,
            h: 40.0,
        };
        let p = placement(&theme, row);
        assert_eq!(p.anchor, row);
        assert_eq!(
            (p.anchor_edge, p.gravity),
            (AnchorEdge::BottomLeft, Gravity::BottomRight)
        );
        assert_eq!(p.offset, (0, theme.spacing.sm as i32));
        let s = submenu_placement(&theme, row);
        assert_eq!(
            (s.anchor_edge, s.gravity),
            (AnchorEdge::TopRight, Gravity::BottomRight),
            "to the right of the row; the compositor flips it left at the edge"
        );
        assert_eq!(s.offset, (0, -(theme.spacing.sm as i32)));
    }

    #[test]
    fn arrows_skip_disabled_entries_and_separators_and_enter_chooses() {
        let mut core = core_of(level());
        assert_eq!(focused(&core), None);
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(focused(&core), Some(entry_key(0)));
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(
            focused(&core),
            Some(entry_key(2)),
            "the separator is skipped"
        );
        press(&mut core, XKB_KEY_DOWN);
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(
            focused(&core),
            Some(entry_key(5)),
            "the disabled entry is skipped"
        );
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(focused(&core), Some(entry_key(0)), "wraps to the top");
        press(&mut core, XKB_KEY_UP);
        assert_eq!(focused(&core), Some(entry_key(5)));
        press(&mut core, XKB_KEY_UP);
        press(&mut core, XKB_KEY_UP);
        press(&mut core, XKB_KEY_UP);
        assert_eq!(focused(&core), Some(entry_key(0)));
        assert_eq!(press(&mut core, RETURN), Outcome::Choose(1));
    }

    #[test]
    fn a_level_opened_from_the_keyboard_focuses_its_first_enabled_entry() {
        let mut core = core_of(level());
        core.focus_first();
        assert_eq!(focused(&core), Some(entry_key(0)));
    }

    #[test]
    fn escape_and_left_close_this_level_only() {
        let mut core = core_of(level());
        assert_eq!(press(&mut core, ESCAPE), Outcome::Close);
        assert_eq!(press(&mut core, XKB_KEY_LEFT), Outcome::Close);
    }

    #[test]
    fn right_and_enter_on_a_submenu_entry_open_it_beside_its_row() {
        let mut core = core_of(level());
        // Right on nothing, or on an entry without a submenu, does nothing.
        assert_eq!(
            press(&mut core, XKB_KEY_RIGHT),
            Outcome::Keep { changed: false }
        );
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(
            press(&mut core, XKB_KEY_RIGHT),
            Outcome::Keep { changed: false }
        );
        // Up from the first wraps to the last: the submenu entry.
        press(&mut core, XKB_KEY_UP);
        assert_eq!(focused(&core), Some(entry_key(5)));
        let want = core
            .tree
            .node_by_key(entry_key(5))
            .map(|id| core.tree.rect(id));
        let opened = press(&mut core, XKB_KEY_RIGHT);
        assert!(
            matches!(opened, Outcome::Open { index: 5, row } if Some(row) == want),
            "{opened:?}"
        );
        assert!(want.is_some_and(|r| r.w > 0.0 && r.h > 0.0));
        assert_eq!(
            press(&mut core, RETURN),
            opened,
            "Enter opens it as Right does"
        );
    }

    fn click(core: &mut LevelCore, index: usize) -> Outcome {
        let r = match core.tree.node_by_key(entry_key(index)) {
            Some(id) => core.tree.rect(id),
            None => panic!("no row {index}"),
        };
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
        core.input(&down, now);
        core.input(&up, now)
    }

    #[test]
    fn a_click_chooses_an_entry_opens_a_submenu_and_ignores_the_rest() {
        let mut core = core_of(level());
        assert_eq!(click(&mut core, 0), Outcome::Choose(1));
        assert_eq!(
            click(&mut core, 2),
            Outcome::Choose(2),
            "a toggle is chosen like any entry"
        );
        assert!(matches!(
            click(&mut core, 5),
            Outcome::Open { index: 5, .. }
        ));
        assert!(
            matches!(click(&mut core, 4), Outcome::Keep { .. }),
            "disabled"
        );
        assert!(
            matches!(click(&mut core, 1), Outcome::Keep { .. }),
            "separator"
        );
    }

    #[test]
    fn a_chain_closes_whole_on_a_choice_or_a_dismissal_but_not_on_escape() {
        assert_eq!(
            child_closed(true, CloseReason::Shell),
            ChildClosed::CloseChain
        );
        assert_eq!(
            child_closed(false, CloseReason::Dismissed),
            ChildClosed::CloseChain
        );
        // Escape or Left in the child: the parent stays.
        assert_eq!(child_closed(false, CloseReason::Shell), ChildClosed::Stay);
        assert_eq!(
            child_closed(false, CloseReason::Replaced),
            ChildClosed::Stay
        );
        assert_eq!(child_closed(false, CloseReason::Parent), ChildClosed::Stay);
    }

    #[test]
    fn a_path_of_entry_ids_finds_the_level_and_a_republished_model_rebuilds_the_rows() {
        let tree = level();
        assert_eq!(level_at(&tree, &[]), Some(&tree));
        let inner = level_at(&tree, &[5]).expect("the submenu");
        assert_eq!(inner.id, 5);
        assert_eq!(level_at(&tree, &[5, 51]), None, "a leaf has no level");
        assert_eq!(level_at(&tree, &[9]), None);

        let mut core = core_of(level());
        press(&mut core, XKB_KEY_DOWN);
        assert_eq!(focused(&core), Some(entry_key(0)));
        // The same level again changes nothing; a new one rebuilds the
        // rows and keeps the focus on the row that is still there.
        core.replace_level(level(), Instant::now());
        assert_eq!(core.level().entries.len(), 6);
        let shorter = MenuTree {
            id: 0,
            entries: vec![entry(1, "Open"), entry(7, "New")],
        };
        core.replace_level(shorter, Instant::now());
        assert_eq!(core.level().entries.len(), 2);
        assert_eq!(focused(&core), Some(entry_key(0)));
        assert_eq!(
            press(&mut core, XKB_KEY_DOWN),
            Outcome::Keep { changed: true }
        );
        assert_eq!(focused(&core), Some(entry_key(1)));
        assert_eq!(press(&mut core, RETURN), Outcome::Choose(7));
    }

    #[test]
    fn rows_past_the_popups_height_are_not_built_and_the_level_asks_for_room() {
        let theme = Theme::dark();
        let tall = MenuTree {
            id: 0,
            entries: (0..8).map(|i| entry(i, "row")).collect(),
        };
        let wanted = level_size(&theme, &tall);
        let mut core = core_of(level());
        let small = core.size();
        assert_eq!(core.resize_wanted(small), None, "the popup fits");
        // The level is republished taller while its popup keeps its size:
        // the level asks for the room once, and meanwhile builds only the
        // rows that fit, so none is clipped and the keyboard reaches none
        // outside the card.
        core.replace_level(tall.clone(), Instant::now());
        assert_eq!(core.size(), wanted);
        assert_eq!(core.resize_wanted(small), Some(wanted));
        assert_eq!(core.resize_wanted(small), None, "asked once");
        core.fit(small.1, Instant::now());
        core.tree.layout_measured(small, fake_measure);
        // The rows the keyboard walks before wrapping to the first: the
        // rows built (a row a reconciliation left fading out takes no
        // focus).
        let walked = |core: &mut LevelCore| {
            let mut count = 0;
            loop {
                press(core, XKB_KEY_DOWN);
                count += 1;
                if focused(core) == Some(entry_key(0)) {
                    return count;
                }
                assert!(count <= 8, "no wrap");
            }
        };
        let shown = drawn_len(&theme, &tall, small.1);
        assert!(shown > 0 && shown < 8, "{shown}");
        assert_eq!(
            walked(&mut core),
            1,
            "the first press focuses the first row"
        );
        assert_eq!(walked(&mut core), shown, "Down wraps within the rows built");
        // The compositor gave the popup the size asked for: every row is
        // built, and nothing more is asked.
        core.fit(wanted.1, Instant::now());
        core.tree.layout_measured(wanted, fake_measure);
        assert_eq!(walked(&mut core), 8);
        assert_eq!(core.resize_wanted(wanted), None);
        // Republished shorter again: the smaller size is asked for.
        core.replace_level(level(), Instant::now());
        assert_eq!(core.resize_wanted(wanted), Some(small));
        // A fraction of a pixel is no reason to ask: the popup rounds up.
        assert!(!outgrown((300.0, 136.0), (300.0, 135.5)));
        assert!(outgrown((300.0, 136.0), (300.0, 136.5)));
    }

    #[test]
    fn index_of_finds_only_this_levels_rows() {
        let level = level();
        assert_eq!(index_of(&level, entry_key(3)), Some(3));
        assert_eq!(index_of(&level, entry_key(6)), None);
        assert_eq!(index_of(&level, Key::named("other")), None);
    }
}
