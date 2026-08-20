//! The editing session both of Grafita's hosts drive.
//!
//! Siderita's embedded modal and the standalone application need the same
//! thing: ask whether a path is text, open it, apply what a widget typed, undo,
//! redo, save, and refuse to close over unsaved work — all while dropping
//! answers that arrive after the user has moved on. Writing that twice would be
//! writing the staleness rules twice, so it lives here and each host keeps only
//! its Qt marshalling.
//!
//! Nothing here performs IO or owns a thread. A method returns the [`Job`] its
//! host should hand to a [`crate::worker::DocumentWorker`] and the [`Event`] its
//! host should act on; state is mirrored from [`SessionState`]. That is what
//! makes the whole state machine testable without a worker, a toolkit or a
//! filesystem.
//!
//! User-facing wording deliberately stays out. The session reports *typed*
//! outcomes, because the same refusal is worded differently by a modal inside a
//! file manager and by an editor that owns its window.

use std::path::{Path, PathBuf};

use celestina_core::{Generation, GenerationClock};

use crate::document::{Conflict, Document, SaveApplication, SaveIntent};
use crate::encoding::Encoding;
use crate::history::Revision;
use crate::open::{Limits, OpenRefusal, OpenedFile, ProbeOutcome};
use crate::position::PositionError;
use crate::probe::Classification;
use crate::recent::Recent;
use crate::save::{Durability, SaveRefusal, SaveReport};
use crate::search::LiveSearch;
use crate::worker::{Completion, Job};

/// Finding and replacing reads and writes this module's own private state, so
/// it lives beside it rather than as a type of its own.
mod find;

/// Why the last action did not do what was asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    /// A file could not be read, or is not editable text.
    Open(OpenRefusal),
    /// A write was refused. The original file is untouched.
    Save(SaveRefusal),
    /// A widget offered a position this document does not have.
    Edit(PositionError),
}

/// Everything a host mirrors into its interface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionState {
    /// A document is open.
    pub active: bool,
    /// The resolved file behind it.
    pub path: PathBuf,
    /// Its display name.
    pub name: String,
    /// The encoding it will be written back in.
    pub encoding: Option<Encoding>,
    /// The container it came out of, when it is an imported document. Its
    /// presence is what a host shows instead of an encoding: the encoding of a
    /// projection is not something anybody chooses.
    pub container: Option<crate::import::Format>,
    pub dirty: bool,
    /// A read or write is in flight.
    pub busy: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// How durable the last completed save was, cleared by the next action.
    pub saved: Option<Durability>,
    /// Why the last action was refused, cleared by the next action.
    pub failure: Option<Failure>,
    /// How the file on disk disagrees with this document.
    pub conflict: Option<Conflict>,
    /// The guarded-close question is being asked.
    pub close_prompt: bool,
    /// How many times the current search pattern occurs.
    pub search_matches: usize,
    /// Which occurrence is selected, counted from zero.
    pub search_index: Option<usize>,
}

/// Something a host must do beyond mirroring [`SessionState`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// Put this text in the widget and place its caret at `caret`, counted in
    /// the UTF-16 code units Qt uses.
    PushText { text: String, caret: usize },
    /// This path is not editable text. A file manager previews it instead; an
    /// editor says so and stays empty.
    Declined {
        path: PathBuf,
        reason: DeclineReason,
    },
    /// The document closed and whatever had the keyboard before should get it
    /// back.
    Closed,
    /// Select this range in the widget and scroll it into view. Both offsets
    /// count the UTF-16 code units Qt uses.
    Select { start: usize, end: usize },
    /// The answer to [`DocumentSession::classify`]: whether this path holds
    /// editable text. Nothing was opened — the caller asked a question.
    Classified { path: PathBuf, editable: bool },
    /// A save was asked for on a document that has no file yet. The host asks
    /// the user where it goes and comes back through
    /// [`DocumentSession::save_as`].
    DestinationNeeded,
}

/// Why a path was not opened for editing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclineReason {
    /// The content is not text.
    NotText,
    /// It is text in an encoding that cannot be mapped back safely.
    UnsupportedEncoding,
    /// It could not be read at all.
    Unreadable,
}

/// What a session method asks of its host.
#[derive(Clone, Debug, Default)]
#[must_use = "a session outcome carries the job to submit and the event to act on"]
pub struct Outcome {
    /// Blocking work for the host's worker.
    pub job: Option<Job>,
    /// A one-off action beyond mirroring state.
    pub event: Option<Event>,
}

impl Outcome {
    fn nothing() -> Self {
        Self::default()
    }

    fn job(job: Job) -> Self {
        Self {
            job: Some(job),
            event: None,
        }
    }

    fn event(event: Event) -> Self {
        Self {
            job: None,
            event: Some(event),
        }
    }
}

/// One open document and the rules around it.
#[derive(Debug)]
pub struct DocumentSession {
    document: Option<Document>,
    state: SessionState,
    limits: Limits,
    clock: GenerationClock,
    /// The newest question asked. An answer stamped below it belongs to a file
    /// the user has already moved past.
    latest: Generation,
    /// The user answered the guarded-close question with "save", so a completed
    /// save must close rather than merely clean the document.
    close_after_save: bool,
    /// The revision of the write currently with the worker, so pressing save
    /// twice over the same document state queues one write rather than two.
    in_flight: Option<Revision>,
    search: LiveSearch,
    /// The generations of classify-only questions in flight, so each answer is
    /// recognised as one and never mistaken for an open the user asked for.
    /// A list rather than one slot: activating two files in quick succession is
    /// two questions, and answering only the last would leave a file unopened.
    pending_classify: Vec<Generation>,
}

impl DocumentSession {
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self {
            document: None,
            state: SessionState::default(),
            limits,
            clock: GenerationClock::default(),
            latest: Generation::INITIAL,
            close_after_save: false,
            in_flight: None,
            search: LiveSearch::default(),
            pending_classify: Vec::new(),
        }
    }

    /// Starts a document that belongs to no file yet.
    ///
    /// Saving it will ask where it goes; until then it is a perfectly ordinary
    /// document that simply has no name.
    pub fn new_document(&mut self) -> Outcome {
        let generation = self.clock.issue().unwrap_or_default();
        let document = Document::empty(generation);
        let text = document.display_text().to_owned();
        self.latest = generation;
        self.state = SessionState {
            active: true,
            name: String::new(),
            encoding: Some(document.encoding()),
            container: document.container_format(),
            ..SessionState::default()
        };
        self.document = Some(document);
        self.close_after_save = false;
        self.in_flight = None;
        self.search = LiveSearch::default();
        self.refresh();
        Outcome::event(Event::PushText { text, caret: 0 })
    }

    /// The documents opened most recently that still exist, newest first.
    ///
    /// Read on demand rather than held: another Grafita window may have opened
    /// something since, and a stale list is the one thing a history must not be.
    #[must_use]
    pub fn recent_documents() -> Vec<PathBuf> {
        Recent::load().existing()
    }

    /// Whether the open document already knows where it is saved.
    #[must_use]
    pub fn has_destination(&self) -> bool {
        self.document.as_ref().is_some_and(Document::has_target)
    }

    /// Writes the document to `path` and binds it there.
    ///
    /// The write happens on the host's worker like any other, but it is a
    /// different job: there is no prior identity to re-verify, because the
    /// document was never bound to this file.
    pub fn save_as(&mut self, path: &Path) -> Outcome {
        let Some(document) = self.document.as_ref() else {
            return Outcome::nothing();
        };
        if path.as_os_str().is_empty() {
            return Outcome::nothing();
        }
        // The destination is known here, so the only question left is whether
        // the text can become bytes at all. Asking before the job is queued is
        // what keeps a refusal from being reported against a file the write
        // never reached.
        // "Save as" on an imported document writes the whole container, exactly
        // as an ordinary save does; the difference is only where the bytes come
        // from, which the document answers.
        let bytes = match document.save_request() {
            SaveIntent::Ready(request) => request.bytes().to_vec(),
            SaveIntent::Unrepresentable(source) => {
                return self.refuse_save(SaveRefusal::Unrepresentable { source })
            }
            SaveIntent::Unwritable(detail) => {
                return self.refuse_save(SaveRefusal::StructureChanged { detail })
            }
            // A document with no file of its own still has bytes to write here:
            // this path is the one that gives it a file.
            SaveIntent::DestinationNeeded => match document.to_bytes() {
                Ok(bytes) => bytes,
                Err(source) => return self.refuse_save(SaveRefusal::Unrepresentable { source }),
            },
        };
        self.state.busy = true;
        self.state.failure = None;
        self.state.saved = None;
        self.in_flight = Some(document.revision());
        Outcome::job(Job::SaveAs {
            path: path.to_path_buf(),
            bytes,
            generation: document.generation(),
            revision: document.revision(),
        })
    }

    /// Publishes a refusal decided before anything was queued.
    ///
    /// Nothing is in flight and nothing was written, so the document keeps its
    /// dirty state and the session goes back to idle carrying the reason.
    fn refuse_save(&mut self, refusal: SaveRefusal) -> Outcome {
        self.state.busy = false;
        self.state.saved = None;
        self.state.failure = Some(Failure::Save(refusal));
        self.refresh();
        Outcome::nothing()
    }

    /// Moves the caret to the start of a line, counting from 1.
    pub fn go_to_line(&mut self, line_number: usize) -> Outcome {
        let Some(document) = self.document.as_ref() else {
            return Outcome::nothing();
        };
        let position = document.position_at_line(line_number);
        let offset = document.caret_utf16(position);
        Outcome::event(Event::Select {
            start: offset,
            end: offset,
        })
    }

    /// Which language the open document is coloured as.
    #[must_use]
    pub fn language(&self) -> crate::highlight::Language {
        self.document
            .as_ref()
            .map_or(crate::highlight::Language::Plain, Document::language)
    }

    /// Colours one line of the open document.
    #[must_use]
    pub fn highlight_line(
        &self,
        index: usize,
        incoming: crate::highlight::LineState,
    ) -> (Vec<crate::highlight::Span>, crate::highlight::LineState) {
        match self.document.as_ref() {
            Some(document) => document.highlight_line(index, incoming),
            None => (Vec::new(), crate::highlight::LineState::Normal),
        }
    }

    /// What the open document indents with, so a host can show it.
    #[must_use]
    pub fn indentation(&self) -> Option<crate::indent::Indentation> {
        self.document.as_ref().map(Document::indentation)
    }

    /// How many lines the document has.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.document.as_ref().map_or(0, Document::line_count)
    }

    /// Where a widget's caret is, for a status line. With no document open the
    /// answer is the first position, which is where a caret would be if there
    /// were one — a host should hide the readout rather than ask.
    #[must_use]
    pub fn caret_location(&self, utf16_offset: usize) -> crate::position::Location {
        self.document.as_ref().map_or(
            crate::position::Location { line: 1, column: 1 },
            |document| document.caret_location(utf16_offset),
        )
    }

    fn select_current(&self) -> Outcome {
        let (Some(found), Some(document)) = (self.search.current(), self.document.as_ref()) else {
            return Outcome::nothing();
        };
        let span = found.span();
        Outcome::event(Event::Select {
            start: document.caret_utf16(span.start()),
            end: document.caret_utf16(span.end()),
        })
    }

    fn push_projection(&self) -> Outcome {
        match self.document.as_ref() {
            Some(document) => Outcome::event(Event::PushText {
                text: document.display_text().to_owned(),
                caret: 0,
            }),
            None => Outcome::nothing(),
        }
    }

    /// Mirrors the search into the state hosts read.
    fn publish_search(&mut self) {
        self.state.search_matches = self.search.total();
        self.state.search_index = self.search.index();
    }

    #[must_use]
    pub const fn state(&self) -> &SessionState {
        &self.state
    }

    #[must_use]
    pub const fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// The text a widget should currently hold.
    #[must_use]
    pub fn display_text(&self) -> &str {
        self.document.as_ref().map_or("", Document::display_text)
    }

    /// Asks whether `path` is editable text. The answer arrives through
    /// [`DocumentSession::receive`].
    pub fn open(&mut self, path: &Path) -> Outcome {
        if path.as_os_str().is_empty() {
            return Outcome::nothing();
        }
        let Ok(generation) = self.clock.issue() else {
            return Outcome::nothing();
        };
        self.latest = generation;
        self.state.busy = true;
        self.state.failure = None;
        self.state.saved = None;
        Outcome::job(Job::Probe {
            path: path.to_path_buf(),
            generation,
            limits: self.limits,
        })
    }

    /// Opens `path` reading it as `encoding`, whatever its bytes look like.
    ///
    /// This is how a single-byte table, an unmarked UTF-16 file or a UTF-32
    /// file becomes a document: the caller has answered the question the probe
    /// cannot. The read still refuses if that encoding cannot write the file
    /// back byte for byte, so naming one is a choice, not an override.
    pub fn open_with(&mut self, path: &Path, encoding: Encoding) -> Outcome {
        if path.as_os_str().is_empty() {
            return Outcome::nothing();
        }
        let Ok(generation) = self.clock.issue() else {
            return Outcome::nothing();
        };
        self.latest = generation;
        self.state.busy = true;
        self.state.failure = None;
        self.state.saved = None;
        Outcome::job(Job::OpenWith {
            path: path.to_path_buf(),
            encoding,
            generation,
            limits: self.limits,
        })
    }

    /// Reads the open document again as `encoding`.
    ///
    /// Refused while the document is dirty, because re-reading the file is how
    /// this works and there is no way to re-read it and keep edits that were
    /// never written. A host offers this on a saved document, or saves first.
    pub fn reopen_with(&mut self, encoding: Encoding) -> Outcome {
        if self.state.dirty {
            return Outcome::nothing();
        }
        let Some(path) = self
            .document
            .as_ref()
            .and_then(Document::target)
            .map(|target| target.resolved().to_path_buf())
        else {
            return Outcome::nothing();
        };
        self.open_with(&path, encoding)
    }

    /// Asks only whether `path` holds editable text, without opening it.
    ///
    /// Activation needs the question without the answer's cost: deciding which
    /// application should open a file must not read the whole file, and must
    /// not disturb whatever document is already open here.
    pub fn classify(&mut self, path: &Path) -> Outcome {
        if path.as_os_str().is_empty() {
            return Outcome::nothing();
        }
        let Ok(generation) = self.clock.issue() else {
            return Outcome::nothing();
        };
        // Deliberately does not move `latest`: this question is about a file
        // the user is opening elsewhere, and treating it as the newest thing
        // asked would make an open already in flight look stale and be dropped.
        self.pending_classify.push(generation);
        Outcome::job(Job::Classify {
            path: path.to_path_buf(),
            generation,
            limits: self.limits,
        })
    }

    /// Applies the whole text a widget now holds.
    ///
    /// A widget that has drifted from the document — a position the buffer
    /// refuses — is re-seated on the document rather than left to diverge
    /// further.
    pub fn apply_display_text(&mut self, text: &str) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        match document.apply_display_text(text) {
            Ok(None) => Outcome::nothing(),
            Ok(Some(_)) => {
                self.refresh();
                self.refresh_search();
                Outcome::nothing()
            }
            Err(error) => {
                self.state.failure = Some(Failure::Edit(error));
                let push = self.push_text_at_caret();
                self.refresh();
                push
            }
        }
    }

    pub fn undo(&mut self) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        let caret = match document.undo() {
            Ok(Some(outcome)) => outcome.caret,
            Ok(None) => return Outcome::nothing(),
            Err(error) => {
                self.state.failure = Some(Failure::Edit(error));
                return Outcome::nothing();
            }
        };
        let caret = document.caret_utf16(caret);
        let text = document.display_text().to_owned();
        self.refresh();
        self.refresh_search();
        Outcome::event(Event::PushText { text, caret })
    }

    pub fn redo(&mut self) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        let caret = match document.redo() {
            Ok(Some(outcome)) => outcome.caret,
            Ok(None) => return Outcome::nothing(),
            Err(error) => {
                self.state.failure = Some(Failure::Edit(error));
                return Outcome::nothing();
            }
        };
        let caret = document.caret_utf16(caret);
        let text = document.display_text().to_owned();
        self.refresh();
        self.refresh_search();
        Outcome::event(Event::PushText { text, caret })
    }

    /// Queues a write. Nothing about the document changes here: it is clean
    /// only once a report for this very revision comes back.
    ///
    /// A clean document writes nothing, and neither does a second request for a
    /// state already with the worker. Queueing that second write would snapshot
    /// the identity the *first* one is about to replace, so its own predecessor
    /// would come back as "another program changed this file".
    pub fn save(&mut self) -> Outcome {
        let Some(document) = self.document.as_ref() else {
            return Outcome::nothing();
        };
        // No file yet: the host has to ask where this document goes. That is
        // an event, not a refusal — a new document is a perfectly good document
        // that simply has not been given a name.
        //
        // Asked before the clean check, and that order is the point. The clean
        // check exists so an unchanged file is not rewritten; a document with
        // no file has nothing to rewrite, so the question does not apply to it.
        // Behind the other order, a new document nobody had typed into yet
        // could not even be given a name: the shortcut answered nothing at all.
        let request = match document.save_request() {
            SaveIntent::DestinationNeeded => return Outcome::event(Event::DestinationNeeded),
            SaveIntent::Unrepresentable(source) => {
                return self.refuse_save(SaveRefusal::Unrepresentable { source })
            }
            SaveIntent::Unwritable(detail) => {
                return self.refuse_save(SaveRefusal::StructureChanged { detail })
            }
            SaveIntent::Ready(request) => request,
        };
        if !self.state.dirty {
            return Outcome::nothing();
        }
        if self.in_flight == Some(document.revision()) {
            return Outcome::nothing();
        }
        self.in_flight = Some(document.revision());
        let job = Job::Save {
            request: Box::new(request),
            generation: document.generation(),
        };
        self.state.busy = true;
        self.state.failure = None;
        self.state.saved = None;
        Outcome::job(job)
    }

    /// Asks to close. A dirty document raises the guarded question instead.
    pub fn request_close(&mut self) -> Outcome {
        if self.state.dirty {
            self.state.close_prompt = true;
            Outcome::nothing()
        } else {
            self.close()
        }
    }

    /// Answers the guarded question: write, then close.
    pub fn save_and_close(&mut self) -> Outcome {
        self.state.close_prompt = false;
        if !self.state.dirty {
            return self.close();
        }
        self.close_after_save = true;
        self.save()
    }

    /// The host asked where the document goes and the user answered "nowhere".
    ///
    /// Without this a cancelled chooser leaves the pending close armed, and
    /// some ordinary save much later closes the document on its own.
    pub fn cancel_save_as(&mut self) -> Outcome {
        self.close_after_save = false;
        Outcome::nothing()
    }

    /// Answers it by throwing the edit away.
    pub fn discard_and_close(&mut self) -> Outcome {
        self.state.close_prompt = false;
        self.close()
    }

    /// Answers it by staying in the document.
    pub fn cancel_close(&mut self) -> Outcome {
        self.state.close_prompt = false;
        Outcome::nothing()
    }

    /// Takes one answer from the worker.
    pub fn receive(&mut self, completion: Completion) -> Outcome {
        match completion {
            Completion::Probed { generation, result } => {
                // Membership is checked before staleness, and deliberately so:
                // a classify does not move `latest`, so an open asked for after
                // it would otherwise make the classify's own answer look stale
                // and drop it — leaving the question the host is waiting on
                // unanswered for ever.
                if !self.pending_classify.contains(&generation) && generation < self.latest {
                    return Outcome::nothing();
                }
                self.receive_probe(generation, *result)
            }
            Completion::Opened { generation, result } => {
                if generation < self.latest {
                    return Outcome::nothing();
                }
                self.state.busy = false;
                self.receive_open(*result)
            }
            Completion::Created {
                generation,
                revision,
                result,
            } => {
                if self.document.as_ref().map(Document::generation) != Some(generation) {
                    return Outcome::nothing();
                }
                self.state.busy = false;
                self.in_flight = None;
                self.receive_created(revision, *result)
            }
            Completion::Saved {
                generation, result, ..
            } => {
                // A report for a document that is no longer open must not be
                // applied to whatever is open now, nor clear a busy flag that
                // belongs to a read still in flight.
                if self.document.as_ref().map(Document::generation) != Some(generation) {
                    return Outcome::nothing();
                }
                self.state.busy = false;
                self.in_flight = None;
                self.receive_save(*result)
            }
        }
    }

    fn receive_probe(
        &mut self,
        generation: Generation,
        result: Result<ProbeOutcome, OpenRefusal>,
    ) -> Outcome {
        // A classify-only question is answered here and goes no further: it
        // opens nothing, touches no state, and reports only what it was asked.
        let classifying = self.pending_classify.contains(&generation);
        if classifying {
            self.pending_classify
                .retain(|pending| *pending != generation);
            return match result {
                Ok(outcome) => Outcome::event(Event::Classified {
                    editable: outcome.classification.is_editable(),
                    path: outcome.path,
                }),
                Err(refusal) => Outcome::event(Event::Classified {
                    path: refusal_path(&refusal),
                    editable: false,
                }),
            };
        }

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(refusal) => {
                self.state.busy = false;
                let path = refusal_path(&refusal);
                self.state.failure = Some(Failure::Open(refusal));
                return Outcome::event(Event::Declined {
                    path,
                    reason: DeclineReason::Unreadable,
                });
            }
        };
        match outcome.classification {
            // Both kinds of document open the same way. Which contract the
            // document turns out to be under is decided by the read, not here.
            Classification::EditableText { .. } | Classification::ImportedDocument => {
                Outcome::job(Job::Open {
                    path: outcome.path,
                    generation,
                    limits: self.limits,
                })
            }
            Classification::Binary { reason } => {
                self.state.busy = false;
                self.state.failure = Some(Failure::Open(OpenRefusal::NotText { reason }));
                Outcome::event(Event::Declined {
                    path: outcome.path,
                    reason: DeclineReason::NotText,
                })
            }
            Classification::UnsupportedEncoding { reason } => {
                self.state.busy = false;
                self.state.failure = Some(Failure::Open(OpenRefusal::UnsupportedEncoding {
                    detail: reason.to_string(),
                }));
                Outcome::event(Event::Declined {
                    path: outcome.path,
                    reason: DeclineReason::UnsupportedEncoding,
                })
            }
        }
    }

    fn receive_open(&mut self, result: Result<OpenedFile, OpenRefusal>) -> Outcome {
        match result {
            Ok(opened) => {
                let path = opened.target.resolved().to_path_buf();
                let document = Document::from_opened(opened);
                let text = document.display_text().to_owned();
                self.state = SessionState {
                    active: true,
                    name: file_name(&path),
                    path,
                    encoding: Some(document.encoding()),
                    container: document.container_format(),
                    ..SessionState::default()
                };
                self.document = Some(document);
                self.close_after_save = false;
                self.in_flight = None;
                // A pattern's matches point into the buffer they were found in.
                // Carrying them into a different document would let a replace
                // splice at offsets that never held a match.
                self.search = LiveSearch::default();
                self.refresh();
                // Remembered only once it actually opened: a file that refused
                // has no business in a list of things you can reopen.
                let mut recent = Recent::load();
                recent.record(&self.state.path);
                recent.store();
                Outcome::event(Event::PushText { text, caret: 0 })
            }
            Err(refusal) => {
                let path = refusal_path(&refusal);
                let reason = decline_reason(&refusal);
                // A remembered document that no longer opens stops being
                // offered: a recent list that leads nowhere is worse than a
                // short one.
                if !path.as_os_str().is_empty() {
                    let mut recent = Recent::load();
                    recent.forget(&path);
                    recent.store();
                }
                self.state.failure = Some(Failure::Open(refusal));
                Outcome::event(Event::Declined { path, reason })
            }
        }
    }

    fn receive_save(&mut self, result: Result<SaveReport, SaveRefusal>) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        let applied = match result {
            Ok(report) => {
                let applied = document.apply_save(&report);
                self.state.saved = Some(report.durability);
                self.state.failure = None;
                applied
            }
            Err(refusal) => {
                document.apply_save_refusal(&refusal);
                self.state.failure = Some(Failure::Save(refusal));
                SaveApplication::StillDirty
            }
        };
        self.refresh();

        let closing = self.close_after_save;
        self.close_after_save = false;
        if closing && matches!(applied, SaveApplication::Clean) && self.state.failure.is_none() {
            return self.close();
        }
        Outcome::nothing()
    }

    /// Takes the answer to a "save as": the document adopts the file that was
    /// just written and becomes an ordinary saved document.
    ///
    /// `revision` is the state the bytes left with. Anything typed while the
    /// worker was writing and syncing is in the document and not in the file,
    /// so it keeps the document dirty exactly as it does for an ordinary save.
    /// A close waiting on this write is then abandoned rather than honoured:
    /// closing would take those keystrokes with it, so the document stays open
    /// and dirty and the user asks again.
    fn receive_created(
        &mut self,
        revision: Revision,
        result: Result<crate::save::CreatedFile, SaveRefusal>,
    ) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        match result {
            Ok(created) => {
                let name = file_name(created.target.resolved());
                let path = created.target.resolved().to_path_buf();
                document.adopt_target(created.target);
                let applied = document.mark_saved_at(revision);
                self.state.name = name;
                self.state.path = path;
                self.state.saved = Some(created.durability);
                self.state.failure = None;
                self.refresh();
                let closing = self.close_after_save;
                self.close_after_save = false;
                if closing && matches!(applied, SaveApplication::Clean) {
                    return self.close();
                }
                Outcome::nothing()
            }
            Err(refusal) => {
                self.state.failure = Some(Failure::Save(refusal));
                self.close_after_save = false;
                Outcome::nothing()
            }
        }
    }

    fn close(&mut self) -> Outcome {
        self.document = None;
        self.close_after_save = false;
        self.in_flight = None;
        self.search = LiveSearch::default();
        self.state = SessionState::default();
        Outcome::event(Event::Closed)
    }

    /// Pushes the current projection with the caret at its start, used when a
    /// widget has drifted out of step with the document.
    fn push_text_at_caret(&self) -> Outcome {
        match self.document.as_ref() {
            Some(document) => Outcome::event(Event::PushText {
                text: document.display_text().to_owned(),
                caret: 0,
            }),
            None => Outcome::nothing(),
        }
    }

    /// Copies the document's derived state into the mirror hosts read.
    fn refresh(&mut self) {
        let Some(document) = self.document.as_ref() else {
            return;
        };
        self.state.dirty = document.is_dirty();
        self.state.can_undo = document.can_undo();
        self.state.can_redo = document.can_redo();
        self.state.conflict = document.conflict().cloned();
    }

    /// Keeps the match list in step with the document after an ordinary edit.
    fn refresh_search(&mut self) {
        if self.search.is_idle() {
            return;
        }
        self.search
            .rescan(self.document.as_ref().map(Document::buffer));
        self.publish_search();
    }

    /// The revision a host can compare a pending save against.
    #[must_use]
    pub fn revision(&self) -> Option<Revision> {
        self.document.as_ref().map(Document::revision)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn refusal_path(refusal: &OpenRefusal) -> PathBuf {
    match refusal {
        OpenRefusal::ChangedWhileReading { path } | OpenRefusal::Io { path, .. } => path.clone(),
        _ => PathBuf::new(),
    }
}

const fn decline_reason(refusal: &OpenRefusal) -> DeclineReason {
    match refusal {
        OpenRefusal::NotText { .. } => DeclineReason::NotText,
        OpenRefusal::UnsupportedEncoding { .. } => DeclineReason::UnsupportedEncoding,
        _ => DeclineReason::Unreadable,
    }
}
