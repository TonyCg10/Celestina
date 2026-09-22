//! The Processes and Applications pages' state, as Qt properties.
//!
//! Rows are index-aligned lists plus a `revision`, like the resources. The
//! projection — what survives the search, in which order, under which
//! application — is decided in `hematita_core::process_view`; this object
//! holds the state that projection reads, applies it to the latest process
//! snapshot, and is the only place a signal is sent from.
//!
//! Nothing here is prose a person reads: sort fields, reasons and the outcome
//! of an action are tokens the page turns into Spanish through `qsTr()`.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use celestina_core::desktop_entry;
use hematita_core::process_view::{self, ProcessRow, SortField};

use crate::lists::{doubles, strings};
use crate::sampler::{self, ProcessReading, ProcessSnapshot, Reason, Section, Snapshot};

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
        // revision — bumped once per applied projection
        // sortField / sortAscending / filterText / grouped — the state QML sets
        // process* — index-aligned rows (see the plan's row contract)
        // group* — index-aligned groups, only when grouped
        // totalCount / shownCount — before and after the filter
        // available / reasonKind / reasonPath — the last read succeeded, or why not
        // actionOutcome / actionPid / actionKind — the last terminate or kill
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QString, sort_field)]
        #[qproperty(bool, sort_ascending)]
        #[qproperty(QString, filter_text)]
        #[qproperty(bool, grouped)]
        #[qproperty(QVariant, process_pids)]
        #[qproperty(QStringList, process_names)]
        #[qproperty(QStringList, process_users)]
        #[qproperty(QVariant, process_cpu_percents)]
        #[qproperty(QVariant, process_memory_kib)]
        #[qproperty(QVariant, process_read_rates)]
        #[qproperty(QVariant, process_write_rates)]
        #[qproperty(QStringList, process_applications)]
        #[qproperty(QVariant, process_group_indices)]
        #[qproperty(QVariant, process_actionable)]
        #[qproperty(QStringList, group_ids)]
        #[qproperty(QStringList, group_names)]
        #[qproperty(QStringList, group_icons)]
        #[qproperty(QVariant, group_cpu_percents)]
        #[qproperty(QVariant, group_memory_kib)]
        #[qproperty(QVariant, group_counts)]
        #[qproperty(i32, total_count)]
        #[qproperty(i32, shown_count)]
        #[qproperty(bool, available)]
        #[qproperty(QString, reason_kind)]
        #[qproperty(QString, reason_path)]
        #[qproperty(QString, action_outcome)]
        #[qproperty(i32, action_pid)]
        #[qproperty(QString, action_kind)]
        #[qproperty(bool, start_failed)]
        type HematitaProcesses = super::HematitaProcessesRust;

        /// Subscribes to the shared sampler, once. The window calls it when
        /// it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaProcesses>);

        /// Re-projects the last snapshot with the current state. QML calls it
        /// after changing the sort, the filter or the grouping.
        #[qinvokable]
        fn refresh(self: Pin<&mut HematitaProcesses>);

        /// SIGTERM to one of the user's own processes.
        #[qinvokable]
        fn terminate(self: Pin<&mut HematitaProcesses>, pid: i32);

        /// SIGKILL to one of the user's own processes.
        #[qinvokable]
        fn kill(self: Pin<&mut HematitaProcesses>, pid: i32);
    }

    impl cxx_qt::Threading for HematitaProcesses {}
}

pub struct HematitaProcessesRust {
    revision: i32,
    sort_field: QString,
    sort_ascending: bool,
    filter_text: QString,
    grouped: bool,
    process_pids: QVariant,
    process_names: QStringList,
    process_users: QStringList,
    process_cpu_percents: QVariant,
    process_memory_kib: QVariant,
    process_read_rates: QVariant,
    process_write_rates: QVariant,
    process_applications: QStringList,
    process_group_indices: QVariant,
    process_actionable: QVariant,
    group_ids: QStringList,
    group_names: QStringList,
    group_icons: QStringList,
    group_cpu_percents: QVariant,
    group_memory_kib: QVariant,
    group_counts: QVariant,
    total_count: i32,
    shown_count: i32,
    available: bool,
    reason_kind: QString,
    reason_path: QString,
    action_outcome: QString,
    action_pid: i32,
    action_kind: QString,
    start_failed: bool,
    started: bool,
    last_generation: u64,
    latest: Option<Arc<ProcessSnapshot>>,
    own_uid: u32,
    /// Desktop id to its name and icon, resolved once per id from the XDG
    /// directories. `None` is a remembered absence, so a missing `.desktop`
    /// is looked for once and not once per tick.
    entries: HashMap<String, Option<(String, String)>>,
}

impl Default for HematitaProcessesRust {
    fn default() -> Self {
        Self {
            revision: 0,
            // The column a table of processes is read by, busiest first.
            sort_field: QString::from(SortField::Cpu.as_str()),
            sort_ascending: false,
            filter_text: QString::default(),
            grouped: false,
            process_pids: doubles(&[]),
            process_names: QStringList::default(),
            process_users: QStringList::default(),
            process_cpu_percents: doubles(&[]),
            process_memory_kib: doubles(&[]),
            process_read_rates: doubles(&[]),
            process_write_rates: doubles(&[]),
            process_applications: QStringList::default(),
            process_group_indices: doubles(&[]),
            process_actionable: doubles(&[]),
            group_ids: QStringList::default(),
            group_names: QStringList::default(),
            group_icons: QStringList::default(),
            group_cpu_percents: doubles(&[]),
            group_memory_kib: doubles(&[]),
            group_counts: doubles(&[]),
            total_count: 0,
            shown_count: 0,
            available: true,
            reason_kind: QString::default(),
            reason_path: QString::default(),
            action_outcome: QString::default(),
            action_pid: 0,
            action_kind: QString::default(),
            start_failed: false,
            started: false,
            last_generation: 0,
            latest: None,
            own_uid: 0,
            entries: HashMap::new(),
        }
    }
}

impl qobject::HematitaProcesses {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let outcome = sampler::subscribe(move |snapshot: &Snapshot| {
            let Some(section) = &snapshot.processes else {
                return;
            };
            let generation = snapshot.generation;
            let section = section.clone();
            let _ = qt.queue(move |processes: Pin<&mut qobject::HematitaProcesses>| {
                processes.apply(generation, section);
            });
        });
        if outcome.is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    /// Applies one process section whole. A section that arrived out of order
    /// is dropped rather than shown beside a newer one.
    fn apply(mut self: Pin<&mut Self>, generation: u64, section: Section<ProcessSnapshot>) {
        if generation <= self.rust().last_generation {
            return;
        }
        self.as_mut().rust_mut().last_generation = generation;
        match section {
            Section::Available(snapshot) => {
                let own_uid = snapshot.own_uid;
                self.as_mut().rust_mut().latest = Some(Arc::new(snapshot));
                self.as_mut().rust_mut().own_uid = own_uid;
                self.as_mut().set_available(true);
                self.as_mut().set_reason_kind(QString::default());
                self.as_mut().set_reason_path(QString::default());
                self.as_mut().refresh();
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().set_available(false);
                self.as_mut().set_reason_kind(QString::from(kind.as_str()));
                self.as_mut().set_reason_path(QString::from(path.as_str()));
            }
        }
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let Some(snapshot) = self.rust().latest.clone() else {
            return;
        };
        let field =
            SortField::from_token(&self.rust().sort_field.to_string()).unwrap_or(SortField::Cpu);
        let ascending = self.rust().sort_ascending;
        let filter = self.rust().filter_text.to_string();
        let grouped = self.rust().grouped;
        let own_uid = snapshot.own_uid;

        let rows: Vec<ProcessRow> = snapshot
            .readings
            .iter()
            .map(|reading| row_of(reading, own_uid))
            .collect();
        let order = process_view::project(&rows, &filter, field, ascending);
        let groups = if grouped {
            process_view::group(&rows, &order)
        } else {
            Vec::new()
        };

        // The group each shown row belongs to; -1 in the flat layout.
        let mut group_of: HashMap<usize, f64> = HashMap::new();
        for (group_index, group) in groups.iter().enumerate() {
            for &member in &group.member_indices {
                let index = i32::try_from(group_index).unwrap_or(-1);
                group_of.insert(member, f64::from(index));
            }
        }
        // The grouped layout lists members group by group; flat keeps `order`.
        let shown: Vec<usize> = if grouped {
            groups
                .iter()
                .flat_map(|group| group.member_indices.iter().copied())
                .collect()
        } else {
            order
        };

        let user_name = |uid: u32| {
            snapshot
                .users
                .get(&uid)
                .cloned()
                .unwrap_or_else(|| uid.to_string())
        };
        let pids = doubles(
            &shown
                .iter()
                .map(|&index| f64::from(rows[index].pid))
                .collect::<Vec<_>>(),
        );
        let cpu_percents = doubles(
            &shown
                .iter()
                .map(|&index| f64::from(rows[index].cpu_percent))
                .collect::<Vec<_>>(),
        );
        let memory = doubles(
            &shown
                .iter()
                .map(|&index| kib_as_f64(rows[index].memory_kib))
                .collect::<Vec<_>>(),
        );
        let reads = doubles(
            &shown
                .iter()
                .map(|&index| rows[index].read_rate)
                .collect::<Vec<_>>(),
        );
        let writes = doubles(
            &shown
                .iter()
                .map(|&index| rows[index].write_rate)
                .collect::<Vec<_>>(),
        );
        let group_indices = doubles(
            &shown
                .iter()
                .map(|&index| group_of.get(&index).copied().unwrap_or(-1.0))
                .collect::<Vec<_>>(),
        );
        let actionable = doubles(
            &shown
                .iter()
                .map(|&index| if rows[index].actionable { 1.0 } else { 0.0 })
                .collect::<Vec<_>>(),
        );
        let names = strings(shown.iter().map(|&index| rows[index].name.clone()));
        let users = strings(shown.iter().map(|&index| user_name(rows[index].uid)));
        let applications = strings(
            shown
                .iter()
                .map(|&index| rows[index].application.clone().unwrap_or_default()),
        );

        self.as_mut().set_process_pids(pids);
        self.as_mut().set_process_names(names);
        self.as_mut().set_process_users(users);
        self.as_mut().set_process_cpu_percents(cpu_percents);
        self.as_mut().set_process_memory_kib(memory);
        self.as_mut().set_process_read_rates(reads);
        self.as_mut().set_process_write_rates(writes);
        self.as_mut().set_process_applications(applications);
        self.as_mut().set_process_group_indices(group_indices);
        self.as_mut().set_process_actionable(actionable);

        let mut group_names = Vec::new();
        let mut group_icons = Vec::new();
        for group in &groups {
            let (name, icon) = self.as_mut().rust_mut().entry_for(&group.id);
            group_names.push(name);
            group_icons.push(icon);
        }
        let ids = strings(groups.iter().map(|group| group.id.clone()));
        let group_cpu = doubles(
            &groups
                .iter()
                .map(|group| f64::from(group.cpu_percent))
                .collect::<Vec<_>>(),
        );
        let group_memory = doubles(
            &groups
                .iter()
                .map(|group| kib_as_f64(group.memory_kib))
                .collect::<Vec<_>>(),
        );
        let group_counts = doubles(
            &groups
                .iter()
                .map(|group| count_as_f64(group.member_indices.len()))
                .collect::<Vec<_>>(),
        );
        self.as_mut().set_group_ids(ids);
        self.as_mut().set_group_names(strings(group_names));
        self.as_mut().set_group_icons(strings(group_icons));
        self.as_mut().set_group_cpu_percents(group_cpu);
        self.as_mut().set_group_memory_kib(group_memory);
        self.as_mut().set_group_counts(group_counts);
        self.as_mut()
            .set_total_count(i32::try_from(rows.len()).unwrap_or(i32::MAX));
        self.as_mut()
            .set_shown_count(i32::try_from(shown.len()).unwrap_or(i32::MAX));
        // Last, so the page rebuilds once, with every list in place.
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    pub fn terminate(self: Pin<&mut Self>, pid: i32) {
        self.send_signal(pid, rustix::process::Signal::TERM, "terminate");
    }

    pub fn kill(self: Pin<&mut Self>, pid: i32) {
        self.send_signal(pid, rustix::process::Signal::KILL, "kill");
    }

    /// The one signal path. A PID the latest snapshot does not show as ours
    /// is refused here, before any syscall.
    fn send_signal(
        mut self: Pin<&mut Self>,
        pid: i32,
        signal: rustix::process::Signal,
        kind: &str,
    ) {
        let outcome = match self.rust().owned_pid(pid) {
            None => "refused",
            Some(target) => match rustix::process::kill_process(target, signal) {
                Ok(()) => "done",
                Err(_) => "failed",
            },
        };
        self.as_mut().set_action_pid(pid);
        self.as_mut().set_action_kind(QString::from(kind));
        self.as_mut().set_action_outcome(QString::from(outcome));
    }
}

impl HematitaProcessesRust {
    /// The PID as a target, only if the latest snapshot shows it as ours and
    /// it is neither init nor this process.
    fn owned_pid(&self, pid: i32) -> Option<rustix::process::Pid> {
        let pid_u32 = u32::try_from(pid).ok()?;
        if pid_u32 <= 1 || pid_u32 == std::process::id() {
            return None;
        }
        let latest = self.latest.as_ref()?;
        let reading = latest
            .readings
            .iter()
            .find(|reading| reading.pid == pid_u32)?;
        (reading.uid == self.own_uid).then(|| rustix::process::Pid::from_raw(pid))?
    }

    /// The application's name and icon from its `.desktop` file, resolved
    /// once per id. A missing entry answers the id as the name and no icon.
    ///
    /// This reads files on the Qt thread: a handful of small ones, once per
    /// application id for the life of the window. It is the same trade
    /// Siderita's icon resolution makes.
    fn entry_for(&mut self, desktop_id: &str) -> (String, String) {
        if let Some(cached) = self.entries.get(desktop_id) {
            return cached
                .clone()
                .unwrap_or_else(|| (desktop_id.to_owned(), String::new()));
        }
        let file = format!("{desktop_id}.desktop");
        let found = desktop_entry::application_dirs()
            .into_iter()
            .find_map(|dir| {
                let text = std::fs::read_to_string(dir.join(&file)).ok()?;
                let entry = desktop_entry::parse(&file, &text)?;
                Some((entry.name, entry.icon))
            })
            .filter(|(name, _)| !name.is_empty());
        self.entries.insert(desktop_id.to_owned(), found.clone());
        found.unwrap_or_else(|| (desktop_id.to_owned(), String::new()))
    }
}

/// Kibibytes as the `double` QML reads. A `double` is exact to 2^53, which
/// is more kibibytes than any machine has.
fn kib_as_f64(kib: u64) -> f64 {
    kib as f64
}

fn count_as_f64(count: usize) -> f64 {
    count as f64
}

fn row_of(reading: &ProcessReading, own_uid: u32) -> ProcessRow {
    ProcessRow {
        pid: reading.pid,
        name: reading.name.clone(),
        uid: reading.uid,
        cpu_percent: reading.cpu_percent.unwrap_or(0.0),
        memory_kib: reading.memory_kib,
        read_rate: reading.io_rate.map_or(0.0, |[read, _]| read),
        write_rate: reading.io_rate.map_or(0.0, |[_, write]| write),
        application: reading.application.clone(),
        actionable: reading.uid == own_uid,
    }
}

#[cfg(test)]
mod tests {
    use super::{row_of, HematitaProcessesRust};
    use crate::sampler::{ProcessReading, ProcessSnapshot};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn reading(pid: u32, uid: u32) -> ProcessReading {
        ProcessReading {
            pid,
            name: "zsh".to_owned(),
            uid,
            cpu_percent: Some(1.5),
            memory_kib: 4736,
            io_rate: Some([10.0, 20.0]),
            application: Some("kitty".to_owned()),
        }
    }

    fn state(own_uid: u32, readings: Vec<ProcessReading>) -> HematitaProcessesRust {
        HematitaProcessesRust {
            own_uid,
            latest: Some(Arc::new(ProcessSnapshot {
                own_uid,
                readings,
                users: Arc::new(HashMap::new()),
            })),
            ..HematitaProcessesRust::default()
        }
    }

    #[test]
    fn a_row_is_actionable_only_when_it_is_the_users_own() {
        let mine = row_of(&reading(42, 1000), 1000);
        assert!(mine.actionable);
        assert_eq!(mine.cpu_percent, 1.5);
        assert_eq!(mine.read_rate, 10.0);
        assert_eq!(mine.write_rate, 20.0);
        assert!(!row_of(&reading(42, 0), 1000).actionable);
    }

    #[test]
    fn a_reading_without_rates_yet_reads_as_zero() {
        let mut waiting = reading(42, 1000);
        waiting.cpu_percent = None;
        waiting.io_rate = None;
        let row = row_of(&waiting, 1000);
        assert_eq!(row.cpu_percent, 0.0);
        assert_eq!(row.read_rate, 0.0);
        assert_eq!(row.write_rate, 0.0);
    }

    #[test]
    fn init_this_process_and_another_users_process_are_never_targets() {
        let own = std::process::id();
        let readings = vec![
            reading(0, 1000),
            reading(1, 1000),
            reading(own, 1000),
            reading(4242, 0),
            reading(4243, 1000),
        ];
        let state = state(1000, readings);
        assert!(state.owned_pid(0).is_none());
        assert!(state.owned_pid(1).is_none());
        assert!(state
            .owned_pid(i32::try_from(own).expect("a pid fits an i32"))
            .is_none());
        assert!(state.owned_pid(-7).is_none());
        // Another user's process is refused even though it is listed.
        assert!(state.owned_pid(4242).is_none());
        // A PID no snapshot lists is refused.
        assert!(state.owned_pid(9_999_999).is_none());
        assert!(state.owned_pid(4243).is_some());
    }

    #[test]
    fn without_a_snapshot_nothing_is_a_target() {
        let empty = HematitaProcessesRust::default();
        assert!(empty.owned_pid(4243).is_none());
    }
}
