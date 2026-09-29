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
//!   become work — classification is free, `stat` is not. "Dotfile" means
//!   what the walk means by it: a name *below the root* that starts with a
//!   dot. A root that itself lives under `~/.var/app/…` is walked, so it is
//!   watched too.
//! - **A folder is not a file.** Moving a whole folder in or out arrives as
//!   one event for the folder and none for what is inside it. A folder that
//!   arrived is a rescan, because its contents were never announced; one that
//!   left takes every record under it with it.
//! - **A burst becomes one resync.** Beyond a bounded batch, folding changes in
//!   one by one is both slower and less certain than walking again: notify can
//!   drop events under pressure, and a rename storm is ambiguous. Saying
//!   "rescan" is the honest answer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use fluorita_core::{MediaKind, SourceSet};
use notify_debouncer_full::notify::event::ModifyKind;
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
    /// Something at this path that is not a media file left — a folder moved
    /// out, moved away or deleted with everything in it. Whatever the library
    /// holds under it is gone with it.
    RemovedTree(PathBuf),
    /// Something happened that cannot be folded in one file at a time.
    Resync(ResyncReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResyncReason {
    /// More changed at once than is worth applying individually.
    Burst,
    /// The watcher itself reported a problem, so events may have been lost.
    Degraded,
    /// A folder arrived or was renamed, or a root itself changed. What is
    /// inside a folder arrives with no event of its own, and a root that went
    /// away is a drive that no longer answers, which only a walk may judge.
    Folder,
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
        // The debouncer's thread judges every event against the roots, so it
        // owns a copy of them; the configuration is small and fixed for the
        // life of one watch.
        let roots = sources.clone();

        let mut debouncer = new_debouncer(COALESCE, None, move |result: DebounceEventResult| {
            let batch = match result {
                Ok(events) => interpret(
                    events
                        .iter()
                        .map(|event| (event.event.kind, event.event.paths.clone())),
                    &roots,
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

    /// Takes every batch already waiting, without waiting for more.
    ///
    /// What a caller does right before a walk: the walk reads the disk as it
    /// is now, so the resyncs the watcher queued until this moment — a run of
    /// bursts, a storm of folder moves — are answered by it, and letting each
    /// ask for a walk of its own would re-walk the library once per 200 ms of
    /// a long copy. The individual changes are handed back rather than
    /// dropped: a walk does not judge every file (a ceiling, a deadline, the
    /// depth bound, a root that did not answer), so the caller applies them
    /// again after it. What arrives during the walk stays queued.
    #[must_use]
    pub fn drain(&self) -> Vec<Vec<LibraryChange>> {
        drain_pending(&self.changes)
    }

    /// Roots the platform refused to watch. Empty is the ordinary case.
    #[must_use]
    pub fn unwatched(&self) -> &[PathBuf] {
        &self.unwatched
    }
}

/// Empties `changes` of what is already there, without waiting.
fn drain_pending(changes: &Receiver<Vec<LibraryChange>>) -> Vec<Vec<LibraryChange>> {
    std::iter::from_fn(|| changes.try_recv().ok()).collect()
}

/// Turns raw events into what the library should do about them.
///
/// Separated from the watcher so the rules — which events matter, which paths
/// are library items, when a folder or a burst becomes a resync — are testable
/// without a watcher. Each path is judged against the root that holds it, the
/// same way the walk judges it.
fn interpret(
    events: impl IntoIterator<Item = (EventKind, Vec<PathBuf>)>,
    sources: &SourceSet,
) -> Vec<LibraryChange> {
    let mut changes: Vec<LibraryChange> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    for (kind, paths) in events {
        // Reading a directory is an event too. Reacting to it would make the
        // scan trigger the scan.
        if matches!(kind, EventKind::Access(_)) {
            continue;
        }
        for path in paths {
            let Some(place) = place_of(sources, &path) else {
                continue;
            };
            if seen.contains(&path) {
                continue;
            }
            seen.push(path.clone());

            let change = match place {
                Place::Root => {
                    moves_folders(kind).then_some(LibraryChange::Resync(ResyncReason::Folder))
                }
                Place::Media => {
                    // A rename arrives as two paths; whether each side exists
                    // is what says which is which, and asking the filesystem
                    // is cheaper than trying to pair the halves of a
                    // `Modify(Name)` event.
                    Some(if path.exists() {
                        LibraryChange::Touched(path)
                    } else {
                        LibraryChange::Removed(path)
                    })
                }
                Place::Other if moves_folders(kind) => match std::fs::symlink_metadata(&path) {
                    // A folder whose contents were never announced. The walk
                    // does not follow symlinks, so neither does this.
                    Ok(metadata) if metadata.is_dir() => {
                        Some(LibraryChange::Resync(ResyncReason::Folder))
                    }
                    // A file that is not media: nothing the library holds.
                    Ok(_) => None,
                    // Gone, and the event says it was a file that is not
                    // media: nothing the library holds went with it.
                    Err(_) if names_a_file(kind) => None,
                    // Gone. If it was a folder, what was under it went too; a
                    // rename does not say which it was, and when it was not a
                    // folder nothing is under it and forgetting finds nothing.
                    Err(_) => Some(LibraryChange::RemovedTree(path)),
                },
                Place::Other => None,
            };
            match change {
                // One walk answers everything else in the batch as well.
                Some(LibraryChange::Resync(reason)) => {
                    return vec![LibraryChange::Resync(reason)];
                }
                Some(change) => changes.push(change),
                None => {}
            }

            if changes.len() > MAX_BATCH {
                return vec![LibraryChange::Resync(ResyncReason::Burst)];
            }
        }
    }
    changes
}

/// What a changed path is, as far as the library can tell from its name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Place {
    /// A configured root itself.
    Root,
    /// A name that classifies as media, below a root and not hidden.
    Media,
    /// Anything else below a root and not hidden: a folder, or a file the
    /// library does not hold.
    Other,
}

/// Where `path` sits, or `None` when it is outside every root or hidden below
/// one.
///
/// The same rules the scan applies, so the watch and the walk agree on what
/// the library contains: a name below the root that starts with a dot is not
/// the library, and only names that classify as media are items. A dot in the
/// root's own ancestry is none of the walk's business, and so none of this.
fn place_of(sources: &SourceSet, path: &Path) -> Option<Place> {
    let relative = sources
        .sources()
        .iter()
        .find_map(|source| path.strip_prefix(source.root()).ok())?;
    if relative.as_os_str().is_empty() {
        return Some(Place::Root);
    }
    if relative
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
    {
        return None;
    }
    Some(if MediaKind::classify_path(path).is_some() {
        Place::Media
    } else {
        Place::Other
    })
}

/// Whether the event itself says its path was a file rather than a folder.
const fn names_a_file(kind: EventKind) -> bool {
    use notify_debouncer_full::notify::event::{CreateKind, RemoveKind};
    matches!(
        kind,
        EventKind::Create(CreateKind::File) | EventKind::Remove(RemoveKind::File)
    )
}

/// Whether an event can bring a folder in or take one away. A folder whose
/// permissions or timestamps changed has not moved, and a rescan for it would
/// be one walk per `chmod`.
const fn moves_folders(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    )
}

#[cfg(test)]
mod tests {
    use super::{drain_pending, interpret, LibraryChange, ResyncReason, MAX_BATCH};
    use fluorita_core::{KindSet, SourceSet};
    use notify_debouncer_full::notify::event::{
        AccessKind, CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode,
    };
    use notify_debouncer_full::notify::EventKind;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("fluorita-watch-tests/{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch");
        directory
    }

    fn rooted(root: &Path) -> SourceSet {
        let mut sources = SourceSet::new();
        sources
            .add(root.to_path_buf(), KindSet::all())
            .expect("an absolute root");
        sources
    }

    /// The fixed root most rules are stated against. Nothing under it exists,
    /// which is what a removal looks like.
    fn media() -> SourceSet {
        rooted(Path::new("/m"))
    }

    #[test]
    fn reading_a_directory_is_not_a_change() {
        // The loop this prevents: a scan opens the folder, notify reports the
        // open, the library rescans, which opens the folder again.
        let changes = interpret(
            [(
                EventKind::Access(AccessKind::Open(
                    notify_debouncer_full::notify::event::AccessMode::Any,
                )),
                vec![PathBuf::from("/m/clip.mkv")],
            )],
            &media(),
        );

        assert!(changes.is_empty());
    }

    #[test]
    fn only_media_names_become_work() {
        let changes = interpret(
            [(
                EventKind::Create(CreateKind::File),
                vec![
                    PathBuf::from("/m/notas.txt"),
                    PathBuf::from("/m/.oculta.png"),
                    PathBuf::from("/m/.cache/dentro.png"),
                ],
            )],
            &media(),
        );

        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn a_file_that_exists_is_touched_and_one_that_does_not_is_removed() {
        let directory = scratch("touched");
        let present = directory.join("presente.png");
        std::fs::write(&present, b"").expect("fixture");
        let absent = directory.join("ausente.png");

        let changes = interpret(
            [(
                EventKind::Create(CreateKind::File),
                vec![present.clone(), absent.clone()],
            )],
            &rooted(&directory),
        );

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

        let changes = interpret(
            [
                (EventKind::Create(CreateKind::File), vec![path.clone()]),
                (
                    EventKind::Modify(ModifyKind::Data(
                        notify_debouncer_full::notify::event::DataChange::Content,
                    )),
                    vec![path.clone()],
                ),
            ],
            &rooted(&directory),
        );

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

        let changes = interpret(
            [(EventKind::Create(CreateKind::File), paths)],
            &rooted(&directory),
        );

        assert_eq!(changes, vec![LibraryChange::Resync(ResyncReason::Burst)]);
    }

    #[test]
    fn a_removal_of_something_that_was_never_media_is_still_ignored() {
        let changes = interpret(
            [(
                EventKind::Remove(RemoveKind::File),
                vec![PathBuf::from("/m/notas.txt")],
            )],
            &media(),
        );

        assert!(changes.is_empty());
    }

    #[test]
    fn a_folder_moved_into_a_root_asks_for_a_rescan() {
        // `mv album/ <music folder>/` is one event for the folder and none for
        // what is inside it, so nothing per-file would ever arrive.
        let root = scratch("moved-in");
        let album = root.join("Album 2.0");
        std::fs::create_dir_all(&album).expect("fixture folder");
        std::fs::write(album.join("pista.flac"), b"").expect("fixture file");

        for kind in [
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            EventKind::Create(CreateKind::Folder),
        ] {
            let changes = interpret([(kind, vec![album.clone()])], &rooted(&root));
            assert_eq!(
                changes,
                vec![LibraryChange::Resync(ResyncReason::Folder)],
                "{kind:?}"
            );
        }
    }

    #[test]
    fn a_folder_moved_out_of_a_root_takes_what_was_under_it() {
        let root = scratch("moved-out");
        let album = root.join("album");

        let changes = interpret(
            [(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                vec![album.clone()],
            )],
            &rooted(&root),
        );

        assert_eq!(changes, vec![LibraryChange::RemovedTree(album)]);
    }

    #[test]
    fn a_folder_renamed_inside_a_root_is_one_rescan() {
        let root = scratch("renamed");
        let after = root.join("after");
        std::fs::create_dir_all(&after).expect("fixture folder");
        let before = root.join("before");

        let changes = interpret(
            [(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                vec![before, after],
            )],
            &rooted(&root),
        );

        assert_eq!(changes, vec![LibraryChange::Resync(ResyncReason::Folder)]);
    }

    #[test]
    fn a_folder_whose_attributes_changed_is_not_a_rescan() {
        let root = scratch("attributes");
        let album = root.join("album");
        std::fs::create_dir_all(&album).expect("fixture folder");

        let changes = interpret(
            [(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)),
                vec![album],
            )],
            &rooted(&root),
        );

        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn a_root_under_a_hidden_directory_is_still_live() {
        // `~/.var/app/<id>/Pictures`: the scan walks it, so the watch must
        // apply what happens in it too.
        let root = scratch(".var/app/Pictures");
        let photo = root.join("foto.png");
        std::fs::write(&photo, b"").expect("fixture");

        let changes = interpret(
            [(EventKind::Create(CreateKind::File), vec![photo.clone()])],
            &rooted(&root),
        );

        assert_eq!(changes, vec![LibraryChange::Touched(photo)]);
    }

    #[test]
    fn a_hidden_directory_inside_a_root_is_still_not_the_library() {
        let root = scratch("hidden-inside");
        let hidden = root.join(".cache");
        std::fs::create_dir_all(&hidden).expect("fixture folder");
        std::fs::write(hidden.join("dentro.png"), b"").expect("fixture file");

        let changes = interpret(
            [
                (EventKind::Create(CreateKind::Folder), vec![hidden.clone()]),
                (
                    EventKind::Create(CreateKind::File),
                    vec![hidden.join("dentro.png")],
                ),
            ],
            &rooted(&root),
        );

        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn a_path_under_no_configured_root_is_ignored() {
        let changes = interpret(
            [(
                EventKind::Remove(RemoveKind::Folder),
                vec![PathBuf::from("/elsewhere/album")],
            )],
            &media(),
        );

        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn a_root_that_went_away_is_a_rescan_rather_than_a_deletion() {
        // An unmounted or renamed root is a drive that did not answer: the
        // scan keeps what it holds. Forgetting it here would eat the drive.
        let changes = interpret(
            [(
                EventKind::Remove(RemoveKind::Folder),
                vec![PathBuf::from("/m")],
            )],
            &media(),
        );

        assert_eq!(changes, vec![LibraryChange::Resync(ResyncReason::Folder)]);
    }

    #[test]
    fn a_queued_run_of_resyncs_collapses_into_the_walk_about_to_happen() {
        let (sender, changes) = std::sync::mpsc::channel();
        for _ in 0..5 {
            sender
                .send(vec![LibraryChange::Resync(ResyncReason::Burst)])
                .expect("queued");
        }
        let removed = LibraryChange::Removed(PathBuf::from("/m/x.png"));
        sender.send(vec![removed.clone()]).expect("queued");

        let drained = drain_pending(&changes);
        assert_eq!(drained.len(), 6);
        // A change that is not a resync is handed back, not lost.
        assert_eq!(drained.last(), Some(&vec![removed]));
        // Nothing waits any more, and draining never blocks on an empty queue.
        assert!(drain_pending(&changes).is_empty());
        // What arrives afterwards is still delivered.
        sender
            .send(vec![LibraryChange::Resync(ResyncReason::Folder)])
            .expect("queued");
        assert_eq!(
            changes.try_recv().ok(),
            Some(vec![LibraryChange::Resync(ResyncReason::Folder)])
        );
    }
}
