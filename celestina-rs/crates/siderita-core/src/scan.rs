use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use celestina_core::{CancellationToken, Generation};

use crate::entry::{DirectoryEntry, EntryKind};
use crate::name_order::compare_names;

#[derive(Clone, Debug)]
pub struct ScanRequest {
    generation: Generation,
    location: PathBuf,
    cancellation: CancellationToken,
}

impl ScanRequest {
    pub(crate) fn new(
        generation: Generation,
        location: PathBuf,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            generation,
            location,
            cancellation,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectorySnapshot {
    generation: Generation,
    location: PathBuf,
    entries: Vec<DirectoryEntry>,
    modified: Option<SystemTime>,
    accessed: Option<SystemTime>,
    created: Option<SystemTime>,
}

impl DirectorySnapshot {
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    #[must_use]
    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.entries
    }

    /// Timestamps of the directory itself, captured by the worker alongside
    /// the entries so UI consumers never need a second metadata read.
    #[must_use]
    pub const fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    #[must_use]
    pub const fn accessed(&self) -> Option<SystemTime> {
        self.accessed
    }

    #[must_use]
    pub const fn created(&self) -> Option<SystemTime> {
        self.created
    }

    #[must_use]
    pub fn visible_entries(&self, show_hidden: bool) -> impl Iterator<Item = &DirectoryEntry> {
        self.entries
            .iter()
            .filter(move |entry| show_hidden || !entry.is_hidden())
    }

    #[cfg(test)]
    pub(crate) fn empty(generation: Generation, location: PathBuf) -> Self {
        Self {
            generation,
            location,
            entries: Vec::new(),
            modified: None,
            accessed: None,
            created: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanError {
    Cancelled {
        generation: Generation,
        location: PathBuf,
    },
    Io {
        generation: Generation,
        location: PathBuf,
        path: PathBuf,
        kind: io::ErrorKind,
        message: String,
    },
}

impl ScanError {
    #[must_use]
    pub const fn generation(&self) -> Generation {
        match self {
            Self::Cancelled { generation, .. } | Self::Io { generation, .. } => *generation,
        }
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        match self {
            Self::Cancelled { location, .. } | Self::Io { location, .. } => location,
        }
    }
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled {
                generation,
                location,
            } => write!(
                formatter,
                "scan {} for '{}' was cancelled",
                generation.value(),
                location.display()
            ),
            Self::Io {
                generation,
                location,
                path,
                kind,
                message,
            } => write!(
                formatter,
                "scan {} for '{}' cannot read '{}': {message} ({kind:?})",
                generation.value(),
                location.display(),
                path.display(),
            ),
        }
    }
}

impl Error for ScanError {}

/// Reads one directory without following symlinks.
///
/// The caller may run this function on any executor. Publication still has to
/// pass through `ScanCoordinator::publish` on the owner thread.
pub fn scan_directory(request: &ScanRequest) -> Result<DirectorySnapshot, ScanError> {
    ensure_not_cancelled(request)?;

    let parent_metadata = fs::metadata(&request.location)
        .map_err(|error| io_error(request, request.location.clone(), error))?;
    if !parent_metadata.is_dir() {
        return Err(ScanError::Io {
            generation: request.generation,
            location: request.location.clone(),
            path: request.location.clone(),
            kind: io::ErrorKind::NotADirectory,
            message: "location is not a directory".to_owned(),
        });
    }

    let directory = fs::read_dir(&request.location)
        .map_err(|error| io_error(request, request.location.clone(), error))?;
    let mut entries = read_entries(request, &parent_metadata, directory)?;

    entries.sort_by(compare_entries);

    Ok(DirectorySnapshot {
        generation: request.generation,
        location: request.location.clone(),
        entries,
        modified: parent_metadata.modified().ok(),
        accessed: parent_metadata.accessed().ok(),
        created: parent_metadata.created().ok(),
    })
}

/// Turns the directory iterator into entries, dropping the names that stopped
/// existing while it ran.
///
/// An entry that vanished between `read_dir` and its metadata read is an entry
/// that is no longer in the directory, not a directory that cannot be listed —
/// and a watched folder is rescanned precisely because it is changing, so this
/// is the ordinary case rather than the exceptional one. Aborting there would
/// throw away every other name for one file a download or a build just removed.
/// Any other error still fails the scan.
fn read_entries<I>(
    request: &ScanRequest,
    parent_metadata: &fs::Metadata,
    directory: I,
) -> Result<Vec<DirectoryEntry>, ScanError>
where
    I: IntoIterator<Item = io::Result<fs::DirEntry>>,
{
    let mut entries = Vec::new();
    for candidate in directory {
        ensure_not_cancelled(request)?;
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_error(request, request.location.clone(), error)),
        };
        let candidate_path = candidate.path();
        match DirectoryEntry::read(&request.location, parent_metadata, candidate) {
            Ok(entry) => entries.push(entry),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io_error(request, candidate_path, error)),
        }
    }
    Ok(entries)
}

fn ensure_not_cancelled(request: &ScanRequest) -> Result<(), ScanError> {
    if request.cancellation.is_cancelled() {
        return Err(ScanError::Cancelled {
            generation: request.generation,
            location: request.location.clone(),
        });
    }
    Ok(())
}

fn io_error(request: &ScanRequest, path: PathBuf, error: io::Error) -> ScanError {
    ScanError::Io {
        generation: request.generation,
        location: request.location.clone(),
        path,
        kind: error.kind(),
        message: error.to_string(),
    }
}

fn compare_entries(left: &DirectoryEntry, right: &DirectoryEntry) -> std::cmp::Ordering {
    entry_rank(left.kind())
        .cmp(&entry_rank(right.kind()))
        .then_with(|| compare_names(left.raw_name(), right.raw_name()))
}

const fn entry_rank(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Symlink => 2,
        EntryKind::Other => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    use celestina_core::GenerationClock;

    use crate::{scan_directory, EntryKind, ScanCoordinator, ScanError};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "celestina-siderita-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scan_preserves_distinct_hardlink_entries() {
        let fixture = TestDirectory::new("hardlinks");
        let first = fixture.path().join("first");
        let second = fixture.path().join("second");
        fs::write(&first, b"same inode").expect("write fixture");
        fs::hard_link(&first, &second).expect("create hardlink");

        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");

        assert_eq!(snapshot.entries().len(), 2);
        assert_ne!(snapshot.entries()[0].id(), snapshot.entries()[1].id());
    }

    #[test]
    fn scan_captures_parent_timestamps_with_the_snapshot() {
        let fixture = TestDirectory::new("parent-metadata");
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");

        let snapshot = scan_directory(&request).expect("scan fixture");

        assert!(snapshot.modified().is_some());
        assert!(snapshot.accessed().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn scan_preserves_non_utf8_names() {
        let fixture = TestDirectory::new("non-utf8");
        let raw_name = OsString::from_vec(vec![b'n', b'a', b'm', b'e', 0xff]);
        fs::write(fixture.path().join(&raw_name), b"content").expect("write fixture");

        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");

        assert_eq!(snapshot.entries().len(), 1);
        assert_eq!(snapshot.entries()[0].raw_name(), raw_name.as_os_str());
    }

    #[test]
    fn directories_sort_before_files_and_hidden_filter_is_non_destructive() {
        let fixture = TestDirectory::new("sort-filter");
        fs::write(fixture.path().join("visible"), b"content").expect("write fixture");
        fs::write(fixture.path().join(".hidden"), b"content").expect("write fixture");
        fs::create_dir(fixture.path().join("folder")).expect("create folder");

        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");

        assert_eq!(snapshot.entries()[0].kind(), EntryKind::Directory);
        assert_eq!(snapshot.entries().len(), 3);
        assert_eq!(snapshot.visible_entries(false).count(), 2);
        assert_eq!(snapshot.visible_entries(true).count(), 3);
    }

    #[test]
    fn an_entry_that_vanishes_mid_scan_is_skipped_not_fatal() {
        let fixture = TestDirectory::new("vanishing");
        fs::write(fixture.path().join("stays"), b"content").expect("write fixture");
        fs::write(fixture.path().join("goes"), b"content").expect("write fixture");

        // The directory iterator is taken first, then one of its names is
        // removed: exactly the window a watcher-driven rescan races with.
        let listing: Vec<_> = fs::read_dir(fixture.path())
            .expect("read fixture directory")
            .collect();
        fs::remove_file(fixture.path().join("goes")).expect("remove fixture entry");

        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");
        let parent = fs::metadata(fixture.path()).expect("read parent metadata");
        let entries =
            super::read_entries(&request, &parent, listing).expect("listing survives the removal");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_name(), OsString::from("stays").as_os_str());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_reports_its_kind_and_whether_it_leads_to_a_directory() {
        let fixture = TestDirectory::new("symlinks");
        fs::create_dir(fixture.path().join("target")).expect("create target directory");
        fs::write(fixture.path().join("file"), b"content").expect("write fixture");
        std::os::unix::fs::symlink(fixture.path().join("target"), fixture.path().join("to-dir"))
            .expect("link to the directory");
        std::os::unix::fs::symlink(fixture.path().join("file"), fixture.path().join("to-file"))
            .expect("link to the file");
        std::os::unix::fs::symlink(fixture.path().join("absent"), fixture.path().join("dangling"))
            .expect("link to nothing");

        let mut coordinator = ScanCoordinator::new();
        let request = coordinator
            .begin(fixture.path())
            .expect("issue scan request");
        let snapshot = scan_directory(&request).expect("scan fixture");
        let by_name = |name: &str| {
            snapshot
                .entries()
                .iter()
                .find(|entry| entry.raw_name() == OsString::from(name).as_os_str())
                .expect("entry is listed")
        };

        // The kind still describes the entry itself; only the target question
        // follows the link.
        assert_eq!(by_name("to-dir").kind(), EntryKind::Symlink);
        assert!(by_name("to-dir").targets_directory());
        assert_eq!(by_name("to-file").kind(), EntryKind::Symlink);
        assert!(!by_name("to-file").targets_directory());
        assert!(!by_name("dangling").targets_directory());
        assert!(by_name("target").targets_directory());
        assert!(!by_name("file").targets_directory());
    }

    #[test]
    fn cancelled_request_stops_before_reading() {
        let fixture = TestDirectory::new("cancelled");
        let mut clock = GenerationClock::default();
        let generation = clock.issue().expect("issue generation");
        let cancellation = celestina_core::CancellationToken::new();
        cancellation.cancel();
        let request =
            super::ScanRequest::new(generation, fixture.path().to_path_buf(), cancellation);

        assert!(matches!(
            scan_directory(&request),
            Err(ScanError::Cancelled { .. })
        ));
    }
}
