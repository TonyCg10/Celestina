//! Duplicate files: instant candidates by size, a verdict by content.
//!
//! `candidates` costs nothing: files of equal size (`st_size`), straight from
//! the tree; equal content implies an equal size, whatever the blocks each
//! copy allocates. `confirm` reads them: files are bucketed by a hash of
//! their first [`HEAD_BYTES`], then by a hash of the whole content, and the
//! survivors are compared byte for byte, so a hash collision can never make
//! two different files look the same. The hash only narrows the work; the
//! comparison is the verdict.
//!
//! The check reads from a [`Members`] list resolved from the tree first, so
//! it holds no reference to the tree while it reads. Every member is opened
//! without following a link and without blocking (`O_NOFOLLOW | O_NONBLOCK`)
//! and read only when the descriptor is a regular file with the device and
//! inode the scan recorded: a link, a FIFO or a device put in a file's place
//! is [`ConfirmError::Changed`], never read and never waited on.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::hash::Hasher;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use rustix::fs::{open, FileType, Mode, OFlags};
use rustix::io::Errno;

use super::identity::{entry_of, entry_of_path};
use super::tree::{Kind, NodeId, Tree};

pub const HEAD_BYTES: usize = 64 * 1024;
const CHUNK_BYTES: usize = 1024 * 1024;

/// A member is opened read-only, never through a link, never waiting for a
/// writer (a FIFO) or a device.
const READ_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

/// Files of equal size: possibly the same content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The `st_size` every member shares, never zero.
    pub size: u64,
    pub nodes: Vec<NodeId>,
}

/// Files proven byte-identical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    pub size: u64,
    pub nodes: Vec<NodeId>,
}

/// One file of a group as the scan saw it: where it is and which entry it
/// was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: NodeId,
    pub path: PathBuf,
    pub dev: u64,
    pub ino: u64,
}

/// A group resolved against the tree: what [`confirm`] reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Members {
    pub size: u64,
    pub files: Vec<Member>,
}

#[derive(Debug)]
pub enum ConfirmError {
    Cancelled,
    Read {
        path: PathBuf,
        source: io::Error,
    },
    /// The entry at `path` is no longer the regular file the scan recorded:
    /// a link, a FIFO, a device or another file took its place.
    Changed {
        path: PathBuf,
    },
}

impl fmt::Display for ConfirmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the content check was cancelled"),
            Self::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            Self::Changed { path } => write!(
                f,
                "{} is no longer the file that was scanned",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ConfirmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Cancelled | Self::Changed { .. } => None,
        }
    }
}

/// Every size shared by two or more files of non-zero size, the groups that
/// free the most first: the bytes allocated by every copy but the largest,
/// then the bigger size, then the lower first id. Nodes by id.
#[must_use]
pub fn candidates(tree: &Tree) -> Vec<Group> {
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        // A second name of a hard-linked file carries no size in the tree.
        if node.kind != Kind::File || node.apparent == 0 {
            continue;
        }
        if let Ok(index) = u32::try_from(index) {
            by_size
                .entry(node.apparent)
                .or_default()
                .push(NodeId(index));
        }
    }
    let freed = |nodes: &[NodeId]| {
        let sizes = nodes
            .iter()
            .filter_map(|id| tree.node(*id).map(|node| node.allocated));
        let (total, largest) = sizes.fold((0u64, 0u64), |(total, largest), bytes| {
            (total.saturating_add(bytes), largest.max(bytes))
        });
        total.saturating_sub(largest)
    };
    let mut groups: Vec<(u64, Group)> = by_size
        .into_iter()
        .filter(|(_, nodes)| nodes.len() >= 2)
        .map(|(size, mut nodes)| {
            nodes.sort_by_key(|id| id.0);
            (freed(&nodes), Group { size, nodes })
        })
        .collect();
    groups.sort_by(|(a_freed, a), (b_freed, b)| {
        b_freed
            .cmp(a_freed)
            .then(b.size.cmp(&a.size))
            .then_with(|| {
                a.nodes
                    .first()
                    .map(|id| id.0)
                    .cmp(&b.nodes.first().map(|id| id.0))
            })
    });
    groups.into_iter().map(|(_, group)| group).collect()
}

/// `group` resolved against `tree`: each member's path and recorded
/// identity. An id the tree does not hold is left out.
#[must_use]
pub fn members(tree: &Tree, group: &Group) -> Members {
    Members {
        size: group.size,
        files: group
            .nodes
            .iter()
            .filter_map(|id| {
                tree.node(*id).map(|node| Member {
                    id: *id,
                    path: tree.path_of(*id),
                    dev: node.dev,
                    ino: node.ino,
                })
            })
            .collect(),
    }
}

/// Splits `members` into the sets of files proven identical; a file
/// identical to no other is left out. `progress` receives the bytes read so
/// far.
///
/// # Errors
///
/// [`ConfirmError::Cancelled`] when `cancel` fires between two files or two
/// chunks; [`ConfirmError::Read`] naming the first file that cannot be read;
/// [`ConfirmError::Changed`] naming the first that is no longer the regular
/// file the scan recorded.
pub fn confirm(
    members: Members,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(u64),
) -> Result<Vec<Verified>, ConfirmError> {
    let mut read = 0u64;
    let size = members.size;
    let mut verified = Vec::new();
    for by_head in bucket(members.files, cancel, |member| {
        hash_file(member, Some(HEAD_BYTES), cancel, &mut read, progress)
    })? {
        for by_whole in bucket(by_head, cancel, |member| {
            hash_file(member, None, cancel, &mut read, progress)
        })? {
            for identical in split_by_content(by_whole, cancel)? {
                verified.push(Verified {
                    size,
                    nodes: identical,
                });
            }
        }
    }
    verified.sort_by_key(|v| v.nodes.first().map_or(u32::MAX, |id| id.0));
    Ok(verified)
}

/// Groups `members` by `key`, keeping only buckets of two or more, in the
/// order each bucket's first member appeared.
fn bucket(
    members: Vec<Member>,
    cancel: &CancellationToken,
    mut key: impl FnMut(&Member) -> Result<u64, ConfirmError>,
) -> Result<Vec<Vec<Member>>, ConfirmError> {
    if members.len() < 2 {
        return Ok(Vec::new());
    }
    let mut order: Vec<u64> = Vec::new();
    let mut buckets: HashMap<u64, Vec<Member>> = HashMap::new();
    for member in members {
        if cancel.is_cancelled() {
            return Err(ConfirmError::Cancelled);
        }
        let hash = key(&member)?;
        let entry = buckets.entry(hash).or_default();
        if entry.is_empty() {
            order.push(hash);
        }
        entry.push(member);
    }
    Ok(order
        .into_iter()
        .filter_map(|hash| buckets.remove(&hash))
        .filter(|b| b.len() >= 2)
        .collect())
}

/// Splits one hash bucket into sets whose bytes are equal to their first
/// member's; singletons are dropped.
fn split_by_content(
    mut rest: Vec<Member>,
    cancel: &CancellationToken,
) -> Result<Vec<Vec<NodeId>>, ConfirmError> {
    let mut sets = Vec::new();
    while rest.len() >= 2 {
        let first = rest.remove(0);
        let mut same = vec![first.id];
        let mut different = Vec::new();
        for member in rest {
            if cancel.is_cancelled() {
                return Err(ConfirmError::Cancelled);
            }
            if same_content(&first, &member, cancel)? {
                same.push(member.id);
            } else {
                different.push(member);
            }
        }
        if same.len() >= 2 {
            same.sort_by_key(|id| id.0);
            sets.push(same);
        }
        rest = different;
    }
    Ok(sets)
}

/// Opens `member` for reading only when it is still the regular file the
/// scan recorded. It is looked up first, so a device or a FIFO in its place
/// is refused without being opened at all; the descriptor is checked again,
/// because the entry can change between the lookup and the open.
fn open_member(member: &Member) -> Result<File, ConfirmError> {
    let path = &member.path;
    let changed = || ConfirmError::Changed { path: path.clone() };
    let failed = |errno: Errno| ConfirmError::Read {
        path: path.clone(),
        source: errno.into(),
    };
    let is_recorded = |kind: FileType, dev: u64, ino: u64| {
        kind == FileType::RegularFile && (dev, ino) == (member.dev, member.ino)
    };
    let listed = entry_of_path(path).map_err(failed)?;
    if !is_recorded(listed.kind, listed.dev, listed.ino) {
        return Err(changed());
    }
    let fd = open(path, READ_FLAGS, Mode::empty()).map_err(|errno| match errno {
        // O_NOFOLLOW met a link.
        Errno::LOOP => changed(),
        other => failed(other),
    })?;
    let opened = entry_of(&fd).map_err(failed)?;
    if !is_recorded(opened.kind, opened.dev, opened.ino) {
        return Err(changed());
    }
    Ok(File::from(fd))
}

/// Fills `buf` as far as the file allows; answers how many bytes were read.
fn read_full(file: &mut File, buf: &mut [u8], path: &Path) -> Result<usize, ConfirmError> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => {
                return Err(ConfirmError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(filled)
}

fn hash_file(
    member: &Member,
    limit: Option<usize>,
    cancel: &CancellationToken,
    read: &mut u64,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, ConfirmError> {
    let path = member.path.as_path();
    let mut file = open_member(member)?;
    let mut hasher = DefaultHasher::new();
    let mut buf = vec![0u8; limit.unwrap_or(CHUNK_BYTES).min(CHUNK_BYTES)];
    let mut remaining = limit;
    loop {
        if cancel.is_cancelled() {
            return Err(ConfirmError::Cancelled);
        }
        let want = remaining.map_or(buf.len(), |r| r.min(buf.len()));
        if want == 0 {
            break;
        }
        let n = read_full(&mut file, &mut buf[..want], path)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
        *read = read.saturating_add(n as u64);
        progress(*read);
        if let Some(r) = remaining.as_mut() {
            *r -= n;
        }
        if n < want {
            break;
        }
    }
    Ok(hasher.finish())
}

fn same_content(a: &Member, b: &Member, cancel: &CancellationToken) -> Result<bool, ConfirmError> {
    let (mut fa, mut fb) = (open_member(a)?, open_member(b)?);
    let (a, b) = (a.path.as_path(), b.path.as_path());
    let mut ba = vec![0u8; CHUNK_BYTES];
    let mut bb = vec![0u8; CHUNK_BYTES];
    loop {
        if cancel.is_cancelled() {
            return Err(ConfirmError::Cancelled);
        }
        let na = read_full(&mut fa, &mut ba, a)?;
        let nb = read_full(&mut fb, &mut bb, b)?;
        if na != nb || ba[..na] != bb[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}
