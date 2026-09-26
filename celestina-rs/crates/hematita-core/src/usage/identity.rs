//! What an entry is, told by identity rather than by the path that led to
//! it: its device, its inode and the mount it belongs to.
//!
//! A mount is recognised by the kernel's mount id (`statx` with
//! `STATX_MNT_ID`, Linux 5.8): a bind mount or a subvolume shares the device
//! number of the folder it is mounted on, and any path text can reach it
//! through a link, but its mount id is its own. Where the kernel does not
//! report the id, `mount` is `None` and the device number and the mount
//! table are what is left to tell a mount by; where it has no `statx` at all,
//! `fstatat` answers the same fields without the id.
//!
//! Every lookup is `AT_SYMLINK_NOFOLLOW | AT_NO_AUTOMOUNT`: a link is an
//! entry of its own, and an automount point is described without being
//! mounted, as `lstat` already did.

use std::os::fd::AsFd;
use std::path::Path;

use rustix::fs::{fstat, makedev, statat, statx, AtFlags, FileType, Stat, Statx, StatxFlags, CWD};
use rustix::io::Errno;

use super::walk::BLOCK_BYTES;

const MASK: StatxFlags = StatxFlags::BASIC_STATS.union(StatxFlags::MNT_ID);
const LOOKUP: AtFlags = AtFlags::SYMLINK_NOFOLLOW.union(AtFlags::NO_AUTOMOUNT);

/// One entry as the kernel describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub dev: u64,
    pub ino: u64,
    pub kind: FileType,
    /// More than one name links to it (`st_nlink > 1`).
    pub linked: bool,
    /// Bytes on disk (`st_blocks` × 512).
    pub allocated: u64,
    /// Bytes as `st_size` reports them.
    pub size: u64,
    /// The mount id, when the kernel reports it.
    pub mount: Option<u64>,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == FileType::Directory
    }

    /// Whether `self` lies on the same mount as `other`: the same device,
    /// and the same mount id whenever both are known.
    pub fn same_mount(&self, other: &Entry) -> bool {
        self.dev == other.dev
            && match (self.mount, other.mount) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }

    fn from_stat(stat: &Stat) -> Self {
        Self {
            dev: stat.st_dev,
            ino: stat.st_ino,
            kind: FileType::from_raw_mode(stat.st_mode),
            linked: stat.st_nlink > 1,
            allocated: u64::try_from(stat.st_blocks)
                .unwrap_or(0)
                .saturating_mul(BLOCK_BYTES),
            size: u64::try_from(stat.st_size).unwrap_or(0),
            mount: None,
        }
    }
}

/// `name` in the folder `dir`, the final component never followed.
pub(crate) fn entry_at<Fd: AsFd>(dir: Fd, name: &Path) -> Result<Entry, Errno> {
    match statx(&dir, name, LOOKUP, MASK) {
        Ok(found) => Ok(from_statx(&found)),
        Err(Errno::NOSYS) => {
            statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW).map(|stat| Entry::from_stat(&stat))
        }
        Err(errno) => Err(errno),
    }
}

/// The entry at `path`, relative to the working directory for a relative
/// one; every component but the last is followed.
pub(crate) fn entry_of_path(path: &Path) -> Result<Entry, Errno> {
    entry_at(CWD, path)
}

/// The entry an open descriptor refers to.
pub(crate) fn entry_of<Fd: AsFd>(fd: Fd) -> Result<Entry, Errno> {
    match statx(&fd, "", AtFlags::EMPTY_PATH, MASK) {
        Ok(found) => Ok(from_statx(&found)),
        Err(Errno::NOSYS) => fstat(&fd).map(|stat| Entry::from_stat(&stat)),
        Err(errno) => Err(errno),
    }
}

fn from_statx(found: &Statx) -> Entry {
    Entry {
        dev: makedev(found.stx_dev_major, found.stx_dev_minor),
        ino: found.stx_ino,
        kind: FileType::from_raw_mode(u32::from(found.stx_mode)),
        linked: found.stx_nlink > 1,
        allocated: found.stx_blocks.saturating_mul(BLOCK_BYTES),
        size: found.stx_size,
        mount: StatxFlags::from_bits_retain(found.stx_mask)
            .contains(StatxFlags::MNT_ID)
            .then_some(found.stx_mnt_id),
    }
}
