//! What a capsule sheet shows under its row: a body, laid out in its own
//! coordinates ((0, 0) is the body box's top-left) and painted translated to
//! where the sheet puts it: the level cards and the session menu.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Role, Theme, Weight};
use celestina_ui::{fit_middle, Interaction, Key, Tree, UiEvent, View};
use celestina_wayland::InputEvent;

use super::{tray_menu::TrayMenuBody, tray_overflow, workspaces, Measure};
use crate::{
    island::{
        pages::tray::{display_title, row_key, TrayRow},
        view::{sheet_more_key, text_style},
    },
    osd::popup::{LevelCore, Outcome},
    pieces::PageId,
    session_menu::{self, SessionAction, SessionCore},
    tray::{
        card::{card_cap, CardInput, CardSlot, CardToggle, CardView, MenuCard},
        menu::ShellEntry,
        pane::MenuOutcome,
        ItemId, MenuTree,
    },
};

/// An Island page shown as a capsule sheet (CEL-D-003): the page's tree and
/// its pointer and keyboard state, under a header the Island builds.
pub struct PageBody {
    page: PageId,
    theme: Theme,
    size: (f32, f32),
    /// The page content's natural size, which `size` was made from.
    content: (f32, f32),
    tree: Tree,
    input: Interaction,
}

impl std::fmt::Debug for PageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageBody")
            .field("page", &self.page)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl PageBody {
    /// The body of `page` showing `view`, `size` big for a page content of
    /// natural size `content`.
    #[must_use]
    pub fn new(
        page: PageId,
        theme: &Theme,
        reduced: bool,
        view: View,
        size: (f32, f32),
        content: (f32, f32),
        now: Instant,
    ) -> PageBody {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view, now);
        PageBody {
            page,
            theme: theme.clone(),
            size,
            content,
            tree,
            input: Interaction::new(),
        }
    }

    /// The page content's natural size the body was sized for.
    #[must_use]
    pub fn content(&self) -> (f32, f32) {
        self.content
    }

    /// The page this body shows.
    #[must_use]
    pub fn page(&self) -> PageId {
        self.page
    }

    /// Shows `view` at `size`: what the pointer and keyboard held stays.
    pub fn apply(&mut self, view: View, size: (f32, f32), content: (f32, f32), now: Instant) {
        self.size = size;
        self.content = content;
        self.tree.apply(view, now);
    }

    /// The tree and its input state, for what the Island does with the
    /// focused node.
    pub fn parts(&mut self) -> (&mut Tree, &mut Interaction) {
        (&mut self.tree, &mut self.input)
    }

    fn set_theme(&mut self, theme: &Theme) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        true
    }

    /// The node of `key`, as a body outcome: the overflow button asks for its
    /// menu, anything else is the page's to read.
    fn outcome_of(&self, key: Key, keyboard: bool) -> BodyOutcome {
        if key == sheet_more_key() {
            if let Some(id) = self.tree.node_by_key(key) {
                return BodyOutcome::More {
                    rect: self.tree.rect(id),
                    keyboard,
                };
            }
        }
        BodyOutcome::Activate { key, keyboard }
    }

    fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } | InputEvent::FocusLost => {
                self.input.key(&mut self.tree, ev)
            }
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::Escape => return BodyOutcome::Close,
                UiEvent::Click(key) => return self.outcome_of(key, false),
                UiEvent::Activate(key) => return self.outcome_of(key, true),
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        BodyOutcome::Keep { changed }
    }
}

/// The workspaces list shown as a capsule sheet (CEL-D-005): the list's tree
/// and its pointer and keyboard state. A row chosen is a workspace to focus.
pub struct WorkspacesBody {
    theme: Theme,
    size: (f32, f32),
    /// The workspaces the rows show, by id.
    ids: Vec<u64>,
    tree: Tree,
    input: Interaction,
}

impl std::fmt::Debug for WorkspacesBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspacesBody")
            .field("size", &self.size)
            .field("ids", &self.ids)
            .finish_non_exhaustive()
    }
}

impl WorkspacesBody {
    /// The body showing `view`, one row for each of `ids`.
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool, view: View, ids: Vec<u64>, now: Instant) -> Self {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view, now);
        WorkspacesBody {
            theme: theme.clone(),
            size: workspaces::list_size(ids.len()),
            ids,
            tree,
            input: Interaction::new(),
        }
    }

    /// Shows `view` again, one row for each of `ids`: what the pointer and
    /// keyboard held stays, the sheet follows the new height.
    pub fn apply(&mut self, view: View, ids: Vec<u64>, now: Instant) {
        self.size = workspaces::list_size(ids.len());
        self.ids = ids;
        self.tree.apply(view, now);
    }

    /// The workspace of the row `key`, when it is one.
    fn workspace_of(&self, key: Key) -> Option<u64> {
        self.ids
            .iter()
            .copied()
            .find(|&id| workspaces::row_key(id) == key)
    }

    fn set_theme(&mut self, theme: &Theme) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        true
    }

    fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } | InputEvent::FocusLost => {
                self.input.key(&mut self.tree, ev)
            }
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::Escape => return BodyOutcome::Close,
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(id) = self.workspace_of(key) {
                        return BodyOutcome::FocusWorkspace(id);
                    }
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        BodyOutcome::Keep { changed }
    }
}

/// The tray's hidden apps shown as a capsule sheet (CEL-I-012): the list's
/// tree and its pointer and keyboard state, and under one app's row the card
/// its menu unfolds as (CEL-D-016). A row chosen is the app's to activate;
/// the Island reads the row as it reads a bar icon.
pub struct TrayOverflowBody {
    theme: Theme,
    size: (f32, f32),
    /// The apps the rows show, with their place in the tray's list.
    rows: Vec<(usize, TrayRow)>,
    /// The titles the rows show, cut to the room each has.
    titles: Vec<String>,
    tree: Tree,
    input: Interaction,
    /// The tallest the window gets.
    room: f32,
    reduced: bool,
    /// The open app's menu under its row, and the other rows' blur.
    card: CardSlot,
    /// The card the tree was last built with.
    applied: Option<CardView>,
    /// The window's height moved with the card at the last sync: the glass
    /// follows it frame by frame.
    following: bool,
}

impl std::fmt::Debug for TrayOverflowBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayOverflowBody")
            .field("size", &self.size)
            .field("card", &self.card)
            .finish_non_exhaustive()
    }
}

impl TrayOverflowBody {
    /// The body showing one row for each of `rows`, in a window of at most
    /// `room` tall. The titles are whole until [`TrayOverflowBody::sync`]
    /// cuts them to the room they have.
    #[must_use]
    pub fn new(
        theme: &Theme,
        reduced: bool,
        rows: Vec<(usize, TrayRow)>,
        room: f32,
        now: Instant,
    ) -> Self {
        let mut body = TrayOverflowBody {
            theme: theme.clone(),
            size: tray_overflow::list_size(theme, 0),
            rows: Vec::new(),
            titles: Vec::new(),
            tree: Tree::new(reduced, theme),
            input: Interaction::new(),
            room,
            reduced,
            card: CardSlot::default(),
            applied: None,
            following: false,
        };
        body.apply(rows, now);
        body
    }

    /// Shows `rows` again: what the pointer and keyboard held stays, the sheet
    /// follows the new height. A title already cut for the same text keeps its
    /// cut until the next fit. A card whose app left the list goes at once.
    pub fn apply(&mut self, rows: Vec<(usize, TrayRow)>, now: Instant) {
        let titles = rows
            .iter()
            .map(|(index, row)| {
                let old = self.rows.iter().position(|(i, r)| {
                    i == index && r.title == row.title && r.status == row.status
                });
                match old.and_then(|at| self.titles.get(at)) {
                    Some(cut) => cut.clone(),
                    None => display_title(row).to_owned(),
                }
            })
            .collect();
        self.rows = rows;
        self.titles = titles;
        self.settle(now);
        self.rebuild(now);
    }

    /// The place of the app at `index` in the rows.
    fn position_of(&self, index: usize) -> Option<usize> {
        self.rows.iter().position(|(i, _)| *i == index)
    }

    /// The room a card has under the rows.
    fn cap(&self) -> f32 {
        card_cap(
            self.room,
            tray_overflow::list_size(&self.theme, self.rows.len()).1,
        )
    }

    /// The card the list shows now.
    fn card_view(&self, now: Instant) -> Option<CardView> {
        self.card.view(now, self.cap())
    }

    /// Drops a card that is gone or whose app left, keeps the card on its
    /// app when the app's place in the list moved, takes the room the rows
    /// leave it, and sets the rows' blur and the size from what remains.
    fn settle(&mut self, now: Instant) {
        if self.card.card().is_some_and(|card| card.gone(now)) {
            self.card.drop_card();
        }
        let rows = &self.rows;
        self.card
            .follow(|item| rows.iter().find(|(_, r)| r.id == *item).map(|(i, _)| *i));
        let cap = self.cap();
        if let Some(card) = self.card.card_mut() {
            card.set_cap(cap, now);
        }
        let open = self.card.open_row().and_then(|row| self.position_of(row));
        self.card
            .blur_rows(open, self.rows.len(), self.reduced, now);
        let view = self.card_view(now);
        let last = view.is_some_and(|v| self.rows.last().is_some_and(|(i, _)| *i == v.index));
        self.size = tray_overflow::list_size_with(
            &self.theme,
            self.rows.len(),
            view.map(|v| (v.box_h, last)),
        );
    }

    /// Builds the tree from the rows, the titles and the card.
    fn rebuild(&mut self, now: Instant) {
        let view = self.card_view(now);
        if let (Some(old), Some(new)) = (self.applied, view) {
            if old.index == new.index && old.box_h != new.box_h {
                // The rows under the card move with it, frame by frame: a
                // slide of their own would lag behind the window's glass.
                self.tree
                    .slide_with(Duration::from_millis(1), celestina_ui::ONE_UI.into(), &[]);
            }
        }
        let tree_view = tray_overflow::overflow_view(&self.theme, &self.rows, &self.titles, view);
        self.tree.apply(tree_view, now);
        self.applied = view;
    }

    /// Follows the card's clocks at `now`: drops what is gone, moves the
    /// size and builds the tree again when the card's box moved.
    fn refresh(&mut self, now: Instant) {
        self.settle(now);
        if self.card_view(now) != self.applied {
            self.rebuild(now);
        }
    }

    /// Cuts every title to the room its row leaves with `measure`; whether a
    /// title changed.
    pub fn fit(&mut self, now: Instant, measure: &mut Measure<'_>) -> bool {
        let open = self
            .card
            .card()
            .filter(|card| card.is_open())
            .map(MenuCard::row);
        let titles = tray_overflow::fit_titles(&self.theme, &self.rows, open, measure);
        if titles == self.titles {
            return false;
        }
        self.titles = titles;
        self.rebuild(now);
        true
    }

    /// Takes in a new theme, the card's clocks and the titles' and labels'
    /// cuts; whether the look changed.
    fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        let before = self.size;
        let themed = self.set_theme(theme, now);
        let mut changed = themed;
        if let Some(pane) = self.card.card_mut().and_then(MenuCard::pane_shown_mut) {
            changed |= pane.set_theme(theme, now);
            changed |= pane.fit_labels(measure, now);
        }
        self.refresh(now);
        changed |= self.fit(now, measure);
        changed |= self.card.card().is_some() && self.size != before;
        self.following = self.size != before || self.card.card().is_some_and(|c| c.moving(now));
        changed
    }

    fn set_theme(&mut self, theme: &Theme, _now: Instant) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        true
    }

    /// Hangs a waiting card from the row of the app at `index`, or folds the
    /// card that hangs there. Another app's card (waiting, open or folding)
    /// is removed at once.
    pub fn toggle_card(
        &mut self,
        index: usize,
        item: ItemId,
        keyboard: bool,
        reduced: bool,
        now: Instant,
    ) -> CardToggle {
        self.reduced = reduced;
        let toggle = self.card.toggle(index, item, keyboard, now);
        self.refresh(now);
        toggle
    }

    /// Gives the card of `item` the app's menu: it unfolds, or shows the
    /// republish in place. `false` when no card of this body holds `item`.
    pub fn arrive_card(
        &mut self,
        item: &ItemId,
        theme: &Theme,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        reduced: bool,
        now: Instant,
    ) -> bool {
        let cap = self.cap();
        let Some(card) = self
            .card
            .card_mut()
            .filter(|card| card.item() == item && card.held())
        else {
            return false;
        };
        self.reduced = reduced;
        card.arrive(theme, tree, shell, tray_overflow::CARD_W, cap, reduced, now);
        self.refresh(now);
        true
    }

    /// Removes the card at once (its app did not answer, or left).
    pub fn drop_card(&mut self, now: Instant) {
        if self.card.drop_card() {
            self.refresh(now);
        }
    }

    /// The tray menu the card holds: its item and whether the app has not
    /// answered. `None` with no card, or a folding one.
    #[must_use]
    pub fn held_card(&self) -> Option<(ItemId, bool)> {
        self.card
            .held()
            .map(|card| (card.item().clone(), card.is_waiting()))
    }

    /// The card's box in body coordinates, while a pane is open in it.
    fn card_rect(&self) -> Option<Rect> {
        self.card.card().filter(|card| card.is_open())?;
        let id = self.tree.node_by_key(tray_overflow::card_key())?;
        let rect = self.tree.rect(id);
        (rect.h > 1.0).then_some(rect)
    }

    fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let rect = self.card_rect();
        let (tree, input) = (&mut self.tree, &mut self.input);
        // The list's hover ends where the card's begins.
        let mut leave = || {
            input.pointer(tree, &InputEvent::PointerLeave, now);
        };
        let taken = self.card.input(ev, rect, now, &mut leave);
        let outcome = match taken {
            CardInput::List => None,
            CardInput::Taken | CardInput::Pane(MenuOutcome::Close) => {
                Some(BodyOutcome::Keep { changed: true })
            }
            CardInput::Pane(MenuOutcome::Keep { changed }) => Some(BodyOutcome::Keep { changed }),
            CardInput::Pane(other) => Some(BodyOutcome::TrayMenu(other)),
        };
        if let Some(outcome) = outcome {
            self.refresh(now);
            return outcome;
        }
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } | InputEvent::FocusLost => {
                self.input.key(&mut self.tree, ev)
            }
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::Escape => return BodyOutcome::Close,
                UiEvent::Click(key) => {
                    return BodyOutcome::Activate {
                        key,
                        keyboard: false,
                    }
                }
                UiEvent::Activate(key) => {
                    return BodyOutcome::Activate {
                        key,
                        keyboard: true,
                    }
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        BodyOutcome::Keep { changed }
    }

    /// Whether the wheel at (`x`, `y`) of the body is the card's.
    fn takes_wheel(&self, x: f32, y: f32) -> bool {
        self.card_rect().is_some_and(|r| r.contains(x, y))
    }

    /// Whether Escape is the card's (back, or fold) rather than the
    /// window's.
    fn takes_escape(&self) -> bool {
        self.card.held().is_some()
    }

    /// Whether the window's height follows the card at `now`.
    fn follows_height(&self, now: Instant) -> bool {
        self.following || self.card.card().is_some_and(|card| card.moving(now))
    }

    /// The focused row of the card's pane, in body coordinates.
    fn focused_rect(&self) -> Option<Rect> {
        let origin = self.card_rect()?;
        let inner = self.card.card()?.pane_ref()?.focused_rect()?;
        Some(Rect {
            x: origin.x + inner.x,
            y: origin.y + inner.y,
            ..inner
        })
    }

    fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        let size = self.size;
        lay(&mut self.tree, size);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "tray overflow sheet: layout fault");
        }
        if let Some(pane) = self.card.card_mut().and_then(MenuCard::pane_shown_mut) {
            pane.layout(lay);
        }
    }

    fn paint(&mut self, painter: &mut Painter, now: Instant) {
        let rows = self.rows.iter().map(|(index, _)| row_key(*index));
        let radius = self.theme.radius_md;
        self.card.paint(
            &mut self.tree,
            painter,
            now,
            rows,
            tray_overflow::card_key(),
            radius,
        );
    }

    /// The fill the row of the app at `index` shows, the pointer's lift not
    /// counted (tests).
    #[cfg(test)]
    #[must_use]
    pub fn row_fill(&self, index: usize) -> Option<Role> {
        let id = self.tree.node_by_key(row_key(index))?;
        self.tree.box_roles(id).0
    }

    /// The weight of the title of the row of the app at `index` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn title_weight_of(&self, index: usize) -> Option<Weight> {
        fn find(view: &View, key: Key) -> Option<&View> {
            if view.key == key {
                return Some(view);
            }
            view.children.iter().find_map(|c| find(c, key))
        }
        let list =
            tray_overflow::overflow_view(&self.theme, &self.rows, &self.titles, self.applied);
        match &find(&list, Key::indexed("tray-title", index as u64))?.kind {
            celestina_ui::ViewKind::Text { style, .. } => Some(style.weight),
            _ => None,
        }
    }

    /// The blur and opacity of the row at place `pos` at `now` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn blur_of(&self, pos: usize, now: Instant) -> (f32, f32) {
        self.card.look(pos, now)
    }

    /// The card's rect in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn card_box(&self) -> Option<Rect> {
        self.applied.filter(|c| c.box_h > 1.0)?;
        let id = self.tree.node_by_key(tray_overflow::card_key())?;
        Some(self.tree.rect(id))
    }

    /// Whether a card is in the body, whatever phase it is in (tests).
    #[cfg(test)]
    #[must_use]
    pub fn has_card(&self) -> bool {
        self.card.card().is_some()
    }

    /// The rect of a row of the card's pane, in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn card_row_rect(&self, key: Key) -> Option<Rect> {
        let origin = self.card_rect()?;
        let tree = self.card.card()?.pane_ref()?.tree();
        let r = tree.rect(tree.node_by_key(key)?);
        Some(Rect {
            x: origin.x + r.x,
            y: origin.y + r.y,
            ..r
        })
    }

    /// Whether the card's pane scrolls (tests).
    #[cfg(test)]
    #[must_use]
    pub fn card_can_scroll(&self) -> bool {
        self.card
            .card()
            .and_then(MenuCard::pane_ref)
            .is_some_and(crate::tray::pane::MenuPane::can_scroll)
    }

    /// How many submenus the card has open (tests).
    #[cfg(test)]
    #[must_use]
    pub fn card_depth(&self) -> usize {
        self.card
            .card()
            .and_then(MenuCard::pane_ref)
            .map_or(0, crate::tray::pane::MenuPane::depth)
    }

    /// Whether the card's pane has a focused row (tests).
    #[cfg(test)]
    #[must_use]
    pub fn card_focus(&self) -> Option<Rect> {
        self.focused_rect()
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now) || self.card.is_animating(now)
    }
}

/// A sheet's body.
#[derive(Debug)]
pub enum SheetBody {
    /// The volume's (or, later, the brightness') sliders and rows.
    Level(Box<LevelCore>),
    /// The session menu's rows and its confirmation card.
    Session(Box<SessionCore>),
    /// An Island page: its content under the subtitle, with the overflow button.
    Page(Box<PageBody>),
    /// The workspaces list: one row per workspace of the output.
    Workspaces(Box<WorkspacesBody>),
    /// The tray's hidden apps: one row per app the bar leaves out.
    TrayOverflow(Box<TrayOverflowBody>),
    /// A tray app's menu, from its bar icon (CEL-D-016).
    TrayMenu(Box<TrayMenuBody>),
}

/// What a body asks of its sheet after an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyOutcome {
    /// The sheet stays; `changed` when the body looks different.
    Keep { changed: bool },
    /// The body is done (Escape): the sheet closes.
    Close,
    /// A session row was chosen: the sheet closes and the Island posts it.
    Session(SessionAction),
    /// A node of a page body was clicked or activated (`keyboard`): the
    /// Island reads it as the page does.
    Activate { key: Key, keyboard: bool },
    /// A workspace row was chosen: the sheet closes and the Island posts it.
    FocusWorkspace(u64),
    /// The overflow button was chosen: `rect` is the button's, in body
    /// coordinates until [`super::Sheets::input`] moves it to the surface's.
    More { rect: Rect, keyboard: bool },
    /// The person chose something in a tray app's menu (CEL-D-016): an
    /// entry, the shell's entry, a submenu to ask the app about, or went
    /// back a level. The Island sends it for the item the body holds. The
    /// pane's own keep and close are [`BodyOutcome::Keep`] and
    /// [`BodyOutcome::Close`].
    TrayMenu(MenuOutcome),
}

/// The height of a session question wrapped at `width`, with `measure`.
fn question_h(measure: &mut Measure<'_>, text: &str, size: f32, width: f32) -> f32 {
    let style = text_style(size, Weight::Regular, false, Role::DangerFillInk);
    measure(text, &style, Some(width)).1
}

impl SheetBody {
    /// The body box's size, logical px.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        match self {
            SheetBody::Level(core) => core.size(),
            SheetBody::Session(core) => core.size(),
            SheetBody::Page(body) => body.size,
            SheetBody::Workspaces(body) => body.size,
            SheetBody::TrayOverflow(body) => body.size,
            SheetBody::TrayMenu(body) => body.size(),
        }
    }

    /// Whether the wheel at (`x`, `y`) of the body belongs to the body (a
    /// slider under the pointer) rather than to the sheet's scroll.
    #[must_use]
    pub fn takes_wheel(&self, x: f32, y: f32, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.slider_at(x, y, now).is_some(),
            SheetBody::TrayMenu(body) => body.takes_wheel(),
            SheetBody::TrayOverflow(body) => body.takes_wheel(x, y),
            SheetBody::Session(_) | SheetBody::Page(_) | SheetBody::Workspaces(_) => false,
        }
    }

    /// Whether the pointer is held on the body (a drag): every pointer event
    /// then belongs to it, wherever it is.
    #[must_use]
    pub fn held(&self) -> bool {
        match self {
            SheetBody::Level(core) => core.held(),
            SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => false,
        }
    }

    /// Sends what the body's throttle holds, now: the last thing the person
    /// chose is never lost to a close or a switch.
    pub fn flush(&mut self, now: Instant) {
        match self {
            SheetBody::Level(core) => {
                core.flush(now, true);
            }
            SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => {}
        }
    }

    /// Lays the body out in its own size with `lay`.
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        match self {
            SheetBody::Level(core) => core.layout_in(lay),
            SheetBody::Session(core) => core.layout_in(lay),
            SheetBody::Page(body) => {
                let size = body.size;
                lay(&mut body.tree, size);
                if let Some(fault) = body.tree.take_fault() {
                    tracing::warn!(page = ?body.page, %fault, "page sheet: layout fault");
                }
            }
            SheetBody::Workspaces(body) => {
                let size = body.size;
                lay(&mut body.tree, size);
                if let Some(fault) = body.tree.take_fault() {
                    tracing::warn!(%fault, "workspaces sheet: layout fault");
                }
            }
            SheetBody::TrayOverflow(body) => body.layout(lay),
            SheetBody::TrayMenu(body) => body.layout(lay),
        }
    }

    /// Paints the body at its own coordinates (the caller translates).
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        match self {
            SheetBody::Level(core) => core.paint(painter, now),
            SheetBody::Session(core) => core.paint(painter, now),
            SheetBody::Page(body) => body.tree.paint(painter, now),
            SheetBody::Workspaces(body) => body.tree.paint(painter, now),
            SheetBody::TrayOverflow(body) => body.paint(painter, now),
            SheetBody::TrayMenu(body) => body.paint(painter, now),
        }
    }

    /// The body's tree; `None` for a body that has none yet (a tray menu
    /// still waiting for its app).
    #[must_use]
    pub fn tree(&self) -> Option<&Tree> {
        match self {
            SheetBody::Level(core) => Some(core.tree()),
            SheetBody::Session(core) => Some(core.tree()),
            SheetBody::Page(body) => Some(&body.tree),
            SheetBody::Workspaces(body) => Some(&body.tree),
            SheetBody::TrayOverflow(body) => Some(&body.tree),
            SheetBody::TrayMenu(body) => body.tree(),
        }
    }

    /// Whether the body's own animations still run.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.is_animating(now),
            SheetBody::Session(core) => core.is_animating(now),
            SheetBody::Page(body) => body.tree.is_animating(now),
            SheetBody::Workspaces(body) => body.tree.is_animating(now),
            SheetBody::TrayOverflow(body) => body.is_animating(now),
            SheetBody::TrayMenu(body) => body.is_animating(now),
        }
    }

    /// How long until something the body holds is due (a throttled send, a
    /// level held while its monitor settles).
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        match self {
            SheetBody::Level(core) => [core.throttle_wait(now), core.graph_wait(now)]
                .into_iter()
                .flatten()
                .min(),
            SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => None,
        }
    }

    /// The rect of the body's node `key`, in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn node_rect(&self, key: celestina_ui::Key) -> Option<celestina_paint::Rect> {
        match self {
            SheetBody::Level(core) => core.node_rect(key),
            SheetBody::Session(core) => core.node_rect(key),
            SheetBody::Page(body) => body.tree.node_by_key(key).map(|id| body.tree.rect(id)),
            SheetBody::Workspaces(body) => body.tree.node_by_key(key).map(|id| body.tree.rect(id)),
            SheetBody::TrayOverflow(body) => {
                body.tree.node_by_key(key).map(|id| body.tree.rect(id))
            }
            SheetBody::TrayMenu(body) => body
                .tree()
                .and_then(|tree| tree.node_by_key(key).map(|id| tree.rect(id))),
        }
    }

    /// Whether Escape is the body's own (the session's confirmation card goes
    /// back to the list) rather than the sheet's close.
    #[must_use]
    pub fn takes_escape(&self) -> bool {
        match self {
            SheetBody::Level(_) => false,
            SheetBody::Session(core) => core.asking(),
            SheetBody::TrayMenu(body) => body.depth() > 0,
            SheetBody::TrayOverflow(body) => body.takes_escape(),
            SheetBody::Page(_) | SheetBody::Workspaces(_) => false,
        }
    }

    /// Whether the window's height follows the body's own motion at `now`
    /// (a tray card unfolding or folding): the window's rect is then the
    /// body's size frame by frame, with no resize morph of its own.
    #[must_use]
    pub fn follows_height(&self, now: Instant) -> bool {
        match self {
            SheetBody::TrayOverflow(body) => body.follows_height(now),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayMenu(_) => false,
        }
    }

    /// The tray menu the body holds, with whether the app has not answered:
    /// a bar icon's menu window, or the hidden-apps card.
    #[must_use]
    pub fn held_menu(&self) -> Option<(ItemId, bool)> {
        match self {
            SheetBody::TrayMenu(body) => Some((body.item().clone(), body.is_waiting())),
            SheetBody::TrayOverflow(body) => body.held_card(),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_) => None,
        }
    }

    /// Gives the tray menu of `item` the app's `tree` (and the shell's
    /// entry): the window opens to its size, the card unfolds, or the
    /// republish shows in place. `false` when the body holds no menu of
    /// `item`.
    pub fn arrive_menu(
        &mut self,
        item: &ItemId,
        theme: &Theme,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        reduced: bool,
        now: Instant,
    ) -> bool {
        match self {
            SheetBody::TrayMenu(body) if body.item() == item => {
                body.arrive(theme, tree, shell, reduced, now);
                true
            }
            SheetBody::TrayOverflow(body) => {
                body.arrive_card(item, theme, tree, shell, reduced, now)
            }
            SheetBody::TrayMenu(_)
            | SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_) => false,
        }
    }

    /// The rect of the row that holds the keyboard focus, in body
    /// coordinates: only a tray menu knows it.
    #[must_use]
    pub fn focused_rect(&self) -> Option<Rect> {
        match self {
            SheetBody::TrayMenu(body) => body.focused_rect(),
            SheetBody::TrayOverflow(body) => body.focused_rect(),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_) => None,
        }
    }

    /// The page the body shows, when it is an Island page.
    #[must_use]
    pub fn page(&self) -> Option<PageId> {
        match self {
            SheetBody::Page(body) => Some(body.page),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => None,
        }
    }

    /// The page body, when it is one.
    pub fn page_body_mut(&mut self) -> Option<&mut PageBody> {
        match self {
            SheetBody::Page(body) => Some(body),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => None,
        }
    }

    /// The workspaces body, when it is one.
    pub fn workspaces_body_mut(&mut self) -> Option<&mut WorkspacesBody> {
        match self {
            SheetBody::Workspaces(body) => Some(body),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::TrayOverflow(_)
            | SheetBody::TrayMenu(_) => None,
        }
    }

    /// The tray overflow body, when it is one.
    pub fn tray_overflow_body_mut(&mut self) -> Option<&mut TrayOverflowBody> {
        match self {
            SheetBody::TrayOverflow(body) => Some(body),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayMenu(_) => None,
        }
    }

    /// The tray menu body, when it is one.
    pub fn tray_menu_body_mut(&mut self) -> Option<&mut TrayMenuBody> {
        match self {
            SheetBody::TrayMenu(body) => Some(body),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_) => None,
        }
    }

    /// The tray menu body, when it is one.
    #[must_use]
    pub fn tray_menu_body(&self) -> Option<&TrayMenuBody> {
        match self {
            SheetBody::TrayMenu(body) => Some(body),
            SheetBody::Level(_)
            | SheetBody::Session(_)
            | SheetBody::Page(_)
            | SheetBody::Workspaces(_)
            | SheetBody::TrayOverflow(_) => None,
        }
    }

    /// Whether the body waits for what it will show (a tray menu the app
    /// has not answered): its window holds as a drop until it arrives.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        matches!(self, SheetBody::TrayMenu(body) if body.is_waiting())
    }

    /// Keeps `h` as the height of a waiting tray menu: the window it takes
    /// over stays at it until the app answers.
    pub fn hold_height(&mut self, h: f32) {
        if let SheetBody::TrayMenu(body) = self {
            body.hold_height(h);
        }
    }

    /// Whether the body waits and keeps the height of the window it took
    /// over.
    #[must_use]
    pub fn holds_height(&self) -> bool {
        matches!(self, SheetBody::TrayMenu(body) if body.holds_height())
    }

    /// Whether the body is the volume's or brightness' level card.
    #[must_use]
    pub fn is_level(&self) -> bool {
        matches!(self, SheetBody::Level(_))
    }

    /// Handles `ev` (already in body coordinates). Names are cut with
    /// `measure`, in hand for this call only.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> BodyOutcome {
        match self {
            SheetBody::Level(core) => {
                let style = core.name_style();
                let mut fit = |name: &str, width: f32| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                };
                match core.input(ev, now, &mut fit) {
                    Outcome::Close => BodyOutcome::Close,
                    Outcome::Keep { changed } => BodyOutcome::Keep { changed },
                }
            }
            SheetBody::Session(core) => {
                let mut fit = |text: &str, size, width| question_h(measure, text, size, width);
                match core.input(ev, now, &mut fit) {
                    session_menu::Outcome::Close(Some(action)) => BodyOutcome::Session(action),
                    session_menu::Outcome::Close(None) => BodyOutcome::Close,
                    session_menu::Outcome::Keep { changed } => BodyOutcome::Keep { changed },
                }
            }
            SheetBody::Page(body) => body.input(ev, now),
            SheetBody::Workspaces(body) => body.input(ev, now),
            SheetBody::TrayOverflow(body) => body.input(ev, now),
            SheetBody::TrayMenu(body) => body.input(ev, now),
        }
    }

    /// Takes in a new theme and the app's newest readings, sends what is due
    /// and says whether the look changed. Names are cut with `measure`.
    pub fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        match self {
            SheetBody::Level(core) => {
                core.cut_monitors(theme, now, |name, width, weight: Weight| {
                    let style = text_style(theme.type_scale.body, weight, false, Role::TextHi);
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                let style = text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi);
                let changed = core.sync(theme, now, |name, width| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                core.flush(now, false);
                changed
            }
            SheetBody::Session(core) => {
                let mut fit = |text: &str, size, width| question_h(measure, text, size, width);
                core.set_theme(theme, now, &mut fit)
            }
            SheetBody::Page(body) => body.set_theme(theme),
            SheetBody::Workspaces(body) => body.set_theme(theme),
            SheetBody::TrayOverflow(body) => body.sync(theme, now, measure),
            SheetBody::TrayMenu(body) => body.sync(theme, now, measure),
        }
    }
}
