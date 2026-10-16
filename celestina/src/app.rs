//! The app: the `celestina_wayland::Shell` of the product. It keeps one
//! Island per output on a `celestina-panel` layer surface, the one painter
//! of the process (created on the first frame of any surface, when the GL
//! context exists), the last niri model and the appearance, and routes the
//! workers' messages: the model to every Island, the appearance to the
//! painter and the trees, a verb to the Island of the focused output. It
//! also opens the Island's overflow menu and makes the menu's choices:
//! quit, do-not-disturb (the shell's own settings file) and reopen.

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
};

use celestina_paint::{Painter, TextSystem, Theme};
use celestina_settings::Appearance;
use celestina_wayland::{
    FrameCx, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputEvent, OutputId,
    RuntimeCtl, Shell, SurfaceCtl, SurfaceId,
};

use crate::{
    command::Verb,
    error_chain,
    island::{
        menu::{self, ChoiceSlot, Menu, MenuChoice, MenuState, SharedPainter},
        pages::NameOwner,
        Island, IslandAction,
    },
    message::Message,
    providers::{
        niri::{NiriModel, NiriProvider, NiriRequest},
        settings::ShellSettings,
    },
};

/// The Island's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-panel";
/// The Island's surface, logical px: the largest page plus its margin. The
/// surface never resizes; only its regions follow the Island (design §9).
pub const SURFACE_SIZE: (u32, u32) = (760, 560);

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

    fn iter(&self) -> impl Iterator<Item = &Slot> {
        self.slots.iter()
    }
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

/// The slot a new menu writes its choice into, emptied first: a choice
/// left by an earlier menu must never fire on a later, unrelated popup
/// close.
fn fresh_choice(slot: &ChoiceSlot) -> ChoiceSlot {
    slot.set(None);
    Rc::clone(slot)
}

/// The product's `Shell`.
pub struct App {
    islands: Islands,
    /// The fonts, until the painter takes them on the first frame.
    fonts: Option<TextSystem>,
    /// The one painter, shared with the open menu.
    painter: Option<SharedPainter>,
    appearance: Appearance,
    /// The shell's own settings (do-not-disturb).
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
    /// Why the run stopped on a failure, for the binary to report.
    fatal: Rc<OnceCell<String>>,
    refusal_logged: bool,
    metrics: bool,
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
    /// and the shell's settings read at start, the slot the niri provider
    /// is put in once it runs,
    /// the slot the app writes why it stopped the run into, and whether
    /// `--metrics` asked for the atlas's occupancy at exit.
    #[must_use]
    pub fn new(
        fonts: TextSystem,
        appearance: Appearance,
        settings: ShellSettings,
        niri: Rc<OnceCell<NiriProvider>>,
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
            fatal,
            refusal_logged: false,
            metrics,
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

    fn redraw_all(&self, ctl: &mut RuntimeCtl<'_>) {
        for slot in self.islands.iter() {
            if let Some(mut surface) = ctl.surface(slot.surface) {
                surface.request_redraw();
            }
        }
    }

    fn output_added(&mut self, info: &celestina_wayland::OutputInfo, ctl: &mut RuntimeCtl<'_>) {
        let width = match info.logical_size.0 {
            0 => SURFACE_SIZE.0,
            w => w.min(SURFACE_SIZE.0),
        };
        let island = Island::new(&info.name, (width as f32, SURFACE_SIZE.1 as f32));
        let spec = LayerSpec {
            output: info.id,
            namespace: NAMESPACE,
            layer: Layer::Top,
            anchor_top: true,
            size: (width, SURFACE_SIZE.1),
            exclusive_zone: island.exclusive_zone(),
            keyboard: KeyboardInteractivity::None,
        };
        match ctl.create_layer_surface(spec) {
            Ok(surface) => self.islands.add(info.id, surface, island),
            Err(err) => tracing::error!(output = %info.name, %err, "no Island on this output"),
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
            Verb::LauncherToggle | Verb::NotificationsToggle | Verb::NotificationsDismissAll => {
                tracing::info!(verb = %verb.words().join(" "), "not delivered yet (CEL-1-E, CEL-1-F)");
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

    /// Makes the overflow menu's choice, once its popup is gone.
    fn menu_chose(&mut self, choice: MenuChoice, surface: SurfaceId, ctl: &mut SurfaceCtl<'_>) {
        match choice {
            MenuChoice::Quit => {
                tracing::info!("quit chosen in the menu");
                ctl.stop();
            }
            // Only the setting changes: nothing honours it until the
            // notification server (CEL-1-F) reads it at its start and on
            // this choice, through its own do-not-disturb flag.
            MenuChoice::DoNotDisturb(on) => match self.settings.set_do_not_disturb(on) {
                Ok(changed) => {
                    tracing::info!(on, changed, "do-not-disturb chosen in the menu");
                    // The open page shows the switch: draw it again. Other
                    // outputs' Islands pick it up on their next frame.
                    ctl.request_redraw();
                }
                Err(err) => tracing::warn!(on, "do-not-disturb not changed: {}", error_chain(&err)),
            },
            MenuChoice::ReopenPage => {
                if let Some(island) = self.islands.get_mut(surface) {
                    island.reopen(ctl);
                }
            }
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

impl Shell for App {
    type Message = Message;

    fn output(&mut self, ev: OutputEvent, ctl: &mut RuntimeCtl<'_>) {
        match ev {
            OutputEvent::Added(info) => self.output_added(&info, ctl),
            OutputEvent::Changed(info) => {
                tracing::debug!(output = %info.name, "output changed; the Island keeps its surface");
            }
            OutputEvent::Removed(output) => {
                let gone = self.islands.remove_output(output);
                if self.keyboard.is_some_and(|k| gone.contains(&k)) {
                    self.keyboard = None;
                }
                tracing::info!(
                    output = output.0,
                    islands = self.islands.len(),
                    "output gone with its Island"
                );
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
                Ok(painter) => self.painter = Some(Rc::new(RefCell::new(painter))),
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
        island.frame(
            cx,
            &mut painter,
            self.model.as_ref(),
            &self.owners,
            self.settings.current().do_not_disturb,
            self.appearance.reduced_motion,
        );
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) {
        let surface = ctl.id();
        match ev {
            InputEvent::FocusEnter => self.keyboard = Some(surface),
            InputEvent::FocusLost if self.keyboard == Some(surface) => self.keyboard = None,
            _ => {}
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
        self.act(actions, surface, &shared, ctl);
        // The menu is gone: a row chosen in it is made now. The Island has
        // one popup at a time, so the slot is that popup's.
        if closed {
            if let Some(choice) = self.menu_choice.take() {
                self.menu_chose(choice, surface, ctl);
            }
        }
    }

    fn message(&mut self, msg: Message, ctl: &mut RuntimeCtl<'_>) {
        match msg {
            Message::Niri(model) => {
                for slot in self.islands.iter() {
                    if slot.island.shows_a_change(Some(&model)) {
                        if let Some(mut surface) = ctl.surface(slot.surface) {
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
            Message::Command(verb) => self.command(verb, ctl),
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
            Island::new("DP-1", (760.0, 560.0)),
        );
        islands.add(
            OutputId(2),
            SurfaceId(20),
            Island::new("HDMI-A-1", (760.0, 560.0)),
        );
        islands
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
            tray: Some(":1.7".into()),
        };
        assert!(!menu_state(&settings, &tray).dnd_held);
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
