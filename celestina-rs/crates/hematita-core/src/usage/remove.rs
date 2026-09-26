//! Permanent deletion: the suite's only one, and the one place it is spelled.
//!
//! [`delete_tree`] refuses before touching anything: the scanned root itself,
//! a path outside it (after resolving `.` and `..` lexically, because
//! resolving links would follow the very link the person asked to delete),
//! a folder on the way that is a link or no longer a folder, a mount root, a
//! path that is gone, or an entry that is not the one the scan recorded.
//!
//! It works on directory descriptors, never on re-resolved paths: the
//! analysed folder is resolved once (every link on the way to it followed,
//! as the walk did, and refused unless the resolved path is still the folder
//! and the mount the given one names), opened with
//! `O_DIRECTORY | O_NOFOLLOW`, every folder down to the target is opened the
//! same way from the previous descriptor, and every entry is removed with
//! `unlinkat` relative to the descriptor of the folder that holds it. A
//! folder swapped for a symbolic link after it was opened cannot redirect
//! the removal, because nothing is looked up by path again. The one thing a
//! descriptor cannot pin is the analysed folder itself being renamed while
//! the deletion runs: the removal then continues inside the folder it
//! opened, wherever that now lives.
//!
//! Mounts are told by identity: every folder on the way, the target and
//! every entry below it must lie on the analysed folder's mount, by the
//! kernel's mount id and by device number, and a path the mount table lists
//! (compared below the resolved folder, which is how the table names it) is
//! refused as well. A bind mount on the same device is therefore refused
//! whatever path reaches it.
//!
//! Before removing anything it walks the whole subtree read-only, the same
//! descent the removal will take, and refuses if any entry in it is on
//! another mount, or if the tree is deeper than [`MAX_DEPTH`]: nothing is
//! touched if any inner folder is a mount. The removal pass repeats every
//! check as the defence against a change between the two passes. Every
//! folder is checked again after it is opened: the descriptor's device,
//! inode and mount must be the ones listed.
//!
//! Removal is depth-first, a folder's files before its subfolders and every
//! folder after its content, asking `cancel` before every entry; a symbolic
//! link is removed as itself and its target is never visited. A deletion
//! that stops midway — cancelled, refused or failed — reports what it had
//! already removed in [`Failure`].

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};

use celestina_core::CancellationToken;
use rustix::fs::{openat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags, CWD};
use rustix::io::Errno;

use super::identity::{entry_at, entry_of, entry_of_path, Entry};

/// Every folder is opened read-only as a directory, never through a link.
const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
/// The folder holding an entry [`check_identity`] checks, reached as its
/// path leads, links included.
const HOLDER_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::CLOEXEC);

/// Folders open at once below the target: one descriptor per level is what
/// makes the walk safe, so the depth is bounded rather than the descriptors
/// shared. A deeper tree fails closed with [`RemoveError::Io`] in the
/// read-only pass, before anything is removed and well before the default
/// limit of 1024 open descriptors could be reached.
pub const MAX_DEPTH: usize = 256;

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
    /// The analysed folder's path now resolves to another folder, or onto
    /// another mount, than the one it names.
    RootMoved,
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
                    Refusal::RootMoved => {
                        "the analysed folder now resolves to another folder or mount"
                    }
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

/// Refuses `path` unless it is still the entry the scan saw there — the
/// same device and inode, read without following a link — and unless it is
/// on the same mount as the folder that holds it (by the kernel's mount id
/// and by device number), so a mount root is refused whatever path names it.
/// It narrows the window in which a replaced entry would be acted on to the
/// time between this check and the operation's own syscalls; it cannot
/// close it.
///
/// # Errors
///
/// [`RemoveError::Refused`] with [`Refusal::Missing`] when nothing is there,
/// [`Refusal::Changed`] when something else is, [`Refusal::MountRoot`] when
/// it is where a filesystem is mounted.
pub fn check_identity(path: &Path, dev: u64, ino: u64) -> Result<(), RemoveError> {
    let refuse = |reason| RemoveError::Refused {
        path: path.to_path_buf(),
        reason,
    };
    let Some(name) = path.file_name() else {
        // `/`, or a path ending in `..`: the root of a mount, or not a name
        // an entry can be checked by.
        return Err(refuse(Refusal::MountRoot));
    };
    let holder_path = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    // The folder that holds the entry, followed if it is reached through a
    // link, and the entry looked up in that very folder.
    let holder = openat(CWD, holder_path, HOLDER_FLAGS, Mode::empty())
        .map_err(|_| refuse(Refusal::Missing))?;
    let holder_entry = entry_of(&holder).map_err(|_| refuse(Refusal::Missing))?;
    let entry = entry_at(&holder, Path::new(name)).map_err(|_| refuse(Refusal::Missing))?;
    if (entry.dev, entry.ino) != (dev, ino) {
        return Err(refuse(Refusal::Changed));
    }
    if entry.same_mount(&holder_entry) {
        Ok(())
    } else {
        Err(refuse(Refusal::MountRoot))
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
    let Ok(relative) = target.strip_prefix(&root) else {
        return Err(refuse(Refusal::Outside).into());
    };
    let base = open_root(&root, path)?;
    let target = base.path.join(relative);
    let (parent_fd, name) = base.open_parent(&target, boundaries)?;
    let entry = match entry_at(&parent_fd, Path::new(&name)) {
        Ok(entry) => entry,
        Err(Errno::NOENT) => return Err(refuse(Refusal::Missing).into()),
        Err(errno) => return Err(io_error(&target, errno).into()),
    };
    if let Some(identity) = expected {
        if (entry.dev, entry.ino) != identity {
            return Err(refuse(Refusal::Changed).into());
        }
    }
    if boundaries.contains(&target) || !entry.same_mount(&base.entry) {
        return Err(refuse(Refusal::MountRoot).into());
    }

    let mut check = Removal {
        root: base.entry,
        boundaries,
        cancel,
        dry: true,
        seen: HashSet::new(),
        removed: Removed::default(),
    };
    check.run(&parent_fd, name.clone(), &target, &entry)?;
    let mut walk = Removal {
        dry: false,
        ..check
    };
    walk.removed = Removed::default();
    match walk.run(&parent_fd, name, &target, &entry) {
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
    /// The analysed folder: every entry removed must lie on its mount.
    root: Entry,
    boundaries: &'a HashSet<PathBuf>,
    cancel: &'a CancellationToken,
    /// The read-only pass: the same descent and checks, nothing removed.
    dry: bool,
    /// (device, inode) of hard-linked files already counted.
    seen: HashSet<(u64, u64)>,
    removed: Removed,
}

impl Removal<'_> {
    /// Removes `name` (at `path`, described by `entry`) from `parent`: a
    /// file or link at once, a folder after everything below it.
    fn run(
        &mut self,
        parent: &OwnedFd,
        name: OsString,
        path: &Path,
        entry: &Entry,
    ) -> Result<(), RemoveError> {
        self.check_cancel()?;
        if !entry.is_dir() {
            return self.unlink_leaf(parent, &name, path, entry);
        }
        let mut stack: Vec<Frame> = Vec::new();
        let top = self.open_folder(parent, name, path.to_path_buf(), entry)?;
        stack.push(top);
        loop {
            let depth = stack.len();
            let Some(frame) = stack.last_mut() else { break };
            let Some(child) = frame.subfolders.pop() else {
                // Everything below is gone: the folder itself, relative to
                // the descriptor of the folder that holds it.
                let Some(done) = stack.pop() else { break };
                let holder = stack.last().map_or(parent, |above| &above.fd);
                self.check_cancel()?;
                if !self.dry {
                    unlinkat(holder, done.name.as_os_str(), AtFlags::REMOVEDIR)
                        .map_err(|errno| io_error(&done.path, errno))?;
                    self.count(done.bytes);
                }
                continue;
            };
            let child_path = frame.path.join(&child);
            self.check_cancel()?;
            let child_entry = entry_at(&frame.fd, Path::new(&child))
                .map_err(|errno| io_error(&child_path, errno))?;
            if !child_entry.is_dir() {
                // Listed as a folder, now something else: removed as itself.
                self.unlink_leaf(&frame.fd, &child, &child_path, &child_entry)?;
                continue;
            }
            if depth >= MAX_DEPTH {
                return Err(RemoveError::Io {
                    path: child_path,
                    source: io::Error::other(format!("deeper than {MAX_DEPTH} folders")),
                });
            }
            let opened = self.open_folder(&frame.fd, child, child_path, &child_entry)?;
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
        entry: &Entry,
    ) -> Result<Frame, RemoveError> {
        if !entry.same_mount(&self.root) || self.boundaries.contains(&path) {
            return Err(mount_root(path));
        }
        let (fd, opened) = open_checked(parent, name.as_os_str(), &path, entry)?;
        if !opened.same_mount(&self.root) {
            return Err(mount_root(path));
        }
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
            let child_entry =
                entry_at(&fd, Path::new(&child)).map_err(|errno| io_error(&child_path, errno))?;
            if child_entry.is_dir() {
                subfolders.push(child);
            } else {
                self.unlink_leaf(&fd, &child, &child_path, &child_entry)?;
            }
        }
        // Popped from the end: reversed, the listing order is kept.
        subfolders.reverse();
        Ok(Frame {
            fd,
            path,
            name,
            bytes: entry.allocated,
            subfolders,
        })
    }

    /// Removes a file, a link or another non-folder entry; one on another
    /// mount (a file bind-mounted in place) is refused in both passes.
    fn unlink_leaf(
        &mut self,
        parent: &OwnedFd,
        name: &OsStr,
        path: &Path,
        entry: &Entry,
    ) -> Result<(), RemoveError> {
        if !entry.same_mount(&self.root) {
            return Err(mount_root(path.to_path_buf()));
        }
        if self.dry {
            return Ok(());
        }
        unlinkat(parent, name, AtFlags::empty()).map_err(|errno| io_error(path, errno))?;
        let bytes = leaf_bytes(entry, &mut self.seen);
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

fn mount_root(path: PathBuf) -> RemoveError {
    RemoveError::Refused {
        path,
        reason: Refusal::MountRoot,
    }
}

/// A file's allocated bytes, zero for a second name of an inode already
/// counted; a link or another non-folder entry counts its own blocks.
///
/// Only a file with more than one name is remembered. Removing a name lowers
/// the link count of the names left, so a later name is looked up whatever
/// its count reads now.
fn leaf_bytes(entry: &Entry, seen: &mut HashSet<(u64, u64)>) -> u64 {
    if entry.kind != FileType::RegularFile {
        return entry.allocated;
    }
    let key = (entry.dev, entry.ino);
    if seen.contains(&key) {
        return 0;
    }
    if entry.linked {
        seen.insert(key);
    }
    entry.allocated
}

/// Opens the folder `name` in `parent` without following a link and refuses
/// it as [`Refusal::Changed`] unless the opened descriptor is the folder
/// `listed` was described as (device and inode): the lookup made before the
/// open cannot see a swap between the two calls. Answers the descriptor with
/// the entry it refers to.
fn open_checked<Fd: AsFd>(
    parent: Fd,
    name: &OsStr,
    path: &Path,
    listed: &Entry,
) -> Result<(OwnedFd, Entry), RemoveError> {
    let fd =
        openat(parent, name, DIR_FLAGS, Mode::empty()).map_err(|errno| io_error(path, errno))?;
    let opened = entry_of(&fd).map_err(|errno| io_error(path, errno))?;
    if (opened.dev, opened.ino) == (listed.dev, listed.ino) {
        Ok((fd, opened))
    } else {
        Err(RemoveError::Refused {
            path: path.to_path_buf(),
            reason: Refusal::Changed,
        })
    }
}

fn io_error(path: &Path, errno: Errno) -> RemoveError {
    RemoveError::Io {
        path: path.to_path_buf(),
        source: io::Error::from(errno),
    }
}

/// The analysed folder, resolved and open: its descriptor, its resolved path
/// and its entry, whose mount every removal must stay on.
struct Base {
    fd: OwnedFd,
    path: PathBuf,
    entry: Entry,
}

/// Resolves `root` (every link on the way followed; the last component a
/// real folder, as the walk requires) and opens it without following a
/// link. The folder opened must be the one `root` names, on the same mount,
/// or the deletion is refused as [`Refusal::RootMoved`]: a component on the
/// way changed between the lookups. Refusals name `target`, the path asked.
fn open_root(root: &Path, target: &Path) -> Result<Base, RemoveError> {
    let refuse = |reason| RemoveError::Refused {
        path: target.to_path_buf(),
        reason,
    };
    let named = entry_of_path(root).map_err(|errno| on_the_way(target, root, errno))?;
    if !named.is_dir() {
        return Err(refuse(Refusal::Symlink {
            at: root.to_path_buf(),
        }));
    }
    let path = fs::canonicalize(root).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => refuse(Refusal::Missing),
        _ => RemoveError::Io {
            path: root.to_path_buf(),
            source: error,
        },
    })?;
    let fd = openat(CWD, &path, DIR_FLAGS, Mode::empty())
        .map_err(|errno| on_the_way(target, &path, errno))?;
    let entry = entry_of(&fd).map_err(|errno| io_error(&path, errno))?;
    if (entry.dev, entry.ino) != (named.dev, named.ino) || entry.mount != named.mount {
        return Err(refuse(Refusal::RootMoved));
    }
    Ok(Base { fd, path, entry })
}

impl Base {
    /// Opens every folder from the analysed one down to `target`'s parent,
    /// each from the previous descriptor without following a link, and
    /// answers the parent's descriptor with `target`'s name in it.
    ///
    /// A folder on the way that is a link or no longer a folder is a
    /// [`Refusal::Symlink`] naming it: the lexical "inside" would not be
    /// where the kernel goes. One that is gone is [`Refusal::Missing`]. One
    /// on another mount, or listed in `boundaries`, is a
    /// [`Refusal::MountRoot`] naming it.
    fn open_parent(
        &self,
        target: &Path,
        boundaries: &HashSet<PathBuf>,
    ) -> Result<(OwnedFd, OsString), RemoveError> {
        let refuse = |reason| RemoveError::Refused {
            path: target.to_path_buf(),
            reason,
        };
        let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
            return Err(refuse(Refusal::IsRoot));
        };
        let relative = parent
            .strip_prefix(&self.path)
            .map_err(|_| refuse(Refusal::Outside))?;
        let mut fd = self.fd.try_clone().map_err(|source| RemoveError::Io {
            path: self.path.clone(),
            source,
        })?;
        let mut current = self.path.clone();
        for component in relative.components() {
            current.push(component);
            fd = openat(&fd, component.as_os_str(), DIR_FLAGS, Mode::empty())
                .map_err(|errno| on_the_way(target, &current, errno))?;
            let on_the_way = entry_of(&fd).map_err(|errno| io_error(&current, errno))?;
            if !on_the_way.same_mount(&self.entry) || boundaries.contains(&current) {
                return Err(mount_root(current));
            }
        }
        Ok((fd, name.to_os_string()))
    }
}

/// A folder on the way to `target` that did not open: [`Refusal::Missing`]
/// when it is gone, [`Refusal::Symlink`] when it is a link or not a folder,
/// an IO error naming it otherwise.
fn on_the_way(target: &Path, at: &Path, errno: Errno) -> RemoveError {
    if errno == Errno::NOENT {
        return RemoveError::Refused {
            path: target.to_path_buf(),
            reason: Refusal::Missing,
        };
    }
    if matches!(errno, Errno::LOOP | Errno::NOTDIR) {
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

#[cfg(test)]
mod tests {
    use super::{entry_of, entry_of_path, open_checked, Entry, Refusal, RemoveError, CWD};
    use std::path::Path;

    #[test]
    fn an_opened_folder_is_checked_against_the_identity_it_was_listed_with() {
        let dir = std::env::temp_dir()
            .canonicalize()
            .expect("temporary folder");
        let listed = entry_of_path(&dir).expect("entry");
        let (fd, opened) =
            open_checked(CWD, dir.as_os_str(), &dir, &listed).expect("same identity");
        assert_eq!(entry_of(&fd).expect("entry").ino, listed.ino);
        assert!(opened.same_mount(&listed));
        let replaced = Entry {
            ino: listed.ino.wrapping_add(1),
            ..listed
        };
        let other = open_checked(CWD, dir.as_os_str(), Path::new("/x"), &replaced);
        assert!(matches!(
            other,
            Err(RemoveError::Refused {
                reason: Refusal::Changed,
                ..
            })
        ));
    }
}
