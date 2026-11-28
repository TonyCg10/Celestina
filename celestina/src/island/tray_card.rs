//! The app menu card of the Island's Tray page (CEL-D-016, design §5.6): a
//! right click, the Menu key or an `ItemIsMenu` left click on a row unfolds
//! the app's menu under the row as the card the hidden-apps window uses
//! ([`MenuCard`]), the other rows blur by their distance from it, and a menu
//! that does not fit scrolls inside the card. The page keeps the card, the
//! rows' blur and the pointer's state; the page view only draws its slot
//! ([`crate::island::pages::tray::Tray`]). The one `CloseMenu` of the item is
//! the app's, when the card leaves [`Island::tray_menus_held`].

use std::time::Instant;

use celestina_paint::{Painter, Rect};
use celestina_wayland::InputEvent;

use super::{
    pages::tray::{self as tray_page, drawn_len, fitting_len},
    Island, IslandAction,
};
use crate::{
    sheet::{tray_overflow::card_key, Measure},
    tray::{
        card::{card_cap, CardInput, CardToggle, CardView, MenuCard},
        menu::ShellEntry,
        pane::MenuOutcome,
        ItemId, MenuTree, TrayCommand,
    },
};

impl Island {
    /// The rows the Tray page draws now.
    pub(super) fn tray_drawn(&self) -> usize {
        drawn_len(&self.page_cx())
    }

    /// The room the rows and a card share on the page, under the lead of a
    /// notice card above it.
    fn tray_room(&self) -> f32 {
        (self.state.max_page_size().1 - super::view::chrome_height(&self.theme) - self.lead)
            .max(0.0)
    }

    /// The most the box of a card under the page row `row` gets: the room
    /// under that row, less the margins around the box. Past it the menu
    /// scrolls inside the card.
    pub(super) fn tray_cap_at(&self, row: usize) -> f32 {
        card_cap(self.tray_room(), (row + 1) as f32 * self.theme.row_height)
    }

    /// The least room a card is opened in: one row of menu and its padding.
    pub(super) fn tray_card_min(&self) -> f32 {
        self.theme.row_height + 8.0
    }

    /// The cap of the card there is (0 with none).
    fn tray_card_cap(&self) -> f32 {
        self.tray_card
            .card()
            .map_or(0.0, |card| self.tray_cap_at(card.row()))
    }

    /// The card as the page view shows it at `now`: its box no taller than
    /// the room under its row, and the rows that do not fit under the box
    /// left out.
    pub(super) fn tray_card_view(&self, now: Instant) -> Option<CardView> {
        let cap = self.tray_card_cap();
        let mut view = self.tray_card.view(now, cap)?;
        if view.shown() {
            // The live box, unfolding or folding: a row leaves as the box
            // reaches it and comes back as the box leaves it.
            let rest = ((cap - view.box_h) / self.theme.row_height)
                .floor()
                .max(0.0);
            view.rows = view.index + 1 + rest as usize;
        }
        Some(view)
    }

    /// Whether the card's height moves at `now`: the page's size follows it
    /// frame by frame, with no resize motion of its own.
    pub(super) fn tray_card_follows(&self, now: Instant) -> bool {
        self.tray_card.card().is_some_and(|c| c.moving(now))
    }

    /// Whether the card unfolds for the first time at `now`: rows leave the
    /// page one by one as the box reaches them, and the page keeps the height
    /// it has until the unfold ends instead of dipping at each row.
    pub(super) fn tray_card_unfolding(&self, now: Instant) -> bool {
        self.tray_card.card().is_some_and(|c| c.unfolding(now))
    }

    /// Whether the card, its rows' blur or its pane need a frame at `now`.
    pub(super) fn tray_card_animating(&self, now: Instant) -> bool {
        self.tray_card.is_animating(now)
    }

    /// Removes the card at once; whether there was one.
    pub(super) fn drop_tray_card(&mut self) -> bool {
        let had = self.tray_card.drop_card();
        if had {
            self.stale = true;
        }
        had
    }

    /// Drops a card that is gone, whose app left, that has no room left or
    /// when the page is not the Tray's; keeps the card on its app when the
    /// app's row moved, takes the room under it, and sets the rows' blur from
    /// what remains.
    fn settle_tray_card(&mut self, now: Instant) {
        let shown = self.on_tray_page();
        if self
            .tray_card
            .card()
            .is_some_and(|card| card.gone(now) || !shown)
        {
            self.drop_tray_card();
        }
        // The app's row is looked for among every row the page could draw,
        // not the ones the card's own box leaves: an app that moved past
        // them keeps its card, which then goes only by the room rule.
        let (rows, fitting) = (&self.tray_rows, fitting_len(&self.page_cx()));
        let had = self.tray_card.card().is_some();
        if !self
            .tray_card
            .follow(|item| rows.iter().take(fitting).position(|r| r.id == *item))
            && had
        {
            self.stale = true;
        }
        // The room under the row the card has now.
        let (min, cap) = (self.tray_card_min(), self.tray_card_cap());
        if self
            .tray_card
            .card()
            .is_some_and(|card| card.is_open() && cap < min)
        {
            self.drop_tray_card();
        }
        let cap = self.tray_card_cap();
        if let Some(card) = self.tray_card.card_mut() {
            card.set_cap(cap, now);
        }
        let open = self.tray_card.open_row();
        let count = self.tray_drawn();
        self.tray_card.blur_rows(open, count, self.reduced, now);
    }

    /// The frame's part for the card: drops what is gone, takes in the theme
    /// and the labels' cuts, follows the pane's height and marks the views
    /// stale when the card's box moved.
    pub(super) fn tray_card_sync(&mut self, now: Instant, measure: &mut Measure<'_>) {
        self.settle_tray_card(now);
        if let Some(card) = self.tray_card.card_mut() {
            if let Some(pane) = card.pane_shown_mut() {
                pane.set_theme(&self.theme, now);
                pane.fit_labels(measure, now);
            }
            card.sync_target(now);
        }
        if self.tray_card_view(now) != self.tray_card_shown {
            self.stale = true;
        }
    }

    /// Hangs a waiting card from the row of the app at `index`, or folds the
    /// card that hangs there. Another app's card is removed at once, with no
    /// fold. `None` when the room under the row cannot hold a card: nothing
    /// is asked for.
    pub(super) fn toggle_tray_card(
        &mut self,
        index: usize,
        item: ItemId,
        keyboard: bool,
        now: Instant,
    ) -> Option<CardToggle> {
        let hangs_here = self.tray_card.held().is_some_and(|c| c.row() == index);
        if !hangs_here && self.tray_cap_at(index) < self.tray_card_min() {
            return None;
        }
        self.stale = true;
        let toggle = self.tray_card.toggle(index, item, keyboard, now);
        self.settle_tray_card(now);
        Some(toggle)
    }

    /// Whether the card holds `item`'s menu (waiting or open).
    pub(super) fn tray_card_holds(&self, item: &ItemId) -> bool {
        self.on_tray_page()
            && self
                .tray_card
                .held()
                .is_some_and(|card| card.item() == item)
    }

    /// Whether the card holds `item`'s menu and the app has answered.
    pub(super) fn tray_card_shows(&self, item: &ItemId) -> bool {
        self.tray_card_holds(item) && self.tray_card.card().is_some_and(MenuCard::is_open)
    }

    /// The card's item while it holds one (waiting or open).
    pub(super) fn tray_card_item(&self) -> Option<ItemId> {
        if !self.on_tray_page() {
            return None;
        }
        self.tray_card.held().map(|card| card.item().clone())
    }

    /// Gives the card of `item` the app's menu: it unfolds, or shows the
    /// republish in place. `false` when the card does not hold `item`.
    pub(super) fn arrive_tray_card(
        &mut self,
        item: &ItemId,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        now: Instant,
    ) -> bool {
        if !self.tray_card_holds(item) {
            return false;
        }
        let width = (self.state.page_size().0 - 2.0 * self.theme.spacing.lg).max(0.0);
        let cap = self.tray_card_cap();
        if cap < self.tray_card_min() {
            // No room for the menu's box: the card goes, and the app is let
            // go of with the rest.
            self.drop_tray_card();
            return true;
        }
        let (theme, reduced) = (self.theme.clone(), self.reduced);
        if let Some(card) = self.tray_card.card_mut() {
            card.arrive(&theme, tree, shell, width, cap, reduced, now);
        }
        self.settle_tray_card(now);
        self.stale = true;
        true
    }

    /// The card's box in the page, while a pane is open in it.
    fn tray_card_rect(&self) -> Option<Rect> {
        self.tray_card.card().filter(|card| card.is_open())?;
        let id = self.page.node_by_key(card_key())?;
        let rect = self.page.rect(id);
        (rect.h > 1.0).then_some(rect)
    }

    /// What the card does with `ev` while the Tray page is open: `None` when
    /// the event is the page's. An entry chosen goes to its app and folds the
    /// card, the page staying; a submenu asks the app.
    pub(super) fn tray_card_route(
        &mut self,
        ev: &InputEvent,
        now: Instant,
    ) -> Option<Vec<IslandAction>> {
        if !self.on_tray_page() {
            return None;
        }
        let rect = self.tray_card_rect();
        let item = self.tray_card.card()?.item().clone();
        let (page, input) = (&mut self.page, &mut self.page_input);
        // The page's hover ends where the card's begins.
        let mut leave = || {
            input.pointer(page, &InputEvent::PointerLeave, now);
        };
        let (actions, fold) = match self.tray_card.input(ev, rect, now, &mut leave) {
            CardInput::List => return None,
            CardInput::Taken
            | CardInput::Pane(MenuOutcome::Keep { .. } | MenuOutcome::Back | MenuOutcome::Close) => {
                (Vec::new(), false)
            }
            CardInput::Pane(MenuOutcome::Choose(id)) => (
                vec![IslandAction::Tray(TrayCommand::MenuEvent(item, id))],
                true,
            ),
            CardInput::Pane(MenuOutcome::Shell(entry)) => {
                (vec![IslandAction::TrayShellEntry(item, entry)], true)
            }
            CardInput::Pane(MenuOutcome::Submenu(id)) => (
                vec![IslandAction::Tray(TrayCommand::AboutToShow(item, id))],
                false,
            ),
        };
        if let Some(card) = self.tray_card.card_mut().filter(|_| fold) {
            card.fold(now);
        }
        self.settle_tray_card(now);
        self.stale = true;
        Some(actions)
    }

    /// Lays the card's pane out, after the page.
    pub(super) fn layout_tray_card(
        &mut self,
        lay: &mut dyn FnMut(&mut celestina_ui::Tree, (f32, f32)),
    ) {
        if let Some(pane) = self.tray_card.card_mut().and_then(MenuCard::pane_shown_mut) {
            pane.layout(lay);
        }
    }

    /// Paints the page's tree: the rows blurred by their distance from the
    /// open one, then the card's content in its box.
    pub(super) fn paint_page_tree(&mut self, painter: &mut Painter, now: Instant) {
        if !self.on_tray_page() {
            self.page.paint(painter, now);
            return;
        }
        let rows = (0..self.tray_drawn()).map(tray_page::row_key);
        let radius = self.theme.radius_md;
        self.tray_card
            .paint(&mut self.page, painter, now, rows, card_key(), radius);
    }
}
