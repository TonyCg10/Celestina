//! Page-backed capsule sheets (CEL-D-003): a bar piece that opened an Island
//! page opens the page's content under the piece's subtitle (its icon and text, with
//! the overflow button at its right) in a sheet that grows from its capsule. The Island
//! keeps its pages for `island open`, its tabs and the notification paths; a
//! page sheet does not share a container with them. Both read one
//! [`PageCx`], and what a node of either does is [`Island::page_action`]:
//! the page's, whichever container shows it.

use std::time::Instant;

use celestina_shell::connectivity::Action;
use celestina_ui::{Interaction, Key, Tree, View};

use super::{
    clock, pages::tabs, view, Island, IslandAction, Measure, PageCx, XKB_KEY_DELETE,
    XKB_KEY_KP_DELETE,
};
use crate::{
    island::pages::{
        activity, bluetooth as bluetooth_page, clock as clock_page, media as media_page,
        network as network_page, notifications, page_view, phone as phone_page,
        power as power_page, sysmon as sysmon_page,
    },
    pieces::{Face, PageId, SheetKind},
    sheet::PageBody,
};

impl Island {
    /// What the pages read, from what the Island knows. The Island's page
    /// and a page sheet are built from the same one, so the `action_for`
    /// helpers find the same keys in both.
    pub(super) fn page_cx(&self) -> PageCx<'_> {
        PageCx {
            content_w: None,
            clipboard: &self.clip_panel,
            theme: &self.theme,
            max_size: self.state.max_page_size(),
            model: self.model.as_ref(),
            output: &self.output,
            owners: &self.owners,
            do_not_disturb: self.do_not_disturb,
            history: &self.history,
            items: &self.tray_rows,
            sysmon: self.page_sysmon.as_ref(),
            weather: self.weather.as_ref(),
            today: self.today,
            month_offset: self.month_offset,
            utc_offset: clock::utc_offset(),
            connectivity: &self.connectivity,
            phone: &self.phone,
            media: &self.media,
        }
    }

    /// The page on screen (or growing): the Island's, else the open page
    /// sheet's.
    pub(super) fn page_shown(&self) -> Option<PageId> {
        if self.state.is_open() {
            Some(self.state.shown_page())
        } else {
            self.sheets.page()
        }
    }

    /// The verb the notifications toggle asks of the Island: close whatever
    /// shows the Notifications page (the Island's page or a sheet), else open it.
    #[must_use]
    pub fn notifications_toggle_verb(&self) -> crate::command::Verb {
        crate::notifications::toggle_verb(self.page_shown())
    }

    /// The tree and the input state of the page shown.
    pub(super) fn shown_page_input(&mut self) -> Option<(&mut Tree, &mut Interaction)> {
        if self.state.is_open() {
            Some((&mut self.page, &mut self.page_input))
        } else {
            self.sheets.page_body_mut().map(PageBody::parts)
        }
    }

    /// The face of `piece`, the bar node that opened the sheet of `page`:
    /// what the piece shows, else the page's icon alone (a piece that left
    /// the bar, or one that gives none).
    fn sheet_face(&self, piece: Key, page: PageId) -> Face {
        self.bar
            .as_ref()
            .and_then(|bar| bar.piece_id_of(piece))
            .and_then(crate::pieces::piece)
            .and_then(|p| p.face(&self.bar_cx()))
            .or_else(|| Face::of_page(page, None))
            .unwrap_or(Face {
                icon: "app-window",
                text: None,
            })
    }

    /// The natural size of the content of `page` as a sheet: its view laid
    /// out on its own (every text measured with the frame's `measure`), as
    /// wide as its widest row and as tall as its rows, whatever size the
    /// Island's page asks for.
    pub(super) fn page_content_size(
        &self,
        page: PageId,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> (f32, f32) {
        let shown = page_view(page);
        let content = shown.view(&self.sheet_cx(page));
        let key = content.key;
        let mut tree = Tree::new(true, &self.theme);
        tree.apply(content, now);
        // Room enough for any page: the root takes its content's size.
        tree.layout_measured((view::PAGE_SHEET_W, 1.0e5), |text, style, width| {
            measure(text, style, width)
        });
        let (w, h) = tree.node_by_key(key).map_or((0.0, 0.0), |id| {
            let r = tree.rect(id);
            (r.w, r.h)
        });
        // A page with a declared width is that wide whatever its rows are.
        (self.declared_content_w(page).unwrap_or(w), h)
    }

    /// What the pages read as a sheet: the Island's, with the content width
    /// of the sheet a page declares ([`PageView::sheet_width`]).
    fn sheet_cx(&self, page: PageId) -> PageCx<'_> {
        PageCx {
            content_w: self.declared_content_w(page),
            ..self.page_cx()
        }
    }

    /// The content width of the sheet `page` declares: its width less the
    /// side padding.
    fn declared_content_w(&self, page: PageId) -> Option<f32> {
        page_view(page)
            .sheet_width()
            .map(|w| (w - 2.0 * self.theme.spacing.lg).max(0.0))
    }

    /// The body view and size of `page` as a sheet of `piece`, for a page
    /// content of natural size `content`: the piece's subtitle over the
    /// content in a box as big as the content.
    pub(super) fn page_sheet_view(
        &self,
        piece: Key,
        page: PageId,
        content: (f32, f32),
    ) -> (View, (f32, f32)) {
        let shown = page_view(page);
        let face = self.sheet_face(piece, page);
        let body = view::page_sheet(
            &self.theme,
            &face,
            shown.title(),
            shown.view(&self.sheet_cx(page)),
            content,
        );
        (body, view::page_sheet_size(&self.theme, content))
    }

    /// Opens `page` as the sheet of `piece`, as big as its content measures
    /// with `measure`. `false` when the bar has no such piece.
    pub(super) fn open_page_sheet(
        &mut self,
        piece: Key,
        page: PageId,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> bool {
        let content = self.page_content_size(page, now, measure);
        self.open_page_sheet_sized(piece, page, content, now)
    }

    /// [`Island::open_page_sheet`] for a content of size `content` already
    /// measured.
    fn open_page_sheet_sized(
        &mut self,
        piece: Key,
        page: PageId,
        content: (f32, f32),
        now: Instant,
    ) -> bool {
        // A page that opens starts fresh, as a page opened on the Island.
        self.forget_page_state();
        let (body, size) = self.page_sheet_view(piece, page, content);
        let body = PageBody::new(page, &self.theme, self.reduced, body, size, content, now);
        self.open_sheet_at(
            piece,
            SheetKind::Page(page),
            crate::sheet::SheetBody::Page(Box::new(body)),
            now,
        )
    }

    /// The views were built again: the open page sheet is measured and built
    /// again at the frame's next step ([`Island::refit_page_sheet`]).
    pub(super) fn apply_page_sheet(&mut self) {
        self.page_sheet_stale = true;
    }

    /// Builds the open page sheet's body again when what the page reads
    /// changed: the focus and hover stay, and the sheet follows the size the
    /// new content measures (a device appearing makes it taller).
    pub(super) fn refit_page_sheet(&mut self, now: Instant, measure: &mut Measure<'_>) {
        if !std::mem::take(&mut self.page_sheet_stale) {
            return;
        }
        let (Some(page), Some((_, piece))) = (self.sheets.page(), self.sheets.anchor_ids()) else {
            return;
        };
        let content = self.page_content_size(page, now, measure);
        let (body, size) = self.page_sheet_view(piece, page, content);
        if let Some(open) = self.sheets.page_body_mut() {
            open.apply(body, size, content, now);
        }
        self.take_pending_focus();
    }

    /// What a page keeps while it is shown, given up when it is not: the
    /// month the Clock page moved to, a clear awaiting its confirmation, the
    /// Delete key held.
    pub(super) fn forget_page_state(&mut self) {
        if self.month_offset != 0 {
            self.month_offset = 0;
            self.stale = true;
        }
        self.forget_confirmation();
        self.delete_held = false;
    }

    /// The page sheet is going away (closing, replaced or gone): its state
    /// goes with it.
    pub(super) fn page_sheet_ends(&mut self) {
        if self.sheets.page().is_some() {
            self.forget_page_state();
        }
    }

    /// The phone duty went off: a page that no longer exists is not left
    /// open, on the Island or as a sheet.
    pub(super) fn phone_duty_off(&mut self, now: Instant) {
        if self.state.is_open() && self.state.shown_page() == PageId::PHONE {
            self.state.switch(PageId::ACTIVITY, now, self.reduced);
        }
        if self.sheets.page() == Some(PageId::PHONE) {
            self.sheets.drop_queue();
            self.close_sheet(now);
        } else if self.sheets.queued_page_sheet() == Some(PageId::PHONE) {
            self.sheets.drop_queue();
        }
    }

    /// The menu's reopen row, with a page sheet open: it closes at once and
    /// grows again from its capsule. `false` when no page sheet is open.
    pub(super) fn reopen_sheet(&mut self, now: Instant) -> bool {
        let (Some(piece), Some(page)) = (self.sheets.piece(), self.sheets.page()) else {
            return false;
        };
        // It grows again at the size it had: no text is measured here.
        let Some(content) = self.sheets.page_body_mut().map(|body| body.content()) else {
            return false;
        };
        self.forget_page_state();
        self.sheets.drop_open(now);
        self.open_page_sheet_sized(piece, page, content, now);
        true
    }

    /// Whether Escape belongs to the overflow menu of a page sheet (it
    /// closes first, the sheet stays).
    pub(super) fn menu_takes_escape(&self, ev: &celestina_wayland::InputEvent) -> bool {
        self.menu.is_some()
            && self.sheets.page().is_some()
            && matches!(
                *ev,
                celestina_wayland::InputEvent::Key {
                    keysym: super::sheets::XKB_KEY_ESCAPE,
                    pressed: true,
                    ..
                }
            )
    }

    /// Whether `keysym` is a Delete key (the keypad's too).
    pub(super) fn is_delete(keysym: u32) -> bool {
        keysym == XKB_KEY_DELETE || keysym == XKB_KEY_KP_DELETE
    }

    /// What a click or Enter on the node `key` does on the Island.
    pub(super) fn activate(&mut self, key: Key, now: Instant) -> Option<IslandAction> {
        match self.activate_chrome(key, now) {
            Some(handled) => handled,
            None => self.page_action(self.state.shown_page(), key, now),
        }
    }

    /// The Island's own nodes: the capsule, the close button, the overflow
    /// button and the tabs. `None` for any other node.
    fn activate_chrome(&mut self, key: Key, now: Instant) -> Option<Option<IslandAction>> {
        if key == view::capsule_key() {
            self.open(PageId::CLOCK, now);
            Some(None)
        } else if key == view::close_key() {
            self.close(now);
            Some(None)
        } else if key == view::more_key() {
            Some(self.page.node_by_key(key).map(|id| IslandAction::OpenMenu {
                more: self.page.rect(id),
                keyboard: false,
            }))
        } else {
            let page = tabs::page_of(key)?;
            self.open(page, now);
            Some(None)
        }
    }

    /// What a click or Enter on the node `key` of `page`'s content does,
    /// whether the Island or a sheet shows it.
    pub(super) fn page_action(
        &mut self,
        page: PageId,
        key: Key,
        _now: Instant,
    ) -> Option<IslandAction> {
        if page == PageId::CLIPBOARD {
            if let Some(handled) = self.clipboard_activate(key) {
                return handled;
            }
        }
        if let Some(step) = clock_page::month_step(key) {
            let offset = step.apply(self.month_offset);
            if offset != self.month_offset {
                self.month_offset = offset;
                self.stale = true;
            }
            None
        } else if key == sysmon_page::open_key() {
            Some(IslandAction::OpenApp(sysmon_page::MONITOR_DESKTOP_ID))
        } else if let Some(action) = self.connectivity_action(page, key) {
            Some(IslandAction::Connectivity(action))
        } else if let Some(action) =
            media_page::action_for(&self.media, &self.theme, self.state.max_page_size(), key)
        {
            Some(action)
        } else if let Some(action) = notifications::action_for(
            &self.history,
            &self.owners,
            &self.theme,
            self.state.max_page_size(),
            key,
        ) {
            Some(action)
        } else if let Some(action) = phone_page::action_for(&self.phone, key) {
            Some(action)
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

    /// What the row `key` of the connectivity `page` offers: only the page
    /// shown answers, and only for a row it drew with an action.
    pub(super) fn connectivity_action(&self, page: PageId, key: Key) -> Option<Action> {
        if self.page_shown() != Some(page) {
            return None;
        }
        let action_for = match page {
            PageId::NETWORK => network_page::action_for,
            PageId::BLUETOOTH => bluetooth_page::action_for,
            PageId::POWER => power_page::action_for,
            _ => return None,
        };
        action_for(
            &self.connectivity,
            &self.theme,
            self.state.max_page_size(),
            key,
        )
    }
}
