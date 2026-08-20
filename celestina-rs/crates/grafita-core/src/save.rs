//! The save that refuses rather than destroys.
//!
//! The sequence is fixed: re-resolve the target, prove it is still the file the
//! document was read from, write a unique sibling temporary, reproduce the
//! original's metadata onto it, re-verify the target once more, and only then
//! rename over it. Every refusal before the rename leaves the original intact
//! and removes the temporary it created; only the directory sync happens after
//! the rename, and its failure is reported as reduced durability, not as an
//! unsaved document.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use celestina_core::CancellationToken;

use crate::encoding::EncodeError;
use crate::history::Revision;
use crate::metadata::MetadataError;
use crate::target::{FileIdentity, Target};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

/// Everything a worker thread needs to write a document, and nothing that ties
/// it to the thread the document lives on.
#[derive(Clone, Debug)]
pub struct SaveRequest {
    target: Target,
    bytes: Vec<u8>,
    revision: Revision,
}

impl SaveRequest {
    #[must_use]
    pub fn new(target: Target, bytes: Vec<u8>, revision: Revision) -> Self {
        Self {
            target,
            bytes,
            revision,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &Target {
        &self.target
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// How durable a completed save is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Durability {
    /// Both the file and its directory entry were synced.
    Durable,
    /// The new bytes are in place and the rename succeeded, but the containing
    /// directory could not be synced, so a sudden power loss could still lose
    /// the entry. The document is saved; only its durability is reduced.
    Reduced { message: String },
}

/// A completed save.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveReport {
    /// The document revision these bytes came from. A host compares it with the
    /// document's current revision before clearing dirty state.
    pub revision: Revision,
    /// The identity of the file as it now stands, which the document must adopt
    /// so the next save does not mistake its own write for an external change.
    pub identity: FileIdentity,
    pub durability: Durability,
}

/// Why a save did not happen. Every variant here leaves the original file
/// exactly as it was.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveRefusal {
    /// The requested path no longer resolves to the file that was opened.
    Retargeted { expected: PathBuf, found: PathBuf },
    /// The resolved file changed on disk since it was read.
    ChangedUnderneath {
        expected: FileIdentity,
        found: FileIdentity,
    },
    /// The resolved file is gone.
    TargetMissing { path: PathBuf },
    /// Part of the original's metadata could not be reproduced.
    MetadataNotReproducible { source: MetadataError },
    /// The text holds a character the document's encoding has no byte for, so
    /// writing it would silently substitute or drop it. Nothing was written.
    Unrepresentable { source: EncodeError },
    /// An imported document's text no longer fits the structure it came from —
    /// a paragraph added or removed, which is structure this editor does not
    /// write. Nothing was written and the container is untouched.
    StructureChanged { detail: String },
    /// The save was cancelled before anything was published.
    Cancelled,
    /// Any other IO failure, tagged with the path it happened on.
    Io {
        path: PathBuf,
        kind: io::ErrorKind,
        message: String,
    },
}

impl SaveRefusal {
    fn io(path: &Path, error: &io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for SaveRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retargeted { expected, found } => write!(
                formatter,
                "the path now leads to '{}' instead of '{}'",
                found.display(),
                expected.display()
            ),
            Self::ChangedUnderneath { .. } => {
                formatter.write_str("the file changed on disk since it was opened")
            }
            Self::TargetMissing { path } => {
                write!(formatter, "'{}' no longer exists", path.display())
            }
            Self::MetadataNotReproducible { source } => {
                write!(formatter, "the original's metadata would be lost: {source}")
            }
            Self::Unrepresentable { source } => write!(
                formatter,
                "this text cannot be written without losing a character: {source}"
            ),
            Self::StructureChanged { detail } => write!(
                formatter,
                "this text no longer fits the document it came from: {detail}"
            ),
            Self::Cancelled => formatter.write_str("the save was cancelled"),
            Self::Io {
                path,
                kind,
                message,
            } => write!(
                formatter,
                "cannot write '{}': {message} ({kind:?})",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SaveRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MetadataNotReproducible { source } => Some(source),
            _ => None,
        }
    }
}

impl From<MetadataError> for SaveRefusal {
    fn from(source: MetadataError) -> Self {
        Self::MetadataNotReproducible { source }
    }
}

/// Writes a document to disk, or refuses without touching the original.
///
/// This performs blocking IO and belongs on a worker, never on a UI thread.
pub fn perform(
    request: &SaveRequest,
    cancellation: &CancellationToken,
) -> Result<SaveReport, SaveRefusal> {
    let target = request.target();
    let metadata = verify_target(target)?;

    let (temporary, mut file) = create_temporary(target.resolved(), target.parent())?;
    let outcome = write_and_publish(request, &temporary, &mut file, &metadata, cancellation);
    if outcome.is_err() {
        // A refusal must not leave a stray sibling next to the user's file.
        let _ = fs::remove_file(&temporary);
    }
    outcome
}

fn write_and_publish(
    request: &SaveRequest,
    temporary: &Path,
    file: &mut File,
    metadata: &fs::Metadata,
    cancellation: &CancellationToken,
) -> Result<SaveReport, SaveRefusal> {
    let target = request.target();

    file.write_all(request.bytes())
        .map_err(|error| SaveRefusal::io(temporary, &error))?;
    file.sync_all()
        .map_err(|error| SaveRefusal::io(temporary, &error))?;

    crate::metadata::reproduce(target.resolved(), metadata, temporary)?;

    if cancellation.is_cancelled() {
        return Err(SaveRefusal::Cancelled);
    }

    // The last look before publishing. It cannot close the window between this
    // check and the rename, but it does catch every change that landed while
    // the bytes were being written and synced.
    verify_target(target)?;

    fs::rename(temporary, target.resolved())
        .map_err(|error| SaveRefusal::io(target.resolved(), &error))?;

    let published = target
        .resolved()
        .metadata()
        .map_err(|error| SaveRefusal::io(target.resolved(), &error))?;

    // Past this point the document is saved. Syncing the directory only decides
    // whether the new entry survives a power loss, so its failure downgrades
    // durability instead of claiming the save did not happen.
    let durability = match File::open(target.parent()).and_then(|parent| parent.sync_all()) {
        Ok(()) => Durability::Durable,
        Err(error) => Durability::Reduced {
            message: error.to_string(),
        },
    };

    Ok(SaveReport {
        revision: request.revision(),
        identity: FileIdentity::of(&published),
        durability,
    })
}

fn verify_target(target: &Target) -> Result<fs::Metadata, SaveRefusal> {
    let resolved = target.requested().canonicalize().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            SaveRefusal::TargetMissing {
                path: target.requested().to_path_buf(),
            }
        } else {
            SaveRefusal::io(target.requested(), &error)
        }
    })?;
    if resolved != target.resolved() {
        return Err(SaveRefusal::Retargeted {
            expected: target.resolved().to_path_buf(),
            found: resolved,
        });
    }

    let metadata = resolved.metadata().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            SaveRefusal::TargetMissing { path: resolved }
        } else {
            SaveRefusal::io(&resolved, &error)
        }
    })?;
    let identity = FileIdentity::of(&metadata);
    if identity != target.identity() {
        return Err(SaveRefusal::ChangedUnderneath {
            expected: target.identity(),
            found: identity,
        });
    }
    Ok(metadata)
}

fn create_temporary(resolved: &Path, parent: &Path) -> Result<(PathBuf, File), SaveRefusal> {
    let name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("documento");
    for _ in 0..10_000 {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{name}.{}-{sequence}.grafita", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(SaveRefusal::io(&candidate, &error)),
        }
    }
    Err(SaveRefusal::Io {
        path: parent.to_path_buf(),
        kind: io::ErrorKind::AlreadyExists,
        message: "could not reserve a temporary next to the document".to_owned(),
    })
}

/// A completed "save as": the file that now exists and how durable it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedFile {
    /// The file the document may now adopt.
    pub target: Target,
    /// How durable the write turned out to be. Reported rather than assumed:
    /// the directory sync can fail, and a host that states durability it did
    /// not observe is telling the user something it does not know.
    pub durability: Durability,
}

/// Writes `bytes` to a path the document does not yet own, and returns the
/// target it may now adopt.
///
/// This is "guardar como", and it is deliberately *not* [`perform`]: there is no
/// prior identity to re-verify and no original metadata to reproduce, because
/// the document was never bound to this file. What it keeps is the part that
/// matters — a unique sibling temporary, written and synced in full, published
/// by an atomic rename — so a failure leaves whatever was there untouched and
/// never a half-written file.
///
/// An existing destination is overwritten, because the caller reached here
/// through a file chooser that already asked. Its metadata is reproduced, so
/// saving over a file does not quietly widen its permissions, and a destination
/// that is a symlink is resolved first, so the write lands on the file the link
/// names instead of replacing the link with a regular file — the same rule
/// [`perform`] follows through [`Target`].
pub fn create(path: &Path, bytes: &[u8]) -> Result<CreatedFile, SaveRefusal> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        .canonicalize()
        .map_err(|error| SaveRefusal::io(path, &error))?;
    let name = path.file_name().ok_or_else(|| {
        SaveRefusal::io(
            path,
            &io::Error::new(
                io::ErrorKind::InvalidInput,
                "the destination has no file name",
            ),
        )
    })?;
    // A destination that already exists is resolved through every link in it,
    // so the temporary is a sibling of the *real* file and the rename replaces
    // that file rather than the link pointing at it.
    let destination = parent.join(name);
    let destination = destination.canonicalize().unwrap_or(destination);
    let directory = destination
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .map_or(parent, Path::to_path_buf);

    // An existing destination keeps its own permissions and ownership: the user
    // asked to write *this* file, not to reset how it is protected.
    let existing = destination.metadata().ok().filter(fs::Metadata::is_file);

    let (temporary, mut file) = create_temporary(&destination, &directory)?;
    let outcome = (|| {
        file.write_all(bytes)
            .map_err(|error| SaveRefusal::io(&temporary, &error))?;
        file.sync_all()
            .map_err(|error| SaveRefusal::io(&temporary, &error))?;
        if let Some(metadata) = existing.as_ref() {
            crate::metadata::reproduce(&destination, metadata, &temporary)?;
        }
        drop(file);
        fs::rename(&temporary, &destination)
            .map_err(|error| SaveRefusal::io(&destination, &error))?;

        // Past the rename the file exists. Syncing the directory only decides
        // whether the new entry survives a power loss, so its failure lowers
        // the durability reported instead of claiming the save did not happen.
        let durability = match File::open(&directory).and_then(|handle| handle.sync_all()) {
            Ok(()) => Durability::Durable,
            Err(error) => Durability::Reduced {
                message: error.to_string(),
            },
        };
        let target =
            Target::resolve(&destination).map_err(|error| SaveRefusal::io(&destination, &error))?;
        Ok(CreatedFile { target, durability })
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    outcome
}
