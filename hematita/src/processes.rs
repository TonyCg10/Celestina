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
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use celestina_core::desktop_entry;
use hematita_core::process;
use hematita_core::process_view::{self, ProcessRow, SortField};
use hematita_core::services::Outcome;

use crate::lists::{doubles, strings};
use crate::privilege;
use crate::publish::{self, PENDING};
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

        /// SIGTERM: directly to one of the user's own processes, or through
        /// `pkexec kill` to somebody else's.
        #[qinvokable]
        fn terminate(self: Pin<&mut HematitaProcesses>, pid: i32);

        /// SIGKILL: directly to one of the user's own processes, or through
        /// `pkexec kill` to somebody else's.
        #[qinvokable]
        fn kill(self: Pin<&mut HematitaProcesses>, pid: i32);

        /// Forgets the last action's outcome. The page calls it when the
        /// selection moves, because an answer about one process is not an
        /// answer about the next. Nothing else clears it — a reading of the
        /// machine two seconds later is not a reason to stop saying that a
        /// signal was refused.
        #[qinvokable]
        fn clear_action(self: Pin<&mut HematitaProcesses>);
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
    /// The token of the action being waited for. Every action takes the next
    /// one, so an outcome that comes back under an older token is dropped.
    action_token: u64,
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
            action_token: 0,
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

    pub fn clear_action(mut self: Pin<&mut Self>) {
        // Forgetting the question also retires its token: an answer that
        // arrives after the person moved on is not written back.
        let token = self.rust().action_token.wrapping_add(1);
        self.as_mut().rust_mut().action_token = token;
        self.as_mut().set_action_outcome(QString::default());
        self.as_mut().set_action_kind(QString::default());
        self.as_mut().set_action_pid(0);
    }

    pub fn terminate(self: Pin<&mut Self>, pid: i32) {
        self.send_signal(
            pid,
            rustix::process::Signal::TERM,
            privilege::Signal::Terminate,
            "terminate",
        );
    }

    pub fn kill(self: Pin<&mut Self>, pid: i32) {
        self.send_signal(
            pid,
            rustix::process::Signal::KILL,
            privilege::Signal::Kill,
            "kill",
        );
    }

    /// The one signal path. A PID that is neither a process the table showed
    /// nor a process at all is refused here, before any syscall — and then
    /// `/proc` is asked again, because the snapshot is up to two seconds old
    /// and a PID that died in that gap may already belong to somebody else's
    /// new process. Only a PID whose start time and owner still match the ones
    /// the table showed is signalled.
    ///
    /// The user's own process is signalled directly. Somebody else's is asked
    /// for through `pkexec kill` on a worker thread, which is the whole of
    /// Hematita's privilege (ADR 0010): the outcome is `pending` until polkit
    /// and the person have answered, and whatever they answer is queued back
    /// here. `init`, this process and a PID the table never showed stay
    /// `refused`.
    fn send_signal(
        mut self: Pin<&mut Self>,
        pid: i32,
        signal: rustix::process::Signal,
        as_root: privilege::Signal,
        kind: &str,
    ) {
        let expected = self.rust().expected_identity(pid);
        let target = self.rust().owned_pid(pid);
        let validated =
            expected.is_some_and(|expected| still_the_same(&expected, &read_identity_now(pid)));
        // The three properties are written together, so the page never reads
        // this action's outcome beside the last one's pid.
        self.as_mut().set_action_pid(pid);
        self.as_mut().set_action_kind(QString::from(kind));
        let token = self.rust().action_token.wrapping_add(1);
        self.as_mut().rust_mut().action_token = token;
        let outcome = match target {
            Ok(target) if validated => match rustix::process::kill_process(target, signal) {
                Ok(()) => Outcome::Done.as_str(),
                Err(_) => Outcome::Failed.as_str(),
            },
            Err(Refusal::Foreign { .. }) if validated => {
                let qt = self.as_mut().qt_thread();
                let asked = u32::try_from(pid).ok().is_some_and(|pid| {
                    privilege::signal_as_root(pid, as_root, move |outcome| {
                        let _ =
                            qt.queue(move |mut processes: Pin<&mut qobject::HematitaProcesses>| {
                                // A prompt can stand for minutes; by the time
                                // it is answered the person may have asked for
                                // something else, and that newer question is
                                // the one the page is showing.
                                if publish::still_current(token, processes.rust().action_token) {
                                    processes
                                        .as_mut()
                                        .set_action_outcome(QString::from(outcome.as_str()));
                                }
                            });
                    })
                    .is_ok()
                });
                if asked {
                    PENDING
                } else {
                    Outcome::Failed.as_str()
                }
            }
            _ => Outcome::Refused.as_str(),
        };
        self.as_mut().set_action_outcome(QString::from(outcome));
    }
}

/// Why a PID is not a direct target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    /// It is a listed process of another user: the kernel refuses the signal,
    /// and `pkexec kill` is the sanctioned way to ask for it.
    Foreign { start_ticks: u64 },
    /// `init`, this very process, a PID no snapshot listed, or a number that
    /// is not a PID at all. Hematita does not ask for any of these.
    NotAllowed,
}

impl HematitaProcessesRust {
    /// The PID as a direct target, or why it is not one: another user's
    /// process is `Foreign` and can still be asked for through polkit, while
    /// `init`, this process and a PID the table never showed are refused
    /// outright.
    fn owned_pid(&self, pid: i32) -> Result<rustix::process::Pid, Refusal> {
        let Ok(pid_u32) = u32::try_from(pid) else {
            return Err(Refusal::NotAllowed);
        };
        if pid_u32 <= 1 || pid_u32 == std::process::id() {
            return Err(Refusal::NotAllowed);
        }
        let Some(latest) = self.latest.as_ref() else {
            return Err(Refusal::NotAllowed);
        };
        let Some(reading) = latest
            .readings
            .iter()
            .find(|reading| reading.pid == pid_u32)
        else {
            return Err(Refusal::NotAllowed);
        };
        if reading.uid != self.own_uid {
            return Err(Refusal::Foreign {
                start_ticks: reading.start_ticks,
            });
        }
        rustix::process::Pid::from_raw(pid).ok_or(Refusal::NotAllowed)
    }

    /// What the latest snapshot says this PID is, for the signal path to
    /// check `/proc` against.
    fn expected_identity(&self, pid: i32) -> Option<ProcessIdentity> {
        let pid_u32 = u32::try_from(pid).ok()?;
        let latest = self.latest.as_ref()?;
        latest
            .readings
            .iter()
            .find(|reading| reading.pid == pid_u32)
            .map(|reading| ProcessIdentity {
                start_ticks: reading.start_ticks,
                uid: reading.uid,
            })
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
/// Who a PID is: when it started, and whose it is. A PID number alone is not
/// an identity, because the kernel reuses numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProcessIdentity {
    start_ticks: u64,
    uid: u32,
}

/// Whether the PID `/proc` describes now is the one the snapshot described.
/// `None` on the right means `/proc` could not answer — the process is gone,
/// or unreadable — which is never the same process.
fn still_the_same(expected: &ProcessIdentity, current: &Option<ProcessIdentity>) -> bool {
    current.as_ref() == Some(expected)
}

/// Reads one PID's start time and owner from `/proc`, now.
///
/// This is blocking IO on the Qt thread: two small files, once per signal a
/// person asked for. It is the only way to close the gap between a snapshot
/// up to two seconds old and the syscall, and it is accepted for the same
/// reason the `.desktop` read is — it happens on a human action, not on a
/// tick.
fn read_identity_now(pid: i32) -> Option<ProcessIdentity> {
    let pid_u32 = u32::try_from(pid).ok()?;
    let directory = Path::new("/proc").join(pid_u32.to_string());
    let stat_text = std::fs::read_to_string(directory.join("stat")).ok()?;
    let stat = process::parse_stat(&stat_text).ok()?;
    let status_text = std::fs::read_to_string(directory.join("status")).ok()?;
    let status = process::parse_status(&status_text).ok()?;
    // `/proc/<pid>/stat` names the pid it describes; if it does not name the
    // one that was asked for, this is not the file it was meant to be.
    (stat.pid == pid_u32).then_some(ProcessIdentity {
        start_ticks: stat.start_ticks,
        uid: status.uid,
    })
}

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
    use super::{row_of, still_the_same, HematitaProcessesRust, ProcessIdentity, Refusal};
    use crate::sampler::{ProcessReading, ProcessSnapshot};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn reading(pid: u32, uid: u32) -> ProcessReading {
        ProcessReading {
            pid,
            start_ticks: 67_467_262,
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
    fn a_pid_is_classified_as_a_target_a_foreign_process_or_not_allowed() {
        let own = std::process::id();
        let readings = vec![
            reading(0, 1000),
            reading(1, 1000),
            reading(own, 1000),
            reading(4242, 0),
            reading(4243, 1000),
        ];
        let state = state(1000, readings);
        // `init`, the kernel's pid 0 and Hematita itself are never asked for.
        assert_eq!(state.owned_pid(0), Err(Refusal::NotAllowed));
        assert_eq!(state.owned_pid(1), Err(Refusal::NotAllowed));
        assert_eq!(
            state.owned_pid(i32::try_from(own).expect("a pid fits an i32")),
            Err(Refusal::NotAllowed)
        );
        // A number that is not a PID, and a PID no snapshot listed.
        assert_eq!(state.owned_pid(-7), Err(Refusal::NotAllowed));
        assert_eq!(state.owned_pid(9_999_999), Err(Refusal::NotAllowed));
        // Another user's listed process is the one refusal polkit can answer,
        // and it carries the identity the signal path re-checks.
        assert_eq!(
            state.owned_pid(4242),
            Err(Refusal::Foreign {
                start_ticks: 67_467_262
            })
        );
        assert!(state.owned_pid(4243).is_ok());
    }

    #[test]
    fn a_pid_is_the_same_process_only_with_the_same_start_time_and_owner() {
        let expected = ProcessIdentity {
            start_ticks: 67_467_262,
            uid: 1000,
        };
        assert!(still_the_same(&expected, &Some(expected)));
        // The PID was recycled between the snapshot and the signal.
        assert!(!still_the_same(
            &expected,
            &Some(ProcessIdentity {
                start_ticks: 99_000_000,
                uid: 1000,
            })
        ));
        // The same start time but another owner cannot happen through reuse;
        // it is refused anyway, because it is not what the table showed.
        assert!(!still_the_same(
            &expected,
            &Some(ProcessIdentity {
                start_ticks: 67_467_262,
                uid: 0,
            })
        ));
        // `/proc` had no answer: the process is gone, which is never the
        // same process.
        assert!(!still_the_same(&expected, &None));
    }

    #[test]
    fn without_a_snapshot_nothing_is_a_target() {
        let empty = HematitaProcessesRust::default();
        assert_eq!(empty.owned_pid(4243), Err(Refusal::NotAllowed));
    }
}
