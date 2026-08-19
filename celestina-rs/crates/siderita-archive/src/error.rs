use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use siderita_ops::OpError;

/// Why an archive could not be read, extracted or written.
///
/// The filesystem half is [`OpError`] unchanged — the same cancellation,
/// already-exists and IO truths the copy/move verbs report — so a host that can
/// already show a failed paste can show a failed extraction. The variants added
/// here are the ones only an archive has.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveError {
    /// A filesystem failure, including [`OpError::Cancelled`].
    Op(OpError),
    /// The bytes are not one of the containers this domain handles.
    UnsupportedFormat { path: PathBuf },
    /// The format is readable but this domain does not create it.
    NotWritable { format: &'static str },
    /// The container is damaged or truncated.
    Malformed { path: PathBuf, reason: String },
    /// A member's stored name escapes the extraction root (absolute path, `..`,
    /// or an escaping symlink target). The extraction refuses it rather than
    /// writing outside the folder the person chose.
    UnsafeMember { name: String },
    /// A name that only a byte-oriented container can carry was asked of one
    /// that stores text (zip). Reported instead of mangling the name.
    NonUtf8Name { name: PathBuf },
    /// Nothing was asked to be compressed.
    NothingToCompress,
    /// The archive is encrypted and no password was supplied. The caller is
    /// expected to ask for one and try again; the domain never prompts.
    PasswordRequired { path: PathBuf },
    /// The supplied password does not open the archive.
    WrongPassword { path: PathBuf },
    /// The format is one this domain delegates, and no tool that reads it is
    /// installed. Names the tool so a host can say what to install.
    ToolMissing {
        format: &'static str,
        tool: &'static str,
    },
}

impl ArchiveError {
    /// Whether this is the cancellation the caller itself requested.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Op(OpError::Cancelled))
    }

    /// Whether the archive is waiting on a password: either none was given or
    /// the one given was wrong. The one question a host answers by *asking a
    /// person*, rather than by reporting a failure.
    pub fn needs_password(&self) -> bool {
        matches!(
            self,
            Self::PasswordRequired { .. } | Self::WrongPassword { .. }
        )
    }

    /// Builds a [`ArchiveError::Malformed`] from any container-level failure.
    pub(crate) fn malformed(path: &std::path::Path, reason: impl fmt::Display) -> Self {
        Self::Malformed {
            path: path.to_path_buf(),
            reason: reason.to_string(),
        }
    }
}

impl From<OpError> for ArchiveError {
    fn from(error: OpError) -> Self {
        Self::Op(error)
    }
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Op(error) => error.fmt(formatter),
            Self::UnsupportedFormat { path } => write!(
                formatter,
                "'{}' is not an archive Siderita can open",
                path.display()
            ),
            Self::NotWritable { format } => {
                write!(formatter, "Siderita does not create {format} archives")
            }
            Self::Malformed { path, reason } => write!(
                formatter,
                "'{}' is damaged or truncated: {reason}",
                path.display()
            ),
            Self::UnsafeMember { name } => write!(
                formatter,
                "the archive holds an entry that would be written outside the destination ('{name}')"
            ),
            Self::NonUtf8Name { name } => write!(
                formatter,
                "'{}' cannot be stored in a zip, whose names are text",
                name.display()
            ),
            Self::NothingToCompress => formatter.write_str("there is nothing to compress"),
            Self::PasswordRequired { path } => {
                write!(formatter, "'{}' is encrypted and needs a password", path.display())
            }
            Self::WrongPassword { path } => write!(
                formatter,
                "the password does not open '{}'",
                path.display()
            ),
            Self::ToolMissing { format, tool } => write!(
                formatter,
                "opening {format} archives needs {tool}, which is not installed"
            ),
        }
    }
}

impl Error for ArchiveError {}
