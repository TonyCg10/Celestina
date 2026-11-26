//! Capsule menus: a piece of a bar capsule opens a window of its own under
//! the bar (CEL-D-002, CEL-D-009). The pure rules ([`geometry`]) and clocks
//! ([`motion`]) hold no painter and no GL; [`Sheets`] owns what is open on one
//! output: the window's rect, its body, the window being left in a switch
//! (it folds into its piece on its own clock), the open that waits for
//! another thing to close and the scroll. The Island wires it to the bar
//! (`island::sheets`).
//!
//! Nothing here stores a measure: whatever cuts or measures text takes the
//! caller's `Measure` for the length of the call, because the painter is
//! already borrowed wherever a sheet is drawn or fed.

pub mod body;
pub mod flight;
pub mod geometry;
pub mod motion;
pub mod subtitle;
pub mod workspaces;

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, TextStyle, Theme};
use celestina_ui::{Key, Scroll, Tree, ViewKind};
use celestina_wayland::InputEvent;

pub use body::{BodyOutcome, PageBody, SheetBody, WorkspacesBody};
use flight::{BarKeys, Flight, Parts, TextPart};
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

/// What a window hangs from: its capsule as it is painted now (without the
/// open piece) and the edge of it the window lines up with, the pill of the
/// open piece (its rect in the bar while it is in its row, the pill of its
/// slot while it is out of it) and the surface.
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
    /// The bar's nodes of the piece, when its icon flies (CEL-D-008).
    keys: Option<BarKeys>,
}

/// The window being left in a switch: its body with the scroll it had, on
/// its own clock, folding into the pill of its own piece.
#[derive(Debug)]
struct Leaving {
    body: SheetBody,
    scroll: f32,
    /// The piece it was open on.
    piece: Key,
    /// The fold's clock.
    motion: SheetMotion,
    /// The window it had (where its body sits while it folds).
    target: Rect,
    /// The pill of its piece, as last seen.
    pill: Rect,
}

/// Where the subtitle's icon and text are for a window at `window`, the body
/// scrolled `scroll`: the body sits at the window's origin, plus the nodes'
/// own rects. `None` when the body has no subtitle icon.
pub(crate) fn subtitle_parts(body: &SheetBody, window: Rect, scroll: f32) -> Option<Parts> {
    let tree = body.tree();
    let shift = |r: Rect| Rect {
        x: r.x + window.x,
        y: r.y + window.y - scroll,
        ..r
    };
    let icon = tree.node_by_key(subtitle::icon_key())?;
    let ViewKind::Icon { key, .. } = tree.kind_of(icon)? else {
        return None;
    };
    let text = tree.node_by_key(subtitle::text_key()).and_then(|id| {
        let ViewKind::Text { text, style, .. } = tree.kind_of(id)? else {
            return None;
        };
        Some(TextPart {
            text: text.clone(),
            rect: shift(tree.rect(id)),
            size: style.size,
        })
    });
    Some(Parts {
        icon: Some(key.clone()),
        icon_rect: shift(tree.rect(icon)),
        text,
        k: None,
    })
}

/// `from` reduced to what the bar's nodes `keys` can carry: a first guess
/// for the end a flight up flies to, until the bar says what it holds.
fn limited_to(from: &Parts, keys: BarKeys) -> Parts {
    Parts {
        icon: from.icon.clone().filter(|_| keys.icon.is_some()),
        text: from.text.clone().filter(|_| keys.text.is_some()),
        ..from.clone()
    }
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
    /// The windows being left in switches, each folding into its own
    /// piece's pill on its own clock until it has folded.
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
    /// The pieces' icons and texts on their way between the bar and a
    /// subtitle, until they land.
    flights: Vec<Flight>,
    /// The flyer of the piece a sheet just opened on, where it was: the
    /// flight down starts from there.
    carried: Option<(Key, Parts)>,
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

    /// The pieces nobody paints in the bar at `now`: the one the sheet is
    /// open under, from its first frame to the end of its close (it is out
    /// of the row, or on its way back into it), and the ones whose icon and
    /// text still fly up to their slot (they show when they land).
    #[must_use]
    pub fn hidden_pieces(&self, now: Instant) -> Vec<Key> {
        let mut hidden: Vec<Key> = self
            .open
            .as_ref()
            .filter(|o| !o.motion.closed(now))
            .map(|o| o.piece)
            .into_iter()
            .collect();
        for f in self.flights.iter().filter(|f| f.up && !f.done(now)) {
            if !hidden.contains(&f.keys.piece) {
                hidden.push(f.keys.piece);
            }
        }
        hidden
    }

    /// The subtitle nodes of the body open on `piece` that nobody paints at
    /// `now`: its icon, and its text when the text flies, while the piece
    /// flies between the subtitle and the bar.
    pub(crate) fn subtitle_hidden(&self, piece: Key, now: Instant) -> Vec<Key> {
        let mut hidden = Vec::new();
        for f in self
            .flights
            .iter()
            .filter(|f| f.keys.piece == piece && f.flying(now))
        {
            if f.flies_icon() {
                hidden.push(subtitle::icon_key());
            }
            if f.flies_text() {
                hidden.push(subtitle::text_key());
            }
        }
        hidden
    }

    /// The open body's subtitle text (the string its piece's text must read
    /// for the text to fly).
    #[must_use]
    pub fn subtitle_text(&self) -> Option<String> {
        let o = self.open.as_ref()?;
        subtitle_parts(&o.body, o.target, self.scroll.offset())?
            .text
            .map(|t| t.text)
    }

    /// Whether any flyer is on its way or waits to start.
    #[must_use]
    pub fn is_flying(&self, now: Instant) -> bool {
        self.flights.iter().any(|f| !f.done(now))
    }

    /// Takes the flyer of `piece` away, if one is painted, and says where it
    /// was at `now`.
    fn take_flyer(&mut self, piece: Key, now: Instant) -> Option<Parts> {
        let mut flyer = None;
        self.flights.retain(|f| {
            if f.keys.piece != piece {
                return true;
            }
            if flyer.is_none() && f.flying(now) {
                flyer = Some(f.current(now));
            }
            false
        });
        flyer
    }

    /// The icon (and text) of the piece the sheet just opened on starts to
    /// fly down from the bar (`from`, its nodes `keys`) into the subtitle,
    /// from where its flyer was when one was still on its way up. Does
    /// nothing under reduced motion or when the body has no subtitle.
    pub fn fly_down(&mut self, keys: BarKeys, from: Parts, now: Instant) {
        let carried = self
            .carried
            .take()
            .filter(|(piece, _)| *piece == keys.piece)
            .map(|(_, parts)| parts);
        let scroll = self.scroll.offset();
        let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) else {
            return;
        };
        if o.motion.reduced() || o.piece != keys.piece {
            return;
        }
        o.keys = Some(keys);
        let Some(to) = subtitle_parts(&o.body, o.target, scroll) else {
            return;
        };
        let from = carried.unwrap_or(from);
        let flight = Flight::down(
            keys,
            from,
            to,
            &o.motion.ladder(),
            o.motion.overshoot(),
            now,
        );
        self.flights.retain(|f| f.keys.piece != keys.piece);
        self.flights.push(flight);
        self.bump();
    }

    /// Points the flyers at their live ends at `now`: a flight down lands on
    /// the subtitle as it is (the window's target under `anchor`), a flight up
    /// on the piece's place in the bar (`bar`). A close's flight takes its
    /// start from the subtitle when it begins, where it is painted (the body
    /// sits at the window's target, whatever the window's rect does). A flyer
    /// whose end is gone (the piece left the bar, the window is gone) is
    /// dropped.
    pub fn resolve_flights(
        &mut self,
        anchor: Option<&Anchor>,
        now: Instant,
        bar: &mut dyn FnMut(BarKeys) -> Option<Parts>,
    ) {
        let open = self.open.as_ref();
        let scroll = self.scroll.offset();
        let shown = anchor.and_then(|a| self.rect(a, now));
        self.flights.retain_mut(|f| {
            if f.up {
                let Some(to) = bar(f.keys) else {
                    return false;
                };
                f.to = to;
                if !f.armed && f.started(now) {
                    if let (Some(o), Some(a), Some(_)) = (open, anchor, shown) {
                        if o.piece == f.keys.piece {
                            let at = Sheets::target_of(a, o.body.size());
                            if let Some(from) = subtitle_parts(&o.body, at, scroll) {
                                f.from = from;
                            }
                        }
                    }
                    f.armed = true;
                }
            } else {
                let live = open
                    .filter(|o| o.piece == f.keys.piece && !o.motion.is_closing())
                    .zip(anchor)
                    .and_then(|(o, a)| {
                        let target = Sheets::target_of(a, o.body.size());
                        subtitle_parts(&o.body, target, scroll)
                    });
                let Some(to) = live else {
                    return false;
                };
                f.to = to;
            }
            true
        });
    }

    /// Paints the flyers, over everything.
    pub fn paint_flights(&self, painter: &mut Painter, dpr: f32, now: Instant) {
        for f in &self.flights {
            f.paint(painter, dpr, now);
        }
    }

    /// The flights (tests).
    #[cfg(test)]
    #[must_use]
    pub fn flights(&self) -> &[Flight] {
        &self.flights
    }

    /// Whether (`x`, `y`) is on the open sheet's subtitle row but not on its
    /// trailing overflow button: that row answers the gestures of the piece
    /// the sheet is open on.
    #[must_use]
    pub fn on_subtitle(&self, anchor: &Anchor, now: Instant, x: f32, y: f32) -> bool {
        let Some(rect) = self.rect(anchor, now) else {
            return false;
        };
        let Some(o) = self.open.as_ref().filter(|o| !o.motion.is_closing()) else {
            return false;
        };
        let at = Sheets::target_of(anchor, o.body.size());
        let (lx, ly) = (x - at.x, y - at.y + self.scroll.offset());
        let tree = o.body.tree();
        let rect_of = |key| tree.node_by_key(key).map(|id| tree.rect(id));
        rect_of(subtitle::row_key()).is_some_and(|row| row.contains(lx, ly))
            && !rect_of(crate::island::view::sheet_more_key()).is_some_and(|m| m.contains(lx, ly))
            // The window clips its body, and the bar's band is not its own.
            && geometry::hit(rect, x, y) == geometry::Hit::Body
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

    /// The last anchor the window knew: where it folds back to when its piece
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
    /// closes the rect folds into the pill of `anchor`.
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
    /// What the glass, the blur and the input regions cover.
    #[must_use]
    pub fn windows(
        &self,
        anchor: Option<&Anchor>,
        surface: (f32, f32),
        now: Instant,
    ) -> Vec<(Rect, f32)> {
        let mut out = Vec::new();
        for l in self.leaving.iter().filter(|l| !l.motion.closed(now)) {
            out.push((l.motion.rect(now, l.pill, surface), l.motion.radius(now)));
        }
        if let (Some(o), Some(rect)) = (self.open.as_ref(), anchor.and_then(|a| self.rect(a, now)))
        {
            out.push((rect, o.motion.radius(now)));
        }
        out.retain(|(r, _)| r.w > 0.0 && r.h > 0.0);
        out
    }

    /// The pill of the piece the window being left folds into, as the bar
    /// has it now.
    pub fn follow_leaving(&mut self, piece: Key, pill: Rect) {
        for l in self.leaving.iter_mut().filter(|l| l.piece == piece) {
            l.pill = pill;
        }
    }

    /// The pieces whose windows are being left, while they fold.
    #[must_use]
    pub fn leaving_pieces(&self, now: Instant) -> Vec<Key> {
        self.leaving
            .iter()
            .filter(|l| !l.motion.closed(now))
            .map(|l| l.piece)
            .collect()
    }

    /// Drops what has finished: a window whose close flight ended, a window
    /// that has folded.
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
        self.flights.retain(|f| !f.done(now));
    }

    /// Sets the scroll's extent: the body's height in the window.
    fn refresh_extent(&mut self) {
        if let Some(o) = &self.open {
            self.scroll.set_extent(o.size.1, o.target.h);
        }
    }

    /// Opens `request` from `anchor`, or, with a sheet already open (not
    /// closing), switches it to the request's body: the old window folds
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
        // The flyer of the piece that opens, if one still flies up to its
        // slot, is where the flight down starts from.
        self.carried = self.take_flyer(piece, now).map(|parts| (piece, parts));
        if self.open.as_ref().is_some_and(|o| !o.motion.is_closing()) {
            self.fly_old_piece_up(piece, now);
        }
        if let Some(o) = self.open.as_mut().filter(|o| !o.motion.is_closing()) {
            // The window being left folds into its own piece's pill on its
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
            });
            o.keys = None;
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
                keys: None,
            });
            self.scroll = Scroll::default();
        }
        self.hovering = false;
        self.refresh_extent();
        self.bump();
    }

    /// A switch from the open piece to `next`: the old piece's icon and
    /// text fly up from the subtitle (or from where its flyer was) to its
    /// slot, which reopens, in the exit step.
    fn fly_old_piece_up(&mut self, next: Key, now: Instant) {
        let Some((piece, keys, reduced, ladder)) = self
            .open
            .as_ref()
            .map(|o| (o.piece, o.keys, o.motion.reduced(), o.motion.ladder()))
        else {
            return;
        };
        let flyer = self.take_flyer(piece, now);
        let Some(keys) = keys.filter(|_| !reduced && piece != next) else {
            return;
        };
        let Some(o) = self.open.as_ref() else {
            return;
        };
        let scroll = self.scroll.offset();
        let Some(from) = flyer.or_else(|| subtitle_parts(&o.body, o.target, scroll)) else {
            return;
        };
        let to = limited_to(&from, keys);
        self.flights
            .push(Flight::up_on_switch(keys, from, to, &ladder, now));
    }

    /// Starts the close: the window folds into the pill of its piece's slot.
    /// What its body holds goes out first. Does nothing without an open
    /// window. The piece's slot reopens at once; its icon and text fly up
    /// 60 ms later.
    pub fn close(&mut self, anchor: &Anchor, now: Instant) {
        let Some(shown) = self.rect(anchor, now) else {
            return;
        };
        let Some(piece) = self
            .open
            .as_ref()
            .filter(|o| !o.motion.is_closing())
            .map(|o| o.piece)
        else {
            return;
        };
        let flyer = self.take_flyer(piece, now);
        let scroll = self.scroll.offset();
        let Some(o) = self.open.as_mut() else {
            return;
        };
        o.body.flush(now);
        o.motion.close(shown, anchor.pill, now);
        let keys = o.keys.filter(|_| !o.motion.reduced());
        let at = Sheets::target_of(anchor, o.body.size());
        let from = flyer
            .clone()
            .or_else(|| subtitle_parts(&o.body, at, scroll));
        if let (Some(keys), Some(from)) = (keys, from) {
            let to = limited_to(&from, keys);
            self.flights
                .push(Flight::up_on_close(keys, from, to, flyer.is_some(), now));
        }
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
            self.flights.clear();
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
    /// slot. A capsule that jumps (a new placement, a text that changed its
    /// width) is followed on the resize morph; one that slides (`sliding`: the
    /// open piece's slot closing or reopening) is followed exactly, frame by
    /// frame, with no morph of its own.
    pub fn follow(&mut self, anchor: &Anchor, sliding: bool, now: Instant) {
        let Some(o) = self.open.as_mut() else {
            return;
        };
        // The frame the slide ends on still moves the capsule a little.
        if sliding {
            self.slide_until = Some(now + SLIDE_GRACE);
        }
        if o.motion.is_closing() {
            // On its way out the window folds into the live pill.
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
        let pointer = !matches!(ev, InputEvent::Key { .. } | InputEvent::FocusLost);
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

    /// Paints the windows: the one being left (folding into its pill) under
    /// the open one. Each is its Haze at its rect and corner radius, then its
    /// body under one rounded clip of that rect. The body sits at its
    /// window's target, so the rect reveals it rather than carrying it.
    pub fn paint(&mut self, painter: &mut Painter, anchor: &Anchor, now: Instant) {
        let surface = anchor.surface;
        let leaving_hidden: Vec<Vec<Key>> = self
            .leaving
            .iter()
            .map(|l| self.subtitle_hidden(l.piece, now))
            .collect();
        let open_hidden = self
            .open
            .as_ref()
            .map(|o| self.subtitle_hidden(o.piece, now))
            .unwrap_or_default();
        for (l, hidden) in self.leaving.iter_mut().zip(&leaving_hidden) {
            if l.motion.closed(now) {
                continue;
            }
            let rect = l.motion.rect(now, l.pill, surface);
            let radius = l.motion.radius(now);
            if rect.w > 0.0 && rect.h > 0.0 {
                haze_at(painter, rect, radius, l.motion.glass_opacity(now));
                let alpha = l.motion.body_opacity(now);
                if alpha > 0.0 {
                    let at = (l.target.x, l.target.y - l.scroll);
                    paint_body(painter, rect, radius, at, alpha, |p| {
                        l.body.paint(p, now, hidden);
                    });
                }
            }
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
        let radius = o.motion.radius(now);
        haze_at(painter, rect, radius, o.motion.glass_opacity(now));
        let alpha = o.motion.body_opacity(now);
        if alpha > 0.0 {
            let at = Sheets::target_of(anchor, o.body.size());
            paint_body(painter, rect, radius, (at.x, at.y - offset), alpha, |p| {
                o.body.paint(p, now, &open_hidden);
            });
        }
    }

    /// Whether the capsule's changes at `now` are the bar's slide (tests).
    #[cfg(test)]
    #[must_use]
    pub fn follows_slide(&self, now: Instant) -> bool {
        slide_follows(self.slide_until, now)
    }

    /// The pill the window being left folds into (tests).
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
        open || folding || self.is_flying(now)
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
    paint: impl FnOnce(&mut Painter),
) {
    painter.push_clip(view, radius);
    painter.push_state();
    painter.translate(at.0, at.1);
    painter.opacity(alpha);
    paint(painter);
    painter.pop_state();
    painter.pop_clip();
}
