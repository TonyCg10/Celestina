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

use celestina_shell::audio::AudioGraph;
use celestina_shell::bounded::{bounded, MAX_TEXT_UNITS};
use celestina_wayland::OutputId;

use crate::copy;
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

/// The most monitors the brightness popup lists.
pub const MAX_MONITORS: usize = 8;

/// A monitor as the brightness popup lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorLevel {
    pub target: Target,
    /// The model, or else the connector (both when two share a model).
    pub name: String,
    /// The monitor of the bar the popup hangs from.
    pub primary: bool,
    /// `None` is unknown.
    pub percent: Option<u8>,
}

/// The shell's picture of the audio devices and the monitors.
#[derive(Debug, Default)]
pub struct Readings {
    audio: AudioLevels,
    /// The devices and playback streams the audio worker last published,
    /// bounded; empty until it publishes one.
    graph: AudioGraph,
    /// The outputs, in announcement order: id and connector name.
    outputs: Vec<(OutputId, String)>,
    /// The model each output announced, when it announced a usable one.
    models: BTreeMap<OutputId, String>,
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

    /// The audio graph the worker last published.
    #[must_use]
    pub fn graph(&self) -> &AudioGraph {
        &self.graph
    }

    /// Records the audio worker's graph (cut to its bounds again, whatever
    /// the sender did); whether it differs from the one held.
    pub fn audio_graph(&mut self, graph: AudioGraph) -> bool {
        std::mem::replace(&mut self.graph, graph.bounded()) != self.graph
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
        self.models.remove(&id);
        self.derive()
    }

    /// The model the compositor announced for an output: kept bounded and
    /// trimmed. An empty one, or `Unknown`, is no model at all, and the
    /// monitor is then named by its connector.
    pub fn set_model(&mut self, id: OutputId, model: Option<&str>) {
        let model = model.map(|m| bounded(m.trim(), MAX_TEXT_UNITS).trim().to_owned());
        match model {
            Some(model) if !model.is_empty() && model != "Unknown" => {
                self.models.insert(id, model);
            }
            _ => {
                self.models.remove(&id);
            }
        }
    }

    /// The duty is off: every reading is forgotten (the outputs stay).
    /// Whether anything was held.
    pub fn clear(&mut self) -> bool {
        let held = self.audio != AudioLevels::default()
            || !self.monitors.is_empty()
            || self.graph != AudioGraph::default();
        self.audio = AudioLevels::default();
        self.graph = AudioGraph::default();
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

    /// The last known level of the monitor `target`; `None` when unknown or
    /// never reported.
    #[must_use]
    pub fn level_of(&self, target: &Target) -> Option<u8> {
        self.monitors
            .get(&(target.source, target.monitor.clone()))
            .copied()
            .flatten()
    }

    /// The monitors the popup of `output`'s bar lists: that bar's own first
    /// (even with no reading yet), then each other output's whose monitor
    /// the worker has reported, by connector; at most [`MAX_MONITORS`].
    /// Empty when `output` does not exist.
    #[must_use]
    pub fn monitors_for(&self, output: &str) -> Vec<MonitorLevel> {
        let Some(first) = self.target(output) else {
            return Vec::new();
        };
        let mut listed = vec![(output.to_owned(), first)];
        let mut others: Vec<&(OutputId, String)> = self
            .outputs
            .iter()
            .filter(|(_, name)| name != output)
            .collect();
        others.sort_by(|a, b| a.1.cmp(&b.1));
        for (_, name) in others {
            let Some(target) = self.target(name) else {
                continue;
            };
            let reported = self
                .monitors
                .contains_key(&(target.source, target.monitor.clone()));
            if reported && !listed.iter().any(|(_, known)| *known == target) {
                listed.push((name.clone(), target));
            }
        }
        listed.truncate(MAX_MONITORS);
        let names: Vec<String> = listed
            .iter()
            .map(|(connector, _)| {
                self.outputs
                    .iter()
                    .find(|(_, name)| name == connector)
                    .and_then(|(id, _)| self.models.get(id))
                    .cloned()
                    .unwrap_or_else(|| connector.clone())
            })
            .collect();
        listed
            .into_iter()
            .enumerate()
            .map(|(at, (connector, target))| {
                let shared = names.iter().filter(|name| **name == names[at]).count() > 1;
                MonitorLevel {
                    name: if shared {
                        copy::monitor_with_connector(&names[at], &connector)
                    } else {
                        names[at].clone()
                    },
                    primary: at == 0,
                    percent: self.level_of(&target),
                    target,
                }
            })
            .collect()
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
    fn the_graph_is_kept_bounded_and_clearing_forgets_it() {
        use celestina_shell::audio::{GraphDevice, MAX_GRAPH_DEVICES};
        let mut r = Readings::new();
        assert!(!r.audio_graph(AudioGraph::default()), "nothing new");
        let sinks = (0..12)
            .map(|id| GraphDevice {
                id,
                name: format!("sink {id}"),
                default: id == 0,
            })
            .collect();
        let graph = AudioGraph {
            outputs: sinks,
            ..AudioGraph::default()
        };
        assert!(r.audio_graph(graph.clone()));
        assert_eq!(r.graph().outputs.len(), MAX_GRAPH_DEVICES);
        assert!(!r.audio_graph(graph), "the same graph changes nothing");
        assert!(r.clear());
        assert_eq!(r.graph(), &AudioGraph::default());
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

    fn model_event(r: &mut Readings, id: u32, model: &str) {
        r.set_model(OutputId(id), Some(model));
    }

    fn names(list: &[MonitorLevel]) -> Vec<&str> {
        list.iter().map(|m| m.name.as_str()).collect()
    }

    #[test]
    fn monitors_list_the_bars_output_first_then_the_others_by_connector() {
        let mut r = with_outputs(&["HDMI-A-1", "DP-2", "DP-1"]);
        r.brightness_event(&ddc("DP-1", Some(10)));
        r.brightness_event(&ddc("DP-2", Some(20)));
        r.brightness_event(&ddc("HDMI-A-1", None));
        let list = r.monitors_for("HDMI-A-1");
        assert_eq!(names(&list), ["HDMI-A-1", "DP-1", "DP-2"]);
        assert_eq!(
            list.iter().map(|m| m.primary).collect::<Vec<_>>(),
            [true, false, false]
        );
        assert_eq!(
            list.iter().map(|m| m.percent).collect::<Vec<_>>(),
            [None, Some(10), Some(20)]
        );
        // The bar's own comes first even with no reading at all.
        let mut bare = with_outputs(&["DP-1"]);
        assert_eq!(names(&bare.monitors_for("DP-1")), ["DP-1"]);
        assert_eq!(bare.monitors_for("DP-1")[0].percent, None);
        assert!(bare.monitors_for("nowhere").is_empty());
        bare.brightness_event(&ddc("DP-1", Some(5)));
        assert_eq!(bare.monitors_for("DP-1")[0].percent, Some(5));
    }

    #[test]
    fn a_monitor_is_named_by_its_model_or_else_its_connector() {
        let mut r = with_outputs(&["DP-1", "DP-2", "DP-3", "DP-4"]);
        for name in ["DP-1", "DP-2", "DP-3", "DP-4"] {
            r.brightness_event(&ddc(name, Some(50)));
        }
        model_event(&mut r, 1, " LG ULTRAGEAR+ ");
        model_event(&mut r, 2, "");
        model_event(&mut r, 3, "Unknown");
        r.set_model(OutputId(4), None);
        assert_eq!(
            names(&r.monitors_for("DP-1")),
            ["LG ULTRAGEAR+", "DP-2", "DP-3", "DP-4"]
        );
        // Two of one model say which is which.
        model_event(&mut r, 2, "LG ULTRAGEAR+");
        assert_eq!(
            names(&r.monitors_for("DP-1")),
            [
                "LG ULTRAGEAR+ (DP-1)",
                "LG ULTRAGEAR+ (DP-2)",
                "DP-3",
                "DP-4"
            ]
        );
        // A model that is then empty is forgotten.
        model_event(&mut r, 2, "");
        assert_eq!(names(&r.monitors_for("DP-1"))[0], "LG ULTRAGEAR+");
        // A model is bounded, and an output going takes its model along.
        model_event(&mut r, 3, &"M".repeat(5000));
        assert!(r.monitors_for("DP-1")[2].name.len() < 5000);
        r.output_removed(OutputId(1));
        r.output_added(OutputId(1), "DP-1");
        assert_eq!(names(&r.monitors_for("DP-1"))[0], "DP-1");
        // Clearing the readings keeps what the outputs announced.
        model_event(&mut r, 4, "Dell");
        r.clear();
        r.brightness_event(&ddc("DP-4", Some(1)));
        assert_eq!(names(&r.monitors_for("DP-1")), ["DP-1", "Dell"]);
    }

    #[test]
    fn only_reported_monitors_are_listed_besides_the_bars_own() {
        let mut r = with_outputs(&["DP-1", "DP-2", "DP-3"]);
        r.brightness_event(&ddc("DP-2", None));
        // DP-3 was never reported: it is not a monitor the worker knows.
        assert_eq!(names(&r.monitors_for("DP-1")), ["DP-1", "DP-2"]);
        assert_eq!(names(&r.monitors_for("DP-3")), ["DP-3", "DP-2"]);
    }

    #[test]
    fn the_backlight_is_listed_as_the_internal_panel() {
        let mut r = with_outputs(&["eDP-1", "DP-1"]);
        r.brightness_event(&backlight("intel_backlight", Some(30)));
        r.brightness_event(&ddc("DP-1", Some(60)));
        let list = r.monitors_for("DP-1");
        assert_eq!(names(&list), ["DP-1", "eDP-1"]);
        assert_eq!(list[1].target.source, BrightnessSource::Backlight);
        assert_eq!(list[1].percent, Some(30));
        assert_eq!(r.level_of(&list[1].target), Some(30));
        let list = r.monitors_for("eDP-1");
        assert_eq!(names(&list), ["eDP-1", "DP-1"]);
        assert!(list[0].primary);
        // Without a backlight reading the panel has nothing to list.
        let mut desk = with_outputs(&["eDP-1", "DP-1"]);
        desk.brightness_event(&ddc("DP-1", Some(60)));
        assert_eq!(names(&desk.monitors_for("DP-1")), ["DP-1"]);
    }

    #[test]
    fn the_list_stops_at_max_monitors() {
        let connectors: Vec<String> = (1..=12).map(|n| format!("DP-{n:02}")).collect();
        let refs: Vec<&str> = connectors.iter().map(String::as_str).collect();
        let mut r = with_outputs(&refs);
        for name in &refs {
            r.brightness_event(&ddc(name, Some(50)));
        }
        let list = r.monitors_for("DP-12");
        assert_eq!(list.len(), MAX_MONITORS);
        assert_eq!(list[0].name, "DP-12", "the bar's own is never cut");
        assert_eq!(list[1].name, "DP-01");
    }
}
