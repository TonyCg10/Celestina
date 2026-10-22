//! The slider popup of the Volume and Brightness pieces (CEL-2-C, plan
//! decision 8): an `xdg_popup` of the Island's surface hung below the piece,
//! a Haze card with the level as a slider and, for the volume, the mute
//! switch and the microphone's own slider and mute switch.
//!
//! The popup acts while it is open: a slider that moves, or a switch that
//! flips, sends a [`Request`] to the app's sink at once (not on close), and
//! the app turns it into worker commands ([`commands`]). What the popup
//! shows is the app's reading, shared through a cell the app writes and the
//! popup reads each frame; a slider also shows its own change before the
//! worker confirms it, and while the pointer is down the readings do not move
//! it, so a late reading never pulls the handle back under the finger.
//!
//! A drag is a burst of values: [`SEND_INTERVAL`] spaces the sends, the newest
//! value of each slider waits, and the last one is always sent (at release, at
//! close, or when its turn comes), so the worker sees a few writes and the
//! final level is never lost to the throttle.

use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use celestina_paint::{silhouette_slices, Rect, Role, Theme, Weight};
use celestina_ui::{slider, view, Interaction, Key, Slider, Tree, UiEvent, View};
use celestina_wayland::{
    AnchorEdge, FrameCx, Gravity, InputEvent, Placement, PopupAction, PopupShell, SurfaceCtl,
};

use super::readings::Target;
use crate::island::menu::SharedPainter;
use crate::{
    copy,
    island::{
        state::SILHOUETTE_STEP,
        view::{centred, switch, text_style},
    },
    pieces::Levels,
    providers::{
        audio::{AudioCommand, Device},
        brightness::{BrightnessCommand, Change},
    },
};

/// The card's width, logical px.
pub const POPUP_W: f32 = 280.0;
/// The shortest gap between two sends of one slider while it is dragged.
pub const SEND_INTERVAL: Duration = Duration::from_millis(80);
/// The arrow keys' step, as the verbs' and the wheel's.
const STEP: u8 = celestina_shell::audio::STEP_PERCENT;
/// The icon beside a heading, logical px.
const ICON: f32 = 18.0;

/// What the popup is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The output's level and mute, and the microphone's.
    Volume,
    /// The monitor's level.
    Brightness,
}

/// A slider or switch of the popup, by what it controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Output,
    Input,
    Brightness,
}

/// What the popup asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Set the level of the control to this percent.
    Set(Control, u8),
    /// Flip the control's mute.
    ToggleMute(Control),
}

/// A worker command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Audio(AudioCommand),
    Brightness(BrightnessCommand),
}

/// The commands a request is. `target` is the monitor the brightness slider
/// drives (the popup's output); a brightness request without one is nothing.
#[must_use]
pub fn commands(request: Request, target: Option<&Target>) -> Vec<Command> {
    let device = |control| match control {
        Control::Output => Some(Device::Output),
        Control::Input => Some(Device::Input),
        Control::Brightness => None,
    };
    match request {
        Request::Set(Control::Brightness, percent) => target
            .map(|target| {
                Command::Brightness(BrightnessCommand::Change {
                    source: target.source,
                    monitor: target.monitor.clone(),
                    change: Change::Set(percent),
                })
            })
            .into_iter()
            .collect(),
        Request::Set(control, percent) => device(control)
            .map(|device| Command::Audio(AudioCommand::SetVolume { device, percent }))
            .into_iter()
            .collect(),
        Request::ToggleMute(control) => device(control)
            .map(|device| Command::Audio(AudioCommand::ToggleMute { device }))
            .into_iter()
            .collect(),
    }
}

/// Where the app receives the popup's requests.
pub type Sink = Rc<dyn Fn(Request)>;

/// What the app writes and the popup reads: the readings it should show.
pub type SharedLevels = Rc<Cell<Levels>>;

fn card_key() -> Key {
    Key::named("level-card")
}

/// The slider of `control`.
#[must_use]
pub fn slider_key(control: Control) -> Key {
    match control {
        Control::Output => Key::named("level-output"),
        Control::Input => Key::named("level-input"),
        Control::Brightness => Key::named("level-brightness"),
    }
}

/// The mute row of `control`.
#[must_use]
pub fn mute_key(control: Control) -> Key {
    match control {
        Control::Output => Key::named("level-mute-output"),
        Control::Input => Key::named("level-mute-input"),
        Control::Brightness => Key::named("level-mute-brightness"),
    }
}

fn control_of_slider(key: Key) -> Option<Control> {
    [Control::Output, Control::Input, Control::Brightness]
        .into_iter()
        .find(|c| slider_key(*c) == key)
}

fn control_of_mute(key: Key) -> Option<Control> {
    [Control::Output, Control::Input]
        .into_iter()
        .find(|c| mute_key(*c) == key)
}

/// A heading row's height, a mute row's height and the slider's.
fn heading_h(theme: &Theme) -> f32 {
    theme.row_height
}

fn slider_h(theme: &Theme) -> f32 {
    theme.slider_handle_size + 2.0 * slider::MARGIN
}

/// The popup's logical size: the card's padding all round and, per section, a
/// heading, a slider and (for audio) a mute row, a small gap between all.
#[must_use]
pub fn popup_size(theme: &Theme, kind: Kind, microphone: bool) -> (f32, f32) {
    let sections = match kind {
        Kind::Volume => 1 + usize::from(microphone),
        Kind::Brightness => 1,
    };
    let rows_per_section = match kind {
        Kind::Volume => 3,
        Kind::Brightness => 2,
    };
    let gaps = sections * rows_per_section - 1;
    let section_h = heading_h(theme)
        + slider_h(theme)
        + match kind {
            Kind::Volume => theme.row_height,
            Kind::Brightness => 0.0,
        };
    (
        POPUP_W,
        2.0 * theme.spacing.md + sections as f32 * section_h + gaps as f32 * theme.spacing.sm,
    )
}

/// The popup hangs centred below the piece's node `anchor` (logical px of the
/// Island's surface), a small gap under it; the compositor slides and flips
/// it to stay on the output.
#[must_use]
pub fn placement(theme: &Theme, anchor: Rect) -> Placement {
    Placement {
        anchor,
        anchor_edge: AnchorEdge::Bottom,
        gravity: Gravity::Bottom,
        offset: (0, theme.spacing.sm as i32),
    }
}

fn heading(theme: &Theme, key: &'static str, icon: &'static str, label: &str, percent: u8) -> View {
    let role = Role::TextHi;
    centred(view::row(Key::named(key)))
        .height(heading_h(theme))
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.md)
        .child(view::icon(Key::named("level-heading-icon"), icon, ICON).tint(role))
        .child(
            view::text(
                Key::named("level-heading-label"),
                label,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .grow(),
        )
        .child(view::text(
            Key::named("level-heading-percent"),
            &copy::osd_percent(percent.min(100)),
            text_style(theme.type_scale.body, Weight::Regular, true, role),
        ))
}

fn mute_row(theme: &Theme, control: Control, label: &str, muted: bool, enabled: bool) -> View {
    let role = if enabled {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let row = centred(view::row(mute_key(control)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.sm, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_sm)
        .child(
            view::text(
                Key::named("level-mute-label"),
                label,
                text_style(theme.type_scale.body, Weight::Regular, false, role),
            )
            .grow(),
        )
        .child(switch(Key::named("level-mute-switch"), muted, enabled));
    if enabled {
        row.interactive().focusable()
    } else {
        row
    }
}

fn level_slider(control: Control, percent: Option<u8>, muted: bool) -> View {
    let value = percent.unwrap_or(0);
    view::slider(
        slider_key(control),
        Slider::new(value)
            .with_step(STEP)
            .with_muted(muted)
            .with_enabled(percent.is_some()),
    )
}

/// The card for `levels`: `microphone` says whether the microphone's section
/// is part of this popup (decided when it opened, so its size never changes).
#[must_use]
pub fn card(theme: &Theme, kind: Kind, microphone: bool, levels: Levels) -> View {
    let (w, h) = popup_size(theme, kind, microphone);
    let mut card = view::col(card_key())
        .haze(theme.radius_md)
        .size(w, h)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm);
    match kind {
        Kind::Volume => {
            let out = levels.audio.output;
            card = card
                .child(heading(
                    theme,
                    "level-heading-output",
                    if out.is_some_and(|l| l.muted) {
                        "media-volume-muted"
                    } else {
                        "media-volume"
                    },
                    copy::PIECE_VOLUME,
                    out.map_or(0, |l| l.percent),
                ))
                .child(level_slider(
                    Control::Output,
                    out.map(|l| l.percent),
                    out.is_some_and(|l| l.muted),
                ))
                .child(mute_row(
                    theme,
                    Control::Output,
                    copy::MUTE,
                    out.is_some_and(|l| l.muted),
                    out.is_some(),
                ));
            if microphone {
                let input = levels.audio.input;
                card = card
                    .child(heading(
                        theme,
                        "level-heading-input",
                        if input.is_some_and(|l| l.muted) {
                            "mic-off"
                        } else {
                            "mic"
                        },
                        copy::MICROPHONE,
                        input.map_or(0, |l| l.percent),
                    ))
                    .child(level_slider(
                        Control::Input,
                        input.map(|l| l.percent),
                        input.is_some_and(|l| l.muted),
                    ))
                    .child(mute_row(
                        theme,
                        Control::Input,
                        copy::MUTE_MICROPHONE,
                        input.is_some_and(|l| l.muted),
                        input.is_some(),
                    ));
            }
        }
        Kind::Brightness => {
            card = card
                .child(heading(
                    theme,
                    "level-heading-brightness",
                    "sun",
                    copy::PIECE_BRIGHTNESS,
                    levels.brightness.unwrap_or(0),
                ))
                .child(level_slider(Control::Brightness, levels.brightness, false));
        }
    }
    card
}

/// What the popup does with an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Stays open; `changed` when its look did (a frame is due).
    Keep { changed: bool },
    /// Closes.
    Close,
}

/// The newest value each slider has not yet sent, and when one last went.
#[derive(Debug, Default)]
struct Throttle {
    pending: [Option<u8>; 3],
    last: Option<Instant>,
}

impl Throttle {
    fn slot(control: Control) -> usize {
        match control {
            Control::Output => 0,
            Control::Input => 1,
            Control::Brightness => 2,
        }
    }

    const CONTROLS: [Control; 3] = [Control::Output, Control::Input, Control::Brightness];

    fn push(&mut self, control: Control, percent: u8) {
        self.pending[Self::slot(control)] = Some(percent);
    }

    /// The requests due at `now`: all of them when `force`, else those whose
    /// turn has come (at most one send per [`SEND_INTERVAL`]).
    fn due(&mut self, now: Instant, force: bool) -> Vec<Request> {
        let ready = force
            || self
                .last
                .is_none_or(|last| now.saturating_duration_since(last) >= SEND_INTERVAL);
        if !ready {
            return Vec::new();
        }
        let sent: Vec<Request> = Self::CONTROLS
            .into_iter()
            .filter_map(|c| {
                self.pending[Self::slot(c)]
                    .take()
                    .map(|p| Request::Set(c, p))
            })
            .collect();
        if !sent.is_empty() {
            self.last = Some(now);
        }
        sent
    }

    /// How long until something pending is due; `None` with nothing pending.
    fn wait(&self, now: Instant) -> Option<Duration> {
        if self.pending.iter().all(Option::is_none) {
            return None;
        }
        let elapsed = self
            .last
            .map_or(SEND_INTERVAL, |last| now.saturating_duration_since(last));
        Some(SEND_INTERVAL.saturating_sub(elapsed))
    }
}

/// The popup without its painter: the tree, the pointer and keyboard state
/// and what it shows.
pub struct LevelCore {
    kind: Kind,
    microphone: bool,
    theme: Theme,
    tree: Tree,
    input: Interaction,
    /// What the popup shows: the app's reading, with the popup's own changes
    /// on top until the worker confirms them.
    shown: Levels,
    /// The last app reading taken in. A reading is adopted when it differs
    /// from this, never merely because it differs from `shown`, which holds
    /// the popup's own changes the cell has not caught up with yet.
    seen: Levels,
    throttle: Throttle,
    sink: Sink,
    cell: SharedLevels,
}

impl std::fmt::Debug for LevelCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelCore")
            .field("kind", &self.kind)
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}

impl LevelCore {
    /// The popup of `kind` over `cell`'s readings in `theme`, under `reduced`
    /// motion. The microphone's section is part of a volume popup when the
    /// microphone has a reading now.
    #[must_use]
    pub fn new(
        theme: &Theme,
        kind: Kind,
        cell: SharedLevels,
        sink: Sink,
        reduced: bool,
        now: Instant,
    ) -> LevelCore {
        let shown = cell.get();
        let microphone = kind == Kind::Volume && shown.audio.input.is_some();
        let mut tree = Tree::new(reduced, theme);
        tree.apply(card(theme, kind, microphone, shown), now);
        LevelCore {
            kind,
            microphone,
            theme: theme.clone(),
            tree,
            input: Interaction::new(),
            shown,
            seen: shown,
            throttle: Throttle::default(),
            sink,
            cell,
        }
    }

    /// Whether the card includes the microphone.
    #[must_use]
    pub fn has_microphone(&self) -> bool {
        self.microphone
    }

    /// Whether a press is held on the card (a drag): readings then do not
    /// move the sliders.
    fn held(&self) -> bool {
        self.tree.highlight().pressed.is_some()
    }

    fn rebuild(&mut self, now: Instant) {
        self.tree.apply(
            card(&self.theme, self.kind, self.microphone, self.shown),
            now,
        );
    }

    /// Takes in a new theme and the app's newest reading; whether the look
    /// changed.
    pub fn sync(&mut self, theme: &Theme, now: Instant) -> bool {
        let mut changed = false;
        if *theme != self.theme {
            self.theme = theme.clone();
            self.tree.set_theme(theme);
            changed = true;
        }
        let external = self.cell.get();
        if !self.held() && external != self.seen {
            self.seen = external;
            self.shown = external;
            changed = true;
        }
        if changed {
            self.rebuild(now);
        }
        changed
    }

    /// Sends what is due at `now` and returns how long until the rest is.
    pub fn flush(&mut self, now: Instant, force: bool) -> Option<Duration> {
        for request in self.throttle.due(now, force) {
            (self.sink)(request);
        }
        self.throttle.wait(now)
    }

    fn set_shown(&mut self, control: Control, percent: u8) {
        let slot = match control {
            Control::Output => self.shown.audio.output.as_mut().map(|l| &mut l.percent),
            Control::Input => self.shown.audio.input.as_mut().map(|l| &mut l.percent),
            Control::Brightness => self.shown.brightness.as_mut(),
        };
        if let Some(slot) = slot {
            *slot = percent;
        }
    }

    fn flip_shown(&mut self, control: Control) {
        let level = match control {
            Control::Output => self.shown.audio.output.as_mut(),
            Control::Input => self.shown.audio.input.as_mut(),
            Control::Brightness => None,
        };
        if let Some(level) = level {
            level.muted = !level.muted;
        }
    }

    /// Handles one event: Escape closes; a slider moved sends its level; a
    /// click or Enter on a mute row flips it.
    pub fn input(&mut self, ev: &InputEvent, now: Instant) -> Outcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::FocusLost => self.input.key(&mut self.tree, ev),
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let mut moved = false;
        for event in &events {
            match event {
                UiEvent::Escape => return Outcome::Close,
                UiEvent::SliderChanged(key, percent) => {
                    if let Some(control) = control_of_slider(*key) {
                        self.set_shown(control, *percent);
                        self.throttle.push(control, *percent);
                        moved = true;
                    }
                }
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(control) = control_of_mute(*key) {
                        self.flip_shown(control);
                        (self.sink)(Request::ToggleMute(control));
                        moved = true;
                    }
                }
                UiEvent::HoverChanged(..) | UiEvent::Scroll(..) | UiEvent::FieldChanged(_) => {}
            }
        }
        if moved {
            self.rebuild(now);
        }
        // The release ends a drag: its final level goes out now.
        let force = matches!(ev, InputEvent::PointerRelease { .. });
        self.flush(now, force);
        Outcome::Keep {
            changed: moved || !events.is_empty() || self.tree.highlight() != before,
        }
    }
}

/// The popup as a popup shell.
pub struct LevelPopup {
    core: LevelCore,
    painter: SharedPainter,
}

impl std::fmt::Debug for LevelPopup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelPopup")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl LevelPopup {
    /// The popup of `kind` over `cell`, drawn with `painter`; its requests go
    /// to `sink`.
    #[must_use]
    pub fn new(
        painter: SharedPainter,
        theme: &Theme,
        kind: Kind,
        cell: SharedLevels,
        sink: Sink,
        reduced: bool,
    ) -> LevelPopup {
        LevelPopup {
            core: LevelCore::new(theme, kind, cell, sink, reduced, Instant::now()),
            painter,
        }
    }

    /// Whether the card includes the microphone, for the size it is opened at.
    #[must_use]
    pub fn has_microphone(&self) -> bool {
        self.core.has_microphone()
    }
}

impl PopupShell for LevelPopup {
    fn frame(&mut self, cx: &mut FrameCx<'_>) {
        let Ok(mut painter) = self.painter.try_borrow_mut() else {
            tracing::warn!("level popup: the painter is in use; frame skipped");
            cx.ctl.request_redraw();
            return;
        };
        let now = cx.now;
        self.core.sync(painter.theme(), now);
        if let Some(wait) = self.core.flush(now, false) {
            cx.ctl.redraw_after(wait);
        }
        let radius = painter.theme().radius_md;
        let dpr = cx.scale as f32;
        let tree = &mut self.core.tree;
        tree.layout(cx.size, painter.text(), dpr);
        if let Some(fault) = tree.take_fault() {
            tracing::warn!(%fault, "level popup: layout fault");
        }
        painter.begin(cx.size, dpr, cx.blur_available);
        tree.paint(&mut painter, now);
        painter.end();
        let silhouette = silhouette_slices(Rect::whole(cx.size), radius, SILHOUETTE_STEP);
        cx.ctl.set_blur_region(&silhouette);
        cx.ctl.set_input_region(&silhouette);
        if tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }

    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) -> PopupAction {
        let now = Instant::now();
        let outcome = self.core.input(&ev, now);
        match outcome {
            Outcome::Close => {
                // What waits is the last thing the person chose.
                self.core.flush(now, true);
                PopupAction::Close
            }
            Outcome::Keep { changed } => {
                if changed {
                    ctl.request_redraw();
                }
                if let Some(wait) = self.core.throttle.wait(now) {
                    ctl.redraw_after(wait);
                }
                PopupAction::Keep
            }
        }
    }

    fn dismissed(&mut self) {
        self.core.flush(Instant::now(), true);
        tracing::debug!("level popup: dismissed by the compositor");
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_ui::BTN_LEFT;
    use celestina_wayland::Modifiers;

    use super::*;
    use crate::pieces::{AudioLevels, Level};
    use crate::providers::brightness::BrightnessSource;

    const ESCAPE: u32 = 0xff1b;
    const RIGHT: u32 = 0xff53;
    const TAB: u32 = 0xff09;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn audio(output: Option<(u8, bool)>, input: Option<(u8, bool)>) -> Levels {
        let level = |l: Option<(u8, bool)>| l.map(|(percent, muted)| Level { percent, muted });
        Levels {
            audio: AudioLevels {
                output: level(output),
                input: level(input),
            },
            brightness: None,
        }
    }

    fn brightness(percent: Option<u8>) -> Levels {
        Levels {
            audio: AudioLevels::default(),
            brightness: percent,
        }
    }

    struct Rig {
        core: LevelCore,
        sent: Rc<std::cell::RefCell<Vec<Request>>>,
        cell: SharedLevels,
    }

    fn rig(kind: Kind, levels: Levels) -> Rig {
        let theme = Theme::dark();
        let sent = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink: Sink = {
            let sent = Rc::clone(&sent);
            Rc::new(move |request| sent.borrow_mut().push(request))
        };
        let cell: SharedLevels = Rc::new(Cell::new(levels));
        let mut core = LevelCore::new(&theme, kind, Rc::clone(&cell), sink, false, Instant::now());
        let size = popup_size(&theme, kind, core.has_microphone());
        core.tree.layout_measured(size, fake_measure);
        Rig { core, sent, cell }
    }

    fn rect_of(core: &LevelCore, key: Key) -> Rect {
        match core.tree.node_by_key(key) {
            Some(id) => core.tree.rect(id),
            None => panic!("no node for {key:?}"),
        }
    }

    fn press(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerPress {
            x,
            y,
            button: BTN_LEFT,
            serial: 1,
        }
    }

    fn release(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerRelease {
            x,
            y,
            button: BTN_LEFT,
            serial: 2,
        }
    }

    fn motion(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerMotion { x, y }
    }

    fn key(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 1,
        }
    }

    #[test]
    fn requests_become_the_workers_commands() {
        let ddc = Target {
            source: BrightnessSource::Ddc,
            monitor: "DP-1".to_owned(),
        };
        assert_eq!(
            commands(Request::Set(Control::Output, 40), None),
            [Command::Audio(AudioCommand::SetVolume {
                device: Device::Output,
                percent: 40
            })]
        );
        assert_eq!(
            commands(Request::Set(Control::Input, 10), Some(&ddc)),
            [Command::Audio(AudioCommand::SetVolume {
                device: Device::Input,
                percent: 10
            })]
        );
        assert_eq!(
            commands(Request::ToggleMute(Control::Input), None),
            [Command::Audio(AudioCommand::ToggleMute {
                device: Device::Input
            })]
        );
        assert_eq!(
            commands(Request::Set(Control::Brightness, 70), Some(&ddc)),
            [Command::Brightness(BrightnessCommand::Change {
                source: BrightnessSource::Ddc,
                monitor: "DP-1".to_owned(),
                change: Change::Set(70)
            })]
        );
        // No monitor, no brightness; brightness has no mute.
        assert!(commands(Request::Set(Control::Brightness, 70), None).is_empty());
        assert!(commands(Request::ToggleMute(Control::Brightness), Some(&ddc)).is_empty());
    }

    #[test]
    fn the_card_fills_the_popup_in_every_shape() {
        let theme = Theme::dark();
        for (kind, levels, mic) in [
            (Kind::Volume, audio(Some((30, false)), None), false),
            (
                Kind::Volume,
                audio(Some((30, false)), Some((80, true))),
                true,
            ),
            (Kind::Brightness, brightness(Some(60)), false),
        ] {
            let rig = rig(kind, levels);
            assert_eq!(rig.core.has_microphone(), mic);
            let size = popup_size(&theme, kind, mic);
            let card = rect_of(&rig.core, card_key());
            assert_eq!((card.x, card.y, card.w, card.h), (0.0, 0.0, size.0, size.1));
            // Every control sits inside the card.
            for control in [Control::Output, Control::Input, Control::Brightness] {
                if let Some(id) = rig.core.tree.node_by_key(slider_key(control)) {
                    let r = rig.core.tree.rect(id);
                    assert!(
                        r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= size.0 && r.y + r.h <= size.1,
                        "{control:?} {r:?} in {size:?}"
                    );
                }
            }
        }
        // The microphone's section only adds height.
        assert!(
            popup_size(&theme, Kind::Volume, true).1 > popup_size(&theme, Kind::Volume, false).1
        );
        assert!(
            popup_size(&theme, Kind::Volume, false).1
                > popup_size(&theme, Kind::Brightness, false).1
        );
    }

    #[test]
    fn the_microphone_section_exists_only_if_it_has_a_reading_at_open() {
        assert!(!rig(Kind::Volume, audio(Some((30, false)), None))
            .core
            .has_microphone());
        assert!(
            rig(Kind::Volume, audio(Some((30, false)), Some((5, false))))
                .core
                .has_microphone()
        );
        // Brightness never has one.
        assert!(!rig(Kind::Brightness, brightness(Some(5)))
            .core
            .has_microphone());
    }

    #[test]
    fn a_press_on_the_slider_sets_the_level_there_and_sends_it() {
        let mut rig = rig(Kind::Brightness, brightness(Some(10)));
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let now = Instant::now();
        let out = rig.core.input(
            &press(track.x + track.w - 1.0, track.y + track.h / 2.0),
            now,
        );
        assert_eq!(out, Outcome::Keep { changed: true });
        assert_eq!(*rig.sent.borrow(), [Request::Set(Control::Brightness, 100)]);
        assert_eq!(rig.core.shown.brightness, Some(100));
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Brightness))
                .map(Slider::value),
            Some(100)
        );
    }

    #[test]
    fn a_drag_is_spaced_and_its_last_level_is_sent_at_the_release() {
        let mut rig = rig(Kind::Brightness, brightness(Some(10)));
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let y = track.y + track.h / 2.0;
        let t0 = Instant::now();
        rig.core.input(&press(track.x + 20.0, y), t0);
        // Moves inside the interval wait; none is sent twice.
        for dx in [40.0, 80.0, 120.0] {
            rig.core.input(&motion(track.x + dx, y), t0);
        }
        assert_eq!(rig.sent.borrow().len(), 1, "one send within the interval");
        let waiting = rig.core.throttle.wait(t0);
        assert!(waiting.is_some_and(|w| w <= SEND_INTERVAL));
        // Its turn comes: the newest value goes, once.
        let later = t0 + SEND_INTERVAL;
        assert!(rig.core.flush(later, false).is_none());
        assert_eq!(rig.sent.borrow().len(), 2);
        // More motion, then the release sends the last level at once.
        rig.core.input(&motion(track.x + 150.0, y), later);
        let before = rig.sent.borrow().len();
        rig.core.input(&release(track.x + 150.0, y), later);
        let sent = rig.sent.borrow();
        assert_eq!(sent.len(), before + 1);
        let Some(Request::Set(Control::Brightness, last)) = sent.last().copied() else {
            panic!("a set was expected: {sent:?}");
        };
        assert_eq!(Some(last), rig.core.shown.brightness);
        assert!(
            rig.core.throttle.wait(later).is_none(),
            "nothing left waiting"
        );
    }

    #[test]
    fn a_reading_does_not_move_a_slider_under_the_pointer_but_does_afterwards() {
        let mut rig = rig(Kind::Brightness, brightness(Some(10)));
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let y = track.y + track.h / 2.0;
        let now = Instant::now();
        rig.core.input(&press(track.x + track.w - 1.0, y), now);
        rig.cell.set(brightness(Some(33)));
        assert!(!rig.core.sync(&Theme::dark(), now), "held: no change");
        assert_eq!(rig.core.shown.brightness, Some(100));
        rig.core.input(&release(track.x + track.w - 1.0, y), now);
        assert!(rig.core.sync(&Theme::dark(), now));
        assert_eq!(rig.core.shown.brightness, Some(33));
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Brightness))
                .map(Slider::value),
            Some(33)
        );
        // The same reading again changes nothing.
        assert!(!rig.core.sync(&Theme::dark(), now));
    }

    #[test]
    fn a_click_on_a_mute_row_flips_it_at_once_and_asks_for_it() {
        let mut rig = rig(Kind::Volume, audio(Some((30, false)), Some((5, true))));
        let now = Instant::now();
        let row = rect_of(&rig.core, mute_key(Control::Output));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now);
        rig.core.input(&release(x, y), now);
        assert_eq!(*rig.sent.borrow(), [Request::ToggleMute(Control::Output)]);
        assert_eq!(rig.core.shown.audio.output.map(|l| l.muted), Some(true));
        assert_eq!(rig.core.shown.audio.output.map(|l| l.percent), Some(30));
        // The microphone's row is its own.
        let row = rect_of(&rig.core, mute_key(Control::Input));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now);
        rig.core.input(&release(x, y), now);
        assert_eq!(
            rig.sent.borrow().last(),
            Some(&Request::ToggleMute(Control::Input))
        );
        assert_eq!(rig.core.shown.audio.input.map(|l| l.muted), Some(false));
    }

    #[test]
    fn the_keyboard_moves_a_slider_and_escape_closes() {
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let now = Instant::now();
        // Tab focuses the slider (the only control); Right steps by 5.
        rig.core.input(&key(TAB), now);
        rig.core.input(&key(RIGHT), now);
        assert_eq!(*rig.sent.borrow(), [Request::Set(Control::Brightness, 55)]);
        assert_eq!(rig.core.input(&key(ESCAPE), now), Outcome::Close);
    }

    #[test]
    fn a_sync_does_not_undo_what_the_popup_changed_before_the_reading_follows() {
        // Keyboard: the cell still holds the old reading after a step.
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let now = Instant::now();
        rig.core.input(&key(TAB), now);
        rig.core.input(&key(RIGHT), now);
        rig.core.sync(&Theme::dark(), now);
        assert_eq!(rig.core.shown.brightness, Some(55));
        rig.core.input(&key(RIGHT), now);
        rig.core.sync(&Theme::dark(), now);
        assert_eq!(rig.core.shown.brightness, Some(60), "steps accumulate");
        // The reading catching up is adopted.
        rig.cell.set(brightness(Some(60)));
        rig.core.sync(&Theme::dark(), now);
        assert_eq!(rig.core.shown.brightness, Some(60));

        // Drag: the release does not jump back to the old reading.
        let mut rig = rig_drag();
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let y = track.y + track.h / 2.0;
        rig.core.input(&press(track.x + track.w - 1.0, y), now);
        rig.core.input(&release(track.x + track.w - 1.0, y), now);
        rig.core.sync(&Theme::dark(), now);
        assert_eq!(rig.core.shown.brightness, Some(100));

        // Mute switch: keeps its flipped state.
        let mut rig = rig_mute();
        let row = rect_of(&rig.core, mute_key(Control::Output));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now);
        rig.core.input(&release(x, y), now);
        rig.core.sync(&Theme::dark(), now);
        assert_eq!(rig.core.shown.audio.output.map(|l| l.muted), Some(true));
    }

    fn rig_drag() -> Rig {
        rig(Kind::Brightness, brightness(Some(10)))
    }

    fn rig_mute() -> Rig {
        rig(Kind::Volume, audio(Some((30, false)), None))
    }

    #[test]
    fn a_control_with_no_reading_takes_no_input() {
        let mut rig = rig(Kind::Volume, audio(None, None));
        let now = Instant::now();
        let row = rect_of(&rig.core, mute_key(Control::Output));
        let slider = rect_of(&rig.core, slider_key(Control::Output));
        for r in [row, slider] {
            let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            rig.core.input(&press(x, y), now);
            rig.core.input(&release(x, y), now);
        }
        assert!(rig.sent.borrow().is_empty());
        assert_eq!(rig.core.shown, audio(None, None));
    }
}
