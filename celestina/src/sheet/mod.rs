//! Capsule menus: a piece of a bar capsule opens a window of its own under
//! the bar (CEL-D-002, CEL-D-009). The piece stays in its slot in the bar,
//! under the grey open pill (CEL-D-014), and the window hangs from it. The
//! pure rules ([`geometry`]) and clocks ([`motion`]) hold no painter and no
//! GL; [`Sheets`] owns what is open on one output: the window's rect, its
//! body, the body it is leaving when another menu of the same capsule takes
//! its place (the window stays and travels to the new menu's shape,
//! CEL-D-015), the window of another capsule that closes towards its own
//! piece on its own clock, the open that waits for the Island's page to close
//! and the scroll. The Island wires it to the bar (`island::sheets`).
//!
//! Nothing here stores a measure: whatever cuts or measures text takes the
//! caller's `Measure` for the length of the call, because the painter is
//! already borrowed wherever a sheet is drawn or fed.

pub mod body;
pub mod drop;
pub mod geometry;
pub mod liquid;
pub mod motion;
pub mod subtitle;
pub mod tray_overflow;
pub mod workspaces;

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, TextStyle, Theme};
use celestina_ui::{Key, Scroll, Tree};
use celestina_wayland::InputEvent;

pub use body::{BodyOutcome, PageBody, SheetBody, TrayOverflowBody, WorkspacesBody};
use drop::{DropFrame, Source};
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

/// One window's glass at a frame: the capsule zone it hangs from, its frame
/// (rect, liquid, glass opacity) and where it hangs on the piece. The
/// Island joins the ones of a zone into one Haze path with that capsule
/// (CEL-D-013).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowGlass {
    pub zone: usize,
    pub frame: DropFrame,
    pub source: Source,
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
    /// The content a travel is leaving, until it has gone (90 ms in).
    old: Option<OldBody>,
    /// The window the content left sat in: with the live target it sizes the
    /// travel's blur layer, so the layer keeps its size through the travel.
    left: Rect,
}

/// The body a travel leaves: it shows (fading and blurring out) where it was
/// laid out, at its own scroll, and takes no input.
#[derive(Debug)]
struct OldBody {
    body: SheetBody,
    scroll: f32,
    /// The window it had.
    target: Rect,
}

/// The window of another capsule left when a menu opens: its body with the
/// scroll it had, on its own clock, closing towards the pill of its own
/// piece.
#[derive(Debug)]
struct Leaving {
    body: SheetBody,
    scroll: f32,
    /// The bar zone whose capsule it hung from.
    zone: usize,
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

/// The owner of what is open on one output's bar: at most one sheet (and the
/// ones still closing on other capsules), plus the one thing that waits for
/// the Island's page to close.
#[derive(Debug, Default)]
pub struct Sheets {
    open: Option<OpenSheet>,
    /// The windows of other capsules left when a menu opened, each closing
    /// towards its own piece's pill on its own clock until it has closed.
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
    /// closing sheet's until its drop merges into the capsule
    /// ([`SheetMotion::merged`]: at [`motion::PIECE_RESTORE_AT`] of the
    /// close, at once under reduced motion). A window taken back by a
    /// switch or a reopen is no longer closing, so its piece is held again.
    #[must_use]
    pub fn held_piece(&self, now: Instant) -> Option<Key> {
        let o = self.open.as_ref()?;
        (!o.motion.merged(now)).then_some(o.piece)
    }

    /// Every piece the bar holds at `now`: the open sheet's first (as
    /// [`Sheets::held_piece`]), then the piece of each window still closing
    /// on another capsule until that window's own merge. Under reduced motion
    /// a window gives its piece back as its close starts, so none of those is
    /// held.
    #[must_use]
    pub fn held_pieces(&self, now: Instant) -> Vec<Key> {
        let mut out: Vec<Key> = self.held_piece(now).into_iter().collect();
        for l in self.leaving.iter().filter(|l| !l.motion.merged(now)) {
            if !out.contains(&l.piece) {
                out.push(l.piece);
            }
        }
        out
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
        self.frame(anchor, now).map(|f| f.rect)
    }

    /// The window's frame at `now`, or `None` when none is shown. A frame
    /// that is not `drawn` is the drop's liquid alone.
    #[must_use]
    pub fn frame(&self, anchor: &Anchor, now: Instant) -> Option<DropFrame> {
        let o = self.open.as_ref().filter(|o| !o.motion.closed(now))?;
        let target = Sheets::target_of(anchor, o.body.size());
        Some(o.motion.frame(now, target, anchor.pill, anchor.surface))
    }

    /// The glass of every window painted at `now`: the ones closing on other
    /// capsules, then the open one (or closing, under `anchor`). A frame
    /// whose window has no area paints none. What the Island's Haze paths
    /// are built from.
    #[must_use]
    pub fn glass(
        &self,
        anchor: Option<&Anchor>,
        surface: (f32, f32),
        now: Instant,
    ) -> Vec<WindowGlass> {
        let mut out = Vec::new();
        let mut push = |zone: usize, mut frame: DropFrame, pill: Rect| {
            if frame.drawn && !(frame.rect.w > 0.0 && frame.rect.h > 0.0) {
                frame.drawn = false;
            }
            out.push(WindowGlass {
                zone,
                frame,
                source: Source::of(pill),
            });
        };
        for l in self.leaving.iter().filter(|l| !l.motion.closed(now)) {
            push(
                l.zone,
                l.motion.frame(now, l.target, l.pill, surface),
                l.pill,
            );
        }
        if let (Some(o), Some(anchor)) = (self.open.as_ref(), anchor) {
            if let Some(frame) = self.frame(anchor, now) {
                push(o.zone, frame, anchor.pill);
            }
        }
        out
    }

    /// Whether (`x`, `y`) is on a window closing on another capsule, which is
    /// still painted for a moment but takes nothing.
    #[must_use]
    pub fn on_leaving(&self, surface: (f32, f32), now: Instant, x: f32, y: f32) -> bool {
        self.leaving.iter().any(|l| {
            let frame = l.motion.frame(now, l.target, l.pill, surface);
            !l.motion.closed(now)
                && frame.drawn
                && geometry::hit(frame.rect, x, y) == geometry::Hit::Body
        })
    }

    /// The pill of the piece the window closing on another capsule closes
    /// towards, as the bar has it now.
    pub fn follow_leaving(&mut self, piece: Key, pill: Rect) {
        for l in self.leaving.iter_mut().filter(|l| l.piece == piece) {
            l.pill = pill;
        }
    }

    /// The pieces whose windows are closing on other capsules, while they
    /// close.
    #[must_use]
    pub fn leaving_pieces(&self, now: Instant) -> Vec<Key> {
        self.leaving
            .iter()
            .filter(|l| !l.motion.closed(now))
            .map(|l| l.piece)
            .collect()
    }

    /// Drops what has finished: a window whose close ended, a window that
    /// has closed, the content a travel has left.
    fn reap(&mut self, now: Instant) {
        if self.open.as_ref().is_some_and(|o| o.motion.closed(now)) {
            self.open = None;
            self.scroll = Scroll::default();
            self.hovering = false;
            self.bump();
        }
        if let Some(o) = self.open.as_mut() {
            if o.old.is_some() && o.motion.old_content(now).is_none() {
                if let Some(mut old) = o.old.take() {
                    old.body.flush(now);
                }
                self.bump();
            }
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

    /// Opens `request` from `anchor`. In this order:
    /// - a window on the request's capsule (open or closing) stays and
    ///   travels to the new menu's shape from the one it shows; the content it
    ///   leaves fades and blurs out on it, and the one it holds longer wins
    ///   when it was already leaving one;
    /// - a window of that capsule still closing in `leaving` is taken back
    ///   and travels from its shape;
    /// - a window open on another capsule goes to `leaving` and closes
    ///   towards its own piece (what its body held goes out first), and the
    ///   new window opens from its piece's pill at once;
    /// - with nothing open, a fresh window grows from the pill.
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
        let fresh =
            |motion: SheetMotion, body: SheetBody, old: Option<OldBody>, left: Rect| OpenSheet {
                zone,
                piece,
                kind,
                body,
                motion,
                anchor: *anchor,
                target,
                size,
                old,
                left,
            };
        if self.open.as_ref().is_some_and(|o| o.zone == zone) {
            self.travel_in_place(fresh, body, theme, now, reduced);
        } else {
            let back = self
                .leaving
                .iter()
                .position(|l| l.zone == zone)
                .map(|i| self.leaving.remove(i));
            self.park_open(now, anchor.surface);
            if let Some(l) = back {
                let mut motion = l.motion;
                let shown = motion.frame(now, l.target, l.pill, anchor.surface);
                let content = motion.content_shown(now);
                motion.set_reduced(reduced);
                motion.travel(shown.rect, shown.radius, content, theme, now);
                let old = motion.old_content(now).is_some().then_some(OldBody {
                    body: l.body,
                    scroll: l.scroll,
                    target: l.target,
                });
                self.open = Some(fresh(motion, body, old, l.target));
            } else {
                let motion = SheetMotion::open(anchor.pill, theme, now, reduced);
                self.open = Some(fresh(motion, body, None, target));
            }
            self.scroll = Scroll::default();
        }
        self.hovering = false;
        self.refresh_extent();
        self.bump();
    }

    /// The open window, on the request's capsule, travels to the request's
    /// shape. `make` builds the window's new state around `body`.
    fn travel_in_place(
        &mut self,
        make: impl FnOnce(SheetMotion, SheetBody, Option<OldBody>, Rect) -> OpenSheet,
        body: SheetBody,
        theme: &Theme,
        now: Instant,
        reduced: bool,
    ) {
        let Some(o) = self.open.take() else {
            return;
        };
        let surface = o.anchor.surface;
        let OpenSheet {
            body: previous,
            mut motion,
            anchor,
            target,
            old,
            ..
        } = o;
        let shown = motion.frame(now, target, anchor.pill, surface);
        let previous_shown = motion.content_shown(now);
        // The content still on its way out, when the window was already
        // leaving one.
        let left = old.and_then(|mut old| match motion.old_content(now) {
            Some(s) => Some((old, s)),
            None => {
                old.body.flush(now);
                None
            }
        });
        // What the body left holds goes out now, as it did when a switch
        // closed it: only its painting stays for the travel's first 90 ms.
        let mut previous = previous;
        previous.flush(now);
        let previous = OldBody {
            body: previous,
            scroll: self.scroll.offset(),
            target,
        };
        // One old content at a time: whichever shows more now (ties: the
        // body that was open); the other is flushed and dropped.
        let (kept, content) = match left {
            Some((old, s)) if s.alpha > previous_shown.alpha => {
                let mut gone = previous;
                gone.body.flush(now);
                (old, s)
            }
            left => {
                if let Some((mut old, _)) = left {
                    old.body.flush(now);
                }
                (previous, previous_shown)
            }
        };
        motion.set_reduced(reduced);
        motion.travel(shown.rect, shown.radius, content, theme, now);
        let left = kept.target;
        let held = if motion.old_content(now).is_some() {
            Some(kept)
        } else {
            let mut kept = kept;
            kept.body.flush(now);
            None
        };
        self.open = Some(make(motion, body, held, left));
        self.scroll = Scroll::default();
    }

    /// Sends the open window, if any, to `leaving` on its close towards its
    /// own piece (the close it already runs, if it was closing). What its
    /// body held goes out first; a content it was leaving is dropped.
    fn park_open(&mut self, now: Instant, surface: (f32, f32)) {
        let Some(o) = self.open.take() else {
            return;
        };
        let OpenSheet {
            zone,
            piece,
            mut body,
            mut motion,
            anchor,
            target,
            old,
            ..
        } = o;
        if let Some(mut old) = old {
            old.body.flush(now);
        }
        if !motion.is_closing() {
            let shown = motion.frame(now, target, anchor.pill, surface);
            body.flush(now);
            motion.close(shown.rect, shown.radius, anchor.pill, now);
        }
        self.leaving.push(Leaving {
            body,
            scroll: self.scroll.offset(),
            zone,
            piece,
            motion,
            target,
            pill: anchor.pill,
            capsule: anchor.capsule,
        });
    }

    /// Starts the close: the window goes towards the pill of its piece. What
    /// its body holds goes out first; a close that cuts a travel short fades
    /// from the content and glass shown then, and drops the content the
    /// travel was leaving. Does nothing without an open window.
    /// The piece gives back its pill and opacity when the drop merges
    /// ([`motion::PIECE_RESTORE_AT`], the bar's clock).
    pub fn close(&mut self, anchor: &Anchor, now: Instant) {
        let Some(shown) = self.frame(anchor, now) else {
            return;
        };
        if !self.is_open() {
            return;
        }
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.body.flush(now);
        if let Some(mut old) = o.old.take() {
            old.body.flush(now);
        }
        o.motion.close(shown.rect, shown.radius, anchor.pill, now);
        self.hovering = false;
        self.bump();
    }

    /// Takes the open sheet away at once, with no exit motion: what its body
    /// holds goes out first. The next [`Sheets::open`] then grows from its
    /// piece's pill.
    pub fn drop_open(&mut self, now: Instant) {
        if let Some(mut old) = self.open.take() {
            old.body.flush(now);
            if let Some(mut left) = old.old.take() {
                left.body.flush(now);
            }
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
                let shown = o
                    .motion
                    .rect(now, o.target, o.anchor.pill, o.anchor.surface);
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

    /// Lays the bodies out (the open one, the content a travel leaves and
    /// the windows closing on other capsules).
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        if let Some(o) = self.open.as_mut() {
            o.body.layout(lay);
            if let Some(old) = o.old.as_mut() {
                old.body.layout(lay);
            }
        }
        for l in &mut self.leaving {
            l.body.layout(lay);
        }
    }

    /// How the bodies are painted at `now`: one entry per window closing on
    /// another capsule (in `leaving`'s order, `None` for one that is not
    /// shown) and the open one's. Each body sits at its window's target,
    /// clipped to its frame's rounded rect, at the frame's content opacity and
    /// blur: the clip reveals the body, it is not scaled with the window.
    fn looks(&self, anchor: &Anchor, now: Instant) -> (Vec<Option<Look>>, Option<Look>) {
        let surface = anchor.surface;
        let leaving = self
            .leaving
            .iter()
            .map(|l| {
                if l.motion.closed(now) {
                    return None;
                }
                let frame = l.motion.frame(now, l.target, l.pill, surface);
                (frame.rect.w > 0.0 && frame.rect.h > 0.0).then(|| Look {
                    frame,
                    at: (l.target.x, l.target.y - l.scroll),
                    layer: blur_layer(
                        l.motion.start_rect(),
                        frame.blur,
                        l.capsule,
                        l.pill,
                        surface,
                    ),
                })
            })
            .collect();
        let open = self.open.as_ref().and_then(|o| {
            let frame = self.frame(anchor, now)?;
            if frame.rect.w <= 0.0 || frame.rect.h <= 0.0 {
                return None;
            }
            let target = Sheets::target_of(anchor, o.body.size());
            let at = (target.x, target.y - self.scroll.offset());
            let layer = if o.motion.is_closing() {
                blur_layer(
                    o.motion.start_rect(),
                    frame.blur,
                    anchor.capsule,
                    anchor.pill,
                    surface,
                )
            } else {
                (cover(&[o.left, target], surface), frame.blur)
            };
            Some(Look { frame, at, layer })
        });
        (leaving, open)
    }

    /// Paints the windows' bodies: the ones closing on other capsules
    /// (towards their pills) under the open one. The glass is not here: the
    /// Island paints it, joined to the capsules, before the bar
    /// ([`Sheets::glass`]). An open window then draws the content it is
    /// leaving and the new body, each at its own opacity and blur, sitting at
    /// its own window's target: the rect reveals them rather than carrying
    /// them.
    pub fn paint(&mut self, painter: &mut Painter, anchor: &Anchor, now: Instant) {
        let (leaving, open) = self.looks(anchor, now);
        for (l, look) in self.leaving.iter_mut().zip(leaving) {
            if let Some(look) = look {
                paint_look(painter, &look, &mut |p| l.body.paint(p, now));
            }
        }
        let Some(look) = open else {
            return;
        };
        let Some(o) = self.open.as_mut() else {
            return;
        };
        let Look { frame, layer, .. } = look;
        if !o.motion.is_closing() {
            if let (Some(old), Some(shown)) = (o.old.as_mut(), o.motion.old_content(now)) {
                let at = (old.target.x, old.target.y - old.scroll);
                paint_content(
                    painter,
                    frame.rect,
                    frame.radius,
                    at,
                    shown.alpha,
                    (layer.0, shown.blur),
                    &mut |p| old.body.paint(p, now),
                );
            }
        }
        paint_look(painter, &look, &mut |p| o.body.paint(p, now));
    }

    /// Whether the capsule's changes at `now` are the bar's slide (tests).
    #[cfg(test)]
    #[must_use]
    pub fn follows_slide(&self, now: Instant) -> bool {
        slide_follows(self.slide_until, now)
    }

    /// The pill the window closing on another capsule closes towards
    /// (tests).
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

    /// Whether a window is closing on another capsule (tests).
    #[cfg(test)]
    #[must_use]
    pub fn is_leaving(&self) -> bool {
        !self.leaving.is_empty()
    }

    /// Whether an old content is held (tests).
    #[cfg(test)]
    #[must_use]
    pub fn is_swapping(&self) -> bool {
        self.open.as_ref().is_some_and(|o| o.old.is_some())
    }

    /// The opacity of the old content at `now` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn swap_alpha(&self, now: Instant) -> Option<f32> {
        let o = self.open.as_ref()?;
        o.old.as_ref()?;
        o.motion.old_content(now).map(|s| s.alpha)
    }

    /// Whether a clock runs and the surface needs a frame. A sheet at rest
    /// asks for none.
    #[must_use]
    pub fn wants_frame(&self, now: Instant) -> bool {
        let open = self.open.as_ref().is_some_and(|o| {
            o.motion.wants_frame(now)
                || o.motion.closed(now)
                || o.old.is_some()
                || o.body.is_animating(now)
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

/// The offscreen layer a closing window's body paints through, with its blur
/// `sigma` in logical px (0 for a window that is not blurred). The layer
/// holds the window as it was shown, the capsule it hung from, the pill it
/// heads to and the blur's reach, on a grid, whole through the close so its
/// images are made once.
fn blur_layer(
    start: Rect,
    sigma: f32,
    capsule: Rect,
    pill: Rect,
    surface: (f32, f32),
) -> (Rect, f32) {
    if sigma <= 0.0 {
        return (pill, 0.0);
    }
    (cover(&[start, pill, capsule], surface), sigma)
}

/// The room a blurred layer needs for `rects`: their union plus
/// [`BLUR_REACH`], its edges cut to [`LAYER_GRID`] and to the surface.
fn cover(rects: &[Rect], surface: (f32, f32)) -> Rect {
    let low = |edge: fn(&Rect) -> f32| rects.iter().map(edge).fold(f32::INFINITY, f32::min);
    let high = |edge: fn(&Rect) -> f32| rects.iter().map(edge).fold(f32::NEG_INFINITY, f32::max);
    let (x0, y0) = (low(|r| r.x) - BLUR_REACH, low(|r| r.y) - BLUR_REACH);
    let (x1, y1) = (
        high(|r| r.x + r.w) + BLUR_REACH,
        high(|r| r.y + r.h) + BLUR_REACH,
    );
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
    }
    let down = |v: f32| (v / LAYER_GRID).floor() * LAYER_GRID;
    let up = |v: f32| (v / LAYER_GRID).ceil() * LAYER_GRID;
    let (x1, y1) = (
        up(x1).min(surface.0).max(0.0),
        up(y1).min(surface.1).max(0.0),
    );
    let (x0, y0) = (down(x0).max(0.0).min(x1), down(y0).max(0.0).min(y1));
    Rect {
        x: x0,
        y: y0,
        w: (x1 - x0).max(0.0),
        h: (y1 - y0).max(0.0),
    }
}

/// A content's blur under this (logical px) is drawn plain.
const MIN_CONTENT_BLUR_PX: f32 = 0.05;

/// What one window's body is painted with at a frame.
#[derive(Clone, Copy, Debug)]
struct Look {
    /// The window's frame: its rect and radius clip the body, its content
    /// and blur dress it.
    frame: DropFrame,
    /// The body's origin in the surface, the scroll taken off.
    at: (f32, f32),
    /// The blur layer and the blur (0: none).
    layer: (Rect, f32),
}

/// Paints one window's body under one rounded clip of its frame, at the
/// frame's content opacity and blur.
fn paint_look(painter: &mut Painter, look: &Look, body: &mut dyn FnMut(&mut Painter)) {
    let DropFrame {
        rect,
        radius,
        content,
        ..
    } = look.frame;
    paint_content(painter, rect, radius, look.at, content, look.layer, body);
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

/// Paints one content of a travelling window: under one rounded clip of
/// `rect`, at `at`, at `alpha`. Below 0.05 px of blur it is plain; otherwise
/// the clip is set outside the blur's layer (the composite honours it) and the
/// content goes through [`Painter::blurred`] over `blur.0`.
fn paint_content(
    painter: &mut Painter,
    rect: Rect,
    radius: f32,
    at: (f32, f32),
    alpha: f32,
    blur: (Rect, f32),
    body: &mut dyn FnMut(&mut Painter),
) {
    if alpha <= 0.0 {
        return;
    }
    let (layer, sigma) = blur;
    if sigma < MIN_CONTENT_BLUR_PX {
        paint_body(painter, rect, radius, at, alpha, body);
        return;
    }
    painter.push_clip(rect, radius);
    painter.push_state();
    painter.opacity(alpha);
    painter.blurred(layer, sigma, &mut |p| {
        p.push_state();
        p.translate(at.0, at.1);
        body(p);
        p.pop_state();
    });
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
    fn the_piece_is_held_until_its_drop_merges() {
        let t0 = Instant::now();
        let piece = Key::named("volume");
        let (mut sheets, anchor) = bare_open(piece, t0);
        assert_eq!(sheets.held_piece(t0), Some(piece));
        assert_eq!(Sheets::default().held_piece(t0), None);
        let at = t0 + ms(500);
        assert_eq!(sheets.held_piece(at), Some(piece), "open and settled");
        sheets.close(&anchor, at);
        // The 400 ms close: held through 319 ms, given back at 320 (80 %).
        assert_eq!(sheets.held_piece(at), Some(piece));
        assert_eq!(sheets.held_piece(at + ms(319)), Some(piece));
        assert_eq!(sheets.held_piece(at + ms(320)), None);
        assert_eq!(sheets.held_piece(at + ms(400)), None);
        assert!((motion::PIECE_RESTORE_AT - drop::MERGE_AT).abs() < f32::EPSILON);
    }

    #[test]
    fn the_piece_is_held_again_when_a_reopen_takes_the_window_back() {
        let t0 = Instant::now();
        let piece = Key::named("volume");
        let (mut sheets, anchor) = bare_open(piece, t0);
        let at = t0 + ms(500);
        sheets.close(&anchor, at);
        // Past the merge the piece is free; the reopen holds it again at the
        // frame it asks, and the window travels (no longer closing).
        assert_eq!(sheets.held_piece(at + ms(330)), None);
        zoned(&mut sheets, 3, piece, &anchor, at + ms(330));
        assert_eq!(sheets.held_piece(at + ms(330)), Some(piece));
        assert!(sheets.is_open());
    }

    #[test]
    fn reduced_motion_gives_the_piece_back_when_the_close_starts() {
        let t0 = Instant::now();
        let piece = Key::named("volume");
        let (mut sheets, anchor) = bare_open(piece, t0);
        let at = t0 + ms(500);
        sheets.open(
            Request {
                zone: 3,
                piece,
                kind: SheetKind::Workspaces,
                body: SheetBody::Workspaces(Box::new(WorkspacesBody::new(
                    &Theme::dark(),
                    true,
                    celestina_ui::view::row(Key::named("body")).size(100.0, 40.0),
                    vec![1],
                    at,
                ))),
            },
            &anchor,
            &Theme::dark(),
            at,
            true,
        );
        assert_eq!(sheets.held_piece(at + ms(200)), Some(piece));
        sheets.close(&anchor, at + ms(200));
        assert_eq!(sheets.held_piece(at + ms(200)), None);
    }

    fn closing(reduced: bool, t0: Instant) -> SheetMotion {
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let mut m = SheetMotion::open(pill, &Theme::dark(), t0, reduced);
        m.close(
            r(930.0, 56.0, 240.0, 236.0),
            20.0,
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
        let from = m.start_rect();
        let (layer, sigma) = blur_layer(from, 6.0, capsule, pill, surface);
        assert!(sigma > 0.0);
        // The window as shown, the capsule, the pill and the blur's reach are
        // inside it, on the grid.
        let (w, h) = (layer.w, layer.h);
        assert!(layer.x <= from.x - 36.0 && layer.x + w >= from.x + from.w + 36.0);
        assert!(layer.y + h >= from.y + from.h + 36.0);
        assert_eq!((layer.x % 32.0, layer.y % 32.0), (0.0, 0.0));
        assert_eq!(blur_layer(from, 6.0, capsule, pill, surface).0, layer);
        // The pill and the capsule sliding a little keep the layer's size.
        let moved = r(pill.x + 7.0, pill.y, pill.w + 5.0, pill.h);
        let wider = r(capsule.x - 4.0, capsule.y, capsule.w + 4.0, capsule.h);
        assert_eq!(blur_layer(from, 6.0, wider, moved, surface).0, layer);
        // No blur, no layer: the body draws plain.
        assert_eq!(blur_layer(from, 0.0, capsule, pill, surface).1, 0.0);
    }

    #[test]
    fn a_layer_is_never_larger_than_the_surface() {
        let t0 = Instant::now();
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let capsule = r(900.0, 8.0, 270.0, 40.0);
        let m = closing(false, t0);
        let (layer, _) = blur_layer(m.start_rect(), 6.0, capsule, pill, (400.0, 200.0));
        assert!(layer.x >= 0.0 && layer.y >= 0.0);
        assert!(layer.x + layer.w <= 400.0 && layer.y + layer.h <= 200.0);
        // A pill beyond the surface leaves an empty layer, drawn plain.
        let far = r(5000.0, 5000.0, 1.0, 1.0);
        let (gone, _) = blur_layer(m.start_rect(), 6.0, far, far, (100.0, 100.0));
        assert!(gone.w >= 0.0 && gone.h >= 0.0);
    }

    #[test]
    fn the_travel_layer_covers_both_bodies_on_the_grid() {
        let surface = (1920.0, 1080.0);
        let old = r(930.0, 56.0, 240.0, 236.0);
        let new = r(800.0, 56.0, 360.0, 300.0);
        let layer = cover(&[old, new], surface);
        for body in [old, new] {
            assert!(layer.x <= body.x - BLUR_REACH);
            assert!(layer.y <= body.y - BLUR_REACH);
            assert!(layer.x + layer.w >= body.x + body.w + BLUR_REACH);
            assert!(layer.y + layer.h >= body.y + body.h + BLUR_REACH);
        }
        for edge in [layer.x, layer.y, layer.x + layer.w, layer.y + layer.h] {
            assert_eq!(edge % LAYER_GRID, 0.0, "{layer:?}");
        }
        assert!(layer.x >= 0.0 && layer.y >= 0.0);
        assert!(layer.x + layer.w <= surface.0 && layer.y + layer.h <= surface.1);
        // The order does not matter, and a body beside the surface's edge is
        // cut to it.
        assert_eq!(cover(&[new, old], surface), layer);
        let cut = cover(&[old, new], (1000.0, 200.0));
        assert!(cut.x + cut.w <= 1000.0 && cut.y + cut.h <= 200.0);
    }

    fn zoned(sheets: &mut Sheets, zone: usize, piece: Key, anchor: &Anchor, now: Instant) {
        let body = WorkspacesBody::new(
            &Theme::dark(),
            false,
            celestina_ui::view::row(Key::named("body")).size(100.0, 40.0),
            vec![1],
            now,
        );
        sheets.open(
            Request {
                zone,
                piece,
                kind: SheetKind::Workspaces,
                body: SheetBody::Workspaces(Box::new(body)),
            },
            anchor,
            &Theme::dark(),
            now,
            false,
        );
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn the_glass_lists_the_open_window_and_every_leaving_one_until_they_close() {
        let t0 = Instant::now();
        let (mut sheets, first) = bare_open(Key::named("volume"), t0);
        let surface = first.surface;
        let got = sheets.glass(Some(&first), surface, t0 + ms(100));
        assert_eq!(got.iter().map(|g| g.zone).collect::<Vec<_>>(), [3]);
        assert_eq!(got[0].source, Source::of(first.pill));
        // Another capsule opens: the old window closes on its own clock.
        let other = Anchor {
            capsule: r(200.0, 8.0, 200.0, 40.0),
            pill: r(240.0, 14.0, 60.0, 28.0),
            ..first
        };
        let t1 = t0 + ms(600);
        zoned(&mut sheets, 1, Key::named("bell"), &other, t1);
        let got = sheets.glass(Some(&other), surface, t1 + ms(50));
        assert_eq!(got.iter().map(|g| g.zone).collect::<Vec<_>>(), [3, 1]);
        assert_eq!(got[0].source, Source::of(first.pill), "its own piece");
        assert_eq!(got[1].source, Source::of(other.pill));
        // The old one is gone when its close ends.
        let got = sheets.glass(Some(&other), surface, t1 + ms(400));
        assert_eq!(got.iter().map(|g| g.zone).collect::<Vec<_>>(), [1]);
        // Nothing is open, nothing is listed.
        assert!(Sheets::default().glass(None, surface, t1).is_empty());
    }

    #[test]
    fn a_leaving_windows_piece_is_held_until_its_own_merge() {
        let t0 = Instant::now();
        let volume = Key::named("volume");
        let bell = Key::named("bell");
        let (mut sheets, first) = bare_open(volume, t0);
        assert_eq!(sheets.held_pieces(t0), [volume]);
        assert!(Sheets::default().held_pieces(t0).is_empty());
        let other = Anchor {
            capsule: r(200.0, 8.0, 200.0, 40.0),
            pill: r(240.0, 14.0, 60.0, 28.0),
            ..first
        };
        let t1 = t0 + ms(600);
        zoned(&mut sheets, 1, bell, &other, t1);
        // The new window's piece first, then the old one until 320 ms (80 %
        // of its 400 ms close).
        assert_eq!(sheets.held_pieces(t1), [bell, volume]);
        assert_eq!(sheets.held_pieces(t1 + ms(319)), [bell, volume]);
        assert_eq!(sheets.held_pieces(t1 + ms(320)), [bell]);
        assert_eq!(sheets.held_pieces(t1 + ms(900)), [bell]);
        // The open one is held through its own close up to its merge, and
        // alone it is the same as `held_piece`.
        sheets.close(&other, t1 + ms(100));
        assert_eq!(sheets.held_pieces(t1 + ms(100)), [bell, volume]);
        assert_eq!(sheets.held_pieces(t1 + ms(330)), [bell]);
        assert_eq!(sheets.held_piece(t1 + ms(330)), Some(bell));
        assert!(sheets.held_pieces(t1 + ms(420)).is_empty());
        // A window taken back is open again and held once.
        let (mut sheets, first) = bare_open(volume, t0);
        zoned(&mut sheets, 1, bell, &other, t1);
        zoned(&mut sheets, 3, volume, &first, t1 + ms(100));
        assert_eq!(sheets.held_pieces(t1 + ms(100)), [volume, bell]);
    }

    #[test]
    fn under_reduced_motion_a_leaving_window_holds_no_piece() {
        let t0 = Instant::now();
        let volume = Key::named("volume");
        let bell = Key::named("bell");
        let pill = r(1040.0, 14.0, 60.0, 28.0);
        let first = Anchor {
            capsule: r(1000.0, 8.0, 200.0, 40.0),
            edge: Edge::Start,
            pill,
            surface: (1920.0, 560.0),
        };
        // A window opened under reduced motion fades where it stands, and
        // gives its piece back as its close starts.
        let body = WorkspacesBody::new(
            &Theme::dark(),
            true,
            celestina_ui::view::row(Key::named("body")).size(100.0, 40.0),
            vec![1],
            t0,
        );
        let mut sheets = Sheets::default();
        sheets.open(
            Request {
                zone: 3,
                piece: volume,
                kind: SheetKind::Workspaces,
                body: SheetBody::Workspaces(Box::new(body)),
            },
            &first,
            &Theme::dark(),
            t0,
            true,
        );
        let other = Anchor {
            capsule: r(200.0, 8.0, 200.0, 40.0),
            pill: r(240.0, 14.0, 60.0, 28.0),
            ..first
        };
        let t1 = t0 + ms(600);
        zoned(&mut sheets, 1, bell, &other, t1);
        assert!(sheets.is_leaving());
        assert_eq!(sheets.held_pieces(t1), [bell]);
        assert_eq!(sheets.held_pieces(t1 + ms(50)), [bell]);
    }

    #[test]
    fn the_body_is_clipped_to_the_drop_and_blurred_while_opening() {
        let t0 = Instant::now();
        let (sheets, anchor) = bare_open(Key::named("volume"), t0);
        // At 70 % of the open the window is drawn, its content is coming in
        // and its blur is going out.
        let now = t0 + ms(322);
        let frame = sheets.frame(&anchor, now).expect("shown");
        assert!(frame.drawn && frame.content > 0.0 && frame.blur > 0.0);
        let (_, open) = sheets.looks(&anchor, now);
        let look = open.expect("a body look");
        assert_eq!(look.frame, frame, "the clip is the frame's rect and radius");
        let target = Sheets::target_of(
            &anchor,
            sheets.open.as_ref().map_or((0.0, 0.0), |o| o.body.size()),
        );
        assert_eq!(look.at, (target.x, target.y), "laid out at its target");
        assert_eq!(look.layer.1, frame.blur);
        assert!(look.layer.0.w > 0.0 && look.layer.0.h > 0.0);
        assert!(
            look.layer.0.x <= target.x - BLUR_REACH,
            "the layer holds the body"
        );
        // A liquid-only frame has a clip of the bulb and no content.
        let early = sheets.looks(&anchor, t0 + ms(100)).1.expect("a look");
        assert!(!early.frame.drawn);
        assert_eq!(early.frame.content, 0.0);
    }

    #[test]
    fn a_closing_body_stays_at_its_target_while_its_clip_shrinks() {
        let t0 = Instant::now();
        let (mut sheets, anchor) = bare_open(Key::named("volume"), t0);
        let settled = t0 + ms(600);
        let target = Sheets::target_of(&anchor, (100.0, 40.0));
        sheets.close(&anchor, settled);
        let mut sizes = Vec::new();
        for n in [0, 60, 120, 160] {
            let look = sheets
                .looks(&anchor, settled + ms(n))
                .1
                .expect("a look while it gathers");
            assert_eq!(look.at.0, target.x, "+{n} ms: not scaled");
            assert_eq!(look.at.1, target.y, "+{n} ms");
            assert!(look.frame.drawn);
            sizes.push(look.frame.rect.w);
        }
        assert!(sizes.windows(2).all(|w| w[1] < w[0]), "{sizes:?}");
    }
}
