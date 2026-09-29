use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::percent;

use crate::error::OpError;

/// Why a Trash record may not be restored where it points. A volume's Trash
/// was written by whoever had the volume before, so its records are hostile
/// input; see [`restore_from_trash`](crate::restore_from_trash).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unrestorable {
    /// A relative `Path=` that climbs out of its directory with `..`.
    ClimbsOut,
    /// A relative `Path=` with no directory to read it from.
    Relative,
    /// A record in a volume's Trash that points outside that volume.
    OutsideVolume,
}

impl std::fmt::Display for Unrestorable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ClimbsOut => "the .trashinfo records a relative Path= that climbs out with ..",
            Self::Relative => {
                "the .trashinfo records a relative Path= with no directory to read it from"
            }
            Self::OutsideVolume => "the .trashinfo in a volume's Trash points outside that volume",
        })
    }
}

/// One recoverable entry in the freedesktop Trash, read from its `.trashinfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrashEntry {
    /// The `info/<name>.trashinfo` path — the identity passed to restore.
    pub info: PathBuf,
    /// The entry's body under `files/<name>`.
    pub trashed: PathBuf,
    /// The absolute path it will be restored to: the recorded `Path=`, or,
    /// when the record is relative, that path under the directory the spec
    /// reads it from (the volume's top directory for a volume's Trash).
    ///
    /// When [`TrashEntry::unrestorable`] is set this is instead the `Path=`
    /// exactly as recorded, which is not a place anything will be written and
    /// must not be shown as one.
    pub original: PathBuf,
    /// Why a restore will refuse this entry, or `None` when it can be
    /// restored to [`TrashEntry::original`].
    pub unrestorable: Option<Unrestorable>,
    /// The spec `DeletionDate=` string, or empty if the record omits it.
    pub deletion_date: String,
    /// The original file name, lossily, for display.
    pub name: String,
}

/// Lists every recoverable entry this user has trashed, most-recently-deleted
/// first: the home Trash and the Trash of every mounted volume.
///
/// All of them, because otherwise deleting from an external drive would look
/// like the file simply vanished — its Trash is on the drive, and only the home
/// one was ever read. An absent Trash is an empty list, not an error; orphan
/// `.trashinfo` records with no matching `files/` body are skipped, since they
/// cannot be restored.
pub fn list_trash() -> Result<Vec<TrashEntry>, OpError> {
    let mut out = Vec::new();
    for root in crate::volume::all_trash_roots() {
        // One unreadable volume must not empty the whole view.
        if let Ok(entries) = list_trash_at(&root) {
            out.extend(entries);
        }
    }
    sort_newest_first(&mut out);
    Ok(out)
}

/// Lists the Trash rooted at `trash_root`. Split out so listing is testable
/// without touching the real `$XDG_DATA_HOME`.
///
/// A record may write its original path relative to the directory its Trash
/// lives in (a volume's top directory, for a volume's Trash), which is the
/// one form this crate never writes and every reader must still understand;
/// [`crate::volume::resolve_original`] is the one rule for it, shared with
/// restore.
pub(crate) fn list_trash_at(trash_root: &Path) -> Result<Vec<TrashEntry>, OpError> {
    let info_dir = trash_root.join("info");
    let entries = match fs::read_dir(&info_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(OpError::io(&info_dir, &error)),
    };

    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| OpError::io(&info_dir, &error))?;
        let info = entry.path();
        if info.extension() != Some(OsStr::new("trashinfo")) {
            continue;
        }
        let Ok(Some(content)) = read_record(&info) else {
            continue;
        };
        let Some(recorded) = parse_original_path(&content) else {
            continue;
        };
        // The same rule restore applies, so the listing never offers a
        // hostile record's path as a place the entry will go back to.
        let (original, unrestorable) = match crate::volume::restore_target(&recorded, trash_root) {
            Ok(original) => (original, None),
            Err(reason) => (recorded, Some(reason)),
        };
        let Some(trashed) = trashed_file_for(&info) else {
            continue;
        };
        // Skip orphan records whose body is already gone — nothing to restore.
        if fs::symlink_metadata(&trashed).is_err() {
            continue;
        }
        let name = original
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| original.to_string_lossy().into_owned());
        out.push(TrashEntry {
            info,
            trashed,
            original,
            unrestorable,
            deletion_date: parse_deletion_date(&content).unwrap_or_default(),
            name,
        });
    }

    sort_newest_first(&mut out);
    Ok(out)
}

/// Spec dates are `YYYY-MM-DDThh:mm:ss`, so lexical order is chronological;
/// newest first, with the name as a stable tie-break.
fn sort_newest_first(out: &mut [TrashEntry]) {
    out.sort_by(|a, b| {
        b.deletion_date
            .cmp(&a.deletion_date)
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// The largest `.trashinfo` this reads. A record is a few hundred bytes; a
/// fully percent-encoded `PATH_MAX` path is 12 KiB. The bound keeps a hostile
/// Trash on a volume from making a reader take in an arbitrary file.
const MAX_RECORD_BYTES: u64 = 64 * 1024;

/// Reads one `.trashinfo`, bounded, refusing anything but a regular file (a
/// FIFO named like a record would otherwise block the reader), or `Ok(None)`
/// when it does not exist.
pub(crate) fn read_record(info: &Path) -> Result<Option<String>, OpError> {
    use celestina_core::atomic_file::{read_bounded, ReadError};
    match read_bounded(info, MAX_RECORD_BYTES) {
        Ok(Some(bytes)) => String::from_utf8(bytes).map(Some).map_err(|_| OpError::Io {
            path: info.to_path_buf(),
            kind: io::ErrorKind::InvalidData,
            message: "the .trashinfo is not UTF-8".to_owned(),
        }),
        Ok(None) => Ok(None),
        Err(error) => {
            let kind = match &error {
                ReadError::NotRegular { .. } => io::ErrorKind::InvalidInput,
                ReadError::TooLarge { .. } => io::ErrorKind::InvalidData,
                ReadError::Io { source, .. } => source.kind(),
            };
            Err(OpError::Io {
                path: info.to_path_buf(),
                kind,
                message: error.to_string(),
            })
        }
    }
}

/// Derives `<trash_root>/files/<name>` from `<trash_root>/info/<name>.trashinfo`.
pub(crate) fn trashed_file_for(info: &Path) -> Option<PathBuf> {
    let info_dir = info.parent()?;
    if info_dir.file_name() != Some(OsStr::new("info")) {
        return None;
    }
    let trash_root = info_dir.parent()?;
    let name = info.file_stem()?; // strips the ".trashinfo" extension
    Some(trash_root.join("files").join(name))
}

/// Reads the `Path=` line from a `.trashinfo` body and percent-decodes it back
/// into a path, byte-for-byte, so a non-UTF-8 original round-trips.
pub(crate) fn parse_original_path(content: &str) -> Option<PathBuf> {
    let value = content
        .lines()
        .find_map(|line| line.strip_prefix("Path="))?;
    let bytes = url_decode(value)?;
    if bytes.is_empty() {
        return None;
    }
    Some(percent::path_from_bytes(&bytes))
}

/// Reads the raw `DeletionDate=` value from a `.trashinfo` body, if present.
pub(crate) fn parse_deletion_date(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("DeletionDate="))
        .map(str::to_owned)
}

/// Reverses [`trash`](crate::trash)'s percent-encoding: `%XX` becomes one byte,
/// every other byte is taken verbatim. Returns `None` on a malformed escape.
pub(crate) fn url_decode(value: &str) -> Option<Vec<u8>> {
    percent::decode_strict(value)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{list_trash_at, url_decode};
    use crate::trash::trash_into;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-trashinfo-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn live() -> CancellationToken {
        CancellationToken::new()
    }

    #[test]
    fn listing_an_absent_trash_is_empty() {
        let dir = TestDir::new("absent");
        let entries = list_trash_at(&dir.path().join("Trash")).expect("list");
        assert!(entries.is_empty());
    }

    #[test]
    fn listing_reports_each_trashed_entry_with_its_origin() {
        let dir = TestDir::new("list");
        let trash_root = dir.path().join("Trash");
        let source = dir.path().join("nota.txt");
        fs::write(&source, b"hi").expect("seed");
        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");

        let entries = list_trash_at(&trash_root).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "nota.txt");
        assert_eq!(entries[0].original, trashed.original);
        assert_eq!(entries[0].info, trashed.info);
        assert!(
            entries[0].deletion_date.contains('T'),
            "records a spec date"
        );
    }

    #[test]
    fn an_orphan_info_without_a_body_is_skipped() {
        let dir = TestDir::new("orphan");
        let trash_root = dir.path().join("Trash");
        let source = dir.path().join("ghost.txt");
        fs::write(&source, b"x").expect("seed");
        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");
        fs::remove_file(&trashed.trashed).expect("delete the body");

        let entries = list_trash_at(&trash_root).expect("list");
        assert!(entries.is_empty(), "an unrestorable orphan is not listed");
    }

    #[test]
    fn url_decode_reverses_percent_encoding() {
        assert_eq!(url_decode("/home/u/a%20b").unwrap(), b"/home/u/a b");
        assert_eq!(url_decode("/x/y.txt").unwrap(), b"/x/y.txt");
        assert!(
            url_decode("/bad%2").is_none(),
            "a truncated escape is rejected"
        );
    }

    /// Review round 2, minor A: the listing applies restore's rule, so a
    /// hostile record is flagged and its path is not offered as a place to
    /// restore to.
    #[test]
    fn a_hostile_volume_record_is_listed_as_unrestorable() {
        let dir = TestDir::new("hostile");
        let top = dir.path().join("volume");
        let trash = top.join(".Trash-1000");
        fs::create_dir_all(trash.join("files")).expect("mk files");
        fs::create_dir_all(trash.join("info")).expect("mk info");
        fs::create_dir_all(top.join("docs")).expect("mk docs");
        for (name, recorded) in [("x.txt", "../x.txt"), ("ok.txt", "docs/ok.txt")] {
            fs::write(trash.join("files").join(name), b"body").expect("body");
            fs::write(
                trash.join("info").join(format!("{name}.trashinfo")),
                format!("[Trash Info]\nPath={recorded}\nDeletionDate=2026-09-26T10:00:00\n"),
            )
            .expect("record");
        }

        let entries = list_trash_at(&trash).expect("list");

        let hostile = entries
            .iter()
            .find(|entry| entry.name == "x.txt")
            .expect("listed");
        assert_eq!(hostile.unrestorable, Some(super::Unrestorable::ClimbsOut));
        assert_eq!(hostile.original, PathBuf::from("../x.txt"));
        let fine = entries
            .iter()
            .find(|entry| entry.name == "ok.txt")
            .expect("listed");
        assert_eq!(fine.unrestorable, None);
        assert_eq!(fine.original, top.join("docs/ok.txt"));
    }
}
