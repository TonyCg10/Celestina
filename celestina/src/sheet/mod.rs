//! Capsule menus: a piece of a bar capsule opens a window of its own under
//! the bar (CEL-D-002, CEL-D-009). The piece stays in its slot in the bar,
//! under the grey open pill (CEL-D-014), and the window hangs from it. The
//! pure rules ([`geometry`]) and clocks ([`motion`]) hold no painter and no
//! GL; [`Sheets`] owns what is open on one output: the window's rect, its
//! body, the window being left in a switch (it closes towards its piece on
//! its own clock), the open that waits for another thing to close and the
//! scroll. The Island wires it to the bar (`island::sheets`).
//!
//! Nothing here stores a measure: whatever cuts or measures text takes the
//! caller's `Measure` for the length of the call, because the painter is
//! already borrowed wherever a sheet is drawn or fed.

pub mod body;
pub mod geometry;
pub mod motion;
pub mod subtitle;
pub mod tray_overflow;
pub mod workspaces;

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, TextStyle, Theme};
use celestina_ui::{Key, Scroll, Tree};
use celestina_wayland::InputEvent;

pub use body::{BodyOutcome, PageBody, SheetBody, TrayOverflowBody, WorkspacesBody};
use geometry::Edge;
use motion::SheetMotion;

use crate::pieces::{PageId, SheetKind};

/// The text measure of the caller: `(text, style, max width)` to `(width,
/// height)`.
pub type Measure<'m> = dyn FnMut(&str, &TextStyle, Option<f32>) -> (f32, f32) + 'm;

/// How long after the last frame the capsule was seen sliding its changes
/// are still the bar's own motion (the frame the slide ends on still moves
/// it a little).
const SLIDE_GRACE: Duration = Duration::from_millis(40);

/// Whether the capsule's changes at `now` are still the bar's own slide.
fn slide_follows(until: Option<Instant>, now: Instant) -> bool {
    until.is_some_and(|until| now < until)
}

/// A change of a rect under this is no change.
const EPS: f32 = 0.01;

/// What a window hangs from: its capsule as it is painted now and the edge of
/// it the window lines up with, the pill of the open piece (its rect in the
/// bar: the piece stays in its slot, CEL-D-014) and the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub capsule: Rect,
    pub edge: Edge,
    pub pill: Rect,
    pub surface: (f32, f32),
}

/// A sheet to open.
#[derive(Debug)]
pub struct Request {
    /// The bar zone whose capsule it grows from.
    pub zone: usize,
    /// The piece that asked.
    pub piece: Key,
    pub kind: SheetKind,
    pub body: SheetBody,
}

/// What waits for the open thing to finish closing.
#[derive(Debug)]
pub enum Next {
    Sheet(Request),
    Page(PageId),
}

/// The sheet on screen, from its first frame to the end of its close.
#[derive(Debug)]
struct OpenSheet {
    zone: usize,
    piece: Key,
    kind: SheetKind,
    body: SheetBody,
    motion: SheetMotion,
    /// The last anchor the bar gave (or the one it opened with).
    anchor: Anchor,
    /// The target rect the motion last knew.
    target: Rect,
    /// The body's size at the last look.
    size: (f32, f32),
}

/// The window being left in a switch: its body with the scroll it had, on
/// its own clock, closing towards the pill of its own piece.
#[derive(Debug)]
struct Leaving {
    body: SheetBody,
    scroll: f32,
    /// The piece it was open on.
    piece: Key,
    /// The close's clock.
    motion: SheetMotion,
    /// The window it had (where its body sits while it closes).
    target: Rect,
    /// The pill of its piece, as last seen.
    pill: Rect,
    /// The capsule it hung from, as last seen (its blur layer's room).
    capsule: Rect,
}

fn same(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < EPS
        && (a.y - b.y).abs() < EPS
        && (a.w - b.w).abs() < EPS
        && (a.h - b.h).abs() < EPS
}

/// `ev` with its pointer position moved by (`-dx`, `-dy`): the event in the
/// body's own coordinates.
fn translated(ev: &InputEvent, dx: f32, dy: f32) -> InputEvent {
    match ev {
        InputEvent::PointerMotion { x, y } => InputEvent::PointerMotion {
            x: x - dx,
            y: y - dy,
        },
        InputEvent::PointerPress {
            x,
            y,
            button,
            serial,
        } => InputEvent::PointerPress {
            x: x - dx,
            y: y - dy,
            button: *button,
            serial: *serial,
        },
        InputEvent::PointerRelease {
            x,
            y,
            button,
            serial,
        } => InputEvent::PointerRelease {
            x: x - dx,
            y: y - dy,
            button: *button,
            serial: *serial,
        },
        InputEvent::Axis {
            x,
            y,
            horizontal,
            vertical,
        } => InputEvent::Axis {
            x: x - dx,
            y: y - dy,
            horizontal: *horizontal,
            vertical: *vertical,
        },
        other => other.clone(),
    }
}

/// The owner of what is open on one output's bar: at most one sheet, plus
/// the one thing that waits for it to close.
#[derive(Debug, Default)]
pub struct Sheets {
    open: Option<OpenSheet>,
    /// The windows being left in switches, each closing towards its own
    /// piece's pill on its own clock until it has closed.
    leaving: Vec<Leaving>,
    queued: Option<Next>,
    scroll: Scroll,
    /// The capsule was sliding at the last follow, or the one before: its
    /// changes are the bar's own motion, not a resize to morph to.
    slide_until: Option<Instant>,
    /// The pointer was last over the body.
    hovering: bool,
    /// Counts what changed the look: the Island asks for a frame when it
    /// moves.
    revision: u32,
}

impl Sheets {
    /// A sheet is open and not on its way out.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.as_ref().is_some_and(|o| !o.motion.is_closing())
    }

    /// A sheet is painted: open, or closing and not yet gone.
    #[must_use]
    pub fn is_shown(&self, now: Instant) -> bool {
        self.open.as_ref().is_some_and(|o| !o.motion.closed(now))
    }

    /// The zone of the sheet shown (open or closing).
    #[must_use]
    pub fn zone(&self, now: Instant) -> Option<usize> {
        self.open
            .as_ref()
            .filter(|o| !o.motion.closed(now))
            .map(|o| o.zone)
    }

    /// The zone and piece of the sheet held, whatever its state.
    #[must_use]
    pub fn anchor_ids(&self) -> Option<(usize, Key)> {
        self.open.as_ref().map(|o| (o.zone, o.piece))
    }

    /// The piece of the open sheet.
    #[must_use]
    pub fn piece(&self) -> Option<Key> {
        self.open
            .as_ref()
            .filter(|o| !o.motion.is_closing())
            .map(|o| o.piece)
    }

    /// The piece the bar holds open at `now`: the open sheet's, and a
    /// closing sheet's until the close reaches [`motion::PIECE_RESTORE_AT`].
    #[must_use]
    pub fn held_piece(&self, now: Instant) -> Option<Key> {
        let o = self.open.as_ref()?;
        o.motion
            .close_fraction(now)
            .is_none_or(|at| at < motion::PIECE_RESTORE_AT)
            .then_some(o.piece)
    }

    /// The open sheet's kind, else the queued one's.
    #[must_use]
    pub fn kind(&self) -> Option<SheetKind> {
        match (&self.open, &self.queued) {
            (Some(o), _) if !o.motion.is_closing() => Some(o.kind),
            (_, Some(Next::Sheet(r))) => Some(r.kind),
            _ => None,
        }
    }

    /// A level body is open (not closing) or queued.
    #[must_use]
    pub fn has_level(&self) -> bool {
        let open = self
            .open
            .as_ref()
            .is_some_and(|o| !o.motion.is_closing() && o.body.is_level());
        let queued = matches!(&self.queued, Some(Next::Sheet(r)) if r.body.is_level());
        open || queued
    }

    /// The page of the open sheet (not closing), when it shows one.
    #[must_use]
    pub fn page(&self) -> Option<PageId> {
        self.open
            .as_ref()
            .filter(|o| !o.motion.is_closing())
            .and_then(|o| o.body.page())
    }

    /// The open sheet's page body (not closing), when it is one.
    pub fn page_body_mut(&mut self) -> Option<&mut PageBody> {
        self.open
            .as_mut()
            .filter(|o| !o.motion.is_closing())
            .and_then(|o| o.body.page_body_mut())
    }

    /// The open workspaces body (not closing), when the sheet is one.
    pub fn workspaces_body_mut(&mut self) -> Option<&mut WorkspacesBody> {
        self.open
            .as_mut()
            .filter(|o| !o.motion.is_closing())
            .and_then(|o| o.body.workspaces_body_mut())
    }

    /// The open tray overflow body (not closing), when the sheet is one.
    pub fn tray_overflow_body_mut(&mut self) -> Option<&mut TrayOverflowBody> {
        self.open
            .as_mut()
            .filter(|o| !o.motion.is_closing())
            .and_then(|o| o.body.tray_overflow_body_mut())
    }

    /// The rect of the open body's node `key` in the surface's coordinates
    /// (where the window puts the body, less the scroll), for what hangs
    /// from it.
    #[must_use]
    pub fn body_rect(&self, key: Key, anchor: &Anchor) -> Option<Rect> {
        let o = self.open.as_ref().filter(|o| !o.motion.is_closing())?;
        let tree = o.body.tree();
        let rect = tree.rect(tree.node_by_key(key)?);
        let at = Sheets::target_of(anchor, o.body.size());
        Some(Rect {
            x: at.x + rect.x,
            y: at.y + rect.y - self.scroll.offset(),
            ..rect
        })
    }

    /// The key of the node of the open body that holds the keyboard focus.
    #[must_use]
    pub fn body_focused_key(&self) -> Option<Key> {
        let o = self.open.as_ref().filter(|o| !o.motion.is_closing())?;
        let tree = o.body.tree();
        tree.highlight().focused.map(|id| tree.key_of(id))
    }

    /// Whether the open body is taller than its window (the sheet scrolls).
    #[must_use]
    pub fn body_overflows(&self, anchor: &Anchor) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| o.body.size().1 > Sheets::target_of(anchor, o.body.size()).h + EPS)
    }

    /// Something waits for the open thing to close.
    #[must_use]
    pub fn has_queued(&self) -> bool {
        self.queued.is_some()
    }

    /// What changed the look so far: it moves when something did.
    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The last anchor the window knew: where it closes towards when its piece
    /// vanishes.
    #[must_use]
    pub fn last_anchor(&self) -> Option<Anchor> {
        self.open.as_ref().map(|o| o.anchor)
    }

    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn target_of(anchor: &Anchor, size: (f32, f32)) -> Rect {
        geometry::window(anchor.capsule, anchor.edge, size, anchor.surface)
    }

    /// The window's rect at `now`, or `None` when none is shown. While it
    /// closes the rect goes towards the pill of `anchor`.
    #[must_use]
    pub fn rect(&self, anchor: &Anchor, now: Instant) -> Option<Rect> {
        let o = self.open.as_ref().filter(|o| !o.motion.closed(now))?;
        let to = if o.motion.is_closing() {
            anchor.pill
        } else {
            Sheets::target_of(anchor, o.body.size())
        };
        Some(o.motion.rect(now, to, anchor.surface))
    }

    /// The windows painted at `now`, each with its corner radius: the one
    /// open (or closing, under `anchor`) and the one being left in a switch.
    /// What the glass, the blur and the input regions cover; a closing
    /// window leaves once its glass is below 35 % (the backdrop blur cannot
    /// fade).
    #[must_use]
    pub fn windows(
        &self,
        anchor: Option<&Anchor>,
        surface: (f32, f32),
        now: Instant,
    ) -> Vec<(Rect, f32)> {
        let mut out = Vec::new();
        let leaving = self
            .leaving
            .iter()
            .filter(|l| !l.motion.closed(now) && l.motion.blur_region(now));
        for l in leaving {
            out.push((l.motion.rect(now, l.pill, surface), l.motion.radius(now)));
        }
        if let (Some(o), Some(rect)) = (self.open.as_ref(), anchor.and_then(|a| self.rect(a, now)))
        {
            if o.motion.blur_region(now) {
                out.push((rect, o.motion.radius(now)));
            }
        }
        out.retain(|(r, _)| r.w > 0.0 && r.h > 0.0);
        out
    }

    /// Whether (`x`, `y`) is on a window being left in a switch, which is
    /// still painted for a moment but takes nothing.
    #[must_use]
    pub fn on_leaving(&self, surface: (f32, f32), now: Instant, x: f32, y: f32) -> bool {
        self.leaving.iter().any(|l| {
            !l.motion.closed(now)
                && geometry::hit(l.motion.rect(now, l.pill, surface), x, y) == geometry::Hit::Body
        })
    }

    /// The pill of the piece the window being left closes towards, as the bar
    /// has it now.
    pub fn follow_leaving(&mut self, piece: Key, pill: Rect) {
        for l in self.leaving.iter_mut().filter(|l| l.piece == piece) {
            l.pill = pill;
        }
    }

    /// The pieces whose windows are being left, while they close.
    #[must_use]
    pub fn leaving_pieces(&self, now: Instant) -> Vec<Key> {
        self.leaving
            .iter()
            .filter(|l| !l.motion.closed(now))
            .map(|l| l.piece)
            .collect()
    }

    /// Drops what has finished: a window whose close ended, a window
    /// that has closed.
    fn reap(&mut self, now: Instant) {
        if self.open.as_ref().is_some_and(|o| o.motion.closed(now)) {
            self.open = None;
            self.scroll = Scroll::default();
            self.hovering = false;
            self.bump();
        }
        let folding = self.leaving.len();
        self.leaving.retain(|l| !l.motion.closed(now));
        if self.leaving.len() != folding {
            self.bump();
        }
    }

    /// Sets the scroll's extent: the body's height in the window.
    fn refresh_extent(&mut self) {
        if let Some(o) = &self.open {
            self.scroll.set_extent(o.size.1, o.target.h);
        }
    }

    /// Opens `request` from `anchor`, or, with a sheet already open (not
    /// closing), switches it to the request's body: the old window closes towards
    /// into its own piece's pill while the new one grows out of its piece's.
    /// What the old body held goes out first.
    pub fn open(
        &mut self,
        request: Request,
        anchor: &Anchor,
        theme: &Theme,
        now: Instant,
        reduced: bool,
    ) {
        self.reap(now);
        // A sheet opened directly supersedes a waiting sheet, not a waiting
        // page.
        if matches!(self.queued, Some(Next::Sheet(_))) {
            self.queued = None;
        }
        let Request {
            zone,
            piece,
            kind,
            body,
        } = request;
        let size = body.size();
        let target = Sheets::target_of(anchor, size);
        if let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) {
            // The window being left closes towards its own piece's pill on its
            // own clock; the new one grows out of its piece's.
            let shown = o.motion.rect(now, o.target, anchor.surface);
            let mut folding = std::mem::replace(
                &mut o.motion,
                SheetMotion::open(anchor.pill, theme, now, reduced),
            );
            folding.close(shown, o.anchor.pill, now);
            let mut old = std::mem::replace(&mut o.body, body);
            old.flush(now);
            self.leaving.push(Leaving {
                body: old,
                scroll: self.scroll.offset(),
                piece: o.piece,
                motion: folding,
                target: o.target,
                pill: o.anchor.pill,
                capsule: o.anchor.capsule,
            });
            o.zone = zone;
            o.piece = piece;
            o.kind = kind;
            o.anchor = *anchor;
            o.target = target;
            o.size = size;
            self.scroll = Scroll::default();
        } else {
            if let Some(mut old) = self.open.take() {
                old.body.flush(now);
            }
            self.open = Some(OpenSheet {
                zone,
                piece,
                kind,
                body,
                motion: SheetMotion::open(anchor.pill, theme, now, reduced),
                anchor: *anchor,
                target,
                size,
            });
            self.scroll = Scroll::default();
        }
        self.hovering = false;
        self.refresh_extent();
        self.bump();
    }

    /// Starts the close: the window goes towards the pill of its piece. What
    /// its body holds goes out first. Does nothing without an open window.
    /// The piece gives back its pill and opacity at the restore moment
    /// ([`motion::PIECE_RESTORE_AT`], the bar's clock).
    pub fn close(&mut self, anchor: &Anchor, now: Instant) {
        let Some(shown) = self.rect(anchor, now) else {
            return;
        };
        if !self.is_open() {
            return;
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.body.flush(now);
        o.motion.close(shown, anchor.pill, now);
        self.hovering = false;
        self.bump();
    }

    /// Takes the open sheet away at once, with no exit motion: what its body
    /// holds goes out first. The next [`Sheets::open`] then grows from its
    /// piece's pill.
    pub fn drop_open(&mut self, now: Instant) {
        if let Some(mut old) = self.open.take() {
            old.body.flush(now);
            self.leaving.clear();
            self.scroll = Scroll::default();
            self.hovering = false;
            self.bump();
        }
    }

    /// Makes `next` wait for the open thing to finish closing (it replaces
    /// what waited).
    pub fn queue(&mut self, next: Next) {
        self.queued = Some(next);
        self.bump();
    }

    /// The page of the page sheet that waits, if one does.
    #[must_use]
    pub fn queued_page_sheet(&self) -> Option<PageId> {
        match &self.queued {
            Some(Next::Sheet(Request {
                kind: SheetKind::Page(page),
                ..
            })) => Some(*page),
            _ => None,
        }
    }

    /// Gives up what waited.
    pub fn drop_queue(&mut self) {
        if self.queued.take().is_some() {
            self.bump();
        }
    }

    /// What waited, once the sheet is gone and, for a sheet, the Island's
    /// page too (`island_closed`).
    pub fn take_due(&mut self, now: Instant, island_closed: bool) -> Option<Next> {
        self.reap(now);
        let ready = !self.is_shown(now)
            && match &self.queued {
                Some(Next::Sheet(_)) => island_closed,
                Some(Next::Page(_)) => true,
                None => false,
            };
        if ready {
            self.bump();
            self.queued.take()
        } else {
            None
        }
    }

    /// The bar moved or changed under the sheet: it follows its capsule and
    /// piece. A capsule that jumps (a new placement, a text that changed its
    /// width) is followed on the resize morph; one that slides (`sliding`: a
    /// tray app hidden or shown beside the piece) is followed exactly, frame
    /// by frame, with no morph of its own.
    pub fn follow(&mut self, anchor: &Anchor, sliding: bool, now: Instant) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        // The frame the slide ends on still moves the capsule a little.
        if sliding {
            self.slide_until = Some(now + SLIDE_GRACE);
        }
        if o.motion.is_closing() {
            // On its way out the window closes towards the live pill.
            o.anchor = *anchor;
            return;
        }
        o.anchor = *anchor;
        self.retarget(now);
    }

    /// Recomputes the target from the stored anchor and the body's size, and
    /// morphs to it when it moved.
    fn retarget(&mut self, now: Instant) {
        let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) else {
            return;
        };
        let size = o.body.size();
        let target = Sheets::target_of(&o.anchor, size);
        if !same(target, o.target) {
            if !slide_follows(self.slide_until, now) {
                let shown = o.motion.rect(now, o.target, o.anchor.surface);
                o.motion.resize(shown, now);
            }
            o.target = target;
            self.revision = self.revision.wrapping_add(1);
        }
        o.size = size;
        self.refresh_extent();
    }

    /// Handles `ev` for the open sheet's body (a key, or a pointer event
    /// over the body or during its drag): the wheel scrolls unless it is
    /// over a slider. `measure` is in hand for this call only.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        anchor: &Anchor,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> BodyOutcome {
        if self.rect(anchor, now).is_none() {
            return BodyOutcome::Keep { changed: false };
        }
        let size = self.open.as_ref().map_or((0.0, 0.0), |o| o.body.size());
        let at = Sheets::target_of(anchor, size);
        let (dx, dy) = (at.x, at.y - self.scroll.offset());
        let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) else {
            return BodyOutcome::Keep { changed: false };
        };
        let pointer = !matches!(
            ev,
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } | InputEvent::FocusLost
        );
        if pointer {
            self.hovering = !matches!(ev, InputEvent::PointerLeave);
        }
        if let InputEvent::Axis { x, y, vertical, .. } = *ev {
            if !o.body.takes_wheel(x - dx, y - dy, now) {
                let moved = self.scroll.wheel(vertical);
                if moved {
                    self.bump();
                }
                return BodyOutcome::Keep { changed: moved };
            }
        }
        let mut outcome = o.body.input(&translated(ev, dx, dy), now, measure);
        // The overflow button's rect leaves in the surface's coordinates.
        if let BodyOutcome::More { rect, keyboard } = outcome {
            outcome = BodyOutcome::More {
                rect: Rect {
                    x: rect.x + dx,
                    y: rect.y + dy,
                    ..rect
                },
                keyboard,
            };
        }
        self.retarget(now);
        // A click that activates a node redraws the button released, whether
        // or not the page then changes.
        if matches!(
            outcome,
            BodyOutcome::Keep { changed: true } | BodyOutcome::Activate { .. }
        ) {
            self.bump();
        }
        outcome
    }

    /// The pointer is no longer over the body: its hover ends.
    pub fn leave_body(&mut self, now: Instant, measure: &mut Measure<'_>) {
        if !std::mem::take(&mut self.hovering) {
            return;
        }
        if let Some(o) = self.open.as_mut() {
            o.body.input(&InputEvent::PointerLeave, now, measure);
            self.bump();
        }
    }

    /// Whether the open body takes Escape itself (a confirmation card goes
    /// back to its list) instead of the sheet closing.
    #[must_use]
    pub fn body_takes_escape(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| !o.motion.is_closing() && o.body.takes_escape())
    }

    /// Whether a drag is held on the body.
    #[must_use]
    pub fn body_holds_pointer(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| !o.motion.is_closing() && o.body.held())
    }

    /// Takes in a new theme and the readings, sends what is due, notices a
    /// body that changed size and drops what has finished. Whether the look
    /// changed. `measure` is the frame's own, in hand for this call only.
    pub fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        self.reap(now);
        let mut changed = false;
        if let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) {
            changed = o.body.sync(theme, now, measure);
        }
        self.retarget(now);
        if changed {
            self.bump();
        }
        changed
    }

    /// Lays the bodies out (the open one and the one being left).
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        if let Some(o) = self.open.as_mut() {
            o.body.layout(lay);
        }
        for l in &mut self.leaving {
            l.body.layout(lay);
        }
    }

    /// Paints the windows: the one being left (closing towards its pill)
    /// under the open one. Each is its Haze at its rect and corner radius,
    /// sharp, then its body under one rounded clip of that rect. A window that
    /// closes draws its body through the blur and shrunk with its rect
    /// towards the pill, as the preview does, while its glass keeps a sharp
    /// edge. The body sits at its window's target, so the rect reveals it
    /// rather than carrying it.
    pub fn paint(&mut self, painter: &mut Painter, anchor: &Anchor, now: Instant) {
        let surface = anchor.surface;
        for l in &mut self.leaving {
            if l.motion.closed(now) {
                continue;
            }
            let rect = l.motion.rect(now, l.pill, surface);
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let look = Look {
                rect,
                radius: l.motion.radius(now),
                glass: l.motion.glass_opacity(now),
                alpha: l.motion.body_opacity(now),
                target: l.target,
                at: (l.target.x, l.target.y - l.scroll),
                layer: blur_layer(&l.motion, l.capsule, l.pill, surface, now),
                start_radius: l.motion.start_radius(),
            };
            paint_window(painter, &look, &mut |p| l.body.paint(p, now));
        }
        let Some(rect) = self.rect(anchor, now) else {
            return;
        };
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let offset = self.scroll.offset();
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let target = Sheets::target_of(anchor, o.body.size());
        let look = Look {
            rect,
            radius: o.motion.radius(now),
            glass: o.motion.glass_opacity(now),
            alpha: o.motion.body_opacity(now),
            target,
            at: (target.x, target.y - offset),
            layer: blur_layer(&o.motion, anchor.capsule, anchor.pill, anchor.surface, now),
            start_radius: o.motion.start_radius(),
        };
        paint_window(painter, &look, &mut |p| o.body.paint(p, now));
    }

    /// Whether the capsule's changes at `now` are the bar's slide (tests).
    #[cfg(test)]
    #[must_use]
    pub fn follows_slide(&self, now: Instant) -> bool {
        slide_follows(self.slide_until, now)
    }

    /// The pill the window being left closes towards (tests).
    #[cfg(test)]
    #[must_use]
    pub fn leaving_pill(&self) -> Option<Rect> {
        self.leaving.last().map(|l| l.pill)
    }

    /// The scroll offset (tests).
    #[cfg(test)]
    #[must_use]
    pub fn scroll(&self) -> &Scroll {
        &self.scroll
    }

    /// The open body's node `key`, in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn body_node_rect(&self, key: Key) -> Option<Rect> {
        self.open.as_ref().and_then(|o| o.body.node_rect(key))
    }

    /// The open body's size (tests).
    #[cfg(test)]
    #[must_use]
    pub fn body_size(&self) -> Option<(f32, f32)> {
        self.open.as_ref().map(|o| o.body.size())
    }

    /// Whether a body is being left in a switch (tests).
    #[cfg(test)]
    #[must_use]
    pub fn is_leaving(&self) -> bool {
        !self.leaving.is_empty()
    }

    /// Whether a clock runs and the surface needs a frame. A sheet at rest
    /// asks for none.
    #[must_use]
    pub fn wants_frame(&self, now: Instant) -> bool {
        let open = self.open.as_ref().is_some_and(|o| {
            o.motion.wants_frame(now) || o.motion.closed(now) || o.body.is_animating(now)
        });
        let folding = !self.leaving.is_empty();
        open || folding
    }

    /// How long until the body's throttle or an asked-level hold is due;
    /// `None` at rest.
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.open
            .as_ref()
            .filter(|o| !o.motion.is_closing())
            .and_then(|o| o.body.wait(now))
    }
}

/// The blur's reach past a window's edge, in logical px: three times the
/// close's blur.
const BLUR_REACH: f32 = 36.0;
/// The grid the blur layer's edges are cut to, in logical px, so the layer
/// keeps its pixel size while the pill and the capsule slide a little.
const LAYER_GRID: f32 = 32.0;

/// The offscreen layer a window's body paints through and its blur in
/// logical px (0 for a window that is not closing, or under reduced motion).
/// The layer holds the window as it was shown, the capsule it hung from, the
/// pill it heads to and the blur's reach, on a grid, whole through the close
/// so its images are made once.
fn blur_layer(
    motion: &SheetMotion,
    capsule: Rect,
    pill: Rect,
    surface: (f32, f32),
    now: Instant,
) -> (Rect, f32) {
    let sigma = motion.blur_px(now);
    if sigma <= 0.0 {
        return (pill, 0.0);
    }
    let from = motion.start_rect();
    let low = |a: f32, b: f32, c: f32| a.min(b).min(c) - BLUR_REACH;
    let high = |a: f32, b: f32, c: f32| a.max(b).max(c) + BLUR_REACH;
    let x0 = low(from.x, pill.x, capsule.x);
    let y0 = low(from.y, pill.y, capsule.y);
    let x1 = high(from.x + from.w, pill.x + pill.w, capsule.x + capsule.w);
    let y1 = high(from.y + from.h, pill.y + pill.h, capsule.y + capsule.h);
    let down = |v: f32| (v / LAYER_GRID).floor() * LAYER_GRID;
    let up = |v: f32| (v / LAYER_GRID).ceil() * LAYER_GRID;
    let (x1, y1) = (
        up(x1).min(surface.0).max(0.0),
        up(y1).min(surface.1).max(0.0),
    );
    let (x0, y0) = (down(x0).max(0.0).min(x1), down(y0).max(0.0).min(y1));
    let layer = Rect {
        x: x0,
        y: y0,
        w: (x1 - x0).max(0.0),
        h: (y1 - y0).max(0.0),
    };
    (layer, sigma)
}

/// What one window is painted with at a frame.
struct Look {
    /// The window's rect and corner radius now.
    rect: Rect,
    radius: f32,
    /// The glass's and the body's opacity.
    glass: f32,
    alpha: f32,
    /// The window as the body was laid out for it, the body's origin in the
    /// surface (the scroll taken off) and the radius the window had.
    target: Rect,
    at: (f32, f32),
    start_radius: f32,
    /// The blur layer and the blur (0: none).
    layer: (Rect, f32),
}

/// How much a body shrinks with its window: the scales that take `target`
/// to `rect`.
fn follow(target: Rect, rect: Rect) -> (f32, f32) {
    let sx = if target.w > 0.0 {
        rect.w / target.w
    } else {
        1.0
    };
    let sy = if target.h > 0.0 {
        rect.h / target.h
    } else {
        1.0
    };
    (sx.max(0.0), sy.max(0.0))
}

/// Paints one window: its glass sharp, then its body, which, while the
/// window closes, goes through the blur and takes the window's shrinking
/// (the preview's `toward`).
fn paint_window(painter: &mut Painter, look: &Look, body: &mut dyn FnMut(&mut Painter)) {
    haze_at(painter, look.rect, look.radius, look.glass);
    if look.alpha <= 0.0 {
        return;
    }
    let (layer, sigma) = look.layer;
    if sigma <= 0.0 {
        paint_body(painter, look.rect, look.radius, look.at, look.alpha, body);
        return;
    }
    let (sx, sy) = follow(look.target, look.rect);
    let (target, rect) = (look.target, look.rect);
    painter.push_state();
    painter.opacity(look.alpha);
    painter.blurred(layer, sigma, &mut |p| {
        p.push_state();
        p.translate(rect.x, rect.y);
        p.scale(sx, sy);
        p.translate(-target.x, -target.y);
        paint_body(p, target, look.start_radius, look.at, 1.0, &mut *body);
        p.pop_state();
    });
    painter.pop_state();
}

/// The window's glass at `alpha`.
fn haze_at(painter: &mut Painter, rect: Rect, radius: f32, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    painter.push_state();
    painter.opacity(alpha);
    painter.haze(rect, radius);
    painter.pop_state();
}

/// Paints one body under one rounded clip of `view`, translated to `at`, at
/// `alpha`.
fn paint_body(
    painter: &mut Painter,
    view: Rect,
    radius: f32,
    at: (f32, f32),
    alpha: f32,
    paint: &mut dyn FnMut(&mut Painter),
) {
    painter.push_clip(view, radius);
    painter.push_state();
    painter.translate(at.0, at.1);
    painter.opacity(alpha);
    paint(painter);
    painter.pop_state();
    painter.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_paint::Theme;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn bare_open(piece: Key, now: Instant) -> (Sheets, Anchor) {
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let anchor = Anchor {
            capsule: r(1000.0, 8.0, 200.0, 40.0),
            edge: Edge::Start,
            pill,
            surface: (1920.0, 560.0),
        };
        let body = WorkspacesBody::new(
            &Theme::dark(),
            false,
            celestina_ui::view::row(Key::named("body")).size(100.0, 40.0),
            vec![1],
            now,
        );
        let mut sheets = Sheets::default();
        sheets.open(
            Request {
                zone: 3,
                piece,
                kind: SheetKind::Workspaces,
                body: SheetBody::Workspaces(Box::new(body)),
            },
            &anchor,
            &Theme::dark(),
            now,
            false,
        );
        (sheets, anchor)
    }

    #[test]
    fn the_piece_is_held_until_its_restore_moment() {
        let t0 = Instant::now();
        let piece = Key::named("volume");
        let (mut sheets, anchor) = bare_open(piece, t0);
        assert_eq!(sheets.held_piece(t0), Some(piece));
        assert_eq!(Sheets::default().held_piece(t0), None);
        let at = t0 + Duration::from_millis(500);
        sheets.close(&anchor, at);
        // 260 ms close: held while its fraction is under the restore moment.
        let close = Duration::from_millis(260);
        if motion::PIECE_RESTORE_AT > 0.0 {
            let before = at + close.mul_f32(motion::PIECE_RESTORE_AT * 0.5);
            assert_eq!(sheets.held_piece(before), Some(piece));
        }
        let from = at + close.mul_f32(motion::PIECE_RESTORE_AT);
        assert_eq!(sheets.held_piece(from), None);
        assert_eq!(sheets.held_piece(at + close), None);
    }

    fn closing(reduced: bool, t0: Instant) -> SheetMotion {
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let mut m = SheetMotion::open(pill, &Theme::dark(), t0, reduced);
        m.close(
            r(930.0, 56.0, 240.0, 236.0),
            pill,
            t0 + Duration::from_millis(400),
        );
        m
    }

    #[test]
    fn a_closing_window_paints_through_one_layer_for_its_whole_close() {
        let t0 = Instant::now();
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let capsule = r(905.0, 8.0, 265.0, 40.0);
        let surface = (1920.0, 560.0);
        let m = closing(false, t0);
        let start = t0 + Duration::from_millis(400);
        let at = |n: u64| start + Duration::from_millis(n);
        let (layer, sigma) = blur_layer(&m, capsule, pill, surface, at(100));
        assert!(sigma > 0.0);
        // The window as shown, the capsule, the pill and the blur's reach are
        // inside it, on the grid.
        let (from, w, h) = (m.start_rect(), layer.w, layer.h);
        assert!(layer.x <= from.x - 36.0 && layer.x + w >= from.x + from.w + 36.0);
        assert!(layer.y + h >= from.y + from.h + 36.0);
        assert_eq!((layer.x % 32.0, layer.y % 32.0), (0.0, 0.0));
        assert_eq!(blur_layer(&m, capsule, pill, surface, at(200)).0, layer);
        // The pill and the capsule sliding a little keep the layer's size.
        let moved = r(pill.x + 7.0, pill.y, pill.w + 5.0, pill.h);
        let wider = r(capsule.x - 4.0, capsule.y, capsule.w + 4.0, capsule.h);
        assert_eq!(blur_layer(&m, wider, moved, surface, at(150)).0, layer);
        // An open window, or a close under reduced motion, has no blur: it
        // draws plain.
        let open = SheetMotion::open(pill, &Theme::dark(), t0, false);
        assert_eq!(blur_layer(&open, capsule, pill, surface, t0).1, 0.0);
        let reduced = closing(true, t0);
        assert_eq!(blur_layer(&reduced, capsule, pill, surface, at(100)).1, 0.0);
    }

    #[test]
    fn a_layer_is_never_larger_than_the_surface() {
        let t0 = Instant::now();
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let capsule = r(900.0, 8.0, 270.0, 40.0);
        let m = closing(false, t0);
        let now = t0 + Duration::from_millis(500);
        let (layer, _) = blur_layer(&m, capsule, pill, (400.0, 200.0), now);
        assert!(layer.x >= 0.0 && layer.y >= 0.0);
        assert!(layer.x + layer.w <= 400.0 && layer.y + layer.h <= 200.0);
        // A pill beyond the surface leaves an empty layer, drawn plain.
        let far = r(5000.0, 5000.0, 1.0, 1.0);
        let (gone, _) = blur_layer(&m, far, far, (100.0, 100.0), now);
        assert!(gone.w >= 0.0 && gone.h >= 0.0);
    }

    #[test]
    fn the_body_shrinks_with_its_window() {
        let target = r(930.0, 56.0, 240.0, 236.0);
        assert_eq!(follow(target, target), (1.0, 1.0));
        assert_eq!(follow(target, r(1000.0, 30.0, 120.0, 59.0)), (0.5, 0.25));
        // A window with no size shrinks nothing and never goes negative.
        assert_eq!(follow(r(0.0, 0.0, 0.0, 0.0), target), (1.0, 1.0));
        assert_eq!(follow(target, r(0.0, 0.0, -5.0, 10.0)).0, 0.0);
    }
}
