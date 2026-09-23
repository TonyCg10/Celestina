//! One folder's listing for the storage section's browsing mode.
//!
//! Blocking: [`list_folder`] runs on the `hematita-browse` thread. It reads
//! one directory without descending; a folder's size is not known until it is
//! scanned, so a directory's apparent size is zero here. Names keep their
//! bytes: the hub enters a folder by index with the exact name, and only the
//! display goes through a lossy conversion.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    Other,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dir => "dir",
            Self::File => "file",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: OsString,
    pub kind: EntryKind,
    pub apparent: u64,
}

/// Folders first, then by name without regard to case, then by the exact
/// bytes so two names differing only in case keep a stable order.
fn browse_order(a: &Entry, b: &Entry) -> Ordering {
    let rank = |entry: &Entry| u8::from(entry.kind != EntryKind::Dir);
    rank(a).cmp(&rank(b)).then_with(|| {
        let left = a.name.to_string_lossy().to_lowercase();
        let right = b.name.to_string_lossy().to_lowercase();
        left.cmp(&right).then_with(|| a.name.cmp(&b.name))
    })
}

/// Lists `path`'s entries, ordered for display. A symbolic link is listed as
/// `Other` and never followed; an entry whose metadata cannot be read is
/// listed as `Other` with no size rather than dropped.
pub fn list_folder(path: &Path) -> Result<Vec<Entry>, io::Error> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(path)? {
        let Ok(item) = item else {
            continue;
        };
        let name = item.file_name();
        let (kind, apparent) = match std::fs::symlink_metadata(item.path()) {
            Ok(meta) if meta.is_dir() => (EntryKind::Dir, 0),
            Ok(meta) if meta.is_file() => (EntryKind::File, meta.len()),
            _ => (EntryKind::Other, 0),
        };
        entries.push(Entry {
            name,
            kind,
            apparent,
        });
    }
    entries.sort_by(browse_order);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry {
            name: OsString::from(name),
            kind,
            apparent: 0,
        }
    }

    #[test]
    fn folders_lead_and_names_ignore_case() {
        let mut entries = [
            entry("b.txt", EntryKind::File),
            entry("Zeta", EntryKind::Dir),
            entry("A.txt", EntryKind::File),
            entry("alpha", EntryKind::Dir),
            entry("link", EntryKind::Other),
        ];
        entries.sort_by(browse_order);
        let names: Vec<_> = entries
            .iter()
            .map(|e| e.name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["alpha", "Zeta", "A.txt", "b.txt", "link"]);
    }

    #[test]
    fn a_real_folder_lists_kinds_and_file_sizes() {
        let root = std::env::temp_dir().join(format!("hematita-browse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).expect("a temporary folder");
        std::fs::write(root.join("five"), b"12345").expect("a temporary file");
        std::os::unix::fs::symlink("sub", root.join("link")).expect("a temporary link");
        let listed = list_folder(&root).expect("a readable folder");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            listed,
            vec![
                Entry {
                    name: "sub".into(),
                    kind: EntryKind::Dir,
                    apparent: 0
                },
                Entry {
                    name: "five".into(),
                    kind: EntryKind::File,
                    apparent: 5
                },
                Entry {
                    name: "link".into(),
                    kind: EntryKind::Other,
                    apparent: 0
                },
            ]
        );
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(list_folder(Path::new("/nonexistent/hematita/folder")).is_err());
    }
}
