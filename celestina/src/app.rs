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
//! the bar's layout, and the `bar` duty, which moves every Island to a
//! surface of the other shape.

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

use celestina_core::desktop_entry::DesktopEntry;
use celestina_paint::{Painter, Rect, TextSystem, Theme};
use celestina_settings::Appearance;
use celestina_shell::phone::PhoneState;
use celestina_shell::settings::{BarLayout, Duty, Settings};
use celestina_wayland::{
    Anchoring, FrameCx, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputEvent, OutputId,
    OutputInfo, PopupId, RuntimeCtl, RuntimeError, Sender, Shell, SurfaceCtl, SurfaceId,
};

use crate::notifications::banners::{self, BannerAction, BannerStack};
use crate::{
    bar,
    bubbles::{self, Bubbles, BubblesHandle, BubblesNews},
    clipboard::{ClipboardDuty, ClipboardMessage, ClipboardState},
    command::{Presence, Verb},
    connectivity::{self, state::ConnectivityState, ConnectivityHandle, Forwarded},
    energy::{self, EnergyHandle},
    error_chain,
    glance::{self, GlanceHandle},
    island::{
        self,
        menu::{self, ChoiceSlot, Menu, MenuChoice, MenuState, SharedPainter},
        pages::{tray::TrayList, NameOwner},
        Island, IslandAction, Shared,
    },
    launcher::{self, recent::RecentLaunches, Launcher, LauncherAction, Switch},
    lock::LockHandle,
    media::control::{route, MediaControl, Routed, Snapshot as MediaSnapshot},
    message::Message,
    notifications::{self, NotificationsState},
    osd::{
        control::{self, Controls, Opening},
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
        menu::{AskSink, Chain, ChoiceSlot as TrayChoiceSlot, MenuContext, MenuModel, TrayMenu},
        ItemId, MenuTree, TrayCommand, TrayEvent,
    },
};

/// The Island's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-panel";
/// The Island's surface, logical px: the largest page plus its margin. The
/// surface never resizes; only its regions follow the Island (design §9).
/// With the bar on it is as wide as the output and as tall.
pub const SURFACE_SIZE: (u32, u32) = (island::state::STRIP_W as u32, 560);

/// The shape of an Island's surface on an output `logical_w` px wide (0
/// when the output has not said) under `settings`: its anchoring, its
/// logical size, and the bar's layout while the `bar` duty is on. Off, the
/// 1.0.0 strip; on, the whole top edge.
fn island_shape(settings: &Settings, logical_w: u32) -> (Anchoring, (u32, u32), Option<BarLayout>) {
    let width = match logical_w {
        0 => SURFACE_SIZE.0,
        w => w,
    };
    match settings.duties.bar() {
        Duty::On => (
            bar::layout::anchoring(),
            (width, SURFACE_SIZE.1),
            Some(settings.bar_layout()),
        ),
        Duty::Off => (
            Anchoring::Top,
            (width.min(SURFACE_SIZE.0), SURFACE_SIZE.1),
            None,
        ),
    }
}

/// The Island's layer surface on `output`, of the shape [`island_shape`]
/// gave. Its namespace is the shell's own, and no bus name is involved: the
/// `bar` duty has no observable owner, so nothing probes for another bar.
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
    /// The bar stays on with another layout: each Island lays it out again.
    Relayout,
    /// The `bar` duty flipped: each Island moves to a surface of the other
    /// shape.
    Reshape,
}

fn bar_change(before: &Settings, after: &Settings) -> BarChange {
    if before.duties.bar() != after.duties.bar() {
        BarChange::Reshape
    } else if after.duties.bar() == Duty::On && before.bar_layout() != after.bar_layout() {
        BarChange::Relayout
    } else {
        BarChange::Nothing
    }
}

/// What moving the Islands to new surfaces needs of the runtime: the app's
/// is its `RuntimeCtl`; a test records the calls.
trait Reshaper {
    fn create(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError>;
    /// Closes `island`'s menus and page through its `surface` before it is
    /// destroyed; whether a menu was open.
    fn detach(&mut self, surface: SurfaceId, island: &mut Island) -> bool;
    fn destroy(&mut self, surface: SurfaceId);
}

impl Reshaper for RuntimeCtl<'_> {
    fn create(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        self.create_layer_surface(spec)
    }

    fn detach(&mut self, surface: SurfaceId, island: &mut Island) -> bool {
        self.surface(surface)
            .is_some_and(|mut ctl| island.detach(&mut ctl))
    }

    fn destroy(&mut self, surface: SurfaceId) {
        self.destroy_surface(surface);
    }
}

/// What [`Islands::reshape`] did: the surfaces it destroyed, and whether a
/// menu was open on one of them.
#[derive(Debug, Default)]
struct Reshaped {
    gone: Vec<SurfaceId>,
    menus_closed: bool,
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
    /// The output's logical width, which a new surface's shape depends on.
    logical_w: u32,
    surface: SurfaceId,
    island: Island,
}

/// The Islands by output and surface.
#[derive(Debug, Default)]
pub struct Islands {
    slots: Vec<Slot>,
}

impl Islands {
    pub fn add(&mut self, output: OutputId, logical_w: u32, surface: SurfaceId, island: Island) {
        self.slots.push(Slot {
            output,
            logical_w,
            surface,
            island,
        });
    }

    /// `output` is now `logical_w` px wide.
    pub fn set_logical_w(&mut self, output: OutputId, logical_w: u32) {
        for slot in self.slots.iter_mut().filter(|s| s.output == output) {
            slot.logical_w = logical_w;
        }
    }

    /// Moves every Island to a surface of the shape `settings` asks for
    /// ([`island_shape`]: `Anchoring::Top` with the duty off, `TopWide` on).
    /// The new surface is made before the old one is destroyed, so an
    /// Island whose new surface cannot be made keeps the old one. The Island
    /// itself is kept ([`Island::detach`]): its menus and its open page
    /// close, and the page it last showed and its motion setting go on; its
    /// first frame on the new surface brings the configured size. It runs
    /// at most once a second: the follower delivers a flip at most once per
    /// [`crate::providers::settings::RESHAPE_GAP`], at its latest value.
    fn reshape(&mut self, settings: &Settings, runtime: &mut dyn Reshaper) -> Reshaped {
        let mut reshaped = Reshaped::default();
        for slot in &mut self.slots {
            let (anchor, size, bar) = island_shape(settings, slot.logical_w);
            let spec = island_layer(slot.output, anchor, size, slot.island.exclusive_zone());
            let new = match runtime.create(spec) {
                Ok(new) => new,
                Err(err) => {
                    tracing::error!(output = %slot.island.output(), %err, "the Island keeps its surface: none of the other shape");
                    continue;
                }
            };
            let old = slot.surface;
            reshaped.menus_closed |= runtime.detach(old, &mut slot.island);
            slot.island.set_bar(bar.as_ref());
            runtime.destroy(old);
            slot.surface = new;
            // The smoke reads this line from stderr.
            tracing::info!(output = %slot.island.output(), bar = bar.is_some(), ?anchor, ?size, old = old.0, new = new.0, "island surface reshaped");
            reshaped.gone.push(old);
        }
        reshaped
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

    /// The exclusive zone of `output`'s Island.
    fn zone_of(&self, output: OutputId) -> Option<i32> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .map(|s| s.island.exclusive_zone())
    }

    /// The surface of `output`'s Island.
    fn surface_of(&self, output: OutputId) -> Option<SurfaceId> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .map(|s| s.surface)
    }
}

/// The banners' surface and the output it is on.
#[derive(Debug)]
struct BannerSurface {
    output: OutputId,
    surface: SurfaceId,
    stack: BannerStack,
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
    recent: RecentLaunches,
    /// The loop's own sender, set once the loop runs: the launcher's close
    /// is posted through it (see [`Message::LauncherClosed`]), and so is a
    /// banner's click asking for the page (see
    /// [`Message::OpenNotificationsPage`]), because an input handler
    /// controls only its own surface.
    sender: Rc<OnceCell<Sender<Message>>>,
    /// The notification server's latest snapshot, holder and queue.
    notifications: NotificationsState,
    /// The banners' surface, while a banner shows.
    banners: Option<BannerSurface>,
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
    /// The menu the last right click asked for.
    pending_menu: Option<PendingMenu>,
    /// The tray menu chain open right now.
    open_tray_menu: Option<OpenTrayMenu>,
    tray_refusal_logged: bool,
    /// The wallpaper duty: its surfaces, textures and requests.
    wallpapers: crate::wallpaper::surface::Wallpapers,
    /// The energy duties' workers, set once the loop runs.
    energy: Rc<OnceCell<EnergyHandle>>,
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
    /// slider popup that is open.
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
            recent: RecentLaunches::default(),
            sender: Rc::new(OnceCell::new()),
            notifications: NotificationsState::new(notifications),
            banners: None,
            fatal,
            refusal_logged: false,
            metrics,
            tray: Rc::new(OnceCell::new()),
            tray_items: TrayList::default(),
            tray_choice: TrayChoiceSlot::default(),
            pending_menu: None,
            open_tray_menu: None,
            tray_refusal_logged: false,
            wallpapers: crate::wallpaper::surface::Wallpapers::default(),
            energy: Rc::new(OnceCell::new()),
            lock: Rc::new(OnceCell::new()),
            glance: Rc::new(OnceCell::new()),
            sysmon: None,
            weather: None,
            clipboard: Rc::new(OnceCell::new()),
            clipboard_state: ClipboardState::default(),
            connectivity: ConnectivityState::default(),
            connectivity_handle: Rc::new(RefCell::new(None)),
            osds: crate::osd::surface::Osds::new(
                Rc::new(OnceCell::new()),
                &Settings::default(),
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
                match wake {
                    island::cadence::Wake::Nothing => {}
                    island::cadence::Wake::Now => surface.request_redraw(),
                    island::cadence::Wake::After(wait) => surface.redraw_after(wait),
                }
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

    /// The launcher's parts: the `recent` launches read at start and the
    /// slot the loop's own sender is put in once the loop runs.
    #[must_use]
    pub fn with_launcher(
        mut self,
        recent: RecentLaunches,
        sender: Rc<OnceCell<Sender<Message>>>,
    ) -> App {
        self.recent = recent;
        self.sender = sender;
        self
    }

    /// Takes the slot the tray host's command queue is put in once it runs.
    #[must_use]
    pub fn with_tray(mut self, tray: Rc<OnceCell<TrayProvider>>) -> App {
        self.tray = tray;
        self
    }

    /// Takes the slot the wallpaper worker's queue is put in once it runs.
    #[must_use]
    pub fn with_wallpaper(
        mut self,
        provider: Rc<OnceCell<crate::providers::wallpaper::WallpaperProvider>>,
    ) -> App {
        self.wallpapers = crate::wallpaper::surface::Wallpapers::new(provider);
        self
    }

    /// `shell.json` changed on disk ([`Message::ShellSettings`]): its value
    /// is the one in force, and what changed is applied live, never by a
    /// restart: the `bar` duty and the bar's layout, the duties (wallpaper,
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
        tracing::info!(
            ?bar,
            bar_on = after.duties.bar() == Duty::On,
            "shell settings changed on disk"
        );
        match bar {
            BarChange::Reshape => self.reshape_islands(ctl),
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
        if before.duties.lock != after.duties.lock {
            if let Some(handle) = self.lock.get() {
                handle.apply(&after);
            }
        }
        // Every change: the workers follow the bar duty, the layout and the
        // location, and an apply that changes nothing is free; it is also
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
        self.osds.settings_changed(&before, &after, ctl, &theme);
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

    /// The `osd` duty or the bar's layout changed: with the duty off the
    /// readings are forgotten (so the pieces vanish) and an open slider popup
    /// closes; the DDC worker is asked to detect once a layout holds the
    /// brightness piece.
    fn levels_settings_changed(
        &mut self,
        before: &Settings,
        after: &Settings,
        ctl: &mut RuntimeCtl<'_>,
    ) {
        if before.duties.osd == Duty::On && after.duties.osd == Duty::Off {
            if let Some(surface) = self.controls.open_surface() {
                if let (Some(island), Some(mut surface_ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
                    island.close_level_popup(&mut surface_ctl);
                }
            }
            if let Some(released) = self.controls.closed() {
                let theme = self.theme();
                self.osds.set_suppressed(released, ctl, &theme);
            }
            if self.controls.readings.clear() {
                self.levels_changed(ctl);
            }
        }
        self.request_brightness_consumer();
    }

    /// Tells the duty's runtime whether the settings in force give the DDC
    /// worker a consumer; it asks the worker to detect only when one
    /// appears, so calling this on every settings change and every output
    /// added costs nothing.
    fn request_brightness_consumer(&self) {
        if let Some(runtime) = self.osds.runtime().get() {
            runtime.set_brightness_consumer(control::brightness_consumer(self.settings.current()));
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
    /// popup shows the new levels.
    fn levels_changed(&mut self, ctl: &mut RuntimeCtl<'_>) {
        self.redraw_all(ctl);
        if let Some((_, popup)) = self.controls.refresh() {
            if let Some(mut surface) = ctl.popup_surface(popup).and_then(|s| ctl.surface(s)) {
                surface.request_redraw();
            }
        }
    }

    /// The slider popup is gone (seen from an input call, which controls
    /// only its own surface): the loop is told to let the display show again.
    fn level_popup_closed(&mut self) {
        if let Some(released) = self.controls.closed() {
            self.post(Message::OsdSuppressed(released));
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
        self.redraw_all(ctl);
        self.sync_banners(ctl);
    }

    /// The banners follow the snapshot's toasts: the surface is created on
    /// the focused output when the first banner shows, redrawn when the
    /// stack changes and destroyed when none remains.
    fn sync_banners(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let toasts = &self.notifications.snapshot().toasts;
        if banners::shown(toasts).is_empty() {
            if let Some(gone) = self.banners.take() {
                ctl.destroy_surface(gone.surface);
                self.banners_gone(gone);
                tracing::info!("banners surface released: no toast left");
            }
            return;
        }
        if self.banners.is_none() {
            self.banners = self.create_banners(ctl);
            // The smoke reads this line from stderr.
            tracing::info!(
                created = self.banners.is_some(),
                toasts = self.notifications.snapshot().toasts.len(),
                "banners surface requested"
            );
        }
        let toasts = &self.notifications.snapshot().toasts;
        let Some(banners) = self.banners.as_mut() else {
            return;
        };
        if banners.stack.set_toasts(toasts) {
            if let Some(mut surface) = ctl.surface(banners.surface) {
                surface.request_redraw();
            }
        }
    }

    /// The banners' surface on the output niri has focused (else the
    /// first), under that output's Island.
    fn create_banners(&self, ctl: &mut RuntimeCtl<'_>) -> Option<BannerSurface> {
        let output = self
            .islands
            .banner_output(focused_output(self.model.as_ref()))?;
        let zone = self.islands.zone_of(output)?;
        let theme = self.theme();
        let spec = LayerSpec {
            output,
            namespace: banners::NAMESPACE,
            layer: Layer::Overlay,
            anchor: Anchoring::Top,
            size: banners::surface_size(&theme, zone),
            // The stack is painted under the Island inside the surface;
            // the surface itself ignores every exclusive zone.
            exclusive_zone: -1,
            keyboard: KeyboardInteractivity::None,
        };
        match ctl.create_layer_surface(spec) {
            Ok(surface) => Some(BannerSurface {
                output,
                surface,
                stack: BannerStack::new(
                    &theme,
                    banners::top_offset(&theme, zone),
                    self.appearance.reduced_motion,
                ),
            }),
            Err(err) => {
                tracing::error!(output = output.0, %err, "no banners on this output");
                None
            }
        }
    }

    /// The banners' surface is gone: a hover it held ends with it, so the
    /// server's expiry runs again.
    fn banners_gone(&mut self, mut gone: BannerSurface) {
        if let Some(command) = gone.stack.release_hover() {
            self.notifications.send(command);
        }
    }

    /// What a gesture on the banners asks for.
    fn banners_act(&mut self, actions: Vec<BannerAction>, output: OutputId) {
        for action in actions {
            match action {
                BannerAction::Send(command) => {
                    self.notifications.send(command);
                }
                BannerAction::OpenPage => match self.sender.get() {
                    Some(sender) => {
                        if sender.send(Message::OpenNotificationsPage(output)).is_err() {
                            tracing::debug!("the loop is gone: the page is not opened");
                        }
                    }
                    None => tracing::debug!("no loop sender: the page is not opened"),
                },
            }
        }
    }

    /// Opens the Notifications page on `output`'s Island.
    fn open_notifications_page(&mut self, output: OutputId, ctl: &mut RuntimeCtl<'_>) {
        let Some(surface) = self.islands.surface_of(output) else {
            tracing::debug!(output = output.0, "the output is gone: no page to open");
            return;
        };
        if let (Some(island), Some(mut ctl)) = (self.islands.get_mut(surface), ctl.surface(surface))
        {
            island.command(&Verb::IslandOpen(PageId::NOTIFICATIONS), &mut ctl);
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
        island.set_bar(bar.as_ref());
        // The shape asked for, logged at creation.
        tracing::info!(output = %info.name, bar = bar.is_some(), ?anchor, ?size, "island surface requested");
        let spec = island_layer(info.id, anchor, size, island.exclusive_zone());
        match ctl.create_layer_surface(spec) {
            Ok(surface) => self
                .islands
                .add(info.id, info.logical_size.0, surface, island),
            Err(err) => tracing::error!(output = %info.name, %err, "no Island on this output"),
        }
        // The new Island draws the media state in force.
        let draw = self.media_sync();
        Self::redraw_surfaces(ctl, &draw);
        // A toast that arrived before any Island existed has no banner yet.
        self.sync_banners(ctl);
    }

    /// The bar's layout changed while it is on: every Island lays it out
    /// again on its next frame; an open page stays as it is.
    fn relayout_bar(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let layout = self.settings.current().bar_layout();
        for slot in &mut self.islands.slots {
            slot.island.set_bar(Some(&layout));
        }
        self.redraw_all(ctl);
    }

    /// The `bar` duty flipped: every Island moves to a surface of the other
    /// shape ([`Islands::reshape`]), and the app forgets what it kept of the
    /// old surfaces.
    fn reshape_islands(&mut self, ctl: &mut RuntimeCtl<'_>) {
        let reshaped = self.islands.reshape(self.settings.current(), ctl);
        if reshaped.menus_closed {
            // Their popups went without a `PopupClosed`, so a row chosen in
            // them is never made.
            self.menu_choice.set(None);
            *self.tray_choice.borrow_mut() = None;
        }
        for old in reshaped.gone {
            self.island_surface_gone(old);
        }
    }

    /// The app's records of an Island surface destroyed while its Island
    /// lives on: the keyboard it held, a tray menu asked for or open there.
    fn island_surface_gone(&mut self, old: SurfaceId) {
        if self.keyboard == Some(old) {
            self.keyboard = None;
        }
        if self.pending_menu.as_ref().is_some_and(|p| p.surface == old) {
            self.pending_menu = None;
        }
        if self.controls.is_open_on(old) {
            self.level_popup_closed();
        }
        if self
            .open_tray_menu
            .as_ref()
            .is_some_and(|o| o.surface == old)
        {
            self.tray_menu_gone();
        }
    }

    fn command(&mut self, verb: Verb, ctl: &mut RuntimeCtl<'_>) {
        match verb {
            Verb::Quit => {
                tracing::info!("quit requested over the bus");
                ctl.stop();
            }
            Verb::IslandToggle | Verb::IslandOpen(_) | Verb::IslandClose => {
                let target = self
                    .islands
                    .target(self.keyboard, focused_output(self.model.as_ref()));
                let Some(surface) = target else {
                    tracing::info!(?verb, "no output: the verb has no Island");
                    return;
                };
                if let (Some(island), Some(mut ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
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
            Verb::NotificationsToggle => {
                let target = self
                    .islands
                    .target(self.keyboard, focused_output(self.model.as_ref()));
                let Some(surface) = target else {
                    tracing::info!(?verb, "no output: the verb has no Island");
                    return;
                };
                if let (Some(island), Some(mut ctl)) =
                    (self.islands.get_mut(surface), ctl.surface(surface))
                {
                    let toggle = notifications::toggle_verb(island.page());
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
        let Some((surface, output)) = self.overlay_surface(ctl) else {
            return;
        };
        let open = Launcher::new(
            surface,
            output,
            &self.theme(),
            self.appearance.reduced_motion,
            self.recent.ids().to_vec(),
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
                        self.recent.record(&entry.id);
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
            LauncherAction::Close => None,
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
        let Some(surface) = self.islands.surface_on(output).or_else(|| {
            self.islands
                .target(None, focused_output(self.model.as_ref()))
        }) else {
            return;
        };
        if let (Some(island), Some(mut ctl)) = (self.islands.get_mut(surface), ctl.surface(surface))
        {
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
                } => {
                    self.pending_menu = Some(PendingMenu {
                        surface,
                        item: item.clone(),
                        row,
                        keyboard,
                    });
                    self.tray_command(TrayCommand::OpenMenu(item));
                }
                IslandAction::Audio(_) | IslandAction::Brightness(_) => {
                    self.level_action(action, surface);
                }
                IslandAction::OpenLevelPopup { kind, anchor } => {
                    let opening = Opening {
                        surface,
                        painter: shared,
                        runtime: self.osds.runtime(),
                        reduced: self.appearance.reduced_motion,
                    };
                    let opened = self.islands.get_mut(surface).is_some_and(|island| {
                        self.controls.open(kind, anchor, island, ctl, &opening)
                    });
                    if opened {
                        self.post(Message::OsdSuppressed(Suppression::Open(
                            control::kinds_of(kind),
                        )));
                    }
                }
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

    /// What is on the Notifications page counts as seen: while an Island
    /// shows it, unread notifications are marked read.
    fn mark_read_if_viewed(&mut self) {
        if self.notifications.snapshot().unread > 0
            && self.islands.iter().any(|s| s.island.shows_notifications())
        {
            self.notifications.send(NotificationsCommand::MarkRead);
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
        let Some(pending) = self.pending_menu.take_if(|p| p.item == *item) else {
            tracing::debug!("a menu nobody asked for is dropped");
            return;
        };
        if menu.entries.is_empty() {
            tracing::debug!("the item's menu is empty; nothing opens");
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
            Some(action) => self.launcher_acted(action, ctl),
            None => {}
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
    fn release_overview(&mut self, mut open: Overview) {
        match self.painter.as_ref().map(|p| p.try_borrow_mut()) {
            Some(Ok(mut painter)) => open.release(&mut painter),
            Some(Err(_)) => {
                tracing::warn!(
                    "overview: the painter is in use; the panel's textures are freed at the next frame"
                );
                self.overview.dying.push(open);
            }
            None => {}
        }
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

impl Shell for App {
    type Message = Message;

    fn output(&mut self, ev: OutputEvent, ctl: &mut RuntimeCtl<'_>) {
        match ev {
            OutputEvent::Added(info) => {
                self.outputs.retain(|o| o.id != info.id);
                self.outputs.push(info.clone());
                self.osds.output_added(&info);
                if self.controls.readings.output_added(info.id, &info.name) {
                    self.levels_changed(ctl);
                }
                self.request_brightness_consumer();
                self.output_added(&info, ctl);
                self.wallpapers.output_added(
                    &info,
                    self.settings.current(),
                    ctl,
                    self.painter.as_ref(),
                );
            }
            OutputEvent::Changed(info) => {
                if let Some(known) = self.outputs.iter_mut().find(|o| o.id == info.id) {
                    *known = info.clone();
                }
                if self.controls.readings.output_added(info.id, &info.name) {
                    self.levels_changed(ctl);
                }
                // The width a surface of the other shape will be made with.
                self.islands.set_logical_w(info.id, info.logical_size.0);
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
                    Ok(mut painter) => open.frame(cx, &mut painter, self.appearance.reduced_motion),
                    Err(_) => {
                        tracing::warn!("the painter is in use; the launcher's frame is skipped");
                        cx.ctl.request_redraw();
                    }
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
        if let (Some(shared), Some(banners)) = (self.painter.as_ref(), self.banners.as_mut()) {
            if banners.surface == cx.surface {
                match shared.try_borrow_mut() {
                    Ok(mut painter) => {
                        banners
                            .stack
                            .frame(cx, &mut painter, self.appearance.reduced_motion);
                    }
                    Err(_) => {
                        tracing::warn!("the painter is in use; the banners' frame is skipped");
                        cx.ctl.request_redraw();
                    }
                }
                return;
            }
        }
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
        island.frame(
            cx,
            &mut painter,
            self.model.as_ref(),
            &state,
            self.appearance.reduced_motion,
        );
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let surface = ctl.id();
        if let Some(banners) = self.banners.as_mut().filter(|b| b.surface == surface) {
            let Some(Ok(mut painter)) = self.painter.as_ref().map(|p| p.try_borrow_mut()) else {
                tracing::warn!("no painter at hand; the banners' input is dropped");
                return;
            };
            let now = std::time::Instant::now();
            banners.stack.layout(&mut painter, now);
            drop(painter);
            let input = banners.stack.pointer(&ev, now);
            if input.redraw {
                ctl.request_redraw();
            }
            let output = banners.output;
            self.banners_act(input.actions, output);
            return;
        }
        match ev {
            InputEvent::FocusEnter => self.keyboard = Some(surface),
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
        let (Some(shared), Some(island)) = (self.painter.clone(), self.islands.get_mut(surface))
        else {
            return;
        };
        let actions = match shared.try_borrow_mut() {
            Ok(mut painter) => island.input(ev, ctl, &mut painter),
            Err(_) => {
                tracing::warn!("the painter is in use; the input is dropped");
                return;
            }
        };
        let tray_menu_open = island.tray_menu_open();
        let level_popup_open = island.level_popup_open();
        self.act(actions, surface, &shared, ctl);
        self.mark_read_if_viewed();
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
            if !level_popup_open && self.controls.is_open_on(surface) {
                self.level_popup_closed();
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
            }
            Message::Appearance(appearance) => {
                if appearance == self.appearance {
                    return;
                }
                self.appearance = appearance;
                let theme = self.theme();
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
            }
            Message::OpenNotificationsPage(output) => {
                self.open_notifications_page(output, ctl);
                self.mark_read_if_viewed();
            }
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
            Message::Wallpaper(event) => {
                self.wallpapers
                    .message(event, &mut self.settings, ctl, self.painter.as_ref());
            }
            Message::ShellSettings(read) => self.shell_settings_read(*read, ctl),
            Message::Energy(event) => event.log(),
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
                self.osds
                    .audio(event, focused_output(self.model.as_ref()), ctl, &theme);
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
                self.osds
                    .brightness(event, focused_output(self.model.as_ref()), ctl, &theme);
            }
            Message::OsdSuppressed(suppression) => {
                let theme = self.theme();
                self.osds.set_suppressed(suppression, ctl, &theme);
            }
            Message::Osd(tick) => {
                let theme = self.theme();
                self.osds.tick(tick, ctl, &theme);
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
            1920,
            SurfaceId(10),
            Island::new("DP-1", (760.0, 560.0)),
        );
        islands.add(
            OutputId(2),
            2560,
            SurfaceId(20),
            Island::new("HDMI-A-1", (760.0, 560.0)),
        );
        islands
    }

    #[test]
    fn only_a_flipped_duty_reshapes_and_only_a_bar_that_is_on_relays_out() {
        let off = Settings::default();
        let mut on = off.clone();
        on.duties.bar = Some(Duty::On);
        assert_eq!(bar_change(&off, &on), BarChange::Reshape);
        assert_eq!(bar_change(&on, &off), BarChange::Reshape);
        assert_eq!(bar_change(&on, &on), BarChange::Nothing);

        let mut edited = on.clone();
        edited.bar.end = Some(Vec::new());
        assert_eq!(bar_change(&on, &edited), BarChange::Relayout);
        // A flip that also edits the layout is one reshape, with the new one.
        let mut off_edited = off.clone();
        off_edited.bar.end = Some(Vec::new());
        assert_eq!(bar_change(&off_edited, &on), BarChange::Reshape);
        // Off, the layout has nothing to lay out.
        assert_eq!(bar_change(&off, &off_edited), BarChange::Nothing);
        // An off the person wrote is the default off; a zone chosen equal to
        // the default is the same layout.
        let mut chosen_off = off.clone();
        chosen_off.duties.bar = Some(Duty::Off);
        assert_eq!(bar_change(&off, &chosen_off), BarChange::Nothing);
        let mut chosen_end = on.clone();
        chosen_end.bar.end = Some(vec![
            "tray".to_owned(),
            "network".to_owned(),
            "bluetooth".to_owned(),
            "volume".to_owned(),
            "brightness".to_owned(),
            "clipboard".to_owned(),
        ]);
        assert_eq!(bar_change(&on, &chosen_end), BarChange::Nothing);
        // Do-not-disturb is not the surfaces' business.
        let mut quiet = on.clone();
        quiet.do_not_disturb = true;
        assert_eq!(bar_change(&on, &quiet), BarChange::Nothing);
    }

    /// What the runtime was asked, in order.
    #[derive(Debug, PartialEq)]
    enum Op {
        Create(OutputId, Anchoring, (u32, u32)),
        Detach(SurfaceId),
        Destroy(SurfaceId),
    }

    /// A runtime that hands out surfaces from 100 and refuses `refuse`'s.
    struct FakeRuntime {
        ops: Vec<Op>,
        next: u32,
        refuse: Option<OutputId>,
        menu_open_on: Option<SurfaceId>,
    }

    impl Reshaper for FakeRuntime {
        fn create(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
            assert_eq!(spec.namespace, NAMESPACE);
            assert_eq!(spec.layer, Layer::Top);
            assert_eq!(spec.keyboard, KeyboardInteractivity::None);
            assert_eq!(spec.exclusive_zone, 48, "the zone does not change");
            self.ops
                .push(Op::Create(spec.output, spec.anchor, spec.size));
            if self.refuse == Some(spec.output) {
                return Err(RuntimeError::UnknownOutput(spec.output));
            }
            self.next += 1;
            Ok(SurfaceId(self.next))
        }

        fn detach(&mut self, surface: SurfaceId, _island: &mut Island) -> bool {
            self.ops.push(Op::Detach(surface));
            self.menu_open_on == Some(surface)
        }

        fn destroy(&mut self, surface: SurfaceId) {
            self.ops.push(Op::Destroy(surface));
        }
    }

    #[test]
    fn a_duty_flip_moves_each_island_to_a_surface_of_the_other_shape_and_back() {
        let mut islands = islands();
        let mut runtime = FakeRuntime {
            ops: Vec::new(),
            next: 100,
            refuse: None,
            menu_open_on: Some(SurfaceId(20)),
        };
        let mut settings = Settings::default();
        settings.duties.bar = Some(Duty::On);

        let on = islands.reshape(&settings, &mut runtime);
        // Each new surface exists before its old one is destroyed.
        assert_eq!(
            runtime.ops,
            [
                Op::Create(OutputId(1), Anchoring::TopWide, (1920, 560)),
                Op::Detach(SurfaceId(10)),
                Op::Destroy(SurfaceId(10)),
                Op::Create(OutputId(2), Anchoring::TopWide, (2560, 560)),
                Op::Detach(SurfaceId(20)),
                Op::Destroy(SurfaceId(20)),
            ]
        );
        assert_eq!(on.gone, [SurfaceId(10), SurfaceId(20)]);
        assert!(on.menus_closed);
        for (old, new) in [(10, 101), (20, 102)] {
            assert!(
                islands.get_mut(SurfaceId(old)).is_none(),
                "no Island on {old}"
            );
            assert!(islands.get_mut(SurfaceId(new)).is_some_and(|i| i.bar_on()));
        }
        assert_eq!(islands.len(), 2);
        assert_eq!(islands.output_of(SurfaceId(101)), Some(OutputId(1)));
        assert_eq!(islands.surface_on(OutputId(2)), Some(SurfaceId(102)));
        assert_eq!(islands.target(None, Some("HDMI-A-1")), Some(SurfaceId(102)));
        // A keyboard still naming a gone surface is ignored.
        assert_eq!(
            islands.target(Some(SurfaceId(20)), None),
            Some(SurfaceId(101))
        );

        // Back off, after the first output shrank, and the second output's
        // new surface cannot be made: it keeps the wide one, bar and all.
        islands.set_logical_w(OutputId(1), 640);
        runtime.ops.clear();
        runtime.refuse = Some(OutputId(2));
        runtime.menu_open_on = None;
        settings.duties.bar = Some(Duty::Off);
        let off = islands.reshape(&settings, &mut runtime);
        assert_eq!(
            runtime.ops,
            [
                Op::Create(OutputId(1), Anchoring::Top, (640, 560)),
                Op::Detach(SurfaceId(101)),
                Op::Destroy(SurfaceId(101)),
                Op::Create(OutputId(2), Anchoring::Top, (760, 560)),
            ]
        );
        assert_eq!(off.gone, [SurfaceId(101)]);
        assert!(!off.menus_closed);
        assert!(islands.get_mut(SurfaceId(103)).is_some_and(|i| !i.bar_on()));
        assert!(islands.get_mut(SurfaceId(102)).is_some_and(|i| i.bar_on()));
        assert_eq!(
            islands
                .get_mut(SurfaceId(103))
                .map(|i| i.output().to_owned()),
            Some("DP-1".to_owned())
        );
    }

    #[test]
    fn the_bar_duty_picks_the_surfaces_shape() {
        let mut settings = Settings::default();
        // Off (the default): the 1.0.0 strip, never wider than 760.
        assert_eq!(
            island_shape(&settings, 1920),
            (Anchoring::Top, (760, 560), None)
        );
        assert_eq!(island_shape(&settings, 640).1, (640, 560));
        assert_eq!(island_shape(&settings, 0).1, (760, 560));
        // On: the whole top edge, with the layout in force.
        settings.duties.bar = Some(Duty::On);
        let (anchor, size, layout) = island_shape(&settings, 1920);
        assert_eq!((anchor, size), (Anchoring::TopWide, (1920, 560)));
        assert_eq!(layout, Some(settings.bar_layout()));
        assert_eq!(
            island_shape(&settings, 0).1,
            (760, 560),
            "until the configure says"
        );
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
}
