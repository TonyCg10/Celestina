//! The storage section's three actions on a selection: open in Siderita,
//! move to the trash, delete permanently.
//!
//! Each runs on a named thread (`hematita-actions`) and answers one
//! [`ActionReport`] through the hub's queue, stamped with the scan generation
//! and the action epoch it was asked under; the hub drops a report that is no
//! longer current. Trash and deletion hold a `CancellationToken` whose other
//! half lives in the `WorkerHandle` the hub keeps; they stop between items
//! and, inside an item, wherever the operation checks it.
//!
//! This file is the only caller of `delete_tree` and the only place that
//! starts `siderita`. Trash goes through `siderita_ops::trash`, the suite's
//! one implementation, so what is trashed here can be restored from
//! Siderita. Deletion is bounded by the scanned root and by the mount table
//! read on this thread; a refusal from `delete_tree` counts as not done.
//! Before either removal, an item that is a mount root, or whose device and
//! inode are no longer the ones the scan recorded, is refused; the deletion
//! hands that same pair to `delete_tree`, which checks it again on the
//! descriptor it opens. A cancelled
//! worker still reports what it removed before it stopped (`cancelled`).
//! A deletion that stopped inside an item after removing part of it reports
//! that item as `partial`, with the bytes it freed; the hub scans what is
//! left of it again and grafts the result in place of the stale subtree.
//! When that stop was a cancellation the worker ends there and the outcome
//! is `cancelled`, the item still listed as partial.
//!
//! The threads are detached like the scan's: a detached thread owns its
//! paths and a `CxxQtThread` whose `queue` drops the report once the hub is
//! gone. The Siderita thread waits for the child it started, so the process
//! is reaped and never lingers as a zombie; the report is queued first.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;

use celestina_core::CancellationToken;
use cxx_qt::CxxQtThread;
use hematita_core::usage::remove::{
    check_identity, delete_tree, Failure, RemoveError, Removed, Scanned,
};
use hematita_core::usage::tree::{Kind, NodeId};
use siderita_ops::OpError;

use crate::analysis::qobject::HematitaAnalysis;
use crate::usage_worker::{mount_boundaries, WorkerHandle};

/// Which action a report answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Open,
    Trash,
    Delete,
}

impl ActionKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Trash => "trash",
            Self::Delete => "delete",
        }
    }
}

/// One entry an action works on: its node, its byte-exact path and the
/// allocated bytes the tree snapshot gives it.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: NodeId,
    pub path: PathBuf,
    pub allocated: u64,
    /// The device, inode and kind the scan recorded, checked again before
    /// acting.
    pub dev: u64,
    pub ino: u64,
    pub kind: Kind,
}

impl Item {
    /// What the scan recorded of the entry, as the core checks it.
    #[must_use]
    pub fn scanned(&self) -> Scanned {
        Scanned {
            dev: self.dev,
            ino: self.ino,
            kind: self.kind,
        }
    }
}

/// What an action did. `removed` lists only the entries that are gone, each
/// with its allocated bytes; the hub prunes exactly these. `partial` lists
/// the items a deletion stopped inside after removing part of them, and
/// `partial_bytes` what those removals freed; the hub grafts a fresh scan of
/// each in place of its stale subtree.
#[derive(Clone, Debug)]
pub struct ActionReport {
    pub kind: ActionKind,
    pub done: usize,
    pub total: usize,
    pub outcome: &'static str,
    pub removed: Vec<(NodeId, u64)>,
    pub partial: Vec<NodeId>,
    pub partial_bytes: u64,
}

/// The typed outcome of `done` of `total` items, `partial` more removed in
/// part, where `refused_all` says every item was refused before anything
/// was touched and `cancelled` that the person stopped it (what was done
/// before still counts).
#[must_use]
pub fn outcome_of(
    done: usize,
    partial: usize,
    total: usize,
    refused_all: bool,
    cancelled: bool,
) -> &'static str {
    if cancelled {
        "cancelled"
    } else if refused_all && total > 0 {
        "refused"
    } else if total > 0 && done == total {
        "done"
    } else if done + partial > 0 {
        "partial"
    } else {
        "failed"
    }
}

fn report(qt: &CxxQtThread<HematitaAnalysis>, generation: u64, epoch: u64, report: ActionReport) {
    let _ = qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
        hub.apply_action(generation, epoch, report);
    });
}

/// Starts `siderita <path>` on the `hematita-actions` thread; a missing
/// binary is `failed`.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_open(
    path: PathBuf,
    generation: u64,
    epoch: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<(), io::Error> {
    std::thread::Builder::new()
        .name("hematita-actions".to_owned())
        .spawn(move || {
            let child = Command::new("siderita").arg(&path).spawn();
            let done = usize::from(child.is_ok());
            report(
                &qt,
                generation,
                epoch,
                ActionReport {
                    kind: ActionKind::Open,
                    done,
                    total: 1,
                    outcome: outcome_of(done, 0, 1, false, false),
                    removed: Vec::new(),
                    partial: Vec::new(),
                    partial_bytes: 0,
                },
            );
            if let Ok(mut child) = child {
                let _ = child.wait();
            }
        })?;
    Ok(())
}

/// How one item ended.
enum Step {
    Removed,
    /// The deletion stopped inside the item after removing part of it;
    /// `cancelled` when the stop was the person's.
    Partial {
        gone: Removed,
        cancelled: bool,
    },
    Refused,
    Failed,
    Cancelled,
}

/// Whether `item` may still be acted on: not a mount root (listed in the
/// mount table, or on another mount than the folder holding it, which
/// `check_identity` tells by mount id), and still the entry the scan saw
/// (same device, inode and kind). Between this check and the operation's own
/// syscalls a replacement can still slip in; the window is that short, not
/// closed.
fn admissible(item: &Item, boundaries: &HashSet<PathBuf>) -> Result<(), Step> {
    if boundaries.contains(&item.path) {
        return Err(Step::Refused);
    }
    match check_identity(&item.path, &item.scanned()) {
        Ok(()) => Ok(()),
        Err(RemoveError::Refused { .. }) => Err(Step::Refused),
        Err(_) => Err(Step::Failed),
    }
}

/// How a deletion that stopped ended: in part when it had removed something
/// (keeping whether a cancellation stopped it, so the loop ends there even
/// on the last item), otherwise by its typed error.
fn stopped(failure: Failure) -> Step {
    if failure.removed.entries > 0 {
        return Step::Partial {
            cancelled: matches!(failure.error, RemoveError::Cancelled),
            gone: failure.removed,
        };
    }
    match failure.error {
        RemoveError::Refused { .. } => Step::Refused,
        RemoveError::Cancelled => Step::Cancelled,
        RemoveError::Io { .. } => Step::Failed,
    }
}

/// Runs `act` over `items` in order, handing each finished item's count to
/// `progress`, and answers, at the end or at a cancellation, what is gone.
fn run(
    kind: ActionKind,
    items: &[Item],
    token: &CancellationToken,
    boundaries: &HashSet<PathBuf>,
    mut progress: impl FnMut(usize),
    mut act: impl FnMut(&Item) -> Step,
) -> ActionReport {
    let total = items.len();
    let mut removed = Vec::new();
    let mut partial = Vec::new();
    let mut partial_bytes = 0_u64;
    let mut refused = 0;
    let mut cancelled = false;
    for item in items {
        if token.is_cancelled() {
            cancelled = true;
            break;
        }
        let step = match admissible(item, boundaries) {
            Ok(()) => act(item),
            Err(step) => step,
        };
        match step {
            Step::Removed => removed.push((item.id, item.allocated)),
            Step::Partial {
                gone,
                cancelled: stop,
            } => {
                partial.push(item.id);
                partial_bytes = partial_bytes.saturating_add(gone.bytes);
                if stop {
                    cancelled = true;
                    progress(removed.len());
                    break;
                }
            }
            Step::Refused => refused += 1,
            Step::Failed => {}
            Step::Cancelled => {
                cancelled = true;
                break;
            }
        }
        progress(removed.len());
    }
    let done = removed.len();
    ActionReport {
        kind,
        done,
        total,
        outcome: outcome_of(done, partial.len(), total, refused == total, cancelled),
        removed,
        partial,
        partial_bytes,
    }
}

/// Where a worker's progress and report go.
struct Target {
    qt: CxxQtThread<HematitaAnalysis>,
    generation: u64,
    epoch: u64,
}

impl Target {
    fn progress(&self, done: usize) {
        let (generation, epoch) = (self.generation, self.epoch);
        let _ = self.qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
            hub.apply_action_progress(generation, epoch, done);
        });
    }

    fn report(&self, report: ActionReport) {
        let (generation, epoch) = (self.generation, self.epoch);
        let _ = self.qt.queue(move |hub: Pin<&mut HematitaAnalysis>| {
            hub.apply_action(generation, epoch, report);
        });
    }
}

/// Moves each item to the trash through `siderita_ops::trash`, after the
/// mount-root and identity checks, stopping at a cancellation.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_trash(
    items: Vec<Item>,
    generation: u64,
    epoch: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<WorkerHandle, io::Error> {
    let (handle, token) = WorkerHandle::pair();
    let target = Target {
        qt,
        generation,
        epoch,
    };
    std::thread::Builder::new()
        .name("hematita-actions".to_owned())
        .spawn(move || {
            let boundaries = mount_boundaries();
            let report = run(
                ActionKind::Trash,
                &items,
                &token,
                &boundaries,
                |done| target.progress(done),
                |item| match siderita_ops::trash(&item.path, &token, &mut |_| {}) {
                    Ok(_) => Step::Removed,
                    Err(OpError::Cancelled) => Step::Cancelled,
                    Err(_) => Step::Failed,
                },
            );
            target.report(report);
        })?;
    Ok(handle)
}

/// Deletes each item permanently with `delete_tree`, bounded by `within` (the
/// scanned root) and the mount table read here, after the identity check.
///
/// # Errors
///
/// The thread could not be created.
pub fn spawn_delete(
    items: Vec<Item>,
    within: PathBuf,
    generation: u64,
    epoch: u64,
    qt: CxxQtThread<HematitaAnalysis>,
) -> Result<WorkerHandle, io::Error> {
    let (handle, token) = WorkerHandle::pair();
    let target = Target {
        qt,
        generation,
        epoch,
    };
    std::thread::Builder::new()
        .name("hematita-actions".to_owned())
        .spawn(move || {
            let boundaries = mount_boundaries();
            let report = run(
                ActionKind::Delete,
                &items,
                &token,
                &boundaries,
                |done| target.progress(done),
                |item| {
                    let expected = Some(item.scanned());
                    match delete_tree(&item.path, &within, &boundaries, expected, &token) {
                        Ok(_) => Step::Removed,
                        Err(failure) => stopped(failure),
                    }
                },
            );
            target.report(report);
        })?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outcome_follows_the_counts() {
        assert_eq!(outcome_of(3, 0, 3, false, false), "done");
        assert_eq!(outcome_of(1, 0, 3, false, false), "partial");
        assert_eq!(outcome_of(0, 1, 3, false, false), "partial");
        assert_eq!(outcome_of(0, 0, 3, false, false), "failed");
        assert_eq!(outcome_of(0, 0, 3, true, false), "refused");
        assert_eq!(outcome_of(0, 0, 0, true, false), "failed");
        assert_eq!(outcome_of(1, 0, 3, false, true), "cancelled");
    }

    fn failure(error: RemoveError, entries: u64) -> Failure {
        Failure {
            error,
            removed: Removed {
                entries,
                bytes: entries * 4096,
            },
        }
    }

    #[test]
    fn a_deletion_that_removed_something_before_stopping_is_partial() {
        let io = || RemoveError::Io {
            path: PathBuf::from("/x/a"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        };
        assert!(matches!(
            stopped(failure(io(), 2)),
            Step::Partial {
                gone: Removed { entries: 2, .. },
                cancelled: false
            }
        ));
        assert!(matches!(
            stopped(failure(RemoveError::Cancelled, 1)),
            Step::Partial {
                cancelled: true,
                ..
            }
        ));
        assert!(matches!(stopped(failure(io(), 0)), Step::Failed));
        assert!(matches!(
            stopped(failure(RemoveError::Cancelled, 0)),
            Step::Cancelled
        ));
        let refused = RemoveError::Refused {
            path: PathBuf::from("/x/a"),
            reason: hematita_core::usage::remove::Refusal::MountRoot,
        };
        assert!(matches!(stopped(failure(refused, 0)), Step::Refused));
    }

    /// A real folder as an admissible item: its recorded identity holds.
    fn real_item(id: u32, path: &std::path::Path) -> Item {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path);
        let (dev, ino) = meta.map_or((0, 0), |m| (m.dev(), m.ino()));
        Item {
            id: NodeId(id),
            path: path.to_path_buf(),
            allocated: 4096,
            dev,
            ino,
            kind: Kind::Dir,
        }
    }

    /// A folder of its own under the temporary directory, removed on drop.
    /// The temporary directory itself is no admissible item where it is a
    /// mount point (a tmpfs `/tmp`): the admission refuses a mount root.
    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "hematita-actions-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).expect("temporary folder");
            Self(path)
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    fn run_one(mut step: impl FnMut() -> Step) -> ActionReport {
        let folder = TempFolder::new();
        let items = [real_item(7, &folder.0)];
        let token = CancellationToken::new();
        run(
            ActionKind::Delete,
            &items,
            &token,
            &HashSet::new(),
            |_| {},
            |_| step(),
        )
    }

    #[test]
    fn a_cancel_inside_the_only_item_is_cancelled_and_partial() {
        let report = run_one(|| stopped(failure(RemoveError::Cancelled, 3)));
        assert_eq!(report.outcome, "cancelled");
        assert_eq!(report.partial, vec![NodeId(7)]);
        assert_eq!(report.partial_bytes, 3 * 4096);
        assert!(report.removed.is_empty());
    }

    #[test]
    fn a_partial_stop_without_a_cancel_stays_partial() {
        let io = || RemoveError::Io {
            path: PathBuf::from("/x/a"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        };
        let report = run_one(move || stopped(failure(io(), 2)));
        assert_eq!(report.outcome, "partial");
        assert_eq!(report.partial, vec![NodeId(7)]);
    }

    #[test]
    fn the_kinds_cross_as_tokens() {
        assert_eq!(ActionKind::Open.as_str(), "open");
        assert_eq!(ActionKind::Trash.as_str(), "trash");
        assert_eq!(ActionKind::Delete.as_str(), "delete");
    }
}
