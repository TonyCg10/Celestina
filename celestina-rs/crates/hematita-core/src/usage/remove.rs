//! Permanent deletion: the suite's only one, and the one place it is spelled.
//!
//! [`delete_tree`] refuses before touching anything: the scanned root itself,
//! a path outside it (after resolving `.` and `..` lexically, because
//! resolving links would follow the very link the person asked to delete),
//! a folder on the way that is a link or no longer a folder, a mount root, a
//! path that is gone, or an entry that is not the one the scan recorded.
//!
//! It works on directory descriptors, never on re-resolved paths: the
//! analysed folder is opened with `O_DIRECTORY | O_NOFOLLOW`, every folder
//! down to the target is opened the same way from the previous descriptor,
//! and every entry is removed with `unlinkat` relative to the descriptor of
//! the folder that holds it. A folder swapped for a symbolic link after it
//! was opened cannot redirect the removal, because nothing is looked up by
//! path again. The one thing a descriptor cannot pin is the analysed folder
//! itself being renamed while the deletion runs: the removal then continues
//! inside the folder it opened, wherever that now lives.
//!
//! Removal is depth-first, a folder's files before its subfolders and every
//! folder after its content, asking `cancel` before every entry; a symbolic
//! link is removed as itself and its target is never visited. A folder that
//! is another device's mount, or listed as a mount boundary, stops the
//! deletion there. A deletion that stops midway — cancelled, refused or
//! failed — reports what it had already removed in [`Failure`].

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use celestina_core::CancellationToken;
use rustix::fs::{
    fstat, openat, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags, Stat, CWD,
};
use rustix::io::Errno;

/// Every folder is opened read-only as a directory, never through a link.
const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// Folders open at once below the target: one descriptor per level is what
/// makes the walk safe, so the depth is bounded rather than the descriptors
/// shared. A deeper tree fails with [`RemoveError::Io`] at that level.
pub const MAX_DEPTH: usize = 4096;

/// What a deletion removed, completed or not.
///
/// `bytes` counts a hard-linked inode once however many of its names were
/// inside the subtree — and counts it even when another name outside the
/// subtree keeps it alive, so the space actually freed can be smaller.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Removed {
    pub entries: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    IsRoot,
    Outside,
    MountRoot,
    Missing,
    /// A component between the analysed folder and the target is not a real
    /// directory (a symbolic link, or no longer a folder), so the lexical
    /// "inside" would not be where the kernel goes.
    Symlink {
        at: PathBuf,
    },
    /// Something else now sits at the path: its device and inode are not
    /// the ones the scan recorded.
    Changed,
}

#[derive(Debug)]
pub enum RemoveError {
    Cancelled,
    Refused { path: PathBuf, reason: Refusal },
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for RemoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the deletion was cancelled"),
            Self::Refused { path, reason } => {
                let why = match reason {
                    Refusal::IsRoot => "it is the analysed folder itself",
                    Refusal::Outside => "it is outside the analysed folder",
                    Refusal::MountRoot => "it is where another filesystem is mounted",
                    Refusal::Missing => "it no longer exists",
                    Refusal::Symlink { .. } => {
                        "a folder on its way is a link or no longer a folder"
                    }
                    Refusal::Changed => "another entry replaced the one that was scanned",
                };
                write!(f, "refused to delete {}: {why}", path.display())
            }
            Self::Io { path, source } => {
                write!(f, "cannot delete {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for RemoveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A deletion that stopped: why, and what it had removed before stopping.
/// A refusal always comes with nothing removed.
#[derive(Debug)]
pub struct Failure {
    pub error: RemoveError,
    pub removed: Removed,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (after removing {} entries)",
            self.error, self.removed.entries
        )
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<RemoveError> for Failure {
    fn from(error: RemoveError) -> Self {
        Self {
            error,
            removed: Removed::default(),
        }
    }
}

/// Refuses `path` unless it is still the entry the scan saw there: the same
/// device and inode, read without following a link. It narrows the window in
/// which a replaced entry would be acted on to the time between this check
/// and the removal's own syscalls; it cannot close it.
///
/// # Errors
///
/// [`RemoveError::Refused`] with [`Refusal::Missing`] when nothing is there,
/// [`Refusal::Changed`] when something else is.
pub fn check_identity(path: &Path, dev: u64, ino: u64) -> Result<(), RemoveError> {
    let refuse = |reason| RemoveError::Refused {
        path: path.to_path_buf(),
        reason,
    };
    let meta = fs::symlink_metadata(path).map_err(|_| refuse(Refusal::Missing))?;
    if (meta.dev(), meta.ino()) == (dev, ino) {
        Ok(())
    } else {
        Err(refuse(Refusal::Changed))
    }
}

/// Permanently deletes `path` and everything below it, provided it lies
/// strictly inside `within` on `within`'s side of every mount.
///
/// `expected` is the device and inode the scan recorded for `path`; the
/// deletion is refused when the entry found there now is another one.
/// `None` skips that check.
///
/// # Errors
///
/// A [`Failure`] whose `error` is [`RemoveError::Refused`] before anything
/// is touched, or when a mount is met below the target;
/// [`RemoveError::Io`] naming the entry that could not be opened, listed or
/// removed; [`RemoveError::Cancelled`] when `cancel` fires. `removed` is
/// what was already gone when it stopped.
pub fn delete_tree(
    path: &Path,
    within: &Path,
    boundaries: &HashSet<PathBuf>,
    expected: Option<(u64, u64)>,
    cancel: &CancellationToken,
) -> Result<Removed, Failure> {
    let refuse = |reason| RemoveError::Refused {
        path: path.to_path_buf(),
        reason,
    };
    let target = normalise(path);
    let root = normalise(within);
    if target == root {
        return Err(refuse(Refusal::IsRoot).into());
    }
    if !target.starts_with(&root) {
        return Err(refuse(Refusal::Outside).into());
    }
    let (parent_fd, name) = open_parent(&root, &target)?;
    let stat = match statat(&parent_fd, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Err(refuse(Refusal::Missing).into()),
        Err(errno) => return Err(io_error(&target, errno).into()),
    };
    if let Some(identity) = expected {
        if (stat.st_dev, stat.st_ino) != identity {
            return Err(refuse(Refusal::Changed).into());
        }
    }
    if boundaries.contains(&target) {
        return Err(refuse(Refusal::MountRoot).into());
    }
    let parent_stat =
        fstat(&parent_fd).map_err(|errno| io_error(target.parent().unwrap_or(&root), errno))?;
    if stat.st_dev != parent_stat.st_dev {
        return Err(refuse(Refusal::MountRoot).into());
    }

    let mut walk = Removal {
        device: stat.st_dev,
        boundaries,
        cancel,
        seen: HashSet::new(),
        removed: Removed::default(),
    };
    match walk.run(parent_fd, name, &target, &stat) {
        Ok(()) => Ok(walk.removed),
        Err(error) => Err(Failure {
            error,
            removed: walk.removed,
        }),
    }
}

/// A folder being emptied: its descriptor, where it is, its name in the
/// folder above, its size before emptying, and the subfolders still to go.
struct Frame {
    fd: OwnedFd,
    path: PathBuf,
    name: OsString,
    bytes: u64,
    subfolders: Vec<OsString>,
}

struct Removal<'a> {
    device: u64,
    boundaries: &'a HashSet<PathBuf>,
    cancel: &'a CancellationToken,
    /// (device, inode) of hard-linked files already counted.
    seen: HashSet<(u64, u64)>,
    removed: Removed,
}

impl Removal<'_> {
    /// Removes `name` (at `path`, described by `stat`) from `parent`: a file
    /// or link at once, a folder after everything below it.
    fn run(
        &mut self,
        parent: OwnedFd,
        name: OsString,
        path: &Path,
        stat: &Stat,
    ) -> Result<(), RemoveError> {
        self.check_cancel()?;
        if !is_dir(stat) {
            return self.unlink_leaf(&parent, &name, path, stat);
        }
        let mut stack: Vec<Frame> = Vec::new();
        let top = self.open_folder(&parent, name, path.to_path_buf(), stat)?;
        stack.push(top);
        loop {
            let depth = stack.len();
            let Some(frame) = stack.last_mut() else { break };
            let Some(child) = frame.subfolders.pop() else {
                // Everything below is gone: the folder itself, relative to
                // the descriptor of the folder that holds it.
                let Some(done) = stack.pop() else { break };
                let holder = stack.last().map_or(&parent, |above| &above.fd);
                self.check_cancel()?;
                unlinkat(holder, done.name.as_os_str(), AtFlags::REMOVEDIR)
                    .map_err(|errno| io_error(&done.path, errno))?;
                self.count(done.bytes);
                continue;
            };
            let child_path = frame.path.join(&child);
            self.check_cancel()?;
            let child_stat = statat(&frame.fd, child.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|errno| io_error(&child_path, errno))?;
            if !is_dir(&child_stat) {
                // Listed as a folder, now something else: removed as itself.
                self.unlink_leaf(&frame.fd, &child, &child_path, &child_stat)?;
                continue;
            }
            if depth >= MAX_DEPTH {
                return Err(RemoveError::Io {
                    path: child_path,
                    source: io::Error::other(format!("deeper than {MAX_DEPTH} folders")),
                });
            }
            let opened = self.open_folder(&frame.fd, child, child_path, &child_stat)?;
            stack.push(opened);
        }
        Ok(())
    }

    /// Opens the folder `name` in `parent` after refusing a mount, removes
    /// its files and links at once, and answers the frame that still holds
    /// its subfolders.
    fn open_folder(
        &mut self,
        parent: &OwnedFd,
        name: OsString,
        path: PathBuf,
        stat: &Stat,
    ) -> Result<Frame, RemoveError> {
        if stat.st_dev != self.device || self.boundaries.contains(&path) {
            return Err(RemoveError::Refused {
                path,
                reason: Refusal::MountRoot,
            });
        }
        let fd = openat(parent, name.as_os_str(), DIR_FLAGS, Mode::empty())
            .map_err(|errno| io_error(&path, errno))?;
        let mut names = Vec::new();
        for entry in Dir::read_from(&fd).map_err(|errno| io_error(&path, errno))? {
            let entry = entry.map_err(|errno| io_error(&path, errno))?;
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            names.push(OsString::from_vec(bytes.to_vec()));
        }
        let mut subfolders = Vec::new();
        for child in names {
            self.check_cancel()?;
            let child_path = path.join(&child);
            let child_stat = statat(&fd, child.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|errno| io_error(&child_path, errno))?;
            if is_dir(&child_stat) {
                subfolders.push(child);
            } else {
                self.unlink_leaf(&fd, &child, &child_path, &child_stat)?;
            }
        }
        // Popped from the end: reversed, the listing order is kept.
        subfolders.reverse();
        Ok(Frame {
            fd,
            path,
            name,
            bytes: blocks(stat),
            subfolders,
        })
    }

    fn unlink_leaf(
        &mut self,
        parent: &OwnedFd,
        name: &OsStr,
        path: &Path,
        stat: &Stat,
    ) -> Result<(), RemoveError> {
        unlinkat(parent, name, AtFlags::empty()).map_err(|errno| io_error(path, errno))?;
        let bytes = leaf_bytes(stat, &mut self.seen);
        self.count(bytes);
        Ok(())
    }

    fn count(&mut self, bytes: u64) {
        add(&mut self.removed, bytes);
    }

    fn check_cancel(&self) -> Result<(), RemoveError> {
        if self.cancel.is_cancelled() {
            Err(RemoveError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn add(removed: &mut Removed, bytes: u64) {
    removed.entries += 1;
    removed.bytes = removed.bytes.saturating_add(bytes);
}

fn is_dir(stat: &Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::Directory
}

/// Allocated bytes (`st_blocks` counts 512-byte units).
fn blocks(stat: &Stat) -> u64 {
    u64::try_from(stat.st_blocks)
        .unwrap_or(0)
        .saturating_mul(super::walk::BLOCK_BYTES)
}

/// A file's allocated bytes, zero for a second name of an inode already
/// counted; a link or another non-folder entry counts its own blocks.
///
/// Only a file with more than one name is remembered. Removing a name lowers
/// the link count of the names left, so a later name is looked up whatever
/// its count reads now.
fn leaf_bytes(stat: &Stat, seen: &mut HashSet<(u64, u64)>) -> u64 {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return blocks(stat);
    }
    let key = (stat.st_dev, stat.st_ino);
    if seen.contains(&key) {
        return 0;
    }
    if stat.st_nlink > 1 {
        seen.insert(key);
    }
    blocks(stat)
}

fn io_error(path: &Path, errno: Errno) -> RemoveError {
    RemoveError::Io {
        path: path.to_path_buf(),
        source: io::Error::from(errno),
    }
}

/// Opens `root` and every folder from it down to `target`'s parent, each
/// from the previous descriptor without following a link, and answers the
/// parent's descriptor with `target`'s name in it.
///
/// A folder on the way that is a link, no longer a folder, or gone is a
/// [`Refusal::Symlink`] naming it: the lexical "inside" would not be where
/// the kernel goes.
fn open_parent(root: &Path, target: &Path) -> Result<(OwnedFd, OsString), RemoveError> {
    let refuse = |reason| RemoveError::Refused {
        path: target.to_path_buf(),
        reason,
    };
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(refuse(Refusal::IsRoot));
    };
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| refuse(Refusal::Outside))?;
    let open = |at: &OwnedFd, name: &OsStr, path: &Path| {
        openat(at, name, DIR_FLAGS, Mode::empty()).map_err(|errno| on_the_way(target, path, errno))
    };
    let mut fd = openat(CWD, root, DIR_FLAGS, Mode::empty())
        .map_err(|errno| on_the_way(target, root, errno))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        fd = open(&fd, component.as_os_str(), &current)?;
    }
    Ok((fd, name.to_os_string()))
}

/// A folder on the way to `target` that did not open: a refusal when it is
/// a link, not a folder or gone, an IO error naming it otherwise.
fn on_the_way(target: &Path, at: &Path, errno: Errno) -> RemoveError {
    if matches!(errno, Errno::LOOP | Errno::NOTDIR | Errno::NOENT) {
        RemoveError::Refused {
            path: target.to_path_buf(),
            reason: Refusal::Symlink {
                at: at.to_path_buf(),
            },
        }
    } else {
        io_error(at, errno)
    }
}

/// Resolves `.` and `..` without touching the filesystem, so the last
/// component is never followed. `..` above the root stays at the root.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(Component::ParentDir);
                }
            }
            other => out.push(other),
        }
    }
    out
}
