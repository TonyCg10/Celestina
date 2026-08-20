//! The standalone application's adapter over `grafita-core`.
//!
//! Like Siderita's embedded surface, this file is the Qt half and nothing else.
//! The document, the edit rules, the loss-free save and the whole
//! open/edit/save/close state machine — staleness included — live in
//! [`DocumentSession`]; here they are marshalled to Qt types, run on the shared
//! worker, and worded for an editor that owns its window.
//!
//! The two hosts share the state machine and word its typed outcomes
//! differently, which is the only difference that turned out to be real: this
//! one names itself and can quit, where the modal names "el editor integrado"
//! and falls back to a preview.

use std::path::{Path, PathBuf};
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use grafita_core::document::Conflict;
use grafita_core::import::Format;
use grafita_core::open::{Limits, OpenRefusal};
use grafita_core::save::{Durability, SaveRefusal};
use grafita_core::search::Query;
use grafita_core::session::{DeclineReason, DocumentSession, Event, Failure, Outcome};
use grafita_core::worker::{Completion, DocumentWorker, Job};
use grafita_core::Encoding;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // active        — a document is open
        // path / name   — the resolved file and its display name
        // windowTitle   — what the compositor shows, dirty marker included
        // encodingLabel — the encoding the document is written back in
        // dirty / busy  — differs from disk / the worker is reading or writing
        // statusText    — a completed action, for the quiet status line
        // errorText     — why the last action was refused; empty when fine
        // conflictText  — how the file on disk disagrees with this document
        // closePrompt   — the guarded-close question is being asked
        #[qobject]
        #[qml_element]
        #[qproperty(bool, active)]
        #[qproperty(QString, path)]
        #[qproperty(QString, name)]
        #[qproperty(QString, window_title)]
        #[qproperty(QString, encoding_label)]
        // encodingNames  — every encoding a document may be read as, in order
        // encodingIndex  — which of them this document uses, or -1
        // encodingRetry  — the file a refusal left waiting for an encoding,
        //                  empty when there is none
        #[qproperty(QStringList, encoding_names)]
        #[qproperty(i32, encoding_index)]
        #[qproperty(QString, encoding_retry)]
        // encodingPrompt — the chooser is up. Owned here for the same reason
        //                  `closePrompt` is: the surface that asks belongs to
        //                  one document, and the document is what knows whether
        //                  there is anything to ask about.
        #[qproperty(bool, encoding_prompt)]
        // imported / containerLabel — this document is the text inside a
        //   container somebody else wrote, and which kind. An imported document
        //   has no encoding to choose: its encoding belongs to the container.
        #[qproperty(bool, imported)]
        #[qproperty(QString, container_label)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, error_text)]
        #[qproperty(QString, conflict_text)]
        #[qproperty(bool, close_prompt)]
        // searchMatches / searchIndex — how many hits, and which is selected
        //                               (-1 when none is)
        // lineCount / indentationLabel — for the go-to-line bound and the
        //                                status line
        #[qproperty(i32, search_matches)]
        #[qproperty(i32, search_index)]
        #[qproperty(i32, line_count)]
        #[qproperty(QString, indentation_label)]
        // caretLine / caretColumn — where the widget's caret is, counted from
        //                           1, the column in characters
        #[qproperty(i32, caret_line)]
        #[qproperty(i32, caret_column)]
        // The numeric language the syntax highlighter colours by.
        #[qproperty(i32, language_id)]
        type GrafitaSession = super::GrafitaSessionRust;

        /// Reports where the widget's caret now is, as the UTF-16 offset Qt
        /// counts in. The line and character column it maps to are the
        /// document's answer, not the widget's.
        #[qinvokable]
        fn set_caret(self: Pin<&mut GrafitaSession>, offset: i32);

        /// The document's text changed underneath the widget — an open, an undo
        /// or a redo. The widget adopts `text` and puts its cursor at `caret`,
        /// counted in the UTF-16 units Qt counts.
        #[qsignal]
        fn document_reset(self: Pin<&mut GrafitaSession>, text: QString, caret: i32);

        /// Select this range in the widget and scroll it into view. Both
        /// offsets count the UTF-16 code units Qt uses.
        #[qsignal]
        fn select_range(self: Pin<&mut GrafitaSession>, start: i32, end: i32);

        /// Sets what to look for. An empty pattern clears the search.
        #[qinvokable]
        fn set_search(
            self: Pin<&mut GrafitaSession>,
            pattern: &QString,
            ignore_case: bool,
            whole_word: bool,
        );

        #[qinvokable]
        fn find_next(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn find_previous(self: Pin<&mut GrafitaSession>);

        /// Replaces the selected occurrence.
        #[qinvokable]
        fn replace_current(self: Pin<&mut GrafitaSession>, replacement: &QString);

        /// Replaces every occurrence, as one undoable action.
        #[qinvokable]
        fn replace_all(self: Pin<&mut GrafitaSession>, replacement: &QString);

        /// Moves the caret to the start of a line, counting from 1.
        #[qinvokable]
        fn go_to_line(self: Pin<&mut GrafitaSession>, line_number: i32);

        /// Asks which encoding this document should be read as, when there is
        /// something to ask about: an open saved document, or a file a refusal
        /// left waiting. Ignored otherwise, so a host may bind it to a key
        /// without testing first.
        #[qinvokable]
        fn request_encoding_chooser(self: Pin<&mut GrafitaSession>);

        /// Withdraws that question.
        #[qinvokable]
        fn cancel_encoding_chooser(self: Pin<&mut GrafitaSession>);

        /// Reads a document as the encoding at `index` in `encodingNames`.
        ///
        /// Applies to the open document when there is one, and otherwise to the
        /// file a refusal left in `encodingRetry`. A document with unsaved work
        /// is left alone: re-reading the file is how this works, and there is
        /// no way to re-read it and keep edits that were never written.
        #[qinvokable]
        fn choose_encoding(self: Pin<&mut GrafitaSession>, index: i32);

        /// The document closed. A host with tabs drops the tab; a host with one
        /// window shows its empty state.
        ///
        /// Separate from [`quit_permitted`]: closing a document and quitting the
        /// application are different things, and reporting only the second left
        /// a closed tab sitting there with nothing in it.
        #[qsignal]
        fn closed(self: Pin<&mut GrafitaSession>);

        /// The window may now close. Emitted only once no unsaved work is left,
        /// so quitting can never discard an edit silently.
        #[qsignal]
        fn quit_permitted(self: Pin<&mut GrafitaSession>);

        /// A save was asked for on a document with no file yet. The window
        /// asks where it goes and answers with [`save_as`].
        #[qsignal]
        fn destination_needed(self: Pin<&mut GrafitaSession>);

        /// The documents opened most recently that still exist, newest first.
        /// Read on demand: another window may have opened something since.
        #[qinvokable]
        fn recent_documents(self: &GrafitaSession) -> QStringList;

        /// Starts a document that belongs to no file yet.
        #[qinvokable]
        fn new_document(self: Pin<&mut GrafitaSession>);

        /// Writes the document to `path` and binds it there.
        #[qinvokable]
        fn save_as(self: Pin<&mut GrafitaSession>, path: &QString);

        /// The same, for the `file://` URL a file chooser answers with.
        ///
        /// The decoding is here rather than in QML because turning a URL into a
        /// local path is a domain rule with one owner: a destination named
        /// `informe#final.txt` arrives percent-encoded, and a surface that cut
        /// the scheme off by hand would create a file with the escape in its
        /// name while reporting success.
        #[qinvokable]
        fn save_url(self: Pin<&mut GrafitaSession>, url: &QString);

        /// The user was asked where the document goes and answered "nowhere".
        /// Any close waiting on that write is disarmed.
        #[qinvokable]
        fn cancel_save_as(self: Pin<&mut GrafitaSession>);

        /// Opens a document by path. Whether it can be edited is decided by its
        /// bytes, never by its name or its MIME entry.
        #[qinvokable]
        fn open_path(self: Pin<&mut GrafitaSession>, path: &QString);

        /// Opens a `file://` URL, for a desktop handler or a drop.
        #[qinvokable]
        fn open_url(self: Pin<&mut GrafitaSession>, url: &QString);

        /// Hands the widget's whole current text back to the document.
        #[qinvokable]
        fn apply_text(self: Pin<&mut GrafitaSession>, text: &QString);

        #[qinvokable]
        fn undo(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn redo(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn save(self: Pin<&mut GrafitaSession>);

        /// Closes the document, asking first when it has unsaved work.
        #[qinvokable]
        fn request_close(self: Pin<&mut GrafitaSession>);

        /// Asks to quit the application, which closes the document first.
        #[qinvokable]
        fn request_quit(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn save_and_close(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn discard_and_close(self: Pin<&mut GrafitaSession>);

        #[qinvokable]
        fn cancel_close(self: Pin<&mut GrafitaSession>);
    }

    impl cxx_qt::Threading for GrafitaSession {}
}

/// Qt-side mirror of the session, plus the worker its jobs run on.
pub struct GrafitaSessionRust {
    active: bool,
    path: QString,
    name: QString,
    window_title: QString,
    encoding_label: QString,
    dirty: bool,
    busy: bool,
    can_undo: bool,
    can_redo: bool,
    status_text: QString,
    error_text: QString,
    encoding_names: cxx_qt_lib::QStringList,
    encoding_index: i32,
    encoding_retry: QString,
    encoding_prompt: bool,
    imported: bool,
    container_label: QString,
    conflict_text: QString,
    close_prompt: bool,
    search_matches: i32,
    search_index: i32,
    line_count: i32,
    indentation_label: QString,
    caret_line: i32,
    caret_column: i32,
    language_id: i32,

    /// The caret the widget last reported, kept so an edit can re-answer it
    /// without the widget having to report it again.
    caret_offset: usize,
    /// A message about one named file, waiting for the publish that would
    /// otherwise replace it with the generic wording of the same refusal.
    pending_error: Option<String>,
    session: DocumentSession,
    worker: Option<DocumentWorker>,
    /// The close under way was asked for in order to quit, so completing it
    /// should end the application rather than leave an empty window.
    quitting: bool,
}

impl Default for GrafitaSessionRust {
    fn default() -> Self {
        Self {
            active: false,
            path: QString::default(),
            name: QString::default(),
            window_title: QString::from("Grafita"),
            encoding_label: QString::default(),
            dirty: false,
            busy: false,
            can_undo: false,
            can_redo: false,
            status_text: QString::default(),
            error_text: QString::default(),
            encoding_names: encoding_names(),
            encoding_index: -1,
            encoding_retry: QString::default(),
            encoding_prompt: false,
            imported: false,
            container_label: QString::default(),
            conflict_text: QString::default(),
            close_prompt: false,
            search_matches: 0,
            search_index: -1,
            line_count: 0,
            indentation_label: QString::default(),
            caret_line: 1,
            caret_column: 1,
            language_id: 0,
            caret_offset: 0,
            pending_error: None,
            session: DocumentSession::new(Limits::default()),
            worker: None,
            quitting: false,
        }
    }
}

impl qobject::GrafitaSession {
    pub fn open_path(mut self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        let outcome = self.as_mut().rust_mut().get_mut().session.open(&path);
        self.dispatch(outcome);
    }

    /// Accepts the `file://` form a desktop handler passes.
    ///
    /// Only local files: Grafita edits a file it can write back atomically, and
    /// a remote URL is not one.
    pub fn open_url(mut self: Pin<&mut Self>, url: &QString) {
        let url = url.to_string();
        match crate::url::local_path(&url) {
            Some(path) => {
                let outcome = self.as_mut().rust_mut().get_mut().session.open(&path);
                self.dispatch(outcome);
            }
            None => {
                self.as_mut()
                    .set_error_text(QString::from("Grafita solo abre archivos locales"));
            }
        }
    }

    /// Deliberately not routed through [`Self::dispatch`]: moving a caret is
    /// not a session action, so it must not clear a refusal message or
    /// re-publish the whole state on every arrow key.
    pub fn set_caret(mut self: Pin<&mut Self>, offset: i32) {
        let offset = usize::try_from(offset).unwrap_or(0);
        self.as_mut().rust_mut().get_mut().caret_offset = offset;
        self.publish_caret();
    }

    fn publish_caret(mut self: Pin<&mut Self>) {
        let location = self.rust().session.caret_location(self.rust().caret_offset);
        self.as_mut()
            .set_caret_line(i32::try_from(location.line).unwrap_or(i32::MAX));
        self.as_mut()
            .set_caret_column(i32::try_from(location.column).unwrap_or(i32::MAX));
    }

    pub fn recent_documents(&self) -> cxx_qt_lib::QStringList {
        DocumentSession::recent_documents()
            .iter()
            .map(|path| QString::from(path.to_string_lossy().as_ref()))
            .collect()
    }

    pub fn new_document(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.new_document();
        self.dispatch(outcome);
    }

    pub fn save_as(mut self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        let outcome = self.as_mut().rust_mut().get_mut().session.save_as(&path);
        self.dispatch(outcome);
    }

    /// The chooser's answer, decoded by the same rule an open is decoded by.
    pub fn save_url(mut self: Pin<&mut Self>, url: &QString) {
        let url = url.to_string();
        match crate::url::local_path(&url) {
            Some(path) => {
                let outcome = self.as_mut().rust_mut().get_mut().session.save_as(&path);
                self.dispatch(outcome);
            }
            None => {
                self.as_mut().rust_mut().get_mut().pending_error =
                    Some("Grafita solo guarda en archivos locales".to_owned());
                self.publish();
            }
        }
    }

    /// The chooser was dismissed. The document stays exactly as it is, and a
    /// close that was waiting for the write stops waiting for it.
    pub fn cancel_save_as(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().quitting = false;
        let outcome = self.as_mut().rust_mut().get_mut().session.cancel_save_as();
        self.dispatch(outcome);
    }

    pub fn apply_text(mut self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .apply_display_text(&text);
        self.dispatch(outcome);
    }

    pub fn set_search(
        mut self: Pin<&mut Self>,
        pattern: &QString,
        ignore_case: bool,
        whole_word: bool,
    ) {
        let pattern = pattern.to_string();
        let query = Query {
            ignore_case,
            whole_word,
        };
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .set_search(&pattern, query);
        self.dispatch(outcome);
    }

    pub fn find_next(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.find_next();
        self.dispatch(outcome);
    }

    pub fn find_previous(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.find_previous();
        self.dispatch(outcome);
    }

    pub fn replace_current(mut self: Pin<&mut Self>, replacement: &QString) {
        let replacement = replacement.to_string();
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .replace_current(&replacement);
        self.dispatch(outcome);
    }

    pub fn replace_all(mut self: Pin<&mut Self>, replacement: &QString) {
        let replacement = replacement.to_string();
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .replace_all(&replacement);
        self.dispatch(outcome);
    }

    pub fn go_to_line(mut self: Pin<&mut Self>, line_number: i32) {
        let line_number = usize::try_from(line_number).unwrap_or(1);
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .go_to_line(line_number);
        self.dispatch(outcome);
    }

    pub fn undo(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.undo();
        self.dispatch(outcome);
    }

    pub fn redo(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.redo();
        self.dispatch(outcome);
    }

    pub fn save(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.save();
        self.dispatch(outcome);
    }

    pub fn request_close(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().quitting = false;
        let outcome = self.as_mut().rust_mut().get_mut().session.request_close();
        self.dispatch(outcome);
    }

    /// Quitting goes through the same guarded close as any other close, so a
    /// dirty document stops the window from disappearing.
    pub fn request_quit(mut self: Pin<&mut Self>) {
        if !self.rust().active {
            self.as_mut().quit_permitted();
            return;
        }
        self.as_mut().rust_mut().get_mut().quitting = true;
        let outcome = self.as_mut().rust_mut().get_mut().session.request_close();
        self.dispatch(outcome);
    }

    pub fn save_and_close(mut self: Pin<&mut Self>) {
        let outcome = self.as_mut().rust_mut().get_mut().session.save_and_close();
        self.dispatch(outcome);
    }

    pub fn discard_and_close(mut self: Pin<&mut Self>) {
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .discard_and_close();
        self.dispatch(outcome);
    }

    pub fn cancel_close(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().quitting = false;
        let outcome = self.as_mut().rust_mut().get_mut().session.cancel_close();
        self.dispatch(outcome);
    }

    /// The single funnel every session answer passes through.
    fn dispatch(mut self: Pin<&mut Self>, outcome: Outcome) {
        if let Some(job) = outcome.job {
            self.as_mut().submit(job);
        }
        match outcome.event {
            Some(Event::PushText { text, caret }) => {
                let caret = i32::try_from(caret).unwrap_or(i32::MAX);
                self.as_mut()
                    .document_reset(QString::from(text.as_str()), caret);
            }
            Some(Event::Declined { path, reason }) => {
                // Held rather than set: `publish` below reports the same
                // refusal in general terms, and whichever ran last would be the
                // only one the user ever saw. The one naming the file wins.
                let message = format!("{}: {}", display_name(&path), decline_text(reason));
                self.as_mut().rust_mut().get_mut().pending_error = Some(message);
                // A file refused for what its bytes are may still be text in an
                // encoding nothing in it declares. Held so the chooser has
                // something to retry; a refusal for any other reason clears it,
                // because naming an encoding cannot make a missing file appear.
                let retry = match reason {
                    DeclineReason::UnsupportedEncoding | DeclineReason::NotText => {
                        QString::from(path.to_string_lossy().as_ref())
                    }
                    _ => QString::default(),
                };
                self.as_mut().set_encoding_retry(retry);
            }
            Some(Event::Select { start, end }) => {
                let start = i32::try_from(start).unwrap_or(i32::MAX);
                let end = i32::try_from(end).unwrap_or(i32::MAX);
                self.as_mut().select_range(start, end);
            }
            // The standalone application never asks a classify-only question:
            // it opens what it is given. Named rather than caught by a
            // wildcard, so a new event has to be considered here too.
            Some(Event::DestinationNeeded) => self.as_mut().destination_needed(),
            Some(Event::Classified { .. }) => {}
            Some(Event::Closed) => {
                let quitting = self.rust().quitting;
                self.as_mut().rust_mut().get_mut().quitting = false;
                self.as_mut().closed();
                if quitting {
                    self.as_mut().quit_permitted();
                }
            }
            None => {}
        }
        self.publish();
    }

    /// Starts the worker on first use and hands it a job.
    fn submit(mut self: Pin<&mut Self>, job: Job) {
        if self.rust().worker.is_none() {
            let qt = self.as_mut().qt_thread();
            match DocumentWorker::new(move |completion| {
                let _ = qt.queue(move |session: Pin<&mut qobject::GrafitaSession>| {
                    session.receive(completion);
                });
            }) {
                Ok(worker) => self.as_mut().rust_mut().get_mut().worker = Some(worker),
                Err(error) => {
                    self.as_mut().set_busy(false);
                    self.as_mut().set_error_text(QString::from(
                        format!("No se pudo iniciar Grafita: {error}").as_str(),
                    ));
                    return;
                }
            }
        }
        let stopped = self
            .rust()
            .worker
            .as_ref()
            .is_some_and(|worker| worker.submit(job).is_err());
        if stopped {
            self.as_mut().set_busy(false);
        }
    }

    pub fn request_encoding_chooser(mut self: Pin<&mut Self>) {
        let session = &self.rust().session;
        // An imported document's encoding is the container's business. There is
        // nothing here for the author to choose, so the question is not asked.
        if session.state().container.is_some() {
            return;
        }
        let open_and_saved = session.state().active && !session.state().dirty;
        let waiting = !self.rust().encoding_retry.is_empty();
        if open_and_saved || waiting {
            self.as_mut().set_encoding_prompt(true);
        }
    }

    pub fn cancel_encoding_chooser(mut self: Pin<&mut Self>) {
        self.as_mut().set_encoding_prompt(false);
    }

    /// Reads a document as the encoding at `index`, on the document that is
    /// open or on the file a refusal left waiting.
    pub fn choose_encoding(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().set_encoding_prompt(false);
        let catalogue = Encoding::catalogue();
        let Some(encoding) = usize::try_from(index)
            .ok()
            .and_then(|index| catalogue.get(index))
            .copied()
        else {
            return;
        };
        let retry = self.rust().encoding_retry.to_string();
        let outcome = if self.rust().session.state().active {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .session
                .reopen_with(encoding)
        } else if retry.is_empty() {
            return;
        } else {
            self.as_mut()
                .rust_mut()
                .get_mut()
                .session
                .open_with(Path::new(&retry), encoding)
        };
        self.as_mut().set_encoding_retry(QString::default());
        self.dispatch(outcome);
    }

    fn receive(mut self: Pin<&mut Self>, completion: Completion) {
        let outcome = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .session
            .receive(completion);
        self.dispatch(outcome);
    }

    /// Copies the session's state into the properties QML binds to.
    ///
    /// A message that names a file outranks the general wording of the same
    /// refusal: "programa: no es texto…" tells the user which of the two files
    /// they just opened was refused, where "Este archivo no es texto" does not.
    fn publish(mut self: Pin<&mut Self>) {
        let state = self.rust().session.state().clone();

        self.as_mut().set_active(state.active);
        self.as_mut().set_dirty(state.dirty);
        self.as_mut().set_busy(state.busy);
        self.as_mut().set_can_undo(state.can_undo);
        self.as_mut().set_can_redo(state.can_redo);
        self.as_mut().set_close_prompt(state.close_prompt);
        self.as_mut()
            .set_path(QString::from(state.path.to_string_lossy().as_ref()));
        self.as_mut().set_name(QString::from(state.name.as_str()));
        self.as_mut().set_encoding_label(QString::from(
            state
                .encoding
                .map(|encoding| encoding.label())
                .unwrap_or(""),
        ));
        self.as_mut().set_imported(state.container.is_some());
        self.as_mut()
            .set_container_label(QString::from(state.container.map_or("", Format::label)));
        let index = state
            .encoding
            .and_then(|encoding| {
                Encoding::catalogue()
                    .iter()
                    .position(|item| *item == encoding)
            })
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);
        self.as_mut().set_encoding_index(index);

        let title = if state.active {
            let marker = if state.dirty { "• " } else { "" };
            let name = if state.name.is_empty() {
                "Sin título"
            } else {
                state.name.as_str()
            };
            format!("{marker}{name} — Grafita")
        } else {
            "Grafita".to_owned()
        };
        self.as_mut()
            .set_window_title(QString::from(title.as_str()));

        let status = match state.saved {
            Some(Durability::Durable) => "Guardado",
            Some(Durability::Reduced { .. }) => "Guardado, pero la carpeta no pudo sincronizarse",
            None => "",
        };
        self.as_mut().set_status_text(QString::from(status));

        let pending = self.as_mut().rust_mut().get_mut().pending_error.take();
        if let Some(message) = pending {
            self.as_mut()
                .set_error_text(QString::from(message.as_str()));
        } else if let Some(failure) = state.failure.as_ref() {
            let text = failure_text(failure);
            self.as_mut().set_error_text(QString::from(text.as_str()));
        } else if state.active {
            self.as_mut().set_error_text(QString::default());
        }

        let conflict = state
            .conflict
            .as_ref()
            .map(conflict_text)
            .unwrap_or_default();
        self.as_mut().set_conflict_text(QString::from(conflict));

        self.as_mut()
            .set_search_matches(i32::try_from(state.search_matches).unwrap_or(i32::MAX));
        self.as_mut().set_search_index(
            state
                .search_index
                .and_then(|index| i32::try_from(index).ok())
                .unwrap_or(-1),
        );
        let lines = self.rust().session.line_count();
        self.as_mut()
            .set_line_count(i32::try_from(lines).unwrap_or(i32::MAX));
        let indentation = self.rust().session.indentation().map(indentation_label);
        self.as_mut()
            .set_indentation_label(QString::from(indentation.unwrap_or("")));
        let language = crate::syntax::language_code(self.rust().session.language());
        self.as_mut().set_language_id(i32::from(language));
        // The same offset can be a different line after an edit, an undo or an
        // open, so the readout is re-derived here rather than waiting for the
        // widget to report a caret that did not itself move.
        self.publish_caret();
    }
}

/// The indentation the document uses, in words for the status line.
const fn indentation_label(indentation: grafita_core::Indentation) -> &'static str {
    match indentation {
        grafita_core::Indentation::Tabs => "Tabuladores",
        grafita_core::Indentation::Spaces { width: 1 } => "1 espacio",
        grafita_core::Indentation::Spaces { width: 2 } => "2 espacios",
        grafita_core::Indentation::Spaces { width: 4 } => "4 espacios",
        grafita_core::Indentation::Spaces { width: 8 } => "8 espacios",
        grafita_core::Indentation::Spaces { .. } => "Espacios",
        grafita_core::Indentation::Mixed => "Indentación mixta",
        grafita_core::Indentation::None => "",
    }
}

/// Every encoding a document may be read as, in the core's catalogue order.
fn encoding_names() -> cxx_qt_lib::QStringList {
    Encoding::catalogue()
        .iter()
        .map(|encoding| QString::from(encoding.label()))
        .collect()
}

fn display_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// The wording is this surface's: Grafita names itself, where the embedded
/// modal speaks of "el editor integrado".
const fn decline_text(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::NotText => "no es texto, así que Grafita no puede editarlo",
        DeclineReason::UnsupportedEncoding => {
            "usa una codificación que Grafita aún no puede editar sin perder sus bytes"
        }
        DeclineReason::Unreadable => "no se pudo leer",
    }
}

fn failure_text(failure: &Failure) -> String {
    match failure {
        Failure::Open(refusal) => open_refusal_text(refusal).to_owned(),
        Failure::Save(refusal) => save_refusal_text(refusal),
        Failure::Edit(_) => "El editor se resincronizó con el documento".to_owned(),
    }
}

const fn open_refusal_text(refusal: &OpenRefusal) -> &'static str {
    match refusal {
        OpenRefusal::NotText { .. } => "Este archivo no es texto",
        OpenRefusal::UnsupportedEncoding { .. } => {
            "Este texto usa una codificación que Grafita aún no puede editar sin perder sus bytes"
        }
        OpenRefusal::TooLarge { .. } => "Este archivo es demasiado grande para Grafita",
        OpenRefusal::ChangedWhileReading { .. } => {
            "El archivo cambió mientras se leía; inténtalo otra vez"
        }
        OpenRefusal::NotImportable { .. } => {
            "Este archivo es un contenedor que Grafita no puede editar"
        }
        OpenRefusal::Cancelled => "",
        OpenRefusal::Io { .. } => "No se pudo leer el archivo",
    }
}

fn save_refusal_text(refusal: &SaveRefusal) -> String {
    match refusal {
        SaveRefusal::Retargeted { .. } => {
            "La ruta ahora lleva a otro archivo; no se ha escrito nada".to_owned()
        }
        SaveRefusal::ChangedUnderneath { .. } => {
            "El archivo cambió en disco desde que se abrió; no se ha escrito nada".to_owned()
        }
        SaveRefusal::TargetMissing { .. } => {
            "El archivo ya no existe; no se ha escrito nada".to_owned()
        }
        SaveRefusal::MetadataNotReproducible { source } => {
            format!("No se guardó para no perder metadatos del original: {source}")
        }
        SaveRefusal::Unrepresentable { source } => format!(
            "«{}» no existe en {}; no se ha escrito nada",
            source.character,
            source.encoding.label()
        ),
        SaveRefusal::StructureChanged { detail } => {
            format!("El texto ya no encaja en el documento original: {detail}")
        }
        SaveRefusal::Cancelled => String::new(),
        SaveRefusal::Io { .. } => "No se pudo escribir el archivo".to_owned(),
    }
}

const fn conflict_text(conflict: &Conflict) -> &'static str {
    match conflict {
        Conflict::ChangedUnderneath => "Otro programa cambió este archivo desde que se abrió",
        Conflict::Retargeted { .. } => "La ruta ahora lleva a otro archivo",
        Conflict::Missing => "El archivo ya no existe",
    }
}
