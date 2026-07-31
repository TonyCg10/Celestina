//! Handing a local file to the backend without mangling its name.
//!
//! The binding's command API takes `&str`, but a Unix filename is bytes and the
//! catalogue deliberately keeps non-UTF-8 names addressable. Converting lossily
//! would open *a different file* — sometimes silently, which is the worst
//! possible failure for a media library.
//!
//! So a path that is valid UTF-8 goes to the backend as a path, and one that is
//! not is opened here and handed over as `fd://<n>`. The open file must outlive
//! the job, which is why the handle owns it.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use crate::error::{EngineError, EngineResult};

#[derive(Debug)]
pub struct SourceHandle {
    url: String,
    /// Kept open for the whole job: `fd://` is only valid while it is.
    _descriptor: Option<File>,
}

impl SourceHandle {
    /// Opens `path` for the backend. The path must be absolute — a relative one
    /// would resolve against a working directory the engine does not control.
    pub fn open(path: &Path) -> EngineResult<Self> {
        if !path.is_absolute() {
            return Err(EngineError::UnusableSource {
                path: path.to_path_buf(),
                reason: "the engine only opens absolute paths",
            });
        }

        if let Some(text) = path.to_str() {
            return Ok(Self {
                url: text.to_owned(),
                _descriptor: None,
            });
        }

        let file = File::open(path).map_err(|source| EngineError::Io {
            operation: "open source",
            path: path.to_path_buf(),
            source,
        })?;
        let url = format!("fd://{}", file.as_raw_fd());
        Ok(Self {
            url,
            _descriptor: Some(file),
        })
    }

    /// What to hand the backend.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Whether the name had to go through a descriptor. Format detection then
    /// rests on content alone, since there is no extension to read.
    #[must_use]
    pub fn is_descriptor(&self) -> bool {
        self._descriptor.is_some()
    }
}

/// A best-effort label for logs. Never use it to reopen anything.
#[must_use]
pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The path a caller must not have modified between two operations.
#[must_use]
pub fn owned(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::SourceHandle;
    use crate::error::EngineError;
    use std::path::Path;

    #[test]
    fn a_utf8_path_goes_to_the_backend_unchanged() {
        let handle = SourceHandle::open(Path::new("/home/toni/Música/canción ñ.flac"))
            .expect("absolute path");

        assert_eq!(handle.url(), "/home/toni/Música/canción ñ.flac");
        assert!(!handle.is_descriptor());
    }

    #[test]
    fn a_relative_path_is_refused_rather_than_resolved() {
        let error = SourceHandle::open(Path::new("clip.mp4")).expect_err("relative path");
        assert!(matches!(error, EngineError::UnusableSource { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_name_travels_as_a_descriptor() {
        use std::ffi::OsStr;
        use std::io::Write;
        use std::os::unix::ffi::OsStrExt;

        let directory = std::env::temp_dir().join("fluorita-engine-source-test");
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let path = directory.join(OsStr::from_bytes(b"mal-\xFF.mp4"));
        let mut file = std::fs::File::create(&path).expect("fixture");
        file.write_all(b"not really media").expect("fixture bytes");

        let handle = SourceHandle::open(&path).expect("absolute path");

        assert!(handle.is_descriptor());
        assert!(handle.url().starts_with("fd://"));
        assert!(
            !handle.url().contains('\u{FFFD}'),
            "the replacement character would mean the name was mangled"
        );

        std::fs::remove_file(&path).ok();
    }
}
