//! What the shell last read of the session's audio and brightness, and which
//! output each monitor belongs to (CEL-2-C, plan decisions 7 and 8).
//!
//! The workers speak of *monitors*: a DDC monitor by the connector niri names
//! its output by, a backlight by its sysfs device name. The bar and the verbs
//! speak of *outputs*. [`Readings`] is the one place that relates them:
//!
//! - a DDC monitor belongs to the output of its own name, if that output
//!   exists;
//! - a backlight belongs to the internal output (`eDP*`, `LVDS*`, `DSI*`) if
//!   one exists, and to none otherwise (its level then shows nowhere and a
//!   verb for it is refused).
//!
//! It is pure state fed by the app from the messages it already receives; it
//! reads and commands nothing. While the `osd` duty is off nothing feeds it,
//! so the pieces that read it are absent.

use std::collections::BTreeMap;

use celestina_wayland::OutputId;

use crate::pieces::{AudioLevels, Level};
use crate::providers::audio::{AudioEvent, Device};
use crate::providers::brightness::{BrightnessEvent, BrightnessSource};

/// Whether the connector `name` is a laptop's own panel.
#[must_use]
pub fn is_internal(name: &str) -> bool {
    ["eDP", "LVDS", "DSI"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The monitor a brightness change addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub source: BrightnessSource,
    pub monitor: String,
}

/// The shell's picture of the audio devices and the monitors.
#[derive(Debug, Default)]
pub struct Readings {
    audio: AudioLevels,
    /// The outputs, in announcement order: id and connector name.
    outputs: Vec<(OutputId, String)>,
    /// The last known level of each monitor; `None` is unknown.
    monitors: BTreeMap<(BrightnessSource, String), Option<u8>>,
    /// The level of the monitor of each output, derived from the above.
    by_output: BTreeMap<String, u8>,
}

impl Readings {
    #[must_use]
    pub fn new() -> Readings {
        Readings::default()
    }

    /// The default devices' readings.
    #[must_use]
    pub fn audio(&self) -> AudioLevels {
        self.audio
    }

    /// The level of each output's monitor, by output name.
    #[must_use]
    pub fn by_output(&self) -> &BTreeMap<String, u8> {
        &self.by_output
    }

    /// Records the audio worker's reading; whether anything changed.
    pub fn audio_event(&mut self, event: AudioEvent) -> bool {
        let level = event.level.map(|l| Level {
            percent: l.percent,
            muted: l.muted,
        });
        let slot = match event.device {
            Device::Output => &mut self.audio.output,
            Device::Input => &mut self.audio.input,
        };
        std::mem::replace(slot, level) != level
    }

    /// Records a monitor's reading; whether what the outputs show changed.
    pub fn brightness_event(&mut self, event: &BrightnessEvent) -> bool {
        self.monitors
            .insert((event.source, event.monitor.clone()), event.percent);
        self.derive()
    }

    /// An output appeared (or was announced again under a new name).
    pub fn output_added(&mut self, id: OutputId, name: &str) -> bool {
        match self.outputs.iter_mut().find(|(known, _)| *known == id) {
            Some(entry) => entry.1 = name.to_owned(),
            None => self.outputs.push((id, name.to_owned())),
        }
        self.derive()
    }

    /// An output is gone.
    pub fn output_removed(&mut self, id: OutputId) -> bool {
        self.outputs.retain(|(known, _)| *known != id);
        self.derive()
    }

    /// The duty is off: every reading is forgotten (the outputs stay).
    /// Whether anything was held.
    pub fn clear(&mut self) -> bool {
        let held = self.audio != AudioLevels::default() || !self.monitors.is_empty();
        self.audio = AudioLevels::default();
        self.monitors.clear();
        self.by_output.clear();
        held
    }

    /// Whether an output of this name exists.
    #[must_use]
    pub fn has_output(&self, name: &str) -> bool {
        self.outputs.iter().any(|(_, known)| known == name)
    }

    /// The monitor a change for `output` addresses: the backlight the shell
    /// has read, for the internal output; else the DDC monitor of that name.
    /// `None` for an output that does not exist.
    #[must_use]
    pub fn target(&self, output: &str) -> Option<Target> {
        if !self.has_output(output) {
            return None;
        }
        if is_internal(output) {
            let backlight = self
                .monitors
                .keys()
                .find(|(source, _)| *source == BrightnessSource::Backlight);
            if let Some((source, monitor)) = backlight {
                return Some(Target {
                    source: *source,
                    monitor: monitor.clone(),
                });
            }
        }
        Some(Target {
            source: BrightnessSource::Ddc,
            monitor: output.to_owned(),
        })
    }

    /// Rebuilds `by_output`; whether it changed.
    fn derive(&mut self) -> bool {
        let internal = self
            .outputs
            .iter()
            .map(|(_, name)| name)
            .find(|name| is_internal(name));
        let mut derived = BTreeMap::new();
        for ((source, monitor), percent) in &self.monitors {
            let Some(percent) = percent else { continue };
            let output = match source {
                BrightnessSource::Ddc => self
                    .outputs
                    .iter()
                    .map(|(_, name)| name)
                    .find(|name| *name == monitor),
                BrightnessSource::Backlight => internal,
            };
            if let Some(output) = output {
                derived.entry(output.clone()).or_insert(*percent);
            }
        }
        std::mem::replace(&mut self.by_output, derived) != self.by_output
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::audio::AudioLevel;

    use super::*;

    fn ddc(monitor: &str, percent: Option<u8>) -> BrightnessEvent {
        BrightnessEvent {
            monitor: monitor.to_owned(),
            percent,
            source: BrightnessSource::Ddc,
        }
    }

    fn backlight(device: &str, percent: Option<u8>) -> BrightnessEvent {
        BrightnessEvent {
            monitor: device.to_owned(),
            percent,
            source: BrightnessSource::Backlight,
        }
    }

    fn with_outputs(names: &[&str]) -> Readings {
        let mut r = Readings::new();
        for (i, name) in names.iter().enumerate() {
            r.output_added(OutputId(i as u32 + 1), name);
        }
        r
    }

    #[test]
    fn internal_connectors_are_the_panels_of_a_laptop() {
        for name in ["eDP-1", "LVDS-1", "DSI-1"] {
            assert!(is_internal(name), "{name}");
        }
        for name in ["DP-1", "HDMI-A-1", "edp-1", "XeDP", ""] {
            assert!(!is_internal(name), "{name}");
        }
    }

    #[test]
    fn a_ddc_monitor_belongs_to_the_output_of_its_name() {
        let mut r = with_outputs(&["DP-1", "HDMI-A-1"]);
        assert!(r.brightness_event(&ddc("DP-1", Some(40))));
        assert_eq!(r.by_output().get("DP-1"), Some(&40));
        assert_eq!(r.by_output().get("HDMI-A-1"), None);
        // The same reading again changes nothing.
        assert!(!r.brightness_event(&ddc("DP-1", Some(40))));
        // A monitor with no output shows nowhere.
        assert!(!r.brightness_event(&ddc("DP-9", Some(10))));
        assert_eq!(r.by_output().len(), 1);
    }

    #[test]
    fn a_reading_that_arrives_before_its_output_shows_once_the_output_does() {
        let mut r = Readings::new();
        assert!(!r.brightness_event(&ddc("DP-1", Some(55))));
        assert!(r.output_added(OutputId(1), "DP-1"));
        assert_eq!(r.by_output().get("DP-1"), Some(&55));
        assert!(r.output_removed(OutputId(1)));
        assert!(r.by_output().is_empty());
    }

    #[test]
    fn unknown_is_not_zero() {
        let mut r = with_outputs(&["DP-1"]);
        r.brightness_event(&ddc("DP-1", Some(0)));
        assert_eq!(r.by_output().get("DP-1"), Some(&0));
        assert!(r.brightness_event(&ddc("DP-1", None)));
        assert!(r.by_output().is_empty());
    }

    #[test]
    fn a_backlight_belongs_to_the_internal_output_or_to_none() {
        let mut desk = with_outputs(&["DP-1"]);
        assert!(!desk.brightness_event(&backlight("intel_backlight", Some(30))));
        assert!(desk.by_output().is_empty());
        assert_eq!(
            desk.target("DP-1").map(|t| t.source),
            Some(BrightnessSource::Ddc)
        );

        let mut laptop = with_outputs(&["eDP-1", "DP-1"]);
        assert!(laptop.brightness_event(&backlight("intel_backlight", Some(30))));
        assert_eq!(laptop.by_output().get("eDP-1"), Some(&30));
        assert_eq!(laptop.by_output().get("DP-1"), None);
        assert_eq!(
            laptop.target("eDP-1"),
            Some(Target {
                source: BrightnessSource::Backlight,
                monitor: "intel_backlight".to_owned()
            })
        );
        assert_eq!(
            laptop.target("DP-1"),
            Some(Target {
                source: BrightnessSource::Ddc,
                monitor: "DP-1".to_owned()
            })
        );
    }

    #[test]
    fn an_unknown_output_has_no_target() {
        let r = with_outputs(&["DP-1"]);
        assert_eq!(r.target("DP-2"), None);
        assert_eq!(r.target(""), None);
        assert!(r.target("DP-1").is_some());
    }

    #[test]
    fn audio_readings_are_kept_per_device_and_report_changes() {
        let mut r = Readings::new();
        let level = |percent, muted| Some(AudioLevel { percent, muted });
        let event = |device, level| AudioEvent { device, level };
        assert!(r.audio_event(event(Device::Output, level(30, false))));
        assert!(!r.audio_event(event(Device::Output, level(30, false))));
        assert!(r.audio_event(event(Device::Input, level(80, true))));
        assert_eq!(
            r.audio(),
            AudioLevels {
                output: Some(Level {
                    percent: 30,
                    muted: false
                }),
                input: Some(Level {
                    percent: 80,
                    muted: true
                }),
            }
        );
        assert!(r.audio_event(event(Device::Output, None)));
        assert!(r.audio().output.is_none() && r.audio().input.is_some());
    }

    #[test]
    fn clearing_forgets_the_readings_and_keeps_the_outputs() {
        let mut r = with_outputs(&["DP-1"]);
        assert!(!r.clear(), "nothing held");
        r.brightness_event(&ddc("DP-1", Some(10)));
        r.audio_event(AudioEvent {
            device: Device::Output,
            level: Some(AudioLevel {
                percent: 1,
                muted: false,
            }),
        });
        assert!(r.clear());
        assert!(r.by_output().is_empty() && r.audio() == AudioLevels::default());
        assert!(r.has_output("DP-1"));
        // The monitors were forgotten: a later output pairs with nothing.
        r.output_added(OutputId(2), "HDMI-A-1");
        assert!(r.by_output().is_empty());
    }
}
