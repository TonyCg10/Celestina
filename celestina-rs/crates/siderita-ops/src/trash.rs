use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;

use crate::error::OpError;
use crate::relocate::{is_cross_device, relocate_by_copy};

/// Where an entry landed after being sent to the Trash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trashed {
    /// The absolute path the entry used to live at.
    pub original: PathBuf,
    /// Its new home under `Trash/files/`.
    pub trashed: PathBuf,
    /// The `Trash/info/<name>.trashinfo` recording where it came from.
    pub info: PathBuf,
}

/// Sends `source` to the freedesktop home Trash (`$XDG_DATA_HOME/Trash`).
///
/// Follows the spec's ordering: an `info/<name>.trashinfo` is created with
/// `O_EXCL` first, which reserves a unique name, and only then is the entry
/// moved into `files/<name>`. On the same filesystem the move is an atomic
/// rename; if the entry lives on another filesystem it is copied, verified and
/// only then removed (the same no-data-loss path as a cross-device move). A
/// failure rolls the reserved info file back.
///
/// Cross-filesystem entries land in the home Trash rather than a per-mount
/// `.Trash-$uid`; using the mount-local trash is a later refinement.
pub fn trash(source: &Path, cancellation: &CancellationToken) -> Result<Trashed, OpError> {
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

    let trash_root = home_trash()?;
    trash_into(source, &trash_root, cancellation)
}

/// Sends `source` into the Trash directory rooted at `trash_root` (which will
/// hold `files/` and `info/`). Split out so the reserve / write / move logic is
/// testable without touching the real `$XDG_DATA_HOME`.
pub(crate) fn trash_into(
    source: &Path,
    trash_root: &Path,
    cancellation: &CancellationToken,
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

    // Reserve a free name by creating its .trashinfo with O_EXCL.
    let (trashed_name, info_path, mut info_file) = reserve_name(&info_dir, name)?;

    let content = trashinfo(&original);
    if let Err(error) = info_file.write_all(content.as_bytes()) {
        let _ = fs::remove_file(&info_path);
        return Err(OpError::io(&info_path, &error));
    }
    drop(info_file);

    let destination = files_dir.join(&trashed_name);
    match fs::rename(source, &destination) {
        Ok(()) => {}
        Err(error) if is_cross_device(&error) => {
            if let Err(moved) = relocate_by_copy(source, &destination, cancellation, &mut |_| {}) {
                let _ = fs::remove_file(&info_path);
                return Err(moved);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let _ = fs::remove_file(&info_path);
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            });
        }
        Err(error) => {
            let _ = fs::remove_file(&info_path);
            return Err(OpError::io(&destination, &error));
        }
    }

    Ok(Trashed {
        original,
        trashed: destination,
        info: info_path,
    })
}

/// Creates `info/<candidate>.trashinfo` with `O_EXCL`, suffixing the name until
/// a free one is found, and returns the reserved name, its info path and handle.
fn reserve_name(info_dir: &Path, base: &OsStr) -> Result<(OsString, PathBuf, File), OpError> {
    for attempt in 0..10_000u32 {
        let candidate = if attempt == 0 {
            base.to_os_string()
        } else {
            let mut suffixed = base.to_os_string();
            suffixed.push(format!(".{attempt}"));
            suffixed
        };

        let mut info_name = candidate.clone();
        info_name.push(".trashinfo");
        let info_path = info_dir.join(&info_name);

        match File::create_new(&info_path) {
            Ok(file) => return Ok((candidate, info_path, file)),
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
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local").join("share"))
        })
        .ok_or_else(|| OpError::Io {
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
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = path_bytes(path);
    let mut out = String::with_capacity(bytes.len());
    for &byte in &bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
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

        let trashed = trash_into(&source, &trash_root, &live()).expect("trash");

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
        let a = trash_into(&first, &trash_root, &live()).expect("trash first");

        // A second, unrelated file with the same name.
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("mk nested");
        let second = nested.join("dup.txt");
        fs::write(&second, b"second").expect("seed second");
        let b = trash_into(&second, &trash_root, &live()).expect("trash second");

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

        let error = trash_into(&ghost, &trash_root, &live()).expect_err("must fail");
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
        let error = trash_into(&source, &trash_root, &token).expect_err("cancelled");
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
}
