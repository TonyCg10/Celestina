//! The Island: a Haze capsule on the top edge of one output that grows into
//! a page (design §5.5). [`state`] is the pure state machine, [`view`] the
//! pure views, [`clock`] and [`workspaces`] what the capsule shows; this file
//! owns one output's Island: its two view trees (the capsule's and the
//! page's), the frame, the input and the commands.
//!
//! The growth is painted from [`state::IslandState`]: the Haze rect at the
//! instant, then each tree under its own opacity, clipped to that rect. The
//! trees hold content laid out at its final place, so nothing in them moves
//! while the Island grows.

pub mod clock;
pub mod menu;
pub mod pages;
pub mod state;
pub mod view;
pub mod workspaces;

use std::time::Instant;

use celestina_paint::{Painter, PixmapId, Rect, Theme};
use celestina_ui::{Interaction, Key, Tree, UiEvent, BTN_LEFT};
use celestina_wayland::{
    CloseReason, FrameCx, InputEvent, KeyboardInteractivity, PopupId, PopupShell, SurfaceCtl,
};

use crate::{
    command::{Page, Verb},
    providers::{niri::NiriModel, notifications::NotificationsCommand},
    tray::{scroll_steps, ItemId, Orientation, TrayCommand},
};
use clock::ClockTick;
use pages::{
    activity, notifications, page_view, tabs,
    tray::{self as tray_page, IconCache, Pixmaps, TrayList, TrayRow},
    HistoryRow, NameOwner, PageCx,
};
use state::IslandState;
use workspaces::{rows_for, Row};

/// What the Island asks the app to do: talk to the compositor, open a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum IslandAction {
    FocusWorkspace(u64),
    FocusWindow(u64),
    /// Something the tray host does: activate, secondary activate, scroll.
    Tray(TrayCommand),
    /// The menu of tray item `item` was asked for from its page `row` (a
    /// rect of the surface), by the pointer or the `keyboard`: the app asks
    /// the host for the menu and hands the popup to
    /// [`Island::open_tray_menu`] once it arrives.
    OpenTrayMenu {
        item: ItemId,
        row: Rect,
        keyboard: bool,
    },
    /// The overflow button at `more` (a rect of the surface) was chosen,
    /// from the `keyboard` or the pointer: the app builds the menu and
    /// hands it to [`Island::open_menu`].
    OpenMenu {
        more: Rect,
        keyboard: bool,
    },
    /// A row's dismiss or action button, or dismiss all, on the
    /// Notifications page: the notification server runs the command.
    Notification(NotificationsCommand),
    /// The Notifications page's switch: the app flips the do-not-disturb
    /// setting, the one owner of the choice.
    ToggleDoNotDisturb,
}

/// Linux `BTN_RIGHT` and `BTN_MIDDLE`.
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
/// The Menu key and F10 (with Shift): the keyboard's context menu.
const XKB_KEY_MENU: u32 = 0xff67;
const XKB_KEY_F10: u32 = 0xffc9;

/// What the Island reads of the app each frame besides the compositor's
/// model: who holds the bus names, the settings in force, the notification
/// history and the tray.
#[derive(Clone, Copy, Debug)]
pub struct Shared<'a> {
    /// Who holds the bus names the pages depend on.
    pub owners: &'a NameOwner,
    /// The do-not-disturb setting in force.
    pub do_not_disturb: bool,
    /// The notification history's rows.
    pub history: &'a [HistoryRow],
    /// The tray's items.
    pub tray: &'a TrayList,
}

/// The painter's textures as the tray's icon cache stores pixmaps in them.
impl Pixmaps for Painter {
    fn upload(&mut self, argb: &[u8], w: u32, h: u32) -> Option<PixmapId> {
        match self.upload_pixmap(argb, w, h) {
            Ok(id) => Some(id),
            Err(err) => {
                tracing::warn!(%err, "a tray icon could not be uploaded");
                None
            }
        }
    }

    fn release(&mut self, id: PixmapId) {
        self.release_pixmap(id);
    }

    fn has_theme_icon(&self, name: &str, size: u32) -> bool {
        Painter::has_theme_icon(self, name, size)
    }
}

/// The wheel's pixels not yet worth a step, per axis, for the row they
/// were spent on.
#[derive(Debug, Default)]
struct ScrollRemainder {
    row: Option<Key>,
    vertical: i32,
    horizontal: i32,
}

/// One output's Island.
#[derive(Debug)]
pub struct Island {
    output: String,
    /// Logical size of the surface.
    size: (f32, f32),
    /// The surface's device pixel ratio at its last frame.
    dpr: f32,
    state: IslandState,
    capsule: Tree,
    page: Tree,
    capsule_input: Interaction,
    page_input: Interaction,
    clock: String,
    clock_minute: Option<u64>,
    tick: ClockTick,
    rows: Vec<Row>,
    /// The last niri model: what the Activity page lists and what a click
    /// on one of its rows asks for.
    model: Option<NiriModel>,
    /// Who holds the bus names the pages depend on.
    owners: NameOwner,
    /// The do-not-disturb setting the Notifications page shows.
    do_not_disturb: bool,
    /// The notification rows the Notifications page lists.
    history: Vec<HistoryRow>,
    /// The page slide the page view was built with.
    applied_slide: f32,
    /// The theme the views were built with.
    theme: Theme,
    reduced: bool,
    /// The views must be built again before the next layout.
    stale: bool,
    /// The growth was in flight at the last frame.
    moving: bool,
    /// The overflow menu, while it is open.
    menu: Option<PopupId>,
    /// The first level of a tray item's menu, while it is open.
    tray_menu: Option<PopupId>,
    /// The tray's items as the page shows them.
    tray_rows: Vec<TrayRow>,
    /// The generation of the tray list `tray_rows` was made from.
    tray_generation: u64,
    /// The pixmaps uploaded for the items' icons.
    icons: IconCache,
    scroll: ScrollRemainder,
}

impl Island {
    /// The capsule at rest for `output`, on a surface of `size` logical px.
    #[must_use]
    pub fn new(output: &str, size: (f32, f32)) -> Island {
        let theme = Theme::dark();
        let mut state = IslandState::new(0.0);
        state.set_surface(size);
        Island {
            output: output.to_owned(),
            size,
            dpr: 1.0,
            state,
            capsule: Tree::new(false, &theme),
            page: Tree::new(false, &theme),
            capsule_input: Interaction::new(),
            page_input: Interaction::new(),
            clock: String::new(),
            clock_minute: None,
            tick: ClockTick::default(),
            rows: Vec::new(),
            model: None,
            owners: NameOwner::default(),
            do_not_disturb: false,
            history: Vec::new(),
            applied_slide: 0.0,
            theme,
            reduced: false,
            stale: true,
            moving: false,
            menu: None,
            tray_menu: None,
            tray_rows: Vec::new(),
            tray_generation: 0,
            icons: IconCache::default(),
            scroll: ScrollRemainder::default(),
        }
    }

    /// The output's name.
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }

    /// The exclusive zone of the Island's surface (the capsule and its gap).
    #[must_use]
    pub fn exclusive_zone(&self) -> i32 {
        self.state.exclusive_zone()
    }

    /// Whether the Notifications page is on screen (or growing): what the
    /// person sees there counts as seen.
    #[must_use]
    pub fn shows_notifications(&self) -> bool {
        self.page() == Some(Page::Notifications)
    }

    /// The open page, if any.
    #[must_use]
    pub fn page(&self) -> Option<Page> {
        self.state.page()
    }

    /// Reduced motion makes the growth and every slide a single step.
    pub fn set_reduced_motion(&mut self, on: bool) {
        if on != self.reduced {
            self.reduced = on;
            self.capsule.set_reduced_motion(on);
            self.page.set_reduced_motion(on);
        }
    }

    /// Whether `model` changes what this Island shows (and is worth a frame).
    #[must_use]
    pub fn shows_a_change(&self, model: Option<&NiriModel>) -> bool {
        self.rows_from(model) != self.rows || (self.state.is_open() && model != self.model.as_ref())
    }

    fn rows_from(&self, model: Option<&NiriModel>) -> Vec<Row> {
        model.map_or_else(Vec::new, |m| rows_for(m, &self.output))
    }

    /// Draws one frame of the Island's surface with `model` (the last niri
    /// model, if any) and what is `shared` with the app, under `reduced`
    /// motion.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        model: Option<&NiriModel>,
        shared: &Shared<'_>,
        reduced: bool,
    ) {
        let Shared {
            owners,
            do_not_disturb,
            history,
            tray,
        } = *shared;
        let now = cx.now;
        self.set_reduced_motion(reduced);
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.capsule.set_theme(&self.theme);
            self.page.set_theme(&self.theme);
            self.stale = true;
        }
        if cx.size != self.size {
            self.size = cx.size;
            self.state.set_surface(cx.size);
            self.stale = true;
        }
        self.dpr = cx.scale as f32;
        let rows = self.rows_from(model);
        if rows != self.rows {
            self.rows = rows;
            self.stale = true;
        }
        if model != self.model.as_ref() {
            self.model = model.cloned();
            self.stale |= self.state.is_open();
        }
        if *owners != self.owners {
            self.owners = owners.clone();
            self.stale |= self.state.is_open();
        }
        if do_not_disturb != self.do_not_disturb {
            self.do_not_disturb = do_not_disturb;
            self.stale |= self.state.is_open();
        }
        if history != self.history {
            self.history = history.to_vec();
            self.stale |= self.state.is_open();
        }
        if tray.generation != self.tray_generation {
            self.tray_generation = tray.generation;
            self.tray_rows = self.icons.rows(&tray.items, painter);
            self.stale |= self.state.is_open();
        }
        self.tick_clock(now, &mut cx.ctl);
        if self.stale || self.state.slide(now) != self.applied_slide {
            self.apply_views(now);
        }
        self.layout(painter);
        self.paint(painter, now, cx.blur_available);
        self.sync_ctl(&mut cx.ctl, now);

        let moving = self.state.wants_frame(now);
        if moving || self.capsule.is_animating(now) || self.page.is_animating(now) {
            cx.ctl.request_redraw();
        }
        if self.moving && !moving {
            tracing::debug!(output = %self.output, open = self.state.is_open(), "island: growth settled");
        }
        self.moving = moving;
    }

    /// Reads the wall clock: a new minute may change the text; the timer is
    /// re-armed for the next boundary whenever its deadline has passed.
    fn tick_clock(&mut self, now: Instant, ctl: &mut SurfaceCtl<'_>) {
        let (secs, nanos) = clock::unix_now();
        let minute = secs / 60;
        if self.clock_minute != Some(minute) {
            self.clock_minute = Some(minute);
            let text = clock::format_local(secs, clock::utc_offset());
            if text != self.clock {
                self.clock = text;
                self.stale = true;
            }
        }
        if self.tick.should_arm(now) {
            let wait = clock::until_next_minute(secs, nanos) + clock::TICK_MARGIN;
            self.tick.arm(now, wait);
            ctl.redraw_after(wait);
        }
    }

    /// Builds both views. The page asks for its size first, so it is laid
    /// out at the size the Island is heading for. While the page slides, the
    /// view is applied every frame with its new offset and the tree's own
    /// slide of moved nodes is off, so the offset is the only motion.
    fn apply_views(&mut self, now: Instant) {
        let w = self.size.0;
        self.capsule
            .apply(view::capsule(&self.theme, &self.clock, &self.rows, w), now);
        let shown = self.state.shown_page();
        let page = page_view(shown);
        let cx = PageCx {
            theme: &self.theme,
            max_size: self.state.max_page_size(),
            model: self.model.as_ref(),
            output: &self.output,
            owners: &self.owners,
            do_not_disturb: self.do_not_disturb,
            history: &self.history,
            items: &self.tray_rows,
        };
        self.state
            .request_page_size(page.wants(&cx), now, self.reduced);
        let slide = self.state.slide(now);
        let moving = slide != 0.0 || self.applied_slide != 0.0;
        if moving {
            self.page.set_reduced_motion(true);
        }
        self.page.apply(
            view::page(
                &self.theme,
                shown,
                page.title(),
                page.view(&cx),
                w,
                self.state.page_size(),
                slide,
            ),
            now,
        );
        if moving {
            self.page.set_reduced_motion(self.reduced);
        }
        self.applied_slide = slide;
        self.stale = false;
    }

    /// Lays both trees out (a no-op when nothing changed) and takes the
    /// capsule's width from its content.
    fn layout(&mut self, painter: &mut Painter) {
        self.capsule.layout(self.size, painter.text(), self.dpr);
        self.page.layout(self.size, painter.text(), self.dpr);
        for tree in [&mut self.capsule, &mut self.page] {
            if let Some(fault) = tree.take_fault() {
                tracing::warn!(output = %self.output, %fault, "island: layout fault");
            }
        }
        if let Some(id) = self.capsule.node_by_key(view::capsule_key()) {
            let w = self.capsule.rect(id).w;
            if w > 0.0 {
                self.state.set_capsule_w(w);
            }
        }
    }

    /// The Haze at the Island's rect, then the capsule and the page, each
    /// under its opacity and clipped to that rect.
    fn paint(&mut self, painter: &mut Painter, now: Instant, blur_available: bool) {
        let rect = self.state.rect(now, self.size.0);
        let radius = self.theme.radius_lg;
        painter.begin(self.size, self.dpr, blur_available);
        painter.haze(rect, radius);
        painter.push_clip(rect, radius);
        let layers = [
            (&mut self.capsule, self.state.capsule_opacity(now)),
            (&mut self.page, self.state.page_opacity(now)),
        ];
        for (tree, opacity) in layers {
            if opacity > 0.0 {
                painter.push_state();
                painter.opacity(opacity);
                tree.paint(painter, now);
                painter.pop_state();
            }
        }
        painter.pop_clip();
        painter.end();
    }

    /// Blur and input regions plus keyboard interactivity for `now`: the
    /// blur follows the silhouette every frame; the input region is the
    /// silhouette at rest and the whole surface while open (so a click
    /// outside the page reaches the Island and closes it); the keyboard is
    /// `None` at rest and `OnDemand` while open (ADR 0007).
    fn sync_ctl(&self, ctl: &mut SurfaceCtl<'_>, now: Instant) {
        let silhouette = self
            .state
            .silhouette(now, self.size.0, self.theme.radius_lg);
        ctl.set_blur_region(&silhouette);
        if self.state.is_open() {
            ctl.set_input_region(&[Rect::whole(self.size)]);
            ctl.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        } else {
            ctl.set_input_region(&silhouette);
            ctl.set_keyboard_interactivity(KeyboardInteractivity::None);
        }
    }

    /// Grows into `page`, or slides to it while already open.
    fn open(&mut self, page: Page, now: Instant) {
        if page != self.state.shown_page() {
            self.stale = true;
        }
        if self.state.is_open() {
            self.state.switch(page, now, self.reduced);
            return;
        }
        self.stale = true;
        // The capsule's hover ends with it.
        self.capsule_input
            .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
        self.state.open(page, now, self.reduced);
    }

    fn close(&mut self, now: Instant) {
        if self.state.is_open() {
            self.page_input
                .pointer(&mut self.page, &InputEvent::PointerLeave, now);
            self.page_input.key(&mut self.page, &InputEvent::FocusLost);
        }
        self.state.close(now, self.reduced);
    }

    /// Handles pointer and keyboard input on the Island's surface: a click
    /// on the capsule opens the page; Escape, a click outside the page and
    /// the keyboard leaving close it; the page's buttons and rows act.
    /// `painter` measures the views if they changed since the last frame,
    /// so what the input hits is what the next frame paints.
    pub fn input(
        &mut self,
        ev: InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        painter: &mut Painter,
    ) -> Vec<IslandAction> {
        let now = Instant::now();
        match ev {
            InputEvent::PopupClosed { id, reason } => {
                if self.menu_closed(id, reason, now) {
                    self.sync_ctl(ctl, now);
                    ctl.request_redraw();
                }
                return Vec::new();
            }
            // The popup's grab still delivers presses on the Island (the
            // same client): like a native menu, such a press only closes it.
            InputEvent::PointerPress { .. } if self.menu.is_some() || self.tray_menu.is_some() => {
                self.close_menu(ctl);
                return Vec::new();
            }
            _ => {}
        }
        if self.stale {
            self.apply_views(now);
        }
        self.layout(painter);
        let before = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.state.is_open(),
        );
        let open = self.state.is_open();
        let mut actions = Vec::new();
        let events = match &ev {
            InputEvent::FocusLost => {
                if open {
                    self.close(now);
                }
                Vec::new()
            }
            InputEvent::Key {
                keysym,
                modifiers,
                pressed: true,
                ..
            } if open
                && (*keysym == XKB_KEY_MENU || (*keysym == XKB_KEY_F10 && modifiers.shift)) =>
            {
                actions.extend(self.menu_key());
                Vec::new()
            }
            InputEvent::Key { .. } if open => self.page_input.key(&mut self.page, &ev),
            InputEvent::PointerPress { x, y, .. }
                if open && !self.state.rect(now, self.size.0).contains(*x, *y) =>
            {
                self.close(now);
                Vec::new()
            }
            // The tray rows' other buttons: the page's pointer handling
            // only knows the left one.
            InputEvent::PointerRelease { x, y, button, .. } if open && *button != BTN_LEFT => {
                actions.extend(self.other_button(*x, *y, *button));
                Vec::new()
            }
            _ if open => self.page_input.pointer(&mut self.page, &ev, now),
            _ => self.capsule_input.pointer(&mut self.capsule, &ev, now),
        };
        for event in events {
            let tray = self.tray_actions(&event);
            if !tray.is_empty() {
                actions.extend(tray);
            } else if let Some(action) = self.on_event(event, now) {
                actions.push(action);
            }
        }
        // The page is closing: its menus go with it.
        if !self.state.is_open() {
            self.close_menu(ctl);
        }
        let after = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.state.is_open(),
        );
        if before != after || !actions.is_empty() || self.stale {
            self.sync_ctl(ctl, now);
            ctl.request_redraw();
        }
        actions
    }

    /// Opens the overflow `menu` of `size` below the overflow button at
    /// `more` (the rect [`IslandAction::OpenMenu`] carried). The runtime
    /// grabs with the serial of the press or key that chose the button.
    /// A second request while one is open is ignored.
    pub fn open_menu(
        &mut self,
        more: Rect,
        size: (f32, f32),
        menu: Box<dyn PopupShell>,
        ctl: &mut SurfaceCtl<'_>,
    ) {
        if self.menu.is_some() || !self.state.is_open() {
            return;
        }
        let id = ctl.open_popup(menu::placement(more), size, menu);
        tracing::debug!(output = %self.output, ?more, popup = id.0, "island: menu opened");
        self.menu = Some(id);
    }

    /// Opens the first level of a tray item's menu, of `size`, below its
    /// page `row` (the rect [`IslandAction::OpenTrayMenu`] carried), and
    /// returns its popup. `None` while the Tray page is not the one shown
    /// (the person switched pages before the host answered) or another
    /// menu is open.
    pub fn open_tray_menu(
        &mut self,
        row: Rect,
        size: (f32, f32),
        menu: Box<dyn PopupShell>,
        ctl: &mut SurfaceCtl<'_>,
    ) -> Option<PopupId> {
        if self.menu.is_some() || self.tray_menu.is_some() || !self.on_tray_page() {
            return None;
        }
        let id = ctl.open_popup(crate::tray::menu::placement(&self.theme, row), size, menu);
        tracing::debug!(output = %self.output, popup = id.0, "island: tray menu opened");
        self.tray_menu = Some(id);
        Some(id)
    }

    /// Whether a tray item's menu hangs from this Island right now.
    #[must_use]
    pub fn tray_menu_open(&self) -> bool {
        self.tray_menu.is_some()
    }

    /// Whether the page lists tray items right now.
    fn on_tray_page(&self) -> bool {
        self.state.is_open() && self.state.shown_page() == Page::Tray
    }

    /// The tray row `key` names, with its index.
    fn tray_row(&self, key: Key) -> Option<(usize, &TrayRow)> {
        if !self.on_tray_page() {
            return None;
        }
        let index = tray_page::index_of(key, self.tray_rows.len())?;
        Some((index, self.tray_rows.get(index)?))
    }

    /// The page's rect of the tray row at `index`.
    fn tray_row_rect(&self, index: usize) -> Option<Rect> {
        let id = self.page.node_by_key(tray_page::row_key(index))?;
        Some(self.page.rect(id))
    }

    /// What a click, Enter, Space or the wheel on a tray row asks of the
    /// host. A click and the keyboard's activation both activate the item;
    /// the keyboard's way to the menu is the Menu key or Shift+F10
    /// ([`Island::menu_key`]), as the pointer's is the right button.
    fn tray_actions(&mut self, event: &UiEvent) -> Vec<IslandAction> {
        match event {
            UiEvent::Click(key) | UiEvent::Activate(key) => self
                .tray_row(*key)
                .map(|(_, row)| IslandAction::Tray(TrayCommand::Activate(row.id.clone())))
                .into_iter()
                .collect(),
            UiEvent::Scroll(key, horizontal, vertical) => {
                self.scroll_actions(*key, *horizontal, *vertical)
            }
            UiEvent::HoverChanged(..) | UiEvent::Escape | UiEvent::FieldChanged(_) => Vec::new(),
        }
    }

    fn open_menu_action(&self, item: ItemId, index: usize, keyboard: bool) -> Option<IslandAction> {
        Some(IslandAction::OpenTrayMenu {
            item,
            row: self.tray_row_rect(index)?,
            keyboard,
        })
    }

    /// A wheel turn of (`horizontal`, `vertical`) logical px over the row
    /// `key`: whole steps of 15 px per axis, the rest kept for the next
    /// turn on the same row. Items expect Qt's `angleDelta` sign (what
    /// Plasma sends them): positive for a wheel rolled away from the
    /// person and for one tilted left, which is the opposite of the
    /// Wayland axis on both axes, so both are negated. The horizontal
    /// direction is listed for a real item in VAL-CEL-NOCTALIA.
    fn scroll_actions(&mut self, key: Key, horizontal: f32, vertical: f32) -> Vec<IslandAction> {
        let Some((_, row)) = self.tray_row(key) else {
            return Vec::new();
        };
        let id = row.id.clone();
        if self.scroll.row != Some(key) {
            self.scroll = ScrollRemainder {
                row: Some(key),
                ..ScrollRemainder::default()
            };
        }
        let px = |v: f32| if v.is_finite() { v.round() as i32 } else { 0 };
        let up = scroll_steps(px(-vertical), &mut self.scroll.vertical);
        let left = scroll_steps(px(-horizontal), &mut self.scroll.horizontal);
        let mut out = Vec::new();
        if up != 0 {
            out.push(IslandAction::Tray(TrayCommand::Scroll(
                id.clone(),
                up,
                Orientation::Vertical,
            )));
        }
        if left != 0 {
            out.push(IslandAction::Tray(TrayCommand::Scroll(
                id,
                left,
                Orientation::Horizontal,
            )));
        }
        out
    }

    /// A middle click on a tray row activates the item secondarily; a right
    /// click asks for its menu.
    fn other_button(&self, x: f32, y: f32, button: u32) -> Vec<IslandAction> {
        if !self.on_tray_page() {
            return Vec::new();
        }
        let hit = (0..self.tray_rows.len().min(tray_page::MAX_ROWS))
            .find(|&i| self.tray_row_rect(i).is_some_and(|r| r.contains(x, y)));
        let Some(index) = hit else {
            return Vec::new();
        };
        let row = &self.tray_rows[index];
        match button {
            BTN_MIDDLE => vec![IslandAction::Tray(TrayCommand::SecondaryActivate(
                row.id.clone(),
            ))],
            BTN_RIGHT if row.has_menu => self
                .open_menu_action(row.id.clone(), index, false)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The Menu key (or Shift+F10) on the focused tray row: its menu.
    fn menu_key(&self) -> Vec<IslandAction> {
        let Some(id) = self.page.highlight().focused else {
            return Vec::new();
        };
        match self.tray_row(self.page.key_of(id)) {
            Some((index, row)) if row.has_menu => self
                .open_menu_action(row.id.clone(), index, true)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Gives the pixmaps uploaded for the tray's icons back to `painter`
    /// (the Island's output went away).
    pub fn release_icons(&mut self, painter: &mut Painter) {
        self.icons.release_all(painter);
        self.tray_rows.clear();
        self.tray_generation = 0;
    }

    /// Closes the menus from the Island's side; the runtime still reports
    /// `PopupClosed`, which then finds nothing to do.
    fn close_menu(&mut self, ctl: &mut SurfaceCtl<'_>) {
        for id in [self.menu.take(), self.tray_menu.take()]
            .into_iter()
            .flatten()
        {
            ctl.close_popup(id);
            ctl.request_redraw();
        }
    }

    /// Popup `id` is gone. A compositor dismissal is a click outside the
    /// shell (another window, another output), which for the page is a
    /// click outside: it closes too; the keyboard's leave that came with it
    /// went to the popup, so no `FocusLost` would close it otherwise. A
    /// press on the Island or the menu's own choice or Escape closes only
    /// the menu. Returns whether the popup was this Island's menu.
    fn menu_closed(&mut self, id: PopupId, reason: CloseReason, now: Instant) -> bool {
        if self.menu == Some(id) {
            self.menu = None;
        } else if self.tray_menu == Some(id) {
            self.tray_menu = None;
        } else {
            return false;
        }
        if reason == CloseReason::Dismissed && self.state.is_open() {
            self.close(now);
        }
        true
    }

    /// The menu's reopen row: the open page shrinks to the capsule at once
    /// and grows again, its focus and hover gone.
    pub fn reopen(&mut self, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        self.reopen_at(now);
        self.sync_ctl(ctl, now);
        ctl.request_redraw();
    }

    fn reopen_at(&mut self, now: Instant) {
        let page = self.state.shown_page();
        self.close(now);
        // A single step back to the capsule, whatever the motion setting.
        self.state.close(now, true);
        self.open(page, now);
    }

    /// What one of the trees' events does: a click or Enter acts on its
    /// node, Escape closes. The overflow button remembers whether the
    /// keyboard chose it, so the menu can start with a focused row.
    fn on_event(&mut self, event: UiEvent, now: Instant) -> Option<IslandAction> {
        let (key, keyboard) = match event {
            UiEvent::Click(key) => (key, false),
            UiEvent::Activate(key) => (key, true),
            UiEvent::Escape => {
                self.close(now);
                return None;
            }
            UiEvent::HoverChanged(..) | UiEvent::Scroll(..) | UiEvent::FieldChanged(_) => {
                return None
            }
        };
        match self.activate(key, now)? {
            IslandAction::OpenMenu { more, .. } => Some(IslandAction::OpenMenu { more, keyboard }),
            action => Some(action),
        }
    }

    /// What a click or Enter on the node `key` does.
    fn activate(&mut self, key: Key, now: Instant) -> Option<IslandAction> {
        if key == view::capsule_key() {
            self.open(self.state.shown_page(), now);
            None
        } else if key == view::close_key() {
            self.close(now);
            None
        } else if key == view::more_key() {
            let id = self.page.node_by_key(key)?;
            Some(IslandAction::OpenMenu {
                more: self.page.rect(id),
                keyboard: false,
            })
        } else if let Some(page) = tabs::page_of(key) {
            self.open(page, now);
            None
        } else if let Some(action) = notifications::action_for(
            &self.history,
            &self.owners,
            &self.theme,
            self.state.max_page_size(),
            key,
        ) {
            Some(action)
        } else {
            // No optimistic state: the compositor's event closes the loop.
            activity::action_for(
                self.model.as_ref()?,
                &self.output,
                &self.theme,
                self.state.max_page_size(),
                key,
            )
        }
    }

    /// Runs an Island verb of `org.celestina.Shell1`; other verbs are not
    /// the Island's and change nothing.
    pub fn command(&mut self, verb: &Verb, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        match *verb {
            Verb::IslandToggle => {
                if self.state.is_open() {
                    self.close(now);
                } else {
                    self.open(self.state.shown_page(), now);
                }
            }
            Verb::IslandOpen(page) => self.open(page, now),
            Verb::IslandClose => self.close(now),
            Verb::LauncherToggle
            | Verb::NotificationsToggle
            | Verb::NotificationsDismissAll
            | Verb::Quit => return,
        }
        self.sync_ctl(ctl, now);
        ctl.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::niri::{Window, Workspace};
    use celestina_wayland::{CloseReason, PopupId};

    fn model(output: &str, active: u64) -> NiriModel {
        NiriModel {
            workspaces: (1..=3)
                .map(|id| Workspace {
                    id,
                    idx: u8::try_from(id).unwrap_or(0),
                    name: None,
                    output: Some(output.to_owned()),
                    is_active: id == active,
                    is_focused: id == active,
                    columns: 1,
                })
                .collect(),
            ..NiriModel::default()
        }
    }

    #[test]
    fn only_a_model_that_changes_this_outputs_rows_is_worth_a_frame() {
        let mut island = Island::new("DP-1", (760.0, 560.0));
        assert!(!island.shows_a_change(None), "no model, no rows");
        assert!(island.shows_a_change(Some(&model("DP-1", 1))));
        island.rows = rows_for(&model("DP-1", 1), "DP-1");
        assert!(!island.shows_a_change(Some(&model("DP-1", 1))));
        assert!(island.shows_a_change(Some(&model("DP-1", 2))));
        // Another output's workspaces are not this Island's.
        assert!(island.shows_a_change(Some(&model("HDMI-A-1", 2))));
        island.rows.clear();
        assert!(!island.shows_a_change(Some(&model("HDMI-A-1", 2))));
    }

    #[test]
    fn rows_activate_focus_requests_and_the_capsule_opens_the_last_page() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.model = Some(model("DP-1", 1));
        assert_eq!(
            island.activate(activity::workspace_key(2), t0),
            Some(IslandAction::FocusWorkspace(2))
        );
        assert_eq!(island.activate(Key::named("nothing"), t0), None);
        assert_eq!(island.page(), None);
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(Page::Activity));
        island.activate(view::close_key(), t0);
        assert_eq!(island.page(), None);
        island.open(Page::Tray, t0);
        island.close(t0);
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(Page::Tray));
    }

    #[test]
    fn opening_another_page_while_open_slides_to_it() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Notifications, t0);
        assert_eq!(island.state.slide(t0), 0.0);
        island.open(Page::Activity, t0);
        assert_eq!(island.page(), Some(Page::Activity));
        assert_eq!(
            island.state.slide(t0),
            1.0,
            "Activity is left of Notifications"
        );
        island.set_reduced_motion(true);
        island.open(Page::Tray, t0);
        assert_eq!(island.page(), Some(Page::Tray));
        assert_eq!(island.state.slide(t0), 0.0);
    }

    #[test]
    fn a_tab_click_slides_to_its_page_and_a_window_row_asks_to_focus() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let mut m = model("DP-1", 1);
        m.windows.push(Window {
            id: 7,
            title: Some("t".to_owned()),
            app_id: None,
            workspace_id: Some(1),
            is_focused: false,
            icon: None,
        });
        island.model = Some(m);
        island.open(Page::Activity, t0);
        assert_eq!(island.activate(tabs::tab_key(Page::Tray), t0), None);
        assert_eq!(island.page(), Some(Page::Tray));
        assert_eq!(island.state.slide(t0), -1.0, "Tray is right of Activity");
        island.open(Page::Activity, t0);
        assert_eq!(
            island.activate(activity::window_key(7), t0),
            Some(IslandAction::FocusWindow(7))
        );
        assert_eq!(island.page(), Some(Page::Activity), "no optimistic state");
    }

    #[test]
    fn a_dismissed_menu_closes_the_page_but_an_island_press_does_not() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Activity, t0);

        // Closed from the Island's side (a press on the Island): the page
        // stays.
        island.menu = Some(PopupId(1));
        assert!(island.menu_closed(PopupId(1), CloseReason::Parent, t0));
        assert!(island.menu.is_none());
        assert!(island.state.is_open());
        // Closed by the menu itself (a row chosen, Escape): the page stays.
        island.menu = Some(PopupId(2));
        assert!(island.menu_closed(PopupId(2), CloseReason::Shell, t0));
        assert!(island.state.is_open());
        // Someone else's popup: ignored.
        island.menu = Some(PopupId(3));
        assert!(!island.menu_closed(PopupId(9), CloseReason::Dismissed, t0));
        assert!(island.state.is_open());
        // Dismissed by the compositor (a click on another window): the
        // page closes with it.
        assert!(island.menu_closed(PopupId(3), CloseReason::Dismissed, t0));
        assert!(island.menu.is_none());
        assert!(!island.state.is_open());
    }

    #[test]
    fn a_parent_or_shell_close_of_the_menu_leaves_the_page_open() {
        let t0 = Instant::now();
        for reason in [CloseReason::Parent, CloseReason::Shell] {
            let mut island = Island::new("DP-1", (760.0, 560.0));
            island.open(Page::Notifications, t0);
            island.menu = Some(PopupId(4));
            assert!(island.menu_closed(PopupId(4), reason, t0), "{reason:?}");
            assert!(island.menu.is_none(), "{reason:?}");
            assert!(island.state.is_open(), "{reason:?}");
            assert_eq!(island.page(), Some(Page::Notifications), "{reason:?}");
        }
    }

    #[test]
    fn the_more_button_asks_for_the_menu_below_itself() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Activity, t0);
        island.apply_views(t0);
        island.page.layout_measured(island.size, |s, _, _| {
            (7.0 * s.chars().count() as f32, 16.0)
        });
        let more = island
            .page
            .node_by_key(view::more_key())
            .map(|id| island.page.rect(id));
        assert!(more.is_some_and(|r| r.w > 0.0));
        assert_eq!(
            island.on_event(UiEvent::Click(view::more_key()), t0),
            more.map(|more| IslandAction::OpenMenu {
                more,
                keyboard: false
            })
        );
        // Enter or Space on the button: the menu starts with a focused row.
        assert_eq!(
            island.on_event(UiEvent::Activate(view::more_key()), t0),
            more.map(|more| IslandAction::OpenMenu {
                more,
                keyboard: true
            })
        );
    }

    /// An open Tray page with `n` items (the first has a menu), laid out.
    fn tray_island(n: usize) -> (Island, Instant) {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.tray_rows = (0..n)
            .map(|i| TrayRow {
                id: ItemId {
                    bus_name: format!(":1.{i}"),
                    path: "/StatusNotifierItem".to_owned(),
                },
                title: format!("item {i}"),
                has_menu: i == 0,
                ..TrayRow::default()
            })
            .collect();
        island.open(Page::Tray, t0);
        island.apply_views(t0);
        island.page.layout_measured(island.size, |s, _, _| {
            (7.0 * s.chars().count() as f32, 16.0)
        });
        (island, t0)
    }

    fn id_of(island: &Island, i: usize) -> ItemId {
        island.tray_rows[i].id.clone()
    }

    #[test]
    fn scroll_on_a_row_sends_scroll_with_the_delta_in_steps() {
        let (mut island, _) = tray_island(2);
        let row = tray_page::row_key(1);
        let id = id_of(&island, 1);
        let scroll = |v: i32, o| IslandAction::Tray(TrayCommand::Scroll(id.clone(), v, o));
        // The wheel's down (positive) is the item's negative; 30 px = 2 steps.
        assert_eq!(
            island.scroll_actions(row, 0.0, 30.0),
            vec![scroll(-2, Orientation::Vertical)]
        );
        // Less than a step waits; two partial turns add up, keeping the rest.
        assert_eq!(island.scroll_actions(row, 0.0, -10.0), vec![]);
        assert_eq!(
            island.scroll_actions(row, 0.0, -10.0),
            vec![scroll(1, Orientation::Vertical)]
        );
        // Horizontal is negated too (a tilt right is the item's negative)
        // and both axes can fire together.
        assert_eq!(
            island.scroll_actions(row, 45.0, 20.0),
            vec![
                scroll(-1, Orientation::Vertical),
                scroll(-3, Orientation::Horizontal)
            ]
        );
        // Another row starts from nothing; a non-row key does nothing.
        assert_eq!(
            island.scroll_actions(tray_page::row_key(0), 0.0, 14.0),
            vec![]
        );
        assert_eq!(
            island.scroll_actions(Key::named("close"), 0.0, 99.0),
            vec![]
        );
        assert_eq!(island.scroll_actions(row, f32::NAN, f32::INFINITY), vec![]);
        // The same events through the tree's Scroll event.
        assert_eq!(
            island.tray_actions(&UiEvent::Scroll(row, 0.0, 15.0)),
            vec![scroll(-1, Orientation::Vertical)]
        );
    }

    #[test]
    fn a_click_and_the_keyboards_activation_both_activate_the_item() {
        let (mut island, _) = tray_island(2);
        let rect0 = island.tray_row_rect(0);
        assert!(rect0.is_some_and(|r| r.w > 0.0 && r.h > 0.0));
        let (a, b) = (id_of(&island, 0), id_of(&island, 1));
        assert_eq!(
            island.tray_actions(&UiEvent::Click(tray_page::row_key(0))),
            vec![IslandAction::Tray(TrayCommand::Activate(a.clone()))]
        );
        // Enter or Space on a row with a menu activates the item as a click
        // does; the menu has its own key (`the_menu_key_opens...`).
        assert_eq!(
            island.tray_actions(&UiEvent::Activate(tray_page::row_key(0))),
            vec![IslandAction::Tray(TrayCommand::Activate(a))]
        );
        assert_eq!(
            island.tray_actions(&UiEvent::Activate(tray_page::row_key(1))),
            vec![IslandAction::Tray(TrayCommand::Activate(b))]
        );
        // Neither a stranger's key nor a closed page acts.
        assert!(island
            .tray_actions(&UiEvent::Click(Key::named("close")))
            .is_empty());
        island.close(Instant::now());
        island.state.close(Instant::now(), true);
        assert!(island
            .tray_actions(&UiEvent::Click(tray_page::row_key(0)))
            .is_empty());
    }

    #[test]
    fn middle_click_activates_secondarily_and_right_click_asks_for_the_menu() {
        let (island, _) = tray_island(2);
        let centre = |i: usize| {
            island
                .tray_row_rect(i)
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .unwrap_or_default()
        };
        let (x0, y0) = centre(0);
        let (x1, y1) = centre(1);
        let (a, b) = (id_of(&island, 0), id_of(&island, 1));
        assert_eq!(
            island.other_button(x1, y1, BTN_MIDDLE),
            vec![IslandAction::Tray(TrayCommand::SecondaryActivate(b))]
        );
        assert_eq!(
            island.other_button(x0, y0, BTN_RIGHT),
            island
                .open_menu_action(a, 0, false)
                .into_iter()
                .collect::<Vec<_>>()
        );
        // A right click on an item without a menu, a click between rows
        // and any other button do nothing.
        assert!(island.other_button(x1, y1, BTN_RIGHT).is_empty());
        assert!(island.other_button(1.0, 1.0, BTN_MIDDLE).is_empty());
        assert!(island.other_button(x0, y0, 0x113).is_empty());
    }

    #[test]
    fn the_menu_key_opens_the_menu_of_the_focused_row_only() {
        let (mut island, _) = tray_island(2);
        assert!(island.menu_key().is_empty(), "nothing focused");
        // Tab focuses the first row (it has a menu), Tab again the second.
        let tab = InputEvent::Key {
            keysym: 0xff09,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        };
        let tab_to = |island: &mut Island, want: Key| {
            for _ in 0..30 {
                island.page_input.key(&mut island.page, &tab);
                let on = island.page.highlight().focused;
                if on.is_some_and(|id| island.page.key_of(id) == want) {
                    return;
                }
            }
            panic!("the focus never reached {want:?}");
        };
        tab_to(&mut island, tray_page::row_key(0));
        assert_eq!(island.menu_key().len(), 1);
        assert!(matches!(
            island.menu_key().first(),
            Some(IslandAction::OpenTrayMenu { keyboard: true, .. })
        ));
        tab_to(&mut island, tray_page::row_key(1));
        assert!(island.menu_key().is_empty(), "the second row has no menu");
    }

    #[test]
    fn the_trays_first_level_closes_like_the_overflow_menu() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Tray, t0);
        island.tray_menu = Some(PopupId(5));
        // Escape or a choice in the menu: the page stays.
        assert!(island.menu_closed(PopupId(5), CloseReason::Shell, t0));
        assert!(island.tray_menu.is_none() && island.state.is_open());
        // The compositor's dismissal (a click outside) closes the page.
        island.tray_menu = Some(PopupId(6));
        assert!(!island.menu_closed(PopupId(1), CloseReason::Dismissed, t0));
        assert!(island.menu_closed(PopupId(6), CloseReason::Dismissed, t0));
        assert!(!island.state.is_open());
    }

    #[test]
    fn reopening_grows_the_open_page_again_from_the_capsule() {
        let t0 = Instant::now();
        let later = t0 + std::time::Duration::from_secs(2);
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Tray, t0);
        assert_eq!(island.state.progress(later), 1.0);
        island.reopen_at(later);
        assert_eq!(island.page(), Some(Page::Tray));
        assert_eq!(island.state.progress(later), 0.0, "from the capsule");
        assert!(island.state.wants_frame(later), "and growing");
    }

    #[test]
    fn a_reopened_page_grows_on_one_clock_even_when_it_asks_for_more() {
        let t0 = Instant::now();
        let later = t0 + std::time::Duration::from_secs(2);
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(Page::Activity, t0);
        // A resize left in flight before the reopen.
        island.state.request_page_size((480.0, 360.0), later, false);
        island.reopen_at(later);
        let frame = later + std::time::Duration::from_millis(16);
        island.state.request_page_size((500.0, 400.0), frame, false);
        for ms in [16_u64, 120, 240, 340] {
            let now = later + std::time::Duration::from_millis(ms);
            let p = island.state.progress(now);
            let r = island.state.rect(now, 760.0);
            let capsule = island.state.rect(later, 760.0).w;
            assert!(
                (r.w - (capsule + (500.0 - capsule) * p)).abs() < 0.01,
                "{ms} ms: {r:?}"
            );
            assert!(island.state.wants_frame(now));
        }
    }

    #[test]
    fn an_open_island_shows_window_changes_a_closed_one_does_not() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let a = model("DP-1", 1);
        island.model = Some(a.clone());
        island.rows = rows_for(&a, "DP-1");
        let mut b = a.clone();
        b.windows.push(Window {
            id: 1,
            title: None,
            app_id: None,
            workspace_id: Some(1),
            is_focused: false,
            icon: None,
        });
        assert!(!island.shows_a_change(Some(&b)));
        island.open(Page::Activity, t0);
        assert!(island.shows_a_change(Some(&b)));
        assert!(!island.shows_a_change(Some(&a)));
    }
}
