//! Walking the configured roots.
//!
//! This is the only part of Fluorita that touches a directory it was not handed
//! directly, so it is deliberately narrow: it reads names and `stat`, decides a
//! kind from the name, and stops. No file is opened, nothing is decoded, and no
//! decoder is started — a library scan of ten thousand photographs must cost
//! ten thousand `stat` calls, not ten thousand decodes.
//!
//! Four bounds keep a scan from becoming an incident: a file ceiling, a depth
//! ceiling, a deadline and a cancellation token checked between entries. A
//! truncated scan says so rather than pretending it saw everything, because a
//! caller that believed it would then mark every unvisited file as missing —
//! and it says *where*: which roots the walk reached the end of, and which
//! directories it knew about and did not read, so one bound reached in one
//! place does not stop every other root from being judged
//! ([`ScanCoverage`]).
//!
//! Symlinks are not followed. A library that follows them can be walked in a
//! circle by one `ln -s`, and the same file would arrive under two names.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;
use fluorita_core::{
    MediaId, MediaKind, MediaRecord, MediaSource, ScanCoverage, SourceIdentity, SourceSet,
};

use crate::error::{EngineError, EngineResult};

/// What one scan may spend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanLimits {
    /// Records to collect before stopping and reporting the scan truncated.
    pub max_files: usize,
    /// How deep below a root to descend. A root itself is depth zero.
    pub max_depth: usize,
    pub deadline: Duration,
}

impl ScanLimits {
    /// Room for a large personal library without letting a pathological tree
    /// (or a mount that turns out to be a whole filesystem) run unbounded.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            max_files: 50_000,
            max_depth: 12,
            deadline: Duration::from_secs(120),
        }
    }
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self::conservative()
    }
}

/// Directories one root may leave unexplored before the pass stops judging
/// that root at all. Each is remembered so what lives under it is kept as it
/// was; past this many, a root is closer to "not walked" than to "walked with
/// a few gaps", and treating it so keeps the set bounded.
pub const MAX_UNEXPLORED_PER_ROOT: usize = 1_024;

/// The result of one pass over the configured roots.
#[derive(Clone, Debug, Default)]
pub struct ScanOutcome {
    pub records: Vec<MediaRecord>,
    /// A bound was reached somewhere, so the records are *not* the whole
    /// library. Said on screen; what may be concluded from the pass is
    /// [`ScanOutcome::coverage`]'s to answer, per root and per directory.
    pub truncated: bool,
    pub directories_visited: usize,
    /// Entries that could not be read at all — a permission, a broken mount.
    /// Counted rather than dropped silently, so a scan that saw half a library
    /// can say so.
    pub unreadable: usize,
    /// Which roots the walk reached the end of, which of them answered, and
    /// which directories it did not read. A root that is walked and reached is
    /// the only case where "the scan did not see it" means "it is gone".
    pub coverage: ScanCoverage,
}

/// Where one root's walk stands.
#[derive(Default)]
struct RootWalk {
    /// Directories noted as unexplored under this root so far.
    unexplored: usize,
    /// More than [`MAX_UNEXPLORED_PER_ROOT`] were needed, so the root is not
    /// judged.
    overflowed: bool,
}

/// Walks every configured root and returns the media it found.
pub fn scan(
    sources: &SourceSet,
    limits: ScanLimits,
    cancellation: &CancellationToken,
) -> EngineResult<ScanOutcome> {
    let started = Instant::now();
    let mut outcome = ScanOutcome::default();

    let mut stopped = false;
    for source in sources.sources() {
        // A ceiling or a deadline ends the whole pass; the roots after it
        // were never looked at, so none of them is judged.
        if stopped {
            break;
        }
        // Asked before walking, and separately from it: `walk` reports a
        // directory it could not open the same way at any depth, and the host
        // needs to know about *this root* specifically before it is allowed to
        // conclude that anything under it was deleted.
        if std::fs::read_dir(source.root()).is_ok() {
            outcome.coverage.reached(source.id());
        }
        let mut root = RootWalk::default();
        let mut walk = Walk {
            source,
            limits: &limits,
            started,
            cancellation,
            outcome: &mut outcome,
            root: &mut root,
            stopped: false,
        };
        walk.directory(source.root(), 0)?;
        stopped = walk.stopped;
        if !stopped && !root.overflowed {
            outcome.coverage.walked(source.id());
        }
    }
    Ok(outcome)
}

/// One root's walk: what it may spend and where it reports.
struct Walk<'a> {
    source: &'a MediaSource,
    limits: &'a ScanLimits,
    started: Instant,
    cancellation: &'a CancellationToken,
    outcome: &'a mut ScanOutcome,
    root: &'a mut RootWalk,
    /// A ceiling or the deadline ended the pass inside this root.
    stopped: bool,
}

impl Walk<'_> {
    /// Stops the whole pass: nothing after this point is looked at.
    fn stop(&mut self) {
        self.stopped = true;
        self.outcome.truncated = true;
    }

    /// Remembers a directory the walk knew about and did not read.
    fn leave_unexplored(&mut self, directory: &Path) {
        if self.root.unexplored >= MAX_UNEXPLORED_PER_ROOT {
            self.root.overflowed = true;
            return;
        }
        self.root.unexplored += 1;
        self.outcome.coverage.unexplored(directory.to_path_buf());
    }

    fn directory(&mut self, directory: &Path, depth: usize) -> EngineResult<()> {
        if self.cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        if depth > self.limits.max_depth {
            // Not read, and so neither seen nor missing: the grid is not the
            // whole library, but nothing under here is judged either.
            self.outcome.truncated = true;
            self.leave_unexplored(directory);
            return Ok(());
        }

        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => {
                // A root that is not mounted, or a directory the user cannot
                // read, is not a failed scan: it is a gap, and the count says
                // so. A gap below the root is also left unjudged — a folder
                // whose permissions changed has not had its files deleted. The
                // root itself is the case `reached` already answers.
                self.outcome.unreadable += 1;
                if depth > 0 {
                    self.leave_unexplored(directory);
                }
                return Ok(());
            }
        };
        self.outcome.directories_visited += 1;

        let mut subdirectories: Vec<PathBuf> = Vec::new();
        for entry in entries {
            if self.cancellation.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            if self.started.elapsed() > self.limits.deadline {
                self.stop();
                return Ok(());
            }
            let Ok(entry) = entry else {
                self.outcome.unreadable += 1;
                continue;
            };
            let name = entry.file_name();
            // A dotfile is configuration, a cache or a trash can — never a
            // library item the user put there to look at.
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                self.outcome.unreadable += 1;
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                subdirectories.push(entry.path());
                continue;
            }
            if !kind.is_file() {
                continue;
            }

            let path = entry.path();
            let Some(media_kind) = MediaKind::classify_path(&path) else {
                continue;
            };
            if !self.source.kinds().contains(media_kind) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                self.outcome.unreadable += 1;
                continue;
            };

            self.outcome.records.push(MediaRecord::new(
                identity_of(&metadata, &path),
                self.source.id(),
                path,
                media_kind,
                SourceIdentity::new(
                    metadata.len(),
                    metadata
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                ),
            ));

            if self.outcome.records.len() >= self.limits.max_files {
                self.stop();
                return Ok(());
            }
        }

        // Depth-first, but after the current directory's files: a shallow
        // library fills the grid before a deep one is walked.
        subdirectories.sort();
        for subdirectory in subdirectories {
            if self.stopped {
                return Ok(());
            }
            self.directory(&subdirectory, depth + 1)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn identity_of(metadata: &std::fs::Metadata, _path: &Path) -> MediaId {
    use std::os::unix::fs::MetadataExt;
    MediaId::filesystem(metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn identity_of(_metadata: &std::fs::Metadata, path: &Path) -> MediaId {
    MediaId::from_path(path)
}

#[cfg(test)]
mod tests {
    use super::{scan, ScanLimits};
    use celestina_core::CancellationToken;
    use fluorita_core::{
        KindSet, MediaId, MediaKind, MediaRecord, SourceId, SourceIdentity, SourceSet,
    };
    use std::path::{Path, PathBuf};

    /// The handle of the one root [`sources`] configures for `root`.
    fn only_root(root: &Path) -> SourceId {
        sources(root, KindSet::all()).sources()[0].id()
    }

    /// Builds a throwaway tree. Empty files are enough: a scan classifies by
    /// name and never opens anything, which is the property under test.
    fn tree(name: &str, files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("fluorita-scan-tests/{name}"));
        let _ = std::fs::remove_dir_all(&root);
        for relative in files {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("fixture directory");
            }
            std::fs::write(&path, b"").expect("fixture file");
        }
        std::fs::create_dir_all(&root).expect("fixture root");
        root
    }

    fn sources(root: &Path, kinds: KindSet) -> SourceSet {
        let mut set = SourceSet::new();
        set.add(root.to_path_buf(), kinds).expect("absolute root");
        set
    }

    fn names(records: &[MediaRecord]) -> Vec<String> {
        let mut names: Vec<String> = records
            .iter()
            .map(|record| {
                record
                    .path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_scan_finds_media_and_ignores_everything_else() {
        let root = tree(
            "mixed",
            &[
                "foto.png",
                "clip.mkv",
                "cancion.flac",
                "notas.txt",
                "sin-extension",
                "subcarpeta/otra.jpg",
            ],
        );

        let outcome = scan(
            &sources(&root, KindSet::all()),
            ScanLimits::conservative(),
            &CancellationToken::new(),
        )
        .expect("the scan completes");

        assert_eq!(
            names(&outcome.records),
            vec!["cancion.flac", "clip.mkv", "foto.png", "otra.jpg"]
        );
        assert!(!outcome.truncated);
        assert!(outcome.coverage.is_walked(only_root(&root)));
        assert_eq!(outcome.directories_visited, 2);
    }

    #[test]
    fn a_source_only_contributes_the_kinds_it_accepts() {
        let root = tree("kinds", &["foto.png", "clip.mkv", "cancion.flac"]);

        let outcome = scan(
            &sources(&root, KindSet::gallery()),
            ScanLimits::conservative(),
            &CancellationToken::new(),
        )
        .expect("the scan completes");

        assert_eq!(names(&outcome.records), vec!["clip.mkv", "foto.png"]);
        assert!(outcome
            .records
            .iter()
            .all(|record| record.kind() != MediaKind::Audio));
    }

    #[test]
    fn hidden_entries_are_not_library_items() {
        let root = tree(
            "hidden",
            &[
                "visible.png",
                ".oculta.png",
                ".cache/dentro.png",
                ".thumbnails/large/x.png",
            ],
        );

        let outcome = scan(
            &sources(&root, KindSet::all()),
            ScanLimits::conservative(),
            &CancellationToken::new(),
        )
        .expect("the scan completes");

        assert_eq!(names(&outcome.records), vec!["visible.png"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_loop_cannot_walk_the_scan_in_a_circle() {
        let root = tree("symlinks", &["real/foto.png"]);
        std::os::unix::fs::symlink(&root, root.join("real/vuelta")).expect("symlink");
        std::os::unix::fs::symlink(root.join("real/foto.png"), root.join("real/copia.png"))
            .expect("symlink");

        let outcome = scan(
            &sources(&root, KindSet::all()),
            ScanLimits::conservative(),
            &CancellationToken::new(),
        )
        .expect("the scan completes");

        // The loop did not hang, and the same file did not arrive twice.
        assert_eq!(names(&outcome.records), vec!["foto.png"]);
    }

    #[test]
    fn a_truncated_scan_says_so_instead_of_looking_complete() {
        let files: Vec<String> = (0..20).map(|index| format!("clip{index}.mkv")).collect();
        let root = tree(
            "ceiling",
            &files.iter().map(String::as_str).collect::<Vec<_>>(),
        );

        let outcome = scan(
            &sources(&root, KindSet::all()),
            ScanLimits {
                max_files: 5,
                ..ScanLimits::conservative()
            },
            &CancellationToken::new(),
        )
        .expect("the scan stops at its ceiling");

        assert_eq!(outcome.records.len(), 5);
        assert!(outcome.truncated);
        assert!(
            !outcome.coverage.is_walked(only_root(&root)),
            "a pass stopped by its ceiling must never decide that a file disappeared"
        );
    }

    #[test]
    fn depth_is_bounded() {
        let root = tree("depth", &["a/b/c/d/hondo.png", "arriba.png"]);

        let outcome = scan(
            &sources(&root, KindSet::all()),
            ScanLimits {
                max_depth: 1,
                ..ScanLimits::conservative()
            },
            &CancellationToken::new(),
        )
        .expect("the scan stops descending");

        assert_eq!(names(&outcome.records), vec!["arriba.png"]);
        // The grid is not the whole library, and says so; but the root was
        // still walked to its end, so what is above the bound can be judged.
        assert!(outcome.truncated);
        assert!(outcome.coverage.is_walked(only_root(&root)));
    }

    #[test]
    fn one_deep_root_does_not_stop_another_from_being_judged() {
        let deep = tree("per-root-deep", &["a/b/c/d/hondo.png", "arriba.png"]);
        let shallow = tree("per-root-shallow", &["plano.png"]);
        let mut set = SourceSet::new();
        let deep_id = set
            .add(deep.clone(), KindSet::all())
            .expect("absolute root");
        let shallow_id = set
            .add(shallow.clone(), KindSet::all())
            .expect("a second root");

        let outcome = scan(
            &set,
            ScanLimits {
                max_depth: 1,
                ..ScanLimits::conservative()
            },
            &CancellationToken::new(),
        )
        .expect("the scan completes");

        assert!(outcome.coverage.is_walked(deep_id));
        assert!(outcome.coverage.is_walked(shallow_id));
        // Below the bound is neither seen nor missing; beside it is judged.
        let below = MediaRecord::new(
            MediaId::filesystem(1, 1),
            deep_id,
            deep.join("a/b/c/d/hondo.png"),
            MediaKind::Image,
            SourceIdentity::new(0, std::time::UNIX_EPOCH),
        );
        let beside = MediaRecord::new(
            MediaId::filesystem(1, 2),
            deep_id,
            deep.join("gone.png"),
            MediaKind::Image,
            SourceIdentity::new(0, std::time::UNIX_EPOCH),
        );
        assert!(!outcome.coverage.judges(&below));
        assert!(outcome.coverage.proves_gone(&beside));
    }

    #[test]
    fn a_ceiling_reached_in_one_root_leaves_the_roots_after_it_unjudged() {
        let files: Vec<String> = (0..20).map(|index| format!("clip{index}.mkv")).collect();
        let first = tree(
            "ceiling-first",
            &files.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let second = tree("ceiling-second", &["otro.png"]);
        let mut set = SourceSet::new();
        let first_id = set.add(first, KindSet::all()).expect("absolute root");
        let second_id = set.add(second, KindSet::all()).expect("a second root");

        let outcome = scan(
            &set,
            ScanLimits {
                max_files: 5,
                ..ScanLimits::conservative()
            },
            &CancellationToken::new(),
        )
        .expect("the scan stops at its ceiling");

        assert!(!outcome.coverage.is_walked(first_id));
        assert!(!outcome.coverage.is_walked(second_id));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_folder_inside_a_root_is_a_gap_not_a_deletion() {
        use std::os::unix::fs::PermissionsExt;

        let root = tree("unreadable-inside", &["cerrada/dentro.png", "fuera.png"]);
        let closed = root.join("cerrada");
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o000))
            .expect("close the folder");
        let readable = std::fs::read_dir(&closed).is_ok();
        let set = sources(&root, KindSet::all());
        let outcome = scan(&set, ScanLimits::conservative(), &CancellationToken::new())
            .expect("the scan completes");
        std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o755))
            .expect("reopen the folder");
        if readable {
            // Running as root: permissions do not close anything, so there is
            // no gap to observe here.
            return;
        }

        let inside = MediaRecord::new(
            MediaId::filesystem(1, 1),
            only_root(&root),
            closed.join("dentro.png"),
            MediaKind::Image,
            SourceIdentity::new(0, std::time::UNIX_EPOCH),
        );
        assert!(outcome.coverage.is_walked(only_root(&root)));
        assert!(!outcome.coverage.judges(&inside));
    }

    #[test]
    fn cancellation_stops_a_scan_in_progress() {
        let root = tree("cancel", &["uno.png", "dos.png"]);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        assert!(matches!(
            scan(
                &sources(&root, KindSet::all()),
                ScanLimits::conservative(),
                &cancellation
            ),
            Err(crate::error::EngineError::Cancelled)
        ));
    }

    #[test]
    fn an_unreadable_root_is_a_gap_not_a_failure() {
        let mut set = SourceSet::new();
        set.add(
            PathBuf::from("/nonexistent/fluorita/library"),
            KindSet::all(),
        )
        .expect("absolute root");

        let outcome = scan(&set, ScanLimits::conservative(), &CancellationToken::new())
            .expect("a missing root does not fail the scan");

        assert!(outcome.records.is_empty());
        assert_eq!(outcome.unreadable, 1);
    }

    #[cfg(unix)]
    #[test]
    fn identity_survives_a_rename() {
        let root = tree("identity", &["antes.mp3"]);
        let set = sources(&root, KindSet::all());

        let before = scan(&set, ScanLimits::conservative(), &CancellationToken::new())
            .expect("the scan completes");
        std::fs::rename(root.join("antes.mp3"), root.join("despues.mp3")).expect("rename");
        let after = scan(&set, ScanLimits::conservative(), &CancellationToken::new())
            .expect("the scan completes");

        assert_eq!(before.records.len(), 1);
        assert_eq!(after.records.len(), 1);
        assert_eq!(
            before.records[0].id(),
            after.records[0].id(),
            "the same file keeps one catalogue entry after a rename"
        );
    }
}
