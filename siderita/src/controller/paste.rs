//! Deciding and performing one paste.
//!
//! Planning is separate from writing because the interesting question is
//! settled before any byte moves: which sources survive, which collisions the
//! user still has to answer, and which "collision" is an entry meeting itself.

use super::{display_name, ConflictStrategy, PasteOutcome};
use celestina_core::CancellationToken;
use siderita_ops::Progress;
use std::path::{Path, PathBuf};

/// What one paste or drop will do, decided before any write starts: which
/// sources survive, what was already settled for them, and which of them still
/// need the user to answer a collision.
pub(crate) struct PastePlan {
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) decisions: Vec<Option<ConflictStrategy>>,
    pub(crate) colliding: Vec<usize>,
    /// The cut entries dropped because they already live in `destination`.
    /// Kept rather than merely counted: when they are the *whole* paste there is
    /// nothing to write, and the caller still has to settle the clipboard they
    /// came from and tell the person why nothing happened.
    pub(crate) same_folder_cuts: Vec<PathBuf>,
}

/// Looks at every source against the name it would take in `destination`.
///
/// Three outcomes. A free name needs no decision. A name taken by a *different*
/// entry is a genuine collision and is queued for the user. A name taken by the
/// source itself — pasting into the folder the entry already lives in — is not a
/// collision at all: it is a request to duplicate, so a copy is settled as
/// `KeepBoth` here and never reaches the conflict dialog. Answering "Reemplazar"
/// there used to trash the target, which *is* the source, and lose the file.
///
/// Identity is dev+inode rather than the two paths spelled the same way: a
/// clipboard can carry `/home/u/./nota.txt`, a bind mount can reach the same
/// file by two names, and a textual comparison misses both.
///
/// A *cut* into the same folder is instead dropped: moving an entry to where it
/// already is means doing nothing, which is what a drop onto its own folder
/// already does. Renaming it to "(copia)" would be a second surprise. It is
/// reported in `same_folder_cuts` so that a paste made *entirely* of such
/// entries can still say so rather than end in silence.
pub(crate) fn plan_paste(sources: Vec<PathBuf>, destination: &Path, cut: bool) -> PastePlan {
    let mut kept = Vec::with_capacity(sources.len());
    let mut decisions = Vec::with_capacity(sources.len());
    let mut colliding = Vec::new();
    let mut same_folder_cuts = Vec::new();

    for source in sources {
        let Some(target) = source.file_name().map(|name| destination.join(name)) else {
            kept.push(source);
            decisions.push(None);
            continue;
        };
        if std::fs::symlink_metadata(&target).is_err() {
            kept.push(source);
            decisions.push(None);
            continue;
        }
        if is_same_entry(&source, &target) {
            if cut {
                same_folder_cuts.push(source);
                continue;
            }
            kept.push(source);
            decisions.push(Some(ConflictStrategy::KeepBoth));
            continue;
        }
        colliding.push(kept.len());
        kept.push(source);
        decisions.push(None);
    }

    PastePlan {
        sources: kept,
        decisions,
        colliding,
        same_folder_cuts,
    }
}

/// Whether two paths name the same filesystem entry, by device and inode of the
/// links themselves (never following them: a symlink and its target are two
/// entries, and pasting one beside the other is a real collision).
#[cfg(unix)]
fn is_same_entry(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(left), Ok(right)) = (
        std::fs::symlink_metadata(left),
        std::fs::symlink_metadata(right),
    ) else {
        return false;
    };
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn is_same_entry(left: &Path, right: &Path) -> bool {
    left == right
}

/// Whether `held` is exactly the set of entries in `sources`.
///
/// The question a consumed cut has to ask before clearing the system clipboard:
/// it is a shared desktop resource, and another application may have copied
/// something into it while the move was running. Order does not matter — the
/// clipboard is a set of entries — but a partial or extended match does: it
/// means the content is no longer the one this paste consumed.
pub(crate) fn holds_exactly(held: &[PathBuf], sources: &[PathBuf]) -> bool {
    !held.is_empty()
        && held.len() == sources.len()
        && held.iter().all(|entry| sources.contains(entry))
}

/// Pastes one source into `destination_dir` on the worker thread, applying the
/// decided `strategy` when the destination is already taken. Records the outcome
/// (failure, skip, undoable move, kept-back cut) into `outcome`.
pub(crate) fn paste_one(
    source: &Path,
    destination_dir: &Path,
    cut: bool,
    strategy: ConflictStrategy,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(Progress),
    outcome: &mut PasteOutcome,
) {
    let Some(name) = source.file_name() else {
        outcome
            .failures
            .push(format!("{}: sin nombre de archivo", display_name(source)));
        return;
    };
    let target = destination_dir.join(name);
    let colliding = std::fs::symlink_metadata(&target).is_ok();

    if !colliding {
        place_into(source, destination_dir, cut, token, on_progress, outcome);
        return;
    }

    outcome.conflict_touched = true;
    match strategy {
        ConflictStrategy::Skip => outcome.skipped += 1,
        ConflictStrategy::Replace => {
            // Trash the existing entry (recoverable) before placing the source,
            // so nothing is hard-deleted to make room.
            if let Err(error) = siderita_ops::trash(&target, token, on_progress) {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                if cut {
                    outcome.unmoved.push(source.to_path_buf());
                }
                return;
            }
            place_into(source, destination_dir, cut, token, on_progress, outcome);
        }
        ConflictStrategy::KeepBoth => {
            let freed = siderita_ops::next_available(destination_dir, name, "copia");
            let result = if cut {
                siderita_ops::move_as(source, &freed, token, on_progress).map(|_| ())
            } else {
                siderita_ops::copy_as(source, &freed, token, on_progress)
            };
            if let Err(error) = result {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                if cut {
                    outcome.unmoved.push(source.to_path_buf());
                }
            }
        }
    }
}

/// The plain placement (copy or move into a directory, keeping the source name),
/// shared by the no-collision path and by "replace" after the old entry is gone.
fn place_into(
    source: &Path,
    destination_dir: &Path,
    cut: bool,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(Progress),
    outcome: &mut PasteOutcome,
) {
    if cut {
        match siderita_ops::move_entry(source, destination_dir, token, on_progress) {
            Ok(moved) => {
                if let Some(parent) = moved.from.parent() {
                    outcome.undo_moves.push((moved.to, parent.to_path_buf()));
                }
            }
            Err(error) => {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                outcome.unmoved.push(source.to_path_buf());
            }
        }
    } else if let Err(error) = siderita_ops::copy(source, destination_dir, token, on_progress) {
        outcome
            .failures
            .push(format!("{}: {error}", display_name(source)));
    }
}

#[cfg(test)]
mod tests {
    use super::{holds_exactly, plan_paste, ConflictStrategy};
    use std::path::PathBuf;

    /// A scratch directory of this test's own, in the repository idiom: named
    /// after the case, the process and the thread so parallel tests never share
    /// one. The paste planner asks the filesystem, so it needs real entries.
    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "siderita-paste-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mk test dir");
        dir
    }

    /// A scratch directory holding one file, and that file's path.
    fn folder_with_one_file(label: &str) -> (PathBuf, PathBuf) {
        let folder = scratch(label);
        let entry = folder.join("nota.txt");
        std::fs::write(&entry, b"x").expect("write fixture");
        (folder, entry)
    }

    #[test]
    fn a_cut_into_its_own_folder_is_dropped_and_reported() {
        let (folder, entry) = folder_with_one_file("cut-self");
        let plan = plan_paste(vec![entry.clone()], &folder, true);
        assert!(plan.sources.is_empty());
        assert_eq!(plan.same_folder_cuts, vec![entry]);
    }

    #[test]
    fn the_same_entry_copied_into_its_own_folder_is_a_duplicate_not_a_collision() {
        let (folder, entry) = folder_with_one_file("copy-self");
        let plan = plan_paste(vec![entry], &folder, false);
        assert_eq!(plan.decisions, vec![Some(ConflictStrategy::KeepBoth)]);
        assert!(plan.colliding.is_empty());
        assert!(plan.same_folder_cuts.is_empty());
    }

    #[test]
    fn a_cut_that_still_has_work_to_do_reports_only_the_dropped_entry() {
        let (folder, entry) = folder_with_one_file("cut-mixed");
        let elsewhere = scratch("cut-mixed-source");
        let other = elsewhere.join("otra.txt");
        std::fs::write(&other, b"y").expect("write fixture");

        let plan = plan_paste(vec![entry.clone(), other.clone()], &folder, true);
        assert_eq!(plan.sources, vec![other]);
        assert_eq!(plan.same_folder_cuts, vec![entry]);
        assert!(plan.colliding.is_empty());
    }

    #[test]
    fn a_clipboard_that_gained_an_entry_is_no_longer_the_one_that_was_consumed() {
        let consumed = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        assert!(holds_exactly(
            &[PathBuf::from("/tmp/b"), PathBuf::from("/tmp/a")],
            &consumed
        ));
        assert!(!holds_exactly(&[PathBuf::from("/tmp/a")], &consumed));
        assert!(!holds_exactly(&[], &consumed));
    }
}
