//! Reading a file into a document, or saying honestly why it cannot be one.
//!
//! The target is resolved and stamped before the read and re-stamped after it,
//! so a file rewritten mid-read is caught instead of producing a document made
//! of two different versions. The classification that decides "editable" is the
//! same one the cheap probe runs, only over the complete bytes.

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use celestina_core::{CancellationToken, Generation};

use crate::encoding::Encoding;
use crate::probe::{classify, BinaryReason, Classification, DEFAULT_PROBE_BYTES};
use crate::target::Target;

/// The ceiling on a document Grafita will hold in memory. An editor is not the
/// right tool past this size, and refusing is better than exhausting the
/// session's memory.
pub const DEFAULT_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// How many times a read retries when the file changes while it is being read.
const READ_ATTEMPTS: u32 = 3;

/// Bounds a host puts on reading. The defaults suit an interactive editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// The largest file that may become a document.
    pub max_bytes: u64,
    /// How much of a file the cheap probe inspects.
    pub probe_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            probe_bytes: DEFAULT_PROBE_BYTES,
        }
    }
}

/// What a cheap probe found, stamped with the request it answers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeOutcome {
    pub generation: Generation,
    pub path: PathBuf,
    pub classification: Classification,
    /// Whether the classification saw the whole file. A prefix verdict is good
    /// enough to offer the editor; opening re-checks the complete bytes.
    pub complete: bool,
}

/// A file read into text, with everything a later save must re-verify.
#[derive(Clone, Debug)]
pub struct OpenedFile {
    pub generation: Generation,
    pub target: Target,
    pub encoding: Encoding,
    pub text: String,
}

/// Why a file did not become a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenRefusal {
    /// The content is not text.
    NotText { reason: BinaryReason },
    /// The content is text in an encoding that cannot be mapped back, so it may
    /// be shown but never advertised as safely editable.
    UnsupportedEncoding { detail: String },
    /// The file is larger than the configured ceiling.
    TooLarge { size: u64, limit: u64 },
    /// The file kept changing while it was being read.
    ChangedWhileReading { path: PathBuf },
    /// The read was cancelled.
    Cancelled,
    /// Any other IO failure, tagged with the path it happened on.
    Io {
        path: PathBuf,
        kind: io::ErrorKind,
        message: String,
    },
}

impl OpenRefusal {
    fn io(path: &Path, error: &io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for OpenRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotText { .. } => formatter.write_str("this file is not text"),
            Self::UnsupportedEncoding { detail } => write!(
                formatter,
                "this text cannot be edited without losing its bytes: {detail}"
            ),
            Self::TooLarge { size, limit } => write!(
                formatter,
                "this file has {size} bytes and the editor accepts up to {limit}"
            ),
            Self::ChangedWhileReading { path } => write!(
                formatter,
                "'{}' kept changing while it was being read",
                path.display()
            ),
            Self::Cancelled => formatter.write_str("the read was cancelled"),
            Self::Io {
                path,
                kind,
                message,
            } => write!(
                formatter,
                "cannot read '{}': {message} ({kind:?})",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OpenRefusal {}

/// Classifies a file by reading at most `limits.probe_bytes` of it.
///
/// This is what a host runs on a keystroke: cheap enough to answer immediately,
/// and never a promise, since [`open`] re-checks the complete file.
pub fn probe(
    path: &Path,
    generation: Generation,
    limits: Limits,
    cancellation: &CancellationToken,
) -> Result<ProbeOutcome, OpenRefusal> {
    if cancellation.is_cancelled() {
        return Err(OpenRefusal::Cancelled);
    }
    let target = Target::resolve(path).map_err(|error| OpenRefusal::io(path, &error))?;
    let size = target.identity().size;
    let wanted = limits.probe_bytes as u64;
    let mut buffer = vec![0u8; size.min(wanted) as usize];
    let mut file =
        fs::File::open(target.resolved()).map_err(|error| OpenRefusal::io(path, &error))?;
    let read = read_fully(&mut file, &mut buffer).map_err(|error| OpenRefusal::io(path, &error))?;
    buffer.truncate(read);

    let complete = (read as u64) >= size;
    Ok(ProbeOutcome {
        generation,
        path: path.to_path_buf(),
        classification: classify(&buffer, complete),
        complete,
    })
}

/// Reads a file completely and decodes it, or refuses with a reason.
///
/// This performs blocking IO and belongs on a worker, never on a UI thread.
pub fn open(
    path: &Path,
    generation: Generation,
    limits: Limits,
    cancellation: &CancellationToken,
) -> Result<OpenedFile, OpenRefusal> {
    for _ in 0..READ_ATTEMPTS {
        if cancellation.is_cancelled() {
            return Err(OpenRefusal::Cancelled);
        }
        let mut target = Target::resolve(path).map_err(|error| OpenRefusal::io(path, &error))?;
        if target.identity().size > limits.max_bytes {
            return Err(OpenRefusal::TooLarge {
                size: target.identity().size,
                limit: limits.max_bytes,
            });
        }

        let bytes = fs::read(target.resolved()).map_err(|error| OpenRefusal::io(path, &error))?;
        if cancellation.is_cancelled() {
            return Err(OpenRefusal::Cancelled);
        }

        // The file must still be the same file, at the same version, as when
        // the identity was taken. Otherwise these bytes may be half of one
        // version and half of another.
        let after = Target::resolve(path).map_err(|error| OpenRefusal::io(path, &error))?;
        if after.resolved() != target.resolved() || after.identity() != target.identity() {
            continue;
        }
        // The size the write actually produced is the one a save compares
        // against; a file that grew between `stat` and `read` would otherwise
        // look changed on the very first save.
        target = after;

        if bytes.len() as u64 > limits.max_bytes {
            return Err(OpenRefusal::TooLarge {
                size: bytes.len() as u64,
                limit: limits.max_bytes,
            });
        }

        let encoding = match classify(&bytes, true) {
            Classification::EditableText { encoding } => encoding,
            Classification::Binary { reason } => return Err(OpenRefusal::NotText { reason }),
            Classification::UnsupportedEncoding { reason } => {
                return Err(OpenRefusal::UnsupportedEncoding {
                    detail: reason.to_string(),
                })
            }
        };
        let text = encoding
            .decode(&bytes)
            .map_err(|reason| OpenRefusal::UnsupportedEncoding {
                detail: reason.to_string(),
            })?;

        return Ok(OpenedFile {
            generation,
            target,
            encoding,
            text,
        });
    }
    Err(OpenRefusal::ChangedWhileReading {
        path: path.to_path_buf(),
    })
}

fn read_fully(file: &mut fs::File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}
