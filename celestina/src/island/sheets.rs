//! The Island's side of the capsule sheets (CEL-D-002): the open-sheet owner
//! [`Sheets`] wired to the bar, the page and the input. At most one of three
//! things is shown: nothing, the Island's page, or one capsule sheet; a
//! closing one still counts as shown, so a request that comes while another
//! thing is shown closes it and waits for it to go (`Sheets::queue`).

use std::time::Instant;

use celestina_paint::{Painter, Rect};
use celestina_ui::Key;
use celestina_wayland::{InputEvent, SurfaceCtl};

use super::{Island, IslandAction, Measure};
use crate::{
    osd::popup::{Kind, LevelCore},
    pieces::SheetKind,
    sheet::{
        geometry::{self, Hit},
        Anchor, BodyOutcome, Next, Request, SheetBody,
    },
};

/// The Escape key.
const XKB_KEY_ESCAPE: u32 = 0xff1b;

impl Island {
    /// Opens the volume sheet of `piece` (a node of the bar, the one the
    /// [`IslandAction::OpenLevelSheet`] named) with `core` as its body: it
    /// grows from the piece's capsule, or switches the sheet already open on
    /// that capsule. Anything else shown closes first and the sheet opens once
    /// it is gone. `false` when the bar has no such piece.
    pub fn open_level_sheet(
        &mut self,
        piece: Key,
        core: LevelCore,
        ctl: &mut SurfaceCtl<'_>,
    ) -> bool {
        let now = Instant::now();
        let opened = self.open_level_sheet_at(piece, core, now);
        if opened {
            self.sync_ctl(ctl, now);
            ctl.request_redraw();
        }
        opened
    }

    pub(super) fn open_level_sheet_at(
        &mut self,
        piece: Key,
        core: LevelCore,
        now: Instant,
    ) -> bool {
        // What waited for a close that has ended opens first: a request now
        // must not slip past it.
        self.open_what_waited(now);
        let Some(zone) = self.bar.as_ref().and_then(|bar| bar.zone_of(piece)) else {
            return false;
        };
        let Some(anchor) = self.live_anchor(zone, piece) else {
            return false;
        };
        let request = Request {
            zone,
            piece,
            kind: SheetKind::Volume,
            body: SheetBody::Level(Box::new(core)),
        };
        let page_shown = self.state.is_open() || self.state.progress(now) > 0.0;
        let switchable = self.sheets.is_open() && self.sheets.zone(now) == Some(zone);
        if page_shown || (self.sheets.is_shown(now) && !switchable) {
            // One thing at a time: what is shown closes, the sheet waits.
            if self.state.is_open() {
                self.close(now);
            }
            self.close_sheet(now);
            self.sheets.queue(Next::Sheet(request));
        } else {
            self.sheets
                .open(request, &anchor, &self.theme, now, self.reduced);
        }
        true
    }

    /// Whether a level sheet is open or waits to open.
    #[must_use]
    pub fn level_sheet_open(&self) -> bool {
        self.sheets.has_level()
    }

    /// Closes the level sheet from the Island's side (the duty went off, the
    /// person chose another output): what is open closes on its exit motion
    /// and what waited is given up.
    pub fn close_level_sheet(&mut self, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        if self.sheets.has_level() {
            self.close_level_sheet_at(now);
            self.sync_ctl(ctl, now);
            ctl.request_redraw();
        }
    }

    /// Closes the level sheet without a surface (the output is gone): the
    /// body is flushed by the close, so what the throttle held is sent
    /// before the Island is dropped.
    pub fn close_level_sheet_now(&mut self) {
        if self.sheets.has_level() {
            self.close_level_sheet_at(Instant::now());
        }
    }

    pub(super) fn close_level_sheet_at(&mut self, now: Instant) {
        if matches!(self.sheets.kind(), Some(SheetKind::Volume)) && self.sheets.is_open() {
            self.close_sheet(now);
        }
        self.sheets.drop_queue();
    }

    /// Whether anything of the Island is open: its page, a sheet, or what
    /// waits to open.
    #[must_use]
    pub fn anything_open(&self) -> bool {
        self.state.is_open() || self.sheets.is_open() || self.sheets.has_queued()
    }

    /// The rect of the bar's node `key`, for what is anchored to it.
    #[must_use]
    pub fn piece_rect(&self, key: Key) -> Option<Rect> {
        self.bar.as_ref().and_then(|bar| bar.node_rect(key))
    }

    /// The anchor of the sheet held: its capsule and piece as the bar has
    /// them, else the last it knew (a piece that vanished).
    pub(super) fn sheet_anchor(&self) -> Option<Anchor> {
        let (zone, piece) = self.sheets.anchor_ids()?;
        self.live_anchor(zone, piece)
            .or_else(|| self.sheets.last_anchor())
    }

    /// Whether [`Island::open_level_sheet`] would find `piece` on the bar.
    #[must_use]
    pub fn can_open_level_sheet(&self, piece: Key) -> bool {
        self.bar
            .as_ref()
            .and_then(|bar| bar.zone_of(piece))
            .and_then(|zone| self.live_anchor(zone, piece))
            .is_some()
    }

    /// The anchor the bar gives now for `piece` in `zone`: `None` when either
    /// is gone.
    pub(super) fn live_anchor(&self, zone: usize, piece: Key) -> Option<Anchor> {
        let bar = self.bar.as_ref()?;
        Some(Anchor {
            capsule: bar.zone_rect(zone)?,
            piece: bar.piece_rect(piece)?,
            surface: self.size,
        })
    }

    /// The sheet's rect at `now`, when one is shown.
    pub(super) fn sheet_rect(&self, now: Instant) -> Option<Rect> {
        self.sheets.rect(&self.sheet_anchor()?, now)
    }

    /// Starts the sheet's close.
    pub(super) fn close_sheet(&mut self, now: Instant) {
        if let Some(anchor) = self.sheet_anchor() {
            self.sheets.close(&anchor, now);
        }
    }

    /// Closes whatever is open, and gives up what waited.
    pub(super) fn close_everything(&mut self, now: Instant) {
        self.sheets.drop_queue();
        self.close_sheet(now);
        self.close(now);
    }

    /// What a piece's sheet request does: the open piece again closes its
    /// sheet; any other asks the app to build the body.
    pub(super) fn sheet_request(
        &mut self,
        key: Key,
        kind: SheetKind,
        now: Instant,
    ) -> Option<IslandAction> {
        if self.sheets.piece() == Some(key) {
            self.close_sheet(now);
            return None;
        }
        let kind = match kind {
            SheetKind::Volume => Kind::Volume,
        };
        Some(IslandAction::OpenLevelSheet { kind, piece: key })
    }

    /// The frame's first part for the sheets: opens what was waiting for the
    /// close to end, and syncs the body (readings, a new theme, a fold) with
    /// the frame's own `measure`.
    pub(super) fn sheets_before_layout(&mut self, now: Instant, measure: &mut Measure<'_>) {
        self.open_what_waited(now);
        self.sheets.sync(&self.theme, now, measure);
    }

    /// Opens what waited for the close to end, if that has ended.
    fn open_what_waited(&mut self, now: Instant) {
        let closed = self.state.progress(now) <= 0.0;
        match self.sheets.take_due(now, closed) {
            Some(Next::Page(page)) => self.open(page, now),
            Some(Next::Sheet(request)) => self.open_request(request, now),
            None => {}
        }
    }

    /// Opens a request that waited, if its piece is still on the bar.
    fn open_request(&mut self, request: Request, now: Instant) {
        let Some(anchor) = self.live_anchor(request.zone, request.piece) else {
            tracing::debug!("island: a waiting sheet's piece is gone; not opened");
            return;
        };
        self.sheets
            .open(request, &anchor, &self.theme, now, self.reduced);
    }

    /// The frame's part after the layout: the sheet follows its capsule and
    /// piece, and closes on its exit motion when either is gone.
    pub(super) fn sheets_after_layout(&mut self, now: Instant) {
        let Some((zone, piece)) = self.sheets.anchor_ids() else {
            return;
        };
        match self.live_anchor(zone, piece) {
            Some(anchor) => self.sheets.follow(&anchor, now),
            None if self.sheets.is_open() => {
                tracing::debug!(output = %self.output, "island: the sheet's piece left the bar; closing");
                if let Some(anchor) = self.sheets.last_anchor() {
                    self.sheets.close(&anchor, now);
                }
            }
            None => {}
        }
    }

    /// Paints the sheet over the Island's capsule and page: its glass, its
    /// row (the capsule's pieces) and its body.
    pub(super) fn paint_sheet(&mut self, painter: &mut Painter, now: Instant) {
        let Some(anchor) = self.sheet_anchor() else {
            return;
        };
        let Some((zone, _)) = self.sheets.anchor_ids() else {
            return;
        };
        let mut bar = self.bar.as_mut();
        let mut row = |p: &mut Painter| {
            if let Some(bar) = bar.as_deref_mut() {
                bar.paint_row(p, now, zone);
            }
        };
        self.sheets.paint(painter, &anchor, now, &mut row);
    }

    /// Escape and the keyboard leaving drop what waited behind a closing
    /// thing, even when nothing is open to receive them.
    pub(super) fn sheet_queue_input(&mut self, ev: &InputEvent, _now: Instant) {
        if !self.sheets.has_queued() {
            return;
        }
        let drops = match *ev {
            InputEvent::FocusLost => true,
            InputEvent::Key {
                keysym, pressed, ..
            } => keysym == XKB_KEY_ESCAPE && pressed,
            _ => false,
        };
        if drops {
            self.sheets.drop_queue();
        }
    }

    /// Input while a sheet is open. `Some` when the sheet took the event;
    /// `None` to let it reach what is outside the sheet (another capsule,
    /// the Island's capsule). The sheet is hit before the bar and the capsule
    /// it covers.
    pub(super) fn sheet_input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Option<Vec<IslandAction>> {
        let anchor = self.sheet_anchor()?;
        let rect = self.sheets.rect(&anchor, now)?;
        match *ev {
            InputEvent::FocusLost => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                Some(Vec::new())
            }
            InputEvent::Key {
                keysym,
                pressed: true,
                ..
            } if keysym == XKB_KEY_ESCAPE => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                Some(Vec::new())
            }
            InputEvent::Key { .. } => {
                let outcome = self.sheets.input(ev, &anchor, now, measure);
                self.apply_outcome(outcome, now);
                Some(Vec::new())
            }
            InputEvent::PointerLeave => {
                self.sheets.leave_body(now, measure);
                None
            }
            InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. }
            | InputEvent::Axis { x, y, .. } => {
                let press = matches!(ev, InputEvent::PointerPress { .. });
                // A drag that began on the body keeps the pointer, wherever
                // it goes.
                if !press && self.sheets.body_holds_pointer() {
                    let outcome = self.sheets.input(ev, &anchor, now, measure);
                    self.apply_outcome(outcome, now);
                    return Some(Vec::new());
                }
                match geometry::hit(rect, x, y) {
                    Hit::Body => {
                        self.pointer_is_the_sheets(true, now);
                        let outcome = self.sheets.input(ev, &anchor, now, measure);
                        self.apply_outcome(outcome, now);
                        Some(Vec::new())
                    }
                    Hit::Row => {
                        self.sheets.leave_body(now, measure);
                        let on_capsule = anchor.capsule.contains(x, y);
                        self.pointer_is_the_sheets(!on_capsule, now);
                        if on_capsule {
                            Some(self.bar_input(ev, false, false, now))
                        } else {
                            Some(Vec::new())
                        }
                    }
                    Hit::Outside => {
                        self.sheets.leave_body(now, measure);
                        if press {
                            let on_bar = self.on_bar(x, y, now);
                            let on_island = self.state.rect(now, self.size.0).contains(x, y);
                            if !on_bar && !on_island {
                                self.sheets.drop_queue();
                                self.close_sheet(now);
                                return Some(Vec::new());
                            }
                        }
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// The pointer is the sheet's: the Island's capsule ends its hover, and
    /// so does the bar when `bar` (a pointer on the sheet's row over its own
    /// capsule is the bar's to hover).
    fn pointer_is_the_sheets(&mut self, bar: bool, now: Instant) {
        if bar {
            if let Some(tree) = self.bar.as_mut() {
                tree.leave(now);
            }
        }
        self.capsule_input
            .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
    }

    /// What a body asked after an event: Escape in it closes the sheet.
    fn apply_outcome(&mut self, outcome: BodyOutcome, now: Instant) {
        if outcome == BodyOutcome::Close {
            self.sheets.drop_queue();
            self.close_sheet(now);
        }
    }
}
