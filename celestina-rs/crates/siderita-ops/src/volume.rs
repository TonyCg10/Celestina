//! Which Trash an entry belongs in.
//!
//! The freedesktop spec puts one Trash in the home directory and one on every
//! other volume, and the difference is not bookkeeping: the home Trash lives on
//! the home filesystem, so trashing a file from another disk into it is a
//! **copy of every byte** onto a disk the person did not choose. A 40 GB folder
//! deleted from an external drive would fill the system disk and take as long as
//! the copy it really is; the same delete into the drive's own Trash is a rename
//! that finishes instantly and leaves the bytes where they were.
//!
//! So the volume is found first, and only an entry that shares the home's
//! filesystem uses the home Trash.
//!
//! The spec offers two homes on a volume, and both are honoured:
//! `$topdir/.Trash/$uid` when the administrator has created a sticky, non-symlink
//! `$topdir/.Trash` for everyone to share, and `$topdir/.Trash-$uid` otherwise —
//! which this crate creates when it is missing, since that is the one a person
//! may make for themselves.

use std::io;
use std::path::{Path, PathBuf};

use crate::error::OpError;

/// The Trash `source` belongs in, and the directory its recorded paths are
/// relative to when the spec allows a relative record.
pub(crate) struct TrashHome {
    pub(crate) root: PathBuf,
    /// The volume's top directory, or `None` for the home Trash.
    pub(crate) top: Option<PathBuf>,
}

/// Chooses the Trash for `source`: the home one when they share a filesystem,
/// the volume's own otherwise.
///
/// A volume whose Trash cannot be created — a read-only mount, a directory the
/// person cannot write — falls back to the home Trash, because a delete that
/// costs a copy is still better than a delete that fails.
pub(crate) fn trash_home_for(source: &Path) -> Result<TrashHome, OpError> {
    let home = crate::trash::home_trash()?;
    let source_device = device_of(source);
    let home_device = device_of(existing_ancestor(&home).as_deref().unwrap_or(&home));
    if source_device.is_none() || source_device == home_device {
        return Ok(TrashHome {
            root: home,
            top: None,
        });
    }

    let Some(top) = top_directory(source) else {
        return Ok(TrashHome {
            root: home,
            top: None,
        });
    };
    match volume_trash(&top) {
        Some(root) => Ok(TrashHome {
            root,
            top: Some(top),
        }),
        None => Ok(TrashHome {
            root: home,
            top: None,
        }),
    }
}

/// The Trash directory on this volume, creating `.Trash-$uid` when needed.
fn volume_trash(top: &Path) -> Option<PathBuf> {
    let shared = top.join(".Trash");
    if is_shared_trash(&shared) {
        let mine = shared.join(uid().to_string());
        if std::fs::create_dir_all(&mine).is_ok() {
            return Some(mine);
        }
    }
    let own = top.join(format!(".Trash-{}", uid()));
    std::fs::create_dir_all(&own).ok().map(|()| own)
}

/// Whether `$topdir/.Trash` is the shared Trash the spec describes: a real
/// directory, not a symlink, with the sticky bit set.
///
/// The three conditions are the spec's own, and they are a security rule rather
/// than a formality: without the sticky bit any user could remove another's
/// trashed files, and a symlink could point the whole volume's Trash anywhere.
fn is_shared_trash(path: &Path) -> bool {
    let Ok(data) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !data.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        data.permissions().mode() & 0o1000 != 0
    }
    #[cfg(not(unix))]
    false
}

/// The mount point `path` lives on: the highest ancestor still on the same
/// filesystem.
///
/// Read from the entries themselves rather than from a mount table, because the
/// question is about the file at hand and `st_dev` answers it exactly, including
/// for a bind mount or a path reached through a symlinked parent.
pub(crate) fn top_directory(path: &Path) -> Option<PathBuf> {
    let start = existing_ancestor(path)?;
    let device = device_of(&start)?;
    let mut top = start.clone();
    let mut cursor = start;
    while let Some(parent) = cursor.parent().map(Path::to_path_buf) {
        if parent == cursor {
            break;
        }
        if device_of(&parent) != Some(device) {
            break;
        }
        top = parent.clone();
        cursor = parent;
    }
    Some(top)
}

/// The nearest ancestor that exists, so a path that has just been removed still
/// answers which volume it was on.
fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut cursor = absolute.as_path();
    loop {
        if cursor.exists() {
            return Some(cursor.to_path_buf());
        }
        cursor = cursor.parent()?;
    }
}

#[cfg(unix)]
fn device_of(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|data| data.dev())
}

#[cfg(not(unix))]
fn device_of(_path: &Path) -> Option<u64> {
    None
}

#[cfg(unix)]
fn uid() -> u32 {
    // `getuid` never fails and needs no FFI here: the effective user owns the
    // process's own runtime directory, which the XDG helper already resolves.
    std::fs::metadata("/proc/self")
        .map(|data| {
            use std::os::unix::fs::MetadataExt;
            data.uid()
        })
        .unwrap_or(0)
}

#[cfg(not(unix))]
fn uid() -> u32 {
    0
}

/// Every Trash directory that currently exists for this user: the home one and
/// one per mounted volume.
///
/// Mount points come from `/proc/self/mounts`, which is the only place that
/// knows what is mounted right now. A volume with no Trash contributes nothing.
pub(crate) fn all_trash_roots() -> Vec<TrashHome> {
    let mut roots = Vec::new();
    if let Ok(home) = crate::trash::home_trash() {
        roots.push(TrashHome {
            root: home,
            top: None,
        });
    }
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mounts") else {
        return roots;
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    for line in mounts.lines() {
        let Some(point) = mount_point(line) else {
            continue;
        };
        if seen.contains(&point) {
            continue;
        }
        seen.push(point.clone());
        for candidate in [
            point.join(format!(".Trash-{}", uid())),
            point.join(".Trash").join(uid().to_string()),
        ] {
            if candidate.join("info").is_dir() {
                roots.push(TrashHome {
                    root: candidate,
                    top: Some(point.clone()),
                });
            }
        }
    }
    roots
}

/// The second field of a `/proc/self/mounts` line, with the octal escapes the
/// kernel writes for spaces and tabs undone.
fn mount_point(line: &str) -> Option<PathBuf> {
    let field = line.split_whitespace().nth(1)?;
    let mut out = Vec::new();
    let bytes = field.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&field[index + 1..index + 4], 8) {
                out.push(value);
                index += 4;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    Some(celestina_core::percent::path_from_bytes(&out))
}

/// The absolute original path a record points at: as written when it is
/// absolute, and resolved against the volume's top directory when the record
/// used the relative form the spec allows there.
pub(crate) fn resolve_original(recorded: &Path, top: Option<&Path>) -> PathBuf {
    if recorded.is_absolute() {
        return recorded.to_path_buf();
    }
    match top {
        Some(top) => top.join(recorded),
        None => recorded.to_path_buf(),
    }
}

/// Why a Trash could not be prepared, for the one caller that must say so.
#[allow(dead_code)]
pub(crate) fn unwritable(path: &Path) -> OpError {
    OpError::Io {
        path: path.to_path_buf(),
        kind: io::ErrorKind::PermissionDenied,
        message: "the volume's Trash could not be created".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{mount_point, resolve_original, top_directory};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_mount_line_gives_its_point_with_escapes_undone() {
        assert_eq!(
            mount_point("/dev/nvme0n1p2 / ext4 rw,relatime 0 0"),
            Some(PathBuf::from("/"))
        );
        assert_eq!(
            mount_point("/dev/sdb1 /run/media/toni/Disco\\040Duro exfat rw 0 0"),
            Some(PathBuf::from("/run/media/toni/Disco Duro"))
        );
        assert_eq!(mount_point(""), None);
    }

    #[test]
    fn the_top_directory_of_a_path_is_a_real_mount_point() {
        // Whatever this checkout is on, walking up from it must stop somewhere
        // that exists and contains it.
        let here = std::env::current_dir().expect("cwd");
        let top = top_directory(&here).expect("top");
        assert!(top.exists());
        assert!(here.starts_with(&top));
    }

    /// The rule this module exists for: an entry on the home filesystem uses
    /// the home Trash, and one on another volume does not.
    #[test]
    fn the_home_filesystem_uses_the_home_trash() {
        let here = std::env::current_dir().expect("cwd");
        let home = super::trash_home_for(&here).expect("home");
        // This checkout and the home Trash share a filesystem in every
        // environment this suite runs in, so the volume path must not be taken.
        let expected = crate::trash::home_trash().expect("home trash");
        assert_eq!(home.root, expected);
        assert_eq!(home.top, None);
    }

    #[test]
    fn a_relative_record_resolves_against_its_volume() {
        assert_eq!(
            resolve_original(
                Path::new("fotos/uno.jpg"),
                Some(Path::new("/run/media/toni/disco"))
            ),
            PathBuf::from("/run/media/toni/disco/fotos/uno.jpg")
        );
        // An absolute record is already the answer, whatever the volume is.
        assert_eq!(
            resolve_original(
                Path::new("/home/toni/uno.txt"),
                Some(Path::new("/run/media"))
            ),
            PathBuf::from("/home/toni/uno.txt")
        );
    }
}
