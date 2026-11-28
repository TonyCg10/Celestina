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
//!
//! The surface spans the output (parity spec §6), and a third tree, the
//! bar's zone capsules ([`bar_tree`]), is painted before the Island; the
//! regions are the union of every painted silhouette. The bar's tree exists
//! once the app hands the Island its layout ([`Island::set_bar`]), before
//! the first frame.

pub mod bar_tree;
pub mod cadence;
pub mod clock;
pub mod menu;
pub(crate) mod open_piece;
mod page_sheets;
pub mod pages;
mod sheets;
pub mod state;
pub mod view;
pub mod workspaces;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use celestina_paint::{slices_of_path, Painter, PixmapId, Rect, TextStyle, Theme};
use celestina_shell::{
    connectivity::Action, glance::Today, notifications::Notification, phone::PhoneState,
    settings::BarLayout,
};
use celestina_ui::{Interaction, Key, Tree, UiEvent, View, BTN_LEFT};
use celestina_wayland::{CloseReason, FrameCx, InputEvent, PopupId, PopupShell, SurfaceCtl};

use crate::{
    bar::silhouette::{self, Shape},
    bubbles::Bubbles,
    clipboard::{verbs::ClipboardVerb, ClipboardState},
    command::Verb,
    connectivity::state::ConnectivityState,
    copy,
    media::{cover::Cover, MediaCommand, PlayerView},
    notifications::{
        banners::{self, BannerAction},
        island_notice::{self, Gesture as NoticeGesture},
    },
    osd::popup as level_popup,
    pieces::{
        self, workspaces as workspaces_piece, AudioAction, AudioLevels, Axis, BarCx,
        BrightnessAction, Gesture, Levels, PageId, PieceAction, SheetKind,
    },
    providers::{
        magnetita::MediaKind, niri::NiriModel, notifications::NotificationsCommand,
        sysmon::SysReading, weather::WeatherReading,
    },
    session_menu::SessionAction,
    sheet::{Measure, Next, Sheets},
    tray::{scroll_steps, ItemId, Orientation, TrayCommand},
};
use bar_tree::BarTree;
use cadence::{Cadence, Offer, Redraws, Wake, SYSMON_BAR_PERIOD};
use clock::{ClockTick, SecondTick};
use pages::{
    clipboard as clipboard_page,
    media::MediaState,
    page_view,
    tray::{self as tray_page, IconCache, Pixmaps, TrayList, TrayRow},
    HistoryRow, NameOwner, PageCx,
};
use state::{IslandState, SILHOUETTE_STEP, STRIP_W};
use workspaces::{rows_for, Row};

/// What the Island asks the app to do: talk to the compositor, open a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum IslandAction {
    FocusWorkspace(u64),
    FocusWindow(u64),
    /// Something the tray host does: activate, secondary activate, scroll.
    Tray(TrayCommand),
    /// The menu of tray item `item` was asked for from its page `row` (a
    /// rect of the surface), by the pointer or the `keyboard`: the app asks
    /// the host for the menu and hands the popup to
    /// [`Island::open_tray_menu`] once it arrives.
    OpenTrayMenu {
        item: ItemId,
        row: Rect,
        keyboard: bool,
        /// Where the pointer was, in output coordinates: what the item
        /// hears in `ContextMenu` when its dbusmenu cannot be had.
        at: (i32, i32),
    },
    /// The overflow button at `more` (a rect of the surface) was chosen,
    /// from the `keyboard` or the pointer: the app builds the menu and
    /// hands it to [`Island::open_menu`].
    OpenMenu {
        more: Rect,
        keyboard: bool,
    },
    /// A row's close or action button on the
    /// Notifications page: the notification server runs the command.
    Notification(NotificationsCommand),
    /// The Notifications page's clear-all button: the notification server
    /// closes what is live, then forgets what has ended.
    ClearNotifications,
    /// The Notifications page's switch: the app flips the do-not-disturb
    /// setting, the one owner of the choice.
    ToggleDoNotDisturb,
    /// A row that starts the application with this desktop id; the app
    /// finds it in the session's applications and starts it as the launcher
    /// does.
    OpenApp(&'static str),
    /// A Clipboard page row or button: the app passes the verb to the
    /// clipboard duty, which queues it and tells the journal how it ended.
    Clipboard(ClipboardVerb),
    /// A connectivity page's row or piece asked for this: the app checks it
    /// against its state and only then hands it to the worker (or starts the
    /// editing UI).
    Connectivity(Action),
    /// The Volume piece's wheel or mute: the app commands the audio worker.
    Audio(AudioAction),
    /// The Brightness piece's wheel: the app commands the brightness workers
    /// for this Island's output.
    Brightness(BrightnessAction),
    /// The Volume or Brightness piece (`piece`, a node of the bar) asked for
    /// its sheet: the app builds the body and hands it to
    /// [`Island::open_level_sheet`].
    OpenLevelSheet {
        kind: level_popup::Kind,
        piece: Key,
    },
    /// A row of the session sheet was chosen (the confirmation card's
    /// button for the dangerous ones): the app posts its verb at once.
    Session(SessionAction),
    /// A transport button of the Phone page: the app asks Magnetita for
    /// the same request `phone media` makes.
    PhoneMedia(MediaKind),
    /// A bubble of the bar: restore the minimised window `window_id`. The
    /// transition may be anchored to the bubble, `bubble` (a rect of the
    /// surface, which covers the output `output`), when it is known.
    RestoreWindow {
        window_id: u64,
        output: String,
        bubble: Option<Rect>,
    },
    /// The overflow bubble: the app opens the selector of minimised
    /// windows.
    OpenSelector,
    /// A Media page button or player row, or the Media piece's middle
    /// click: the media worker runs the command on the shown player.
    Media(MediaCommand),
}

/// How long a clear-all waits between one row's leave and the next's.
const CLEAR_STEP: Duration = Duration::from_millis(30);
/// Linux `BTN_RIGHT` and `BTN_MIDDLE`.
pub(crate) const BTN_RIGHT: u32 = 0x111;
pub(crate) const BTN_MIDDLE: u32 = 0x112;
/// The Menu key and F10 (with Shift): the keyboard's context menu.
pub(super) const XKB_KEY_MENU: u32 = 0xff67;
pub(super) const XKB_KEY_F10: u32 = 0xffc9;
/// The Delete key and the keypad's: they remove the focused clipboard entry.
const XKB_KEY_DELETE: u32 = 0xffff;
const XKB_KEY_KP_DELETE: u32 = 0xff9f;

/// What the Island reads of the app each frame besides the compositor's
/// model: who holds the bus names, the settings in force, the notification
/// history and the tray.
#[derive(Clone, Copy, Debug)]
pub struct Shared<'a> {
    /// Who holds the bus names the pages depend on.
    pub owners: &'a NameOwner,
    /// The do-not-disturb setting in force.
    pub do_not_disturb: bool,
    /// The notification history's rows.
    pub history: &'a [HistoryRow],
    /// The tray's items.
    pub tray: &'a TrayList,
    /// The system monitor's newest reading.
    pub sysmon: Option<&'a SysReading>,
    /// The weather's reading.
    pub weather: Option<&'a WeatherReading>,
    /// Whether `duties.clipboard` is on.
    pub clipboard_on: bool,
    /// The clipboard worker's latest rows: previews only.
    pub clipboard: &'a ClipboardState,
    /// The default audio devices' readings (empty while the `osd` duty is
    /// off).
    pub audio: AudioLevels,
    /// The brightness of each output's monitor, by output name.
    pub brightness: &'a std::collections::BTreeMap<String, u8>,
    /// The phone as Magnetita lists it (CEL-2-J).
    pub phone: &'a PhoneState,
    /// Whether `duties.phone` is on: the Phone tab and `island open phone`
    /// exist only then (CEL-2-J).
    pub phone_duty: bool,
    /// The minimised windows Melibea keeps (CEL-2-J).
    pub bubbles: &'a Bubbles,
}

/// Whether the bar's clipboard piece draws something else for `new` than
/// for `old`: it reads only whether the duty is on and whether recording is
/// paused, never the rows.
fn bar_clipboard_changed(
    old_on: bool,
    old: &ClipboardState,
    new_on: bool,
    new: &ClipboardState,
) -> bool {
    old_on != new_on || old.paused() != new.paused()
}

/// The painter's textures as the tray's icon cache stores pixmaps in them.
impl Pixmaps for Painter {
    fn upload(&mut self, argb: &[u8], w: u32, h: u32) -> Option<PixmapId> {
        match self.upload_pixmap(argb, w, h) {
            Ok(id) => Some(id),
            Err(err) => {
                tracing::warn!(%err, "a tray icon could not be uploaded");
                None
            }
        }
    }

    fn release(&mut self, id: PixmapId) {
        self.release_pixmap(id);
    }

    fn has_theme_icon(&self, name: &str, size: u32) -> bool {
        Painter::has_theme_icon(self, name, size)
    }
}

/// A decoded cover as the painter takes pixmaps: ARGB32 in network byte
/// order. `None` when the buffer is not `width * height` RGBA texels.
fn argb_of(cover: &Cover) -> Option<Vec<u8>> {
    let texels = (cover.width as usize).checked_mul(cover.height as usize)?;
    if texels.checked_mul(4)? != cover.rgba.len() {
        return None;
    }
    Some(
        cover
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|t| [t[3], t[0], t[1], t[2]])
            .collect(),
    )
}

/// How long after a wheel notch over the workspaces piece the next one steps
/// from the workspace that notch asked for (the slider wheel's burst).
const WORKSPACE_WHEEL_BURST: std::time::Duration = std::time::Duration::from_millis(1500);

/// The wheel's pixels not yet worth a step, per axis, for the row they
/// were spent on.
#[derive(Debug, Default)]
struct ScrollRemainder {
    row: Option<Key>,
    vertical: i32,
    horizontal: i32,
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
    /// The notices surface paints the capsule while this is set.
    lent: bool,
    /// The newest notice the app routed to this Island: it is a card at the
    /// top of the page while the page is open (§5.5).
    notice: Option<Notification>,
    /// What the page keeps free at its top for the card, logical px: the
    /// card's room while shown, kept while the page closes so it does not
    /// jump, zero when a page opens without one.
    lead: f32,
    /// The pointer is over the card.
    card_over: bool,
    /// The server's clock was last told to hold for the card.
    card_held: bool,
    /// The scroll gesture that last closed the card's notice.
    card_wheel: island_notice::WheelLatch,
    clock: String,
    clock_minute: Option<u64>,
    tick: ClockTick,
    /// The page's `HH:MM:SS` clock, refreshed only while a page is open.
    clock_seconds: String,
    second_tick: SecondTick,
    rows: Vec<Row>,
    /// The last niri model: what the Activity page lists and what a click
    /// on one of its rows asks for.
    model: Option<NiriModel>,
    /// Who holds the bus names the pages depend on.
    owners: NameOwner,
    /// The do-not-disturb setting the Notifications page shows.
    do_not_disturb: bool,
    /// The notification rows the Notifications page lists.
    history: Vec<HistoryRow>,
    /// The rows the Notifications page draws: `history`, but while a
    /// clear-all folds them one by one, the rows that have not left yet.
    presented: Vec<HistoryRow>,
    /// A clear-all in flight: when it was pressed and the rows it folds
    /// (the ones the page shows, marked folding).
    clearing: Option<(Instant, Vec<HistoryRow>)>,
    /// The rows a clear-all asked the worker to forget: hidden from the page
    /// until the worker's snapshot no longer holds them.
    cleared: Vec<u32>,
    /// What the connectivity pieces and pages read (CEL-2-F): a copy of
    /// the app's, pushed by [`Island::set_connectivity`].
    connectivity: ConnectivityState,
    /// The page slide the page view was built with.
    applied_slide: f32,
    /// The theme the views were built with.
    theme: Theme,
    reduced: bool,
    /// The views must be built again before the next layout.
    stale: bool,
    /// The open page sheet's body is built again at the frame's next step
    /// ([`Island::refit_page_sheet`]): the views were built again.
    page_sheet_stale: bool,
    /// The growth was in flight at the last frame.
    moving: bool,
    /// The overflow menu, while it is open.
    menu: Option<PopupId>,
    /// The first level of a tray item's menu, while it is open.
    tray_menu: Option<PopupId>,
    /// The tray's items as the page shows them.
    tray_rows: Vec<TrayRow>,
    /// The tray's overflow window is open: the chevron is turned.
    overflow_shown: bool,
    /// The generation of the tray list `tray_rows` was made from.
    tray_generation: u64,
    /// The output scale the tray rows' icons were resolved at.
    tray_dpr: f32,
    /// The clipboard rows the panel was made from, and whether the duty was
    /// on.
    clipboard: ClipboardState,
    clipboard_on: bool,
    /// What the Clipboard page and piece read, with its thumbnails uploaded.
    clip_panel: clipboard_page::ClipboardPanel,
    clip_textures: clipboard_page::Textures,
    /// A clear was asked for and awaits its confirmation.
    confirming_clear: bool,
    /// The node to focus once the page is next built: where the keyboard
    /// goes when the confirmation step appears or is given up.
    pending_focus: Option<Key>,
    /// A Delete key is down: its auto-repeat must not remove more entries.
    delete_held: bool,
    /// The phone the bar's pieces read (CEL-2-J).
    phone: PhoneState,
    /// Whether the phone duty is on, as the last frame saw it.
    phone_duty: bool,
    /// Whether a polkit prompt holds the keyboard (N-25): nothing opens and
    /// the surface asks for no keyboard while it does. The app sets it
    /// before every frame and input call.
    keyboard_claimed: bool,
    /// The minimised windows the bar's pieces read (CEL-2-J).
    bubbles: Bubbles,
    /// The pixmaps uploaded for the items' icons.
    icons: IconCache,
    /// The players the Media piece and page show (CEL-2-D).
    media: MediaState,
    /// The covers uploaded for the players: bus name, the art url they
    /// were decoded from and their texture.
    covers: HashMap<String, (String, PixmapId)>,
    scroll: ScrollRemainder,
    /// The bar's zone capsules, from the first layout handed over
    /// ([`Island::set_bar`]) on; `None` only between [`Island::new`] and it.
    bar: Option<BarTree>,
    /// The capsule sheet that is open, and what waits for it to close.
    sheets: Sheets,
    /// What the Volume and Brightness pieces read.
    levels: Levels,
    /// The open tray menu hangs from the bar, not from the Tray page, so it
    /// outlives the page.
    tray_menu_on_bar: bool,
    /// The last tray menu asked for was a bar icon's.
    bar_menu_asked: bool,
    /// The item whose menu was asked for from a row of the tray's overflow
    /// window.
    overflow_menu_asked: Option<ItemId>,
    /// The item whose open tray card hangs from its row in the overflow
    /// window: the card goes when the window closes or loses that row.
    tray_menu_from_window: Option<ItemId>,
    /// The last pointer position seen, in the surface's logical coordinates.
    /// The surface is anchored to the output's whole top edge, so these are
    /// the output's coordinates too.
    pointer: (f32, f32),
    /// The workspace the wheel over the workspaces piece last asked for and
    /// when: a notch within [`WORKSPACE_WHEEL_BURST`] steps from it, not from
    /// the model's active workspace, which may not have caught up.
    workspace_wheel: Option<(u64, Instant)>,
    /// The system monitor's reading as the bar shows it: refreshed at most
    /// once per [`SYSMON_BAR_PERIOD`].
    bar_sysmon: Cadence<SysReading>,
    /// The system monitor's newest reading, for the open page.
    page_sysmon: Option<SysReading>,
    /// The weather's reading.
    weather: Option<WeatherReading>,
    /// Today's local date, refreshed with the minute.
    today: Option<Today>,
    /// The month the Clock page shows, in months from the current one;
    /// bounded by [`clock_page::MAX_OFFSET`] and reset when the page closes.
    month_offset: i32,
}

/// A point of the surface as the integer output coordinates an item hears
/// in `ContextMenu`. The surface is anchored to the output's whole top
/// edge, so its coordinates are the output's; a non-finite value is 0.
fn output_point((x, y): (f32, f32)) -> (i32, i32) {
    let whole = |v: f32| if v.is_finite() { v.round() as i32 } else { 0 };
    (whole(x), whole(y))
}

/// The room the page is laid out in on a surface of `size`: never wider
/// than the 1.0.0 strip, so a page keeps its size on the bar's wide surface.
fn page_room(size: (f32, f32)) -> (f32, f32) {
    (size.0.min(STRIP_W), size.1)
}

impl Island {
    /// The capsule at rest for `output`, on a surface of `size` logical px.
    #[must_use]
    pub fn new(output: &str, size: (f32, f32)) -> Island {
        let theme = Theme::dark();
        let mut state = IslandState::new(0.0);
        state.set_surface(page_room(size));
        Island {
            output: output.to_owned(),
            size,
            dpr: 1.0,
            state,
            capsule: Tree::new(false, &theme),
            page: Tree::new(false, &theme),
            capsule_input: Interaction::new(),
            page_input: Interaction::new(),
            lent: false,
            notice: None,
            lead: 0.0,
            card_over: false,
            card_held: false,
            card_wheel: island_notice::WheelLatch::default(),
            clock: String::new(),
            clock_minute: None,
            tick: ClockTick::default(),
            clock_seconds: String::new(),
            second_tick: SecondTick::default(),
            rows: Vec::new(),
            model: None,
            owners: NameOwner::default(),
            do_not_disturb: false,
            history: Vec::new(),
            presented: Vec::new(),
            clearing: None,
            cleared: Vec::new(),
            connectivity: ConnectivityState::default(),
            applied_slide: 0.0,
            theme,
            reduced: false,
            stale: true,
            page_sheet_stale: false,
            moving: false,
            menu: None,
            tray_menu: None,
            tray_rows: Vec::new(),
            tray_generation: 0,
            overflow_shown: false,
            tray_dpr: 1.0,
            clipboard: ClipboardState::default(),
            clipboard_on: false,
            clip_panel: clipboard_page::ClipboardPanel::default(),
            clip_textures: clipboard_page::Textures::default(),
            confirming_clear: false,
            pending_focus: None,
            delete_held: false,
            phone: PhoneState::Absent,
            phone_duty: false,
            keyboard_claimed: false,
            bubbles: Bubbles::default(),
            icons: IconCache::default(),
            media: MediaState::default(),
            covers: HashMap::new(),
            scroll: ScrollRemainder::default(),
            workspace_wheel: None,
            bar: None,
            sheets: Sheets::default(),
            levels: Levels::default(),
            tray_menu_on_bar: false,
            bar_menu_asked: false,
            overflow_menu_asked: None,
            tray_menu_from_window: None,
            pointer: (0.0, 0.0),
            bar_sysmon: Cadence::new(SYSMON_BAR_PERIOD),
            page_sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
        }
    }

    /// The system monitor's reading as the bar shows it.
    #[must_use]
    pub fn bar_sysmon(&self) -> Option<&SysReading> {
        self.bar_sysmon.shown()
    }

    /// The system monitor's newest reading, as the open page shows it.
    #[must_use]
    pub fn page_sysmon(&self) -> Option<&SysReading> {
        self.page_sysmon.as_ref()
    }

    /// The weather's reading.
    #[must_use]
    pub fn weather(&self) -> Option<&WeatherReading> {
        self.weather.as_ref()
    }

    /// Today's local date, as of the last minute the Island read.
    #[must_use]
    pub fn today(&self) -> Option<Today> {
        self.today
    }

    /// What the system monitor's `newest` reading at `now` asks of this
    /// Island's surface: a frame while a page is open (the page reads the
    /// newest reading), a frame when the bar's held value changes, a timed
    /// frame when it is held back, and nothing otherwise, so a closed
    /// Island's rest stays at one frame a minute.
    #[must_use]
    pub fn sysmon_wake(&self, newest: Option<&SysReading>, now: Instant) -> Wake {
        if self.sysmon_page_open() && newest != self.page_sysmon.as_ref() {
            return Wake::Now;
        }
        if self.bar.is_none() {
            return Wake::Nothing;
        }
        match self.bar_sysmon.peek(newest, now) {
            Offer::Same => Wake::Nothing,
            Offer::Changed => Wake::Now,
            Offer::Pending(wait) => Wake::After(wait),
        }
    }

    /// Whether the System page is the one shown (the Island's or a sheet's):
    /// the only page that reads the newest reading.
    fn sysmon_page_open(&self) -> bool {
        self.page_shown() == Some(PageId::SYSMON)
    }

    /// The bar's layout in force on this Island: what [`Island::set_bar`]
    /// handed over last, `None` before the first layout.
    #[must_use]
    pub fn bar_layout(&self) -> Option<&BarLayout> {
        self.bar.as_ref().map(BarTree::layout)
    }

    /// Gives the bar `layout`: the first time it makes the bar's tree, after
    /// that it lays the bar out again only when the layout differs. The
    /// surface's shape is the app's (its width comes with every frame).
    /// Takes effect on the next frame, which the caller asks for.
    pub fn set_bar(&mut self, layout: &BarLayout) {
        match self.bar.as_mut() {
            Some(bar) => {
                if !bar.set_layout(layout) {
                    return;
                }
            }
            None => self.bar = Some(BarTree::new(layout, &self.theme, self.reduced)),
        }
        // The capsule's dots follow the bar.
        self.stale = true;
    }

    /// The app's connectivity state changed: the pages and the bar's pieces
    /// read `state` from now on. Returns whether this Island shows any of it
    /// (a page is open or the bar has its layout), so the caller asks for a
    /// frame only then.
    pub fn set_connectivity(&mut self, state: &ConnectivityState) -> bool {
        if *state == self.connectivity {
            return false;
        }
        self.connectivity.clone_from(state);
        let open = self.page_shown().is_some();
        self.stale |= open;
        self.invalidate_bar();
        open || self.bar.is_some()
    }

    /// What the bar's pieces read, from what the Island knows. The hidden
    /// tray ids are the bar's own layout's (`BarTree` fills them in).
    fn bar_cx(&self) -> BarCx<'_> {
        BarCx {
            clipboard: &self.clip_panel,
            theme: &self.theme,
            output: &self.output,
            model: self.model.as_ref(),
            owners: &self.owners,
            do_not_disturb: self.do_not_disturb,
            history: &self.history,
            items: &self.tray_rows,
            tray_hidden: &[],
            tray_open: self.overflow_shown,
            sysmon: self.bar_sysmon.shown(),
            weather: self.weather.as_ref(),
            today: self.today,
            connectivity: &self.connectivity,
            levels: self.levels,
            phone: &self.phone,
            bubbles: &self.bubbles,
            media: &self.media,
        }
    }

    /// Runs `f` on the bar with the context its pieces read; `None` while
    /// the bar has no layout yet.
    fn with_bar<R>(&mut self, f: impl FnOnce(&mut BarTree, &BarCx<'_>) -> R) -> Option<R> {
        let mut bar = self.bar.take()?;
        let out = f(&mut bar, &self.bar_cx());
        self.bar = Some(bar);
        Some(out)
    }

    /// Something the bar's pieces read changed. A page sheet's subtitle
    /// repeats what its piece shows, so it is built again too.
    fn invalidate_bar(&mut self) {
        self.stale |= self.sheets.page().is_some();
        if let Some(bar) = self.bar.as_mut() {
            bar.invalidate();
        }
    }

    /// Makes the clipboard panel from the rows last shown and the duty's
    /// state, uploading new thumbnails to `pixmaps`. A clear awaiting
    /// confirmation is given up when nothing is left to clear.
    fn rebuild_clipboard(&mut self, pixmaps: &mut impl Pixmaps) {
        if self.clipboard.rows().is_empty() || !self.clipboard_on {
            self.confirming_clear = false;
        }
        self.clip_panel = self.clip_textures.panel(
            self.clipboard_on,
            &self.clipboard,
            self.confirming_clear,
            pixmaps,
        );
    }

    /// Gives up a clear that awaits its confirmation (the page closed or was
    /// left).
    fn forget_confirmation(&mut self) {
        self.pending_focus = None;
        if self.confirming_clear {
            self.confirming_clear = false;
            self.clip_panel.confirming_clear = false;
            self.stale = true;
            self.invalidate_bar();
        }
    }

    /// Whether the Clipboard page is the one shown right now (the Island's or
    /// a sheet's).
    #[must_use]
    pub fn shows_clipboard_page(&self) -> bool {
        self.page_shown() == Some(PageId::CLIPBOARD)
    }

    /// What a click or Enter on the Clipboard page's node `key` does: a verb
    /// for the app, or only a change of the confirmation step.
    fn clipboard_activate(&mut self, key: Key) -> Option<Option<IslandAction>> {
        if !self.shows_clipboard_page() {
            return None;
        }
        let action = clipboard_page::action_for(
            &self.clip_panel,
            &self.theme,
            self.state.max_page_size(),
            key,
        )?;
        Some(self.run_clipboard(action))
    }

    fn run_clipboard(&mut self, action: clipboard_page::PageAction) -> Option<IslandAction> {
        use clipboard_page::PageAction;
        match action {
            PageAction::AskClear | PageAction::CancelClear => {
                self.confirming_clear = action == PageAction::AskClear;
                self.clip_panel.confirming_clear = self.confirming_clear;
                // The safe button takes the keyboard when the question
                // appears; Clear comes back with it when it is given up.
                self.pending_focus = Some(if self.confirming_clear {
                    clipboard_page::cancel_key()
                } else {
                    clipboard_page::clear_key()
                });
                self.stale = true;
                None
            }
            PageAction::Verb(verb) => {
                if verb == ClipboardVerb::Clear {
                    self.forget_confirmation();
                }
                Some(IslandAction::Clipboard(verb))
            }
        }
    }

    /// Delete on the focused clipboard row removes its entry.
    fn delete_key(&mut self) -> Option<IslandAction> {
        let key = {
            let (tree, _) = self.shown_page_input()?;
            tree.key_of(tree.highlight().focused?)
        };
        let action = clipboard_page::remove_index(
            &self.clip_panel,
            &self.theme,
            self.state.max_page_size(),
            key,
        )?;
        self.run_clipboard(action)
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

    /// Whether the Notifications page is on screen (or growing), as the
    /// Island's page or a sheet: what the person sees there counts as seen.
    #[must_use]
    pub fn shows_notifications(&self) -> bool {
        self.page_shown() == Some(PageId::NOTIFICATIONS)
    }

    /// The open page, if any.
    #[must_use]
    pub fn page(&self) -> Option<PageId> {
        self.state.page()
    }

    /// Reduced motion makes the growth and every slide a single step.
    pub fn set_reduced_motion(&mut self, on: bool) {
        if on != self.reduced {
            self.reduced = on;
            self.capsule.set_reduced_motion(on);
            self.page.set_reduced_motion(on);
            if let Some(bar) = self.bar.as_mut() {
                bar.set_reduced_motion(on);
            }
        }
    }

    /// Whether `model` changes what this Island shows (and is worth a frame).
    #[must_use]
    pub fn shows_a_change(&self, model: Option<&NiriModel>) -> bool {
        self.rows_from(model) != self.rows
            || (self.page_shown().is_some() && model != self.model.as_ref())
    }

    fn rows_from(&self, model: Option<&NiriModel>) -> Vec<Row> {
        model.map_or_else(Vec::new, |m| rows_for(m, &self.output))
    }

    /// Draws one frame of the Island's surface with `model` (the last niri
    /// model, if any) and what is `shared` with the app, under `reduced`
    /// motion.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        model: Option<&NiriModel>,
        shared: &Shared<'_>,
        reduced: bool,
    ) {
        let Shared {
            owners,
            do_not_disturb,
            history,
            tray,
            sysmon,
            weather,
            clipboard_on,
            clipboard,
            audio,
            brightness,
            phone,
            phone_duty,
            bubbles,
        } = *shared;
        let now = cx.now;
        self.set_reduced_motion(reduced);
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.capsule.set_theme(&self.theme);
            self.page.set_theme(&self.theme);
            if let Some(bar) = self.bar.as_mut() {
                bar.set_theme(&self.theme);
            }
            self.stale = true;
        }
        if cx.size != self.size {
            self.size = cx.size;
            self.state.set_surface(page_room(cx.size));
            self.stale = true;
        }
        self.dpr = cx.scale as f32;
        let rows = self.rows_from(model);
        if rows != self.rows {
            self.rows = rows;
            self.stale = true;
        }
        // What the pages show is rebuilt only while one is open; the bar
        // shows it at rest too.
        let mut seen = false;
        if model != self.model.as_ref() {
            self.model = model.cloned();
            seen = true;
        }
        if *owners != self.owners {
            self.owners = owners.clone();
            seen = true;
        }
        if do_not_disturb != self.do_not_disturb {
            self.do_not_disturb = do_not_disturb;
            seen = true;
        }
        if history != self.history {
            self.history = history.to_vec();
            seen = true;
        }
        let presented = self.presented_at(now);
        if presented != self.presented {
            self.presented = presented;
            seen = true;
        }
        if self.clearing.is_some() {
            cx.ctl.redraw_after(CLEAR_STEP);
        }
        let levels = Levels {
            audio,
            brightness: brightness.get(&self.output).copied(),
        };
        if levels != self.levels {
            self.levels = levels;
            seen = true;
        }
        if phone_duty != self.phone_duty {
            self.phone_duty = phone_duty;
            // A page that no longer exists is not left open.
            if !phone_duty {
                self.phone_duty_off(now);
            }
            seen = true;
        }
        if *phone != self.phone {
            self.phone = phone.clone();
            seen = true;
        }
        if *bubbles != self.bubbles {
            self.bubbles = bubbles.clone();
            seen = true;
        }
        if tray.generation != self.tray_generation || self.dpr != self.tray_dpr {
            self.tray_generation = tray.generation;
            self.tray_dpr = self.dpr;
            self.tray_rows = self.icons.rows(&tray.items, self.dpr, painter);
            seen = true;
        }
        if self.mirror_weather(weather) {
            seen = true;
        }
        if clipboard_on != self.clipboard_on || *clipboard != self.clipboard {
            // The bar's piece reads only whether the duty is on and paused;
            // the rows are the page's, drawn while it is the one shown.
            let bar_reads =
                bar_clipboard_changed(self.clipboard_on, &self.clipboard, clipboard_on, clipboard);
            self.clipboard_on = clipboard_on;
            self.clipboard = clipboard.clone();
            self.rebuild_clipboard(painter);
            if bar_reads {
                seen = true;
            } else if self.shows_clipboard_page() {
                self.stale = true;
            }
        }
        if seen {
            self.stale |= self.page_shown().is_some()
                || matches!(
                    self.sheets.kind(),
                    Some(SheetKind::Workspaces | SheetKind::TrayOverflow)
                );
            self.invalidate_bar();
        }
        if self.menu.is_some() && self.page_shown().is_none() {
            // The sheet went by another way than a press: its menu goes too.
            self.close_page_menus(&mut cx.ctl);
        }
        if self.tray_card_orphaned() {
            self.close_menu(&mut cx.ctl);
        }
        self.hold_sysmon(sysmon, now, &mut cx.ctl);
        self.tick_clock(now, &mut cx.ctl);
        let dpr = self.dpr;
        self.sheets_before_layout(now, &mut |s: &str, st: &TextStyle, w: Option<f32>| {
            painter.text().measure(s, st, w, dpr)
        });
        if self.stale || self.state.slide(now) != self.applied_slide {
            self.apply_views(now);
        }
        self.refit_page_sheet(now, &mut |s: &str, st: &TextStyle, w: Option<f32>| {
            painter.text().measure(s, st, w, dpr)
        });
        self.layout(painter, now);
        self.sheets_after_layout(now);
        self.paint(painter, now, cx.blur_available);
        self.sync_ctl(&mut cx.ctl, now);

        let moving = self.state.wants_frame(now);
        let bar_moving = self.bar.as_ref().is_some_and(|b| b.is_animating(now));
        if moving
            || self.capsule.is_animating(now)
            || self.page.is_animating(now)
            || bar_moving
            || self.sheets.wants_frame(now)
        {
            cx.ctl.request_redraw();
        }
        if let Some(wait) = self.sheets.wait(now) {
            cx.ctl.redraw_after(wait);
        }
        if self.moving && !moving {
            tracing::debug!(output = %self.output, open = self.state.is_open(), "island: growth settled");
        }
        self.moving = moving;
    }

    /// The rows the Notifications page draws at `now`: while a clear-all
    /// folds the rows, row `i` leaves at `i` times [`CLEAR_STEP`] after the
    /// press, top to bottom, and the empty list follows the last one;
    /// otherwise the history.
    fn presented_at(&mut self, now: Instant) -> Vec<HistoryRow> {
        let history = &self.history;
        self.cleared
            .retain(|id| history.iter().any(|r| r.id == *id));
        if let Some((start, rows)) = &self.clearing {
            let left = (now.saturating_duration_since(*start).as_millis() / CLEAR_STEP.as_millis())
                .saturating_add(1);
            if let Some(n) = usize::try_from(left).ok().filter(|n| *n < rows.len()) {
                return rows[n..].to_vec();
            }
            self.clearing = None;
        }
        // Until the worker's snapshot forgets them, the cleared rows stay
        // out, so none comes back between the fold and the snapshot.
        let cleared = &self.cleared;
        history
            .iter()
            .filter(|r| !cleared.contains(&r.id))
            .cloned()
            .collect()
    }

    /// The frame's part for the system monitor: mirrors `newest`
    /// ([`Island::mirror_sysmon`]) and asks `to` for the deadline of the copy
    /// still held back, or takes the earlier one back when none is.
    fn hold_sysmon(&mut self, newest: Option<&SysReading>, now: Instant, to: &mut impl Redraws) {
        Wake::held(self.mirror_sysmon(newest, now)).apply(to);
    }

    /// The system monitor's `newest` reading: the open page takes it as it
    /// is, the bar through its cadence. Returns the wait to arm for the
    /// frame that shows a held-back value on the bar.
    fn mirror_sysmon(&mut self, newest: Option<&SysReading>, now: Instant) -> Option<Duration> {
        if newest != self.page_sysmon.as_ref() {
            self.page_sysmon = newest.cloned();
            self.stale |= self.sysmon_page_open();
        }
        match self.bar_sysmon.offer(newest, now) {
            Offer::Same => None,
            Offer::Changed => {
                self.invalidate_bar();
                None
            }
            Offer::Pending(wait) => self.bar.is_some().then_some(wait),
        }
    }

    /// Reads the wall clock: a new minute may change the text; the minute
    /// deadline is asked for again every frame and moves to the next
    /// boundary once it has passed.
    fn tick_clock(&mut self, now: Instant, ctl: &mut SurfaceCtl<'_>) {
        let (secs, nanos) = clock::unix_now();
        for wait in self.clock_waits(now, secs, nanos).into_iter().flatten() {
            ctl.redraw_after(wait);
        }
    }

    /// [`Island::tick_clock`] at wall-clock time `secs`.`nanos`: refreshes
    /// the texts and returns the waits to arm, the minute's then the
    /// second's.
    fn clock_waits(&mut self, now: Instant, secs: u64, nanos: u32) -> [Option<Duration>; 2] {
        let minute = secs / 60;
        if self.clock_minute != Some(minute) {
            self.clock_minute = Some(minute);
            let offset = clock::utc_offset();
            let text = clock::format_local(secs, offset);
            if text != self.clock {
                self.clock = text;
                self.stale = true;
            }
            self.set_today(secs, offset);
        }
        // Asked again every frame: the seconds' earlier timer replaces the
        // minute's on the surface (see `ClockTick`).
        let minute_wait = self.tick.wait(now, secs, nanos);
        [Some(minute_wait), self.tick_seconds(now, secs, nanos)]
    }

    /// Takes the weather's `reading`. A change returns `true` and sets
    /// `stale`: the capsule shows the reading, so it is rebuilt once.
    fn mirror_weather(&mut self, reading: Option<&WeatherReading>) -> bool {
        if reading == self.weather.as_ref() {
            return false;
        }
        self.weather = reading.cloned();
        self.stale = true;
        true
    }

    /// Today's date at wall-clock `secs` and UTC `offset`: a new day redraws
    /// what reads it.
    fn set_today(&mut self, secs: u64, offset: i64) {
        let today = i64::try_from(secs)
            .ok()
            .zip(i32::try_from(offset).ok())
            .and_then(|(secs, offset)| Today::from_unix(secs, offset));
        if today != self.today {
            self.today = today;
            // The capsule shows the date: a new day is one frame.
            self.stale = true;
            self.invalidate_bar();
        }
    }

    /// The page's seconds clock: refreshed and timed only while a page is
    /// open, so a closed Island still draws one frame a minute. Returns the
    /// wait to arm for the next second.
    fn tick_seconds(&mut self, now: Instant, secs: u64, nanos: u32) -> Option<Duration> {
        let open = self.state.is_open();
        let wait = self.second_tick.wait(open, now, nanos);
        if open {
            let text = clock::format_local_seconds(secs, clock::utc_offset());
            if text != self.clock_seconds {
                self.clock_seconds = text;
                self.stale = true;
            }
        }
        wait
    }

    /// Gives the focus a page action asked for (a clipboard action, a
    /// notification removed with the keyboard) to its node, when a page is
    /// shown.
    pub(super) fn take_pending_focus(&mut self) {
        if let Some(key) = self.pending_focus.take() {
            if self.page_shown().is_some() {
                if let Some((tree, input)) = self.shown_page_input() {
                    input.focus_key(tree, key);
                }
            }
        }
    }

    /// The capsule's view: the clock, the date and the weather (and the
    /// workspaces when the bar shows none), as laid out on this surface.
    /// The notices surface builds the same view, so what it paints for the
    /// rest Island is what the panel paints.
    #[must_use]
    pub fn capsule_view(&self) -> View {
        let rows = view::capsule_shows_workspaces(self.bar.as_ref().map(BarTree::layout))
            .then_some(&self.rows[..]);
        let date = self
            .today
            .map(|today| copy::short_date(today.weekday, today.day));
        let degrees = self.weather.as_ref().map(|r| {
            (
                pieces::weather::condition_icon(r.code, r.daylight),
                copy::bar_weather(r.celsius),
            )
        });
        let glance = view::Glance {
            date: date.as_deref(),
            weather: degrees.as_ref().map(|(icon, text)| (*icon, text.as_str())),
        };
        view::capsule_with(&self.theme, &self.clock, glance, rows, self.size.0)
    }

    /// The Island's rect with no page open: centred, y 8, 40 high, as wide
    /// as the capsule.
    #[must_use]
    pub fn rest_rect(&self) -> Rect {
        self.state.rest_rect(self.size.0)
    }

    /// Lends the capsule to the notices surface (`true`) or takes it back:
    /// while lent, the Island paints no capsule glass or tree, leaves its
    /// rect out of its blur and input regions and takes no pointer input
    /// over it. The bar keeps the width it placed itself around. A page
    /// that opens is the Island's again.
    pub fn lend_capsule(&mut self, lent: bool) {
        if lent && self.state.is_open() {
            // The page is the Island's: it is not lent while it is out.
            return;
        }
        if lent && !self.lent {
            // Its hover ends with it.
            self.capsule_input.pointer(
                &mut self.capsule,
                &InputEvent::PointerLeave,
                Instant::now(),
            );
        }
        self.lent = lent;
    }

    /// Whether the page is open (asked and not yet closed).
    #[must_use]
    pub fn page_open(&self) -> bool {
        self.state.is_open()
    }

    /// Opens `page` at `now` without a surface: what the app's tests use to
    /// have a page open on an output.
    #[cfg(test)]
    pub(crate) fn open_for_test(&mut self, page: PageId, now: Instant) {
        self.open(page, now);
    }

    /// The notice the app routes to this Island (the newest toast, while
    /// the Island's page is open on this output), or `None`. Whether
    /// anything changed, so the panel is redrawn.
    pub fn set_notice(&mut self, notice: Option<Notification>, _now: Instant) -> bool {
        if self.notice == notice {
            return false;
        }
        self.notice = notice;
        self.stale = true;
        true
    }

    /// Whether the Island holds a notice (its page is open and it was routed
    /// one).
    #[must_use]
    pub fn has_notice(&self) -> bool {
        self.notice.is_some()
    }

    /// The notice shown as a card: while the page is open and is not the
    /// Notifications page (which lists it).
    fn card(&self) -> Option<&Notification> {
        self.notice
            .as_ref()
            .filter(|_| self.state.is_open() && self.state.shown_page() != PageId::NOTIFICATIONS)
    }

    /// Where the card is on the surface, once laid out.
    fn card_rect(&self) -> Option<Rect> {
        let id = self.card()?.id;
        let node = self.page.node_by_key(banners::banner_key(id))?;
        Some(self.page.rect(node))
    }

    /// Closes the page at `now` without a surface.
    #[cfg(test)]
    pub(crate) fn close_for_test(&mut self, now: Instant) {
        self.close(now);
    }

    /// The id of the notice the Island holds.
    #[cfg(test)]
    pub(crate) fn notice_id(&self) -> Option<u32> {
        self.notice.as_ref().map(|n| n.id)
    }

    /// Whether the server's clock is to be held (`Some(true)`) or let go
    /// (`Some(false)`) now: it is held while the pointer is over the card,
    /// and let go when the pointer leaves or the card goes. `None` while
    /// that is what it was told last.
    pub fn take_card_hold(&mut self) -> Option<bool> {
        let want = self.card_over && self.card().is_some();
        (want != self.card_held).then(|| {
            self.card_held = want;
            want
        })
    }

    /// The Island is going: whether it held the server's clock for its card,
    /// which then has to let go. Told once.
    pub fn release_card_hold(&mut self) -> bool {
        self.card_over = false;
        std::mem::take(&mut self.card_held)
    }

    /// The pointer is over the card (what a pointer event over it sets).
    #[cfg(test)]
    pub(crate) fn hover_card_for_test(&mut self) {
        self.card_over = true;
    }

    /// Whether (`x`, `y`) is over the card.
    fn over_card(&self, x: f32, y: f32) -> bool {
        self.card_rect().is_some_and(|r| r.contains(x, y))
    }

    /// Whether `key` is a node of the card.
    fn card_has(&self, key: Key) -> bool {
        self.card()
            .is_some_and(|n| !banners::actions_for(std::slice::from_ref(n), key).is_empty())
    }

    /// What a gesture on the card asks for, as the Island's actions; a click
    /// on the body also opens the Notifications page here.
    fn card_actions(&mut self, gesture: NoticeGesture, now: Instant) -> Vec<IslandAction> {
        let Some(notice) = self.card().cloned() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for action in island_notice::gesture_on(std::slice::from_ref(&notice), notice.id, gesture) {
            match action {
                BannerAction::Send(command) => out.push(IslandAction::Notification(command)),
                BannerAction::OpenPage => self.open(PageId::NOTIFICATIONS, now),
                BannerAction::OpenClock => {}
            }
        }
        out
    }

    /// Whether the page is open or still closing at `now`: the Island has
    /// not shrunk back to the capsule.
    #[must_use]
    pub fn page_out(&self, now: Instant) -> bool {
        self.state.is_open() || self.state.progress(now) > 0.0
    }

    /// Whether the capsule is lent out.
    #[must_use]
    pub fn capsule_lent(&self) -> bool {
        self.lent
    }

    /// Whether the panel paints its capsule at `now`: not while lent at
    /// rest.
    fn paints_capsule(&self, now: Instant) -> bool {
        !(self.lent && !self.state.is_open() && self.state.progress(now) <= 0.0)
    }

    /// Builds both views. The page asks for its size first, so it is laid
    /// out at the size the Island is heading for. While the page slides, the
    /// view is applied every frame with its new offset and the tree's own
    /// slide of moved nodes is off, so the offset is the only motion.
    fn apply_views(&mut self, now: Instant) {
        let w = self.size.0;
        self.capsule.apply(self.capsule_view(), now);
        let shown = self.state.shown_page();
        let page = page_view(shown);
        let wants = page.wants(&self.page_cx());
        let card = self.card().cloned();
        // The card's room: kept while the page closes so the shrinking page
        // does not jump, and dropped when a page opens without a card.
        self.lead = match &card {
            Some(n) => banners::page_card_room(n),
            None if self.state.is_open() => 0.0,
            None => self.lead,
        };
        let wants = (wants.0, wants.1 + self.lead);
        self.state.request_page_size(wants, now, self.reduced);
        let slide = self.state.slide(now);
        let moving = slide != 0.0 || self.applied_slide != 0.0;
        if moving {
            self.page.set_reduced_motion(true);
        }
        self.page.apply(
            view::page_with_card(
                &self.theme,
                shown,
                page.title(),
                &self.clock_seconds,
                page.view(&self.page_cx()),
                w,
                self.state.page_size(),
                slide,
                (
                    self.lead,
                    card.as_ref()
                        .map(|n| banners::page_card(&self.theme, n, self.state.page_size().0)),
                ),
            ),
            now,
        );
        if moving {
            self.page.set_reduced_motion(self.reduced);
        }
        self.apply_page_sheet();
        self.apply_workspaces_sheet(now);
        self.apply_tray_overflow_sheet(now);
        // A page sheet takes the focus once it is built again
        // ([`Island::refit_page_sheet`]).
        if self.state.is_open() || self.sheets.page().is_none() {
            self.take_pending_focus();
        }
        self.applied_slide = slide;
        self.stale = false;
    }

    /// Lays the trees out (a no-op when nothing changed) and takes the
    /// capsule's width from its content.
    fn layout(&mut self, painter: &mut Painter, now: Instant) {
        let dpr = self.dpr;
        self.layout_with(now, &mut |tree: &mut Tree, room: (f32, f32)| {
            tree.layout(room, painter.text(), dpr);
        });
    }

    /// [`Island::layout`] with any way of laying a tree out in a room: the
    /// capsule and the page, then the bar placed around the Island's width
    /// at `now`.
    fn layout_with(&mut self, now: Instant, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        lay(&mut self.capsule, self.size);
        lay(&mut self.page, self.size);
        self.sheets.layout(lay);
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
        let (surface_w, island_w) = (self.size.0, self.state.rect(now, self.size.0).w);
        let open = self.sheets.held_piece(now);
        let also: Vec<Key> = self
            .sheets
            .held_pieces(now)
            .into_iter()
            .filter(|k| Some(*k) != open)
            .collect();
        let overflow = self.sheets.kind() == Some(SheetKind::TrayOverflow)
            && self.sheets.piece() == Some(pieces::tray::chevron_key());
        if overflow != self.overflow_shown {
            self.overflow_shown = overflow;
            self.invalidate_bar();
        }
        let fault = self.with_bar(|bar, cx| {
            bar.set_open(open, &also, now);
            bar.update(cx, surface_w, island_w, now, lay);
            bar.take_fault()
        });
        if let Some(fault) = fault.flatten() {
            tracing::warn!(output = %self.output, %fault, "island: bar layout fault");
        }
    }

    /// The painted silhouettes at `now`: each of the bar's capsules, then
    /// the Island's rect. A menu window's glass is no silhouette here: its
    /// bands come from its path ([`Island::blur_bands`]).
    fn shapes(&self, now: Instant) -> Vec<Shape> {
        let radius = self.theme.radius_lg;
        let mut shapes = self
            .bar
            .as_ref()
            .map_or_else(Vec::new, |bar| bar.shapes(now, radius));
        if self.paints_capsule(now) {
            shapes.push((self.state.rect(now, self.size.0), radius));
        }
        shapes
    }

    /// The glass of the menu windows and of the capsules they hang from,
    /// each path under its opacity; then the bar's capsules (without the
    /// glass those paths hold), the Haze at the Island's rect, the capsule
    /// and the page, each under its opacity and clipped to that rect; last
    /// the windows' bodies.
    fn paint(&mut self, painter: &mut Painter, now: Instant, blur_available: bool) {
        let rect = self.state.rect(now, self.size.0);
        let radius = self.theme.radius_lg;
        painter.begin(self.size, self.dpr, blur_available);
        for glass in self.prepare_glass(now) {
            painter.push_state();
            painter.opacity(glass.opacity);
            painter.haze_path(&glass.path);
            painter.pop_state();
        }
        if let Some(bar) = self.bar.as_mut() {
            bar.paint(painter, now);
        }
        let capsule = self.paints_capsule(now);
        if capsule {
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
        }
        self.paint_sheet(painter, now);
        painter.end();
    }

    /// The blur region at `now`: the bands of the capsules and of the
    /// Island's rect, and the bands of every glass path (a window, its drop,
    /// its neck and the capsule they hang from, CEL-D-013). A capsule joined
    /// to a path is left out of the first set, for its path holds it: the
    /// region would cover it twice and spend rects on the bound. Every frame
    /// covers every pixel the glass paints. The one exception is a window
    /// whose glass fades under reduced motion: below
    /// [`sheets::BLUR_REGION_MIN_GLASS`] it is out of the region.
    pub(super) fn blur_bands(&self, now: Instant) -> Vec<Rect> {
        let paths = self.glass_paths(now);
        let joined: Vec<usize> = paths.iter().filter_map(|p| p.joined).collect();
        let radius = self.theme.radius_lg;
        let mut shapes = self
            .bar
            .as_ref()
            .map_or_else(Vec::new, |bar| bar.shapes_without(now, radius, &joined));
        if self.paints_capsule(now) {
            shapes.push((self.state.rect(now, self.size.0), radius));
        }
        let mut bands = silhouette::union(&shapes);
        for p in &paths {
            if p.joined.is_none() && p.opacity < sheets::BLUR_REGION_MIN_GLASS {
                continue;
            }
            bands.extend(slices_of_path(&p.path, SILHOUETTE_STEP));
        }
        bands
    }

    /// Blur and input regions plus keyboard interactivity for `now`: the
    /// blur follows the painted silhouettes every frame (the Island's, and
    /// the bar's capsules while the bar is on); the input region is those
    /// silhouettes at rest and the whole surface while open (so a click
    /// outside the page reaches the Island and closes it); the keyboard is
    /// `None` at rest and `Exclusive` while anything is shown: niri gives an
    /// exclusive layer surface on `Top` the keyboard at once, so Escape, Tab
    /// and Enter work from the first frame (an `OnDemand` one is focused only
    /// by a press that finds it already asking). The catcher takes the
    /// presses below the surface.
    fn sync_ctl(&self, ctl: &mut SurfaceCtl<'_>, now: Instant) {
        let shapes = self.shapes(now);
        let open = self.anything_shown(now);
        ctl.set_blur_region(&self.blur_bands(now));
        ctl.set_input_region(&silhouette::input_region(&shapes, open, self.size));
        ctl.set_keyboard_interactivity(self.keyboard_interactivity(now));
    }

    /// Whether the page or a sheet is painted (closing ones included): the
    /// input region is then the whole surface and the keyboard `OnDemand`.
    fn anything_shown(&self, now: Instant) -> bool {
        // What waits keeps the keyboard too: dropping it would send the
        // `FocusLost` that gives the queue up.
        self.state.is_open() || self.sheets.is_shown(now) || self.sheets.has_queued()
    }

    /// Grows into `page`, or slides to it while already open.
    fn open(&mut self, page: PageId, now: Instant) {
        if self.keyboard_claimed {
            tracing::info!("island open: a polkit prompt holds the keyboard; nothing was done");
            return;
        }
        if page == PageId::PHONE && !self.phone_duty {
            tracing::info!("island open phone: the phone duty is off; nothing was done");
            return;
        }
        if self.sheets.is_shown(now) {
            // One thing at a time: the sheet closes, the page opens once it
            // is gone.
            self.close_sheet(now);
            self.sheets.queue(Next::Page(page));
            return;
        }
        // A page asked for supersedes a sheet that waited behind it.
        self.sheets.drop_queue();
        if page != self.state.shown_page() {
            self.stale = true;
            self.forget_confirmation();
        }
        if self.state.is_open() {
            self.state.switch(page, now, self.reduced);
            return;
        }
        self.stale = true;
        // The capsule's hover ends with it.
        self.capsule_input
            .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
        self.state.open(page, now, self.reduced);
        // The page grows out of the panel's own capsule: a notices surface
        // that held it yields in this same turn.
        self.lent = false;
    }

    fn close(&mut self, now: Instant) {
        self.forget_page_state();
        if self.state.is_open() {
            self.page_input
                .pointer(&mut self.page, &InputEvent::PointerLeave, now);
            self.page_input.key(&mut self.page, &InputEvent::FocusLost);
        }
        self.state.close(now, self.reduced);
    }

    /// Handles pointer and keyboard input on the Island's surface: a click
    /// on the capsule opens the page; Escape, a click outside the page and
    /// the keyboard leaving close it; the page's buttons and rows act; a
    /// pointer on one of the bar's capsules (outside the open page) goes to
    /// the bar's pieces ([`Island::bar_input`]). `painter` measures the
    /// views if they changed since the last frame, so what the input hits
    /// is what the next frame paints.
    pub fn input(
        &mut self,
        ev: InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        painter: &mut Painter,
    ) -> Vec<IslandAction> {
        let now = Instant::now();
        if self.input_before_layout(&ev, ctl, now) {
            return Vec::new();
        }
        if self.stale {
            self.apply_views(now);
        }
        let dpr = self.dpr;
        self.refit_page_sheet(now, &mut |s: &str, st: &TextStyle, w: Option<f32>| {
            painter.text().measure(s, st, w, dpr)
        });
        self.layout(painter, now);
        // The painter is in hand here: the sheet's body cuts and measures
        // with it, never with a closure that borrows it again.
        let mut measure =
            |s: &str, st: &TextStyle, w: Option<f32>| painter.text().measure(s, st, w, dpr);
        self.route(ev, ctl, now, &mut measure)
    }

    /// What input does before anything is laid out: a closed popup, and a
    /// press while a menu is open. Returns whether the event is spent.
    fn input_before_layout(
        &mut self,
        ev: &InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        now: Instant,
    ) -> bool {
        match *ev {
            // Escape closes the page sheet's open menu first; the sheet stays.
            _ if self.menu_takes_escape(ev) => {
                self.close_menu(ctl);
                true
            }
            InputEvent::PopupClosed { id, reason } => {
                if self.menu_closed(id, reason, now) {
                    self.sync_ctl(ctl, now);
                    ctl.request_redraw();
                }
                true
            }
            // The popup's grab still delivers presses on the Island (the
            // same client): like a native menu, such a press only closes it.
            InputEvent::PointerPress { .. } if self.menu.is_some() || self.tray_menu.is_some() => {
                self.close_menu(ctl);
                true
            }
            _ => false,
        }
    }

    /// Whether (`x`, `y`) is the bar's: on one of its capsules and not on
    /// the open page.
    fn on_bar(&self, x: f32, y: f32, now: Instant) -> bool {
        self.bar.as_ref().is_some_and(|bar| bar.contains(x, y, now))
            && !(self.state.is_open() && self.state.rect(now, self.size.0).contains(x, y))
    }

    /// [`Island::dispatch`]es a laid-out event, closes the page's menus if
    /// the page is closing, and commits the regions and a frame when
    /// anything changed.
    fn route(
        &mut self,
        ev: InputEvent,
        ctl: &mut SurfaceCtl<'_>,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Vec<IslandAction> {
        let before = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.bar.as_ref().map(BarTree::highlight),
            self.state.is_open(),
            self.sheets.revision(),
        );
        let actions = self.dispatch(ev, now, measure);
        // The page or sheet is closing: its menus go with it.
        if self.page_shown().is_none() {
            self.close_page_menus(ctl);
        }
        if self.tray_card_orphaned() {
            self.close_menu(ctl);
        }
        let after = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.bar.as_ref().map(BarTree::highlight),
            self.state.is_open(),
            self.sheets.revision(),
        );
        if before != after || !actions.is_empty() || self.stale {
            self.sync_ctl(ctl, now);
            ctl.request_redraw();
        }
        if let Some(wait) = self.sheets.wait(now) {
            ctl.redraw_after(wait);
        }
        actions
    }

    /// Sends a laid-out event where it belongs: the page, the capsule or the
    /// bar. A pointer that moves from one to another leaves the first.
    fn dispatch(
        &mut self,
        ev: InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Vec<IslandAction> {
        self.sheet_queue_input(&ev, now);
        if let InputEvent::PointerMotion { x, y }
        | InputEvent::PointerPress { x, y, .. }
        | InputEvent::PointerRelease { x, y, .. } = ev
        {
            self.pointer = (x, y);
        }
        if self.sheets.is_open() {
            if let Some(actions) = self.sheet_input(&ev, now, measure) {
                return actions;
            }
        }
        let open = self.state.is_open();
        let point = match ev {
            InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. }
            | InputEvent::Axis { x, y, .. } => Some((x, y)),
            _ => None,
        };
        let on_bar = point.is_some_and(|(x, y)| self.on_bar(x, y, now));
        // The pointer over the card holds the server's clock.
        if let Some((x, y)) = point {
            self.card_over = !on_bar && open && self.over_card(x, y);
            if !self.card_over {
                self.card_wheel.reset();
            }
        } else if matches!(ev, InputEvent::PointerLeave) {
            self.card_over = false;
            self.card_wheel.reset();
        }
        if on_bar {
            // The pointer is the bar's: the Island's tree under it ends its
            // hover and press.
            let island = if open {
                (&mut self.page_input, &mut self.page)
            } else {
                (&mut self.capsule_input, &mut self.capsule)
            };
            island.0.pointer(island.1, &InputEvent::PointerLeave, now);
        } else if point.is_some() || matches!(ev, InputEvent::PointerLeave) {
            if let Some(bar) = self.bar.as_mut() {
                bar.leave(now);
            }
        }
        let mut actions = Vec::new();
        let events = match &ev {
            _ if on_bar => {
                actions.extend(self.bar_input(&ev, open, now, measure));
                Vec::new()
            }
            InputEvent::FocusLost => {
                if open {
                    self.close(now);
                }
                Vec::new()
            }
            InputEvent::Key {
                keysym,
                modifiers,
                pressed: true,
                ..
            } if open
                && (*keysym == XKB_KEY_MENU || (*keysym == XKB_KEY_F10 && modifiers.shift)) =>
            {
                actions.extend(self.menu_key());
                Vec::new()
            }
            InputEvent::Key {
                keysym,
                pressed: true,
                ..
            } if open
                && (*keysym == XKB_KEY_DELETE || *keysym == XKB_KEY_KP_DELETE)
                && self.shows_clipboard_page() =>
            {
                // A held key repeats as `KeyRepeat`, which this arm ignores:
                // one press removes one entry, the next needs the key
                // released in between.
                if !std::mem::replace(&mut self.delete_held, true) {
                    actions.extend(self.delete_key());
                }
                Vec::new()
            }
            InputEvent::Key {
                keysym,
                pressed: false,
                ..
            } if open && (*keysym == XKB_KEY_DELETE || *keysym == XKB_KEY_KP_DELETE) => {
                self.delete_held = false;
                self.page_input.key(&mut self.page, &ev)
            }
            InputEvent::Key { .. } | InputEvent::KeyRepeat { .. } if open => {
                self.page_input.key(&mut self.page, &ev)
            }
            // Outside the page: neither where it is painted nor where it is
            // headed (a press on a card that is still growing is its own).
            InputEvent::PointerPress { x, y, .. }
                if open
                    && !self.state.rect(now, self.size.0).contains(*x, *y)
                    && !self.state.open_rect(self.size.0).contains(*x, *y) =>
            {
                self.close(now);
                Vec::new()
            }
            // The card's right and middle clicks and wheel up.
            InputEvent::PointerRelease { x, y, button, .. }
                if open
                    && (*button == BTN_RIGHT || *button == BTN_MIDDLE)
                    && self.over_card(*x, *y) =>
            {
                let gesture = if *button == BTN_RIGHT {
                    NoticeGesture::Right
                } else {
                    NoticeGesture::Middle
                };
                actions.extend(self.card_actions(gesture, now));
                Vec::new()
            }
            InputEvent::Axis { x, y, vertical, .. }
                if open && *vertical < 0.0 && self.over_card(*x, *y) =>
            {
                // One scroll gesture closes one notice, however many axis
                // events it sends.
                if self.card_wheel.up(*vertical, now) {
                    actions.extend(self.card_actions(NoticeGesture::WheelUp, now));
                }
                Vec::new()
            }
            // The tray rows' other buttons: the page's pointer handling
            // only knows the left one.
            InputEvent::PointerRelease { x, y, button, .. } if open && *button != BTN_LEFT => {
                actions.extend(self.other_button(*x, *y, *button));
                Vec::new()
            }
            _ if open => self.page_input.pointer(&mut self.page, &ev, now),
            // Lent: the notices surface owns the capsule.
            _ if !self.paints_capsule(now) => Vec::new(),
            _ => self.capsule_input.pointer(&mut self.capsule, &ev, now),
        };
        for event in events {
            if let UiEvent::Click(key) = event {
                if self.card_has(key) {
                    actions.extend(self.card_actions(NoticeGesture::Left(key), now));
                    continue;
                }
            }
            let tray = self.tray_actions(&event);
            if !tray.is_empty() {
                actions.extend(tray);
            } else if let Some(action) = self.on_event(event, now) {
                actions.push(action);
            }
        }
        // A Tray row's menu was asked for last, not a bar icon's.
        if !on_bar
            && actions
                .iter()
                .any(|a| matches!(a, IslandAction::OpenTrayMenu { .. }))
        {
            self.bar_menu_asked = false;
        }
        actions
    }

    /// A pointer event on the bar: what its pieces ask for, as the Island's
    /// actions. A page asked for opens (or the open page slides to it).
    /// While a page is open, a press on the bar but on none of its pieces
    /// is a press outside the page and closes it; a piece's click, middle
    /// or right click closes it too unless it asked for a page; the wheel
    /// never does. An open window follows the same rules: a press on the bar but
    /// on no piece is a press outside, and a piece asking for a sheet leaves
    /// the window's own switch or close to the app's answer
    /// ([`Island::open_level_sheet`]) except for the open piece itself, which
    /// closes it.
    fn bar_input(
        &mut self,
        ev: &InputEvent,
        open: bool,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Vec<IslandAction> {
        let Some(input) = self.with_bar(|bar, cx| bar.pointer(ev, cx, now)) else {
            return Vec::new();
        };
        let sheet = self.sheets.is_open();
        if !input.on_piece && matches!(ev, InputEvent::PointerPress { .. }) {
            if open {
                self.close(now);
            }
            if sheet {
                self.sheets.drop_queue();
                self.close_sheet(now);
            }
        }
        self.apply_asked(input.asked, open, now, measure)
    }

    /// What the pieces answered to the gestures asked of them: the Island's
    /// actions, a sheet or page opened, and what a plain gesture closes. The
    /// bar's own input ends here.
    fn apply_asked(
        &mut self,
        asked: Vec<(Key, Gesture, PieceAction)>,
        open: bool,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Vec<IslandAction> {
        let sheet = self.sheets.is_open();
        let mut out = Vec::new();
        for (key, gesture, action) in asked {
            let scroll = matches!(gesture, Gesture::Scroll(..));
            let request = matches!(action, PieceAction::OpenPage(_) | PieceAction::Sheet(_));
            let action = match gesture {
                Gesture::Scroll(Axis::Vertical, steps) if key == workspaces_piece::key() => {
                    self.workspace_wheel_action(steps, now)
                }
                _ => action,
            };
            match action {
                PieceAction::None => {}
                PieceAction::OpenPage(page) => {
                    out.extend(self.sheet_request(key, SheetKind::Page(page), now, measure));
                }
                PieceAction::Sheet(kind) => out.extend(self.sheet_request(key, kind, now, measure)),
                PieceAction::FocusWorkspace(id) => out.push(IslandAction::FocusWorkspace(id)),
                PieceAction::Tray(command) => out.push(IslandAction::Tray(command)),
                PieceAction::TrayMenu(item) => {
                    if let Some(row) = self.bar.as_ref().and_then(|bar| bar.node_rect(key)) {
                        self.bar_menu_asked = true;
                        self.overflow_menu_asked = None;
                        out.push(IslandAction::OpenTrayMenu {
                            item,
                            row,
                            keyboard: false,
                            at: output_point(self.pointer),
                        });
                    }
                }
                PieceAction::TrayContext(item) => {
                    let (x, y) = output_point(self.pointer);
                    out.push(IslandAction::Tray(TrayCommand::ContextMenu(item, x, y)));
                }
                PieceAction::Audio(action) => out.push(IslandAction::Audio(action)),
                PieceAction::Brightness(action) => out.push(IslandAction::Brightness(action)),
                PieceAction::Notification(command) => out.push(IslandAction::Notification(command)),
                PieceAction::Connectivity(action) => out.push(IslandAction::Connectivity(action)),
                PieceAction::RestoreWindow(window_id) => out.push(IslandAction::RestoreWindow {
                    window_id,
                    output: self.output.clone(),
                    bubble: self.bar.as_ref().and_then(|bar| bar.node_rect(key)),
                }),
                PieceAction::OpenSelector => out.push(IslandAction::OpenSelector),
                PieceAction::Media(command) => out.push(IslandAction::Media(command)),
            }
            if open && !scroll && self.state.is_open() {
                self.close(now);
            }
            if sheet && !scroll && !request && self.sheets.is_open() {
                self.close_sheet(now);
            }
        }
        out
    }

    /// What `steps` wheel notches over the workspaces piece ask for: the
    /// workspace that far from the last target of this burst, else from the
    /// active one. A notch that would not move asks for nothing.
    fn workspace_wheel_action(&mut self, steps: i32, now: Instant) -> PieceAction {
        let Some(model) = self.model.as_ref() else {
            return PieceAction::None;
        };
        let rows = rows_for(model, &self.output);
        let pending = self
            .workspace_wheel
            .filter(|&(_, at)| now.duration_since(at) < WORKSPACE_WHEEL_BURST)
            .map(|(id, _)| id)
            // The model caught up: it is the truth again.
            .filter(|&id| !rows.iter().any(|r| r.id == id && r.active));
        match workspaces_piece::step_from(&rows, pending, steps) {
            Some(id) => {
                self.workspace_wheel = Some((id, now));
                PieceAction::FocusWorkspace(id)
            }
            None => PieceAction::None,
        }
    }

    /// Opens the overflow `menu` of `size` below the overflow button at
    /// `more` (the rect [`IslandAction::OpenMenu`] carried). The runtime
    /// grabs with the serial of the press or key that chose the button.
    /// A second request while one is open is ignored.
    pub fn open_menu(
        &mut self,
        more: Rect,
        size: (f32, f32),
        menu: Box<dyn PopupShell>,
        ctl: &mut SurfaceCtl<'_>,
    ) {
        if self.menu.is_some() || self.page_shown().is_none() {
            return;
        }
        let id = ctl.open_popup(menu::placement(more), size, menu);
        tracing::debug!(output = %self.output, ?more, popup = id.0, "island: menu opened");
        self.menu = Some(id);
    }

    /// Opens the first level of a tray item's menu, of `size`, below its
    /// page `row` or bar icon (the rect [`IslandAction::OpenTrayMenu`]
    /// carried), and returns its popup. `None` while another menu is open,
    /// or, for a page row's menu, while the Tray page is not the one shown
    /// (the person switched pages before the host answered).
    pub fn open_tray_menu(
        &mut self,
        row: Rect,
        size: (f32, f32),
        menu: Box<dyn PopupShell>,
        ctl: &mut SurfaceCtl<'_>,
    ) -> Option<PopupId> {
        let on_bar = self.bar_menu_asked && self.bar.is_some();
        if self.menu.is_some() || self.tray_menu.is_some() || !(on_bar || self.on_tray_page()) {
            return None;
        }
        let id = ctl.open_popup(crate::tray::menu::placement(&self.theme, row), size, menu);
        tracing::debug!(output = %self.output, popup = id.0, on_bar, "island: tray menu opened");
        self.tray_menu = Some(id);
        self.tray_menu_on_bar = on_bar;
        let asked = self.overflow_menu_asked.take();
        self.tray_menu_from_window = asked.filter(|_| on_bar);
        self.bar_menu_asked = false;
        Some(id)
    }

    /// Whether the tray card hangs from an overflow window that is no longer
    /// open, or that no longer lists the card's app (it was shown, or it
    /// went): the card goes with it.
    pub(crate) fn tray_card_orphaned(&self) -> bool {
        let Some(item) = self.tray_menu_from_window.as_ref() else {
            return false;
        };
        let listed = self.sheets.is_open()
            && self.sheets.kind() == Some(SheetKind::TrayOverflow)
            && self.overflow_rows().iter().any(|(_, row)| row.id == *item);
        self.tray_menu.is_some() && !listed
    }

    /// Whether a tray item's menu hangs from this Island right now.
    #[must_use]
    pub fn tray_menu_open(&self) -> bool {
        self.tray_menu.is_some()
    }

    /// Whether the page lists tray items right now.
    fn on_tray_page(&self) -> bool {
        self.state.is_open() && self.state.shown_page() == PageId::TRAY
    }

    /// The tray row `key` names, with its index.
    fn tray_row(&self, key: Key) -> Option<(usize, &TrayRow)> {
        if !self.on_tray_page() {
            return None;
        }
        let index = tray_page::index_of(key, self.tray_rows.len())?;
        Some((index, self.tray_rows.get(index)?))
    }

    /// The page's rect of the tray row at `index`.
    fn tray_row_rect(&self, index: usize) -> Option<Rect> {
        let id = self.page.node_by_key(tray_page::row_key(index))?;
        Some(self.page.rect(id))
    }

    /// What a click, Enter, Space or the wheel on a tray row asks of the
    /// host. A click and the keyboard's activation both activate the item;
    /// the keyboard's way to the menu is the Menu key or Shift+F10
    /// ([`Island::menu_key`]), as the pointer's is the right button.
    fn tray_actions(&mut self, event: &UiEvent) -> Vec<IslandAction> {
        match event {
            UiEvent::Click(key) | UiEvent::Activate(key) => {
                let keyboard = matches!(event, UiEvent::Activate(_));
                self.tray_row(*key)
                    .map(|(index, row)| {
                        // An item that is only a menu opens it on the click.
                        if row.item_is_menu {
                            self.menu_for(index, row, keyboard, self.pointer)
                        } else {
                            Some(IslandAction::Tray(TrayCommand::Activate(row.id.clone())))
                        }
                    })
                    .into_iter()
                    .flatten()
                    .collect()
            }
            UiEvent::Scroll(key, horizontal, vertical) => {
                self.scroll_actions(*key, *horizontal, *vertical)
            }
            UiEvent::HoverChanged(..)
            | UiEvent::Escape
            | UiEvent::FieldChanged(_)
            | UiEvent::SliderChanged(..) => Vec::new(),
        }
    }

    fn open_menu_action(
        &self,
        item: ItemId,
        index: usize,
        keyboard: bool,
        at: (i32, i32),
    ) -> Option<IslandAction> {
        Some(IslandAction::OpenTrayMenu {
            item,
            row: self.tray_row_rect(index)?,
            keyboard,
            at,
        })
    }

    /// What the menu of the tray row at `index` asks for: its dbusmenu when
    /// it serves one, else the item's own through `ContextMenu`. `pointer`
    /// is where the pointer was; the keyboard's menu is placed at the row's
    /// centre instead.
    fn menu_for(
        &self,
        index: usize,
        row: &TrayRow,
        keyboard: bool,
        pointer: (f32, f32),
    ) -> Option<IslandAction> {
        let rect = self.tray_row_rect(index)?;
        let at = output_point(if keyboard {
            (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
        } else {
            pointer
        });
        // An item with an `Id` also gets the shell's own card (hide or show);
        // the host asks the app for its own menu when it has no dbusmenu.
        if row.has_menu || !row.sni_id.is_empty() {
            return self.open_menu_action(row.id.clone(), index, keyboard, at);
        }
        Some(IslandAction::Tray(TrayCommand::ContextMenu(
            row.id.clone(),
            at.0,
            at.1,
        )))
    }

    /// A wheel turn of (`horizontal`, `vertical`) logical px over the row
    /// `key`: whole steps of 15 px per axis, the rest kept for the next
    /// turn on the same row. Items expect Qt's `angleDelta` sign (what
    /// Plasma sends them): positive for a wheel rolled away from the
    /// person and for one tilted left, which is the opposite of the
    /// Wayland axis on both axes, so both are negated. The horizontal
    /// direction is listed for a real item in VAL-CEL-NOCTALIA.
    fn scroll_actions(&mut self, key: Key, horizontal: f32, vertical: f32) -> Vec<IslandAction> {
        let Some((_, row)) = self.tray_row(key) else {
            return Vec::new();
        };
        let id = row.id.clone();
        self.scroll_item(key, id, horizontal, vertical)
    }

    /// [`Island::scroll_actions`] for the item `id` whose row is `key`, on
    /// the page or in the overflow window.
    fn scroll_item(
        &mut self,
        key: Key,
        id: ItemId,
        horizontal: f32,
        vertical: f32,
    ) -> Vec<IslandAction> {
        if self.scroll.row != Some(key) {
            self.scroll = ScrollRemainder {
                row: Some(key),
                ..ScrollRemainder::default()
            };
        }
        let px = |v: f32| if v.is_finite() { v.round() as i32 } else { 0 };
        let up = scroll_steps(px(-vertical), &mut self.scroll.vertical);
        let left = scroll_steps(px(-horizontal), &mut self.scroll.horizontal);
        let mut out = Vec::new();
        if up != 0 {
            out.push(IslandAction::Tray(TrayCommand::Scroll(
                id.clone(),
                up,
                Orientation::Vertical,
            )));
        }
        if left != 0 {
            out.push(IslandAction::Tray(TrayCommand::Scroll(
                id,
                left,
                Orientation::Horizontal,
            )));
        }
        out
    }

    /// A middle click on a tray row activates the item secondarily; a right
    /// click asks for its menu.
    fn other_button(&self, x: f32, y: f32, button: u32) -> Vec<IslandAction> {
        if !self.on_tray_page() {
            return Vec::new();
        }
        let hit = (0..self.tray_rows.len().min(tray_page::MAX_ROWS))
            .find(|&i| self.tray_row_rect(i).is_some_and(|r| r.contains(x, y)));
        let Some(index) = hit else {
            return Vec::new();
        };
        let row = &self.tray_rows[index];
        match button {
            BTN_MIDDLE => vec![IslandAction::Tray(TrayCommand::SecondaryActivate(
                row.id.clone(),
            ))],
            BTN_RIGHT => self
                .menu_for(index, row, false, (x, y))
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The Menu key (or Shift+F10) on the focused tray row: its menu.
    fn menu_key(&self) -> Vec<IslandAction> {
        let Some(id) = self.page.highlight().focused else {
            return Vec::new();
        };
        match self.tray_row(self.page.key_of(id)) {
            Some((index, row)) => self
                .menu_for(index, row, true, self.pointer)
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Gives the pixmaps uploaded for the tray's icons back to `painter`
    /// (the Island's output went away).
    pub fn release_icons(&mut self, painter: &mut Painter) {
        self.icons.release_all(painter);
        self.clip_textures.release_all(painter);
        self.clipboard = ClipboardState::default();
        self.clip_panel = clipboard_page::ClipboardPanel::default();
        self.release_covers(painter);
        self.tray_rows.clear();
        self.tray_generation = 0;
        self.invalidate_bar();
    }

    /// Takes the media state the app keeps: whether the duty is `on`, the
    /// `players` on the bus (sorted by bus name) and the bus name of the
    /// shown one. Covers of players that left, or whose art changed, are
    /// released to `pixmaps`. Returns whether a frame is worth drawing:
    /// the bar redraws only when what it shows changed (a player appeared
    /// or left, play or pause, the line), the page only while it is on
    /// screen, so a position tick at rest costs nothing.
    pub fn set_media(
        &mut self,
        pixmaps: &mut impl Pixmaps,
        duty_on: bool,
        players: &[PlayerView],
        active: Option<&str>,
    ) -> bool {
        self.covers.retain(|bus, (url, id)| {
            let current = duty_on
                && players
                    .iter()
                    .any(|p| p.bus_name == *bus && !p.art_url.is_empty() && p.art_url == *url);
            if !current {
                pixmaps.release(*id);
            }
            current
        });
        let mut next = MediaState {
            players: players.to_vec(),
            active: active.map(str::to_owned),
            cover: None,
            duty_on,
            unavailable: duty_on && self.media.unavailable,
        };
        next.cover = self.cover_of(&next);
        self.replace_media(next)
    }

    /// Takes whether the session bus cannot be followed right now: the
    /// views stop presenting the last players as live (a note on the page,
    /// no piece on the bar). Returns whether a frame is worth drawing.
    pub fn set_media_unavailable(&mut self, unavailable: bool) -> bool {
        let unavailable = unavailable && self.media.duty_on;
        if unavailable == self.media.unavailable {
            return false;
        }
        let mut next = self.media.clone();
        next.unavailable = unavailable;
        self.replace_media(next)
    }

    /// Takes the decoded `cover` of the player `bus_name` whose art was
    /// `art_url` (`None`: the art could not be decoded). A cover for a
    /// player that is gone, or for art it no longer names, is dropped.
    /// Uploads to `pixmaps`, releasing the previous texture. Returns
    /// whether a frame is worth drawing, as [`Island::set_media`] does.
    pub fn set_media_cover(
        &mut self,
        pixmaps: &mut impl Pixmaps,
        bus_name: &str,
        art_url: &str,
        cover: Option<&Cover>,
    ) -> bool {
        let wanted = self.media.duty_on
            && self
                .media
                .players
                .iter()
                .any(|p| p.bus_name == bus_name && p.art_url == art_url);
        if !wanted {
            return false;
        }
        // The loop repeats every cover of the snapshot at each players event
        // (about once a second while anything plays): a texture already held
        // for this art is kept, not uploaded again.
        if cover.is_some()
            && self
                .covers
                .get(bus_name)
                .is_some_and(|(url, _)| url == art_url)
        {
            return false;
        }
        if let Some((_, old)) = self.covers.remove(bus_name) {
            pixmaps.release(old);
        }
        let uploaded = cover.and_then(|c| {
            let argb = argb_of(c)?;
            pixmaps.upload(&argb, c.width, c.height)
        });
        if let Some(id) = uploaded {
            self.covers
                .insert(bus_name.to_owned(), (art_url.to_owned(), id));
        }
        let mut next = self.media.clone();
        next.cover = self.cover_of(&next);
        self.replace_media(next)
    }

    /// The texture of `state`'s shown player's cover.
    fn cover_of(&self, state: &MediaState) -> Option<PixmapId> {
        let bus = &state.shown()?.bus_name;
        self.covers.get(bus).map(|(_, id)| *id)
    }

    /// Becomes `next`; whether a frame is worth drawing.
    fn replace_media(&mut self, next: MediaState) -> bool {
        if next == self.media {
            return false;
        }
        let bar_changed =
            pieces::media::bar_signature(&next) != pieces::media::bar_signature(&self.media);
        self.media = next;
        let on_page = self.page_shown() == Some(PageId::MEDIA);
        if bar_changed {
            self.invalidate_bar();
        }
        if on_page {
            self.stale = true;
        }
        bar_changed || on_page
    }

    /// Gives every cover texture back to `pixmaps`.
    fn release_covers(&mut self, pixmaps: &mut impl Pixmaps) {
        for (_, (_, id)) in self.covers.drain() {
            pixmaps.release(id);
        }
        if self.media.cover.take().is_some() {
            self.invalidate_bar();
            self.stale = true;
        }
    }

    /// Closes the menus from the Island's side; the runtime still reports
    /// `PopupClosed`, which then finds nothing to do. The polkit prompt calls
    /// it on every Island, a bar icon's menu included: a popup with a grab
    /// could otherwise keep the keyboard from the card (N-25).
    pub(crate) fn close_menu(&mut self, ctl: &mut SurfaceCtl<'_>) {
        self.tray_menu_on_bar = false;
        self.tray_menu_from_window = None;
        for id in [self.menu.take(), self.tray_menu.take()]
            .into_iter()
            .flatten()
        {
            ctl.close_popup(id);
            ctl.request_redraw();
        }
    }

    /// Closes the menus that hang from the page: the overflow menu and a
    /// Tray row's; a bar icon's menu does not depend on the page.
    fn close_page_menus(&mut self, ctl: &mut SurfaceCtl<'_>) {
        let tray = if self.tray_menu_on_bar {
            None
        } else {
            self.tray_menu.take()
        };
        for id in [self.menu.take(), tray].into_iter().flatten() {
            ctl.close_popup(id);
            ctl.request_redraw();
        }
    }

    /// Popup `id` is gone. A compositor dismissal is a click outside the
    /// shell (another window, another output), which for the page is a
    /// click outside: it closes too; the keyboard's leave that came with it
    /// went to the popup, so no `FocusLost` would close it otherwise. A
    /// press on the Island or the menu's own choice or Escape closes only
    /// the menu. Returns whether the popup was this Island's menu.
    fn menu_closed(&mut self, id: PopupId, reason: CloseReason, now: Instant) -> bool {
        if self.menu == Some(id) {
            self.menu = None;
        } else if self.tray_menu == Some(id) {
            self.tray_menu = None;
            self.tray_menu_on_bar = false;
            self.tray_menu_from_window = None;
        } else {
            return false;
        }
        if reason == CloseReason::Dismissed {
            if self.state.is_open() {
                self.close(now);
            } else if self.sheets.page().is_some()
                || self.sheets.kind() == Some(SheetKind::TrayOverflow)
            {
                // A page sheet closes as the page does, and so does the
                // tray's window a card hangs from.
                self.sheets.drop_queue();
                self.close_sheet(now);
            }
        }
        true
    }

    /// The menu's reopen row: the open page shrinks to the capsule at once
    /// and grows again, its focus and hover gone.
    pub fn reopen(&mut self, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        self.reopen_at(now);
        self.sync_ctl(ctl, now);
        ctl.request_redraw();
    }

    fn reopen_at(&mut self, now: Instant) {
        if self.reopen_sheet(now) {
            return;
        }
        let page = self.state.shown_page();
        self.close(now);
        // A single step back to the capsule, whatever the motion setting.
        self.state.close(now, true);
        self.open(page, now);
    }

    /// What one of the trees' events does: a click or Enter acts on its
    /// node, Escape closes. The overflow button remembers whether the
    /// keyboard chose it, so the menu can start with a focused row.
    fn on_event(&mut self, event: UiEvent, now: Instant) -> Option<IslandAction> {
        let (key, keyboard) = match event {
            UiEvent::Click(key) => (key, false),
            UiEvent::Activate(key) => (key, true),
            UiEvent::Escape => {
                self.close(now);
                return None;
            }
            UiEvent::HoverChanged(..)
            | UiEvent::Scroll(..)
            | UiEvent::FieldChanged(_)
            | UiEvent::SliderChanged(..) => return None,
        };
        match self.activate(key, now)? {
            IslandAction::OpenMenu { more, .. } => Some(IslandAction::OpenMenu { more, keyboard }),
            action => Some(action),
        }
    }

    /// Runs an Island verb of `org.celestina.Shell1`; other verbs are not
    /// the Island's and change nothing.
    pub fn command(&mut self, verb: &Verb, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        if !self.command_at(verb, now) {
            return;
        }
        tracing::debug!(
            output = %self.output,
            page = ?self.state.shown_page(),
            open = self.state.is_open(),
            "island: command applied"
        );
        self.sync_ctl(ctl, now);
        ctl.request_redraw();
    }

    /// [`Island::command`] at `now`, without the surface: whether the verb was
    /// the Island's.
    fn command_at(&mut self, verb: &Verb, now: Instant) -> bool {
        match *verb {
            Verb::IslandToggle => {
                if self.state.is_open() || self.sheets.is_open() {
                    self.close_everything(now);
                } else {
                    self.open(self.state.shown_page(), now);
                }
            }
            Verb::IslandOpen(page) => self.open(page, now),
            Verb::IslandClose => self.close_everything(now),
            Verb::LauncherToggle
            | Verb::Overview(_)
            | Verb::Shortcuts(_)
            | Verb::NotificationsToggle
            | Verb::NotificationsDismissAll
            | Verb::NotificationsClear
            | Verb::WallpaperSet { .. }
            | Verb::WallpaperNext(_)
            | Verb::WallpaperPrev(_)
            | Verb::WallpaperMode(_)
            | Verb::WallpaperRetry
            | Verb::Caffeine(_)
            | Verb::NightLight(_)
            | Verb::Session(_)
            | Verb::Lock
            | Verb::LockAndSuspend
            | Verb::Clipboard(_)
            | Verb::Volume(_)
            | Verb::Microphone(_)
            | Verb::Brightness { .. }
            | Verb::BubblesSelect
            | Verb::BubblesMinimize
            | Verb::PhoneMedia(_)
            | Verb::Quit => return false,
        }
        true
    }
}

impl Redraws for SurfaceCtl<'_> {
    fn request_redraw(&mut self) {
        SurfaceCtl::request_redraw(self);
    }

    fn redraw_after_withdrawable(&mut self, wait: Duration) {
        SurfaceCtl::redraw_after_withdrawable(self, wait);
    }

    fn withdraw_redraw(&mut self) {
        SurfaceCtl::withdraw_redraw(self);
    }
}

#[cfg(test)]
mod tests {
    use super::pages::{
        activity, bluetooth as bluetooth_page, clock as clock_page, media as media_page,
        network as network_page, power as power_page, sysmon as sysmon_page, tabs,
    };
    use super::*;
    use crate::providers::niri::{Window, Workspace};
    use celestina_shell::audio::Direction;
    use celestina_wayland::{CloseReason, PopupId};

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

    // The bar (CEL-2-A, parity spec §6).

    /// Lays every tree out with 7 px per char, 16 px tall.
    fn lay_fake(island: &mut Island, now: Instant) {
        island.refit_page_sheet(now, &mut fake_measure);
        island.layout_with(now, &mut |tree: &mut Tree, room: (f32, f32)| {
            tree.layout_measured(room, fake_measure);
        });
    }

    /// What `input` does after the popups: the views, the layout, then the
    /// event.
    fn feed(island: &mut Island, ev: InputEvent, now: Instant) -> Vec<IslandAction> {
        feed_with(island, ev, now, &mut fake_measure)
    }

    /// 7 px per char, 16 px tall: `lay_fake`'s measure.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    /// [`feed`] with the measure the input has in hand.
    fn feed_with(
        island: &mut Island,
        ev: InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> Vec<IslandAction> {
        if island.stale {
            island.apply_views(now);
        }
        lay_fake(island, now);
        island.dispatch(ev, now, measure)
    }

    fn press(x: f32, y: f32, button: u32) -> InputEvent {
        InputEvent::PointerPress {
            x,
            y,
            button,
            serial: 1,
        }
    }

    fn release(x: f32, y: f32, button: u32) -> InputEvent {
        InputEvent::PointerRelease {
            x,
            y,
            button,
            serial: 2,
        }
    }

    /// A motion, a press and a release of `button` at (`x`, `y`).
    fn click_at(
        island: &mut Island,
        (x, y): (f32, f32),
        button: u32,
        now: Instant,
    ) -> Vec<IslandAction> {
        let mut out = feed(island, InputEvent::PointerMotion { x, y }, now);
        out.extend(feed(island, press(x, y, button), now));
        out.extend(feed(island, release(x, y, button), now));
        out
    }

    /// The bar's zone `rect`, which the test expects to be placed.
    fn zone(rect: Option<Rect>) -> Rect {
        match rect {
            Some(r) => r,
            None => panic!("the zone is not placed"),
        }
    }

    /// The start zone's rect.
    fn start_zone(island: &Island) -> Rect {
        zone(
            island
                .bar
                .as_ref()
                .and_then(BarTree::placement)
                .and_then(|p| p.start),
        )
    }

    /// Clear-all folds the rows one after another: row `i` leaves `i` steps
    /// after the press, and the empty list follows the last one.
    #[test]
    fn clear_all_staggers_the_rows_by_30_ms() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let row = |id| HistoryRow {
            id,
            summary: format!("n{id}"),
            ..HistoryRow::default()
        };
        let mut island = Island::new("DP-1", (1000.0, 700.0));
        island.history = (1..=4).map(row).collect();
        island.presented = island.history.clone();
        let key = pages::notifications::clear_all_key();
        let action = island.page_action(PageId::NOTIFICATIONS, key, t0);
        assert_eq!(action, Some(IslandAction::ClearNotifications));
        // The worker forgets everything at once; the page still folds.
        island.history.clear();
        let ids = |island: &mut Island, at| -> Vec<u32> {
            island.presented_at(at).iter().map(|r| r.id).collect()
        };
        assert_eq!(ids(&mut island, t0), [2, 3, 4], "row 1 leaves at 0 ms");
        assert_eq!(ids(&mut island, ms(29)), [2, 3, 4]);
        assert_eq!(ids(&mut island, ms(30)), [3, 4]);
        assert_eq!(ids(&mut island, ms(60)), [4]);
        assert_eq!(ids(&mut island, ms(90)), Vec::<u32>::new(), "the last one");
        assert!(island.clearing.is_none(), "then the empty list shows");
        // Reduced motion: the rows go at once.
        island.history = (1..=4).map(row).collect();
        island.presented = island.history.clone();
        island.set_reduced_motion(true);
        island.page_action(PageId::NOTIFICATIONS, key, t0);
        assert!(island.clearing.is_none());
    }

    /// Only the rows the page builds fold, and no row comes back between the
    /// end of the fold and the worker's empty snapshot.
    #[test]
    fn clear_all_folds_the_shown_rows_and_keeps_every_cleared_row_out_until_the_worker_answers() {
        let t0 = Instant::now();
        let row = |id| HistoryRow {
            id,
            summary: format!("n{id}"),
            ..HistoryRow::default()
        };
        let mut island = Island::new("DP-1", (1000.0, 400.0));
        island.history = (1..=30).map(row).collect();
        island.presented = island.history.clone();
        let key = pages::notifications::clear_all_key();
        assert_eq!(
            island.page_action(PageId::NOTIFICATIONS, key, t0),
            Some(IslandAction::ClearNotifications)
        );
        let folding = island.clearing.as_ref().map_or(0, |(_, rows)| rows.len());
        assert!(folding > 0 && folding < 30, "folds {folding} of 30");
        assert!(island
            .clearing
            .as_ref()
            .is_some_and(|(_, rows)| rows.iter().all(|r| r.folding)));
        // The fold ends after the shown rows only.
        let end = t0 + CLEAR_STEP * u32::try_from(folding).unwrap_or(0);
        // The worker has not answered: its history still holds the rows.
        assert!(island.presented_at(end).is_empty(), "none comes back");
        assert!(island.presented_at(end).is_empty());
        // A notification that arrives meanwhile shows; the answer drops the
        // cleared ids from the memory.
        island.history.push(row(99));
        assert_eq!(island.presented_at(end).len(), 1);
        island.history = vec![row(99)];
        assert_eq!(island.presented_at(end).len(), 1);
        assert!(island.cleared.is_empty());
    }

    fn centre(r: Rect) -> (f32, f32) {
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    }

    /// The rect of the bar's node `key`.
    fn bar_rect(island: &Island, key: Key) -> Rect {
        match island.bar.as_ref().and_then(|bar| bar.node_rect(key)) {
            Some(r) => r,
            None => panic!("no bar node {key:?}"),
        }
    }

    /// An Island on a surface `w` wide with the bar on (the default layout),
    /// three workspaces (the second active) and one tray item with a menu,
    /// laid out at rest.
    fn bar_island(w: f32) -> (Island, Instant) {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (w, 560.0));
        island.model = Some(model("DP-1", 2));
        island.rows = rows_for(&model("DP-1", 2), "DP-1");
        island.tray_rows = vec![TrayRow {
            id: ItemId {
                bus_name: ":1.7".to_owned(),
                path: "/StatusNotifierItem".to_owned(),
            },
            title: "item".to_owned(),
            has_menu: true,
            ..TrayRow::default()
        }];
        island.set_bar(&BarLayout::default());
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        (island, t0)
    }

    /// Whether the pixel centre (`x` + 0.5, `y` + 0.5) is inside the rounded
    /// rect `shape`.
    fn painted(shape: Shape, x: i32, y: i32) -> bool {
        let (rect, radius) = shape;
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        if !rect.contains(px, py) {
            return false;
        }
        let rad = radius.min(rect.w / 2.0).min(rect.h / 2.0);
        let cx = px.clamp(rect.x + rad, rect.x + rect.w - rad);
        let cy = py.clamp(rect.y + rad, rect.y + rect.h - rad);
        (px - cx).powi(2) + (py - cy).powi(2) <= rad * rad
    }

    /// No painted pixel without a region, no region more than one band
    /// away from a painted shape, over the rows `0..height`.
    fn assert_regions_follow(shapes: &[Shape], bands: &[Rect], width: i32, height: i32) {
        let step = state::SILHOUETTE_STEP;
        for y in 0..height {
            for x in 0..width {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let covered = bands.iter().any(|b| b.contains(px, py));
                if shapes.iter().any(|&s| painted(s, x, y)) {
                    assert!(covered, "painted ({x},{y}) has no region");
                } else if covered {
                    let near = shapes.iter().any(|&(r, _)| {
                        px >= r.x - step
                            && py >= r.y - step
                            && px < r.x + r.w + step
                            && py < r.y + r.h + step
                    });
                    assert!(near, "region over ({x},{y}) far from anything painted");
                }
            }
        }
    }

    #[test]
    fn with_every_zone_empty_the_island_is_the_capsule_alone() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (1920.0, 560.0));
        island.rows = rows_for(&model("DP-1", 1), "DP-1");
        island.set_bar(&BarLayout {
            start: Vec::new(),
            centre: Vec::new(),
            end: Vec::new(),
            ..BarLayout::default()
        });
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        // No Workspaces piece placed: the capsule keeps its dots.
        assert!(island.capsule.node_by_key(Key::named("dots")).is_some());
        let radius = island.theme.radius_lg;
        assert_eq!(
            island.shapes(t0),
            vec![(island.state.rect(t0, 1920.0), radius)]
        );
        // The regions are the Island's silhouette alone.
        assert_eq!(
            silhouette::union(&island.shapes(t0)),
            island.state.silhouette(t0, 1920.0, radius)
        );
        // Nothing on the surface routes to a bar.
        assert!(!island.on_bar(10.0, 20.0, t0));
    }

    #[test]
    fn on_the_wide_surface_the_pages_keep_their_strip_sizes() {
        let (island, _) = bar_island(1920.0);
        assert_eq!(
            island.state.max_page_size(),
            (
                STRIP_W - 2.0 * state::PAGE_MARGIN,
                560.0 - state::TOP_GAP - state::PAGE_MARGIN
            )
        );
    }

    #[test]
    fn the_seconds_clock_and_its_timer_exist_only_while_a_page_is_open() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.stale = false;
        // Closed: no timer, no text, nothing marked stale.
        assert_eq!(island.tick_seconds(t0, 3_723, 250_000_000), None);
        assert!(island.clock_seconds.is_empty() && !island.stale);
        // Open: one timer for the next second, the text, a rebuild.
        island.open(PageId::ACTIVITY, t0);
        island.stale = false;
        let wait = island.tick_seconds(t0, 3_723, 250_000_000);
        assert_eq!(wait, Some(Duration::from_millis(750) + clock::TICK_MARGIN));
        assert_eq!(island.clock_seconds.len(), 8);
        assert!(island.stale);
        // Not re-armed before its deadline; an unchanged second rebuilds nothing.
        island.stale = false;
        let soon = t0 + Duration::from_millis(100);
        assert_eq!(island.tick_seconds(soon, 3_723, 350_000_000), None);
        assert!(!island.stale);
        // A new second changes the text; the next boundary re-arms.
        let later = t0 + Duration::from_millis(900);
        assert!(island.tick_seconds(later, 3_724, 50_000_000).is_some());
        assert!(island.stale);
        // Closing drops the timer.
        island.close(later);
        assert_eq!(island.tick_seconds(later, 3_725, 0), None);
        // Reopened at once, it arms afresh instead of waiting out the old deadline.
        island.open(PageId::ACTIVITY, later);
        assert!(island.tick_seconds(later, 3_725, 0).is_some());
    }

    /// A surface's one redraw timer as the runtime keeps it
    /// (`celestina_wayland`'s `RedrawTimer`): an earlier deadline replaces
    /// the armed one, a later one is dropped, and firing leaves nothing.
    #[derive(Debug, Default)]
    struct EarliestTimer(Option<Instant>);

    impl EarliestTimer {
        fn arm(&mut self, at: Instant) {
            if self.0.is_none_or(|armed| at < armed) {
                self.0 = Some(at);
            }
        }
    }

    /// The wall clock at the test's `t0`: a minute boundary.
    const WALL_T0: Duration = Duration::from_secs(16_667 * 60);

    /// One frame at `t0 + t`, its clock's waits armed on `timer`.
    fn clock_frame(island: &mut Island, timer: &mut EarliestTimer, t0: Instant, t: Duration) {
        let (now, wall) = (t0 + t, WALL_T0 + t);
        let waits = island.clock_waits(now, wall.as_secs(), wall.subsec_nanos());
        for wait in waits.into_iter().flatten() {
            timer.arm(now + wait);
        }
    }

    /// Fires `timer` and draws the frame it wakes, until `until` after `t0`.
    fn run_timer(island: &mut Island, timer: &mut EarliestTimer, t0: Instant, until: Duration) {
        while let Some(deadline) = timer.0 {
            let t = deadline.duration_since(t0);
            if t > until {
                break;
            }
            timer.0 = None;
            clock_frame(island, timer, t0, t);
        }
    }

    /// The minute the wall clock reads `t` after `t0`.
    fn minute_at(t: Duration) -> u64 {
        (WALL_T0 + t).as_secs() / 60
    }

    #[test]
    fn closing_a_page_leaves_the_minute_timer_armed() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.set_reduced_motion(true);
        let mut timer = EarliestTimer::default();
        clock_frame(&mut island, &mut timer, t0, Duration::ZERO);
        run_timer(&mut island, &mut timer, t0, secs(10));
        // A page opens: its seconds timer displaces the minute's.
        island.open(PageId::ACTIVITY, t0 + secs(10));
        clock_frame(&mut island, &mut timer, t0, secs(10));
        run_timer(&mut island, &mut timer, t0, secs(70));
        // It closes past a minute boundary: the minute timer must still run.
        island.close(t0 + secs(70));
        clock_frame(&mut island, &mut timer, t0, secs(70));
        run_timer(&mut island, &mut timer, t0, secs(200));
        assert_eq!(island.clock_minute, Some(minute_at(secs(200))));
        let next = timer.0.map(|d| d.duration_since(t0));
        assert!(
            next.is_some_and(|n| n > secs(200) && n <= secs(241)),
            "{next:?}"
        );
    }

    #[test]
    fn the_bar_draws_its_zones_and_the_capsule_drops_its_dots() {
        let (island, _) = bar_island(1920.0);
        // The Workspaces piece shows the dots: the capsule is the clock.
        assert!(island.capsule.node_by_key(Key::named("dots")).is_none());
        let placement = island.bar.as_ref().and_then(BarTree::placement);
        let p = match placement {
            Some(p) => p,
            None => panic!("no placement"),
        };
        assert!(p.start.is_some() && p.end.is_some());
        assert!(p.centre_left.is_none() && p.centre_right.is_none());
        // The dots and the bell are in the start zone, the icon in the end.
        let (start, end) = (zone(p.start), zone(p.end));
        for key in [Key::indexed("dot", 1), Key::named("bell")] {
            let r = bar_rect(&island, key);
            assert!(r.x >= start.x && r.x + r.w <= start.x + start.w, "{key:?}");
        }
        let icon = bar_rect(&island, tray_page::row_key(0));
        assert!(icon.x >= end.x && icon.x + icon.w <= end.x + end.w);
    }

    #[test]
    fn the_regions_are_the_painted_silhouettes_closed_growing_and_open() {
        let (mut island, t0) = bar_island(1920.0);
        let radius = island.theme.radius_lg;
        let t1 = t0 + std::time::Duration::from_millis(5);
        let phases = [
            (t0, false),
            (t1 + std::time::Duration::from_millis(120), true),
            (t1 + std::time::Duration::from_millis(400), true),
        ];
        for (n, (now, open)) in phases.into_iter().enumerate() {
            if open && !island.state.is_open() {
                island.open(PageId::ACTIVITY, t1);
            }
            island.apply_views(now);
            lay_fake(&mut island, now);
            let p = match island.bar.as_ref().and_then(BarTree::placement) {
                Some(p) => p,
                None => panic!("no placement"),
            };
            let rect = island.state.rect(now, 1920.0);
            // What is painted: each zone capsule where the layout put it,
            // then the Island at its rect of the instant.
            let mut painted_shapes: Vec<Shape> = [p.start, p.centre_left, p.centre_right, p.end]
                .into_iter()
                .flatten()
                .map(|r| (r, radius))
                .collect();
            painted_shapes.push((rect, radius));
            assert_eq!(island.shapes(now), painted_shapes, "phase {n}");
            // No capsule ever overlaps the Island.
            for (r, _) in &painted_shapes[..painted_shapes.len() - 1] {
                assert!(
                    r.x + r.w <= rect.x || r.x >= rect.x + rect.w,
                    "phase {n}: {r:?}"
                );
            }
            let height = (rect.y + rect.h) as i32 + 4;
            assert_regions_follow(
                &painted_shapes,
                &silhouette::union(&painted_shapes),
                1920,
                height,
            );
            let input =
                silhouette::input_region(&island.shapes(now), island.state.is_open(), island.size);
            if island.state.is_open() {
                assert_eq!(input, [Rect::whole(island.size)], "phase {n}");
            } else {
                assert_eq!(input, silhouette::union(&painted_shapes), "phase {n}");
            }
        }
        assert!(
            island.state.progress(phases[1].0) > 0.0 && island.state.progress(phases[1].0) < 1.0
        );
    }

    #[test]
    fn the_idle_stretch_of_the_bar_has_no_region_and_hits_nothing() {
        let (mut island, t0) = bar_island(1920.0);
        let start = start_zone(&island);
        let capsule = island.state.rect(t0, 1920.0);
        let idle = (
            (start.x + start.w + capsule.x) / 2.0,
            state::TOP_GAP + state::CAPSULE_H / 2.0,
        );
        let region = silhouette::input_region(&island.shapes(t0), false, island.size);
        assert!(!region.iter().any(|b| b.contains(idle.0, idle.1)));
        assert!(!island.on_bar(idle.0, idle.1, t0));
        assert!(click_at(&mut island, idle, BTN_LEFT, t0).is_empty());
        assert!(!island.state.is_open());
        assert_eq!(island.capsule.highlight().hovered, None);
        assert!(island
            .bar
            .as_ref()
            .is_some_and(|b| b.highlight().hovered.is_none()));
    }

    #[test]
    fn the_bars_pieces_act_at_rest() {
        let (mut island, t0) = bar_island(1920.0);
        let id = island.tray_rows[0].id.clone();
        // The workspaces piece opens its sheet, and the wheel over it steps
        // to the next workspace (the second is active).
        let dot = centre(bar_rect(&island, Key::indexed("dot", 3)));
        assert!(click_at(&mut island, dot, BTN_LEFT, t0).is_empty());
        step(&mut island, t0);
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Workspaces)
        );
        island.close_sheet(t0);
        // The row settles.
        step(&mut island, t0 + ms(400));
        step(&mut island, t0 + ms(800));
        let t0 = t0 + ms(800);
        let wheel_down = InputEvent::Axis {
            x: dot.0,
            y: dot.1,
            horizontal: 0.0,
            vertical: 15.0,
        };
        assert_eq!(
            feed(&mut island, wheel_down, t0),
            [IslandAction::FocusWorkspace(3)]
        );
        // The tray icon: activate, secondary activate, the menu below the
        // icon, and the wheel in steps with Qt's sign.
        let rect = bar_rect(&island, tray_page::row_key(0));
        let icon = centre(rect);
        assert_eq!(
            click_at(&mut island, icon, BTN_LEFT, t0),
            [IslandAction::Tray(TrayCommand::Activate(id.clone()))]
        );
        assert_eq!(
            click_at(&mut island, icon, BTN_MIDDLE, t0),
            [IslandAction::Tray(TrayCommand::SecondaryActivate(
                id.clone()
            ))]
        );
        assert_eq!(
            click_at(&mut island, icon, BTN_RIGHT, t0),
            [IslandAction::OpenTrayMenu {
                item: id.clone(),
                row: rect,
                keyboard: false,
                at: (icon.0.round() as i32, icon.1.round() as i32),
            }]
        );
        assert!(island.bar_menu_asked);
        let wheel = InputEvent::Axis {
            x: icon.0,
            y: icon.1,
            horizontal: 0.0,
            vertical: 30.0,
        };
        assert_eq!(
            feed(&mut island, wheel, t0),
            [IslandAction::Tray(TrayCommand::Scroll(
                id,
                -2,
                Orientation::Vertical
            ))]
        );
        assert!(!island.state.is_open(), "nothing opened a page");
        // The bell opens the Notifications page as a sheet.
        let bell = centre(bar_rect(&island, Key::named("bell")));
        assert!(click_at(&mut island, bell, BTN_LEFT, t0).is_empty());
        assert_eq!(island.sheets.page(), Some(PageId::NOTIFICATIONS));
        assert_eq!(island.page(), None, "the Island's page stays shut");
    }

    #[test]
    fn the_level_pieces_appear_with_a_reading_and_ask_for_their_workers_and_sheet() {
        use crate::pieces::Level;
        let (mut island, t0) = bar_island(1920.0);
        let volume = Key::named("volume");
        let brightness = Key::named("brightness");
        // Without a reading neither piece has a node.
        let bar = island.bar.as_ref().expect("a bar");
        assert!(bar.node_rect(volume).is_none() && bar.node_rect(brightness).is_none());

        island.levels = Levels {
            audio: AudioLevels {
                output: Some(Level {
                    percent: 40,
                    muted: false,
                }),
                input: None,
            },
            brightness: Some(70),
        };
        island.invalidate_bar();
        lay_fake(&mut island, t0);
        // The new nodes fade in: the input comes after they are shown.
        let t0 = t0 + std::time::Duration::from_secs(1);

        // The wheel steps with the Wayland sign (down lowers) and the right
        // and middle clicks flip the mute.
        let at = centre(bar_rect(&island, volume));
        let wheel = |vertical| InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical,
        };
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t0,
        );
        assert_eq!(
            feed(&mut island, wheel(15.0), t0),
            [IslandAction::Audio(AudioAction::Step(Direction::Down, 1))]
        );
        assert_eq!(
            feed(&mut island, wheel(-15.0), t0),
            [IslandAction::Audio(AudioAction::Step(Direction::Up, 1))]
        );
        for button in [BTN_MIDDLE, BTN_RIGHT] {
            assert_eq!(
                click_at(&mut island, at, button, t0),
                [IslandAction::Audio(AudioAction::ToggleMute)]
            );
        }
        // A click asks for the volume sheet, at the piece's node.
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t0),
            [IslandAction::OpenLevelSheet {
                kind: level_popup::Kind::Volume,
                piece: volume,
            }]
        );

        let at = centre(bar_rect(&island, brightness));
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t0,
        );
        let wheel = InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical: -15.0,
        };
        assert_eq!(
            feed(&mut island, wheel, t0),
            [IslandAction::Brightness(BrightnessAction::Step(
                Direction::Up,
                1
            ))]
        );
        // The middle and right clicks mean nothing here.
        assert!(click_at(&mut island, at, BTN_RIGHT, t0).is_empty());
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t0),
            [IslandAction::OpenLevelSheet {
                kind: level_popup::Kind::Brightness,
                piece: brightness,
            }]
        );
        assert!(!island.state.is_open(), "no piece opened a page");
    }

    #[test]
    fn the_session_piece_opens_its_sheet() {
        let (mut island, t0) = bar_island(1920.0);
        let session = Key::named("session");
        let at = centre(bar_rect(&island, session));
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t0,
        );
        assert!(
            click_at(&mut island, at, BTN_LEFT, t0).is_empty(),
            "the Island builds the body itself: no round trip to the app"
        );
        step(&mut island, t0);
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Session)
        );
        assert_eq!(island.sheets.piece(), Some(session));
        assert!(!island.state.is_open(), "no page opened");
        assert_eq!(
            island.sheets.body_size(),
            Some(crate::session_menu::list_size(&island.theme))
        );
        // The piece asking again closes it (the piece itself is out of the
        // row while its sheet is open).
        let later = t0 + ms(500);
        assert_eq!(
            island.sheet_request(
                session,
                crate::pieces::SheetKind::Session,
                later,
                &mut fake_measure
            ),
            None
        );
        assert!(!island.sheets.is_open());
    }

    /// The workspaces sheet open on the bar's piece and settled.
    fn workspaces_sheet() -> (Island, Instant) {
        let (mut island, t0) = bar_island(1920.0);
        let at = centre(bar_rect(&island, Key::named("workspaces")));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        step(&mut island, t0);
        let t = t0 + ms(500);
        step(&mut island, t);
        (island, t)
    }

    /// The centre of the workspaces row of `id`, in surface coordinates.
    fn ws_row_at(island: &Island, id: u64, now: Instant) -> (f32, f32) {
        let sheet = shown_rect(island, now);
        let Some(r) = island
            .sheets
            .body_node_rect(crate::sheet::workspaces::row_key(id))
        else {
            panic!("no workspaces row {id}");
        };
        (sheet.x + r.x + r.w / 2.0, sheet.y + r.y + r.h / 2.0)
    }

    #[test]
    fn the_workspaces_sheet_is_wider_than_the_start_capsule_so_it_grows() {
        let (island, t) = workspaces_sheet();
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Workspaces)
        );
        assert_eq!(island.sheets.piece(), Some(Key::named("workspaces")));
        assert!(!island.state.is_open(), "no page opened");
        let start = start_zone(&island);
        let sheet = shown_rect(&island, t);
        assert_eq!(sheet.w, crate::sheet::workspaces::LIST_W);
        assert!(start.w < sheet.w, "premise: the capsule is narrower");
        assert!(sheet.w > start.w, "it grows past the capsule");
        assert_eq!(
            island.sheets.body_size(),
            Some(crate::sheet::workspaces::list_size(3))
        );
    }

    #[test]
    fn a_polkit_prompt_holding_the_keyboard_refuses_the_workspaces_sheet() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_keyboard_claimed(true);
        let at = centre(bar_rect(&island, Key::named("workspaces")));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        step(&mut island, t0);
        assert!(!island.sheets.is_open() && !island.sheets.has_queued());
    }

    #[test]
    fn a_row_click_focuses_its_workspace_and_closes_the_sheet() {
        let (mut island, t) = workspaces_sheet();
        let at = ws_row_at(&island, 3, t);
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t),
            [IslandAction::FocusWorkspace(3)]
        );
        assert!(!island.sheets.is_open(), "the sheet is closing");
        assert!(!island.sheets.has_queued());
    }

    #[test]
    fn the_wheel_steps_with_the_sheet_open_and_keeps_it_open() {
        // The workspaces sheet is open: its piece is still in the row and
        // answers the wheel itself.
        let (mut island, t0) = bar_island(1920.0);
        assert_eq!(
            island.sheet_request(
                workspaces_piece::key(),
                crate::pieces::SheetKind::Workspaces,
                t0,
                &mut fake_measure
            ),
            None
        );
        step(&mut island, t0);
        let t = t0 + ms(500);
        step(&mut island, t);
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Workspaces)
        );
        let at = centre(bar_rect(&island, workspaces_piece::key()));
        let wheel = |vertical: f32| InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical,
        };
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t,
        );
        // The second workspace is active: down is the third, up the first.
        assert_eq!(
            feed(&mut island, wheel(15.0), t),
            [IslandAction::FocusWorkspace(3)]
        );
        assert!(island.sheets.is_open());
        // The model has not caught up: up from the target (3) is 2.
        assert_eq!(
            feed(&mut island, wheel(-15.0), t),
            [IslandAction::FocusWorkspace(2)]
        );
        assert!(island.sheets.is_open());
        assert_eq!(island.sheets.piece(), Some(workspaces_piece::key()));
    }

    #[test]
    fn fast_wheel_notches_step_from_the_last_target_not_the_stale_model() {
        let (mut island, t0) = wide_start_island();
        let piece = centre(bar_rect(&island, Key::named("workspaces")));
        feed(
            &mut island,
            InputEvent::PointerMotion {
                x: piece.0,
                y: piece.1,
            },
            t0,
        );
        let notch = |island: &mut Island, at: Instant| {
            feed(
                island,
                InputEvent::Axis {
                    x: piece.0,
                    y: piece.1,
                    horizontal: 0.0,
                    vertical: 15.0,
                },
                at,
            )
        };
        // Workspace 2 is active and the model never catches up.
        for (i, want) in [3, 4, 5].into_iter().enumerate() {
            assert_eq!(
                notch(&mut island, t0 + ms(50 * i as u64)),
                [IslandAction::FocusWorkspace(want)]
            );
        }
        // The burst over, a notch counts from the model again.
        assert_eq!(
            notch(&mut island, t0 + ms(5000)),
            [IslandAction::FocusWorkspace(3)]
        );
    }

    #[test]
    fn a_queued_workspaces_sheet_opens_with_the_list_of_that_moment() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        island.open(PageId::ACTIVITY, t0);
        let at = centre(bar_rect(&island, Key::named("workspaces")));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        assert!(island.sheets.has_queued());
        let mut more = model("DP-1", 3);
        more.workspaces.push(celestina_shell_workspace(4));
        island.rows = rows_for(&more, "DP-1");
        island.model = Some(more);
        island.stale = true;
        island.invalidate_bar();
        let t1 = t0 + ms(50);
        step(&mut island, t1);
        let t2 = t0 + ms(600);
        step(&mut island, t2);
        step(&mut island, t2 + ms(16));
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Workspaces)
        );
        assert_eq!(
            island.sheets.body_size(),
            Some(crate::sheet::workspaces::list_size(4))
        );
        assert!(island
            .sheets
            .body_node_rect(crate::sheet::workspaces::row_key(4))
            .is_some());
    }

    #[test]
    fn a_model_change_rebuilds_the_open_list() {
        let (mut island, t) = workspaces_sheet();
        let before = island.sheets.body_size();
        let mut more = model("DP-1", 2);
        more.workspaces.push(celestina_shell_workspace(4));
        island.rows = rows_for(&more, "DP-1");
        island.model = Some(more);
        island.stale = true;
        island.invalidate_bar();
        step(&mut island, t);
        let after = island.sheets.body_size();
        assert_eq!(after, Some(crate::sheet::workspaces::list_size(4)));
        assert_ne!(before, after);
        assert!(island
            .sheets
            .body_node_rect(crate::sheet::workspaces::row_key(4))
            .is_some());
        assert!(island.sheets.is_open());
        // The height follows on the resize motion.
        let settle = t + ms(500);
        step(&mut island, settle);
        assert!(
            (shown_rect(&island, settle).h
                - geometry::window(
                    zone(island.bar.as_ref().and_then(|b| b.zone_rect(0))),
                    geometry::Edge::Start,
                    after.unwrap_or_default(),
                    island.size
                )
                .h)
                .abs()
                < 0.5
        );
    }

    /// The session sheet open and settled, with its first look done.
    fn session_sheet() -> (Island, Instant) {
        let (mut island, t0) = bar_island(1920.0);
        let session = Key::named("session");
        let at = centre(bar_rect(&island, session));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        step(&mut island, t0);
        let t = t0 + ms(500);
        step(&mut island, t);
        (island, t)
    }

    /// The centre of the session body's node `key`, in surface coordinates.
    fn session_node_at(island: &Island, key: Key, now: Instant) -> (f32, f32) {
        let sheet = shown_rect(island, now);
        let Some(r) = island.sheets.body_node_rect(key) else {
            panic!("no session node {key:?}");
        };
        (sheet.x + r.x + r.w / 2.0, sheet.y + r.y + r.h / 2.0)
    }

    #[test]
    fn a_session_choice_is_one_action_and_the_sheet_closes() {
        let (mut island, t) = session_sheet();
        let at = session_node_at(&island, SessionAction::Lock.key(), t);
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t),
            [IslandAction::Session(SessionAction::Lock)]
        );
        assert!(!island.sheets.is_open(), "the sheet is closing");
        assert!(island.sheets.is_shown(t), "on its exit motion");
        assert!(!island.sheets.has_queued());
    }

    #[test]
    fn a_dangerous_row_asks_first_and_the_sheet_grows_down() {
        let (mut island, t) = session_sheet();
        let list = island.sheets.body_size();
        assert_eq!(
            list,
            Some(crate::session_menu::list_size(&island.theme)),
            "the list's own size"
        );
        let before = shown_rect(&island, t);
        assert_eq!(before.y, geometry::WINDOW_TOP);
        let at = session_node_at(&island, SessionAction::Reboot.key(), t);
        assert!(
            click_at(&mut island, at, BTN_LEFT, t).is_empty(),
            "asking is no action"
        );
        let grown = island.sheets.body_size();
        assert!(grown.is_some_and(|g| g.1 > list.map_or(0.0, |l| l.1)));
        // The sheet follows its body: the same top, a taller sheet.
        let later = t + ms(600);
        step(&mut island, later);
        let after = shown_rect(&island, later);
        assert_eq!(after.y, geometry::WINDOW_TOP, "the top edge stays");
        assert!(after.h > before.h);
        // Cancel shrinks it back.
        let cancel = session_node_at(&island, crate::session_menu::cancel_key(), later);
        assert!(click_at(&mut island, cancel, BTN_LEFT, later).is_empty());
        assert_eq!(island.sheets.body_size(), list);
        let end = later + ms(600);
        step(&mut island, end);
        let back = shown_rect(&island, end);
        assert_eq!((back.y, back.h), (before.y, before.h));
    }

    #[test]
    fn the_confirm_card_measures_its_question_with_the_measure_in_hand() {
        let (mut island, t) = session_sheet();
        let calls = Cell::new(0_u32);
        let mut measure = |s: &str, st: &TextStyle, w: Option<f32>| {
            calls.set(calls.get() + 1);
            if w.is_some() {
                (200.0, 48.0)
            } else {
                fake_measure(s, st, w)
            }
        };
        let at = session_node_at(&island, SessionAction::PowerOff.key(), t);
        feed_with(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t,
            &mut measure,
        );
        feed_with(&mut island, press(at.0, at.1, BTN_LEFT), t, &mut measure);
        let seen = calls.get();
        feed_with(&mut island, release(at.0, at.1, BTN_LEFT), t, &mut measure);
        assert!(
            calls.get() > seen,
            "the question was measured by the call's"
        );
        assert_eq!(
            island.sheets.body_size(),
            Some(crate::session_menu::confirm_size(&island.theme, 48.0)),
            "the card is as tall as the measured question"
        );
    }

    #[test]
    fn escape_in_the_confirm_card_cancels_and_escape_in_the_list_closes_the_sheet() {
        let (mut island, t) = session_sheet();
        let list = island.sheets.body_size();
        let at = session_node_at(&island, SessionAction::LogOut.key(), t);
        click_at(&mut island, at, BTN_LEFT, t);
        assert_ne!(island.sheets.body_size(), list, "the card is shown");
        assert!(feed(&mut island, escape(), t).is_empty());
        assert!(island.sheets.is_open(), "Escape on the card goes back");
        assert_eq!(island.sheets.body_size(), list);
        assert!(feed(&mut island, escape(), t).is_empty());
        assert!(!island.sheets.is_open(), "Escape on the list closes");
    }

    #[test]
    fn a_bar_piece_while_open_switches_the_page_or_acts_and_closes_it() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        island.open(PageId::ACTIVITY, t0);
        // The bell closes the Island's page and opens the Notifications
        // sheet once it is gone.
        let bell = centre(bar_rect(&island, Key::named("bell")));
        assert!(click_at(&mut island, bell, BTN_LEFT, t0).is_empty());
        assert!(!island.state.is_open(), "one thing at a time");
        assert!(island.sheets.has_queued());
        let t1 = t0 + ms(500);
        step(&mut island, t1);
        assert_eq!(island.sheets.page(), Some(PageId::NOTIFICATIONS));
        // The wheel on the tray icon scrolls and keeps the sheet.
        let icon = centre(bar_rect(&island, tray_page::row_key(0)));
        let wheel = InputEvent::Axis {
            x: icon.0,
            y: icon.1,
            horizontal: 0.0,
            vertical: -15.0,
        };
        assert_eq!(feed(&mut island, wheel, t1).len(), 1);
        assert!(island.sheets.is_open());
        // A tray icon acts and closes the sheet, as a click outside it does.
        let id = island.tray_rows[0].id.clone();
        assert_eq!(
            click_at(&mut island, icon, BTN_LEFT, t1),
            [IslandAction::Tray(TrayCommand::Activate(id))]
        );
        assert!(!island.sheets.is_open());
        // A right click on the icon asks for its menu and closes the page.
        let t2 = t1 + ms(500);
        step(&mut island, t2);
        island.open(PageId::ACTIVITY, t2);
        let actions = click_at(&mut island, icon, BTN_RIGHT, t2);
        assert!(matches!(
            actions.as_slice(),
            [IslandAction::OpenTrayMenu {
                keyboard: false,
                ..
            }]
        ));
        assert!(!island.state.is_open());
        assert!(island.bar_menu_asked);
    }

    #[test]
    fn escape_a_click_outside_and_focus_loss_still_close_with_the_bar_on() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        let escape = InputEvent::Key {
            keysym: 0xff1b,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        };
        island.open(PageId::ACTIVITY, t0);
        feed(&mut island, escape, t0);
        assert!(!island.state.is_open(), "Escape");

        island.open(PageId::ACTIVITY, t0);
        assert!(feed(&mut island, press(5.0, 500.0, BTN_LEFT), t0).is_empty());
        assert!(
            !island.state.is_open(),
            "a click outside the page and the bar"
        );

        // A press on a capsule's empty stretch is a press outside the page.
        island.open(PageId::ACTIVITY, t0);
        let start = start_zone(&island);
        let empty = (start.x + 2.0, start.y + start.h / 2.0);
        assert!(island.on_bar(empty.0, empty.1, t0));
        assert!(feed(&mut island, press(empty.0, empty.1, BTN_LEFT), t0).is_empty());
        assert!(
            !island.state.is_open(),
            "a press on the bar but on no piece"
        );

        island.open(PageId::ACTIVITY, t0);
        feed(&mut island, InputEvent::FocusLost, t0);
        assert!(!island.state.is_open(), "the keyboard leaving");
    }

    #[test]
    fn the_page_wins_over_the_bar_where_they_meet_and_a_row_menu_is_the_pages() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        island.open(PageId::TRAY, t0);
        feed(
            &mut island,
            InputEvent::PointerMotion { x: 1.0, y: 1.0 },
            t0,
        );
        let page = island.state.rect(t0, 1920.0);
        let inside = centre(page);
        assert!(!island.on_bar(inside.0, inside.1, t0));
        // A right click on the page's first row asks for its menu: the
        // page's, so the bar's request is forgotten.
        island.bar_menu_asked = true;
        let row = match island.tray_row_rect(0) {
            Some(r) => centre(r),
            None => panic!("no tray row"),
        };
        let actions = click_at(&mut island, row, BTN_RIGHT, t0);
        assert!(matches!(
            actions.as_slice(),
            [IslandAction::OpenTrayMenu { .. }]
        ));
        assert!(!island.bar_menu_asked);
        assert!(island.state.is_open());
    }

    #[test]
    fn a_rebuild_that_moves_nothing_lets_a_press_recoil_run_on() {
        let (mut island, t0) = bar_island(1920.0);
        let dot = centre(bar_rect(&island, Key::indexed("dot", 3)));
        feed(&mut island, press(dot.0, dot.1, BTN_LEFT), t0);
        let recoiling = |island: &Island| island.bar.as_ref().is_some_and(|b| b.is_animating(t0));
        assert!(recoiling(&island));
        // The pieces' state changed (a new niri model, say) and the
        // capsules stay where they are.
        island.invalidate_bar();
        lay_fake(&mut island, t0);
        assert!(recoiling(&island), "the recoil was not cut short");
    }

    #[test]
    fn a_new_layout_rebuilds_the_bar_and_the_same_one_changes_nothing() {
        let (mut island, t0) = bar_island(1920.0);
        island.stale = false;
        island.set_bar(&BarLayout::default());
        assert!(!island.stale, "the same layout");
        let mut only_tray = BarLayout::default();
        only_tray.start.clear();
        island.set_bar(&only_tray);
        assert!(island.stale);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        // Workspaces is no longer placed: the capsule shows the dots again
        // and the start zone is gone.
        assert!(island.capsule.node_by_key(Key::named("dots")).is_some());
        let p = island.bar.as_ref().and_then(BarTree::placement);
        assert!(p.is_some_and(|p| p.start.is_none() && p.end.is_some()));
    }

    #[test]
    fn a_layout_edit_while_a_page_is_open_moves_the_bar_and_leaves_the_page() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        island.open(PageId::NOTIFICATIONS, t0);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let page = island.state.rect(t0, 1920.0);
        let bell = bar_rect(&island, Key::named("bell"));
        assert!(bell.x < page.x, "the bell starts in the start zone");

        let edited = BarLayout {
            start: vec!["workspaces".to_owned()],
            end: vec!["notifications".to_owned(), "tray".to_owned()],
            ..BarLayout::default()
        };
        island.set_bar(&edited);
        island.apply_views(t0);
        lay_fake(&mut island, t0);

        assert_eq!(island.page(), Some(PageId::NOTIFICATIONS));
        assert_eq!(island.state.rect(t0, 1920.0), page, "the page did not move");
        let moved = bar_rect(&island, Key::named("bell"));
        assert!(moved.x > page.x + page.w, "the bell is in the end zone now");
        // The page still wins where it is, and the moved bell still acts.
        let inside = centre(page);
        assert!(!island.on_bar(inside.0, inside.1, t0));
        assert!(island.on_bar(centre(moved).0, centre(moved).1, t0));
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
    fn rows_activate_focus_requests_and_the_capsule_opens_the_clock_page() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.model = Some(model("DP-1", 1));
        assert_eq!(
            island.activate(activity::workspace_key(2), t0),
            Some(IslandAction::FocusWorkspace(2))
        );
        assert_eq!(island.activate(Key::named("nothing"), t0), None);
        assert_eq!(island.page(), None);
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(PageId::CLOCK));
        island.activate(view::close_key(), t0);
        assert_eq!(island.page(), None);
    }

    #[test]
    fn a_click_on_the_capsule_opens_the_clock_page() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.model = Some(model("DP-1", 1));
        // From rest, after another page was the last one: the capsule opens
        // the clock page, `island toggle` still reopens the last one.
        island.open(PageId::TRAY, t0);
        island.close(t0);
        assert_eq!(island.page(), None);
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(PageId::CLOCK));
        island.close(t0);
        let t1 = t0 + Duration::from_secs(1);
        island.open(PageId::TRAY, t1);
        island.close(t1);
        let t2 = t1 + Duration::from_secs(1);
        assert!(island.command_at(&Verb::IslandToggle, t2));
        assert_eq!(island.page(), Some(PageId::TRAY));
    }

    #[test]
    fn a_weather_reading_change_sets_stale_through_the_frames_mirror() {
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let reading = some_weather();
        island.stale = false;
        assert!(island.mirror_weather(Some(&reading)));
        assert!(island.stale, "a first reading rebuilds the capsule");
        island.stale = false;
        assert!(!island.mirror_weather(Some(&reading)));
        assert!(!island.stale, "the same reading costs no frame");
        let colder = WeatherReading {
            celsius: 3,
            ..reading
        };
        assert!(island.mirror_weather(Some(&colder)));
        assert!(island.stale);
        island.stale = false;
        assert!(island.mirror_weather(None));
        assert!(island.stale, "a withdrawn reading rebuilds it too");
    }

    #[test]
    fn a_new_day_or_reading_rebuilds_the_capsule_once() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.stale = false;
        island.set_today(1_792_281_480, 0);
        assert!(island.stale, "a first date rebuilds the capsule");
        island.stale = false;
        island.set_today(1_792_281_480 + 60, 0);
        assert!(!island.stale, "the same day costs no frame");
        island.set_today(1_792_281_480 + 86_400, 0);
        assert!(island.stale, "a new day rebuilds it");
        island.apply_views(t0);
        assert!(!island.stale);
        island.weather = Some(some_weather());
        island.stale = true;
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let text =
            |island: &Island, name: &str| island.capsule.node_by_key(Key::named(name)).is_some();
        assert!(text(&island, "capsule-date") && text(&island, "capsule-weather-text"));
    }

    #[test]
    fn the_phone_page_opens_only_while_the_phone_duty_is_on() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        assert!(!island.phone_duty, "off by default");
        island.open(PageId::PHONE, t0);
        assert_eq!(island.page(), None, "`island open phone` is a no-op");
        island.open(PageId::TRAY, t0);
        island.open(PageId::PHONE, t0);
        assert_eq!(island.page(), Some(PageId::TRAY), "an open page stays");
        island.close(t0);
        island.phone_duty = true;
        island.open(PageId::PHONE, t0);
        assert_eq!(island.page(), Some(PageId::PHONE));
    }

    #[test]
    fn opening_another_page_while_open_slides_to_it() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::NOTIFICATIONS, t0);
        assert_eq!(island.state.slide(t0), 0.0);
        island.open(PageId::ACTIVITY, t0);
        assert_eq!(island.page(), Some(PageId::ACTIVITY));
        assert_eq!(
            island.state.slide(t0),
            1.0,
            "Activity is left of Notifications"
        );
        island.set_reduced_motion(true);
        island.open(PageId::TRAY, t0);
        assert_eq!(island.page(), Some(PageId::TRAY));
        assert_eq!(island.state.slide(t0), 0.0);
    }

    #[test]
    fn a_tab_click_slides_to_its_page_and_a_window_row_asks_to_focus() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let mut m = model("DP-1", 1);
        m.windows.push(Window {
            id: 7,
            title: Some("t".to_owned()),
            app_id: None,
            workspace_id: Some(1),
            is_focused: false,
            icon: None,
            ..Default::default()
        });
        island.model = Some(m);
        island.open(PageId::ACTIVITY, t0);
        assert_eq!(island.activate(tabs::tab_key(PageId::TRAY), t0), None);
        assert_eq!(island.page(), Some(PageId::TRAY));
        assert_eq!(island.state.slide(t0), -1.0, "Tray is right of Activity");
        island.open(PageId::ACTIVITY, t0);
        assert_eq!(
            island.activate(activity::window_key(7), t0),
            Some(IslandAction::FocusWindow(7))
        );
        assert_eq!(island.page(), Some(PageId::ACTIVITY), "no optimistic state");
    }

    #[test]
    fn a_dismissed_menu_closes_the_page_but_an_island_press_does_not() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::ACTIVITY, t0);

        // Closed from the Island's side (a press on the Island): the page
        // stays.
        island.menu = Some(PopupId(1));
        assert!(island.menu_closed(PopupId(1), CloseReason::Parent, t0));
        assert!(island.menu.is_none());
        assert!(island.state.is_open());
        // Closed by the menu itself (a row chosen, Escape): the page stays.
        island.menu = Some(PopupId(2));
        assert!(island.menu_closed(PopupId(2), CloseReason::Shell, t0));
        assert!(island.state.is_open());
        // Someone else's popup: ignored.
        island.menu = Some(PopupId(3));
        assert!(!island.menu_closed(PopupId(9), CloseReason::Dismissed, t0));
        assert!(island.state.is_open());
        // Dismissed by the compositor (a click on another window): the
        // page closes with it.
        assert!(island.menu_closed(PopupId(3), CloseReason::Dismissed, t0));
        assert!(island.menu.is_none());
        assert!(!island.state.is_open());
    }

    #[test]
    fn a_parent_or_shell_close_of_the_menu_leaves_the_page_open() {
        let t0 = Instant::now();
        for reason in [CloseReason::Parent, CloseReason::Shell] {
            let mut island = Island::new("DP-1", (760.0, 560.0));
            island.open(PageId::NOTIFICATIONS, t0);
            island.menu = Some(PopupId(4));
            assert!(island.menu_closed(PopupId(4), reason, t0), "{reason:?}");
            assert!(island.menu.is_none(), "{reason:?}");
            assert!(island.state.is_open(), "{reason:?}");
            assert_eq!(island.page(), Some(PageId::NOTIFICATIONS), "{reason:?}");
        }
    }

    #[test]
    fn the_more_button_asks_for_the_menu_below_itself() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::ACTIVITY, t0);
        island.apply_views(t0);
        island.page.layout_measured(island.size, |s, _, _| {
            (7.0 * s.chars().count() as f32, 16.0)
        });
        let more = island
            .page
            .node_by_key(view::more_key())
            .map(|id| island.page.rect(id));
        assert!(more.is_some_and(|r| r.w > 0.0));
        assert_eq!(
            island.on_event(UiEvent::Click(view::more_key()), t0),
            more.map(|more| IslandAction::OpenMenu {
                more,
                keyboard: false
            })
        );
        // Enter or Space on the button: the menu starts with a focused row.
        assert_eq!(
            island.on_event(UiEvent::Activate(view::more_key()), t0),
            more.map(|more| IslandAction::OpenMenu {
                more,
                keyboard: true
            })
        );
    }

    /// An open Tray page with `n` items (the first has a menu), laid out.
    fn tray_island(n: usize) -> (Island, Instant) {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.tray_rows = (0..n)
            .map(|i| TrayRow {
                id: ItemId {
                    bus_name: format!(":1.{i}"),
                    path: "/StatusNotifierItem".to_owned(),
                },
                title: format!("item {i}"),
                has_menu: i == 0,
                ..TrayRow::default()
            })
            .collect();
        island.open(PageId::TRAY, t0);
        island.apply_views(t0);
        island.page.layout_measured(island.size, |s, _, _| {
            (7.0 * s.chars().count() as f32, 16.0)
        });
        (island, t0)
    }

    fn id_of(island: &Island, i: usize) -> ItemId {
        island.tray_rows[i].id.clone()
    }

    #[test]
    fn scroll_on_a_row_sends_scroll_with_the_delta_in_steps() {
        let (mut island, _) = tray_island(2);
        let row = tray_page::row_key(1);
        let id = id_of(&island, 1);
        let scroll = |v: i32, o| IslandAction::Tray(TrayCommand::Scroll(id.clone(), v, o));
        // The wheel's down (positive) is the item's negative; 30 px = 2 steps.
        assert_eq!(
            island.scroll_actions(row, 0.0, 30.0),
            vec![scroll(-2, Orientation::Vertical)]
        );
        // Less than a step waits; two partial turns add up, keeping the rest.
        assert_eq!(island.scroll_actions(row, 0.0, -10.0), vec![]);
        assert_eq!(
            island.scroll_actions(row, 0.0, -10.0),
            vec![scroll(1, Orientation::Vertical)]
        );
        // Horizontal is negated too (a tilt right is the item's negative)
        // and both axes can fire together.
        assert_eq!(
            island.scroll_actions(row, 45.0, 20.0),
            vec![
                scroll(-1, Orientation::Vertical),
                scroll(-3, Orientation::Horizontal)
            ]
        );
        // Another row starts from nothing; a non-row key does nothing.
        assert_eq!(
            island.scroll_actions(tray_page::row_key(0), 0.0, 14.0),
            vec![]
        );
        assert_eq!(
            island.scroll_actions(Key::named("close"), 0.0, 99.0),
            vec![]
        );
        assert_eq!(island.scroll_actions(row, f32::NAN, f32::INFINITY), vec![]);
        // The same events through the tree's Scroll event.
        assert_eq!(
            island.tray_actions(&UiEvent::Scroll(row, 0.0, 15.0)),
            vec![scroll(-1, Orientation::Vertical)]
        );
    }

    #[test]
    fn a_click_and_the_keyboards_activation_both_activate_the_item() {
        let (mut island, _) = tray_island(2);
        let rect0 = island.tray_row_rect(0);
        assert!(rect0.is_some_and(|r| r.w > 0.0 && r.h > 0.0));
        let (a, b) = (id_of(&island, 0), id_of(&island, 1));
        assert_eq!(
            island.tray_actions(&UiEvent::Click(tray_page::row_key(0))),
            vec![IslandAction::Tray(TrayCommand::Activate(a.clone()))]
        );
        // Enter or Space on a row with a menu activates the item as a click
        // does; the menu has its own key (`the_menu_key_opens...`).
        assert_eq!(
            island.tray_actions(&UiEvent::Activate(tray_page::row_key(0))),
            vec![IslandAction::Tray(TrayCommand::Activate(a))]
        );
        assert_eq!(
            island.tray_actions(&UiEvent::Activate(tray_page::row_key(1))),
            vec![IslandAction::Tray(TrayCommand::Activate(b))]
        );
        // Neither a stranger's key nor a closed page acts.
        assert!(island
            .tray_actions(&UiEvent::Click(Key::named("close")))
            .is_empty());
        island.close(Instant::now());
        island.state.close(Instant::now(), true);
        assert!(island
            .tray_actions(&UiEvent::Click(tray_page::row_key(0)))
            .is_empty());
    }

    #[test]
    fn middle_click_activates_secondarily_and_right_click_asks_for_the_menu() {
        let (island, _) = tray_island(2);
        let centre = |i: usize| {
            island
                .tray_row_rect(i)
                .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                .unwrap_or_default()
        };
        let (x0, y0) = centre(0);
        let (x1, y1) = centre(1);
        let (a, b) = (id_of(&island, 0), id_of(&island, 1));
        assert_eq!(
            island.other_button(x1, y1, BTN_MIDDLE),
            vec![IslandAction::Tray(TrayCommand::SecondaryActivate(b))]
        );
        assert_eq!(
            island.other_button(x0, y0, BTN_RIGHT),
            island
                .open_menu_action(a, 0, false, (x0.round() as i32, y0.round() as i32))
                .into_iter()
                .collect::<Vec<_>>()
        );
        // A right click on an item without a menu asks the item for its
        // own, at the pointer; a click between rows and any other button do
        // nothing.
        assert_eq!(
            island.other_button(x1 + 0.4, y1, BTN_RIGHT),
            vec![IslandAction::Tray(TrayCommand::ContextMenu(
                id_of(&island, 1),
                x1.round() as i32,
                y1.round() as i32
            ))]
        );
        assert!(island.other_button(1.0, 1.0, BTN_MIDDLE).is_empty());
        assert!(island.other_button(x0, y0, 0x113).is_empty());
    }

    #[test]
    fn the_menu_key_opens_the_menu_of_the_focused_row() {
        let (mut island, _) = tray_island(2);
        assert!(island.menu_key().is_empty(), "nothing focused");
        // Tab focuses the first row (it has a menu), Tab again the second.
        let tab = InputEvent::Key {
            keysym: 0xff09,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        };
        let tab_to = |island: &mut Island, want: Key| {
            for _ in 0..30 {
                island.page_input.key(&mut island.page, &tab);
                let on = island.page.highlight().focused;
                if on.is_some_and(|id| island.page.key_of(id) == want) {
                    return;
                }
            }
            panic!("the focus never reached {want:?}");
        };
        tab_to(&mut island, tray_page::row_key(0));
        assert_eq!(island.menu_key().len(), 1);
        assert!(matches!(
            island.menu_key().first(),
            Some(IslandAction::OpenTrayMenu { keyboard: true, .. })
        ));
        tab_to(&mut island, tray_page::row_key(1));
        assert!(
            matches!(
                island.menu_key().as_slice(),
                [IslandAction::Tray(TrayCommand::ContextMenu(..))]
            ),
            "the second row has no dbusmenu: the item's own menu"
        );
    }

    #[test]
    fn item_is_menu_opens_the_menu_on_left_click() {
        let (mut island, _) = tray_island(3);
        island.tray_rows[0].item_is_menu = true;
        island.tray_rows[1].item_is_menu = true;
        island.pointer = (30.4, 40.6);
        // With a dbusmenu: its menu opens, not an activation.
        let first = island.tray_actions(&UiEvent::Click(tray_page::row_key(0)));
        assert!(
            matches!(
                first.as_slice(),
                [IslandAction::OpenTrayMenu {
                    keyboard: false,
                    at: (30, 41),
                    ..
                }]
            ),
            "{first:?}"
        );
        // Without one: the item's own menu, at the pointer.
        assert_eq!(
            island.tray_actions(&UiEvent::Click(tray_page::row_key(1))),
            vec![IslandAction::Tray(TrayCommand::ContextMenu(
                id_of(&island, 1),
                30,
                41
            ))]
        );
        // An item that is not only a menu still activates.
        assert_eq!(
            island.tray_actions(&UiEvent::Click(tray_page::row_key(2))),
            vec![IslandAction::Tray(TrayCommand::Activate(id_of(&island, 2)))]
        );
    }

    #[test]
    fn the_trays_first_level_closes_like_the_overflow_menu() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::TRAY, t0);
        island.tray_menu = Some(PopupId(5));
        // Escape or a choice in the menu: the page stays.
        assert!(island.menu_closed(PopupId(5), CloseReason::Shell, t0));
        assert!(island.tray_menu.is_none() && island.state.is_open());
        // The compositor's dismissal (a click outside) closes the page.
        island.tray_menu = Some(PopupId(6));
        assert!(!island.menu_closed(PopupId(1), CloseReason::Dismissed, t0));
        assert!(island.menu_closed(PopupId(6), CloseReason::Dismissed, t0));
        assert!(!island.state.is_open());
    }

    #[test]
    fn reopening_grows_the_open_page_again_from_the_capsule() {
        let t0 = Instant::now();
        let later = t0 + std::time::Duration::from_secs(2);
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::TRAY, t0);
        assert_eq!(island.state.progress(later), 1.0);
        island.reopen_at(later);
        assert_eq!(island.page(), Some(PageId::TRAY));
        assert_eq!(island.state.progress(later), 0.0, "from the capsule");
        assert!(island.state.wants_frame(later), "and growing");
    }

    #[test]
    fn a_reopened_page_grows_on_one_clock_even_when_it_asks_for_more() {
        let t0 = Instant::now();
        let later = t0 + std::time::Duration::from_secs(2);
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::ACTIVITY, t0);
        // A resize left in flight before the reopen.
        island.state.request_page_size((480.0, 360.0), later, false);
        island.reopen_at(later);
        let frame = later + std::time::Duration::from_millis(16);
        island.state.request_page_size((500.0, 400.0), frame, false);
        for ms in [16_u64, 120, 240, 340] {
            let now = later + std::time::Duration::from_millis(ms);
            let p = island.state.progress(now);
            let r = island.state.rect(now, 760.0);
            let capsule = island.state.rect(later, 760.0).w;
            assert!(
                (r.w - (capsule + (500.0 - capsule) * p)).abs() < 0.01,
                "{ms} ms: {r:?}"
            );
            assert!(island.state.wants_frame(now));
        }
    }

    // Connectivity (CEL-2-F).

    use crate::connectivity::state::sample;
    use celestina_shell::connectivity::{Editor, Section};

    // `key_event` and `ENTER` are the clipboard tests' (CEL-2-G), below.
    const TAB: u32 = 0xff09;
    const SPACE: u32 = 0x20;
    const ESCAPE: u32 = 0xff1b;

    /// An Island with every connectivity service ready, `page` open and laid
    /// out (motion reduced, so the page is at its place at once).
    fn connectivity_island(page: PageId) -> (Island, Instant) {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.set_reduced_motion(true);
        island.set_connectivity(&sample::ready());
        island.open(page, t0);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        (island, t0)
    }

    fn page_rect(island: &Island, key: Key) -> Rect {
        match island.page.node_by_key(key) {
            Some(id) => island.page.rect(id),
            None => panic!("no page node {key:?}"),
        }
    }

    /// Tabs through the page until `want` has the focus.
    fn tab_to(island: &mut Island, want: Key, t0: Instant) {
        for _ in 0..40 {
            feed(island, key_event(TAB), t0);
            let on = island.page.highlight().focused;
            if on.is_some_and(|id| island.page.key_of(id) == want) {
                return;
            }
        }
        panic!("the focus never reached {want:?}");
    }

    #[test]
    fn a_click_enter_or_space_on_a_row_asks_for_its_typed_action() {
        let (mut island, t0) = connectivity_island(PageId::NETWORK);
        let cafe = network_page::row_key("cafe");
        let join = IslandAction::Connectivity(Action::JoinSaved(sample::id("cafe")));
        // The pointer.
        let at = centre(page_rect(&island, cafe));
        assert_eq!(click_at(&mut island, at, BTN_LEFT, t0), vec![join.clone()]);
        // The keyboard: the row takes the focus, Enter and Space act on it.
        tab_to(&mut island, cafe, t0);
        assert_eq!(feed(&mut island, key_event(ENTER), t0), vec![join.clone()]);
        assert_eq!(feed(&mut island, key_event(SPACE), t0), [join]);
        // The connected row offers to leave it; the editor row the editor.
        let home = network_page::row_key("home");
        tab_to(&mut island, home, t0);
        assert_eq!(
            feed(&mut island, key_event(ENTER), t0),
            [IslandAction::Connectivity(Action::Leave(sample::id(
                "home"
            )))]
        );
        tab_to(&mut island, network_page::edit_key(), t0);
        assert_eq!(
            feed(&mut island, key_event(ENTER), t0),
            [IslandAction::Connectivity(Action::OpenEditor(
                Editor::Network
            ))]
        );
        // Escape closes the page, as on every page.
        assert!(feed(&mut island, key_event(ESCAPE), t0).is_empty());
        assert!(!island.state.is_open());
    }

    #[test]
    fn the_bluetooth_and_power_pages_act_from_the_keyboard_too() {
        let (mut island, t0) = connectivity_island(PageId::BLUETOOTH);
        let headset = bluetooth_page::row_key("AA:BB:CC:DD:EE:01");
        tab_to(&mut island, headset, t0);
        assert_eq!(
            feed(&mut island, key_event(ENTER), t0),
            [IslandAction::Connectivity(Action::DisconnectDevice(
                sample::address("AA:BB:CC:DD:EE:01")
            ))]
        );

        let (mut island, t0) = connectivity_island(PageId::POWER);
        let performance = power_page::profile_key("performance");
        tab_to(&mut island, performance, t0);
        assert_eq!(
            feed(&mut island, key_event(SPACE), t0),
            [IslandAction::Connectivity(Action::SetProfile(
                sample::profile("performance")
            ))]
        );
        assert!(feed(&mut island, key_event(ESCAPE), t0).is_empty());
        assert!(!island.state.is_open());
    }

    #[test]
    fn a_row_answers_only_while_its_page_is_on_screen() {
        let (mut island, t0) = connectivity_island(PageId::NETWORK);
        let cafe = network_page::row_key("cafe");
        assert!(island.activate(cafe, t0).is_some());
        // Another page shown: the network row's key asks for nothing.
        island.open(PageId::TRAY, t0);
        assert_eq!(island.activate(cafe, t0), None);
        island.open(PageId::BLUETOOTH, t0);
        assert_eq!(island.activate(cafe, t0), None);
        // Closed: nothing at all.
        island.open(PageId::NETWORK, t0);
        island.close(t0);
        assert_eq!(island.activate(cafe, t0), None);
        // A row the page offers nothing for (an unsaved enterprise network)
        // is no action either.
        island.open(PageId::NETWORK, t0);
        assert_eq!(island.activate(network_page::row_key("work"), t0), None);
    }

    #[test]
    fn a_page_for_an_absent_service_explains_and_offers_nothing() {
        let t0 = Instant::now();
        for (page, why) in [
            (PageId::NETWORK, crate::copy::NET_UNAVAILABLE),
            (PageId::BLUETOOTH, crate::copy::BT_UNAVAILABLE),
            (PageId::POWER, crate::copy::POWER_UNAVAILABLE),
        ] {
            let mut island = Island::new("DP-1", (760.0, 560.0));
            island.set_reduced_motion(true);
            island.open(page, t0);
            island.apply_views(t0);
            lay_fake(&mut island, t0);
            let cx = PageCx {
                content_w: None,
                theme: &island.theme,
                max_size: island.state.max_page_size(),
                model: None,
                output: "DP-1",
                owners: &island.owners,
                do_not_disturb: false,
                history: &[],
                items: &[],
                sysmon: None,
                weather: None,
                today: None,
                month_offset: 0,
                utc_offset: 0,
                clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,

                connectivity: &island.connectivity,
                phone: &celestina_shell::phone::PhoneState::Absent,
                media: &crate::island::pages::media::NO_MEDIA,
            };
            let texts = texts_of(&page_view(page).view(&cx));
            assert!(texts.iter().any(|t| t == why), "{page:?}: {texts:?}");
            assert!(island.page.node_by_key(Key::named("note-text")).is_some());
            // Tab goes round the page's own controls only (close, more, the
            // tabs); Enter on any of them asks for no connectivity action.
            for _ in 0..12 {
                let acted = feed(&mut island, key_event(TAB), t0);
                assert!(acted.is_empty());
                let acted = island
                    .page
                    .highlight()
                    .focused
                    .map(|id| island.page.key_of(id))
                    .and_then(|key| island.connectivity_action(page, key));
                assert_eq!(acted, None, "{page:?}");
            }
        }
    }

    fn texts_of(view: &celestina_ui::View) -> Vec<String> {
        let mut out = Vec::new();
        if let celestina_ui::ViewKind::Text { text, .. } = &view.kind {
            out.push(text.clone());
        }
        for child in &view.children {
            out.extend(texts_of(child));
        }
        out
    }

    #[test]
    fn the_power_profile_pieces_middle_click_takes_the_same_route() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (1920.0, 560.0));
        island.set_connectivity(&sample::ready());
        let layout = BarLayout {
            end: vec!["power-profile".to_owned()],
            ..BarLayout::default()
        };
        island.set_bar(&layout);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let piece = centre(bar_rect(&island, Key::named("power-profile")));
        assert_eq!(
            click_at(&mut island, piece, BTN_MIDDLE, t0),
            [IslandAction::Connectivity(Action::CycleProfile)]
        );
    }

    #[test]
    fn an_unchanged_state_asks_for_no_frame_and_a_closed_island_none_at_all() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.stale = false;
        let ready = sample::ready();
        // Closed with the bar off: the copy is taken, nothing is shown.
        assert!(!island.set_connectivity(&ready));
        assert_eq!(island.connectivity, ready);
        assert!(!island.stale);
        // The same state again: nothing at all.
        assert!(!island.set_connectivity(&ready));
        // Open: a change rebuilds the page and is worth a frame.
        island.open(PageId::NETWORK, t0);
        island.stale = false;
        let mut pending = ready.clone();
        assert_eq!(pending.begin(&Action::Refresh(Section::Network)), Ok(()));
        assert!(!island.set_connectivity(&pending), "a refresh marks no row");
        assert!(pending
            .begin(&Action::JoinSaved(sample::id("cafe")))
            .is_ok());
        assert!(island.set_connectivity(&pending));
        assert!(island.stale);
        island.stale = false;
        assert!(!island.set_connectivity(&pending));
        assert!(!island.stale);
        // Closed with the bar on: its pieces show the state.
        island.close(t0);
        island.state.close(t0, true);
        island.set_bar(&BarLayout::default());
        assert!(island.set_connectivity(&ready));
        assert!(!island.set_connectivity(&ready));
    }

    #[test]
    fn an_open_island_shows_window_changes_a_closed_one_does_not() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let a = model("DP-1", 1);
        island.model = Some(a.clone());
        island.rows = rows_for(&a, "DP-1");
        let mut b = a.clone();
        b.windows.push(Window {
            id: 1,
            title: None,
            app_id: None,
            workspace_id: Some(1),
            is_focused: false,
            icon: None,
            ..Default::default()
        });
        assert!(!island.shows_a_change(Some(&b)));
        island.open(PageId::ACTIVITY, t0);
        assert!(island.shows_a_change(Some(&b)));
        assert!(!island.shows_a_change(Some(&a)));
    }

    // The glance slices (CEL-2-K).

    fn sample(cpu: u8) -> SysReading {
        SysReading {
            cpu,
            ram: 40,
            ram_used_kib: 4,
            ram_total_kib: 10,
            temp_c: Some(50),
            cpu_history: vec![cpu],
            ram_history: vec![40],
        }
    }

    fn bar_stale(island: &Island) -> bool {
        island.bar.as_ref().is_some_and(BarTree::is_stale)
    }

    #[test]
    fn a_sample_every_two_seconds_reaches_a_resting_bar_once_per_ten() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        // Nothing yet, nothing new: no frame.
        assert_eq!(island.sysmon_wake(None, t0), Wake::Nothing);
        // The first reading is shown at once.
        let first = sample(1);
        assert_eq!(island.sysmon_wake(Some(&first), t0), Wake::Now);
        assert_eq!(island.mirror_sysmon(Some(&first), t0), None);
        assert!(bar_stale(&island));
        lay_fake(&mut island, t0);
        assert!(!bar_stale(&island));
        // Every two seconds a new sample: held back, with one timed frame
        // armed for when it is due, and the bar not rebuilt.
        let mut frames = 0;
        for n in 1..=15u8 {
            let now = t0 + 2 * s * u32::from(n);
            let newest = sample(n + 1);
            match island.sysmon_wake(Some(&newest), now) {
                Wake::Nothing => panic!("a new sample is never nothing"),
                Wake::After(wait) => {
                    assert!(wait <= SYSMON_BAR_PERIOD);
                    assert_eq!(island.mirror_sysmon(Some(&newest), now), Some(wait));
                    assert!(!bar_stale(&island), "held back at {n}");
                }
                Wake::Now => {
                    frames += 1;
                    assert_eq!(island.mirror_sysmon(Some(&newest), now), None);
                    assert!(bar_stale(&island));
                    lay_fake(&mut island, now);
                }
            }
            assert_eq!(island.page_sysmon(), Some(&newest));
        }
        // Thirty seconds: three bar changes, at 10, 20 and 30 s.
        assert_eq!(frames, 3);
        assert_eq!(island.bar_sysmon(), Some(&sample(16)));
        // The same reading again wakes nothing.
        let later = t0 + 31 * s;
        assert_eq!(island.sysmon_wake(Some(&sample(16)), later), Wake::Nothing);
        // A withdrawal is shown at once.
        assert_eq!(island.sysmon_wake(None, later), Wake::Now);
        island.mirror_sysmon(None, later);
        assert_eq!(island.bar_sysmon(), None);
    }

    /// A surface as the runtime keeps it: the one redraw timer
    /// (`celestina_wayland`'s `RedrawTimer`, the real one), and the frames
    /// asked for at once.
    struct FakeSurface {
        now: Instant,
        timer: celestina_wayland::surface::RedrawTimer,
        requested: u32,
    }

    impl FakeSurface {
        fn new(now: Instant) -> Self {
            Self {
                now,
                timer: celestina_wayland::surface::RedrawTimer::default(),
                requested: 0,
            }
        }

        /// The deadline armed, as a time after `t0`.
        fn due(&self, t0: Instant) -> Option<Duration> {
            self.timer.deadline().map(|d| d.duration_since(t0))
        }
    }

    impl Redraws for FakeSurface {
        fn request_redraw(&mut self) {
            self.requested += 1;
        }

        fn redraw_after_withdrawable(&mut self, wait: Duration) {
            self.timer.arm_withdrawable(self.now + wait);
        }

        fn withdraw_redraw(&mut self) {
            self.timer.withdraw();
        }
    }

    /// The system monitor's reading at `t0 + t`, as the app carries it out
    /// (`App::sysmon_read`): the wake on the surface.
    fn read_at(
        island: &Island,
        surface: &mut FakeSurface,
        t0: Instant,
        t: Duration,
        newest: Option<&SysReading>,
    ) -> Wake {
        surface.now = t0 + t;
        let wake = island.sysmon_wake(newest, surface.now);
        wake.apply(surface);
        wake
    }

    /// The frame the surface draws at `t0 + t` (`Island::frame`'s part for the
    /// system monitor): the timer has fired when `fired`.
    fn frame_at(
        island: &mut Island,
        surface: &mut FakeSurface,
        t0: Instant,
        t: Duration,
        newest: Option<&SysReading>,
        fired: bool,
    ) {
        surface.now = t0 + t;
        if fired {
            surface.timer.fired();
        }
        island.hold_sysmon(newest, surface.now, surface);
    }

    #[test]
    fn a_reading_withdrawn_while_a_copy_is_held_back_leaves_no_deadline() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        let mut surface = FakeSurface::new(t0);
        let first = sample(1);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        // The second reading is held back until ten seconds.
        let second = sample(2);
        let wake = read_at(&island, &mut surface, t0, 4 * s, Some(&second));
        assert_eq!(wake, Wake::After(6 * s));
        assert_eq!(surface.due(t0), Some(10 * s));
        // The monitor stops: the withdrawal is shown at once, and the frame
        // that shows it takes the held-back copy's deadline back.
        let wake = read_at(&island, &mut surface, t0, 9 * s, None);
        assert_eq!(wake, Wake::Now);
        frame_at(&mut island, &mut surface, t0, 9 * s, None, false);
        assert_eq!(island.bar_sysmon(), None);
        assert_eq!(surface.due(t0), None, "no frame at ten seconds");
    }

    #[test]
    fn a_reading_that_comes_back_to_what_the_bar_shows_withdraws_the_copy() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        let mut surface = FakeSurface::new(t0);
        let first = sample(1);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        read_at(&island, &mut surface, t0, 2 * s, Some(&sample(2)));
        assert_eq!(surface.due(t0), Some(10 * s));
        // The next reading equals what the bar shows: nothing is due.
        let wake = read_at(&island, &mut surface, t0, 4 * s, Some(&first));
        assert_eq!(wake, Wake::Nothing);
        assert_eq!(surface.due(t0), None);
        assert_eq!(surface.requested, 1, "only the first reading drew");
    }

    #[test]
    fn a_held_back_copy_still_arrives_once_when_its_period_ends() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        let mut surface = FakeSurface::new(t0);
        let first = sample(1);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        // Two more readings, the newest the one the bar will show.
        read_at(&island, &mut surface, t0, 2 * s, Some(&sample(2)));
        let newest = sample(3);
        read_at(&island, &mut surface, t0, 4 * s, Some(&newest));
        // The deadline is the period's end, not before, not moved by a
        // second reading.
        assert_eq!(surface.due(t0), Some(10 * s));
        assert_eq!(island.bar_sysmon(), Some(&first));
        // The timer fires: the frame shows the newest reading, once.
        frame_at(&mut island, &mut surface, t0, 10 * s, Some(&newest), true);
        assert_eq!(island.bar_sysmon(), Some(&newest));
        assert_eq!(surface.due(t0), None);
        assert_eq!(surface.requested, 1, "no frame is asked for at once");
    }

    #[test]
    fn the_clocks_minute_survives_a_withdrawn_copy() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        let mut surface = FakeSurface::new(t0);
        let first = sample(1);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        // The frame asks for the minute, later than the copy's deadline.
        surface.timer.arm(t0 + 60 * s);
        read_at(&island, &mut surface, t0, 4 * s, Some(&sample(2)));
        assert_eq!(surface.due(t0), Some(10 * s));
        frame_at(&mut island, &mut surface, t0, 9 * s, None, false);
        assert_eq!(surface.due(t0), Some(60 * s), "the minute's tick is kept");
        // A wait earlier than the copy's is kept as well.
        let (mut island, t0) = bar_island(1920.0);
        let mut surface = FakeSurface::new(t0);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        read_at(&island, &mut surface, t0, 4 * s, Some(&sample(2)));
        surface.timer.arm(t0 + 5 * s);
        assert_eq!(surface.due(t0), Some(5 * s));
        // At five seconds the earlier wait fires and the frame still holds
        // the copy back; the withdrawal then leaves only the later wait.
        frame_at(&mut island, &mut surface, t0, 5 * s, Some(&sample(2)), true);
        assert_eq!(surface.due(t0), Some(10 * s));
        surface.timer.arm(t0 + 12 * s);
        frame_at(&mut island, &mut surface, t0, 6 * s, None, false);
        assert_eq!(surface.due(t0), Some(12 * s), "the copy's 10 s went");
    }

    #[test]
    fn a_copy_held_back_stays_due_while_the_system_page_opens_and_closes() {
        let (mut island, t0) = bar_island(1920.0);
        let s = Duration::from_secs(1);
        let mut surface = FakeSurface::new(t0);
        let first = sample(1);
        read_at(&island, &mut surface, t0, Duration::ZERO, Some(&first));
        frame_at(
            &mut island,
            &mut surface,
            t0,
            Duration::ZERO,
            Some(&first),
            false,
        );
        let second = sample(2);
        read_at(&island, &mut surface, t0, 2 * s, Some(&second));
        // The page opens and already shows the newest reading: a reading
        // equal to it still leaves the bar's copy due.
        island.open(PageId::SYSMON, t0 + 3 * s);
        frame_at(&mut island, &mut surface, t0, 3 * s, Some(&second), false);
        assert_eq!(island.page_sysmon(), Some(&second));
        assert_eq!(
            read_at(&island, &mut surface, t0, 4 * s, Some(&second)),
            Wake::After(6 * s)
        );
        assert_eq!(surface.due(t0), Some(10 * s));
        // A new reading while the page is open is a frame now, which keeps
        // the copy's deadline asked for.
        let third = sample(3);
        assert_eq!(
            read_at(&island, &mut surface, t0, 6 * s, Some(&third)),
            Wake::Now
        );
        frame_at(&mut island, &mut surface, t0, 6 * s, Some(&third), false);
        assert_eq!(surface.due(t0), Some(10 * s));
        // The page closes: the copy is still due, and arrives.
        island.close(t0 + 7 * s);
        frame_at(&mut island, &mut surface, t0, 7 * s, Some(&third), false);
        assert_eq!(surface.due(t0), Some(10 * s));
        frame_at(&mut island, &mut surface, t0, 10 * s, Some(&third), true);
        assert_eq!(island.bar_sysmon(), Some(&third));
        assert_eq!(surface.due(t0), None);
    }

    #[test]
    fn an_open_page_takes_every_sample_and_no_bar_wakes_nothing() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        // Bar off and closed: nothing reads the system monitor.
        assert_eq!(island.sysmon_wake(Some(&sample(1)), t0), Wake::Nothing);
        island.open(PageId::SYSMON, t0);
        assert_eq!(island.sysmon_wake(Some(&sample(1)), t0), Wake::Now);
        island.stale = false;
        island.mirror_sysmon(Some(&sample(1)), t0);
        assert!(island.stale);
        let soon = t0 + Duration::from_secs(2);
        assert_eq!(island.sysmon_wake(Some(&sample(2)), soon), Wake::Now);
        island.mirror_sysmon(Some(&sample(2)), soon);
        assert_eq!(island.page_sysmon(), Some(&sample(2)));
        // The bar's value is held back all the same.
        assert_eq!(island.bar_sysmon(), Some(&sample(1)));
        assert_eq!(island.sysmon_wake(Some(&sample(2)), soon), Wake::Nothing);
    }

    /// The default layout with the glance pieces added to the end zone.
    fn glance_layout() -> BarLayout {
        let mut layout = BarLayout::default();
        layout.end.extend(
            ["sysmon", "weather", "clock"]
                .iter()
                .map(|id| (*id).to_owned()),
        );
        layout
    }

    fn some_weather() -> WeatherReading {
        WeatherReading {
            celsius: 12,
            code: 0,
            daylight: true,
            label: "Madrid".to_owned(),
            taken_unix: 0,
        }
    }

    #[test]
    fn the_glance_pieces_read_the_bars_slices_and_a_click_opens_their_page_sheets() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_bar(&glance_layout());
        // Nothing to read yet: no sysmon, no weather; the date alone shows.
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(island
            .bar
            .as_ref()
            .and_then(|b| b.node_rect(Key::named("sysmon")))
            .is_none());
        island.set_today(1_792_281_480, 0);
        island.mirror_sysmon(Some(&sample(23)), t0);
        island.weather = Some(some_weather());
        island.invalidate_bar();
        // Pieces that just appeared fade in; a click needs them visible.
        let t0 = t0 + Duration::from_secs(1);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let t0 = t0 + Duration::from_secs(1);
        lay_fake(&mut island, t0);

        // The bar reads the held-back sample, the page the newest.
        let soon = t0 + Duration::from_secs(2);
        island.mirror_sysmon(Some(&sample(77)), soon);
        assert_eq!(island.bar_cx().sysmon, Some(&sample(23)));
        assert_eq!(island.page_sysmon(), Some(&sample(77)));

        let mut t = t0;
        for (key, page) in [
            (Key::named("sysmon"), PageId::SYSMON),
            (Key::named("weather"), PageId::CLOCK),
            (Key::named("clock-date"), PageId::CLOCK),
        ] {
            island.close_everything(t);
            t += Duration::from_millis(400);
            step(&mut island, t);
            let at = centre(bar_rect(&island, key));
            assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
            step(&mut island, t);
            assert_eq!(island.sheets.page(), Some(page), "{key:?}");
            assert_eq!(island.sheets.piece(), Some(key));
            assert_eq!(island.page(), None, "the Island's page stays shut");
        }
    }

    #[test]
    fn the_calendar_buttons_move_the_month_within_bounds_and_closing_resets_it() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.set_today(1_792_281_480, 0);
        island.open(PageId::CLOCK, t0);
        island.stale = false;
        assert_eq!(island.activate(clock_page::next_key(), t0), None);
        assert_eq!(island.month_offset, 1);
        assert!(island.stale, "a month change redraws the page");
        assert_eq!(island.activate(clock_page::next_key(), t0), None);
        assert_eq!(island.activate(clock_page::previous_key(), t0), None);
        assert_eq!(island.month_offset, 1);
        island.stale = false;
        assert_eq!(island.activate(clock_page::today_key(), t0), None);
        assert_eq!(island.month_offset, 0);
        assert!(island.stale);
        // Today again changes nothing.
        island.stale = false;
        island.activate(clock_page::today_key(), t0);
        assert!(!island.stale);
        // The bound holds however long the button is held.
        for _ in 0..(2 * clock_page::MAX_OFFSET + 10) {
            island.activate(clock_page::next_key(), t0);
        }
        assert_eq!(island.month_offset, clock_page::MAX_OFFSET);
        // Closing the page forgets the month; reopening shows this one.
        island.close(t0);
        assert_eq!(island.month_offset, 0);
        // The page is built from the offset it has.
        island.activate(clock_page::previous_key(), t0);
        assert_eq!(island.month_offset, -1);
    }

    #[test]
    fn the_monitor_row_asks_the_app_to_open_the_resource_monitor() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::SYSMON, t0);
        assert_eq!(
            island.activate(sysmon_page::open_key(), t0),
            Some(IslandAction::OpenApp("org.celestina.Hematita.desktop"))
        );
        assert_eq!(island.page(), Some(PageId::SYSMON));
    }

    #[test]
    fn only_the_system_page_reads_the_newest_sample() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.open(PageId::ACTIVITY, t0);
        assert_eq!(island.sysmon_wake(Some(&sample(1)), t0), Wake::Nothing);
        island.stale = false;
        island.mirror_sysmon(Some(&sample(1)), t0);
        assert!(!island.stale, "the Activity page does not read it");
        // Sliding to the System page makes the next change a frame.
        island.open(PageId::SYSMON, t0);
        assert_eq!(island.sysmon_wake(Some(&sample(2)), t0), Wake::Now);
        island.stale = false;
        island.mirror_sysmon(Some(&sample(2)), t0);
        assert!(island.stale);
        // And sliding away stops the frames again.
        island.open(PageId::CLOCK, t0);
        assert_eq!(island.sysmon_wake(Some(&sample(3)), t0), Wake::Nothing);
    }

    #[test]
    fn a_new_day_rebuilds_the_bar_and_a_new_minute_does_not() {
        let (mut island, t0) = bar_island(1920.0);
        // 2026-10-17 23:58 UTC.
        let late = 1_792_281_480;
        island.set_today(late, 0);
        assert_eq!(
            island.today().map(|t| (t.year, t.month, t.day)),
            Some((2026, 10, 17))
        );
        assert!(bar_stale(&island));
        lay_fake(&mut island, t0);
        island.set_today(late + 60, 0);
        assert!(!bar_stale(&island));
        island.set_today(late + 180, 0);
        assert_eq!(island.today().map(|t| t.day), Some(18));
        assert!(bar_stale(&island));
        // The same instant two hours east is already the next day.
        island.set_today(late, 7_200);
        assert_eq!(island.today().map(|t| t.day), Some(18));
    }

    // The Clipboard page (CEL-2-G).

    /// An open Clipboard page with the text rows `texts`, laid out.
    fn clip_island(texts: &[&str]) -> (Island, Instant) {
        use crate::clipboard::worker::{ClipboardEvent, Preview, Row};
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.set_reduced_motion(true);
        let rows: Vec<Row> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| Row {
                index,
                preview: Preview::Text((*text).to_owned()),
            })
            .collect();
        let total = rows.len();
        island.clipboard.apply(ClipboardEvent::Changed {
            rows,
            total,
            snapshot: 1,
        });
        island.clipboard_on = true;
        island.rebuild_clipboard(&mut clipboard_page::FakePixmaps::default());
        island.open(PageId::CLIPBOARD, t0);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        (island, t0)
    }

    fn key_event(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        }
    }

    fn key_release(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: false,
            serial: 0,
        }
    }

    fn focused_key(island: &Island) -> Option<Key> {
        island
            .page
            .highlight()
            .focused
            .map(|id| island.page.key_of(id))
    }

    /// Tab until the node `want` has the focus.
    fn focus(island: &mut Island, want: Key, now: Instant) {
        for _ in 0..40 {
            feed(island, key_event(0xff09), now);
            let on = island.page.highlight().focused;
            if on.is_some_and(|id| island.page.key_of(id) == want) {
                return;
            }
        }
        panic!("the focus never reached {want:?}");
    }

    const ENTER: u32 = 0xff0d;

    #[test]
    fn enter_on_a_clipboard_row_selects_that_entry() {
        let (mut island, t0) = clip_island(&["first", "second"]);
        focus(&mut island, clipboard_page::row_key(1), t0);
        assert_eq!(
            feed(&mut island, key_event(ENTER), t0),
            vec![IslandAction::Clipboard(ClipboardVerb::SelectShown {
                index: 1,
                snapshot: 1
            })]
        );
    }

    #[test]
    fn a_click_on_a_clipboard_row_selects_that_entry() {
        let (mut island, t0) = clip_island(&["first", "second"]);
        let row = island
            .page
            .node_by_key(clipboard_page::row_key(0))
            .map(|id| island.page.rect(id))
            .expect("a laid out row");
        let actions = click_at(&mut island, centre(row), BTN_LEFT, t0);
        assert_eq!(
            actions,
            vec![IslandAction::Clipboard(ClipboardVerb::SelectShown {
                index: 0,
                snapshot: 1
            })]
        );
    }

    #[test]
    fn delete_removes_only_the_focused_clipboard_entry() {
        let (mut island, t0) = clip_island(&["first", "second"]);
        assert!(
            feed(&mut island, key_event(XKB_KEY_DELETE), t0).is_empty(),
            "nothing focused"
        );
        feed(&mut island, key_release(XKB_KEY_DELETE), t0);
        focus(&mut island, clipboard_page::row_key(1), t0);
        let remove_1 = vec![IslandAction::Clipboard(ClipboardVerb::RemoveShown {
            index: 1,
            snapshot: 1,
        })];
        assert_eq!(feed(&mut island, key_event(XKB_KEY_DELETE), t0), remove_1);
        feed(&mut island, key_release(XKB_KEY_DELETE), t0);
        assert_eq!(
            feed(&mut island, key_event(XKB_KEY_KP_DELETE), t0),
            remove_1
        );
        feed(&mut island, key_release(XKB_KEY_KP_DELETE), t0);
        // On a button it removes nothing.
        focus(&mut island, clipboard_page::pause_key(), t0);
        assert!(feed(&mut island, key_event(XKB_KEY_DELETE), t0).is_empty());
    }

    #[test]
    fn a_held_delete_removes_one_entry_not_all_of_them() {
        let (mut island, t0) = clip_island(&["first", "second", "third"]);
        focus(&mut island, clipboard_page::row_key(0), t0);
        let first = feed(&mut island, key_event(XKB_KEY_DELETE), t0);
        assert_eq!(first.len(), 1);
        // The auto-repeat is more presses with no release between them.
        for _ in 0..5 {
            assert!(feed(&mut island, key_event(XKB_KEY_DELETE), t0).is_empty());
        }
        // The keypad's Delete is the same key held.
        assert!(feed(&mut island, key_event(XKB_KEY_KP_DELETE), t0).is_empty());
        feed(&mut island, key_release(XKB_KEY_DELETE), t0);
        assert_eq!(
            feed(&mut island, key_event(XKB_KEY_DELETE), t0).len(),
            1,
            "a new press after the release acts again"
        );
        // Closing forgets a key whose release was never seen.
        island.close(t0);
        island.open(PageId::CLIPBOARD, t0);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        focus(&mut island, clipboard_page::row_key(0), t0);
        assert_eq!(feed(&mut island, key_event(XKB_KEY_DELETE), t0).len(), 1);
    }

    #[test]
    fn the_bar_redraws_for_the_duty_and_the_pause_but_not_for_rows() {
        use crate::clipboard::worker::ClipboardEvent;
        let mut a = ClipboardState::default();
        let mut b = ClipboardState::default();
        assert!(!bar_clipboard_changed(true, &a, true, &b));
        assert!(bar_clipboard_changed(true, &a, false, &b), "duty toggled");
        b.apply(ClipboardEvent::Changed {
            rows: Vec::new(),
            total: 3,
            snapshot: 2,
        });
        assert!(!bar_clipboard_changed(true, &a, true, &b), "rows only");
        a.apply(ClipboardEvent::Paused(true));
        assert!(bar_clipboard_changed(true, &a, true, &b), "pause toggled");
    }

    #[test]
    fn only_an_island_showing_the_clipboard_page_is_a_clipboard_page_redraw() {
        let (mut island, t0) = clip_island(&["a"]);
        assert!(island.shows_clipboard_page());
        island.open(PageId::TRAY, t0);
        assert!(!island.shows_clipboard_page());
        island.close(t0);
        assert!(!island.shows_clipboard_page());
    }

    #[test]
    fn delete_does_nothing_off_the_clipboard_page() {
        let (mut island, t0) = clip_island(&["first"]);
        focus(&mut island, clipboard_page::row_key(0), t0);
        island.open(PageId::TRAY, t0);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(feed(&mut island, key_event(XKB_KEY_DELETE), t0).is_empty());
    }

    #[test]
    fn clearing_needs_a_confirmation_and_closing_forgets_it() {
        let (mut island, t0) = clip_island(&["first"]);
        focus(&mut island, clipboard_page::clear_key(), t0);
        // The first Enter only asks.
        assert!(feed(&mut island, key_event(ENTER), t0).is_empty());
        assert!(island.confirming_clear && island.clip_panel.confirming_clear);
        assert!(island.stale, "the page is drawn again with the question");
        // The question is drawn, and Cancel (the safe button) holds the
        // keyboard.
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert_eq!(focused_key(&island), Some(clipboard_page::cancel_key()));
        feed(&mut island, key_event(0xff09), t0);
        assert!(island
            .page
            .node_by_key(clipboard_page::confirm_key())
            .is_some());
        // Confirming sends the verb and ends the question.
        focus(&mut island, clipboard_page::confirm_key(), t0);
        assert_eq!(
            feed(&mut island, key_event(ENTER), t0),
            vec![IslandAction::Clipboard(ClipboardVerb::Clear)]
        );
        assert!(!island.confirming_clear && !island.clip_panel.confirming_clear);
    }

    #[test]
    fn cancelling_closing_or_leaving_the_page_gives_the_clear_up() {
        let (mut island, t0) = clip_island(&["first"]);
        let ask = |island: &mut Island| {
            focus(island, clipboard_page::clear_key(), t0);
            feed(island, key_event(ENTER), t0);
            assert!(island.confirming_clear);
        };
        ask(&mut island);
        focus(&mut island, clipboard_page::cancel_key(), t0);
        assert!(feed(&mut island, key_event(ENTER), t0).is_empty());
        assert!(!island.confirming_clear);
        // Giving up puts the keyboard back on Clear.
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert_eq!(focused_key(&island), Some(clipboard_page::clear_key()));

        ask(&mut island);
        island.close(t0);
        assert!(!island.confirming_clear && !island.clip_panel.confirming_clear);

        let (mut island, t0) = clip_island(&["first"]);
        ask(&mut island);
        island.open(PageId::TRAY, t0);
        assert!(!island.confirming_clear);
        // Nothing left to clear: the question goes with the entries.
        let (mut island, _) = clip_island(&["first"]);
        ask(&mut island);
        island
            .clipboard
            .apply(crate::clipboard::worker::ClipboardEvent::Changed {
                rows: Vec::new(),
                total: 0,
                snapshot: 2,
            });
        island.rebuild_clipboard(&mut clipboard_page::FakePixmaps::default());
        assert!(!island.confirming_clear && !island.clip_panel.confirming_clear);
    }

    // Media (CEL-2-D).

    /// Hands out ids from 1 and remembers what it was asked.
    #[derive(Default)]
    struct FakePixmaps {
        next: u32,
        uploads: Vec<(Vec<u8>, u32, u32)>,
        released: Vec<PixmapId>,
    }

    impl Pixmaps for FakePixmaps {
        fn upload(&mut self, argb: &[u8], w: u32, h: u32) -> Option<PixmapId> {
            self.next += 1;
            self.uploads.push((argb.to_vec(), w, h));
            Some(PixmapId(self.next))
        }

        fn release(&mut self, id: PixmapId) {
            self.released.push(id);
        }

        fn has_theme_icon(&self, _: &str, _: u32) -> bool {
            false
        }
    }

    fn player(n: u32, playing: bool, art: &str) -> PlayerView {
        PlayerView {
            bus_name: format!("org.mpris.MediaPlayer2.p{n}"),
            identity: format!("p{n}"),
            playing,
            title: "Title".to_owned(),
            artist: "Artist".to_owned(),
            album: String::new(),
            art_url: art.to_owned(),
            length_ms: 100_000,
            position_ms: 1_000,
            pinned: false,
        }
    }

    fn cover(rgba: [u8; 4]) -> Cover {
        Cover {
            width: 2,
            height: 2,
            rgba: rgba.repeat(4),
        }
    }

    /// A bar island whose end zone holds the Media piece.
    fn media_island() -> (Island, Instant) {
        let (mut island, t0) = bar_island(1920.0);
        let mut layout = BarLayout::default();
        layout.end.push("media".to_owned());
        island.set_bar(&layout);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        island.stale = false;
        (island, t0)
    }

    fn media_key() -> Key {
        Key::named("media")
    }

    #[test]
    fn the_bar_shows_the_media_piece_only_while_a_player_is_shown() {
        let (mut island, t0) = media_island();
        let mut px = FakePixmaps::default();
        assert!(island
            .bar
            .as_ref()
            .is_some_and(|b| b.node_rect(media_key()).is_none()));
        let players = [player(1, true, "")];
        assert!(island.set_media(&mut px, true, &players, Some(&players[0].bus_name)));
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(island
            .bar
            .as_ref()
            .is_some_and(|b| b.node_rect(media_key()).is_some()));
        // The player leaves: the bar is asked for again (the piece draws
        // nothing, as the piece's own tests pin).
        assert!(island.set_media(&mut px, true, &[], None));
        assert!(pieces::media::bar_signature(&island.media).is_none());
    }

    #[test]
    fn a_position_tick_costs_no_frame_unless_the_media_page_is_open() {
        let (mut island, t0) = media_island();
        let mut px = FakePixmaps::default();
        let mut players = vec![player(1, true, "")];
        let bus = players[0].bus_name.clone();
        assert!(island.set_media(&mut px, true, &players, Some(&bus)));
        // At rest, and with another page open: progress changes nothing
        // anyone sees.
        for page in [None, Some(PageId::TRAY)] {
            if let Some(page) = page {
                island.open(page, t0);
            }
            island.stale = false;
            players[0].position_ms += 1_000;
            assert!(
                !island.set_media(&mut px, true, &players, Some(&bus)),
                "{page:?}"
            );
            assert!(!island.stale, "{page:?}");
        }
        // The same state again is nothing too.
        assert!(!island.set_media(&mut px, true, &players, Some(&bus)));
        // With the Media page open the progress bar moves.
        island.open(PageId::MEDIA, t0);
        island.stale = false;
        players[0].position_ms += 1_000;
        assert!(island.set_media(&mut px, true, &players, Some(&bus)));
        assert!(island.stale);
        // What the bar shows changing redraws it at rest.
        island.close(t0);
        island.state.close(t0, true);
        island.stale = false;
        players[0].playing = false;
        assert!(island.set_media(&mut px, true, &players, Some(&bus)));
    }

    #[test]
    fn a_page_button_and_the_pieces_middle_click_become_media_actions() {
        let (mut island, t0) = media_island();
        let mut px = FakePixmaps::default();
        let players = [player(1, true, ""), player(2, false, "")];
        island.set_media(&mut px, true, &players, Some(&players[0].bus_name));
        // Let the piece's entrance settle.
        let t0 = t0 + Duration::from_secs(2);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let t0 = t0 + Duration::from_secs(2);
        lay_fake(&mut island, t0);
        let centre_of = bar_rect(&island, media_key());
        assert_eq!(
            click_at(
                &mut island,
                (centre_of.x + 12.0, centre(centre_of).1),
                BTN_MIDDLE,
                t0
            ),
            [IslandAction::Media(MediaCommand::PlayPause)]
        );
        // The piece's click opens the page as a sheet.
        click_at(&mut island, centre(centre_of), BTN_LEFT, t0);
        assert_eq!(island.sheets.page(), Some(PageId::MEDIA));
        // The page's keys act like their buttons.
        for (key, want) in [
            (media_page::prev_key(), MediaCommand::Previous),
            (media_page::play_key(), MediaCommand::PlayPause),
            (media_page::next_key(), MediaCommand::Next),
            (
                media_page::player_key(&players[1].bus_name),
                MediaCommand::Pin(Some(players[1].bus_name.clone())),
            ),
        ] {
            assert_eq!(
                island.page_action(PageId::MEDIA, key, t0),
                Some(IslandAction::Media(want)),
            );
        }
        // A duty switched off takes the buttons away.
        island.set_media(&mut px, false, &[], None);
        assert_eq!(
            island.page_action(PageId::MEDIA, media_page::play_key(), t0),
            None
        );
    }

    #[test]
    fn a_cover_is_uploaded_once_per_art_and_released_when_it_goes() {
        let (mut island, _) = media_island();
        let mut px = FakePixmaps::default();
        let players = [
            player(1, true, "file:///a.png"),
            player(2, false, "file:///b.png"),
        ];
        let bus = players[0].bus_name.clone();
        island.set_media(&mut px, true, &players, Some(&bus));
        assert!(island.media.cover.is_none());
        // RGBA in, ARGB out.
        island.set_media_cover(&mut px, &bus, "file:///a.png", Some(&cover([1, 2, 3, 255])));
        assert_eq!(px.uploads.len(), 1);
        assert_eq!(px.uploads[0].0[..4], [255, 1, 2, 3]);
        assert_eq!((px.uploads[0].1, px.uploads[0].2), (2, 2));
        assert_eq!(island.media.cover, Some(PixmapId(1)));
        // The same position update keeps the texture.
        island.set_media(&mut px, true, &players, Some(&bus));
        assert_eq!(island.media.cover, Some(PixmapId(1)));
        assert!(px.released.is_empty());
        // A cover for art the player no longer names, or for a stranger, is
        // dropped before it costs a texture.
        assert!(!island.set_media_cover(&mut px, &bus, "file:///old.png", Some(&cover([0; 4]))));
        assert!(!island.set_media_cover(
            &mut px,
            "org.mpris.MediaPlayer2.x",
            "file:///a.png",
            Some(&cover([0; 4]))
        ));
        assert_eq!(px.uploads.len(), 1);
        // Another player's cover is kept for later but not drawn.
        let other = players[1].bus_name.clone();
        island.set_media_cover(&mut px, &other, "file:///b.png", Some(&cover([9; 4])));
        assert_eq!(island.media.cover, Some(PixmapId(1)));
        // The art changes: the old texture goes at once.
        let mut next = players.clone();
        next[0].art_url = "file:///c.png".to_owned();
        island.set_media(&mut px, true, &next, Some(&bus));
        assert_eq!(px.released, [PixmapId(1)]);
        assert!(island.media.cover.is_none());
        // A refused decode (None) leaves no cover; a cover is held until its
        // art changes.
        island.set_media_cover(&mut px, &bus, "file:///c.png", None);
        assert!(island.media.cover.is_none());
        island.set_media_cover(&mut px, &bus, "file:///c.png", Some(&cover([5; 4])));
        let id = island.media.cover;
        assert!(id.is_some());
        let uploads = px.uploads.len();
        assert!(!island.set_media_cover(&mut px, &bus, "file:///c.png", Some(&cover([6; 4]))));
        assert_eq!((island.media.cover, px.uploads.len()), (id, uploads));
        // A player that leaves releases its texture; the duty off, the rest.
        island.set_media(&mut px, true, &next[..1], Some(&bus));
        assert!(px.released.contains(&PixmapId(2)));
        let live = island.media.cover.expect("cover");
        assert_eq!(Some(live), id);
        island.set_media(&mut px, false, &[], None);
        assert!(px.released.contains(&live));
        assert!(island.covers.is_empty() && island.media.cover.is_none());
    }

    #[test]
    fn a_malformed_cover_is_refused_and_a_cover_change_redraws_only_the_open_page() {
        let (mut island, t0) = media_island();
        let mut px = FakePixmaps::default();
        let players = [player(1, true, "file:///a.png")];
        let bus = players[0].bus_name.clone();
        island.set_media(&mut px, true, &players, Some(&bus));
        let short = Cover {
            width: 4,
            height: 4,
            rgba: vec![0; 10],
        };
        assert!(!island.set_media_cover(&mut px, &bus, "file:///a.png", Some(&short)));
        assert!(px.uploads.is_empty());
        // The bar does not draw covers: at rest no frame.
        island.stale = false;
        assert!(!island.set_media_cover(&mut px, &bus, "file:///a.png", Some(&cover([1; 4]))));
        assert!(!island.stale);
        island.open(PageId::MEDIA, t0);
        let next = [player(1, true, "file:///b.png")];
        island.set_media(&mut px, true, &next, Some(&bus));
        island.stale = false;
        assert!(island.set_media_cover(&mut px, &bus, "file:///b.png", Some(&cover([2; 4]))));
        assert!(island.stale);
    }

    #[test]
    fn a_players_event_every_second_uploads_each_cover_once() {
        let (mut island, _) = media_island();
        let mut px = FakePixmaps::default();
        let players = [
            player(1, true, "file:///a.png"),
            player(2, false, "file:///b.png"),
        ];
        let bus = players[0].bus_name.clone();
        let covers = [
            (players[0].bus_name.clone(), "file:///a.png", cover([1; 4])),
            (players[1].bus_name.clone(), "file:///b.png", cover([2; 4])),
        ];
        // What the loop does at each players event: the players, then every
        // cover of the snapshot again.
        for _ in 0..5 {
            island.set_media(&mut px, true, &players, Some(&bus));
            for (name, url, c) in &covers {
                island.set_media_cover(&mut px, name, url, Some(c));
            }
        }
        assert_eq!(px.uploads.len(), 2, "one upload per cover");
        assert!(px.released.is_empty());
    }

    #[test]
    fn an_unavailable_bus_takes_the_players_off_the_views_until_it_is_back() {
        let (mut island, _) = media_island();
        let mut px = FakePixmaps::default();
        let players = [player(1, true, "")];
        let bus = players[0].bus_name.clone();
        island.set_media(&mut px, true, &players, Some(&bus));
        assert!(pieces::media::bar_signature(&island.media).is_some());
        assert!(island.set_media_unavailable(true));
        assert!(!island.set_media_unavailable(true), "said twice: no frame");
        assert!(island.media.shown().is_none());
        assert!(pieces::media::bar_signature(&island.media).is_none());
        // A players event keeps the mark until the control clears it.
        island.set_media(&mut px, true, &players, Some(&bus));
        assert!(island.media.unavailable);
        assert!(island.set_media_unavailable(false));
        assert!(island.media.shown().is_some());
        // Off clears it for good.
        island.set_media_unavailable(true);
        island.set_media(&mut px, false, &[], None);
        assert!(!island.media.unavailable);
        assert!(!island.set_media_unavailable(true), "nothing runs");
    }
    // The capsule sheets (CEL-D-002).

    use crate::osd::monitor_rows::MonitorFeed;
    use crate::osd::popup::{
        body_size, Feeds, LevelCore, Request as LevelRequest, SharedGraph, SharedLevels, Sink,
        SHEET_W,
    };
    use crate::pieces::Level;
    use crate::sheet::geometry::{self, Hit};
    use celestina_shell::audio::AudioGraph;
    use std::{cell::Cell, cell::RefCell, rc::Rc, time::Duration};

    const ESC: u32 = 0xff1b;

    fn escape() -> InputEvent {
        InputEvent::Key {
            keysym: ESC,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            pressed: true,
            serial: 0,
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// A volume body at 40 % that sends its requests nowhere.
    fn volume_core(theme: &Theme) -> LevelCore {
        sent_core(
            theme,
            &Rc::new(RefCell::new(Vec::new())),
            AudioGraph::default(),
        )
    }

    /// A volume body over `graph` whose requests land in `sent`.
    fn sent_core(
        theme: &Theme,
        sent: &Rc<RefCell<Vec<LevelRequest>>>,
        graph: AudioGraph,
    ) -> LevelCore {
        sent_core_with(theme, sent, graph, false)
    }

    /// [`sent_core`], with the microphone's section when `mic`.
    fn sent_core_with(
        theme: &Theme,
        sent: &Rc<RefCell<Vec<LevelRequest>>>,
        graph: AudioGraph,
        mic: bool,
    ) -> LevelCore {
        let sink: Sink = {
            let sent = Rc::clone(sent);
            Rc::new(move |request| sent.borrow_mut().push(request))
        };
        let levels = Levels {
            audio: AudioLevels {
                output: Some(Level {
                    percent: 40,
                    muted: false,
                }),
                input: mic.then_some(Level {
                    percent: 30,
                    muted: false,
                }),
            },
            brightness: None,
        };
        let cell: SharedLevels = Rc::new(Cell::new(levels));
        let graph: SharedGraph = Rc::new(RefCell::new(graph));
        let mut fit = |name: &str, _: f32| name.to_owned();
        LevelCore::new(
            theme,
            level_popup::Kind::Volume,
            Feeds {
                cell,
                graph,
                monitors: MonitorFeed::default(),
            },
            sink,
            false,
            Instant::now(),
            &mut fit,
        )
    }

    /// The bar with the volume and brightness pieces shown, a second after
    /// they appeared (their nodes have faded in).
    fn level_island() -> (Island, Instant) {
        let (mut island, t0) = bar_island(1920.0);
        island.levels = Levels {
            audio: AudioLevels {
                output: Some(Level {
                    percent: 40,
                    muted: false,
                }),
                input: None,
            },
            brightness: Some(70),
        };
        island.invalidate_bar();
        lay_fake(&mut island, t0);
        (island, t0 + Duration::from_secs(1))
    }

    /// The frame's parts that move the sheets, at `now`.
    fn step(island: &mut Island, now: Instant) {
        island.sheets_before_layout(now, &mut fake_measure);
        if island.stale {
            island.apply_views(now);
        }
        lay_fake(island, now);
        island.sheets_after_layout(now);
    }

    /// Frames every 16 ms from `from` to `to`, as the surface runs them while
    /// something moves.
    fn run_frames(island: &mut Island, from: Instant, to: Instant) {
        let mut t = from;
        while t < to {
            t = (t + ms(16)).min(to);
            step(island, t);
        }
    }

    fn volume() -> Key {
        Key::named("volume")
    }

    fn brightness_key() -> Key {
        Key::named("brightness")
    }

    /// Opens the volume sheet on `piece`, as the app does after the action.
    fn open_on(island: &mut Island, piece: Key, now: Instant) -> bool {
        let core = volume_core(&island.theme);
        let opened = island.open_level_sheet_at(piece, core, now);
        step(island, now);
        opened
    }

    fn end_capsule(island: &Island) -> Rect {
        zone(island.bar.as_ref().and_then(|b| b.zone_rect(3)))
    }

    fn near(a: Rect, b: Rect) -> bool {
        (a.x - b.x).abs() < 0.01
            && (a.y - b.y).abs() < 0.01
            && (a.w - b.w).abs() < 0.01
            && (a.h - b.h).abs() < 0.01
    }

    fn shown_rect(island: &Island, now: Instant) -> Rect {
        match island.sheet_rect(now) {
            Some(r) => r,
            None => panic!("no sheet shown at {now:?}"),
        }
    }

    /// An open sheet, settled, with the volume piece open.
    fn settled() -> (Island, Instant) {
        let (mut island, t0) = level_island();
        assert!(open_on(&mut island, volume(), t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        (island, t)
    }

    /// A brightness body at 70 % over one monitor, its requests sent nowhere.
    fn brightness_core(theme: &Theme) -> LevelCore {
        let sink: Sink = Rc::new(|_| {});
        let cell: SharedLevels = Rc::new(Cell::new(Levels {
            audio: AudioLevels::default(),
            brightness: Some(70),
        }));
        let graph: SharedGraph = Rc::new(RefCell::new(AudioGraph::default()));
        let mut fit = |name: &str, _: f32| name.to_owned();
        LevelCore::new(
            theme,
            level_popup::Kind::Brightness,
            Feeds {
                cell,
                graph,
                monitors: MonitorFeed::default(),
            },
            sink,
            false,
            Instant::now(),
            &mut fit,
        )
    }

    #[test]
    fn the_brightness_piece_opens_its_sheet() {
        let (mut island, t0) = level_island();
        let at = centre(bar_rect(&island, brightness_key()));
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t0),
            [IslandAction::OpenLevelSheet {
                kind: level_popup::Kind::Brightness,
                piece: brightness_key(),
            }]
        );
        assert!(!island.sheets.is_open(), "the app builds the body first");
        // The app hands the body back: the sheet grows from the end capsule.
        let core = brightness_core(&island.theme);
        let body = core.size();
        assert_eq!(body.0, SHEET_W);
        assert!(island.open_level_sheet_at(brightness_key(), core, t0));
        step(&mut island, t0);
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Brightness)
        );
        assert!(island.level_sheet_open());
        // The piece asking again closes it (it stays in its row, under the pill).
        let later = t0 + ms(500);
        assert_eq!(
            island.sheet_request(
                brightness_key(),
                crate::pieces::SheetKind::Brightness,
                later,
                &mut fake_measure
            ),
            None
        );
        assert!(!island.sheets.is_open());
    }

    #[test]
    fn switching_the_volume_sheet_to_the_brightness_sheet_morphs_in_place() {
        let (mut island, t0) = level_island();
        assert!(open_on(&mut island, volume(), t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let core = brightness_core(&island.theme);
        assert!(island.open_level_sheet_at(brightness_key(), core, t));
        step(&mut island, t);
        // One sheet, switched: it never closed and nothing waits, and no
        // second window is left behind.
        assert!(island.sheets.is_open());
        assert!(!island.sheets.has_queued());
        assert!(!island.sheets.is_leaving());
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Brightness)
        );
        assert_eq!(island.sheets.piece(), Some(brightness_key()));
        assert!(island.level_sheet_open());
    }

    #[test]
    fn a_volume_click_asks_for_its_sheet_at_its_node() {
        let (mut island, t0) = level_island();
        let at = centre(bar_rect(&island, volume()));
        assert_eq!(
            click_at(&mut island, at, BTN_LEFT, t0),
            [IslandAction::OpenLevelSheet {
                kind: level_popup::Kind::Volume,
                piece: Key::named("volume"),
            }]
        );
        assert!(!island.sheets.is_open(), "the app builds the body first");
    }

    #[test]
    fn the_window_grows_from_the_open_pieces_pill_and_settles_on_its_target() {
        let (mut island, t0) = level_island();
        let end = end_capsule(&island);
        let piece = bar_rect(&island, volume());
        assert!(open_on(&mut island, volume(), t0));
        // The drop starts as a bulb on the capsule's edge, under its piece.
        let start = shown_rect(&island, t0);
        assert!(
            (start.x + start.w / 2.0 - (piece.x + piece.w / 2.0)).abs() < 0.01
                && start.w <= piece.w,
            "starts under the pill: {start:?}"
        );
        let body = volume_core(&island.theme).size();
        assert_eq!(
            body,
            body_size(
                &island.theme,
                level_popup::Kind::Volume,
                false,
                Default::default()
            )
        );
        let settle = t0 + ms(460);
        step(&mut island, settle);
        let target = geometry::window(end, geometry::Edge::End, body, island.size);
        assert!(
            near(shown_rect(&island, settle), target),
            "{:?} vs {target:?}",
            shown_rect(&island, settle)
        );
        assert_eq!(target.w, SHEET_W);
        // Midway it is between the two.
        let mid = shown_rect(&island, t0 + ms(300));
        assert!(mid.h > piece.h && mid.h <= target.h * 1.2);
        assert_eq!(target.y, geometry::WINDOW_TOP);
        assert_eq!(target.x + target.w, end.x + end.w, "the right edges agree");
    }

    /// The bar with thirty workspaces: its start capsule is wider than a 320
    /// px body, so a sheet from the bell is a window over it.
    fn wide_start_island() -> (Island, Instant) {
        let (mut island, t0) = bar_island(1920.0);
        let mut many = model("DP-1", 2);
        many.workspaces = (1..=30).map(celestina_shell_workspace).collect();
        island.rows = rows_for(&many, "DP-1");
        island.model = Some(many);
        island.stale = true;
        island.invalidate_bar();
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        (island, t0 + Duration::from_secs(1))
    }

    fn celestina_shell_workspace(id: u64) -> crate::providers::niri::Workspace {
        crate::providers::niri::Workspace {
            id,
            idx: u8::try_from(id).unwrap_or(0),
            name: None,
            output: Some("DP-1".to_owned()),
            is_active: id == 2,
            is_focused: id == 2,
            columns: 1,
        }
    }

    #[test]
    fn a_start_window_lines_up_with_its_capsule_and_every_other_piece_stays_on_the_bar() {
        let (mut island, t0) = wide_start_island();
        let bell = Key::named("bell");
        let others = [
            Key::indexed("dot", 1),
            Key::indexed("dot", 20),
            Key::indexed("dot", 30),
        ];
        let before = start_zone(&island);
        assert!(open_on(&mut island, bell, t0));
        let settle = t0 + ms(500);
        run_frames(&mut island, t0, settle);
        // The bell stays in its capsule: the capsule keeps its extent.
        let start = start_zone(&island);
        assert_eq!(start, before, "the bell keeps its slot");
        assert!(
            start.w > SHEET_W,
            "premise: the capsule is wider than the body"
        );
        let sheet = shown_rect(&island, settle);
        assert_eq!(sheet.x, start.x, "its left edge is the capsule's left edge");
        assert_eq!(sheet.y, geometry::WINDOW_TOP);
        // Every other piece stays on the bar, where it is hit: the window
        // is under it.
        for key in others {
            let at = centre(bar_rect(&island, key));
            assert!(start.contains(at.0, at.1), "{key:?} is on the capsule");
            assert!(!sheet.contains(at.0, at.1), "{key:?} is not covered");
        }
        // A press outside the window only closes it.
        let far = (sheet.x + sheet.w + 200.0, sheet.y + 10.0);
        assert!(!sheet.contains(far.0, far.1));
        assert!(click_at(&mut island, far, BTN_LEFT, settle).is_empty());
        assert!(!island.sheets.is_open());
        // A piece in the window acts: the workspaces piece switches the
        // sheet to its own list.
        let (mut island, t0) = wide_start_island();
        assert!(open_on(&mut island, bell, t0));
        let settle = t0 + ms(500);
        step(&mut island, settle);
        let sheet = shown_rect(&island, settle);
        let near = centre(bar_rect(&island, Key::indexed("dot", 30)));
        assert!(
            !sheet.contains(near.0, near.1),
            "on the bar, not the window"
        );
        assert!(click_at(&mut island, near, BTN_LEFT, settle).is_empty());
        assert_eq!(
            island.sheets.kind(),
            Some(crate::pieces::SheetKind::Workspaces)
        );
    }

    #[test]
    fn the_window_keeps_its_capsules_edge_while_the_capsule_keeps_its_extent() {
        // A start window lines up with its capsule's left edge, and the bar
        // paints every capsule whatever the window does: at every frame the
        // capsule's extent is one shape and the window another, and the
        // extent does not change with the open.
        let (mut island, t0) = wide_start_island();
        assert!(open_on(&mut island, Key::named("bell"), t0));
        let mut widths = Vec::new();
        for t in [0, 16, 60, 120, 175, 250, 349, 400, 460] {
            let now = t0 + ms(t);
            step(&mut island, now);
            let extent = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(0, now)));
            let sheet = shown_rect(&island, now);
            let shapes = island.shapes(now);
            assert_eq!(
                shapes.iter().filter(|s| s.0 == extent).count(),
                1,
                "+{t} ms: the capsule keeps its glass"
            );
            // The window is no shape: its glass is a path, and the region
            // holds its bands from 52 % of the open.
            assert_eq!(shapes.iter().filter(|s| s.0 == sheet).count(), 0);
            if t >= 250 {
                let bands = island.blur_bands(now);
                assert!(
                    bands
                        .iter()
                        .any(|b| b.contains(sheet.x + sheet.w / 2.0, sheet.y + sheet.h / 2.0)),
                    "+{t} ms: the window's bands"
                );
            }
            if t >= 460 {
                assert!((sheet.x - extent.x).abs() < 0.01, "{sheet:?} vs {extent:?}");
                assert_eq!(sheet.y, geometry::WINDOW_TOP);
            }
            widths.push(extent.w);
        }
        assert!(
            widths.iter().all(|w| (*w - widths[0]).abs() < 0.01),
            "the capsule kept its width: {widths:?}"
        );
        // An end window lines up with the right edge at rest, whatever the
        // capsule does meanwhile.
        let (mut island, t0) = level_island();
        assert!(open_on(&mut island, volume(), t0));
        for t in [0, 16, 60, 120, 175, 250, 349, 400, 460] {
            let now = t0 + ms(t);
            step(&mut island, now);
            let extent = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(3, now)));
            assert!(
                island
                    .blur_bands(now)
                    .iter()
                    .any(|b| b.contains(extent.x + extent.w / 2.0, extent.y + extent.h / 2.0)),
                "+{t} ms"
            );
        }
        let end = t0 + ms(500);
        let extent = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(3, end)));
        let sheet = shown_rect(&island, end);
        assert!((sheet.x + sheet.w - (extent.x + extent.w)).abs() < 0.01);
        // At rest it has not asked for a frame.
        let rest = t0 + Duration::from_secs(2);
        step(&mut island, rest);
        assert!(!island.bar.as_ref().is_some_and(|b| b.is_animating(rest)));
        assert!(!island.sheets.wants_frame(rest));
    }

    #[test]
    fn an_only_piece_keeps_its_capsule_and_a_window() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_bar(&BarLayout {
            start: vec!["notifications".to_owned()],
            centre: Vec::new(),
            end: Vec::new(),
            tray_hidden: Vec::new(),
        });
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let bell = Key::named("bell");
        let start_before = island.bar.as_ref().and_then(|b| b.capsule_extent(0, t0));
        assert_eq!(
            island.sheet_request(
                bell,
                crate::pieces::SheetKind::Page(PageId::NOTIFICATIONS),
                t0,
                &mut fake_measure
            ),
            None
        );
        for t in [0, 50, 175, 400] {
            let now = t0 + ms(t);
            step(&mut island, now);
            let sheet = shown_rect(&island, now);
            assert!(
                sheet.w > 0.0 && sheet.h > 0.0 && sheet.y >= 8.0,
                "+{t} ms: {sheet:?}"
            );
            assert!(island
                .shapes(now)
                .iter()
                .all(|s| [s.0.x, s.0.y, s.0.w, s.0.h].iter().all(|v| v.is_finite())));
        }
        let now = t0 + ms(500);
        // The capsule keeps its width: the piece stays in it.
        let extent = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(0, now)));
        assert_eq!(Some(extent), start_before);
        let sheet = shown_rect(&island, now);
        assert_eq!(sheet.y, geometry::WINDOW_TOP);
        assert_eq!(
            geometry::hit(sheet, sheet.x + 2.0, sheet.y + 20.0),
            geometry::Hit::Body
        );
        // The pill is the bell's own rect; the window hangs from it.
        let pill = island.bar.as_ref().and_then(|b| b.pill_of(bell, now));
        assert_eq!(pill, Some(bar_rect(&island, bell)));
        assert!(island.sheets.body_size().is_some());
    }

    #[test]
    fn a_second_piece_of_the_same_capsule_switches_in_place() {
        let (mut island, t0) = settled_pair();
        // Open on the volume; the brightness is its neighbour.
        let before = shown_rect(&island, t0);
        let core = volume_core(&island.theme);
        assert!(island.open_level_sheet_at(brightness_key(), core, t0));
        for t in [0, 10, 60, 120, 200, 259, 260, 400] {
            let now = t0 + ms(t);
            step(&mut island, now);
            assert!(island.sheets.is_open(), "open at +{t} ms");
            assert!(!island.state.is_open());
            // One window travels; the old content is held for 90 ms.
            assert!(!island.sheets.is_leaving(), "leaving at +{t} ms");
            assert_eq!(island.sheets.is_swapping(), t < 90, "swapping at +{t} ms");
        }
        assert_eq!(island.sheets.piece(), Some(brightness_key()));
        // The rect started from where it was.
        assert!(shown_rect(&island, t0).w > 0.0 && before.w > 0.0);
    }

    fn settled_pair() -> (Island, Instant) {
        settled()
    }

    #[test]
    fn asking_for_the_open_piece_again_closes_its_sheet() {
        let (mut island, t0) = settled();
        assert_eq!(
            island.sheet_request(
                volume(),
                crate::pieces::SheetKind::Volume,
                t0,
                &mut fake_measure
            ),
            None,
            "no new request"
        );
        assert!(!island.sheets.is_open(), "closing");
        assert!(island.sheets.is_shown(t0 + ms(10)), "still painted");
        step(&mut island, t0 + ms(400));
        assert!(island.sheet_rect(t0 + ms(400)).is_none(), "gone");
    }

    #[test]
    fn a_piece_without_a_sheet_acts_and_closes_it_and_the_wheel_never_does() {
        let (mut island, t0) = level_island();
        // The sheet is open on the brightness piece; the volume is its
        // neighbour in the row.
        assert!(open_on(&mut island, brightness_key(), t0));
        let t0 = t0 + ms(400);
        step(&mut island, t0);
        // The wheel on the volume piece steps the volume; the sheet stays.
        let at = centre(bar_rect(&island, volume()));
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t0,
        );
        let wheel = InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical: -15.0,
        };
        assert_eq!(
            feed(&mut island, wheel, t0),
            [IslandAction::Audio(AudioAction::Step(Direction::Up, 1))]
        );
        assert!(island.sheets.is_open(), "the wheel never closes it");
        // A right click on the volume piece flips the mute and closes it.
        assert_eq!(
            click_at(&mut island, at, BTN_RIGHT, t0),
            [IslandAction::Audio(AudioAction::ToggleMute)]
        );
        assert!(!island.sheets.is_open(), "an acting gesture closes it");
    }

    #[test]
    fn a_press_outside_escape_and_focus_loss_close_the_sheet() {
        for how in 0..3 {
            let (mut island, t0) = settled();
            match how {
                0 => assert!(feed(&mut island, press(5.0, 500.0, BTN_LEFT), t0).is_empty()),
                1 => assert!(feed(&mut island, escape(), t0).is_empty()),
                _ => assert!(feed(&mut island, InputEvent::FocusLost, t0).is_empty()),
            }
            assert!(!island.sheets.is_open(), "way {how}");
            assert!(!island.sheets.has_queued());
        }
    }

    #[test]
    fn the_window_never_covers_the_bar_and_takes_only_its_own_input() {
        // A narrow surface puts the end window next to the Island's capsule.
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (520.0, 560.0));
        island.set_bar(&BarLayout {
            start: vec!["notifications".to_owned()],
            centre: Vec::new(),
            end: vec!["volume".to_owned()],
            tray_hidden: Vec::new(),
        });
        island.levels = Levels {
            audio: AudioLevels {
                output: Some(Level {
                    percent: 40,
                    muted: false,
                }),
                input: None,
            },
            brightness: None,
        };
        island.invalidate_bar();
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        let t0 = t0 + Duration::from_secs(1);
        assert!(open_on(&mut island, volume(), t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let sheet = shown_rect(&island, t);
        let capsule = island.state.rect(t, island.size.0);
        // The window hangs under the bar: it covers none of the capsule.
        assert!(sheet.y >= capsule.y + capsule.h, "{sheet:?} {capsule:?}");
        let over = (
            (sheet.x.max(capsule.x) + (sheet.x + sheet.w).min(capsule.x + capsule.w)) / 2.0,
            capsule.y + capsule.h / 2.0,
        );
        assert!(capsule.contains(over.0, over.1) && !sheet.contains(over.0, over.1));
        assert!(matches!(geometry::hit(sheet, over.0, over.1), Hit::Outside));
        // A press on the window's body is the window's.
        let body = (sheet.x + sheet.w / 2.0, sheet.y + sheet.h - 4.0);
        assert!(click_at(&mut island, body, BTN_LEFT, t).is_empty());
        assert!(!island.state.is_open());
        assert!(island.sheets.is_open());
        // A press on the Island's capsule is the Island's: the window
        // closes, the page waits for it.
        assert!(click_at(&mut island, over, BTN_LEFT, t).is_empty());
        assert!(!island.sheets.is_open());
        assert!(island.sheets.has_queued());
    }

    #[test]
    fn the_island_capsule_while_a_sheet_is_open_closes_it_then_opens_the_page() {
        let (mut island, t0) = settled();
        let capsule = island.state.rect(t0, island.size.0);
        let at = centre(capsule);
        assert!(!shown_rect(&island, t0).contains(at.0, at.1));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        assert!(!island.sheets.is_open() && island.sheets.is_shown(t0 + ms(1)));
        assert!(!island.state.is_open(), "not yet");
        assert!(island.sheets.has_queued());
        step(&mut island, t0 + ms(100));
        assert!(!island.state.is_open(), "still closing at +100 ms");
        step(&mut island, t0 + ms(400));
        assert!(island.state.is_open(), "the page opens at +400 ms");
        assert_eq!(island.page(), Some(PageId::CLOCK), "the clock page");
        assert!(!island.sheets.is_shown(t0 + ms(400)));
    }

    #[test]
    fn a_volume_click_while_a_page_is_open_closes_the_page_then_opens_the_sheet() {
        let (mut island, t0) = level_island();
        island.open(PageId::ACTIVITY, t0);
        step(&mut island, t0 + ms(400));
        let t = t0 + ms(500);
        let at = centre(bar_rect(&island, volume()));
        let asked = click_at(&mut island, at, BTN_LEFT, t);
        assert_eq!(
            asked,
            [IslandAction::OpenLevelSheet {
                kind: level_popup::Kind::Volume,
                piece: volume(),
            }]
        );
        assert!(!island.state.is_open(), "the page is closing");
        // The app answers: the sheet waits for the page to go.
        assert!(open_on(&mut island, volume(), t));
        assert!(!island.sheets.is_shown(t));
        assert!(island.sheets.has_queued());
        assert!(island.level_sheet_open(), "queued counts as open");
        step(&mut island, t + ms(100));
        assert!(!island.sheets.is_shown(t + ms(100)));
        step(&mut island, t + ms(160));
        assert!(
            island.sheets.is_open(),
            "the sheet opens once the page is gone"
        );
        assert!(island.state.progress(t + ms(160)) <= 0.0);
    }

    #[test]
    fn island_close_and_toggle_close_a_sheet_and_open_goes_in_sequence() {
        let (mut island, t0) = settled();
        assert!(island.command_at(&Verb::IslandClose, t0));
        assert!(!island.sheets.is_open());
        step(&mut island, t0 + ms(400));
        assert!(!island.sheets.is_shown(t0 + ms(400)));

        let (mut island, t0) = settled();
        assert!(island.command_at(&Verb::IslandToggle, t0));
        assert!(!island.sheets.is_open(), "toggle closes whichever is open");
        assert!(!island.state.is_open());

        let (mut island, t0) = settled();
        assert!(island.command_at(&Verb::IslandOpen(PageId::ACTIVITY), t0));
        assert!(!island.state.is_open(), "the sheet closes first");
        assert!(!island.sheets.is_open());
        step(&mut island, t0 + ms(400));
        assert_eq!(island.page(), Some(PageId::ACTIVITY));
        // Toggle with nothing open opens the last page.
        step(&mut island, t0 + ms(800));
        assert!(island.command_at(&Verb::IslandToggle, t0 + ms(800)));
        assert!(!island.state.is_open());
        assert!(island.command_at(&Verb::IslandToggle, t0 + ms(900)));
        assert_eq!(island.page(), Some(PageId::ACTIVITY));
    }

    #[test]
    fn never_two_things_open_at_once_through_any_sequence() {
        #[derive(Clone, Copy)]
        enum Op {
            ClickVolume,
            ClickBrightness,
            ClickBell,
            ClickIsland,
            Escape,
            Open,
            Toggle,
            Close,
            ClickAway,
        }
        let script = [
            Op::ClickVolume,
            Op::ClickVolume,
            Op::ClickIsland,
            Op::ClickVolume,
            Op::Open,
            Op::ClickBell,
            Op::ClickVolume,
            Op::Escape,
            Op::ClickVolume,
            Op::Toggle,
            Op::Toggle,
            Op::ClickBrightness,
            Op::ClickIsland,
            Op::ClickAway,
            Op::ClickVolume,
            Op::Open,
            Op::Close,
            Op::ClickVolume,
            Op::ClickBell,
            Op::Toggle,
            Op::ClickVolume,
        ];
        let (mut island, mut now) = level_island();
        for gap in [0_u64, 40, 90, 160] {
            for (n, op) in script.into_iter().enumerate() {
                now += ms(gap);
                step(&mut island, now);
                let centre_of = |island: &Island, key| centre(bar_rect(island, key));
                let actions = match op {
                    Op::ClickVolume => {
                        let at = centre_of(&island, volume());
                        click_at(&mut island, at, BTN_LEFT, now)
                    }
                    Op::ClickBrightness => {
                        let at = centre_of(&island, brightness_key());
                        click_at(&mut island, at, BTN_LEFT, now)
                    }
                    Op::ClickBell => {
                        let at = centre_of(&island, Key::named("bell"));
                        click_at(&mut island, at, BTN_LEFT, now)
                    }
                    Op::ClickIsland => {
                        let at = centre(island.state.rect(now, island.size.0));
                        click_at(&mut island, at, BTN_LEFT, now)
                    }
                    Op::Escape => feed(&mut island, escape(), now),
                    Op::ClickAway => click_at(&mut island, (5.0, 500.0), BTN_LEFT, now),
                    Op::Open => {
                        island.command_at(&Verb::IslandOpen(PageId::ACTIVITY), now);
                        Vec::new()
                    }
                    Op::Toggle => {
                        island.command_at(&Verb::IslandToggle, now);
                        Vec::new()
                    }
                    Op::Close => {
                        island.command_at(&Verb::IslandClose, now);
                        Vec::new()
                    }
                };
                for action in actions {
                    if let IslandAction::OpenLevelSheet { piece, .. } = action {
                        let core = volume_core(&island.theme);
                        island.open_level_sheet_at(piece, core, now);
                    }
                }
                for at in [now, now + ms(20)] {
                    step(&mut island, at);
                    assert!(
                        !(island.state.is_open() && island.sheets.is_open()),
                        "gap {gap} op {n}: page and sheet both open"
                    );
                    let page_painted = island.state.progress(at) > 0.0
                        && island.state.rect(at, island.size.0).w > 0.0
                        && island.state.rect(at, island.size.0).h > 0.0;
                    let sheet_painted = island
                        .sheet_rect(at)
                        .is_some_and(|r| r.w > 0.0 && r.h > 0.0);
                    assert!(
                        !(page_painted && sheet_painted),
                        "gap {gap} op {n}: both painted"
                    );
                }
            }
        }
    }

    #[test]
    fn the_regions_and_keyboard_follow_the_sheet() {
        let (mut island, t0) = level_island();
        let radius = island.theme.radius_lg;
        let end = end_capsule(&island);
        assert!(!island.anything_shown(t0));
        let rest = island.shapes(t0);
        assert!(open_on(&mut island, volume(), t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        assert!(island.anything_shown(t), "keyboard OnDemand, whole input");
        let sheet = shown_rect(&island, t);
        let shapes = island.shapes(t);
        assert!(
            !shapes.contains(&(sheet, radius)),
            "the window's glass is a path"
        );
        let extent = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(3, t)));
        assert!(
            shapes.contains(&(extent, radius)),
            "the end capsule keeps its glass"
        );
        let _ = end;
        let bands = island.blur_bands(t);
        // A band covers the window and another the capsule.
        let covered = |x: f32, y: f32| bands.iter().any(|b| b.contains(x, y));
        assert!(covered(sheet.x + sheet.w / 2.0, sheet.y + sheet.h / 2.0));
        assert!(covered(
            extent.x + extent.w / 2.0,
            extent.y + extent.h / 2.0
        ));
        // The gap between them is no one's.
        let gap_y = extent.y + extent.h + 4.0;
        assert!(gap_y < sheet.y);
        assert!(
            !covered(sheet.x + sheet.w / 2.0, gap_y),
            "no band in the gap"
        );
        assert_eq!(
            silhouette::input_region(&shapes, island.anything_shown(t), island.size),
            [Rect::whole(island.size)]
        );
        // Closed again: today's regions.
        island.close_sheet(t);
        let gone = t + ms(400);
        step(&mut island, gone);
        assert!(!island.anything_shown(gone));
        // The piece gives back its pill and the capsule keeps its extent.
        let later = gone + ms(400);
        step(&mut island, later);
        assert_eq!(island.shapes(later), rest);
    }

    /// A graph with enough applications to make a body taller than a sheet.
    fn busy_graph() -> AudioGraph {
        use celestina_shell::audio::{AudioLevel, GraphDevice, GraphStream};
        AudioGraph {
            outputs: (0..6)
                .map(|id| GraphDevice {
                    id,
                    name: format!("Speakers {id}"),
                    default: id == 0,
                })
                .collect(),
            inputs: Vec::new(),
            streams: (0..12)
                .map(|id| GraphStream {
                    id: 100 + id,
                    name: format!("App {id}"),
                    level: Some(AudioLevel {
                        percent: 30,
                        muted: false,
                    }),
                    playing: true,
                })
                .collect(),
        }
    }

    #[test]
    fn a_tall_body_scrolls_and_a_wheel_over_a_slider_moves_the_slider() {
        let (mut island, t0) = level_island();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let core = sent_core_with(&island.theme, &sent, busy_graph(), true);
        assert!(island.open_level_sheet_at(volume(), core, t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let sheet = shown_rect(&island, t);
        assert!(
            island.sheets.scroll().can_scroll(),
            "a body taller than 496: {:?}",
            island.sheets.body_size()
        );
        assert_eq!(sheet.h, geometry::max_height(island.size));
        // The wheel over a slider moves the slider, not the scroll.
        let slider = match island.sheets.body_node_rect(crate::osd::popup::slider_key(
            crate::osd::popup::Control::Output,
        )) {
            Some(r) => r,
            None => panic!("no output slider"),
        };
        let at = (
            sheet.x + slider.x + slider.w / 2.0,
            sheet.y + slider.y + slider.h / 2.0,
        );
        let wheel = |at: (f32, f32), vertical| InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical,
        };
        feed(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t,
        );
        feed(&mut island, wheel(at, -15.0), t);
        assert_eq!(island.sheets.scroll().offset(), 0.0);
        island
            .sheets
            .sync(&island.theme.clone(), t + ms(200), &mut fake_measure);
        assert_eq!(
            *sent.borrow(),
            [LevelRequest::Set(crate::osd::popup::Control::Output, 41)]
        );
        // Over the rows' empty space the wheel scrolls, one px per px.
        let empty = (sheet.x + 4.0, sheet.y + 2.0);
        feed(
            &mut island,
            InputEvent::PointerMotion {
                x: empty.0,
                y: empty.1,
            },
            t,
        );
        feed(&mut island, wheel(empty, 30.0), t);
        assert_eq!(island.sheets.scroll().offset(), 30.0);
        feed(&mut island, wheel(empty, 10_000.0), t);
        let max = island.sheets.scroll().offset();
        assert!(max > 30.0);
        feed(&mut island, wheel(empty, -10_000.0), t);
        assert_eq!(island.sheets.scroll().offset(), 0.0);
        assert!(island.sheets.is_open(), "scrolling never closes");
    }

    #[test]
    fn an_open_sheet_at_rest_asks_for_no_frame() {
        // A sheet whose window cuts its row (the costliest case), and one that
        // does not.
        let (mut cut, t0) = wide_start_island();
        assert!(open_on(&mut cut, Key::named("bell"), t0));
        let (plain, t1) = settled();
        for (island, t) in [(&mut cut, t0), (&mut { plain }, t1)] {
            let later = t + Duration::from_secs(1);
            run_frames(island, t, later);
            assert!(island.sheets.is_open());
            assert!(!island.sheets.wants_frame(later), "no frame at rest");
            assert_eq!(island.sheets.wait(later), None, "no timer at rest");
        }
    }

    #[test]
    fn a_sheet_whose_piece_leaves_the_bar_closes() {
        let (mut island, t0) = settled();
        let t = t0 + ms(10);
        // The reading is lost: the volume piece has nothing to show.
        island.levels.audio.output = None;
        island.invalidate_bar();
        step(&mut island, t);
        assert!(island
            .bar
            .as_ref()
            .is_some_and(|b| b.piece_rect(volume()).is_none()));
        assert!(!island.sheets.is_open(), "closing on the exit motion");
        assert!(island.sheets.is_shown(t + ms(10)));
        let end = island.sheets.last_anchor().map(|a| a.capsule);
        assert!(end.is_some());
        // It flies to the last capsule rect it had and is gone.
        let r = shown_rect(&island, t + ms(75));
        assert!(r.w > 0.0);
        step(&mut island, t + ms(400));
        assert!(island.sheet_rect(t + ms(400)).is_none());
        assert!(!island.sheets.is_shown(t + ms(400)));
    }

    #[test]
    fn a_capsule_that_changes_width_while_open_carries_the_sheet() {
        let (mut island, t0) = settled();
        let before = shown_rect(&island, t0);
        let capsule = end_capsule(&island);
        // "40 %" widens to "100 %": the capsule is wider by a few chars.
        island.levels.audio.output = Some(Level {
            percent: 100,
            muted: false,
        });
        island.invalidate_bar();
        let t = t0 + ms(20);
        step(&mut island, t);
        let wider = end_capsule(&island);
        assert!(wider.w >= capsule.w);
        let settle = t + ms(500);
        step(&mut island, settle);
        let body = volume_core(&island.theme).size();
        let want = geometry::window(wider, geometry::Edge::End, body, island.size);
        assert!(near(shown_rect(&island, settle), want));
        assert!(island.sheets.is_open());
        let _ = before;
    }

    #[test]
    fn a_fold_inside_island_input_cuts_with_the_measure_in_hand() {
        let (mut island, t0) = level_island();
        let core = sent_core(
            &island.theme,
            &Rc::new(RefCell::new(Vec::new())),
            busy_graph(),
        );
        assert!(island.open_level_sheet_at(volume(), core, t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let sheet = shown_rect(&island, t);
        let header = match island
            .sheets
            .body_node_rect(crate::osd::audio_rows::section_key(
                crate::osd::audio_rows::Section::Output,
            )) {
            Some(r) => r,
            None => panic!("no output header"),
        };
        let at = (
            sheet.x + header.x + header.w / 2.0,
            sheet.y + header.y + header.h / 2.0,
        );
        let calls = Cell::new(0_u32);
        let mut measure = |s: &str, st: &TextStyle, w: Option<f32>| {
            calls.set(calls.get() + 1);
            fake_measure(s, st, w)
        };
        let before = island.sheets.body_size();
        feed_with(
            &mut island,
            InputEvent::PointerMotion { x: at.0, y: at.1 },
            t,
            &mut measure,
        );
        feed_with(&mut island, press(at.0, at.1, BTN_LEFT), t, &mut measure);
        let seen = calls.get();
        feed_with(&mut island, release(at.0, at.1, BTN_LEFT), t, &mut measure);
        assert!(
            calls.get() > seen,
            "the fold cut names with the measure in hand"
        );
        assert_ne!(
            island.sheets.body_size(),
            before,
            "the fold changed the body"
        );
        // The sheet notices at its next look and morphs.
        let later = t + ms(10);
        step(&mut island, later);
        assert!(island.sheets.is_open());
    }

    #[test]
    fn closing_a_level_sheet_sends_what_the_throttle_holds() {
        let (mut island, t0) = level_island();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let core = sent_core(&island.theme, &sent, AudioGraph::default());
        assert!(island.open_level_sheet_at(volume(), core, t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let sheet = shown_rect(&island, t);
        let slider = match island.sheets.body_node_rect(crate::osd::popup::slider_key(
            crate::osd::popup::Control::Output,
        )) {
            Some(r) => r,
            None => panic!("no output slider"),
        };
        let y = sheet.y + slider.y + slider.h / 2.0;
        let x0 = sheet.x + slider.x + 20.0;
        // A drag: the first level goes out, the next two wait on the throttle.
        feed(&mut island, InputEvent::PointerMotion { x: x0, y }, t);
        feed(&mut island, press(x0, y, BTN_LEFT), t);
        feed(
            &mut island,
            InputEvent::PointerMotion { x: x0 + 30.0, y },
            t,
        );
        feed(
            &mut island,
            InputEvent::PointerMotion { x: x0 + 60.0, y },
            t,
        );
        let sent_so_far = sent.borrow().len();
        assert_eq!(
            sent_so_far,
            1,
            "the throttle holds the rest: {:?}",
            sent.borrow()
        );
        assert!(island.sheets.wait(t).is_some());
        // Escape: closing flushes what waits.
        feed(&mut island, escape(), t);
        assert!(!island.sheets.is_open());
        assert!(
            sent.borrow().len() > sent_so_far,
            "what the throttle held went out"
        );
    }

    #[test]
    fn a_held_arrow_keeps_stepping_a_level_sheets_focused_slider() {
        use crate::osd::popup::Control;
        const TAB: u32 = 0xff09;
        const RIGHT: u32 = 0xff53;
        let (mut island, t0) = level_island();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let core = sent_core(&island.theme, &sent, AudioGraph::default());
        assert!(island.open_level_sheet_at(volume(), core, t0));
        let mut t = t0 + ms(500);
        step(&mut island, t);
        let output_level = |sent: &Rc<RefCell<Vec<LevelRequest>>>| {
            sent.borrow().iter().rev().find_map(|r| match r {
                LevelRequest::Set(Control::Output, v) => Some(*v),
                _ => None,
            })
        };
        // Tab until a pressed Right steps the output slider.
        for _ in 0..20 {
            feed(&mut island, key_event(TAB), t);
            feed(&mut island, key_event(RIGHT), t);
            t += ms(200);
            island
                .sheets
                .sync(&island.theme.clone(), t, &mut fake_measure);
            if output_level(&sent).is_some() {
                break;
            }
        }
        let Some(pressed) = output_level(&sent) else {
            panic!("no Tab reached the output slider: {:?}", sent.borrow());
        };
        // The held key's repeats keep stepping it.
        let repeat = InputEvent::KeyRepeat {
            keysym: RIGHT,
            utf8: None,
            modifiers: celestina_wayland::Modifiers::default(),
            serial: 0,
        };
        feed(&mut island, repeat.clone(), t);
        feed(&mut island, repeat, t);
        t += ms(200);
        island
            .sheets
            .sync(&island.theme.clone(), t, &mut fake_measure);
        assert!(
            output_level(&sent).is_some_and(|held| held > pressed),
            "the repeats stepped nothing: {:?}",
            sent.borrow()
        );
    }

    #[test]
    fn a_level_sheet_closed_without_a_surface_sends_what_the_throttle_holds() {
        let (mut island, t0) = level_island();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let core = sent_core(&island.theme, &sent, AudioGraph::default());
        assert!(island.open_level_sheet_at(volume(), core, t0));
        let t = t0 + ms(500);
        step(&mut island, t);
        let sheet = shown_rect(&island, t);
        let slider = match island.sheets.body_node_rect(crate::osd::popup::slider_key(
            crate::osd::popup::Control::Output,
        )) {
            Some(r) => r,
            None => panic!("no output slider"),
        };
        let y = sheet.y + slider.y + slider.h / 2.0;
        let x0 = sheet.x + slider.x + 20.0;
        // A drag: the first level goes out, the next two wait on the throttle.
        feed(&mut island, InputEvent::PointerMotion { x: x0, y }, t);
        feed(&mut island, press(x0, y, BTN_LEFT), t);
        feed(
            &mut island,
            InputEvent::PointerMotion { x: x0 + 30.0, y },
            t,
        );
        feed(
            &mut island,
            InputEvent::PointerMotion { x: x0 + 60.0, y },
            t,
        );
        let sent_so_far = sent.borrow().len();
        assert_eq!(
            sent_so_far,
            1,
            "the throttle holds the rest: {:?}",
            sent.borrow()
        );
        assert!(island.sheets.wait(t).is_some());
        // The output is gone: the app closes the sheet before the Island is
        // dropped (`Sheets` has no Drop flush).
        island.close_level_sheet_now();
        assert!(!island.sheets.is_open());
        assert!(!island.level_sheet_open());
        assert!(
            sent.borrow().len() > sent_so_far,
            "what the throttle held went out"
        );
    }

    #[test]
    fn under_reduced_motion_what_waited_opens_on_the_same_input() {
        let (mut island, t0) = level_island();
        island.set_reduced_motion(true);
        assert!(open_on(&mut island, volume(), t0));
        let at = centre(island.state.rect(t0, island.size.0));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        // The window fades out (nothing flies), and what waited opens once
        // it has.
        assert!(island.sheets.is_shown(t0 + ms(1)), "it still fades");
        let done = t0 + ms(400);
        step(&mut island, done);
        assert!(!island.sheets.is_shown(done));
        assert!(island.state.is_open(), "the page opens when it has faded");
    }

    #[test]
    fn escape_and_focus_loss_drop_what_waited_behind_a_closing_sheet() {
        for ev in [escape(), InputEvent::FocusLost] {
            let (mut island, t0) = settled();
            island.command_at(&Verb::IslandOpen(PageId::ACTIVITY), t0);
            assert!(island.sheets.has_queued());
            feed(&mut island, ev, t0 + ms(20));
            assert!(!island.sheets.has_queued());
            step(&mut island, t0 + ms(400));
            assert!(!island.state.is_open(), "the page never opened");
            assert!(!island.sheets.is_shown(t0 + ms(400)));
        }
    }

    #[test]
    fn the_keyboard_dropping_does_not_drop_the_sheet_that_waits_for_the_page() {
        let (mut island, t0) = level_island();
        island.open(PageId::ACTIVITY, t0);
        let t = t0 + ms(500);
        step(&mut island, t);
        let at = centre(bar_rect(&island, volume()));
        assert_eq!(click_at(&mut island, at, BTN_LEFT, t).len(), 1);
        assert!(open_on(&mut island, volume(), t));
        assert!(island.sheets.has_queued());
        // The Island itself drops the keyboard when nothing is shown: the
        // compositor's leave must find the keyboard still held.
        // The compositor sends the leave only when the keyboard was dropped.
        if !island.anything_shown(t) {
            feed(&mut island, InputEvent::FocusLost, t);
        }
        assert!(
            island.anything_shown(t),
            "the keyboard stays while it waits"
        );
        assert!(island.sheets.has_queued(), "the request survived");
        step(&mut island, t + ms(160));
        assert!(
            island.sheets.is_open(),
            "the sheet opens once the page is gone"
        );
    }

    #[test]
    fn a_sheet_opened_in_the_gap_after_a_close_does_not_swallow_the_waiting_page() {
        let (mut island, t0) = settled();
        let at = centre(island.state.rect(t0, island.size.0));
        assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
        assert!(island.sheets.has_queued());
        // The owner itself keeps a waiting page when a sheet opens directly.
        let t = t0 + ms(160);
        let zone = 3;
        let anchor = match island.live_anchor(zone, volume(), t) {
            Some(a) => a,
            None => panic!("no anchor"),
        };
        let request = crate::sheet::Request {
            zone,
            piece: volume(),
            kind: crate::pieces::SheetKind::Volume,
            body: crate::sheet::SheetBody::Level(Box::new(volume_core(&island.theme))),
        };
        island
            .sheets
            .open(request, &anchor, &island.theme.clone(), t, false);
        assert!(island.sheets.has_queued(), "the page still waits");
        // And through the Island, the page opens first and the sheet follows.
        let (mut island, t0) = settled();
        let at = centre(island.state.rect(t0, island.size.0));
        click_at(&mut island, at, BTN_LEFT, t0);
        let t = t0 + ms(160);
        assert!(open_on(&mut island, volume(), t));
        assert!(!(island.state.is_open() && island.sheets.is_open()));
        step(&mut island, t + ms(400));
        assert!(island.sheets.is_open() && !island.state.is_open());
        // The newer request wins: the page that waited is given up, so it
        // does not open on its own when that sheet closes.
        assert!(!island.sheets.has_queued(), "the page no longer waits");
        let later = t + ms(500);
        island.close_sheet(later);
        step(&mut island, later + ms(1));
        step(&mut island, later + ms(400));
        assert!(!island.sheets.is_shown(later + ms(400)));
        assert!(!island.state.is_open(), "the page stays closed");
        assert!(!island.anything_open());
    }

    mod drop;
    mod lent;
    mod notice_card;
    mod open_piece;
    mod page_sheets;
    mod tray_overflow;
    mod window;
}
