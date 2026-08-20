//! Siderita's adapter for the embedded Grafita editor.
//!
//! Everything about what text is, how it is edited and when it may be written
//! lives in `grafita-core`; the whole open/edit/save/close state machine — and
//! the staleness rules around it — lives in its [`DocumentSession`]. This file
//! is the Qt half and nothing else: it moves values between `QString` and Rust,
//! runs the session's jobs on the shared worker, and words the session's typed
//! outcomes for a modal inside a file manager.
//!
//! It is its own QObject rather than more surface on `SideritaController`: an
//! open document has its own state and its own lifetime, shares nothing with
//! folder scanning, and must be able to go away without disturbing the folder
//! underneath.
//!
//! The text widget never owns the text. It is handed `grafita-core`'s line-feed
//! projection and reports the whole string back on every change; the session
//! turns that into the one splice it represents, which is what keeps a CRLF
//! file from being silently rewritten by Qt.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use grafita_core::document::Conflict;
use grafita_core::open::{Limits, OpenRefusal};
use grafita_core::save::{Durability, SaveRefusal};
use grafita_core::session::{DeclineReason, DocumentSession, Event, Failure, Outcome};
use grafita_core::worker::{Completion, DocumentWorker, Job};

/// The embedded surface is a modal inside a file manager, not a workbench. A
/// document past this size belongs in the standalone application, where the
/// user chose to open an editor in the first place.
const EMBEDDED_MAX_BYTES: u64 = 8 * 1024 * 1024;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // active        — a document is open and the modal should be showing
        // path / name   — the file's path key (ADR 0008) and its display name
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
        #[qproperty(QString, encoding_label)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, error_text)]
        #[qproperty(QString, conflict_text)]
        #[qproperty(bool, close_prompt)]
        // caretLine / caretColumn — where the widget's caret is, counted from
        //                           1, the column in characters
        #[qproperty(i32, caret_line)]
        #[qproperty(i32, caret_column)]
        type GrafitaEditor = super::GrafitaEditorRust;

        /// The document's text changed underneath the widget — an open, an undo
        /// or a redo. The widget adopts `text` and puts its cursor at `caret`,
        /// which counts the UTF-16 units Qt counts.
        ///
        /// Assigning this text back is not an edit: the core recognises its own
        /// projection and records nothing, so no re-entrancy flag is needed.
        #[qsignal]
        fn document_reset(self: Pin<&mut GrafitaEditor>, text: QString, caret: i32);

        /// The probed file is not editable text. The caller falls back to the
        /// quick-look preview for this file, named by its path key.
        #[qsignal]
        fn preview_declined(self: Pin<&mut GrafitaEditor>, path: QString, reason: QString);

        /// The document closed and the folder underneath should take the
        /// keyboard back.
        #[qsignal]
        fn closed(self: Pin<&mut GrafitaEditor>);

        /// The answer to [`request_launch`]: whether the file this path key
        /// names holds editable text, so activation can pick the editor or the
        /// desktop's handler. The key is the same one the caller passed in.
        #[qsignal]
        fn launch_decided(self: Pin<&mut GrafitaEditor>, path: QString, editable: bool);

        /// Asks — by content, never by name — whether the file the path key
        /// `path` names is text Grafita should open. Nothing is opened here;
        /// the answer arrives as [`launch_decided`].
        #[qinvokable]
        fn request_launch(self: Pin<&mut GrafitaEditor>, path: &QString);

        /// Opens the file the path key `path` names in the standalone Grafita
        /// application.
        ///
        /// Reports whether the launcher could be started at all; a missing
        /// binary is a truthful failure, not a silent no-op.
        #[qinvokable]
        fn launch_standalone(self: Pin<&mut GrafitaEditor>, path: &QString) -> bool;

        /// Classifies the file the path key `path` names and opens the editor
        /// when it is editable text.
        /// Non-text answers with [`preview_declined`] instead.
        #[qinvokable]
        fn request_preview(self: Pin<&mut GrafitaEditor>, path: &QString);

        /// Hands the widget's whole current text back to the document.
        #[qinvokable]
        fn apply_text(self: Pin<&mut GrafitaEditor>, text: &QString);

        /// Reports where the widget's caret now is, as the UTF-16 offset Qt
        /// counts in. Which line and character column that is remains the
        /// document's answer, so both surfaces agree on it.
        #[qinvokable]
        fn set_caret(self: Pin<&mut GrafitaEditor>, offset: i32);

        #[qinvokable]
        fn undo(self: Pin<&mut GrafitaEditor>);

        #[qinvokable]
        fn redo(self: Pin<&mut GrafitaEditor>);

        /// Queues a write. The document stays dirty until the worker reports a
        /// completed save for the revision the user is looking at.
        #[qinvokable]
        fn save(self: Pin<&mut GrafitaEditor>);

        /// Asks to close. A dirty document raises the guarded-close question
        /// instead of closing.
        #[qinvokable]
        fn request_close(self: Pin<&mut GrafitaEditor>);

        /// Answers the guarded-close question: write, then close.
        #[qinvokable]
        fn save_and_close(self: Pin<&mut GrafitaEditor>);

        /// Answers it by throwing the edit away.
        #[qinvokable]
        fn discard_and_close(self: Pin<&mut GrafitaEditor>);

        /// Answers it by staying in the document.
        #[qinvokable]
        fn cancel_close(self: Pin<&mut GrafitaEditor>);
    }

    impl cxx_qt::Threading for GrafitaEditor {}
}

/// Qt-side mirror of the session, plus the worker the session's jobs run on.
pub struct GrafitaEditorRust {
    active: bool,
    path: QString,
    name: QString,
    encoding_label: QString,
    dirty: bool,
    busy: bool,
    can_undo: bool,
    can_redo: bool,
    status_text: QString,
    error_text: QString,
    conflict_text: QString,
    close_prompt: bool,
    caret_line: i32,
    caret_column: i32,

    /// The caret the widget last reported, kept so an edit can re-answer it
    /// without the widget having to report it again.
    caret_offset: usize,
    session: DocumentSession,
    worker: Option<DocumentWorker>,
}

impl Default for GrafitaEditorRust {
    fn default() -> Self {
        Self {
            active: false,
            path: QString::default(),
            name: QString::default(),
            encoding_label: QString::default(),
            dirty: false,
            busy: false,
            can_undo: false,
            can_redo: false,
            status_text: QString::default(),
            error_text: QString::default(),
            conflict_text: QString::default(),
            close_prompt: false,
            caret_line: 1,
            caret_column: 1,
            caret_offset: 0,
            session: DocumentSession::new(Limits {
                max_bytes: EMBEDDED_MAX_BYTES,
                ..Limits::default()
            }),
            worker: None,
        }
    }
}

impl qobject::GrafitaEditor {
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

    pub fn request_preview(mut self: Pin<&mut Self>, path: &QString) {
        let Ok(path) = crate::pathkey::decode(path) else {
            return;
        };
        let outcome = self.as_mut().rust_mut().get_mut().session.open(&path);
        self.dispatch(outcome);
    }

    pub fn request_launch(mut self: Pin<&mut Self>, path: &QString) {
        let Ok(path) = crate::pathkey::decode(path) else {
            return;
        };
        let outcome = self.as_mut().rust_mut().get_mut().session.classify(&path);
        self.dispatch(outcome);
    }

    pub fn launch_standalone(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let Ok(path) = crate::pathkey::decode(path) else {
            return false;
        };
        match crate::controller::shell::spawn_detached("grafita", &path) {
            Ok(()) => true,
            Err(error) => {
                self.as_mut().set_error_text(QString::from(error.as_str()));
                false
            }
        }
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
        let outcome = self.as_mut().rust_mut().get_mut().session.cancel_close();
        self.dispatch(outcome);
    }

    /// The single funnel every session answer passes through: run its job, act
    /// on its event, and mirror its state into the Qt properties.
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
                let path = crate::pathkey::publish(&path);
                self.as_mut()
                    .preview_declined(path, QString::from(decline_text(reason)));
            }
            Some(Event::Classified { path, editable }) => {
                let path = crate::pathkey::publish(&path);
                self.as_mut().launch_decided(path, editable);
            }
            // The embedded surface only ever edits a file that already
            // exists — it is reached by pressing Space on one — so it can never
            // be the thing that needs somewhere to go. Named rather than caught
            // by a wildcard, so a new event has to be considered here too.
            Some(Event::DestinationNeeded | Event::Select { .. }) => {}
            Some(Event::Closed) => self.as_mut().closed(),
            None => {}
        }
        self.publish();
    }

    /// Starts the worker on first use and hands it a job.
    fn submit(mut self: Pin<&mut Self>, job: Job) {
        if self.rust().worker.is_none() {
            let qt = self.as_mut().qt_thread();
            match DocumentWorker::new(move |completion| {
                let _ = qt.queue(move |editor: Pin<&mut qobject::GrafitaEditor>| {
                    editor.receive(completion);
                });
            }) {
                Ok(worker) => self.as_mut().rust_mut().get_mut().worker = Some(worker),
                Err(error) => {
                    self.as_mut().set_busy(false);
                    self.as_mut().set_error_text(QString::from(
                        format!("No se pudo iniciar el editor: {error}").as_str(),
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

    /// Takes one answer from the worker, on the GUI thread.
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
    fn publish(mut self: Pin<&mut Self>) {
        let state = self.rust().session.state().clone();

        self.as_mut().set_active(state.active);
        self.as_mut().set_dirty(state.dirty);
        self.as_mut().set_busy(state.busy);
        self.as_mut().set_can_undo(state.can_undo);
        self.as_mut().set_can_redo(state.can_redo);
        self.as_mut().set_close_prompt(state.close_prompt);
        self.as_mut().set_path(crate::pathkey::publish(&state.path));
        self.as_mut().set_name(QString::from(state.name.as_str()));
        self.as_mut().set_encoding_label(QString::from(
            state
                .encoding
                .map(|encoding| encoding.label())
                .unwrap_or(""),
        ));

        let status = match state.saved {
            Some(Durability::Durable) => "Guardado",
            Some(Durability::Reduced { .. }) => "Guardado, pero la carpeta no pudo sincronizarse",
            None => "",
        };
        self.as_mut().set_status_text(QString::from(status));

        let error = state.failure.as_ref().map(failure_text).unwrap_or_default();
        self.as_mut().set_error_text(QString::from(error.as_str()));

        let conflict = state
            .conflict
            .as_ref()
            .map(conflict_text)
            .unwrap_or_default();
        self.as_mut().set_conflict_text(QString::from(conflict));
        // The same offset can be a different line after an edit, an undo or an
        // open, so the readout is re-derived here rather than waiting for the
        // widget to report a caret that did not itself move.
        self.publish_caret();
    }
}

/// The wording belongs to this surface, not to the core: a modal inside a file
/// manager says "el editor integrado", where the standalone application would
/// name itself.
const fn decline_text(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::NotText => "este archivo no es texto",
        DeclineReason::UnsupportedEncoding => {
            "este texto usa una codificación que aún no se puede editar sin perder sus bytes"
        }
        DeclineReason::Unreadable => "no se pudo leer este archivo",
    }
}

fn failure_text(failure: &Failure) -> String {
    match failure {
        Failure::Open(refusal) => open_refusal_text(refusal).to_owned(),
        Failure::Save(refusal) => save_refusal_text(refusal),
        // A widget and its document out of step is a defect, not a decision the
        // user made; it is reported plainly rather than blamed on the file.
        Failure::Edit(_) => "El editor se resincronizó con el documento".to_owned(),
    }
}

const fn open_refusal_text(refusal: &OpenRefusal) -> &'static str {
    match refusal {
        OpenRefusal::NotText { .. } => "Este archivo no es texto",
        OpenRefusal::UnsupportedEncoding { .. } => {
            "Este texto usa una codificación que aún no se puede editar sin perder sus bytes"
        }
        OpenRefusal::TooLarge { .. } => "Este archivo es demasiado grande para el editor integrado",
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
