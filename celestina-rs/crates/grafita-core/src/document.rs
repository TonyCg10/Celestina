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

use crate::buffer::{Fragment, Replacement, TextBuffer};
use crate::display::{self, LineMap};
use crate::encoding::{EncodeError, Encoding};
use crate::highlight::{self, Language, LineState, Span as HighlightSpan};
use crate::history::{History, Revision};
use crate::import::Imported;
use crate::indent::{self, Indentation};
use crate::open::OpenedFile;
use crate::position::{Location, Position, PositionError, Span};
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

/// What saving a document would do, answered before a byte is written.
///
/// The three answers are separate because a host reacts differently to each:
/// one asks a question, one reports a refusal, and one is the write itself.
#[derive(Clone, Debug)]
pub enum SaveIntent {
    /// The document has no file yet, so the host has to ask where it goes.
    /// This is a question, not a refusal.
    DestinationNeeded,
    /// The text holds a character this document's encoding has no byte for.
    /// Nothing is written and the file on disk is untouched.
    Unrepresentable(EncodeError),
    /// An imported document's text no longer fits the structure it came from.
    /// Nothing is written and the container on disk is untouched.
    Unwritable(String),
    /// The write, with its bytes already snapshotted.
    Ready(SaveRequest),
}

impl SaveIntent {
    /// The write, when there is one. A host reacts to each answer separately;
    /// this is for a caller that only cares whether a write was produced.
    #[must_use]
    pub fn ready(self) -> Option<SaveRequest> {
        match self {
            Self::Ready(request) => Some(request),
            Self::DestinationNeeded | Self::Unrepresentable(_) | Self::Unwritable(_) => None,
        }
    }
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
    /// The container this document came out of, when it came out of one. Its
    /// presence makes this an imported document: the text is a projection of a
    /// structure, and saving writes that structure back rather than these
    /// bytes.
    imported: Option<Imported>,
    /// The line-feed-only text a widget edits, kept in step with the buffer.
    /// Holding it rather than rebuilding it per keystroke is what lets
    /// [`Document::apply_display_text`] recognise the document's own projection
    /// coming back and treat it as no edit at all.
    projection: String,
    /// Where each line starts in that projection, kept in step by the same
    /// splice that keeps the projection. It is what turns every caret and
    /// offset question from a walk of the document into a walk of one line
    /// (GRA-P1, docs/evidence/2026-09-02-apps-performance-audit.md).
    map: LineMap,
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
            map: LineMap::of(&buffer),
            buffer,
            history: History::default(),
            target: Some(opened.target),
            encoding: opened.encoding,
            generation: opened.generation,
            revision: Revision::INITIAL,
            conflict: None,
            imported: opened.imported,
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
            map: LineMap::of(&buffer),
            buffer,
            history: History::default(),
            target: None,
            encoding: Encoding::Utf8,
            generation,
            revision: Revision::INITIAL,
            conflict: None,
            undone_group: None,
            redone_group: None,
            imported: None,
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
    ///
    /// Adopting the file is all this does. Whether the document is *clean* is a
    /// separate question with a separate answer — [`Document::mark_saved_at`] —
    /// because the write took time and the keys pressed during it are in the
    /// document but not in the file.
    pub fn adopt_target(&mut self, target: Target) {
        self.target = Some(target);
        self.conflict = None;
    }

    /// Pins the savepoint when `revision` is still the document's own.
    ///
    /// A write that left with an older revision does not describe the document
    /// as it stands, so clearing dirty state on it would mark keystrokes saved
    /// that were never written.
    pub fn mark_saved_at(&mut self, revision: Revision) -> SaveApplication {
        if revision == self.revision {
            self.history.mark_saved();
            SaveApplication::Clean
        } else {
            SaveApplication::StillDirty
        }
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

    /// Whether this document is the text inside a container somebody else
    /// wrote, rather than the bytes of a file.
    #[must_use]
    pub const fn is_imported(&self) -> bool {
        self.imported.is_some()
    }

    /// What kind of container this document came out of, if any.
    #[must_use]
    pub fn container_format(&self) -> Option<crate::import::Format> {
        self.imported.as_ref().map(Imported::format)
    }

    /// The bytes this document would write, or the character that stops it.
    ///
    /// An imported document writes its whole container: the text goes back into
    /// the part it came from and every other part is copied as the bytes it
    /// already was.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        self.encoding.encode(&self.buffer.to_text())
    }

    /// Applies one replacement to the buffer and keeps the projection and the
    /// line map in step by splicing exactly the lines the edit touched.
    ///
    /// This is the only way any text mutation reaches the buffer — the edits,
    /// the undo and the redo all pass through here — because a projection or a
    /// map updated on most paths is a projection that lies on the others.
    /// Rebuilding both per keystroke instead was the O(document) cost recorded
    /// as GRA-P1 in `docs/evidence/2026-09-02-apps-performance-audit.md`.
    fn apply_to_buffer(
        &mut self,
        span: Span,
        fragment: &Fragment,
    ) -> Result<Replacement, PositionError> {
        let start = span.start();
        let end = span.end();
        // The splice bounds must be read off the map *before* the buffer
        // moves. A span the buffer will refuse never mutates anything, so the
        // guard only has to keep the map lookups in bounds and can leave the
        // refusal itself to the buffer's own validation.
        let lines = self.buffer.line_count();
        let bounds = if start.line < lines && end.line < lines {
            Some((self.map.byte_start(start.line), self.map.byte_end(end.line)))
        } else {
            None
        };
        let replacement = self.buffer.replace(span, fragment)?;
        let (byte_from, byte_to) =
            bounds.expect("the buffer accepted a span past its own last line");
        self.map.splice(
            &self.buffer,
            start.line,
            end.line - start.line + 1,
            replacement.inserted_end.line - start.line + 1,
        );
        let patch: Vec<&str> = self.buffer.lines()[start.line..=replacement.inserted_end.line]
            .iter()
            .map(crate::buffer::Line::text)
            .collect();
        self.projection
            .replace_range(byte_from..byte_to, &patch.join("\n"));
        Ok(replacement)
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
        let replacement = self.apply_to_buffer(span, &inserted)?;
        self.history.record(
            span,
            replacement.removed,
            inserted,
            replacement.inserted_end,
            caret_before,
        );
        self.revision = self.revision.next();
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
        self.map.utf16_offset_at(&self.buffer, position)
    }

    /// Where a widget's caret is, worded for a status line: line and character
    /// column, both counted from one.
    #[must_use]
    pub fn caret_location(&self, utf16_offset: usize) -> Location {
        self.map.location_at_utf16(&self.buffer, utf16_offset)
    }

    /// The UTF-16 offset of `line`'s first character in the projection — the
    /// offset a widget's `positionToRectangle` accepts. A line past the end
    /// answers just past the last line, so a stale ask cannot panic.
    #[must_use]
    pub fn line_start_utf16(&self, line: usize) -> usize {
        self.map.utf16_start(line)
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
        match self.apply_to_buffer(change.span_after(), change.removed()) {
            Ok(_) => {
                let caret = change.caret_before();
                self.history.finish_undo(change);
                self.revision = self.revision.next();
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
        match self.apply_to_buffer(change.span_before(), change.inserted()) {
            Ok(replacement) => {
                self.history.finish_redo(change);
                self.revision = self.revision.next();
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

    /// What saving this document would do, decided before anything is written.
    ///
    /// The bytes are snapshotted here, so later keystrokes cannot change what
    /// the worker writes. The three answers are separate because a host reacts
    /// differently to each: one asks a question, one reports a refusal, and one
    /// is the write.
    #[must_use]
    pub fn save_request(&self) -> SaveIntent {
        let Some(target) = self.target.as_ref() else {
            return SaveIntent::DestinationNeeded;
        };
        let bytes = match self.imported.as_ref() {
            Some(imported) => match imported.to_bytes(&self.buffer.to_text()) {
                Ok(bytes) => bytes,
                Err(source) => return SaveIntent::Unwritable(source.to_string()),
            },
            None => match self.to_bytes() {
                Ok(bytes) => bytes,
                Err(source) => return SaveIntent::Unrepresentable(source),
            },
        };
        SaveIntent::Ready(SaveRequest::new(target.clone(), bytes, self.revision))
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
        self.mark_saved_at(report.revision)
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
            // Unrepresentable is about the text in this window, not about the
            // file, so it raises no conflict banner either.
            SaveRefusal::MetadataNotReproducible { .. }
            | SaveRefusal::Unrepresentable { .. }
            | SaveRefusal::StructureChanged { .. }
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
        self.map = LineMap::of(&self.buffer);
        self.history = History::default();
        self.target = Some(opened.target);
        self.encoding = opened.encoding;
        self.generation = opened.generation;
        self.revision = self.revision.next();
        self.conflict = None;
        Freshness::Applied
    }
}
