use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fs::{DirEntry, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Stable identity for one directory entry within a snapshot.
///
/// The parent directory identity plus the raw filename keeps hardlinks with
/// different names distinct and never requires lossy UTF-8 conversion.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntryId {
    parent: ParentId,
    name: OsString,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ParentId {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(not(unix))]
    Path(PathBuf),
}

impl EntryId {
    pub(crate) fn new(_parent: &Path, parent_metadata: &Metadata, name: &OsStr) -> Self {
        #[cfg(unix)]
        let parent = ParentId::Unix {
            device: parent_metadata.dev(),
            inode: parent_metadata.ino(),
        };

        #[cfg(not(unix))]
        let parent = ParentId::Path(_parent.to_path_buf());

        Self {
            parent,
            name: name.to_os_string(),
        }
    }

    #[must_use]
    pub fn raw_name(&self) -> &OsStr {
        &self.name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    id: EntryId,
    name: OsString,
    path: PathBuf,
    kind: EntryKind,
    size: u64,
    modified: Option<SystemTime>,
    hidden: bool,
}

impl DirectoryEntry {
    pub(crate) fn read(
        parent: &Path,
        parent_metadata: &Metadata,
        entry: DirEntry,
    ) -> io::Result<Self> {
        let name = entry.file_name();
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        let kind = if file_type.is_dir() {
            EntryKind::Directory
        } else if file_type.is_file() {
            EntryKind::File
        } else if file_type.is_symlink() {
            EntryKind::Symlink
        } else {
            EntryKind::Other
        };

        Ok(Self {
            id: EntryId::new(parent, parent_metadata, &name),
            hidden: is_hidden(&name),
            name,
            path,
            kind,
            size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    #[must_use]
    pub const fn id(&self) -> &EntryId {
        &self.id
    }

    #[must_use]
    pub fn raw_name(&self) -> &OsStr {
        &self.name
    }

    #[must_use]
    pub fn display_name(&self) -> Cow<'_, str> {
        self.name.to_string_lossy()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }
}

#[cfg(unix)]
fn is_hidden(name: &OsStr) -> bool {
    name.as_bytes().first() == Some(&b'.')
}

#[cfg(not(unix))]
fn is_hidden(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}
