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

use crate::document::{Conflict, Document, SaveApplication};
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
            ..SessionState::default()
        };
        self.document = Some(document);
        self.close_after_save = false;
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
        self.state.busy = true;
        self.state.failure = None;
        self.state.saved = None;
        Outcome::job(Job::SaveAs {
            path: path.to_path_buf(),
            bytes: document.to_bytes(),
            generation: document.generation(),
            revision: document.revision(),
        })
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
    pub fn save(&mut self) -> Outcome {
        let Some(document) = self.document.as_ref() else {
            return Outcome::nothing();
        };
        // No file yet: the host has to ask where this document goes. That is
        // an event, not a refusal — a new document is a perfectly good document
        // that simply has not been given a name.
        let Some(request) = document.save_request() else {
            return Outcome::event(Event::DestinationNeeded);
        };
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
        self.close_after_save = true;
        self.save()
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
                if generation < self.latest {
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
                generation, result, ..
            } => {
                if self.document.as_ref().map(Document::generation) != Some(generation) {
                    return Outcome::nothing();
                }
                self.state.busy = false;
                self.receive_created(*result)
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
            Classification::EditableText { .. } => Outcome::job(Job::Open {
                path: outcome.path,
                generation,
                limits: self.limits,
            }),
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
                    ..SessionState::default()
                };
                self.document = Some(document);
                self.close_after_save = false;
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
    fn receive_created(&mut self, result: Result<crate::target::Target, SaveRefusal>) -> Outcome {
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        match result {
            Ok(target) => {
                let name = file_name(target.resolved());
                let path = target.resolved().to_path_buf();
                document.adopt_target(target);
                self.state.name = name;
                self.state.path = path;
                self.state.saved = Some(Durability::Durable);
                self.state.failure = None;
                self.refresh();
                let closing = self.close_after_save;
                self.close_after_save = false;
                if closing {
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
