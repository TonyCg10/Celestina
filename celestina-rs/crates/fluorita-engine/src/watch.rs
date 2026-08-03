//! Noticing what changed, instead of walking everything again.
//!
//! A full scan of the author's library costs 251 µs, so this is not about
//! speed today — it is about a library that keeps up *while it is open*. A file
//! dropped into a watched folder should appear without the user asking, and one
//! that goes away should say so.
//!
//! Three rules the suite already paid for, and this inherits:
//!
//! - **Access events are ignored.** Reading a directory is itself an event, so
//!   reacting to `Access` makes a scan trigger the scan that triggers it.
//!   Siderita learned that with its folder watch; nothing here repeats it.
//! - **The same things are not library items here either.** A dotfile, a
//!   symlink and a name that classifies as nothing are dropped before they
//!   become work — classification is free, `stat` is not.
//! - **A burst becomes one resync.** Beyond a bounded batch, folding changes in
//!   one by one is both slower and less certain than walking again: notify can
//!   drop events under pressure, and a rename storm is ambiguous. Saying
//!   "rescan" is the honest answer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use fluorita_core::{MediaKind, SourceSet};
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::error::{EngineError, EngineResult};

/// How long events are coalesced before they arrive. The same 200 ms Siderita's
/// folder watch settled on: long enough that a copy of many files lands as one
/// batch, short enough that a single drop feels immediate.
const COALESCE: Duration = Duration::from_millis(200);

/// Paths in one batch beyond which a full rescan is the better answer.
const MAX_BATCH: usize = 512;

/// What the library should do about a change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryChange {
    /// This path is media that appeared or changed: stat it and fold it in.
    Touched(PathBuf),
    /// This path was media and is gone.
    Removed(PathBuf),
    /// Something happened that cannot be folded in one file at a time.
    Resync(ResyncReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResyncReason {
    /// More changed at once than is worth applying individually.
    Burst,
    /// The watcher itself reported a problem, so events may have been lost.
    Degraded,
}

/// A live watch over the configured roots.
///
/// Dropping it stops watching: the debouncer owns the platform watch and its
/// thread, and both go with it.
pub struct LibraryWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    changes: Receiver<Vec<LibraryChange>>,
    /// Roots that could not be watched. Reported rather than hidden: a library
    /// that silently stopped noticing one folder looks like a bug in the scan.
    unwatched: Vec<PathBuf>,
}

impl LibraryWatcher {
    /// Starts watching every configured root, recursively.
    pub fn start(sources: &SourceSet) -> EngineResult<Self> {
        let (sender, changes) = mpsc::channel::<Vec<LibraryChange>>();

        let mut debouncer = new_debouncer(COALESCE, None, move |result: DebounceEventResult| {
            let batch = match result {
                Ok(events) => interpret(
                    events
                        .iter()
                        .map(|event| (event.event.kind, event.event.paths.clone())),
                ),
                // Lost events mean the catalogue and the disk may disagree in a
                // way no individual update can fix.
                Err(_) => vec![LibraryChange::Resync(ResyncReason::Degraded)],
            };
            if !batch.is_empty() {
                let _ = sender.send(batch);
            }
        })
        .map_err(|_| EngineError::UnusableSource {
            path: PathBuf::from("<watch>"),
            reason: "the filesystem watcher could not be created",
        })?;

        let mut unwatched = Vec::new();
        for source in sources.sources() {
            if debouncer
                .watch(source.root(), RecursiveMode::Recursive)
                .is_err()
            {
                unwatched.push(source.root().to_path_buf());
            }
        }

        Ok(Self {
            _debouncer: debouncer,
            changes,
            unwatched,
        })
    }

    /// The next batch of changes, or `None` if nothing arrived in `timeout`.
    pub fn poll(&self, timeout: Duration) -> Option<Vec<LibraryChange>> {
        match self.changes.recv_timeout(timeout) {
            Ok(batch) => Some(batch),
            Err(RecvTimeoutError::Timeout) => None,
            // The debouncer is gone; so is the watch.
            Err(RecvTimeoutError::Disconnected) => None,
        }
    }

    /// Roots the platform refused to watch. Empty is the ordinary case.
    #[must_use]
    pub fn unwatched(&self) -> &[PathBuf] {
        &self.unwatched
    }
}

/// Turns raw events into what the library should do about them.
///
/// Separated from the watcher so the rules — which events matter, which paths
/// are library items, when a burst becomes a resync — are testable without a
/// filesystem.
fn interpret(events: impl IntoIterator<Item = (EventKind, Vec<PathBuf>)>) -> Vec<LibraryChange> {
    let mut changes: Vec<LibraryChange> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    for (kind, paths) in events {
        // Reading a directory is an event too. Reacting to it would make the
        // scan trigger the scan.
        if matches!(kind, EventKind::Access(_)) {
            continue;
        }
        for path in paths {
            if !is_library_item(&path) {
                continue;
            }
            if seen.contains(&path) {
                continue;
            }
            seen.push(path.clone());

            // A rename arrives as two paths; whether each side exists is what
            // says which is which, and asking the filesystem is cheaper than
            // trying to pair the halves of a `Modify(Name)` event.
            if path.exists() {
                changes.push(LibraryChange::Touched(path));
            } else {
                changes.push(LibraryChange::Removed(path));
            }

            if changes.len() > MAX_BATCH {
                return vec![LibraryChange::Resync(ResyncReason::Burst)];
            }
        }
    }
    changes
}

/// The same rules the scan applies, so the watch and the walk agree on what the
/// library contains: no dotfiles, and only names that classify as media.
fn is_library_item(path: &Path) -> bool {
    if path
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
    {
        return false;
    }
    MediaKind::classify_path(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::{interpret, is_library_item, LibraryChange, ResyncReason, MAX_BATCH};
    use notify_debouncer_full::notify::event::{AccessKind, CreateKind, RemoveKind};
    use notify_debouncer_full::notify::EventKind;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("fluorita-watch-tests/{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch");
        directory
    }

    #[test]
    fn reading_a_directory_is_not_a_change() {
        // The loop this prevents: a scan opens the folder, notify reports the
        // open, the library rescans, which opens the folder again.
        let changes = interpret([(
            EventKind::Access(AccessKind::Open(
                notify_debouncer_full::notify::event::AccessMode::Any,
            )),
            vec![PathBuf::from("/m/clip.mkv")],
        )]);

        assert!(changes.is_empty());
    }

    #[test]
    fn only_media_names_become_work() {
        let changes = interpret([(
            EventKind::Create(CreateKind::File),
            vec![
                PathBuf::from("/m/notas.txt"),
                PathBuf::from("/m/.oculta.png"),
                PathBuf::from("/m/.cache/dentro.png"),
                PathBuf::from("/m/sin-extension"),
            ],
        )]);

        assert!(changes.is_empty(), "{changes:?}");
        assert!(!is_library_item(Path::new("/m/.oculta.png")));
        assert!(is_library_item(Path::new("/m/clip.mkv")));
    }

    #[test]
    fn a_file_that_exists_is_touched_and_one_that_does_not_is_removed() {
        let directory = scratch("touched");
        let present = directory.join("presente.png");
        std::fs::write(&present, b"").expect("fixture");
        let absent = directory.join("ausente.png");

        let changes = interpret([(
            EventKind::Create(CreateKind::File),
            vec![present.clone(), absent.clone()],
        )]);

        assert_eq!(
            changes,
            vec![
                LibraryChange::Touched(present),
                LibraryChange::Removed(absent)
            ]
        );
    }

    #[test]
    fn the_same_path_twice_in_one_batch_is_one_change() {
        let directory = scratch("dedup");
        let path = directory.join("a.png");
        std::fs::write(&path, b"").expect("fixture");

        let changes = interpret([
            (EventKind::Create(CreateKind::File), vec![path.clone()]),
            (
                EventKind::Modify(notify_debouncer_full::notify::event::ModifyKind::Data(
                    notify_debouncer_full::notify::event::DataChange::Content,
                )),
                vec![path.clone()],
            ),
        ]);

        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn a_burst_asks_for_a_rescan_instead_of_a_thousand_updates() {
        let directory = scratch("burst");
        let paths: Vec<PathBuf> = (0..=MAX_BATCH + 1)
            .map(|index| {
                let path = directory.join(format!("clip{index}.mkv"));
                std::fs::write(&path, b"").expect("fixture");
                path
            })
            .collect();

        let changes = interpret([(EventKind::Create(CreateKind::File), paths)]);

        assert_eq!(changes, vec![LibraryChange::Resync(ResyncReason::Burst)]);
    }

    #[test]
    fn a_removal_of_something_that_was_never_media_is_still_ignored() {
        let changes = interpret([(
            EventKind::Remove(RemoveKind::File),
            vec![PathBuf::from("/m/notas.txt")],
        )]);

        assert!(changes.is_empty());
    }
}
