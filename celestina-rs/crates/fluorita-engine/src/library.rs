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
//! caller that believed it would then mark every unvisited file as missing.
//!
//! Symlinks are not followed. A library that follows them can be walked in a
//! circle by one `ln -s`, and the same file would arrive under two names.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use celestina_core::CancellationToken;
use fluorita_core::{
    MediaId, MediaKind, MediaRecord, MediaSource, SourceId, SourceIdentity, SourceSet,
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

/// The result of one complete pass over the configured roots.
#[derive(Clone, Debug, Default)]
pub struct ScanOutcome {
    pub records: Vec<MediaRecord>,
    /// A bound was reached, so this is *not* the whole library. Reconciliation
    /// must not mark anything missing from a truncated pass.
    pub truncated: bool,
    pub directories_visited: usize,
    /// Entries that could not be read at all — a permission, a broken mount.
    /// Counted rather than dropped silently, so a scan that saw half a library
    /// can say so.
    pub unreadable: usize,
    /// The roots that actually answered. A root missing from this set was not
    /// read — an unplugged drive, a share that is down, a directory the user
    /// cannot open — so nothing may be concluded about the files under it.
    /// A root that is here and complete is the only case where "the scan did
    /// not see it" means "it is gone".
    pub reached: BTreeSet<SourceId>,
}

impl ScanOutcome {
    /// Whether this pass may be used to decide that a file has disappeared.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        !self.truncated
    }
}

/// Walks every configured root and returns the media it found.
pub fn scan(
    sources: &SourceSet,
    limits: ScanLimits,
    cancellation: &CancellationToken,
) -> EngineResult<ScanOutcome> {
    let started = Instant::now();
    let mut outcome = ScanOutcome::default();

    for source in sources.sources() {
        if outcome.truncated {
            break;
        }
        // Asked before walking, and separately from it: `walk` reports a
        // directory it could not open the same way at any depth, and the host
        // needs to know about *this root* specifically before it is allowed to
        // conclude that anything under it was deleted.
        if std::fs::read_dir(source.root()).is_ok() {
            outcome.reached.insert(source.id());
        }
        walk(
            source,
            source.root(),
            0,
            &limits,
            started,
            cancellation,
            &mut outcome,
        )?;
    }
    Ok(outcome)
}

fn walk(
    source: &MediaSource,
    directory: &Path,
    depth: usize,
    limits: &ScanLimits,
    started: Instant,
    cancellation: &CancellationToken,
    outcome: &mut ScanOutcome,
) -> EngineResult<()> {
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    if depth > limits.max_depth {
        outcome.truncated = true;
        return Ok(());
    }

    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => {
            // A root that is not mounted, or a directory the user cannot read,
            // is not a failed scan: it is a gap, and the count says so.
            outcome.unreadable += 1;
            return Ok(());
        }
    };
    outcome.directories_visited += 1;

    let mut subdirectories: Vec<PathBuf> = Vec::new();
    for entry in entries {
        if cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        if started.elapsed() > limits.deadline {
            outcome.truncated = true;
            return Ok(());
        }
        let Ok(entry) = entry else {
            outcome.unreadable += 1;
            continue;
        };
        let name = entry.file_name();
        // A dotfile is configuration, a cache or a trash can — never a library
        // item the user put there to look at.
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            outcome.unreadable += 1;
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
        if !source.kinds().contains(media_kind) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            outcome.unreadable += 1;
            continue;
        };

        outcome.records.push(MediaRecord::new(
            identity_of(&metadata, &path),
            source.id(),
            path,
            media_kind,
            SourceIdentity::new(
                metadata.len(),
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ),
        ));

        if outcome.records.len() >= limits.max_files {
            outcome.truncated = true;
            return Ok(());
        }
    }

    // Depth-first, but after the current directory's files: a shallow library
    // fills the grid before a deep one is walked.
    subdirectories.sort();
    for subdirectory in subdirectories {
        if outcome.truncated {
            return Ok(());
        }
        walk(
            source,
            &subdirectory,
            depth + 1,
            limits,
            started,
            cancellation,
            outcome,
        )?;
    }
    Ok(())
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
    use fluorita_core::{KindSet, MediaKind, MediaRecord, SourceSet};
    use std::path::{Path, PathBuf};

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
        assert!(outcome.is_complete());
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
            !outcome.is_complete(),
            "a truncated pass must never decide that a file disappeared"
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
        assert!(outcome.truncated);
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
