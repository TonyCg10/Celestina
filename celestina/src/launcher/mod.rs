//! The launcher (design §5.5, Launcher): a Haze card with a search field on
//! top and up to eight rows under it, centred on its own layer surface
//! (`celestina-overlay`, `Layer::Overlay`, anchored to every edge so it
//! fills the output, keyboard `Exclusive`). Outside the card the surface is
//! fully transparent: no dim, and the compositor blur covers the card only.
//! [`state`] is the pure query and selection, [`view`] the pure view,
//! [`commands`] the `/` switches, [`spawn`] the start of an application and
//! [`usage`] the use the ranking lifts ([`recent`] only seeds it); this file owns one open
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
//!
//! The same overlay is the `Mod+K` selector of the minimised windows
//! (CEL-2-J, plan decision D3; [`selector`]): [`Launcher::selector`] opens
//! it over the bubbles source. There Tab and Shift+Tab move the selection
//! as Up and Down do, Enter or a click restores the selected window, and
//! Delete asks the selected window to close without closing the selector
//! (the row leaves when Melibea's next projection does; Delete edits the
//! field only while no window row is selected). Escape, a press outside the
//! card and focus loss close it as they close the launcher.

pub mod commands;
pub mod groups;
pub mod recent;
pub mod selector;
pub mod spawn;
pub mod state;
pub mod usage;
pub mod view;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use celestina_core::desktop_entry::{DesktopEntry, Locale};
use celestina_paint::{silhouette_slices, Painter, Rect, Role, Theme};
use celestina_ui::{focus_ring, Interaction, Scroll, Tree, UiEvent, FOCUS_WIDTH, ONE_UI};
use celestina_wayland::{FrameCx, InputEvent, Modifiers, OutputId, SurfaceCtl, SurfaceId};

pub use commands::{commands_for, Switch};
pub use state::{AppRow, LauncherState, ListView, Row, Source, Tab};

use crate::scrolling::{self, THUMB_LINGER};

use crate::bubbles::Bubbles;
use crate::island::state::SILHOUETTE_STEP;

/// The launcher's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-overlay";

const XKB_KEY_BACKSPACE: u32 = 0xff08;
const XKB_KEY_TAB: u32 = 0xff09;
const XKB_KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const XKB_KEY_DELETE: u32 = 0xffff;
const XKB_KEY_UP: u32 = 0xff52;
const XKB_KEY_DOWN: u32 = 0xff54;
const XKB_KEY_LEFT: u32 = 0xff51;
const XKB_KEY_RIGHT: u32 = 0xff53;
const XKB_KEY_LOWER_G: u32 = 0x67;
const XKB_KEY_UPPER_G: u32 = 0x47;
const XKB_KEY_LOWER_V: u32 = 0x76;
const XKB_KEY_UPPER_V: u32 = 0x56;

/// How long a held Backspace sweeps the query before it empties it.
pub const SWEEP: Duration = Duration::from_millis(450);

/// How long Backspace must stay down before the sweep starts: niri's default
/// key-repeat delay, so the hold feels like the start of a repeat. The
/// launcher times the hold itself so the sweep does not wait on the
/// compositor's delay; a repeat that arrives first starts it at once.
pub const HOLD_DELAY: Duration = Duration::from_millis(600);

/// The state of a Backspace press (design §5.3): the first press deletes one
/// grapheme as the field does; held for [`HOLD_DELAY`] (or repeated), with
/// text left to sweep, it starts the sweep, which empties the query at its
/// end unless the key is released first.
#[derive(Clone, Copy, Debug)]
enum Hold {
    Idle,
    /// A plain Backspace went down at `since` and is not released.
    Pressed {
        since: Instant,
    },
    /// Backspace went down during a composition: until it is released it
    /// edits as the field does and never sweeps.
    Blocked,
    Sweeping {
        since: Instant,
    },
}

/// How long the launcher stays open after the calculator's result is copied,
/// so the row can say "Copied" before it closes.
pub const COPY_CLOSE: Duration = Duration::from_millis(350);

/// The most cut names [`Launcher`] keeps: a few screens of grid cells.
const NAMES_KEPT: usize = 512;

/// What the launcher asks the app to do; each one closes it except
/// [`LauncherAction::RequestClose`], [`LauncherAction::Copy`] and
/// [`LauncherAction::SetView`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LauncherAction {
    /// Start this application.
    Launch(Box<DesktopEntry>),
    /// Run this switch.
    Switch(Switch),
    /// The selector: restore this minimised window.
    Restore(u64),
    /// The selector: ask this minimised window to close; the selector
    /// stays open and the row stays until Melibea's projection drops it.
    RequestClose(u64),
    /// Copy this text (the calculator's result) to the clipboard; the
    /// launcher stays open for [`COPY_CLOSE`] to say so, then finishes.
    Copy(String),
    /// The author switched between the list and the grid: the app keeps the
    /// choice; the launcher stays open.
    SetView(ListView),
    /// Close without acting.
    Close,
}

impl LauncherAction {
    /// Whether the overlay closes for this action.
    #[must_use]
    pub fn closes(&self) -> bool {
        !matches!(
            self,
            LauncherAction::RequestClose(_) | LauncherAction::Copy(_) | LauncherAction::SetView(_)
        )
    }
}

/// A key chord of the launcher of applications.
#[derive(Clone, Copy)]
enum Chord {
    /// Ctrl+Tab and Ctrl+Shift+Tab: the next or previous tab.
    Step(i32),
    /// Alt+1..Alt+9: the tab at this position.
    Open(usize),
    /// Ctrl+G: the list or the grid.
    ToggleView,
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
    /// The hold-Backspace sweep (the launcher of applications only).
    hold: Hold,
    /// The clipboard duty runs, so the calculator row can copy.
    can_copy: bool,
    /// When the result was copied; the launcher finishes [`COPY_CLOSE`]
    /// later. Any close resets it.
    copied_at: Option<Instant>,
    /// The wait after a copy is over; taken once by [`Launcher::take_finished`].
    finished: bool,
    /// How far the list is scrolled.
    scroll: Scroll,
    /// The scroll bar shows until then: the offset last changed 800 ms
    /// before.
    thumb_until: Option<Instant>,
    /// The next layout moves the column at once (the wheel, a new answer)
    /// instead of sliding it.
    jump_next: bool,
    /// The grid's names cut to their cells, by name and width, so a rebuild
    /// (each frame of the sweep) measures none twice. Emptied when the theme
    /// changes and when it outgrows [`NAMES_KEPT`].
    names: HashMap<(String, u32), String>,
}

impl Launcher {
    /// The launcher on `surface` of `output`, in `theme` under `reduced`
    /// motion, over `index` with the use `scores` and the names' `locale`,
    /// its field empty and focused, on the Recent tab in `view`.
    #[must_use]
    pub fn new(
        surface: SurfaceId,
        output: OutputId,
        theme: &Theme,
        reduced: bool,
        scores: HashMap<String, u64>,
        locale: Option<Locale>,
        can_copy: bool,
        view: ListView,
        index: &[DesktopEntry],
    ) -> Launcher {
        let mut state = LauncherState::new(scores, locale);
        state.view = view;
        state.update(index);
        let mut launcher = Self::with_state(surface, output, theme, reduced, state);
        launcher.can_copy = can_copy;
        launcher
    }

    /// The selector on `surface` of `output` over the minimised windows
    /// `bubbles`, its field empty and focused, the most recent window
    /// selected.
    #[must_use]
    pub fn selector(
        surface: SurfaceId,
        output: OutputId,
        theme: &Theme,
        reduced: bool,
        bubbles: &Bubbles,
    ) -> Launcher {
        let state = LauncherState::selector(bubbles.clone());
        Self::with_state(surface, output, theme, reduced, state)
    }

    fn with_state(
        surface: SurfaceId,
        output: OutputId,
        theme: &Theme,
        reduced: bool,
        state: LauncherState,
    ) -> Launcher {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(
            view::launcher(
                &mut |s, _, _| s.to_owned(),
                theme,
                &state,
                (0.0, 0.0),
                None,
                &view::ListScroll::default(),
            ),
            Instant::now(),
        );
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
            hold: Hold::Idle,
            can_copy: false,
            copied_at: None,
            finished: false,
            scroll: Scroll::default(),
            thumb_until: None,
            jump_next: false,
            names: HashMap::new(),
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

    /// Whether this overlay is the selector of the minimised windows.
    #[must_use]
    pub fn is_selector(&self) -> bool {
        self.state.is_selector()
    }

    /// The selector's windows changed: the query is answered again and the
    /// selection follows its window. Returns whether a frame is due; the
    /// launcher of applications ignores it.
    pub fn set_bubbles(&mut self, bubbles: &Bubbles) -> bool {
        if self.state.set_bubbles(bubbles) {
            if self.state.rows.is_empty() {
                self.keyboard_moved = false;
            }
            self.stale = true;
        }
        self.stale
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
        view::card_rect(self.size, view::card_size(&self.theme, &self.state.source))
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
        let selected = self.state.selected;
        let tab = self.state.tab;
        self.state.update(index);
        if self.state.tab != tab {
            // The open group emptied: All shows from its top.
            self.keyboard_moved = false;
            self.scroll_to_top();
            self.stale = true;
        }
        self.sync_extent();
        if self.state.selected != selected {
            // The selection fell back to the first row: show it.
            self.reveal_selected(Instant::now(), true);
        }
        self.stale |= before != self.state.rows;
        self.stale
    }

    /// Draws one frame: the card, the selected row's ring once the keyboard
    /// moved it, the blur and input regions, the caret for the input method.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        reduced: bool,
        index: &[DesktopEntry],
    ) {
        let now = cx.now;
        let sweep_due = self.advance(now, index);
        self.tree.set_reduced_motion(reduced);
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.tree.set_theme(&self.theme);
            self.names.clear();
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
        if let Some((row, area)) = self.ring_parts(now) {
            // The whole row's ring, cut where the area ends.
            let (rect, radius) = focus_ring(row, self.theme.radius_md);
            painter.push_clip(area, 0.0);
            painter.stroke_rounded(rect, radius, FOCUS_WIDTH, self.theme.color(Role::FocusRing));
            painter.pop_clip();
        }
        painter.end();
        let (input, blur) = regions(self.size, &self.theme, &self.state.source);
        cx.ctl.set_blur_region(&blur);
        cx.ctl.set_input_region(&input);
        if let Some(caret) = self.caret() {
            cx.ctl.set_text_input_rect(caret);
        }
        if sweep_due || self.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }

    /// The clipboard took nothing for the last [`LauncherAction::Copy`]: the
    /// row stops saying copied, no close is pending, and Enter can try again.
    pub fn copy_refused(&mut self) {
        self.copied_at = None;
        self.state.copied = false;
        self.stale = true;
    }

    /// Whether the wait after a copy is over, once: the app closes the
    /// launcher then, as it does for [`LauncherAction::Close`].
    pub fn take_finished(&mut self) -> bool {
        std::mem::take(&mut self.finished)
    }

    /// Moves the launcher's timed parts to `now`: the wait after a copy
    /// and the hold-Backspace sweep. Returns whether another frame is due
    /// for either.
    fn advance(&mut self, now: Instant, index: &[DesktopEntry]) -> bool {
        let copying = self.advance_copy(now);
        let lingering = self.advance_thumb(now);
        self.advance_sweep(now, index) || copying || lingering
    }

    /// While the scroll bar lingers a frame is due; once [`THUMB_LINGER`]
    /// has passed the view is rebuilt once without it, and the tree fades it
    /// out.
    fn advance_thumb(&mut self, now: Instant) -> bool {
        let Some(until) = self.thumb_until else {
            return false;
        };
        if now >= until {
            self.thumb_until = None;
            self.stale = true;
            return false;
        }
        true
    }

    /// Sets the scroll's extent from the rows and the area: the list's
    /// height in the area's.
    fn sync_extent(&mut self) {
        let row = self.theme.row_height;
        self.scroll.set_extent(
            view::content_height(&self.theme, &self.state),
            view::AREA_ROWS as f32 * row,
        );
    }

    /// The offset moved: the bar shows until [`THUMB_LINGER`] after `now`,
    /// and the view is built again.
    fn scrolled(&mut self, now: Instant) {
        self.thumb_until = Some(now + THUMB_LINGER);
        self.stale = true;
    }

    /// Scrolls the least that brings the selected row fully into view. A
    /// move shows the bar; `jump` moves the column at once instead of
    /// sliding it. Returns whether the offset moved.
    fn reveal_selected(&mut self, now: Instant, jump: bool) -> bool {
        self.sync_extent();
        let (top, height) = view::item_span(&self.theme, &self.state, self.state.selected);
        let want = scrolling::reveal(
            self.scroll.offset(),
            top,
            height,
            view::AREA_ROWS as f32 * self.theme.row_height,
        );
        let moved = self.scroll.wheel(want - self.scroll.offset());
        if moved {
            self.jump_next |= jump;
            self.scrolled(now);
        }
        moved
    }

    /// A new answer: the list starts at the top, with no bar.
    fn scroll_to_top(&mut self) {
        if self.scroll.offset() != 0.0 {
            self.scroll = Scroll::default();
            self.jump_next = true;
        }
        self.thumb_until = None;
        self.sync_extent();
    }

    /// Whether a wheel on the node `key` scrolls the list: the area or one
    /// of its rows.
    fn scrolls_list(&self, key: celestina_ui::Key) -> bool {
        key == view::area_key() || view::row_of(key, self.state.rows.len()).is_some()
    }

    /// While a copy waits a frame is due; once [`COPY_CLOSE`] has passed the
    /// launcher is finished.
    fn advance_copy(&mut self, now: Instant) -> bool {
        let Some(at) = self.copied_at else {
            return false;
        };
        if now.saturating_duration_since(at) >= COPY_CLOSE {
            self.copied_at = None;
            self.finished = true;
            return false;
        }
        true
    }

    /// Moves the hold-Backspace sweep to `now`: the view is rebuilt while it
    /// runs, and at its end the query is cleared. Returns whether another
    /// frame is due for it.
    fn advance_sweep(&mut self, now: Instant, index: &[DesktopEntry]) -> bool {
        if let Hold::Pressed { since } = self.hold {
            if now.saturating_duration_since(since) < HOLD_DELAY {
                return true;
            }
            self.start_sweep(now);
            if !matches!(self.hold, Hold::Sweeping { .. }) {
                return false;
            }
        }
        let Hold::Sweeping { since } = self.hold else {
            return false;
        };
        if now.saturating_duration_since(since) >= SWEEP {
            self.hold = Hold::Idle;
            self.clear_query(index);
            return false;
        }
        self.stale = true;
        true
    }

    /// Starts the sweep at `now` when there is text to sweep and no
    /// composition; otherwise the hold stays as it is.
    fn start_sweep(&mut self, now: Instant) {
        if !self.composing() && !self.state.query.text().is_empty() {
            self.hold = Hold::Sweeping { since: now };
            self.stale = true;
        } else {
            self.hold = Hold::Idle;
        }
    }

    /// The sweep to draw at `now`, while Backspace is held long enough.
    fn sweep(&self, now: Instant) -> Option<view::Sweep> {
        let Hold::Sweeping { since } = self.hold else {
            return None;
        };
        let field = self.tree.field(view::FIELD)?;
        let text = field.run().map_or(0.0, |r| r.size().0);
        // The wash stays inside the field however far the text runs past it.
        let visible = self
            .tree
            .node_by_key(view::field_key())
            .map_or(0.0, |id| self.tree.rect(id).w);
        Some(view::Sweep {
            progress: now.saturating_duration_since(since).as_secs_f32() / SWEEP.as_secs_f32(),
            text_end: (text - field.scroll_x()).clamp(0.0, visible.max(0.0)),
        })
    }

    /// Builds the view when it is stale and lays the tree out (a no-op when
    /// nothing changed), so input hits what the next frame paints. Nothing
    /// is laid out before the first frame tells the surface's size, so the
    /// card never slides from a provisional place.
    fn layout(&mut self, painter: &mut Painter, now: Instant) {
        if !self.sized() {
            return;
        }
        let dpr = self.dpr;
        let text = painter.text();
        let mut names = std::mem::take(&mut self.names);
        if names.len() > NAMES_KEPT {
            names.clear();
        }
        self.apply(now, &mut |name, style, width| {
            names
                .entry((name.to_owned(), width.to_bits()))
                .or_insert_with(|| {
                    view::fit_end(name, width, |t| text.measure(t, style, None, dpr).0)
                })
                .clone()
        });
        self.names = names;
        self.tree.layout(self.size, painter.text(), self.dpr);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "launcher: layout fault");
        }
    }

    /// Builds the view when it is stale; `fit` cuts the grid's names.
    fn apply(&mut self, now: Instant, fit: view::Fit<'_>) {
        if self.stale {
            let sweep = self.sweep(now);
            self.sync_extent();
            let scroll = view::ListScroll::new(
                &self.theme,
                &self.state,
                self.scroll.offset(),
                self.thumb_until.is_some(),
            );
            if std::mem::take(&mut self.jump_next) {
                // Every node that moved is drawn at its new place at once:
                // the column too, so a keyboard slide still running ends.
                // (Marking the column "still" would leave that slide on.)
                self.tree.slide_with(Duration::ZERO, ONE_UI.into(), &[]);
            }
            self.tree.apply(
                view::launcher(fit, &self.theme, &self.state, self.size, sweep, &scroll),
                now,
            );
            self.stale = false;
        }
    }

    /// The first frame told the surface's size.
    fn sized(&self) -> bool {
        self.size.0 > 0.0 && self.size.1 > 0.0
    }

    /// The selected row's rect where it is drawn at `now`, and the area's,
    /// while the keyboard moved the selection and the row is built.
    fn ring_parts(&self, now: Instant) -> Option<(Rect, Rect)> {
        if !self.keyboard_moved || self.state.rows.is_empty() {
            return None;
        }
        let key = match self.state.chosen() {
            Some(Row::Calc { .. }) => view::calc_key(),
            _ => view::row_key(self.state.selected),
        };
        // A row beyond the built ones may still be fading out in the tree,
        // where it was: it is not on screen.
        let built = view::built_items(&self.theme, &self.state, self.scroll.offset());
        if !built.contains(&self.state.selected) {
            return None;
        }
        let row = self.tree.painted_rect(self.tree.node_by_key(key)?, now);
        let area = self
            .tree
            .painted_rect(self.tree.node_by_key(view::area_key())?, now);
        // A row scrolled out of view has no ring.
        Self::cut(row, area).map(|_| (row, area))
    }

    /// `row` cut to `area`, `None` when they do not meet.
    fn cut(row: Rect, area: Rect) -> Option<Rect> {
        let x = row.x.max(area.x);
        let y = row.y.max(area.y);
        let w = (row.x + row.w).min(area.x + area.w) - x;
        let h = (row.y + row.h).min(area.y + area.h) - y;
        (w > 0.0 && h > 0.0).then_some(Rect { x, y, w, h })
    }

    /// The selected row's ring rect: its rect cut to the area's, or `None`
    /// when the row is scrolled out of view (or the keyboard did not move
    /// the selection).
    #[cfg(test)]
    fn ring(&self, now: Instant) -> Option<Rect> {
        let (row, area) = self.ring_parts(now)?;
        Self::cut(row, area)
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
    /// what a changed query is answered from (the selector answers from its
    /// windows). Returns what the app must do, which closes the launcher
    /// unless [`LauncherAction::closes`] says otherwise.
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

    /// [`Launcher::input`] without the painter and the surface. Any action
    /// that closes the launcher ends a copy's wait, so no second close
    /// follows it.
    fn handle(&mut self, ev: &InputEvent, index: &[DesktopEntry], now: Instant) -> Outcome {
        let outcome = self.handle_event(ev, index, now);
        if outcome.action.as_ref().is_some_and(LauncherAction::closes) {
            self.copied_at = None;
        }
        outcome
    }

    fn handle_event(&mut self, ev: &InputEvent, index: &[DesktopEntry], now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let mut outcome = Outcome::default();
        let events = match ev {
            InputEvent::FocusLost => {
                self.hold = Hold::Idle;
                outcome.action = Some(LauncherAction::Close);
                return outcome;
            }
            InputEvent::FocusEnter { .. } | InputEvent::PopupClosed { .. } => return outcome,
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
                pressed: true,
                ..
            }
            | InputEvent::KeyRepeat {
                keysym: keysym @ (XKB_KEY_UP | XKB_KEY_DOWN),
                ..
            } => {
                let delta = if *keysym == XKB_KEY_UP { -1 } else { 1 };
                if view::use_grid(&self.state) {
                    self.state.move_grid(0, delta);
                } else {
                    self.state.move_selection(delta);
                }
                self.keyboard_moved = true;
                self.reveal_selected(now, false);
                self.stale = true;
                outcome.changed = true;
                return outcome;
            }
            // The grid's Left and Right move the selection while the field
            // is empty; with text they are the caret's.
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_LEFT | XKB_KEY_RIGHT),
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        shift: false,
                        ..
                    },
                pressed: true,
                ..
            }
            | InputEvent::KeyRepeat {
                keysym: keysym @ (XKB_KEY_LEFT | XKB_KEY_RIGHT),
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        shift: false,
                        ..
                    },
                ..
            } if view::use_grid(&self.state) && self.field_is_empty() => {
                self.state
                    .move_grid(if *keysym == XKB_KEY_LEFT { -1 } else { 1 }, 0);
                self.keyboard_moved = true;
                self.reveal_selected(now, false);
                self.stale = true;
                outcome.changed = true;
                return outcome;
            }
            // The launcher's chords (the selector keeps today's keys): they
            // act on a press, and their repeats are swallowed.
            InputEvent::Key {
                keysym,
                modifiers,
                pressed: true,
                ..
            }
            | InputEvent::KeyRepeat {
                keysym, modifiers, ..
            } if !self.state.is_selector() && Self::chord_of(*keysym, *modifiers).is_some() => {
                let is_press = matches!(ev, InputEvent::Key { .. });
                let chord = Self::chord_of(*keysym, *modifiers);
                if is_press {
                    match chord {
                        Some(Chord::Step(delta)) => {
                            self.step_tab(delta, index);
                            outcome.changed = true;
                        }
                        Some(Chord::Open(n)) => {
                            if let Some(tab) = self.state.visible_tabs().get(n).copied() {
                                self.show_tab(tab, index);
                                outcome.changed = true;
                            }
                        }
                        Some(Chord::ToggleView) => {
                            outcome.action = Some(self.toggle_view());
                            outcome.changed = true;
                        }
                        None => {}
                    }
                }
                return outcome;
            }
            InputEvent::Key {
                keysym: keysym @ (XKB_KEY_TAB | XKB_KEY_ISO_LEFT_TAB),
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        shift,
                        // Caps Lock does not change what Tab means.
                        ..
                    },
                pressed: true,
                ..
            } if self.state.is_selector() => {
                let back = *shift || *keysym == XKB_KEY_ISO_LEFT_TAB;
                self.state.move_selection(if back { -1 } else { 1 });
                self.keyboard_moved = true;
                self.reveal_selected(now, false);
                self.stale = true;
                outcome.changed = true;
                return outcome;
            }
            InputEvent::Key {
                keysym: XKB_KEY_DELETE,
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        shift: false,
                        // Caps Lock does not change what Delete means.
                        ..
                    },
                pressed: true,
                ..
            } if self.chosen_window().is_some() && self.delete_is_free() => {
                outcome.action = self.chosen_window().map(LauncherAction::RequestClose);
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
            InputEvent::Key {
                keysym: XKB_KEY_BACKSPACE,
                pressed: false,
                ..
            } => {
                // A release ends the hold; a sweep in flight is cancelled and
                // the text stays.
                if matches!(self.hold, Hold::Sweeping { .. }) {
                    self.stale = true;
                    outcome.changed = true;
                }
                self.hold = Hold::Idle;
                return outcome;
            }
            InputEvent::Key {
                keysym: XKB_KEY_BACKSPACE,
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        // Shift and Caps Lock do not change what a held
                        // Backspace means, as in the field.
                        ..
                    },
                pressed: true,
                ..
            } if !self.state.is_selector() => match self.hold {
                Hold::Blocked => self.input.key(&mut self.tree, ev),
                Hold::Idle => {
                    self.hold = if self.composing() {
                        Hold::Blocked
                    } else {
                        Hold::Pressed { since: now }
                    };
                    self.input.key(&mut self.tree, ev)
                }
                Hold::Pressed { .. } => {
                    self.start_sweep(now);
                    outcome.changed = true;
                    return outcome;
                }
                Hold::Sweeping { .. } => return outcome,
            },
            // A repeated Backspace does what a press does while the key is
            // held: it starts the sweep, or is swallowed by one that runs.
            // With no hold it deletes as the field does and starts none.
            InputEvent::KeyRepeat {
                keysym: XKB_KEY_BACKSPACE,
                modifiers:
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        logo: false,
                        ..
                    },
                ..
            } if !self.state.is_selector() => match self.hold {
                Hold::Pressed { .. } => {
                    self.start_sweep(now);
                    outcome.changed = true;
                    return outcome;
                }
                Hold::Sweeping { .. } => return outcome,
                Hold::Blocked | Hold::Idle => self.input.key(&mut self.tree, ev),
            },
            // Another key's release (a letter let go after Backspace went
            // down) leaves the hold alone.
            InputEvent::Key { pressed: false, .. } => self.input.key(&mut self.tree, ev),
            InputEvent::Key { .. }
            | InputEvent::KeyRepeat { .. }
            | InputEvent::Preedit { .. }
            | InputEvent::CommitText(_)
            | InputEvent::Paste(_) => {
                self.hold = Hold::Idle;
                self.input.key(&mut self.tree, ev)
            }
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
                        // The copied result is no longer on the screen.
                        self.copied_at = None;
                        self.keyboard_moved = false;
                        self.scroll_to_top();
                        self.stale = true;
                    }
                }
                UiEvent::Escape => {
                    outcome.action = Some(LauncherAction::Close);
                    return outcome;
                }
                UiEvent::Activate(key) if key == view::field_key() => {
                    if let Some(action) = self.chosen(now) {
                        outcome.action = Some(action);
                        return outcome;
                    }
                }
                UiEvent::Click(key) if key == view::clear_key() => {
                    self.clear_query(index);
                }
                UiEvent::Click(key) if key == view::view_button_key() => {
                    outcome.action = Some(self.toggle_view());
                    outcome.changed = true;
                    return outcome;
                }
                UiEvent::Click(key) => {
                    let tabs = self.state.visible_tabs();
                    if let Some(i) = view::tab_of(key, tabs.len()) {
                        self.show_tab(tabs[i], index);
                        continue;
                    }
                    if let Some(row) = view::row_of(key, self.state.rows.len()) {
                        self.state.selected = row;
                        if let Some(action) = self.chosen(now) {
                            outcome.action = Some(action);
                            return outcome;
                        }
                    }
                }
                UiEvent::Scroll(key, _, vertical) if self.scrolls_list(key) => {
                    // The wheel jumps: no slide, the column is drawn at its
                    // new place at once.
                    self.sync_extent();
                    if self.scroll.wheel(vertical) {
                        self.jump_next = true;
                        self.scrolled(now);
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

    /// What the launcher's chord `keysym` with `modifiers` means, if it is
    /// one: Ctrl+Tab and Ctrl+Shift+Tab step the tabs, Alt+1..Alt+9 open
    /// the n-th, Ctrl+G toggles the view.
    fn chord_of(keysym: u32, modifiers: Modifiers) -> Option<Chord> {
        let Modifiers {
            ctrl,
            alt,
            logo,
            shift,
            ..
        } = modifiers;
        match keysym {
            XKB_KEY_TAB | XKB_KEY_ISO_LEFT_TAB if ctrl && !alt && !logo => {
                let back = shift || keysym == XKB_KEY_ISO_LEFT_TAB;
                Some(Chord::Step(if back { -1 } else { 1 }))
            }
            0x31..=0x39 if alt && !ctrl && !logo => Some(Chord::Open((keysym - 0x31) as usize)),
            XKB_KEY_LOWER_G | XKB_KEY_UPPER_G if ctrl && !alt && !logo => Some(Chord::ToggleView),
            _ => None,
        }
    }

    /// The search field holds no text.
    fn field_is_empty(&self) -> bool {
        self.tree
            .field(view::FIELD)
            .is_none_or(|f| f.text().is_empty())
    }

    /// Opens the tab `delta` places from the open one, wrapping.
    fn step_tab(&mut self, delta: i32, index: &[DesktopEntry]) {
        let tabs = self.state.visible_tabs();
        let next = state::wrapped(self.state.tab_position(), delta, tabs.len());
        if let Some(tab) = tabs.get(next) {
            self.show_tab(*tab, index);
        }
    }

    /// Shows `tab` with an empty query and field, from its top.
    fn show_tab(&mut self, tab: Tab, index: &[DesktopEntry]) {
        self.state.set_tab(tab, index);
        if let Some(field) = self.tree.field_mut(view::FIELD) {
            field.set_text("");
        }
        self.keyboard_moved = false;
        self.copied_at = None;
        self.hold = Hold::Idle;
        self.scroll_to_top();
        self.stale = true;
    }

    /// Flips between the list and the grid, from the first row; the app
    /// is asked to keep the choice.
    fn toggle_view(&mut self) -> LauncherAction {
        self.state.view = match self.state.view {
            ListView::List => ListView::Grid,
            ListView::Grid => ListView::List,
        };
        self.state.selected = 0;
        self.keyboard_moved = false;
        self.scroll_to_top();
        self.stale = true;
        LauncherAction::SetView(self.state.view)
    }

    /// Empties the search field and answers the empty query: the clear
    /// button's click.
    fn clear_query(&mut self, index: &[DesktopEntry]) {
        if let Some(field) = self.tree.field_mut(view::FIELD) {
            field.set_text("");
        }
        self.state.set_query("", index);
        self.keyboard_moved = false;
        self.scroll_to_top();
        self.stale = true;
    }

    /// The input method is composing text in the field.
    fn composing(&self) -> bool {
        self.tree
            .field(view::FIELD)
            .is_some_and(|f| f.preedit().is_some())
    }

    /// Whether Delete does nothing in the search field, so the selector may
    /// take it to ask the selected window to close: the field is empty, or its
    /// caret sits at the end with no selection. Anywhere else Delete edits the
    /// text and belongs to the field.
    fn delete_is_free(&self) -> bool {
        let Some(field) = self.tree.field(view::FIELD) else {
            return true;
        };
        field.selection().is_none() && field.cursor() >= field.text().len()
    }

    /// The selector's selected window, if a window row is selected.
    fn chosen_window(&self) -> Option<u64> {
        match self.state.chosen()? {
            Row::Bubble { window_id, .. } => Some(*window_id),
            Row::App(_) | Row::Switch(_) | Row::Calc { .. } => None,
        }
    }

    /// What choosing the selected row asks for. The calculator row copies:
    /// without the clipboard duty, or once it has copied, it asks nothing;
    /// otherwise the copy starts the wait before the launcher finishes.
    fn chosen(&mut self, now: Instant) -> Option<LauncherAction> {
        match self.state.chosen()? {
            Row::App(app) => Some(LauncherAction::Launch(Box::new(app.entry.clone()))),
            Row::Switch(switch) => Some(LauncherAction::Switch(*switch)),
            Row::Bubble { window_id, .. } => Some(LauncherAction::Restore(*window_id)),
            Row::Calc { result } => {
                if !self.can_copy || self.state.copied {
                    return None;
                }
                let result = result.clone();
                self.copied_at = Some(now);
                self.state.copied = true;
                self.stale = true;
                Some(LauncherAction::Copy(result))
            }
        }
    }
}

/// The input and blur regions of a launcher surface of `size` in `theme`:
/// the input region is the whole surface, so a press outside the card
/// reaches the launcher and not the window under it; the blur region is the
/// card's silhouette, so the compositor blurs nothing outside it.
#[must_use]
pub fn regions(size: (f32, f32), theme: &Theme, source: &Source) -> (Vec<Rect>, Vec<Rect>) {
    let card = view::card_rect(size, view::card_size(theme, source));
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
        launcher_copying(index, true)
    }

    fn launcher_copying(index: &[DesktopEntry], can_copy: bool) -> Launcher {
        let mut l = Launcher::new(
            SurfaceId(5),
            OutputId(1),
            &Theme::dark(),
            false,
            HashMap::new(),
            None,
            can_copy,
            ListView::List,
            index,
        );
        // The default tab is Recent and these tests list every entry.
        l.state.set_tab(state::Tab::All, index);
        l
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
        l.apply(Instant::now(), &mut |s, _, _| s.to_owned());
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
        let source = Source::Apps;
        let (input, blur) = regions(surface, &theme, &source);
        assert_eq!(input, [Rect::whole(surface)]);
        let card = view::card_rect(surface, view::card_size(&theme, &source));
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
        assert_eq!((card.x, card.y), (1000.0, 498.0));
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

    fn bubbles(ids: &[u64]) -> Bubbles {
        Bubbles {
            windows: ids
                .iter()
                .map(|&id| celestina_shell::melibea::Window {
                    id,
                    app_id: Some(format!("app{id}")),
                    title: Some(format!("Window {id}")),
                    icon_name: None,
                })
                .collect(),
            link: crate::bubbles::Link::Live,
        }
    }

    fn selector(windows: &Bubbles) -> Launcher {
        Launcher::selector(SurfaceId(6), OutputId(1), &Theme::dark(), false, windows)
    }

    fn shift_tab() -> InputEvent {
        InputEvent::Key {
            keysym: XKB_KEY_ISO_LEFT_TAB,
            utf8: None,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            pressed: true,
            serial: 1,
        }
    }

    #[test]
    fn enter_in_the_selector_restores_the_most_recent_window() {
        let mut l = selector(&bubbles(&[5, 3, 8]));
        assert!(l.is_selector());
        let enter = l.handle(&key(RETURN, None, false), &[], Instant::now());
        assert_eq!(enter.action, Some(LauncherAction::Restore(5)));
        assert!(enter.action.as_ref().is_some_and(LauncherAction::closes));
    }

    #[test]
    fn tab_and_shift_tab_move_the_selection_with_wrap_in_the_selector() {
        let mut l = selector(&bubbles(&[1, 2, 3]));
        let now = Instant::now();
        let tab = l.handle(&key(XKB_KEY_TAB, None, false), &[], now);
        assert!(tab.changed && tab.action.is_none());
        assert_eq!(l.state().selected, 1);
        assert!(l.keyboard_moved, "the ring shows after Tab");
        l.handle(&shift_tab(), &[], now);
        l.handle(&shift_tab(), &[], now);
        assert_eq!(
            l.state().selected,
            2,
            "Shift+Tab from the first is the last"
        );
        // Shift with the plain Tab keysym is Shift+Tab too.
        let mut shifted = key(XKB_KEY_TAB, None, false);
        if let InputEvent::Key { modifiers, .. } = &mut shifted {
            modifiers.shift = true;
        }
        l.handle(&shifted, &[], now);
        assert_eq!(l.state().selected, 1);
        l.handle(&key(XKB_KEY_DOWN, None, false), &[], now);
        l.handle(&key(XKB_KEY_DOWN, None, false), &[], now);
        assert_eq!(l.state().selected, 0, "Down wraps as in the launcher");
        let enter = l.handle(&key(RETURN, None, false), &[], now);
        assert_eq!(enter.action, Some(LauncherAction::Restore(1)));
    }

    #[test]
    fn delete_asks_the_selected_window_to_close_and_keeps_the_selector() {
        let mut l = selector(&bubbles(&[1, 2, 3]));
        let now = Instant::now();
        l.handle(&key(XKB_KEY_DOWN, None, false), &[], now);
        let delete = l.handle(&key(XKB_KEY_DELETE, None, false), &[], now);
        assert_eq!(delete.action, Some(LauncherAction::RequestClose(2)));
        assert!(!LauncherAction::RequestClose(2).closes());
        // The row stays until the projection drops it...
        assert_eq!(l.state().rows.len(), 3);
        assert_eq!(l.state().selected, 1);
        // ...and then the selection takes the row in its place.
        assert!(l.set_bubbles(&bubbles(&[1, 3])));
        assert_eq!(
            l.state().chosen().map(|r| match r {
                Row::Bubble { window_id, .. } => *window_id,
                other => panic!("{other:?}"),
            }),
            Some(3)
        );
        // With a modifier, Delete is the field's.
        let mut ctrl_delete = key(XKB_KEY_DELETE, None, true);
        assert_eq!(l.handle(&ctrl_delete, &[], now).action, None);
        if let InputEvent::Key { modifiers, .. } = &mut ctrl_delete {
            modifiers.ctrl = false;
            modifiers.shift = true;
        }
        assert_eq!(l.handle(&ctrl_delete, &[], now).action, None);
    }

    #[test]
    fn delete_edits_the_search_text_when_the_caret_is_not_at_its_end() {
        let mut l = selector(&bubbles(&[1, 2, 3]));
        let now = Instant::now();
        typed(&mut l, "Window", &[]);
        // Caret at the end: Delete does nothing in the field, so it is the
        // selector's.
        assert_eq!(
            l.handle(&key(XKB_KEY_DELETE, None, false), &[], now).action,
            Some(LauncherAction::RequestClose(1))
        );
        // Caret one step back: Delete removes the next character instead.
        l.handle(&key(XKB_KEY_LEFT, None, false), &[], now);
        let delete = l.handle(&key(XKB_KEY_DELETE, None, false), &[], now);
        assert_eq!(delete.action, None, "no close request while editing");
        assert!(delete.changed);
        let text = l.tree.field(view::FIELD).map(|f| f.text().to_owned());
        assert_eq!(text.as_deref(), Some("Windo"));
    }

    fn repeat(keysym: u32, text: Option<&str>) -> InputEvent {
        InputEvent::KeyRepeat {
            keysym,
            utf8: text.map(str::to_owned),
            modifiers: Modifiers::default(),
            serial: 1,
        }
    }

    #[test]
    fn a_repeated_down_moves_the_selection() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("ac", "Ac")];
        let mut l = launcher(&index);
        let now = Instant::now();
        let o = l.handle(&repeat(XKB_KEY_DOWN, None), &index, now);
        assert!(o.changed && o.action.is_none());
        assert_eq!(l.state().selected, 1);
        l.handle(&repeat(XKB_KEY_DOWN, None), &index, now);
        assert_eq!(l.state().selected, 2);
        l.handle(&repeat(XKB_KEY_UP, None), &index, now);
        assert_eq!(l.state().selected, 1);
    }

    #[test]
    fn a_repeated_letter_types() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        let now = Instant::now();
        l.handle(&repeat(0x61, Some("a")), &index, now);
        l.handle(&repeat(0x61, Some("a")), &index, now);
        assert_eq!(text(&l), "aa");
    }

    #[test]
    fn a_repeated_delete_in_the_selector_closes_no_window() {
        let mut l = selector(&bubbles(&[1, 2, 3]));
        let now = Instant::now();
        assert_eq!(
            l.handle(&repeat(XKB_KEY_DELETE, None), &[], now).action,
            None
        );
        // A repeated Tab does not move the selection either.
        l.handle(&repeat(XKB_KEY_TAB, None), &[], now);
        assert_eq!(l.state().selected, 0);
        // The press still asks for the close.
        assert_eq!(
            l.handle(&key(XKB_KEY_DELETE, None, false), &[], now).action,
            Some(LauncherAction::RequestClose(1))
        );
    }

    #[test]
    fn a_repeated_backspace_starts_the_sweep_then_is_swallowed() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        assert_eq!(text(&l), "stea");
        assert!(matches!(l.hold, Hold::Pressed { .. }));
        let o = l.handle(&repeat(XKB_KEY_BACKSPACE, None), &index, t0 + MS(10));
        assert!(o.changed && o.action.is_none());
        assert!(sweeping(&l));
        assert_eq!(text(&l), "stea", "the repeat deletes nothing");
        l.handle(&repeat(XKB_KEY_BACKSPACE, None), &index, t0 + MS(40));
        assert!(sweeping(&l));
        assert_eq!(text(&l), "stea", "later repeats are swallowed");
    }

    #[test]
    fn a_repeated_backspace_with_no_hold_deletes_as_the_field_does() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&repeat(XKB_KEY_BACKSPACE, None), &index, t0);
        assert_eq!(text(&l), "stea");
        assert!(matches!(l.hold, Hold::Idle), "a repeat starts no hold");
    }

    #[test]
    fn without_windows_the_selector_chooses_nothing_and_still_closes() {
        for windows in [bubbles(&[]), Bubbles::default()] {
            let mut l = selector(&windows);
            let now = Instant::now();
            assert!(l.state().rows.is_empty());
            for ev in [
                key(RETURN, None, false),
                key(XKB_KEY_DELETE, None, false),
                key(XKB_KEY_TAB, None, false),
            ] {
                assert_eq!(l.handle(&ev, &[], now).action, None, "{ev:?}");
            }
            assert_eq!(l.state().selected, 0);
            assert_eq!(
                l.handle(&key(ESCAPE, None, false), &[], now).action,
                Some(LauncherAction::Close)
            );
            assert_eq!(
                l.handle(&InputEvent::FocusLost, &[], now).action,
                Some(LauncherAction::Close)
            );
        }
    }

    #[test]
    fn typing_in_the_selector_filters_by_title_and_a_long_paste_is_clamped() {
        let mut l = selector(&bubbles(&[1, 2, 12]));
        typed(&mut l, "window 1", &[]);
        let ids: Vec<u64> = l
            .state()
            .rows
            .iter()
            .map(|r| match r {
                Row::Bubble { window_id, .. } => *window_id,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(ids, [1, 12]);
        let enter = l.handle(&key(RETURN, None, false), &[], Instant::now());
        assert_eq!(enter.action, Some(LauncherAction::Restore(1)));

        let mut l = selector(&bubbles(&[1]));
        let long = "x".repeat(10_000);
        l.handle(&InputEvent::Paste(long), &[], Instant::now());
        assert_eq!(
            l.state().query.text().chars().count(),
            celestina_ui::MAX_CHARS
        );
        assert!(l.state().rows.is_empty());
    }

    #[test]
    fn a_press_outside_the_selectors_card_closes_without_acting() {
        let windows = bubbles(&[1, 2]);
        let mut l = selector(&windows);
        sized(&mut l, (2560.0, 1440.0));
        assert_eq!(
            l.handle(&press(10.0, 10.0), &[], Instant::now()).action,
            Some(LauncherAction::Close)
        );
        // A click on a row restores its window.
        let mut l = selector(&windows);
        sized(&mut l, (2560.0, 1440.0));
        let Some(row) = l
            .tree
            .node_by_key(view::row_key(1))
            .map(|id| l.tree.rect(id))
        else {
            panic!("the second row is laid out");
        };
        let (x, y) = (row.x + row.w / 2.0, row.y + row.h / 2.0);
        let now = Instant::now();
        assert_eq!(l.handle(&press(x, y), &[], now).action, None);
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 3,
        };
        assert_eq!(
            l.handle(&release, &[], now).action,
            Some(LauncherAction::Restore(2))
        );
    }

    #[test]
    fn the_launcher_of_applications_keeps_tab_and_delete_for_the_field() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut l = launcher(&index);
        assert!(!l.is_selector());
        assert!(!l.set_bubbles(&bubbles(&[1])), "it takes no windows");
        let now = Instant::now();
        l.handle(&key(XKB_KEY_TAB, None, false), &index, now);
        assert_eq!(l.state().selected, 0, "Tab does not move the rows here");
        assert_eq!(
            l.handle(&key(XKB_KEY_DELETE, None, false), &index, now)
                .action,
            None
        );
    }

    #[test]
    fn a_new_index_answers_the_open_query_again() {
        let mut l = launcher(&[]);
        assert!(l.state().rows.is_empty());
        let index = [entry("aa", "Aa")];
        assert!(l.set_index(&index));
        assert_eq!(
            l.state().rows,
            [Row::App(Box::new(AppRow {
                entry: index[0].clone(),
                name: "Aa".to_owned(),
                detail: String::new(),
            }))]
        );
    }

    /// Presses and releases the pointer on the node `key` names.
    fn click_on(l: &mut Launcher, key: celestina_ui::Key, index: &[DesktopEntry]) -> Outcome {
        let rect = l
            .tree
            .node_by_key(key)
            .map(|id| l.tree.rect(id))
            .expect("the node is laid out");
        let (x, y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let now = Instant::now();
        l.handle(&press(x, y), index, now);
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 3,
        };
        l.handle(&release, index, now)
    }

    #[test]
    fn the_clear_button_empties_the_query_and_keeps_the_launcher_and_the_focus() {
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut l = launcher(&index);
        sized(&mut l, (2560.0, 1440.0));
        typed(&mut l, "ab", &index);
        l.apply(Instant::now(), &mut |s, _, _| s.to_owned());
        l.tree
            .layout_measured((2560.0, 1440.0), |s, _, _| (7.0 * s.len() as f32, 16.0));
        assert_eq!(l.state().query.text(), "ab");

        let o = click_on(&mut l, view::clear_key(), &index);
        assert_eq!(o.action, None, "the launcher stays open");
        assert!(o.changed);
        assert_eq!(l.state().query.text(), "");
        assert_eq!(
            l.tree.field(view::FIELD).map(|f| f.text().to_owned()),
            Some(String::new())
        );
        assert_eq!(l.state().rows.len(), 2, "an empty query lists the index");
        assert!(l.stale);
        // The focus stays in the field: the next key still types.
        typed(&mut l, "a", &index);
        assert_eq!(l.state().query.text(), "a");
    }

    fn backspace(pressed: bool, shift: bool, ctrl: bool) -> InputEvent {
        InputEvent::Key {
            keysym: XKB_KEY_BACKSPACE,
            utf8: None,
            modifiers: Modifiers {
                shift,
                ctrl,
                ..Modifiers::default()
            },
            pressed,
            serial: 1,
        }
    }

    fn text(l: &Launcher) -> String {
        l.tree
            .field(view::FIELD)
            .map(|f| f.text().to_owned())
            .unwrap_or_default()
    }

    fn sweeping(l: &Launcher) -> bool {
        matches!(l.hold, Hold::Sweeping { .. })
    }

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn holding_backspace_sweeps_then_clears() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        assert_eq!(text(&l), "stea", "the first press deletes one letter");
        assert!(!sweeping(&l));
        // The first repeat starts the sweep and deletes nothing.
        let o = l.handle(&backspace(true, false, false), &index, t0 + MS(10));
        assert!(o.changed && o.action.is_none());
        assert_eq!(text(&l), "stea");
        assert!(sweeping(&l));
        l.handle(&backspace(true, false, false), &index, t0 + MS(40));
        assert_eq!(text(&l), "stea", "later repeats are swallowed");
        // Half way: the text is still there and a frame is due.
        assert!(l.advance(t0 + MS(10) + SWEEP / 2, &index));
        assert_eq!(text(&l), "stea");
        // At the end the query is empty, in the field and in the state.
        assert!(!l.advance(t0 + MS(10) + SWEEP, &index));
        assert_eq!(text(&l), "");
        assert_eq!(l.state().query.text(), "");
        assert!(matches!(l.hold, Hold::Idle));
        assert!(l.stale);
    }

    #[test]
    fn releasing_before_the_end_keeps_the_text_and_a_new_query_answers() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        l.handle(&backspace(true, false, false), &index, t0);
        assert!(sweeping(&l));
        let o = l.handle(&backspace(false, false, false), &index, t0 + MS(200));
        assert!(o.changed, "the sweep leaves the view");
        assert!(matches!(l.hold, Hold::Idle));
        assert!(!l.advance(t0 + SWEEP, &index));
        assert_eq!(text(&l), "stea");
        // Review focus 5: the next typing answers as usual.
        typed(&mut l, "m", &index);
        assert_eq!(l.state().query.text(), "steam");
        l.handle(&backspace(true, false, false), &index, t0 + SWEEP);
        assert_eq!(text(&l), "stea", "a fresh press is one grapheme again");
        assert!(!sweeping(&l));
    }

    #[test]
    fn repeats_never_delete_letters() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        for i in 0..6 {
            l.handle(&backspace(true, false, false), &index, t0 + MS(i * 30));
        }
        assert_eq!(text(&l), "stea", "one grapheme, not six");
    }

    #[test]
    fn a_repeat_on_an_empty_query_starts_no_sweep() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "a", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        assert_eq!(text(&l), "");
        l.handle(&backspace(true, false, false), &index, t0 + MS(30));
        assert!(!sweeping(&l));
        assert!(matches!(l.hold, Hold::Idle));
        assert!(!l.advance(t0 + HOLD_DELAY * 2, &index));
    }

    #[test]
    fn another_keys_release_keeps_the_hold() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        let release_m = InputEvent::Key {
            keysym: 0x6d,
            utf8: Some("m".to_owned()),
            modifiers: Modifiers::default(),
            pressed: false,
            serial: 2,
        };
        l.handle(&release_m, &index, t0 + MS(20));
        assert!(matches!(l.hold, Hold::Pressed { .. }));
        assert!(l.advance(t0 + HOLD_DELAY, &index));
        assert!(sweeping(&l));
    }

    #[test]
    fn a_hold_with_no_repeats_sweeps_after_the_delay() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        assert_eq!(text(&l), "stea");
        assert!(
            l.advance(t0 + HOLD_DELAY - MS(1), &index),
            "a frame keeps timing"
        );
        assert!(!sweeping(&l));
        assert!(l.advance(t0 + HOLD_DELAY, &index));
        assert!(sweeping(&l));
        assert!(!l.advance(t0 + HOLD_DELAY + SWEEP, &index));
        assert_eq!(text(&l), "");
        assert_eq!(l.state().query.text(), "");
    }

    #[test]
    fn a_ctrl_backspace_is_not_a_hold() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "one two three", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, true), &index, t0);
        assert_eq!(text(&l), "one two ");
        l.handle(&backspace(true, false, true), &index, t0 + MS(30));
        assert_eq!(text(&l), "one ", "a repeat deletes the next word");
        assert!(matches!(l.hold, Hold::Idle));
    }

    #[test]
    fn a_preedit_blocks_the_hold() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "abcdef", &index);
        l.handle(
            &InputEvent::Preedit {
                text: "ni".to_owned(),
                cursor: 2,
            },
            &index,
            Instant::now(),
        );
        assert!(l
            .tree
            .field(view::FIELD)
            .is_some_and(|f| f.preedit().is_some()));
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        l.handle(&backspace(true, false, false), &index, t0 + MS(30));
        // The first press ended the composition (a fresh press afterwards),
        // so the repeat started nothing.
        assert!(!sweeping(&l));
        assert!(!l.advance(t0 + SWEEP * 2, &index));
        let kept = l.state().query.text().to_owned();
        assert!(kept.len() >= 4, "two presses deleted at most two: {kept:?}");

        // A composition that starts during a hold ends it.
        l.handle(&backspace(false, false, false), &index, t0);
        l.handle(&backspace(true, false, false), &index, t0);
        assert!(matches!(l.hold, Hold::Pressed { .. }));
        l.handle(
            &InputEvent::Preedit {
                text: "ni".to_owned(),
                cursor: 2,
            },
            &index,
            t0,
        );
        l.handle(&backspace(true, false, false), &index, t0 + MS(30));
        assert!(!sweeping(&l));
        assert!(!l.advance(t0 + SWEEP * 2, &index));
        assert!(!l.state().query.text().is_empty(), "no sweep emptied it");
    }

    #[test]
    fn shift_backspace_deletes_one_and_a_single_press_never_sweeps() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, true, false), &index, t0);
        assert_eq!(text(&l), "stea");
        assert!(!sweeping(&l));
        l.handle(&backspace(false, true, false), &index, t0 + MS(100));
        assert!(!l.advance(t0 + HOLD_DELAY * 2, &index));
        assert_eq!(text(&l), "stea");
    }

    #[test]
    fn focus_loss_during_the_sweep_leaves_no_hold_behind() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        l.handle(&backspace(true, false, false), &index, t0);
        assert!(sweeping(&l));
        let o = l.handle(&InputEvent::FocusLost, &index, t0 + MS(100));
        assert_eq!(o.action, Some(LauncherAction::Close));
        assert!(matches!(l.hold, Hold::Idle));
        assert!(!l.advance(t0 + SWEEP * 2, &index));
        assert_eq!(text(&l), "stea");
        // The next open starts idle.
        assert!(matches!(launcher(&index).hold, Hold::Idle));
    }

    #[test]
    fn any_other_key_ends_the_hold() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "steam", &index);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &index, t0);
        l.handle(&key(u32::from('x'), Some("x"), false), &index, t0);
        assert!(matches!(l.hold, Hold::Idle));
        l.handle(&backspace(true, false, false), &index, t0);
        l.handle(&backspace(true, false, false), &index, t0);
        assert!(sweeping(&l));
        l.handle(&key(XKB_KEY_LEFT, None, false), &index, t0);
        assert!(matches!(l.hold, Hold::Idle));
    }

    #[test]
    fn the_selector_has_no_sweep() {
        let mut l = selector(&bubbles(&[1, 2]));
        typed(&mut l, "ab", &[]);
        let t0 = Instant::now();
        l.handle(&backspace(true, false, false), &[], t0);
        l.handle(&backspace(true, false, false), &[], t0 + MS(30));
        assert_eq!(text(&l), "", "the repeat deletes as the field does");
        assert!(matches!(l.hold, Hold::Idle));
    }

    #[test]
    fn enter_on_the_calculator_row_copies_and_the_launcher_closes_350_ms_later() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2*(3+4)^2", &index);
        let t0 = Instant::now();
        let enter = l.handle(&key(RETURN, None, false), &index, t0);
        assert_eq!(enter.action, Some(LauncherAction::Copy("98".to_owned())));
        assert!(
            !enter.action.as_ref().is_some_and(LauncherAction::closes),
            "a copy does not close at once"
        );
        assert!(l.state().copied && l.stale, "the row says it copied");
        assert!(
            l.advance(t0 + Duration::from_millis(100), &index),
            "redraws"
        );
        assert!(!l.take_finished());
        l.advance(t0 + Duration::from_millis(349), &index);
        assert!(!l.take_finished(), "not yet at +349 ms");
        l.advance(t0 + Duration::from_millis(350), &index);
        assert!(l.take_finished(), "done at +350 ms");
        assert!(!l.take_finished(), "and only once");
        assert!(!l.advance(t0 + Duration::from_millis(400), &index));
        assert!(!l.take_finished());
    }

    #[test]
    fn a_refused_copy_neither_says_copied_nor_closes() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        let t0 = Instant::now();
        let enter = l.handle(&key(RETURN, None, false), &index, t0);
        assert_eq!(enter.action, Some(LauncherAction::Copy("4".to_owned())));
        l.copy_refused();
        assert!(!l.state().copied, "the row does not say copied");
        assert!(!l.advance(t0 + COPY_CLOSE, &index));
        assert!(!l.take_finished(), "the launcher stays open");
        let again = l.handle(&key(RETURN, None, false), &index, t0 + COPY_CLOSE);
        assert_eq!(
            again.action,
            Some(LauncherAction::Copy("4".to_owned())),
            "Enter can try again"
        );
    }

    #[test]
    fn a_second_enter_does_not_copy_again() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        let t0 = Instant::now();
        let enter = l.handle(&key(RETURN, None, false), &index, t0);
        assert_eq!(enter.action, Some(LauncherAction::Copy("4".to_owned())));
        let again = l.handle(&key(RETURN, None, false), &index, t0);
        assert_eq!(again.action, None, "the copy stands once");
    }

    #[test]
    fn without_the_clipboard_duty_enter_on_the_calculator_row_does_nothing() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher_copying(&index, false);
        typed(&mut l, "2+2", &index);
        l.stale = false;
        let t0 = Instant::now();
        let enter = l.handle(&key(RETURN, None, false), &index, t0);
        assert_eq!(enter.action, None);
        assert!(!l.state().copied && !l.stale, "nothing changes");
        assert!(!l.advance(t0 + Duration::from_secs(1), &index));
        assert!(!l.take_finished(), "and the launcher stays open");
    }

    #[test]
    fn escape_inside_the_wait_closes_once_and_the_copy_does_not_close_again() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        let t0 = Instant::now();
        l.handle(&key(RETURN, None, false), &index, t0);
        let esc = l.handle(&key(ESCAPE, None, false), &index, t0);
        assert_eq!(esc.action, Some(LauncherAction::Close));
        l.advance(t0 + Duration::from_millis(500), &index);
        assert!(!l.take_finished(), "the close reset the wait");
    }

    #[test]
    fn losing_the_focus_inside_the_wait_resets_it_too() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        let t0 = Instant::now();
        l.handle(&key(RETURN, None, false), &index, t0);
        let lost = l.handle(&InputEvent::FocusLost, &index, t0);
        assert_eq!(lost.action, Some(LauncherAction::Close));
        l.advance(t0 + Duration::from_millis(500), &index);
        assert!(!l.take_finished());
    }

    #[test]
    fn typing_after_the_copy_cancels_the_pending_close() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        let t0 = Instant::now();
        l.handle(&key(RETURN, None, false), &index, t0);
        typed(&mut l, "3", &index);
        assert!(!l.state().copied);
        l.advance(t0 + Duration::from_millis(500), &index);
        assert!(!l.take_finished());
    }

    #[test]
    fn the_calculator_row_is_chosen_not_launched_and_the_selector_never_has_it() {
        let index = [entry("aa", "Aa")];
        let mut l = launcher(&index);
        typed(&mut l, "2+2", &index);
        assert_eq!(l.state().selected, 0);
        assert!(matches!(l.state().rows[0], Row::Calc { .. }));
        assert_eq!(l.chosen_window(), None);
    }

    // The scrolled list (CEL-I-017).

    fn tools(n: usize) -> Vec<DesktopEntry> {
        (0..n)
            .map(|i| entry(&format!("t{i:03}"), &format!("Tool {i:03}")))
            .collect()
    }

    /// A sized launcher over `n` applications.
    fn listing(index: &[DesktopEntry]) -> Launcher {
        let mut l = launcher(index);
        sized(&mut l, (2560.0, 1440.0));
        l
    }

    /// What a frame does at `now`: builds the view when stale and lays out.
    fn relayout(l: &mut Launcher, now: Instant) {
        l.apply(now, &mut |s, _, _| s.to_owned());
        l.tree
            .layout_measured(l.size, |s, _, _| (7.0 * s.len() as f32, 16.0));
    }

    fn rect_of(l: &Launcher, key: celestina_ui::Key) -> Rect {
        l.tree
            .node_by_key(key)
            .map(|id| l.tree.rect(id))
            .expect("the node is laid out")
    }

    fn wheel_at(x: f32, y: f32, vertical: f32) -> InputEvent {
        InputEvent::Axis {
            x,
            y,
            horizontal: 0.0,
            vertical,
        }
    }

    /// Wheels `vertical` px over the row `i`.
    fn wheel_over_row(
        l: &mut Launcher,
        i: usize,
        vertical: f32,
        index: &[DesktopEntry],
        now: Instant,
    ) {
        let r = rect_of(l, view::row_key(i));
        l.handle(
            &wheel_at(r.x + r.w / 2.0, r.y + r.h / 2.0, vertical),
            index,
            now,
        );
        relayout(l, now);
    }

    fn down(l: &mut Launcher, times: usize, index: &[DesktopEntry], now: Instant) {
        for _ in 0..times {
            l.handle(&key(XKB_KEY_DOWN, None, false), index, now);
        }
        relayout(l, now);
    }

    #[test]
    fn a_wheel_over_a_row_scrolls_the_list_at_once_and_shows_the_bar() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        assert!(l.thumb_until.is_none());
        wheel_over_row(&mut l, 2, 120.0, &index, t0);
        assert_eq!(l.scroll.offset(), 120.0);
        assert_eq!(l.thumb_until, Some(t0 + scrolling::THUMB_LINGER));
        let area = rect_of(&l, view::area_key());
        assert_eq!(rect_of(&l, view::inner_key()).y, area.y - 120.0);
        // The wheel jumps: the column is drawn where it is laid out.
        let inner = l.tree.node_by_key(view::inner_key()).expect("inner");
        assert_eq!(l.tree.painted_rect(inner, t0), l.tree.rect(inner));
        assert!(l.tree.node_by_key(view::thumb_key()).is_some());
        // Row 3 (120 / 40) is the first row in view.
        assert_eq!(rect_of(&l, view::row_key(3)).y, area.y);
        // On the area itself, past the last row's end it also scrolls.
        let r = rect_of(&l, view::row_key(4));
        l.handle(&wheel_at(r.x + 4.0, r.y + 4.0, -50.0), &index, t0);
        assert_eq!(l.scroll.offset(), 70.0);
    }

    #[test]
    fn a_wheel_over_the_field_or_the_tab_row_does_not_scroll() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        let field = rect_of(&l, view::field_key());
        l.handle(&wheel_at(field.x + 20.0, field.y + 5.0, 120.0), &index, t0);
        assert_eq!(l.scroll.offset(), 0.0);
        assert!(l.thumb_until.is_none());
        // On the card's padding and outside the card: no list key either.
        let card = l.card();
        l.handle(&wheel_at(card.x + 2.0, card.y + 2.0, 120.0), &index, t0);
        l.handle(&wheel_at(card.x - 20.0, card.y + 200.0, 120.0), &index, t0);
        assert_eq!(l.scroll.offset(), 0.0);
    }

    #[test]
    fn a_list_that_fits_has_nothing_to_scroll() {
        let index = tools(5);
        let mut l = listing(&index);
        let t0 = Instant::now();
        wheel_over_row(&mut l, 1, 120.0, &index, t0);
        assert_eq!(l.scroll.offset(), 0.0);
        assert!(l.thumb_until.is_none(), "no bar for a list that fits");
        assert!(l.tree.node_by_key(view::thumb_key()).is_none());
    }

    #[test]
    fn down_from_the_last_visible_row_reveals_the_next_with_a_slide() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        down(&mut l, 7, &index, t0);
        assert_eq!(l.state().selected, 7);
        assert_eq!(l.scroll.offset(), 0.0, "row 7 is in view");
        down(&mut l, 1, &index, t0);
        assert_eq!(l.state().selected, 8);
        assert_eq!(l.scroll.offset(), 40.0);
        assert!(l.thumb_until.is_some());
        let inner = l.tree.node_by_key(view::inner_key()).expect("inner");
        let (laid, drawn) = (l.tree.rect(inner), l.tree.painted_rect(inner, t0));
        assert!(
            drawn.y > laid.y,
            "the column still slides up from where it was"
        );
        let later = t0 + Duration::from_millis(400);
        assert_eq!(l.tree.painted_rect(inner, later), l.tree.rect(inner));
    }

    #[test]
    fn up_from_the_first_row_wraps_to_the_last_at_the_bottom_and_back() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        l.handle(&key(XKB_KEY_UP, None, false), &index, t0);
        relayout(&mut l, t0);
        assert_eq!(l.state().selected, 29);
        assert_eq!(l.scroll.offset(), 30.0 * 40.0 - 8.0 * 40.0);
        l.handle(&key(XKB_KEY_DOWN, None, false), &index, t0);
        relayout(&mut l, t0);
        assert_eq!(l.state().selected, 0);
        assert_eq!(l.scroll.offset(), 0.0);
    }

    #[test]
    fn typing_resets_the_offset_but_a_new_index_keeps_it() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        wheel_over_row(&mut l, 2, 200.0, &index, t0);
        assert_eq!(l.scroll.offset(), 200.0);
        // The same answers again: the offset stays.
        assert!(!l.set_index(&index));
        assert_eq!(l.scroll.offset(), 200.0);
        // A longer index: the selected row (0) still exists, the offset stays.
        let longer = tools(40);
        assert!(l.set_index(&longer));
        relayout(&mut l, t0);
        assert_eq!(l.scroll.offset(), 200.0);
        // A shorter one clamps it to its end (10 rows: 400 - 320).
        let short = tools(10);
        assert!(l.set_index(&short));
        relayout(&mut l, t0);
        assert_eq!(l.scroll.offset(), 80.0);
        // The selection was reset to row 0 by nothing here; it is visible.
        assert_eq!(l.state().selected, 0);

        let mut l = listing(&longer);
        wheel_over_row(&mut l, 2, 200.0, &longer, t0);
        typed(&mut l, "t", &longer);
        relayout(&mut l, t0);
        assert_eq!(l.scroll.offset(), 0.0, "a new answer starts at the top");
        assert!(l.thumb_until.is_none(), "and no bar shows for it");
    }

    #[test]
    fn a_new_index_that_drops_the_selected_row_brings_the_new_selection_into_view() {
        let index = tools(40);
        let mut l = listing(&index);
        let t0 = Instant::now();
        down(&mut l, 30, &index, t0);
        assert_eq!(l.state().selected, 30);
        let short = tools(10);
        assert!(l.set_index(&short));
        relayout(&mut l, t0);
        assert_eq!(l.state().selected, 0);
        assert_eq!(l.scroll.offset(), 0.0);
    }

    #[test]
    fn the_bar_stays_800_ms_after_the_last_move_then_is_rebuilt_once_without_it() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        wheel_over_row(&mut l, 2, 120.0, &index, t0);
        l.stale = false;
        assert!(
            l.advance(t0 + Duration::from_millis(799), &index),
            "a frame is due"
        );
        assert!(!l.stale, "nothing to rebuild while it lingers");
        assert!(l.thumb_until.is_some());
        assert!(!l.advance(t0 + Duration::from_millis(800), &index));
        assert!(l.thumb_until.is_none());
        assert!(l.stale, "rebuilt once with the bar hidden");
        relayout(&mut l, t0 + Duration::from_millis(800));
        assert!(!l.stale);
        assert!(
            !l.advance(t0 + Duration::from_millis(900), &index),
            "and no more"
        );
        assert!(!l.stale);
        // Another move starts the 800 ms again.
        wheel_over_row(&mut l, 5, 40.0, &index, t0 + Duration::from_secs(2));
        assert!(l.thumb_until.is_some());
    }

    #[test]
    fn the_ring_is_none_for_a_row_out_of_view_and_clipped_for_a_half_visible_one() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        down(&mut l, 1, &index, t0);
        l.handle(&key(XKB_KEY_UP, None, false), &index, t0);
        relayout(&mut l, t0);
        assert_eq!(l.state().selected, 0);
        let area = rect_of(&l, view::area_key());
        let full = l.ring(t0).expect("row 0 is in view");
        assert_eq!((full.y, full.h), (area.y, 40.0));
        // 20 px of row 0 are scrolled away: the ring is cut at the area.
        wheel_over_row(&mut l, 2, 20.0, &index, t0);
        let half = l.ring(t0).expect("row 0 is half in view");
        assert_eq!((half.y, half.h), (area.y, 20.0));
        assert!(half.y >= area.y && half.y + half.h <= area.y + area.h);
        // Scrolled fully away: no ring, not outside the card.
        wheel_over_row(&mut l, 5, 400.0, &index, t0);
        assert_eq!(l.ring(t0), None);
        // Row 8 revealed at the bottom, then 20 px back up: its lower half
        // is under the area's end, and the ring is cut there.
        wheel_over_row(&mut l, 12, -1000.0, &index, t0);
        down(&mut l, 8, &index, t0);
        assert_eq!(l.state().selected, 8);
        assert_eq!(l.scroll.offset(), 40.0);
        wheel_over_row(&mut l, 5, -20.0, &index, t0);
        let cut = l.ring(t0).expect("half of row 8 is in view");
        assert_eq!((cut.y, cut.h), (area.y + area.h - 20.0, 20.0));
    }

    #[test]
    fn a_click_on_a_half_hidden_row_launches_that_row() {
        let index = tools(30);
        let mut l = listing(&index);
        let t0 = Instant::now();
        wheel_over_row(&mut l, 2, 20.0, &index, t0);
        let area = rect_of(&l, view::area_key());
        let row0 = rect_of(&l, view::row_key(0));
        assert_eq!(row0.y, area.y - 20.0, "laid out above the area's top");
        let (x, y) = (area.x + 100.0, area.y + 5.0);
        l.handle(&press(x, y), &index, t0);
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 3,
        };
        assert_eq!(
            l.handle(&release, &index, t0).action,
            Some(LauncherAction::Launch(Box::new(index[0].clone()))),
            "the hidden half's row, not the one under"
        );
        // Above the area the same row is not under the pointer.
        let (x, y) = (area.x + 100.0, area.y - 5.0);
        l.handle(&press(x, y), &index, t0);
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: 0x110,
            serial: 3,
        };
        assert_eq!(l.handle(&release, &index, t0).action, None);
    }

    #[test]
    fn holding_down_through_three_hundred_rows_restarts_the_slide_each_time() {
        let index = tools(300);
        let mut l = listing(&index);
        let t0 = Instant::now();
        let step = Duration::from_millis(40);
        for i in 1..300u32 {
            let now = t0 + step * i;
            l.handle(&repeat(XKB_KEY_DOWN, None), &index, now);
            relayout(&mut l, now);
            assert_eq!(l.state().selected, i as usize);
            let inner = l.tree.node_by_key(view::inner_key()).expect("inner");
            let lag = l.tree.painted_rect(inner, now).y - l.tree.rect(inner).y;
            assert!(
                (0.0..=4.0 * 40.0).contains(&lag),
                "the column is drawn {lag} px behind its place at step {i}"
            );
            // Rows that left are fading out and do not count.
            let built: Vec<_> = l
                .tree
                .painted_children(inner, now)
                .into_iter()
                .map(|(key, _, _)| key)
                .collect();
            assert!(
                built.contains(&view::row_key(i as usize)),
                "the selected row is built at step {i}"
            );
            // At most the rows in view and the margin on both sides.
            assert!(
                built.len() <= 8 + 2 * 4 + 1,
                "{} rows at step {i}",
                built.len()
            );
        }
        assert_eq!(l.scroll.offset(), 300.0 * 40.0 - 8.0 * 40.0);
    }

    fn chord(keysym: u32, ctrl: bool, alt: bool, shift: bool) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers {
                ctrl,
                alt,
                shift,
                ..Modifiers::default()
            },
            pressed: true,
            serial: 1,
        }
    }

    fn grouped() -> Vec<DesktopEntry> {
        let with = |id: &str, cats: &str| DesktopEntry {
            categories: vec![cats.to_owned()],
            ..entry(id, id)
        };
        vec![
            with("zed", "Development"),
            with("fox", "Network"),
            with("chess", "Game"),
        ]
    }

    /// A launcher on the Recent tab (the way the app opens it), sized.
    fn opened(index: &[DesktopEntry], view: ListView) -> Launcher {
        let mut l = Launcher::new(
            SurfaceId(5),
            OutputId(1),
            &Theme::dark(),
            false,
            HashMap::new(),
            None,
            true,
            view,
            index,
        );
        sized(&mut l, (2560.0, 1440.0));
        l
    }

    fn open_tab(l: &Launcher) -> usize {
        let tabs = l.state.visible_tabs();
        tabs.iter().position(|t| *t == l.state.tab).expect("open")
    }

    #[test]
    fn the_launcher_opens_on_recent_with_the_view_it_is_given() {
        let index = grouped();
        let l = opened(&index, ListView::Grid);
        assert_eq!(l.state.tab, state::Tab::Recent);
        assert_eq!(l.state.view, ListView::Grid);
    }

    #[test]
    fn ctrl_tab_steps_the_tabs_and_wraps_and_the_shifted_chord_goes_back() {
        let index = grouped();
        let mut l = opened(&index, ListView::List);
        let count = l.state.visible_tabs().len();
        assert_eq!(count, 5);
        let now = Instant::now();
        let o = l.handle(&chord(XKB_KEY_TAB, true, false, false), &index, now);
        assert_eq!(o.action, None);
        assert_eq!(open_tab(&l), 1);
        for _ in 0..count - 1 {
            l.handle(&chord(XKB_KEY_TAB, true, false, false), &index, now);
        }
        assert_eq!(open_tab(&l), 0, "wrapped");
        l.handle(&chord(XKB_KEY_TAB, true, false, true), &index, now);
        assert_eq!(open_tab(&l), count - 1, "Ctrl+Shift+Tab wraps backwards");
        l.handle(
            &chord(XKB_KEY_ISO_LEFT_TAB, true, false, false),
            &index,
            now,
        );
        assert_eq!(open_tab(&l), count - 2);
        // A repeat is swallowed.
        let rep = InputEvent::KeyRepeat {
            keysym: XKB_KEY_TAB,
            utf8: None,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            serial: 1,
        };
        l.handle(&rep, &index, now);
        assert_eq!(open_tab(&l), count - 2);
    }

    #[test]
    fn alt_digits_open_the_nth_tab_and_beyond_the_count_do_nothing() {
        let index = grouped();
        let mut l = opened(&index, ListView::List);
        let now = Instant::now();
        l.handle(&chord(0x33, false, true, false), &index, now);
        assert_eq!(l.state.tab, l.state.visible_tabs()[2]);
        l.handle(&chord(0x39, false, true, false), &index, now);
        assert_eq!(l.state.tab, l.state.visible_tabs()[2]);
        assert_eq!(text(&l), "");
        // Alt with a letter reaches the field as before: it inserts nothing.
        let mut a = chord(u32::from('a'), false, true, false);
        if let InputEvent::Key { utf8, .. } = &mut a {
            *utf8 = Some("a".to_owned());
        }
        let o = l.handle(&a, &index, now);
        assert_eq!(o.action, None);
        assert_eq!(text(&l), "");
        assert_eq!(l.state.tab, l.state.visible_tabs()[2]);
    }

    #[test]
    fn ctrl_g_toggles_the_view_without_closing() {
        let index = grouped();
        let mut l = opened(&index, ListView::List);
        let now = Instant::now();
        let o = l.handle(&chord(0x67, true, false, false), &index, now);
        assert_eq!(o.action, Some(LauncherAction::SetView(ListView::Grid)));
        assert!(!LauncherAction::SetView(ListView::Grid).closes());
        assert_eq!(l.state.view, ListView::Grid);
        assert_eq!(l.state.selected, 0);
        let o = l.handle(&chord(0x47, true, false, true), &index, now);
        assert_eq!(o.action, Some(LauncherAction::SetView(ListView::List)));
        assert_eq!(l.state.view, ListView::List);
    }

    #[test]
    fn a_click_on_the_view_button_toggles_the_view() {
        let index = grouped();
        let mut l = opened(&index, ListView::List);
        let o = click_on(&mut l, view::view_button_key(), &index);
        assert_eq!(o.action, Some(LauncherAction::SetView(ListView::Grid)));
    }

    #[test]
    fn grid_right_moves_the_selection_only_while_the_field_is_empty() {
        let index = tools(14);
        let mut l = opened(&index, ListView::Grid);
        let now = Instant::now();
        l.handle(&key(XKB_KEY_TAB, None, true), &index, now); // Ctrl+Tab: All
        assert_eq!(l.state.tab, state::Tab::All);
        l.handle(&key(XKB_KEY_RIGHT, None, false), &index, now);
        assert_eq!(l.state.selected, 1);
        l.handle(&key(XKB_KEY_DOWN, None, false), &index, now);
        assert_eq!(l.state.selected, 7, "a line of six down");
        let rep = InputEvent::KeyRepeat {
            keysym: XKB_KEY_RIGHT,
            utf8: None,
            modifiers: Modifiers::default(),
            serial: 1,
        };
        l.handle(&rep, &index, now);
        assert_eq!(l.state.selected, 8);
        typed(&mut l, "tool", &index);
        l.handle(&key(XKB_KEY_RIGHT, None, false), &index, now);
        assert_eq!(l.state.selected, 0, "the caret moves, not the selection");
    }

    #[test]
    fn the_list_leaves_left_and_right_to_the_field() {
        let index = tools(14);
        let mut l = listing(&index);
        l.handle(&key(XKB_KEY_RIGHT, None, false), &index, Instant::now());
        assert_eq!(l.state.selected, 0);
    }

    fn calc_grid(n: usize) -> (Vec<DesktopEntry>, Launcher) {
        let index: Vec<_> = (0..n)
            .map(|i| entry(&format!("t{i:03}"), &format!("2+2 Tool {i:03}")))
            .collect();
        let mut l = opened(&index, ListView::Grid);
        l.state.set_tab(state::Tab::All, &index);
        typed(&mut l, "2+2", &index);
        relayout(&mut l, Instant::now());
        (index, l)
    }

    #[test]
    fn grid_down_from_the_calculator_row_reaches_cell_zero_and_the_scroll_counts_80_px_lines() {
        let (index, mut l) = calc_grid(60);
        let now = Instant::now();
        assert!(matches!(l.state.rows[0], Row::Calc { .. }));
        l.handle(&key(XKB_KEY_DOWN, None, false), &index, now);
        assert_eq!(l.state.selected, 1);
        l.handle(&key(XKB_KEY_UP, None, false), &index, now);
        assert_eq!(l.state.selected, 0);
        // Up from the calculator row wraps to the last line: its top is
        // 40 + 9 * 80, the extent 40 + 10 * 80.
        l.handle(&key(XKB_KEY_UP, None, false), &index, now);
        assert!(l.state.selected >= 55);
        assert_eq!(l.scroll.offset(), 40.0 + 10.0 * 80.0 - 320.0);
        l.scroll.wheel(10_000.0);
        assert_eq!(l.scroll.offset(), 520.0);
    }

    #[test]
    fn a_click_on_a_tab_while_typing_empties_the_query_and_the_field() {
        let index = grouped();
        let mut l = opened(&index, ListView::List);
        let now = Instant::now();
        l.handle(&key(XKB_KEY_TAB, None, true), &index, now);
        typed(&mut l, "zed", &index);
        assert!(l.state.searching());
        relayout(&mut l, now);
        // The open tab, All, is clickable while a query is typed.
        click_on(&mut l, view::tab_key(1), &index);
        assert_eq!(text(&l), "");
        assert_eq!(l.state.query.text(), "");
        assert_eq!(l.state.tab, state::Tab::All);
        assert_eq!(l.state.rows.len(), 3);
        relayout(&mut l, now);
        click_on(&mut l, view::tab_key(2), &index);
        assert_eq!(l.state.tab, l.state.visible_tabs()[2]);
    }

    #[test]
    fn a_group_that_empties_falls_back_to_all_with_the_scroll_at_the_top() {
        let mut index: Vec<_> = (0..30)
            .map(|i| DesktopEntry {
                categories: vec!["Game".to_owned()],
                ..entry(&format!("g{i:02}"), &format!("Game {i:02}"))
            })
            .collect();
        index.push(entry("plain", "Plain"));
        let mut l = opened(&index, ListView::List);
        let now = Instant::now();
        let games = state::Tab::Group(groups::Group::Games);
        let n = l.state.visible_tabs().iter().position(|t| *t == games);
        let digit = 0x31 + u32::try_from(n.expect("a Games tab")).expect("small");
        l.handle(&chord(digit, false, true, false), &index, now);
        assert_eq!(l.state.tab, games);
        down(&mut l, 12, &index, now);
        assert!(l.scroll.offset() > 0.0);
        let rest = [entry("plain", "Plain")];
        assert!(l.set_index(&rest));
        assert_eq!(l.state.tab, state::Tab::All);
        assert_eq!(l.state.selected, 0);
        assert_eq!(l.scroll.offset(), 0.0);
        assert!(!l.keyboard_moved);
    }

    #[test]
    fn the_selector_ignores_the_tab_and_view_chords() {
        let windows = bubbles(&[1, 2]);
        let mut l = selector(&windows);
        let now = Instant::now();
        for ev in [
            chord(XKB_KEY_TAB, true, false, false),
            chord(0x32, false, true, false),
            chord(0x67, true, false, false),
        ] {
            let o = l.handle(&ev, &[], now);
            assert_eq!(o.action, None);
        }
        assert_eq!(l.state.view, ListView::List);
        assert_eq!(l.state.tab, state::Tab::Recent);
        assert_eq!(l.state.selected, 0);
    }
}
