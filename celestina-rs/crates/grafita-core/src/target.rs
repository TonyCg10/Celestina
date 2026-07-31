//! The file a document came from, and the identity a save must still find.
//!
//! Grafita follows symlinks instead of replacing them, so the target is the
//! resolved file. Everything a later save has to compare against — the resolved
//! path, the inode it named, and that inode's size and modification time — is
//! captured once and carried with the document.

use std::fs::Metadata;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The inode a document was read from, as it stood at that moment.
///
/// Device and inode catch a replaced file; size and modification time catch an
/// in-place rewrite that kept the inode. Together they are what "changed
/// underneath" means.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: i64,
}

impl FileIdentity {
    #[must_use]
    pub fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.size(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

/// The ownership and permission bits a save has to reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ownership {
    pub mode: u32,
    pub user: u32,
    pub group: u32,
}

impl Ownership {
    #[must_use]
    pub fn of(metadata: &Metadata) -> Self {
        Self {
            mode: metadata.mode(),
            user: metadata.uid(),
            group: metadata.gid(),
        }
    }

    /// The permission bits alone, without the file-type bits `mode` carries.
    #[must_use]
    pub const fn permission_bits(self) -> u32 {
        self.mode & 0o7777
    }
}

/// The resolved file behind a requested path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    requested: PathBuf,
    resolved: PathBuf,
    parent: PathBuf,
    identity: FileIdentity,
    ownership: Ownership,
}

impl Target {
    /// Resolves `requested` and captures what a later save must re-verify.
    ///
    /// Resolution happens before the read so that the whole symlink chain, not
    /// just the final name, is what the document is bound to.
    pub fn resolve(requested: &Path) -> io::Result<Self> {
        let resolved = requested.canonicalize()?;
        let metadata = resolved.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("'{}' is not a regular file", resolved.display()),
            ));
        }
        let parent = resolved.parent().map(Path::to_path_buf).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("'{}' has no parent directory", resolved.display()),
            )
        })?;
        Ok(Self {
            requested: requested.to_path_buf(),
            resolved,
            parent,
            identity: FileIdentity::of(&metadata),
            ownership: Ownership::of(&metadata),
        })
    }

    #[must_use]
    pub fn requested(&self) -> &Path {
        &self.requested
    }

    #[must_use]
    pub fn resolved(&self) -> &Path {
        &self.resolved
    }

    #[must_use]
    pub fn parent(&self) -> &Path {
        &self.parent
    }

    #[must_use]
    pub const fn identity(&self) -> FileIdentity {
        self.identity
    }

    #[must_use]
    pub const fn ownership(&self) -> Ownership {
        self.ownership
    }

    /// Adopts the identity a completed save produced, so the next save does not
    /// mistake this document's own write for someone else's change.
    pub(crate) fn adopt(&mut self, identity: FileIdentity) {
        self.identity = identity;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::Target;
    use crate::testing::scratch_directory;

    #[test]
    fn resolving_follows_a_symlink_to_the_file_it_names() {
        let root = scratch_directory("target-symlink");
        let file = root.join("real.txt");
        let link = root.join("link");
        fs::write(&file, b"contenido").expect("write");
        std::os::unix::fs::symlink(&file, &link).expect("symlink");

        let target = Target::resolve(&link).expect("resolve");

        assert_eq!(target.requested(), link);
        assert_eq!(target.resolved(), file.canonicalize().expect("canonical"));
        assert_eq!(target.parent(), root.canonicalize().expect("canonical"));
        assert_eq!(target.identity().size, 9);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolving_refuses_anything_that_is_not_a_regular_file() {
        let root = scratch_directory("target-directory");

        let error = Target::resolve(&root).expect_err("must refuse a directory");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);

        let error = Target::resolve(&root.join("missing")).expect_err("must refuse a ghost");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);

        let _ = fs::remove_dir_all(root);
    }
}
