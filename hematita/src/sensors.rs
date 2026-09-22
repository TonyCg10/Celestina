//! The Sensors page's state, as Qt properties: every chip and channel of the
//! latest snapshot as index-aligned lists, plus the session's extremes per
//! channel, which only this object remembers.
//!
//! Nothing here is prose a person reads: kinds, loads and reasons are tokens
//! the page turns into Spanish through `qsTr()`, and a chip's driver name and
//! a channel's label are the kernel's own data, shown raw.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use hematita_core::sensors::ChannelKind;

use crate::lists::{doubles, strings};
use crate::publish;
use crate::sampler::{self, Reason, Section, SensorSnapshot, Snapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // revision — bumped once, after every list is in place
        // chip* — one entry per chip: its `hwmonN` key, its driver name and
        //   how many channels it published
        // channel* — one entry per channel of every chip in chip order: the
        //   chip's index, the kind token, the kernel's index within the kind,
        //   its label, its value, the session's extremes, the kernel's two
        //   limits (0 for absent) and the load token
        // available/reason* — whether `/sys/class/hwmon` could be read
        // startFailed — the sampling thread could not be created
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QStringList, chip_keys)]
        #[qproperty(QStringList, chip_names)]
        #[qproperty(QVariant, chip_counts)]
        #[qproperty(QVariant, channel_chips)]
        #[qproperty(QStringList, channel_kinds)]
        #[qproperty(QVariant, channel_indices)]
        #[qproperty(QStringList, channel_labels)]
        #[qproperty(QVariant, channel_values)]
        #[qproperty(QVariant, channel_mins)]
        #[qproperty(QVariant, channel_maxs)]
        #[qproperty(QVariant, channel_limit_max)]
        #[qproperty(QVariant, channel_limit_crit)]
        #[qproperty(QStringList, channel_loads)]
        #[qproperty(bool, available)]
        #[qproperty(QString, reason_kind)]
        #[qproperty(QString, reason_path)]
        #[qproperty(bool, start_failed)]
        type HematitaSensors = super::HematitaSensorsRust;

        /// Starts the sampler, once. The window calls it when it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaSensors>);
    }

    impl cxx_qt::Threading for HematitaSensors {}
}

pub struct HematitaSensorsRust {
    revision: i32,
    chip_keys: QStringList,
    chip_names: QStringList,
    chip_counts: QVariant,
    channel_chips: QVariant,
    channel_kinds: QStringList,
    channel_indices: QVariant,
    channel_labels: QStringList,
    channel_values: QVariant,
    channel_mins: QVariant,
    channel_maxs: QVariant,
    channel_limit_max: QVariant,
    channel_limit_crit: QVariant,
    channel_loads: QStringList,
    available: bool,
    reason_kind: QString,
    reason_path: QString,
    start_failed: bool,
    started: bool,
    last_generation: u64,
    /// `chipKey/chipName/kind/index` → (session min, session max).
    extremes: HashMap<String, (f64, f64)>,
}

impl Default for HematitaSensorsRust {
    fn default() -> Self {
        Self {
            revision: 0,
            chip_keys: QStringList::default(),
            chip_names: QStringList::default(),
            chip_counts: doubles(&[]),
            channel_chips: doubles(&[]),
            channel_kinds: QStringList::default(),
            channel_indices: doubles(&[]),
            channel_labels: QStringList::default(),
            channel_values: doubles(&[]),
            channel_mins: doubles(&[]),
            channel_maxs: doubles(&[]),
            channel_limit_max: doubles(&[]),
            channel_limit_crit: doubles(&[]),
            channel_loads: QStringList::default(),
            available: true,
            reason_kind: QString::default(),
            reason_path: QString::default(),
            start_failed: false,
            started: false,
            last_generation: 0,
            extremes: HashMap::new(),
        }
    }
}

/// The session's extremes for one channel after seeing `value`: the first
/// reading is both ends, and every later one widens whichever end it passes.
#[must_use]
fn fold_extremes(previous: Option<(f64, f64)>, value: f64) -> (f64, f64) {
    match previous {
        Some((min, max)) => (min.min(value), max.max(value)),
        None => (value, value),
    }
}

/// The name a channel's extremes are remembered under. The `hwmonN` key is
/// stable for the session, which is exactly as long as the extremes mean
/// anything — but only while it names the same device: the index of a device
/// that goes away can be handed to the next one to bind. The chip's driver
/// name is therefore part of the key, so a re-bound index starts fresh
/// extremes instead of inheriting another device's minimum and maximum.
fn extreme_key(chip_key: &str, chip_name: &str, kind: ChannelKind, index: u32) -> String {
    format!("{chip_key}/{chip_name}/{}/{index}", kind.as_str())
}

impl qobject::HematitaSensors {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let outcome = sampler::subscribe(move |snapshot: &Snapshot| {
            let generation = snapshot.generation;
            let section = snapshot.sensors.clone();
            let _ = qt.queue(move |sensors: Pin<&mut qobject::HematitaSensors>| {
                sensors.apply(generation, section);
            });
        });
        if outcome.is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    /// Every published list back to empty. A failed tick shows its reason and
    /// nothing else.
    fn clear_lists(mut self: Pin<&mut Self>) {
        self.as_mut().set_chip_keys(QStringList::default());
        self.as_mut().set_chip_names(QStringList::default());
        self.as_mut().set_chip_counts(doubles(&[]));
        self.as_mut().set_channel_chips(doubles(&[]));
        self.as_mut().set_channel_kinds(QStringList::default());
        self.as_mut().set_channel_indices(doubles(&[]));
        self.as_mut().set_channel_labels(QStringList::default());
        self.as_mut().set_channel_values(doubles(&[]));
        self.as_mut().set_channel_mins(doubles(&[]));
        self.as_mut().set_channel_maxs(doubles(&[]));
        self.as_mut().set_channel_limit_max(doubles(&[]));
        self.as_mut().set_channel_limit_crit(doubles(&[]));
        self.as_mut().set_channel_loads(QStringList::default());
    }

    fn apply(mut self: Pin<&mut Self>, generation: u64, section: Section<SensorSnapshot>) {
        if !publish::accepts(generation, self.rust().last_generation) {
            return;
        }
        self.as_mut().rust_mut().last_generation = generation;
        let snapshot = match section {
            Section::Available(snapshot) => {
                self.as_mut().set_available(true);
                self.as_mut().set_reason_kind(QString::default());
                self.as_mut().set_reason_path(QString::default());
                snapshot
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().set_available(false);
                self.as_mut().set_reason_kind(QString::from(kind.as_str()));
                self.as_mut().set_reason_path(QString::from(path.as_str()));
                // The last good chips are not this tick's reading, and a
                // column of values frozen beside a line saying they could not
                // be read is worse than no values: clear every list and bump
                // the ticket, so the page shows only the reason. The extremes
                // map is left alone — a tick that reads again resumes the
                // session's minima and maxima instead of starting over.
                self.as_mut().clear_lists();
                let ticket = publish::ticket(generation);
                self.as_mut().set_revision(ticket);
                return;
            }
        };

        // First pass: the extremes, which are the only thing this object
        // remembers between snapshots. A channel the machine no longer
        // publishes takes its extremes with it.
        {
            let state = &mut *self.as_mut().rust_mut();
            let mut seen: HashSet<String> = HashSet::new();
            for chip in &snapshot.chips {
                for channel in &chip.channels {
                    let key = extreme_key(&chip.key, &chip.name, channel.kind, channel.index);
                    let folded = fold_extremes(state.extremes.get(&key).copied(), channel.value);
                    state.extremes.insert(key.clone(), folded);
                    seen.insert(key);
                }
            }
            state.extremes.retain(|key, _| seen.contains(key));
        }

        // Second pass: the lists, reading the extremes back.
        let mut chip_keys = Vec::new();
        let mut chip_names = Vec::new();
        let mut chip_counts = Vec::new();
        let mut chips = Vec::new();
        let mut kinds = Vec::new();
        let mut indices = Vec::new();
        let mut labels = Vec::new();
        let mut values = Vec::new();
        let mut mins = Vec::new();
        let mut maxs = Vec::new();
        let mut limit_max = Vec::new();
        let mut limit_crit = Vec::new();
        let mut loads = Vec::new();
        for (chip_index, chip) in snapshot.chips.iter().enumerate() {
            chip_keys.push(chip.key.clone());
            chip_names.push(chip.name.clone());
            chip_counts.push(chip.channels.len() as f64);
            for channel in &chip.channels {
                let key = extreme_key(&chip.key, &chip.name, channel.kind, channel.index);
                let (min, max) = self
                    .rust()
                    .extremes
                    .get(&key)
                    .copied()
                    .unwrap_or((channel.value, channel.value));
                chips.push(chip_index as f64);
                kinds.push(channel.kind.as_str().to_owned());
                indices.push(f64::from(channel.index));
                labels.push(channel.label.clone());
                values.push(channel.value);
                mins.push(min);
                maxs.push(max);
                limit_max.push(channel.limit_max.unwrap_or(0.0));
                limit_crit.push(channel.limit_crit.unwrap_or(0.0));
                loads.push(
                    match channel.kind {
                        ChannelKind::Temperature => {
                            publish::thermal_load(channel.value, channel.limit_crit)
                        }
                        _ => "normal",
                    }
                    .to_owned(),
                );
            }
        }

        self.as_mut().set_chip_keys(strings(chip_keys));
        self.as_mut().set_chip_names(strings(chip_names));
        self.as_mut().set_chip_counts(doubles(&chip_counts));
        self.as_mut().set_channel_chips(doubles(&chips));
        self.as_mut().set_channel_kinds(strings(kinds));
        self.as_mut().set_channel_indices(doubles(&indices));
        self.as_mut().set_channel_labels(strings(labels));
        self.as_mut().set_channel_values(doubles(&values));
        self.as_mut().set_channel_mins(doubles(&mins));
        self.as_mut().set_channel_maxs(doubles(&maxs));
        self.as_mut().set_channel_limit_max(doubles(&limit_max));
        self.as_mut().set_channel_limit_crit(doubles(&limit_crit));
        self.as_mut().set_channel_loads(strings(loads));
        // Last, so a page that rebuilds on `revision` never reads one list
        // from this snapshot beside another from the last.
        let ticket = publish::ticket(generation);
        self.as_mut().set_revision(ticket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extremes_start_at_the_first_reading_and_only_widen() {
        assert_eq!(fold_extremes(None, 42.0), (42.0, 42.0));
        assert_eq!(fold_extremes(Some((42.0, 42.0)), 50.0), (42.0, 50.0));
        assert_eq!(fold_extremes(Some((42.0, 50.0)), 30.0), (30.0, 50.0));
        assert_eq!(fold_extremes(Some((30.0, 50.0)), 40.0), (30.0, 50.0));
    }

    #[test]
    fn an_extreme_is_remembered_per_chip_name_kind_and_index() {
        assert_eq!(
            extreme_key("hwmon6", "k10temp", ChannelKind::Temperature, 3),
            "hwmon6/k10temp/temperature/3"
        );
        assert_ne!(
            extreme_key("hwmon6", "it8696", ChannelKind::Fan, 1),
            extreme_key("hwmon7", "it8696", ChannelKind::Fan, 1)
        );
        // The same index re-bound to another device is another channel.
        assert_ne!(
            extreme_key("hwmon6", "k10temp", ChannelKind::Temperature, 1),
            extreme_key("hwmon6", "amdgpu", ChannelKind::Temperature, 1)
        );
    }
}
