//! The same small change, to many files at once.
//!
//! **What may be applied to a selection, and what may not.** A batch offers
//! only operations that mean the same thing on every picture it is given:
//! turning, mirroring, and removing what a photograph carries. Cropping and
//! resizing are absolute — a rectangle measured on one photograph names a
//! different part of the next one, and a size that fits one distorts another —
//! so they are individual by construction. Annotation is individual by
//! definition: a word belongs where it was written.
//!
//! That is not a simplification of a batch editor. It is the whole of what a
//! batch can honestly be without asking the person to accept a result they did
//! not see, and it happens to be the two things a folder of photographs
//! actually needs: all of them are sideways, or all of them are about to be
//! sent to somebody.
//!
//! **Why the accounting is a type.** A run over forty files will meet files it
//! cannot act on — a PNG has no EXIF to remove, an MP4 is not an image — and
//! files that fail. Reporting "done" for a run in which eleven items were
//! skipped is the kind of quiet success this counts instead.

use crate::edit::{EditCapabilities, Operation};
use crate::media::MediaKind;
use crate::metadata::MetadataCapabilities;
use std::path::Path;

/// What a selection can be asked to do to itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BatchOperation {
    /// A quarter turn, clockwise or anticlockwise.
    Turn { clockwise: bool },
    /// A mirror across one axis.
    Mirror { horizontal: bool },
    /// Remove everything the file says about where, when and with what it was
    /// made.
    Forget,
}

impl BatchOperation {
    pub const ALL: [Self; 5] = [
        Self::Turn { clockwise: true },
        Self::Turn { clockwise: false },
        Self::Mirror { horizontal: true },
        Self::Mirror { horizontal: false },
        Self::Forget,
    ];

    /// The [`Operation`] this is, for the operations that are edits. `Forget`
    /// is not one: it rewrites a header and draws nothing.
    #[must_use]
    pub const fn as_edit(self) -> Option<Operation> {
        match self {
            Self::Turn { .. } => Some(Operation::Rotate),
            Self::Mirror { .. } => Some(Operation::Flip),
            Self::Forget => None,
        }
    }

    /// Whether one item admits this operation, decided from its kind and its
    /// name — before the run opens anything.
    ///
    /// Both answers come from the matrices that already own them. A batch that
    /// decided for itself which files it could act on would be a third opinion
    /// beside the editor's and the metadata panel's.
    #[must_use]
    pub fn admits(self, kind: MediaKind, path: &Path) -> bool {
        match self.as_edit() {
            Some(operation) => EditCapabilities::of(kind, path).admits(operation).is_some(),
            None => MetadataCapabilities::of(kind, path).strips_private_facts(),
        }
    }
}

/// What became of one item in a run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemOutcome {
    /// The file was rewritten.
    Done,
    /// The item does not admit this operation, so nothing was attempted. Not a
    /// failure: a run over a mixed folder is expected to meet these.
    Skipped,
    /// It was attempted and refused.
    Failed,
}

/// The running tally of one batch.
///
/// Kept as counts rather than as a list of names: a surface shows progress and
/// a sentence at the end, and holding forty paths to build that sentence would
/// be holding them for nothing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatchProgress {
    pub total: usize,
    pub done: usize,
    pub skipped: usize,
    pub failed: usize,
    /// True once the run stopped early because it was asked to.
    pub cancelled: bool,
}

impl BatchProgress {
    #[must_use]
    pub const fn of(total: usize) -> Self {
        Self {
            total,
            done: 0,
            skipped: 0,
            failed: 0,
            cancelled: false,
        }
    }

    /// Records one item's outcome.
    pub fn record(&mut self, outcome: ItemOutcome) {
        match outcome {
            ItemOutcome::Done => self.done += 1,
            ItemOutcome::Skipped => self.skipped += 1,
            ItemOutcome::Failed => self.failed += 1,
        }
    }

    /// How many have been accounted for.
    #[must_use]
    pub const fn settled(&self) -> usize {
        self.done + self.skipped + self.failed
    }

    /// Whether every item has been accounted for.
    #[must_use]
    pub const fn finished(&self) -> bool {
        self.settled() >= self.total
    }

    /// Whether anything at all was written. A run that changed nothing is not
    /// a success to be reported as one.
    #[must_use]
    pub const fn changed_anything(&self) -> bool {
        self.done > 0
    }
}

#[cfg(test)]
mod tests {
    use super::{BatchOperation, BatchProgress, ItemOutcome};
    use crate::edit::Operation;
    use crate::media::MediaKind;
    use std::path::Path;

    #[test]
    fn a_batch_offers_only_what_means_the_same_on_every_picture() {
        // Five operations, and not one of them is a crop, a resize or an
        // annotation.
        assert_eq!(BatchOperation::ALL.len(), 5);
        assert!(BatchOperation::ALL.iter().all(|operation| !matches!(
            operation.as_edit(),
            Some(Operation::Crop | Operation::Resize | Operation::Annotate)
        )));
    }

    #[test]
    fn what_an_item_admits_comes_from_the_matrices_that_already_own_it() {
        let turn = BatchOperation::Turn { clockwise: true };
        let forget = BatchOperation::Forget;

        assert!(turn.admits(MediaKind::Image, Path::new("/m/foto.jpg")));
        assert!(turn.admits(MediaKind::Image, Path::new("/m/captura.png")));
        assert!(
            !turn.admits(MediaKind::Video, Path::new("/m/clip.mkv")),
            "a film is not turned by the picture editor"
        );

        assert!(forget.admits(MediaKind::Image, Path::new("/m/foto.jpg")));
        assert!(
            !forget.admits(MediaKind::Image, Path::new("/m/captura.png")),
            "a PNG carries no EXIF to remove"
        );
        assert!(!forget.admits(MediaKind::Audio, Path::new("/m/pista.flac")));
    }

    #[test]
    fn a_run_that_skipped_half_the_folder_says_so() {
        let mut progress = BatchProgress::of(4);
        progress.record(ItemOutcome::Done);
        progress.record(ItemOutcome::Skipped);
        progress.record(ItemOutcome::Skipped);
        assert!(!progress.finished());
        assert_eq!(progress.settled(), 3);

        progress.record(ItemOutcome::Failed);
        assert!(progress.finished());
        assert_eq!(progress.done, 1);
        assert_eq!(progress.skipped, 2);
        assert_eq!(progress.failed, 1);
        assert!(progress.changed_anything());
    }

    #[test]
    fn a_run_that_wrote_nothing_is_not_a_success() {
        let mut progress = BatchProgress::of(2);
        progress.record(ItemOutcome::Skipped);
        progress.record(ItemOutcome::Failed);
        assert!(progress.finished());
        assert!(!progress.changed_anything());
    }
}
