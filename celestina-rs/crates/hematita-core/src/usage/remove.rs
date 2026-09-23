//! Permanent deletion: the suite's only one, and the one place it is spelled.
//!
//! [`delete_tree`] refuses before touching anything: the scanned root itself,
//! a path outside it (after resolving `.` and `..` lexically, because
//! resolving links would follow the very link the person asked to delete),
//! a mount root, or a path that is gone. It then lists the whole subtree
//! without following a symbolic link and refuses again if any directory in
//! it is another device's mount. Only then does it remove, children before
//! parents, asking `cancel` before every entry; a symbolic link is removed as
//! itself and its target is never visited.

use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use celestina_core::CancellationToken;

/// What a completed deletion removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    pub entries: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    IsRoot,
    Outside,
    MountRoot,
    Missing,
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

/// Permanently deletes `path` and everything below it, provided it lies
/// strictly inside `within` on `within`'s side of every mount.
///
/// # Errors
///
/// [`RemoveError::Refused`] before anything is touched; [`RemoveError::Io`]
/// naming the entry that could not be listed or removed;
/// [`RemoveError::Cancelled`] when `cancel` fires, with what was already
/// removed gone and nothing after the check touched.
pub fn delete_tree(
    path: &Path,
    within: &Path,
    cancel: &CancellationToken,
) -> Result<Removed, RemoveError> {
    let refuse = |reason| RemoveError::Refused {
        path: path.to_path_buf(),
        reason,
    };
    let target = normalise(path);
    let root = normalise(within);
    if target == root {
        return Err(refuse(Refusal::IsRoot));
    }
    if !target.starts_with(&root) {
        return Err(refuse(Refusal::Outside));
    }
    let Ok(meta) = fs::symlink_metadata(&target) else {
        return Err(refuse(Refusal::Missing));
    };
    let parent = target.parent().ok_or_else(|| refuse(Refusal::IsRoot))?;
    let parent_meta = fs::symlink_metadata(parent).map_err(|source| RemoveError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    if meta.dev() != parent_meta.dev() {
        return Err(refuse(Refusal::MountRoot));
    }

    let plan = list_post_order(&target, meta, cancel)?;

    let mut removed = Removed {
        entries: 0,
        bytes: 0,
    };
    for entry in &plan.entries {
        if cancel.is_cancelled() {
            return Err(RemoveError::Cancelled);
        }
        let result = if entry.is_dir {
            fs::remove_dir(&entry.path)
        } else {
            fs::remove_file(&entry.path)
        };
        result.map_err(|source| RemoveError::Io {
            path: entry.path.clone(),
            source,
        })?;
        removed.entries += 1;
        removed.bytes = removed.bytes.saturating_add(entry.bytes);
    }
    Ok(removed)
}

struct Entry {
    path: PathBuf,
    is_dir: bool,
    /// Allocated bytes, zero for a second name of an inode already counted.
    bytes: u64,
}

struct Plan {
    entries: Vec<Entry>,
}

/// Every entry under `top` (itself included), children before their parent,
/// without following a link; refuses when a directory below is another
/// device's mount.
fn list_post_order(
    top: &Path,
    top_meta: fs::Metadata,
    cancel: &CancellationToken,
) -> Result<Plan, RemoveError> {
    let device = top_meta.dev();
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();
    // (path, metadata, children already pushed)
    let mut stack = vec![(top.to_path_buf(), top_meta, false)];
    while let Some((path, meta, expanded)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(RemoveError::Cancelled);
        }
        let is_dir = meta.is_dir();
        if !is_dir || expanded {
            let first =
                !meta.is_file() || meta.nlink() < 2 || seen.insert((meta.dev(), meta.ino()));
            let bytes = if first {
                meta.blocks().saturating_mul(super::walk::BLOCK_BYTES)
            } else {
                0
            };
            entries.push(Entry {
                path,
                is_dir,
                bytes,
            });
            continue;
        }
        if meta.dev() != device {
            return Err(RemoveError::Refused {
                path,
                reason: Refusal::MountRoot,
            });
        }
        let read = fs::read_dir(&path).map_err(|source| RemoveError::Io {
            path: path.clone(),
            source,
        })?;
        stack.push((path.clone(), meta, true));
        for item in read {
            let item = item.map_err(|source| RemoveError::Io {
                path: path.clone(),
                source,
            })?;
            let child = item.path();
            let child_meta = fs::symlink_metadata(&child).map_err(|source| RemoveError::Io {
                path: child.clone(),
                source,
            })?;
            stack.push((child, child_meta, false));
        }
    }
    Ok(Plan { entries })
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
