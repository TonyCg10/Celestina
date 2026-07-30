//! Filesystem policy for files received through the share plugin.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PARTIAL: AtomicU64 = AtomicU64::new(1);

/// The XDG downloads dir for received files: `$XDG_DOWNLOAD_DIR`, else what
/// `xdg-user-dir` reports, else `~/Downloads`.
pub(crate) fn download_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DOWNLOAD_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(output) = std::process::Command::new("xdg-user-dir")
        .arg("DOWNLOAD")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Downloads")
}

/// Only the file-name component of a shared name, so a crafted path cannot
/// escape the downloads dir.
pub(crate) fn safe_filename(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("archivo")
        .to_owned()
}

/// Atomically create an internal partial file in the destination filesystem.
/// It stays hidden from the user's final names until the complete payload has
/// been verified and synced.
pub(crate) fn create_partial(dir: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..10_000 {
        let sequence = NEXT_PARTIAL.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            ".magnetita-receive-{}-{sequence}.part",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve a unique partial file",
    ))
}

/// Atomically publish a complete partial without overwriting any existing
/// destination. A hard link is the portable no-clobber primitive available in
/// `std`; partial and destination share a directory, so they share a filesystem.
pub(crate) fn publish(partial: &Path, dir: &Path, name: &str) -> io::Result<PathBuf> {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 0..10_000 {
        let filename = match (n, ext) {
            (0, _) => name.to_owned(),
            (_, Some(ext)) => format!("{stem} ({n}).{ext}"),
            (_, None) => format!("{stem} ({n})"),
        };
        let candidate = dir.join(filename);
        match fs::hard_link(partial, &candidate) {
            Ok(()) => {
                let _ = fs::remove_file(partial);
                return Ok(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many files share the requested name",
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::{create_partial, publish, safe_filename, NEXT_PARTIAL};
    use std::sync::atomic::Ordering;

    #[test]
    fn a_remote_path_cannot_escape_the_download_directory() {
        assert_eq!(safe_filename("../../secrets.txt"), "secrets.txt");
        assert_eq!(safe_filename(""), "archivo");
    }

    #[test]
    fn publishing_is_atomic_and_never_clobbers_an_existing_name() {
        let root = std::env::temp_dir().join(format!(
            "magnetita-publish-{}-{}",
            std::process::id(),
            NEXT_PARTIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("foto.jpg"), b"existing").unwrap();

        let (partial, mut file) = create_partial(&root).unwrap();
        file.write_all(b"received").unwrap();
        file.sync_all().unwrap();
        drop(file);
        let destination = publish(&partial, &root, "foto.jpg").unwrap();

        assert_eq!(destination.file_name().unwrap(), "foto (1).jpg");
        assert_eq!(fs::read(root.join("foto.jpg")).unwrap(), b"existing");
        assert_eq!(fs::read(destination).unwrap(), b"received");
        assert!(!partial.exists());
        let _ = fs::remove_dir_all(root);
    }
}
