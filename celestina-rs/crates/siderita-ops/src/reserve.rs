//! Renaming onto a name nobody else can take in the meantime.
//!
//! `std::fs::rename` follows POSIX: if the destination exists, it is replaced,
//! silently and atomically. Every move verb here therefore looks first and
//! renames second — and between those two steps another writer can create the
//! name, which the rename then destroys.
//!
//! That window used to be justified as "a single-user manager races only with
//! other applications". It stops being true the moment this manager runs two of
//! its own operations at once, and the loss it costs is exactly the one the
//! whole crate exists to prevent.
//!
//! Closing it without `unsafe` (this crate forbids FFI, so `renameat2`'s
//! `RENAME_NOREPLACE` is out of reach) is done by *reserving* the name: the
//! destination is created empty and exclusively — `create_new` for a file,
//! `create_dir` for a directory, both atomic and both failing when the name is
//! already taken — and the rename then lands on the reservation this very call
//! owns. Whatever it replaces, it is never someone else's data.

use std::fs;
use std::io;
use std::path::Path;

use crate::error::OpError;

/// Renames `source` to `destination`, refusing to replace anything that is
/// already there.
///
/// The reservation matches the source's kind because `rename(2)` will not put a
/// directory over a file or a file over a directory. A directory reservation is
/// left empty, which is the one case POSIX does allow a directory to replace.
///
/// The failure is handed back raw rather than as an [`OpError`], because one
/// caller must still tell `EXDEV` — a move across filesystems, which falls back
/// to copy → verify → remove — from every other failure.
///
/// On any failure the reservation is removed, so a failed move leaves no empty
/// stub behind and the fallback path finds the destination free.
pub(crate) fn rename_without_replacing(
    source: &Path,
    destination: &Path,
    source_is_directory: bool,
) -> Result<(), RenameFailure> {
    reserve(destination, source_is_directory)?;
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            release(destination, source_is_directory);
            Err(RenameFailure::Io(error))
        }
    }
}

/// Why a no-replace rename did not happen.
#[derive(Debug)]
pub(crate) enum RenameFailure {
    /// Another entry already holds the destination name.
    Taken,
    /// The rename itself failed; the reservation has been given back.
    Io(io::Error),
}

impl RenameFailure {
    /// The failure as this crate's error, for a caller with nothing special to
    /// decide.
    pub(crate) fn into_op_error(self, destination: &Path) -> OpError {
        match self {
            RenameFailure::Taken => OpError::AlreadyExists {
                path: destination.to_path_buf(),
            },
            RenameFailure::Io(error) => OpError::io(destination, &error),
        }
    }
}

/// Takes the name, or reports that somebody else holds it.
fn reserve(destination: &Path, directory: bool) -> Result<(), RenameFailure> {
    let taken = if directory {
        fs::create_dir(destination)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map(drop)
    };
    match taken {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(RenameFailure::Taken),
        Err(error) => Err(RenameFailure::Io(error)),
    }
}

/// Gives the reservation back after a failed rename.
fn release(destination: &Path, directory: bool) {
    let _ = if directory {
        fs::remove_dir(destination)
    } else {
        fs::remove_file(destination)
    };
}

#[cfg(test)]
mod tests {
    use super::rename_without_replacing;
    use std::fs;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("siderita-reserve-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn a_file_moves_onto_a_free_name() {
        let dir = scratch("free");
        let source = dir.join("uno.txt");
        fs::write(&source, b"datos").expect("write");
        let destination = dir.join("dos.txt");
        rename_without_replacing(&source, &destination, false).expect("rename");
        assert_eq!(fs::read(&destination).expect("read"), b"datos");
        assert!(!source.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_occupied_name_is_never_replaced() {
        let dir = scratch("occupied");
        let source = dir.join("uno.txt");
        fs::write(&source, b"nuevo").expect("write");
        let destination = dir.join("dos.txt");
        fs::write(&destination, b"existente").expect("write");

        let error = rename_without_replacing(&source, &destination, false)
            .expect_err("must refuse")
            .into_op_error(&destination);
        assert!(
            matches!(error, crate::error::OpError::AlreadyExists { .. }),
            "{error:?}"
        );
        // Both entries survive, byte for byte.
        assert_eq!(fs::read(&destination).expect("read"), b"existente");
        assert_eq!(fs::read(&source).expect("read"), b"nuevo");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_moves_as_a_directory() {
        let dir = scratch("directory");
        let source = dir.join("carpeta");
        fs::create_dir(&source).expect("mkdir");
        fs::write(source.join("dentro.txt"), b"x").expect("write");
        let destination = dir.join("movida");
        rename_without_replacing(&source, &destination, true).expect("rename");
        assert!(destination.join("dentro.txt").is_file());
        assert!(!source.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_rename_leaves_no_stub_where_it_tried() {
        let dir = scratch("stub");
        let missing = dir.join("no-existe");
        let destination = dir.join("destino.txt");
        let error = rename_without_replacing(&missing, &destination, false)
            .expect_err("must fail")
            .into_op_error(&destination);
        assert!(
            matches!(error, crate::error::OpError::Io { .. }),
            "{error:?}"
        );
        assert!(
            !destination.exists(),
            "the reservation outlived the failed rename"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
