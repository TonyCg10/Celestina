use std::fs;
use std::io;
use std::path::Path;

use crate::error::OpError;
use crate::trashinfo::trashed_file_for;

/// Permanently deletes a single trashed entry: removes its `files/<name>` body
/// and then its `info/<name>.trashinfo` record. This is the irreversible
/// counterpart to [`restore_from_trash`](crate::restore_from_trash) — the entry
/// is gone from disk, not moved anywhere.
///
/// Takes the info-file path — the same identity [`list_home_trash`] reports and
/// restore consumes — so a Trash browser can purge exactly what it listed. The
/// body is unlinked without following a symlink (a symlinked directory is
/// unlinked, never recursed into), and a directory body is removed whole. A body
/// that is already gone (an orphan record) is not an error: the stray
/// `.trashinfo` is still removed so the entry leaves the Trash. The record is
/// dropped last, so a failure removing the body leaves the entry still listed
/// and retryable rather than orphaning the info.
///
/// [`list_home_trash`]: crate::list_home_trash
pub fn purge_from_trash(info: &Path) -> Result<(), OpError> {
    let trashed = trashed_file_for(info).ok_or_else(|| OpError::Io {
        path: info.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the .trashinfo is not inside a Trash info/ directory".to_owned(),
    })?;

    match fs::symlink_metadata(&trashed) {
        Ok(meta) if meta.is_dir() => {
            fs::remove_dir_all(&trashed).map_err(|error| OpError::io(&trashed, &error))?;
        }
        Ok(_) => {
            fs::remove_file(&trashed).map_err(|error| OpError::io(&trashed, &error))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(OpError::io(&trashed, &error)),
    }

    match fs::remove_file(info) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(OpError::io(info, &error)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::purge_from_trash;
    use crate::trash::trash_into;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-purge-{label}-{}-{nonce}",
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
    fn purge_deletes_the_body_and_the_record() {
        let dir = TestDir::new("basic");
        let source = dir.path().join("note.txt");
        fs::write(&source, b"gone for good").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        assert!(trashed.trashed.exists() && trashed.info.exists());

        purge_from_trash(&trashed.info).expect("purge");

        assert!(!trashed.trashed.exists(), "the body is gone");
        assert!(!trashed.info.exists(), "the record is gone");
    }

    #[test]
    fn purge_removes_a_directory_body_whole() {
        let dir = TestDir::new("tree");
        let source = dir.path().join("folder");
        fs::create_dir(&source).expect("seed dir");
        fs::write(source.join("child.txt"), b"x").expect("seed child");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        assert!(trashed.trashed.is_dir());

        purge_from_trash(&trashed.info).expect("purge");

        assert!(!trashed.trashed.exists(), "the whole tree is gone");
        assert!(!trashed.info.exists(), "the record is gone");
    }

    #[test]
    fn purge_of_an_orphan_record_still_removes_it() {
        let dir = TestDir::new("orphan");
        let source = dir.path().join("ghost.txt");
        fs::write(&source, b"x").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");
        fs::remove_file(&trashed.trashed).expect("delete the body first");

        purge_from_trash(&trashed.info).expect("purge tolerates a missing body");

        assert!(!trashed.info.exists(), "the stray record is removed");
    }
}
