//! language-contract: product-copy
//!
//! The running write operations, and the one surface that shows them.
//!
//! Until now a write verb *claimed* the application: one progress bar, one
//! Cancel button, one `op_running` flag, and every other verb refused while it
//! was set. That is why a paste could not start while a trash was running — and
//! why one stuck extraction froze every write in Siderita.
//!
//! What replaces it is a register of jobs. Each has its own identity, its own
//! cancellation token and its own counters, so several run at once and each is
//! cancelled on its own.
//!
//! The register belongs to the **process**, not to a tab. A copy started in one
//! tab is still running when a person switches to another, and a surface that
//! only knew about its own tab's work would tell them it had finished. Every
//! controller therefore publishes the same list, and a change wakes all of them
//! through the Qt threads they registered on start.
//!
//! Each job can also be held. Pausing rides on the cancellation token because
//! the points a long operation asks "should I stop?" are exactly the points at
//! which it is safe to wait — see `celestina_core::CancellationToken::pause`.
//!
//! The marker above declares the Spanish here: a job's label is the line a
//! person reads while it runs.

use core::pin::Pin;
use std::sync::{Mutex, OnceLock};

use celestina_core::CancellationToken;
use cxx_qt::Threading;
use cxx_qt_lib::QString;

use super::qobject;

/// The process-wide register: every running job, and every controller that
/// wants to be told about them.
struct Registry {
    jobs: Vec<Job>,
    next_id: u64,
    listeners: Vec<cxx_qt::CxxQtThread<qobject::SideritaController>>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            jobs: Vec::new(),
            next_id: 0,
            listeners: Vec::new(),
        })
    })
}

/// Asks every controller to publish the register again.
///
/// A queue that fails names a controller whose window is gone, so it is dropped
/// here rather than accumulating for the life of the process.
fn wake_listeners() {
    let Ok(mut state) = registry().lock() else {
        return;
    };
    state.listeners.retain(|listener| {
        listener
            .queue(|controller| controller.publish_jobs())
            .is_ok()
    });
}

/// What a job is doing, as a stable token rather than as its Spanish label: the
/// surface picks an icon from this, and a translated word must never decide
/// which glyph a person sees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JobKind {
    Copy,
    Move,
    Trash,
    Compress,
    Extract,
}

impl JobKind {
    /// The icon name the catalogue resolves for this action.
    fn icon(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Move => "scissors",
            Self::Trash => "user-trash",
            Self::Compress => "archive-compress",
            Self::Extract => "archive-extract",
        }
    }
}

/// One job as the surface reads it: every column already worded, so QML works
/// nothing out on its own.
struct Row {
    id: String,
    label: String,
    icon: String,
    current: String,
    detail: String,
    percent: String,
    steps: String,
    paused: String,
}

/// One running write operation.
pub(crate) struct Job {
    /// Identity that survives reordering, so a Cancel button keeps naming the
    /// same work as jobs come and go.
    pub(crate) id: u64,
    /// What it is doing, in the words a person reads: `Copiando`, `Extrayendo…`.
    pub(crate) label: String,
    /// The same, as the token the surface draws an icon from.
    pub(crate) kind: JobKind,
    /// The entry it reached.
    pub(crate) current: String,
    /// The throttled byte read-out.
    pub(crate) detail: String,
    /// Entries finished and entries asked for.
    pub(crate) done: i32,
    pub(crate) total: i32,
    /// Bytes moved, and how many there are in total when that can be known.
    ///
    /// Knowing the total is what turns a ring that merely turns into one that
    /// fills, and a byte count into "so much of so much". It is asked of the
    /// archive before the work starts; zero means it could not be known.
    pub(crate) bytes_done: u64,
    pub(crate) bytes_total: u64,
    /// How many times this job has reported progress.
    ///
    /// It is what turns the ring of a job whose end cannot be known. The turn
    /// deliberately comes from the *data* rather than from a QML animation: an
    /// animation moves the render node, and on the author's machine a shape
    /// moved that way never repainted — the ring sat still through two attempts
    /// at fixing it. A step that arrives with each progress report changes the
    /// arc's geometry, which cannot be ignored, and has the honest side effect
    /// that a job which stops reporting stops turning.
    pub(crate) steps: i32,
    /// Its own token: cancelling one job never touches another.
    pub(crate) cancel: CancellationToken,
}

impl qobject::SideritaController {
    /// Subscribes this controller to the process-wide register, so work started
    /// anywhere reaches its surface too.
    pub(crate) fn watch_jobs(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        if let Ok(mut state) = registry().lock() {
            state.listeners.push(thread);
        }
        self.publish_jobs();
    }

    /// Registers a new job and hands back its id and cancellation token.
    ///
    /// Nothing is refused here. Two jobs writing into the same folder are safe
    /// because the domain reserves each destination name atomically
    /// (`siderita_ops`' no-replace rename), so the loser of a race is told the
    /// name is taken instead of overwriting the winner.
    pub(crate) fn start_job(
        mut self: Pin<&mut Self>,
        label: &str,
        kind: JobKind,
        total: usize,
    ) -> (u64, CancellationToken) {
        let token = CancellationToken::new();
        let id = {
            let Ok(mut state) = registry().lock() else {
                return (0, token);
            };
            state.next_id += 1;
            let id = state.next_id;
            state.jobs.push(Job {
                id,
                label: label.to_owned(),
                kind,
                current: String::new(),
                detail: String::new(),
                done: 0,
                total: total.min(i32::MAX as usize) as i32,
                bytes_done: 0,
                bytes_total: 0,
                steps: 0,
                cancel: token.clone(),
            });
            id
        };
        self.as_mut().set_status_text(QString::from(label));
        wake_listeners();
        (id, token)
    }

    /// Records how far a job has got. A job that has already ended is ignored,
    /// which is what makes a late progress message from a finished worker
    /// harmless.
    pub(crate) fn job_reached(
        self: Pin<&mut Self>,
        id: u64,
        done: i32,
        current: Option<String>,
        detail: Option<String>,
    ) {
        {
            let Ok(mut state) = registry().lock() else {
                return;
            };
            let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) else {
                return;
            };
            job.done = done;
            job.steps = job.steps.wrapping_add(1).max(0);
            if let Some(current) = current {
                job.current = current;
            }
            if let Some(detail) = detail {
                job.detail = detail;
            }
        }
        wake_listeners();
    }

    /// Records how many bytes a job has moved, and how many it will move in
    /// all. A total of zero means it is not known, which is what a ring reads as
    /// "turn, do not fill".
    /// `detail` is the same report in the words a person reads, so one message
    /// carries both and the entry counter is left alone — it counts entries, and
    /// bytes are not entries.
    pub(crate) fn job_weighed(
        self: Pin<&mut Self>,
        id: u64,
        done: u64,
        total: u64,
        detail: String,
    ) {
        {
            let Ok(mut state) = registry().lock() else {
                return;
            };
            let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) else {
                return;
            };
            job.bytes_done = done;
            job.bytes_total = total;
            job.detail = detail;
            job.steps = job.steps.wrapping_add(1).max(0);
        }
        wake_listeners();
    }

    /// Removes a finished job from the register.
    pub(crate) fn end_job(self: Pin<&mut Self>, id: u64) {
        if let Ok(mut state) = registry().lock() {
            state.jobs.retain(|job| job.id != id);
        }
        wake_listeners();
        self.publish_jobs();
    }

    /// Cancels one job by id, leaving every other one running.
    pub fn cancel_job(self: Pin<&mut Self>, id: f64) {
        let id = id as u64;
        if let Ok(state) = registry().lock() {
            if let Some(job) = state.jobs.iter().find(|job| job.id == id) {
                job.cancel.cancel();
            }
        }
        wake_listeners();
        self.publish_jobs();
    }

    /// Holds one job where it is, or lets it carry on. The same button, because
    /// to a person it is one state with two faces.
    pub fn toggle_job_paused(self: Pin<&mut Self>, id: f64) {
        let id = id as u64;
        if let Ok(state) = registry().lock() {
            if let Some(job) = state.jobs.iter().find(|job| job.id == id) {
                if job.cancel.is_paused() {
                    job.cancel.resume();
                } else {
                    job.cancel.pause();
                }
            }
        }
        wake_listeners();
        self.publish_jobs();
    }

    /// Cancels every running job: the Cancel that belongs to the whole surface
    /// rather than to one row.
    pub fn cancel_all_jobs(self: Pin<&mut Self>) {
        if let Ok(state) = registry().lock() {
            for job in &state.jobs {
                job.cancel.cancel();
            }
        }
        wake_listeners();
        self.publish_jobs();
    }

    /// The cancellation token of a job that is already registered — for work
    /// that pauses for a person's answer and then resumes as the same job.
    pub(crate) fn job_token(&self, id: u64) -> CancellationToken {
        registry()
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .jobs
                    .iter()
                    .find(|job| job.id == id)
                    .map(|job| job.cancel.clone())
            })
            .unwrap_or_default()
    }

    /// Publishes the register onto the properties QML reads.
    ///
    /// One list per column, all of them the same length: the surface draws a row
    /// per job from them and works nothing out on its own. `op_running` is the
    /// single scalar left — "is anything writing" is still a question other
    /// parts of the application ask.
    pub(crate) fn publish_jobs(mut self: Pin<&mut Self>) {
        let running = registry()
            .lock()
            .map(|state| !state.jobs.is_empty())
            .unwrap_or(false);
        self.as_mut().set_op_running(running);
        self.publish_job_rows();
    }

    /// The per-job rows, as the parallel lists the operations surface consumes.
    fn publish_job_rows(mut self: Pin<&mut Self>) {
        // Named fields rather than an eight-wide tuple: the columns are only
        // parallel lists once they cross into QML, and until then they deserve
        // to be readable.
        let Ok(state) = registry().lock() else {
            return;
        };
        let rows: Vec<Row> = state
            .jobs
            .iter()
            .map(|job| Row {
                id: job.id.to_string(),
                label: job.label.clone(),
                icon: job.kind.icon().to_owned(),
                current: job.current.clone(),
                // What the row shows on its right: the byte read-out, or the
                // count when there is more than one entry to get through.
                detail: if job.detail.is_empty() && job.total > 1 {
                    format!("{} de {}", job.done, job.total)
                } else {
                    job.detail.clone()
                },
                // How full its ring is, as hundredths — or `-1` when there is no
                // fraction to show. Bytes first, because they are the finest
                // measure available and the only one that moves inside a single
                // 26 GB member; then entries, for a batch of several; and only
                // then "unknown", which is what a lone archive of unmeasurable
                // size gets.
                percent: if let Some(share) =
                    (100 * job.bytes_done.min(job.bytes_total)).checked_div(job.bytes_total)
                {
                    share.to_string()
                } else if job.total > 1 {
                    (100 * i64::from(job.done.min(job.total)) / i64::from(job.total)).to_string()
                } else {
                    "-1".to_owned()
                },
                steps: job.steps.to_string(),
                // Held or running: the surface shows one button with two faces,
                // and the truth of which face is the token's.
                paused: if job.cancel.is_paused() { "1" } else { "0" }.to_owned(),
            })
            .collect();
        drop(state);
        let mut ids = cxx_qt_lib::QStringList::default();
        let mut labels = cxx_qt_lib::QStringList::default();
        let mut currents = cxx_qt_lib::QStringList::default();
        let mut details = cxx_qt_lib::QStringList::default();
        let mut percents = cxx_qt_lib::QStringList::default();
        let mut icons = cxx_qt_lib::QStringList::default();
        let mut steps = cxx_qt_lib::QStringList::default();
        let mut held = cxx_qt_lib::QStringList::default();
        for row in &rows {
            ids.append(QString::from(row.id.as_str()));
            labels.append(QString::from(row.label.as_str()));
            icons.append(QString::from(row.icon.as_str()));
            currents.append(QString::from(row.current.as_str()));
            details.append(QString::from(row.detail.as_str()));
            percents.append(QString::from(row.percent.as_str()));
            steps.append(QString::from(row.steps.as_str()));
            held.append(QString::from(row.paused.as_str()));
        }
        self.as_mut().set_op_ids(ids);
        self.as_mut().set_op_labels(labels);
        self.as_mut().set_op_currents(currents);
        self.as_mut().set_op_details(details);
        self.as_mut().set_op_percents(percents);
        self.as_mut().set_op_icons(icons);
        self.as_mut().set_op_steps(steps);
        self.as_mut().set_op_paused(held);
    }
}
