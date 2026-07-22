use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::OpError;
use crate::trash::home_trash;

/// One recoverable entry in the freedesktop Trash, read from its `.trashinfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrashEntry {
    /// The `info/<name>.trashinfo` path — the identity passed to restore.
    pub info: PathBuf,
    /// The entry's body under `files/<name>`.
    pub trashed: PathBuf,
    /// The absolute path it will be restored to (the recorded `Path=`).
    pub original: PathBuf,
    /// The spec `DeletionDate=` string, or empty if the record omits it.
    pub deletion_date: String,
    /// The original file name, lossily, for display.
    pub name: String,
}

/// Lists the recoverable entries in the home Trash (`$XDG_DATA_HOME/Trash`),
/// most-recently-deleted first. An absent Trash is an empty list, not an error;
/// orphan `.trashinfo` records with no matching `files/` body are skipped, since
/// they cannot be restored.
pub fn list_home_trash() -> Result<Vec<TrashEntry>, OpError> {
    let root = home_trash()?;
    list_trash_at(&root)
}

/// Lists the Trash rooted at `trash_root`. Split out so listing is testable
/// without touching the real `$XDG_DATA_HOME`.
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
        let Ok(content) = fs::read_to_string(&info) else {
            continue;
        };
        let Some(original) = parse_original_path(&content) else {
            continue;
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
            deletion_date: parse_deletion_date(&content).unwrap_or_default(),
            name,
        });
    }

    // Spec dates are `YYYY-MM-DDThh:mm:ss`, so lexical order is chronological;
    // newest first, with the name as a stable tie-break.
    out.sort_by(|a, b| {
        b.deletion_date
            .cmp(&a.deletion_date)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
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
    Some(path_from_bytes(&bytes))
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
    let raw = value.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'%' => {
                let high = hex_value(*raw.get(index + 1)?)?;
                let low = hex_value(*raw.get(index + 2)?)?;
                out.push((high << 4) | low);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(unix)]
pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
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
        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");

        let entries = list_trash_at(&trash_root).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "nota.txt");
        assert_eq!(entries[0].original, trashed.original);
        assert_eq!(entries[0].info, trashed.info);
        assert!(entries[0].deletion_date.contains('T'), "records a spec date");
    }

    #[test]
    fn an_orphan_info_without_a_body_is_skipped() {
        let dir = TestDir::new("orphan");
        let trash_root = dir.path().join("Trash");
        let source = dir.path().join("ghost.txt");
        fs::write(&source, b"x").expect("seed");
        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        fs::remove_file(&trashed.trashed).expect("delete the body");

        let entries = list_trash_at(&trash_root).expect("list");
        assert!(entries.is_empty(), "an unrestorable orphan is not listed");
    }

    #[test]
    fn url_decode_reverses_percent_encoding() {
        assert_eq!(url_decode("/home/u/a%20b").unwrap(), b"/home/u/a b");
        assert_eq!(url_decode("/x/y.txt").unwrap(), b"/x/y.txt");
        assert!(url_decode("/bad%2").is_none(), "a truncated escape is rejected");
    }
}
