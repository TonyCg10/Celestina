//! The walk: what is under a folder, on its own mount, as an indexed tree.
//!
//! Iterative with an explicit stack, so a path a thousand levels deep cannot
//! overflow ours. Every entry is described without following it: a symbolic
//! link is an entry with its own small size and is never followed, so a loop
//! or a link to a bigger disk cannot inflate the result. A directory on
//! another mount is listed as a leaf with no size: mounts are analysed from
//! their own root. The mount is told by the kernel's mount id, so a bind
//! mount or a subvolume on the root's own device is a boundary whatever path
//! reaches it; the device number and the mount table stay as the fallback.
//!
//! The requested root is resolved first, on the thread that scans: every
//! link on the way to it is followed once, the tree keeps the resolved path,
//! and every path the walk builds is one the mount table can name. A root
//! whose resolved path is no longer the folder, or the mount, the requested
//! path named is refused.
//!
//! A hard link counts once per scan; only a file with more than one name is
//! remembered for that, so the set stays as small as the links it tracks.
//! An unreadable directory is marked and the walk goes on: the person gets a
//! partial truth with the count of what was refused, never a silent hole.
//! The tree holds at most [`MAX_ENTRIES`] entries: a walk that meets more
//! stops with [`ScanError::TooManyEntries`] instead of growing without bound.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use rustix::fs::FileType;

use super::identity::{entry_of_path, Entry};
use super::tree::{Kind, Node, NodeId, Tree};

/// Entries between two progress reports (and two cancellation checks inside
/// one large directory).
pub const PROGRESS_EVERY: u64 = 512;
/// `st_blocks` counts 512-byte units whatever the filesystem's block size.
pub(crate) const BLOCK_BYTES: u64 = 512;
/// The most entries, the root included, one [`scan`] holds. A node costs
/// about 150 bytes with its name, so the ceiling keeps a scan of `/` near
/// 1.5 GB at worst; an ordinary system holds a few million entries.
pub const MAX_ENTRIES: usize = 10_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub files: u64,
    pub bytes: u64,
    pub current: PathBuf,
}

#[derive(Debug)]
pub enum ScanError {
    Cancelled,
    Root {
        path: PathBuf,
        source: io::Error,
    },
    NotADirectory {
        path: PathBuf,
    },
    /// More entries under `path` than the walk's ceiling, `limit`: the walk
    /// stops rather than hold an unbounded tree (or alias two `NodeId`s).
    TooManyEntries {
        path: PathBuf,
        limit: usize,
    },
}

/// Why a requested root was refused after it was resolved: the resolved path
/// is not the folder, or not on the mount, the requested one named. It is
/// the source of the [`ScanError::Root`] that carries it.
#[derive(Debug)]
pub struct RootMoved {
    pub resolved: PathBuf,
}

impl fmt::Display for RootMoved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "it now resolves to {}, another folder or another mount",
            self.resolved.display()
        )
    }
}

impl std::error::Error for RootMoved {}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the scan was cancelled"),
            Self::Root { path, source } => {
                write!(f, "cannot read the folder {}: {source}", path.display())
            }
            Self::NotADirectory { path } => write!(f, "{} is not a folder", path.display()),
            Self::TooManyEntries { path, limit } => write!(
                f,
                "{} holds more than {limit} entries, the most one scan holds",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Root { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Walks `root` on its own mount into a [`Tree`] whose sizes are
/// aggregates of each subtree, holding at most [`MAX_ENTRIES`] entries.
///
/// `boundaries` are mount targets read from `/proc/self/mountinfo`; a
/// directory at one of them (other than `root`) is a leaf like one on
/// another mount. They are compared with paths below the resolved root,
/// which is how the table names them.
///
/// `progress` is called every [`PROGRESS_EVERY`] entries with the running
/// totals; `cancel` is asked before every directory and at the same cadence
/// inside one, so a cancellation lands within a few hundred entries.
///
/// # Errors
///
/// As [`scan_bounded`] with a ceiling of [`MAX_ENTRIES`].
pub fn scan(
    root: &Path,
    boundaries: &HashSet<PathBuf>,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Tree, ScanError> {
    scan_bounded(root, boundaries, MAX_ENTRIES, cancel, progress)
}

/// [`scan`] with its own ceiling: the tree holds at most `max_entries`
/// entries, the root included.
///
/// # Errors
///
/// [`ScanError::Root`] or [`ScanError::NotADirectory`] when the root itself
/// cannot be analysed (a symbolic link to a folder is not followed, a link
/// on the way to it is), including a [`RootMoved`] source when the resolved
/// path is not the folder or the mount the requested one named;
/// [`ScanError::TooManyEntries`] when the walk meets more than `max_entries`;
/// [`ScanError::Cancelled`] when `cancel` fires; nothing else stops the walk.
pub fn scan_bounded(
    root: &Path,
    boundaries: &HashSet<PathBuf>,
    max_entries: usize,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Tree, ScanError> {
    let (resolved, top) = resolve_root(root)?;
    // A `NodeId` addresses at most `u32::MAX + 1` nodes.
    let limit = max_entries.min(u32::MAX as usize);
    let too_many = || ScanError::TooManyEntries {
        path: root.to_path_buf(),
        limit: max_entries,
    };
    if limit == 0 {
        return Err(too_many());
    }
    let root_id = NodeId(0);
    let mut nodes = vec![Node {
        name: resolved
            .file_name()
            .map_or_else(|| OsString::from("/"), OsString::from),
        kind: Kind::Dir,
        parent: None,
        allocated: 0,
        apparent: 0,
        files_below: 0,
        others_below: 0,
        unreadable: false,
        other_device: false,
        dev: top.dev,
        ino: top.ino,
        children: Vec::new(),
    }];
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    let mut hard_link_names = 0u64;
    let mut stack: Vec<(NodeId, PathBuf)> = vec![(root_id, resolved.clone())];
    let mut unreadable_dirs = 0u32;
    let mut entries_seen = 0u64;
    let mut files = 0u64;
    let mut bytes = 0u64;

    while let Some((dir_id, dir_path)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let Ok(read) = fs::read_dir(&dir_path) else {
            if let Some(dir) = nodes.get_mut(dir_id.0 as usize) {
                dir.unreadable = true;
            }
            unreadable_dirs = unreadable_dirs.saturating_add(1);
            continue;
        };
        for entry in read {
            // An entry that vanished or cannot be described between the
            // listing and the lookup is simply not there any more.
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Ok(found) = entry_of_path(&path) else {
                continue;
            };
            entries_seen += 1;
            let kind = match found.kind {
                FileType::Directory => Kind::Dir,
                FileType::RegularFile => Kind::File,
                _ => Kind::Other,
            };
            let counted =
                kind == Kind::File && (!found.linked || seen_inodes.insert((found.dev, found.ino)));
            if kind == Kind::File && !counted {
                hard_link_names += 1;
            }
            let sized = counted || kind == Kind::Other;
            let allocated = if sized { found.allocated } else { 0 };
            let apparent = if sized { found.size } else { 0 };
            // A mount is a boundary by its mount id or its device number, or
            // by its place in the mount table where the kernel reports no id.
            let other_device =
                kind == Kind::Dir && (!found.same_mount(&top) || boundaries.contains(&path));
            if nodes.len() >= limit {
                return Err(too_many());
            }
            let id = u32::try_from(nodes.len())
                .map(NodeId)
                .map_err(|_| too_many())?;
            nodes.push(Node {
                name: entry.file_name(),
                kind,
                parent: Some(dir_id),
                allocated,
                apparent,
                files_below: u64::from(counted),
                others_below: u64::from(kind == Kind::Other),
                unreadable: false,
                other_device,
                dev: found.dev,
                ino: found.ino,
                children: Vec::new(),
            });
            if let Some(dir) = nodes.get_mut(dir_id.0 as usize) {
                dir.children.push(id);
            }
            if counted {
                files += 1;
                bytes = bytes.saturating_add(allocated);
            }
            if kind == Kind::Dir && !other_device {
                stack.push((id, path));
            }
            if entries_seen % PROGRESS_EVERY == 0 {
                progress(Progress {
                    files,
                    bytes,
                    current: dir_path.clone(),
                });
                if cancel.is_cancelled() {
                    return Err(ScanError::Cancelled);
                }
            }
        }
    }
    aggregate(&mut nodes);
    Ok(Tree {
        root: root_id,
        path: resolved,
        device: top.dev,
        nodes,
        unreadable_dirs,
        hard_link_names,
    })
}

/// Resolves the requested `root` to the path of the folder it names, every
/// link on the way followed and the last component never, and answers that
/// path with the folder's entry.
///
/// The requested path and the resolved one are described separately; they
/// must be the same folder on the same mount, or a component changed
/// between the two lookups and the root is refused with [`RootMoved`].
fn resolve_root(root: &Path) -> Result<(PathBuf, Entry), ScanError> {
    let refuse = |source: io::Error| ScanError::Root {
        path: root.to_path_buf(),
        source,
    };
    let named = entry_of_path(root).map_err(|errno| refuse(errno.into()))?;
    if !named.is_dir() {
        return Err(ScanError::NotADirectory {
            path: root.to_path_buf(),
        });
    }
    let resolved = fs::canonicalize(root).map_err(refuse)?;
    let found = entry_of_path(&resolved).map_err(|errno| refuse(errno.into()))?;
    if (found.dev, found.ino) != (named.dev, named.ino) || found.mount != named.mount {
        return Err(refuse(io::Error::other(RootMoved { resolved })));
    }
    Ok((resolved, found))
}

/// [`scan`] without progress: the fresh subtree a graft puts in place of
/// one a stopped deletion left stale.
///
/// # Errors
///
/// As [`scan`].
pub fn scan_subtree(
    root: &Path,
    boundaries: &HashSet<PathBuf>,
    cancel: &CancellationToken,
) -> Result<Tree, ScanError> {
    scan(root, boundaries, cancel, &mut |_| {})
}

/// Adds every node's sizes into its parent's, bottom-up.
///
/// A child is always pushed after its parent, so walking the arena backwards
/// visits every child before the parent it contributes to.
fn aggregate(nodes: &mut [Node]) {
    for index in (1..nodes.len()).rev() {
        let (allocated, apparent, files, others, parent) = {
            let node = &nodes[index];
            (
                node.allocated,
                node.apparent,
                node.files_below,
                node.others_below,
                node.parent,
            )
        };
        if let Some(parent) = parent.and_then(|p| nodes.get_mut(p.0 as usize)) {
            parent.allocated = parent.allocated.saturating_add(allocated);
            parent.apparent = parent.apparent.saturating_add(apparent);
            parent.files_below = parent.files_below.saturating_add(files);
            parent.others_below = parent.others_below.saturating_add(others);
        }
    }
}
