//! The tray menu as a pane (CEL-D-016, design §5.6): one app's DBusMenu as a
//! column of rows with drill-in submenus, drawn in a body's own coordinates.
//! The bar's menu window, the hidden-apps card and the Tray page's card
//! share it; the holder (the glass, the drop, the card's fold) is theirs.
//!
//! The pane shows one level at a time, the subtree of its [`MenuTree`] at a
//! path of entry ids. Below the root, the first row is a back row with the
//! parent entry's label. A level taller than the room scrolls itself: the
//! wheel moves its offset, a focus move reveals the focused row and a thin
//! bar shows for a moment after the offset changes. Going into a submenu or
//! back slides the two levels sideways; with reduced motion they swap.
//!
//! The pane does not act. [`MenuOutcome`] says what the person did (choose an
//! entry, ask the item about a submenu, go back, close) and the holder sends
//! it. A republished tree replaces the pane's: it falls back to the deepest
//! level the new tree still has, and an awaited submenu opens once the item
//! has filled it. Rows, heights and the focus are all derived from the tree
//! and the frame, so the offset and the reveal never depend on a layout.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Role, Theme, Weight};
use celestina_ui::{
    fit_middle, view, Interaction, Key, Motion, Scroll, Tree, UiEvent, View, ONE_UI,
};
use celestina_wayland::{InputEvent, Modifiers};

use super::{
    menu::{
        entry_key, entry_row_labelled, index_of, level_at, row_at, row_count, rows, separator,
        separator_h, shell_row, short, MenuRow, ShellEntry, MARK, MENU_W, XKB_KEY_DOWN,
        XKB_KEY_LEFT, XKB_KEY_RIGHT, XKB_KEY_TAB, XKB_KEY_UP,
    },
    MenuTree,
};
use crate::island::view::{centred, text_style};
use crate::scrolling::{self, THUMB_INSET, THUMB_LINGER, THUMB_RADIUS, THUMB_W};
use crate::sheet::{translated, Measure};

/// The pane's width and the padding between it and its rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaneFrame {
    /// The pane's width.
    pub width: f32,
    /// The padding left and right of the rows.
    pub pad_x: f32,
    /// The padding over the first row and under the last.
    pub pad_y: f32,
}

impl PaneFrame {
    /// A bar menu window: 300 wide, 8 px of padding all round.
    pub(crate) const WINDOW: PaneFrame = PaneFrame {
        width: MENU_W,
        pad_x: 8.0,
        pad_y: 8.0,
    };

    /// A card `width` wide: its rows touch its sides and it has 4 px under
    /// and over them.
    pub(crate) fn card(width: f32) -> PaneFrame {
        PaneFrame {
            width,
            pad_x: 0.0,
            pad_y: 4.0,
        }
    }

    /// The frame with every length finite and not negative.
    fn sane(self) -> PaneFrame {
        PaneFrame {
            width: length(self.width),
            pad_x: length(self.pad_x),
            pad_y: length(self.pad_y),
        }
    }
}

/// What the pane did with an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuOutcome {
    /// Stays as it is; `changed` when its look did (a frame is due).
    Keep { changed: bool },
    /// The entry with this `dbusmenu` id was chosen.
    Choose(i32),
    /// The shell's own entry was chosen.
    Shell(ShellEntry),
    /// The entry with this id has a submenu: the holder asks the item's
    /// `AboutToShow` for it. The pane is in the submenu already unless the
    /// item has not filled it yet.
    Submenu(i32),
    /// The pane went up one level.
    Back,
    /// Escape at the root: the holder closes (a window) or folds (a card).
    Close,
}

/// The pane's root node.
fn pane_key() -> Key {
    Key::named("tray-pane")
}

/// The back row of a sublevel.
pub(crate) fn back_key() -> Key {
    Key::named("tray-pane-back")
}

/// The back row's text.
fn back_label_key() -> Key {
    Key::named("tray-pane-back-label")
}

/// A finite, non-negative length: NaN, infinities and negatives count as 0.
fn length(x: f32) -> f32 {
    if x.is_finite() {
        x.max(0.0)
    } else {
        0.0
    }
}

/// The height of `row`.
fn row_h(theme: &Theme, row: MenuRow<'_>) -> f32 {
    match row {
        MenuRow::Separator => separator_h(theme),
        MenuRow::Entry(_) | MenuRow::Shell(_) => theme.row_height,
    }
}

/// The height of the level `menu`: the padding over and under the rows, the
/// back row when there is one, and every row.
fn content_height(
    theme: &Theme,
    frame: PaneFrame,
    menu: &MenuTree,
    shell: Option<ShellEntry>,
    back: bool,
) -> f32 {
    let back = if back { theme.row_height } else { 0.0 };
    let rows: f32 = rows(menu, shell).map(|row| row_h(theme, row)).sum();
    2.0 * frame.pad_y + back + rows
}

/// The back row: `chevron-left` in the check column and the parent entry's
/// label.
fn back_row(theme: &Theme, label: &str) -> View {
    centred(view::row(back_key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(
            view::icon(Key::named("tray-pane-back-mark"), "chevron-left", MARK).tint(Role::TextHi),
        )
        .child(
            view::text(
                back_label_key(),
                label,
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
            )
            .grow(),
        )
        .interactive()
        .focusable()
}

/// Where a label sits, which decides the room it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Spot {
    /// An entry with no submenu.
    Plain,
    /// An entry with a chevron after its label.
    Chevron,
    /// The back row.
    Back,
}

/// Labels cut to their room by measure (a card is narrower than a label may
/// be): keyed by the label as [`short`] leaves it and its place.
#[derive(Clone, Debug, Default)]
struct Cuts(HashMap<(String, Spot), String>);

impl Cuts {
    /// `label` (already [`short`]) as the row at `spot` shows it.
    fn of(&self, label: &str, spot: Spot) -> String {
        self.0
            .get(&(label.to_owned(), spot))
            .cloned()
            .unwrap_or_else(|| label.to_owned())
    }
}

/// The view of the level `menu`: its rows (the shell's entry after a line,
/// when `shell` is `Some`), under the back row when `back` has a label. Its
/// height is [`content_height`]; the holder's glass is behind it.
#[cfg(test)]
fn build_view(
    theme: &Theme,
    frame: PaneFrame,
    menu: &MenuTree,
    shell: Option<ShellEntry>,
    back: Option<&str>,
) -> View {
    build_view_cut(theme, frame, menu, shell, back, &Cuts::default())
}

/// [`build_view`] with the labels `cuts` holds.
fn build_view_cut(
    theme: &Theme,
    frame: PaneFrame,
    menu: &MenuTree,
    shell: Option<ShellEntry>,
    back: Option<&str>,
    cuts: &Cuts,
) -> View {
    view::col(pane_key())
        .size(
            frame.width,
            content_height(theme, frame, menu, shell, back.is_some()),
        )
        .padding_xy(frame.pad_x, frame.pad_y)
        .children(back.map(|label| back_row(theme, &cuts.of(label, Spot::Back))))
        .children(rows(menu, shell).enumerate().map(|(i, row)| match row {
            MenuRow::Separator => separator(theme, i),
            MenuRow::Entry(entry) => {
                let spot = if entry.submenu.is_some() {
                    Spot::Chevron
                } else {
                    Spot::Plain
                };
                let label = cuts.of(&short(&entry.label), spot);
                entry_row_labelled(theme, i, entry, &label)
            }
            MenuRow::Shell(entry) => shell_row(theme, i, entry),
        }))
}

/// The shell's entry of the level at `path`: the root's alone.
fn shell_at(shell: Option<ShellEntry>, path: &[i32]) -> Option<ShellEntry> {
    if path.is_empty() {
        shell
    } else {
        None
    }
}

/// The parent entry's label for the level at `path`, cut; `None` at the root
/// and for a path the tree has lost.
fn back_label_at(root: &MenuTree, path: &[i32]) -> Option<String> {
    let (id, parent) = path.split_last()?;
    let entry = level_at(root, parent)?
        .entries
        .iter()
        .find(|e| e.id == *id)?;
    Some(short(&entry.label))
}

/// The view, height and back label of the level at `path` of `root`.
fn view_at(
    theme: &Theme,
    frame: PaneFrame,
    root: &MenuTree,
    shell: Option<ShellEntry>,
    path: &[i32],
    cuts: &Cuts,
) -> (View, f32, Option<String>) {
    let empty = MenuTree::default();
    let menu = level_at(root, path).unwrap_or(&empty);
    let label = back_label_at(root, path);
    let shell = shell_at(shell, path);
    let height = content_height(theme, frame, menu, shell, label.is_some());
    let view = build_view_cut(theme, frame, menu, shell, label.as_deref(), cuts);
    (view, height, label)
}

/// A level for `path` of `root` with a tree of its own.
fn new_level(
    theme: &Theme,
    frame: PaneFrame,
    root: &MenuTree,
    shell: Option<ShellEntry>,
    reduced: bool,
    path: Vec<i32>,
    cuts: &Cuts,
    now: Instant,
) -> Level {
    let (view, height, back_label) = view_at(theme, frame, root, shell, &path, cuts);
    let mut tree = Tree::new(reduced, theme);
    tree.apply(view, now);
    Level {
        path,
        tree,
        input: Interaction::new(),
        back_label,
        height,
    }
}

/// The key press an Up or Down arrow stands for: Shift+Tab and Tab, so the
/// focus walks the enabled rows. A repeat is a press: a held arrow keeps
/// moving the focus. `None` for any other event.
pub(crate) fn arrows_as_tab(ev: &InputEvent) -> Option<InputEvent> {
    let (keysym, utf8, modifiers, serial, pressed) = match ev {
        InputEvent::Key {
            keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
            utf8,
            modifiers,
            pressed,
            serial,
        } => (*keysym, utf8, modifiers, *serial, *pressed),
        InputEvent::KeyRepeat {
            keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
            utf8,
            modifiers,
            serial,
        } => (*keysym, utf8, modifiers, *serial, true),
        _ => return None,
    };
    Some(InputEvent::Key {
        keysym: XKB_KEY_TAB,
        utf8: utf8.clone(),
        modifiers: Modifiers {
            shift: keysym == XKB_KEY_UP,
            ..*modifiers
        },
        pressed,
        serial,
    })
}

/// One level on the stack: its path of entry ids and the tree that shows it.
#[derive(Debug)]
struct Level {
    /// The entry ids from the root to this level.
    path: Vec<i32>,
    tree: Tree,
    input: Interaction,
    /// The parent entry's label, cut: the back row's text. `None` at the root.
    back_label: Option<String>,
    /// The level's content height, as built.
    height: f32,
}

/// Which way a slide goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    /// Into a submenu: the level it left is the one under the new one.
    In,
    /// Back to the parent: the level it left is `Slide::gone`.
    Out,
}

/// Two levels moving sideways.
#[derive(Debug)]
struct Slide {
    dir: Dir,
    start: Instant,
    /// The scroll offset the level that is leaving had.
    from_offset: f32,
    /// The level a back slide is leaving, out of the stack already.
    gone: Option<Level>,
}

/// A submenu the item was asked about and has not filled: it opens once a
/// republished tree carries its entries.
#[derive(Clone, Debug)]
struct Awaiting {
    /// The path of the submenu, its entry's id last.
    path: Vec<i32>,
    /// Whether the keyboard asked: the first entry then starts focused.
    keyboard: bool,
}

/// The row the keyboard has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Back,
    Row(usize),
}

/// A tray item's menu as a scrolling pane with drill-in submenus.
#[derive(Debug)]
pub(crate) struct MenuPane {
    theme: Theme,
    root: MenuTree,
    shell: Option<ShellEntry>,
    frame: PaneFrame,
    /// The tallest the pane gets: its room.
    max_h: f32,
    reduced: bool,
    /// The root level, shown while no submenu is open.
    base: Level,
    /// The submenus open below it, in order; the last is shown.
    subs: Vec<Level>,
    slide: Option<Slide>,
    awaiting: Option<Awaiting>,
    scroll: Scroll,
    /// The scroll bar shows until then.
    thumb_until: Option<Instant>,
    /// Labels cut by measure ([`MenuPane::fit_labels`]).
    cuts: Cuts,
}

impl MenuPane {
    /// The pane for `tree` in `theme`, `frame` wide and at most `max_h`
    /// tall, with the shell's entry at the end of its root when `shell` is
    /// `Some`. Under `reduced` motion a level change does not slide.
    #[must_use]
    pub(crate) fn new(
        theme: &Theme,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        frame: PaneFrame,
        max_h: f32,
        reduced: bool,
        now: Instant,
    ) -> MenuPane {
        let frame = frame.sane();
        let base = new_level(
            theme,
            frame,
            &tree,
            shell,
            reduced,
            Vec::new(),
            &Cuts::default(),
            now,
        );
        let mut pane = MenuPane {
            theme: theme.clone(),
            root: tree,
            shell,
            frame,
            max_h: length(max_h),
            reduced,
            base,
            subs: Vec::new(),
            slide: None,
            awaiting: None,
            scroll: Scroll::default(),
            thumb_until: None,
            cuts: Cuts::default(),
        };
        pane.sync_extent();
        pane
    }

    /// A level for `path` with a tree of its own.
    fn make_level(&self, path: Vec<i32>, now: Instant) -> Level {
        new_level(
            &self.theme,
            self.frame,
            &self.root,
            self.shell,
            self.reduced,
            path,
            &self.cuts,
            now,
        )
    }

    /// How many levels are open, the root's included.
    fn count(&self) -> usize {
        1 + self.subs.len()
    }

    /// The level at `depth`: 0 is the root.
    fn level_at_depth(&mut self, depth: usize) -> Option<&mut Level> {
        match depth.checked_sub(1) {
            None => Some(&mut self.base),
            Some(i) => self.subs.get_mut(i),
        }
    }

    /// Shows the tree the level at `depth` builds now, keeping what its
    /// pointer and keyboard hold.
    fn rebuild(&mut self, depth: usize, now: Instant) {
        let Some(path) = self.level_at_depth(depth).map(|l| l.path.clone()) else {
            return;
        };
        let (view, height, label) = view_at(
            &self.theme,
            self.frame,
            &self.root,
            self.shell,
            &path,
            &self.cuts,
        );
        if let Some(level) = self.level_at_depth(depth) {
            level.tree.apply(view, now);
            level.height = height;
            level.back_label = label;
        }
    }

    /// The level shown.
    fn top(&self) -> &Level {
        self.subs.last().unwrap_or(&self.base)
    }

    fn top_mut(&mut self) -> &mut Level {
        match self.subs.last_mut() {
            Some(level) => level,
            None => &mut self.base,
        }
    }

    /// The level under the one shown, while a slide into the shown one runs.
    fn below_top_mut(&mut self) -> Option<&mut Level> {
        match self.subs.len() {
            0 => None,
            1 => Some(&mut self.base),
            n => self.subs.get_mut(n - 2),
        }
    }

    /// The pane's size: its frame's width and the level's content, as tall
    /// as its room allows.
    #[must_use]
    pub(crate) fn size(&self) -> (f32, f32) {
        (self.frame.width, self.top().height.min(self.max_h))
    }

    /// Takes a new room for the pane: it is at most `max_h` tall and
    /// scrolls past it. Whether the size changed.
    pub(crate) fn set_max_h(&mut self, max_h: f32) -> bool {
        let max_h = length(max_h);
        if (max_h - self.max_h).abs() <= f32::EPSILON {
            return false;
        }
        self.max_h = max_h;
        self.sync_extent();
        true
    }

    /// The shown level's content height, the padding included.
    #[must_use]
    pub(crate) fn content_h(&self) -> f32 {
        self.top().height
    }

    /// How many levels below the root are open.
    #[must_use]
    pub(crate) fn depth(&self) -> usize {
        self.subs.len()
    }

    /// Sets the scroll's extent from the shown level and the room.
    fn sync_extent(&mut self) {
        let (content, viewport) = (self.top().height, self.size().1);
        self.scroll.set_extent(content, viewport);
    }

    /// The row the keyboard has in the shown level, if it is still there.
    fn focus(&self) -> Option<Focus> {
        let level = self.top();
        let id = level.tree.highlight().focused?;
        let key = level.tree.key_of(id);
        if key == back_key() {
            return level.back_label.is_some().then_some(Focus::Back);
        }
        let menu = level_at(&self.root, &level.path)?;
        let count = row_count(menu, shell_at(self.shell, &level.path));
        index_of(count, key).map(Focus::Row)
    }

    /// The top and height of `focus` in the shown level's content.
    fn span(&self, focus: Focus) -> Option<(f32, f32)> {
        let level = self.top();
        let menu = level_at(&self.root, &level.path)?;
        let shell = shell_at(self.shell, &level.path);
        let mut top = self.frame.pad_y;
        if level.back_label.is_some() {
            if focus == Focus::Back {
                return Some((top, self.theme.row_height));
            }
            top += self.theme.row_height;
        }
        let Focus::Row(index) = focus else {
            return None;
        };
        top += rows(menu, shell)
            .take(index)
            .map(|row| row_h(&self.theme, row))
            .sum::<f32>();
        let height = row_h(&self.theme, row_at(menu, shell, index)?);
        Some((top, height))
    }

    /// The focused row's rect in pane coordinates, the scroll taken off.
    #[must_use]
    pub(crate) fn focused_rect(&self) -> Option<Rect> {
        let (top, height) = self.span(self.focus()?)?;
        Some(Rect {
            x: self.frame.pad_x,
            y: top - self.scroll.offset(),
            w: (self.frame.width - 2.0 * self.frame.pad_x).max(0.0),
            h: height,
        })
    }

    /// Scrolls the least that shows the focused row with the pane's padding
    /// around it. Whether the offset moved.
    fn reveal_focus(&mut self) -> bool {
        let Some((top, height)) = self.focus().and_then(|focus| self.span(focus)) else {
            return false;
        };
        let margin = self.frame.pad_y;
        let want = scrolling::reveal(
            self.scroll.offset(),
            top - margin,
            height + 2.0 * margin,
            self.size().1,
        );
        self.scroll.wheel(want - self.scroll.offset())
    }

    /// The offset moved at `now`: the bar shows until [`THUMB_LINGER`] after.
    fn scrolled(&mut self, now: Instant) {
        self.thumb_until = Some(now + THUMB_LINGER);
    }

    /// Focuses the first row the keyboard can reach below the back row: a
    /// menu opened from the keyboard starts there, so Enter chooses at once.
    pub(crate) fn focus_first(&mut self) {
        let first = {
            let level = self.top();
            level_at(&self.root, &level.path).and_then(|menu| {
                let shell = shell_at(self.shell, &level.path);
                rows(menu, shell).position(|row| match row {
                    MenuRow::Entry(entry) => entry.enabled,
                    MenuRow::Shell(_) => true,
                    MenuRow::Separator => false,
                })
            })
        };
        let Some(first) = first else {
            return;
        };
        let level = self.top_mut();
        level.input.focus_key(&mut level.tree, entry_key(first));
        self.reveal_focus();
    }

    /// Shows `tree` (and the shell's entry) instead. Every level whose path
    /// the tree still has is rebuilt, and the focus stays on its row when
    /// that row is still there; a level that is gone falls back to the
    /// deepest one left. An awaited submenu opens once it has entries.
    /// Whether the look changed.
    pub(crate) fn replace(
        &mut self,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        now: Instant,
    ) -> bool {
        if tree == self.root && shell == self.shell {
            return false;
        }
        self.root = tree;
        self.shell = shell;
        let keep = self
            .subs
            .iter()
            .take_while(|level| level_at(&self.root, &level.path).is_some())
            .count();
        if keep < self.subs.len() {
            // The level on screen is not the same one: its slide and its
            // offset went with it.
            self.subs.truncate(keep);
            self.slide = None;
            self.scroll = Scroll::default();
        }
        for depth in 0..self.count() {
            self.rebuild(depth, now);
        }
        self.sync_extent();
        if self.reveal_focus() {
            self.scrolled(now);
        }
        self.open_awaited(now);
        true
    }

    /// Opens the awaited submenu once its entries are in the tree; forgets
    /// it when its entry or its parent level is gone.
    fn open_awaited(&mut self, now: Instant) {
        let Some(wait) = self.awaiting.clone() else {
            return;
        };
        let parent = &wait.path[..wait.path.len().saturating_sub(1)];
        let here = self.top().path.as_slice() == parent;
        match level_at(&self.root, &wait.path) {
            None => self.awaiting = None,
            _ if !here => self.awaiting = None,
            Some(sub) if !sub.entries.is_empty() => self.push_level(wait.path, wait.keyboard, now),
            Some(_) => {}
        }
    }

    /// Shows the submenu at `path` as a new level, sliding in from the right.
    fn push_level(&mut self, path: Vec<i32>, keyboard: bool, now: Instant) {
        self.awaiting = None;
        let from_offset = self.scroll.offset();
        // The pointer is not over the level it leaves any more.
        let level = self.top_mut();
        level
            .input
            .pointer(&mut level.tree, &InputEvent::PointerLeave, now);
        let level = self.make_level(path, now);
        self.subs.push(level);
        self.slide = (!self.reduced).then_some(Slide {
            dir: Dir::In,
            start: now,
            from_offset,
            gone: None,
        });
        self.scroll = Scroll::default();
        self.sync_extent();
        if keyboard {
            self.focus_first();
        }
    }

    /// Goes up one level, sliding out to the right; the parent starts at the
    /// top and reveals its focused row. Whether there was a level to leave.
    fn go_back(&mut self, now: Instant) -> bool {
        self.awaiting = None;
        let from_offset = self.scroll.offset();
        let Some(gone) = self.subs.pop() else {
            return false;
        };
        self.slide = (!self.reduced).then_some(Slide {
            dir: Dir::Out,
            start: now,
            from_offset,
            gone: Some(gone),
        });
        self.scroll = Scroll::default();
        self.sync_extent();
        if self.reveal_focus() {
            self.scrolled(now);
        }
        true
    }

    /// What activating row `index` of the shown level means; a submenu is
    /// entered (or awaited) here.
    fn activate(&mut self, index: usize, keyboard: bool, now: Instant) -> MenuOutcome {
        let level = self.top();
        let shell = shell_at(self.shell, &level.path);
        let Some(menu) = level_at(&self.root, &level.path) else {
            return MenuOutcome::Keep { changed: false };
        };
        let (id, submenu) = match row_at(menu, shell, index) {
            Some(MenuRow::Entry(entry)) if entry.enabled => (entry.id, entry.submenu.is_some()),
            Some(MenuRow::Shell(entry)) => return MenuOutcome::Shell(entry),
            _ => return MenuOutcome::Keep { changed: false },
        };
        if !submenu {
            return MenuOutcome::Choose(id);
        }
        let mut path = level.path.clone();
        path.push(id);
        let filled = level_at(&self.root, &path).is_some_and(|sub| !sub.entries.is_empty());
        if filled {
            self.push_level(path, keyboard, now);
        } else {
            self.awaiting = Some(Awaiting { path, keyboard });
        }
        MenuOutcome::Submenu(id)
    }

    /// Whether the focused row is an entry with a submenu.
    fn focused_submenu(&self) -> Option<usize> {
        let Focus::Row(index) = self.focus()? else {
            return None;
        };
        let level = self.top();
        let menu = level_at(&self.root, &level.path)?;
        match row_at(menu, shell_at(self.shell, &level.path), index)? {
            MenuRow::Entry(entry) if entry.submenu.is_some() => Some(index),
            _ => None,
        }
    }

    /// Handles one event. Up and Down (or Tab) move the focus through the
    /// enabled rows; Enter and Space activate the focused row; Right does
    /// for a submenu entry; Left and Escape go back one level (Escape at the
    /// root asks to close); a click activates a row; the wheel scrolls.
    /// Pointer events are in pane coordinates.
    pub(crate) fn input(&mut self, ev: &InputEvent, now: Instant) -> MenuOutcome {
        self.settle(now);
        match ev {
            InputEvent::Key {
                keysym: XKB_KEY_LEFT,
                pressed: true,
                ..
            } => {
                return if self.go_back(now) {
                    MenuOutcome::Back
                } else {
                    MenuOutcome::Keep { changed: false }
                };
            }
            InputEvent::Key {
                keysym: XKB_KEY_RIGHT,
                pressed: true,
                ..
            } => {
                return match self.focused_submenu() {
                    Some(index) => self.activate(index, true, now),
                    None => MenuOutcome::Keep { changed: false },
                };
            }
            InputEvent::Axis { vertical, .. } => {
                return MenuOutcome::Keep {
                    changed: self.wheel(*vertical, now),
                };
            }
            _ => {}
        }
        let tab = arrows_as_tab(ev);
        let ev = tab.as_ref().unwrap_or(ev);
        let at = (self.slide_x(now).1, -self.scroll.offset());
        let level = self.top_mut();
        let before = level.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } | InputEvent::FocusLost => {
                level.input.key(&mut level.tree, ev)
            }
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => level
                .input
                .pointer(&mut level.tree, &translated(ev, at.0, at.1), now),
        };
        let focus_moved = level.tree.highlight().focused != before.focused;
        let changed = !events.is_empty() || level.tree.highlight() != before;
        let mut scrolled = false;
        if focus_moved && self.reveal_focus() {
            self.scrolled(now);
            scrolled = true;
        }
        for event in &events {
            let (UiEvent::Click(key) | UiEvent::Activate(key)) = event else {
                if matches!(event, UiEvent::Escape) {
                    return if self.go_back(now) {
                        MenuOutcome::Back
                    } else {
                        MenuOutcome::Close
                    };
                }
                continue;
            };
            let keyboard = matches!(event, UiEvent::Activate(_));
            if *key == back_key() {
                if self.go_back(now) {
                    return MenuOutcome::Back;
                }
                continue;
            }
            let count = {
                let level = self.top();
                level_at(&self.root, &level.path)
                    .map_or(0, |menu| row_count(menu, shell_at(self.shell, &level.path)))
            };
            if let Some(index) = index_of(count, *key) {
                let outcome = self.activate(index, keyboard, now);
                if outcome != (MenuOutcome::Keep { changed: false }) {
                    return outcome;
                }
            }
        }
        MenuOutcome::Keep {
            changed: changed || scrolled,
        }
    }

    /// Scrolls by `vertical` logical px (positive goes down the content).
    /// Whether the offset moved; the bar shows for a moment when it did.
    pub(crate) fn wheel(&mut self, vertical: f32, now: Instant) -> bool {
        let moved = self.scroll.wheel(vertical);
        if moved {
            self.scrolled(now);
        }
        moved
    }

    /// Whether the shown level is taller than the pane.
    #[must_use]
    pub(crate) fn can_scroll(&self) -> bool {
        self.scroll.can_scroll()
    }

    /// Shows the pane in `theme`: every level is rebuilt. Whether the theme
    /// was a new one.
    pub(crate) fn set_theme(&mut self, theme: &Theme, now: Instant) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.cuts = Cuts::default();
        for depth in 0..self.count() {
            if let Some(level) = self.level_at_depth(depth) {
                level.tree.set_theme(theme);
            }
            self.rebuild(depth, now);
        }
        if let Some(gone) = self.slide.as_mut().and_then(|s| s.gone.as_mut()) {
            gone.tree.set_theme(theme);
        }
        self.sync_extent();
        true
    }

    /// Cuts every label that is wider than its room to it, measuring with
    /// `measure` (a card is narrower than a label may be). Only labels not
    /// cut before are measured; the levels are rebuilt when one is new.
    /// Whether the look changed.
    pub(crate) fn fit_labels(&mut self, measure: &mut Measure<'_>, now: Instant) -> bool {
        let md = self.theme.spacing.md;
        let plain = (self.frame.width - 2.0 * self.frame.pad_x - 2.0 * md - MARK - md).max(0.0);
        let mut wanted: Vec<(String, Spot)> = Vec::new();
        collect_labels(&self.root, &mut wanted);
        let style = text_style(
            self.theme.type_scale.body,
            Weight::Regular,
            false,
            Role::TextHi,
        );
        let mut added = false;
        for (label, spot) in wanted {
            if self.cuts.0.contains_key(&(label.clone(), spot)) {
                continue;
            }
            let room = match spot {
                Spot::Chevron => (plain - md - MARK).max(0.0),
                Spot::Plain | Spot::Back => plain,
            };
            let cut = fit_middle(&label, room, |t| measure(t, &style, None).0);
            self.cuts.0.insert((label, spot), cut);
            added = true;
        }
        if !added {
            return false;
        }
        for depth in 0..self.count() {
            self.rebuild(depth, now);
        }
        true
    }

    /// The label the row at `index` of the shown level has, as it is built
    /// (tests).
    #[cfg(test)]
    pub(crate) fn shown_label(&self, index: usize) -> Option<String> {
        let level = self.top();
        let menu = level_at(&self.root, &level.path)?;
        match row_at(menu, shell_at(self.shell, &level.path), index)? {
            MenuRow::Entry(entry) => {
                let spot = if entry.submenu.is_some() {
                    Spot::Chevron
                } else {
                    Spot::Plain
                };
                Some(self.cuts.of(&short(&entry.label), spot))
            }
            MenuRow::Separator | MenuRow::Shell(_) => None,
        }
    }

    /// How long a slide lasts.
    fn slide_duration(&self) -> Duration {
        Motion::Normal.duration(&self.theme.motion)
    }

    /// How far the slide at `now` is, 0 to 1; `None` when none runs.
    fn slide_progress(&self, now: Instant) -> Option<f32> {
        let slide = self.slide.as_ref()?;
        let total = self.slide_duration();
        let elapsed = now.saturating_duration_since(slide.start);
        if elapsed >= total {
            return None;
        }
        Some(elapsed.as_secs_f32() / total.as_secs_f32())
    }

    /// Drops a slide that ended.
    fn settle(&mut self, now: Instant) {
        if self.slide.is_some() && self.slide_progress(now).is_none() {
            self.slide = None;
        }
    }

    /// The x of the level a slide is leaving (`None` when none runs) and of
    /// the level shown, at `now`.
    fn slide_x(&self, now: Instant) -> (Option<f32>, f32) {
        let (Some(t), Some(slide)) = (self.slide_progress(now), self.slide.as_ref()) else {
            return (None, 0.0);
        };
        let (w, eased) = (self.frame.width, ONE_UI.eval(t));
        match slide.dir {
            Dir::In => (Some(-w * eased), w * (1.0 - eased)),
            Dir::Out => (Some(w * eased), -w * (1.0 - eased)),
        }
    }

    /// The scroll bar's rect at `now`, in pane coordinates; `None` when it
    /// is not showing or the level fits.
    fn thumb_rect(&self, now: Instant) -> Option<Rect> {
        if !self.thumb_until.is_some_and(|until| now < until) {
            return None;
        }
        let (y, h) = scrolling::thumb(self.scroll.offset(), self.top().height, self.size().1)?;
        Some(Rect {
            x: self.frame.width - THUMB_INSET - THUMB_W,
            y,
            w: THUMB_W,
            h,
        })
    }

    /// Lays the shown level out (and the one a slide is leaving) in its own
    /// size with `lay`.
    pub(crate) fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        let width = self.frame.width;
        let mut lay_one = |level: &mut Level| {
            lay(&mut level.tree, (width, level.height));
            if let Some(fault) = level.tree.take_fault() {
                tracing::warn!(%fault, "tray pane: layout fault");
            }
        };
        match self.slide.as_ref().map(|slide| slide.dir) {
            Some(Dir::In) => {
                if let Some(below) = self.below_top_mut() {
                    lay_one(below);
                }
            }
            Some(Dir::Out) => {
                if let Some(gone) = self.slide.as_mut().and_then(|slide| slide.gone.as_mut()) {
                    lay_one(gone);
                }
            }
            None => {}
        }
        lay_one(self.top_mut());
    }

    /// Paints the shown level at its offset, and the one a slide is leaving
    /// beside it, clipped to the pane; then the scroll bar. The holder's
    /// glass is its own.
    pub(crate) fn paint(&mut self, painter: &mut Painter, now: Instant) {
        self.settle(now);
        let size = self.size();
        let (old_x, x) = self.slide_x(now);
        let offset = self.scroll.offset();
        painter.push_clip(Rect::whole(size), 0.0);
        if let (Some(old_x), Some((dir, from))) = (
            old_x,
            self.slide
                .as_ref()
                .map(|slide| (slide.dir, slide.from_offset)),
        ) {
            let old = match dir {
                Dir::In => self.below_top_mut(),
                Dir::Out => self.slide.as_mut().and_then(|slide| slide.gone.as_mut()),
            };
            if let Some(old) = old {
                paint_level(painter, old, old_x, from, now);
            }
        }
        paint_level(painter, self.top_mut(), x, offset, now);
        if let Some(thumb) = self.thumb_rect(now) {
            let color = painter.theme().color(Role::ControlHover);
            painter.fill_rounded(thumb, THUMB_RADIUS, color);
        }
        painter.pop_clip();
    }

    /// Whether a frame is due: a slide runs, the scroll bar shows, or the
    /// shown level's own animations run.
    #[must_use]
    pub(crate) fn is_animating(&self, now: Instant) -> bool {
        self.slide_progress(now).is_some()
            || self.thumb_until.is_some_and(|until| now < until)
            || self.top().tree.is_animating(now)
    }

    /// The shown level's tree.
    #[must_use]
    pub(crate) fn tree(&self) -> &Tree {
        &self.top().tree
    }
}

/// Every label of `level` and below as a row shows it, with its place.
fn collect_labels(level: &MenuTree, out: &mut Vec<(String, Spot)>) {
    for entry in level.entries.iter().filter(|e| !e.separator) {
        let label = short(&entry.label);
        match &entry.submenu {
            Some(sub) => {
                out.push((label.clone(), Spot::Chevron));
                out.push((label, Spot::Back));
                collect_labels(sub, out);
            }
            None => out.push((label, Spot::Plain)),
        }
    }
}

/// Paints `level` shifted by (`x`, `-offset`).
fn paint_level(painter: &mut Painter, level: &mut Level, x: f32, offset: f32, now: Instant) {
    painter.push_state();
    painter.translate(x, -offset);
    level.tree.paint(painter, now);
    painter.pop_state();
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use celestina_paint::{IconKey, Rect, Role, TextStyle, Theme};
    use celestina_ui::{Key, Motion, View, ViewKind, ONE_UI};
    use celestina_wayland::{InputEvent, Modifiers};
    use unicode_segmentation::UnicodeSegmentation;

    use super::*;
    use crate::copy;
    use crate::tray::{
        menu::{
            entry_key, level_at, separator_h, ShellEntry, MAX_LABEL_GRAPHEMES, MENU_W,
            XKB_KEY_DOWN, XKB_KEY_LEFT, XKB_KEY_RIGHT, XKB_KEY_UP,
        },
        MenuEntry, MenuTree,
    };

    const RETURN: u32 = 0xff0d;
    const ESCAPE: u32 = 0xff1b;
    const TAB: u32 = 0xff09;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
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

    fn submenu(id: i32, label: &str, entries: Vec<MenuEntry>) -> MenuEntry {
        MenuEntry {
            submenu: Some(MenuTree { id, entries }),
            ..entry(id, label)
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
                submenu(5, "More", vec![entry(51, "Inner")]),
            ],
        }
    }

    /// `level()` with a second submenu level under "Inner".
    fn deep() -> MenuTree {
        let mut tree = level();
        tree.entries[5] = submenu(
            5,
            "More",
            vec![
                entry(50, "Sibling"),
                submenu(51, "Inner", vec![entry(511, "Deepest")]),
            ],
        );
        tree
    }

    fn lay(pane: &mut MenuPane) {
        pane.layout(&mut |tree, size| tree.layout_measured(size, fake_measure));
    }

    fn pane_in(
        tree: MenuTree,
        shell: Option<ShellEntry>,
        frame: PaneFrame,
        max_h: f32,
        reduced: bool,
        now: Instant,
    ) -> MenuPane {
        let mut pane = MenuPane::new(&Theme::dark(), tree, shell, frame, max_h, reduced, now);
        lay(&mut pane);
        pane
    }

    fn pane_of(tree: MenuTree, shell: Option<ShellEntry>, now: Instant) -> MenuPane {
        pane_in(tree, shell, PaneFrame::WINDOW, 496.0, false, now)
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

    fn press(pane: &mut MenuPane, keysym: u32, now: Instant) -> MenuOutcome {
        let outcome = pane.input(&key(keysym), now);
        lay(pane);
        outcome
    }

    fn focused(pane: &MenuPane) -> Option<Key> {
        let tree = pane.tree();
        tree.highlight().focused.map(|id| tree.key_of(id))
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn text_of(v: &View) -> Option<&str> {
        match &v.kind {
            ViewKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    fn glyph_of(v: &View) -> Option<&'static str> {
        match &v.kind {
            ViewKind::Icon {
                key: IconKey::Semantic(name),
                ..
            } => Some(name),
            _ => None,
        }
    }

    fn rect_of(pane: &MenuPane, key: Key) -> Rect {
        let tree = pane.tree();
        match tree.node_by_key(key) {
            Some(id) => tree.rect(id),
            None => panic!("no node for the key"),
        }
    }

    /// Where the pane's first level has its row `index`, in pane coordinates.
    fn click(pane: &mut MenuPane, key: Key, now: Instant) -> MenuOutcome {
        let r = rect_of(pane, key);
        let at = (r.x + r.w / 2.0, r.y + r.h / 2.0 - pane.scroll.offset());
        let down = InputEvent::PointerPress {
            x: at.0,
            y: at.1,
            button: celestina_ui::BTN_LEFT,
            serial: 2,
        };
        let up = InputEvent::PointerRelease {
            x: at.0,
            y: at.1,
            button: celestina_ui::BTN_LEFT,
            serial: 3,
        };
        pane.input(&down, now);
        pane.input(&up, now)
    }

    #[test]
    fn rows_map_entries_separators_toggles_and_disabled_state() {
        let theme = Theme::dark();
        let tree = level();
        let v = build_view(&theme, PaneFrame::WINDOW, &tree, None, None);
        let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        assert_eq!(keys, (0..6).map(entry_key).collect::<Vec<_>>());
        let row = |i: usize| &v.children[i];
        assert!(
            row(0).interactive && row(0).focusable,
            "an entry is a control"
        );
        assert!(!row(1).interactive && !row(1).focusable, "a line is not");
        assert!(find(row(1), Key::indexed("tray-menu-line", 1)).is_some());
        assert!(!row(4).interactive && !row(4).focusable, "disabled entry");
        let check = |i: u64| find(&v, Key::indexed("tray-menu-check", i));
        assert_eq!(check(2).and_then(glyph_of), Some("check"), "checked toggle");
        for unchecked in [0, 3, 4] {
            assert!(check(unchecked).and_then(glyph_of).is_none(), "{unchecked}");
        }
        let role_of = |i: u64| match find(&v, Key::indexed("tray-menu-label", i)).map(|c| &c.kind) {
            Some(ViewKind::Text { style, .. }) => Some(style.role),
            _ => None,
        };
        assert_eq!(role_of(0), Some(Role::TextHi));
        assert_eq!(role_of(4), Some(Role::TextFaint));
        assert_eq!(
            find(&v, Key::indexed("tray-menu-more", 5)).and_then(glyph_of),
            Some("chevron-right")
        );
        assert!(find(&v, Key::indexed("tray-menu-more", 0)).is_none());
        // Laid out: 40 high rows, 17 high separator, 8 px of padding.
        let pane = pane_of(tree, None, Instant::now());
        let at = |i: usize| rect_of(&pane, entry_key(i));
        assert_eq!(
            (at(0).x, at(0).y, at(0).w, at(0).h),
            (8.0, 8.0, 284.0, 40.0)
        );
        assert_eq!(at(1).h, 17.0);
        assert_eq!(at(1).h, separator_h(&theme));
        assert_eq!(at(2).y, 8.0 + 40.0 + 17.0);
        assert_eq!(rect_of(&pane, Key::indexed("tray-menu-check", 2)).w, 16.0);
    }

    #[test]
    fn the_root_ends_with_the_shell_entry_after_one_separator() {
        let theme = Theme::dark();
        for (shell, glyph, label) in [
            (ShellEntry::Hide, "eye-off", copy::TRAY_HIDE),
            (ShellEntry::Show, "eye", copy::TRAY_SHOW),
        ] {
            let tree = level();
            let v = build_view(&theme, PaneFrame::WINDOW, &tree, Some(shell), None);
            let n = v.children.len();
            assert_eq!(n, tree.entries.len() + 2);
            let line = &v.children[n - 2];
            assert!(find(line, Key::indexed("tray-menu-line", (n - 2) as u64)).is_some());
            assert!(!line.interactive);
            let last = &v.children[n - 1];
            assert!(last.interactive && last.focusable);
            let at = (n - 1) as u64;
            assert_eq!(
                find(last, Key::indexed("tray-menu-check", at)).and_then(glyph_of),
                Some(glyph)
            );
            assert_eq!(
                find(last, Key::indexed("tray-menu-label", at)).and_then(text_of),
                Some(label)
            );
        }
        // No `Id`, no shell entry.
        let v = build_view(&theme, PaneFrame::WINDOW, &level(), None, None);
        assert_eq!(v.children.len(), level().entries.len());
        // A level already ending in a separator gets no second line.
        let mut ending = level();
        ending.entries.push(separator_entry());
        let v = build_view(
            &theme,
            PaneFrame::WINDOW,
            &ending,
            Some(ShellEntry::Show),
            None,
        );
        assert_eq!(v.children.len(), ending.entries.len() + 1);
        // Only the root has it: a sublevel shows the app's entries and the back row.
        let mut pane = pane_of(level(), Some(ShellEntry::Hide), Instant::now());
        press(&mut pane, XKB_KEY_UP, Instant::now());
        assert_eq!(
            focused(&pane),
            Some(entry_key(7)),
            "Up from nothing: the shell entry"
        );
        let tree = level();
        let sub = level_at(&tree, &[5]).expect("the submenu");
        let v = build_view(&theme, PaneFrame::WINDOW, sub, None, Some("More"));
        assert_eq!(v.children.len(), 1 + sub.entries.len());
    }

    #[test]
    fn an_app_without_entries_gets_the_shell_entry_alone() {
        let now = Instant::now();
        let mut pane = pane_of(MenuTree::default(), Some(ShellEntry::Hide), now);
        assert_eq!(pane.size(), (300.0, 8.0 + 40.0 + 8.0));
        assert_eq!(
            press(&mut pane, XKB_KEY_DOWN, now),
            MenuOutcome::Keep { changed: true }
        );
        assert_eq!(focused(&pane), Some(entry_key(0)));
        assert_eq!(
            press(&mut pane, RETURN, now),
            MenuOutcome::Shell(ShellEntry::Hide)
        );
        // Without the entry nothing is listed.
        let bare = pane_of(MenuTree::default(), None, now);
        assert_eq!(bare.size(), (300.0, 16.0));
        assert!(!bare.can_scroll());
    }

    #[test]
    fn labels_are_cut_at_36_graphemes() {
        assert_eq!(short("Open"), "Open");
        let exact = "a".repeat(MAX_LABEL_GRAPHEMES);
        assert_eq!(short(&exact), exact, "36 fit");
        let cut = short(&"x".repeat(100));
        assert_eq!(
            cut,
            format!("{}\u{2026}", "x".repeat(35)),
            "35 kept and the ellipsis"
        );
        // A grapheme is a cluster: 'e' with its accent counts once.
        let accents = "e\u{301}".repeat(40);
        let cut = short(&accents);
        assert_eq!(cut.graphemes(true).count(), MAX_LABEL_GRAPHEMES);
        assert!(cut.ends_with('\u{2026}'));
        // The row and the back row show the cut label.
        let theme = Theme::dark();
        let tree = MenuTree {
            id: 0,
            entries: vec![entry(1, &"y".repeat(100))],
        };
        let v = build_view(
            &theme,
            PaneFrame::WINDOW,
            &tree,
            None,
            Some(&short(&"z".repeat(100))),
        );
        assert_eq!(
            find(&v, Key::indexed("tray-menu-label", 0)).and_then(text_of),
            Some(short(&"y".repeat(100)).as_str())
        );
        assert_eq!(
            find(&v, back_label_key()).and_then(text_of),
            Some(short(&"z".repeat(100)).as_str())
        );
    }

    #[test]
    fn the_window_frame_is_300_wide_with_8_of_padding() {
        let now = Instant::now();
        let three = MenuTree {
            id: 0,
            entries: vec![entry(1, "a"), entry(2, "b"), entry(3, "c")],
        };
        let pane = pane_of(three.clone(), None, now);
        assert_eq!(pane.size(), (300.0, 136.0));
        assert_eq!(pane.content_h(), 136.0);
        assert_eq!(pane.depth(), 0);
        assert_eq!(PaneFrame::WINDOW.width, MENU_W);
        // A card is as wide as asked, with 4 px under and over and none across.
        let card = pane_in(three, None, PaneFrame::card(264.0), 496.0, false, now);
        assert_eq!(card.size(), (264.0, 4.0 + 120.0 + 4.0));
        let first = rect_of(&card, entry_key(0));
        assert_eq!((first.x, first.y, first.w), (0.0, 4.0, 264.0));
        // The window never outgrows the room it has.
        let tall = MenuTree {
            id: 0,
            entries: (0..30).map(|i| entry(i, "row")).collect(),
        };
        let pane = pane_in(tall, None, PaneFrame::WINDOW, 200.0, false, now);
        assert_eq!(pane.size(), (300.0, 200.0));
        assert_eq!(pane.content_h(), 16.0 + 30.0 * 40.0);
    }

    #[test]
    fn arrows_skip_disabled_entries_and_separators_and_enter_chooses() {
        let now = Instant::now();
        let mut pane = pane_of(level(), None, now);
        assert_eq!(focused(&pane), None);
        press(&mut pane, XKB_KEY_DOWN, now);
        assert_eq!(focused(&pane), Some(entry_key(0)));
        press(&mut pane, XKB_KEY_DOWN, now);
        assert_eq!(
            focused(&pane),
            Some(entry_key(2)),
            "the separator is skipped"
        );
        press(&mut pane, XKB_KEY_DOWN, now);
        press(&mut pane, XKB_KEY_DOWN, now);
        assert_eq!(
            focused(&pane),
            Some(entry_key(5)),
            "the disabled entry is skipped"
        );
        press(&mut pane, XKB_KEY_DOWN, now);
        assert_eq!(focused(&pane), Some(entry_key(0)), "wraps to the top");
        press(&mut pane, XKB_KEY_UP, now);
        assert_eq!(focused(&pane), Some(entry_key(5)));
        for _ in 0..3 {
            press(&mut pane, XKB_KEY_UP, now);
        }
        assert_eq!(focused(&pane), Some(entry_key(0)));
        // A held key repeats: the repeat moves the focus as the press does.
        let repeat = InputEvent::KeyRepeat {
            keysym: XKB_KEY_DOWN,
            utf8: None,
            modifiers: Modifiers::default(),
            serial: 1,
        };
        assert_eq!(
            pane.input(&repeat, now),
            MenuOutcome::Keep { changed: true }
        );
        assert_eq!(focused(&pane), Some(entry_key(2)));
        press(&mut pane, XKB_KEY_UP, now);
        assert_eq!(press(&mut pane, RETURN, now), MenuOutcome::Choose(1));
        // Tab and Shift+Tab are what the arrows turn into.
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let up = InputEvent::Key {
            keysym: XKB_KEY_UP,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 9,
        };
        assert_eq!(
            arrows_as_tab(&up),
            Some(InputEvent::Key {
                keysym: TAB,
                utf8: None,
                modifiers: shift,
                pressed: true,
                serial: 9,
            })
        );
        assert_eq!(
            arrows_as_tab(&key(XKB_KEY_DOWN)),
            Some(InputEvent::Key {
                keysym: TAB,
                utf8: None,
                modifiers: Modifiers::default(),
                pressed: true,
                serial: 1,
            })
        );
        assert_eq!(arrows_as_tab(&key(RETURN)), None);
        assert_eq!(
            arrows_as_tab(&repeat).map(|e| matches!(e, InputEvent::Key { .. })),
            Some(true)
        );
    }

    #[test]
    fn right_or_enter_on_a_submenu_drills_in_and_asks_about_to_show() {
        let t0 = Instant::now();
        let mut pane = pane_of(level(), None, t0);
        // Right on nothing, or on an entry without a submenu, does nothing.
        assert_eq!(
            press(&mut pane, XKB_KEY_RIGHT, t0),
            MenuOutcome::Keep { changed: false }
        );
        press(&mut pane, XKB_KEY_DOWN, t0);
        assert_eq!(
            press(&mut pane, XKB_KEY_RIGHT, t0),
            MenuOutcome::Keep { changed: false }
        );
        assert_eq!(pane.depth(), 0);
        press(&mut pane, XKB_KEY_UP, t0);
        assert_eq!(
            focused(&pane),
            Some(entry_key(5)),
            "Up from the first wraps to the last"
        );
        assert_eq!(press(&mut pane, XKB_KEY_RIGHT, t0), MenuOutcome::Submenu(5));
        assert_eq!(pane.depth(), 1);
        // The first row of a sublevel is the back row, with the parent's label.
        let back = rect_of(&pane, back_key());
        assert_eq!((back.x, back.y, back.h), (8.0, 8.0, 40.0));
        let level_now = pane.top();
        assert_eq!(level_now.back_label.as_deref(), Some("More"));
        assert_eq!(rect_of(&pane, entry_key(0)).y, 8.0 + 40.0);
        assert_eq!(pane.size().1, 8.0 + 40.0 + 40.0 + 8.0);
        // Opened from the keyboard, the first entry of the sublevel is focused.
        assert_eq!(focused(&pane), Some(entry_key(0)));
        // Left goes back, the parent's row focused again; Enter does as Right.
        assert_eq!(press(&mut pane, XKB_KEY_LEFT, t0), MenuOutcome::Back);
        assert_eq!(focused(&pane), Some(entry_key(5)));
        assert_eq!(press(&mut pane, RETURN, t0), MenuOutcome::Submenu(5));
        assert_eq!(pane.depth(), 1);
    }

    #[test]
    fn a_submenu_without_entries_waits_and_opens_when_a_republish_fills_it() {
        let t0 = Instant::now();
        let mut waiting = level();
        waiting.entries[5] = submenu(5, "More", Vec::new());
        let mut pane = pane_of(waiting.clone(), None, t0);
        press(&mut pane, XKB_KEY_UP, t0);
        assert_eq!(
            press(&mut pane, RETURN, t0),
            MenuOutcome::Submenu(5),
            "AboutToShow is asked"
        );
        assert_eq!(pane.depth(), 0, "nothing to show yet");
        // A republish that changes something else opens nothing.
        let mut other = waiting.clone();
        other.entries[0].label = "Open now".to_owned();
        assert!(pane.replace(other, None, t0 + ms(10)));
        assert_eq!(pane.depth(), 0);
        // The item filled it: the pane opens it.
        assert!(pane.replace(level(), None, t0 + ms(20)));
        assert_eq!(pane.depth(), 1);
        assert_eq!(pane.top().back_label.as_deref(), Some("More"));
        // An awaited entry that vanished is forgotten for good.
        let mut pane = pane_of(waiting, None, t0);
        press(&mut pane, XKB_KEY_UP, t0);
        press(&mut pane, RETURN, t0);
        let mut gone = level();
        gone.entries.pop();
        pane.replace(gone, None, t0 + ms(10));
        pane.replace(level(), None, t0 + ms(20));
        assert_eq!(pane.depth(), 0, "it was not asked for again");
        // Nothing changed, nothing to redraw.
        assert!(!pane.replace(level(), None, t0 + ms(30)));
    }

    #[test]
    fn the_drill_in_slides_200_ms_and_back_reverses() {
        let t0 = Instant::now();
        let theme = Theme::dark();
        assert_eq!(Motion::Normal.duration(&theme.motion), ms(200));
        let mut pane = pane_of(level(), None, t0);
        press(&mut pane, XKB_KEY_UP, t0);
        press(&mut pane, RETURN, t0);
        assert_eq!(pane.depth(), 1);
        let w = 300.0;
        assert_eq!(pane.slide_x(t0), (Some(0.0), w));
        let half = t0 + ms(100);
        let e = ONE_UI.eval(0.5);
        assert_eq!(pane.slide_x(half), (Some(-w * e), w * (1.0 - e)));
        assert!(pane.is_animating(t0 + ms(199)));
        assert_eq!(pane.slide_x(t0 + ms(200)), (None, 0.0), "settled");
        assert!(!pane.is_animating(t0 + ms(200)));
        // Back: the sublevel leaves to the right, the parent comes from the left.
        let t1 = t0 + ms(500);
        assert_eq!(press(&mut pane, XKB_KEY_LEFT, t1), MenuOutcome::Back);
        assert_eq!(pane.slide_x(t1), (Some(0.0), -w));
        assert_eq!(pane.slide_x(t1 + ms(100)), (Some(w * e), -w * (1.0 - e)));
        assert_eq!(pane.slide_x(t1 + ms(200)), (None, 0.0));
        assert!(pane.is_animating(t1 + ms(1)));
        assert!(!pane.is_animating(t1 + ms(200)));
    }

    #[test]
    fn left_and_escape_go_back_and_restore_the_parents_focus() {
        let t0 = Instant::now();
        let mut pane = pane_of(level(), None, t0);
        // At the root Left does nothing and Escape closes.
        assert_eq!(
            press(&mut pane, XKB_KEY_LEFT, t0),
            MenuOutcome::Keep { changed: false }
        );
        assert_eq!(press(&mut pane, ESCAPE, t0), MenuOutcome::Close);
        press(&mut pane, XKB_KEY_DOWN, t0);
        press(&mut pane, XKB_KEY_DOWN, t0);
        press(&mut pane, XKB_KEY_DOWN, t0);
        assert_eq!(focused(&pane), Some(entry_key(3)));
        press(&mut pane, XKB_KEY_DOWN, t0);
        assert_eq!(focused(&pane), Some(entry_key(5)));
        press(&mut pane, RETURN, t0);
        assert_eq!(pane.depth(), 1);
        assert_eq!(
            press(&mut pane, ESCAPE, t0),
            MenuOutcome::Back,
            "Escape goes back one level"
        );
        assert_eq!(pane.depth(), 0);
        assert_eq!(
            focused(&pane),
            Some(entry_key(5)),
            "the parent's focus is back"
        );
        // Two levels deep: each Left goes one up and restores that level's row.
        let mut pane = pane_of(deep(), None, t0);
        press(&mut pane, XKB_KEY_UP, t0);
        press(&mut pane, RETURN, t0);
        assert_eq!(pane.depth(), 1);
        press(&mut pane, XKB_KEY_DOWN, t0);
        assert_eq!(focused(&pane), Some(entry_key(1)), "Inner");
        assert_eq!(press(&mut pane, RETURN, t0), MenuOutcome::Submenu(51));
        assert_eq!(pane.depth(), 2);
        assert_eq!(press(&mut pane, XKB_KEY_LEFT, t0), MenuOutcome::Back);
        assert_eq!(focused(&pane), Some(entry_key(1)));
        assert_eq!(press(&mut pane, XKB_KEY_LEFT, t0), MenuOutcome::Back);
        assert_eq!(focused(&pane), Some(entry_key(5)));
        assert_eq!(press(&mut pane, ESCAPE, t0), MenuOutcome::Close);
    }

    #[test]
    fn a_click_on_the_back_row_goes_back() {
        let t0 = Instant::now();
        let mut pane = pane_of(level(), None, t0);
        assert_eq!(click(&mut pane, entry_key(5), t0), MenuOutcome::Submenu(5));
        lay(&mut pane);
        assert_eq!(pane.depth(), 1);
        assert_eq!(focused(&pane), None, "a click leaves the keyboard alone");
        // The back row is a control; Enter on it goes back too.
        let t1 = t0 + ms(300);
        assert_eq!(click(&mut pane, back_key(), t1), MenuOutcome::Back);
        assert_eq!(pane.depth(), 0);
        lay(&mut pane);
        // A click on an entry of the sublevel chooses it.
        let t2 = t1 + ms(300);
        click(&mut pane, entry_key(5), t2);
        lay(&mut pane);
        assert_eq!(
            click(&mut pane, entry_key(0), t2 + ms(300)),
            MenuOutcome::Choose(51)
        );
        // Enter on the focused back row goes back.
        lay(&mut pane);
        press(&mut pane, XKB_KEY_UP, t2 + ms(300));
        assert_eq!(focused(&pane), Some(entry_key(0)));
        press(&mut pane, XKB_KEY_UP, t2 + ms(300));
        assert_eq!(focused(&pane), Some(back_key()));
        assert_eq!(press(&mut pane, RETURN, t2 + ms(300)), MenuOutcome::Back);
        assert_eq!(pane.depth(), 0);
    }

    #[test]
    fn a_republish_keeps_the_focus_on_its_row() {
        let t0 = Instant::now();
        let mut pane = pane_of(level(), None, t0);
        press(&mut pane, XKB_KEY_DOWN, t0);
        press(&mut pane, XKB_KEY_DOWN, t0);
        assert_eq!(focused(&pane), Some(entry_key(2)));
        // The same tree again changes nothing.
        assert!(!pane.replace(level(), None, t0 + ms(5)));
        // A longer tree: the row that is still there keeps the focus.
        let mut longer = level();
        longer.entries.push(entry(9, "Extra"));
        assert!(pane.replace(longer, None, t0 + ms(10)));
        assert_eq!(focused(&pane), Some(entry_key(2)));
        assert_eq!(
            pane.content_h(),
            16.0 + 6.0 * 40.0 + 17.0,
            "7 rows, one line"
        );
        // A shorter tree that drops the focused row leaves no focus.
        let shorter = MenuTree {
            id: 0,
            entries: vec![entry(1, "Open"), entry(7, "New")],
        };
        assert!(pane.replace(shorter, None, t0 + ms(20)));
        lay(&mut pane);
        assert_eq!(focused(&pane), None);
        assert!(pane.focused_rect().is_none());
        press(&mut pane, XKB_KEY_DOWN, t0 + ms(30));
        assert_eq!(focused(&pane), Some(entry_key(0)));
        assert_eq!(
            press(&mut pane, XKB_KEY_DOWN, t0 + ms(30)),
            MenuOutcome::Keep { changed: true }
        );
        assert_eq!(
            press(&mut pane, RETURN, t0 + ms(30)),
            MenuOutcome::Choose(7)
        );
        // A shell entry added by a republish shows too.
        assert!(pane.replace(level(), Some(ShellEntry::Hide), t0 + ms(40)));
        assert_eq!(
            pane.content_h(),
            16.0 + 6.0 * 40.0 + 2.0 * 17.0,
            "line and entry"
        );
    }

    #[test]
    fn a_republish_that_removes_the_open_submenu_falls_back_to_the_deepest_level_left() {
        let t0 = Instant::now();
        let open_two_deep = |t: Instant| {
            let mut pane = pane_of(deep(), None, t);
            press(&mut pane, XKB_KEY_UP, t);
            press(&mut pane, RETURN, t);
            press(&mut pane, XKB_KEY_DOWN, t);
            assert_eq!(press(&mut pane, RETURN, t), MenuOutcome::Submenu(51));
            assert_eq!(pane.depth(), 2);
            pane
        };
        let check = |pane: &mut MenuPane, depth: usize| {
            assert_eq!(pane.depth(), depth);
            lay(pane);
            if let Some(f) = focused(pane) {
                let tree = pane.tree();
                assert!(tree.node_by_key(f).is_some(), "the focus is on a live row");
                assert!(pane.focused_rect().is_some_and(|r| r.w > 0.0 && r.h > 0.0));
            }
            assert!(pane.size().1.is_finite() && pane.size().1 > 0.0);
            // And the keyboard still works.
            press(pane, XKB_KEY_DOWN, t0 + ms(900));
            press(pane, XKB_KEY_UP, t0 + ms(900));
        };
        // "Inner" loses its submenu: the pane is one level up.
        let mut pane = open_two_deep(t0);
        let mut without_inner = deep();
        without_inner.entries[5] = submenu(5, "More", vec![entry(50, "Sibling")]);
        assert!(pane.replace(without_inner, None, t0 + ms(50)));
        check(&mut pane, 1);
        // "More" is gone altogether: the root.
        let mut pane = open_two_deep(t0);
        let mut without_more = deep();
        without_more.entries.pop();
        assert!(pane.replace(without_more, None, t0 + ms(50)));
        check(&mut pane, 0);
        // The same, in the middle of the slide of the last drill-in.
        let mut pane = open_two_deep(t0);
        assert!(pane.is_animating(t0 + ms(40)));
        pane.replace(level(), None, t0 + ms(40));
        assert_eq!(
            pane.slide_x(t0 + ms(41)),
            (None, 0.0),
            "the slide went with its level"
        );
        check(&mut pane, 1);
        assert_eq!(Some(pane.top().path.clone()), Some(vec![5]));
        let mut pane = open_two_deep(t0);
        pane.replace(MenuTree::default(), None, t0 + ms(40));
        check(&mut pane, 0);
        assert!(
            !pane.is_animating(t0 + ms(2000)),
            "no slide is left running"
        );
        // A level that stays but loses every entry keeps the back row.
        let mut pane = open_two_deep(t0);
        let mut empty_inner = deep();
        empty_inner.entries[5] = submenu(
            5,
            "More",
            vec![entry(50, "Sibling"), submenu(51, "Inner", vec![])],
        );
        pane.replace(empty_inner, None, t0 + ms(60));
        assert_eq!(pane.depth(), 2);
        assert_eq!(pane.size().1, 8.0 + 40.0 + 8.0);
        check(&mut pane, 2);
    }

    #[test]
    fn a_long_level_scrolls_and_the_keyboard_reaches_the_shell_entry_in_view() {
        let t0 = Instant::now();
        let tall = MenuTree {
            id: 0,
            entries: (0..64).map(|i| entry(i, "row")).collect(),
        };
        let mut pane = pane_of(tall, Some(ShellEntry::Hide), t0);
        assert_eq!(pane.size().1, 496.0, "the window stops at the room it has");
        assert!(pane.content_h() > 496.0);
        assert!(pane.can_scroll());
        // 64 entries and the shell entry; the line before it takes no focus.
        for step in 1..=65 {
            press(&mut pane, XKB_KEY_DOWN, t0 + ms(step));
            let r = pane.focused_rect().expect("a focused row");
            assert!(r.y >= 0.0 && r.y + r.h <= 496.0, "step {step}: {r:?}");
        }
        assert_eq!(focused(&pane), Some(entry_key(65)), "the shell entry");
        let last = pane.focused_rect().expect("the shell entry");
        assert_eq!(last.y + last.h + 8.0, 496.0, "the padding shows under it");
        assert_eq!(pane.scroll.offset(), pane.content_h() - 496.0);
        assert_eq!(
            press(&mut pane, RETURN, t0),
            MenuOutcome::Shell(ShellEntry::Hide)
        );
        // Down once more wraps to the top and scrolls back up.
        press(&mut pane, XKB_KEY_DOWN, t0 + ms(900));
        assert_eq!(focused(&pane), Some(entry_key(0)));
        assert_eq!(pane.scroll.offset(), 0.0);
        // Up from the top goes to the end.
        press(&mut pane, XKB_KEY_UP, t0 + ms(901));
        assert_eq!(focused(&pane), Some(entry_key(65)));
        assert!(pane.focused_rect().is_some_and(|r| r.y + r.h <= 496.0));
    }

    #[test]
    fn the_wheel_scrolls_and_the_thumb_lingers_800_ms() {
        let t0 = Instant::now();
        let tall = MenuTree {
            id: 0,
            entries: (0..30).map(|i| entry(i, "row")).collect(),
        };
        let mut pane = pane_in(tall, None, PaneFrame::WINDOW, 400.0, false, t0);
        let settled = t0 + ms(1000);
        assert!(!pane.is_animating(settled), "no bar until the offset moves");
        assert_eq!(pane.thumb_rect(settled), None);
        assert!(
            pane.wheel(30.0, settled),
            "the wheel moves the offset by its px"
        );
        assert_eq!(pane.scroll.offset(), 30.0, "no slide");
        let thumb = pane.thumb_rect(settled).expect("the bar");
        assert_eq!((thumb.w, thumb.x), (4.0, 300.0 - 6.0));
        assert_eq!(pane.thumb_rect(settled + ms(799)).map(|r| r.w), Some(4.0));
        let (y, h) = crate::scrolling::thumb(30.0, pane.content_h(), 400.0).expect("scrolls");
        assert_eq!((thumb.y, thumb.h), (y, h));
        assert!(h >= crate::scrolling::THUMB_MIN);
        assert!(
            pane.is_animating(settled + ms(799)),
            "a frame is due while it shows"
        );
        assert_eq!(
            pane.thumb_rect(settled + ms(800)),
            None,
            "gone after 800 ms"
        );
        assert!(
            !pane.is_animating(settled + ms(800)),
            "and no frame is asked for"
        );
        // The offset stays inside the content.
        assert!(pane.wheel(100_000.0, settled + ms(900)));
        assert_eq!(pane.scroll.offset(), pane.content_h() - 400.0);
        assert!(!pane.wheel(10.0, settled + ms(900)), "already at the end");
        assert!(pane.wheel(-100_000.0, settled + ms(900)));
        assert_eq!(pane.scroll.offset(), 0.0);
        // The wheel event of the surface does the same through `input`.
        let axis = InputEvent::Axis {
            x: 10.0,
            y: 10.0,
            horizontal: 0.0,
            vertical: 40.0,
        };
        assert_eq!(
            pane.input(&axis, settled + ms(2000)),
            MenuOutcome::Keep { changed: true }
        );
        assert_eq!(pane.scroll.offset(), 40.0);
        // A pointer press scrolled down hits the row under it.
        lay(&mut pane);
        let at = (150.0, 8.0 + 5.0 * 40.0 + 20.0 - 40.0);
        let pressed = InputEvent::PointerPress {
            x: at.0,
            y: at.1,
            button: celestina_ui::BTN_LEFT,
            serial: 4,
        };
        let released = InputEvent::PointerRelease {
            x: at.0,
            y: at.1,
            button: celestina_ui::BTN_LEFT,
            serial: 5,
        };
        pane.input(&pressed, settled + ms(2100));
        assert_eq!(
            pane.input(&released, settled + ms(2100)),
            MenuOutcome::Choose(5)
        );
        // A level that fits has no bar.
        let mut small = pane_of(level(), None, t0);
        assert!(!small.can_scroll());
        assert!(!small.wheel(30.0, t0));
        assert_eq!(small.thumb_rect(t0), None);
    }

    #[test]
    fn drilling_in_starts_the_new_level_at_the_top() {
        let t0 = Instant::now();
        let mut entries: Vec<MenuEntry> = (0..30).map(|i| entry(i, "row")).collect();
        entries.push(submenu(
            100,
            "Deep one",
            (200..240).map(|i| entry(i, "inner")).collect(),
        ));
        let mut pane = pane_in(
            MenuTree { id: 0, entries },
            None,
            PaneFrame::WINDOW,
            400.0,
            false,
            t0,
        );
        // Scroll down to the submenu's row by the keyboard and drill in.
        press(&mut pane, XKB_KEY_UP, t0);
        assert_eq!(focused(&pane), Some(entry_key(30)));
        assert!(pane.scroll.offset() > 0.0);
        let parent_offset = pane.scroll.offset();
        assert_eq!(press(&mut pane, RETURN, t0), MenuOutcome::Submenu(100));
        assert_eq!(pane.depth(), 1);
        assert_eq!(pane.scroll.offset(), 0.0, "the new level starts at the top");
        assert_eq!(focused(&pane), Some(entry_key(0)));
        assert!(pane.focused_rect().is_some_and(|r| r.y >= 0.0));
        // Down the sublevel, then back: the parent starts at the top and
        // reveals its focused row.
        for _ in 0..20 {
            press(&mut pane, XKB_KEY_DOWN, t0);
        }
        assert!(pane.scroll.offset() > 0.0);
        assert_eq!(press(&mut pane, XKB_KEY_LEFT, t0), MenuOutcome::Back);
        assert_eq!(focused(&pane), Some(entry_key(30)));
        assert_eq!(
            pane.scroll.offset(),
            parent_offset,
            "its row is in view again"
        );
        let r = pane.focused_rect().expect("the row");
        assert!(r.y >= 0.0 && r.y + r.h <= pane.size().1);
    }

    #[test]
    fn reduced_motion_drills_in_without_a_slide() {
        let t0 = Instant::now();
        let mut pane = pane_in(level(), None, PaneFrame::WINDOW, 496.0, true, t0);
        press(&mut pane, XKB_KEY_UP, t0);
        press(&mut pane, RETURN, t0);
        assert_eq!(pane.depth(), 1);
        assert_eq!(pane.slide_x(t0), (None, 0.0));
        assert!(!pane.is_animating(t0 + ms(1)));
        assert_eq!(
            press(&mut pane, XKB_KEY_LEFT, t0 + ms(2)),
            MenuOutcome::Back
        );
        assert_eq!(pane.slide_x(t0 + ms(2)), (None, 0.0));
        assert!(!pane.is_animating(t0 + ms(3)));
    }

    #[test]
    fn nan_and_huge_inputs_stay_finite() {
        let t0 = Instant::now();
        for max_h in [f32::NAN, 0.0, -5.0, f32::INFINITY, f32::MAX] {
            let mut pane = pane_in(
                level(),
                Some(ShellEntry::Hide),
                PaneFrame::WINDOW,
                max_h,
                false,
                t0,
            );
            let (w, h) = pane.size();
            assert!(w.is_finite() && h.is_finite() && h >= 0.0, "{max_h}: {h}");
            assert!(pane.content_h().is_finite());
            for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30] {
                pane.wheel(v, t0);
                assert!(pane.scroll.offset().is_finite());
            }
            press(&mut pane, XKB_KEY_UP, t0);
            if let Some(r) = pane.focused_rect() {
                assert!([r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()));
            }
            if let Some(r) = pane.thumb_rect(t0) {
                assert!([r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()));
            }
        }
        let frame = PaneFrame::card(f32::NAN);
        let pane = pane_in(level(), None, frame, 300.0, false, t0);
        assert!(pane.size().0.is_finite());
        let huge = PaneFrame {
            width: f32::INFINITY,
            pad_x: f32::NAN,
            pad_y: -3.0,
        };
        let pane = pane_in(level(), None, huge, 300.0, false, t0);
        assert!(pane.size().0.is_finite() && pane.size().1.is_finite());
        // The wheel of a pane that cannot scroll, and a NaN pointer, do nothing wrong.
        let mut pane = pane_of(level(), None, t0);
        let nan = InputEvent::PointerMotion {
            x: f32::NAN,
            y: f32::NAN,
        };
        assert!(matches!(pane.input(&nan, t0), MenuOutcome::Keep { .. }));
        assert!(!pane.wheel(f32::NAN, t0));
    }

    #[test]
    fn the_theme_can_change_under_an_open_pane() {
        let t0 = Instant::now();
        let mut pane = pane_of(deep(), Some(ShellEntry::Hide), t0);
        press(&mut pane, XKB_KEY_UP, t0);
        assert_eq!(focused(&pane), Some(entry_key(7)), "the shell entry");
        assert_eq!(
            press(&mut pane, XKB_KEY_RIGHT, t0),
            MenuOutcome::Keep { changed: false }
        );
        assert_eq!(pane.depth(), 0, "the shell entry has no submenu");
        let before = pane.size();
        pane.set_theme(&Theme::dark(), t0);
        assert_eq!(pane.size(), before, "the same theme changes nothing");
        let mut other = Theme::dark();
        other.radius_sm += 1.0;
        pane.set_theme(&other, t0 + ms(10));
        lay(&mut pane);
        assert_eq!(pane.size(), before);
        assert_eq!(focused(&pane), Some(entry_key(7)), "the focus stays");
        assert_eq!(pane.focused_rect().map(|r| r.h), Some(40.0));
        pane.focus_first();
        assert_eq!(focused(&pane), Some(entry_key(0)));
    }
}
