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

use crate::encoding::{Encoding, EncodingGuess};
use crate::import::{ImportError, Imported};
use crate::probe::{classify_with, BinaryReason, Classification, DEFAULT_PROBE_BYTES};
use crate::target::Target;

/// The ceiling on a document Grafita will hold in memory. An editor is not the
/// right tool past this size, and refusing is better than exhausting the
/// session's memory.
pub const DEFAULT_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// How many times a read retries when the file changes while it is being read.
const READ_ATTEMPTS: u32 = 3;

/// Bounds a host puts on reading. The defaults suit an interactive editor.
// No `PartialEq`: comparing function pointers means nothing, and nothing
// compares limits.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The largest file that may become a document.
    pub max_bytes: u64,
    /// How much of a file the cheap probe inspects.
    pub probe_bytes: usize,
    /// Guesses the encoding of bytes that are not UTF-8. `None` keeps the
    /// refusal: this crate carries no detector, so a host that wants one (the
    /// Grafita application does) supplies it.
    pub guess: Option<EncodingGuess>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            probe_bytes: DEFAULT_PROBE_BYTES,
            guess: None,
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
    /// The container this text came out of, when it came out of one. Its
    /// presence is what makes the document an imported one: the text is a
    /// projection and saving goes back through the container.
    pub imported: Option<Imported>,
}

/// Why a file did not become a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenRefusal {
    /// The content is not text.
    NotText { reason: BinaryReason },
    /// The content is a container, and its document could not be read.
    NotImportable { detail: String },
    /// The content is text in an encoding that cannot be mapped back, so it may
    /// be shown but never advertised as safely editable.
    UnsupportedEncoding { detail: String },
    /// The file, or the content a container unpacks to, is larger than the
    /// configured ceiling. `size` is the file's length, or, when reading
    /// stopped at the ceiling, the first length known to pass it: a lower
    /// bound, because the rest was never read.
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
            Self::NotImportable { detail } => write!(
                formatter,
                "this file is a container Grafita cannot edit: {detail}"
            ),
            Self::UnsupportedEncoding { detail } => write!(
                formatter,
                "this text cannot be edited without losing its bytes: {detail}"
            ),
            Self::TooLarge { size, limit } => write!(
                formatter,
                "this document has at least {size} bytes and the editor accepts up to {limit}"
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
        classification: classify_with(&buffer, complete, limits.guess),
        complete,
    })
}

/// Reads a file completely and decodes it with the encoding the caller names.
///
/// This is the only way a single-byte table, an unmarked UTF-16 file or a
/// UTF-32 file becomes a document: nothing in those bytes proves which one they
/// are, and this crate does not guess. The classification is skipped on
/// purpose — the caller has answered the question it exists to ask — but the
/// contract is not: the decoded text is re-encoded and compared with the bytes
/// that were read, so an encoding that cannot reproduce the file is refused
/// before it can be edited.
///
/// This performs blocking IO and belongs on a worker, never on a UI thread.
pub fn open_with(
    path: &Path,
    encoding: Encoding,
    generation: Generation,
    limits: Limits,
    cancellation: &CancellationToken,
) -> Result<OpenedFile, OpenRefusal> {
    read_document(path, generation, limits, cancellation, |bytes| {
        let text = encoding
            .decode(bytes)
            .map_err(|reason| OpenRefusal::UnsupportedEncoding {
                detail: reason.to_string(),
            })?;
        match encoding.encode(&text) {
            Ok(written) if written == bytes => Ok((encoding, text, None)),
            Ok(_) => Err(OpenRefusal::UnsupportedEncoding {
                detail: format!(
                    "{} reads this file but does not write it back byte for byte",
                    encoding.label()
                ),
            }),
            Err(source) => Err(OpenRefusal::UnsupportedEncoding {
                detail: source.to_string(),
            }),
        }
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
    read_document(path, generation, limits, cancellation, |bytes| {
        // One decision, not two. The probe and the open must agree on what a
        // file is, and they agree by asking the same function: a classify that
        // said "binary" while the reader knew better is exactly how a `.docx`
        // came to be refused before anything tried to read it.
        let encoding = match classify_with(bytes, true, limits.guess) {
            Classification::ImportedDocument => {
                return match Imported::open(bytes.to_vec(), limits.max_bytes, cancellation) {
                    Ok(imported) => {
                        let text = imported.text().to_owned();
                        Ok((Encoding::Utf8, text, Some(imported)))
                    }
                    Err(ImportError::Cancelled) => Err(OpenRefusal::Cancelled),
                    Err(source) => Err(match source.exceeded_limit() {
                        Some(limit) => OpenRefusal::TooLarge {
                            size: limit.saturating_add(1),
                            limit,
                        },
                        None => OpenRefusal::NotImportable {
                            detail: source.to_string(),
                        },
                    }),
                }
            }
            Classification::EditableText { encoding } => encoding,
            Classification::Binary { reason } => return Err(OpenRefusal::NotText { reason }),
            Classification::UnsupportedEncoding { reason } => {
                return Err(OpenRefusal::UnsupportedEncoding {
                    detail: reason.to_string(),
                })
            }
        };
        let text = encoding
            .decode(bytes)
            .map_err(|reason| OpenRefusal::UnsupportedEncoding {
                detail: reason.to_string(),
            })?;
        Ok((encoding, text, None))
    })
}

/// The read both open paths share: the retry, the two identity stamps and the
/// size ceiling. Only the decision of what the bytes mean differs, and that is
/// what `interpret` answers.
fn read_document(
    path: &Path,
    generation: Generation,
    limits: Limits,
    cancellation: &CancellationToken,
    interpret: impl Fn(&[u8]) -> Result<(Encoding, String, Option<Imported>), OpenRefusal>,
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

        // The `stat` above is only a first answer: the file can grow before
        // it is read, so the read itself stops one byte past the ceiling.
        let mut bytes = Vec::new();
        fs::File::open(target.resolved())
            .and_then(|file| {
                file.take(limits.max_bytes.saturating_add(1))
                    .read_to_end(&mut bytes)
            })
            .map_err(|error| OpenRefusal::io(path, &error))?;
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

        let (encoding, text, imported) = interpret(&bytes)?;

        return Ok(OpenedFile {
            generation,
            target,
            encoding,
            text,
            imported,
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
