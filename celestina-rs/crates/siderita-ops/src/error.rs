use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::name::NameError;

/// Why a filesystem operation could not complete.
///
/// Every variant carries enough context to show the user a truthful message and
/// to reason about what did — or, deliberately, did not — change on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpError {
    /// The cancellation token was tripped before the operation touched disk.
    Cancelled,
    /// The requested name is not a usable single path component.
    InvalidName(NameError),
    /// The target already exists and the operation refuses to overwrite it.
    AlreadyExists { path: PathBuf },
    /// A required source path was missing when the operation ran.
    SourceMissing { path: PathBuf },
    /// The destination lies inside the source, which would recurse forever.
    DestinationInsideSource {
        source: PathBuf,
        destination: PathBuf,
    },
    /// The source is a file type this domain will not copy (socket, fifo, device).
    UnsupportedFileType { path: PathBuf },
    /// Any other IO failure, tagged with the path it happened on.
    Io {
        path: PathBuf,
        kind: io::ErrorKind,
        message: String,
    },
}

impl OpError {
    /// Builds an [`OpError::Io`] from a `std::io::Error` and the path it failed on.
    pub(crate) fn io(path: &Path, error: &io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("the operation was cancelled"),
            Self::InvalidName(reason) => write!(formatter, "invalid name: {reason}"),
            Self::AlreadyExists { path } => {
                write!(formatter, "'{}' already exists", path.display())
            }
            Self::SourceMissing { path } => {
                write!(formatter, "'{}' no longer exists", path.display())
            }
            Self::DestinationInsideSource {
                source,
                destination,
            } => write!(
                formatter,
                "cannot copy '{}' into itself ('{}')",
                source.display(),
                destination.display()
            ),
            Self::UnsupportedFileType { path } => {
                write!(
                    formatter,
                    "'{}' is a file type Siderita cannot copy",
                    path.display()
                )
            }
            Self::Io {
                path,
                kind,
                message,
            } => write!(
                formatter,
                "cannot operate on '{}': {message} ({kind:?})",
                path.display()
            ),
        }
    }
}

impl Error for OpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidName(reason) => Some(reason),
            _ => None,
        }
    }
}

impl From<NameError> for OpError {
    fn from(reason: NameError) -> Self {
        Self::InvalidName(reason)
    }
}
