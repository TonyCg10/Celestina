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
pub mod state;
pub mod view;
pub mod workspaces;

use std::time::Instant;

use celestina_paint::{Painter, Rect, Theme};
use celestina_ui::{Interaction, Key, Tree, UiEvent};
use celestina_wayland::{FrameCx, InputEvent, KeyboardInteractivity, SurfaceCtl};

use crate::{
    command::{Page, Verb},
    providers::niri::NiriModel,
};
use clock::ClockTick;
use state::IslandState;
use workspaces::{rows_for, Row};

/// What the Island asks the app to do: talk to the compositor, open a menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IslandAction {
    FocusWorkspace(u64),
    FocusWindow(u64),
    /// Open the overflow menu below this rect of the surface (CEL-1-D).
    OpenMenu(Rect),
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
    /// The theme the views were built with.
    theme: Theme,
    reduced: bool,
    /// The views must be built again before the next layout.
    stale: bool,
    /// The growth was in flight at the last frame.
    moving: bool,
}

impl Island {
    /// The capsule at rest for `output`, on a surface of `size` logical px.
    #[must_use]
    pub fn new(output: &str, size: (f32, f32)) -> Island {
        let theme = Theme::dark();
        Island {
            output: output.to_owned(),
            size,
            dpr: 1.0,
            state: IslandState::new(0.0),
            capsule: Tree::new(false, &theme),
            page: Tree::new(false, &theme),
            capsule_input: Interaction::new(),
            page_input: Interaction::new(),
            clock: String::new(),
            clock_minute: None,
            tick: ClockTick::default(),
            rows: Vec::new(),
            theme,
            reduced: false,
            stale: true,
            moving: false,
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
        self.rows_from(model) != self.rows
    }

    fn rows_from(&self, model: Option<&NiriModel>) -> Vec<Row> {
        model.map_or_else(Vec::new, |m| rows_for(m, &self.output))
    }

    /// Draws one frame of the Island's surface with `model` (the last niri
    /// model, if any) under `reduced` motion.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        model: Option<&NiriModel>,
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
            self.stale = true;
        }
        self.dpr = cx.scale as f32;
        let rows = self.rows_from(model);
        if rows != self.rows {
            self.rows = rows;
            self.stale = true;
        }
        self.tick_clock(now, &mut cx.ctl);
        if self.stale {
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

    fn apply_views(&mut self, now: Instant) {
        let w = self.size.0;
        self.capsule
            .apply(view::capsule(&self.theme, &self.clock, &self.rows, w), now);
        self.page.apply(
            view::page(&self.theme, self.state.shown_page(), &self.rows, w),
            now,
        );
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

    fn open(&mut self, page: Page, now: Instant) {
        if page != self.state.shown_page() {
            self.stale = true;
        }
        if !self.state.is_open() {
            // The capsule's hover ends with it.
            self.capsule_input
                .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
        }
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
            match event {
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(action) = self.activate(key, now) {
                        actions.push(action);
                    }
                }
                UiEvent::Escape => self.close(now),
                UiEvent::HoverChanged(..) | UiEvent::Scroll(..) => {}
            }
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
            Some(IslandAction::OpenMenu(self.page.rect(id)))
        } else {
            // No optimistic state: the compositor's event closes the loop.
            self.rows
                .iter()
                .find(|r| view::workspace_key(r.id) == key)
                .map(|r| IslandAction::FocusWorkspace(r.id))
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
    use crate::providers::niri::Workspace;

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
        island.rows = rows_for(&model("DP-1", 1), "DP-1");
        assert_eq!(
            island.activate(view::workspace_key(2), t0),
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
}
