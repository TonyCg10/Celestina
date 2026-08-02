//! The one type both hosts drive: buffer, history, dirty and conflict state,
//! and the save request that leaves for a worker.
//!
//! Nothing here blocks or performs IO. The document produces a [`SaveRequest`]
//! and consumes a [`SaveReport`]; the worker in between belongs to the host.
//! Both directions are stamped — opens with a generation, saves with a
//! revision — so a reply that arrives after newer work is recognised as stale
//! instead of overwriting it.

use std::path::PathBuf;

use celestina_core::Generation;

use crate::buffer::{Fragment, TextBuffer};
use crate::display;
use crate::encoding::Encoding;
use crate::highlight::{self, Language, LineState, Span as HighlightSpan};
use crate::history::{History, Revision};
use crate::indent::{self, Indentation};
use crate::open::OpenedFile;
use crate::position::{Position, PositionError, Span};
use crate::save::{SaveRefusal, SaveReport, SaveRequest};
use crate::search;
use crate::target::Target;

/// A disagreement between the document and the file it came from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Conflict {
    /// The file changed on disk after this document was read.
    ChangedUnderneath,
    /// The path now resolves somewhere else.
    Retargeted { found: PathBuf },
    /// The file is gone.
    Missing,
}

/// What an applied edit left behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditOutcome {
    /// Where the caret belongs now.
    pub caret: Position,
    /// The document revision this edit produced.
    pub revision: Revision,
}

/// What a save report did to the document's dirty state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveApplication {
    /// The saved bytes are the document's current bytes: it is clean.
    Clean,
    /// The document was edited after the save started, so it stays dirty. The
    /// write still happened and its identity was adopted.
    StillDirty,
}

/// Whether a stamped reply was fresh enough to apply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Freshness {
    Applied,
    Stale,
}

/// An open document.
#[derive(Clone, Debug)]
pub struct Document {
    buffer: TextBuffer,
    history: History,
    /// The file this document is bound to. `None` until a new document has
    /// been saved somewhere: an unsaved scratch buffer has no identity to
    /// re-verify and nothing on disk to protect.
    target: Option<Target>,
    encoding: Encoding,
    generation: Generation,
    revision: Revision,
    conflict: Option<Conflict>,
    /// The group of the change most recently undone or redone, so the rest of
    /// its action can follow it.
    undone_group: Option<u64>,
    redone_group: Option<u64>,
    /// The line-feed-only text a widget edits, kept in step with the buffer.
    /// Holding it rather than rebuilding it per keystroke is what lets
    /// [`Document::apply_display_text`] recognise the document's own projection
    /// coming back and treat it as no edit at all.
    projection: String,
}

impl Document {
    /// Builds a document from a completed read.
    #[must_use]
    pub fn from_opened(opened: OpenedFile) -> Self {
        let buffer = TextBuffer::from_text(&opened.text);
        Self {
            undone_group: None,
            redone_group: None,
            projection: display::project(&buffer),
            buffer,
            history: History::default(),
            target: Some(opened.target),
            encoding: opened.encoding,
            generation: opened.generation,
            revision: Revision::INITIAL,
            conflict: None,
        }
    }

    #[must_use]
    pub const fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// A brand-new document that belongs to no file yet.
    ///
    /// UTF-8 with line feeds, because that is what a new file on this desktop
    /// should be; an *opened* document still keeps whatever it came with.
    #[must_use]
    pub fn empty(generation: Generation) -> Self {
        let buffer = TextBuffer::from_text("");
        Self {
            projection: display::project(&buffer),
            buffer,
            history: History::default(),
            target: None,
            encoding: Encoding::Utf8,
            generation,
            revision: Revision::INITIAL,
            conflict: None,
            undone_group: None,
            redone_group: None,
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }

    /// Whether this document has somewhere to save to without being asked.
    #[must_use]
    pub const fn has_target(&self) -> bool {
        self.target.is_some()
    }

    /// Adopts the file a "save as" just wrote, after which every ordinary save
    /// rule applies to it.
    pub fn adopt_target(&mut self, target: Target) {
        self.target = Some(target);
        self.conflict = None;
        self.history.mark_saved();
    }

    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    #[must_use]
    pub const fn conflict(&self) -> Option<&Conflict> {
        self.conflict.as_ref()
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.history.is_clean()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The document's text, terminators as they stand.
    #[must_use]
    pub fn text(&self) -> String {
        self.buffer.to_text()
    }

    /// The bytes this document would write.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.encoding.encode(&self.buffer.to_text())
    }

    /// Replaces `span` with `text`, recording an undoable change.
    ///
    /// The buffer is validated before anything moves, so an invalid span leaves
    /// the document, its history and its revision untouched.
    pub fn replace(
        &mut self,
        span: Span,
        text: &str,
        caret_before: Position,
    ) -> Result<EditOutcome, PositionError> {
        let inserted = Fragment::inserted(text, self.buffer.dominant_newline());
        let replacement = self.buffer.replace(span, &inserted)?;
        self.history.record(
            span,
            replacement.removed,
            inserted,
            replacement.inserted_end,
            caret_before,
        );
        self.revision = self.revision.next();
        self.projection = display::project(&self.buffer);
        Ok(EditOutcome {
            caret: replacement.inserted_end,
            revision: self.revision,
        })
    }

    /// Replaces one found occurrence.
    ///
    /// An ordinary splice, so undo, the savepoint and the dirty flag treat it
    /// exactly as they treat typing.
    pub fn replace_match(
        &mut self,
        found: search::Match,
        replacement: &str,
    ) -> Result<EditOutcome, PositionError> {
        let span = found.span();
        self.replace(span, replacement, span.start())
    }

    /// Replaces every occurrence of `pattern`, as a single undoable action.
    ///
    /// The splices run from the end of the document backwards, so each one is
    /// applied at a position the earlier ones have not moved. They share an
    /// undo group, which is what makes one keystroke's worth of intent one
    /// keystroke's worth of undo.
    ///
    /// Nothing is replaced line by line as whole lines, so terminators are
    /// never rewritten — a mixed-newline file stays mixed.
    pub fn replace_all(
        &mut self,
        pattern: &str,
        replacement: &str,
        query: search::Query,
    ) -> Result<usize, PositionError> {
        let matches = search::find_all(&self.buffer, pattern, query);
        if matches.is_empty() {
            return Ok(0);
        }

        self.history.open_group();
        let result = (|| {
            for found in matches.iter().rev() {
                self.replace_match(*found, replacement)?;
            }
            Ok(matches.len())
        })();
        self.history.close_group();
        result
    }

    /// Which language this document is coloured as.
    ///
    /// Chosen from the resolved file's name — the one place a name decides
    /// anything, and it decides only colour. Whether the file could be opened
    /// at all was settled by its bytes.
    #[must_use]
    pub fn language(&self) -> Language {
        self.target.as_ref().map_or(Language::Plain, |target| {
            Language::for_path(target.resolved())
        })
    }

    /// Colours one line of the projection, given what the previous line left.
    ///
    /// Line indices match the projection a widget holds, so a host can
    /// re-colour just the lines that changed.
    #[must_use]
    pub fn highlight_line(
        &self,
        index: usize,
        incoming: LineState,
    ) -> (Vec<HighlightSpan>, LineState) {
        match self.buffer.line(index) {
            Some(line) => highlight::line(line.text(), self.language(), incoming),
            None => (Vec::new(), LineState::Normal),
        }
    }

    /// What this document indents with.
    ///
    /// Measured from the document itself, so inserting a level never imposes a
    /// style the file does not use.
    #[must_use]
    pub fn indentation(&self) -> Indentation {
        indent::detect(&self.buffer)
    }

    /// The start of a line, counting from 1 as every editor and compiler does.
    ///
    /// Out-of-range numbers clamp to the nearest real line: "go to line 900" in
    /// a 40-line file means the end, and refusing to move would be less useful
    /// than going as far as the document allows.
    #[must_use]
    pub fn position_at_line(&self, line_number: usize) -> Position {
        let index = line_number.saturating_sub(1);
        let last = self.buffer.line_count().saturating_sub(1);
        Position::new(index.min(last), 0)
    }

    /// How many lines the document has, for a host to bound its input.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// Every occurrence of `pattern` in the document, in order.
    #[must_use]
    pub fn find_all(&self, pattern: &str, query: search::Query) -> Vec<search::Match> {
        search::find_all(&self.buffer, pattern, query)
    }

    /// The next occurrence at or after `from`, wrapping at the end.
    #[must_use]
    pub fn find_next(
        &self,
        pattern: &str,
        query: search::Query,
        from: Position,
    ) -> Option<search::Match> {
        search::next(&self.buffer, pattern, query, from)
    }

    /// The previous occurrence before `from`, wrapping at the start.
    #[must_use]
    pub fn find_previous(
        &self,
        pattern: &str,
        query: search::Query,
        from: Position,
    ) -> Option<search::Match> {
        search::previous(&self.buffer, pattern, query, from)
    }

    /// The line-feed-only text a toolkit's text widget shows and edits.
    ///
    /// Terminators are projected, never rewritten: the document keeps whatever
    /// each line ended with, and only this view flattens them.
    #[must_use]
    pub fn display_text(&self) -> &str {
        &self.projection
    }

    /// Where `position` sits in the projection, counted in the UTF-16 code
    /// units Qt's text widgets use for a cursor.
    #[must_use]
    pub fn caret_utf16(&self, position: Position) -> usize {
        display::utf16_offset_at(&self.buffer, position)
    }

    /// Takes the whole text a widget now holds and applies the one difference
    /// that explains it.
    ///
    /// Text identical to the current projection is not an edit and returns
    /// `None`, so a host may push the document's own projection back into its
    /// widget — after an undo, say — without that echo being recorded.
    pub fn apply_display_text(
        &mut self,
        proposed: &str,
    ) -> Result<Option<EditOutcome>, PositionError> {
        let Some(edit) = display::reconcile(&self.buffer, &self.projection, proposed) else {
            return Ok(None);
        };
        let caret_before = edit.span.start();
        self.replace(edit.span, &edit.text, caret_before).map(Some)
    }

    /// Inserts `text` at `at`.
    pub fn insert(&mut self, at: Position, text: &str) -> Result<EditOutcome, PositionError> {
        self.replace(Span::empty(at), text, at)
    }

    /// Deletes `span`, leaving the caret where it started.
    pub fn delete(&mut self, span: Span) -> Result<EditOutcome, PositionError> {
        self.replace(span, "", span.end())
    }

    /// Reverts the last change. Returns `None` when there is nothing to revert.
    /// Reverses the last action.
    ///
    /// An action, not a splice: changes recorded inside a group — a
    /// replace-all — are reversed together, because that is what the user did
    /// once and expects to undo once.
    pub fn undo(&mut self) -> Result<Option<EditOutcome>, PositionError> {
        let mut outcome = self.undo_one()?;
        if let Some(group) = self.undone_group {
            while self.history.peek_undo_group() == Some(group) {
                match self.undo_one()? {
                    Some(step) => outcome = Some(step),
                    None => break,
                }
            }
        }
        Ok(outcome)
    }

    fn undo_one(&mut self) -> Result<Option<EditOutcome>, PositionError> {
        let Some(change) = self.history.take_undo() else {
            self.undone_group = None;
            return Ok(None);
        };
        self.undone_group = change.group();
        match self.buffer.replace(change.span_after(), change.removed()) {
            Ok(_) => {
                let caret = change.caret_before();
                self.history.finish_undo(change);
                self.revision = self.revision.next();
                self.projection = display::project(&self.buffer);
                Ok(Some(EditOutcome {
                    caret,
                    revision: self.revision,
                }))
            }
            Err(error) => {
                self.history.restore_undo(change);
                Err(error)
            }
        }
    }

    /// Reapplies the last reverted change. Returns `None` when there is none.
    /// Replays the last undone action, group and all.
    pub fn redo(&mut self) -> Result<Option<EditOutcome>, PositionError> {
        let mut outcome = self.redo_one()?;
        if let Some(group) = self.redone_group {
            while self.history.peek_redo_group() == Some(group) {
                match self.redo_one()? {
                    Some(step) => outcome = Some(step),
                    None => break,
                }
            }
        }
        Ok(outcome)
    }

    fn redo_one(&mut self) -> Result<Option<EditOutcome>, PositionError> {
        let Some(change) = self.history.take_redo() else {
            self.redone_group = None;
            return Ok(None);
        };
        self.redone_group = change.group();
        match self.buffer.replace(change.span_before(), change.inserted()) {
            Ok(replacement) => {
                self.history.finish_redo(change);
                self.revision = self.revision.next();
                self.projection = display::project(&self.buffer);
                Ok(Some(EditOutcome {
                    caret: replacement.inserted_end,
                    revision: self.revision,
                }))
            }
            Err(error) => {
                self.history.restore_redo(change);
                Err(error)
            }
        }
    }

    /// The write this document would perform, or `None` when it has no file yet
    /// and the host must ask where to put it.
    ///
    /// The bytes are snapshotted here, so later keystrokes cannot change what
    /// the worker writes.
    #[must_use]
    pub fn save_request(&self) -> Option<SaveRequest> {
        self.target
            .as_ref()
            .map(|target| SaveRequest::new(target.clone(), self.to_bytes(), self.revision))
    }

    /// Applies a completed save.
    ///
    /// The written file's identity is adopted either way — the document caused
    /// that write, and treating it as an external change would make every save
    /// after the first report a false conflict. Only the dirty state depends on
    /// whether the document moved on in the meantime.
    pub fn apply_save(&mut self, report: &SaveReport) -> SaveApplication {
        if let Some(target) = self.target.as_mut() {
            target.adopt(report.identity);
        }
        self.conflict = None;
        if report.revision == self.revision {
            self.history.mark_saved();
            SaveApplication::Clean
        } else {
            SaveApplication::StillDirty
        }
    }

    /// Records what a refused save says about the file on disk.
    ///
    /// Only refusals that describe the target become conflicts; a transient IO
    /// failure or a cancellation says nothing about the file's state and must
    /// not raise a conflict banner the user cannot act on.
    pub fn apply_save_refusal(&mut self, refusal: &SaveRefusal) -> Option<&Conflict> {
        self.conflict = match refusal {
            SaveRefusal::ChangedUnderneath { .. } => Some(Conflict::ChangedUnderneath),
            SaveRefusal::Retargeted { found, .. } => Some(Conflict::Retargeted {
                found: found.clone(),
            }),
            SaveRefusal::TargetMissing { .. } => Some(Conflict::Missing),
            SaveRefusal::MetadataNotReproducible { .. }
            | SaveRefusal::Cancelled
            | SaveRefusal::Io { .. } => None,
        };
        self.conflict.as_ref()
    }

    /// Replaces the document with a newer read of the same file, discarding
    /// history and any conflict.
    ///
    /// A read stamped at or below the current generation is stale — the host
    /// has since asked for a newer one — and is refused rather than applied.
    pub fn adopt_reload(&mut self, opened: OpenedFile) -> Freshness {
        if opened.generation <= self.generation {
            return Freshness::Stale;
        }
        self.buffer = TextBuffer::from_text(&opened.text);
        self.projection = display::project(&self.buffer);
        self.history = History::default();
        self.target = Some(opened.target);
        self.encoding = opened.encoding;
        self.generation = opened.generation;
        self.revision = self.revision.next();
        self.conflict = None;
        Freshness::Applied
    }
}
