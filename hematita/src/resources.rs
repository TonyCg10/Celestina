// language-contract: product-copy
//! The Performance page's state, as Qt properties.
//!
//! This object owns the only policy numbers in Hematita — what counts as
//! elevated and critical — and the two history rings. It receives whole
//! snapshots from the sampler on the Qt thread and republishes them as typed
//! properties; nothing here reads a file.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QVariant};

use hematita_core::history::Ring;

use crate::sampler::{Sampler, Section, Snapshot};

/// Above this a value is worth noticing; above [`CRITICAL_PERCENT`] it is
/// worth interrupting for. The page maps these to appearance; the numbers are
/// policy and live here, not in the theme.
pub const ELEVATED_PERCENT: u8 = 80;
pub const CRITICAL_PERCENT: u8 = 90;

/// The state name the theme colours by.
#[must_use]
pub fn load_name(percent: u8) -> &'static str {
    if percent >= CRITICAL_PERCENT {
        "critical"
    } else if percent >= ELEVATED_PERCENT {
        "elevated"
    } else {
        "normal"
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        // The histories are minute-long sequences of fractions. `QList<f64>`
        // is what they are, and the engine hands it to JavaScript as an array
        // — but its cxx-qt alias, `QList_f64`, is a name qmllint cannot
        // resolve, so every binding that read a history was an
        // `unresolved-type` warning and this project suppresses none. A
        // `QVariant` carrying a variant list is the one shape both the linter
        // and the engine understand; the boxing is the price of a truthful
        // lint.
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // available / unavailableReason — the last snapshot had CPU and
        //   memory, or why it did not
        // generation — bumped per applied snapshot so QML can animate
        //   "a new sample arrived" on one signal
        // cpu* — aggregate percent (-1 before the first rate), load name,
        //   model, MHz (0 when unknown), core count, one minute of history
        //   as fractions 0..=1 oldest first
        // memory* / swap* — kibibytes as doubles (QML has no 64-bit int)
        #[qobject]
        #[qml_element]
        #[qproperty(bool, available)]
        #[qproperty(QString, unavailable_reason)]
        #[qproperty(i32, generation)]
        #[qproperty(i32, cpu_percent)]
        #[qproperty(QString, cpu_load)]
        #[qproperty(QString, cpu_model)]
        #[qproperty(i32, cpu_frequency_mhz)]
        #[qproperty(i32, cpu_cores)]
        #[qproperty(QVariant, cpu_history)]
        #[qproperty(i32, memory_percent)]
        #[qproperty(QString, memory_load)]
        #[qproperty(f64, memory_used_kib)]
        #[qproperty(f64, memory_total_kib)]
        #[qproperty(QVariant, memory_history)]
        #[qproperty(f64, swap_used_kib)]
        #[qproperty(f64, swap_total_kib)]
        type HematitaResources = super::HematitaResourcesRust;

        /// Starts the sampler, once. The window calls it when it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaResources>);
    }

    impl cxx_qt::Threading for HematitaResources {}
}

pub struct HematitaResourcesRust {
    available: bool,
    unavailable_reason: QString,
    generation: i32,
    cpu_percent: i32,
    cpu_load: QString,
    cpu_model: QString,
    cpu_frequency_mhz: i32,
    cpu_cores: i32,
    cpu_history: QVariant,
    memory_percent: i32,
    memory_load: QString,
    memory_used_kib: f64,
    memory_total_kib: f64,
    memory_history: QVariant,
    swap_used_kib: f64,
    swap_total_kib: f64,
    cpu_ring: Ring,
    memory_ring: Ring,
    last_generation: u64,
    sampler: Option<Sampler>,
}

impl Default for HematitaResourcesRust {
    fn default() -> Self {
        // The CPU identity is read from `/proc` like every other fact, so it
        // arrives with the first snapshot rather than being read here: this
        // constructor runs on the Qt thread.
        Self {
            available: false,
            unavailable_reason: QString::default(),
            generation: 0,
            cpu_percent: -1,
            cpu_load: QString::from("normal"),
            cpu_model: QString::default(),
            cpu_frequency_mhz: 0,
            cpu_cores: 0,
            cpu_history: ring_list(&Ring::new()),
            memory_percent: 0,
            memory_load: QString::from("normal"),
            memory_used_kib: 0.0,
            memory_total_kib: 0.0,
            memory_history: ring_list(&Ring::new()),
            swap_used_kib: 0.0,
            swap_total_kib: 0.0,
            cpu_ring: Ring::new(),
            memory_ring: Ring::new(),
            last_generation: 0,
            sampler: None,
        }
    }
}

fn ring_list(ring: &Ring) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for value in ring.values() {
        list.append(QVariant::from(&f64::from(value)));
    }
    QVariant::from(&list)
}

impl qobject::HematitaResources {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().sampler.is_some() {
            return;
        }
        let qt = self.qt_thread();
        match Sampler::spawn(move |snapshot| {
            let _ = qt.queue(move |resources: Pin<&mut qobject::HematitaResources>| {
                resources.apply(snapshot);
            });
        }) {
            Ok(sampler) => self.as_mut().rust_mut().sampler = Some(sampler),
            Err(error) => {
                self.as_mut().set_available(false);
                self.as_mut().set_unavailable_reason(QString::from(
                    format!("No se pudo iniciar la lectura: {error}").as_str(),
                ));
            }
        }
    }

    /// Applies one whole snapshot. A snapshot older than the last applied one
    /// is dropped: the thread publishes in order, but the queue does not
    /// promise to.
    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        if snapshot.generation <= self.rust().last_generation {
            return;
        }
        self.as_mut().rust_mut().last_generation = snapshot.generation;

        if let Some(identity) = &snapshot.identity {
            let model = QString::from(identity.cpu_model.as_str());
            let cores = i32::try_from(identity.cpu_cores).unwrap_or(i32::MAX);
            self.as_mut().set_cpu_model(model);
            self.as_mut().set_cpu_cores(cores);
        }

        let mut reasons = Vec::new();
        match &snapshot.cpu {
            Section::Available(Some(reading)) => {
                let percent = reading.aggregate_percent;
                self.as_mut()
                    .rust_mut()
                    .cpu_ring
                    .push(f32::from(percent) / 100.0);
                let history = ring_list(&self.rust().cpu_ring);
                self.as_mut().set_cpu_percent(i32::from(percent));
                self.as_mut()
                    .set_cpu_load(QString::from(load_name(percent)));
                let mhz = reading
                    .frequency_mhz
                    .map_or(0, |mhz| i32::try_from(mhz).unwrap_or(i32::MAX));
                self.as_mut().set_cpu_frequency_mhz(mhz);
                self.as_mut().set_cpu_history(history);
            }
            Section::Available(None) => {}
            Section::Unavailable(reason) => reasons.push(reason.clone()),
        }
        match &snapshot.memory {
            Section::Available(memory) => {
                let percent = memory.used_percent();
                self.as_mut()
                    .rust_mut()
                    .memory_ring
                    .push(f32::from(percent) / 100.0);
                let history = ring_list(&self.rust().memory_ring);
                self.as_mut().set_memory_percent(i32::from(percent));
                self.as_mut()
                    .set_memory_load(QString::from(load_name(percent)));
                // Kibibytes never reach 2^53, so the double is exact.
                self.as_mut().set_memory_used_kib(memory.used_kib as f64);
                self.as_mut().set_memory_total_kib(memory.total_kib as f64);
                self.as_mut().set_swap_used_kib(memory.swap_used_kib as f64);
                self.as_mut()
                    .set_swap_total_kib(memory.swap_total_kib as f64);
                self.as_mut().set_memory_history(history);
            }
            Section::Unavailable(reason) => reasons.push(reason.clone()),
        }

        let available = reasons.is_empty();
        self.as_mut().set_available(available);
        self.as_mut()
            .set_unavailable_reason(QString::from(reasons.join("; ").as_str()));
        // Wrapped into the positive half of an i32: QML has no 64-bit integer,
        // and the property is a change ticket, not a count anybody adds to.
        let generation = i32::try_from(snapshot.generation % u64::from(u32::MAX / 2)).unwrap_or(0);
        self.as_mut().set_generation(generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_names_the_state_the_page_paints() {
        assert_eq!(load_name(0), "normal");
        assert_eq!(load_name(79), "normal");
        assert_eq!(load_name(ELEVATED_PERCENT), "elevated");
        assert_eq!(load_name(89), "elevated");
        assert_eq!(load_name(CRITICAL_PERCENT), "critical");
        assert_eq!(load_name(100), "critical");
    }
}
