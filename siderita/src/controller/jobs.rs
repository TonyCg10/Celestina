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
//! cancelled on its own. The scalar properties the current surface reads are
//! kept, published from the register: `op_running` is now "at least one job is
//! alive" and the rest describe the job at the front. `op_count` says how many
//! there are, which is what lets the surface say more than one is running
//! without redrawing itself.
//!
//! The marker above declares the Spanish here: a job's label is the line a
//! person reads while it runs.

use core::pin::Pin;

use celestina_core::CancellationToken;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use super::qobject;

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
            let state = self.as_mut().rust_mut().get_mut();
            state.next_job_id += 1;
            let id = state.next_job_id;
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
        self.publish_jobs();
        (id, token)
    }

    /// Records how far a job has got. A job that has already ended is ignored,
    /// which is what makes a late progress message from a finished worker
    /// harmless.
    pub(crate) fn job_reached(
        mut self: Pin<&mut Self>,
        id: u64,
        done: i32,
        current: Option<String>,
        detail: Option<String>,
    ) {
        {
            let state = self.as_mut().rust_mut().get_mut();
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
        self.publish_jobs();
    }

    /// Records how many bytes a job has moved, and how many it will move in
    /// all. A total of zero means it is not known, which is what a ring reads as
    /// "turn, do not fill".
    /// `detail` is the same report in the words a person reads, so one message
    /// carries both and the entry counter is left alone — it counts entries, and
    /// bytes are not entries.
    pub(crate) fn job_weighed(
        mut self: Pin<&mut Self>,
        id: u64,
        done: u64,
        total: u64,
        detail: String,
    ) {
        {
            let state = self.as_mut().rust_mut().get_mut();
            let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) else {
                return;
            };
            job.bytes_done = done;
            job.bytes_total = total;
            job.detail = detail;
            job.steps = job.steps.wrapping_add(1).max(0);
        }
        self.publish_jobs();
    }

    /// Removes a finished job from the register.
    pub(crate) fn end_job(mut self: Pin<&mut Self>, id: u64) {
        self.as_mut()
            .rust_mut()
            .get_mut()
            .jobs
            .retain(|job| job.id != id);
        self.publish_jobs();
    }

    /// Cancels one job by id, leaving every other one running.
    pub fn cancel_job(mut self: Pin<&mut Self>, id: f64) {
        let id = id as u64;
        if let Some(job) = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .jobs
            .iter()
            .find(|job| job.id == id)
        {
            job.cancel.cancel();
        }
        self.publish_jobs();
    }

    /// Cancels every running job: the Cancel that belongs to the whole surface
    /// rather than to one row.
    pub fn cancel_all_jobs(mut self: Pin<&mut Self>) {
        for job in &self.as_mut().rust_mut().get_mut().jobs {
            job.cancel.cancel();
        }
        self.publish_jobs();
    }

    /// The cancellation token of a job that is already registered — for work
    /// that pauses for a person's answer and then resumes as the same job.
    pub(crate) fn job_token(&self, id: u64) -> CancellationToken {
        self.rust()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .map(|job| job.cancel.clone())
            .unwrap_or_default()
    }

    /// Publishes the register onto the properties QML reads.
    ///
    /// One list per column, all of them the same length: the surface draws a row
    /// per job from them and works nothing out on its own. `op_running` is the
    /// single scalar left — "is anything writing" is still a question other
    /// parts of the application ask.
    pub(crate) fn publish_jobs(mut self: Pin<&mut Self>) {
        let running = !self.rust().jobs.is_empty();
        self.as_mut().set_op_running(running);
        self.publish_job_rows();
    }

    /// The per-job rows, as the parallel lists the operations surface consumes.
    fn publish_job_rows(mut self: Pin<&mut Self>) {
        let rows: Vec<(String, String, String, String, String, String, String)> = self
            .rust()
            .jobs
            .iter()
            .map(|job| {
                (
                    job.id.to_string(),
                    job.label.clone(),
                    job.kind.icon().to_owned(),
                    job.current.clone(),
                    // What the row shows on its right: the byte read-out, or the
                    // count when there is more than one entry to get through.
                    if job.detail.is_empty() && job.total > 1 {
                        format!("{} de {}", job.done, job.total)
                    } else {
                        job.detail.clone()
                    },
                    // How full its ring is, as hundredths — or `-1` when there
                    // is no fraction to show. Bytes first, because they are the
                    // finest measure available and the only one that moves
                    // inside a single 26 GB member; then entries, for a batch of
                    // several; and only then "unknown", which is what a lone
                    // archive of unmeasurable size gets.
                    if let Some(share) =
                        (100 * job.bytes_done.min(job.bytes_total)).checked_div(job.bytes_total)
                    {
                        share.to_string()
                    } else if job.total > 1 {
                        (100 * i64::from(job.done.min(job.total)) / i64::from(job.total))
                            .to_string()
                    } else {
                        "-1".to_owned()
                    },
                    job.steps.to_string(),
                )
            })
            .collect();
        let mut ids = cxx_qt_lib::QStringList::default();
        let mut labels = cxx_qt_lib::QStringList::default();
        let mut currents = cxx_qt_lib::QStringList::default();
        let mut details = cxx_qt_lib::QStringList::default();
        let mut percents = cxx_qt_lib::QStringList::default();
        let mut icons = cxx_qt_lib::QStringList::default();
        let mut steps = cxx_qt_lib::QStringList::default();
        for (id, label, icon, current, detail, percent, step) in &rows {
            ids.append(QString::from(id.as_str()));
            labels.append(QString::from(label.as_str()));
            icons.append(QString::from(icon.as_str()));
            currents.append(QString::from(current.as_str()));
            details.append(QString::from(detail.as_str()));
            percents.append(QString::from(percent.as_str()));
            steps.append(QString::from(step.as_str()));
        }
        self.as_mut().set_op_ids(ids);
        self.as_mut().set_op_labels(labels);
        self.as_mut().set_op_currents(currents);
        self.as_mut().set_op_details(details);
        self.as_mut().set_op_percents(percents);
        self.as_mut().set_op_icons(icons);
        self.as_mut().set_op_steps(steps);
    }
}
