//! What the controller can reverse, and how a collision is settled.
//!
//! Two small vocabularies the write verbs share: `fileops` records one, `paste`
//! decides the other and `trash` reads both. They live beside the verbs rather
//! than inside the bridge module, which owns the Qt surface and not the meaning
//! of an operation.

use std::ffi::OsString;
use std::path::PathBuf;

/// How to reverse the last loss-free operation. Only the three verbs the
/// roadmap names as undoable are recorded — create and copy are not, since
/// undoing them would mean deleting data the user did not ask to lose.
pub(crate) enum UndoAction {
    /// A rename: the entry now sits at `renamed`; put its `old_name` back.
    Rename {
        renamed: PathBuf,
        old_name: OsString,
    },
    /// One or more moves (a cut-paste): move each entry from where it landed
    /// back into the directory it came from.
    Move { entries: Vec<(PathBuf, PathBuf)> },
    /// One or more sends-to-Trash: restore each from its recorded `.trashinfo`.
    Trash { infos: Vec<PathBuf> },
}

impl UndoAction {
    /// A short Spanish label for what undo will reverse, for the menu/tooltip.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Rename { .. } => "Deshacer renombrar",
            Self::Move { .. } => "Deshacer mover",
            Self::Trash { .. } => "Deshacer enviar a la papelera",
        }
    }
}

/// How to resolve entries whose paste destination already exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConflictStrategy {
    /// Leave the existing entry; the source is not pasted.
    Skip,
    /// Send the existing entry to Trash (recoverable), then paste over it.
    Replace,
    /// Paste beside the existing entry under a freed "(copia)" name.
    KeepBoth,
}

impl ConflictStrategy {
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        match key {
            "skip" => Some(Self::Skip),
            "replace" => Some(Self::Replace),
            "keepboth" => Some(Self::KeepBoth),
            _ => None,
        }
    }
}
