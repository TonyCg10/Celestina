use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;
use crate::relocate::{is_cross_device, relocate_by_copy};

/// The paths a successful restore moved between.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Restored {
    /// Where the entry lived under `Trash/files/` before the restore.
    pub from: PathBuf,
    /// The original location it was returned to.
    pub to: PathBuf,
}

/// Restores a trashed entry from the freedesktop Trash back to the original path
/// recorded in its `info/<name>.trashinfo`.
///
/// This is the inverse of [`trash`](crate::trash): it reads the `Path=` the info
/// file recorded, locates the matching `files/<name>` entry, and moves it back —
/// an atomic `rename` on the same filesystem, or the loss-free copy → verify →
/// remove-source path across filesystems. It **refuses to overwrite**: if
/// something already occupies the original path the restore is reported, never
/// resolved by destroying data. The `.trashinfo` is removed only after the entry
/// is safely back in place.
///
/// Takes the info-file path (not the trashed file) because the info file is the
/// spec's authoritative record of where the entry belongs, so the same primitive
/// serves both undo-of-trash and a CP2 Trash browser.
pub fn restore_from_trash(info: &Path, cancellation: &CancellationToken) -> Result<Restored, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }

    let content = match fs::read_to_string(info) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: info.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(info, &error)),
    };

    let original = parse_original_path(&content).ok_or_else(|| OpError::Io {
        path: info.to_path_buf(),
        kind: io::ErrorKind::InvalidData,
        message: "the .trashinfo has no decodable Path= entry".to_owned(),
    })?;

    let trashed = trashed_file_for(info).ok_or_else(|| OpError::Io {
        path: info.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the .trashinfo is not inside a Trash info/ directory".to_owned(),
    })?;

    // The entry the info file describes must actually be in files/.
    match fs::symlink_metadata(&trashed) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing { path: trashed });
        }
        Err(error) => return Err(OpError::io(&trashed, &error)),
    }

    // Never clobber whatever now lives at the original location.
    if fs::symlink_metadata(&original).is_ok() {
        return Err(OpError::AlreadyExists { path: original });
    }

    match fs::rename(&trashed, &original) {
        Ok(()) => {}
        Err(error) if is_cross_device(&error) => {
            relocate_by_copy(&trashed, &original, cancellation, &mut |_| {})?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The destination's parent directory is gone; the entry stays in Trash.
            return Err(OpError::io(&original, &error));
        }
        Err(error) => return Err(OpError::io(&original, &error)),
    }

    // The entry is safely back; drop the now-orphan info record. A failure here
    // leaves a harmless dangling .trashinfo rather than undoing the restore.
    let _ = fs::remove_file(info);

    Ok(Restored {
        from: trashed,
        to: original,
    })
}

/// Derives `<trash_root>/files/<name>` from `<trash_root>/info/<name>.trashinfo`.
fn trashed_file_for(info: &Path) -> Option<PathBuf> {
    let info_dir = info.parent()?;
    if info_dir.file_name() != Some(std::ffi::OsStr::new("info")) {
        return None;
    }
    let trash_root = info_dir.parent()?;
    let name = info.file_stem()?; // strips the ".trashinfo" extension
    Some(trash_root.join("files").join(name))
}

/// Reads the `Path=` line from a `.trashinfo` body and percent-decodes it back
/// into a path, byte-for-byte, so a non-UTF-8 original round-trips.
fn parse_original_path(content: &str) -> Option<PathBuf> {
    let value = content
        .lines()
        .find_map(|line| line.strip_prefix("Path="))?;
    let bytes = url_decode(value)?;
    if bytes.is_empty() {
        return None;
    }
    Some(path_from_bytes(&bytes))
}

/// Reverses [`trash`](crate::trash)'s percent-encoding: `%XX` becomes one byte,
/// every other byte is taken verbatim. Returns `None` on a malformed escape.
fn url_decode(value: &str) -> Option<Vec<u8>> {
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
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{restore_from_trash, url_decode};
    use crate::error::OpError;
    use crate::trash::trash_into;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-restore-{label}-{}-{nonce}",
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
    fn restore_returns_a_trashed_file_to_where_it_came_from() {
        let dir = TestDir::new("basic");
        let source = dir.path().join("note.txt");
        fs::write(&source, b"bring me back").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        assert!(!source.exists(), "trashed file left its origin");

        let restored = restore_from_trash(&trashed.info, &live()).expect("restore");

        assert_eq!(restored.to, source);
        assert_eq!(fs::read(&source).expect("read restored"), b"bring me back");
        assert!(!trashed.trashed.exists(), "the Trash copy is gone");
        assert!(!trashed.info.exists(), "the .trashinfo is gone");
    }

    #[test]
    fn restore_refuses_to_overwrite_something_at_the_origin() {
        let dir = TestDir::new("occupied");
        let source = dir.path().join("dup.txt");
        fs::write(&source, b"old").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        // Something new takes the original name before we restore.
        fs::write(&source, b"new tenant").expect("reoccupy");

        let error = restore_from_trash(&trashed.info, &live()).expect_err("must refuse");
        assert!(matches!(error, OpError::AlreadyExists { .. }));
        // The refusal is loss-free: both the tenant and the trashed copy survive.
        assert_eq!(fs::read(&source).expect("tenant intact"), b"new tenant");
        assert!(trashed.trashed.exists(), "the trashed copy is kept");
        assert!(trashed.info.exists(), "the info record is kept");
    }

    #[test]
    fn restore_reports_a_missing_trashed_entry() {
        let dir = TestDir::new("missing");
        let source = dir.path().join("gone.txt");
        fs::write(&source, b"x").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        fs::remove_file(&trashed.trashed).expect("delete the trashed copy");

        let error = restore_from_trash(&trashed.info, &live()).expect_err("must fail");
        assert!(matches!(error, OpError::SourceMissing { .. }));
    }

    #[test]
    fn restore_round_trips_a_name_with_spaces() {
        let dir = TestDir::new("spaces");
        let source = dir.path().join("a b c.txt");
        fs::write(&source, b"spaced").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        let restored = restore_from_trash(&trashed.info, &live()).expect("restore");

        assert_eq!(restored.to, source);
        assert_eq!(fs::read(&source).expect("read"), b"spaced");
    }

    #[test]
    fn url_decode_reverses_percent_encoding() {
        assert_eq!(url_decode("/home/u/a%20b").unwrap(), b"/home/u/a b");
        assert_eq!(url_decode("/x/y.txt").unwrap(), b"/x/y.txt");
        assert!(url_decode("/bad%2").is_none(), "a truncated escape is rejected");
    }
}
