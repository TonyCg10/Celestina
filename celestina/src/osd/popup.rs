//! The level controls of the Volume and Brightness pieces: the body of a
//! capsule sheet (CEL-D-002, CEL-D-003), the level as a slider and, for the
//! volume, the mute switch and the microphone's own slider and mute switch.
//! The body is 320 wide and draws no card: the sheet is the glass.
//!
//! The volume body also lists what the audio worker's graph holds (CEL-I-004):
//! under the output the sinks, as rows of which a click makes one the default;
//! under the microphone the sources, the same way; and last the applications
//! that play sound, each with its own mute and slider. Each list sits under a
//! header that folds it, and all start folded: a folded list shows the default
//! device, or the applications that play now. The rows, and with them the
//! body's size, change only when a header folds or unfolds ([`Shape`]), and
//! the sheet then follows the new size: a row updates in place (the default
//! mark, a level), a stream that ends greys out and takes no input, and one
//! that starts waits for the next fold or unfold.
//!
//! The body acts while it is open: a slider that moves, or a switch that
//! flips, sends a [`Request`] to the app's sink at once (not on close), and
//! the app turns it into worker commands ([`commands`]). What the body
//! shows is the app's reading, shared through a cell the app writes and the
//! body reads each frame; a slider also shows its own change before the
//! worker confirms it, and while the pointer is down the readings do not move
//! it, so a late reading never pulls the handle back under the finger.
//!
//! A drag is a burst of values: [`SEND_INTERVAL`] spaces the sends, the newest
//! value of each slider waits, and the last one is always sent (at release, at
//! close, or when its turn comes), so the worker sees a few writes and the
//! final level is never lost to the throttle.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use celestina_paint::{Role, TextStyle, Theme, Weight};
use celestina_shell::audio::AudioGraph;
use celestina_ui::{fit_middle, slider, view, Interaction, Key, Slider, Tree, UiEvent, View};
use celestina_wayland::InputEvent;

use super::audio_rows::{
    apps_list, device_list, list_h, section_of, Audio, GraphMirror, Lists, Section, Shape,
};
use super::monitor_rows::{
    monitors_card, monitors_size, MonitorFeed, MonitorMirror, SharedMonitors, ASK_HOLD,
};
use super::readings::Target;
use crate::island::menu::SharedPainter;
use crate::sheet::subtitle::{subtitle, SUBTITLE_H};
use crate::{
    copy,
    island::view::{centred, switch, text_style},
    pieces::{brightness::Brightness, volume::Volume, Levels},
    providers::{
        audio::{AudioCommand, Device},
        brightness::{BrightnessCommand, Change},
    },
    tray::model::scroll_steps,
};

/// How long after a wheel notch the next one on the same slider steps from
/// the level the wheel set rather than from what is shown.
const WHEEL_BURST: Duration = Duration::from_millis(1500);

/// The card's width when it lists devices and applications: room for an
/// application's name beside its switch and slider.
pub const AUDIO_W: f32 = 320.0;
/// The width of a level body in a capsule sheet (CEL-D-002).
pub const SHEET_W: f32 = 320.0;
/// The shortest gap between two sends of one slider while it is dragged.
pub const SEND_INTERVAL: Duration = Duration::from_millis(80);
/// The arrow keys' step, as the verbs' and the wheel's.
const STEP: u8 = celestina_shell::audio::STEP_PERCENT;
/// The icon beside a heading, logical px.
pub(super) const ICON: f32 = 18.0;

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
    /// The playback stream of this node id.
    Stream(u32),
    /// The monitor listed at this row of a popup of several.
    Monitor(u8),
    /// Every monitor with a reading, from the "All" slider.
    AllMonitors,
}

/// What the popup asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Make the listed device of this node id the default one.
    SetDefault(u32),
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

/// The commands a request is. `targets` are the monitors the brightness
/// sliders drive, in the order of the popup's rows (the popup's own output
/// first); a brightness request without its monitor is nothing, and so is the
/// "All" slider's, which the popup splits into one request per monitor.
#[must_use]
pub fn commands(request: Request, targets: &[Target]) -> Vec<Command> {
    let device = |control| match control {
        Control::Output => Some(Device::Output),
        Control::Input => Some(Device::Input),
        Control::Brightness | Control::Stream(_) | Control::Monitor(_) | Control::AllMonitors => {
            None
        }
    };
    let change = |target: Option<&Target>, percent| {
        target
            .map(|target| {
                Command::Brightness(BrightnessCommand::Change {
                    source: target.source,
                    monitor: target.monitor.clone(),
                    change: Change::Set(percent),
                })
            })
            .into_iter()
            .collect()
    };
    match request {
        Request::SetDefault(id) => vec![Command::Audio(AudioCommand::SetDefault { id })],
        Request::Set(Control::Stream(id), percent) => {
            vec![Command::Audio(AudioCommand::SetStreamVolume {
                id,
                percent,
            })]
        }
        Request::ToggleMute(Control::Stream(id)) => {
            vec![Command::Audio(AudioCommand::ToggleStreamMute { id })]
        }
        Request::Set(Control::Brightness, percent) => change(targets.first(), percent),
        Request::Set(Control::Monitor(at), percent) => {
            change(targets.get(usize::from(at)), percent)
        }
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

/// What the app writes and the popup reads beside the levels: the devices
/// and streams of the audio worker's last graph.
pub type SharedGraph = Rc<RefCell<AudioGraph>>;

pub(super) fn card_key() -> Key {
    Key::named("level-card")
}

/// The slider of `control`.
#[must_use]
pub fn slider_key(control: Control) -> Key {
    match control {
        Control::Output => Key::named("level-output"),
        Control::Input => Key::named("level-input"),
        Control::Brightness => Key::named("level-brightness"),
        Control::Stream(id) => Key::indexed("level-stream", u64::from(id)),
        Control::Monitor(at) => Key::indexed("level-monitor", u64::from(at)),
        Control::AllMonitors => Key::named("level-monitors-all"),
    }
}

/// The mute row of `control`.
#[must_use]
pub fn mute_key(control: Control) -> Key {
    match control {
        Control::Output => Key::named("level-mute-output"),
        Control::Input => Key::named("level-mute-input"),
        Control::Brightness => Key::named("level-mute-brightness"),
        Control::Stream(id) => Key::indexed("level-stream-mute", u64::from(id)),
        Control::Monitor(at) => Key::indexed("level-monitor-mute", u64::from(at)),
        Control::AllMonitors => Key::named("level-monitors-all-mute"),
    }
}

/// The slider control of `key`: the popup's own, a listed stream's or a
/// listed monitor's.
fn control_of_slider(key: Key, mirror: &GraphMirror, monitors: &MonitorMirror) -> Option<Control> {
    [Control::Output, Control::Input, Control::Brightness]
        .into_iter()
        .find(|c| slider_key(*c) == key)
        .or_else(|| mirror.stream_slider(key))
        .or_else(|| monitors.control_of(key))
}

/// The mute control of `key`: the popup's own or a listed stream's.
fn control_of_mute(key: Key, mirror: &GraphMirror) -> Option<Control> {
    [Control::Output, Control::Input]
        .into_iter()
        .find(|c| mute_key(*c) == key)
        .or_else(|| mirror.stream_mute(key))
}

/// A heading row's height, a mute row's height and the slider's.
pub(super) fn heading_h(theme: &Theme) -> f32 {
    theme.row_height
}

/// The subtitle row that starts a level body (CEL-D-008): the piece's icon
/// and level, as the bar shows them. It replaces the output's (and the
/// brightness') heading row.
pub(super) fn level_subtitle(theme: &Theme, kind: Kind, levels: Levels) -> View {
    let (face, name) = match kind {
        Kind::Volume => (Volume::face_of(levels), copy::PIECE_VOLUME),
        Kind::Brightness => (Brightness::face_of(levels), copy::PIECE_BRIGHTNESS),
    };
    subtitle(theme, &face, name, None)
}

pub(super) fn slider_h(theme: &Theme) -> f32 {
    theme.slider_handle_size + 2.0 * slider::MARGIN
}

/// The heights of the card's children, top to bottom: for audio a heading, a
/// slider and a mute row, then the list of devices if any; the microphone's
/// section the same way; then the applications' list.
fn children_h(theme: &Theme, kind: Kind, microphone: bool, shape: Shape) -> Vec<f32> {
    let lists = Lists::of(kind, microphone, shape);
    let mut heights = vec![SUBTITLE_H, slider_h(theme)];
    if kind == Kind::Volume {
        heights.push(theme.row_height);
        if lists.outputs > 0 {
            heights.push(list_h(theme, lists.outputs));
        }
        if microphone {
            heights.extend([heading_h(theme), slider_h(theme), theme.row_height]);
            if lists.inputs > 0 {
                heights.push(list_h(theme, lists.inputs));
            }
        }
        if lists.apps {
            heights.push(list_h(theme, lists.streams));
        }
    }
    heights
}

/// The size of a level body in a capsule sheet: 320 wide, the space above
/// its first row, the rows and the gaps between them, and the space below.
/// `shape` is ignored for brightness.
#[must_use]
pub fn body_size(theme: &Theme, kind: Kind, microphone: bool, shape: Shape) -> (f32, f32) {
    let heights = children_h(theme, kind, microphone, shape);
    let gaps = heights.len() - 1;
    (
        SHEET_W,
        theme.spacing.xs
            + heights.iter().sum::<f32>()
            + gaps as f32 * theme.spacing.sm
            + theme.spacing.md,
    )
}

pub(super) fn heading(
    theme: &Theme,
    key: &'static str,
    icon: &'static str,
    label: &str,
    percent: u8,
) -> View {
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

pub(super) fn level_slider(control: Control, percent: Option<u8>, muted: bool) -> View {
    let value = percent.unwrap_or(0);
    view::slider(
        slider_key(control),
        Slider::new(value)
            .with_step(STEP)
            .with_muted(muted)
            .with_enabled(percent.is_some()),
    )
}

/// The card for `levels` and `audio`: the body of a capsule sheet, which is
/// the glass already, so it draws no Haze and pads less above (the row of the
/// capsule sits over it). `microphone` says whether the
/// microphone's section is part of this popup and `shape` how many device and
/// application rows it shows (the microphone decided when it opened, the rows
/// when a section last folded or unfolded).
#[must_use]
pub fn card(
    theme: &Theme,
    kind: Kind,
    microphone: bool,
    levels: Levels,
    audio: Audio<'_>,
    shape: Shape,
) -> View {
    let lists = Lists::of(kind, microphone, shape);
    let (w, h) = body_size(theme, kind, microphone, shape);
    let mut card = view::col(card_key())
        .size(w, h)
        .padding_sides(theme.spacing.xs, theme.spacing.md, theme.spacing.md)
        .gap(theme.spacing.sm);
    match kind {
        Kind::Volume => {
            let out = levels.audio.output;
            card = card
                .child(level_subtitle(theme, kind, levels))
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
            if lists.outputs > 0 {
                card = card.child(device_list(
                    theme,
                    audio,
                    Section::Output,
                    "level-devices-output",
                    copy::AUDIO_OUTPUT_DEVICE,
                    (&audio.opened.outputs, &audio.live.outputs),
                    lists.outputs,
                ));
            }
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
                if lists.inputs > 0 {
                    card = card.child(device_list(
                        theme,
                        audio,
                        Section::Input,
                        "level-devices-input",
                        copy::AUDIO_INPUT_DEVICE,
                        (&audio.opened.inputs, &audio.live.inputs),
                        lists.inputs,
                    ));
                }
            }
            if lists.apps {
                card = card.child(apps_list(theme, audio, lists.streams));
            }
        }
        Kind::Brightness => {
            card = card
                .child(level_subtitle(theme, kind, levels))
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

/// The newest value each slider has not yet sent (one entry per control, a
/// stream's included), in the order first moved, and when one last went.
#[derive(Debug, Default)]
struct Throttle {
    pending: Vec<(Control, u8)>,
    last: Option<Instant>,
}

impl Throttle {
    fn push(&mut self, control: Control, percent: u8) {
        match self.pending.iter_mut().find(|(c, _)| *c == control) {
            Some(entry) => entry.1 = percent,
            None => self.pending.push((control, percent)),
        }
    }

    /// The requests due at `now`: all of them when `force`, else those whose
    /// turn has come (at most one send per [`SEND_INTERVAL`]). A value whose
    /// control is not `alive` any more (a stream that ended) is dropped, not
    /// sent.
    fn due(&mut self, now: Instant, force: bool, alive: impl Fn(Control) -> bool) -> Vec<Request> {
        let ready = force
            || self
                .last
                .is_none_or(|last| now.saturating_duration_since(last) >= SEND_INTERVAL);
        if !ready {
            return Vec::new();
        }
        let sent: Vec<Request> = self
            .pending
            .drain(..)
            .filter(|(control, _)| alive(*control))
            .map(|(control, percent)| Request::Set(control, percent))
            .collect();
        if !sent.is_empty() {
            self.last = Some(now);
        }
        sent
    }

    /// How long until something pending is due; `None` with nothing pending.
    fn wait(&self, now: Instant) -> Option<Duration> {
        if self.pending.is_empty() {
            return None;
        }
        let elapsed = self
            .last
            .map_or(SEND_INTERVAL, |last| now.saturating_duration_since(last));
        Some(SEND_INTERVAL.saturating_sub(elapsed))
    }
}

/// What a level card reads: the app's levels, the audio worker's graph and
/// the monitors a brightness card lists.
pub struct Feeds {
    pub cell: SharedLevels,
    pub graph: SharedGraph,
    pub monitors: MonitorFeed,
}

/// The popup without its painter: the tree, the pointer and keyboard state
/// and what it shows.
pub struct LevelCore {
    kind: Kind,
    microphone: bool,
    /// The devices and streams: the rows listed and what they show.
    mirror: GraphMirror,
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
    /// The slider the wheel last turned and the pixels short of a whole
    /// notch, so a smooth wheel steps once per notch.
    wheel_rest: (Option<Key>, i32),
    /// The control the wheel last set, the level it set and when: the next
    /// notch within [`WHEEL_BURST`] steps from that level, not from what is
    /// shown, so a late read-back of an earlier notch cannot lose a step.
    wheel_last: Option<(Control, u8, Instant)>,
    /// The single brightness slider's last asked level and when: shown until
    /// the monitor reads it back or the hold passes, as each row of the
    /// monitor list does.
    brightness_asked: Option<(u8, Instant)>,
    sink: Sink,
    cell: SharedLevels,
    graph: SharedGraph,
    /// The monitors a brightness sheet lists, when it lists several.
    monitors: MonitorMirror,
    monitor_cell: SharedMonitors,
    /// The theme the monitors' names were last cut for.
    cut_for: Option<Theme>,
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
    /// The popup of `kind` over `cell`'s readings and `graph`'s devices and
    /// streams in `theme`, under `reduced` motion. The
    /// microphone's section is part of a volume sheet when the microphone has
    /// a reading now, and the rows it lists are those `graph` holds now,
    /// every section folded. `fit` cuts the names to their columns during
    /// this call only: nothing stores a measure, so none can borrow a painter
    /// that is in use.
    #[must_use]
    pub fn new(
        theme: &Theme,
        kind: Kind,
        feeds: Feeds,
        sink: Sink,
        reduced: bool,
        now: Instant,
        fit: &mut dyn FnMut(&str, f32) -> String,
    ) -> LevelCore {
        let Feeds {
            cell,
            graph,
            monitors,
        } = feeds;
        let shown = cell.get();
        let microphone = kind == Kind::Volume && shown.audio.input.is_some();
        let mirror = match kind {
            Kind::Volume => GraphMirror::open(&graph.borrow(), theme, &mut *fit),
            Kind::Brightness => GraphMirror::default(),
        };
        let monitor_cell = Rc::clone(&monitors.levels);
        let monitors = match kind {
            Kind::Brightness => MonitorMirror::open(&monitors),
            Kind::Volume => MonitorMirror::default(),
        };
        let mut tree = Tree::new(reduced, theme);
        tree.apply(
            if monitors.is_multi() {
                monitors_card(theme, &monitors, shown)
            } else {
                card(
                    theme,
                    kind,
                    microphone,
                    shown,
                    mirror.audio(),
                    mirror.shape(),
                )
            },
            now,
        );
        LevelCore {
            kind,
            microphone,
            mirror,
            theme: theme.clone(),
            tree,
            input: Interaction::new(),
            shown,
            seen: shown,
            throttle: Throttle::default(),
            wheel_rest: (None, 0),
            wheel_last: None,
            brightness_asked: None,
            sink,
            cell,
            graph,
            monitors,
            monitor_cell,
            cut_for: None,
        }
    }

    /// What the card controls.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Whether the card includes the microphone.
    #[must_use]
    pub fn has_microphone(&self) -> bool {
        self.microphone
    }

    /// How many device and application rows the card shows.
    #[must_use]
    pub fn shape(&self) -> Shape {
        self.mirror.shape()
    }

    /// The size the card has now, logical px: the sheet reads it each frame,
    /// so a header folded or unfolded moves the sheet's edge.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        if self.monitors.is_multi() {
            monitors_size(&self.theme, self.monitors.row_count())
        } else {
            body_size(&self.theme, self.kind, self.microphone, self.shape())
        }
    }

    /// How long until the popup's own default mark, or a monitor row's held
    /// level, gives way, if one stands: the frame then shows the reading.
    #[must_use]
    pub fn graph_wait(&self, now: Instant) -> Option<Duration> {
        // A drag defers adoption, so no frame is due for it: asking for one
        // would repeat every frame until the release.
        if self.held() {
            None
        } else {
            let brightness = self
                .brightness_asked
                .map(|(_, since)| (since + ASK_HOLD).saturating_duration_since(now));
            [self.mirror.wait(now), self.monitors.wait(now), brightness]
                .into_iter()
                .flatten()
                .min()
        }
    }

    /// Whether a press is held on the card (a drag): readings then do not
    /// move the sliders.
    #[must_use]
    pub fn held(&self) -> bool {
        self.tree.highlight().pressed.is_some()
    }

    fn rebuild(&mut self, now: Instant) {
        let view = if self.monitors.is_multi() {
            monitors_card(&self.theme, &self.monitors, self.shown)
        } else {
            card(
                &self.theme,
                self.kind,
                self.microphone,
                self.shown,
                self.mirror.audio(),
                self.shape(),
            )
        };
        self.tree.apply(view, now);
    }

    /// Cuts the monitors' names to their column, at the weight each row draws
    /// it in, when the popup has not yet (its first frame) or the theme
    /// changed. `fit` is the measure the caller has in hand: the frame's own
    /// painter, which is borrowed there, so no closure that borrows it again
    /// may be used.
    pub fn cut_monitors(
        &mut self,
        theme: &Theme,
        now: Instant,
        fit: impl FnMut(&str, f32, Weight) -> String,
    ) {
        if !self.monitors.is_multi() || self.cut_for.as_ref() == Some(theme) {
            return;
        }
        self.cut_for = Some(theme.clone());
        self.monitors.cut(fit);
        self.rebuild(now);
    }

    /// Takes in a new theme and the app's newest reading; whether the look
    /// changed. `fit` cuts names with the measure the caller has in hand (the
    /// frame's own painter: the popup's stored cutter cannot borrow a painter
    /// that is in use), for a new theme's columns and for the name of a
    /// default device that no row listed.
    pub fn sync(
        &mut self,
        theme: &Theme,
        now: Instant,
        mut fit: impl FnMut(&str, f32) -> String,
    ) -> bool {
        let mut changed = false;
        if *theme != self.theme {
            self.theme = theme.clone();
            self.tree.set_theme(theme);
            if self.kind == Kind::Volume {
                self.mirror.recut(theme, &mut fit);
            }
            changed = true;
        }
        let external = self.cell.get();
        if !self.held() && external != self.seen {
            self.seen = external;
            self.shown = external;
            changed = true;
        }
        if !self.held() {
            changed |= self.hold_brightness(now);
        }
        if !self.held() && self.monitors.adopt(&self.monitor_cell, now) {
            changed = true;
        }
        if self.kind == Kind::Volume && !self.held() && self.mirror.sync(&self.graph.borrow(), now)
        {
            self.mirror.cut_defaults(&self.theme, &mut fit);
            changed = true;
        }
        if changed {
            self.rebuild(now);
        }
        changed
    }

    /// Sends what is due at `now` and returns how long until the rest is.
    pub fn flush(&mut self, now: Instant, force: bool) -> Option<Duration> {
        let due = {
            // The worker's graph, not the shown one: while a drag holds the
            // shown one, a stream that ended is already gone here.
            let live = self.graph.borrow();
            let rows = self.monitors.row_count();
            self.throttle.due(now, force, |control| match control {
                Control::Stream(id) => live.has_stream(id),
                Control::Monitor(at) => usize::from(at) < rows,
                Control::Output | Control::Input | Control::Brightness | Control::AllMonitors => {
                    true
                }
            })
        };
        for request in due {
            (self.sink)(request);
        }
        self.throttle.wait(now)
    }

    /// The style a device or application name is cut with in this card.
    #[must_use]
    pub fn name_style(&self) -> TextStyle {
        name_style(&self.theme)
    }

    /// How long until something the throttle holds is due.
    #[must_use]
    pub fn throttle_wait(&self, now: Instant) -> Option<Duration> {
        self.throttle.wait(now)
    }

    /// Lays the card out in its size with `lay` (a body is laid out in its
    /// own coordinates, (0, 0) at its top-left).
    pub fn layout_in(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        let size = self.size();
        lay(&mut self.tree, size);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "level body: layout fault");
        }
    }

    /// Paints the card at its own coordinates.
    pub fn paint(&mut self, painter: &mut celestina_paint::Painter, now: Instant) {
        self.tree.paint(painter, now);
    }

    /// The card's tree (the sheet reads its subtitle's place from it).
    #[must_use]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Whether the card's own animations (hover, a handle) still run.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now)
    }

    /// The rect of the card's node `key` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn node_rect(&self, key: Key) -> Option<celestina_paint::Rect> {
        self.tree.node_by_key(key).map(|id| self.tree.rect(id))
    }

    /// The key of the slider under (`x`, `y`) of the card, if one is.
    #[must_use]
    pub fn slider_at(&self, x: f32, y: f32, now: Instant) -> Option<Key> {
        self.tree.slider_at(x, y, now)
    }

    fn set_shown(&mut self, control: Control, percent: u8, now: Instant) {
        let slot = match control {
            Control::Output => self.shown.audio.output.as_mut().map(|l| &mut l.percent),
            Control::Input => self.shown.audio.input.as_mut().map(|l| &mut l.percent),
            Control::Brightness => {
                if self.shown.brightness.is_some() {
                    self.brightness_asked = Some((percent, now));
                }
                self.shown.brightness.as_mut()
            }
            Control::Stream(id) => return self.mirror.set_level(id, percent),
            Control::Monitor(at) => return self.monitors.set_level(usize::from(at), percent, now),
            Control::AllMonitors => return,
        };
        if let Some(slot) = slot {
            *slot = percent;
        }
    }

    /// Keeps the single brightness slider at its asked level while the
    /// monitor's readings say otherwise, until it reads that level back or
    /// [`ASK_HOLD`] passes. Whether what is shown changed.
    fn hold_brightness(&mut self, now: Instant) -> bool {
        let Some((want, since)) = self.brightness_asked else {
            return false;
        };
        let read = self.seen.brightness;
        let shown = if read == Some(want) || now.duration_since(since) >= ASK_HOLD {
            self.brightness_asked = None;
            read
        } else {
            Some(want)
        };
        let changed = self.shown.brightness != shown && self.shown.brightness.is_some();
        if self.shown.brightness.is_some() {
            self.shown.brightness = shown;
        }
        changed
    }

    /// The popup asks `percent` of `control`: shown at once and sent through
    /// the throttle. All gives every monitor with a reading the level, each
    /// as a request of its own.
    fn ask_level(&mut self, control: Control, percent: u8, now: Instant) {
        if control == Control::AllMonitors {
            for at in self.monitors.known() {
                let row = Control::Monitor(u8::try_from(at).unwrap_or(u8::MAX));
                self.set_shown(row, percent, now);
                self.throttle.push(row, percent);
            }
        } else {
            self.set_shown(control, percent, now);
            self.throttle.push(control, percent);
        }
    }

    /// The level `control`'s slider shows, if it has one.
    fn shown_level(&self, control: Control) -> Option<u8> {
        match control {
            Control::Output => self.shown.audio.output.map(|l| l.percent),
            Control::Input => self.shown.audio.input.map(|l| l.percent),
            Control::Brightness => self.shown.brightness,
            Control::Stream(id) => self.mirror.stream_level(id),
            Control::Monitor(at) => self.monitors.level(usize::from(at)),
            Control::AllMonitors => self.monitors.mean(),
        }
    }

    /// A wheel turn of `vertical` px over `control`'s slider (`key`): one
    /// percent per notch, up raises; a slider with no level takes none.
    /// Whether the level moved.
    fn wheel(&mut self, key: Key, control: Control, vertical: f32, now: Instant) -> bool {
        if self.held() {
            return false;
        }
        if self.wheel_rest.0 != Some(key) {
            self.wheel_rest = (Some(key), 0);
        }
        let px = if vertical.is_finite() {
            vertical.round() as i32
        } else {
            0
        };
        let notches = scroll_steps(px, &mut self.wheel_rest.1);
        let shown = self.shown_level(control).filter(|_| notches != 0);
        let Some(level) = shown.map(|shown| match self.wheel_last {
            Some((last, level, at)) if last == control && now.duration_since(at) < WHEEL_BURST => {
                level
            }
            _ => shown,
        }) else {
            return false;
        };
        let percent = (i32::from(level) - notches).clamp(0, 100);
        let percent = u8::try_from(percent).unwrap_or(level);
        // All starts from the rounded mean; rows that differ still take the
        // level, so 100 and 99 meet at 100 on a notch up.
        let unmoved = if control == Control::AllMonitors {
            self.monitors
                .known()
                .into_iter()
                .all(|at| self.monitors.level(at) == Some(percent))
        } else {
            percent == level
        };
        if unmoved {
            return false;
        }
        self.wheel_last = Some((control, percent, now));
        self.ask_level(control, percent, now);
        true
    }

    fn flip_shown(&mut self, control: Control) {
        let level = match control {
            Control::Output => self.shown.audio.output.as_mut(),
            Control::Input => self.shown.audio.input.as_mut(),
            Control::Brightness | Control::Monitor(_) | Control::AllMonitors => None,
            Control::Stream(id) => return self.mirror.flip_mute(id),
        };
        if let Some(level) = level {
            level.muted = !level.muted;
        }
    }

    /// Handles one event: Escape closes; a slider moved sends its level; a
    /// click or Enter on a mute row flips it, on a device row makes it the
    /// default, on a section's header folds or unfolds the section (`fit`
    /// cuts the names of the rows it lists, during this call only).
    pub fn input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        fit: &mut dyn FnMut(&str, f32) -> String,
    ) -> Outcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::FocusLost => self.input.key(&mut self.tree, ev),
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let mut moved = false;
        for event in &events {
            match event {
                UiEvent::Escape => return Outcome::Close,
                UiEvent::SliderChanged(key, percent) => {
                    if let Some(control) = control_of_slider(*key, &self.mirror, &self.monitors) {
                        self.ask_level(control, *percent, now);
                        moved = true;
                    }
                }
                UiEvent::Scroll(key, _, vertical) => {
                    if let Some(control) = control_of_slider(*key, &self.mirror, &self.monitors) {
                        moved |= self.wheel(*key, control, *vertical, now);
                    }
                }
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(control) = control_of_mute(*key, &self.mirror) {
                        self.flip_shown(control);
                        (self.sink)(Request::ToggleMute(control));
                        moved = true;
                    } else if let Some(section) = section_of(*key) {
                        self.mirror.toggle(section, &self.theme, &mut *fit);
                        moved = true;
                    } else if let Some(id) = self.mirror.device_of(*key) {
                        // The row answers only while the worker lists it;
                        // the default's own row asks for nothing.
                        if self.mirror.has_device(id) && self.mirror.mark_default(id, now) {
                            (self.sink)(Request::SetDefault(id));
                            moved = true;
                        }
                    }
                }
                UiEvent::HoverChanged(..) | UiEvent::FieldChanged(_) => {}
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

/// The style a device or application name is cut with.
pub(crate) fn name_style(theme: &Theme) -> TextStyle {
    text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi)
}

/// A cutter that measures with the shared `painter` for the length of one
/// call: it finds the painter borrowed (a frame is drawing) and then leaves a
/// name whole. Built per call, never stored.
pub(crate) fn shared_fit(
    painter: &SharedPainter,
    style: TextStyle,
) -> impl FnMut(&str, f32) -> String {
    let painter = Rc::clone(painter);
    move |name: &str, width: f32| match painter.try_borrow_mut() {
        Ok(mut painter) => fit_middle(name, width, |text| {
            painter.text().measure(text, &style, None, 1.0).0
        }),
        Err(_) => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::{Rect, TextStyle};
    use celestina_ui::BTN_LEFT;
    use celestina_wayland::Modifiers;

    use celestina_shell::audio::{AudioLevel, GraphDevice, GraphStream};

    use super::*;
    use crate::osd::audio_rows::{
        self, compact_h, device_key, device_mark_key, header_h, section_key, Folds,
    };
    use crate::osd::monitor_rows::MonitorRow;
    use crate::pieces::{AudioLevels, Level};
    use crate::providers::brightness::BrightnessSource;

    const ESCAPE: u32 = 0xff1b;
    const RIGHT: u32 = 0xff53;
    const TAB: u32 = 0xff09;
    const ENTER: u32 = 0xff0d;

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

    fn device(id: u32, name: &str, default: bool) -> GraphDevice {
        GraphDevice {
            id,
            name: name.to_owned(),
            default,
        }
    }

    fn stream(id: u32, name: &str, level: Option<(u8, bool)>) -> GraphStream {
        GraphStream {
            id,
            name: name.to_owned(),
            level: level.map(|(percent, muted)| AudioLevel { percent, muted }),
            playing: false,
        }
    }

    /// `stream` as one that plays.
    fn playing(mut stream: GraphStream) -> GraphStream {
        stream.playing = true;
        stream
    }

    /// Two sinks (the first the default), two sources, two streams (Firefox
    /// plays, Mpv is paused).
    fn full_graph() -> AudioGraph {
        AudioGraph {
            outputs: vec![device(5, "Speakers", true), device(6, "Headphones", false)],
            inputs: vec![device(9, "Desk mic", true), device(10, "Webcam mic", false)],
            streams: vec![
                playing(stream(20, "Firefox", Some((60, false)))),
                stream(21, "Mpv", Some((30, true))),
            ],
        }
    }

    struct Rig {
        core: LevelCore,
        sent: Rc<std::cell::RefCell<Vec<Request>>>,
        cell: SharedLevels,
        graph: SharedGraph,
        monitor_cell: SharedMonitors,
    }

    fn rig(kind: Kind, levels: Levels) -> Rig {
        rig_with(kind, levels, AudioGraph::default())
    }

    /// Cuts a name as the fake measure sees it: what the frame's painter does.
    fn cut(name: &str, width: f32) -> String {
        let style = text_style(
            Theme::dark().type_scale.body,
            Weight::Regular,
            false,
            Role::TextHi,
        );
        fit_middle(name, width, |s| fake_measure(s, &style, None).0)
    }

    fn rig_with(kind: Kind, levels: Levels, graph: AudioGraph) -> Rig {
        rig_with_fit(kind, levels, graph, cut)
    }

    /// A rig whose popup opens with `fit` as its cutter.
    fn rig_with_fit(
        kind: Kind,
        levels: Levels,
        graph: AudioGraph,
        fit: impl FnMut(&str, f32) -> String,
    ) -> Rig {
        rig_full(kind, levels, graph, MonitorFeed::default(), fit)
    }

    fn rig_full(
        kind: Kind,
        levels: Levels,
        graph: AudioGraph,
        monitors: MonitorFeed,
        mut fit: impl FnMut(&str, f32) -> String,
    ) -> Rig {
        let theme = Theme::dark();
        let sent = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink: Sink = {
            let sent = Rc::clone(&sent);
            Rc::new(move |request| sent.borrow_mut().push(request))
        };
        let cell: SharedLevels = Rc::new(Cell::new(levels));
        let graph: SharedGraph = Rc::new(RefCell::new(graph));
        let monitor_cell = Rc::clone(&monitors.levels);
        let mut core = LevelCore::new(
            &theme,
            kind,
            Feeds {
                cell: Rc::clone(&cell),
                graph: Rc::clone(&graph),
                monitors,
            },
            sink,
            false,
            Instant::now(),
            &mut fit,
        );
        let size = core.size();
        core.tree.layout_measured(size, fake_measure);
        Rig {
            core,
            sent,
            cell,
            graph,
            monitor_cell,
        }
    }

    /// Lays the tree out again after the shown rows changed.
    fn relayout(rig: &mut Rig) {
        let size = rig.core.size();
        rig.core.tree.layout_measured(size, fake_measure);
    }

    /// Clicks the header of `section`, as a person does, and lays out again.
    fn toggle(rig: &mut Rig, section: Section) {
        click(rig, section_key(section));
        relayout(rig);
    }

    /// Flips every section the popup has (without the microphone, not its).
    fn unfold_all(rig: &mut Rig) {
        for section in [Section::Output, Section::Input, Section::Apps] {
            if rig.core.tree.node_by_key(section_key(section)).is_some() {
                toggle(rig, section);
            }
        }
    }

    fn centre_of(core: &LevelCore, key: Key) -> (f32, f32) {
        let r = rect_of(core, key);
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    }

    fn click(rig: &mut Rig, key: Key) {
        let (x, y) = centre_of(&rig.core, key);
        let now = Instant::now();
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
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
            commands(Request::Set(Control::Output, 40), &[]),
            [Command::Audio(AudioCommand::SetVolume {
                device: Device::Output,
                percent: 40
            })]
        );
        assert_eq!(
            commands(Request::Set(Control::Input, 10), std::slice::from_ref(&ddc)),
            [Command::Audio(AudioCommand::SetVolume {
                device: Device::Input,
                percent: 10
            })]
        );
        assert_eq!(
            commands(Request::ToggleMute(Control::Input), &[]),
            [Command::Audio(AudioCommand::ToggleMute {
                device: Device::Input
            })]
        );
        assert_eq!(
            commands(
                Request::Set(Control::Brightness, 70),
                std::slice::from_ref(&ddc)
            ),
            [Command::Brightness(BrightnessCommand::Change {
                source: BrightnessSource::Ddc,
                monitor: "DP-1".to_owned(),
                change: Change::Set(70)
            })]
        );
        assert_eq!(
            commands(Request::SetDefault(42), &[]),
            [Command::Audio(AudioCommand::SetDefault { id: 42 })]
        );
        assert_eq!(
            commands(
                Request::Set(Control::Stream(7), 55),
                std::slice::from_ref(&ddc)
            ),
            [Command::Audio(AudioCommand::SetStreamVolume {
                id: 7,
                percent: 55
            })]
        );
        assert_eq!(
            commands(Request::ToggleMute(Control::Stream(7)), &[]),
            [Command::Audio(AudioCommand::ToggleStreamMute { id: 7 })]
        );
        // A list of monitors: each row addresses its own, the first is the
        // popup's output; the All slider and a mute address none.
        let hdmi = Target {
            source: BrightnessSource::Backlight,
            monitor: "intel_backlight".to_owned(),
        };
        let both = [ddc.clone(), hdmi.clone()];
        assert_eq!(
            commands(Request::Set(Control::Brightness, 5), &both),
            commands(Request::Set(Control::Monitor(0), 5), &both)
        );
        assert_eq!(
            commands(Request::Set(Control::Monitor(1), 20), &both),
            [Command::Brightness(BrightnessCommand::Change {
                source: BrightnessSource::Backlight,
                monitor: "intel_backlight".to_owned(),
                change: Change::Set(20)
            })]
        );
        assert!(commands(Request::Set(Control::Monitor(2), 20), &both).is_empty());
        assert!(commands(Request::Set(Control::AllMonitors, 20), &both).is_empty());
        assert!(commands(Request::ToggleMute(Control::Monitor(0)), &both).is_empty());
        assert!(commands(Request::ToggleMute(Control::AllMonitors), &both).is_empty());
        // No monitor, no brightness; brightness has no mute.
        assert!(commands(Request::Set(Control::Brightness, 70), &[]).is_empty());
        assert!(commands(
            Request::ToggleMute(Control::Brightness),
            std::slice::from_ref(&ddc)
        )
        .is_empty());
    }

    #[test]
    fn the_card_fills_the_body_in_every_shape() {
        let theme = Theme::dark();
        let shape = Shape::default();
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
            let size = body_size(&theme, kind, mic, shape);
            assert_eq!(size.0, SHEET_W);
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
            body_size(&theme, Kind::Volume, true, shape).1
                > body_size(&theme, Kind::Volume, false, shape).1
        );
        assert!(
            body_size(&theme, Kind::Volume, false, shape).1
                > body_size(&theme, Kind::Brightness, false, shape).1
        );
        // The brightness body is as wide as the volume's.
        assert_eq!(body_size(&theme, Kind::Brightness, false, shape).0, SHEET_W);
    }

    #[test]
    fn the_brightness_body_is_320_wide_single_or_listed() {
        let theme = Theme::dark();
        // The single slider: 320 wide, the level padding all round.
        let rig = rig(Kind::Brightness, brightness(Some(60)));
        let single = body_size(&theme, Kind::Brightness, false, Shape::default());
        assert_eq!(single.0, SHEET_W);
        assert_eq!(rig.core.size(), single);
        let card = rect_of(&rig.core, card_key());
        assert_eq!((card.w, card.h), single);
        let first = rect_of(&rig.core, crate::sheet::subtitle::row_key());
        assert_eq!(first.y, theme.spacing.xs);
        // Several monitors: the list is the same 320 wide, padded the same.
        let feed = MonitorFeed {
            rows: (0..3)
                .map(|at| MonitorRow {
                    name: format!("monitor {at}"),
                    primary: at == 0,
                })
                .collect(),
            levels: Rc::new(RefCell::new(vec![Some(20), Some(40), None])),
        };
        let listed = rig_full(
            Kind::Brightness,
            brightness(Some(20)),
            AudioGraph::default(),
            feed,
            cut,
        );
        let size = monitors_size(&theme, 3);
        assert_eq!(size.0, SHEET_W);
        assert_eq!(listed.core.size(), size);
        let card = rect_of(&listed.core, card_key());
        assert_eq!((card.w, card.h), size);
        let first = rect_of(&listed.core, crate::sheet::subtitle::row_key());
        assert_eq!(first.y, theme.spacing.xs);
        let rows = rect_of(&listed.core, Key::named("level-monitors"));
        assert_eq!(rows.y + rows.h + theme.spacing.md, size.1);
    }

    #[test]
    fn the_volume_body_is_320_wide_and_as_tall_as_its_rows() {
        let theme = Theme::dark();
        let shape = Shape::default();
        for (levels, mic) in [
            (audio(Some((30, false)), None), false),
            (audio(Some((30, false)), Some((80, true))), true),
        ] {
            let rig = rig(Kind::Volume, levels);
            assert_eq!(rig.core.has_microphone(), mic);
            let size = body_size(&theme, Kind::Volume, mic, shape);
            assert_eq!(size.0, SHEET_W);
            assert_eq!(rig.core.size(), size);
            let card = rect_of(&rig.core, card_key());
            assert_eq!((card.x, card.y, card.w, card.h), (0.0, 0.0, size.0, size.1));
            // The rows: the space above is `xs`, the space below `md`, and
            // the first row starts at the top padding.
            let first = rect_of(&rig.core, crate::sheet::subtitle::row_key());
            assert_eq!(first.y, theme.spacing.xs);
            assert_eq!(first.x, theme.spacing.md);
            let last = [Key::named("level-mute-output"), mute_key(Control::Input)]
                .into_iter()
                .rev()
                .filter_map(|k| {
                    rig.core
                        .tree
                        .node_by_key(k)
                        .map(|id| rig.core.tree.rect(id))
                })
                .next();
            let last = last.map_or(0.0, |r| r.y + r.h);
            assert_eq!(last + theme.spacing.md, size.1);
        }
        // No Haze: the sheet is the glass. Wider with lists or not, always 320.
        assert_eq!(
            body_size(&theme, Kind::Volume, true, shape).0,
            body_size(&theme, Kind::Volume, false, Shape::default()).0
        );
        assert!(
            body_size(&theme, Kind::Volume, true, shape).1
                > body_size(&theme, Kind::Volume, false, shape).1
        );
    }

    /// The text of the node `key` of `view`.
    fn text_in(view: &View, key: Key) -> Option<String> {
        if view.key == key {
            if let celestina_ui::ViewKind::Text { text, .. } = &view.kind {
                return Some(text.clone());
            }
        }
        view.children.iter().find_map(|c| text_in(c, key))
    }

    #[test]
    fn the_level_body_has_a_subtitle_and_no_heading_row_for_its_own_level() {
        use crate::sheet::subtitle::{row_key, text_key, SUBTITLE_H};
        let theme = Theme::dark();
        let mic = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        let single = rig(Kind::Brightness, brightness(Some(60)));
        for rig in [&mic, &single] {
            let first = rect_of(&rig.core, row_key());
            assert_eq!((first.y, first.h), (theme.spacing.xs, SUBTITLE_H));
            for gone in ["level-heading-output", "level-heading-brightness"] {
                assert!(rig.core.tree.node_by_key(Key::named(gone)).is_none());
            }
        }
        // The microphone keeps its own heading, under the output's rows.
        assert!(mic
            .core
            .tree
            .node_by_key(Key::named("level-heading-input"))
            .is_some());
        // The height is the subtitle, the rows and the gaps, as before.
        let heights = children_h(&theme, Kind::Brightness, false, Shape::default());
        assert_eq!(heights[0], SUBTITLE_H);
        assert_eq!(
            single.core.size().1,
            theme.spacing.xs + SUBTITLE_H + theme.spacing.sm + slider_h(&theme) + theme.spacing.md
        );
        assert_eq!(
            text_in(&shown_card(&single.core), text_key()).as_deref(),
            Some(copy::osd_percent(60).as_str())
        );
    }

    #[test]
    fn the_volume_subtitle_shows_the_live_percent() {
        use crate::sheet::subtitle::text_key;
        let mut rig = rig(Kind::Volume, audio(Some((30, false)), None));
        let shown = |rig: &Rig| text_in(&shown_card(&rig.core), text_key());
        assert_eq!(shown(&rig).as_deref(), Some(copy::osd_percent(30).as_str()));
        // The volume moves while the sheet is open.
        rig.cell.set(audio(Some((64, false)), None));
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        assert_eq!(shown(&rig).as_deref(), Some(copy::osd_percent(64).as_str()));
        // No reading: the menu's name.
        let none = rig_with(Kind::Volume, audio(None, None), AudioGraph::default());
        assert_eq!(
            text_in(&shown_card(&none.core), text_key()).as_deref(),
            Some(copy::PIECE_VOLUME)
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
            &mut cut,
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
        rig.core.input(&press(track.x + 20.0, y), t0, &mut cut);
        // Moves inside the interval wait; none is sent twice.
        for dx in [40.0, 80.0, 120.0] {
            rig.core.input(&motion(track.x + dx, y), t0, &mut cut);
        }
        assert_eq!(rig.sent.borrow().len(), 1, "one send within the interval");
        let waiting = rig.core.throttle.wait(t0);
        assert!(waiting.is_some_and(|w| w <= SEND_INTERVAL));
        // Its turn comes: the newest value goes, once.
        let later = t0 + SEND_INTERVAL;
        assert!(rig.core.flush(later, false).is_none());
        assert_eq!(rig.sent.borrow().len(), 2);
        // More motion, then the release sends the last level at once.
        rig.core.input(&motion(track.x + 150.0, y), later, &mut cut);
        let before = rig.sent.borrow().len();
        rig.core
            .input(&release(track.x + 150.0, y), later, &mut cut);
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
        rig.core
            .input(&press(track.x + track.w - 1.0, y), now, &mut cut);
        rig.cell.set(brightness(Some(33)));
        assert!(!rig.core.sync(&Theme::dark(), now, cut), "held: no change");
        assert_eq!(rig.core.shown.brightness, Some(100));
        rig.core
            .input(&release(track.x + track.w - 1.0, y), now, &mut cut);
        // After the release the monitor has not read 100 back yet: the
        // slider keeps it until the hold passes (CEL-I-009), then follows.
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!(rig.core.shown.brightness, Some(100));
        assert!(rig.core.sync(&Theme::dark(), now + ASK_HOLD, cut));
        assert_eq!(rig.core.shown.brightness, Some(33));
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Brightness))
                .map(Slider::value),
            Some(33)
        );
        // The same reading again changes nothing.
        assert!(!rig.core.sync(&Theme::dark(), now + ASK_HOLD, cut));
    }

    #[test]
    fn a_click_on_a_mute_row_flips_it_at_once_and_asks_for_it() {
        let mut rig = rig(Kind::Volume, audio(Some((30, false)), Some((5, true))));
        let now = Instant::now();
        let row = rect_of(&rig.core, mute_key(Control::Output));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
        assert_eq!(*rig.sent.borrow(), [Request::ToggleMute(Control::Output)]);
        assert_eq!(rig.core.shown.audio.output.map(|l| l.muted), Some(true));
        assert_eq!(rig.core.shown.audio.output.map(|l| l.percent), Some(30));
        // The microphone's row is its own.
        let row = rect_of(&rig.core, mute_key(Control::Input));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
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
        rig.core.input(&key(TAB), now, &mut cut);
        rig.core.input(&key(RIGHT), now, &mut cut);
        assert_eq!(*rig.sent.borrow(), [Request::Set(Control::Brightness, 55)]);
        assert_eq!(rig.core.input(&key(ESCAPE), now, &mut cut), Outcome::Close);
    }

    #[test]
    fn a_sync_does_not_undo_what_the_popup_changed_before_the_reading_follows() {
        // Keyboard: the cell still holds the old reading after a step.
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let now = Instant::now();
        rig.core.input(&key(TAB), now, &mut cut);
        rig.core.input(&key(RIGHT), now, &mut cut);
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!(rig.core.shown.brightness, Some(55));
        rig.core.input(&key(RIGHT), now, &mut cut);
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!(rig.core.shown.brightness, Some(60), "steps accumulate");
        // The reading catching up is adopted.
        rig.cell.set(brightness(Some(60)));
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!(rig.core.shown.brightness, Some(60));

        // Drag: the release does not jump back to the old reading.
        let mut rig = rig_drag();
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let y = track.y + track.h / 2.0;
        rig.core
            .input(&press(track.x + track.w - 1.0, y), now, &mut cut);
        rig.core
            .input(&release(track.x + track.w - 1.0, y), now, &mut cut);
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!(rig.core.shown.brightness, Some(100));

        // Mute switch: keeps its flipped state.
        let mut rig = rig_mute();
        let row = rect_of(&rig.core, mute_key(Control::Output));
        let (x, y) = (row.x + 10.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
        rig.core.sync(&Theme::dark(), now, cut);
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
            rig.core.input(&press(x, y), now, &mut cut);
            rig.core.input(&release(x, y), now, &mut cut);
        }
        assert!(rig.sent.borrow().is_empty());
        assert_eq!(rig.core.shown, audio(None, None));
    }

    /// Whether `view` or anything under it is keyed `key`.
    fn view_has(view: &View, key: Key) -> bool {
        view.key == key || view.children.iter().any(|c| view_has(c, key))
    }

    /// The card the popup shows now, as a view: what the rows say, free of
    /// the nodes still fading out of the tree.
    fn shown_card(core: &LevelCore) -> View {
        card(
            &Theme::dark(),
            core.kind,
            core.microphone,
            core.shown,
            core.mirror.audio(),
            core.shape(),
        )
    }

    /// Whether the row of device `id` wears the default's check.
    fn marked(rig: &Rig, id: u32) -> bool {
        view_has(&shown_card(&rig.core), device_mark_key(id))
    }

    /// The y of the top of a node.
    fn y_of(core: &LevelCore, key: Key) -> f32 {
        rect_of(core, key).y
    }

    #[test]
    fn the_volume_card_lists_devices_and_streams_in_order() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        assert!(rig.core.has_microphone());
        unfold_all(&mut rig);
        assert_eq!(
            rig.core.shape(),
            Shape {
                outputs: 2,
                inputs: 2,
                streams: 2,
                listed: true,
            }
        );
        // Design section 3, top to bottom: the output's slider and mute,
        // its devices, the microphone's slider and mute, its devices, then
        // the applications; each list under its header.
        let order = [
            slider_key(Control::Output),
            mute_key(Control::Output),
            section_key(Section::Output),
            device_key(5),
            device_key(6),
            slider_key(Control::Input),
            mute_key(Control::Input),
            section_key(Section::Input),
            device_key(9),
            device_key(10),
            section_key(Section::Apps),
            slider_key(Control::Stream(20)),
            slider_key(Control::Stream(21)),
        ];
        let ys: Vec<f32> = order.iter().map(|k| y_of(&rig.core, *k)).collect();
        assert!(ys.windows(2).all(|w| w[0] < w[1]), "not in order: {ys:?}");
        // The labels are the copy's, and the card holds all of it.
        let theme = Theme::dark();
        let size = body_size(&theme, Kind::Volume, true, rig.core.shape());
        let card = rect_of(&rig.core, card_key());
        assert_eq!((card.w, card.h), size);
        assert_eq!(size.0, AUDIO_W);
        for key in order {
            let r = rect_of(&rig.core, key);
            assert!(
                r.x >= 0.0 && r.x + r.w <= size.0 && r.y + r.h <= size.1,
                "{key:?} {r:?} in {size:?}"
            );
        }
        assert!(rig
            .core
            .tree
            .node_by_key(Key::named("level-apps-none"))
            .is_none());
        // No application plays: the one quiet line, no rows.
        let quiet = rig_with(
            Kind::Volume,
            audio(Some((30, false)), None),
            AudioGraph {
                outputs: vec![device(5, "Speakers", true)],
                ..AudioGraph::default()
            },
        );
        assert!(quiet
            .core
            .tree
            .node_by_key(Key::named("level-apps-none"))
            .is_some());
        // Without the microphone its devices are not listed either.
        let mut no_mic = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        unfold_all(&mut no_mic);
        assert!(no_mic.core.tree.node_by_key(device_key(9)).is_none());
        assert!(no_mic.core.tree.node_by_key(device_key(5)).is_some());
    }

    /// Whether the card the popup shows has a node keyed `key`.
    fn has_node(rig: &Rig, key: Key) -> bool {
        view_has(&shown_card(&rig.core), key)
    }

    #[test]
    fn sections_start_folded_every_time_the_popup_opens() {
        let mut first = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        assert_eq!(first.core.mirror.audio().folds, Folds::default());
        for section in [Section::Output, Section::Input, Section::Apps] {
            assert!(!first.core.mirror.audio().folds.is_open(section));
        }
        unfold_all(&mut first);
        for section in [Section::Output, Section::Input, Section::Apps] {
            assert!(first.core.mirror.audio().folds.is_open(section));
        }
        // The state is not kept: the next popup is folded again.
        let second = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        for section in [Section::Output, Section::Input, Section::Apps] {
            assert!(!second.core.mirror.audio().folds.is_open(section));
        }
        assert_eq!(second.core.size(), first_folded_size());
        assert!(second.core.size().1 < first.core.size().1);
    }

    /// What a volume popup over `full_graph` with the microphone measures
    /// when every section is folded.
    fn first_folded_size() -> (f32, f32) {
        rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        )
        .core
        .size()
    }

    #[test]
    fn folded_sections_show_the_default_device_and_the_streams_that_play() {
        // Here the second stream plays and the first is paused.
        let mut graph = full_graph();
        graph.streams[0].playing = false;
        graph.streams[1].playing = true;
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            graph,
        );
        // Folded: the default sink and source with their check, the stream
        // that plays; not the other devices, nor the paused stream.
        for (key, shown) in [
            (device_key(5), true),
            (device_key(6), false),
            (device_key(9), true),
            (device_key(10), false),
            (slider_key(Control::Stream(20)), false),
            (slider_key(Control::Stream(21)), true),
        ] {
            assert_eq!(has_node(&rig, key), shown, "{key:?} folded");
        }
        assert!(marked(&rig, 5) && marked(&rig, 9));
        assert!(!has_node(&rig, Key::named("level-apps-none")));
        assert_eq!(
            rig.core.shape(),
            Shape {
                outputs: 1,
                inputs: 1,
                streams: 1,
                listed: true,
            }
        );
        // Unfolded: every device and every stream, the paused one too.
        unfold_all(&mut rig);
        for key in [
            device_key(5),
            device_key(6),
            device_key(9),
            device_key(10),
            slider_key(Control::Stream(20)),
            slider_key(Control::Stream(21)),
        ] {
            assert!(has_node(&rig, key), "{key:?} unfolded");
        }
        // Folded again: back to the default and the stream that plays.
        unfold_all(&mut rig);
        assert!(!has_node(&rig, device_key(6)));
        assert!(!has_node(&rig, slider_key(Control::Stream(20))));
        assert!(has_node(&rig, slider_key(Control::Stream(21))));
    }

    #[test]
    fn with_no_application_playing_the_folded_list_says_so() {
        let mut graph = full_graph();
        graph.streams[0].playing = false;
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), graph);
        assert!(has_node(&rig, Key::named("level-apps-none")));
        assert!(!has_node(&rig, slider_key(Control::Stream(20))));
        assert!(!has_node(&rig, slider_key(Control::Stream(21))));
        // Unfolded, the paused ones are rows and the line is gone.
        toggle(&mut rig, Section::Apps);
        assert!(!has_node(&rig, Key::named("level-apps-none")));
        assert!(has_node(&rig, slider_key(Control::Stream(20))));
        assert!(has_node(&rig, slider_key(Control::Stream(21))));
        // No stream at all: the line, folded or not.
        let mut none = rig_with(
            Kind::Volume,
            audio(Some((30, false)), None),
            AudioGraph {
                outputs: vec![device(5, "Speakers", true)],
                ..AudioGraph::default()
            },
        );
        assert!(has_node(&none, Key::named("level-apps-none")));
        toggle(&mut none, Section::Apps);
        assert!(has_node(&none, Key::named("level-apps-none")));
    }

    #[test]
    fn a_header_click_changes_the_height_and_keeps_the_top_edge() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        let folded = rig.core.size();
        click(&mut rig, section_key(Section::Output));
        let unfolded = rig.core.size();
        assert_eq!(unfolded.0, folded.0, "the width stays");
        assert!(unfolded.1 > folded.1, "{unfolded:?} grows from {folded:?}");
        // The size is what the sheet reads: the same until something folds.
        assert_eq!(rig.core.size(), unfolded);
        relayout(&mut rig);
        let card = rect_of(&rig.core, card_key());
        assert_eq!((card.x, card.y), (0.0, 0.0), "the top edge stays");
        assert_eq!((card.w, card.h), unfolded);
        // Folding it back gives the old size.
        click(&mut rig, section_key(Section::Output));
        assert_eq!(rig.core.size(), folded);
        // A click on a row does not change the size.
        click(&mut rig, section_key(Section::Output));
        relayout(&mut rig);
        let unfolded = rig.core.size();
        click(&mut rig, device_key(6));
        assert_eq!(rig.core.size(), unfolded);
    }

    #[test]
    fn a_folded_section_follows_a_new_default_listed_or_not_at_the_same_height() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        let folded = rig.core.size();
        assert!(marked(&rig, 5) && has_node(&rig, device_key(5)));
        // (a) A listed device becomes the default: its row replaces the old.
        let mut graph = full_graph();
        graph.outputs = vec![device(5, "Speakers", false), device(6, "Headphones", true)];
        *rig.graph.borrow_mut() = graph;
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        assert!(has_node(&rig, device_key(6)) && marked(&rig, 6));
        assert!(!has_node(&rig, device_key(5)));
        assert_eq!(rig.core.size(), folded);
        // (b) A device that appeared after the open becomes the default: it
        // is the row, checked, and the old default is not offered.
        let mut graph = full_graph();
        graph.outputs = vec![
            device(5, "Speakers", false),
            device(6, "Headphones", false),
            device(7, "USB headset", true),
        ];
        *rig.graph.borrow_mut() = graph;
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        assert!(has_node(&rig, device_key(7)) && marked(&rig, 7));
        assert!(!has_node(&rig, device_key(5)) && !has_node(&rig, device_key(6)));
        assert!(rig
            .core
            .mirror
            .audio()
            .labels
            .contains(&(7, "USB headset".to_owned())));
        assert_eq!(rig.core.size(), folded);
        relayout(&mut rig);
        click(&mut rig, device_key(7));
        assert!(rig.sent.borrow().is_empty(), "the default asks for nothing");
        // The next unfold lists it with the others.
        toggle(&mut rig, Section::Output);
        assert!(has_node(&rig, device_key(5)) && has_node(&rig, device_key(7)));
    }

    #[test]
    fn a_folded_list_with_no_default_anywhere_still_shows_one_row() {
        let mut graph = full_graph();
        for d in graph.outputs.iter_mut() {
            d.default = false;
        }
        let rig = rig_with(Kind::Volume, audio(Some((30, false)), None), graph);
        assert_eq!(rig.core.shape().outputs, 1);
        assert!(has_node(&rig, device_key(5)) && !has_node(&rig, device_key(6)));
    }

    #[test]
    fn a_default_that_appeared_is_cut_with_the_frames_measure_not_the_stored_one() {
        // No cutter is stored: the one given at open is used during that
        // call only (here it would leave every name whole), and a later sync
        // cuts with the measure it is handed.
        let mut rig = rig_with_fit(
            Kind::Volume,
            audio(Some((30, false)), None),
            full_graph(),
            |name, _| name.to_owned(),
        );
        let used = std::cell::Cell::new(0_u32);
        let mut counting = |name: &str, width: f32| {
            used.set(used.get() + 1);
            cut(name, width)
        };
        let long = "USB headset with a name that runs far past the column of a row";
        let mut graph = full_graph();
        graph.outputs = vec![
            device(5, "Speakers", false),
            device(6, "Headphones", false),
            device(7, long, true),
        ];
        *rig.graph.borrow_mut() = graph;
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), &mut counting));
        assert!(used.get() > 0, "the measure passed in is the one used");
        let label = rig
            .core
            .mirror
            .audio()
            .labels
            .iter()
            .find(|(id, _)| *id == 7)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        assert!(label.contains('\u{2026}'), "cut: {label}");
        let column = audio_rows::device_name_w(&Theme::dark());
        assert!(7.0 * label.chars().count() as f32 <= column, "{label}");
    }

    #[test]
    fn a_new_theme_cuts_every_name_again_to_its_new_columns() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), long_graph());
        toggle(&mut rig, Section::Output);
        let label = |rig: &Rig| {
            rig.core
                .mirror
                .audio()
                .labels
                .iter()
                .find(|(id, _)| *id == 6)
                .map(|(_, l)| l.chars().count())
                .unwrap_or_default()
        };
        let before = label(&rig);
        let mut roomy = Theme::dark();
        roomy.spacing.md += 20.0;
        assert!(rig.core.sync(&roomy, Instant::now(), cut));
        let after = label(&rig);
        assert!(
            after < before,
            "{after} chars in a narrower column, was {before}"
        );
        assert!(7.0 * after as f32 <= audio_rows::device_name_w(&roomy));
    }

    #[test]
    fn a_section_listed_at_open_keeps_its_header_and_the_width_while_open() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        // The worker reports nothing at all (an unreadable status).
        *rig.graph.borrow_mut() = AudioGraph::default();
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        for section in [Section::Output, Section::Input, Section::Apps] {
            toggle(&mut rig, section);
            assert!(has_node(&rig, section_key(section)), "{section:?} open");
            assert_eq!(rig.core.size().0, AUDIO_W);
        }
        // And each folds back.
        for section in [Section::Output, Section::Input, Section::Apps] {
            toggle(&mut rig, section);
            assert!(has_node(&rig, section_key(section)), "{section:?} folded");
            assert!(!rig.core.mirror.audio().folds.is_open(section));
        }
        assert_eq!(rig.core.size().0, AUDIO_W);
    }

    #[test]
    fn a_header_works_from_the_keyboard() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        let now = Instant::now();
        let folded = rig.core.size();
        assert!(!has_node(&rig, device_key(6)));
        // Tab stops: the output's slider and mute row, then its header.
        for _ in 0..3 {
            rig.core.input(&key(TAB), now, &mut cut);
        }
        rig.core.input(&key(ENTER), now, &mut cut);
        relayout(&mut rig);
        assert!(rig.core.mirror.audio().folds.is_open(Section::Output));
        assert!(has_node(&rig, device_key(6)));
        assert!(rig.core.size().1 > folded.1, "the unfold grew the body");
        // The header keeps the focus: Space folds it again.
        rig.core.input(&key(0x20), now, &mut cut);
        relayout(&mut rig);
        assert!(!rig.core.mirror.audio().folds.is_open(Section::Output));
        assert!(!has_node(&rig, device_key(6)));
        // The header is a focusable row the keyboard reaches, and nothing
        // was sent to the worker by it.
        assert!(rig.sent.borrow().is_empty());
    }

    #[test]
    fn what_starts_playing_waits_for_the_next_fold_and_the_height_never_moves_alone() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        let folded = rig.core.size();
        // Mpv starts to play and a new stream appears while Aplicaciones is
        // folded: nothing is added, the size is the same.
        {
            let mut live = rig.graph.borrow_mut();
            live.streams[1].playing = true;
            live.streams
                .push(playing(stream(22, "Newcomer", Some((10, false)))));
        }
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        relayout(&mut rig);
        assert!(!has_node(&rig, slider_key(Control::Stream(21))));
        assert!(!has_node(&rig, slider_key(Control::Stream(22))));
        assert_eq!(rig.core.size(), folded);
        // Unfolding and folding Aplicaciones lists them again.
        toggle(&mut rig, Section::Apps);
        assert!(has_node(&rig, slider_key(Control::Stream(22))));
        toggle(&mut rig, Section::Apps);
        assert!(has_node(&rig, slider_key(Control::Stream(21))));
        assert!(has_node(&rig, slider_key(Control::Stream(22))));
        assert!(rig.core.size().1 > folded.1, "two more rows");
        // Its names are cut for their column like the others'.
        assert!(rig
            .core
            .mirror
            .audio()
            .labels
            .iter()
            .any(|(id, l)| *id == 22 && l == "Newcomer"));
    }

    #[test]
    fn the_default_device_row_is_selected() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        unfold_all(&mut rig);
        assert!(marked(&rig, 5) && !marked(&rig, 6));
        assert!(marked(&rig, 9) && !marked(&rig, 10));
        // The default changes elsewhere while the popup is open: the mark
        // moves, the sliders follow the new device, the popup asks nothing.
        let mut graph = full_graph();
        graph.outputs = vec![device(5, "Speakers", false), device(6, "Headphones", true)];
        *rig.graph.borrow_mut() = graph;
        rig.cell.set(audio(Some((45, true)), Some((80, false))));
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        assert!(!marked(&rig, 5) && marked(&rig, 6));
        assert_eq!(rig.core.shown.audio.output.map(|l| l.percent), Some(45));
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Output))
                .map(Slider::value),
            Some(45)
        );
        assert!(rig.sent.borrow().is_empty());
        // The size and the rows are the ones it had.
        let size = body_size(&Theme::dark(), Kind::Volume, true, rig.core.shape());
        assert_eq!(rig.core.size(), size);
        relayout(&mut rig);
        let card = rect_of(&rig.core, card_key());
        assert_eq!((card.w, card.h), size);
    }

    #[test]
    fn a_click_on_a_device_row_requests_set_default() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        unfold_all(&mut rig);
        click(&mut rig, device_key(6));
        assert_eq!(*rig.sent.borrow(), [Request::SetDefault(6)]);
        // The mark follows at once, before the worker's graph does.
        assert!(marked(&rig, 6) && !marked(&rig, 5));
        // The sources are their own list.
        click(&mut rig, device_key(10));
        assert_eq!(rig.sent.borrow().last(), Some(&Request::SetDefault(10)));
        assert!(marked(&rig, 10) && !marked(&rig, 9));
        assert!(marked(&rig, 6), "the other list keeps its own");
        // The default's own row asks for nothing.
        let before = rig.sent.borrow().len();
        click(&mut rig, device_key(10));
        assert_eq!(rig.sent.borrow().len(), before);
        // The keyboard: the output's slider, its mute row, its header, then
        // the two device rows are the first five stops; Enter on the fifth
        // asks for it.
        let mut keyed = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        toggle(&mut keyed, Section::Output);
        let now = Instant::now();
        for _ in 0..5 {
            keyed.core.input(&key(TAB), now, &mut cut);
        }
        keyed.core.input(&key(ENTER), now, &mut cut);
        assert_eq!(*keyed.sent.borrow(), [Request::SetDefault(6)]);
    }

    #[test]
    fn a_refused_default_change_gives_way_to_the_workers_graph() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            full_graph(),
        );
        let t0 = Instant::now();
        unfold_all(&mut rig);
        click(&mut rig, device_key(6));
        assert_eq!(*rig.sent.borrow(), [Request::SetDefault(6)]);
        assert!(marked(&rig, 6));
        // The worker refused and published nothing: the mark stands for its
        // hold, then the worker's graph (unchanged) takes it back.
        assert!(!rig.core.sync(&Theme::dark(), t0, cut), "inside the hold");
        assert!(marked(&rig, 6));
        let wait = rig.core.graph_wait(t0);
        assert!(wait.is_some_and(|w| w > Duration::ZERO), "a frame is due");
        let later = t0 + audio_rows::MARK_HOLD + Duration::from_millis(50);
        assert!(rig.core.sync(&Theme::dark(), later, cut));
        assert!(marked(&rig, 5) && !marked(&rig, 6));
        assert!(rig.core.graph_wait(later).is_none());
        // A confirmed change is not undone by the hold: the new graph is
        // adopted when it arrives and nothing waits after it.
        click(&mut rig, device_key(6));
        let mut graph = full_graph();
        graph.outputs = vec![device(5, "Speakers", false), device(6, "Headphones", true)];
        *rig.graph.borrow_mut() = graph;
        rig.core.sync(&Theme::dark(), later, cut);
        assert!(marked(&rig, 6));
        assert!(rig.core.graph_wait(later).is_none());
        assert!(!rig
            .core
            .sync(&Theme::dark(), later + audio_rows::MARK_HOLD * 2, cut));
        assert!(marked(&rig, 6));
    }

    #[test]
    fn a_stream_slider_requests_its_own_level() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        toggle(&mut rig, Section::Apps);
        let track = rect_of(&rig.core, slider_key(Control::Stream(21)));
        let now = Instant::now();
        rig.core.input(
            &press(track.x + track.w - 1.0, track.y + track.h / 2.0),
            now,
            &mut cut,
        );
        assert_eq!(*rig.sent.borrow(), [Request::Set(Control::Stream(21), 100)]);
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Stream(21)))
                .map(Slider::value),
            Some(100)
        );
        // The other stream and the default devices keep their levels.
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Stream(20)))
                .map(Slider::value),
            Some(60)
        );
        assert_eq!(rig.core.shown.audio.output.map(|l| l.percent), Some(30));
        rig.core.input(
            &release(track.x + track.w - 1.0, track.y + track.h / 2.0),
            now,
            &mut cut,
        );
        // A click on a stream's mute button flips its mute, and only its.
        click(&mut rig, mute_key(Control::Stream(20)));
        assert_eq!(
            rig.sent.borrow().last(),
            Some(&Request::ToggleMute(Control::Stream(20)))
        );
        assert_eq!(
            rig.core
                .mirror
                .audio()
                .live
                .streams
                .iter()
                .map(|s| s.level.map(|l| l.muted))
                .collect::<Vec<_>>(),
            [Some(true), Some(true)]
        );
    }

    #[test]
    fn a_vanished_stream_row_is_disabled_and_its_pending_value_dropped() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        toggle(&mut rig, Section::Apps);
        toggle(&mut rig, Section::Output);
        let track = rect_of(&rig.core, slider_key(Control::Stream(20)));
        let y = track.y + track.h / 2.0;
        let t0 = Instant::now();
        // A drag: the first value goes at once, the next ones wait.
        rig.core.input(&press(track.x + 10.0, y), t0, &mut cut);
        rig.core.input(&motion(track.x + 40.0, y), t0, &mut cut);
        rig.core.input(&motion(track.x + 60.0, y), t0, &mut cut);
        assert_eq!(rig.sent.borrow().len(), 1);
        assert!(rig.core.throttle.wait(t0).is_some(), "a value waits");
        // The stream ends while the slider is held.
        rig.graph.borrow_mut().streams.remove(0);
        // Nothing panics; the release would send the last value, but its id
        // is gone: dropped, and nothing is left waiting.
        rig.core.input(&release(track.x + 60.0, y), t0, &mut cut);
        assert_eq!(rig.sent.borrow().len(), 1, "{:?}", rig.sent.borrow());
        assert!(rig.core.throttle.wait(t0).is_none());
        // The row greys out after the release, and takes no input.
        assert!(rig.core.sync(&Theme::dark(), t0, cut));
        let slider = rig.core.tree.slider(slider_key(Control::Stream(20)));
        assert_eq!(slider.map(|s| s.enabled()), Some(false));
        relayout(&mut rig);
        click(&mut rig, mute_key(Control::Stream(20)));
        let (x, y) = centre_of(&rig.core, slider_key(Control::Stream(20)));
        rig.core.input(&press(x, y), t0, &mut cut);
        rig.core.input(&release(x, y), t0, &mut cut);
        assert_eq!(rig.sent.borrow().len(), 1, "a gone row sends nothing");
        // The other stream is untouched, and the row is still there.
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Stream(21)))
                .map(|s| s.enabled()),
            Some(true)
        );
        // A vanished device row is greyed out the same way.
        rig.graph.borrow_mut().outputs.remove(1);
        rig.core.sync(&Theme::dark(), t0, cut);
        relayout(&mut rig);
        let before = rig.sent.borrow().len();
        click(&mut rig, device_key(6));
        assert_eq!(rig.sent.borrow().len(), before);
    }

    #[test]
    fn the_tallest_volume_body_scrolls_inside_a_544_sheet() {
        let theme = Theme::dark();
        let graph = |n: u32| AudioGraph {
            outputs: (0..n).map(|id| device(id, "d", id == 0)).collect(),
            inputs: (0..n).map(|id| device(100 + id, "d", id == 0)).collect(),
            streams: (0..n)
                .map(|id| playing(stream(200 + id, "s", None)))
                .collect(),
        };
        let all_open = Folds::default();
        let open = |sections: &[Section]| {
            let mut folds = all_open;
            let mut mirror = GraphMirror::open(&graph(12), &theme, |name, _| name.to_owned());
            for section in sections {
                mirror.toggle(*section, &theme, |name, _| name.to_owned());
                folds = mirror.audio().folds;
            }
            (folds, mirror.shape())
        };
        // The window's viewport: the surface's room under the bar (496).
        let viewport = crate::sheet::geometry::max_height((1920.0, 560.0));
        // Folded: one device each and the streams that play.
        let (_, folded) = open(&[]);
        assert_eq!(
            folded,
            Shape {
                outputs: 1,
                inputs: 1,
                streams: 8,
                listed: true
            }
        );
        let (w, folded_h) = body_size(&theme, Kind::Volume, true, folded);
        assert_eq!(w, SHEET_W);
        // Everything unfolded: eight devices, eight sources and eight
        // streams (twelve of each were listed), headers included: taller
        // than the sheet's viewport, so the body scrolls.
        let (_, full) = open(&[Section::Output, Section::Input, Section::Apps]);
        assert_eq!(
            full,
            Shape {
                outputs: 8,
                inputs: 8,
                streams: 8,
                listed: true
            }
        );
        let (_, full_h) = body_size(&theme, Kind::Volume, true, full);
        assert!(
            full_h > viewport,
            "{full_h} px fits the {viewport} px viewport: nothing to scroll"
        );
        let mut scroll = celestina_ui::Scroll::default();
        scroll.set_extent(full_h, viewport);
        assert!(scroll.can_scroll());
        assert!(scroll.wheel(1000.0));
        assert_eq!(scroll.offset(), full_h - viewport);
        // The plain body (no lists, no microphone) fits and never scrolls.
        let (_, plain) = body_size(&theme, Kind::Volume, false, Shape::default());
        scroll.set_extent(plain, viewport);
        assert!(!scroll.can_scroll());
        assert!(folded_h < full_h);
        // Each fold adds only its rows: seven devices, seven sources.
        let (_, out_only) = open(&[Section::Output]);
        let (_, out_h) = body_size(&theme, Kind::Volume, true, out_only);
        assert_eq!(out_h - folded_h, 7.0 * compact_h(&theme));
        // The compact rows keep the touch target of the slider's own height,
        // and so does a header.
        assert!(compact_h(&theme) >= slider_h(&theme));
        assert!(header_h(&theme) >= slider_h(&theme));
        // Brightness ignores the shape.
        assert_eq!(
            body_size(&theme, Kind::Brightness, false, full),
            body_size(&theme, Kind::Brightness, false, Shape::default())
        );

        // Twelve sinks: unfolded, eight rows show and the body is the size of
        // its rows, whatever the viewport.
        let mut graph = AudioGraph::default();
        for id in 0..12 {
            graph
                .outputs
                .push(device(100 + id, &format!("Sink {id}"), id == 0));
        }
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), graph);
        toggle(&mut rig, Section::Output);
        let rows = (100..112)
            .filter(|id| rig.core.tree.node_by_key(device_key(*id)).is_some())
            .count();
        assert_eq!(rows, 8);
        assert_eq!(rig.core.shape().outputs, 8);
        assert_eq!(
            rig.core.size(),
            body_size(&theme, Kind::Volume, false, rig.core.shape())
        );
    }

    #[test]
    fn rows_update_in_place_while_the_height_stays_until_the_next_fold() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        toggle(&mut rig, Section::Apps);
        let opened_card = rect_of(&rig.core, card_key());
        let opened_slider = rect_of(&rig.core, slider_key(Control::Stream(21)));
        {
            let mut live = rig.graph.borrow_mut();
            live.streams.push(stream(22, "Newcomer", Some((10, false))));
            live.streams[1].level = Some(AudioLevel {
                percent: 77,
                muted: false,
            });
        }
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        relayout(&mut rig);
        assert!(!has_node(&rig, slider_key(Control::Stream(22))));
        assert_eq!(rect_of(&rig.core, card_key()), opened_card);
        assert_eq!(
            rect_of(&rig.core, slider_key(Control::Stream(21))),
            opened_slider
        );
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::Stream(21)))
                .map(Slider::value),
            Some(77)
        );
    }

    /// The x of the left edge of a node.
    fn x_of(core: &LevelCore, key: Key) -> f32 {
        rect_of(core, key).x
    }

    /// A graph whose names run past their columns.
    fn long_graph() -> AudioGraph {
        AudioGraph {
            outputs: vec![
                device(5, "Speakers", true),
                device(
                    6,
                    "Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI)",
                    false,
                ),
                device(
                    7,
                    "Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI 2)",
                    false,
                ),
                device(8, "Bluetooth Headset", false),
            ],
            inputs: vec![device(9, "Desk mic", true)],
            streams: vec![
                playing(stream(
                    20,
                    "speech-dispatcher-dummy-with-a-long-name",
                    Some((60, false)),
                )),
                playing(stream(21, "Zen", Some((30, true)))),
            ],
        }
    }

    #[test]
    fn device_names_start_at_one_x_whatever_their_length_or_the_check() {
        let mut rig = rig_with(
            Kind::Volume,
            audio(Some((30, false)), Some((80, false))),
            long_graph(),
        );
        unfold_all(&mut rig);
        let xs: Vec<f32> = [5, 6, 7, 8, 9]
            .into_iter()
            .map(|id| x_of(&rig.core, audio_rows::device_name_key(id)))
            .collect();
        assert!(xs.iter().all(|x| *x == xs[0]), "names start at {xs:?}");
        // Right of the check's column, in the row.
        let row = rect_of(&rig.core, device_key(5));
        let theme = Theme::dark();
        assert_eq!(xs[0], row.x + theme.spacing.sm + ICON + theme.spacing.sm);
        // And every name is as wide as the column, long or short.
        let ws: Vec<f32> = [5, 6, 7, 8]
            .into_iter()
            .map(|id| rect_of(&rig.core, audio_rows::device_name_key(id)).w)
            .collect();
        assert!(ws.iter().all(|w| *w == ws[0]), "{ws:?}");
    }

    #[test]
    fn long_names_are_cut_in_the_middle_and_stay_distinct() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), long_graph());
        unfold_all(&mut rig);
        let labels = rig.core.mirror.audio().labels.to_vec();
        let label = |id: u32| {
            labels
                .iter()
                .find(|(of, _)| *of == id)
                .map(|(_, l)| l.clone())
                .expect("a label")
        };
        let (a, b) = (label(6), label(7));
        assert_ne!(a, b, "{a} / {b}");
        assert!(
            a.contains('\u{2026}') && b.ends_with("(HDMI 2)"),
            "{a} / {b}"
        );
        // Each cut fits its column as measured (7 px per character here).
        let theme = Theme::dark();
        let column = audio_rows::device_name_w(&theme);
        for l in [&a, &b] {
            assert!(7.0 * l.chars().count() as f32 <= column, "{l}");
        }
        assert_eq!(label(5), "Speakers", "a name that fits is untouched");
        // The application names are cut to their own column.
        let long = label(20);
        assert!(long.contains('\u{2026}'));
        assert!(7.0 * long.chars().count() as f32 <= audio_rows::stream_name_w(&theme));
        assert_eq!(label(21), "Zen");
    }

    #[test]
    fn every_application_row_has_the_same_columns() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), long_graph());
        toggle(&mut rig, Section::Apps);
        let slider = |id| rect_of(&rig.core, slider_key(Control::Stream(id)));
        let mute = |id| rect_of(&rig.core, mute_key(Control::Stream(id)));
        let (s20, s21) = (slider(20), slider(21));
        assert_eq!((s20.x, s20.w), (s21.x, s21.w));
        assert_eq!(s20.w, audio_rows::STREAM_SLIDER_W);
        assert_eq!((mute(20).x, mute(20).w), (mute(21).x, mute(21).w));
        // The switch sits right of the slider, at the edge of the mute rows'.
        assert!(mute(20).x >= s20.x + s20.w);
        let out_switch = rect_of(&rig.core, mute_key(Control::Output));
        let card = rect_of(&rig.core, card_key());
        let md = Theme::dark().spacing.md;
        assert_eq!(out_switch.x + out_switch.w, card.w - md);
        assert!((mute(20).x + mute(20).w - (card.w - md - Theme::dark().spacing.sm)).abs() < 0.5);
    }

    #[test]
    fn no_frame_is_asked_for_the_mark_while_a_drag_holds_the_sliders() {
        let mut rig = rig_with(Kind::Volume, audio(Some((30, false)), None), full_graph());
        toggle(&mut rig, Section::Output);
        let t0 = Instant::now();
        click(&mut rig, device_key(6));
        let past = t0 + audio_rows::MARK_HOLD * 2;
        assert!(rig.core.graph_wait(past).is_some(), "due once it is over");
        let track = rect_of(&rig.core, slider_key(Control::Stream(20)));
        let y = track.y + track.h / 2.0;
        rig.core.input(&press(track.x + 10.0, y), past, &mut cut);
        assert!(rig.core.graph_wait(past).is_none(), "held: nothing to ask");
        rig.core.input(&release(track.x + 10.0, y), past, &mut cut);
        assert!(rig.core.graph_wait(past).is_some());
        assert!(rig.core.sync(&Theme::dark(), past, cut));
        assert!(rig.core.graph_wait(past).is_none());
    }

    #[test]
    fn rows_keep_their_columns_even_when_a_name_is_not_cut() {
        // Names left whole (as if the measure were wrong) must not push the
        // check's column or the slider out of place.
        let theme = Theme::dark();
        let graph = long_graph();
        let mut mirror = GraphMirror::open(&graph, &theme, |name, _| name.to_owned());
        for section in [Section::Output, Section::Input, Section::Apps] {
            mirror.toggle(section, &theme, |name, _| name.to_owned());
        }
        let levels = audio(Some((30, false)), Some((80, false)));
        let shape = mirror.shape();
        let mut tree = Tree::new(false, &theme);
        tree.apply(
            card(&theme, Kind::Volume, true, levels, mirror.audio(), shape),
            Instant::now(),
        );
        tree.layout_measured(body_size(&theme, Kind::Volume, true, shape), fake_measure);
        let x = |key| match tree.node_by_key(key) {
            Some(id) => tree.rect(id).x,
            None => panic!("no node for {key:?}"),
        };
        let names: Vec<f32> = [5, 6, 7, 8]
            .into_iter()
            .map(|id| x(audio_rows::device_name_key(id)))
            .collect();
        assert!(names.iter().all(|n| *n == names[0]), "{names:?}");
        assert_eq!(
            x(slider_key(Control::Stream(20))),
            x(slider_key(Control::Stream(21)))
        );
    }

    /// Whether the laid-out tree holds a node keyed `key`.
    fn in_tree(rig: &Rig, key: Key) -> bool {
        rig.core.tree.node_by_key(key).is_some()
    }

    fn feed(rows: &[(&str, bool, Option<u8>)]) -> MonitorFeed {
        MonitorFeed {
            rows: rows
                .iter()
                .map(|(name, primary, _)| MonitorRow {
                    name: (*name).to_owned(),
                    primary: *primary,
                })
                .collect(),
            levels: Rc::new(RefCell::new(rows.iter().map(|(_, _, l)| *l).collect())),
        }
    }

    fn rig_monitors(rows: &[(&str, bool, Option<u8>)]) -> Rig {
        rig_full(
            Kind::Brightness,
            brightness(rows.first().and_then(|r| r.2)),
            AudioGraph::default(),
            feed(rows),
            cut,
        )
    }

    fn three() -> [(&'static str, bool, Option<u8>); 3] {
        [
            ("LG ULTRAGEAR+", true, Some(30)),
            ("DELL U2720Q", false, Some(60)),
            ("DP-3", false, None),
        ]
    }

    fn monitor_slider(at: u8) -> Key {
        slider_key(Control::Monitor(at))
    }

    #[test]
    fn one_monitor_keeps_the_single_slider_body() {
        let rig = rig_monitors(&[("LG ULTRAGEAR+", true, Some(30))]);
        let theme = Theme::dark();
        assert_eq!(
            rig.core.size(),
            body_size(&theme, Kind::Brightness, false, Shape::default())
        );
        assert_eq!(rig.core.size().0, SHEET_W);
        assert!(!in_tree(&rig, slider_key(Control::AllMonitors)));
        assert!(!in_tree(&rig, monitor_slider(0)));
        assert!(in_tree(&rig, slider_key(Control::Brightness)));
    }

    #[test]
    fn several_monitors_open_the_list_at_the_sheet_width() {
        let rig = rig_monitors(&three());
        let theme = Theme::dark();
        let size = monitors_size(&theme, 3);
        assert_eq!(rig.core.size(), size);
        assert_eq!(size.0, SHEET_W);
        let card = rect_of(&rig.core, card_key());
        assert_eq!((card.x, card.y, card.w, card.h), (0.0, 0.0, size.0, size.1));
        // All, the divider, then the rows in order, each inside the card.
        let ys = [
            y_of(&rig.core, crate::sheet::subtitle::row_key()),
            y_of(&rig.core, slider_key(Control::AllMonitors)),
            y_of(&rig.core, Key::named("level-monitors-divider")),
            y_of(&rig.core, Key::indexed("level-monitor-row", 0)),
            y_of(&rig.core, Key::indexed("level-monitor-row", 1)),
            y_of(&rig.core, Key::indexed("level-monitor-row", 2)),
        ];
        assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
        let divider = rect_of(&rig.core, Key::named("level-monitors-divider"));
        assert_eq!(divider.h, crate::osd::monitor_rows::DIVIDER_H);
        for at in 0..3u8 {
            let row = rect_of(&rig.core, Key::indexed("level-monitor-row", u64::from(at)));
            assert!(
                row.y + row.h <= size.1 && row.x + row.w <= size.0,
                "{row:?}"
            );
            let name = rect_of(&rig.core, Key::indexed("level-monitor-name", u64::from(at)));
            let slider = rect_of(&rig.core, monitor_slider(at));
            assert_eq!(name.w, crate::osd::monitor_rows::MONITOR_NAME_W);
            assert!(slider.x >= name.x + name.w, "the slider follows the name");
            assert!(slider.w > 60.0, "the slider grows: {slider:?}");
        }
        // The columns line up whatever the name.
        let x0 = x_of(&rig.core, monitor_slider(0));
        assert_eq!(x0, x_of(&rig.core, monitor_slider(1)));
        // The mean of the known levels heads the card.
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::AllMonitors))
                .map(Slider::value),
            Some(45)
        );
        // A monitor with no reading shows no percent.
        assert!(in_tree(&rig, Key::indexed("level-monitor-percent", 1)));
        assert!(!in_tree(&rig, Key::indexed("level-monitor-percent", 2)));
    }

    /// A wheel turn of `vertical` px at the centre of `key`'s node.
    fn wheel_over(rig: &mut Rig, key: Key, vertical: f32, now: Instant) {
        let r = rect_of(&rig.core, key);
        rig.core.input(
            &InputEvent::Axis {
                x: r.x + r.w / 2.0,
                y: r.y + r.h / 2.0,
                horizontal: 0.0,
                vertical,
            },
            now,
            &mut cut,
        );
    }

    #[test]
    fn the_wheel_moves_a_monitor_row_one_percent_per_notch() {
        let mut rig = rig_monitors(&three());
        let now = Instant::now();
        // One notch down lowers the row by one; two notches up raise it by two.
        wheel_over(&mut rig, monitor_slider(0), 15.0, now);
        assert_eq!(rig.core.monitors.level(0), Some(29));
        wheel_over(
            &mut rig,
            monitor_slider(0),
            -30.0,
            now + Duration::from_secs(1),
        );
        assert_eq!(rig.core.monitors.level(0), Some(31));
        assert_eq!(rig.core.monitors.level(1), Some(60), "the other row stays");
        rig.core.flush(now + Duration::from_secs(2), true);
        assert_eq!(
            rig.sent.borrow().last(),
            Some(&Request::Set(Control::Monitor(0), 31))
        );
        // Half a notch moves nothing until the rest of it comes.
        wheel_over(&mut rig, monitor_slider(1), 7.0, now);
        assert_eq!(rig.core.monitors.level(1), Some(60));
        wheel_over(&mut rig, monitor_slider(1), 8.0, now);
        assert_eq!(rig.core.monitors.level(1), Some(59));
    }

    #[test]
    fn the_wheel_on_all_moves_every_monitor_with_a_reading() {
        let mut rig = rig_monitors(&three());
        let now = Instant::now();
        // The mean of 30 and 60 is 45: one notch down sets both to 44.
        wheel_over(&mut rig, slider_key(Control::AllMonitors), 15.0, now);
        assert_eq!(rig.core.monitors.level(0), Some(44));
        assert_eq!(rig.core.monitors.level(1), Some(44));
        assert_eq!(rig.core.monitors.level(2), None, "no reading, no level");
    }

    #[test]
    fn the_wheel_moves_the_output_one_percent_and_stops_at_the_ends() {
        let mut rig = rig(Kind::Volume, audio(Some((99, false)), None));
        let now = Instant::now();
        wheel_over(&mut rig, slider_key(Control::Output), -15.0, now);
        assert_eq!(rig.core.shown_level(Control::Output), Some(100));
        wheel_over(&mut rig, slider_key(Control::Output), -15.0, now);
        assert_eq!(rig.core.shown_level(Control::Output), Some(100), "capped");
        rig.core.flush(now + Duration::from_secs(1), true);
        assert_eq!(
            *rig.sent.borrow(),
            [Request::Set(Control::Output, 100)],
            "the capped notch sends nothing more"
        );
    }

    #[test]
    fn a_late_read_back_does_not_lose_a_wheel_notch() {
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let now = Instant::now();
        wheel_over(&mut rig, slider_key(Control::Brightness), -15.0, now);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(51));
        // The monitor still reports the level before the notch.
        rig.cell.set(brightness(Some(50)));
        rig.core.sync(&Theme::dark(), now, cut);
        // The next notch of the burst steps from what the wheel set.
        wheel_over(
            &mut rig,
            slider_key(Control::Brightness),
            -15.0,
            now + Duration::from_millis(300),
        );
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(52));
    }

    #[test]
    fn the_wheel_does_nothing_while_a_slider_is_dragged() {
        let mut rig = rig_monitors(&three());
        let track = rect_of(&rig.core, monitor_slider(1));
        let now = Instant::now();
        rig.core.input(
            &press(track.x + 1.0, track.y + track.h / 2.0),
            now,
            &mut cut,
        );
        let dragged = rig.core.monitors.level(1);
        wheel_over(&mut rig, monitor_slider(0), 15.0, now);
        assert_eq!(rig.core.monitors.level(0), Some(30));
        assert_eq!(rig.core.monitors.level(1), dragged);
    }

    #[test]
    fn the_single_brightness_slider_keeps_its_level_until_the_monitor_reads_it_back() {
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let t0 = Instant::now();
        let track = rect_of(&rig.core, slider_key(Control::Brightness));
        let (x, y) = (track.x + track.w - 1.0, track.y + track.h / 2.0);
        rig.core.input(&press(x, y), t0, &mut cut);
        rig.core.input(&release(x, y), t0, &mut cut);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(100));
        // A read-back of an earlier write of the drag: the slider stays.
        rig.cell.set(brightness(Some(70)));
        rig.core
            .sync(&Theme::dark(), t0 + Duration::from_secs(1), cut);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(100));
        assert!(rig.core.graph_wait(t0 + Duration::from_secs(1)).is_some());
        // The monitor reads the asked level back: the hold ends.
        rig.cell.set(brightness(Some(100)));
        rig.core
            .sync(&Theme::dark(), t0 + Duration::from_secs(2), cut);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(100));
        assert!(rig.core.graph_wait(t0 + Duration::from_secs(2)).is_none());
        // A reading from elsewhere then shows at once.
        rig.cell.set(brightness(Some(40)));
        rig.core
            .sync(&Theme::dark(), t0 + Duration::from_secs(3), cut);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(40));
    }

    #[test]
    fn the_single_brightness_slider_shows_the_reading_after_the_hold() {
        let mut rig = rig(Kind::Brightness, brightness(Some(50)));
        let t0 = Instant::now();
        wheel_over(&mut rig, slider_key(Control::Brightness), -15.0, t0);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(51));
        // The monitor never confirms 51.
        rig.core.sync(&Theme::dark(), t0 + ASK_HOLD, cut);
        assert_eq!(rig.core.shown_level(Control::Brightness), Some(50));
    }

    #[test]
    fn the_wheel_on_all_brings_close_monitors_together() {
        let mut rig = rig_monitors(&[
            ("LG ULTRAGEAR+", true, Some(100)),
            ("DELL U2720Q", false, Some(99)),
        ]);
        // The rounded mean is 100: a notch up still sets the 99 one to 100.
        wheel_over(
            &mut rig,
            slider_key(Control::AllMonitors),
            -15.0,
            Instant::now(),
        );
        assert_eq!(rig.core.monitors.level(0), Some(100));
        assert_eq!(rig.core.monitors.level(1), Some(100));
    }

    #[test]
    fn the_wheel_does_nothing_on_a_slider_with_no_reading() {
        let mut rig = rig_monitors(&three());
        wheel_over(&mut rig, monitor_slider(2), 15.0, Instant::now());
        assert_eq!(rig.core.monitors.level(2), None);
        assert!(rig.sent.borrow().is_empty());
    }

    #[test]
    fn a_todos_drag_sets_every_known_monitor_and_sends_one_request_each() {
        let mut rig = rig_monitors(&three());
        let track = rect_of(&rig.core, slider_key(Control::AllMonitors));
        let now = Instant::now();
        rig.core.input(
            &press(track.x + track.w - 1.0, track.y + track.h / 2.0),
            now,
            &mut cut,
        );
        assert_eq!(
            *rig.sent.borrow(),
            [
                Request::Set(Control::Monitor(0), 100),
                Request::Set(Control::Monitor(1), 100)
            ]
        );
        // The rows follow at once; the one with no reading stays as it is.
        assert_eq!(rig.core.monitors.known(), [0, 1]);
        assert_eq!(
            rig.core.tree.slider(monitor_slider(1)).map(Slider::value),
            Some(100)
        );
        assert_eq!(
            rig.core.tree.slider(monitor_slider(2)).map(Slider::value),
            Some(0)
        );
        assert_eq!(rig.core.monitors.mean(), Some(100));
    }

    #[test]
    fn a_monitor_without_reading_is_greyed_and_takes_no_input() {
        let mut rig = rig_monitors(&three());
        let slider = rect_of(&rig.core, monitor_slider(2));
        let (x, y) = (slider.x + slider.w - 1.0, slider.y + slider.h / 2.0);
        let now = Instant::now();
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
        assert!(rig.sent.borrow().is_empty());
        assert_eq!(rig.core.monitors.known(), [0, 1]);
        assert!(!rig
            .core
            .tree
            .slider(monitor_slider(2))
            .is_some_and(|s| s.enabled()));
        assert!(rig
            .core
            .tree
            .slider(monitor_slider(0))
            .is_some_and(|s| s.enabled()));
    }

    #[test]
    fn todos_then_a_row_sends_each_monitors_last_value() {
        let mut rig = rig_monitors(&three());
        let all = rect_of(&rig.core, slider_key(Control::AllMonitors));
        let t0 = Instant::now();
        rig.core
            .input(&press(all.x + 20.0, all.y + all.h / 2.0), t0, &mut cut);
        rig.core
            .input(&release(all.x + 20.0, all.y + all.h / 2.0), t0, &mut cut);
        rig.sent.borrow_mut().clear();
        // Then the second row alone.
        let row = rect_of(&rig.core, monitor_slider(1));
        let (x, y) = (row.x + row.w - 1.0, row.y + row.h / 2.0);
        rig.core.input(&press(x, y), t0, &mut cut);
        rig.core.input(&release(x, y), t0, &mut cut);
        assert_eq!(*rig.sent.borrow(), [Request::Set(Control::Monitor(1), 100)]);
        // Mixed within one interval: each monitor's newest value is sent
        // once, at the release.
        rig.sent.borrow_mut().clear();
        let t1 = t0 + SEND_INTERVAL * 2;
        let all = rect_of(&rig.core, slider_key(Control::AllMonitors));
        let ay = all.y + all.h / 2.0;
        rig.core.input(&press(all.x + 30.0, ay), t1, &mut cut);
        rig.sent.borrow_mut().clear();
        rig.core.input(&motion(all.x + 90.0, ay), t1, &mut cut);
        rig.core.input(&motion(all.x + 150.0, ay), t1, &mut cut);
        rig.core.input(&release(all.x + 150.0, ay), t1, &mut cut);
        let sent = rig.sent.borrow().clone();
        assert_eq!(sent.len(), 2, "{sent:?}");
        let (Request::Set(Control::Monitor(0), a), Request::Set(Control::Monitor(1), b)) =
            (sent[0], sent[1])
        else {
            panic!("{sent:?}");
        };
        assert_eq!(a, b);
        assert_eq!(Some(a), rig.core.monitors.mean());
    }

    #[test]
    fn readings_update_rows_in_place_and_the_size_stays() {
        let mut rig = rig_monitors(&three());
        let size = rig.core.size();
        let card = rect_of(&rig.core, card_key());
        let row = rect_of(&rig.core, monitor_slider(1));
        *rig.monitor_cell.borrow_mut() = vec![Some(10), Some(80), Some(55)];
        assert!(rig.core.sync(&Theme::dark(), Instant::now(), cut));
        relayout(&mut rig);
        assert_eq!(rig.core.size(), size);
        assert_eq!(rect_of(&rig.core, card_key()), card);
        assert_eq!(rect_of(&rig.core, monitor_slider(1)), row);
        assert_eq!(
            rig.core.tree.slider(monitor_slider(1)).map(Slider::value),
            Some(80)
        );
        // The monitor that had no reading has one, and takes input now.
        assert!(rig
            .core
            .tree
            .slider(monitor_slider(2))
            .is_some_and(|s| s.enabled()));
        assert_eq!(
            rig.core
                .tree
                .slider(slider_key(Control::AllMonitors))
                .map(Slider::value),
            Some(48)
        );
        // The same reading again changes nothing.
        assert!(!rig.core.sync(&Theme::dark(), Instant::now(), cut));
        // A reading does not move a row under the pointer.
        let track = rect_of(&rig.core, monitor_slider(0));
        let y = track.y + track.h / 2.0;
        rig.core
            .input(&press(track.x + 20.0, y), Instant::now(), &mut cut);
        *rig.monitor_cell.borrow_mut() = vec![Some(99), Some(80), Some(55)];
        let shown = rig.core.monitors.known();
        assert!(!rig.core.sync(&Theme::dark(), Instant::now(), cut));
        assert_eq!(shown, [0, 1, 2]);
        assert_ne!(
            rig.core.tree.slider(monitor_slider(0)).map(Slider::value),
            Some(99)
        );
    }

    #[test]
    fn the_names_are_cut_at_their_rows_weight_once_per_theme() {
        let mut rig = rig_monitors(&[
            ("A very long monitor model name", true, Some(30)),
            ("Another very long model name", false, Some(60)),
        ]);
        let theme = Theme::dark();
        let mut seen = Vec::new();
        rig.core
            .cut_monitors(&theme, Instant::now(), |name, width, weight| {
                seen.push((name.to_owned(), width, weight));
                name.chars().take(5).collect()
            });
        assert_eq!(
            seen.iter()
                .map(|(_, w, weight)| (*w, *weight))
                .collect::<Vec<_>>(),
            [
                (crate::osd::monitor_rows::MONITOR_NAME_W, Weight::Semibold),
                (crate::osd::monitor_rows::MONITOR_NAME_W, Weight::Regular)
            ]
        );
        // Again with the same theme: nothing is measured.
        rig.core
            .cut_monitors(&theme, Instant::now(), |_, _, _| panic!("cut twice"));
        // A popup of one monitor cuts nothing.
        let mut single = rig_monitors(&[("Only", true, Some(1))]);
        single
            .core
            .cut_monitors(&theme, Instant::now(), |_, _, _| panic!("no list"));
    }

    #[test]
    fn a_late_reading_of_one_row_does_not_move_another() {
        let mut rig = rig_monitors(&[
            ("LG ULTRAGEAR+", true, Some(30)),
            ("DELL U2720Q", false, Some(60)),
        ]);
        // A drag of All sets both rows; the worker has read neither yet.
        let all = rect_of(&rig.core, slider_key(Control::AllMonitors));
        let (x, y) = (all.x + all.w - 1.0, all.y + all.h / 2.0);
        let now = Instant::now();
        rig.core.input(&press(x, y), now, &mut cut);
        rig.core.input(&release(x, y), now, &mut cut);
        let value = |rig: &Rig, at| rig.core.tree.slider(monitor_slider(at)).map(Slider::value);
        assert_eq!((value(&rig, 0), value(&rig, 1)), (Some(100), Some(100)));
        // A read-back of an earlier write of the drag arrives for the second
        // monitor: neither row moves, both keep what the popup set.
        *rig.monitor_cell.borrow_mut() = vec![Some(30), Some(70)];
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!((value(&rig, 0), value(&rig, 1)), (Some(100), Some(100)));
        // The second monitor reads the asked level back; the first still has
        // not: each row follows its own monitor only.
        *rig.monitor_cell.borrow_mut() = vec![Some(30), Some(100)];
        rig.core.sync(&Theme::dark(), now, cut);
        assert_eq!((value(&rig, 0), value(&rig, 1)), (Some(100), Some(100)));
        // After the hold, a row the monitor never confirmed shows its reading.
        rig.core.sync(
            &Theme::dark(),
            now + super::super::monitor_rows::ASK_HOLD,
            cut,
        );
        assert_eq!((value(&rig, 0), value(&rig, 1)), (Some(30), Some(100)));
    }
}
