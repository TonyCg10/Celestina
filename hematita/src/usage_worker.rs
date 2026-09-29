//! The storage section's long readers: the scan, the content check of
//! duplicate candidates and the graft scan of what a stopped deletion left.
//!
//! Each runs on its own named thread (`hematita-scan`, `hematita-confirm`,
//! `hematita-graft`)
//! and holds a `CancellationToken` whose other half lives in the handle the
//! hub keeps. Dropping the handle cancels: a new scan replaces the old handle
//! and so stops the old walk within a few hundred entries.
//!
//! The threads are detached by design, not joined. Joining would block the
//! Qt thread for as long as the walk takes to reach its next cancellation
//! check. A detached thread holds nothing the hub owns: the walks own their
//! root path (which the core resolves there, on the worker, never on the Qt
//! thread), the check a `Weak<Tree>` it upgrades just long enough to copy
//! one group's paths, and each a `CxxQtThread`
//! whose `queue` drops a result once the hub is gone. Every result carries
//! the generation it was asked under; the hub drops one that is no longer
//! current, so a cancelled worker finishing late changes nothing.
//!
//! Blocking IO, `/proc/self/mountinfo` included, happens only on these
//! threads and on the actions thread, which reads the mount table through
//! [`mount_boundaries`].

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Weak;
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;
use cxx_qt::CxxQtThread;
use hematita_core::usage::duplicates::{self, ConfirmError, Group};
use hematita_core::usage::mounts::{mount_targets, parse_mountinfo};
use hematita_core::usage::tree::{NodeId, Tree};
use hematita_core::usage::walk::{self, Progress};

use crate::analysis::qobject::HematitaAnalysis;
use crate::analysis_view::{findings, Findings};

/// The least time between two progress publications of one scan.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

const MOUNTINFO: &str = "/proc/self/mountinfo";

/// A running worker; dropping it cancels the work.
pub struct WorkerHandle {
    cancel: CancellationToken,
}

impl WorkerHandle {
    /// A handle and the token the worker it guards checks.
    #[must_use]
    pub fn pair() -> (Self, CancellationToken) {
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        (Self { cancel }, token)
    }

    /// Asks the worker to stop while the handle is kept, so its report of
    /// what it already did still lands.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// The mount targets the walk and the deletion stop at. Without the table
/// both still stop at other devices; only same-device mounts (bind mounts,
/// subvolumes) would go unrecognised.
pub fn mount_boundaries() -> HashSet<PathBuf> {
    std::fs::read_to_string(MOUNTINFO)
        .map(|text| mount_targets(&parse_mountinfo(&text)))
        .unwrap_or_default()
}

/// Whether a progress report is due, `interval` after the last one sent.
fn due(last: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= interval)
}

/// Scans `root` on the `hematita-scan` thread, queueing progress at most
/// every [`PROGRESS_INTERVAL`] and the tree with its findings (or why there
/// is none) at the end.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_scan(
    root: PathBuf,
    generation: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<WorkerHandle, io::Error> {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    std::thread::Builder::new()
        .name("hematita-scan".to_owned())
        .spawn(move || {
            let boundaries = mount_boundaries();
            let mut last: Option<Instant> = None;
            let mut report = |progress: Progress| {
                let now = Instant::now();
                if !due(last, now, PROGRESS_INTERVAL) {
                    return;
                }
                last = Some(now);
                let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                    hub.apply_progress(generation, &progress);
                });
            };
            // The findings walk the whole arena once; here, not on Qt's thread.
            let scanned = walk::scan(&root, &boundaries, &token, &mut report).map(|tree| {
                let found = findings(&tree);
                (tree, found)
            });
            let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                hub.apply_tree(generation, scanned);
            });
        })?;
    Ok(WorkerHandle { cancel })
}

/// Scans `root` again on the `hematita-graft` thread, for the folder `id` a
/// deletion stopped inside, bounded by the mount table read here, and queues the fresh subtree with its findings
/// (or why there is none); the hub grafts it in place of the stale one.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_graft(
    root: PathBuf,
    id: NodeId,
    generation: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<WorkerHandle, io::Error> {
    let (handle, token) = WorkerHandle::pair();
    std::thread::Builder::new()
        .name("hematita-graft".to_owned())
        .spawn(move || {
            let boundaries = mount_boundaries();
            let scanned: Result<(Tree, Findings), _> =
                walk::scan_subtree(&root, &boundaries, &token).map(|tree| {
                    let found = findings(&tree);
                    (tree, found)
                });
            let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                hub.apply_graft(generation, id, scanned);
            });
        })?;
    Ok(handle)
}

/// Checks `groups` — each with its index among the hub's candidates — by
/// content on the `hematita-confirm` thread, one group at a time, so the page
/// fills as each verdict lands; then says it is done.
/// A group with a file that cannot be read keeps its unverified row and is
/// reported as unreadable. Every result carries `epoch`, the hub's confirm
/// epoch when the check started: a cancel, a restart or a pruning moves it,
/// so a verdict about an older tree, or a finish from a cancelled check,
/// never lands.
///
/// The check holds the tree weakly and upgrades it once per group only to
/// copy that group's paths and recorded identities, dropping the strong
/// reference before it reads a byte, so a pruning or a graft on the Qt
/// thread finds the hub's `Arc` unshared and changes it in place; when the
/// upgrade fails (the analysis is gone) the check stops. A copy that is no
/// longer the file the scan recorded (a link, a FIFO, another file) is
/// never read, and its group is reported as unreadable.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_confirm(
    tree: Weak<Tree>,
    groups: Vec<(usize, Group)>,
    generation: u64,
    epoch: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<WorkerHandle, io::Error> {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    std::thread::Builder::new()
        .name("hematita-confirm".to_owned())
        .spawn(move || {
            for (index, group) in &groups {
                let index = *index;
                let Some(strong) = tree.upgrade() else {
                    return;
                };
                let files = duplicates::members(&strong, group);
                drop(strong);
                let checked = duplicates::confirm(files, &token, &mut |_| {});
                match checked {
                    Ok(verified) => {
                        let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                            hub.apply_verified(generation, epoch, index, verified);
                        });
                    }
                    Err(ConfirmError::Cancelled) => return,
                    Err(ConfirmError::Read { .. } | ConfirmError::Changed { .. }) => {
                        let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                            hub.apply_unreadable(generation, epoch, index);
                        });
                    }
                }
            }
            let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
                hub.confirm_finished(generation, epoch);
            });
        })?;
    Ok(WorkerHandle { cancel })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_report_is_due_and_the_next_waits_the_interval() {
        let start = Instant::now();
        assert!(due(None, start, PROGRESS_INTERVAL));
        assert!(!due(
            Some(start),
            start + Duration::from_millis(100),
            PROGRESS_INTERVAL
        ));
        assert!(due(
            Some(start),
            start + PROGRESS_INTERVAL,
            PROGRESS_INTERVAL
        ));
    }

    #[test]
    fn dropping_a_handle_cancels_its_token() {
        let cancel = CancellationToken::new();
        let seen = cancel.clone();
        drop(WorkerHandle { cancel });
        assert!(seen.is_cancelled());
    }
}
