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

use celestina_paint::{Painter, Rect, Theme};
use celestina_ui::{Interaction, Key, Tree, UiEvent};
use celestina_wayland::{
    CloseReason, FrameCx, InputEvent, KeyboardInteractivity, PopupId, PopupShell, SurfaceCtl,
};

use crate::{
    command::{Page, Verb},
    providers::niri::NiriModel,
};
use clock::ClockTick;
use pages::{activity, page_view, tabs, NameOwner, PageCx};
use state::IslandState;
use workspaces::{rows_for, Row};

/// What the Island asks the app to do: talk to the compositor, open a menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IslandAction {
    FocusWorkspace(u64),
    FocusWindow(u64),
    /// The overflow button at `more` (a rect of the surface) was chosen,
    /// from the `keyboard` or the pointer: the app builds the menu and
    /// hands it to [`Island::open_menu`].
    OpenMenu {
        more: Rect,
        keyboard: bool,
    },
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
            applied_slide: 0.0,
            theme,
            reduced: false,
            stale: true,
            moving: false,
            menu: None,
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
    /// model, if any), the bus-name `owners` and the `do_not_disturb`
    /// setting in force, under `reduced` motion.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        model: Option<&NiriModel>,
        owners: &NameOwner,
        do_not_disturb: bool,
        reduced: bool,
    ) {
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
            history: &[],
            items: &[],
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
            InputEvent::PointerPress { .. } if self.menu.is_some() => {
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
        let events = match &ev {
            InputEvent::FocusLost => {
                if open {
                    self.close(now);
                }
                Vec::new()
            }
            InputEvent::Key { .. } if open => self.page_input.key(&mut self.page, &ev),
            InputEvent::PointerPress { x, y, .. }
                if open && !self.state.rect(now, self.size.0).contains(*x, *y) =>
            {
                self.close(now);
                Vec::new()
            }
            _ if open => self.page_input.pointer(&mut self.page, &ev, now),
            _ => self.capsule_input.pointer(&mut self.capsule, &ev, now),
        };
        let mut actions = Vec::new();
        for event in events {
            if let Some(action) = self.on_event(event, now) {
                actions.push(action);
            }
        }
        // The page is closing: its menu goes with it.
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

    /// Closes the menu from the Island's side; the runtime still reports
    /// `PopupClosed`, which then finds nothing to do.
    fn close_menu(&mut self, ctl: &mut SurfaceCtl<'_>) {
        if let Some(id) = self.menu.take() {
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
        if self.menu != Some(id) {
            return false;
        }
        self.menu = None;
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
