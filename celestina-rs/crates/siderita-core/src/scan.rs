use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::{CancellationToken, Generation};

use crate::entry::{DirectoryEntry, EntryKind};

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
    let mut entries = Vec::new();

    for candidate in directory {
        ensure_not_cancelled(request)?;
        let candidate =
            candidate.map_err(|error| io_error(request, request.location.clone(), error))?;
        let candidate_path = candidate.path();
        let entry = DirectoryEntry::read(&request.location, &parent_metadata, candidate)
            .map_err(|error| io_error(request, candidate_path, error))?;
        entries.push(entry);
    }

    entries.sort_by(compare_entries);

    Ok(DirectorySnapshot {
        generation: request.generation,
        location: request.location.clone(),
        entries,
    })
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

fn compare_names(left: &OsStr, right: &OsStr) -> std::cmp::Ordering {
    left.cmp(right)
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
