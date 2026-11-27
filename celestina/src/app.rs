//! The app: the `celestina_wayland::Shell` of the product. It keeps one
//! Island per output on a `celestina-panel` layer surface, the one painter
//! of the process (created on the first frame of any surface, when the GL
//! context exists), the last niri model and the appearance, and routes the
//! workers' messages: the model to every Island, the appearance to the
//! painter and the trees, a verb to the Island of the focused output. It
//! also opens the Island's overflow menu and makes the menu's choices:
//! quit, do-not-disturb (the shell's own settings file) and reopen. It opens
//! the launcher on its own surface on the output with the keyboard, keeps the
//! desktop index it searches, starts what it launches, records the launch,
//! and runs the switches chosen in it. A change to `shell.json` applies live:
//! the bar's layout is laid out again on the surfaces the Islands have.

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use celestina_core::desktop_entry::{DesktopEntry, Locale};
use celestina_paint::{Painter, Rect, TextSystem, Theme};
use celestina_settings::Appearance;
use celestina_shell::notifications::Notification;
use celestina_shell::phone::PhoneState;
use celestina_shell::settings::{BarLayout, Duties, Duty, Settings};
use celestina_wayland::{
    Anchoring, FrameCx, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputEvent, OutputId,
    OutputInfo, PopupId, RuntimeCtl, RuntimeError, Sender, Shell, SurfaceCtl, SurfaceId,
};

use crate::notifications::{
    banners::{self, BannerAction, NoticePainter},
    handover::{Answer, Handover, Post},
    island_notice::Level,
};
use crate::{
    bar,
    bubbles::{self, Bubbles, BubblesHandle, BubblesNews},
    catcher::{self, Catchers},
    clipboard::{ClipboardDuty, ClipboardMessage, ClipboardState},
    command::{Presence, Verb},
    connectivity::{self, state::ConnectivityState, ConnectivityHandle, Forwarded},
    energy::{self, EnergyHandle},
    error_chain,
    glance::{self, GlanceHandle},
    island::{
        self, clock,
        clock::ClockTick,
        menu::{self, ChoiceSlot, Menu, MenuChoice, MenuState, SharedPainter},
        pages::{tray::TrayList, NameOwner},
        Island, IslandAction, Shared,
    },
    launcher::{self, usage::Usage, Launcher, LauncherAction, Switch},
    lock::LockHandle,
    media::control::{route, MediaControl, Routed, Snapshot as MediaSnapshot},
    message::Message,
    notifications::NotificationsState,
    osd::{
        control::{self, Controls, Opened, Opening},
        surface::{Order, Placement},
        verbs::{self as level_verbs, NoWorkers},
        Suppression,
    },
    overview::{
        self,
        flow::{capture_request, Step},
        slot::{NotAsked, Slot as OverviewSlot},
        Mode as OverviewMode, Overview, OverviewAction,
    },
    phone::{self, PhoneHandle, PhoneNews},
    pieces::PageId,
    polkit::{
        self,
        duty::{Change, LauncherSlot, PolkitDuty},
        state::Ctx,
        Notice, PolkitState, PromptHost,
    },
    providers::{
        magnetita::{MagnetitaEvent, MediaKind},
        melibea::{BubbleAction, MelibeaEvent},
        niri::{NiriModel, NiriProvider, NiriRequest},
        notifications::{NotificationsCommand, NotificationsProvider},
        settings::ShellSettings,
        sysmon::SysReading,
        tray::TrayProvider,
        weather::WeatherReading,
    },
    tray::{
        menu::{
            AskSink, Chain, ChoiceSlot as TrayChoiceSlot, MenuContext, MenuModel,
            ShellChoiceSlot as TrayShellSlot, ShellEntry, TrayMenu,
        },
        ItemId, MenuTree, TrayCommand, TrayEvent,
    },
};

/// The Island's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-panel";
/// The Island's surface, logical px: the largest page plus its margin. The
/// surface never resizes; only its regions follow the Island (design §9).
/// It is as wide as the output and as tall as this; the width here stands
/// in until the output says its own.
pub const SURFACE_SIZE: (u32, u32) = (island::state::STRIP_W as u32, 560);

/// The shape of an Island's surface on an output `logical_w` px wide (0
/// when the output has not said) under `settings`: its anchoring (the whole
/// top edge, the one shape since the cutover), its logical size, and the
/// bar's layout in force.
fn island_shape(settings: &Settings, logical_w: u32) -> (Anchoring, (u32, u32), BarLayout) {
    let width = match logical_w {
        0 => SURFACE_SIZE.0,
        w => w,
    };
    (
        bar::layout::anchoring(),
        (width, SURFACE_SIZE.1),
        settings.bar_layout(),
    )
}

/// The Island's layer surface on `output`, of the shape [`island_shape`]
/// gave. Its namespace is the shell's own, and no bus name is involved: the
/// bar has no observable owner, so nothing probes for another bar.
fn island_layer(
    output: OutputId,
    anchor: Anchoring,
    size: (u32, u32),
    exclusive_zone: i32,
) -> LayerSpec {
    LayerSpec {
        output,
        namespace: NAMESPACE,
        layer: Layer::Top,
        anchor,
        size,
        exclusive_zone,
        keyboard: KeyboardInteractivity::None,
    }
}

/// What a new `shell.json` asks of the Islands' surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BarChange {
    Nothing,
    /// The bar has another layout: each Island lays it out again on the
    /// surface it has.
    Relayout,
}

fn bar_change(before: &Settings, after: &Settings) -> BarChange {
    if before.bar_layout() == after.bar_layout() {
        BarChange::Nothing
    } else {
        BarChange::Relayout
    }
}

/// Gives `island` the media state in `snapshot`; whether a frame is worth
/// drawing.
fn media_island(island: &mut Island, painter: &mut Painter, snapshot: &MediaSnapshot) -> bool {
    let mut draw = island.set_media(
        painter,
        snapshot.on,
        &snapshot.players,
        snapshot.active.as_deref(),
    );
    draw |= island.set_media_unavailable(snapshot.unavailable);
    // An Island that already holds a cover for the same art ignores it: only
    // a changed `art_url` (or a new Island) uploads.
    for (bus_name, art_url, cover) in &snapshot.covers {
        draw |= island.set_media_cover(painter, bus_name, art_url, Some(cover));
    }
    draw
}

/// One output's Island and its surface.
#[derive(Debug)]
struct Slot {
    output: OutputId,
    surface: SurfaceId,
    island: Island,
}

/// The Islands by output and surface.
#[derive(Debug, Default)]
pub struct Islands {
    slots: Vec<Slot>,
}

impl Islands {
    pub fn add(&mut self, output: OutputId, surface: SurfaceId, island: Island) {
        self.slots.push(Slot {
            output,
            surface,
            island,
        });
    }

    /// Gives every Island `layout`; their surfaces stay as they are.
    fn set_bar(&mut self, layout: &BarLayout) {
        for slot in &mut self.slots {
            slot.island.set_bar(layout);
        }
    }

    /// Drops the Islands of `output` (its surfaces are already gone);
    /// returns their surfaces.
    pub fn remove_output(&mut self, output: OutputId) -> Vec<SurfaceId> {
        let gone: Vec<SurfaceId> = self
            .slots
            .iter()
            .filter(|s| s.output == output)
            .map(|s| s.surface)
            .collect();
        self.slots.retain(|s| s.output != output);
        gone
    }

    /// `output`'s Islands are going: whether one of them held the server's
    /// clock for its card, which must then let go. Call it before
    /// [`Islands::remove_output`]; told once.
    pub fn release_card_holds(&mut self, output: OutputId) -> bool {
        let mut held = false;
        for slot in self.slots.iter_mut().filter(|s| s.output == output) {
            held |= slot.island.release_card_hold();
        }
        held
    }

    /// Closes the level sheets of `output`'s Islands, flushing what their
    /// throttles hold. Their surfaces may be gone already: no surface is
    /// touched.
    pub fn close_level_sheets(&mut self, output: OutputId) {
        for slot in self.slots.iter_mut().filter(|s| s.output == output) {
            slot.island.close_level_sheet_now();
        }
    }

    /// Gives the tray icons of `output`'s Islands back to `painter`.
    pub fn release_icons(&mut self, output: OutputId, painter: &mut Painter) {
        for slot in self.slots.iter_mut().filter(|s| s.output == output) {
            slot.island.release_icons(painter);
        }
    }

    pub fn get_mut(&mut self, surface: SurfaceId) -> Option<&mut Island> {
        self.slots
            .iter_mut()
            .find(|s| s.surface == surface)
            .map(|s| &mut s.island)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The surface a verb goes to: the Island holding the keyboard, else
    /// the Island of the output niri has focused, else the first.
    #[must_use]
    pub fn target(
        &self,
        keyboard: Option<SurfaceId>,
        focused_output: Option<&str>,
    ) -> Option<SurfaceId> {
        keyboard
            .filter(|k| self.slots.iter().any(|s| s.surface == *k))
            .or_else(|| {
                let name = focused_output?;
                self.slots
                    .iter()
                    .find(|s| s.island.output() == name)
                    .map(|s| s.surface)
            })
            .or_else(|| self.slots.first().map(|s| s.surface))
    }

    /// The output of the Island on `surface`.
    #[must_use]
    pub fn output_of(&self, surface: SurfaceId) -> Option<OutputId> {
        self.slots
            .iter()
            .find(|s| s.surface == surface)
            .map(|s| s.output)
    }

    /// The surface of `output`'s Island.
    #[must_use]
    pub fn surface_on(&self, output: OutputId) -> Option<SurfaceId> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .map(|s| s.surface)
    }

    fn iter(&self) -> impl Iterator<Item = &Slot> {
        self.slots.iter()
    }

    /// The output the banners go to: niri's focused output when an Island
    /// is on it, else the first Island's.
    #[must_use]
    pub fn banner_output(&self, focused_output: Option<&str>) -> Option<OutputId> {
        focused_output
            .and_then(|name| self.slots.iter().find(|s| s.island.output() == name))
            .or_else(|| self.slots.first())
            .map(|s| s.output)
    }

    /// The surfaces to redraw when what every Island shows changed: all but
    /// the one on `covered`, the output whose capsule a notices surface is
    /// about to lend or return, and whose redraw then carries the change.
    fn redraw_targets(&self, covered: Option<OutputId>) -> Vec<SurfaceId> {
        self.slots
            .iter()
            .filter(|s| Some(s.output) != covered)
            .map(|s| s.surface)
            .collect()
    }

    /// `output`'s Island.
    fn island_of(&self, output: OutputId) -> Option<&Island> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .map(|s| &s.island)
    }

    /// Lends `output`'s capsule to the notices surface, or takes it back;
    /// whether an Island is there.
    fn lend(&mut self, output: OutputId, lent: bool) -> bool {
        match self.slots.iter_mut().find(|s| s.output == output) {
            Some(slot) => {
                slot.island.lend_capsule(lent);
                true
            }
            None => false,
        }
    }

    /// Does what the hand-over answered on `output`. Returns whether its
    /// panel is to be redrawn and whether the release is to be posted for
    /// the next loop turn.
    fn answer(&mut self, answer: Answer, output: OutputId) -> (bool, bool) {
        match answer {
            Answer::Lend => (self.lend(output, true), false),
            Answer::Return => (self.lend(output, false), true),
            Answer::Nothing => (false, false),
        }
    }

    /// A notices surface on `output` goes for another reason (the focus
    /// moved): the panel takes the capsule back if it was lent. Whether the
    /// panel is to be redrawn.
    fn retire(&mut self, handover: &mut Handover, output: OutputId) -> bool {
        handover.retire() && self.lend(output, false)
    }

    /// Where the notices and the pages are at `now`, for the display's choice
    /// of output and position: a page is there while it is open or still
    /// shrinking back to the capsule.
    fn placement(&self, notices: Option<OutputId>, now: std::time::Instant) -> Placement {
        Placement {
            notices,
            pages: self.pages_out(now),
        }
    }

    /// Whether `output`'s Island has its page open.
    fn page_open_on(&self, output: OutputId) -> bool {
        self.island_of(output).is_some_and(Island::page_open)
    }

    /// Whether `output`'s Island page is open or still closing at `now`.
    fn page_out_on(&self, output: OutputId, now: std::time::Instant) -> bool {
        self.island_of(output).is_some_and(|i| i.page_out(now))
    }

    /// The outputs whose Island page is open or still closing at `now`.
    fn pages_out(&self, now: std::time::Instant) -> Vec<OutputId> {
        self.slots
            .iter()
            .filter(|s| s.island.page_out(now))
            .map(|s| s.output)
            .collect()
    }

    /// Routes the newest toast (§5.5): when the page of `wanted`'s Island is
    /// open, that Island takes it as a card and the others hold none;
    /// otherwise no Island holds one and the notices surface shows it.
    fn route_notice(
        &mut self,
        wanted: Option<OutputId>,
        toast: Option<&Notification>,
        now: std::time::Instant,
    ) -> NoticeRoute {
        let carded = toast.is_some() && wanted.is_some_and(|o| self.page_open_on(o));
        let mut redraw = Vec::new();
        for slot in &mut self.slots {
            let mine = if carded && Some(slot.output) == wanted {
                toast.cloned()
            } else {
                None
            };
            if slot.island.set_notice(mine, now) {
                redraw.push(slot.surface);
            }
        }
        NoticeRoute { carded, redraw }
    }

    /// Whether an Island holds a notice as a card.
    fn any_notice(&self) -> bool {
        self.slots.iter().any(|s| s.island.has_notice())
    }

    /// What each Island wants the server's clock to do for its card now
    /// (`true` holds it, `false` lets it go).
    fn take_card_holds(&mut self) -> Vec<bool> {
        self.slots
            .iter_mut()
            .filter_map(|slot| slot.island.take_card_hold())
            .collect()
    }

    /// The surface of `output`'s Island.
    fn surface_of(&self, output: OutputId) -> Option<SurfaceId> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .map(|s| s.surface)
    }
}

/// Where the newest toast went.
#[derive(Debug)]
struct NoticeRoute {
    /// The banner output's Island took it as a card: no surface.
    carded: bool,
    /// The panels whose notice changed.
    redraw: Vec<SurfaceId>,
}

/// The notices' surface, the output it is on, what it paints and where the
/// Island's capsule is.
#[derive(Debug)]
struct BannerSurface {
    output: OutputId,
    surface: SurfaceId,
    notice: NoticePainter,
    handover: Handover,
    /// The minute deadline of the clock face it paints.
    clock: ClockTick,
}

/// The output niri's focused workspace is on.
fn focused_output(model: Option<&NiriModel>) -> Option<&str> {
    model?
        .workspaces
        .iter()
        .find(|w| w.is_focused)
        .and_then(|w| w.output.as_deref())
}

/// What the overflow menu shows: the do-not-disturb setting in force, and
/// whether another program holds the notifications name (the row is then
/// disabled and says so).
fn menu_state(settings: &ShellSettings, owners: &NameOwner) -> MenuState {
    MenuState {
        do_not_disturb: settings.current().do_not_disturb,
        dnd_held: owners.notifications.is_some(),
    }
}

/// `owners` with the notifications holder replaced by `holder`; the tray's
/// is kept.
fn with_notifications_owner(owners: &NameOwner, holder: Option<&str>) -> NameOwner {
    NameOwner {
        notifications: holder.map(str::to_owned),
        tray: owners.tray.clone(),
    }
}

/// The slot a new menu writes its choice into, emptied first: a choice
/// left by an earlier menu must never fire on a later, unrelated popup
/// close.
fn fresh_choice(slot: &ChoiceSlot) -> ChoiceSlot {
    slot.set(None);
    Rc::clone(slot)
}

/// A tray menu asked for and not yet delivered: where to hang it once the
/// host answers.
#[derive(Debug)]
struct PendingMenu {
    surface: SurfaceId,
    item: ItemId,
    row: Rect,
    keyboard: bool,
    /// When an empty menu stops waiting for its late layout; set by the
    /// first empty one.
    until: Option<Instant>,
}

/// How long an empty menu root waits for the `LayoutUpdated` that fills it.
const MENU_LATE_LAYOUT: Duration = Duration::from_millis(500);

/// What a menu from the host means for the request waiting for it.
#[derive(Debug)]
enum Pending {
    /// Nobody asked for it, or the item is not the one asked for.
    Unasked,
    /// The request, to open the menu now.
    Open(PendingMenu),
    /// The root is empty: the request stays, for this much longer.
    Wait(Duration),
    /// The root stayed empty past its wait: the request is dropped.
    GiveUp(PendingMenu),
}

impl Pending {
    /// What a menu of `item` that is `empty` or not does to `pending`,
    /// at `now`. An empty root keeps the request for [`MENU_LATE_LAYOUT`]
    /// from the first empty one, so one `LayoutUpdated` can fill it.
    fn of(pending: &mut Option<PendingMenu>, item: &ItemId, empty: bool, now: Instant) -> Pending {
        if pending.as_ref().is_none_or(|p| p.item != *item) {
            return Pending::Unasked;
        }
        if !empty {
            return pending.take().map_or(Pending::Unasked, Pending::Open);
        }
        let Some(request) = pending.as_mut() else {
            return Pending::Unasked;
        };
        let until = *request.until.get_or_insert(now + MENU_LATE_LAYOUT);
        match until.checked_duration_since(now).filter(|d| !d.is_zero()) {
            Some(left) => Pending::Wait(left),
            None => pending.take().map_or(Pending::Unasked, Pending::GiveUp),
        }
    }

    /// Puts `request` in `pending`; the request it replaces, when that was
    /// for another item.
    fn replace(pending: &mut Option<PendingMenu>, request: PendingMenu) -> Option<PendingMenu> {
        pending
            .replace(request)
            .filter(|old| pending.as_ref().is_some_and(|new| new.item != old.item))
    }

    /// The item of the request that waited past its time, left in `pending`
    /// (its card may still open, with the shell's entry alone).
    fn due(pending: &Option<PendingMenu>, now: Instant) -> Option<ItemId> {
        pending
            .as_ref()
            .filter(|p| p.until.is_some_and(|until| until <= now))
            .map(|p| p.item.clone())
    }

    /// The request that waited past its time, taken out of `pending`.
    fn expired(pending: &mut Option<PendingMenu>, now: Instant) -> Option<PendingMenu> {
        pending.take_if(|p| p.until.is_some_and(|until| until <= now))
    }
}

/// Writes `bar.tray_hidden` for the shell's `entry` on the item whose `Id`
/// is `sni_id`: hiding appends it once, showing removes it. Whether the
/// list changed.
fn apply_shell_entry(
    settings: &mut ShellSettings,
    sni_id: &str,
    entry: ShellEntry,
) -> Result<bool, crate::providers::settings::ShellSettingsError> {
    settings.set_tray_hidden(sni_id, entry == ShellEntry::Hide)
}

/// The tray menu chain that is open: its Island, its item, the model its
/// levels show and its popups, so a republished menu reaches the levels
/// and the host hears when the chain is gone.
#[derive(Debug)]
struct OpenTrayMenu {
    surface: SurfaceId,
    item: ItemId,
    root: PopupId,
    cx: MenuContext,
}

/// The product's `Shell`.
pub struct App {
    islands: Islands,
    /// The fonts, until the painter takes them on the first frame.
    fonts: Option<TextSystem>,
    /// The one painter, shared with the open menu.
    painter: Option<SharedPainter>,
    appearance: Appearance,
    /// The shell's own settings (do-not-disturb, the bar's layout and the
    /// duties), kept in step with the file by its follower.
    settings: ShellSettings,
    /// Where the open menu leaves the row chosen in it.
    menu_choice: ChoiceSlot,
    model: Option<NiriModel>,
    /// Who holds the bus names the Notifications and Tray pages depend on;
    /// nobody until the owner detection of CEL-1-F / CEL-1-G fills it.
    owners: NameOwner,
    /// The Island surface holding the keyboard.
    keyboard: Option<SurfaceId>,
    /// The compositor's request queue, set once the provider runs.
    niri: Rc<OnceCell<NiriProvider>>,
    /// The open launcher, if any.
    launcher: Option<Launcher>,
    /// The session's applications, from the desktop index's worker.
    index: Vec<DesktopEntry>,
    /// The applications launched last, which the ranking lifts.
    usage: Usage,
    /// The loop's own sender, set once the loop runs: the launcher's close
    /// is posted through it (see [`Message::LauncherClosed`]), and so is a
    /// banner's click asking for the page (see
    /// [`Message::OpenNotificationsPage`]), because an input handler
    /// controls only its own surface.
    sender: Rc<OnceCell<Sender<Message>>>,
    /// The notification server's latest snapshot, holder and queue.
    notifications: NotificationsState,
    /// The notices' surface, while the Island shows a notice (or is still
    /// shrinking back from one).
    banners: Option<BannerSurface>,
    /// The level the display last ordered on the Island, until it leaves:
    /// a notices surface made later (or on another output) starts with it.
    island_level: Option<Level>,
    /// Surfaces of notices that left an output whose focus moved: their
    /// capsule is back on the panel and they go on the next loop turn.
    banners_retiring: Vec<SurfaceId>,
    /// The outputs whose Island page was open when the notices last looked.
    pages_seen: Vec<OutputId>,
    /// The catchers under the Islands that show something (CEL-D-003).
    catchers: Catchers,
    /// Why the run stopped on a failure, for the binary to report.
    fatal: Rc<OnceCell<String>>,
    refusal_logged: bool,
    metrics: bool,
    /// The tray host's command queue, set once the host runs.
    tray: Rc<OnceCell<TrayProvider>>,
    /// The tray's items, shared by every Island.
    tray_items: TrayList,
    /// Where the open tray menu leaves the entry chosen in it.
    tray_choice: TrayChoiceSlot,
    /// Where a tray menu leaves the shell's own entry it chose.
    tray_shell_choice: TrayShellSlot,
    /// The menu the last right click asked for.
    pending_menu: Option<PendingMenu>,
    /// The tray menu chain open right now.
    open_tray_menu: Option<OpenTrayMenu>,
    tray_refusal_logged: bool,
    /// The wallpaper duty: its surfaces, textures and requests.
    wallpapers: crate::wallpaper::surface::Wallpapers,
    /// The energy duties' workers, set once the loop runs.
    energy: Rc<OnceCell<EnergyHandle>>,
    /// The `polkit` duty (CEL-2-I): nothing while off, the agent and the
    /// prompt state while on.
    polkit: PolkitDuty,
    /// The session lock's side of the shell, set once the loop runs.
    lock: Rc<OnceCell<LockHandle>>,
    /// The glance workers (system monitor, weather), set once the loop runs.
    glance: Rc<OnceCell<GlanceHandle>>,
    /// The system monitor's newest reading, while its worker runs.
    sysmon: Option<SysReading>,
    /// The weather's reading, while its worker runs.
    weather: Option<WeatherReading>,
    /// The clipboard duty's worker, set once the loop runs.
    clipboard: Rc<OnceCell<ClipboardDuty>>,
    /// The latest rows the clipboard worker published: previews only, empty
    /// while the duty is off.
    clipboard_state: ClipboardState,
    /// What the connectivity workers last said, and the requests in flight;
    /// every Island gets a copy when it changes.
    connectivity: ConnectivityState,
    /// The connectivity workers, put in once the loop runs and taken out by
    /// the binary at exit to be joined.
    connectivity_handle: Rc<RefCell<Option<ConnectivityHandle>>>,
    /// The on-screen display duty: its surface and its readings.
    osds: crate::osd::surface::Osds,
    /// The audio and brightness readings the bar's pieces show, and the
    /// level sheet that is open.
    controls: Controls,
    /// The phone duty's worker, set once the loop runs (CEL-2-J).
    phone: Rc<OnceCell<PhoneHandle>>,
    /// The phone as the last accepted news shows it.
    phone_state: PhoneState,
    /// The bubbles duty's worker, set once the loop runs (CEL-2-J).
    bubbles: Rc<OnceCell<BubblesHandle>>,
    /// The minimised windows as the last accepted news shows them.
    bubbles_state: Bubbles,
    /// The close requests Melibea took and has not settled (no second Delete
    /// for the same window meanwhile).
    bubble_closes: bubbles::PendingCloses,
    /// The media duty's worker and the state it published, set once the loop runs.
    media: Rc<OnceCell<MediaControl>>,
    /// The media state in force was not given to the Islands (the painter
    /// was in use): the next event or frame gives it.
    media_dirty: bool,
    /// The overview: its state machine, panel and short-lived workers.
    overview: OverviewSlot,
    /// The outputs as the compositor last described them, for the capture
    /// request and the overview's output name.
    outputs: Vec<OutputInfo>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("islands", &self.islands.len())
            .field("painter", &self.painter.is_some())
            .field("appearance", &self.appearance)
            .finish_non_exhaustive()
    }
}

impl App {
    /// The app before any output: `fonts` for the painter, the appearance
    /// and the shell's settings read at start, the slots the niri provider
    /// and the notification worker's handle are put in once they run,
    /// the slot the app writes why it stopped the run into, and whether
    /// `--metrics` asked for the atlas's occupancy at exit.
    #[must_use]
    pub fn new(
        fonts: TextSystem,
        appearance: Appearance,
        settings: ShellSettings,
        niri: Rc<OnceCell<NiriProvider>>,
        notifications: Rc<OnceCell<NotificationsProvider>>,
        fatal: Rc<OnceCell<String>>,
        metrics: bool,
    ) -> App {
        App {
            islands: Islands::default(),
            fonts: Some(fonts),
            painter: None,
            appearance,
            settings,
            menu_choice: ChoiceSlot::default(),
            model: None,
            owners: NameOwner::default(),
            keyboard: None,
            niri,
            launcher: None,
            index: Vec::new(),
            usage: Usage::default(),
            sender: Rc::new(OnceCell::new()),
            notifications: NotificationsState::new(notifications),
            banners: None,
            island_level: None,
            banners_retiring: Vec::new(),
            pages_seen: Vec::new(),
            catchers: Catchers::default(),
            fatal,
            refusal_logged: false,
            metrics,
            tray: Rc::new(OnceCell::new()),
            tray_items: TrayList::default(),
            tray_choice: TrayChoiceSlot::default(),
            tray_shell_choice: TrayShellSlot::default(),
            pending_menu: None,
            open_tray_menu: None,
            tray_refusal_logged: false,
            wallpapers: crate::wallpaper::surface::Wallpapers::default(),
            energy: Rc::new(OnceCell::new()),
            polkit: PolkitDuty::new(Rc::new(OnceCell::new())),
            lock: Rc::new(OnceCell::new()),
            glance: Rc::new(OnceCell::new()),
            sysmon: None,
            weather: None,
            clipboard: Rc::new(OnceCell::new()),
            clipboard_state: ClipboardState::default(),
            connectivity: ConnectivityState::default(),
            connectivity_handle: Rc::new(RefCell::new(None)),
            // A quiet placeholder (every duty off) until `with_osd` brings
            // the runtime and the settings in force: nothing is shown on a
            // duty the settings have not confirmed.
            osds: crate::osd::surface::Osds::new(
                Rc::new(OnceCell::new()),
                &Settings {
                    duties: Duties::all_off(),
                    ..Settings::default()
                },
                &Theme::dark(),
            ),
            controls: Controls::new(),
            phone: Rc::new(OnceCell::new()),
            phone_state: PhoneState::Absent,
            bubbles: Rc::new(OnceCell::new()),
            bubbles_state: Bubbles::default(),
            bubble_closes: bubbles::PendingCloses::default(),
            media: Rc::new(OnceCell::new()),
            media_dirty: false,
            overview: OverviewSlot::default(),
            outputs: Vec::new(),
        }
    }

    /// Takes the slot the polkit launcher is put in once the loop runs. The
    /// `polkit` duty starts the agent through it, at start when the duty is on
    /// and live whenever `shell.json` flips it.
    #[must_use]
    pub fn with_polkit(mut self, launcher: LauncherSlot) -> App {
        self.polkit = PolkitDuty::new(launcher);
        self
    }

    /// Takes the slot the glance handle is put in once the loop runs.
    #[must_use]
    pub fn with_glance(mut self, glance: Rc<OnceCell<GlanceHandle>>) -> App {
        self.glance = glance;
        self
    }

    /// The system monitor's reading arrived (or was withdrawn). Each Island
    /// is woken only as far as it shows the change ([`Island::sysmon_wake`]):
    /// a sample every two seconds must not redraw a bar at rest. A reading
    /// from a worker no longer running is dropped.
    fn sysmon_read(&mut self, reading: Option<SysReading>, ctl: &mut RuntimeCtl<'_>) {
        let running = self.glance.get().is_some_and(|g| g.running().sysmon);
        self.sysmon = glance::admit(running, reading);
        let now = std::time::Instant::now();
        for slot in self.islands.iter() {
            let wake = slot.island.sysmon_wake(self.sysmon.as_ref(), now);
            if let Some(mut surface) = ctl.surface(slot.surface) {
                wake.apply(&mut surface);
            }
        }
    }

    /// The weather's reading arrived (or was withdrawn); a reading from a
    /// worker no longer running is dropped.
    fn weather_read(&mut self, reading: Option<WeatherReading>, ctl: &mut RuntimeCtl<'_>) {
        let running = self.glance.get().is_some_and(|g| g.running().weather);
        let reading = glance::admit(running, reading);
        if reading != self.weather {
            self.weather = reading;
            self.redraw_all(ctl);
        }
    }

    /// The glance workers follow `settings`; a reading whose worker stopped
    /// is withdrawn.
    fn apply_glance(&mut self, settings: &Settings, ctl: &mut RuntimeCtl<'_>) {
        let Some(handle) = self.glance.get() else {
            return;
        };
        let running = handle.apply(settings);
        if !running.sysmon && self.sysmon.is_some() {
            self.sysmon_read(None, ctl);
        }
        if !running.weather {
            self.weather_read(None, ctl);
        }
    }

    /// Takes the slot the clipboard duty is put in once the loop runs.
    #[must_use]
    pub fn with_clipboard(mut self, clipboard: Rc<OnceCell<ClipboardDuty>>) -> App {
        self.clipboard = clipboard;
        self
    }

    /// The clipboard history as the worker last published it, previews only.
    #[must_use]
    pub fn clipboard_state(&self) -> &ClipboardState {
        &self.clipboard_state
    }

    /// Takes the slot the connectivity workers' handle is put in once the
    /// loop runs.
    #[must_use]
    pub fn with_connectivity(mut self, handle: Rc<RefCell<Option<ConnectivityHandle>>>) -> App {
        self.connectivity_handle = handle;
        self
    }

    /// Takes the slot the on-screen display's workers and timer are put in
    /// once the loop runs, under the settings read at start.
    #[must_use]
    pub fn with_osd(mut self, runtime: Rc<OnceCell<crate::osd::OsdRuntime>>) -> App {
        self.osds = crate::osd::surface::Osds::new(runtime, self.settings.current(), &self.theme());
        self
    }

    /// Takes the slots the phone and bubbles handles are put in once the loop
    /// runs.
    #[must_use]
    pub fn with_phone_and_bubbles(
        mut self,
        phone: Rc<OnceCell<PhoneHandle>>,
        bubbles: Rc<OnceCell<BubblesHandle>>,
    ) -> App {
        self.phone = phone;
        self.bubbles = bubbles;
        self
    }

    /// Takes the slot the media control is put in once the loop runs.
    #[must_use]
    pub fn with_media(mut self, media: Rc<OnceCell<MediaControl>>) -> App {
        self.media = media;
        self
    }

    /// Takes the slot the energy handle is put in once the loop runs.
    #[must_use]
    pub fn with_energy(mut self, energy: Rc<OnceCell<EnergyHandle>>) -> App {
        self.energy = energy;
        self
    }

    /// Takes the slot the session lock's handle is put in once the loop runs.
    #[must_use]
    pub fn with_lock(mut self, lock: Rc<OnceCell<LockHandle>>) -> App {
        self.lock = lock;
        self
    }

    /// The launcher's parts: the `usage` record read at start and the
    /// slot the loop's own sender is put in once the loop runs.
    #[must_use]
    pub fn with_launcher(mut self, usage: Usage, sender: Rc<OnceCell<Sender<Message>>>) -> App {
        self.usage = usage;
        self.sender = sender;
        self
    }

    /// Takes the slot the tray host's command queue is put in once it runs.
    #[must_use]
    pub fn with_tray(mut self, tray: Rc<OnceCell<TrayProvider>>) -> App {
        self.tray = tray;
        self
    }

    /// Takes the slot the wallpaper worker's queue is put in once it runs, and
    /// the store the session lock reads the wallpaper thumbnails from.
    #[must_use]
    pub fn with_wallpaper(
        mut self,
        provider: Rc<OnceCell<crate::providers::wallpaper::WallpaperProvider>>,
        thumbnails: crate::wallpaper::thumbnail::ThumbnailStore,
    ) -> App {
        self.wallpapers = crate::wallpaper::surface::Wallpapers::new(provider, thumbnails);
        self
    }

    /// `shell.json` changed on disk ([`Message::ShellSettings`]): its value
    /// is the one in force, and what changed is applied live, never by a
    /// restart: the bar's layout, the duties (wallpaper,
    /// energy and media), the energy switches and do-not-disturb. The media
    /// duty is applied on every adopted read, not only when its switch
    /// changed, so a worker that failed to start is tried again by the next
    /// write that changes the file while the duty stays on. The shell's own
    /// writes arrive as the value in force and do nothing. A new output reads
    /// the settings in force when it comes.
    fn shell_settings_read(
        &mut self,
        read: crate::providers::settings::ShellRead,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        let before = self.settings.current().clone();
        let adopted = self.settings.adopt_read(read);
        // A clean read ends the protection even when it equals the defaults
        // in force (and so changes nothing): a clipboard deletion deferred
        // at start on the protected defaults' word is done now.
        if let Some(duty) = self.clipboard.get() {
            if duty.deletion_deferred() && !self.settings.is_protected() {
                duty.apply(self.settings.current(), false);
            }
        }
        if !adopted {
            return;
        }
        let after = self.settings.current().clone();
        let bar = bar_change(&before, &after);
        tracing::info!(?bar, "shell settings changed on disk");
        match bar {
            BarChange::Relayout => self.relayout_bar(ctl),
            BarChange::Nothing => {}
        }
        self.wallpapers
            .settings_changed(&before, &after, ctl, self.painter.as_ref());
        if before.do_not_disturb != after.do_not_disturb {
            self.notifications
                .send(NotificationsCommand::SetQuiet(after.do_not_disturb));
            self.redraw_all(ctl);
        }
        if before.duties != after.duties || before.energy != after.energy {
            if let Some(handle) = self.energy.get() {
                handle.apply(&after);
            }
        }
        if before.duties.polkit != after.duties.polkit {
            self.polkit_apply(ctl);
        }
        if before.duties.lock != after.duties.lock {
            if let Some(handle) = self.lock.get() {
                handle.apply(&after);
            }
        }
        // Every change: the workers follow the layout and the location, and
        // an apply that changes nothing is free; it is also
        // what starts again a weather worker found gone.
        self.apply_glance(&after, ctl);
        if before.duties.clipboard != after.duties.clipboard {
            if let Some(duty) = self.clipboard.get() {
                duty.apply(&after, self.settings.is_protected());
                if !duty.running() {
                    self.clipboard_state.reset();
                }
            }
            // The bar's clipboard piece appears or goes with the duty, even
            // over an empty history: every Island redraws.
            self.redraw_all(ctl);
        }
        let theme = self.theme();
        // The level sheet closes (and sends what its throttle holds) before
        // the duty's workers are torn down.
        if before.duties.osd == Duty::On && after.duties.osd == Duty::Off {
            self.close_open_level_control(ctl);
        }
        self.osds.settings_changed(&before, &after, ctl, &theme);
        self.level_orders(ctl);
        self.levels_settings_changed(&before, &after, ctl);
        if before.duties != after.duties {
            self.apply_phone_and_bubbles(&after, ctl);
        }
        if self.media.get().is_some_and(|media| media.apply(&after)) {
            let draw = self.media_sync();
            Self::redraw_surfaces(ctl, &draw);
        }
    }

    /// Brings the phone and bubbles workers to `settings`; a duty that went
    /// off takes its state with it, and the bars are drawn again once.
    fn apply_phone_and_bubbles(&mut self, settings: &Settings, ctl: &mut RuntimeCtl<'_>) {
        let mut changed = false;
        if let Some(handle) = self.phone.get() {
            if handle.apply(settings) && !handle.running() {
                changed |= phone::reduce(&mut self.phone_state, MagnetitaEvent::Absent);
            }
        }
        if let Some(handle) = self.bubbles.get() {
            if handle.apply(settings) && !handle.running() {
                self.bubble_closes.clear();
                if bubbles::reduce(&mut self.bubbles_state, &MelibeaEvent::Absent) {
                    changed = true;
                    self.refresh_selector(ctl);
                }
            }
        }
        if changed {
            self.redraw_all(ctl);
        }
    }

    /// Passes a clipboard verb (from the command socket or the Island's
    /// page) to the duty; the outcome is logged and carries no content.
    fn clipboard_verb(&self, verb: &Verb) {
        let Some(duty) = self.clipboard.get() else {
            tracing::warn!(?verb, "no clipboard duty: the verb does nothing");
            return;
        };
        if let Some(outcome) = crate::clipboard::verbs::run(verb, duty) {
            outcome.log();
        }
    }

    /// One report of the clipboard worker: kept as the latest rows when it
    /// comes from the worker that runs now, dropped when it is a stopped
    /// worker's late event.
    fn clipboard_event(&mut self, message: ClipboardMessage, ctl: &mut RuntimeCtl<'_>) {
        let current = self
            .clipboard
            .get()
            .is_some_and(|duty| duty.accepts(message.generation));
        if !current {
            return;
        }
        message.event.log();
        let paused = self.clipboard_state.paused();
        if self.clipboard_state.apply(message.event) {
            // The bar reads only whether the duty is on and paused; the rows
            // are the page's. Nothing else on screen changed with them.
            if self.clipboard_state.paused() != paused {
                self.redraw_all(ctl);
            } else {
                self.redraw_clipboard_pages(ctl);
            }
        }
    }

    /// Redraws the Islands that show the Clipboard page, and only those.
    fn redraw_clipboard_pages(&self, ctl: &mut RuntimeCtl<'_>) {
        for slot in self
            .islands
            .iter()
            .filter(|slot| slot.island.shows_clipboard_page())
        {
            if let Some(mut surface) = ctl.surface(slot.surface) {
                surface.request_redraw();
            }
        }
    }

    /// Closes the level sheet that is open, on its Island.
    fn close_open_level_control(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let Some(surface) = self.controls.open_surface() else {
            return;
        };
        if let (Some(island), Some(mut surface_ctl)) =
            (self.islands.get_mut(surface), ctl.surface(surface))
        {
            island.close_level_sheet(&mut surface_ctl);
        }
    }

    /// Opens the level sheet of `kind` from `piece` on `surface`'s Island and
    /// tells the display: what was released first, then what is quiet now.
    fn open_level(
        &mut self,
        kind: crate::osd::popup::Kind,
        piece: celestina_ui::Key,
        surface: SurfaceId,
        shared: &SharedPainter,
        ctl: &mut SurfaceCtl<'_>,
    ) {
        let opening = Opening {
            surface,
            painter: shared,
            runtime: self.osds.runtime(),
            reduced: self.appearance.reduced_motion,
        };
        let Some(island) = self.islands.get_mut(surface) else {
            return;
        };
        let Opened { opened, released } = self.controls.open(kind, piece, island, ctl, &opening);
        if let Some(released) = released {
            self.post(Message::OsdSuppressed(released.suppression));
            if let Some(elsewhere) = released.elsewhere {
                self.post(Message::CloseLevelSheet(elsewhere));
            }
        }
        if opened {
            self.post(Message::OsdSuppressed(Suppression::Open(
                control::kinds_of(kind),
            )));
        }
    }

    /// The Island of `surface` says whether its level sheet is still open: a
    /// sheet that closed (by any way) lets the display show again, once.
    fn settle_level_sheet(&mut self, surface: SurfaceId) {
        let Some(open) = self
            .islands
            .get_mut(surface)
            .map(|island| island.level_sheet_open())
        else {
            return;
        };
        if let Some(released) = self.controls.settle(surface, open) {
            self.post(Message::OsdSuppressed(released));
        }
    }

    /// The `osd` duty or the bar's layout changed: with the duty off the
    /// readings are forgotten (so the pieces vanish) and an open level sheet
    /// closes. Whether a layout holds the brightness piece is the runtime's
    /// to read, in `OsdRuntime::apply`.
    fn levels_settings_changed(
        &mut self,
        before: &Settings,
        after: &Settings,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        if before.duties.osd == Duty::On && after.duties.osd == Duty::Off {
            if let Some(released) = self.controls.closed() {
                let theme = self.theme();
                self.osds.set_suppressed(released, ctl, &theme);
                self.level_orders(ctl);
            }
            if self.controls.readings.clear() {
                self.levels_changed(ctl);
            }
        }
    }

    /// Whether the workers' readings are taken in: not after the duty went
    /// off, so a reading still queued then cannot bring the pieces back.
    fn levels_live(&self) -> bool {
        let running = self
            .osds
            .runtime()
            .get()
            .is_some_and(crate::osd::OsdRuntime::running);
        control::readings_accepted(self.settings.current(), running)
    }

    /// The readings changed: the Islands draw the pieces again and an open
    /// sheet shows the new levels.
    fn levels_changed(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.redraw_all(ctl);
        self.popup_refreshed(ctl);
    }

    /// An open level sheet shows what the readings and the audio graph hold
    /// now.
    fn popup_refreshed(&mut self, ctl: &mut RuntimeCtl<'_>) {
        if let Some(mut surface) = self.controls.refresh().and_then(|s| ctl.surface(s)) {
            surface.request_redraw();
        }
    }

    /// Posts `message` to the loop itself.
    fn post(&self, message: Message) {
        match self.sender.get() {
            Some(sender) => {
                if sender.send(message).is_err() {
                    tracing::debug!("the loop is gone: the message is dropped");
                }
            }
            None => tracing::debug!("no loop sender: the message is dropped"),
        }
    }

    /// A Volume or Brightness piece's action on the Island of `surface`.
    fn level_action(&mut self, action: IslandAction, surface: SurfaceId) {
        let Some(runtime) = self.osds.runtime().get() else {
            return;
        };
        match action {
            IslandAction::Audio(action) => self.controls.audio(action, runtime),
            IslandAction::Brightness(action) => {
                if let Some(island) = self.islands.get_mut(surface) {
                    self.controls.brightness(action, island.output(), runtime);
                }
            }
            _ => {}
        }
    }

    /// The open selector, if any, shows the minimised windows as they are
    /// now; its selection follows its window.
    fn refresh_selector(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let Some(open) = self.launcher.as_mut().filter(|l| l.is_selector()) else {
            return;
        };
        if open.set_bubbles(&self.bubbles_state) {
            if let Some(mut surface) = ctl.surface(open.surface()) {
                surface.request_redraw();
            }
        }
    }

    /// News from the Magnetita worker: only the running worker's is taken.
    fn phone_news(&mut self, news: PhoneNews, ctl: &mut RuntimeCtl<'_>) {
        if !self
            .phone
            .get()
            .is_some_and(|handle| handle.accepts(news.run))
        {
            return;
        }
        if phone::reduce(&mut self.phone_state, news.event) {
            self.redraw_all(ctl);
        }
    }

    /// News from the Melibea worker: only the running worker's is taken. How
    /// an action ended goes to the journal and nowhere else for now.
    fn bubbles_news(&mut self, news: BubblesNews, ctl: &mut RuntimeCtl<'_>) {
        if !self
            .bubbles
            .get()
            .is_some_and(|handle| handle.accepts(news.run))
        {
            return;
        }
        bubbles::log(&news.event);
        self.bubble_closes.observe(&news.event);
        if bubbles::reduce(&mut self.bubbles_state, &news.event) {
            self.refresh_selector(ctl);
            self.redraw_all(ctl);
        }
    }

    /// `bubbles minimize` and `phone media` (`bubbles select` opens a
    /// surface: [`App::select_bubbles`]).
    fn bubbles_and_phone_verb(&self, verb: &Verb) {
        match verb {
            Verb::BubblesMinimize => {
                let Some(handle) = self.bubbles.get() else {
                    tracing::info!("bubbles minimize: no bubbles handle; nothing was done");
                    return;
                };
                match handle.request(bubbles::minimize_action(self.appearance.reduced_motion)) {
                    Ok(id) => tracing::info!(id = id.0, "bubbles minimize: asked Melibea"),
                    Err(why) => tracing::info!("bubbles minimize: nothing was done: {why}"),
                }
            }
            Verb::PhoneMedia(kind) => self.phone_media(*kind),
            _ => {}
        }
    }

    /// Asks Magnetita for the media action `kind` on the connected phone:
    /// what `phone media` and the Phone page's buttons both do.
    fn phone_media(&self, kind: MediaKind) {
        let Some(request) = phone::media_request(&self.phone_state, kind) else {
            tracing::info!("phone media: no phone is connected; nothing was done");
            return;
        };
        let Some(handle) = self.phone.get() else {
            tracing::info!("phone media: no phone handle; nothing was done");
            return;
        };
        match handle.request(request) {
            Ok(()) => tracing::info!("phone media {}: asked Magnetita", kind.as_str()),
            Err(why) => tracing::info!("phone media: nothing was done: {why}"),
        }
    }

    /// Asks Melibea to bring the minimised window `window_id` back, with
    /// the transition `bubbles::restore_action` chooses.
    fn restore_window(&self, window_id: u64, output: &str, bubble: Option<Rect>) {
        let Some(handle) = self.bubbles.get() else {
            tracing::info!("bubbles restore: no bubbles handle; nothing was done");
            return;
        };
        // The bubble's rectangle is on its own output: it only travels with
        // the restore when the window is not known to be on another one.
        let known = self
            .model
            .as_ref()
            .and_then(|model| bubbles::window_output(model, window_id));
        let bubble = bubble.filter(|_| bubbles::anchor_allowed(known, output));
        let action = bubbles::restore_action(
            window_id,
            output,
            bubble.map(|r| (r.x, r.y, r.w, r.h)),
            self.appearance.reduced_motion,
        );
        match handle.request(action) {
            Ok(id) => tracing::info!(id = id.0, window_id, "bubbles restore: asked Melibea"),
            Err(why) => tracing::info!("bubbles restore: nothing was done: {why}"),
        }
    }

    /// Gives every Island the media state in force; returns the surfaces
    /// worth a frame. Without the painter nothing is given: the first frame
    /// gives it all.
    fn media_sync(&mut self) -> Vec<SurfaceId> {
        let (Some(media), Some(shared)) = (self.media.get(), self.painter.as_ref()) else {
            return Vec::new();
        };
        let snapshot = media.snapshot();
        let Ok(mut painter) = shared.try_borrow_mut() else {
            tracing::warn!("the painter is in use; the media state is given on the next frame");
            self.media_dirty = true;
            return Vec::new();
        };
        self.media_dirty = false;
        let mut draw = Vec::new();
        for slot in &mut self.islands.slots {
            if media_island(&mut slot.island, &mut painter, &snapshot) {
                draw.push(slot.surface);
            }
        }
        draw
    }

    /// The media worker's news ([`Message::Media`]).
    fn media_event(&mut self, event: crate::media::MediaEvent, ctl: &mut RuntimeCtl<'_>) {
        let Some(media) = self.media.get() else {
            return;
        };
        let draw = match route(media, event) {
            Routed::Nothing => return,
            Routed::Sync => self.media_sync(),
            Routed::Cover {
                bus_name,
                art_url,
                cover,
            } => {
                let Some(shared) = self.painter.as_ref() else {
                    // Kept by the control; the first frame gives it.
                    return;
                };
                let Ok(mut painter) = shared.try_borrow_mut() else {
                    // The whole snapshot carries the cover: one re-sync.
                    tracing::warn!("the painter is in use; a cover is given on the next frame");
                    self.media_dirty = true;
                    return;
                };
                let mut draw = Vec::new();
                for slot in &mut self.islands.slots {
                    if slot.island.set_media_cover(
                        &mut *painter,
                        &bus_name,
                        &art_url,
                        cover.as_deref(),
                    ) {
                        draw.push(slot.surface);
                    }
                }
                draw
            }
        };
        Self::redraw_surfaces(ctl, &draw);
    }

    fn redraw_surfaces(ctl: &mut RuntimeCtl<'_>, surfaces: &[SurfaceId]) {
        for surface in surfaces {
            if let Some(mut surface) = ctl.surface(*surface) {
                surface.request_redraw();
            }
        }
    }

    /// Records who holds the bus names the pages depend on; the Islands
    /// pick it up on their next frame. Detecting the owners on the bus is
    /// the job of the notification server and the tray host (CEL-1-F,
    /// CEL-1-G). Returns whether anything changed, so the caller knows to
    /// ask for a redraw.
    pub fn set_owners(&mut self, owners: NameOwner) -> bool {
        self.owners.replace(owners)
    }

    fn theme(&self) -> Theme {
        Theme::dark().scaled(self.appearance.text_scale.factor())
    }

    /// The notification state changed: every Island draws it again. The
    /// banners and the Notifications page extend this.
    fn notifications_changed(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.mark_read_if_viewed();
        // Sync first: the output whose capsule the notices surface holds is
        // redrawn by the lend and the return, which paint the latest state.
        self.sync_banners(ctl);
        // Only while a lend or a return is on its way: the steady lent panel
        // still draws its bar, bell and unread mark included.
        let covered = self
            .banners
            .as_ref()
            .filter(|b| b.handover.answer_pending())
            .map(|b| b.output);
        for surface in self.islands.redraw_targets(covered) {
            if let Some(mut s) = ctl.surface(surface) {
                s.request_redraw();
            }
        }
    }

    /// The notices follow the snapshot's toasts: the surface is created on
    /// the focused output when the first one shows, moves with the focus,
    /// gets the changes and stays until the Island is back at rest (the
    /// release comes from [`App::notice_release`]).
    fn sync_banners(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let now = std::time::Instant::now();
        let toasts = self.notifications.snapshot().toasts.clone();
        let showing = !banners::shown(&toasts).is_empty();
        let wanted = self
            .islands
            .banner_output(focused_output(self.model.as_ref()));
        // A page that opened on the surface's output is the Island's now:
        // the surface yields at once, without its shrink, and a level it
        // showed is not shown again at the bottom.
        if self
            .banners
            .as_ref()
            .is_some_and(|b| self.islands.page_open_on(b.output))
        {
            self.banners_yield(ctl);
            self.island_level = None;
        }
        if showing
            && self
                .banners
                .as_ref()
                .is_some_and(|b| Some(b.output) != wanted)
        {
            self.banners_follow_focus(ctl);
        }
        // The page of the banner output is open: the newest toast is a card
        // at its top and there is no surface for it.
        let route = self.islands.route_notice(wanted, toasts.first(), now);
        for surface in route.redraw {
            if let Some(mut s) = ctl.surface(surface) {
                s.request_redraw();
            }
        }
        self.send_card_holds();
        if route.carded {
            return;
        }
        // The page is still shrinking: the toast waits for the Island to be
        // back at the capsule, or the surface would grow over the page.
        if self.banners.is_none() && wanted.is_some_and(|o| self.islands.page_out_on(o, now)) {
            return;
        }
        if self.banners.is_none() {
            if !showing {
                return;
            }
            self.banners = self.create_banners(ctl);
            // The smoke reads this line from stderr.
            tracing::info!(
                created = self.banners.is_some(),
                toasts = toasts.len(),
                "banners surface requested"
            );
        }
        let Some(banners) = self.banners.as_mut() else {
            return;
        };
        if let Some(island) = self.islands.island_of(banners.output) {
            banners.notice.set_rest(island.rest_rect(), now);
        }
        let changed = banners.notice.set_toasts(&toasts, now);
        let hold = banners.notice.take_hold();
        let surface = banners.surface;
        if let Some(command) = hold {
            self.notifications.send(command);
        }
        if banners.notice.needs_frame(changed, now) {
            if let Some(mut s) = ctl.surface(surface) {
                s.request_redraw();
            }
        }
    }

    /// The notices surface on the output niri has focused (else the
    /// first): the Island's rest rect there is what it grows from.
    fn create_banners(&self, ctl: &mut RuntimeCtl<'_>) -> Option<BannerSurface> {
        let output = self
            .islands
            .banner_output(focused_output(self.model.as_ref()))?;
        self.create_banners_on(output, ctl)
    }

    /// The notices surface on `output`, which starts with the level the
    /// display last ordered.
    fn create_banners_on(
        &self,
        output: OutputId,
        ctl: &mut RuntimeCtl<'_>,
    ) -> Option<BannerSurface> {
        let rest = self.islands.island_of(output)?.rest_rect();
        match ctl.create_layer_surface(banners::layer_spec(output)) {
            Ok(surface) => Some(BannerSurface {
                output,
                surface,
                notice: {
                    let mut notice =
                        NoticePainter::new(&self.theme(), rest, self.appearance.reduced_motion);
                    notice.set_level(self.island_level, std::time::Instant::now());
                    notice
                },
                handover: Handover::new(),
                clock: ClockTick::default(),
            }),
            Err(err) => {
                tracing::error!(output = output.0, %err, "no banners on this output");
                None
            }
        }
    }

    /// Where the notices and the pages are, for the display's choice of
    /// output and of position.
    fn osd_placement(&self) -> Placement {
        self.islands.placement(
            self.banners.as_ref().map(|b| b.output),
            std::time::Instant::now(),
        )
    }

    /// Carries out what the display asked of the Island's level: the level
    /// goes to the notices surface (made, for a level alone, on the output
    /// the display chose) and the server's clock is held or released with
    /// it.
    fn level_orders(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let now = std::time::Instant::now();
        for order in self.osds.take_orders() {
            match order {
                Order::Show(output, level) => {
                    // The page of this output is still out: the Island is
                    // its, and a surface now would grow over it. The display
                    // places a level away from a page that is out, so this
                    // only guards a page that opened since.
                    if self.banners.is_none() && self.islands.page_out_on(output, now) {
                        self.island_level = None;
                        continue;
                    }
                    self.island_level = Some(level);
                    if self.banners.is_none() {
                        self.banners = self.create_banners_on(output, ctl);
                        // The smoke's `notify` phase greps the toasts' line;
                        // a level alone has its own.
                        tracing::info!(
                            created = self.banners.is_some(),
                            "notices surface requested for a level"
                        );
                    }
                }
                Order::Clear => self.island_level = None,
            }
            let level = self.island_level;
            let Some(banners) = self.banners.as_mut() else {
                continue;
            };
            if let Some(island) = self.islands.island_of(banners.output) {
                banners.notice.set_rest(island.rest_rect(), now);
            }
            banners.notice.set_level(level, now);
            let hold = banners.notice.take_hold();
            let surface = banners.surface;
            if let Some(command) = hold {
                self.notifications.send(command);
            }
            if let Some(mut s) = ctl.surface(surface) {
                s.request_redraw();
            }
        }
    }

    /// The notices' surface is gone: a hold it kept ends with it, so the
    /// server's expiry runs again.
    fn banners_gone(&mut self, mut gone: BannerSurface) {
        if let Some(command) = gone.notice.release_hover() {
            self.notifications.send(command);
        }
    }

    /// Redraws the panel of `output`.
    fn redraw_output(&self, output: OutputId, ctl: &mut RuntimeCtl<'_>) {
        if let Some(surface) = self.islands.surface_of(output) {
            if let Some(mut s) = ctl.surface(surface) {
                s.request_redraw();
            }
        }
    }

    /// The focus moved to another output while a notice shows: the old
    /// output's panel takes its capsule back, its surface goes on the next
    /// turn, and the new output gets its own (its Island lends the capsule
    /// after the new surface's first frame).
    /// A surface that showed only a level goes the same way, without a
    /// shrink (a known limit: the focus must move within the level's second
    /// and a half, and a notification arrive in it).
    fn banners_follow_focus(&mut self, ctl: &mut RuntimeCtl<'_>) {
        if let Some(output) = self.banners_yield(ctl) {
            tracing::info!(output = output.0, "banners follow the focus");
        }
    }

    /// The notices surface goes without a shrink: its output's panel takes
    /// the capsule back at once (when it was lent), the surface is destroyed
    /// on the next turn and the hold it kept ends. The output it was on.
    fn banners_yield(&mut self, ctl: &mut RuntimeCtl<'_>) -> Option<OutputId> {
        let mut old = self.banners.take()?;
        if self.islands.retire(&mut old.handover, old.output) {
            self.redraw_output(old.output, ctl);
        }
        self.banners_retiring.push(old.surface);
        self.post(Message::NoticeRelease(old.surface));
        if let Some(command) = old.notice.release_hover() {
            self.notifications.send(command);
        }
        Some(old.output)
    }

    /// Tells the server's clock what the cards of the open pages want.
    fn send_card_holds(&mut self) {
        for hold in self.islands.take_card_holds() {
            self.notifications.send(NotificationsCommand::Hover(hold));
        }
    }

    /// A page opened or closed somewhere: the notices follow (a surface
    /// yields to a page that opened, a card goes back to the surface when
    /// its page closes).
    fn pages_changed(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.pages_seen = self.islands.pages_out(std::time::Instant::now());
        self.sync_banners(ctl);
    }

    /// Posts [`Message::PagesChanged`] when the set of open pages is not the
    /// one the notices last saw. Cheap: called after input and frames.
    fn watch_pages(&mut self) {
        let out = self.islands.pages_out(std::time::Instant::now());
        if out != self.pages_seen {
            self.pages_seen = out;
            self.post(Message::PagesChanged);
        }
    }

    /// The first frame of the notices surface is out: its output's panel
    /// lends the capsule, unless nothing is left to show.
    fn notice_lend(&mut self, surface: SurfaceId, ctl: &mut RuntimeCtl<'_>) {
        let now = std::time::Instant::now();
        let Some(banners) = self.banners.as_mut().filter(|b| b.surface == surface) else {
            return;
        };
        let wants = banners.notice.wants_surface(now);
        let output = banners.output;
        let answer = banners.handover.lend(wants);
        self.notice_answer(answer, surface, output, ctl);
    }

    /// The Island is back at rest: the panel takes the capsule back and
    /// redraws it first; the surface goes on the next turn.
    fn notice_rest(&mut self, surface: SurfaceId, ctl: &mut RuntimeCtl<'_>) {
        let now = std::time::Instant::now();
        let Some(banners) = self.banners.as_mut().filter(|b| b.surface == surface) else {
            return;
        };
        let wants = banners.notice.wants_surface(now);
        let output = banners.output;
        let answer = banners.handover.rest(wants);
        self.notice_answer(answer, surface, output, ctl);
    }

    /// Does what the hand-over answered: lend the capsule, or take it back,
    /// redraw the panel and post the release for the next turn.
    fn notice_answer(
        &mut self,
        answer: Answer,
        surface: SurfaceId,
        output: OutputId,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        let (redraw, release) = self.islands.answer(answer, output);
        if redraw {
            self.redraw_output(output, ctl);
        }
        if release {
            self.post(Message::NoticeRelease(surface));
        }
    }

    /// The turn after the capsule came back: the surface goes unless a
    /// notice arrived meanwhile, in which case it is reused.
    fn notice_release(&mut self, surface: SurfaceId, ctl: &mut RuntimeCtl<'_>) {
        if let Some(at) = self.banners_retiring.iter().position(|s| *s == surface) {
            self.banners_retiring.remove(at);
            ctl.destroy_surface(surface);
            return;
        }
        let now = std::time::Instant::now();
        let goes = self.banners.as_ref().is_some_and(|b| {
            b.surface == surface && b.handover.release(b.notice.wants_surface(now))
        });
        if !goes {
            return;
        }
        if let Some(gone) = self.banners.take() {
            ctl.destroy_surface(gone.surface);
            self.banners_gone(gone);
            tracing::info!("banners surface released: no toast left");
        }
    }

    /// What a gesture on the banners asks for.
    fn banners_act(&mut self, actions: Vec<BannerAction>, output: OutputId) {
        for action in actions {
            match action {
                BannerAction::Send(command) => {
                    self.notifications.send(command);
                }
                BannerAction::OpenPage => self.post(Message::OpenNotificationsPage(output)),
                BannerAction::OpenClock => self.post(Message::OpenClockPage(output)),
            }
        }
    }

    /// One frame of the notices surface: the Island growing into the
    /// notice, painted over the panel's rest rect. When it is out, the
    /// hand-over of the capsule is posted (the panel lends it on the next
    /// loop turn, not in this frame).
    fn notice_frame(&mut self, cx: &mut FrameCx<'_>) {
        let (Some(shared), Some(banners)) = (self.painter.as_ref(), self.banners.as_mut()) else {
            return;
        };
        let Some(island) = self.islands.island_of(banners.output) else {
            return;
        };
        let Ok(mut painter) = shared.try_borrow_mut() else {
            tracing::warn!("the painter is in use; the banners' frame is skipped");
            cx.ctl.request_redraw();
            return;
        };
        let wants =
            banners
                .notice
                .frame(cx, &mut painter, island.rest_rect(), island.capsule_view());
        drop(painter);
        let post = banners.handover.frame(wants).map(|post| match post {
            Post::Lend => Message::NoticeLend(banners.surface),
            Post::Rest => Message::NoticeRest(banners.surface),
        });
        // The clock face rolls over at the minute while it holds the capsule.
        let (secs, nanos) = clock::unix_now();
        if let Some(wait) = banners::clock_wait(
            banners.handover.holds(),
            &mut banners.clock,
            cx.now,
            secs,
            nanos,
        ) {
            cx.ctl.redraw_after(wait);
        }
        if let Some(message) = post {
            self.post(message);
        }
    }

    /// Opens `page` on `output`'s Island.
    fn open_page_on(&mut self, output: OutputId, page: PageId, ctl: &mut RuntimeCtl<'_>) {
        if self.polkit_claims_keyboard() {
            tracing::info!("island: page not opened while a polkit prompt holds the keyboard");
            return;
        }
        let Some(surface) = self.islands.surface_of(output) else {
            tracing::debug!(output = output.0, "the output is gone: no page to open");
            return;
        };
        let claimed = self.polkit_claims_keyboard();
        if let (Some(island), Some(mut ctl)) = (self.islands.get_mut(surface), ctl.surface(surface))
        {
            // The Island hears the claim before the verb: its own flag is
            // only refreshed on frame and input.
            island.set_keyboard_claimed(claimed);
            island.command(&Verb::IslandOpen(page), &mut ctl);
        }
    }

    fn redraw_all(&self, ctl: &mut RuntimeCtl<'_>) {
        for slot in self.islands.iter() {
            if let Some(mut surface) = ctl.surface(slot.surface) {
                surface.request_redraw();
            }
        }
    }

    fn output_added(&mut self, info: &celestina_wayland::OutputInfo, ctl: &mut RuntimeCtl<'_>) {
        let (anchor, size, bar) = island_shape(self.settings.current(), info.logical_size.0);
        let mut island = Island::new(&info.name, (size.0 as f32, size.1 as f32));
        island.set_bar(&bar);
        // The shape asked for, logged at creation.
        tracing::info!(output = %info.name, ?anchor, ?size, "island surface requested");
        let spec = island_layer(info.id, anchor, size, island.exclusive_zone());
        match ctl.create_layer_surface(spec) {
            Ok(surface) => self.islands.add(info.id, surface, island),
            Err(err) => tracing::error!(output = %info.name, %err, "no Island on this output"),
        }
        // The new Island draws the media state in force.
        let draw = self.media_sync();
        Self::redraw_surfaces(ctl, &draw);
        // A toast that arrived before any Island existed has no banner yet.
        self.sync_banners(ctl);
    }

    /// The bar's layout changed: every Island lays it out again on its next
    /// frame, on the surface it has; an open page stays as it is.
    fn relayout_bar(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.islands.set_bar(&self.settings.current().bar_layout());
        self.redraw_all(ctl);
    }

    fn command(&mut self, verb: Verb, ctl: &mut RuntimeCtl<'_>) {
        match verb {
            Verb::Quit => {
                tracing::info!("quit requested over the bus");
                ctl.stop();
            }
            Verb::IslandToggle | Verb::IslandOpen(_) if self.polkit_claims_keyboard() => {
                tracing::info!(
                    ?verb,
                    "island: not opened while a polkit prompt holds the keyboard"
                );
            }
            Verb::IslandToggle | Verb::IslandOpen(_) | Verb::IslandClose => {
                let target = self
                    .islands
                    .target(self.keyboard, focused_output(self.model.as_ref()));
                let Some(surface) = target else {
                    tracing::info!(?verb, "no output: the verb has no Island");
                    return;
                };
                let claimed = self.polkit_claims_keyboard();
                if let (Some(island), Some(mut ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
                    // The Island hears the claim before the verb.
                    island.set_keyboard_claimed(claimed);
                    island.command(&verb, &mut ctl);
                }
            }
            Verb::Caffeine(_) | Verb::NightLight(_) | Verb::Session(_) => {
                let Some(handle) = self.energy.get() else {
                    tracing::warn!(?verb, "no energy handle: the verb does nothing");
                    return;
                };
                if let Some(outcome) = energy::verbs::run(verb, &mut self.settings, handle) {
                    outcome.log();
                }
            }
            Verb::Lock | Verb::LockAndSuspend => match self.lock.get() {
                Some(handle) => {
                    handle.verb(&verb);
                }
                None => tracing::warn!(?verb, "no lock handle: the verb does nothing"),
            },
            Verb::Clipboard(_) => self.clipboard_verb(&verb),
            Verb::Volume(_) | Verb::Microphone(_) | Verb::Brightness { .. } => {
                let focused = focused_output(self.model.as_ref());
                let outcome = match self.osds.runtime().get() {
                    Some(runtime) => {
                        level_verbs::run(&verb, runtime, &self.controls.readings, focused)
                    }
                    None => level_verbs::run(&verb, &NoWorkers, &self.controls.readings, focused),
                };
                if let Some(outcome) = outcome {
                    outcome.log();
                }
            }
            Verb::BubblesSelect => self.select_bubbles(ctl),
            Verb::BubblesMinimize | Verb::PhoneMedia(_) => {
                self.bubbles_and_phone_verb(&verb);
            }
            Verb::LauncherToggle => {
                self.close_overview(ctl);
                self.toggle_launcher(ctl);
            }
            Verb::Overview(presence) => self.overview_verb(OverviewMode::Desktop, presence, ctl),
            Verb::Shortcuts(presence) => self.overview_verb(OverviewMode::Shortcuts, presence, ctl),
            Verb::WallpaperSet { .. }
            | Verb::WallpaperNext(_)
            | Verb::WallpaperPrev(_)
            | Verb::WallpaperMode(_)
            | Verb::WallpaperRetry => {
                self.wallpapers
                    .command(&verb, &mut self.settings, ctl, self.painter.as_ref());
            }
            Verb::NotificationsDismissAll => {
                self.notifications.send(NotificationsCommand::DismissAll);
            }
            Verb::NotificationsClear => {
                self.notifications.send(NotificationsCommand::Clear);
            }
            Verb::NotificationsToggle if self.polkit_claims_keyboard() => {
                tracing::info!(
                    "notifications: not toggled while a polkit prompt holds the keyboard"
                );
            }
            Verb::NotificationsToggle => {
                let target = self
                    .islands
                    .target(self.keyboard, focused_output(self.model.as_ref()));
                let Some(surface) = target else {
                    tracing::info!(?verb, "no output: the verb has no Island");
                    return;
                };
                let claimed = self.polkit_claims_keyboard();
                if let (Some(island), Some(mut ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
                    // The Island hears the claim before the verb.
                    island.set_keyboard_claimed(claimed);
                    let toggle = island.notifications_toggle_verb();
                    island.command(&toggle, &mut ctl);
                }
            }
        }
    }

    /// Closes the open launcher (or the open selector: one overlay at a
    /// time), or opens one on the output with the keyboard: the Island
    /// holding it, else niri's focused output, else the first. The surface
    /// is created now and destroyed on close.
    fn toggle_launcher(&mut self, ctl: &mut RuntimeCtl<'_>) {
        if let Some(open) = self.launcher.take() {
            tracing::info!("launcher: closed by the command");
            close_surface(open.surface(), ctl);
            return;
        }
        if self.polkit_claims_keyboard() {
            tracing::info!("launcher: not opened while a polkit prompt holds the keyboard");
            return;
        }
        let Some((surface, output)) = self.overlay_surface(ctl) else {
            return;
        };
        let open = Launcher::new(
            surface,
            output,
            &self.theme(),
            self.appearance.reduced_motion,
            self.usage.scores(launcher::usage::unix_now()),
            Locale::from_env(),
            self.clipboard.get().is_some_and(ClipboardDuty::running),
            self.settings.current().launcher.view,
            &self.index,
        );
        if let Some(mut s) = ctl.surface(surface) {
            open.opened(&mut s);
        }
        tracing::info!(
            output = output.0,
            entries = self.index.len(),
            "launcher: opened"
        );
        self.launcher = Some(open);
    }

    /// Starts the application with desktop id `id` the way the launcher does
    /// (never recorded as a launch). An id the session's applications lack is
    /// logged and nothing starts.
    fn open_app(&self, id: &str) {
        let Some(entry) = self.index.iter().find(|entry| entry.id == id) else {
            tracing::warn!(%id, "an Island row names an application the session lacks");
            return;
        };
        match launcher::spawn::spawn(entry) {
            Ok(()) => tracing::info!(%id, "island: application started"),
            Err(err) => tracing::warn!("island: {}", error_chain(&err)),
        }
    }

    /// `bubbles select` (`Mod+K`, and the overflow bubble): closes the open
    /// selector (a toggle), or opens one where the launcher opens. The
    /// launcher of applications, if open, is replaced by it: one overlay at
    /// a time. With the bubbles duty off nothing happens but a journal line.
    fn select_bubbles(&mut self, ctl: &mut RuntimeCtl<'_>) {
        if let Some(open) = self.launcher.take_if(|l| l.is_selector()) {
            tracing::info!("bubbles select: closed by the command");
            close_surface(open.surface(), ctl);
            return;
        }
        if self.settings.current().duties.bubbles != Duty::On {
            tracing::info!("bubbles select: the bubbles duty is off; nothing was done");
            return;
        }
        if self.polkit_claims_keyboard() {
            tracing::info!("bubbles select: not opened while a polkit prompt holds the keyboard");
            return;
        }
        // One overlay at a time: an open overview goes, as it does for the
        // launcher's toggle.
        self.close_overview(ctl);
        if let Some(open) = self.launcher.take() {
            tracing::info!("launcher: replaced by the bubbles selector");
            close_surface(open.surface(), ctl);
        }
        let Some((surface, output)) = self.overlay_surface(ctl) else {
            return;
        };
        let open = Launcher::selector(
            surface,
            output,
            &self.theme(),
            self.appearance.reduced_motion,
            &self.bubbles_state,
        );
        if let Some(mut s) = ctl.surface(surface) {
            open.opened(&mut s);
        }
        tracing::info!(
            output = output.0,
            windows = self.bubbles_state.windows.len(),
            link = ?self.bubbles_state.link,
            "bubbles select: opened"
        );
        self.launcher = Some(open);
    }

    /// A new overlay surface for the launcher or the selector, on the
    /// output with the keyboard: the Island holding it, else niri's focused
    /// output, else the first.
    fn overlay_surface(&self, ctl: &mut RuntimeCtl<'_>) -> Option<(SurfaceId, OutputId)> {
        let output = self
            .islands
            .target(self.keyboard, focused_output(self.model.as_ref()))
            .and_then(|surface| self.islands.output_of(surface));
        let Some(output) = output else {
            tracing::info!("launcher: no output to open on");
            return None;
        };
        // The whole output, the Island's band included: -1 ignores every
        // exclusive zone and pushes no window; the card is centred inside.
        let spec = LayerSpec {
            output,
            namespace: launcher::NAMESPACE,
            layer: Layer::Overlay,
            anchor: Anchoring::Fill,
            size: (0, 0),
            exclusive_zone: -1,
            keyboard: KeyboardInteractivity::Exclusive,
        };
        match ctl.create_layer_surface(spec) {
            Ok(surface) => Some((surface, output)),
            Err(err) => {
                tracing::error!(%err, "launcher: no surface");
                None
            }
        }
    }

    /// The launcher asked for `action` during an input call: it is closed
    /// now (its text input off, the keyboard released) and its surface is
    /// destroyed when the posted [`Message::LauncherClosed`] arrives, which
    /// also runs a chosen switch.
    fn launcher_acted(&mut self, action: LauncherAction, ctl: &mut SurfaceCtl<'_>) {
        let Some(open) = self.launcher.take() else {
            return;
        };
        Launcher::closing(ctl);
        ctl.set_keyboard_interactivity(KeyboardInteractivity::None);
        let switch = match action {
            LauncherAction::Launch(entry) => {
                match launcher::spawn::spawn(&entry) {
                    Ok(()) => {
                        tracing::info!(id = %entry.id, "launcher: launched");
                        self.usage.record(&entry.id, launcher::usage::unix_now());
                    }
                    Err(err) => tracing::warn!("launcher: {}", error_chain(&err)),
                }
                None
            }
            LauncherAction::Switch(switch) => Some(switch),
            // No bubble to travel from: no anchor (D3).
            LauncherAction::Restore(window_id) => {
                self.restore_window(window_id, "", None);
                None
            }
            LauncherAction::RequestClose(window_id) => {
                // Not a closing action; `launcher_input` keeps it out.
                self.close_window(window_id);
                None
            }
            // Not a closing action; `launcher_input` keeps it out.
            LauncherAction::Copy(_) | LauncherAction::SetView(_) | LauncherAction::Close => None,
        };
        let closed = Message::LauncherClosed {
            surface: open.surface(),
            output: open.output(),
            switch,
        };
        match self.sender.get().map(|sender| sender.send(closed)) {
            Some(Ok(())) => {}
            _ => {
                tracing::warn!("launcher: the loop cannot be reached; its surface stays until exit")
            }
        }
    }

    /// Runs `switch` chosen in the launcher on `output`: opens the page on
    /// that output's Island, or turns do-not-disturb over as the
    /// Notifications page's switch does.
    fn run_switch(&mut self, switch: Switch, output: OutputId, ctl: &mut RuntimeCtl<'_>) {
        tracing::info!(?switch, "launcher: switch chosen");
        let Some(page) = switch.page() else {
            if self.owners.notifications.is_some() {
                tracing::info!("do-not-disturb is held by another program; not changed");
                return;
            }
            let on = !self.settings.current().do_not_disturb;
            if self.set_do_not_disturb(on) {
                self.redraw_all(ctl);
            }
            return;
        };
        if self.polkit_claims_keyboard() {
            tracing::info!("launcher: switch dropped while a polkit prompt holds the keyboard");
            return;
        }
        let Some(surface) = self.islands.surface_on(output).or_else(|| {
            self.islands
                .target(None, focused_output(self.model.as_ref()))
        }) else {
            return;
        };
        let claimed = self.polkit_claims_keyboard();
        if let (Some(island), Some(mut ctl)) = (self.islands.get_mut(surface), ctl.surface(surface))
        {
            // The Island hears the claim before the verb: its own flag is
            // only refreshed on frame and input.
            island.set_keyboard_claimed(claimed);
            island.command(&Verb::IslandOpen(page), &mut ctl);
        }
    }

    /// Sets do-not-disturb durably and tells the notification server, which
    /// holds the banners back while it is on (it also reads the setting once
    /// at its start); whether it is now `on` and changed. The one owner of
    /// the flag: the menu, the Notifications page and the launcher's switch
    /// all come here, and each redraws with the control it holds.
    fn set_do_not_disturb(&mut self, on: bool) -> bool {
        match self.settings.set_do_not_disturb(on) {
            Ok(changed) => {
                tracing::info!(on, changed, "do-not-disturb set");
                self.notifications.send(NotificationsCommand::SetQuiet(on));
                changed
            }
            Err(err) => {
                tracing::warn!(on, "do-not-disturb not changed: {}", error_chain(&err));
                false
            }
        }
    }

    /// The Island's actions after its input: compositor requests, and the
    /// overflow menu built here (it needs the shared painter, the settings
    /// and the choice slot) and handed to the Island to open.
    fn act(
        &mut self,
        actions: Vec<IslandAction>,
        surface: SurfaceId,
        shared: &SharedPainter,
        ctl: &mut SurfaceCtl<'_>,
    ) {
        for action in actions {
            match action {
                IslandAction::FocusWorkspace(id) => self.request(NiriRequest::FocusWorkspace(id)),
                IslandAction::FocusWindow(id) => self.request(NiriRequest::FocusWindow(id)),
                IslandAction::Notification(command) => {
                    self.notifications.send(command);
                }
                IslandAction::ClearNotifications => {
                    self.notifications.send(NotificationsCommand::Clear);
                }
                IslandAction::ToggleDoNotDisturb => {
                    // The page disables its switch while another program
                    // holds the flag; this keeps the rule if one slips by.
                    if self.owners.notifications.is_some() {
                        tracing::info!("do-not-disturb is held by another program; not changed");
                        continue;
                    }
                    let on = !self.settings.current().do_not_disturb;
                    if self.set_do_not_disturb(on) {
                        // The open page shows the switch: draw it again.
                        // Other outputs' Islands pick it up on their next
                        // frame.
                        ctl.request_redraw();
                    }
                }
                IslandAction::Clipboard(verb) => self.clipboard_verb(&Verb::Clipboard(verb)),
                IslandAction::PhoneMedia(kind) => self.phone_media(kind),
                IslandAction::RestoreWindow {
                    window_id,
                    output,
                    bubble,
                } => self.restore_window(window_id, &output, bubble),
                IslandAction::OpenSelector => {
                    // Opening a surface needs the runtime: the loop hears
                    // the verb `Mod+K` sends.
                    let verb = Message::Command(Verb::BubblesSelect);
                    if !self.sender.get().is_some_and(|s| s.send(verb).is_ok()) {
                        tracing::warn!("bubbles select: the loop cannot be reached; no selector");
                    }
                }
                IslandAction::Media(command) => match self.media.get() {
                    Some(media) => media.command(command),
                    None => tracing::debug!(?command, "no media control: the command is dropped"),
                },
                IslandAction::Tray(command) => self.tray_command(command),
                IslandAction::OpenApp(id) => self.open_app(id),
                IslandAction::Connectivity(action) => {
                    if self.connectivity_request(action) {
                        // The Island that asked shows the pending row now;
                        // the others pick it up with the worker's news.
                        ctl.request_redraw();
                    }
                }
                IslandAction::OpenTrayMenu {
                    item,
                    row,
                    keyboard,
                    at,
                } => {
                    let request = PendingMenu {
                        surface,
                        item: item.clone(),
                        row,
                        keyboard,
                        until: None,
                    };
                    // A request for another item replaces the old one: its
                    // item must not stay followed with an `opened` and no
                    // `closed`.
                    if let Some(old) = Pending::replace(&mut self.pending_menu, request) {
                        self.give_up_menu(old);
                    }
                    self.tray_command(TrayCommand::OpenMenu(item, at.0, at.1));
                }
                IslandAction::Audio(_) | IslandAction::Brightness(_) => {
                    self.level_action(action, surface);
                }
                IslandAction::OpenLevelSheet { kind, piece } => {
                    self.open_level(kind, piece, surface, shared, ctl);
                }
                // No popup grab is held, so the verb goes out at once; the
                // lock maps over the closing sheet.
                IslandAction::Session(action) => self.post(Message::Command(action.verb())),
                IslandAction::OpenMenu { more, keyboard } => {
                    let Ok(painter) = shared.try_borrow() else {
                        tracing::warn!("the painter is in use; the menu is not opened");
                        continue;
                    };
                    let theme = painter.theme().clone();
                    drop(painter);
                    let menu = Menu::new(
                        Rc::clone(shared),
                        &theme,
                        menu_state(&self.settings, &self.owners),
                        self.appearance.reduced_motion,
                        keyboard,
                        fresh_choice(&self.menu_choice),
                    );
                    if let Some(island) = self.islands.get_mut(surface) {
                        island.open_menu(more, menu::menu_size(&theme), Box::new(menu), ctl);
                    }
                }
            }
        }
    }

    /// A connectivity page's row or piece asked for `action`: checked by the
    /// app's state, queued on its worker only if the state accepts it, or
    /// the editing UI started. Every Island gets the new state; returns
    /// whether it changed (the row shows pending, a refusal, a failure or the
    /// editor's note).
    fn connectivity_request(&mut self, action: celestina_shell::connectivity::Action) -> bool {
        let forwarded = match self.connectivity_handle.try_borrow() {
            Ok(handle) => connectivity::forward(&mut self.connectivity, handle.as_ref(), action),
            Err(_) => connectivity::forward(&mut self.connectivity, None, action),
        };
        if let Forwarded::Editor(editor) = forwarded {
            connectivity::editor::launch(&mut self.connectivity, editor);
        }
        self.share_connectivity().is_some()
    }

    /// Gives every Island the app's connectivity state; the surfaces whose
    /// Island shows it, or `None` when nothing changed for any.
    fn share_connectivity(&mut self) -> Option<Vec<SurfaceId>> {
        let shown: Vec<SurfaceId> = self
            .islands
            .slots
            .iter_mut()
            .filter_map(|slot| {
                slot.island
                    .set_connectivity(&self.connectivity)
                    .then_some(slot.surface)
            })
            .collect();
        (!shown.is_empty()).then_some(shown)
    }

    /// A connectivity worker's news: applied to the state, and only a change
    /// reaches the Islands that show it, so unchanged news costs no frame.
    fn connectivity_event(
        &mut self,
        event: &connectivity::ConnectivityEvent,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        if !self.connectivity.apply(event) {
            return;
        }
        for surface in self.share_connectivity().unwrap_or_default() {
            if let Some(mut surface) = ctl.surface(surface) {
                surface.request_redraw();
            }
        }
    }

    /// Makes the overflow menu's choice, once its popup is gone.
    fn menu_chose(&mut self, choice: MenuChoice, surface: SurfaceId, ctl: &mut SurfaceCtl<'_>) {
        match choice {
            MenuChoice::Quit => {
                tracing::info!("quit chosen in the menu");
                ctl.stop();
            }
            // The setting changes and the notification server hears it.
            MenuChoice::DoNotDisturb(on) => {
                if self.set_do_not_disturb(on) {
                    // The open page shows the switch: draw it again. Other
                    // outputs' Islands pick it up on their next frame.
                    ctl.request_redraw();
                }
            }
            MenuChoice::ReopenPage => {
                if let Some(island) = self.islands.get_mut(surface) {
                    island.reopen(ctl);
                }
            }
        }
    }

    /// Posts a [`Message::SyncCatchers`] when an Island shows (or stopped
    /// showing) something its catcher does not match. Every change of what is
    /// shown comes with a frame or an input call on its surface, which is
    /// where this runs.
    fn catchers_due(&self) {
        let now = std::time::Instant::now();
        let due = self
            .islands
            .iter()
            .any(|slot| slot.island.wants_catcher(now) != self.catchers.has(slot.surface));
        if due {
            self.post(Message::SyncCatchers);
        }
    }

    /// Maps a catcher under every Island that shows something and destroys
    /// the ones whose Island shows nothing.
    fn sync_catchers(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let now = std::time::Instant::now();
        for slot in self.islands.iter() {
            let wants = slot.island.wants_catcher(now);
            match (wants, self.catchers.has(slot.surface)) {
                (true, false) => match ctl.create_layer_surface(catcher::spec(slot.output)) {
                    Ok(surface) => self.catchers.add(slot.surface, slot.output, surface),
                    Err(err) => {
                        tracing::error!(output = slot.output.0, %err, "no catcher on this output");
                    }
                },
                (false, true) => {
                    if let Some(surface) = self.catchers.take(slot.surface) {
                        ctl.destroy_surface(surface);
                    }
                }
                _ => {}
            }
        }
    }

    /// A press on the catcher `pressed`: its Island closes whatever it shows
    /// (the page, the sheet, what waits), as a press outside the sheet does.
    fn catcher_pressed(&mut self, pressed: SurfaceId, ctl: &mut RuntimeCtl<'_>) {
        let Some(island) = self.catchers.island_of(pressed) else {
            return;
        };
        let (Some(mut surface), Some(slot)) = (
            ctl.surface(island),
            self.islands.slots.iter_mut().find(|s| s.surface == island),
        ) else {
            return;
        };
        slot.island.command(&Verb::IslandClose, &mut surface);
    }

    /// What is on the Notifications page counts as seen: while an Island
    /// shows it, unread notifications are marked read.
    fn mark_read_if_viewed(&mut self) {
        if self.notifications.snapshot().unread > 0
            && self.islands.iter().any(|s| s.island.shows_notifications())
        {
            // Seen here and now: the next frame draws it read, before the
            // worker's own snapshot says so.
            self.notifications.mark_viewed();
        }
    }

    /// Queues `command` for the tray host; a refusal (queue full, host
    /// stopped because another program holds the watcher) is logged once.
    fn tray_command(&mut self, command: TrayCommand) {
        let Some(tray) = self.tray.get() else {
            tracing::debug!(?command, "no tray host: the command is dropped");
            return;
        };
        if let Err(refused) = tray.command(command) {
            if !self.tray_refusal_logged {
                self.tray_refusal_logged = true;
                tracing::warn!(
                    ?refused,
                    "the tray host refused a command (its queue is full or it stopped); later refusals are not logged"
                );
            }
        }
    }

    /// The tray host's news: the owner of the watcher name, the items, or a
    /// menu the last right click asked for.
    fn tray_event(&mut self, event: TrayEvent, ctl: &mut RuntimeCtl<'_>) {
        match event {
            TrayEvent::Owner(tray) => {
                let owners = NameOwner {
                    notifications: self.owners.notifications.clone(),
                    tray,
                };
                if self.set_owners(owners) {
                    self.redraw_all(ctl);
                }
            }
            TrayEvent::Items(items) => {
                if self.tray_items.replace(items) {
                    self.redraw_all(ctl);
                }
            }
            TrayEvent::Menu(item, menu) => self.open_tray_menu(&item, menu, ctl),
            TrayEvent::NoMenu(item) => {
                // The host already asked the app for its own menu; the shell
                // adds its card with the one entry (an app that shows its
                // menu takes the grab and the card closes).
                if let Some(pending) = self.pending_menu.take_if(|p| p.item == item) {
                    self.open_tray_card(pending, MenuTree::default(), ctl);
                }
            }
        }
    }

    /// A menu of `item` from the host: the open chain of that item shows
    /// it (a republished menu, or the submenu an `AboutToShow` filled);
    /// else it hangs below the row that asked for it, if one did and its
    /// Island still shows the Tray page.
    fn open_tray_menu(&mut self, item: &ItemId, menu: MenuTree, ctl: &mut RuntimeCtl<'_>) {
        if let Some(open) = self.open_tray_menu.as_ref().filter(|o| o.item == *item) {
            open.cx.shared.borrow_mut().replace(menu);
            let popups: Vec<PopupId> = std::iter::once(open.root)
                .chain(open.cx.chain.borrow().iter().copied())
                .collect();
            for popup in popups {
                if let Some(mut surface) = ctl.popup_surface(popup).and_then(|s| ctl.surface(s)) {
                    surface.request_redraw();
                }
            }
            return;
        }
        let pending = match Pending::of(
            &mut self.pending_menu,
            item,
            menu.entries.is_empty(),
            Instant::now(),
        ) {
            Pending::Unasked => {
                tracing::debug!("a menu nobody asked for is dropped");
                return;
            }
            Pending::Wait(left) => {
                tracing::debug!("the item's menu is empty; waiting for its layout");
                let surface = self.pending_menu.as_ref().map(|p| p.surface);
                if let Some(mut surface) = surface.and_then(|id| ctl.surface(id)) {
                    surface.redraw_after(left);
                }
                return;
            }
            Pending::GiveUp(gone) => {
                self.give_up_menu_or_card(gone, ctl);
                return;
            }
            Pending::Open(pending) => pending,
        };
        self.open_tray_card(pending, menu, ctl);
    }

    /// The shell's entry for `item`'s menu: hide it while it is in the bar,
    /// show it while it is hidden; `None` for an item without an `Id`, which
    /// `bar.tray_hidden` cannot list (and today's filter never hides).
    fn tray_shell_entry_for(&self, item: &ItemId) -> Option<ShellEntry> {
        let row = self.tray_items.items.iter().find(|i| i.id == *item)?;
        if row.sni_id.is_empty() {
            return None;
        }
        let layout = self.settings.current().bar_layout();
        if layout.tray_hidden.contains(&row.sni_id) {
            Some(ShellEntry::Show)
        } else {
            // Hide only an id the list can take, so the entry never does
            // nothing.
            layout.can_hide(&row.sni_id).then_some(ShellEntry::Hide)
        }
    }

    /// Hangs the card of `pending` with `menu` as the app's entries (none
    /// for an app without a menu, which then holds only the shell's entry),
    /// if its Island still shows what asked for it.
    fn open_tray_card(&mut self, pending: PendingMenu, menu: MenuTree, ctl: &mut RuntimeCtl<'_>) {
        let shell_entry = self.tray_shell_entry_for(&pending.item);
        if menu.entries.is_empty() && shell_entry.is_none() {
            tracing::debug!("the item has no menu and cannot be hidden; no card");
            return;
        }
        let (Some(shared), Some(mut surface)) =
            (self.painter.clone(), ctl.surface(pending.surface))
        else {
            return;
        };
        let Ok(painter) = shared.try_borrow() else {
            tracing::warn!("the painter is in use; the tray menu is not opened");
            return;
        };
        let theme = painter.theme().clone();
        drop(painter);
        *self.tray_choice.borrow_mut() = None;
        *self.tray_shell_choice.borrow_mut() = None;
        let ask: AskSink = {
            let tray = Rc::clone(&self.tray);
            Rc::new(move |command| {
                if let Some(refused) = tray.get().and_then(|t| t.command(command).err()) {
                    tracing::debug!(?refused, "the tray host refused a menu's request");
                }
            })
        };
        let cx = MenuContext {
            painter: shared,
            item: pending.item.clone(),
            shared: Rc::new(RefCell::new(MenuModel {
                tree: menu,
                generation: 1,
            })),
            choice: Rc::clone(&self.tray_choice),
            ask,
            chain: Chain::default(),
            reduced: self.appearance.reduced_motion,
            shell: shell_entry,
            shell_choice: Rc::clone(&self.tray_shell_choice),
        };
        let shell = TrayMenu::new(&cx, &theme, Vec::new(), pending.keyboard);
        let size = shell.size();
        let Some(island) = self.islands.get_mut(pending.surface) else {
            return;
        };
        if let Some(root) = island.open_tray_menu(pending.row, size, Box::new(shell), &mut surface)
        {
            self.open_tray_menu = Some(OpenTrayMenu {
                surface: pending.surface,
                item: pending.item,
                root,
                cx,
            });
        }
    }

    /// An empty menu that waited for its layout past [`MENU_LATE_LAYOUT`]
    /// is dropped, and the host stops following it.
    /// An app that can be hidden or shown still gets its card, with the
    /// shell's entry alone: the same card an app without a menu gets, posted
    /// as the host's `NoMenu` so the frame needs no popup context.
    fn expire_pending_menu(&mut self, now: Instant) {
        let Some(item) = Pending::due(&self.pending_menu, now) else {
            return;
        };
        if self.tray_shell_entry_for(&item).is_some() {
            self.tray_command(TrayCommand::CloseMenu(item.clone()));
            if let Some(waiting) = self.pending_menu.as_mut() {
                // Handled once: the message takes the request.
                waiting.until = None;
            }
            self.post(Message::Tray(TrayEvent::NoMenu(item)));
        } else if let Some(gone) = Pending::expired(&mut self.pending_menu, now) {
            self.give_up_menu(gone);
        }
    }

    /// [`App::give_up_menu`], except that an app that can be hidden or shown
    /// gets its card with the shell's entry alone.
    fn give_up_menu_or_card(&mut self, gone: PendingMenu, ctl: &mut RuntimeCtl<'_>) {
        if self.tray_shell_entry_for(&gone.item).is_some() {
            self.tray_command(TrayCommand::CloseMenu(gone.item.clone()));
            self.open_tray_card(gone, MenuTree::default(), ctl);
        } else {
            self.give_up_menu(gone);
        }
    }

    /// An item's menu stayed empty past [`MENU_LATE_LAYOUT`]: nothing opens
    /// and the item's menu is no longer followed.
    fn give_up_menu(&mut self, gone: PendingMenu) {
        tracing::debug!("the item's menu stayed empty; nothing opens");
        self.tray_command(TrayCommand::CloseMenu(gone.item));
    }

    /// The shell's entry chosen in `item`'s menu: writes `bar.tray_hidden`
    /// and lays every bar out again, so the icon leaves or enters its slot
    /// on every output.
    fn tray_shell_entry(&mut self, item: &ItemId, entry: ShellEntry, ctl: &mut RuntimeCtl<'_>) {
        let Some(sni_id) = self
            .tray_items
            .items
            .iter()
            .find(|i| i.id == *item)
            .map(|i| i.sni_id.clone())
        else {
            tracing::debug!("the item went before its shell entry was made");
            return;
        };
        match apply_shell_entry(&mut self.settings, &sni_id, entry) {
            Ok(true) => {
                tracing::info!(?entry, id = %sni_id, "tray_hidden written");
                self.relayout_bar(ctl);
            }
            Ok(false) => {
                tracing::info!(?entry, id = %sni_id, "tray_hidden: nothing to write");
            }
            Err(err) => tracing::warn!(?entry, "tray_hidden not changed: {}", error_chain(&err)),
        }
    }

    /// The open chain is gone (its root popup closed, or its Island's
    /// output went away): the host stops following the item's menu.
    fn tray_menu_gone(&mut self) {
        if let Some(open) = self.open_tray_menu.take() {
            self.tray_command(TrayCommand::CloseMenu(open.item));
        }
    }

    fn request(&mut self, request: NiriRequest) {
        let Some(niri) = self.niri.get() else {
            tracing::debug!(?request, "no niri provider: the request is dropped");
            return;
        };
        if let Err(Message::NiriRequestRefused(refused)) = niri.request(request) {
            self.refused(refused);
        }
    }

    /// A refused compositor request is logged once per run: the next ones
    /// fail for the same reason.
    fn refused(&mut self, request: NiriRequest) {
        if !self.refusal_logged {
            self.refusal_logged = true;
            tracing::warn!(
                ?request,
                "niri refused a request (its queue is full or its worker stopped); later refusals are not logged"
            );
        }
    }
}

impl App {
    /// Input on the launcher's surface.
    fn launcher_input(&mut self, ev: &InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let (Some(shared), Some(open)) = (self.painter.clone(), self.launcher.as_mut()) else {
            return;
        };
        let action = match shared.try_borrow_mut() {
            Ok(mut painter) => open.input(ev, ctl, &mut painter, &self.index),
            Err(_) => {
                tracing::warn!("the painter is in use; the launcher's input is dropped");
                return;
            }
        };
        match action {
            Some(LauncherAction::RequestClose(window_id)) => self.close_window(window_id),
            Some(LauncherAction::SetView(view)) => {
                // The open launcher already shows the view; a refused write
                // (a protected file) only means it is not kept.
                if let Err(err) = self.settings.set_launcher_view(view) {
                    tracing::warn!("launcher view not kept: {}", error_chain(&err));
                }
            }
            Some(LauncherAction::Copy(text)) => {
                if !self.copy_to_clipboard(text) {
                    // The worker took nothing: the row must not say copied
                    // nor close the launcher.
                    if let Some(open) = self.launcher.as_mut() {
                        open.copy_refused();
                    }
                    ctl.request_redraw();
                }
            }
            Some(action) => self.launcher_acted(action, ctl),
            None => {}
        }
    }

    /// Puts the calculator's `text` on the clipboard through the duty's
    /// worker; a refusal is logged (the text stays out of the log). The
    /// launcher stays open for its short "Copied" and closes itself.
    fn copy_to_clipboard(&self, text: String) -> bool {
        let Some(duty) = self.clipboard.get() else {
            tracing::warn!("launcher: no clipboard duty; the result was not copied");
            return false;
        };
        match duty.offer_text(text) {
            Ok(()) => {
                tracing::info!("launcher: result copied");
                true
            }
            Err(refused) => {
                tracing::warn!(?refused, "launcher: the result was not copied");
                false
            }
        }
    }

    /// Asks Melibea to close the minimised window `window_id` (Delete in
    /// the selector); the row leaves when the projection drops the window.
    fn close_window(&mut self, window_id: u64) {
        let Some(handle) = self.bubbles.get() else {
            tracing::info!("bubbles close: no bubbles handle; nothing was done");
            return;
        };
        if self.bubble_closes.pending(window_id) {
            tracing::info!(
                window_id,
                "bubbles close: already asked; waiting for Melibea"
            );
            return;
        }
        match handle.request(BubbleAction::Close { window_id }) {
            Ok(id) => {
                self.bubble_closes.asked(window_id, id);
                tracing::info!(id = id.0, window_id, "bubbles close: asked Melibea");
            }
            Err(why) => tracing::info!("bubbles close: nothing was done: {why}"),
        }
    }
}

impl App {
    /// A verb of the overview (`wanted` Desktop) or of the shortcuts
    /// (`wanted` Shortcuts).
    fn overview_verb(
        &mut self,
        wanted: OverviewMode,
        presence: Presence,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        if !matches!(presence, Presence::Close) && self.polkit_claims_keyboard() {
            tracing::info!("overview: not opened while a polkit prompt holds the keyboard");
            return;
        }
        let steps = self.overview.flow.verb(wanted, presence);
        self.overview_steps(steps, ctl);
    }

    /// Runs what the overview's state machine asked for.
    fn overview_steps(&mut self, steps: Vec<Step>, ctl: &mut RuntimeCtl<'_>) {
        let mut queue = std::collections::VecDeque::from(steps);
        while let Some(step) = queue.pop_front() {
            match step {
                Step::RequestCapture => {
                    // Nothing of the shell's own overlays may be in the
                    // picture: the launcher goes before the capture.
                    if let Some(launcher) = self.launcher.take() {
                        close_surface(launcher.surface(), ctl);
                    }
                    let Some((output, info)) = self.overview_target() else {
                        tracing::info!("overview: no output to open on");
                        self.overview.flow.closed();
                        self.overview.clear_pending();
                        queue.clear();
                        continue;
                    };
                    self.overview.target = Some(output);
                    if let Err(why) = self.overview_request_capture(&info) {
                        tracing::info!(?why, "overview: opening with geometry only");
                        queue.extend(self.overview.flow.capture_failed());
                    }
                }
                Step::CancelCapture => {
                    tracing::info!("overview: the capture is given up");
                    self.overview.cancel_capture();
                }
                Step::OpenSurface(mode) => {
                    self.overview.stop_timer();
                    self.open_overview_surface(mode, ctl);
                    if !self.overview.flow.is_active() {
                        // No surface could be made and the flow closed:
                        // nothing after it (bindings, previews) may run.
                        queue.clear();
                    }
                }
                Step::SetMode(mode) => {
                    if let Some(open) = self.overview.panel.as_mut() {
                        if open.set_mode(mode) {
                            if let Some(mut surface) = ctl.surface(open.surface()) {
                                surface.request_redraw();
                            }
                        }
                    }
                }
                Step::LoadKeybinds => {
                    if let Some(unavailable) = self.overview_load_keybinds() {
                        self.overview.keybinds = Some(unavailable);
                        queue.push_back(Step::ApplyKeybinds);
                    }
                }
                Step::ApplyPreviews => {
                    let previews = std::mem::take(&mut self.overview.previews);
                    if let Some(open) = self.overview.panel.as_mut() {
                        let mut due = false;
                        for (id, preview) in previews {
                            due |= open.set_preview(id, preview);
                        }
                        if due {
                            if let Some(mut surface) = ctl.surface(open.surface()) {
                                surface.request_redraw();
                            }
                        }
                    }
                }
                Step::ApplyKeybinds => {
                    if let (Some(open), Some(keybinds)) =
                        (self.overview.panel.as_mut(), self.overview.keybinds.take())
                    {
                        if open.set_keybinds(keybinds) {
                            if let Some(mut surface) = ctl.surface(open.surface()) {
                                surface.request_redraw();
                            }
                        }
                    }
                }
                Step::Close => self.destroy_overview(ctl),
            }
        }
    }

    /// The output the overview opens on (the one with the keyboard, as the
    /// launcher picks it) and its description.
    fn overview_target(&self) -> Option<(OutputId, OutputInfo)> {
        let output = self
            .islands
            .target(self.keyboard, focused_output(self.model.as_ref()))
            .and_then(|surface| self.islands.output_of(surface))?;
        let info = self.outputs.iter().find(|o| o.id == output)?.clone();
        Some((output, info))
    }

    /// Asks the capture worker for one frame of `output`, the one the panel
    /// opens on, and starts the timer that bounds the wait.
    fn overview_request_capture(&mut self, output: &OutputInfo) -> Result<(), NotAsked> {
        let (Some(model), Some(sender)) = (self.model.as_ref(), self.sender.get()) else {
            return Err(NotAsked::NothingToCapture);
        };
        let generation = self.overview.flow.generation();
        let request = capture_request(model, output, generation);
        let spawner = crate::capture::Live::new(sender.clone());
        self.overview.request_capture(&spawner, request)?;
        let timer_sender = sender.clone();
        self.overview.start_timer(move || {
            let _ = timer_sender.send(Message::OverviewCaptureTimeout(generation));
        });
        Ok(())
    }

    /// Reads the niri config's bindings off the loop's thread, for an open
    /// panel only. When the reader thread cannot start, the question is not
    /// counted as asked and the answer to hand the panel is "unreadable".
    fn overview_load_keybinds(&mut self) -> Option<crate::providers::keybinds::Keybinds> {
        if !self.overview.flow.is_active() || self.overview.panel.is_none() {
            return None;
        }
        let sender = self.sender.get()?;
        let generation = self.overview.flow.generation();
        match crate::providers::keybinds::spawn_load(sender.clone(), generation) {
            Ok(worker) => {
                self.overview.hold_reader(generation, worker);
                None
            }
            Err(err) => {
                tracing::warn!(%err, "overview: the bindings could not be read");
                self.overview.flow.reader_failed();
                Some(crate::providers::keybinds::Keybinds::Unavailable(
                    crate::providers::keybinds::KeybindsUnavailable::Unreadable,
                ))
            }
        }
    }

    /// Creates the overview's surface in `mode` on the output chosen before
    /// the capture (or, for an opening without one, the output with the
    /// keyboard). The flow is closed again when it cannot, also when the
    /// chosen output is gone: the panel never moves to another one.
    fn open_overview_surface(&mut self, mode: OverviewMode, ctl: &mut RuntimeCtl<'_>) {
        // A prompt may have appeared while the capture was awaited: the
        // panel's exclusive keyboard would starve its proof.
        if self.polkit_claims_keyboard() {
            tracing::info!("overview: not opened while a polkit prompt holds the keyboard");
            self.overview.flow.closed();
            self.overview.clear_pending();
            return;
        }
        let chosen = match self.overview.target {
            Some(id) => self
                .outputs
                .iter()
                .find(|o| o.id == id)
                .map(|o| (id, o.clone())),
            None => self.overview_target(),
        };
        let Some((output, info)) = chosen else {
            tracing::info!("overview: no output to open on");
            self.overview.flow.closed();
            self.overview.clear_pending();
            return;
        };
        if let Some(launcher) = self.launcher.take() {
            close_surface(launcher.surface(), ctl);
        }
        let theme = self.theme();
        let spec = LayerSpec {
            output,
            namespace: overview::NAMESPACE,
            layer: Layer::Overlay,
            anchor: Anchoring::Fill,
            size: (0, 0),
            exclusive_zone: -1,
            keyboard: KeyboardInteractivity::Exclusive,
        };
        let surface = match ctl.create_layer_surface(spec) {
            Ok(surface) => surface,
            Err(err) => {
                tracing::error!(%err, "overview: no surface");
                self.overview.flow.closed();
                self.overview.clear_pending();
                return;
            }
        };
        let empty = NiriModel::default();
        let mut open = Overview::new(
            surface,
            output,
            &info.name,
            &theme,
            self.appearance.reduced_motion,
            self.model.as_ref().unwrap_or(&empty),
        );
        open.set_mode(mode);
        if let Some(mut s) = ctl.surface(surface) {
            open.opened(&mut s);
        }
        tracing::info!(
            output = output.0,
            shortcuts = mode == OverviewMode::Shortcuts,
            "overview: opened"
        );
        self.overview.panel = Some(open);
    }

    /// Frees the textures of a panel that is going away. If the painter is
    /// busy (it should not be outside a frame or an input call), the panel is
    /// kept in the slot's `dying` list and freed at the next frame or close.
    fn release_overview(&mut self, open: Overview) {
        release_overview_panel(&mut self.overview, self.painter.as_ref(), open);
    }

    /// Frees the textures of panels whose release had to wait.
    fn release_dying_overviews(&mut self) {
        if self.overview.dying.is_empty() {
            return;
        }
        if let Some(Ok(mut painter)) = self.painter.as_ref().map(|p| p.try_borrow_mut()) {
            for mut open in self.overview.dying.drain(..) {
                open.release(&mut painter);
            }
        }
    }

    /// Closes whatever there is of the overview from the loop: gives up a
    /// wait, or releases and destroys the panel.
    fn close_overview(&mut self, ctl: &mut RuntimeCtl<'_>) {
        if self.overview.flow.is_active() {
            let steps = self
                .overview
                .flow
                .verb(OverviewMode::Desktop, Presence::Close);
            self.overview_steps(steps, ctl);
        }
    }

    fn destroy_overview(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.overview.clear_pending();
        self.release_dying_overviews();
        if let Some(open) = self.overview.panel.take() {
            tracing::info!("overview: closed");
            let surface = open.surface();
            self.release_overview(open);
            if let Some(mut s) = ctl.surface(surface) {
                Overview::closing(&mut s);
            }
            ctl.destroy_surface(surface);
        }
    }

    /// Input on the overview's surface.
    fn overview_input(&mut self, ev: &InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let (Some(shared), Some(open)) = (self.painter.clone(), self.overview.panel.as_mut())
        else {
            return;
        };
        let action = match shared.try_borrow_mut() {
            Ok(mut painter) => open.input(ev, ctl, &mut painter),
            Err(_) => {
                tracing::warn!("the painter is in use; the overview's input is dropped");
                return;
            }
        };
        let Some(action) = action else {
            return;
        };
        if action == OverviewAction::SwitchMode {
            let mode = open.state().mode;
            for step in self.overview.flow.mode_switched(mode) {
                if step != Step::LoadKeybinds {
                    continue;
                }
                if let Some(unavailable) = self.overview_load_keybinds() {
                    if let Some(open) = self.overview.panel.as_mut() {
                        if open.set_keybinds(unavailable) {
                            ctl.request_redraw();
                        }
                    }
                }
            }
            return;
        }
        self.overview_acted(action, ctl);
    }

    /// The overview asked to close during an input call: it is closed now and
    /// its surface is destroyed when the posted [`Message::OverviewClosed`]
    /// arrives; a chosen window or workspace is focused first.
    fn overview_acted(&mut self, action: OverviewAction, ctl: &mut SurfaceCtl<'_>) {
        let Some(open) = self.overview.panel.take() else {
            return;
        };
        let surface = open.surface();
        self.overview.flow.closed();
        self.overview.clear_pending();
        self.release_overview(open);
        Overview::closing(ctl);
        ctl.set_keyboard_interactivity(KeyboardInteractivity::None);
        match action {
            OverviewAction::Focus(id) => self.request(NiriRequest::FocusWindow(id)),
            OverviewAction::FocusWorkspace(id) => self.request(NiriRequest::FocusWorkspace(id)),
            OverviewAction::SwitchMode | OverviewAction::Close => {}
        }
        tracing::info!(?action, "overview: closed by the person");
        let closed = Message::OverviewClosed { surface };
        if !self
            .sender
            .get()
            .is_some_and(|sender| sender.send(closed).is_ok())
        {
            tracing::warn!("overview: the loop cannot be reached; its surface stays until exit");
        }
    }

    /// The capture's reply. Only the wait of its own opening uses it: the
    /// pictures wait for the panel, which opens now. A reply of another
    /// opening, or one that comes after the timeout or a close, is dropped
    /// with its pixels.
    fn overview_captured(&mut self, reply: crate::capture::CaptureReply, ctl: &mut RuntimeCtl<'_>) {
        self.overview.capture_replied(reply.generation);
        let steps = self.overview.flow.captured(reply.generation);
        if steps.is_empty() {
            tracing::debug!(
                generation = reply.generation,
                "overview: a capture reply nobody waits for is dropped"
            );
            return;
        }
        self.overview.stop_timer();
        match reply.result {
            crate::capture::CaptureResult::Done { previews } => self.overview.previews = previews,
            crate::capture::CaptureResult::Unavailable(reason) => {
                tracing::info!(%reason, "overview: no previews");
            }
        }
        self.overview_steps(steps, ctl);
        // A reply nobody can use is dropped with its pixels.
        if self.overview.panel.is_none() {
            self.overview.previews.clear();
        }
    }

    /// The bindings of opening `generation`: shown if that opening's panel is
    /// still up, dropped otherwise.
    fn overview_keybinds_loaded(
        &mut self,
        generation: u64,
        keybinds: crate::providers::keybinds::Keybinds,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        self.overview.reader_replied(generation);
        let steps = self.overview.flow.keybinds_loaded(generation);
        if steps.is_empty() {
            tracing::debug!(
                generation,
                "overview: bindings nobody waits for are dropped"
            );
            return;
        }
        match &keybinds {
            crate::providers::keybinds::Keybinds::Groups(groups) => {
                tracing::info!("overview: shortcuts loaded: {} group(s)", groups.len());
            }
            crate::providers::keybinds::Keybinds::Unavailable(why) => {
                tracing::info!(?why, "overview: shortcuts unavailable");
            }
        }
        self.overview.keybinds = Some(keybinds);
        self.overview_steps(steps, ctl);
        self.overview.keybinds = None;
    }
}

/// Destroys the launcher's `surface`, its text input off first.
fn close_surface(surface: SurfaceId, ctl: &mut RuntimeCtl<'_>) {
    if let Some(mut s) = ctl.surface(surface) {
        Launcher::closing(&mut s);
    }
    ctl.destroy_surface(surface);
}

/// What the polkit prompt may do through the app: make and destroy its
/// surfaces, close the surfaces that hold the keyboard, and raise its notices
/// through the shell's own notification server (CEL-2-I).
/// Frees the textures of a closed overview panel. If the painter is busy the
/// panel waits in the slot's `dying` list for the next frame or close.
fn release_overview_panel(
    slot: &mut OverviewSlot,
    painter: Option<&SharedPainter>,
    mut open: Overview,
) {
    match painter.map(|p| p.try_borrow_mut()) {
        Some(Ok(mut painter)) => open.release(&mut painter),
        Some(Err(_)) => {
            tracing::warn!(
                "overview: the painter is in use; the panel's textures are freed at the next frame"
            );
            slot.dying.push(open);
        }
        None => {}
    }
}

struct PolkitHost<'a, 'b> {
    ctl: &'a mut RuntimeCtl<'b>,
    launcher: &'a mut Option<Launcher>,
    islands: &'a mut Islands,
    notifications: &'a mut NotificationsState,
    overview: &'a mut OverviewSlot,
    painter: Option<&'a SharedPainter>,
}

impl PromptHost for PolkitHost<'_, '_> {
    fn create_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        self.ctl.create_layer_surface(spec)
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.ctl.destroy_surface(id);
    }

    fn request_redraw(&mut self, id: SurfaceId) {
        if let Some(mut surface) = self.ctl.surface(id) {
            surface.request_redraw();
        }
    }

    /// The launcher, every open Island page and every menu of the bar go: two
    /// exclusive overlays (or a popup grab) would leave the choice to the
    /// compositor and starve the proof.
    fn close_keyboard_surfaces(&mut self) {
        if let Some(open) = self.launcher.take() {
            tracing::info!("launcher: closed by a polkit prompt");
            close_surface(open.surface(), self.ctl);
        }
        for slot in &mut self.islands.slots {
            let Some(mut surface) = self.ctl.surface(slot.surface) else {
                continue;
            };
            // The Island yields the keyboard from now on and what it shows
            // (its page, a sheet, what waits) closes.
            slot.island.set_keyboard_claimed(true);
            if slot.island.anything_open() {
                slot.island.command(&Verb::IslandClose, &mut surface);
            }
            // A menu hanging from the bar (a tray icon's) survives the page
            // closing and may hold a popup grab: it goes too (N-25).
            slot.island.close_menu(&mut surface);
        }
        // The overview's panel holds the keyboard exclusively too: the wait
        // for its capture is given up and an open panel is destroyed.
        if self.overview.flow.is_active() || self.overview.panel.is_some() {
            self.overview.flow.closed();
            self.overview.clear_pending();
            if let Some(open) = self.overview.panel.take() {
                tracing::info!("overview: closed by a polkit prompt");
                let surface = open.surface();
                release_overview_panel(self.overview, self.painter, open);
                if let Some(mut s) = self.ctl.surface(surface) {
                    Overview::closing(&mut s);
                }
                self.ctl.destroy_surface(surface);
            }
        }
    }

    fn notice(&mut self, notice: Notice) {
        let incoming = polkit::notice_incoming(notice);
        if self
            .notifications
            .send(NotificationsCommand::Post(Box::new(incoming)))
        {
            return;
        }
        // Another program holds the notification name (or the worker is gone):
        // the banner cannot be shown, so the journal carries the notice.
        polkit::notice_unraised(notice);
    }
}

impl App {
    /// Whether a polkit prompt holds the keyboard (or has surfaces left to
    /// destroy that do): nothing else that takes it opens meanwhile.
    fn polkit_claims_keyboard(&self) -> bool {
        self.polkit
            .state()
            .is_some_and(PolkitState::keyboard_claimed)
    }

    /// Runs `f` on the polkit state with a host over the rest of the app;
    /// `None` when the duty has started no agent.
    fn polkit_run<R>(
        &mut self,
        ctl: &mut RuntimeCtl<'_>,
        f: impl FnOnce(&mut PolkitState, &Ctx<'_>, &mut dyn PromptHost) -> R,
    ) -> Option<R> {
        let focused = focused_output(self.model.as_ref()).map(str::to_owned);
        let polkit = self.polkit.state_mut()?;
        let mut host = PolkitHost {
            ctl,
            launcher: &mut self.launcher,
            islands: &mut self.islands,
            notifications: &mut self.notifications,
            overview: &mut self.overview,
            painter: self.painter.as_ref(),
        };
        let ctx = Ctx {
            now: std::time::Instant::now(),
            focused_output: focused.as_deref(),
        };
        Some(f(polkit, &ctx, &mut host))
    }

    /// Brings the `polkit` duty to what `shell.json` says: starts the agent
    /// and the prompt state, or tears them down (every request cancelled, the
    /// helper conversation closed, the text wiped, the surfaces destroyed,
    /// the agent unregistered). A duty that is on and whose registration was
    /// refused is left as it is: off and on again is the retry.
    fn polkit_apply(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let want = self.settings.current().duties.polkit;
        let theme = self.theme();
        let reduced = self.appearance.reduced_motion;
        let mut host = PolkitHost {
            ctl,
            launcher: &mut self.launcher,
            islands: &mut self.islands,
            notifications: &mut self.notifications,
            overview: &mut self.overview,
            painter: self.painter.as_ref(),
        };
        let change = self
            .polkit
            .apply(want, &theme, reduced, std::time::Instant::now(), &mut host);
        if change != Change::Unchanged {
            tracing::info!(?change, "polkit: the duty was applied");
            self.arm_polkit_timer(ctl);
        }
    }

    /// Wakes the loop at the prompt's next deadline through the first Island's
    /// surface, which always exists while a prompt can: a card the compositor
    /// never configures has no frames of its own to be woken by.
    fn arm_polkit_timer(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let Some(wait) = self
            .polkit
            .state()
            .and_then(|p| p.next_wake(std::time::Instant::now()))
        else {
            return;
        };
        let carrier = self.islands.iter().next().map(|slot| slot.surface);
        if let Some(mut surface) = carrier.and_then(|id| ctl.surface(id)) {
            surface.redraw_after(wait);
        }
    }

    /// Posts [`Message::PolkitSettle`] to the loop: a prompt ended during an
    /// input or frame call, which cannot destroy a surface or map the next.
    fn post_polkit_settle(&self) {
        match self
            .sender
            .get()
            .map(|sender| sender.send(Message::PolkitSettle))
        {
            Some(Ok(())) => {}
            _ => tracing::warn!("polkit: the loop cannot be reached; the prompt's surfaces stay"),
        }
    }

    /// Input on one of the prompt's surfaces.
    fn polkit_input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let now = std::time::Instant::now();
        let surface = ctl.id();
        let Some(polkit) = self.polkit.state_mut() else {
            return;
        };
        let outcome = polkit.input(surface, ev, now);
        if outcome.redraw {
            ctl.request_redraw();
        }
        if outcome.settle {
            // The keyboard and the pointer are released now; the surface
            // goes when the posted message arrives.
            ctl.set_keyboard_interactivity(KeyboardInteractivity::None);
            ctl.set_input_region(&[]);
        }
        if let Some(wait) = polkit.next_wake(now) {
            ctl.redraw_after(wait);
        }
        if outcome.settle {
            self.post_polkit_settle();
        }
    }
}

impl Shell for App {
    type Message = Message;

    fn output(&mut self, ev: OutputEvent, ctl: &mut RuntimeCtl<'_>) {
        match ev {
            OutputEvent::Added(info) => {
                self.polkit.note_output(&info);
                self.polkit_run(ctl, |polkit, _, host| polkit.output_added(&info, host));
                self.outputs.retain(|o| o.id != info.id);
                self.outputs.push(info.clone());
                self.osds.output_added(&info);
                let named = self.controls.readings.output_added(info.id, &info.name);
                self.controls
                    .readings
                    .set_model(info.id, ctl.output_model(info.id).as_deref());
                if named {
                    self.levels_changed(ctl);
                }
                self.output_added(&info, ctl);
                self.wallpapers.output_added(
                    &info,
                    self.settings.current(),
                    ctl,
                    self.painter.as_ref(),
                );
            }
            OutputEvent::Changed(info) => {
                self.polkit.note_output(&info);
                if let Some(known) = self.outputs.iter_mut().find(|o| o.id == info.id) {
                    *known = info.clone();
                }
                let named = self.controls.readings.output_added(info.id, &info.name);
                self.controls
                    .readings
                    .set_model(info.id, ctl.output_model(info.id).as_deref());
                if named {
                    self.levels_changed(ctl);
                }
                tracing::debug!(output = %info.name, "output changed; the Island keeps its surface");
                self.osds.output_changed(&info);
                self.wallpapers.output_changed(
                    &info,
                    self.settings.current(),
                    ctl,
                    self.painter.as_ref(),
                );
            }
            OutputEvent::Removed(output) => {
                self.polkit.forget_output(output);
                self.polkit_run(ctl, |polkit, cx, host| {
                    polkit.output_removed(output, cx.now);
                    if polkit.needs_settle(cx.now) {
                        polkit.settle(cx, host);
                    }
                });
                self.arm_polkit_timer(ctl);
                self.osds.output_removed(output);
                if self.controls.readings.output_removed(output) {
                    self.levels_changed(ctl);
                }
                self.outputs.retain(|o| o.id != output);
                if self.overview.output_removed(output) {
                    tracing::info!("overview: the opening is given up with its output");
                }
                if self
                    .overview
                    .panel
                    .as_ref()
                    .is_some_and(|o| o.output() == output)
                {
                    // Its surface went with the output.
                    if let Some(open) = self.overview.panel.take() {
                        self.release_overview(open);
                    }
                    self.overview.flow.closed();
                    self.overview.clear_pending();
                    tracing::info!("overview: closed with its output");
                }
                self.wallpapers
                    .output_removed(output, ctl, self.painter.as_ref());
                if self.launcher.as_ref().is_some_and(|l| l.output() == output) {
                    // Its surface went with the output.
                    self.launcher = None;
                    tracing::info!("launcher: closed with its output");
                }
                // A card held the server's clock under the pointer: its
                // Island goes with the output, so the hold ends here.
                if self.islands.release_card_holds(output) {
                    self.notifications.send(NotificationsCommand::Hover(false));
                }
                // The banners' surface went with the output: no frame for
                // it; they come back on another output if any remain.
                if let Some(banners) = self.banners.take_if(|b| b.output == output) {
                    self.banners_gone(banners);
                }
                if let Some(shared) = self.painter.clone() {
                    if let Ok(mut painter) = shared.try_borrow_mut() {
                        self.islands.release_icons(output, &mut painter);
                    }
                }
                // A level sheet closes (and sends what its throttle holds)
                // while its Island still exists: `Sheets` has no Drop flush.
                self.islands.close_level_sheets(output);
                self.catchers.remove_output(output);
                let gone = self.islands.remove_output(output);
                if self.keyboard.is_some_and(|k| gone.contains(&k)) {
                    self.keyboard = None;
                }
                // The slider popup went with its Island: the display must
                // not stay suppressed for good.
                if let Some(released) = self.controls.surfaces_gone(&gone) {
                    self.post(Message::OsdSuppressed(released));
                }
                if self
                    .open_tray_menu
                    .as_ref()
                    .is_some_and(|o| gone.contains(&o.surface))
                {
                    self.tray_menu_gone();
                }
                tracing::info!(
                    output = output.0,
                    islands = self.islands.len(),
                    "output gone with its Island"
                );
                self.sync_banners(ctl);
            }
        }
    }

    fn frame(&mut self, cx: &mut FrameCx<'_>) {
        if self.painter.is_none() {
            let Some(fonts) = self.fonts.take() else {
                // The painter failed once and the loop is stopping: no
                // second attempt.
                return;
            };
            match Painter::new(cx.proc_address, self.theme(), fonts) {
                Ok(painter) => {
                    self.painter = Some(Rc::new(RefCell::new(painter)));
                    // Media published before the painter existed.
                    self.media_sync();
                }
                Err(err) => {
                    // Blank Islands holding their exclusive zones help
                    // nobody: the run ends and the binary reports why.
                    let why = format!("cannot create the painter: {}", error_chain(&err));
                    tracing::error!("{why}; stopping");
                    let _ = self.fatal.set(why);
                    cx.stop();
                    return;
                }
            }
        }
        self.expire_pending_menu(cx.now);
        let (mut prompt_surface, mut settle) = (false, false);
        if let Some(polkit) = self.polkit.state_mut() {
            polkit.tick(cx.now);
            prompt_surface = polkit.owns(cx.surface);
            if prompt_surface {
                match self.painter.as_ref().map(|p| p.try_borrow_mut()) {
                    Some(Ok(mut painter)) => {
                        polkit.frame(cx, &mut painter);
                    }
                    _ => {
                        tracing::warn!("the painter is in use; the prompt's frame is skipped");
                        cx.ctl.request_redraw();
                    }
                }
            } else if polkit.keyboard_claimed() {
                // Every frame keeps the prompt's timer armed.
                if let Some(wait) = polkit.next_wake(cx.now) {
                    cx.ctl.redraw_after(wait);
                }
            }
            settle = polkit.needs_settle(cx.now);
        }
        if settle {
            self.post_polkit_settle();
        }
        if prompt_surface {
            return;
        }
        if self.media_dirty {
            // The Island drawn now is among those given; the others get it
            // on their next frame.
            self.media_sync();
        }
        // A closed overview whose textures had to wait is freed now.
        self.release_dying_overviews();
        if self.wallpapers.owns(cx.surface) {
            if let Some(shared) = self.painter.as_ref() {
                if let Some(failed) = self.wallpapers.frame(cx, shared, self.settings.current()) {
                    // The surface goes when the posted failure arrives.
                    if !self
                        .sender
                        .get()
                        .is_some_and(|sender| sender.send(Message::Wallpaper(failed)).is_ok())
                    {
                        tracing::warn!(
                            "wallpaper: the loop cannot be reached; the failed surface stays"
                        );
                    }
                }
            }
            return;
        }
        if self.osds.owns(cx.surface) {
            if let Some(shared) = self.painter.as_ref() {
                self.osds.frame(cx, shared, self.appearance.reduced_motion);
            }
            return;
        }
        self.wallpapers
            .scale_seen(cx.output, cx.scale, self.settings.current());
        if let (Some(shared), Some(open)) = (self.painter.as_ref(), self.launcher.as_mut()) {
            if open.surface() == cx.surface {
                match shared.try_borrow_mut() {
                    Ok(mut painter) => open.frame(
                        cx,
                        &mut painter,
                        self.appearance.reduced_motion,
                        &self.index,
                    ),
                    Err(_) => {
                        tracing::warn!("the painter is in use; the launcher's frame is skipped");
                        cx.ctl.request_redraw();
                    }
                }
                if open.take_finished() {
                    // The copy said "Copied": close as a Close does.
                    self.launcher_acted(LauncherAction::Close, &mut cx.ctl);
                }
                return;
            }
        }
        if let (Some(shared), Some(open)) = (self.painter.as_ref(), self.overview.panel.as_mut()) {
            if open.surface() == cx.surface {
                match shared.try_borrow_mut() {
                    Ok(mut painter) => open.frame(cx, &mut painter, self.appearance.reduced_motion),
                    Err(_) => {
                        tracing::warn!("the painter is in use; the overview's frame is skipped");
                        cx.ctl.request_redraw();
                    }
                }
                return;
            }
        }
        if self.catchers.owns(cx.surface) {
            match self.painter.as_ref().map(|p| p.try_borrow_mut()) {
                Some(Ok(mut painter)) => {
                    // Nothing is painted: a transparent buffer maps it.
                    painter.begin(cx.size, cx.scale as f32, false);
                    painter.end();
                    cx.ctl.set_blur_region(&[]);
                    cx.ctl.set_input_region(&catcher::region_for(cx.size));
                }
                _ => {
                    tracing::warn!("the painter is in use; the catcher's frame is skipped");
                    cx.ctl.request_redraw();
                }
            }
            return;
        }
        if self
            .banners
            .as_ref()
            .is_some_and(|b| b.surface == cx.surface)
        {
            self.notice_frame(cx);
            return;
        }
        let claimed = self.polkit_claims_keyboard();
        let (Some(shared), Some(island)) =
            (self.painter.as_ref(), self.islands.get_mut(cx.surface))
        else {
            return;
        };
        let Ok(mut painter) = shared.try_borrow_mut() else {
            tracing::warn!("the painter is in use; the Island's frame is skipped");
            cx.ctl.request_redraw();
            return;
        };
        let state = Shared {
            owners: &self.owners,
            do_not_disturb: self.settings.current().do_not_disturb,
            history: self.notifications.rows(),
            tray: &self.tray_items,
            sysmon: self.sysmon.as_ref(),
            weather: self.weather.as_ref(),
            clipboard_on: self.settings.current().duties.clipboard == Duty::On,
            clipboard: &self.clipboard_state,
            audio: self.controls.readings.audio(),
            brightness: self.controls.readings.by_output(),
            phone: &self.phone_state,
            phone_duty: self.settings.current().duties.phone == Duty::On,
            bubbles: &self.bubbles_state,
        };
        island.set_keyboard_claimed(claimed);
        island.frame(
            cx,
            &mut painter,
            self.model.as_ref(),
            &state,
            self.appearance.reduced_motion,
        );
        drop(painter);
        self.watch_pages();
        self.settle_level_sheet(cx.surface);
        // A sheet that opened from the queue in this frame is seen too.
        self.mark_read_if_viewed();
        self.catchers_due();
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let surface = ctl.id();
        if self.polkit.state().is_some_and(|p| p.owns(surface)) {
            self.polkit_input(ev, ctl);
            return;
        }
        if let Some(banners) = self.banners.as_mut().filter(|b| b.surface == surface) {
            let Some(Ok(mut painter)) = self.painter.as_ref().map(|p| p.try_borrow_mut()) else {
                tracing::warn!("no painter at hand; the banners' input is dropped");
                return;
            };
            let now = std::time::Instant::now();
            banners.notice.prepare(&mut painter, now);
            drop(painter);
            let input = banners.notice.pointer(&ev, now);
            if input.redraw {
                ctl.request_redraw();
            }
            let output = banners.output;
            self.banners_act(input.actions, output);
            return;
        }
        if self.catchers.owns(surface) {
            // The press is spent here: it closes what the Island shows and
            // never reaches the window under it.
            if catcher::closes(&ev) {
                self.post(Message::CatcherPress(surface));
            }
            return;
        }
        match ev {
            InputEvent::FocusEnter { .. } => self.keyboard = Some(surface),
            InputEvent::FocusLost if self.keyboard == Some(surface) => self.keyboard = None,
            _ => {}
        }
        if self
            .launcher
            .as_ref()
            .is_some_and(|l| l.surface() == surface)
        {
            self.launcher_input(&ev, ctl);
            return;
        }
        if self
            .overview
            .panel
            .as_ref()
            .is_some_and(|o| o.surface() == surface)
        {
            self.overview_input(&ev, ctl);
            return;
        }
        let closed = matches!(ev, InputEvent::PopupClosed { .. });
        let claimed = self.polkit_claims_keyboard();
        let (Some(shared), Some(island)) = (self.painter.clone(), self.islands.get_mut(surface))
        else {
            return;
        };
        let actions = match shared.try_borrow_mut() {
            Ok(mut painter) => {
                island.set_keyboard_claimed(claimed);
                island.input(ev, ctl, &mut painter)
            }
            Err(_) => {
                tracing::warn!("the painter is in use; the input is dropped");
                return;
            }
        };
        let tray_menu_open = island.tray_menu_open();
        self.act(actions, surface, &shared, ctl);
        // The pointer over the card holds the server's clock; the page may
        // have opened or closed.
        self.send_card_holds();
        self.watch_pages();
        self.settle_level_sheet(surface);
        self.mark_read_if_viewed();
        self.catchers_due();
        // The menu is gone: a row chosen in it is made now. The Island has
        // one popup at a time, so the slot is that popup's.
        if closed {
            if let Some(choice) = self.menu_choice.take() {
                self.menu_chose(choice, surface, ctl);
            }
            // A tray menu's chosen entry goes to its item once the chain
            // is gone, and the host hears that the chain closed.
            let chosen = self.tray_choice.borrow_mut().take();
            if let Some((item, entry)) = chosen {
                self.tray_command(TrayCommand::MenuEvent(item, entry));
            }
            // The shell's own entry is not the app's: the loop writes the
            // list and every bar follows.
            let shell_chosen = self.tray_shell_choice.borrow_mut().take();
            if let Some((item, entry)) = shell_chosen {
                self.post(Message::TrayShellEntry(item, entry));
            }
            if !tray_menu_open
                && self
                    .open_tray_menu
                    .as_ref()
                    .is_some_and(|o| o.surface == surface)
            {
                self.tray_menu_gone();
            }
        }
    }

    fn message(&mut self, msg: Message, ctl: &mut RuntimeCtl<'_>) {
        if self.media_dirty {
            // The last give was skipped (the painter was in use).
            let draw = self.media_sync();
            Self::redraw_surfaces(ctl, &draw);
        }
        match msg {
            Message::Niri(model) => {
                for slot in self.islands.iter() {
                    if slot.island.shows_a_change(Some(&model)) {
                        if let Some(mut surface) = ctl.surface(slot.surface) {
                            surface.request_redraw();
                        }
                    }
                }
                if let Some(open) = self.overview.panel.as_mut() {
                    if open.set_model(&model) {
                        if let Some(mut surface) = ctl.surface(open.surface()) {
                            surface.request_redraw();
                        }
                    }
                }
                self.model = Some(model);
                // The notices follow the focus to another output, a card
                // as much as the surface.
                if self.banners.is_some() || self.islands.any_notice() {
                    self.sync_banners(ctl);
                }
            }
            Message::Appearance(appearance) => {
                if appearance == self.appearance {
                    return;
                }
                self.appearance = appearance;
                let theme = self.theme();
                let reduced = self.appearance.reduced_motion;
                if let Some(polkit) = self.polkit.state_mut() {
                    polkit.set_appearance(theme.clone(), reduced);
                }
                match self.painter.as_ref().map(|p| p.try_borrow_mut()) {
                    Some(Ok(mut painter)) => painter.set_theme(theme),
                    Some(Err(_)) => tracing::warn!("the painter is in use; the theme waits"),
                    None => {}
                }
                self.redraw_all(ctl);
            }
            Message::NiriRequestRefused(request) => self.refused(request),
            Message::Command(verb) => {
                self.command(verb, ctl);
                self.mark_read_if_viewed();
                // A verb that opened or closed a page: the notices follow.
                self.watch_pages();
            }
            Message::OpenNotificationsPage(output) => {
                self.open_page_on(output, PageId::NOTIFICATIONS, ctl);
                self.mark_read_if_viewed();
                self.watch_pages();
            }
            Message::OpenClockPage(output) => {
                self.open_page_on(output, PageId::CLOCK, ctl);
                self.watch_pages();
            }
            Message::PagesChanged => self.pages_changed(ctl),
            Message::NoticeLend(surface) => self.notice_lend(surface, ctl),
            Message::NoticeRest(surface) => self.notice_rest(surface, ctl),
            Message::NoticeRelease(surface) => self.notice_release(surface, ctl),
            Message::DesktopIndex(entries) => {
                tracing::info!(entries = entries.len(), "desktop index received");
                self.index = entries;
                if let Some(open) = self.launcher.as_mut() {
                    if open.set_index(&self.index) {
                        if let Some(mut surface) = ctl.surface(open.surface()) {
                            surface.request_redraw();
                        }
                    }
                }
            }
            Message::LauncherClosed {
                surface,
                output,
                switch,
            } => {
                close_surface(surface, ctl);
                if let Some(switch) = switch {
                    self.run_switch(switch, output, ctl);
                    // `/notificaciones` may have opened the page.
                    self.mark_read_if_viewed();
                }
            }
            Message::Notifications(event) => {
                let applied = self.notifications.apply(event);
                // `set_owners` keeps the tray's holder.
                let owners = with_notifications_owner(&self.owners, self.notifications.owner());
                let owner_changed = self.set_owners(owners);
                if applied.is_changed() || owner_changed {
                    self.notifications_changed(ctl);
                }
            }
            Message::Tray(event) => self.tray_event(event, ctl),
            Message::TrayShellEntry(item, entry) => self.tray_shell_entry(&item, entry, ctl),
            Message::Wallpaper(event) => {
                self.wallpapers
                    .message(event, &mut self.settings, ctl, self.painter.as_ref());
            }
            Message::ShellSettings(read) => self.shell_settings_read(*read, ctl),
            Message::Energy(event) => event.log(),
            Message::PolkitApply => self.polkit_apply(ctl),
            Message::Polkit(event) => {
                event.log();
                if let crate::providers::polkit::PolkitEvent::Registration(state) = &event {
                    self.polkit.registration_reported(state.clone());
                }
                self.polkit_run(ctl, |polkit, cx, host| polkit.on_event(event, cx, host));
                self.arm_polkit_timer(ctl);
            }
            Message::PolkitHelper { id, event } => {
                self.polkit_run(ctl, |polkit, cx, host| {
                    polkit.on_helper(id, event, cx, host)
                });
                self.arm_polkit_timer(ctl);
            }
            Message::PolkitSettle => {
                self.polkit_run(ctl, |polkit, cx, host| polkit.settle(cx, host));
                self.arm_polkit_timer(ctl);
            }
            Message::Lock(message) => match self.lock.get() {
                Some(handle) => handle.message(message),
                None => tracing::warn!("no lock handle: a lock message was dropped"),
            },
            Message::Sysmon(reading) => self.sysmon_read(reading, ctl),
            Message::Weather(reading) => self.weather_read(reading, ctl),
            Message::Clipboard(message) => self.clipboard_event(message, ctl),
            Message::Connectivity(event) => self.connectivity_event(&event, ctl),
            Message::Audio(event) => {
                if !self.levels_live() {
                    return;
                }
                if self.controls.readings.audio_event(event) {
                    self.levels_changed(ctl);
                }
                let theme = self.theme();
                let place = self.osd_placement();
                self.osds.audio(
                    event,
                    focused_output(self.model.as_ref()),
                    place,
                    ctl,
                    &theme,
                );
                self.level_orders(ctl);
            }
            // The volume popup's rows; no piece draws it, and the display
            // never hears of it.
            Message::AudioGraph(graph) => {
                if self.controls.audio_graph(graph, self.levels_live()) {
                    self.popup_refreshed(ctl);
                }
            }
            // A monitor's reading; the first one of each monitor is its
            // baseline.
            Message::Brightness(event) => {
                if !self.levels_live() {
                    return;
                }
                if self.controls.readings.brightness_event(&event) {
                    self.levels_changed(ctl);
                }
                let theme = self.theme();
                let place = self.osd_placement();
                self.osds.brightness(
                    event,
                    focused_output(self.model.as_ref()),
                    place,
                    ctl,
                    &theme,
                );
                self.level_orders(ctl);
            }
            Message::CloseLevelSheet(surface) => {
                if let (Some(island), Some(mut surface_ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
                    island.close_level_sheet(&mut surface_ctl);
                }
            }
            Message::OsdSuppressed(suppression) => {
                let theme = self.theme();
                self.osds.set_suppressed(suppression, ctl, &theme);
                self.level_orders(ctl);
            }
            Message::Osd(tick) => {
                let theme = self.theme();
                self.osds.tick(tick, ctl, &theme);
                self.level_orders(ctl);
            }
            Message::Melibea(news) => self.bubbles_news(*news, ctl),
            Message::Phone(news) => self.phone_news(*news, ctl),
            Message::Media(event) => self.media_event(event, ctl),
            Message::Captured(reply) => self.overview_captured(reply, ctl),
            Message::KeybindsLoaded {
                generation,
                keybinds,
            } => self.overview_keybinds_loaded(generation, keybinds, ctl),
            Message::OverviewClosed { surface } => ctl.destroy_surface(surface),
            Message::SyncCatchers => self.sync_catchers(ctl),
            Message::CatcherPress(surface) => self.catcher_pressed(surface, ctl),
            Message::OverviewCaptureTimeout(generation) => {
                let steps = self.overview.flow.timeout(generation);
                if !steps.is_empty() {
                    tracing::info!("overview: the capture did not reply in time");
                }
                self.overview_steps(steps, ctl);
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if !self.metrics {
            return;
        }
        if let Some(Ok(painter)) = self.painter.as_ref().map(|p| p.try_borrow()) {
            let usage = painter.atlas_usage();
            tracing::info!(
                glyphs = usage.glyphs,
                texels = usage.texels,
                page_texels = usage.page_texels,
                occupancy = format!("{:.4}", usage.fraction()),
                "metrics: glyph atlas at exit"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::niri::Workspace;

    fn islands() -> Islands {
        let mut islands = Islands::default();
        islands.add(
            OutputId(1),
            SurfaceId(10),
            Island::new("DP-1", (1920.0, 560.0)),
        );
        islands.add(
            OutputId(2),
            SurfaceId(20),
            Island::new("HDMI-A-1", (2560.0, 560.0)),
        );
        islands
    }

    #[test]
    fn focus_moving_to_another_output_hands_the_capsule_back() {
        let mut islands = islands();
        let (dp, hdmi) = (OutputId(1), OutputId(2));
        // The notices surface on DP-1 drew its first frame: DP-1 lends.
        let mut first = Handover::new();
        assert_eq!(first.frame(true), Some(Post::Lend));
        assert_eq!(islands.answer(first.lend(true), dp), (true, false));
        assert!(islands.island_of(dp).is_some_and(Island::capsule_lent));
        // The focus moves while the notice is out: DP-1 gets its capsule
        // back at once and its surface is released on the next turn.
        assert!(islands.retire(&mut first, dp), "DP-1's panel is redrawn");
        assert!(!islands.island_of(dp).is_some_and(Island::capsule_lent));
        // HDMI-A-1's own surface lends only after its first frame.
        let mut second = Handover::new();
        assert!(!islands.island_of(hdmi).is_some_and(Island::capsule_lent));
        assert_eq!(second.frame(true), Some(Post::Lend));
        assert_eq!(islands.answer(second.lend(true), hdmi), (true, false));
        assert!(islands.island_of(hdmi).is_some_and(Island::capsule_lent));
        // At no point was a capsule missing on an output with no surface
        // lending it: the old one is back.
        assert!(!islands.island_of(dp).is_some_and(Island::capsule_lent));
        // The end on HDMI-A-1: the capsule comes back, then the release.
        assert_eq!(second.frame(false), Some(Post::Rest));
        assert_eq!(islands.answer(second.rest(false), hdmi), (true, true));
        assert!(!islands.island_of(hdmi).is_some_and(Island::capsule_lent));
        // A retired surface that never lent has nothing to give back.
        let mut never = Handover::new();
        assert!(!islands.retire(&mut never, dp));
    }

    fn toast(id: u32) -> celestina_shell::notifications::Notification {
        celestina_shell::notifications::Notification {
            id,
            app_name: "app".to_owned(),
            summary: format!("n{id}"),
            body: String::new(),
            image: None,
            actions: Vec::new(),
            urgency: celestina_shell::notifications::Urgency::Normal,
            posted_ms: 0,
            expires_at_ms: None,
            read: false,
        }
    }

    #[test]
    fn a_toast_goes_to_the_open_page_of_the_banner_output_only() {
        let mut islands = islands();
        let (dp, hdmi) = (OutputId(1), OutputId(2));
        let now = Instant::now();
        // No page open: the notices surface takes it, no Island gets a card.
        let route = islands.route_notice(Some(dp), Some(&toast(1)), now);
        assert!(!route.carded);
        assert!(route.redraw.is_empty());
        // The page opens on the banner output: that Island takes the newest.
        if let Some(island) = islands.slots.iter_mut().find(|s| s.output == dp) {
            island.island.open_for_test(PageId::CLOCK, now);
        }
        let route = islands.route_notice(Some(dp), Some(&toast(2)), now);
        assert!(route.carded);
        assert_eq!(route.redraw, vec![SurfaceId(10)]);
        assert_eq!(islands.island_of(dp).map(Island::notice_id), Some(Some(2)));
        assert_eq!(islands.island_of(hdmi).map(Island::notice_id), Some(None));
        assert!(
            islands.any_notice(),
            "a card is a notice the focus must move"
        );
        // The same toast again changes nothing (no redraw).
        let route = islands.route_notice(Some(dp), Some(&toast(2)), now);
        assert!(route.carded && route.redraw.is_empty());
        // A page open on another output than the banner's is not the card's.
        let route = islands.route_notice(Some(hdmi), Some(&toast(2)), now);
        assert!(!route.carded);
        assert_eq!(islands.island_of(dp).map(Island::notice_id), Some(None));
        // The notice goes: the card goes.
        let route = islands.route_notice(Some(dp), None, now);
        assert!(!route.carded);
        assert_eq!(islands.island_of(dp).map(Island::notice_id), Some(None));
        assert!(!islands.any_notice());
    }

    #[test]
    fn a_closing_page_is_still_out_until_the_island_is_back_at_the_capsule() {
        let mut islands = islands();
        let dp = OutputId(1);
        let t = Instant::now();
        if let Some(slot) = islands.slots.iter_mut().find(|s| s.output == dp) {
            slot.island.open_for_test(PageId::CLOCK, t);
            slot.island
                .close_for_test(t + std::time::Duration::from_secs(1));
        }
        let closing = t + std::time::Duration::from_millis(1100);
        assert!(!islands.page_open_on(dp), "closed as far as the verb goes");
        assert!(islands.page_out_on(dp, closing), "but still shrinking");
        assert_eq!(islands.pages_out(closing), vec![dp]);
        let done = t + std::time::Duration::from_secs(3);
        assert!(!islands.page_out_on(dp, done));
        assert!(islands.pages_out(done).is_empty());
    }

    #[test]
    fn a_closing_page_is_a_page_for_the_levels_placement() {
        let mut islands = islands();
        let (dp, hdmi) = (OutputId(1), OutputId(2));
        let t = Instant::now();
        if let Some(slot) = islands.slots.iter_mut().find(|s| s.output == dp) {
            slot.island.open_for_test(PageId::CLOCK, t);
            slot.island
                .close_for_test(t + std::time::Duration::from_secs(1));
        }
        let closing = t + std::time::Duration::from_millis(1100);
        let place = islands.placement(Some(hdmi), closing);
        assert_eq!(place.pages, vec![dp], "still shrinking: still a page");
        assert_eq!(place.notices, Some(hdmi));
        let done = t + std::time::Duration::from_secs(3);
        assert!(islands.placement(None, done).pages.is_empty());
    }

    #[test]
    fn removing_an_output_releases_the_hold_of_its_card_once() {
        let mut islands = islands();
        let (dp, hdmi) = (OutputId(1), OutputId(2));
        let now = Instant::now();
        for slot in islands.slots.iter_mut() {
            slot.island.open_for_test(PageId::CLOCK, now);
            slot.island.set_notice(Some(toast(1)), now);
            slot.island.hover_card_for_test();
        }
        assert_eq!(islands.take_card_holds(), vec![true, true]);
        // Nothing to say about an output that held nothing.
        assert!(!islands.release_card_holds(OutputId(9)));
        // DP-1 goes with its pointer on the card: its hold must end.
        assert!(islands.release_card_holds(dp));
        assert!(!islands.release_card_holds(dp), "once");
        assert_eq!(islands.remove_output(dp), vec![SurfaceId(10)]);
        // The other Island still holds its own, and is not asked twice.
        assert!(islands.take_card_holds().is_empty());
        assert!(islands.release_card_holds(hdmi));
        assert!(!islands.release_card_holds(hdmi), "once");
    }

    #[test]
    fn the_page_opening_takes_the_capsule_back_and_the_surface_goes_without_a_shrink() {
        let mut islands = islands();
        let dp = OutputId(1);
        let now = Instant::now();
        let mut handover = Handover::new();
        assert_eq!(handover.frame(true), Some(Post::Lend));
        assert_eq!(islands.answer(handover.lend(true), dp), (true, false));
        assert!(islands.island_of(dp).is_some_and(Island::capsule_lent));
        assert!(!islands.page_open_on(dp));
        // The page opens on the lent Island: the capsule is the panel's in
        // the same turn.
        if let Some(slot) = islands.slots.iter_mut().find(|s| s.output == dp) {
            slot.island.open_for_test(PageId::CLOCK, now);
        }
        assert!(islands.page_open_on(dp));
        assert!(!islands.island_of(dp).is_some_and(Island::capsule_lent));
        // The surface retires: nothing is left to give back, and it is
        // released on the next turn whatever it still wants to show.
        assert!(islands.retire(&mut handover, dp), "its panel is redrawn");
        assert!(handover.release(false));
        // The panel has to be able to lend again for the next notice.
        let mut next = Handover::new();
        assert_eq!(next.frame(true), Some(Post::Lend));
        assert_eq!(islands.answer(next.lend(true), dp), (true, false));
        assert!(
            !islands.island_of(dp).is_some_and(Island::capsule_lent),
            "an open page is never lent out"
        );
    }

    #[test]
    fn the_lent_outputs_panel_skips_the_redraw_a_snapshot_change_would_ask() {
        let islands = islands();
        assert_eq!(
            islands.redraw_targets(None),
            vec![SurfaceId(10), SurfaceId(20)]
        );
        // The panel the lend and the return redraw anyway is left out;
        // another output's panel redraws as before.
        assert_eq!(
            islands.redraw_targets(Some(OutputId(1))),
            vec![SurfaceId(20)]
        );
        assert_eq!(
            islands.redraw_targets(Some(OutputId(2))),
            vec![SurfaceId(10)]
        );
    }

    fn pending(item: u32) -> Option<PendingMenu> {
        Some(PendingMenu {
            surface: SurfaceId(10),
            item: self::item(item),
            row: Rect::whole((1.0, 1.0)),
            keyboard: false,
            until: None,
        })
    }

    fn item(n: u32) -> ItemId {
        ItemId {
            bus_name: format!(":1.{n}"),
            path: "/StatusNotifierItem".to_owned(),
        }
    }

    #[test]
    fn empty_root_waits_for_one_layout_update() {
        let t0 = Instant::now();
        let mut request = pending(1);
        // The empty root keeps the request, with the whole wait left...
        assert!(matches!(
            Pending::of(&mut request, &item(1), true, t0),
            Pending::Wait(left) if left == MENU_LATE_LAYOUT
        ));
        // ...a second empty one does not restart the wait...
        let t1 = t0 + Duration::from_millis(200);
        assert!(matches!(
            Pending::of(&mut request, &item(1), true, t1),
            Pending::Wait(left) if left == Duration::from_millis(300)
        ));
        // ...and the layout that brings entries opens it.
        let t2 = t0 + Duration::from_millis(250);
        assert!(matches!(
            Pending::of(&mut request, &item(1), false, t2),
            Pending::Open(_)
        ));
        assert!(request.is_none());
    }

    #[test]
    fn a_due_request_stays_for_its_card_until_the_message_takes_it() {
        let t0 = Instant::now();
        let mut request = pending(1);
        assert_eq!(Pending::due(&request, t0), None, "not waiting yet");
        assert!(matches!(
            Pending::of(&mut request, &item(1), true, t0),
            Pending::Wait(_)
        ));
        assert_eq!(
            Pending::due(&request, t0 + Duration::from_millis(499)),
            None
        );
        let at = t0 + MENU_LATE_LAYOUT;
        assert_eq!(Pending::due(&request, at), Some(item(1)));
        assert!(request.is_some(), "the request is kept for the card");
        // The host's `NoMenu` handling takes it.
        assert!(request.take_if(|p| p.item == item(1)).is_some());
    }

    #[test]
    fn empty_root_gives_up_after_500_ms() {
        let t0 = Instant::now();
        let mut request = pending(1);
        assert!(
            Pending::expired(&mut request, t0).is_none(),
            "not waiting yet"
        );
        assert!(matches!(
            Pending::of(&mut request, &item(1), true, t0),
            Pending::Wait(_)
        ));
        let before = t0 + Duration::from_millis(499);
        assert!(Pending::expired(&mut request, before).is_none());
        // The frame at the deadline drops it...
        let at = t0 + MENU_LATE_LAYOUT;
        assert!(Pending::expired(&mut request, at).is_some());
        assert!(request.is_none());
        // ...and so does an empty root that comes in late.
        let mut late = pending(1);
        assert!(matches!(
            Pending::of(&mut late, &item(1), true, t0),
            Pending::Wait(_)
        ));
        assert!(matches!(
            Pending::of(&mut late, &item(1), true, at),
            Pending::GiveUp(_)
        ));
        assert!(late.is_none());
    }

    #[test]
    fn a_request_for_another_item_gives_up_the_old_one() {
        let mut slot = None;
        assert!(Pending::replace(&mut slot, pending(1).expect("a request")).is_none());
        // The same item again (two quick clicks): nothing to give up.
        assert!(Pending::replace(&mut slot, pending(1).expect("a request")).is_none());
        let old = Pending::replace(&mut slot, pending(2).expect("a request"));
        assert_eq!(old.map(|o| o.item), Some(item(1)));
        assert_eq!(slot.map(|p| p.item), Some(item(2)));
    }

    #[test]
    fn a_menu_nobody_asked_for_is_unasked() {
        let t0 = Instant::now();
        let mut request = pending(1);
        assert!(matches!(
            Pending::of(&mut request, &item(2), false, t0),
            Pending::Unasked
        ));
        assert!(request.is_some(), "another item's menu leaves it waiting");
        assert!(matches!(
            Pending::of(&mut None, &item(1), false, t0),
            Pending::Unasked
        ));
    }

    #[test]
    fn only_a_changed_layout_relays_the_bar_out() {
        let settings = Settings::default();
        assert_eq!(bar_change(&settings, &settings), BarChange::Nothing);
        let mut edited = settings.clone();
        edited.bar.end = Some(Vec::new());
        assert_eq!(bar_change(&settings, &edited), BarChange::Relayout);
        assert_eq!(bar_change(&edited, &settings), BarChange::Relayout);
        // A zone chosen equal to the default is the same layout.
        let mut chosen_end = settings.clone();
        chosen_end.bar.end = Some(vec![
            "tray".to_owned(),
            "network".to_owned(),
            "bluetooth".to_owned(),
            "volume".to_owned(),
            "brightness".to_owned(),
            "clipboard".to_owned(),
            "session".to_owned(),
        ]);
        assert_eq!(bar_change(&settings, &chosen_end), BarChange::Nothing);
        // Do-not-disturb and the duties are not the surfaces' business.
        let mut quiet = settings.clone();
        quiet.do_not_disturb = true;
        quiet.duties.osd = Duty::Off;
        assert_eq!(bar_change(&settings, &quiet), BarChange::Nothing);
    }

    #[test]
    fn a_layout_change_lays_the_bar_out_on_the_surfaces_the_islands_have() {
        let mut islands = islands();
        let mut only_tray = BarLayout::default();
        only_tray.start.clear();
        islands.set_bar(&only_tray);
        // Nothing is made or destroyed: the same two surfaces, on the same
        // outputs.
        assert_eq!(islands.len(), 2);
        assert_eq!(islands.surface_on(OutputId(1)), Some(SurfaceId(10)));
        assert_eq!(islands.surface_on(OutputId(2)), Some(SurfaceId(20)));
        assert_eq!(islands.output_of(SurfaceId(20)), Some(OutputId(2)));
        // Every Island received the layout.
        for surface in [SurfaceId(10), SurfaceId(20)] {
            let island = islands.get_mut(surface).expect("the Island");
            assert_eq!(island.bar_layout(), Some(&only_tray), "{surface:?}");
        }
        // The same layout again changes nothing; a new one reaches both.
        islands.set_bar(&only_tray);
        islands.set_bar(&BarLayout::default());
        for surface in [SurfaceId(10), SurfaceId(20)] {
            let island = islands.get_mut(surface).expect("the Island");
            assert_eq!(island.bar_layout(), Some(&BarLayout::default()));
        }
    }

    #[test]
    fn the_surface_is_the_whole_top_edge_with_the_layout_in_force() {
        let mut settings = Settings::default();
        let (anchor, size, layout) = island_shape(&settings, 1920);
        assert_eq!((anchor, size), (Anchoring::TopWide, (1920, 560)));
        assert_eq!(layout, BarLayout::default());
        // Narrow outputs get their own width too; none, the stand-in.
        assert_eq!(island_shape(&settings, 640).1, (640, 560));
        assert_eq!(
            island_shape(&settings, 0).1,
            SURFACE_SIZE,
            "until the configure says"
        );
        // The layout handed over is the one in force.
        settings.bar.end = Some(Vec::new());
        assert_eq!(island_shape(&settings, 1920).2, settings.bar_layout());
        assert_eq!(island_shape(&settings, 1920).0, Anchoring::TopWide);
    }

    #[test]
    fn a_removed_output_drops_its_island_and_keeps_the_others() {
        let mut islands = islands();
        assert_eq!(islands.remove_output(OutputId(1)), vec![SurfaceId(10)]);
        assert_eq!(islands.len(), 1);
        assert!(islands.get_mut(SurfaceId(10)).is_none());
        assert_eq!(
            islands
                .get_mut(SurfaceId(20))
                .map(|i| i.output().to_owned()),
            Some("HDMI-A-1".to_owned())
        );
        assert!(islands.remove_output(OutputId(9)).is_empty());
        assert_eq!(islands.remove_output(OutputId(2)), vec![SurfaceId(20)]);
        assert!(islands.is_empty());
        assert_eq!(islands.target(None, None), None);
    }

    #[test]
    fn a_verb_goes_to_the_keyboards_island_then_niris_focused_output_then_the_first() {
        let islands = islands();
        assert_eq!(
            islands.target(Some(SurfaceId(20)), Some("DP-1")),
            Some(SurfaceId(20))
        );
        assert_eq!(islands.target(None, Some("HDMI-A-1")), Some(SurfaceId(20)));
        assert_eq!(islands.target(None, Some("DP-9")), Some(SurfaceId(10)));
        assert_eq!(islands.target(None, None), Some(SurfaceId(10)));
        // A keyboard surface that is not an Island's is ignored.
        assert_eq!(
            islands.target(Some(SurfaceId(99)), None),
            Some(SurfaceId(10))
        );
    }

    #[test]
    fn the_menu_shows_the_setting_and_disables_it_while_the_name_is_held() {
        let settings = ShellSettings::load_from(None);
        let free = menu_state(&settings, &NameOwner::default());
        assert_eq!(
            free,
            MenuState {
                do_not_disturb: false,
                dnd_held: false
            }
        );
        let held = NameOwner {
            notifications: Some("org.freedesktop.Notifications is held by :1.42 (noctalia)".into()),
            tray: None,
        };
        assert!(menu_state(&settings, &held).dnd_held);
        // The tray's holder does not concern do-not-disturb.
        let tray = NameOwner {
            notifications: None,
            tray: Some(crate::tray::NameHolder {
                unique: ":1.7".into(),
                comm: None,
            }),
        };
        assert!(!menu_state(&settings, &tray).dnd_held);
    }

    #[test]
    fn the_notifications_holder_replaces_its_name_and_keeps_the_trays() {
        let tray = crate::tray::NameHolder {
            unique: ":1.7".into(),
            comm: None,
        };
        let current = NameOwner {
            notifications: None,
            tray: Some(tray.clone()),
        };
        let held = with_notifications_owner(&current, Some(":1.42 (noctalia)"));
        assert_eq!(held.notifications.as_deref(), Some(":1.42 (noctalia)"));
        assert_eq!(held.tray.as_ref(), Some(&tray));
        let mut owners = current.clone();
        assert!(owners.replace(held.clone()));
        assert!(!owners.replace(held));
        let freed = with_notifications_owner(&owners, None);
        assert_eq!(freed.notifications, None);
        assert_eq!(freed.tray, Some(tray));
    }

    #[test]
    fn a_new_menu_starts_from_an_empty_choice_slot() {
        let slot = ChoiceSlot::default();
        slot.set(Some(MenuChoice::Quit));
        let handed = fresh_choice(&slot);
        assert!(Rc::ptr_eq(&slot, &handed), "the menu writes the app's slot");
        assert_eq!(slot.take(), None, "the stale quit is gone");
        handed.set(Some(MenuChoice::ReopenPage));
        assert_eq!(slot.take(), Some(MenuChoice::ReopenPage));
    }

    #[test]
    fn niris_focused_output_is_the_focused_workspaces() {
        let ws = |id: u64, output: &str, focused: bool| Workspace {
            id,
            idx: 1,
            name: None,
            output: Some(output.to_owned()),
            is_active: true,
            is_focused: focused,
            columns: 0,
        };
        let model = NiriModel {
            workspaces: vec![ws(1, "DP-1", false), ws(2, "HDMI-A-1", true)],
            ..NiriModel::default()
        };
        assert_eq!(focused_output(Some(&model)), Some("HDMI-A-1"));
        assert_eq!(focused_output(Some(&NiriModel::default())), None);
        assert_eq!(focused_output(None), None);
    }

    #[test]
    fn hide_entry_moves_the_id_into_tray_hidden() {
        let dir = std::env::temp_dir().join(format!("cel-i-012-hide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let file = dir.join("shell.json");
        let mut settings = ShellSettings::load_from(Some(file.clone()));
        let before = settings.current().bar_layout().tray_hidden;
        assert!(!before.contains(&"steam".to_owned()));

        assert!(apply_shell_entry(&mut settings, "steam", ShellEntry::Hide).expect("written"));
        let mut want = before.clone();
        want.push("steam".to_owned());
        assert_eq!(settings.current().bar_layout().tray_hidden, want);
        // Hiding again changes nothing; showing removes it, the rest stays.
        assert!(!apply_shell_entry(&mut settings, "steam", ShellEntry::Hide).expect("same"));
        let on_disk = std::fs::read_to_string(&file).expect("the file");
        assert!(on_disk.contains("steam"));
        assert!(apply_shell_entry(&mut settings, "steam", ShellEntry::Show).expect("written"));
        assert_eq!(settings.current().bar_layout().tray_hidden, before);
        std::fs::remove_dir_all(&dir).ok();
    }
}
