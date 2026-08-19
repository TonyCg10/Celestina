use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Whether the freed name is for a file, whose extension must survive the
/// suffix, or for a directory, which has none.
///
/// A directory name is not `stem.extension`, and treating it as one is how
/// `web-2.1.2` came back as `web-2.1 (copia).2`: the trailing `.2` is part of a
/// version, not a type. The caller knows which of the two it is creating, so it
/// says, instead of the domain guessing from the spelling.
/// A compound extension (`tar.gz`) is one the caller names outright: `Path`
/// only ever sees the last component of it, and `notas.tar (copia).gz` is not
/// the name anybody meant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameShape<'a> {
    /// A file: the suffix goes before its last extension.
    File,
    /// A directory: the suffix goes at the end of the whole name.
    Directory,
    /// A file whose extension is this whole trailing suffix, without the dot.
    Extension(&'a str),
}

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
pub fn next_available(dir: &Path, name: &OsStr, marker: &str, shape: NameShape<'_>) -> PathBuf {
    let as_path = Path::new(name);
    let (stem, extension) = match shape {
        NameShape::File => (as_path.file_stem().unwrap_or(name), as_path.extension()),
        NameShape::Directory => (name, None),
        NameShape::Extension(extension) => match strip_suffix(name, extension) {
            Some(stem) => (stem, Some(OsStr::new(extension))),
            // The name does not end in the extension the caller named; treating
            // it as one would invent a type the file does not have.
            None => (as_path.file_stem().unwrap_or(name), as_path.extension()),
        },
    };

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

/// `name` without a trailing `.extension`, byte-wise and case-insensitively on
/// ASCII, or `None` when it does not end in one.
#[cfg(unix)]
fn strip_suffix<'a>(name: &'a OsStr, extension: &str) -> Option<&'a OsStr> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = name.as_bytes();
    let suffix = format!(".{extension}").to_ascii_lowercase();
    let lower = bytes.to_ascii_lowercase();
    if !lower.ends_with(suffix.as_bytes()) || lower.len() == suffix.len() {
        return None;
    }
    Some(OsStr::from_bytes(&bytes[..bytes.len() - suffix.len()]))
}

#[cfg(not(unix))]
fn strip_suffix<'a>(name: &'a OsStr, extension: &str) -> Option<&'a OsStr> {
    let _ = (name, extension);
    None
}

#[cfg(test)]
mod tests {
    use super::{next_available, NameShape};
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
        let first = next_available(&dir, OsStr::new("nota.txt"), "copia", NameShape::File);
        assert_eq!(first.file_name().unwrap(), OsStr::new("nota (copia).txt"));

        // Occupy it and the plain name; the next free is "(copia 2)".
        std::fs::write(&first, b"x").expect("seed copia");
        std::fs::write(dir.join("nota.txt"), b"x").expect("seed orig");
        let second = next_available(&dir, OsStr::new("nota.txt"), "copia", NameShape::File);
        assert_eq!(
            second.file_name().unwrap(),
            OsStr::new("nota (copia 2).txt")
        );

        // A name without an extension keeps the suffix at the end.
        let no_ext = next_available(&dir, OsStr::new("carpeta"), "copia", NameShape::Directory);
        assert_eq!(no_ext.file_name().unwrap(), OsStr::new("carpeta (copia)"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_keeps_its_whole_name_before_the_marker() {
        let dir = scratch("directory");
        // `web-2.1.2` is a version, not a name with a `.2` type: the freed name
        // must not cut it in half.
        let freed = next_available(&dir, OsStr::new("web-2.1.2"), "copia", NameShape::Directory);
        assert_eq!(freed.file_name().unwrap(), OsStr::new("web-2.1.2 (copia)"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compound_extension_is_kept_whole_when_the_caller_names_it() {
        let dir = scratch("compound");
        let freed = next_available(
            &dir,
            OsStr::new("notas.tar.gz"),
            "nuevo",
            NameShape::Extension("tar.gz"),
        );
        assert_eq!(
            freed.file_name().unwrap(),
            OsStr::new("notas (nuevo).tar.gz")
        );

        // A name that does not carry that extension is treated as any file.
        let plain = next_available(
            &dir,
            OsStr::new("notas.txt"),
            "nuevo",
            NameShape::Extension("tar.gz"),
        );
        assert_eq!(plain.file_name().unwrap(), OsStr::new("notas (nuevo).txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_marker_is_the_callers_word() {
        let dir = scratch("marker");
        let copy = next_available(&dir, OsStr::new("a.txt"), "copy", NameShape::File);
        assert_eq!(copy.file_name().unwrap(), OsStr::new("a (copy).txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
