//! The bar's side of the duty (CEL-2-C, plan decisions 5, 8 and 9): what the
//! Volume and Brightness pieces and their popup ask of the workers, and the
//! readings they show.
//!
//! [`Controls`] is the one place that holds the readings ([`Readings`]) and
//! the slider popup that is open. Everything it does to the world is a
//! command to a worker through [`Workers`], so it is tested without a bus, a
//! child or a compositor; the app only forwards its messages and its
//! actions here.
//!
//! - A **piece action** (wheel step, mute) becomes worker commands for the
//!   default output, or for the monitor of the Island's output.
//! - **Opening a popup** builds its card over a cell of readings the app keeps
//!   current, asks the worker for a fresh reading (one per monitor for the
//!   DDC worker, never concurrent) and remembers which popup it is, so the
//!   display can be suppressed while it is open and released when it closes.
//! - **DDC is gated** on a consumer: the worker is asked to detect once,
//!   when the duty is on and a layout comes to hold the `brightness` piece
//!   (see `OsdRuntime::set_brightness_consumer`); a step or a slider is a
//!   change, which the worker detects for lazily, never a detection request.
//! - **Closing a popup** releases the display's kinds with the last level the
//!   popup asked of each, so their read-backs are not announced.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use celestina_paint::Rect;
use celestina_shell::osd::Kind as OsdKind;
use celestina_shell::settings::{BarLayout, Duty, Settings};
use celestina_wayland::{PopupId, SurfaceCtl, SurfaceId};

use super::popup::{self, Command, Control, Kind, LevelPopup, Request, SharedLevels, Sink};
use super::readings::{Readings, Target};
use super::verbs::Workers;
use super::{OsdRuntime, Suppression};
use crate::island::{menu::SharedPainter, Island};
use crate::pieces::{AudioAction, BrightnessAction, Levels};
use crate::providers::audio::{AudioCommand, Device};
use crate::providers::brightness::{BrightnessCommand, Change};

/// The id of the brightness piece in a layout.
const BRIGHTNESS_PIECE: &str = "brightness";

/// Whether `settings` give the DDC worker a consumer without any reading: the
/// duty is on, the bar is on and its layout holds the `brightness` piece.
#[must_use]
pub fn brightness_consumer(settings: &Settings) -> bool {
    if settings.duties.osd != Duty::On || settings.duties.bar() != Duty::On {
        return false;
    }
    let BarLayout {
        start, centre, end, ..
    } = settings.bar_layout();
    [start, centre, end]
        .iter()
        .any(|zone| zone.iter().any(|id| id == BRIGHTNESS_PIECE))
}

/// Whether the app takes in the workers' readings: only while the duty is on
/// and its workers run, so a reading still queued when the duty went off
/// does not bring the pieces back with a stale level.
#[must_use]
pub fn readings_accepted(settings: &Settings, running: bool) -> bool {
    settings.duties.osd == Duty::On && running
}

/// The display's kinds a popup of `kind` drives.
#[must_use]
pub fn kinds_of(kind: Kind) -> &'static [OsdKind] {
    match kind {
        Kind::Volume => &[OsdKind::Volume, OsdKind::Microphone],
        Kind::Brightness => &[OsdKind::Brightness],
    }
}

/// The last level the open popup asked of each kind (`None` for a mute
/// toggle), in the order first asked.
type Asked = Rc<RefCell<Vec<(OsdKind, Option<u8>)>>>;

/// Records `request` as the last thing asked of its kind.
fn record(asked: &Asked, request: Request) {
    let (control, level) = match request {
        Request::Set(control, percent) => (control, Some(percent)),
        Request::ToggleMute(control) => (control, None),
    };
    let kind = match control {
        Control::Output => OsdKind::Volume,
        Control::Input => OsdKind::Microphone,
        Control::Brightness => OsdKind::Brightness,
    };
    let mut asked = asked.borrow_mut();
    match asked.iter_mut().find(|(of, _)| *of == kind) {
        Some(entry) => entry.1 = level,
        None => asked.push((kind, level)),
    }
}

/// The worker commands of a Volume piece action; the popup is not one.
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
        AudioAction::Popup => Vec::new(),
    }
}

/// The worker commands of a Brightness piece action on `target`; the popup
/// is not one, and neither is anything without a monitor. A DDC step is a
/// change only: the worker detects lazily for the first one and keeps what
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

/// What opening a popup needs besides the Island: its surface, the process's
/// painter, the duty's runtime (the popup's requests go there) and the
/// motion setting.
#[derive(Clone, Copy)]
pub struct Opening<'a> {
    pub surface: SurfaceId,
    pub painter: &'a SharedPainter,
    pub runtime: &'a Rc<std::cell::OnceCell<OsdRuntime>>,
    pub reduced: bool,
}

/// The slider popup that is open.
#[derive(Debug)]
struct Open {
    surface: SurfaceId,
    popup: PopupId,
    output: String,
    kind: Kind,
    cell: SharedLevels,
    asked: Asked,
}

/// The readings and the slider popup.
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

    /// Opens the slider popup of `kind` below the piece at `anchor` of
    /// `island` (controlled by `ctl`). Returns whether it opened; then the
    /// display must be suppressed until [`Controls::closed`] says it is gone.
    pub fn open(
        &mut self,
        kind: Kind,
        anchor: Rect,
        island: &mut Island,
        ctl: &mut SurfaceCtl<'_>,
        with: &Opening<'_>,
    ) -> bool {
        let Opening {
            surface,
            painter,
            runtime,
            reduced,
        } = *with;
        if self.open.is_some() {
            return false;
        }
        let Ok(borrowed) = painter.try_borrow() else {
            tracing::warn!("the painter is in use; the level popup is not opened");
            return false;
        };
        let theme = borrowed.theme().clone();
        drop(borrowed);
        let output = island.output().to_owned();
        let target = self.readings.target(&output);
        if kind == Kind::Brightness && target.is_none() {
            tracing::debug!(%output, "no monitor for this output; no brightness popup");
            return false;
        }
        let cell: SharedLevels = Rc::new(Cell::new(self.levels_for(&output)));
        let asked = Asked::default();
        let sink: Sink = {
            let runtime = Rc::clone(runtime);
            let asked = Rc::clone(&asked);
            Rc::new(move |request: Request| {
                record(&asked, request);
                if let Some(runtime) = runtime.get() {
                    deliver(runtime, popup::commands(request, target.as_ref()));
                }
            })
        };
        let shell = LevelPopup::new(
            Rc::clone(painter),
            &theme,
            kind,
            Rc::clone(&cell),
            sink,
            reduced,
        );
        let size = popup::popup_size(&theme, kind, shell.has_microphone());
        let Some(popup) = island.open_level_popup(anchor, size, Box::new(shell), ctl) else {
            return false;
        };
        self.open = Some(Open {
            surface,
            popup,
            output,
            kind,
            cell,
            asked,
        });
        // A fresh reading for what the popup shows; the DDC worker reads one
        // monitor at a time and never on a timer.
        if let Some(runtime) = runtime.get() {
            let read = match kind {
                Kind::Volume => Command::Audio(AudioCommand::ReadBoth),
                Kind::Brightness => Command::Brightness(BrightnessCommand::Read),
            };
            deliver(runtime, vec![read]);
        }
        true
    }

    /// The popup (and its surface) to redraw because the readings changed:
    /// the cell it reads is brought up to date first.
    #[must_use]
    pub fn refresh(&self) -> Option<(SurfaceId, PopupId)> {
        let open = self.open.as_ref()?;
        let levels = self.levels_for(&open.output);
        if open.cell.get() == levels {
            return None;
        }
        open.cell.set(levels);
        Some((open.surface, open.popup))
    }

    /// Whether the open popup hangs from `surface`.
    #[must_use]
    pub fn is_open_on(&self, surface: SurfaceId) -> bool {
        self.open.as_ref().is_some_and(|o| o.surface == surface)
    }

    /// The popup is gone (closed, dismissed, or its Island with it): what
    /// the display must hear, if there was one.
    pub fn closed(&mut self) -> Option<Suppression> {
        let open = self.open.take()?;
        let asked = open.asked.borrow().clone();
        Some(Suppression::Closed {
            kinds: kinds_of(open.kind),
            asked,
        })
    }

    /// Surfaces went (an output and its Islands): the popup hanging from one
    /// of them went too, and is released like a closed one.
    pub fn surfaces_gone(&mut self, gone: &[SurfaceId]) -> Option<Suppression> {
        let surface = self.open_surface()?;
        if gone.contains(&surface) {
            self.closed()
        } else {
            None
        }
    }

    /// The surface of the open popup's Island.
    #[must_use]
    pub fn open_surface(&self) -> Option<SurfaceId> {
        self.open.as_ref().map(|o| o.surface)
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::audio::Direction;
    use celestina_shell::settings::{BarChoice, Duties};

    use super::*;
    use crate::providers::brightness::BrightnessSource;

    fn settings(osd: Duty, bar: Duty, end: &[&str]) -> Settings {
        Settings {
            duties: Duties {
                osd,
                bar: Some(bar),
                ..Duties::default()
            },
            bar: BarChoice {
                end: Some(end.iter().map(|s| (*s).to_owned()).collect()),
                ..BarChoice::default()
            },
            ..Settings::default()
        }
    }

    #[test]
    fn ddc_has_a_consumer_only_with_the_duty_the_bar_and_the_piece() {
        let with = ["tray", "brightness"];
        assert!(brightness_consumer(&settings(Duty::On, Duty::On, &with)));
        assert!(!brightness_consumer(&settings(Duty::Off, Duty::On, &with)));
        assert!(!brightness_consumer(&settings(Duty::On, Duty::Off, &with)));
        assert!(!brightness_consumer(&settings(
            Duty::On,
            Duty::On,
            &["tray", "volume"]
        )));
        assert!(!brightness_consumer(&Settings::default()));
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
        assert!(audio_commands(AudioAction::Popup).is_empty());
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
        assert!(brightness_commands(BrightnessAction::Popup, Some(&ddc)).is_empty());
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

    /// A popup as `Controls::open` records it, without an Island.
    fn opened(controls: &mut Controls, kind: Kind, surface: SurfaceId) -> Asked {
        let asked = Asked::default();
        controls.open = Some(Open {
            surface,
            popup: celestina_wayland::PopupId(7),
            output: "DP-1".to_owned(),
            kind,
            cell: Rc::new(Cell::new(Levels::default())),
            asked: Rc::clone(&asked),
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
            record(&asked, request);
        }
        assert_eq!(
            controls.closed(),
            Some(Suppression::Closed {
                kinds: &[OsdKind::Volume, OsdKind::Microphone],
                asked: vec![(OsdKind::Volume, Some(45)), (OsdKind::Microphone, None)],
            })
        );
        assert!(controls.closed().is_none(), "released once");
        assert_eq!(
            Suppression::Open(kinds_of(Kind::Brightness)),
            Suppression::Open(&[OsdKind::Brightness])
        );
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
            })
        );
        assert!(!controls.is_open_on(SurfaceId(3)));
    }

    #[test]
    fn readings_are_taken_only_while_the_duty_s_workers_run() {
        let on = settings(Duty::On, Duty::On, &["volume"]);
        let off = settings(Duty::Off, Duty::On, &["volume"]);
        assert!(readings_accepted(&on, true));
        assert!(!readings_accepted(&on, false), "workers not running yet");
        assert!(
            !readings_accepted(&off, true),
            "late messages after the off"
        );
        assert!(!readings_accepted(&off, false));
    }
}
