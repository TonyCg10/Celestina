use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;
use crate::relocate::{is_cross_device, relocate_by_copy};
use crate::reserve::{rename_without_replacing, RenameFailure};
use crate::trashinfo::{parse_original_path, read_record, trashed_file_for};

/// The paths a successful restore moved between.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Restored {
    /// Where the entry lived under `Trash/files/` before the restore.
    pub from: PathBuf,
    /// The original location it was returned to.
    pub to: PathBuf,
    /// Entries a restore by copy (from a Trash on another filesystem) left
    /// under `Trash/files/` because they arrived or changed during the copy;
    /// see [`crate::Moved::left_behind`]. When there are any the `.trashinfo`
    /// is kept, so what is left stays listed in the Trash. Empty for a rename.
    pub left_behind: Vec<PathBuf>,
}

/// Restores a trashed entry from the freedesktop Trash back to the original path
/// recorded in its `info/<name>.trashinfo`.
///
/// This is the inverse of [`trash`](crate::trash): it reads the `Path=` the info
/// file recorded, locates the matching `files/<name>` entry, and moves it back —
/// a rename on the same filesystem, or the loss-free copy → sync → verify →
/// remove-source path across filesystems. It **refuses to overwrite**: if
/// something already occupies the original path, or takes it while the restore
/// runs, the restore is reported, never resolved by destroying data. The
/// `.trashinfo` is removed only after the entry is safely back in place.
///
/// A relative `Path=` — the form GIO writes in a volume's `.Trash-$uid` — is
/// read from the directory the spec names (the volume's top directory), the
/// same rule [`list_trash`](crate::list_trash) shows. A record is refused, and
/// kept, when it stays relative even then, when a relative one climbs out with
/// `..`, or when a volume's record points outside that volume (whoever had the
/// volume before wrote it). The `.trashinfo` is read bounded, as a regular
/// file only.
///
/// Takes the info-file path (not the trashed file) because the info file is the
/// spec's authoritative record of where the entry belongs, so the same primitive
/// serves both undo-of-trash and a CP2 Trash browser.
pub fn restore_from_trash(
    info: &Path,
    cancellation: &CancellationToken,
) -> Result<Restored, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }

    let Some(content) = read_record(info)? else {
        return Err(OpError::SourceMissing {
            path: info.to_path_buf(),
        });
    };

    let recorded = parse_original_path(&content).ok_or_else(|| OpError::Io {
        path: info.to_path_buf(),
        kind: io::ErrorKind::InvalidData,
        message: "the .trashinfo has no decodable Path= entry".to_owned(),
    })?;

    let trashed = trashed_file_for(info).ok_or_else(|| OpError::Io {
        path: info.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the .trashinfo is not inside a Trash info/ directory".to_owned(),
    })?;

    // `trashed_file_for` accepted the info path, so it is `<root>/info/<name>`.
    let trash_root = info
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(""));
    let original =
        crate::volume::restore_target(&recorded, trash_root).map_err(|reason| OpError::Io {
            path: info.to_path_buf(),
            kind: io::ErrorKind::InvalidData,
            message: reason.to_string(),
        })?;

    // The entry the info file describes must actually be in files/.
    let trashed_is_directory = match fs::symlink_metadata(&trashed) {
        Ok(data) => data.is_dir(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing { path: trashed });
        }
        Err(error) => return Err(OpError::io(&trashed, &error)),
    };

    // Never clobber whatever now lives at the original location. This look is
    // the honest early answer; the no-replace rename below is what keeps it
    // true against another writer.
    if fs::symlink_metadata(&original).is_ok() {
        return Err(OpError::AlreadyExists { path: original });
    }

    let left_behind = match rename_without_replacing(&trashed, &original, trashed_is_directory) {
        Ok(()) => Vec::new(),
        Err(RenameFailure::Io(error)) if is_cross_device(&error) => {
            relocate_by_copy(&trashed, &original, cancellation, &mut |_| {})?
        }
        // Either way the entry stays in Trash: the origin was taken, or its
        // parent directory is gone.
        Err(failure) => return Err(failure.into_op_error(&original)),
    };

    // The entry is safely back; drop the now-orphan info record. A failure here
    // leaves a harmless dangling .trashinfo rather than undoing the restore.
    // What a restore by copy left keeps its record, so it stays listed.
    if left_behind.is_empty() {
        let _ = fs::remove_file(info);
    }

    Ok(Restored {
        from: trashed,
        to: original,
        left_behind,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::restore_from_trash;
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

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");
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

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");
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

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");
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

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");
        let restored = restore_from_trash(&trashed.info, &live()).expect("restore");

        assert_eq!(restored.to, source);
        assert_eq!(fs::read(&source).expect("read"), b"spaced");
    }

    /// Writes a Trash record by hand, the way another tool (GIO) writes one.
    fn plant_record(trash_root: &Path, name: &str, recorded: &str, body: &[u8]) -> PathBuf {
        fs::create_dir_all(trash_root.join("files")).expect("mk files");
        fs::create_dir_all(trash_root.join("info")).expect("mk info");
        fs::write(trash_root.join("files").join(name), body).expect("seed body");
        let info = trash_root.join("info").join(format!("{name}.trashinfo"));
        fs::write(
            &info,
            format!("[Trash Info]\nPath={recorded}\nDeletionDate=2026-09-26T10:00:00\n"),
        )
        .expect("seed record");
        info
    }

    /// SID-4: GIO writes `$topdir/.Trash-$uid` records relative to `$topdir`.
    /// The audit saw one restored relative to the process's working directory.
    #[test]
    fn a_relative_record_restores_under_its_volume_top() {
        let dir = TestDir::new("relative");
        let top = dir.path();
        fs::create_dir(top.join("fotos")).expect("mk origin folder");
        let info = plant_record(
            &top.join(".Trash-1000"),
            "uno.jpg",
            "fotos/uno.jpg",
            b"foto",
        );

        let restored = restore_from_trash(&info, &live()).expect("restore");

        assert_eq!(restored.to, top.join("fotos/uno.jpg"));
        assert_eq!(
            fs::read(top.join("fotos/uno.jpg")).expect("restored"),
            b"foto"
        );
        assert!(!info.exists(), "the record is gone");
    }

    /// Review M-3: a volume's Trash is written by whoever had the volume
    /// before. A record there that climbs out with `..`, names an absolute
    /// path outside the volume, or goes through a folder on the volume that
    /// is a symlink to elsewhere, is refused; nothing lands outside.
    #[test]
    fn a_volume_record_that_leaves_its_volume_is_refused() {
        let dir = TestDir::new("escape");
        let top = dir.path().join("volume");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).expect("mk outside");
        fs::create_dir_all(&top).expect("mk top");
        std::os::unix::fs::symlink(&outside, top.join("fotos")).expect("plant link");
        let trash = top.join(".Trash-1000");
        let absolute_outside = dir.path().join("absolute.txt");
        let records = [
            ("climbs.txt", "../escaped-by-probe.txt".to_owned()),
            (
                "absolute.txt",
                absolute_outside.to_string_lossy().into_owned(),
            ),
            ("linked.txt", "fotos/linked.txt".to_owned()),
        ];

        for (name, recorded) in &records {
            let info = plant_record(&trash, name, recorded, b"hostile");
            let error = restore_from_trash(&info, &live()).expect_err("must refuse");
            assert!(
                matches!(
                    error,
                    OpError::Io {
                        kind: std::io::ErrorKind::InvalidData,
                        ..
                    }
                ),
                "{recorded}: {error:?}"
            );
            assert!(info.exists(), "{recorded}: the record was dropped");
            assert!(
                trash.join("files").join(name).exists(),
                "{recorded}: body moved"
            );
        }
        assert!(!dir.path().join("escaped-by-probe.txt").exists());
        assert!(!absolute_outside.exists());
        assert!(!outside.join("linked.txt").exists());
    }

    /// An absolute record inside the volume, which is what this crate writes,
    /// still restores.
    #[test]
    fn an_absolute_record_inside_its_volume_restores() {
        let dir = TestDir::new("absolute-inside");
        let top = dir.path().join("volume");
        fs::create_dir_all(top.join("docs")).expect("mk docs");
        let original = top.join("docs/nota.txt");
        let info = plant_record(
            &top.join(".Trash-1000"),
            "nota.txt",
            &original.to_string_lossy(),
            b"mine",
        );

        let restored = restore_from_trash(&info, &live()).expect("restore");

        assert_eq!(restored.to, original);
        assert_eq!(fs::read(&original).expect("restored"), b"mine");
    }
}
