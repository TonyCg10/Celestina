use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::{percent, CancellationToken};

use crate::copy::Progress;
use crate::error::OpError;
use crate::relocate::{is_cross_device, relocate_by_copy};
use crate::reserve::{rename_without_replacing, RenameFailure};

/// Where an entry landed after being sent to the Trash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trashed {
    /// The absolute path the entry used to live at.
    pub original: PathBuf,
    /// Its new home under `Trash/files/`.
    pub trashed: PathBuf,
    /// The `Trash/info/<name>.trashinfo` recording where it came from.
    pub info: PathBuf,
    /// Entries a trashing by copy (into a Trash on another filesystem) left
    /// at the original location because they arrived or changed during the
    /// copy; see [`crate::Moved::left_behind`]. Empty for a rename.
    pub left_behind: Vec<PathBuf>,
}

/// Sends `source` to the Trash it belongs in: the home one
/// (`$XDG_DATA_HOME/Trash`) when they share a filesystem, and the volume's own
/// (`.Trash-$uid`, or a sticky shared `.Trash/$uid`) otherwise.
///
/// The record's `Path=` is relative to the volume's top directory in a
/// volume's Trash, as GIO writes it, so the entry still restores when the
/// volume is mounted somewhere else; in the home Trash it is absolute.
///
/// Follows the spec's ordering: an `info/<name>.trashinfo` is created with
/// `O_EXCL` first, which reserves a unique name, and only then is the entry
/// moved into `files/<name>`. That move never replaces anything: a
/// `files/<name>` that already exists (an orphan body, or one another tool
/// left) makes the name unusable and the next free one is taken. On the same
/// filesystem the move is a rename; if the entry lives on another filesystem
/// it is copied, synced, verified and only then removed (the same no-data-loss
/// path as a cross-device move). A failure rolls the reserved info file back.
///
/// Which Trash is chosen matters for more than tidiness: trashing into the home
/// Trash from another disk copies every byte onto the home filesystem, so a
/// large folder deleted from an external drive would fill the system disk and
/// take as long as the copy it really is. In its own volume's Trash the same
/// delete is a rename.
///
/// A volume whose Trash cannot be created falls back to the home one, and that
/// case is a real copy (see [`trash_into`]) — so a caller must still run this
/// off its UI thread and use `progress`, exactly like a copy or move.
pub fn trash(
    source: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Trashed, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }

    match fs::symlink_metadata(source) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(source, &error)),
    }

    let root = crate::volume::trash_home_for(source)?;
    trash_into(source, &root, cancellation, progress)
}

/// Sends `source` into the Trash directory rooted at `trash_root` (which will
/// hold `files/` and `info/`). Split out so the reserve / write / move logic is
/// testable without touching the real `$XDG_DATA_HOME`.
pub(crate) fn trash_into(
    source: &Path,
    trash_root: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Trashed, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }

    let name = source.file_name().ok_or_else(|| OpError::Io {
        path: source.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the source has no file name to trash".to_owned(),
    })?;
    let original = std::path::absolute(source).unwrap_or_else(|_| source.to_path_buf());

    let files_dir = trash_root.join("files");
    let info_dir = trash_root.join("info");
    fs::create_dir_all(&files_dir).map_err(|error| OpError::io(&files_dir, &error))?;
    fs::create_dir_all(&info_dir).map_err(|error| OpError::io(&info_dir, &error))?;

    let source_is_directory = match fs::symlink_metadata(source) {
        Ok(data) => data.is_dir(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(source, &error)),
    };
    // Relative to the volume's top in a volume's Trash, absolute at home.
    let content = trashinfo(&crate::volume::recorded_path(&original, trash_root));

    let mut attempt = 0;
    loop {
        // Reserve a free name by creating its .trashinfo with O_EXCL.
        let (trashed_name, info_path, mut info_file, next) =
            reserve_name(&info_dir, &files_dir, name, attempt)?;
        attempt = next;

        if let Err(error) = info_file.write_all(content.as_bytes()) {
            let _ = fs::remove_file(&info_path);
            return Err(OpError::io(&info_path, &error));
        }
        drop(info_file);

        let destination = files_dir.join(&trashed_name);
        let left_behind = match rename_without_replacing(source, &destination, source_is_directory)
        {
            Ok(()) => Vec::new(),
            // Another entry took `files/<name>` after the name was chosen:
            // give the record back and take the next name.
            Err(RenameFailure::Taken) => {
                let _ = fs::remove_file(&info_path);
                continue;
            }
            Err(RenameFailure::Io(error)) if is_cross_device(&error) => {
                match relocate_by_copy(source, &destination, cancellation, progress) {
                    Ok(left_behind) => left_behind,
                    Err(OpError::AlreadyExists { path }) if path == destination => {
                        let _ = fs::remove_file(&info_path);
                        continue;
                    }
                    Err(moved) => {
                        let _ = fs::remove_file(&info_path);
                        return Err(moved);
                    }
                }
            }
            Err(RenameFailure::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                let _ = fs::remove_file(&info_path);
                return Err(OpError::SourceMissing {
                    path: source.to_path_buf(),
                });
            }
            Err(RenameFailure::Io(error)) => {
                let _ = fs::remove_file(&info_path);
                return Err(OpError::io(&destination, &error));
            }
        };

        return Ok(Trashed {
            original,
            trashed: destination,
            info: info_path,
            left_behind,
        });
    }
}

/// Creates `info/<candidate>.trashinfo` with `O_EXCL`, suffixing the name until
/// a free one is found, starting at suffix `from`, and returns the reserved
/// name, its info path and handle, and the suffix to resume from if the move
/// then finds `files/<name>` taken after all.
///
/// A candidate whose `files/<name>` already exists is skipped as well, so an
/// orphan body is never the target of a move; the move itself refuses to
/// replace one that appears later.
fn reserve_name(
    info_dir: &Path,
    files_dir: &Path,
    base: &OsStr,
    from: u32,
) -> Result<(OsString, PathBuf, File, u32), OpError> {
    for attempt in from..10_000u32 {
        let candidate = if attempt == 0 {
            base.to_os_string()
        } else {
            let mut suffixed = base.to_os_string();
            suffixed.push(format!(".{attempt}"));
            suffixed
        };

        match fs::symlink_metadata(files_dir.join(&candidate)) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(OpError::io(&files_dir.join(&candidate), &error)),
        }

        let mut info_name = candidate.clone();
        info_name.push(".trashinfo");
        let info_path = info_dir.join(&info_name);

        match File::create_new(&info_path) {
            Ok(file) => return Ok((candidate, info_path, file, attempt + 1)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(OpError::io(&info_path, &error)),
        }
    }

    Err(OpError::Io {
        path: info_dir.to_path_buf(),
        kind: io::ErrorKind::AlreadyExists,
        message: "could not find a free Trash name after 10000 attempts".to_owned(),
    })
}

fn trashinfo(original: &Path) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        url_encode(original),
        deletion_date_now()
    )
}

/// Home Trash directory, from `$XDG_DATA_HOME` or `$HOME/.local/share`.
pub(crate) fn home_trash() -> Result<PathBuf, OpError> {
    let data_home = celestina_core::xdg::data_home().ok_or_else(|| OpError::Io {
        path: PathBuf::new(),
        kind: io::ErrorKind::NotFound,
        message: "no XDG_DATA_HOME or HOME to locate the Trash".to_owned(),
    })?;
    Ok(data_home.join("Trash"))
}

/// Percent-encodes a path per the Trash spec: unreserved bytes and `/` are kept,
/// everything else becomes `%XX`. Operates on raw bytes, so non-UTF-8 paths
/// round-trip.
fn url_encode(path: &Path) -> String {
    percent::encode(&percent::path_bytes(path))
}

fn deletion_date_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    format_utc(seconds as i64)
}

/// Formats a Unix timestamp as a spec `YYYY-MM-DDThh:mm:ss` string, in UTC.
/// Local time would need a timezone database this crate deliberately avoids.
fn format_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// Howard Hinnant's civil-from-days: a Unix day count to (year, month, day).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{format_utc, trash_into, url_encode};
    use crate::error::OpError;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-trash-{label}-{}-{nonce}",
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
    fn trashing_moves_the_file_and_records_where_it_came_from() {
        let dir = TestDir::new("basic");
        let source = dir.path().join("note.txt");
        fs::write(&source, b"bin me").expect("seed");
        let trash_root = dir.path().join("Trash");

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");

        assert!(!source.exists(), "source is gone");
        assert_eq!(fs::read(&trashed.trashed).expect("read trashed"), b"bin me");
        assert_eq!(trashed.trashed, trash_root.join("files/note.txt"));

        let info = fs::read_to_string(&trashed.info).expect("read info");
        assert!(info.starts_with("[Trash Info]\n"));
        assert!(info.contains(&format!("Path={}\n", url_encode(&trashed.original))));
        assert!(info.contains("\nDeletionDate="));
    }

    #[test]
    fn a_name_collision_is_suffixed_not_overwritten() {
        let dir = TestDir::new("collision");
        let trash_root = dir.path().join("Trash");

        let first = dir.path().join("dup.txt");
        fs::write(&first, b"first").expect("seed first");
        let a = trash_into(&first, &trash_root, &live(), &mut |_| {}).expect("trash first");

        // A second, unrelated file with the same name.
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("mk nested");
        let second = nested.join("dup.txt");
        fs::write(&second, b"second").expect("seed second");
        let b = trash_into(&second, &trash_root, &live(), &mut |_| {}).expect("trash second");

        assert_ne!(
            a.trashed, b.trashed,
            "the second must not clobber the first"
        );
        assert_eq!(fs::read(&a.trashed).expect("read a"), b"first");
        assert_eq!(fs::read(&b.trashed).expect("read b"), b"second");
    }

    #[test]
    fn trashing_a_missing_source_reports_it_and_leaves_no_info() {
        let dir = TestDir::new("missing");
        let trash_root = dir.path().join("Trash");
        let ghost = dir.path().join("ghost");

        let error = trash_into(&ghost, &trash_root, &live(), &mut |_| {}).expect_err("must fail");
        assert!(matches!(error, OpError::SourceMissing { .. }));

        let info_dir = trash_root.join("info");
        let leftovers = fs::read_dir(&info_dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(leftovers, 0, "the reserved info file must be rolled back");
    }

    #[test]
    fn a_cancelled_trash_does_nothing() {
        let dir = TestDir::new("cancel");
        let source = dir.path().join("keep.txt");
        fs::write(&source, b"keep").expect("seed");
        let trash_root = dir.path().join("Trash");

        let token = CancellationToken::new();
        token.cancel();
        let error = trash_into(&source, &trash_root, &token, &mut |_| {}).expect_err("cancelled");
        assert!(matches!(error, OpError::Cancelled));
        assert!(source.exists());
    }

    #[test]
    fn url_encoding_keeps_slashes_and_escapes_spaces() {
        assert_eq!(url_encode(Path::new("/home/u/a b")), "/home/u/a%20b");
        assert_eq!(url_encode(Path::new("/x/y.txt")), "/x/y.txt");
    }

    #[test]
    fn format_utc_matches_a_known_instant() {
        // 2021-01-01T00:00:00 UTC = 1609459200.
        assert_eq!(format_utc(1_609_459_200), "2021-01-01T00:00:00");
    }

    /// SID-14: a body already sitting at `files/<name>` without a record (an
    /// orphan, or another tool's) is never replaced by a new trashing.
    #[test]
    fn an_orphan_body_in_files_is_never_replaced() {
        let dir = TestDir::new("orphan-body");
        let trash_root = dir.path().join("Trash");
        fs::create_dir_all(trash_root.join("files")).expect("mk files");
        let orphan = trash_root.join("files/note.txt");
        fs::write(&orphan, b"orphan").expect("seed orphan");
        let source = dir.path().join("note.txt");
        fs::write(&source, b"new").expect("seed source");

        let trashed = trash_into(&source, &trash_root, &live(), &mut |_| {}).expect("trash");

        assert_ne!(trashed.trashed, orphan, "the orphan's name was reused");
        assert_eq!(fs::read(&orphan).expect("orphan kept"), b"orphan");
        assert_eq!(fs::read(&trashed.trashed).expect("trashed"), b"new");
    }

    /// Review round 2, minor B: in a volume's Trash the record is relative to
    /// the volume's top, so it restores after the volume is mounted at another
    /// path. Two directories stand in for the two mount points.
    #[test]
    fn a_volume_record_is_relative_and_survives_a_remount() {
        let dir = TestDir::new("remount");
        let first = dir.path().join("first-mount");
        fs::create_dir_all(first.join("docs")).expect("mk docs");
        let source = first.join("docs/nota uno.txt");
        fs::write(&source, b"on the stick").expect("seed");

        let trashed =
            trash_into(&source, &first.join(".Trash-1000"), &live(), &mut |_| {}).expect("trash");
        let record = fs::read_to_string(&trashed.info).expect("read info");
        assert!(record.contains("\nPath=docs/nota%20uno.txt\n"), "{record}");

        let second = dir.path().join("second-mount");
        fs::rename(&first, &second).expect("remount elsewhere");
        let info = second.join(".Trash-1000/info/nota uno.txt.trashinfo");
        let restored = crate::restore_from_trash(&info, &live()).expect("restore");

        assert_eq!(restored.to, second.join("docs/nota uno.txt"));
        assert_eq!(fs::read(&restored.to).expect("back"), b"on the stick");
    }

    /// The home Trash keeps writing the absolute path.
    #[test]
    fn a_home_record_stays_absolute() {
        let dir = TestDir::new("home-absolute");
        let source = dir.path().join("nota.txt");
        fs::write(&source, b"x").expect("seed");
        let trashed =
            trash_into(&source, &dir.path().join("Trash"), &live(), &mut |_| {}).expect("trash");
        let record = fs::read_to_string(&trashed.info).expect("read info");
        assert!(
            record.contains(&format!("\nPath={}\n", url_encode(&source))),
            "{record}"
        );
    }
}
