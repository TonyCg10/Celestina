use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The first name in `dir` of the form `stem (marker).ext`, then
/// `stem (marker 2).ext`, … that does not already exist.
///
/// This is the "keep both" name policy for [`copy_as`](crate::copy_as) and
/// [`move_as`](crate::move_as): both take an explicit destination and leave the
/// choosing to the caller, and this is what any caller would otherwise reinvent.
/// `marker` is the localized word the UI shows (e.g. `"copia"`) — the domain
/// owns the algorithm (collision search, keeping the suffix before the
/// extension), the app owns the wording. Byte-wise on the stem, so a non-UTF-8
/// name survives.
pub fn next_available(dir: &Path, name: &OsStr, marker: &str) -> PathBuf {
    let as_path = Path::new(name);
    let stem = as_path.file_stem().unwrap_or(name);
    let extension = as_path.extension();

    for attempt in 1u64.. {
        let mut candidate = stem.to_os_string();
        if attempt == 1 {
            candidate.push(format!(" ({marker})"));
        } else {
            candidate.push(format!(" ({marker} {attempt})"));
        }
        if let Some(extension) = extension {
            candidate.push(".");
            candidate.push(extension);
        }
        let path = dir.join(&candidate);
        if std::fs::symlink_metadata(&path).is_err() {
            return path;
        }
    }
    unreachable!("the free-name search always terminates before u64 wraps")
}

#[cfg(test)]
mod tests {
    use super::next_available;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "siderita-available-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("mk test dir");
        dir
    }

    #[test]
    fn suffixes_the_marker_around_the_extension() {
        let dir = scratch("ext");

        // Nothing exists yet → the first "(copia)" name.
        let first = next_available(&dir, OsStr::new("nota.txt"), "copia");
        assert_eq!(first.file_name().unwrap(), OsStr::new("nota (copia).txt"));

        // Occupy it and the plain name; the next free is "(copia 2)".
        std::fs::write(&first, b"x").expect("seed copia");
        std::fs::write(dir.join("nota.txt"), b"x").expect("seed orig");
        let second = next_available(&dir, OsStr::new("nota.txt"), "copia");
        assert_eq!(
            second.file_name().unwrap(),
            OsStr::new("nota (copia 2).txt")
        );

        // A name without an extension keeps the suffix at the end.
        let no_ext = next_available(&dir, OsStr::new("carpeta"), "copia");
        assert_eq!(no_ext.file_name().unwrap(), OsStr::new("carpeta (copia)"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_marker_is_the_callers_word() {
        let dir = scratch("marker");
        let copy = next_available(&dir, OsStr::new("a.txt"), "copy");
        assert_eq!(copy.file_name().unwrap(), OsStr::new("a (copy).txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
