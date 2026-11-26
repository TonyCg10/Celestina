//! The bar's side of the duty (CEL-2-C, plan decisions 5, 8 and 9): what the
//! Volume and Brightness pieces and their sheets ask of the workers, and the
//! readings they show.
//!
//! [`Controls`] is the one place that holds the readings ([`Readings`]) and
//! the level sheet that is open. Everything it does to the world is a
//! command to a worker through [`Workers`], so it is tested without a bus, a
//! child or a compositor; the app only forwards its messages and its
//! actions here.
//!
//! - A **piece action** (wheel step, mute) becomes worker commands for the
//!   default output, or for the monitor of the Island's output.
//! - **Opening a sheet** builds its body over a cell of readings the app keeps
//!   current, asks the worker for a fresh reading (one per monitor for the
//!   DDC worker, never concurrent) and remembers which control is open, so the
//!   display can be suppressed while it is open and released when it closes.
//! - **DDC is gated** on a consumer: while the duty is on and a layout holds
//!   the `brightness` piece the worker is told to show it (`OsdRuntime::apply`
//!   reads [`brightness_consumer`] at start and on every settings change). It
//!   then detects once and reads once each monitor whose level is unknown,
//!   so the piece has something to draw; a step or a slider is a change,
//!   which the worker detects for lazily, never a detection request.
//! - **The audio graph is gated** the same way: while the duty is on and a
//!   layout holds the `volume` piece, the audio worker is told to read it
//!   ([`volume_consumer`]); [`Controls`] hands the latest one to a volume
//!   sheet, which lists its devices and applications. The graph never
//!   reaches the display.
//! - **Closing a sheet** releases the display's kinds with the last level the
//!   sheet asked of each, so their read-backs are not announced.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use celestina_shell::audio::AudioGraph;
use celestina_shell::osd::Kind as OsdKind;
use celestina_shell::settings::{BarLayout, Duty, Settings};
use celestina_ui::Key;
use celestina_wayland::{SurfaceCtl, SurfaceId};

use super::monitor_rows::{MonitorFeed, MonitorRow, SharedMonitors};
use super::popup::{
    self, Command, Control, Kind, LevelCore, Request, SharedGraph, SharedLevels, Sink,
};
use super::readings::{Readings, Target};
use super::verbs::Workers;
use super::{OsdRuntime, Suppression};
use crate::island::{menu::SharedPainter, Island};
use crate::pieces::{AudioAction, BrightnessAction, Levels};
use crate::providers::audio::{AudioCommand, Device};
use crate::providers::brightness::{BrightnessCommand, Change};

/// The id of the brightness piece in a layout.
const BRIGHTNESS_PIECE: &str = "brightness";

/// The id of the volume piece in a layout.
const VOLUME_PIECE: &str = "volume";

/// Whether the duty is on and the bar's layout holds the piece `id`.
fn piece_placed(settings: &Settings, id: &str) -> bool {
    if settings.duties.osd != Duty::On {
        return false;
    }
    let BarLayout {
        start, centre, end, ..
    } = settings.bar_layout();
    [start, centre, end]
        .iter()
        .any(|zone| zone.iter().any(|placed| placed == id))
}

/// Whether `settings` give the DDC worker a consumer without any reading: the
/// duty is on and the bar's layout holds the `brightness` piece.
#[must_use]
pub fn brightness_consumer(settings: &Settings) -> bool {
    piece_placed(settings, BRIGHTNESS_PIECE)
}

/// Whether `settings` give the audio graph a consumer: the duty is on and
/// the bar's layout holds the `volume` piece, whose sheet lists it.
#[must_use]
pub fn volume_consumer(settings: &Settings) -> bool {
    piece_placed(settings, VOLUME_PIECE)
}

/// Whether the app takes in the workers' readings: only while the duty is on
/// and its workers run, so a reading still queued when the duty went off
/// does not bring the pieces back with a stale level.
#[must_use]
pub fn readings_accepted(settings: &Settings, running: bool) -> bool {
    settings.duties.osd == Duty::On && running
}

/// The display's kinds a sheet of `kind` drives.
#[must_use]
pub fn kinds_of(kind: Kind) -> &'static [OsdKind] {
    match kind {
        Kind::Volume => &[OsdKind::Volume, OsdKind::Microphone],
        Kind::Brightness => &[OsdKind::Brightness],
    }
}

/// The last level the open sheet asked of each kind (`None` for a mute
/// toggle), in the order first asked.
type Asked = Rc<RefCell<Vec<(OsdKind, Option<u8>)>>>;

/// The rows of a monitor list the sheet asked a level of.
type Written = Rc<RefCell<std::collections::BTreeSet<u8>>>;

/// Notes the row `request` addresses, if it is a monitor's.
fn note_written(written: &Written, request: Request) {
    if let Request::Set(Control::Monitor(row), _) = request {
        written.borrow_mut().insert(row);
    }
}

/// Records `request` as the last thing asked of its kind. A stream is not a
/// kind of the display, so its requests are not recorded. A new default
/// device is a change of whichever direction it is in `graph`: the display
/// stays quiet for the read-back of the device's level, as for a mute.
fn record(asked: &Asked, request: Request, graph: &AudioGraph) {
    let (kind, level) = match request {
        Request::Set(Control::Stream(_), _) | Request::ToggleMute(Control::Stream(_)) => return,
        Request::SetDefault(id) => {
            let kind = if graph.outputs.iter().any(|d| d.id == id) {
                OsdKind::Volume
            } else if graph.inputs.iter().any(|d| d.id == id) {
                OsdKind::Microphone
            } else {
                return;
            };
            (kind, None)
        }
        // A monitor of the list keeps the display quiet for the whole settle
        // after the close, as a mute does: the sheet has no one level to
        // name, and none of the readings that follow is news.
        Request::Set(Control::Monitor(_) | Control::AllMonitors, _) => (OsdKind::Brightness, None),
        Request::Set(control, percent) => (kind_of(control), Some(percent)),
        Request::ToggleMute(control) => (kind_of(control), None),
    };
    let mut asked = asked.borrow_mut();
    match asked.iter_mut().find(|(of, _)| *of == kind) {
        Some(entry) => entry.1 = level,
        None => asked.push((kind, level)),
    }
}

/// The display's kind of a default device or a monitor (never a stream).
fn kind_of(control: Control) -> OsdKind {
    match control {
        Control::Output | Control::Stream(_) => OsdKind::Volume,
        Control::Input => OsdKind::Microphone,
        Control::Brightness | Control::Monitor(_) | Control::AllMonitors => OsdKind::Brightness,
    }
}

/// The worker commands of a Volume piece action.
#[must_use]
pub fn audio_commands(action: AudioAction) -> Vec<AudioCommand> {
    match action {
        AudioAction::Step(direction, steps) => (0..steps)
            .map(|_| AudioCommand::Step {
                device: Device::Output,
                direction,
            })
            .collect(),
        AudioAction::ToggleMute => vec![AudioCommand::ToggleMute {
            device: Device::Output,
        }],
    }
}

/// The worker commands of a Brightness piece action on `target`; anything
/// without a monitor is none. A DDC step is a change only: the worker detects lazily for the first one and keeps what
/// it found, so a step never asks for a detection.
#[must_use]
pub fn brightness_commands(
    action: BrightnessAction,
    target: Option<&Target>,
) -> Vec<BrightnessCommand> {
    let (BrightnessAction::Step(direction, steps), Some(target)) = (action, target) else {
        return Vec::new();
    };
    (0..steps)
        .map(|_| BrightnessCommand::Change {
            source: target.source,
            monitor: target.monitor.clone(),
            change: Change::Step(direction),
        })
        .collect()
}

/// Hands `commands` to the workers; a refusal is logged by the worker's
/// handle and never blocks.
pub fn deliver(workers: &dyn Workers, commands: Vec<Command>) {
    for command in commands {
        match command {
            Command::Audio(command) => {
                let _ = workers.audio(command);
            }
            Command::Brightness(command) => {
                let _ = workers.brightness(command);
            }
        }
    }
}

/// What opening a sheet needs besides the Island: its surface, the process's
/// painter, the duty's runtime (the body's requests go there) and the
/// motion setting.
#[derive(Clone, Copy)]
pub struct Opening<'a> {
    pub surface: SurfaceId,
    pub painter: &'a SharedPainter,
    pub runtime: &'a Rc<std::cell::OnceCell<OsdRuntime>>,
    pub reduced: bool,
}

/// A control released by an open: what the display must hear, and the
/// surface it was open on when that is not the one opening (its Island is to
/// be told to close it, `Message::CloseLevelSheet`).
#[derive(Debug, PartialEq)]
pub struct Released {
    pub suppression: Suppression,
    pub elsewhere: Option<SurfaceId>,
}

/// What [`Controls::open`] did: whether the new control opened and what it
/// released first.
#[derive(Debug, Default, PartialEq)]
pub struct Opened {
    pub opened: bool,
    pub released: Option<Released>,
}

/// The level control that is open.
#[derive(Debug)]
struct Open {
    surface: SurfaceId,
    output: String,
    kind: Kind,
    cell: SharedLevels,
    graph: SharedGraph,
    asked: Asked,
    /// The monitors a brightness sheet lists, in the order of its rows; the
    /// sheet's own output first. Empty for a volume sheet.
    targets: Vec<Target>,
    written: Written,
    /// What each of `targets` last read, shared with the body; used only
    /// when the body lists several.
    monitors: SharedMonitors,
}

/// Why a level control is not opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    NoMonitor,
    PieceGone,
}

/// The readings and the level sheet.
#[derive(Debug, Default)]
pub struct Controls {
    pub readings: Readings,
    open: Option<Open>,
}

impl Controls {
    #[must_use]
    pub fn new() -> Controls {
        Controls::default()
    }

    /// What the pieces of `output`'s bar read.
    #[must_use]
    pub fn levels_for(&self, output: &str) -> Levels {
        Levels {
            audio: self.readings.audio(),
            brightness: self.readings.by_output().get(output).copied(),
        }
    }

    /// Runs a Volume piece action.
    pub fn audio(&self, action: AudioAction, workers: &dyn Workers) {
        deliver(
            workers,
            audio_commands(action)
                .into_iter()
                .map(Command::Audio)
                .collect(),
        );
    }

    /// Runs a Brightness piece action for the bar of `output`.
    pub fn brightness(&self, action: BrightnessAction, output: &str, workers: &dyn Workers) {
        let target = self.readings.target(output);
        deliver(
            workers,
            brightness_commands(action, target.as_ref())
                .into_iter()
                .map(Command::Brightness)
                .collect(),
        );
    }

    /// Opens the level control of `kind` as a capsule sheet of `island`
    /// (controlled by `ctl`) grown from the bar's `piece`. A control already
    /// held is released first: one on another surface is named in
    /// `elsewhere` (the app closes it there), one on this surface is
    /// switched by the Island (one morph). When it opened, the display must
    /// be suppressed until [`Controls::settle`] or [`Controls::closed`]
    /// releases it.
    pub fn open(
        &mut self,
        kind: Kind,
        piece: Key,
        island: &mut Island,
        ctl: &mut SurfaceCtl<'_>,
        with: &Opening<'_>,
    ) -> Opened {
        let Opening {
            surface,
            painter,
            runtime,
            reduced,
        } = *with;
        // Every check that can refuse comes before anything is released: a
        // refused open leaves what is held as it was.
        let Ok(borrowed) = painter.try_borrow() else {
            tracing::warn!("the painter is in use; the level control is not opened");
            return Opened {
                opened: false,
                released: None,
            };
        };
        let theme = borrowed.theme().clone();
        drop(borrowed);
        if let Err(refusal) = self.precheck(kind, piece, island) {
            tracing::debug!(?refusal, "the level control is not opened");
            return Opened {
                opened: false,
                released: None,
            };
        }
        let released = self.release_for(surface);
        let none = |released| Opened {
            opened: false,
            released,
        };
        let output = island.output().to_owned();
        let target = self.readings.target(&output);
        // Several monitors make a list; one keeps the single slider. The
        // list is fixed now: a monitor that appears later is listed the next
        // time the sheet opens.
        let listed = match kind {
            Kind::Brightness => self.readings.monitors_for(&output),
            Kind::Volume => Vec::new(),
        };
        let feed = if listed.len() >= 2 {
            MonitorFeed {
                rows: listed
                    .iter()
                    .map(|m| MonitorRow {
                        name: m.name.clone(),
                        primary: m.primary,
                    })
                    .collect(),
                levels: Rc::new(RefCell::new(listed.iter().map(|m| m.percent).collect())),
            }
        } else {
            MonitorFeed::default()
        };
        let targets: Vec<Target> = if feed.rows.is_empty() {
            target.into_iter().collect()
        } else {
            listed.into_iter().map(|m| m.target).collect()
        };
        let monitors = Rc::clone(&feed.levels);
        let cell: SharedLevels = Rc::new(Cell::new(self.levels_for(&output)));
        let graph: SharedGraph = Rc::new(RefCell::new(self.readings.graph().clone()));
        let asked = Asked::default();
        let written = Written::default();
        let sink: Sink = {
            let runtime = Rc::clone(runtime);
            let asked = Rc::clone(&asked);
            let graph = Rc::clone(&graph);
            let targets = targets.clone();
            let written = Rc::clone(&written);
            Rc::new(move |request: Request| {
                record(&asked, request, &graph.borrow());
                note_written(&written, request);
                if let Some(runtime) = runtime.get() {
                    deliver(runtime, popup::commands(request, &targets));
                }
            })
        };
        // The painter is lent to the body's name cutter for this call only:
        // `act` runs after `Island::input` gave it back.
        let core = LevelCore::new(
            &theme,
            kind,
            popup::Feeds {
                cell: Rc::clone(&cell),
                graph: Rc::clone(&graph),
                monitors: feed,
            },
            sink,
            reduced,
            Instant::now(),
            &mut popup::shared_fit(painter, popup::name_style(&theme)),
        );
        if !island.open_level_sheet(piece, core, ctl) {
            // Nothing may stay open untracked.
            island.close_level_sheet(ctl);
            return none(released);
        }
        self.open = Some(Open {
            surface,
            output,
            kind,
            cell,
            graph,
            asked,
            targets,
            written,
            monitors,
        });
        // A fresh reading for what the control shows; the DDC worker reads
        // one monitor at a time and never on a timer.
        if let Some(runtime) = runtime.get() {
            if kind == Kind::Volume {
                runtime.retry_graph_gate();
            }
            let read = match kind {
                Kind::Volume => Command::Audio(AudioCommand::ReadBoth),
                Kind::Brightness => Command::Brightness(BrightnessCommand::Read),
            };
            deliver(runtime, vec![read]);
        }
        Opened {
            opened: true,
            released,
        }
    }

    /// What would refuse the open of `kind` from `piece` on `island`, decided
    /// before anything is released.
    fn precheck(&self, kind: Kind, piece: Key, island: &Island) -> Result<(), Refusal> {
        if kind == Kind::Brightness && self.readings.target(island.output()).is_none() {
            return Err(Refusal::NoMonitor);
        }
        if !island.can_open_level_sheet(piece) {
            return Err(Refusal::PieceGone);
        }
        Ok(())
    }

    /// Releases the control held on a surface other than `surface`: the
    /// display hears it now and the app closes it there.
    fn release_elsewhere(&mut self, surface: SurfaceId) -> Option<Released> {
        let held_on = self.open_surface().filter(|held| *held != surface)?;
        Some(Released {
            suppression: self.closed()?,
            elsewhere: Some(held_on),
        })
    }

    /// Releases the control held, if any, for a new one on `surface`: one on
    /// another surface is named for the app to close; one on this surface is
    /// switched by the Island, and only the display hears its release.
    fn release_for(&mut self, surface: SurfaceId) -> Option<Released> {
        if let Some(released) = self.release_elsewhere(surface) {
            return Some(released);
        }
        self.open_surface()?;
        Some(Released {
            suppression: self.closed()?,
            elsewhere: None,
        })
    }

    /// The audio worker's graph arrived: taken in only while the duty's
    /// workers are `live` (a graph still queued when the duty went off is
    /// dropped); whether the graph changed, so that an open sheet is
    /// refreshed ([`Controls::refresh`]). It touches the readings only: the
    /// display is never told of a graph.
    pub fn audio_graph(&mut self, graph: AudioGraph, live: bool) -> bool {
        live && self.readings.audio_graph(graph)
    }

    /// The surface to redraw because the readings changed (the Island paints
    /// the sheet): the cell the open control reads is brought up to date
    /// first.
    #[must_use]
    pub fn refresh(&self) -> Option<SurfaceId> {
        let open = self.open.as_ref()?;
        let levels = self.levels_for(&open.output);
        let mut changed = false;
        if open.cell.get() != levels {
            open.cell.set(levels);
            changed = true;
        }
        // A sheet that lists several monitors shows each one's reading.
        if open.monitors.borrow().len() >= 2 {
            let levels: Vec<Option<u8>> = open
                .targets
                .iter()
                .map(|target| self.readings.level_of(target))
                .collect();
            if *open.monitors.borrow() != levels {
                open.monitors.replace(levels);
                changed = true;
            }
        }
        // Only a volume sheet lists the graph.
        if open.kind == Kind::Volume && *open.graph.borrow() != *self.readings.graph() {
            open.graph.replace(self.readings.graph().clone());
            changed = true;
        }
        changed.then_some(open.surface)
    }

    /// The Island of `surface` says whether its level sheet is still open (or
    /// waits to open): a held sheet that no longer is has closed, by any way,
    /// and the display hears it once.
    pub fn settle(&mut self, surface: SurfaceId, level_open: bool) -> Option<Suppression> {
        let open = self.open.as_ref()?;
        if open.surface != surface || level_open {
            return None;
        }
        self.closed()
    }

    /// Whether the open sheet is on `surface`'s Island.
    #[must_use]
    pub fn is_open_on(&self, surface: SurfaceId) -> bool {
        self.open.as_ref().is_some_and(|o| o.surface == surface)
    }

    /// The sheet is gone (closed, or its Island with it): what
    /// the display must hear, if there was one.
    pub fn closed(&mut self) -> Option<Suppression> {
        let open = self.open.take()?;
        let asked = open.asked.borrow().clone();
        let written = open.written.borrow().len();
        Some(Suppression::Closed {
            kinds: kinds_of(open.kind),
            asked,
            written,
        })
    }

    /// Surfaces went (an output and its Islands): the sheet on one of them
    /// went too, and is released like a closed one.
    pub fn surfaces_gone(&mut self, gone: &[SurfaceId]) -> Option<Suppression> {
        let surface = self.open_surface()?;
        if gone.contains(&surface) {
            self.closed()
        } else {
            None
        }
    }

    /// The surface of the open sheet's Island.
    #[must_use]
    pub fn open_surface(&self) -> Option<SurfaceId> {
        self.open.as_ref().map(|o| o.surface)
    }
}

/// Settings with the `osd` duty as given and `end` as the bar's end zone, for
/// the tests of whoever reads [`brightness_consumer`].
#[cfg(test)]
pub(crate) fn test_settings(osd: Duty, end: &[&str]) -> Settings {
    use celestina_shell::settings::{BarChoice, Duties};
    Settings {
        duties: Duties {
            osd,
            ..Duties::default()
        },
        bar: BarChoice {
            end: Some(end.iter().map(|s| (*s).to_owned()).collect()),
            ..BarChoice::default()
        },
        ..Settings::default()
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::audio::{Direction, GraphDevice};

    use super::test_settings as settings;
    use super::*;
    use crate::providers::brightness::BrightnessSource;

    #[test]
    fn ddc_has_a_consumer_only_with_the_duty_and_the_piece() {
        let with = ["tray", "brightness"];
        assert!(brightness_consumer(&settings(Duty::On, &with)));
        assert!(!brightness_consumer(&settings(Duty::Off, &with)));
        assert!(!brightness_consumer(&settings(
            Duty::On,
            &["tray", "volume"]
        )));
        // The default end zone places the piece and the default duty is on.
        assert!(brightness_consumer(&Settings::default()));
    }

    #[test]
    fn a_wheel_step_is_one_command_per_step_for_the_default_output() {
        assert_eq!(
            audio_commands(AudioAction::Step(Direction::Up, 3)),
            vec![
                AudioCommand::Step {
                    device: Device::Output,
                    direction: Direction::Up
                };
                3
            ]
        );
        assert_eq!(
            audio_commands(AudioAction::ToggleMute),
            [AudioCommand::ToggleMute {
                device: Device::Output
            }]
        );
    }

    #[test]
    fn a_brightness_step_addresses_the_outputs_monitor_without_asking_to_detect() {
        let ddc = Target {
            source: BrightnessSource::Ddc,
            monitor: "DP-1".to_owned(),
        };
        let step = BrightnessAction::Step(Direction::Down, 2);
        let commands = brightness_commands(step, Some(&ddc));
        // The worker detects lazily on a change; a step never asks again.
        assert_eq!(commands.len(), 2);
        assert!(commands.iter().all(|c| matches!(
            c,
            BrightnessCommand::Change {
                source: BrightnessSource::Ddc,
                change: Change::Step(Direction::Down),
                ..
            }
        )));
        let backlight = Target {
            source: BrightnessSource::Backlight,
            monitor: "intel_backlight".to_owned(),
        };
        let commands = brightness_commands(step, Some(&backlight));
        assert_eq!(commands.len(), 2);
        assert!(brightness_commands(step, None).is_empty());
    }

    #[test]
    fn the_levels_of_a_bar_are_the_audio_and_its_outputs_monitor() {
        use crate::providers::brightness::BrightnessEvent;
        use celestina_wayland::OutputId;
        let mut controls = Controls::new();
        controls.readings.output_added(OutputId(1), "DP-1");
        controls.readings.output_added(OutputId(2), "HDMI-A-1");
        controls.readings.brightness_event(&BrightnessEvent {
            monitor: "DP-1".to_owned(),
            percent: Some(35),
            source: BrightnessSource::Ddc,
        });
        assert_eq!(controls.levels_for("DP-1").brightness, Some(35));
        assert_eq!(controls.levels_for("HDMI-A-1").brightness, None);
        assert_eq!(controls.levels_for("nowhere").brightness, None);
        // No popup is open: nothing to refresh, nothing to release.
        assert!(controls.refresh().is_none());
        assert!(controls.closed().is_none());
        assert!(!controls.is_open_on(SurfaceId(1)));
    }

    #[test]
    fn a_refused_open_releases_nothing_and_leaves_no_orphan() {
        use crate::island::Island;
        use celestina_shell::settings::BarLayout;
        let mut controls = Controls::new();
        let held = opened(&mut controls, Kind::Volume, SurfaceId(1));
        let _ = held;
        let piece = Key::named("volume");
        // No bar: the piece is not on it.
        let mut island = Island::new("DP-1", (1920.0, 560.0));
        assert_eq!(
            controls.precheck(Kind::Volume, piece, &island),
            Err(Refusal::PieceGone)
        );
        // A bar without the piece: still gone.
        island.set_bar(&BarLayout::default());
        assert_eq!(
            controls.precheck(Kind::Volume, piece, &island),
            Err(Refusal::PieceGone)
        );
        // No monitor for this output.
        assert_eq!(
            controls.precheck(Kind::Brightness, Key::named("brightness"), &island),
            Err(Refusal::NoMonitor)
        );
        // Every refusal left the held control as it was: tracked, on its
        // surface, to be released by its own end.
        assert!(controls.is_open_on(SurfaceId(1)));
        assert!(controls.closed().is_some());
    }

    /// A control as `Controls::open` records it, without an Island.
    fn opened(controls: &mut Controls, kind: Kind, surface: SurfaceId) -> Asked {
        let asked = Asked::default();
        controls.open = Some(Open {
            surface,
            output: "DP-1".to_owned(),
            kind,
            cell: Rc::new(Cell::new(Levels::default())),
            graph: Rc::new(RefCell::new(AudioGraph::default())),
            asked: Rc::clone(&asked),
            targets: Vec::new(),
            written: Written::default(),
            monitors: SharedMonitors::default(),
        });
        asked
    }

    #[test]
    fn a_closed_popup_releases_its_kinds_with_the_last_level_asked_of_each() {
        let mut controls = Controls::new();
        let asked = opened(&mut controls, Kind::Volume, SurfaceId(3));
        for request in [
            Request::Set(popup::Control::Output, 30),
            Request::Set(popup::Control::Input, 10),
            Request::Set(popup::Control::Output, 45),
            Request::ToggleMute(popup::Control::Input),
        ] {
            record(&asked, request, &AudioGraph::default());
        }
        assert_eq!(
            controls.closed(),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Volume, OsdKind::Microphone],
                asked: vec![(OsdKind::Volume, Some(45)), (OsdKind::Microphone, None)],
                written: 0,
            })
        );
        assert!(controls.closed().is_none(), "released once");
        assert_eq!(
            Suppression::Open(kinds_of(Kind::Brightness)),
            Suppression::Open(&[OsdKind::Brightness])
        );
    }

    fn device(id: u32, default: bool) -> GraphDevice {
        GraphDevice {
            id,
            name: format!("device {id}"),
            default,
        }
    }

    #[test]
    fn a_new_default_device_quiets_the_kind_of_its_direction_and_a_stream_none() {
        let graph = AudioGraph {
            outputs: vec![device(5, true), device(6, false)],
            inputs: vec![device(9, true)],
            ..AudioGraph::default()
        };
        let asked = Asked::default();
        record(&asked, Request::Set(popup::Control::Output, 40), &graph);
        record(&asked, Request::SetDefault(6), &graph);
        record(&asked, Request::SetDefault(9), &graph);
        // A stream and a device nobody lists are not the display's.
        record(&asked, Request::Set(popup::Control::Stream(70), 10), &graph);
        record(
            &asked,
            Request::ToggleMute(popup::Control::Stream(70)),
            &graph,
        );
        record(&asked, Request::SetDefault(1234), &graph);
        assert_eq!(
            *asked.borrow(),
            [(OsdKind::Volume, None), (OsdKind::Microphone, None)]
        );
    }

    #[test]
    fn a_new_graph_redraws_the_island_of_an_open_volume_sheet() {
        let mut controls = Controls::new();
        let sinks = AudioGraph {
            outputs: vec![device(5, true)],
            ..AudioGraph::default()
        };
        assert!(controls.readings.audio_graph(sinks.clone()));
        assert!(controls.refresh().is_none(), "no popup, nothing to redraw");
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert_eq!(controls.refresh(), Some(SurfaceId(3)));
        assert_eq!(
            controls.open.as_ref().map(|o| o.graph.borrow().clone()),
            Some(sinks)
        );
        assert!(controls.refresh().is_none(), "the same graph again");
        controls.closed();
        // A brightness sheet lists no devices: the graph is none of its news.
        opened(&mut controls, Kind::Brightness, SurfaceId(4));
        assert!(controls.readings.audio_graph(AudioGraph::default()));
        assert!(controls.refresh().is_none());
    }

    #[test]
    fn no_graph_message_reaches_the_osd() {
        // The app's arm for `Message::AudioGraph` is this call and a redraw
        // of the popup it names. The display is told of a level (by the
        // app, from `Message::Audio`) or of a popup opening and closing
        // (a `Suppression`), and a graph is neither.
        let mut controls = Controls::new();
        let before = controls.levels_for("DP-1");
        let asked = opened(&mut controls, Kind::Volume, SurfaceId(3));
        let graph = AudioGraph {
            outputs: vec![device(5, true), device(6, false)],
            inputs: vec![device(9, true)],
            ..AudioGraph::default()
        };
        // Live: the readings take it, and the popup is the one thing that
        // redraws.
        assert!(controls.audio_graph(graph.clone(), true));
        assert_eq!(controls.readings.graph(), &graph);
        assert_eq!(controls.refresh(), Some(SurfaceId(3)));
        assert_eq!(
            controls.open.as_ref().map(|o| o.graph.borrow().clone()),
            Some(graph.clone())
        );
        // No level moved, so nothing for the display to compare; the popup
        // is still open and asked nothing of the display.
        assert_eq!(controls.levels_for("DP-1"), before);
        assert!(asked.borrow().is_empty());
        assert!(controls.is_open_on(SurfaceId(3)));
        // The same graph again: nothing to redraw.
        assert!(!controls.audio_graph(graph.clone(), true));
        // A graph queued past the duty's end is dropped.
        let other = AudioGraph {
            outputs: vec![device(7, true)],
            ..AudioGraph::default()
        };
        assert!(!controls.audio_graph(other, false));
        assert_eq!(controls.readings.graph(), &graph);
        // Closing the popup still releases only the display's kinds.
        assert_eq!(
            controls.closed(),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Volume, OsdKind::Microphone],
                asked: Vec::new(),
                written: 0,
            })
        );
    }

    #[test]
    fn a_volume_sheet_quiets_its_kinds_and_releases_them_once() {
        let mut controls = Controls::new();
        let asked = opened(&mut controls, Kind::Volume, SurfaceId(3));
        record(
            &asked,
            Request::Set(popup::Control::Output, 55),
            &AudioGraph::default(),
        );
        assert_eq!(
            kinds_of(Kind::Volume),
            &[OsdKind::Volume, OsdKind::Microphone]
        );
        // Open on its Island: nothing to release; another Island's answer is
        // not this sheet's.
        assert!(controls.settle(SurfaceId(3), true).is_none());
        assert!(controls.settle(SurfaceId(4), false).is_none());
        assert_eq!(
            controls.settle(SurfaceId(3), false),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Volume, OsdKind::Microphone],
                asked: vec![(OsdKind::Volume, Some(55))],
                written: 0,
            })
        );
        assert!(controls.settle(SurfaceId(3), false).is_none(), "once");
        assert!(controls.closed().is_none());
    }

    #[test]
    fn switching_volume_to_brightness_morphs_and_swaps_the_quieted_kinds() {
        let mut controls = Controls::new();
        let asked = opened(&mut controls, Kind::Volume, SurfaceId(3));
        record(
            &asked,
            Request::Set(popup::Control::Output, 55),
            &AudioGraph::default(),
        );
        // The brightness opens on the same surface: the volume's kinds are
        // released first, with what it asked, and nothing is named for the
        // app to close elsewhere (the Island switches its sheet).
        assert_eq!(
            controls.release_for(SurfaceId(3)),
            Some(Released {
                suppression: Suppression::Closed {
                    kinds: &[OsdKind::Volume, OsdKind::Microphone],
                    asked: vec![(OsdKind::Volume, Some(55))],
                    written: 0,
                },
                elsewhere: None,
            })
        );
        assert!(controls.open_surface().is_none());
        // Then the brightness is the control held, and its kind alone is
        // quiet.
        opened(&mut controls, Kind::Brightness, SurfaceId(3));
        assert_eq!(
            Suppression::Open(kinds_of(Kind::Brightness)),
            Suppression::Open(&[OsdKind::Brightness])
        );
        assert!(controls.is_open_on(SurfaceId(3)));
        // Nothing is released twice.
        assert!(controls.settle(SurfaceId(3), true).is_none());
        assert!(controls.closed().is_some());
        assert!(controls.closed().is_none());
    }

    #[test]
    fn every_way_a_level_sheet_closes_releases_the_display_once() {
        // Escape, an outside press, a switch to the Island page, another
        // piece, the piece vanishing: the Island's sheet is no longer open.
        let mut controls = Controls::new();
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert!(controls.settle(SurfaceId(3), false).is_some());
        assert!(controls.settle(SurfaceId(3), false).is_none());
        assert!(controls.closed().is_none());
        // The duty off closes the sheet and then `closed` releases.
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert!(controls.closed().is_some());
        assert!(controls.settle(SurfaceId(3), false).is_none());
        // The output gone.
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert!(controls.surfaces_gone(&[SurfaceId(3)]).is_some());
        assert!(controls.settle(SurfaceId(3), false).is_none());
        assert!(controls.surfaces_gone(&[SurfaceId(3)]).is_none());
        // Another output opens its own: the first is released, named for the
        // app to close, and releases nothing a second time.
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        let released = controls.release_elsewhere(SurfaceId(4));
        assert!(matches!(
            released,
            Some(Released {
                suppression: Suppression::Closed { .. },
                elsewhere: Some(SurfaceId(3)),
            })
        ));
        assert!(controls.settle(SurfaceId(3), false).is_none());
        assert!(controls.closed().is_none());
        // A switch on the same surface releases the old kinds; the new one
        // is quieted by the app's `Suppression::Open`.
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert!(controls.release_elsewhere(SurfaceId(3)).is_none());
        assert!(controls.open_surface().is_some(), "still held");
        assert!(controls.closed().is_some());
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        assert!(controls.settle(SurfaceId(3), true).is_none(), "still open");
        // The brightness sheet is released by `settle` like the volume's.
        controls.closed();
        opened(&mut controls, Kind::Brightness, SurfaceId(3));
        assert!(matches!(
            controls.settle(SurfaceId(3), false),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Brightness],
                ..
            })
        ));
        assert!(controls.closed().is_none());
    }

    #[test]
    fn opening_a_level_sheet_on_another_output_closes_the_first() {
        let mut controls = Controls::new();
        opened(&mut controls, Kind::Volume, SurfaceId(3));
        let released = controls.release_elsewhere(SurfaceId(5)).expect("released");
        assert_eq!(released.elsewhere, Some(SurfaceId(3)));
        assert!(matches!(released.suppression, Suppression::Closed { .. }));
        assert!(!controls.is_open_on(SurfaceId(3)));
        // Nothing held: nothing to release.
        assert!(controls.release_elsewhere(SurfaceId(5)).is_none());
    }

    #[test]
    fn the_graph_has_a_consumer_only_with_the_duty_and_the_volume_piece() {
        let with = ["tray", "volume"];
        assert!(volume_consumer(&settings(Duty::On, &with)));
        assert!(!volume_consumer(&settings(Duty::Off, &with)));
        assert!(!volume_consumer(&settings(
            Duty::On,
            &["tray", "brightness"]
        )));
    }

    #[test]
    fn a_popup_whose_island_went_with_its_output_is_released() {
        let mut controls = Controls::new();
        opened(&mut controls, Kind::Brightness, SurfaceId(3));
        assert!(controls.surfaces_gone(&[SurfaceId(9)]).is_none());
        assert!(controls.is_open_on(SurfaceId(3)));
        assert_eq!(
            controls.surfaces_gone(&[SurfaceId(9), SurfaceId(3)]),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Brightness],
                asked: Vec::new(),
                written: 0,
            })
        );
        assert!(!controls.is_open_on(SurfaceId(3)));
    }

    #[test]
    fn readings_are_taken_only_while_the_duty_s_workers_run() {
        let on = settings(Duty::On, &["volume"]);
        let off = settings(Duty::Off, &["volume"]);
        assert!(readings_accepted(&on, true));
        assert!(!readings_accepted(&on, false), "workers not running yet");
        assert!(
            !readings_accepted(&off, true),
            "late messages after the off"
        );
        assert!(!readings_accepted(&off, false));
    }

    fn two_monitors() -> Controls {
        use crate::providers::brightness::BrightnessEvent;
        use celestina_wayland::OutputId;
        let mut controls = Controls::new();
        controls.readings.output_added(OutputId(1), "DP-1");
        controls.readings.output_added(OutputId(2), "DP-2");
        for (monitor, percent) in [("DP-1", 20), ("DP-2", 70)] {
            controls.readings.brightness_event(&BrightnessEvent {
                monitor: monitor.to_owned(),
                percent: Some(percent),
                source: BrightnessSource::Ddc,
            });
        }
        controls
    }

    #[test]
    fn a_multi_monitor_popup_quiets_brightness_for_the_whole_settle() {
        let mut controls = two_monitors();
        let asked = opened(&mut controls, Kind::Brightness, SurfaceId(3));
        let written = controls.open.as_ref().map(|o| Rc::clone(&o.written));
        for request in [
            Request::Set(popup::Control::Monitor(1), 40),
            Request::Set(popup::Control::Monitor(0), 55),
            Request::Set(popup::Control::Monitor(2), 55),
            Request::Set(popup::Control::Monitor(1), 60),
        ] {
            record(&asked, request, &AudioGraph::default());
            note_written(&written.clone().expect("open"), request);
        }
        // None is the mute's: the display stays quiet until the settle ends,
        // whatever level the monitors read back.
        assert_eq!(
            controls.closed(),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Brightness],
                asked: vec![(OsdKind::Brightness, None)],
                written: 3,
            })
        );
        assert_eq!(kind_of(popup::Control::Monitor(2)), OsdKind::Brightness);
        assert_eq!(kind_of(popup::Control::AllMonitors), OsdKind::Brightness);
        // The single slider still names its level.
        let single = Asked::default();
        record(
            &single,
            Request::Set(popup::Control::Brightness, 30),
            &AudioGraph::default(),
        );
        assert_eq!(*single.borrow(), [(OsdKind::Brightness, Some(30))]);
    }

    #[test]
    fn refresh_brings_every_listed_monitor_up_to_date() {
        use crate::providers::brightness::BrightnessEvent;
        let mut controls = two_monitors();
        let targets: Vec<Target> = controls
            .readings
            .monitors_for("DP-1")
            .into_iter()
            .map(|m| m.target)
            .collect();
        let levels: SharedMonitors = Rc::new(RefCell::new(vec![Some(20), Some(70)]));
        opened(&mut controls, Kind::Brightness, SurfaceId(3));
        let current = controls.levels_for("DP-1");
        if let Some(open) = controls.open.as_mut() {
            open.cell.set(current);
            open.targets = targets;
            open.monitors = Rc::clone(&levels);
        }
        assert!(controls.refresh().is_none(), "nothing moved");
        controls.readings.brightness_event(&BrightnessEvent {
            monitor: "DP-2".to_owned(),
            percent: Some(45),
            source: BrightnessSource::Ddc,
        });
        // Only the second monitor moved, and it is not the bar's own.
        assert_eq!(controls.refresh(), Some(SurfaceId(3)));
        assert_eq!(*levels.borrow(), [Some(20), Some(45)]);
        assert!(controls.refresh().is_none());
        controls.readings.brightness_event(&BrightnessEvent {
            monitor: "DP-1".to_owned(),
            percent: None,
            source: BrightnessSource::Ddc,
        });
        assert_eq!(controls.refresh(), Some(SurfaceId(3)));
        assert_eq!(*levels.borrow(), [None, Some(45)]);
    }
}
