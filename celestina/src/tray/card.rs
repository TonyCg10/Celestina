//! The card an app's menu unfolds as under its row (CEL-D-016, design §5.6):
//! the hidden-apps window's, and later the Tray page's. [`MenuCard`] holds the
//! item, the [`MenuPane`] once the app has answered and the card's own
//! motion (unfold on a spring with its content blurring in, fold, retarget
//! when a drill-in changes the height); [`RowBlurs`] holds how blurred the
//! other rows are while a card is open, and [`paint_with_row_blur`] paints
//! them so. The holder (the window, the page) owns the list and the rows'
//! looks; nothing here knows a window.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Theme};
use celestina_ui::{Easing, Key, Tree, ONE_UI};

use celestina_wayland::InputEvent;

use super::{
    menu::{ShellEntry, XKB_KEY_ESCAPE},
    pane::{MenuOutcome, MenuPane, PaneFrame},
    ItemId, MenuTree,
};
use crate::sheet::translated;

/// The card's unfold and fold last this long.
const CARD_MS: u64 = 350;
/// The rows' blur moves for this long.
const ROW_MS: u64 = 250;
/// The overshoot of the card's unfold spring.
const UNFOLD_OVERSHOOT: f32 = 1.15;
/// The blur of the card's content at the start of the unfold and the end of
/// the fold, in logical px.
const CONTENT_BLUR: f32 = 12.0;
/// The space above and below an open card (2 and 4 px).
pub(crate) const MARGINS: f32 = 6.0;

/// The blur of a row `distance` rows from the open one, in logical px.
#[must_use]
pub(crate) fn blur_level(distance: usize) -> f32 {
    match distance {
        0 => 0.0,
        1 => 1.5,
        2 => 3.0,
        _ => 6.0,
    }
}

/// The opacity of a row blurred by `sigma`: 60 % off at 6 px.
#[must_use]
pub(crate) fn blurred_opacity(sigma: f32) -> f32 {
    1.0 - 0.6 * sigma / 6.0
}

/// The room a card has under a list `list_h` tall in a window of at most
/// `max_h`, less the margins around it.
#[must_use]
pub(crate) fn card_cap(max_h: f32, list_h: f32) -> f32 {
    (max_h - list_h - MARGINS).max(0.0)
}

/// How blurred each row of a list is, moving over 250 ms from where it is.
#[derive(Clone, Debug, Default)]
pub(crate) struct RowBlurs {
    from: Vec<f32>,
    to: Vec<f32>,
    start: Option<Instant>,
}

impl RowBlurs {
    /// The sigma of row `index` at `now`.
    fn sigma(&self, index: usize, now: Instant) -> f32 {
        let to = self.to.get(index).copied().unwrap_or(0.0);
        let Some(start) = self.start else {
            return to;
        };
        let from = self.from.get(index).copied().unwrap_or(0.0);
        let t = now.saturating_duration_since(start).as_secs_f32() / (ROW_MS as f32 / 1000.0);
        from + (to - from) * ONE_UI.eval(t)
    }

    /// Blurs the `count` rows by their distance from the row `open` (none
    /// when `None`: all sharp), from the blur each has at `now`. Under
    /// `reduced` motion they change at once.
    pub(crate) fn set_open(
        &mut self,
        open: Option<usize>,
        count: usize,
        reduced: bool,
        now: Instant,
    ) {
        let to: Vec<f32> = (0..count)
            .map(|i| open.map_or(0.0, |o| blur_level(i.abs_diff(o))))
            .collect();
        if to == self.to {
            return;
        }
        self.from = (0..count).map(|i| self.sigma(i, now)).collect();
        self.to = to;
        self.start = (!reduced).then_some(now);
    }

    /// The blur and the opacity of row `index` at `now`.
    #[must_use]
    pub(crate) fn look(&self, index: usize, now: Instant) -> (f32, f32) {
        let sigma = self.sigma(index, now);
        (sigma, blurred_opacity(sigma))
    }

    /// Whether the rows are still moving.
    #[must_use]
    pub(crate) fn is_animating(&self, now: Instant) -> bool {
        self.start
            .is_some_and(|s| now.saturating_duration_since(s) < Duration::from_millis(ROW_MS))
    }
}

/// A length moving from `from` to `to`.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: f32,
    to: f32,
    start: Instant,
    dur: Duration,
    ease: Easing,
}

impl Glide {
    fn at(h: f32) -> Glide {
        Glide {
            from: h,
            to: h,
            start: Instant::now(),
            dur: Duration::ZERO,
            ease: Easing::Bezier(ONE_UI),
        }
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.dur.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f32() / self.dur.as_secs_f32())
            .clamp(0.0, 1.0)
    }

    fn value(&self, now: Instant) -> f32 {
        self.from + (self.to - self.from) * self.ease.eval(self.progress(now))
    }

    /// Whether it is still on its way (the instant it arrives counts).
    fn moving(&self, now: Instant) -> bool {
        !self.dur.is_zero() && now <= self.start + self.dur
    }
}

/// Where a card is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The app has not answered: no pane, no height.
    Waiting,
    /// Unfolding since this instant, then at rest.
    Open(Instant),
    /// Folding since this instant.
    Folding(Instant),
    /// Nothing left to show.
    Gone,
}

/// One app's menu under its row.
pub(crate) struct MenuCard {
    /// The app's place in the list the row is of.
    row: usize,
    item: ItemId,
    keyboard: bool,
    pane: Option<MenuPane>,
    phase: Phase,
    height: Glide,
    /// The height the pane wants (the glide's target while open).
    wanted: f32,
    /// The tallest the card gets: the pane scrolls past it.
    cap: f32,
    reduced: bool,
    /// How long a drill-in takes to retarget the height.
    retarget: Duration,
}

impl std::fmt::Debug for MenuCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuCard")
            .field("row", &self.row)
            .field("item", &self.item)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl MenuCard {
    /// The card of `item`, the app of row `row`, while the app has not
    /// answered. `keyboard` asks for the first entry focused on arrival.
    #[must_use]
    pub(crate) fn waiting(row: usize, item: ItemId, keyboard: bool) -> MenuCard {
        MenuCard {
            row,
            item,
            keyboard,
            pane: None,
            phase: Phase::Waiting,
            height: Glide::at(0.0),
            wanted: 0.0,
            cap: 0.0,
            reduced: false,
            retarget: Duration::from_millis(200),
        }
    }

    /// Takes in the app's menu: unfolds the card to the pane's height (at
    /// most `cap`) when it was waiting, and shows the republish in place
    /// when it is open. A folding or gone card takes nothing. Whether the
    /// look changed.
    pub(crate) fn arrive(
        &mut self,
        theme: &Theme,
        tree: MenuTree,
        shell: Option<ShellEntry>,
        width: f32,
        cap: f32,
        reduced: bool,
        now: Instant,
    ) -> bool {
        match self.phase {
            Phase::Waiting => {
                self.reduced = reduced;
                self.retarget = celestina_ui::Motion::Normal.duration(&theme.motion);
                let mut pane = MenuPane::new(
                    theme,
                    tree,
                    shell,
                    PaneFrame::card(width),
                    cap,
                    reduced,
                    now,
                );
                if self.keyboard {
                    pane.focus_first();
                }
                self.cap = cap;
                self.wanted = pane.size().1;
                self.pane = Some(pane);
                self.phase = Phase::Open(now);
                self.height = if reduced {
                    Glide {
                        from: self.wanted,
                        to: self.wanted,
                        ..Glide::at(0.0)
                    }
                } else {
                    Glide {
                        from: 0.0,
                        to: self.wanted,
                        start: now,
                        dur: Duration::from_millis(CARD_MS),
                        ease: Easing::OutBack(UNFOLD_OVERSHOOT),
                    }
                };
                true
            }
            Phase::Open(_) => {
                self.set_cap(cap, now);
                let changed = self
                    .pane
                    .as_mut()
                    .is_some_and(|pane| pane.replace(tree, shell, now));
                self.sync_target(now);
                changed
            }
            Phase::Folding(_) | Phase::Gone => false,
        }
    }

    /// Follows the pane's height (a drill-in or a republish changed it): the
    /// card's height retargets on `ONE_UI` over 200 ms from where it is.
    pub(crate) fn sync_target(&mut self, now: Instant) {
        if !matches!(self.phase, Phase::Open(_)) {
            return;
        }
        let Some(want) = self
            .pane
            .as_ref()
            .map(|pane| pane.content_h().min(self.cap))
        else {
            return;
        };
        if (want - self.wanted).abs() <= 0.01 {
            return;
        }
        let here = self.height.value(now);
        self.wanted = want;
        self.height = if self.reduced {
            Glide {
                from: want,
                to: want,
                ..Glide::at(0.0)
            }
        } else {
            Glide {
                from: here,
                to: want,
                start: now,
                dur: self.retarget,
                ease: Easing::Bezier(ONE_UI),
            }
        };
    }

    /// Folds the card: its height goes to 0 on `ONE_UI` over 350 ms with the
    /// content blurring out. A card that is not open yet, or under reduced
    /// motion, goes at once.
    pub(crate) fn fold(&mut self, now: Instant) {
        match self.phase {
            Phase::Open(_) if !self.reduced => {
                self.height = Glide {
                    from: self.height.value(now),
                    to: 0.0,
                    start: now,
                    dur: Duration::from_millis(CARD_MS),
                    ease: Easing::Bezier(ONE_UI),
                };
                self.phase = Phase::Folding(now);
            }
            Phase::Waiting | Phase::Open(_) => self.phase = Phase::Gone,
            Phase::Folding(_) | Phase::Gone => {}
        }
    }

    /// Whether nothing is left of the card at `now`.
    #[must_use]
    pub(crate) fn gone(&self, now: Instant) -> bool {
        match self.phase {
            Phase::Gone => true,
            Phase::Folding(start) => {
                now.saturating_duration_since(start) >= Duration::from_millis(CARD_MS)
            }
            Phase::Waiting | Phase::Open(_) => false,
        }
    }

    /// The card's box height at `now`, the margins not included.
    #[must_use]
    pub(crate) fn box_height(&self, now: Instant) -> f32 {
        match self.phase {
            Phase::Waiting | Phase::Gone => 0.0,
            Phase::Open(_) | Phase::Folding(_) => self.height.value(now).max(0.0),
        }
    }

    /// The content's opacity and blur at `now`.
    #[must_use]
    pub(crate) fn content(&self, now: Instant) -> (f32, f32) {
        let t = |start: Instant| {
            (now.saturating_duration_since(start).as_secs_f32() / (CARD_MS as f32 / 1000.0))
                .clamp(0.0, 1.0)
        };
        match self.phase {
            Phase::Waiting | Phase::Gone => (0.0, 0.0),
            Phase::Open(_) if self.reduced => (1.0, 0.0),
            Phase::Open(start) => {
                let t = t(start);
                let blur = if t >= 1.0 {
                    0.0
                } else {
                    CONTENT_BLUR * (1.0 - ONE_UI.eval(t))
                };
                (((t - 0.12) / 0.5).clamp(0.0, 1.0), blur)
            }
            Phase::Folding(start) => {
                let t = t(start);
                ((1.0 - t / 0.6).clamp(0.0, 1.0), CONTENT_BLUR * t)
            }
        }
    }

    /// The pane while the card is open and takes input.
    pub(crate) fn pane_mut(&mut self) -> Option<&mut MenuPane> {
        match self.phase {
            Phase::Open(_) => self.pane.as_mut(),
            Phase::Waiting | Phase::Folding(_) | Phase::Gone => None,
        }
    }

    /// The pane while the card is open.
    #[must_use]
    pub(crate) fn pane_ref(&self) -> Option<&MenuPane> {
        match self.phase {
            Phase::Open(_) => self.pane.as_ref(),
            Phase::Waiting | Phase::Folding(_) | Phase::Gone => None,
        }
    }

    /// The pane while it is on screen (open or folding): what is laid out
    /// and painted.
    pub(crate) fn pane_shown_mut(&mut self) -> Option<&mut MenuPane> {
        match self.phase {
            Phase::Open(_) | Phase::Folding(_) => self.pane.as_mut(),
            Phase::Waiting | Phase::Gone => None,
        }
    }

    /// The item whose menu this is.
    #[must_use]
    pub(crate) fn item(&self) -> &ItemId {
        &self.item
    }

    /// The row the card hangs from.
    #[must_use]
    pub(crate) fn row(&self) -> usize {
        self.row
    }

    /// Moves the card to `row`: its app's place in the list changed.
    pub(crate) fn set_row(&mut self, row: usize) {
        self.row = row;
    }

    /// Takes the room the list leaves now: the pane scrolls past it, and an
    /// open card's height glides to it when the menu is taller. A waiting
    /// card keeps it for the answer.
    pub(crate) fn set_cap(&mut self, cap: f32, now: Instant) {
        let cap = if cap.is_finite() { cap.max(0.0) } else { 0.0 };
        if (cap - self.cap).abs() <= 0.01 {
            return;
        }
        self.cap = cap;
        if let Some(pane) = self.pane.as_mut() {
            pane.set_max_h(cap);
        }
        self.sync_target(now);
    }

    /// Whether the room the list leaves can show a box at all: a card
    /// without it takes no key.
    #[must_use]
    pub(crate) fn has_room(&self) -> bool {
        self.cap > 1.0
    }

    /// Whether the card is in its first unfold at `now` (the instant it ends
    /// counts): its box grows from 0 to the pane's height.
    #[must_use]
    pub(crate) fn unfolding(&self, now: Instant) -> bool {
        match self.phase {
            Phase::Open(start) => {
                !self.reduced
                    && now.saturating_duration_since(start) <= Duration::from_millis(CARD_MS)
            }
            Phase::Waiting | Phase::Folding(_) | Phase::Gone => false,
        }
    }

    /// Whether the app has not answered yet.
    #[must_use]
    pub(crate) fn is_waiting(&self) -> bool {
        self.phase == Phase::Waiting
    }

    /// Whether the card is open (answered, not folding): what the Island
    /// holds the item for.
    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        matches!(self.phase, Phase::Open(_))
    }

    /// Whether the card holds its item's menu: waiting or open.
    #[must_use]
    pub(crate) fn held(&self) -> bool {
        self.is_waiting() || self.is_open()
    }

    /// Whether the card's height is moving at `now` (the window's glass
    /// follows it frame by frame, with no resize morph).
    #[must_use]
    pub(crate) fn moving(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Open(_) | Phase::Folding(_)) && self.height.moving(now)
    }

    /// Whether a frame is due: the height moves, the content blurs or the
    /// pane animates.
    #[must_use]
    pub(crate) fn is_animating(&self, now: Instant) -> bool {
        let content = match self.phase {
            Phase::Open(start) => {
                !self.reduced
                    && now.saturating_duration_since(start) < Duration::from_millis(CARD_MS)
            }
            Phase::Folding(_) => true,
            Phase::Waiting | Phase::Gone => false,
        };
        self.moving(now)
            || content
            || self.pane.as_ref().is_some_and(|pane| match self.phase {
                Phase::Open(_) | Phase::Folding(_) => pane.is_animating(now),
                Phase::Waiting | Phase::Gone => false,
            })
    }
}

/// The least blur of a card's content worth an offscreen layer, logical px.
const MIN_CARD_BLUR_PX: f32 = 0.05;

/// Paints the open card's content in its `rect` (the box the holder's tree
/// laid out, at its painted place): clipped to the box's corners `radius`,
/// under the card's opacity and blur at `now`. Nothing for a card with no
/// pane on screen or no height.
pub(crate) fn paint_card_content(
    painter: &mut Painter,
    card: &mut MenuCard,
    rect: Rect,
    radius: f32,
    now: Instant,
) {
    let (alpha, blur) = card.content(now);
    let Some(pane) = card.pane_shown_mut() else {
        return;
    };
    if alpha <= 0.0 || rect.h <= 1.0 {
        return;
    }
    painter.push_clip(rect, radius);
    painter.push_state();
    painter.opacity(alpha);
    let mut draw = |p: &mut Painter| {
        p.push_state();
        p.translate(rect.x, rect.y);
        pane.paint(p, now);
        p.pop_state();
    };
    if blur > MIN_CARD_BLUR_PX {
        painter.blurred(grown(rect, 3.0 * blur), blur, &mut draw);
    } else {
        draw(painter);
    }
    painter.pop_state();
    painter.pop_clip();
}

/// Paints `tree` with the rows `rows` blurred: each is `(row key, sigma,
/// opacity)`. The tree is painted without them and each one is painted
/// alone through [`Painter::blurred`] over its rect grown by 3 sigma, at its
/// opacity. A row with no blur and full opacity is painted with the rest.
pub(crate) fn paint_with_row_blur(
    tree: &mut Tree,
    painter: &mut Painter,
    now: Instant,
    rows: &[(Key, f32, f32)],
) {
    let blurred: Vec<(Key, f32, f32)> = rows
        .iter()
        .copied()
        .filter(|&(_, sigma, opacity)| sigma > 0.0 || opacity < 1.0)
        .collect();
    let hidden: Vec<Key> = blurred.iter().map(|r| r.0).collect();
    tree.paint_hiding(painter, now, &hidden);
    for (key, sigma, opacity) in blurred {
        let Some(id) = tree.node_by_key(key) else {
            continue;
        };
        let rect = tree.painted_rect(id, now);
        painter.push_state();
        painter.opacity(opacity.clamp(0.0, 1.0));
        painter.blurred(grown(rect, 3.0 * sigma), sigma, &mut |p| {
            tree.paint_node(p, now, key);
        });
        painter.pop_state();
    }
}

/// `rect` grown by `by` on every side.
fn grown(rect: Rect, by: f32) -> Rect {
    Rect {
        x: rect.x - by,
        y: rect.y - by,
        w: rect.w + 2.0 * by,
        h: rect.h + 2.0 * by,
    }
}

/// The card a list shows under the row of the app at `index`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardView {
    /// The app's place in the tray's list: its row's key carries it.
    pub index: usize,
    /// Whether the card holds its app's menu (waiting or open): the row
    /// shows its grey. A folding card does not.
    pub held: bool,
    /// Whether the app has answered and the card is open: the row then shows
    /// its title in semibold.
    pub answered: bool,
    /// The box's height now; a box of 1 px or less is not shown.
    pub box_h: f32,
    /// The most rows the list draws while the box shows (`usize::MAX`: all
    /// of them). A list that cannot hold the rows under the box and the box
    /// leaves the last ones out.
    pub rows: usize,
}

impl CardView {
    /// Whether the holder has the box in it.
    pub(crate) fn shown(&self) -> bool {
        self.box_h > 1.0
    }
}

/// What toggling an app's card did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardToggle {
    /// A waiting card hangs from the row now: the app is to be asked for
    /// its menu.
    Asked,
    /// The row's card was open (or waiting) and folds.
    Folded,
}

/// What the card did with an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CardInput {
    /// The event is the list's.
    List,
    /// The card took it (a waiting card folded on Escape).
    Taken,
    /// The open card's pane took it and said this; a `Close` has folded the
    /// card already.
    Pane(MenuOutcome),
}

/// The app menu card of a list of app rows (the hidden-apps window, the Tray
/// page), with the other rows' blur and whether the pointer is over its
/// pane. The holder owns the rows, the tree and what an outcome means.
#[derive(Debug, Default)]
pub(crate) struct CardSlot {
    card: Option<MenuCard>,
    blurs: RowBlurs,
    /// The pointer is over the card's pane.
    hot: bool,
}

impl CardSlot {
    /// The card, whatever phase it is in.
    #[must_use]
    pub(crate) fn card(&self) -> Option<&MenuCard> {
        self.card.as_ref()
    }

    #[must_use]
    pub(crate) fn card_mut(&mut self) -> Option<&mut MenuCard> {
        self.card.as_mut()
    }

    /// The card while it holds its app's menu (waiting or open).
    #[must_use]
    pub(crate) fn held(&self) -> Option<&MenuCard> {
        self.card.as_ref().filter(|card| card.held())
    }

    /// The row of the card while it is open (the app answered): the rows
    /// blur by their distance from it. A waiting card keeps its press fill
    /// and a folding one lets the rows sharpen.
    #[must_use]
    pub(crate) fn open_row(&self) -> Option<usize> {
        self.card
            .as_ref()
            .filter(|card| card.is_open())
            .map(MenuCard::row)
    }

    /// Keeps the card on its app: `row_of` says where the list has the
    /// app's row now. The card moves there; an app with no row takes the card
    /// at once. Whether a card remains.
    pub(crate) fn follow(&mut self, row_of: impl FnOnce(&ItemId) -> Option<usize>) -> bool {
        let Some(card) = self.card.as_mut() else {
            return false;
        };
        match row_of(card.item()) {
            Some(row) => {
                card.set_row(row);
                true
            }
            None => {
                self.drop_card();
                false
            }
        }
    }

    /// The card as the list shows it at `now`, its box no taller than `cap`.
    #[must_use]
    pub(crate) fn view(&self, now: Instant, cap: f32) -> Option<CardView> {
        self.card.as_ref().map(|card| CardView {
            index: card.row(),
            held: card.held(),
            answered: card.is_open(),
            box_h: card.box_height(now).min(cap),
            rows: usize::MAX,
        })
    }

    /// Hangs a waiting card from the row of the app at `index`, or folds the
    /// card that hangs there. Another app's card (waiting, open or folding)
    /// is removed at once.
    pub(crate) fn toggle(
        &mut self,
        index: usize,
        item: ItemId,
        keyboard: bool,
        now: Instant,
    ) -> CardToggle {
        if let Some(card) = self.card.as_mut() {
            if card.row() == index && card.held() {
                card.fold(now);
                return CardToggle::Folded;
            }
        }
        self.card = Some(MenuCard::waiting(index, item, keyboard));
        self.hot = false;
        CardToggle::Asked
    }

    /// Removes the card at once; whether there was one.
    pub(crate) fn drop_card(&mut self) -> bool {
        self.hot = false;
        self.card.take().is_some()
    }

    /// Blurs the rows by their distance from the held card's, at `open` of
    /// `count` rows (`None`: every row goes sharp).
    pub(crate) fn blur_rows(
        &mut self,
        open: Option<usize>,
        count: usize,
        reduced: bool,
        now: Instant,
    ) {
        self.blurs.set_open(open, count, reduced, now);
    }

    /// The blur and opacity of the row at place `index` at `now` (tests).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn look(&self, index: usize, now: Instant) -> (f32, f32) {
        self.blurs.look(index, now)
    }

    /// Whether the card, its pane or the rows' blur need a frame at `now`.
    #[must_use]
    pub(crate) fn is_animating(&self, now: Instant) -> bool {
        self.blurs.is_animating(now) || self.card.as_ref().is_some_and(|c| c.is_animating(now))
    }

    /// Gives `ev` to the card: a waiting card folds on Escape; an open card
    /// takes every key, and the pointer and the wheel over `rect` (its box
    /// in the list's coordinates). `leave` ends the list's hover when the
    /// pointer enters the box.
    pub(crate) fn input(
        &mut self,
        ev: &InputEvent,
        rect: Option<Rect>,
        now: Instant,
        leave: &mut dyn FnMut(),
    ) -> CardInput {
        let Some(card) = self.card.as_mut() else {
            return CardInput::List;
        };
        if card.is_waiting() {
            if matches!(
                ev,
                InputEvent::Key { keysym, pressed: true, .. } if *keysym == XKB_KEY_ESCAPE
            ) {
                card.fold(now);
                return CardInput::Taken;
            }
            return CardInput::List;
        }
        if !card.is_open() || !card.has_room() {
            return CardInput::List;
        }
        match *ev {
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } => {
                return CardInput::Pane(self.pane_event(ev, now));
            }
            InputEvent::Axis { x, y, .. }
            | InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. } => {
                if let Some(r) = rect.filter(|r| r.contains(x, y)) {
                    if !std::mem::replace(&mut self.hot, true) {
                        leave();
                    }
                    return CardInput::Pane(self.pane_event(&translated(ev, r.x, r.y), now));
                }
            }
            _ => {}
        }
        if std::mem::take(&mut self.hot) {
            self.pane_event(&InputEvent::PointerLeave, now);
        }
        CardInput::List
    }

    /// Gives `ev` (in the pane's coordinates) to the open card's pane; a
    /// `Close` (Escape at its root) folds the card.
    fn pane_event(&mut self, ev: &InputEvent, now: Instant) -> MenuOutcome {
        let Some(card) = self.card.as_mut() else {
            return MenuOutcome::Keep { changed: false };
        };
        let Some(outcome) = card.pane_mut().map(|pane| pane.input(ev, now)) else {
            return MenuOutcome::Keep { changed: false };
        };
        card.sync_target(now);
        if outcome == MenuOutcome::Close {
            card.fold(now);
        }
        outcome
    }

    /// Paints `tree` with its rows `rows` (each `(key, place)`) blurred by
    /// their distance from the card, then the card's content in the box
    /// keyed `card_key`, with corners of `radius`.
    pub(crate) fn paint(
        &mut self,
        tree: &mut Tree,
        painter: &mut Painter,
        now: Instant,
        rows: impl Iterator<Item = Key>,
        card_key: Key,
        radius: f32,
    ) {
        let looks: Vec<(Key, f32, f32)> = rows
            .enumerate()
            .map(|(pos, key)| {
                let (sigma, opacity) = self.blurs.look(pos, now);
                (key, sigma, opacity)
            })
            .collect();
        paint_with_row_blur(tree, painter, now, &looks);
        let (Some(id), Some(card)) = (tree.node_by_key(card_key), self.card.as_mut()) else {
            return;
        };
        let rect = tree.painted_rect(id, now);
        paint_card_content(painter, card, rect, radius, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::MenuEntry;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn item() -> ItemId {
        ItemId {
            bus_name: ":1.7".to_owned(),
            path: "/StatusNotifierItem".to_owned(),
        }
    }

    fn menu(n: i32) -> MenuTree {
        MenuTree {
            id: 0,
            entries: (1..=n)
                .map(|i| MenuEntry {
                    id: i,
                    label: format!("Entry {i}"),
                    enabled: true,
                    ..MenuEntry::default()
                })
                .collect(),
        }
    }

    fn open_card(n: i32, cap: f32, reduced: bool, now: Instant) -> MenuCard {
        let mut card = MenuCard::waiting(2, item(), false);
        assert!(card.is_waiting());
        assert_eq!(card.box_height(now), 0.0);
        assert!(card.arrive(&Theme::dark(), menu(n), None, 264.0, cap, reduced, now));
        card
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn rows_blur_by_distance_with_their_opacity() {
        let levels: Vec<f32> = (0..5).map(blur_level).collect();
        assert_eq!(levels, [0.0, 1.5, 3.0, 6.0, 6.0]);
        let opacities: Vec<f32> = levels.iter().map(|&s| blurred_opacity(s)).collect();
        for (got, want) in opacities.iter().zip([1.0, 0.85, 0.7, 0.4, 0.4]) {
            assert!(near(*got, want), "{got} vs {want}");
        }
        // Row 1 of 6 open: distances 1, 0, 1, 2, 3, 4.
        let t0 = Instant::now();
        let mut rows = RowBlurs::default();
        rows.set_open(Some(1), 6, true, t0);
        let sigmas: Vec<f32> = (0..6).map(|i| rows.look(i, t0).0).collect();
        assert_eq!(sigmas, [1.5, 0.0, 1.5, 3.0, 6.0, 6.0]);
        assert!(near(rows.look(4, t0).1, 0.4));
        assert!(!rows.is_animating(t0), "reduced motion is at once");
    }

    #[test]
    fn row_blur_moves_over_250_ms_and_clears_on_fold() {
        let t0 = Instant::now();
        let mut rows = RowBlurs::default();
        rows.set_open(Some(0), 3, false, t0);
        assert_eq!(rows.look(2, t0).0, 0.0, "starts from sharp");
        assert!(rows.is_animating(t0 + ms(100)));
        let mid = rows.look(2, t0 + ms(125)).0;
        assert!(near(mid, 3.0 * ONE_UI.eval(0.5)), "{mid}");
        assert!(near(rows.look(2, t0 + ms(250)).0, 3.0));
        assert!(!rows.is_animating(t0 + ms(250)));
        // The fold sharpens them from where they are, with the same curve.
        let t1 = t0 + ms(400);
        rows.set_open(None, 3, false, t1);
        assert!(near(rows.look(2, t1).0, 3.0));
        let mid = rows.look(2, t1 + ms(125)).0;
        assert!(near(mid, 3.0 * (1.0 - ONE_UI.eval(0.5))), "{mid}");
        assert!(near(rows.look(2, t1 + ms(250)).0, 0.0));
        assert!(near(rows.look(2, t1 + ms(250)).1, 1.0));
        // Changing the target mid-way starts from the blur it has.
        let mut rows = RowBlurs::default();
        rows.set_open(Some(0), 3, false, t0);
        let here = rows.look(2, t0 + ms(125)).0;
        rows.set_open(None, 3, false, t0 + ms(125));
        assert!(near(rows.look(2, t0 + ms(125)).0, here));
    }

    #[test]
    fn the_card_unfolds_on_the_spring_with_its_content_blurred_in() {
        let t0 = Instant::now();
        let card = open_card(3, 260.0, false, t0);
        // 3 rows of 40 and 4 px of padding over and under.
        let want = 3.0 * 40.0 + 8.0;
        let spring = Easing::OutBack(1.15);
        for n in [0u64, 50, 100, 175, 250, 350] {
            let t = n as f32 / 350.0;
            let h = card.box_height(t0 + ms(n));
            assert!(near(h, want * spring.eval(t)), "{n} ms: {h}");
        }
        assert!(card.box_height(t0 + ms(250)) > want, "it overshoots");
        let (op0, blur0) = card.content(t0);
        assert_eq!((op0, blur0), (0.0, 12.0));
        assert_eq!(card.content(t0 + ms(42)).0, 0.0, "opacity 0 up to t = 0.12");
        assert!(near(card.content(t0 + ms(350)).1, 0.0));
        assert_eq!(card.content(t0 + ms(350)).0, 1.0);
        assert_eq!(card.content(t0 + ms(217)).0, 1.0, "full by t = 0.62");
        assert!(card.content(t0 + ms(100)).0 > 0.0 && card.content(t0 + ms(100)).0 < 1.0);
        let mid = card.content(t0 + ms(175)).1;
        assert!(near(mid, 12.0 * (1.0 - ONE_UI.eval(0.5))), "{mid}");
        assert!(card.is_animating(t0 + ms(300)));
        assert!(!card.is_animating(t0 + ms(351)), "at rest asks for nothing");
        assert_eq!(card.content(t0 + ms(900)), (1.0, 0.0));
        assert!(near(card.box_height(t0 + ms(900)), want));
    }

    #[test]
    fn the_card_folds_in_350_ms() {
        let t0 = Instant::now();
        let mut card = open_card(3, 260.0, false, t0);
        let t1 = t0 + ms(1000);
        let h = card.box_height(t1);
        card.fold(t1);
        assert!(!card.is_open() && !card.held() && !card.gone(t1));
        assert!(card.pane_mut().is_none(), "a folding card takes no input");
        assert!(card.pane_shown_mut().is_some(), "but it is still drawn");
        for n in [0u64, 100, 200, 349] {
            let want = h * (1.0 - ONE_UI.eval(n as f32 / 350.0));
            assert!(near(card.box_height(t1 + ms(n)), want), "{n} ms");
        }
        let (op, blur) = card.content(t1 + ms(105));
        assert!(near(blur, 12.0 * 0.3), "{blur}");
        assert!(near(op, 1.0 - 0.3 / 0.6), "{op}");
        assert!(
            near(card.content(t1 + ms(210)).0, 0.0),
            "opacity 0 by 210 ms"
        );
        assert!(!card.gone(t1 + ms(349)));
        assert!(card.gone(t1 + ms(350)));
        assert!(card.is_animating(t1 + ms(200)));
        // A folding card takes no menu.
        assert!(!card.arrive(&Theme::dark(), menu(1), None, 264.0, 260.0, false, t1));
    }

    #[test]
    fn the_card_is_capped_by_the_room_the_list_leaves() {
        // A list of 230 px on the 560 surface (496 for windows): 260 left.
        assert_eq!(card_cap(496.0, 230.0), 260.0);
        assert_eq!(card_cap(100.0, 230.0), 0.0);
        let t0 = Instant::now();
        let mut card = open_card(30, card_cap(496.0, 230.0), false, t0);
        let t1 = t0 + ms(1000);
        assert!(near(card.box_height(t1), 260.0), "{}", card.box_height(t1));
        let pane = card.pane_mut().expect("the pane");
        assert!(pane.can_scroll(), "past the cap the pane scrolls");
        assert_eq!(pane.size(), (264.0, 260.0));
        assert!(pane.content_h() > 600.0 - 100.0);
    }

    #[test]
    fn a_drill_in_retargets_the_height_on_200_ms() {
        let t0 = Instant::now();
        let mut card = open_card(1, 260.0, false, t0);
        let t1 = t0 + ms(1000);
        let before = card.box_height(t1);
        // The republish brings more entries: the height glides to them.
        assert!(card.arrive(&Theme::dark(), menu(4), None, 264.0, 260.0, false, t1));
        let want = 4.0 * 40.0 + 8.0;
        assert!(near(card.box_height(t1), before));
        let mid = card.box_height(t1 + ms(100));
        assert!(
            near(mid, before + (want - before) * ONE_UI.eval(0.5)),
            "{mid}"
        );
        assert!(near(card.box_height(t1 + ms(200)), want));
        assert!(card.moving(t1 + ms(150)));
        assert!(!card.moving(t1 + ms(201)));
        // Its content does not blur for a drill-in.
        assert_eq!(card.content(t1 + ms(50)), (1.0, 0.0));
    }

    #[test]
    fn a_slot_follows_its_app_to_a_new_row_and_goes_when_the_app_has_none() {
        let t0 = Instant::now();
        let mut slot = CardSlot::default();
        assert!(!slot.follow(|_| Some(0)), "no card, nothing follows");
        slot.toggle(5, item(), false, t0);
        assert!(slot.follow(|i| (*i == item()).then_some(3)));
        assert_eq!(slot.card().map(MenuCard::row), Some(3));
        assert_eq!(slot.view(t0, 100.0).map(|v| v.index), Some(3));
        assert!(!slot.follow(|_| None));
        assert!(slot.card().is_none(), "an app with no row takes its card");
    }

    #[test]
    fn the_open_row_is_the_answered_cards() {
        let t0 = Instant::now();
        let mut slot = CardSlot::default();
        assert_eq!(slot.open_row(), None);
        slot.toggle(2, item(), false, t0);
        assert_eq!(slot.open_row(), None, "waiting: no blur yet");
        if let Some(c) = slot.card_mut() {
            c.arrive(&Theme::dark(), menu(2), None, 264.0, 260.0, false, t0);
        }
        assert_eq!(slot.open_row(), Some(2));
        if let Some(c) = slot.card_mut() {
            c.fold(t0 + ms(1000));
        }
        assert_eq!(slot.open_row(), None, "a folding card sharpens the rows");
    }

    #[test]
    fn an_open_card_takes_the_cap_it_is_given_and_a_card_with_none_takes_no_keys() {
        let t0 = Instant::now();
        let mut card = open_card(30, 260.0, false, t0);
        let t1 = t0 + ms(1000);
        assert!(near(card.box_height(t1), 260.0));
        // The room shrinks: the card's height glides to it.
        card.set_cap(120.0, t1);
        assert!(near(card.box_height(t1), 260.0));
        assert!(near(card.box_height(t1 + ms(200)), 120.0));
        assert_eq!(card.pane_ref().map(|p| p.size().1), Some(120.0));
        // And grows back.
        card.set_cap(200.0, t1 + ms(300));
        assert!(near(card.box_height(t1 + ms(600)), 200.0));
        // A card with no room takes no key.
        let mut slot = CardSlot::default();
        slot.toggle(0, item(), false, t0);
        if let Some(c) = slot.card_mut() {
            c.arrive(&Theme::dark(), menu(3), None, 264.0, 0.0, false, t0);
        }
        let down = InputEvent::Key {
            keysym: 0xff54,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        };
        let mut leave = || {};
        assert_eq!(slot.input(&down, None, t1, &mut leave), CardInput::List);
        if let Some(c) = slot.card_mut() {
            c.set_cap(100.0, t1);
        }
        assert!(matches!(
            slot.input(&down, None, t1, &mut leave),
            CardInput::Pane(_)
        ));
    }

    #[test]
    fn reduced_motion_unfolds_and_folds_at_once() {
        let t0 = Instant::now();
        let mut card = open_card(3, 260.0, true, t0);
        let want = 3.0 * 40.0 + 8.0;
        assert!(near(card.box_height(t0), want));
        assert_eq!(card.content(t0), (1.0, 0.0));
        assert!(!card.is_animating(t0));
        assert!(!card.moving(t0));
        card.fold(t0 + ms(10));
        assert!(card.gone(t0 + ms(10)));
        assert_eq!(card.box_height(t0 + ms(10)), 0.0);
        // A waiting card has nothing to fold: it is gone.
        let mut waiting = MenuCard::waiting(0, item(), true);
        waiting.fold(t0);
        assert!(waiting.gone(t0));
    }
}
