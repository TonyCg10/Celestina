//! The walk: what is under a folder, on this device, as an indexed tree.
//!
//! Iterative with an explicit stack, so a path a thousand levels deep cannot
//! overflow ours. `symlink_metadata` throughout: a symbolic link is an entry
//! with its own small size and is never followed, so a loop or a link to a
//! bigger disk cannot inflate the result. A directory on another device is
//! listed as a leaf with no size: mounts are analysed from their own root.
//! A hard link counts once per scan; only a file with more than one name is
//! remembered for that, so the set stays as small as the links it tracks. An unreadable directory is marked and
//! the walk goes on: the person gets a partial truth with the count of what
//! was refused, never a silent hole.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use super::tree::{Kind, Node, NodeId, Tree};

/// Entries between two progress reports (and two cancellation checks inside
/// one large directory).
pub const PROGRESS_EVERY: u64 = 512;
/// `st_blocks` counts 512-byte units whatever the filesystem's block size.
pub(crate) const BLOCK_BYTES: u64 = 512;

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
    /// More entries than a `NodeId` can address; the walk stops rather than
    /// alias two nodes.
    TooManyEntries {
        path: PathBuf,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the scan was cancelled"),
            Self::Root { path, source } => {
                write!(f, "cannot read the folder {}: {source}", path.display())
            }
            Self::NotADirectory { path } => write!(f, "{} is not a folder", path.display()),
            Self::TooManyEntries { path } => write!(
                f,
                "{} holds more entries than one scan can index",
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

/// Walks `root` on its own device into a [`Tree`] whose sizes are
/// aggregates of each subtree.
///
/// `boundaries` are mount targets read from `/proc/self/mountinfo`; a
/// directory at one of them (other than `root`) is a leaf like one on
/// another device.
///
/// `progress` is called every [`PROGRESS_EVERY`] entries with the running
/// totals; `cancel` is asked before every directory and at the same cadence
/// inside one, so a cancellation lands within a few hundred entries.
///
/// # Errors
///
/// [`ScanError::Root`] or [`ScanError::NotADirectory`] when the root itself
/// cannot be analysed (a symbolic link to a folder is not followed);
/// [`ScanError::Cancelled`] when `cancel` fires; nothing else stops the walk.
pub fn scan(
    root: &Path,
    boundaries: &HashSet<PathBuf>,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Tree, ScanError> {
    let meta = fs::symlink_metadata(root).map_err(|source| ScanError::Root {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotADirectory {
            path: root.to_path_buf(),
        });
    }
    let device = meta.dev();
    let root_id = NodeId(0);
    let mut nodes = vec![Node {
        name: root
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
        dev: meta.dev(),
        ino: meta.ino(),
        children: Vec::new(),
    }];
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
    let mut hard_link_names = 0u64;
    let mut stack: Vec<(NodeId, PathBuf)> = vec![(root_id, root.to_path_buf())];
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
            // An entry that vanished or cannot be stat'ed between the listing
            // and the lstat is simply not there any more.
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            entries_seen += 1;
            let kind = if meta.is_dir() {
                Kind::Dir
            } else if meta.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            let counted = kind == Kind::File
                && (meta.nlink() < 2 || seen_inodes.insert((meta.dev(), meta.ino())));
            if kind == Kind::File && !counted {
                hard_link_names += 1;
            }
            let sized = counted || kind == Kind::Other;
            let allocated = if sized {
                meta.blocks().saturating_mul(BLOCK_BYTES)
            } else {
                0
            };
            let apparent = if sized { meta.len() } else { 0 };
            // A mount is a boundary by its device number, or by its place in
            // the mount table when a bind mount or a btrfs subvolume shares
            // the device with its parent.
            let other_device =
                kind == Kind::Dir && (meta.dev() != device || boundaries.contains(&path));
            let id =
                u32::try_from(nodes.len())
                    .map(NodeId)
                    .map_err(|_| ScanError::TooManyEntries {
                        path: root.to_path_buf(),
                    })?;
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
                dev: meta.dev(),
                ino: meta.ino(),
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
        path: root.to_path_buf(),
        device,
        nodes,
        unreadable_dirs,
        hard_link_names,
    })
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
