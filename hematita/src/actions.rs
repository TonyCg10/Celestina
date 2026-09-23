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
//!
//! The threads are detached like the scan's: a detached thread owns its
//! paths and a `CxxQtThread` whose `queue` drops the report once the hub is
//! gone. The Siderita thread waits for the child it started, so the process
//! is reaped and never lingers as a zombie; the report is queued first.

use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;

use cxx_qt::CxxQtThread;
use hematita_core::usage::remove::{delete_tree, RemoveError};
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
/// every item was refused before anything was touched.
#[must_use]
pub fn outcome_of(done: usize, total: usize, refused_all: bool) -> &'static str {
    if refused_all && total > 0 {
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
                    outcome: outcome_of(done, 1, false),
                    removed: Vec::new(),
                },
            );
            if let Ok(mut child) = child {
                let _ = child.wait();
            }
        })?;
    Ok(())
}

/// Moves each item to the trash through `siderita_ops::trash`, stopping at a
/// cancellation.
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
    std::thread::Builder::new()
        .name("hematita-actions".to_owned())
        .spawn(move || {
            let total = items.len();
            let mut removed = Vec::new();
            for item in &items {
                match siderita_ops::trash(&item.path, &token, &mut |_| {}) {
                    Ok(_) => removed.push((item.id, item.allocated)),
                    Err(OpError::Cancelled) => break,
                    Err(_) => {}
                }
            }
            let done = removed.len();
            report(
                &qt,
                generation,
                epoch,
                ActionReport {
                    kind: ActionKind::Trash,
                    done,
                    total,
                    outcome: outcome_of(done, total, false),
                    removed,
                },
            );
        })?;
    Ok(handle)
}

/// Deletes each item permanently with `delete_tree`, bounded by `within` (the
/// scanned root) and the mount table read here.
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
    std::thread::Builder::new()
        .name("hematita-actions".to_owned())
        .spawn(move || {
            let boundaries = mount_boundaries();
            let total = items.len();
            let mut removed = Vec::new();
            let mut refused = 0;
            for item in &items {
                match delete_tree(&item.path, &within, &boundaries, &token) {
                    Ok(_) => removed.push((item.id, item.allocated)),
                    Err(RemoveError::Refused { .. }) => refused += 1,
                    Err(RemoveError::Cancelled) => break,
                    Err(RemoveError::Io { .. }) => {}
                }
            }
            let done = removed.len();
            report(
                &qt,
                generation,
                epoch,
                ActionReport {
                    kind: ActionKind::Delete,
                    done,
                    total,
                    outcome: outcome_of(done, total, refused == total),
                    removed,
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
        assert_eq!(outcome_of(3, 3, false), "done");
        assert_eq!(outcome_of(1, 3, false), "partial");
        assert_eq!(outcome_of(0, 3, false), "failed");
        assert_eq!(outcome_of(0, 3, true), "refused");
        assert_eq!(outcome_of(0, 0, true), "failed");
    }

    #[test]
    fn the_kinds_cross_as_tokens() {
        assert_eq!(ActionKind::Open.as_str(), "open");
        assert_eq!(ActionKind::Trash.as_str(), "trash");
        assert_eq!(ActionKind::Delete.as_str(), "delete");
    }
}
