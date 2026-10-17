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
//! While the `bar` duty is on (parity spec §6) the surface spans the
//! output, and a third tree, the bar's zone capsules ([`bar_tree`]), is
//! painted before the Island; the regions are the union of every painted
//! silhouette. With the bar off there is no third tree and the surface keeps
//! its 1.0.0 shape.

pub mod bar_tree;
pub mod cadence;
pub mod clock;
pub mod menu;
pub mod pages;
pub mod state;
pub mod view;
pub mod workspaces;

use std::time::{Duration, Instant};

use celestina_paint::{Painter, PixmapId, Rect, Theme};
use celestina_shell::glance::Today;
use celestina_shell::settings::BarLayout;
use celestina_ui::{Interaction, Key, Tree, UiEvent, BTN_LEFT};
use celestina_wayland::{
    CloseReason, FrameCx, InputEvent, KeyboardInteractivity, PopupId, PopupShell, SurfaceCtl,
};

use crate::{
    bar::silhouette::{self, Shape},
    clipboard::{verbs::ClipboardVerb, ClipboardState},
    command::Verb,
    pieces::{BarCx, Gesture, PageId, PieceAction},
    providers::{
        niri::NiriModel, notifications::NotificationsCommand, sysmon::SysReading,
        weather::WeatherReading,
    },
    tray::{scroll_steps, ItemId, Orientation, TrayCommand},
};
use bar_tree::BarTree;
use cadence::{Cadence, Offer, Wake, SYSMON_BAR_PERIOD};
use clock::{ClockTick, SecondTick};
use pages::{
    activity, clipboard as clipboard_page, clock as clock_page, notifications, page_view,
    sysmon as sysmon_page, tabs,
    tray::{self as tray_page, IconCache, Pixmaps, TrayList, TrayRow},
    HistoryRow, NameOwner, PageCx,
};
use state::{IslandState, STRIP_W};
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
    },
    /// The overflow button at `more` (a rect of the surface) was chosen,
    /// from the `keyboard` or the pointer: the app builds the menu and
    /// hands it to [`Island::open_menu`].
    OpenMenu {
        more: Rect,
        keyboard: bool,
    },
    /// A row's dismiss or action button, or dismiss all, on the
    /// Notifications page: the notification server runs the command.
    Notification(NotificationsCommand),
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
}

/// Linux `BTN_RIGHT` and `BTN_MIDDLE`.
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
/// The Menu key and F10 (with Shift): the keyboard's context menu.
const XKB_KEY_MENU: u32 = 0xff67;
const XKB_KEY_F10: u32 = 0xffc9;
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
    /// The page slide the page view was built with.
    applied_slide: f32,
    /// The theme the views were built with.
    theme: Theme,
    reduced: bool,
    /// The views must be built again before the next layout.
    stale: bool,
    /// The growth was in flight at the last frame.
    moving: bool,
    /// The overflow menu, while it is open.
    menu: Option<PopupId>,
    /// The first level of a tray item's menu, while it is open.
    tray_menu: Option<PopupId>,
    /// The tray's items as the page shows them.
    tray_rows: Vec<TrayRow>,
    /// The generation of the tray list `tray_rows` was made from.
    tray_generation: u64,
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
    /// The pixmaps uploaded for the items' icons.
    icons: IconCache,
    scroll: ScrollRemainder,
    /// The bar's zone capsules, while the `bar` duty is on.
    bar: Option<BarTree>,
    /// The open tray menu hangs from the bar, not from the Tray page, so it
    /// outlives the page.
    tray_menu_on_bar: bool,
    /// The last tray menu asked for was a bar icon's.
    bar_menu_asked: bool,
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
            applied_slide: 0.0,
            theme,
            reduced: false,
            stale: true,
            moving: false,
            menu: None,
            tray_menu: None,
            tray_rows: Vec::new(),
            tray_generation: 0,
            clipboard: ClipboardState::default(),
            clipboard_on: false,
            clip_panel: clipboard_page::ClipboardPanel::default(),
            clip_textures: clipboard_page::Textures::default(),
            confirming_clear: false,
            pending_focus: None,
            delete_held: false,
            icons: IconCache::default(),
            scroll: ScrollRemainder::default(),
            bar: None,
            tray_menu_on_bar: false,
            bar_menu_asked: false,
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

    /// Whether the System page is the one the open Island shows: the only
    /// page that reads the newest reading.
    fn sysmon_page_open(&self) -> bool {
        self.state.is_open() && self.state.shown_page() == PageId::SYSMON
    }

    /// Turns the bar on with `layout`, or off with `None`. Off, nothing of
    /// the bar exists and the Island's regions and input are 1.0.0's (its
    /// open page now also shows the seconds clock); the surface's shape is
    /// the app's (its width comes with every frame). Takes effect on the
    /// next frame, which the caller asks for.
    pub fn set_bar(&mut self, layout: Option<&BarLayout>) {
        match (layout, self.bar.as_mut()) {
            (None, None) => return,
            (None, Some(_)) => self.bar = None,
            (Some(layout), Some(bar)) => {
                if !bar.set_layout(layout) {
                    return;
                }
            }
            (Some(layout), None) => {
                self.bar = Some(BarTree::new(layout, &self.theme, self.reduced));
            }
        }
        // The capsule's dots follow the bar.
        self.stale = true;
    }

    /// Whether the bar is on.
    #[must_use]
    pub fn bar_on(&self) -> bool {
        self.bar.is_some()
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
            sysmon: self.bar_sysmon.shown(),
            weather: self.weather.as_ref(),
            today: self.today,
        }
    }

    /// Runs `f` on the bar with the context its pieces read; `None` while
    /// the bar is off.
    fn with_bar<R>(&mut self, f: impl FnOnce(&mut BarTree, &BarCx<'_>) -> R) -> Option<R> {
        let mut bar = self.bar.take()?;
        let out = f(&mut bar, &self.bar_cx());
        self.bar = Some(bar);
        Some(out)
    }

    /// Something the bar's pieces read changed.
    fn invalidate_bar(&mut self) {
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

    /// Whether the Clipboard page is the one shown right now.
    #[must_use]
    pub fn shows_clipboard_page(&self) -> bool {
        self.state.is_open() && self.state.shown_page() == PageId::CLIPBOARD
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
        let id = self.page.highlight().focused?;
        let action = clipboard_page::remove_index(
            &self.clip_panel,
            &self.theme,
            self.state.max_page_size(),
            self.page.key_of(id),
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

    /// Whether the Notifications page is on screen (or growing): what the
    /// person sees there counts as seen.
    #[must_use]
    pub fn shows_notifications(&self) -> bool {
        self.page() == Some(PageId::NOTIFICATIONS)
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
        self.rows_from(model) != self.rows || (self.state.is_open() && model != self.model.as_ref())
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
        if tray.generation != self.tray_generation {
            self.tray_generation = tray.generation;
            self.tray_rows = self.icons.rows(&tray.items, painter);
            seen = true;
        }
        if weather != self.weather.as_ref() {
            self.weather = weather.cloned();
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
            self.stale |= self.state.is_open();
            self.invalidate_bar();
        }
        if let Some(wait) = self.mirror_sysmon(sysmon, now) {
            cx.ctl.redraw_after(wait);
        }
        self.tick_clock(now, &mut cx.ctl);
        if self.stale || self.state.slide(now) != self.applied_slide {
            self.apply_views(now);
        }
        self.layout(painter, now);
        self.paint(painter, now, cx.blur_available);
        self.sync_ctl(&mut cx.ctl, now);

        let moving = self.state.wants_frame(now);
        let bar_moving = self.bar.as_ref().is_some_and(|b| b.is_animating(now));
        if moving || self.capsule.is_animating(now) || self.page.is_animating(now) || bar_moving {
            cx.ctl.request_redraw();
        }
        if self.moving && !moving {
            tracing::debug!(output = %self.output, open = self.state.is_open(), "island: growth settled");
        }
        self.moving = moving;
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

    /// Today's date at wall-clock `secs` and UTC `offset`: a new day redraws
    /// what reads it.
    fn set_today(&mut self, secs: u64, offset: i64) {
        let today = i64::try_from(secs)
            .ok()
            .zip(i32::try_from(offset).ok())
            .and_then(|(secs, offset)| Today::from_unix(secs, offset));
        if today != self.today {
            self.today = today;
            self.stale |= self.state.is_open();
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

    /// Builds both views. The page asks for its size first, so it is laid
    /// out at the size the Island is heading for. While the page slides, the
    /// view is applied every frame with its new offset and the tree's own
    /// slide of moved nodes is off, so the offset is the only motion.
    fn apply_views(&mut self, now: Instant) {
        let w = self.size.0;
        let rows = view::capsule_shows_workspaces(self.bar.as_ref().map(BarTree::layout))
            .then_some(&self.rows[..]);
        self.capsule
            .apply(view::capsule_with(&self.theme, &self.clock, rows, w), now);
        let shown = self.state.shown_page();
        let page = page_view(shown);
        let cx = PageCx {
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
        };
        self.state
            .request_page_size(page.wants(&cx), now, self.reduced);
        let slide = self.state.slide(now);
        let moving = slide != 0.0 || self.applied_slide != 0.0;
        if moving {
            self.page.set_reduced_motion(true);
        }
        self.page.apply(
            view::page(
                &self.theme,
                shown,
                page.title(),
                &self.clock_seconds,
                page.view(&cx),
                w,
                self.state.page_size(),
                slide,
            ),
            now,
        );
        if moving {
            self.page.set_reduced_motion(self.reduced);
        }
        if let Some(key) = self.pending_focus.take() {
            if self.shows_clipboard_page() {
                self.page_input.focus_key(&mut self.page, key);
            }
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
        let fault = self.with_bar(|bar, cx| {
            bar.update(cx, surface_w, island_w, now, lay);
            bar.take_fault()
        });
        if let Some(fault) = fault.flatten() {
            tracing::warn!(output = %self.output, %fault, "island: bar layout fault");
        }
    }

    /// The painted silhouettes at `now`: each of the bar's capsules, then
    /// the Island's rect.
    fn shapes(&self, now: Instant) -> Vec<Shape> {
        let radius = self.theme.radius_lg;
        let mut shapes = self
            .bar
            .as_ref()
            .map_or_else(Vec::new, |bar| bar.shapes(now, radius));
        shapes.push((self.state.rect(now, self.size.0), radius));
        shapes
    }

    /// The bar's capsules, then the Haze at the Island's rect, then the
    /// capsule and the page, each under its opacity and clipped to that
    /// rect.
    fn paint(&mut self, painter: &mut Painter, now: Instant, blur_available: bool) {
        let rect = self.state.rect(now, self.size.0);
        let radius = self.theme.radius_lg;
        painter.begin(self.size, self.dpr, blur_available);
        if let Some(bar) = self.bar.as_mut() {
            bar.paint(painter, now);
        }
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
        painter.end();
    }

    /// Blur and input regions plus keyboard interactivity for `now`: the
    /// blur follows the painted silhouettes every frame (the Island's, and
    /// the bar's capsules while the bar is on); the input region is those
    /// silhouettes at rest and the whole surface while open (so a click
    /// outside the page reaches the Island and closes it); the keyboard is
    /// `None` at rest and `OnDemand` while open (ADR 0007).
    fn sync_ctl(&self, ctl: &mut SurfaceCtl<'_>, now: Instant) {
        let shapes = self.shapes(now);
        let open = self.state.is_open();
        ctl.set_blur_region(&silhouette::union(&shapes));
        ctl.set_input_region(&silhouette::input_region(&shapes, open, self.size));
        ctl.set_keyboard_interactivity(if open {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::None
        });
    }

    /// Grows into `page`, or slides to it while already open.
    fn open(&mut self, page: PageId, now: Instant) {
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
    }

    fn close(&mut self, now: Instant) {
        if self.month_offset != 0 {
            self.month_offset = 0;
            self.stale = true;
        }
        self.forget_confirmation();
        self.delete_held = false;
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
        self.layout(painter, now);
        self.route(ev, ctl, now)
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
        self.bar.as_ref().is_some_and(|bar| bar.contains(x, y))
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
    ) -> Vec<IslandAction> {
        let before = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.bar.as_ref().map(BarTree::highlight),
            self.state.is_open(),
        );
        let actions = self.dispatch(ev, now);
        // The page is closing: its menus go with it.
        if !self.state.is_open() {
            self.close_page_menus(ctl);
        }
        let after = (
            self.capsule.highlight(),
            self.page.highlight(),
            self.bar.as_ref().map(BarTree::highlight),
            self.state.is_open(),
        );
        if before != after || !actions.is_empty() || self.stale {
            self.sync_ctl(ctl, now);
            ctl.request_redraw();
        }
        actions
    }

    /// Sends a laid-out event where it belongs: the page, the capsule or the
    /// bar. A pointer that moves from one to another leaves the first.
    fn dispatch(&mut self, ev: InputEvent, now: Instant) -> Vec<IslandAction> {
        let open = self.state.is_open();
        let point = match ev {
            InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. }
            | InputEvent::Axis { x, y, .. } => Some((x, y)),
            _ => None,
        };
        let on_bar = point.is_some_and(|(x, y)| self.on_bar(x, y, now));
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
                actions.extend(self.bar_input(&ev, open, now));
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
                // The keyboard repeats a held key as more presses and the
                // toolkit does not say which is which: one press removes
                // one entry, the next needs the key released in between.
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
            InputEvent::Key { .. } if open => self.page_input.key(&mut self.page, &ev),
            InputEvent::PointerPress { x, y, .. }
                if open && !self.state.rect(now, self.size.0).contains(*x, *y) =>
            {
                self.close(now);
                Vec::new()
            }
            // The tray rows' other buttons: the page's pointer handling
            // only knows the left one.
            InputEvent::PointerRelease { x, y, button, .. } if open && *button != BTN_LEFT => {
                actions.extend(self.other_button(*x, *y, *button));
                Vec::new()
            }
            _ if open => self.page_input.pointer(&mut self.page, &ev, now),
            _ => self.capsule_input.pointer(&mut self.capsule, &ev, now),
        };
        for event in events {
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
    /// never does.
    fn bar_input(&mut self, ev: &InputEvent, open: bool, now: Instant) -> Vec<IslandAction> {
        let Some(input) = self.with_bar(|bar, cx| bar.pointer(ev, cx, now)) else {
            return Vec::new();
        };
        if open && !input.on_piece && matches!(ev, InputEvent::PointerPress { .. }) {
            self.close(now);
        }
        let mut out = Vec::new();
        for (key, gesture, action) in input.asked {
            let scroll = matches!(gesture, Gesture::Scroll(..));
            let page = matches!(action, PieceAction::OpenPage(_));
            match action {
                PieceAction::None => {}
                PieceAction::OpenPage(page) => self.open(page, now),
                PieceAction::FocusWorkspace(id) => out.push(IslandAction::FocusWorkspace(id)),
                PieceAction::Tray(command) => out.push(IslandAction::Tray(command)),
                PieceAction::TrayMenu(item) => {
                    if let Some(row) = self.bar.as_ref().and_then(|bar| bar.node_rect(key)) {
                        self.bar_menu_asked = true;
                        out.push(IslandAction::OpenTrayMenu {
                            item,
                            row,
                            keyboard: false,
                        });
                    }
                }
                PieceAction::Notification(command) => out.push(IslandAction::Notification(command)),
            }
            if open && !scroll && !page && self.state.is_open() {
                self.close(now);
            }
        }
        out
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
        if self.menu.is_some() || !self.state.is_open() {
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
        self.bar_menu_asked = false;
        Some(id)
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
            UiEvent::Click(key) | UiEvent::Activate(key) => self
                .tray_row(*key)
                .map(|(_, row)| IslandAction::Tray(TrayCommand::Activate(row.id.clone())))
                .into_iter()
                .collect(),
            UiEvent::Scroll(key, horizontal, vertical) => {
                self.scroll_actions(*key, *horizontal, *vertical)
            }
            UiEvent::HoverChanged(..) | UiEvent::Escape | UiEvent::FieldChanged(_) => Vec::new(),
        }
    }

    fn open_menu_action(&self, item: ItemId, index: usize, keyboard: bool) -> Option<IslandAction> {
        Some(IslandAction::OpenTrayMenu {
            item,
            row: self.tray_row_rect(index)?,
            keyboard,
        })
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
            BTN_RIGHT if row.has_menu => self
                .open_menu_action(row.id.clone(), index, false)
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
            Some((index, row)) if row.has_menu => self
                .open_menu_action(row.id.clone(), index, true)
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
        self.tray_rows.clear();
        self.tray_generation = 0;
        self.invalidate_bar();
    }

    /// Closes the menus from the Island's side; the runtime still reports
    /// `PopupClosed`, which then finds nothing to do.
    fn close_menu(&mut self, ctl: &mut SurfaceCtl<'_>) {
        self.tray_menu_on_bar = false;
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

    /// Its surface is about to be destroyed while the Island lives on (the
    /// `bar` duty flipped and the app gives it a surface of the other
    /// shape): its menus close through `ctl`, the open page closes at once,
    /// and every hover, press and focus ends, so the Island starts at rest
    /// on its new surface. The page it last showed, its motion setting and
    /// what its pages read are kept. Returns whether a menu was open, whose
    /// choice can no longer arrive.
    pub fn detach(&mut self, ctl: &mut SurfaceCtl<'_>) -> bool {
        let menus = self.detach_at(Instant::now());
        for &id in &menus {
            ctl.close_popup(id);
        }
        !menus.is_empty()
    }

    /// [`Island::detach`] without the surface: the menus it must close.
    fn detach_at(&mut self, now: Instant) -> Vec<PopupId> {
        self.tray_menu_on_bar = false;
        self.bar_menu_asked = false;
        let menus = [self.menu.take(), self.tray_menu.take()]
            .into_iter()
            .flatten()
            .collect();
        self.close(now);
        // A single step back to the capsule, whatever the motion setting.
        self.state.close(now, true);
        self.capsule_input
            .pointer(&mut self.capsule, &InputEvent::PointerLeave, now);
        if let Some(bar) = self.bar.as_mut() {
            bar.leave(now);
        }
        // The timers were the old surface's: the first frame on the new one
        // arms them afresh.
        self.tick = ClockTick::default();
        self.second_tick = SecondTick::default();
        self.stale = true;
        menus
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
        } else {
            return false;
        }
        if reason == CloseReason::Dismissed && self.state.is_open() {
            self.close(now);
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
            UiEvent::HoverChanged(..) | UiEvent::Scroll(..) | UiEvent::FieldChanged(_) => {
                return None
            }
        };
        match self.activate(key, now)? {
            IslandAction::OpenMenu { more, .. } => Some(IslandAction::OpenMenu { more, keyboard }),
            action => Some(action),
        }
    }

    /// What a click or Enter on the node `key` does.
    fn activate(&mut self, key: Key, now: Instant) -> Option<IslandAction> {
        if let Some(handled) = self.clipboard_activate(key) {
            handled
        } else if key == view::capsule_key() {
            self.open(self.state.shown_page(), now);
            None
        } else if key == view::close_key() {
            self.close(now);
            None
        } else if key == view::more_key() {
            let id = self.page.node_by_key(key)?;
            Some(IslandAction::OpenMenu {
                more: self.page.rect(id),
                keyboard: false,
            })
        } else if let Some(page) = tabs::page_of(key) {
            self.open(page, now);
            None
        } else if let Some(step) = clock_page::month_step(key) {
            let offset = step.apply(self.month_offset);
            if offset != self.month_offset {
                self.month_offset = offset;
                self.stale = true;
            }
            None
        } else if key == sysmon_page::open_key() {
            Some(IslandAction::OpenApp(sysmon_page::MONITOR_DESKTOP_ID))
        } else if let Some(action) = notifications::action_for(
            &self.history,
            &self.owners,
            &self.theme,
            self.state.max_page_size(),
            key,
        ) {
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

    /// Runs an Island verb of `org.celestina.Shell1`; other verbs are not
    /// the Island's and change nothing.
    pub fn command(&mut self, verb: &Verb, ctl: &mut SurfaceCtl<'_>) {
        let now = Instant::now();
        match *verb {
            Verb::IslandToggle => {
                if self.state.is_open() {
                    self.close(now);
                } else {
                    self.open(self.state.shown_page(), now);
                }
            }
            Verb::IslandOpen(page) => self.open(page, now),
            Verb::IslandClose => self.close(now),
            Verb::LauncherToggle
            | Verb::NotificationsToggle
            | Verb::NotificationsDismissAll
            | Verb::WallpaperSet { .. }
            | Verb::WallpaperNext(_)
            | Verb::WallpaperPrev(_)
            | Verb::WallpaperMode(_)
            | Verb::WallpaperRetry
            | Verb::Caffeine(_)
            | Verb::NightLight(_)
            | Verb::Session(_)
            | Verb::Clipboard(_)
            | Verb::Quit => return,
        }
        self.sync_ctl(ctl, now);
        ctl.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::niri::{Window, Workspace};
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
        island.layout_with(now, &mut |tree: &mut Tree, room: (f32, f32)| {
            tree.layout_measured(room, |s, _, _| (7.0 * s.chars().count() as f32, 16.0));
        });
    }

    /// What `input` does after the popups: the views, the layout, then the
    /// event.
    fn feed(island: &mut Island, ev: InputEvent, now: Instant) -> Vec<IslandAction> {
        if island.stale {
            island.apply_views(now);
        }
        lay_fake(island, now);
        island.dispatch(ev, now)
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
        island.set_bar(Some(&BarLayout::default()));
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
    fn with_the_bar_off_the_island_is_the_strip_it_was() {
        let t0 = Instant::now();
        let mut island = Island::new("DP-1", (760.0, 560.0));
        island.rows = rows_for(&model("DP-1", 1), "DP-1");
        island.set_bar(None);
        assert!(!island.bar_on());
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(island.capsule.node_by_key(Key::named("dots")).is_some());
        let radius = island.theme.radius_lg;
        assert_eq!(
            island.shapes(t0),
            vec![(island.state.rect(t0, 760.0), radius)]
        );
        // The regions are the Island's silhouette alone, as in 1.0.0.
        assert_eq!(
            silhouette::union(&island.shapes(t0)),
            island.state.silhouette(t0, 760.0, radius)
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
    fn a_new_surface_gets_the_minute_timer_again() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut island = Island::new("DP-1", (760.0, 560.0));
        let mut old = EarliestTimer::default();
        clock_frame(&mut island, &mut old, t0, Duration::ZERO);
        island.open(PageId::ACTIVITY, t0 + secs(5));
        clock_frame(&mut island, &mut old, t0, secs(5));
        // The duty flips: the old surface and its timer go, the Island
        // draws its first frame on a surface with nothing armed.
        let _menus = island.detach_at(t0 + secs(5));
        let mut new = EarliestTimer::default();
        clock_frame(&mut island, &mut new, t0, secs(5));
        run_timer(&mut island, &mut new, t0, secs(130));
        assert_eq!(island.clock_minute, Some(minute_at(secs(130))));
        assert!(
            new.0.is_some(),
            "the minute timer is armed on the new surface"
        );
    }

    #[test]
    fn the_bar_draws_its_zones_and_the_capsule_drops_its_dots() {
        let (mut island, t0) = bar_island(1920.0);
        assert!(island.bar_on());
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
        // The bar off again: the dots come back.
        island.set_bar(None);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(island.capsule.node_by_key(Key::named("dots")).is_some());
        assert!(island.shapes(t0).len() == 1);
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
        // A dot focuses its workspace.
        let dot = centre(bar_rect(&island, Key::indexed("dot", 3)));
        assert_eq!(
            click_at(&mut island, dot, BTN_LEFT, t0),
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
                keyboard: false
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
        // The bell opens the Notifications page.
        let bell = centre(bar_rect(&island, Key::named("bell")));
        assert!(click_at(&mut island, bell, BTN_LEFT, t0).is_empty());
        assert_eq!(island.page(), Some(PageId::NOTIFICATIONS));
    }

    #[test]
    fn a_bar_piece_while_open_switches_the_page_or_acts_and_closes_it() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_reduced_motion(true);
        island.open(PageId::ACTIVITY, t0);
        // The bell slides the open page to Notifications; it stays open.
        let bell = centre(bar_rect(&island, Key::named("bell")));
        assert!(click_at(&mut island, bell, BTN_LEFT, t0).is_empty());
        assert_eq!(island.page(), Some(PageId::NOTIFICATIONS));
        // The wheel on the tray icon scrolls and keeps the page.
        let icon = centre(bar_rect(&island, tray_page::row_key(0)));
        let wheel = InputEvent::Axis {
            x: icon.0,
            y: icon.1,
            horizontal: 0.0,
            vertical: -15.0,
        };
        assert_eq!(feed(&mut island, wheel, t0).len(), 1);
        assert!(island.state.is_open());
        // A dot focuses its workspace and closes the page, as a click
        // outside it does.
        let dot = centre(bar_rect(&island, Key::indexed("dot", 1)));
        assert_eq!(
            click_at(&mut island, dot, BTN_LEFT, t0),
            [IslandAction::FocusWorkspace(1)]
        );
        assert!(!island.state.is_open());
        // A right click on the icon asks for its menu and closes the page.
        island.open(PageId::ACTIVITY, t0);
        let actions = click_at(&mut island, icon, BTN_RIGHT, t0);
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
        island.set_bar(Some(&BarLayout::default()));
        assert!(!island.stale, "the same layout");
        let mut only_tray = BarLayout::default();
        only_tray.start.clear();
        island.set_bar(Some(&only_tray));
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
    fn leaving_a_surface_closes_the_menus_and_the_page_at_once_and_keeps_the_rest() {
        let (mut island, t0) = bar_island(1920.0);
        island.open(PageId::TRAY, t0);
        island.menu = Some(PopupId(1));
        island.tray_menu = Some(PopupId(2));
        island.tray_menu_on_bar = true;
        island.bar_menu_asked = true;

        assert_eq!(island.detach_at(t0), [PopupId(1), PopupId(2)]);
        assert!(island.menu.is_none() && island.tray_menu.is_none());
        assert!(!island.tray_menu_on_bar && !island.bar_menu_asked);
        // Closed, at once even with motion on.
        assert!(!island.state.is_open());
        assert_eq!(island.state.progress(t0), 0.0);
        assert!(!island.state.wants_frame(t0));
        assert!(!island.reduced, "the motion setting is kept");
        assert!(island.detach_at(t0).is_empty(), "nothing left to close");

        // On its new surface, the other shape: the bar goes, and the capsule
        // opens the page it last showed.
        island.set_bar(None);
        island.apply_views(t0);
        lay_fake(&mut island, t0);
        assert!(island.capsule.node_by_key(Key::named("dots")).is_some());
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(PageId::TRAY));
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
        island.set_bar(Some(&edited));
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
    fn rows_activate_focus_requests_and_the_capsule_opens_the_last_page() {
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
        assert_eq!(island.page(), Some(PageId::ACTIVITY));
        island.activate(view::close_key(), t0);
        assert_eq!(island.page(), None);
        island.open(PageId::TRAY, t0);
        island.close(t0);
        island.activate(view::capsule_key(), t0);
        assert_eq!(island.page(), Some(PageId::TRAY));
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
                .open_menu_action(a, 0, false)
                .into_iter()
                .collect::<Vec<_>>()
        );
        // A right click on an item without a menu, a click between rows
        // and any other button do nothing.
        assert!(island.other_button(x1, y1, BTN_RIGHT).is_empty());
        assert!(island.other_button(1.0, 1.0, BTN_MIDDLE).is_empty());
        assert!(island.other_button(x0, y0, 0x113).is_empty());
    }

    #[test]
    fn the_menu_key_opens_the_menu_of_the_focused_row_only() {
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
        assert!(island.menu_key().is_empty(), "the second row has no menu");
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
    fn the_glance_pieces_read_the_bars_slices_and_a_click_opens_their_pages() {
        let (mut island, t0) = bar_island(1920.0);
        island.set_bar(Some(&glance_layout()));
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

        for (key, page) in [
            (Key::named("sysmon"), PageId::SYSMON),
            (Key::named("weather"), PageId::CLOCK),
            (Key::named("clock-date"), PageId::CLOCK),
        ] {
            island.close(t0);
            let at = centre(bar_rect(&island, key));
            assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
            assert_eq!(island.page(), Some(page), "{key:?}");
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
}
