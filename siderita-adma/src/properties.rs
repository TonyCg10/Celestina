//! Get-Info for a single entry: the metadata a properties panel shows —
//! permissions, owner, MIME type, timestamps, symlink target — plus a bounded,
//! cancellable recursive folder size.
//!
//! The pure formatting (mode → `rwxr-xr-x`, uid/gid → names, epoch → local
//! `YYYY-MM-DD HH:MM`) is unit-tested; the metadata read and the directory walk
//! touch the filesystem.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use celestina_core::CancellationToken;

/// Everything the properties panel needs about one entry, already formatted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Properties {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub mime: String,
    /// A file's size in bytes; `None` for a directory (computed separately).
    pub size: Option<u64>,
    pub permissions: String,
    pub owner: String,
    pub modified: String,
    pub accessed: String,
    pub symlink_target: Option<String>,
    pub is_dir: bool,
}

/// Reads the metadata of `path` (the link itself, not its target) and formats it
/// for display. The recursive size of a directory is deliberately not computed
/// here — see [`directory_size`].
pub fn gather(path: &Path) -> Properties {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let mut props = Properties {
        name,
        path: path.to_string_lossy().into_owned(),
        ..Properties::default()
    };

    let Ok(meta) = fs::symlink_metadata(path) else {
        props.kind = "No disponible".to_owned();
        return props;
    };

    let file_type = meta.file_type();
    props.is_dir = file_type.is_dir();
    props.symlink_target = if file_type.is_symlink() {
        fs::read_link(path)
            .ok()
            .map(|target| target.to_string_lossy().into_owned())
    } else {
        None
    };

    props.kind = if file_type.is_symlink() {
        "Enlace simbólico".to_owned()
    } else if file_type.is_dir() {
        "Carpeta".to_owned()
    } else if file_type.is_file() {
        "Archivo".to_owned()
    } else {
        "Especial".to_owned()
    };

    props.mime = if file_type.is_dir() {
        "inode/directory".to_owned()
    } else {
        crate::apps::detect_mime(path).unwrap_or_default()
    };

    props.size = (!file_type.is_dir()).then(|| meta.len());
    props.permissions = format_permissions(meta.mode());
    props.owner = format_owner(meta.uid(), meta.gid());
    props.modified = format_time(meta.mtime());
    props.accessed = format_time(meta.atime());
    props
}

/// Sums the sizes of every regular file under `dir`, recursively, without
/// following symlinks (so a symlink loop can't run away or double-count).
/// Honours `cancellation`; a directory it cannot read is skipped rather than
/// aborting the whole total.
pub fn directory_size(dir: &Path, cancellation: &CancellationToken) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        if cancellation.is_cancelled() {
            break;
        }
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            if cancellation.is_cancelled() {
                break;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let file_type = meta.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }

    total
}

/// Formats the low 9 permission bits as `rwxr-xr-x`.
fn format_permissions(mode: u32) -> String {
    const FLAGS: [(u32, char); 9] = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    FLAGS
        .iter()
        .map(|&(bit, ch)| if mode & bit != 0 { ch } else { '-' })
        .collect()
}

/// `usuario · grupo`, resolving the names from `/etc/passwd` and `/etc/group`,
/// falling back to the numeric id when a name is not found.
fn format_owner(uid: u32, gid: u32) -> String {
    let user = lookup_name("/etc/passwd", uid).unwrap_or_else(|| uid.to_string());
    let group = lookup_name("/etc/group", gid).unwrap_or_else(|| gid.to_string());
    format!("{user} · {group}")
}

/// Looks an id up in a colon-separated `name:x:id:...` database (passwd/group).
fn lookup_name(database: &str, id: u32) -> Option<String> {
    let contents = fs::read_to_string(database).ok()?;
    let wanted = id.to_string();
    for line in contents.lines() {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next();
        if fields.next() == Some(wanted.as_str()) {
            return Some(name.to_owned());
        }
    }
    None
}

/// Formats a Unix timestamp (seconds since the epoch) as local
/// `YYYY-MM-DD HH:MM` via `localtime_r`. An empty string for a zero/absent time.
fn format_time(secs: i64) -> String {
    if secs == 0 {
        return String::new();
    }
    // SAFETY: localtime_r writes into a fully-owned, zeroed `tm`; time is a
    // valid `time_t` and the call has no other effects.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let time = secs as libc::time_t;
    let result = unsafe { libc::localtime_r(&time, &mut tm) };
    if result.is_null() {
        return String::new();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
    )
}

#[cfg(test)]
mod tests {
    use super::{format_owner, format_permissions, format_time, lookup_name};

    #[test]
    fn permissions_format_the_rwx_triplets() {
        assert_eq!(format_permissions(0o755), "rwxr-xr-x");
        assert_eq!(format_permissions(0o640), "rw-r-----");
        assert_eq!(format_permissions(0o000), "---------");
        // High bits (file type, setuid) are ignored.
        assert_eq!(format_permissions(0o100644), "rw-r--r--");
    }

    #[test]
    fn owner_resolves_root_from_the_real_passwd() {
        // uid 0 is root on every Unix; the group name varies (root/wheel), so
        // only assert the user half here.
        let owner = format_owner(0, 0);
        assert!(owner.starts_with("root · "), "got {owner}");
    }

    #[test]
    fn lookup_falls_back_to_none_for_a_missing_id() {
        assert!(lookup_name("/etc/passwd", 4_294_967_000).is_none());
    }

    #[test]
    fn a_zero_time_is_blank_and_others_are_shaped() {
        assert_eq!(format_time(0), "");
        // A known instant just has to come out in the YYYY-MM-DD HH:MM shape.
        let formatted = format_time(1_700_000_000);
        assert_eq!(formatted.len(), 16, "got {formatted}");
        assert_eq!(&formatted[4..5], "-");
        assert_eq!(&formatted[10..11], " ");
    }
}
