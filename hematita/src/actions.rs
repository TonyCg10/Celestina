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
//! inode are no longer the ones the scan recorded, is refused. A cancelled
//! worker still reports what it removed before it stopped (`cancelled`).
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
use hematita_core::usage::remove::{check_identity, delete_tree, RemoveError};
use hematita_core::usage::tree::NodeId;
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
    /// The device and inode the scan recorded, checked again before acting.
    pub dev: u64,
    pub ino: u64,
}

/// What an action did. `removed` lists only the entries that are gone, each
/// with its allocated bytes; the hub prunes exactly these.
#[derive(Clone, Debug)]
pub struct ActionReport {
    pub kind: ActionKind,
    pub done: usize,
    pub total: usize,
    pub outcome: &'static str,
    pub removed: Vec<(NodeId, u64)>,
}

/// The typed outcome of `done` of `total` items, where `refused_all` says
/// every item was refused before anything was touched and `cancelled` that
/// the person stopped it (what was done before still counts).
#[must_use]
pub fn outcome_of(done: usize, total: usize, refused_all: bool, cancelled: bool) -> &'static str {
    if cancelled {
        "cancelled"
    } else if refused_all && total > 0 {
        "refused"
    } else if total > 0 && done == total {
        "done"
    } else if done > 0 {
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
                    outcome: outcome_of(done, 1, false, false),
                    removed: Vec::new(),
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
    Refused,
    Failed,
    Cancelled,
}

/// Whether `item` may still be acted on: not a mount root, and still the
/// entry the scan saw (same device and inode). Between this check and the
/// operation's own syscalls a replacement can still slip in; the window is
/// that short, not closed.
fn admissible(item: &Item, boundaries: &HashSet<PathBuf>) -> Result<(), Step> {
    if boundaries.contains(&item.path) {
        return Err(Step::Refused);
    }
    match check_identity(&item.path, item.dev, item.ino) {
        Ok(()) => Ok(()),
        Err(RemoveError::Refused { .. }) => Err(Step::Refused),
        Err(_) => Err(Step::Failed),
    }
}

/// Runs `act` over `items` in order, reporting each finished item's count
/// and, at the end or at a cancellation, what is gone.
fn run(
    kind: ActionKind,
    items: &[Item],
    token: &CancellationToken,
    target: &Target,
    boundaries: &HashSet<PathBuf>,
    mut act: impl FnMut(&Item) -> Step,
) {
    let total = items.len();
    let mut removed = Vec::new();
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
            Step::Refused => refused += 1,
            Step::Failed => {}
            Step::Cancelled => {
                cancelled = true;
                break;
            }
        }
        target.progress(removed.len());
    }
    let done = removed.len();
    target.report(ActionReport {
        kind,
        done,
        total,
        outcome: outcome_of(done, total, refused == total, cancelled),
        removed,
    });
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
            run(
                ActionKind::Trash,
                &items,
                &token,
                &target,
                &boundaries,
                |item| match siderita_ops::trash(&item.path, &token, &mut |_| {}) {
                    Ok(_) => Step::Removed,
                    Err(OpError::Cancelled) => Step::Cancelled,
                    Err(_) => Step::Failed,
                },
            );
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
            run(
                ActionKind::Delete,
                &items,
                &token,
                &target,
                &boundaries,
                |item| match delete_tree(&item.path, &within, &boundaries, &token) {
                    Ok(_) => Step::Removed,
                    Err(RemoveError::Refused { .. }) => Step::Refused,
                    Err(RemoveError::Cancelled) => Step::Cancelled,
                    Err(RemoveError::Io { .. }) => Step::Failed,
                },
            );
        })?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outcome_follows_the_counts() {
        assert_eq!(outcome_of(3, 3, false, false), "done");
        assert_eq!(outcome_of(1, 3, false, false), "partial");
        assert_eq!(outcome_of(0, 3, false, false), "failed");
        assert_eq!(outcome_of(0, 3, true, false), "refused");
        assert_eq!(outcome_of(0, 0, true, false), "failed");
        assert_eq!(outcome_of(1, 3, false, true), "cancelled");
    }

    #[test]
    fn the_kinds_cross_as_tokens() {
        assert_eq!(ActionKind::Open.as_str(), "open");
        assert_eq!(ActionKind::Trash.as_str(), "trash");
        assert_eq!(ActionKind::Delete.as_str(), "delete");
    }
}
