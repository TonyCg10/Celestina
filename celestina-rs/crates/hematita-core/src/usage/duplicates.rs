//! Duplicate files: instant candidates by size, a verdict by content.
//!
//! `candidates` costs nothing: files of equal allocated size, straight from
//! the tree. `confirm` reads them: files are bucketed by a hash of their
//! first [`HEAD_BYTES`], then by a hash of the whole content, and the
//! survivors are compared byte for byte, so a hash collision can never make
//! two different files look the same. The hash only narrows the work; the
//! comparison is the verdict.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::hash::Hasher;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use super::tree::{Kind, NodeId, Tree};

pub const HEAD_BYTES: usize = 64 * 1024;
const CHUNK_BYTES: usize = 1024 * 1024;

/// Files of equal allocated size: possibly the same content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub size: u64,
    pub nodes: Vec<NodeId>,
}

/// Files proven byte-identical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    pub size: u64,
    pub nodes: Vec<NodeId>,
}

#[derive(Debug)]
pub enum ConfirmError {
    Cancelled,
    Read { path: PathBuf, source: io::Error },
}

impl fmt::Display for ConfirmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the content check was cancelled"),
            Self::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfirmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Cancelled => None,
        }
    }
}

/// Every size shared by two or more files with a non-zero allocation,
/// biggest size first, nodes by id.
#[must_use]
pub fn candidates(tree: &Tree) -> Vec<Group> {
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        if node.kind != Kind::File || node.allocated == 0 {
            continue;
        }
        if let Ok(index) = u32::try_from(index) {
            by_size
                .entry(node.allocated)
                .or_default()
                .push(NodeId(index));
        }
    }
    let mut groups: Vec<Group> = by_size
        .into_iter()
        .filter(|(_, nodes)| nodes.len() >= 2)
        .map(|(size, mut nodes)| {
            nodes.sort_by_key(|id| id.0);
            Group { size, nodes }
        })
        .collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.size));
    groups
}

/// Splits `group` into the sets of files proven identical; a file identical
/// to no other is left out. `progress` receives the bytes read so far.
///
/// # Errors
///
/// [`ConfirmError::Cancelled`] when `cancel` fires between two files or two
/// chunks; [`ConfirmError::Read`] naming the first file that cannot be read.
pub fn confirm(
    tree: &Tree,
    group: &Group,
    cancel: &CancellationToken,
    progress: &mut dyn FnMut(u64),
) -> Result<Vec<Verified>, ConfirmError> {
    let mut read = 0u64;
    let members: Vec<(NodeId, PathBuf)> = group
        .nodes
        .iter()
        .map(|id| (*id, tree.path_of(*id)))
        .collect();

    let mut verified = Vec::new();
    for by_head in bucket(members, cancel, |path| {
        hash_file(path, Some(HEAD_BYTES), cancel, &mut read, progress)
    })? {
        for by_whole in bucket(by_head, cancel, |path| {
            hash_file(path, None, cancel, &mut read, progress)
        })? {
            for identical in split_by_content(by_whole, cancel)? {
                verified.push(Verified {
                    size: group.size,
                    nodes: identical,
                });
            }
        }
    }
    verified.sort_by_key(|v| v.nodes.first().map_or(u32::MAX, |id| id.0));
    Ok(verified)
}

type Member = (NodeId, PathBuf);

/// Groups `members` by `key`, keeping only buckets of two or more, in the
/// order each bucket's first member appeared.
fn bucket(
    members: Vec<Member>,
    cancel: &CancellationToken,
    mut key: impl FnMut(&Path) -> Result<u64, ConfirmError>,
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
        let hash = key(&member.1)?;
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
        let mut same = vec![first.0];
        let mut different = Vec::new();
        for member in rest {
            if cancel.is_cancelled() {
                return Err(ConfirmError::Cancelled);
            }
            if same_content(&first.1, &member.1, cancel)? {
                same.push(member.0);
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

fn open(path: &Path) -> Result<File, ConfirmError> {
    File::open(path).map_err(|source| ConfirmError::Read {
        path: path.to_path_buf(),
        source,
    })
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
    path: &Path,
    limit: Option<usize>,
    cancel: &CancellationToken,
    read: &mut u64,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, ConfirmError> {
    let mut file = open(path)?;
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

fn same_content(a: &Path, b: &Path, cancel: &CancellationToken) -> Result<bool, ConfirmError> {
    let (mut fa, mut fb) = (open(a)?, open(b)?);
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
