//! The overview (design §6, Overview): a Haze card with two tabs on top and,
//! in Desktop mode, the workspaces of one output as strips of columns with a
//! tile per window, on its own layer surface (`celestina-overview`,
//! `Layer::Overlay`, anchored to every edge so it fills the output, keyboard
//! `Exclusive`). Outside the card the surface is fully transparent and the
//! compositor blurs the card only. [`state`] is the pure mode and selection,
//! [`view`] the pure view; this file owns one open overview: its tree, its
//! frame, its previews and its input, as [`crate::launcher::Launcher`] does.
//!
//! The app creates the surface when the overview opens and destroys it on
//! close. The overview asks to close on Escape, on a press anywhere outside
//! the card and on keyboard focus loss; choosing a window or a workspace
//! (Enter or a click) asks for its focus and closes too. The input region is
//! the whole surface, so a press outside the card reaches the overview and
//! never the window under it. Arrows move the selection, which then shows
//! the focus ring (it appears only once the keyboard moved the selection);
//! Tab switches the tab, `1` and `2` pick Desktop and Shortcuts; in
//! Shortcuts the arrows, Page Up/Down, Home and End scroll the rows.
//!
//! Previews arrive as RGBA8 [`Thumb`]s keyed by workspace id (one whole-output
//! capture per output, drawn behind that workspace's tiles), and are uploaded
//! to the GPU by the next frame (they never wait on the painter in the
//! caller). The caller must call [`Overview::release`]
//! before dropping an overview so the textures are freed.

pub mod flow;
pub mod slot;
pub mod state;
pub mod view;

use std::{collections::HashMap, time::Instant};

use celestina_paint::{silhouette_slices, Painter, PixmapId, Rect, Role, Theme};
use celestina_shell::thumbnail::Thumb;
use celestina_ui::{focus_ring, Interaction, Tree, UiEvent, FOCUS_WIDTH};
use celestina_wayland::{FrameCx, InputEvent, OutputId, SurfaceCtl, SurfaceId};

pub use state::{Dir, Mode, OverviewState, Scroll, Selection, ShortcutLine, Target};
pub use view::PreviewPixmap;

use crate::{
    capture::MAX_PREVIEWS,
    island::state::SILHOUETTE_STEP,
    providers::{keybinds::Keybinds, niri::NiriModel},
};

/// The overview's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-overview";

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const XKB_KEY_RETURN: u32 = 0xff0d;
const XKB_KEY_KP_ENTER: u32 = 0xff8d;
const XKB_KEY_ESCAPE: u32 = 0xff1b;
const XKB_KEY_LEFT: u32 = 0xff51;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_RIGHT: u32 = 0xff53;
const XKB_KEY_DOWN: u32 = 0xff54;
const XKB_KEY_HOME: u32 = 0xff50;
const XKB_KEY_PAGE_UP: u32 = 0xff55;
const XKB_KEY_PAGE_DOWN: u32 = 0xff56;
const XKB_KEY_END: u32 = 0xff57;
const XKB_KEY_1: u32 = 0x31;
const XKB_KEY_2: u32 = 0x32;

/// What the overview asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewAction {
    /// Focus this window (niri's id), then close.
    Focus(u64),
    /// Focus this workspace (niri's id), then close.
    FocusWorkspace(u64),
    /// The mode changed (already applied here, the overview stays open): a
    /// Shortcuts tab may now need its bindings.
    SwitchMode,
    /// Close without acting.
    Close,
}

/// One open overview.
pub struct Overview {
    surface: SurfaceId,
    output: OutputId,
    /// The surface's logical size, (0, 0) until its first frame.
    size: (f32, f32),
    dpr: f32,
    theme: Theme,
    tree: Tree,
    input: Interaction,
    state: OverviewState,
    /// Previews not yet uploaded, by workspace id.
    pending: HashMap<u64, Thumb>,
    /// Uploaded previews, by workspace id.
    pixmaps: HashMap<u64, PreviewPixmap>,
    /// Textures a newer preview replaced, freed at the next frame.
    stale_pixmaps: Vec<PixmapId>,
    /// The view must be built again before the next layout.
    stale: bool,
    /// The keyboard moved the selection: the selected tile shows the ring.
    keyboard_moved: bool,
}

impl std::fmt::Debug for Overview {
    /// Ids, size and counts only: previews (pending or uploaded) are never
    /// printed, so no pixel reaches a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Overview")
            .field("surface", &self.surface)
            .field("output", &self.output)
            .field("size", &self.size)
            .field("mode", &self.state.mode)
            .field("pending_previews", &self.pending.len())
            .field("previews", &self.pixmaps.len())
            .finish_non_exhaustive()
    }
}

impl Overview {
    /// The overview on `surface` of `output` (niri's name `output_name`), in
    /// `theme` under `reduced` motion, over `model`, in Desktop mode.
    #[must_use]
    pub fn new(
        surface: SurfaceId,
        output: OutputId,
        output_name: &str,
        theme: &Theme,
        reduced: bool,
        model: &NiriModel,
    ) -> Overview {
        let state = OverviewState::new(model, output_name);
        let mut tree = Tree::new(reduced, theme);
        let pixmaps = HashMap::new();
        tree.apply(
            view::overview(theme, &state, (0.0, 0.0), &pixmaps),
            Instant::now(),
        );
        Overview {
            surface,
            output,
            size: (0.0, 0.0),
            dpr: 1.0,
            theme: theme.clone(),
            tree,
            input: Interaction::new(),
            state,
            pending: HashMap::new(),
            pixmaps,
            stale_pixmaps: Vec::new(),
            stale: false,
            keyboard_moved: false,
        }
    }

    #[must_use]
    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    #[must_use]
    pub fn output(&self) -> OutputId {
        self.output
    }

    /// The mode and the selection, for the app and the tests.
    #[must_use]
    pub fn state(&self) -> &OverviewState {
        &self.state
    }

    /// The surface just appeared: it draws its first frame, which also sets
    /// the input region once the surface's size is known.
    pub fn opened(&self, ctl: &mut SurfaceCtl<'_>) {
        ctl.request_redraw();
    }

    /// The surface is about to go: no input region any more.
    pub fn closing(ctl: &mut SurfaceCtl<'_>) {
        ctl.set_input_region(&[]);
    }

    /// The card's rect in the surface.
    #[must_use]
    pub fn card(&self) -> Rect {
        view::card_rect(self.size, view::card_size(self.size))
    }

    /// A new compositor model. Returns whether a frame is due.
    pub fn set_model(&mut self, model: &NiriModel) -> bool {
        let changed = self.state.set_model(model);
        self.stale |= changed;
        changed
    }

    /// Switches to `mode` for a verb (the tabs switch it inside
    /// [`Overview::input`]). Returns whether a frame is due.
    pub fn set_mode(&mut self, mode: Mode) -> bool {
        let changed = self.state.set_mode(mode);
        self.stale |= changed;
        changed
    }

    /// The bindings the Shortcuts mode shows, just read. Returns whether a
    /// frame is due.
    pub fn set_keybinds(&mut self, keybinds: Keybinds) -> bool {
        let changed = self.state.set_keybinds(keybinds);
        self.stale |= changed;
        changed
    }

    /// Hands the overview workspace `id`'s preview, replacing the one it had.
    /// Returns whether it was kept: at most [`MAX_PREVIEWS`] workspaces have
    /// one (one per output).
    pub fn set_preview(&mut self, id: u64, thumb: Thumb) -> bool {
        let known = self.pending.contains_key(&id) || self.pixmaps.contains_key(&id);
        if !known && self.pending.len() + self.pixmaps.len() >= MAX_PREVIEWS {
            return false;
        }
        if let Some(old) = self.pixmaps.remove(&id) {
            self.stale_pixmaps.push(old.id);
        }
        self.pending.insert(id, thumb);
        self.stale = true;
        true
    }

    /// Frees every texture the overview holds; call before dropping it.
    pub fn release(&mut self, painter: &mut Painter) {
        self.pending.clear();
        for pixmap in self
            .pixmaps
            .drain()
            .map(|(_, p)| p.id)
            .chain(self.stale_pixmaps.drain(..))
        {
            painter.release_pixmap(pixmap);
        }
    }

    /// Uploads the previews that arrived and frees the replaced ones. A
    /// preview the GPU refuses is dropped: its workspace draws without one.
    fn upload(&mut self, painter: &mut Painter) {
        for id in self.stale_pixmaps.drain(..) {
            painter.release_pixmap(id);
        }
        for (id, thumb) in self.pending.drain() {
            match painter.upload_rgba(&thumb.rgba, thumb.width, thumb.height) {
                Ok(pixmap) => {
                    self.pixmaps.insert(
                        id,
                        PreviewPixmap {
                            id: pixmap,
                            size: (thumb.width, thumb.height),
                        },
                    );
                }
                Err(err) => {
                    tracing::warn!(workspace = id, error = %err, "overview: cannot upload a preview")
                }
            }
        }
    }

    /// Draws one frame: the card, the selected tile's ring once the keyboard
    /// moved it, the blur and input regions.
    pub fn frame(&mut self, cx: &mut FrameCx<'_>, painter: &mut Painter, reduced: bool) {
        let now = cx.now;
        self.tree.set_reduced_motion(reduced);
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.tree.set_theme(&self.theme);
            self.stale = true;
        }
        if cx.size != self.size {
            self.size = cx.size;
            self.stale = true;
        }
        if !self.pending.is_empty() || !self.stale_pixmaps.is_empty() {
            self.upload(painter);
            self.stale = true;
        }
        self.dpr = cx.scale as f32;
        self.layout(painter, now);
        painter.begin(self.size, self.dpr, cx.blur_available);
        self.tree.paint(painter, now);
        if let Some(ring) = self.ring() {
            let (rect, radius) = focus_ring(ring, self.theme.radius_md);
            painter.stroke_rounded(rect, radius, FOCUS_WIDTH, self.theme.color(Role::FocusRing));
        }
        painter.end();
        let (input, blur) = regions(self.size, &self.theme);
        cx.ctl.set_blur_region(&blur);
        cx.ctl.set_input_region(&input);
        if self.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }

    /// Builds the view when it is stale and lays the tree out, so input hits
    /// what the next frame paints. Nothing is laid out before the first
    /// frame tells the surface's size.
    fn layout(&mut self, painter: &mut Painter, now: Instant) {
        if !self.sized() {
            return;
        }
        self.apply(now);
        self.tree.layout(self.size, painter.text(), self.dpr);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "overview: layout fault");
        }
    }

    fn apply(&mut self, now: Instant) {
        if self.stale {
            let body = view::body_size(view::card_size(self.size), &self.theme);
            self.state
                .set_viewport(view::shortcut_rows(body.1, &self.theme));
            self.tree.apply(
                view::overview(&self.theme, &self.state, self.size, &self.pixmaps),
                now,
            );
            self.stale = false;
        }
    }

    fn sized(&self) -> bool {
        self.size.0 > 0.0 && self.size.1 > 0.0
    }

    /// The selected tile's rect (or the selected empty workspace's header)
    /// while the keyboard moved the selection, in Desktop mode.
    fn ring(&self) -> Option<Rect> {
        if !self.keyboard_moved || self.state.mode != Mode::Desktop {
            return None;
        }
        let key = match self.state.target()? {
            Target::Window(id) => view::window_key(id),
            Target::Workspace(id) => view::workspace_key(id),
        };
        Some(self.tree.rect(self.tree.node_by_key(key)?))
    }

    /// Handles one input event on the overview's surface; `painter` measures
    /// the view if it changed since the last frame. Returns what the app
    /// must do: everything but [`OverviewAction::SwitchMode`] closes it.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        painter: &mut Painter,
    ) -> Option<OverviewAction> {
        let now = Instant::now();
        self.layout(painter, now);
        let outcome = self.handle(ev, now);
        if outcome.changed {
            self.layout(painter, now);
            ctl.request_redraw();
        }
        outcome.action
    }

    /// [`Overview::input`] without the painter and the surface.
    fn handle(&mut self, ev: &InputEvent, now: Instant) -> Outcome {
        let mut outcome = Outcome::default();
        match ev {
            InputEvent::FocusLost => outcome.action = Some(OverviewAction::Close),
            InputEvent::Key {
                keysym,
                pressed: true,
                ..
            } => self.key(*keysym, &mut outcome),
            InputEvent::PointerPress { x, y, .. }
                if self.sized() && !self.card().contains(*x, *y) =>
            {
                outcome.action = Some(OverviewAction::Close);
            }
            InputEvent::PointerPress { .. }
            | InputEvent::PointerRelease { .. }
            | InputEvent::PointerMotion { .. }
            | InputEvent::PointerLeave
            | InputEvent::Axis { .. } => {
                if matches!(ev, InputEvent::PointerPress { .. }) {
                    self.keyboard_moved = false;
                }
                let before = self.tree.highlight();
                let events = self.input.pointer(&mut self.tree, ev, now);
                outcome.changed = !events.is_empty() || self.tree.highlight() != before;
                for event in events {
                    if let UiEvent::Click(key) = event {
                        self.click(key, &mut outcome);
                    }
                }
            }
            _ => {}
        }
        outcome
    }

    /// A key press: Escape closes, Enter chooses, arrows move, Tab, `1` and
    /// `2` switch the mode.
    fn key(&mut self, keysym: u32, outcome: &mut Outcome) {
        match keysym {
            XKB_KEY_ESCAPE => outcome.action = Some(OverviewAction::Close),
            XKB_KEY_TAB | XKB_KEY_ISO_LEFT_TAB => self.switch(self.state.mode.other(), outcome),
            XKB_KEY_1 => self.switch(Mode::Desktop, outcome),
            XKB_KEY_2 => self.switch(Mode::Shortcuts, outcome),
            XKB_KEY_RETURN | XKB_KEY_KP_ENTER if self.state.mode == Mode::Desktop => {
                outcome.action = self.state.target().map(action_of);
            }
            XKB_KEY_LEFT | XKB_KEY_UP | XKB_KEY_RIGHT | XKB_KEY_DOWN
                if self.state.mode == Mode::Desktop =>
            {
                let dir = match keysym {
                    XKB_KEY_LEFT => Dir::Left,
                    XKB_KEY_UP => Dir::Up,
                    XKB_KEY_RIGHT => Dir::Right,
                    _ => Dir::Down,
                };
                self.keyboard_moved = true;
                if self.state.move_selection(dir) {
                    self.stale = true;
                }
                outcome.changed = true;
            }
            XKB_KEY_UP | XKB_KEY_DOWN | XKB_KEY_PAGE_UP | XKB_KEY_PAGE_DOWN | XKB_KEY_HOME
            | XKB_KEY_END
                if self.state.mode == Mode::Shortcuts =>
            {
                let step = match keysym {
                    XKB_KEY_UP => Scroll::Up,
                    XKB_KEY_DOWN => Scroll::Down,
                    XKB_KEY_PAGE_UP => Scroll::PageUp,
                    XKB_KEY_PAGE_DOWN => Scroll::PageDown,
                    XKB_KEY_HOME => Scroll::Home,
                    _ => Scroll::End,
                };
                if self.state.scroll_by(step) {
                    self.stale = true;
                    outcome.changed = true;
                }
            }
            _ => {}
        }
    }

    fn switch(&mut self, mode: Mode, outcome: &mut Outcome) {
        if self.state.set_mode(mode) {
            self.stale = true;
            outcome.changed = true;
            outcome.action = Some(OverviewAction::SwitchMode);
        }
    }

    /// A click on the node `key`: a tab switches, a tile focuses its window,
    /// a header its workspace.
    fn click(&mut self, key: celestina_ui::Key, outcome: &mut Outcome) {
        if let Some(mode) = view::tab_of(key) {
            self.switch(mode, outcome);
            return;
        }
        if self.state.mode != Mode::Desktop {
            return;
        }
        let ids: Vec<(u64, Vec<u64>)> = self
            .state
            .workspaces
            .iter()
            .map(|ws| (ws.id, ws.grid.iter().flatten().copied().collect()))
            .collect();
        for (workspace, windows) in ids {
            if view::workspace_key(workspace) == key && self.state.select_workspace(workspace) {
                outcome.action = Some(OverviewAction::FocusWorkspace(workspace));
                return;
            }
            if let Some(&window) = windows.iter().find(|&&w| view::window_key(w) == key) {
                self.state.select_window(window);
                outcome.action = Some(OverviewAction::Focus(window));
                return;
            }
        }
    }
}

/// What choosing `target` asks for.
fn action_of(target: Target) -> OverviewAction {
    match target {
        Target::Window(id) => OverviewAction::Focus(id),
        Target::Workspace(id) => OverviewAction::FocusWorkspace(id),
    }
}

/// The input and blur regions of an overview surface of `size` in `theme`:
/// the input region is the whole surface, so a press outside the card
/// reaches the overview and not the window under it; the blur region is the
/// card's silhouette.
#[must_use]
pub fn regions(size: (f32, f32), theme: &Theme) -> (Vec<Rect>, Vec<Rect>) {
    let card = view::card_rect(size, view::card_size(size));
    (
        vec![Rect::whole(size)],
        silhouette_slices(card, theme.radius_lg, SILHOUETTE_STEP),
    )
}

/// What one input event did.
#[derive(Debug, Default)]
struct Outcome {
    action: Option<OverviewAction>,
    /// The overview's look changed: a frame is due.
    changed: bool,
}

#[cfg(test)]
mod tests {
    use celestina_wayland::Modifiers;

    use super::{state::fixtures::model, *};

    fn key(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 1,
        }
    }

    fn overview() -> Overview {
        Overview::new(
            SurfaceId(5),
            OutputId(1),
            "DP-1",
            &Theme::dark(),
            false,
            &model(),
        )
    }

    fn act(o: &mut Overview, ev: &InputEvent) -> Option<OverviewAction> {
        o.handle(ev, Instant::now()).action
    }

    fn sized(o: &mut Overview, size: (f32, f32)) {
        o.size = size;
        o.stale = true;
        o.apply(Instant::now());
        o.tree
            .layout_measured(size, |s, _, _| (7.0 * s.len() as f32, 16.0));
    }

    fn press(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerPress {
            x,
            y,
            button: 0x110,
            serial: 2,
        }
    }

    #[test]
    fn arrows_move_the_selection_and_enter_focuses_it() {
        let mut o = overview();
        assert!(!o.keyboard_moved);
        assert_eq!(act(&mut o, &key(XKB_KEY_RIGHT)), None);
        assert!(o.keyboard_moved, "the ring shows after an arrow");
        assert_eq!(
            act(&mut o, &key(XKB_KEY_RETURN)),
            Some(OverviewAction::Focus(11))
        );
        assert_eq!(act(&mut o, &key(XKB_KEY_DOWN)), None);
        assert_eq!(
            act(&mut o, &key(XKB_KEY_KP_ENTER)),
            Some(OverviewAction::Focus(12))
        );
        // Past the column's last row: the empty workspace, which Enter focuses.
        act(&mut o, &key(XKB_KEY_DOWN));
        assert_eq!(
            act(&mut o, &key(XKB_KEY_RETURN)),
            Some(OverviewAction::FocusWorkspace(2))
        );
    }

    #[test]
    fn escape_and_focus_loss_close() {
        let mut o = overview();
        assert_eq!(
            act(&mut o, &key(XKB_KEY_ESCAPE)),
            Some(OverviewAction::Close)
        );
        assert_eq!(
            act(&mut o, &InputEvent::FocusLost),
            Some(OverviewAction::Close)
        );
    }

    #[test]
    fn tab_one_and_two_switch_the_mode_without_closing() {
        let mut o = overview();
        assert_eq!(
            act(&mut o, &key(XKB_KEY_TAB)),
            Some(OverviewAction::SwitchMode)
        );
        assert_eq!(o.state().mode, Mode::Shortcuts);
        assert_eq!(act(&mut o, &key(XKB_KEY_2)), None, "already there: nothing");
        assert_eq!(
            act(&mut o, &key(XKB_KEY_RETURN)),
            None,
            "Enter does nothing here"
        );
        assert_eq!(
            act(&mut o, &key(XKB_KEY_1)),
            Some(OverviewAction::SwitchMode)
        );
        assert_eq!(o.state().mode, Mode::Desktop);
        assert_eq!(
            act(&mut o, &key(XKB_KEY_ISO_LEFT_TAB)),
            Some(OverviewAction::SwitchMode)
        );
    }

    #[test]
    fn shortcuts_keys_scroll_the_rows_and_never_close() {
        use celestina_shell::keybinds::{Bind, BindGroup};
        let mut o = overview();
        sized(&mut o, (1920.0, 1080.0));
        act(&mut o, &key(XKB_KEY_2));
        assert!(o.set_keybinds(Keybinds::Groups(vec![BindGroup {
            name: None,
            enter: None,
            binds: (0..100)
                .map(|i| Bind {
                    chord: format!("Mod+{i}"),
                    title: None,
                    action: String::new(),
                    program: None,
                })
                .collect(),
        }])));
        o.apply(Instant::now());
        assert_eq!(act(&mut o, &key(XKB_KEY_DOWN)), None);
        assert_eq!(o.state().scroll_offset(), 1);
        assert_eq!(act(&mut o, &key(XKB_KEY_END)), None);
        let last = o.state().scroll_offset();
        assert!(last > 1);
        assert_eq!(act(&mut o, &key(XKB_KEY_PAGE_UP)), None);
        assert!(o.state().scroll_offset() < last);
        assert_eq!(act(&mut o, &key(XKB_KEY_HOME)), None);
        assert_eq!(o.state().scroll_offset(), 0);
        assert_eq!(
            act(&mut o, &key(XKB_KEY_RETURN)),
            None,
            "Enter acts on Desktop only"
        );
    }

    #[test]
    fn an_empty_model_chooses_nothing_and_still_closes() {
        let mut o = Overview::new(
            SurfaceId(5),
            OutputId(1),
            "DP-1",
            &Theme::dark(),
            false,
            &NiriModel::default(),
        );
        assert_eq!(act(&mut o, &key(XKB_KEY_RETURN)), None);
        assert_eq!(act(&mut o, &key(XKB_KEY_DOWN)), None);
        assert_eq!(
            act(&mut o, &key(XKB_KEY_ESCAPE)),
            Some(OverviewAction::Close)
        );
    }

    #[test]
    fn a_press_outside_the_card_closes_and_inside_does_not() {
        let mut o = overview();
        assert_eq!(
            act(&mut o, &press(10.0, 10.0)),
            None,
            "no size yet: no card"
        );
        sized(&mut o, (1920.0, 1080.0));
        let card = o.card();
        // The card is capped at 1680 wide and centred on whole pixels.
        assert_eq!((card.x, card.y), (120.0, 48.0));
        for (x, y) in [
            (10.0, 10.0),
            (card.x - 1.0, card.y + 10.0),
            (card.x + card.w, 100.0),
            (1919.0, 1079.0),
        ] {
            assert_eq!(
                act(&mut o, &press(x, y)),
                Some(OverviewAction::Close),
                "({x}, {y})"
            );
        }
        assert_eq!(act(&mut o, &press(card.x + 2.0, card.y + 2.0)), None);
    }

    #[test]
    fn a_click_on_a_tile_focuses_it_a_header_its_workspace_and_a_tab_switches() {
        let mut o = overview();
        sized(&mut o, (1920.0, 1080.0));
        let centre = |o: &Overview, key| {
            o.tree.node_by_key(key).map(|id| {
                let r = o.tree.rect(id);
                (r.x + r.w / 2.0, r.y + r.h / 2.0)
            })
        };
        let click = |o: &mut Overview, (x, y): (f32, f32)| {
            let now = Instant::now();
            assert_eq!(o.handle(&press(x, y), now).action, None);
            o.handle(
                &InputEvent::PointerRelease {
                    x,
                    y,
                    button: 0x110,
                    serial: 3,
                },
                now,
            )
            .action
        };
        let Some(tile) = centre(&o, view::window_key(30)) else {
            panic!("window 30 is laid out");
        };
        assert_eq!(click(&mut o, tile), Some(OverviewAction::Focus(30)));
        assert_eq!(o.state().target(), Some(Target::Window(30)));
        let Some(header) = centre(&o, view::workspace_key(2)) else {
            panic!("workspace 2 is laid out");
        };
        assert_eq!(
            click(&mut o, header),
            Some(OverviewAction::FocusWorkspace(2))
        );
        let Some(tab) = centre(&o, view::tab_key(Mode::Shortcuts)) else {
            panic!("the tab is laid out");
        };
        assert_eq!(click(&mut o, tab), Some(OverviewAction::SwitchMode));
        assert_eq!(o.state().mode, Mode::Shortcuts);
    }

    #[test]
    fn the_input_region_is_the_surface_and_the_blur_the_card() {
        let theme = Theme::dark();
        let surface = (1920.0, 1080.0);
        let (input, blur) = regions(surface, &theme);
        assert_eq!(input, [Rect::whole(surface)]);
        let card = view::card_rect(surface, view::card_size(surface));
        assert_eq!(
            blur,
            silhouette_slices(card, theme.radius_lg, SILHOUETTE_STEP)
        );
        assert!(blur.iter().all(|r| r.x >= card.x
            && r.y >= card.y
            && r.x + r.w <= card.x + card.w
            && r.y + r.h <= card.y + card.h));
    }

    #[test]
    fn a_new_model_marks_a_frame_due_only_when_it_changed() {
        let mut o = overview();
        assert!(!o.set_model(&model()));
        let mut m = model();
        m.windows.pop();
        assert!(o.set_model(&m));
    }

    #[test]
    fn the_debug_form_prints_no_pixels() {
        let mut o = overview();
        o.set_preview(
            7,
            Thumb {
                width: 1,
                height: 1,
                rgba: vec![231, 232, 233, 255],
            },
        );
        let text = format!("{o:?}");
        assert!(text.contains("pending_previews: 1"), "{text}");
        assert!(!text.contains("231") && !text.contains("rgba"), "{text}");
    }

    #[test]
    fn previews_are_bounded_and_a_newer_one_replaces_the_old() {
        let mut o = overview();
        let thumb = |w: u32| Thumb {
            width: w,
            height: 1,
            rgba: vec![0; w as usize * 4],
        };
        for id in 0..MAX_PREVIEWS as u64 {
            assert!(o.set_preview(id, thumb(2)));
        }
        assert!(!o.set_preview(999, thumb(2)), "one more than the bound");
        assert!(
            o.set_preview(0, thumb(3)),
            "a known workspace may be replaced"
        );
        assert_eq!(o.pending.len(), MAX_PREVIEWS);
        assert_eq!(o.pending.get(&0).map(|t| t.width), Some(3));
    }
}
