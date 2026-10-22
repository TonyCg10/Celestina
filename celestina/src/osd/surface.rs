//! The on-screen display's surface (CEL-2-C, plan decisions 2, 3 and 9):
//! one `celestina-osd` layer surface in the `Overlay` layer, with no keyboard
//! and an empty input region, near the bottom edge (centred, or in the
//! bottom-right corner as `osd.position` says) of the output niri reports
//! focused, else the first output.
//!
//! [`Osds`] is the app's one handle on the display. The rules:
//!
//! - **Rest is nothing.** With the duty off, or on with nothing shown and the
//!   last exit over, there is no surface and so no frame.
//! - **Show.** A change the pure machine (`celestina_shell::osd::Osd`)
//!   announces creates the surface if there is none, or reuses the one on
//!   screen (also while it fades out): another kind replaces the content in
//!   place, the same kind updates it, and either re-arms the timeout. One
//!   display at a time; it stays on the output it was created on.
//! - **Exit.** The timeout, a kind switched off or a device that went away
//!   takes the content away: the card fades out on the motion contract's
//!   exit, and a retire timer destroys the surface once the fade is over (a
//!   surface cannot be destroyed from its own frame). A show before the
//!   retire fires keeps the surface; the stale retire is ignored by its
//!   generation.
//! - **Readings.** The first reading of each device is its baseline and shows
//!   nothing. Brightness is keyed per monitor: each monitor's first reading
//!   is its own baseline, and a monitor's change is a change even when
//!   another monitor sits at the same level. A reading of no level (a device
//!   gone, a failed read) forgets that baseline.
//! - **Duty off.** Any surface is destroyed at once, the baselines are
//!   forgotten and the workers stop (see [`super::OsdRuntime`]).
//!
//! The decisions are written against [`Host`], so they are tested without a
//! compositor or GL; [`Osds`]' public methods supply the real one.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use celestina_paint::Theme;
use celestina_shell::osd::{Effect, Kind, Osd, Reading, Shown};
use celestina_shell::settings::{Duty, OsdPosition, Settings};
use celestina_wayland::{
    Anchoring, FrameCx, KeyboardInteractivity, Layer, LayerSpec, OutputId, OutputInfo, RuntimeCtl,
    SurfaceId,
};

use super::view::{Metrics, OsdView};
use super::{OsdRuntime, OsdTick, Suppression, NAMESPACE};
use crate::island::menu::SharedPainter;
use crate::providers::audio::{AudioEvent, Device};
use crate::providers::brightness::BrightnessEvent;

/// What the display's decisions do to the world: the compositor and the
/// loop's timer.
pub trait Host {
    /// Creates the display's surface on `output` at `position`; `None` when
    /// it cannot.
    fn create(&mut self, output: OutputId, position: OsdPosition) -> Option<SurfaceId>;
    /// Destroys the display's surface.
    fn destroy(&mut self, surface: SurfaceId);
    /// Whether `surface` is still live: the compositor may have closed it
    /// (the runtime then destroyed it without telling the product).
    fn alive(&mut self, surface: SurfaceId) -> bool;
    /// Asks for a frame of `surface`.
    fn redraw(&mut self, surface: SurfaceId);
    /// Delivers `tick` to the loop `after` from now.
    fn arm(&mut self, after: Duration, tick: OsdTick);
}

/// The surface while it exists.
#[derive(Debug)]
struct Live {
    surface: SurfaceId,
    output: OutputId,
    /// What it shows; `None` while the card fades out.
    shown: Option<Shown>,
    /// The tree, made on the first frame (the painter's theme is needed).
    view: Option<OsdView>,
}

/// The on-screen display's state in the app.
#[derive(Debug)]
pub struct Osds {
    machine: Osd,
    duty: bool,
    position: OsdPosition,
    /// The last level per monitor; a monitor not here has no baseline yet.
    monitors: BTreeMap<String, u8>,
    /// The outputs, in announcement order: id and connector name.
    outputs: Vec<(OutputId, String)>,
    live: Option<Live>,
    /// The generation of the last retire timer; a newer show bumps it.
    retire: u64,
    /// How long the exit fade takes, with a frame's margin.
    exit: Duration,
    /// The origin of the machine's clock.
    epoch: Instant,
    /// The duty's workers and timer, set once the loop runs.
    runtime: Rc<OnceCell<OsdRuntime>>,
}

/// One frame at 60 Hz and a little: the retire waits this past the fade, so
/// the fade's last frame is drawn before the surface goes.
const RETIRE_MARGIN: Duration = Duration::from_millis(50);

impl Osds {
    /// The display under `settings`, before the loop runs; `runtime` is the
    /// slot its workers and timer are put in.
    #[must_use]
    pub fn new(runtime: Rc<OnceCell<OsdRuntime>>, settings: &Settings, theme: &Theme) -> Self {
        Self {
            machine: Osd::new(&settings.osd),
            duty: settings.duties.osd == Duty::On,
            position: settings.osd.position,
            monitors: BTreeMap::new(),
            outputs: Vec::new(),
            live: None,
            retire: 0,
            exit: Duration::from_millis(u64::from(theme.motion.exit)) + RETIRE_MARGIN,
            epoch: Instant::now(),
            runtime,
        }
    }

    /// Whether `surface` is the display's.
    #[must_use]
    pub fn owns(&self, surface: SurfaceId) -> bool {
        self.live.as_ref().is_some_and(|l| l.surface == surface)
    }

    /// The slot the duty's workers and timer are put in, for the bar's
    /// pieces and the verbs that command those workers.
    #[must_use]
    pub fn runtime(&self) -> &Rc<OnceCell<OsdRuntime>> {
        &self.runtime
    }

    /// Whether the display has a surface (for tests and the smoke's log).
    #[must_use]
    pub fn has_surface(&self) -> bool {
        self.live.is_some()
    }

    fn now(&self) -> Duration {
        self.epoch.elapsed()
    }

    // --- the app's hooks: each builds the real host and runs the decision.

    /// An output appeared.
    pub fn output_added(&mut self, info: &OutputInfo) {
        self.outputs.retain(|(id, _)| *id != info.id);
        self.outputs.push((info.id, info.name.clone()));
        if let Some(runtime) = self.runtime.get() {
            runtime.outputs_changed();
        }
    }

    /// An output's name may have changed.
    pub fn output_changed(&mut self, info: &OutputInfo) {
        if let Some(entry) = self.outputs.iter_mut().find(|(id, _)| *id == info.id) {
            entry.1.clone_from(&info.name);
        }
    }

    /// An output is gone, and the display's surface with it if it was there.
    pub fn output_removed(&mut self, output: OutputId) {
        self.removed(output);
        if let Some(runtime) = self.runtime.get() {
            runtime.outputs_changed();
        }
    }

    /// The settings in force changed from `before`: the workers follow the
    /// duty, and the display its switches.
    pub fn settings_changed(
        &mut self,
        before: &Settings,
        after: &Settings,
        ctl: &mut RuntimeCtl<'_>,
        theme: &Theme,
    ) {
        if before.duties.osd == after.duties.osd && before.osd == after.osd {
            return;
        }
        if let Some(runtime) = self.runtime.get() {
            runtime.apply(after);
        }
        let runtime = Rc::clone(&self.runtime);
        let mut host = AppHost::new(ctl, runtime.get(), theme);
        let now = self.now();
        self.apply(after, now, &mut host);
    }

    /// The audio worker's reading.
    pub fn audio(
        &mut self,
        event: AudioEvent,
        focused: Option<&str>,
        ctl: &mut RuntimeCtl<'_>,
        theme: &Theme,
    ) {
        let runtime = Rc::clone(&self.runtime);
        let mut host = AppHost::new(ctl, runtime.get(), theme);
        let now = self.now();
        self.audio_read(event, focused, now, &mut host);
    }

    /// A brightness worker's reading.
    pub fn brightness(
        &mut self,
        event: BrightnessEvent,
        focused: Option<&str>,
        ctl: &mut RuntimeCtl<'_>,
        theme: &Theme,
    ) {
        let runtime = Rc::clone(&self.runtime);
        let mut host = AppHost::new(ctl, runtime.get(), theme);
        let now = self.now();
        self.brightness_read(event, focused, now, &mut host);
    }

    /// The slider popup opened or closed: while it is open the kinds it
    /// drives show nothing and one of them on screen leaves, because the
    /// popup shows its own value; the readings it drives still move the
    /// baselines, and the levels it asked for that arrive after it closed
    /// are not announced either (see `celestina_shell::osd::Osd::release`).
    pub fn set_suppressed(
        &mut self,
        suppression: Suppression,
        ctl: &mut RuntimeCtl<'_>,
        theme: &Theme,
    ) {
        let runtime = Rc::clone(&self.runtime);
        let mut host = AppHost::new(ctl, runtime.get(), theme);
        let now = self.now();
        self.suppress(suppression, now, &mut host);
    }

    /// One of the display's timers came due, or the duty's reaper is done.
    pub fn tick(&mut self, tick: OsdTick, ctl: &mut RuntimeCtl<'_>, theme: &Theme) {
        if tick == OsdTick::Reaped {
            if let Some(runtime) = self.runtime.get() {
                runtime.resume();
            }
            return;
        }
        let runtime = Rc::clone(&self.runtime);
        let mut host = AppHost::new(ctl, runtime.get(), theme);
        self.ticked(tick, &mut host);
    }

    /// A frame of the display's surface.
    pub fn frame(&mut self, cx: &mut FrameCx<'_>, shared: &SharedPainter, reduced: bool) {
        let Some(live) = self.live.as_mut().filter(|l| l.surface == cx.surface) else {
            return;
        };
        let Ok(mut painter) = shared.try_borrow_mut() else {
            tracing::warn!("the painter is in use; the display's frame is skipped");
            cx.ctl.request_redraw();
            return;
        };
        let theme = painter.theme().clone();
        let view = live
            .view
            .get_or_insert_with(|| OsdView::new(&theme, reduced));
        view.frame(cx, &mut painter, live.shown, reduced);
    }

    // --- the decisions, against any host.

    fn apply(&mut self, settings: &Settings, now: Duration, host: &mut impl Host) {
        let on = settings.duties.osd == Duty::On;
        self.position = settings.osd.position;
        if !on {
            if self.duty {
                tracing::info!("osd duty off: no display, baselines forgotten");
            }
            self.duty = false;
            if let Some(live) = self.live.take() {
                host.destroy(live.surface);
            }
            // The next time the duty is on, every first reading is a
            // baseline again.
            self.machine = Osd::new(&settings.osd);
            self.monitors.clear();
            self.retire = self.retire.wrapping_add(1);
            return;
        }
        if !self.duty {
            self.duty = true;
            self.machine = Osd::new(&settings.osd);
            self.monitors.clear();
            return;
        }
        let effect = self.machine.configure(&settings.osd);
        self.effect(effect, None, now, host);
    }

    fn suppress(&mut self, suppression: Suppression, now: Duration, host: &mut impl Host) {
        match suppression {
            Suppression::Open(kinds) => {
                let effect = self.machine.suppress(kinds);
                self.effect(effect, None, now, host);
            }
            Suppression::Closed { kinds, asked } => self.machine.release(kinds, &asked, now),
        }
    }

    fn audio_read(
        &mut self,
        event: AudioEvent,
        focused: Option<&str>,
        now: Duration,
        host: &mut impl Host,
    ) {
        if !self.duty {
            return;
        }
        let kind = match event.device {
            Device::Output => Kind::Volume,
            Device::Input => Kind::Microphone,
        };
        let Some(level) = event.level else {
            self.forget(kind, host);
            return;
        };
        let reading = Reading {
            percent: level.percent,
            muted: level.muted,
        };
        let effect = self.machine.observe(kind, reading, now);
        self.effect(effect, focused, now, host);
    }

    fn brightness_read(
        &mut self,
        event: BrightnessEvent,
        focused: Option<&str>,
        now: Duration,
        host: &mut impl Host,
    ) {
        if !self.duty {
            return;
        }
        let Some(percent) = event.percent else {
            // Unknown is never 0: the monitor's next reading is a baseline.
            self.monitors.remove(&event.monitor);
            return;
        };
        let previous = self.monitors.insert(event.monitor, percent);
        let reading = Reading {
            percent,
            muted: false,
        };
        if previous.is_none() || previous == Some(percent) {
            self.machine.unchanged(Kind::Brightness, reading);
            return;
        }
        let effect = self.machine.changed(Kind::Brightness, reading, now);
        self.effect(effect, focused, now, host);
    }

    fn ticked(&mut self, tick: OsdTick, host: &mut impl Host) {
        match tick {
            OsdTick::Expire(generation) => {
                if self.machine.expire(generation) {
                    self.leave(host);
                }
            }
            OsdTick::Retire(generation) => {
                if generation != self.retire {
                    return;
                }
                if let Some(live) = self.live.take_if(|l| l.shown.is_none()) {
                    host.destroy(live.surface);
                    tracing::debug!("osd: surface released after its exit");
                }
            }
            // The runtime's, handled by `Osds::tick`.
            OsdTick::Reaped => {}
        }
    }

    fn removed(&mut self, output: OutputId) {
        self.outputs.retain(|(id, _)| *id != output);
        if self.live.as_ref().is_some_and(|l| l.output == output) {
            // The surface went with the output. The machine's timer still
            // ends what it shows; nothing is drawn meanwhile.
            self.live = None;
            self.retire = self.retire.wrapping_add(1);
        }
    }

    /// A device went away: its baseline is forgotten, and what it showed
    /// leaves.
    fn forget(&mut self, kind: Kind, host: &mut impl Host) {
        let showing = self.machine.shown().is_some();
        self.machine.forget(kind);
        if showing && self.machine.shown().is_none() {
            self.leave(host);
        }
    }

    fn effect(
        &mut self,
        effect: Effect,
        focused: Option<&str>,
        now: Duration,
        host: &mut impl Host,
    ) {
        match effect {
            Effect::None => {}
            Effect::Hide => self.leave(host),
            Effect::Show(shown, timer) => {
                self.show(shown, focused, host);
                host.arm(
                    timer.deadline.saturating_sub(now),
                    OsdTick::Expire(timer.generation),
                );
            }
        }
    }

    /// Puts `shown` on screen: on the surface there is (live), else on a new
    /// one on the focused output.
    fn show(&mut self, shown: Shown, focused: Option<&str>, host: &mut impl Host) {
        // A pending retire must not take the surface this show reuses.
        self.retire = self.retire.wrapping_add(1);
        if let Some(live) = self.live.as_mut() {
            if host.alive(live.surface) {
                live.shown = Some(shown);
                host.redraw(live.surface);
                return;
            }
            tracing::debug!("osd: the compositor closed the surface; a new one is made");
            self.live = None;
        }
        let Some(output) = self.target(focused) else {
            tracing::debug!("osd: no output to show on");
            return;
        };
        match host.create(output, self.position) {
            Some(surface) => {
                self.live = Some(Live {
                    surface,
                    output,
                    shown: Some(shown),
                    view: None,
                });
                host.redraw(surface);
            }
            None => tracing::debug!("osd: no surface; nothing shown"),
        }
    }

    /// The content goes: the card fades out and the surface is retired
    /// after the fade.
    fn leave(&mut self, host: &mut impl Host) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        live.shown = None;
        host.redraw(live.surface);
        self.retire = self.retire.wrapping_add(1);
        host.arm(self.exit, OsdTick::Retire(self.retire));
    }

    /// The output to show on: niri's focused one when it is known here,
    /// else the first.
    fn target(&self, focused: Option<&str>) -> Option<OutputId> {
        focused
            .and_then(|name| self.outputs.iter().find(|(_, n)| n == name))
            .or_else(|| self.outputs.first())
            .map(|(id, _)| *id)
    }
}

/// The real host: the runtime's surfaces and the loop's timer.
struct AppHost<'a, 'b> {
    ctl: &'a mut RuntimeCtl<'b>,
    runtime: Option<&'a OsdRuntime>,
    theme: &'a Theme,
}

impl<'a, 'b> AppHost<'a, 'b> {
    fn new(ctl: &'a mut RuntimeCtl<'b>, runtime: Option<&'a OsdRuntime>, theme: &'a Theme) -> Self {
        Self {
            ctl,
            runtime,
            theme,
        }
    }
}

impl Host for AppHost<'_, '_> {
    fn create(&mut self, output: OutputId, position: OsdPosition) -> Option<SurfaceId> {
        // Above every window, sized to the card and its margin, under other
        // surfaces' exclusive zones (0), no keyboard, and the empty input
        // region every surface starts with (the frame keeps it empty).
        let spec = LayerSpec {
            output,
            namespace: NAMESPACE,
            layer: Layer::Overlay,
            anchor: match position {
                OsdPosition::Bottom => Anchoring::Bottom,
                OsdPosition::BottomRight => Anchoring::BottomRight,
            },
            size: Metrics::of(self.theme).surface(),
            exclusive_zone: 0,
            keyboard: KeyboardInteractivity::None,
        };
        match self.ctl.create_layer_surface(spec) {
            Ok(surface) => {
                // The smoke reads this line from stderr.
                tracing::info!(output = output.0, ?position, "osd surface created");
                Some(surface)
            }
            Err(err) => {
                tracing::warn!(output = output.0, %err, "no osd surface on this output");
                None
            }
        }
    }

    fn destroy(&mut self, surface: SurfaceId) {
        self.ctl.destroy_surface(surface);
        // The smoke reads this line from stderr.
        tracing::info!("osd surface destroyed");
    }

    fn alive(&mut self, surface: SurfaceId) -> bool {
        self.ctl.surface(surface).is_some()
    }

    fn redraw(&mut self, surface: SurfaceId) {
        if let Some(mut ctl) = self.ctl.surface(surface) {
            ctl.request_redraw();
        }
    }

    fn arm(&mut self, after: Duration, tick: OsdTick) {
        // The machine's deadlines are on the display's own clock; the loop's
        // timer takes an instant.
        match self.runtime {
            Some(runtime) => runtime.arm(Instant::now() + after, tick),
            None => tracing::warn!(?tick, "osd: the loop's timer is not ready; not armed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::audio::AudioLevel;
    use celestina_shell::settings::{Duties, OsdKinds, OsdSettings};

    use super::*;
    use crate::providers::brightness::BrightnessSource;

    /// Records what the decisions asked of the world.
    #[derive(Debug, Default)]
    struct Fake {
        next: u32,
        created: Vec<(OutputId, OsdPosition, SurfaceId)>,
        destroyed: Vec<SurfaceId>,
        /// Surfaces the compositor closed behind the product's back.
        closed: Vec<SurfaceId>,
        redraws: usize,
        armed: Vec<(Duration, OsdTick)>,
        refuse: bool,
    }

    impl Fake {
        fn live(&self) -> usize {
            self.created.len() - self.destroyed.len()
        }

        fn last_tick(&self) -> OsdTick {
            self.armed.last().expect("a timer was armed").1
        }
    }

    impl Host for Fake {
        fn create(&mut self, output: OutputId, position: OsdPosition) -> Option<SurfaceId> {
            if self.refuse {
                return None;
            }
            self.next += 1;
            let surface = SurfaceId(self.next);
            self.created.push((output, position, surface));
            Some(surface)
        }
        fn destroy(&mut self, surface: SurfaceId) {
            self.destroyed.push(surface);
        }
        fn alive(&mut self, surface: SurfaceId) -> bool {
            !self.closed.contains(&surface) && !self.destroyed.contains(&surface)
        }
        fn redraw(&mut self, _: SurfaceId) {
            self.redraws += 1;
        }
        fn arm(&mut self, after: Duration, tick: OsdTick) {
            self.armed.push((after, tick));
        }
    }

    const T: Duration = Duration::from_secs(5);

    fn settings(on: bool) -> Settings {
        Settings {
            duties: Duties {
                osd: if on { Duty::On } else { Duty::Off },
                ..Duties::default()
            },
            ..Settings::default()
        }
    }

    fn info(id: u32, name: &str) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_owned(),
            logical_size: (1920, 1080),
            scale: 1.0,
            refresh_mhz: 60_000,
        }
    }

    fn osds(on: bool) -> Osds {
        let mut osds = Osds::new(Rc::new(OnceCell::new()), &settings(on), &Theme::dark());
        osds.output_added(&info(1, "DP-1"));
        osds.output_added(&info(2, "HDMI-A-1"));
        osds
    }

    fn volume(percent: u8, muted: bool) -> AudioEvent {
        AudioEvent {
            device: Device::Output,
            level: Some(AudioLevel { percent, muted }),
        }
    }

    fn mic(percent: u8) -> AudioEvent {
        AudioEvent {
            device: Device::Input,
            level: Some(AudioLevel {
                percent,
                muted: false,
            }),
        }
    }

    fn bright(monitor: &str, percent: Option<u8>) -> BrightnessEvent {
        BrightnessEvent {
            monitor: monitor.to_owned(),
            percent,
            source: BrightnessSource::Ddc,
        }
    }

    /// Fires the last armed timer.
    fn fire(osds: &mut Osds, host: &mut Fake) {
        let tick = host.last_tick();
        osds.ticked(tick, host);
    }

    #[test]
    fn the_slider_popup_hides_the_display_and_keeps_it_hidden_until_it_closes() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_some()));
        // The popup opens: what is on screen leaves through the exit.
        osds.suppress(Suppression::Open(VOLUME), T, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_none()));
        // The popup drives the volume: nothing is shown for it.
        osds.audio_read(volume(60, false), None, T, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_none()));
        assert_eq!(host.created.len(), 1);
        // Closed: the level it left is the baseline, so nothing is announced.
        osds.suppress(closed(VOLUME, &[(Kind::Volume, Some(60))]), T, &mut host);
        osds.audio_read(volume(60, false), None, T, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_none()));
        // A later change is shown again.
        osds.audio_read(volume(65, false), None, T, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_some()));
    }

    #[test]
    fn suppressing_with_nothing_shown_does_nothing() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.suppress(Suppression::Open(VOLUME), T, &mut host);
        osds.suppress(closed(VOLUME, &[]), T, &mut host);
        assert!(host.created.is_empty() && host.armed.is_empty());
    }

    const VOLUME: &[Kind] = &[Kind::Volume, Kind::Microphone];
    const BRIGHTNESS: &[Kind] = &[Kind::Brightness];

    fn closed(kinds: &'static [Kind], asked: &[(Kind, Option<u8>)]) -> Suppression {
        Suppression::Closed {
            kinds,
            asked: asked.to_vec(),
        }
    }

    #[test]
    fn a_brightness_key_shows_while_the_volume_popup_is_open() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.brightness_read(bright("DP-1", Some(50)), None, T, &mut host);
        osds.suppress(Suppression::Open(VOLUME), T, &mut host);
        osds.brightness_read(bright("DP-1", Some(55)), None, T, &mut host);
        let shown = osds.live.as_ref().and_then(|l| l.shown).expect("shown");
        assert_eq!(shown.kind, Kind::Brightness);
    }

    #[test]
    fn the_brightness_popup_s_last_level_read_back_after_it_closed_is_not_announced() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.brightness_read(bright("DP-1", Some(50)), None, T, &mut host);
        osds.suppress(Suppression::Open(BRIGHTNESS), T, &mut host);
        osds.brightness_read(bright("DP-1", Some(40)), None, T, &mut host);
        osds.suppress(
            closed(BRIGHTNESS, &[(Kind::Brightness, Some(20))]),
            T,
            &mut host,
        );
        // The write still in flight and the one owed read back afterwards.
        osds.brightness_read(bright("DP-1", Some(30)), None, T, &mut host);
        osds.brightness_read(bright("DP-1", Some(20)), None, T, &mut host);
        assert!(host.created.is_empty(), "the popup's own change");
        // A key press afterwards is shown.
        osds.brightness_read(bright("DP-1", Some(15)), None, T, &mut host);
        assert_eq!(host.created.len(), 1);
    }

    #[test]
    fn baselines_and_rest_create_no_surface() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(mic(30), None, T, &mut host);
        osds.brightness_read(bright("DP-1", Some(70)), None, T, &mut host);
        osds.brightness_read(bright("HDMI-A-1", Some(20)), None, T, &mut host);
        // Unchanged readings are not changes either.
        osds.audio_read(volume(40, false), None, T, &mut host);
        assert!(host.created.is_empty());
        assert!(host.armed.is_empty());
        assert!(!osds.has_surface());
    }

    #[test]
    fn a_change_creates_one_surface_on_the_focused_output_with_its_timeout() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), Some("HDMI-A-1"), T, &mut host);
        assert_eq!(host.created.len(), 1);
        assert_eq!(host.created[0].0, OutputId(2));
        assert_eq!(host.created[0].1, OsdPosition::Bottom);
        assert!(matches!(host.last_tick(), OsdTick::Expire(_)));
        assert!(osds.owns(host.created[0].2));
        assert_eq!(
            osds.live
                .as_ref()
                .and_then(|l| l.shown)
                .map(|s| s.reading.percent),
            Some(45)
        );
    }

    #[test]
    fn without_a_known_focused_output_the_first_output_is_used() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), Some("eDP-9"), T, &mut host);
        assert_eq!(host.created[0].0, OutputId(1));
        // And with no output at all, nothing is created.
        let mut lonely = Osds::new(Rc::new(OnceCell::new()), &settings(true), &Theme::dark());
        let mut host = Fake::default();
        lonely.audio_read(volume(40, false), None, T, &mut host);
        lonely.audio_read(volume(45, false), None, T, &mut host);
        assert!(host.created.is_empty());
        assert!(!lonely.has_surface());
    }

    #[test]
    fn another_kind_replaces_the_content_in_place_and_rearms() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(mic(30), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        let first = host.last_tick();
        osds.audio_read(mic(35), Some("HDMI-A-1"), T, &mut host);
        let second = host.last_tick();
        assert_eq!(host.created.len(), 1, "one display at a time");
        assert_ne!(first, second);
        let shown = osds.live.as_ref().and_then(|l| l.shown).expect("shown");
        assert_eq!(shown.kind, Kind::Microphone);
        // The old timeout does nothing.
        osds.ticked(first, &mut host);
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_some()));
    }

    #[test]
    fn the_timeout_fades_the_card_and_the_retire_destroys_the_surface() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        fire(&mut osds, &mut host);
        // Fading: the surface stays, with nothing shown, until the retire.
        assert!(osds.has_surface());
        assert!(osds.live.as_ref().is_some_and(|l| l.shown.is_none()));
        assert!(matches!(host.last_tick(), OsdTick::Retire(_)));
        assert_eq!(host.armed.last().map(|a| a.0), Some(osds.exit));
        fire(&mut osds, &mut host);
        assert!(!osds.has_surface());
        assert_eq!(host.live(), 0, "rest is no surface");
    }

    #[test]
    fn a_change_during_the_exit_rearms_on_the_same_surface() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        fire(&mut osds, &mut host);
        let retire = host.last_tick();
        osds.audio_read(volume(50, false), None, T, &mut host);
        assert_eq!(host.created.len(), 1, "the fading surface is reused");
        // The stale retire leaves it alone.
        osds.ticked(retire, &mut host);
        assert!(osds.has_surface());
        assert!(host.destroyed.is_empty());
        assert!(matches!(host.last_tick(), OsdTick::Expire(_)));
    }

    #[test]
    fn a_surface_the_compositor_closed_is_made_again() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        host.closed.push(host.created[0].2);
        osds.audio_read(volume(50, false), None, T, &mut host);
        assert_eq!(host.created.len(), 2);
        assert!(osds.owns(host.created[1].2));
    }

    #[test]
    fn the_output_going_takes_the_surface_and_the_next_show_uses_another() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), Some("DP-1"), T, &mut host);
        osds.output_removed(OutputId(1));
        assert!(!osds.has_surface());
        assert!(host.destroyed.is_empty(), "it went with the output");
        osds.audio_read(volume(50, false), Some("DP-1"), T, &mut host);
        assert_eq!(host.created.last().map(|c| c.0), Some(OutputId(2)));
    }

    #[test]
    fn brightness_is_keyed_per_monitor() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.brightness_read(bright("DP-1", Some(50)), None, T, &mut host);
        osds.brightness_read(bright("DP-1", Some(60)), None, T, &mut host);
        assert_eq!(host.created.len(), 1);
        // A second monitor's first reading is its baseline, even at 60.
        let armed = host.armed.len();
        osds.brightness_read(bright("HDMI-A-1", Some(40)), None, T, &mut host);
        assert_eq!(host.armed.len(), armed);
        // Its move to the level the first one shows is still a change.
        osds.brightness_read(bright("HDMI-A-1", Some(60)), None, T, &mut host);
        assert_eq!(host.armed.len(), armed + 1);
        // Unknown forgets the baseline: the next reading is a baseline.
        osds.brightness_read(bright("HDMI-A-1", None), None, T, &mut host);
        osds.brightness_read(bright("HDMI-A-1", Some(10)), None, T, &mut host);
        assert_eq!(host.armed.len(), armed + 1);
    }

    #[test]
    fn a_device_without_a_reading_takes_its_display_away() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, true), None, T, &mut host);
        let shown = osds.live.as_ref().and_then(|l| l.shown).expect("shown");
        assert!(shown.reading.muted);
        assert_eq!(shown.reading.percent, 45);
        osds.audio_read(
            AudioEvent {
                device: Device::Output,
                level: None,
            },
            None,
            T,
            &mut host,
        );
        assert!(matches!(host.last_tick(), OsdTick::Retire(_)));
        fire(&mut osds, &mut host);
        assert!(!osds.has_surface());
        // A new device's first reading is a baseline.
        osds.audio_read(volume(90, false), None, T, &mut host);
        assert_eq!(host.created.len(), 1);
    }

    #[test]
    fn the_duty_off_creates_nothing() {
        let mut osds = osds(false);
        let mut host = Fake::default();
        for percent in [10, 20, 30] {
            osds.audio_read(volume(percent, false), None, T, &mut host);
            osds.brightness_read(bright("DP-1", Some(percent)), None, T, &mut host);
        }
        assert!(host.created.is_empty());
        assert!(host.armed.is_empty());
    }

    #[test]
    fn switching_the_duty_off_live_destroys_the_display_and_forgets_the_baselines() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        let surface = host.created[0].2;
        osds.apply(&settings(false), T, &mut host);
        assert_eq!(host.destroyed, vec![surface], "at once, no fade");
        assert!(!osds.has_surface());
        // Its timers are stale now.
        let armed = host.armed.clone();
        for (_, tick) in armed {
            osds.ticked(tick, &mut host);
        }
        assert_eq!(host.destroyed.len(), 1);
        // Back on: the first readings are baselines again.
        osds.apply(&settings(true), T, &mut host);
        osds.audio_read(volume(80, false), None, T, &mut host);
        assert_eq!(host.created.len(), 1);
        osds.audio_read(volume(85, false), None, T, &mut host);
        assert_eq!(host.created.len(), 2);
    }

    #[test]
    fn a_kind_switched_off_live_takes_its_display_away_and_the_position_applies_next_time() {
        let mut osds = osds(true);
        let mut host = Fake::default();
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        let mut next = settings(true);
        next.osd = OsdSettings {
            position: OsdPosition::BottomRight,
            kinds: OsdKinds {
                volume: false,
                ..OsdKinds::default()
            },
            ..OsdSettings::default()
        };
        osds.apply(&next, T, &mut host);
        assert!(matches!(host.last_tick(), OsdTick::Retire(_)));
        fire(&mut osds, &mut host);
        assert!(!osds.has_surface());
        osds.audio_read(volume(50, false), None, T, &mut host);
        assert_eq!(host.created.len(), 1, "volume is off");
        osds.audio_read(mic(30), None, T, &mut host);
        osds.audio_read(mic(40), None, T, &mut host);
        assert_eq!(
            host.created.last().map(|c| c.1),
            Some(OsdPosition::BottomRight)
        );
    }

    #[test]
    fn a_refused_surface_shows_nothing_and_holds_nothing() {
        let mut osds = osds(true);
        let mut host = Fake {
            refuse: true,
            ..Fake::default()
        };
        osds.audio_read(volume(40, false), None, T, &mut host);
        osds.audio_read(volume(45, false), None, T, &mut host);
        assert!(!osds.has_surface());
        // Its timeout comes and goes harmlessly.
        fire(&mut osds, &mut host);
        assert!(!osds.has_surface());
    }
}
