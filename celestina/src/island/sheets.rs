//! The Island's side of the capsule menus (CEL-D-002, CEL-D-009): the
//! open-window owner [`Sheets`] wired to the bar, the page and the input. At
//! most one of three things is shown: nothing, the Island's page, or one
//! capsule window (and, for 260 ms of a switch, the one it closes towards its piece); a
//! closing one still counts as shown, so a request that comes while another
//! thing is shown closes it and waits for it to go (`Sheets::queue`).

use std::time::Instant;

use celestina_paint::{Painter, Rect};
use celestina_ui::Key;
use celestina_wayland::{InputEvent, KeyboardInteractivity, SurfaceCtl};

use super::{
    output_point,
    pages::tray::{index_of, row_key, TrayRow},
    workspaces::rows_for,
    Island, IslandAction, Measure, XKB_KEY_F10, XKB_KEY_MENU,
};
use crate::{
    osd::popup::{Kind, LevelCore},
    pieces::{
        tray as tray_piece, workspaces as workspaces_piece, Gesture, PageId, PieceAction, SheetKind,
    },
    session_menu::SessionCore,
    sheet::{
        geometry::{self, Edge, Hit},
        workspaces::workspaces_view,
        Anchor, BodyOutcome, Next, Request, SheetBody, TrayOverflowBody, WorkspacesBody,
    },
};

/// The Escape key.
pub(super) const XKB_KEY_ESCAPE: u32 = 0xff1b;

impl Island {
    /// Opens the level sheet of `piece` (a node of the bar, the one the
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
        let kind = match core.kind() {
            Kind::Volume => SheetKind::Volume,
            Kind::Brightness => SheetKind::Brightness,
        };
        self.open_sheet_at(piece, kind, SheetBody::Level(Box::new(core)), now)
    }

    /// Opens `body` as the sheet of `kind` on `piece`'s capsule: it grows
    /// from it, switches the sheet already open on that capsule, or waits for
    /// what else is shown to close. `false` when the bar has no such piece.
    pub(super) fn open_sheet_at(
        &mut self,
        piece: Key,
        kind: SheetKind,
        body: SheetBody,
        now: Instant,
    ) -> bool {
        if self.keyboard_claimed {
            tracing::info!("island: a polkit prompt holds the keyboard; no sheet opens");
            return false;
        }
        self.open_what_waited(now);
        let Some(zone) = self.bar.as_ref().and_then(|bar| bar.zone_of(piece)) else {
            return false;
        };
        let Some(anchor) = self.live_anchor(zone, piece, now) else {
            return false;
        };
        let request = Request {
            zone,
            piece,
            kind,
            body,
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
            // A page sheet switched away from takes its state with it.
            self.page_sheet_ends();
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
        if self.sheets.has_level() && self.sheets.is_open() {
            self.close_sheet(now);
        }
        self.sheets.drop_queue();
    }

    /// The keyboard the surface asks for at `now`: `Exclusive` while anything
    /// is shown or waits, `None` at rest.
    pub(super) fn keyboard_interactivity(&self, now: Instant) -> KeyboardInteractivity {
        // A polkit prompt holds the keyboard: nothing here takes it (N-25).
        if self.anything_shown(now) && !self.keyboard_claimed {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::None
        }
    }

    /// Whether the app owes this Island a catcher (`crate::catcher`) at
    /// `now`: exactly while a page or a sheet is shown, closing ones and
    /// what waits included.
    #[must_use]
    pub fn wants_catcher(&self, now: Instant) -> bool {
        self.anything_shown(now) && !self.keyboard_claimed
    }

    /// Whether a polkit prompt holds the keyboard. While it does nothing
    /// opens, the surface asks for no keyboard and the catcher goes.
    pub fn set_keyboard_claimed(&mut self, claimed: bool) {
        self.keyboard_claimed = claimed;
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

    /// The anchor of the window held: its capsule and the piece's pill as the
    /// bar has them, else the last it knew (a piece that vanished: the window
    /// closes towards the last pill it had).
    pub(super) fn sheet_anchor(&self, now: Instant) -> Option<Anchor> {
        let (zone, piece) = self.sheets.anchor_ids()?;
        self.live_anchor(zone, piece, now)
            .or_else(|| self.sheets.last_anchor())
    }

    /// Whether [`Island::open_level_sheet`] would find `piece` on the bar.
    #[must_use]
    pub fn can_open_level_sheet(&self, piece: Key) -> bool {
        let now = Instant::now();
        self.bar
            .as_ref()
            .and_then(|bar| bar.zone_of(piece))
            .and_then(|zone| self.live_anchor(zone, piece, now))
            .is_some()
    }

    /// The anchor the bar gives at `now` for `piece` in `zone`: the capsule
    /// as it is painted (the pieces beside the open one may be sliding), the
    /// edge of it its zone lines up with and the piece's pill. `None` when
    /// either is gone.
    pub(super) fn live_anchor(&self, zone: usize, piece: Key, now: Instant) -> Option<Anchor> {
        let bar = self.bar.as_ref()?;
        Some(Anchor {
            capsule: bar.capsule_extent(zone, now)?,
            edge: Edge::of_zone(zone),
            pill: geometry::pill(bar.pill_of(piece, now)?),
            surface: self.size,
        })
    }

    /// The windows painted at `now` with their corner radii, for the glass
    /// and the regions.
    pub(super) fn sheet_windows(&self, now: Instant) -> Vec<(Rect, f32)> {
        let anchor = self.sheet_anchor(now);
        self.sheets.windows(anchor.as_ref(), self.size, now)
    }

    /// The window's rect at `now`, when one is shown (tests).
    #[cfg(test)]
    pub(super) fn sheet_rect(&self, now: Instant) -> Option<Rect> {
        self.sheets.rect(&self.sheet_anchor(now)?, now)
    }

    /// Starts the sheet's close.
    pub(super) fn close_sheet(&mut self, now: Instant) {
        self.page_sheet_ends();
        if let Some(anchor) = self.sheet_anchor(now) {
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
    /// sheet; the session menu is built here and a page is built and
    /// measured here with `measure`; a level sheet asks the app to build the body.
    pub(super) fn sheet_request(
        &mut self,
        key: Key,
        kind: SheetKind,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Option<IslandAction> {
        if self.sheets.piece() == Some(key) {
            self.close_sheet(now);
            return None;
        }
        let kind = match kind {
            SheetKind::Volume => Kind::Volume,
            SheetKind::Brightness => Kind::Brightness,
            SheetKind::Page(page) => {
                if page == PageId::PHONE && !self.phone_duty {
                    tracing::info!("a phone sheet: the phone duty is off; nothing was done");
                } else {
                    self.open_page_sheet(key, page, now, measure);
                }
                return None;
            }
            SheetKind::Workspaces => {
                self.open_workspaces_sheet(key, now);
                return None;
            }
            SheetKind::TrayOverflow => {
                self.open_tray_overflow_sheet(key, now);
                return None;
            }
            SheetKind::Session => {
                let core = SessionCore::new(&self.theme, self.reduced, now);
                self.open_sheet_at(
                    key,
                    SheetKind::Session,
                    SheetBody::Session(Box::new(core)),
                    now,
                );
                return None;
            }
        };
        Some(IslandAction::OpenLevelSheet { kind, piece: key })
    }

    /// The workspaces list and its body ids, from the model: one row per
    /// workspace of this output. `None` while there is none.
    fn workspaces_list(&self) -> Option<(celestina_ui::View, Vec<u64>)> {
        let model = self.model.as_ref()?;
        let rows = rows_for(model, &self.output);
        if rows.is_empty() {
            return None;
        }
        let view = workspaces_view(&self.theme, &rows, &model.windows);
        Some((view, rows.iter().map(|r| r.id).collect()))
    }

    /// Opens the workspaces list as the sheet of `piece`, through the same
    /// gates as any sheet. `false` when there is nothing to list or the bar
    /// has no such piece.
    fn open_workspaces_sheet(&mut self, piece: Key, now: Instant) -> bool {
        let Some((view, ids)) = self.workspaces_list() else {
            return false;
        };
        let body = WorkspacesBody::new(&self.theme, self.reduced, view, ids, now);
        self.open_sheet_at(
            piece,
            SheetKind::Workspaces,
            SheetBody::Workspaces(Box::new(body)),
            now,
        )
    }

    /// Builds the open workspaces list again (the model changed): the focus
    /// and hover stay, the sheet follows a new height on its resize motion.
    pub(super) fn apply_workspaces_sheet(&mut self, now: Instant) {
        if self.sheets.workspaces_body_mut().is_none() {
            return;
        }
        let Some((view, ids)) = self.workspaces_list() else {
            // The output has no workspace left: nothing to list.
            self.sheets.drop_queue();
            self.close_sheet(now);
            return;
        };
        if let Some(body) = self.sheets.workspaces_body_mut() {
            body.apply(view, ids, now);
        }
    }

    /// The apps the bar hides, with their place in the tray's list: what
    /// the overflow window lists.
    pub(super) fn overflow_rows(&self) -> Vec<(usize, &TrayRow)> {
        let hidden = self
            .bar
            .as_ref()
            .map_or(&[][..], |bar| bar.layout().tray_hidden.as_slice());
        tray_piece::hidden_rows(&self.tray_rows, hidden).collect()
    }

    /// The hidden apps the overflow list shows, with their place in the
    /// tray's list. `None` while none is hidden.
    fn overflow_list(&self) -> Option<Vec<(usize, TrayRow)>> {
        let rows: Vec<(usize, TrayRow)> = self
            .overflow_rows()
            .into_iter()
            .map(|(index, row)| (index, row.clone()))
            .collect();
        (!rows.is_empty()).then_some(rows)
    }

    /// Opens the hidden apps' list as the sheet of `piece` (the chevron),
    /// through the same gates as any sheet. `false` when no app is hidden or
    /// the bar has no such piece.
    fn open_tray_overflow_sheet(&mut self, piece: Key, now: Instant) -> bool {
        let Some(rows) = self.overflow_list() else {
            return false;
        };
        let body = TrayOverflowBody::new(&self.theme, self.reduced, rows, now);
        self.open_sheet_at(
            piece,
            SheetKind::TrayOverflow,
            SheetBody::TrayOverflow(Box::new(body)),
            now,
        )
    }

    /// Builds the open overflow list again (the hidden list or the
    /// registered apps changed): the focus and hover stay and the sheet
    /// follows a new height; with no hidden app left it closes.
    pub(super) fn apply_tray_overflow_sheet(&mut self, now: Instant) {
        if self.sheets.tray_overflow_body_mut().is_none() {
            return;
        }
        let Some(rows) = self.overflow_list() else {
            // The last hidden app left (shown again, or it unregistered): the
            // chevron goes with it.
            self.sheets.drop_queue();
            self.close_sheet(now);
            return;
        };
        if let Some(body) = self.sheets.tray_overflow_body_mut() {
            body.apply(rows, now);
        }
    }

    /// The index in the tray's list of the hidden app whose window row is
    /// `key`.
    fn overflow_index(&self, key: Key) -> Option<usize> {
        let index = index_of(key, self.tray_rows.len())?;
        self.overflow_rows()
            .iter()
            .any(|&(i, _)| i == index)
            .then_some(index)
    }

    /// What `gesture` on the window row `key` asks, as on the app's bar
    /// icon: a click or Enter activates (the window closes on the choice), a
    /// middle click activates secondarily, the app's menu hangs from the row
    /// (the window stays under it) and `keyboard` places it at the row.
    fn overflow_row_gesture(
        &mut self,
        key: Key,
        gesture: Gesture,
        keyboard: bool,
        now: Instant,
    ) -> Vec<IslandAction> {
        let Some(index) = self.overflow_index(key) else {
            return Vec::new();
        };
        let Some(row) = self.tray_rows.get(index).cloned() else {
            return Vec::new();
        };
        let rect = self
            .sheet_anchor(now)
            .and_then(|anchor| self.sheets.body_rect(row_key(index), &anchor));
        let at = |rect: Option<Rect>, pointer: (f32, f32)| {
            output_point(match (keyboard, rect) {
                (true, Some(r)) => (r.x + r.w / 2.0, r.y + r.h / 2.0),
                _ => pointer,
            })
        };
        match tray_piece::row_action(&row, gesture) {
            PieceAction::Tray(command) => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                vec![IslandAction::Tray(command)]
            }
            PieceAction::TrayMenu(item) => {
                let Some(row) = rect else {
                    return Vec::new();
                };
                self.bar_menu_asked = true;
                self.overflow_menu_asked = Some(item.clone());
                vec![IslandAction::OpenTrayMenu {
                    item,
                    row,
                    keyboard,
                    at: at(rect, self.pointer),
                }]
            }
            PieceAction::TrayContext(item) => {
                let (x, y) = at(rect, self.pointer);
                vec![IslandAction::Tray(crate::tray::TrayCommand::ContextMenu(
                    item, x, y,
                ))]
            }
            _ => Vec::new(),
        }
    }

    /// The hidden app whose window row holds (`x`, `y`).
    fn overflow_row_at(&self, anchor: &Anchor, x: f32, y: f32) -> Option<Key> {
        self.overflow_rows().into_iter().find_map(|(index, _)| {
            let key = row_key(index);
            self.sheets
                .body_rect(key, anchor)
                .filter(|r| r.contains(x, y))
                .map(|_| key)
        })
    }

    /// What the tray window's rows do with the buttons and the wheel the body
    /// does not know: a middle click activates secondarily, a right click
    /// asks for the menu, the wheel scrolls the app (unless the list is taller
    /// than its window, which then scrolls). `None` for everything else,
    /// which is the body's.
    fn overflow_pointer(
        &mut self,
        ev: &InputEvent,
        anchor: &Anchor,
        now: Instant,
    ) -> Option<Vec<IslandAction>> {
        if self.sheets.kind() != Some(SheetKind::TrayOverflow) {
            return None;
        }
        match *ev {
            InputEvent::PointerRelease { x, y, button, .. }
                if button == super::BTN_MIDDLE || button == super::BTN_RIGHT =>
            {
                let key = self.overflow_row_at(anchor, x, y)?;
                let gesture = if button == super::BTN_MIDDLE {
                    Gesture::Middle
                } else {
                    Gesture::Context
                };
                Some(self.overflow_row_gesture(key, gesture, false, now))
            }
            InputEvent::Axis {
                x,
                y,
                horizontal,
                vertical,
            } if !self.sheets.body_overflows(anchor) => {
                let key = self.overflow_row_at(anchor, x, y)?;
                let id = self.tray_rows.get(self.overflow_index(key)?)?.id.clone();
                Some(self.scroll_item(key, id, horizontal, vertical))
            }
            _ => None,
        }
    }

    /// The Menu key (or Shift+F10) on the focused row of the tray window:
    /// the app's menu, placed at the row.
    fn overflow_menu_key(&mut self, now: Instant) -> Vec<IslandAction> {
        match self.sheets.body_focused_key() {
            Some(key) => self.overflow_row_gesture(key, Gesture::Context, true, now),
            None => Vec::new(),
        }
    }

    /// The frame's first part for the sheets: opens what was waiting for the
    /// close to end, and syncs the body (readings, a new theme, a resize) with
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
        if self.keyboard_claimed {
            return;
        }
        if request.kind == SheetKind::Page(PageId::PHONE) && !self.phone_duty {
            tracing::info!("island: a waiting phone sheet; the phone duty is off; not opened");
            return;
        }
        let Some(anchor) = self.live_anchor(request.zone, request.piece, now) else {
            tracing::debug!("island: a waiting sheet's piece is gone; not opened");
            return;
        };
        // What the page read while the request waited was not watched, and a
        // workspaces list built at the click may be out of date.
        if matches!(
            request.kind,
            SheetKind::Page(_) | SheetKind::Workspaces | SheetKind::TrayOverflow
        ) {
            self.stale = true;
        }
        self.sheets
            .open(request, &anchor, &self.theme, now, self.reduced);
    }

    /// The frame's part after the layout: the window follows its capsule and
    /// piece, and closes on its exit motion when either is gone; the window
    /// being left closes towards its piece's pill as the bar has it.
    pub(super) fn sheets_after_layout(&mut self, now: Instant) {
        for piece in self.sheets.leaving_pieces(now) {
            if let Some(pill) = self.bar.as_ref().and_then(|bar| bar.pill_of(piece, now)) {
                self.sheets.follow_leaving(piece, geometry::pill(pill));
            }
        }
        let Some((zone, piece)) = self.sheets.anchor_ids() else {
            return;
        };
        match self.live_anchor(zone, piece, now) {
            Some(anchor) => {
                let sliding = self
                    .bar
                    .as_ref()
                    .is_some_and(|bar| bar.is_sliding(zone, now));
                self.sheets.follow(&anchor, sliding, now);
            }
            None if self.sheets.is_open() => {
                tracing::debug!(output = %self.output, "island: the sheet's piece left the bar; closing");
                self.page_sheet_ends();
                if let Some(anchor) = self.sheets.last_anchor() {
                    self.sheets.close(&anchor, now);
                }
            }
            None => {}
        }
    }

    /// Paints the windows over the bar, the Island's capsule and its page:
    /// each one's glass and body.
    pub(super) fn paint_sheet(&mut self, painter: &mut Painter, now: Instant) {
        let Some(anchor) = self.sheet_anchor(now) else {
            return;
        };
        self.sheets.paint(painter, &anchor, now);
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

    /// Input while a window is open. `Some` when the window took the event;
    /// `None` to let it reach what is outside it: the bar (its other pieces,
    /// another capsule, the glass) and the Island's capsule. The window is
    /// hit first, but it never covers the bar, so a press on the bar reaches
    /// it and its switch or close-then-open rules.
    pub(super) fn sheet_input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Option<Vec<IslandAction>> {
        let anchor = self.sheet_anchor(now)?;
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
            } if keysym == XKB_KEY_ESCAPE && !self.sheets.body_takes_escape() => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                Some(Vec::new())
            }
            InputEvent::Key {
                keysym,
                modifiers,
                pressed: true,
                ..
            } if self.sheets.kind() == Some(SheetKind::TrayOverflow)
                && (keysym == XKB_KEY_MENU || (keysym == XKB_KEY_F10 && modifiers.shift)) =>
            {
                Some(self.overflow_menu_key(now))
            }
            InputEvent::Key {
                keysym, pressed, ..
            } => {
                if Island::is_delete(keysym) {
                    if !pressed {
                        self.delete_held = false;
                    } else if self.sheets.page() == Some(PageId::CLIPBOARD) {
                        // One press removes one entry; a held key's repeat
                        // needs a release in between.
                        let held = std::mem::replace(&mut self.delete_held, true);
                        return Some(if held {
                            Vec::new()
                        } else {
                            self.delete_key().into_iter().collect()
                        });
                    }
                }
                let outcome = self.sheets.input(ev, &anchor, now, measure);
                Some(self.apply_outcome(outcome, now))
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
                    return Some(self.apply_outcome(outcome, now));
                }
                let gesture = matches!(
                    ev,
                    InputEvent::Axis { .. }
                        | InputEvent::PointerPress { .. }
                        | InputEvent::PointerRelease { .. }
                );
                // Whatever it lands on, a release ends a press on the
                // subtitle.
                let pressed = match ev {
                    InputEvent::PointerRelease { .. } => self.subtitle_press.take(),
                    _ => None,
                };
                if gesture && self.sheets.on_subtitle(&anchor, now, x, y) {
                    if let Some(out) = self.subtitle_gesture(ev, pressed, now, measure) {
                        self.pointer_is_the_sheets(now);
                        return Some(out);
                    }
                }
                match geometry::hit(rect, x, y) {
                    Hit::Body => {
                        self.pointer_is_the_sheets(now);
                        if let Some(actions) = self.overflow_pointer(ev, &anchor, now) {
                            return Some(actions);
                        }
                        let outcome = self.sheets.input(ev, &anchor, now, measure);
                        Some(self.apply_outcome(outcome, now))
                    }
                    Hit::Outside => {
                        self.sheets.leave_body(now, measure);
                        // A window still painted as it closes in a switch
                        // takes nothing and closes nothing.
                        if press && self.sheets.on_leaving(anchor.surface, now, x, y) {
                            return Some(Vec::new());
                        }
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

    /// A press, release or wheel turn on the subtitle of the open sheet: it
    /// answers as the piece the sheet is open on, through the very gestures
    /// and actions the bar's input uses (the wheel steps the workspaces or the
    /// level, a right click mutes, a plain click closes the sheet). A press
    /// is only the start of a click: a release makes one only when the press
    /// (`pressed`, its button) began on the subtitle. A gesture the piece has
    /// no action for does nothing and closes nothing. The subtitle shows no
    /// hover or press plate of its own. `None` hands the event on to the
    /// body: a wheel over the subtitle of a piece without a wheel action
    /// scrolls the body, and a release whose press began elsewhere is the
    /// body's.
    fn subtitle_gesture(
        &mut self,
        ev: &InputEvent,
        pressed: Option<u32>,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Option<Vec<IslandAction>> {
        let piece = self.sheets.piece()?;
        match *ev {
            InputEvent::PointerPress { button, .. } => {
                self.subtitle_press = Some(button);
                return Some(Vec::new());
            }
            InputEvent::PointerRelease { button, .. } if pressed != Some(button) => return None,
            InputEvent::Axis { .. } => {
                let wheel = piece == workspaces_piece::key()
                    || self
                        .with_bar(|bar, cx| bar.takes_wheel(piece, cx))
                        .unwrap_or(false);
                if !wheel {
                    return None;
                }
            }
            _ => {}
        }
        let asked = self
            .with_bar(|bar, cx| bar.ask_for(piece, ev, cx))
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, gesture, action)| {
                matches!(gesture, Gesture::Scroll(..)) || *action != PieceAction::None
            })
            .collect();
        Some(self.apply_asked(asked, false, now, measure))
    }

    /// The pointer is the window's: the bar and the Island's capsule end
    /// their hover.
    fn pointer_is_the_sheets(&mut self, now: Instant) {
        if let Some(tree) = self.bar.as_mut() {
            tree.leave(now);
        }
        self.capsule_input
            .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
    }

    /// What a body asked after an event: Escape in it closes the sheet; a
    /// session choice closes it too and is the one action that comes out; a
    /// page's node is read as the page reads it, and its overflow button asks
    /// for the menu.
    fn apply_outcome(&mut self, outcome: BodyOutcome, now: Instant) -> Vec<IslandAction> {
        match outcome {
            BodyOutcome::Keep { .. } => Vec::new(),
            BodyOutcome::Close => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                Vec::new()
            }
            BodyOutcome::Session(action) => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                vec![IslandAction::Session(action)]
            }
            BodyOutcome::FocusWorkspace(id) => {
                self.sheets.drop_queue();
                self.close_sheet(now);
                vec![IslandAction::FocusWorkspace(id)]
            }
            BodyOutcome::Activate { key, keyboard }
                if self.sheets.kind() == Some(SheetKind::TrayOverflow) =>
            {
                self.overflow_row_gesture(key, Gesture::Primary, keyboard, now)
            }
            BodyOutcome::Activate { key, keyboard: _ } => {
                let Some(page) = self.sheets.page() else {
                    return Vec::new();
                };
                self.page_action(page, key, now).into_iter().collect()
            }
            BodyOutcome::More { rect, keyboard } => {
                vec![IslandAction::OpenMenu {
                    more: rect,
                    keyboard,
                }]
            }
        }
    }
}
