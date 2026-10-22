//! The launcher (design §5.5, Launcher): a Haze card with a search field on
//! top and up to eight rows under it, centred on its own layer surface
//! (`celestina-overlay`, `Layer::Overlay`, anchored to every edge so it
//! fills the output, keyboard `Exclusive`). Outside the card the surface is
//! fully transparent: no dim, and the compositor blur covers the card only.
//! [`state`] is the pure query and selection, [`view`] the pure view,
//! [`commands`] the `/` switches, [`spawn`] the start of an application and
//! [`recent`] the launches the ranking lifts; this file owns one open
//! launcher: its tree, its frame and its input.
//!
//! The app creates the surface on `celestina launcher toggle` and destroys
//! it on close. The launcher closes on Escape, on a launch or a switch
//! chosen with Enter or a click, on a press anywhere outside the card, and
//! on keyboard focus loss. The input region is the whole surface, so a
//! press outside the card reaches the launcher, never the window under it;
//! closing on it does not wait for the compositor to move the keyboard.
//! Nothing depends on focus-follows-mouse.
//!
//! The field holds the focus from the start, so typing goes to it; its
//! caret marks it. Up and Down move the selected row (wrapping), which then
//! also shows the focus ring: the ring appears only once the keyboard moved
//! the selection. Ctrl+V asks the runtime for the clipboard's text; the
//! input method's composition and commits and the pasted text edit the field
//! like keys do. While the launcher is open its surface asks for IME text
//! input with the caret's rectangle.

pub mod commands;
pub mod recent;
pub mod spawn;
pub mod state;
pub mod view;

use std::time::Instant;

use celestina_core::desktop_entry::DesktopEntry;
use celestina_paint::{silhouette_slices, Painter, Rect, Role, Theme};
use celestina_ui::{focus_ring, Interaction, Tree, UiEvent, FOCUS_WIDTH};
use celestina_wayland::{FrameCx, InputEvent, Modifiers, OutputId, SurfaceCtl, SurfaceId};

pub use commands::{commands_for, Switch};
pub use state::{LauncherState, Row};

use crate::island::state::SILHOUETTE_STEP;

/// The launcher's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-overlay";

const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_DOWN: u32 = 0xff54;
const XKB_KEY_LOWER_V: u32 = 0x76;
const XKB_KEY_UPPER_V: u32 = 0x56;

/// What the launcher asks the app to do; each one closes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LauncherAction {
    /// Start this application.
    Launch(Box<DesktopEntry>),
    /// Run this switch.
    Switch(Switch),
    /// Close without acting.
    Close,
}

/// One open launcher.
#[derive(Debug)]
pub struct Launcher {
    surface: SurfaceId,
    output: OutputId,
    /// The surface's logical size, (0, 0) until its first frame: the
    /// compositor sizes the surface to the output.
    size: (f32, f32),
    dpr: f32,
    theme: Theme,
    tree: Tree,
    input: Interaction,
    state: LauncherState,
    /// The view must be built again before the next layout.
    stale: bool,
    /// The keyboard moved the selection: the selected row shows the ring.
    keyboard_moved: bool,
}

impl Launcher {
    /// The launcher on `surface` of `output`, in `theme` under `reduced`
    /// motion, over `index` with the `recent` launches (most recent first),
    /// its field empty and focused.
    #[must_use]
    pub fn new(
        surface: SurfaceId,
        output: OutputId,
        theme: &Theme,
        reduced: bool,
        recent: Vec<String>,
        index: &[DesktopEntry],
    ) -> Launcher {
        let mut state = LauncherState::new(recent);
        state.update(index);
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view::launcher(theme, &state, (0.0, 0.0)), Instant::now());
        let mut input = Interaction::new();
        // The field is the only node in the Tab order: a Tab focuses it.
        input.key(
            &mut tree,
            &InputEvent::Key {
                keysym: XKB_KEY_TAB,
                utf8: None,
                modifiers: Modifiers::default(),
                pressed: true,
                serial: 0,
            },
        );
        Launcher {
            surface,
            output,
            size: (0.0, 0.0),
            dpr: 1.0,
            theme: theme.clone(),
            tree,
            input,
            state,
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

    /// The query and the rows, for the app and the tests.
    #[must_use]
    pub fn state(&self) -> &LauncherState {
        &self.state
    }

    /// The surface just appeared: it takes IME text input and draws its
    /// first frame, which also sets the input region once the surface's
    /// size is known.
    pub fn opened(&self, ctl: &mut SurfaceCtl<'_>) {
        ctl.set_text_input_enabled(true);
        ctl.request_redraw();
    }

    /// The card's rect in the surface.
    #[must_use]
    pub fn card(&self) -> Rect {
        view::card_rect(self.size, view::card_size(&self.theme))
    }

    /// The surface is about to go: no more text input for it.
    pub fn closing(ctl: &mut SurfaceCtl<'_>) {
        ctl.set_text_input_enabled(false);
        ctl.set_input_region(&[]);
    }

    /// A new desktop index: the query is answered again. Returns whether
    /// the rows changed (a frame is due).
    pub fn set_index(&mut self, index: &[DesktopEntry]) -> bool {
        let before = self.state.rows.clone();
        self.state.update(index);
        self.stale |= before != self.state.rows;
        self.stale
    }

    /// Draws one frame: the card, the selected row's ring once the keyboard
    /// moved it, the blur and input regions, the caret for the input method.
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
        if let Some(caret) = self.caret() {
            cx.ctl.set_text_input_rect(caret);
        }
        if self.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }

    /// Builds the view when it is stale and lays the tree out (a no-op when
    /// nothing changed), so input hits what the next frame paints. Nothing
    /// is laid out before the first frame tells the surface's size, so the
    /// card never slides from a provisional place.
    fn layout(&mut self, painter: &mut Painter, now: Instant) {
        if !self.sized() {
            return;
        }
        self.apply(now);
        self.tree.layout(self.size, painter.text(), self.dpr);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "launcher: layout fault");
        }
    }

    fn apply(&mut self, now: Instant) {
        if self.stale {
            self.tree
                .apply(view::launcher(&self.theme, &self.state, self.size), now);
            self.stale = false;
        }
    }

    /// The first frame told the surface's size.
    fn sized(&self) -> bool {
        self.size.0 > 0.0 && self.size.1 > 0.0
    }

    /// The selected row's rect while the keyboard moved the selection.
    fn ring(&self) -> Option<Rect> {
        if !self.keyboard_moved || self.state.rows.is_empty() {
            return None;
        }
        let id = self.tree.node_by_key(view::row_key(self.state.selected))?;
        Some(self.tree.rect(id))
    }

    /// The caret's rectangle in the surface, for the input method.
    fn caret(&self) -> Option<Rect> {
        let node = self.tree.node_by_key(view::field_key())?;
        let field = self.tree.field(view::FIELD)?;
        let rect = self.tree.rect(node);
        Some(Rect {
            x: rect.x + field.caret_x() - field.scroll_x(),
            y: rect.y,
            w: 1.0,
            h: rect.h,
        })
    }

    /// Handles one input event on the launcher's surface; `painter`
    /// measures the view if it changed since the last frame, and `index` is
    /// what a changed query is answered from. Returns what the app must do,
    /// which always closes the launcher.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        painter: &mut Painter,
        index: &[DesktopEntry],
    ) -> Option<LauncherAction> {
        let now = Instant::now();
        self.layout(painter, now);
        let outcome = self.handle(ev, index, now);
        if outcome.paste {
            ctl.request_paste();
        }
        if outcome.changed {
            self.layout(painter, now);
            if let Some(caret) = self.caret() {
                ctl.set_text_input_rect(caret);
            }
            ctl.request_redraw();
        }
        outcome.action
    }

    /// [`Launcher::input`] without the painter and the surface.
    fn handle(&mut self, ev: &InputEvent, index: &[DesktopEntry], now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let mut outcome = Outcome::default();
        let events = match ev {
            InputEvent::FocusLost => {
                outcome.action = Some(LauncherAction::Close);
                return outcome;
            }
            InputEvent::FocusEnter | InputEvent::PopupClosed { .. } => return outcome,
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
                pressed: true,
                ..
            } => {
                self.state
                    .move_selection(if *keysym == XKB_KEY_UP { -1 } else { 1 });
                self.keyboard_moved = true;
                self.stale = true;
                outcome.changed = true;
                return outcome;
            }
            InputEvent::Key {
                keysym: XKB_KEY_LOWER_V | XKB_KEY_UPPER_V,
                modifiers:
                    Modifiers {
                        ctrl: true,
                        alt: false,
                        logo: false,
                        ..
                    },
                pressed: true,
                ..
            } => {
                outcome.paste = true;
                return outcome;
            }
            InputEvent::Key { .. }
            | InputEvent::Preedit { .. }
            | InputEvent::CommitText(_)
            | InputEvent::Paste(_) => self.input.key(&mut self.tree, ev),
            InputEvent::PointerPress { x, y, .. }
                if self.sized() && !self.card().contains(*x, *y) =>
            {
                outcome.action = Some(LauncherAction::Close);
                return outcome;
            }
            InputEvent::PointerPress { .. } => {
                self.keyboard_moved = false;
                self.input.pointer(&mut self.tree, ev, now)
            }
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        outcome.changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::FieldChanged(view::FIELD) => {
                    let text = self
                        .tree
                        .field(view::FIELD)
                        .map(|f| f.text().to_owned())
                        .unwrap_or_default();
                    if self.state.set_query(&text, index) {
                        self.keyboard_moved = false;
                        self.stale = true;
                    }
                }
                UiEvent::Escape => {
                    outcome.action = Some(LauncherAction::Close);
                    return outcome;
                }
                UiEvent::Activate(key) if key == view::field_key() => {
                    if let Some(action) = self.chosen() {
                        outcome.action = Some(action);
                        return outcome;
                    }
                }
                UiEvent::Click(key) => {
                    if let Some(row) = view::row_of(key, self.state.rows.len()) {
                        self.state.selected = row;
                        if let Some(action) = self.chosen() {
                            outcome.action = Some(action);
                            return outcome;
                        }
                    }
                }
                UiEvent::FieldChanged(_)
                | UiEvent::Activate(_)
                | UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        outcome.changed |= self.stale;
        outcome
    }

    /// What choosing the selected row asks for.
    fn chosen(&self) -> Option<LauncherAction> {
        match self.state.chosen()? {
            Row::App(entry) => Some(LauncherAction::Launch(entry.clone())),
            Row::Switch(switch) => Some(LauncherAction::Switch(*switch)),
        }
    }
}

/// The input and blur regions of a launcher surface of `size` in `theme`:
/// the input region is the whole surface, so a press outside the card
/// reaches the launcher and not the window under it; the blur region is the
/// card's silhouette, so the compositor blurs nothing outside it.
#[must_use]
pub fn regions(size: (f32, f32), theme: &Theme) -> (Vec<Rect>, Vec<Rect>) {
    let card = view::card_rect(size, view::card_size(theme));
    (
        vec![Rect::whole(size)],
        silhouette_slices(card, theme.radius_lg, SILHOUETTE_STEP),
    )
}

/// What one input event did.
#[derive(Debug, Default)]
struct Outcome {
    action: Option<LauncherAction>,
    /// The launcher's look changed: a frame is due.
    changed: bool,
    /// Ctrl+V: the clipboard's text is wanted.
    paste: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETURN: u32 = 0xff0d;
    const ESCAPE: u32 = 0xff1b;

    fn entry(id: &str, name: &str) -> DesktopEntry {
        DesktopEntry {
            id: format!("{id}.desktop"),
            name: name.to_owned(),
            exec: id.to_owned(),
            is_application: true,
            ..DesktopEntry::default()
        }
    }

    fn key(keysym: u32, text: Option<&str>, ctrl: bool) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: text.map(str::to_owned),
            modifiers: Modifiers {
                ctrl,
                ..Modifiers::default()
            },
            pressed: true,
            serial: 1,
        }
    }

    fn launcher(index: &[DesktopEntry]) -> Launcher {
        Launcher::new(
            SurfaceId(5),
            OutputId(1),
            &Theme::dark(),
            false,
            Vec::new(),
            index,
        )
    }

    fn typed(l: &mut Launcher, text: &str, index: &[DesktopEntry]) {
        for c in text.chars() {
            let s = c.to_string();
            let o = l.handle(&key(u32::from(c), Some(&s), false), index, Instant::now());
            assert_eq!(o.action, None);
        }
    }

    #[test]
    fn typing_answers_the_query_and_enter_launches_the_selected_row() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("ac", "Ac")];
        let mut l = launcher(&index);
        assert_eq!(l.state().rows.len(), 3, "an empty query lists the index");
        typed(&mut l, "a", &index);
        assert_eq!(l.state().query.text(), "a");
        let down = l.handle(&key(XKB_KEY_DOWN, None, false), &index, Instant::now());
        assert!(down.changed && down.action.is_none());
        assert!(l.keyboard_moved, "the ring shows after the arrow");
        let enter = l.handle(&key(RETURN, None, false), &index, Instant::now());
        assert_eq!(
            enter.action,
            Some(LauncherAction::Launch(Box::new(index[1].clone())))
        );
    }

    #[test]
    fn a_slash_lists_the_switches_and_an_unknown_command_chooses_nothing() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "/ban", &index);
        assert_eq!(l.state().rows, [Row::Switch(Switch::Bandeja)]);
        let enter = l.handle(&key(RETURN, None, false), &index, Instant::now());
        assert_eq!(enter.action, Some(LauncherAction::Switch(Switch::Bandeja)));

        let mut l = launcher(&index);
        typed(&mut l, "/zzz", &index);
        assert!(l.state().rows.is_empty());
        let enter = l.handle(&key(RETURN, None, false), &index, Instant::now());
        assert_eq!(enter.action, None, "Enter on nothing does nothing");
    }

    #[test]
    fn escape_and_focus_loss_close_and_ctrl_v_asks_for_the_clipboard() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        let paste = l.handle(
            &key(XKB_KEY_LOWER_V, Some("v"), true),
            &index,
            Instant::now(),
        );
        assert!(paste.paste && paste.action.is_none());
        assert_eq!(l.state().query.text(), "", "Ctrl+V types nothing itself");
        let pasted = l.handle(
            &InputEvent::Paste("a\nb".to_owned()),
            &index,
            Instant::now(),
        );
        assert!(pasted.changed);
        assert_eq!(l.state().query.text(), "a b");
        assert_eq!(
            l.handle(&key(ESCAPE, None, false), &index, Instant::now())
                .action,
            Some(LauncherAction::Close)
        );
        assert_eq!(
            l.handle(&InputEvent::FocusLost, &index, Instant::now())
                .action,
            Some(LauncherAction::Close)
        );
    }

    /// What the first frame does to the tree, with a fixed text measure.
    fn sized(l: &mut Launcher, size: (f32, f32)) {
        l.size = size;
        l.stale = true;
        l.apply(Instant::now());
        l.tree
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
    fn the_input_region_is_the_surface_and_the_blur_the_card() {
        let theme = Theme::dark();
        let surface = (2560.0, 1440.0);
        let (input, blur) = regions(surface, &theme);
        assert_eq!(input, [Rect::whole(surface)]);
        let card = view::card_rect(surface, view::card_size(&theme));
        assert_eq!(
            blur,
            silhouette_slices(card, theme.radius_lg, SILHOUETTE_STEP)
        );
        assert!(!blur.is_empty());
        assert!(
            blur.iter().all(|r| r.x >= card.x
                && r.y >= card.y
                && r.x + r.w <= card.x + card.w
                && r.y + r.h <= card.y + card.h),
            "nothing outside the card is blurred"
        );
    }

    #[test]
    fn a_press_outside_the_card_closes_and_inside_does_not() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut l = launcher(&index);
        sized(&mut l, (2560.0, 1440.0));
        let card = l.card();
        assert_eq!((card.x, card.y), (1000.0, 520.0));
        for (x, y) in [
            (10.0, 10.0),
            (card.x - 1.0, card.y + 10.0),
            (card.x + card.w, card.y + 10.0),
            (card.x + 10.0, card.y + card.h),
            (2559.0, 1439.0),
        ] {
            assert_eq!(
                l.handle(&press(x, y), &index, Instant::now()).action,
                Some(LauncherAction::Close),
                "a press at ({x}, {y}) is outside the card"
            );
        }
        // Inside the card, on its padding: nothing happens.
        let o = l.handle(&press(card.x + 2.0, card.y + 2.0), &index, Instant::now());
        assert_eq!(o.action, None);
        // A click on a row still launches it.
        let row = l
            .tree
            .node_by_key(view::row_key(1))
            .map(|id| l.tree.rect(id));
        let Some(row) = row else {
            panic!("the second row is laid out");
        };
        assert!(card.contains(row.x, row.y));
        let (x, y) = (row.x + row.w / 2.0, row.y + row.h / 2.0);
        let now = Instant::now();
        assert_eq!(l.handle(&press(x, y), &index, now).action, None);
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 3,
        };
        assert_eq!(
            l.handle(&release, &index, now).action,
            Some(LauncherAction::Launch(Box::new(index[1].clone())))
        );
    }

    #[test]
    fn nothing_is_closed_or_laid_out_before_the_surface_has_a_size() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        assert!(!l.sized());
        assert_eq!(
            l.handle(&press(10.0, 10.0), &index, Instant::now()).action,
            None,
            "no size yet: no card to be outside of"
        );
    }

    #[test]
    fn the_query_is_clamped_to_the_fields_limit() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        let long = "x".repeat(celestina_ui::MAX_CHARS * 4);
        l.handle(&InputEvent::Paste(long), &index, Instant::now());
        assert_eq!(
            l.state().query.text().chars().count(),
            celestina_ui::MAX_CHARS
        );
    }

    #[test]
    fn a_new_index_answers_the_open_query_again() {
        let mut l = launcher(&[]);
        assert!(l.state().rows.is_empty());
        let index = [entry("aa", "Aa")];
        assert!(l.set_index(&index));
        assert_eq!(l.state().rows, [Row::App(Box::new(index[0].clone()))]);
    }
}
