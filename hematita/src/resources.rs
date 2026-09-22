//! The Performance page's state, as Qt properties.
//!
//! Rows travel as index-aligned lists plus a `revision` ticket rather than a
//! native model: CXX-Qt 0.9 cannot override `QAbstractListModel`'s virtuals
//! from Rust, and the suite already publishes list data this way. The page
//! rebuilds its rows when `revision` changes and never binds to a single list,
//! so it never sees one column from this second beside another from the last.
//!
//! Nothing here is prose a person reads: kinds, states and reasons are tokens
//! the page turns into Spanish through `qsTr()`.

use std::collections::HashMap;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use hematita_core::history::Ring;

use crate::lists::{nested, strings, widen};
use crate::publish::{self, Kind};
use crate::sampler::{self, Reason, Section, Snapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        // Lists of lists of doubles cross as a `QVariant`: it is the one shape
        // both qmllint and the engine resolve (see H1-D), and the page reads
        // them by row index.
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // revision — bumped once, after every list is in place
        // resource* — index-aligned rows: key, kind token, data label, state
        //   token, reason token and path, load token, kind-contract numbers,
        //   minute of history as fractions
        // cpuCoreHistories — one minute per core, fractions
        // cpuModel — the processor's name, for the CPU detail
        // startFailed — the sampling thread could not be created
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QStringList, resource_keys)]
        #[qproperty(QStringList, resource_kinds)]
        #[qproperty(QStringList, resource_labels)]
        #[qproperty(QStringList, resource_states)]
        #[qproperty(QStringList, resource_reason_kinds)]
        #[qproperty(QStringList, resource_reason_paths)]
        #[qproperty(QStringList, resource_loads)]
        #[qproperty(QVariant, resource_numbers)]
        #[qproperty(QVariant, resource_histories)]
        #[qproperty(QVariant, cpu_core_histories)]
        #[qproperty(QString, cpu_model)]
        #[qproperty(bool, start_failed)]
        type HematitaResources = super::HematitaResourcesRust;

        /// Starts the sampler, once. The window calls it when it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaResources>);

        /// Stops the shared sampler thread and waits for it. The window calls
        /// it when it goes away, so the thread does not outlive the objects
        /// its snapshots are queued to.
        #[qinvokable]
        fn shutdown(self: Pin<&mut HematitaResources>);
    }

    impl cxx_qt::Threading for HematitaResources {}
}

/// One published row before it is split into columns.
struct Row {
    key: String,
    kind: Kind,
    label: String,
    state: &'static str,
    reason: Option<Reason>,
    load: &'static str,
    numbers: Vec<f64>,
    history: Vec<f32>,
}

impl Row {
    fn unavailable(key: String, kind: Kind, label: String, reason: Reason) -> Self {
        Self {
            key,
            kind,
            label,
            state: "unavailable",
            reason: Some(reason),
            load: "normal",
            numbers: Vec::new(),
            history: Vec::new(),
        }
    }
}

pub struct HematitaResourcesRust {
    revision: i32,
    resource_keys: QStringList,
    resource_kinds: QStringList,
    resource_labels: QStringList,
    resource_states: QStringList,
    resource_reason_kinds: QStringList,
    resource_reason_paths: QStringList,
    resource_loads: QStringList,
    resource_numbers: QVariant,
    resource_histories: QVariant,
    cpu_core_histories: QVariant,
    cpu_model: QString,
    start_failed: bool,
    /// One ring per row key; a key that leaves the machine takes its ring
    /// with it, a key that returns starts a fresh minute.
    rings: HashMap<String, Ring>,
    core_rings: Vec<Ring>,
    cpu_cores: usize,
    gpu_id: String,
    last_generation: u64,
    started: bool,
}

impl Default for HematitaResourcesRust {
    fn default() -> Self {
        Self {
            revision: 0,
            resource_keys: QStringList::default(),
            resource_kinds: QStringList::default(),
            resource_labels: QStringList::default(),
            resource_states: QStringList::default(),
            resource_reason_kinds: QStringList::default(),
            resource_reason_paths: QStringList::default(),
            resource_loads: QStringList::default(),
            resource_numbers: nested(&[]),
            resource_histories: nested(&[]),
            cpu_core_histories: nested(&[]),
            cpu_model: QString::default(),
            start_failed: false,
            rings: HashMap::new(),
            core_rings: Vec::new(),
            cpu_cores: 0,
            gpu_id: String::new(),
            last_generation: 0,
            started: false,
        }
    }
}

impl qobject::HematitaResources {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let outcome = sampler::subscribe(move |snapshot: &Snapshot| {
            let snapshot = snapshot.clone();
            let _ = qt.queue(move |resources: Pin<&mut qobject::HematitaResources>| {
                resources.apply(snapshot);
            });
        });
        if outcome.is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    pub fn shutdown(self: Pin<&mut Self>) {
        sampler::stop();
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        if !publish::accepts(snapshot.generation, self.rust().last_generation) {
            return;
        }
        self.as_mut().rust_mut().last_generation = snapshot.generation;

        if let Some(identity) = &snapshot.identity {
            let model = QString::from(identity.cpu_model.as_str());
            self.as_mut().set_cpu_model(model);
            let mut state = self.as_mut().rust_mut();
            state.cpu_cores = identity.cpu_cores;
            state.gpu_id = identity.gpu_id.clone();
            state.core_rings = (0..identity.cpu_cores).map(|_| Ring::new()).collect();
        }

        let rows = self.as_mut().rust_mut().rows_from(&snapshot);

        let keys = strings(rows.iter().map(|row| row.key.clone()));
        let kinds = strings(rows.iter().map(|row| row.kind.as_str().to_owned()));
        let labels = strings(rows.iter().map(|row| row.label.clone()));
        let states = strings(rows.iter().map(|row| row.state.to_owned()));
        let reason_kinds = strings(rows.iter().map(|row| {
            row.reason
                .as_ref()
                .map_or(String::new(), |reason| reason.kind.as_str().to_owned())
        }));
        let reason_paths = strings(rows.iter().map(|row| {
            row.reason
                .as_ref()
                .map_or(String::new(), |reason| reason.path.clone())
        }));
        let loads = strings(rows.iter().map(|row| row.load.to_owned()));
        let numbers = nested(
            &rows
                .iter()
                .map(|row| row.numbers.clone())
                .collect::<Vec<_>>(),
        );
        let histories = nested(
            &rows
                .iter()
                .map(|row| widen(&row.history))
                .collect::<Vec<_>>(),
        );
        let cores = nested(
            &self
                .rust()
                .core_rings
                .iter()
                .map(|ring| widen(&ring.values()))
                .collect::<Vec<_>>(),
        );

        self.as_mut().set_resource_keys(keys);
        self.as_mut().set_resource_kinds(kinds);
        self.as_mut().set_resource_labels(labels);
        self.as_mut().set_resource_states(states);
        self.as_mut().set_resource_reason_kinds(reason_kinds);
        self.as_mut().set_resource_reason_paths(reason_paths);
        self.as_mut().set_resource_loads(loads);
        self.as_mut().set_resource_numbers(numbers);
        self.as_mut().set_resource_histories(histories);
        self.as_mut().set_cpu_core_histories(cores);
        // Last, so the page rebuilds once, with every column in place.
        let ticket = publish::ticket(snapshot.generation);
        self.as_mut().set_revision(ticket);
    }
}

impl HematitaResourcesRust {
    /// Builds the rows for one snapshot and advances the rings. Rings whose
    /// key is absent from this snapshot are dropped.
    fn rows_from(&mut self, snapshot: &Snapshot) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        // Rings advance only from the second snapshot on, so CPU (which has
        // no rate on the first) and everything else stay aligned.
        let advance = snapshot.generation >= 2;

        // CPU
        match &snapshot.cpu {
            Section::Available(Some(reading)) => {
                let history =
                    self.push("cpu", publish::fraction(reading.aggregate_percent), advance);
                // The count the page shows and the rings it draws come from
                // this second's reading, so a hot-plugged core (or a
                // `/proc/stat` unreadable at startup) is not frozen at
                // whatever the identity read once.
                let cores = reading.core_percents.len();
                if self.core_rings.len() != cores {
                    self.core_rings = (0..cores).map(|_| Ring::new()).collect();
                    self.cpu_cores = cores;
                }
                for (ring, percent) in self.core_rings.iter_mut().zip(&reading.core_percents) {
                    if advance {
                        ring.push(publish::fraction(*percent));
                    }
                }
                rows.push(Row {
                    key: "cpu".to_owned(),
                    kind: Kind::Cpu,
                    label: self.cpu_model_string(),
                    state: "ready",
                    reason: None,
                    load: publish::load_name(reading.aggregate_percent),
                    numbers: publish::cpu_numbers(reading, cores),
                    history,
                });
            }
            Section::Available(None) => rows.push(Row {
                key: "cpu".to_owned(),
                kind: Kind::Cpu,
                label: self.cpu_model_string(),
                state: "waiting",
                reason: None,
                load: "normal",
                numbers: Vec::new(),
                history: self.peek("cpu"),
            }),
            Section::Unavailable(reason) => rows.push(Row::unavailable(
                "cpu".to_owned(),
                Kind::Cpu,
                self.cpu_model_string(),
                reason.clone(),
            )),
        }
        seen.push("cpu".to_owned());

        // Memory
        match &snapshot.memory {
            Section::Available(memory) => {
                let percent = memory.used_percent();
                let history = self.push("memory", publish::fraction(percent), advance);
                rows.push(Row {
                    key: "memory".to_owned(),
                    kind: Kind::Memory,
                    label: String::new(),
                    state: "ready",
                    reason: None,
                    load: publish::load_name(percent),
                    numbers: publish::memory_numbers(memory),
                    history,
                });
            }
            Section::Unavailable(reason) => rows.push(Row::unavailable(
                "memory".to_owned(),
                Kind::Memory,
                String::new(),
                reason.clone(),
            )),
        }
        seen.push("memory".to_owned());

        // GPU (absent is not a row)
        if let Some(section) = &snapshot.gpu {
            match section {
                Section::Available(reading) => {
                    let history =
                        self.push("gpu", publish::fraction(reading.busy_percent), advance);
                    rows.push(Row {
                        key: "gpu".to_owned(),
                        kind: Kind::Gpu,
                        label: self.gpu_id.clone(),
                        state: "ready",
                        reason: None,
                        load: publish::load_name(reading.busy_percent),
                        numbers: publish::gpu_numbers(reading),
                        history,
                    });
                }
                Section::Unavailable(reason) => rows.push(Row::unavailable(
                    "gpu".to_owned(),
                    Kind::Gpu,
                    self.gpu_id.clone(),
                    reason.clone(),
                )),
            }
            seen.push("gpu".to_owned());
        }

        // Disks
        for disk in &snapshot.disks {
            let key = format!("disk:{}", disk.info.name);
            let label = if disk.info.model.is_empty() {
                disk.info.name.clone()
            } else {
                disk.info.model.clone()
            };
            match &disk.rate {
                Section::Available(rate) => {
                    let history = self.push_throughput(&key, *rate, advance);
                    rows.push(Row {
                        key: key.clone(),
                        kind: Kind::Disk,
                        label,
                        state: if rate.is_some() { "ready" } else { "waiting" },
                        reason: None,
                        load: "normal",
                        numbers: publish::disk_numbers(&disk.info, *rate),
                        history,
                    });
                }
                Section::Unavailable(reason) => {
                    rows.push(Row::unavailable(
                        key.clone(),
                        Kind::Disk,
                        label,
                        reason.clone(),
                    ));
                }
            }
            seen.push(key);
        }

        // Interfaces
        for interface in &snapshot.interfaces {
            let key = format!("net:{}", interface.info.name);
            match &interface.rate {
                Section::Available(rate) => {
                    let history = self.push_throughput(&key, *rate, advance);
                    rows.push(Row {
                        key: key.clone(),
                        kind: Kind::Network,
                        label: interface.info.name.clone(),
                        state: if rate.is_some() { "ready" } else { "waiting" },
                        reason: None,
                        load: "normal",
                        numbers: publish::network_numbers(&interface.info, *rate),
                        history,
                    });
                }
                Section::Unavailable(reason) => rows.push(Row::unavailable(
                    key.clone(),
                    Kind::Network,
                    interface.info.name.clone(),
                    reason.clone(),
                )),
            }
            seen.push(key);
        }

        self.rings.retain(|key, _| seen.contains(key));
        rows
    }

    fn cpu_model_string(&self) -> String {
        self.cpu_model.to_string()
    }

    /// Pushes a fraction into a row's ring and answers its values.
    fn push(&mut self, key: &str, value: f32, advance: bool) -> Vec<f32> {
        let ring = self.rings.entry(key.to_owned()).or_default();
        if advance {
            ring.push(value);
        }
        ring.values()
    }

    /// Pushes a throughput into a row's ring and answers it scaled by its
    /// own peak, which is the only sensible ceiling for bytes per second.
    fn push_throughput(&mut self, key: &str, rate: Option<[f64; 2]>, advance: bool) -> Vec<f32> {
        let ring = self.rings.entry(key.to_owned()).or_default();
        if advance && rate.is_some() {
            ring.push(publish::throughput(rate));
        }
        ring.fractions()
    }

    fn peek(&self, key: &str) -> Vec<f32> {
        self.rings
            .get(key)
            .map_or_else(|| Ring::new().values(), Ring::values)
    }
}
