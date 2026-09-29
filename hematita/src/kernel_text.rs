//! The one reader of the kernel's text files — `/proc`, `/sys` and the
//! account table — for the sampler, the signal path and the storage section.
//!
//! A whole file is read through `celestina_core::atomic_file::read_bounded`:
//! a regular file only, opened without waiting for a writer, never more than
//! [`TEXT_LIMIT`] bytes. A command line is the one file read in part: only
//! its first argument is ever shown, so only its first [`ARGV0_LIMIT`] bytes
//! are read, however long the whole command line is. Both block on the
//! filesystem and belong on a worker thread, or on the Qt thread only for a
//! single small read a person asked for.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use celestina_core::atomic_file::{self, ReadError};
use rustix::fs::{open, Mode, OFlags};

/// The largest kernel text file read whole. The biggest ones read are
/// `/proc/cpuinfo` on a machine with hundreds of threads and the mount table
/// of a host running containers, both well under a mebibyte.
pub const TEXT_LIMIT: u64 = 4 * 1024 * 1024;

/// How much of `/proc/PID/cmdline` is read: `PATH_MAX`, which bounds any
/// first argument that names a program.
pub const ARGV0_LIMIT: u64 = 4096;

/// A command line is opened read-only, never waiting for a writer.
const PREFIX_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

/// Why a kernel text file has no text for the caller.
#[derive(Debug)]
pub enum TextError {
    /// Nothing is at the path: a process that has just exited, a device
    /// without the attribute.
    Missing { path: PathBuf },
    /// Not a regular file, larger than the limit, or unreadable.
    File(ReadError),
    /// The bytes are not UTF-8.
    NotText { path: PathBuf },
}

impl TextError {
    /// Whether the file was refused for holding more than the limit.
    #[must_use]
    pub fn is_too_large(&self) -> bool {
        matches!(self, Self::File(ReadError::TooLarge { .. }))
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(formatter, "{} does not exist", path.display()),
            Self::File(error) => error.fmt(formatter),
            Self::NotText { path } => write!(formatter, "{} is not UTF-8", path.display()),
        }
    }
}

impl Error for TextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::File(error) => Some(error),
            Self::Missing { .. } | Self::NotText { .. } => None,
        }
    }
}

/// The whole text of a kernel file of at most [`TEXT_LIMIT`] bytes.
///
/// # Errors
///
/// [`TextError`]: missing, refused (not regular, too large), unreadable, or
/// not UTF-8.
pub fn read_text(path: &Path) -> Result<String, TextError> {
    read_text_within(path, TEXT_LIMIT)
}

/// [`read_text`] with another limit.
///
/// # Errors
///
/// As [`read_text`].
pub fn read_text_within(path: &Path, limit: u64) -> Result<String, TextError> {
    let bytes = atomic_file::read_bounded(path, limit)
        .map_err(TextError::File)?
        .ok_or_else(|| TextError::Missing {
            path: path.to_path_buf(),
        })?;
    String::from_utf8(bytes).map_err(|_| TextError::NotText {
        path: path.to_path_buf(),
    })
}

/// At most the first `limit` bytes of a regular file: a prefix, not a
/// verdict on the file's size. A FIFO or a device in its place is refused
/// without being read or waited on.
///
/// # Errors
///
/// The open or the read failed, or the path is not a regular file.
pub fn read_prefix(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = File::from(open(path, PREFIX_FLAGS, Mode::empty())?);
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of the test's own under the temporary directory, removed
    /// when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "hematita-kernel-text-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a scratch folder");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_within_the_limit_is_read_whole() {
        let scratch = Scratch::new("small");
        let path = scratch.0.join("stat");
        std::fs::write(&path, "cpu  1 2 3 4\n").expect("a fixture");
        assert_eq!(read_text(&path).expect("read"), "cpu  1 2 3 4\n");
    }

    #[test]
    fn an_oversized_file_is_refused_not_read_whole() {
        let scratch = Scratch::new("large");
        let path = scratch.0.join("cpuinfo");
        std::fs::write(&path, "x".repeat(4097)).expect("a fixture");
        let refused = read_text_within(&path, 4096).expect_err("over the limit");
        assert!(refused.is_too_large(), "{refused}");
        assert_eq!(
            read_text_within(&path, 4097).expect("at the limit").len(),
            4097
        );
    }

    #[test]
    fn a_missing_file_and_a_fifo_are_typed_and_never_waited_on() {
        let scratch = Scratch::new("fifo");
        let missing = scratch.0.join("gone");
        assert!(matches!(
            read_text(&missing),
            Err(TextError::Missing { .. })
        ));
        let fifo = scratch.0.join("status");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::RUSR | Mode::WUSR).expect("a FIFO");
        assert!(matches!(read_text(&fifo), Err(TextError::File(_))));
        assert!(read_prefix(&fifo, ARGV0_LIMIT).is_err());
    }

    #[test]
    fn a_huge_command_line_is_read_only_up_to_its_first_argument_bound() {
        let scratch = Scratch::new("cmdline");
        let path = scratch.0.join("cmdline");
        let mut cmdline = b"/usr/lib/electron/electron\0".to_vec();
        cmdline.extend(std::iter::repeat_n(b'a', 1024 * 1024));
        std::fs::write(&path, &cmdline).expect("a fixture");
        let prefix = read_prefix(&path, ARGV0_LIMIT).expect("read");
        assert_eq!(prefix.len(), 4096);
        let words = hematita_core::process::parse_cmdline(&prefix);
        assert_eq!(
            words.first().map(String::as_str),
            Some("/usr/lib/electron/electron")
        );
    }

    #[test]
    fn text_that_is_not_utf8_is_typed() {
        let scratch = Scratch::new("bytes");
        let path = scratch.0.join("comm");
        std::fs::write(&path, [0xff, 0xfe]).expect("a fixture");
        assert!(matches!(read_text(&path), Err(TextError::NotText { .. })));
    }
}
