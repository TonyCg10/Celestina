//! The editing session as a state machine: what a host is told to do, and what
//! it is protected from. Driven without a worker — the jobs are run inline —
//! because the point of splitting the session out was that it can be.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use celestina_core::{CancellationToken, Generation, GenerationClock};
use grafita_core::open::{open, probe, Limits, OpenRefusal};
use grafita_core::save::perform;
use grafita_core::session::{DeclineReason, DocumentSession, Event, Failure};
use grafita_core::worker::{Completion, Job};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(1);

fn scratch(label: &str) -> PathBuf {
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "grafita-session-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
}

/// Runs a session's job inline, the way a worker would, and hands the
/// answer back. No threads: the session is a state machine and the point of
/// the split is that it can be tested as one.
fn pump(session: &mut DocumentSession, job: Job) -> Option<Event> {
    let cancellation = CancellationToken::new();
    let completion = match job {
        Job::Probe {
            path,
            generation,
            limits,
        }
        | Job::Classify {
            path,
            generation,
            limits,
        } => Completion::Probed {
            generation,
            result: Box::new(probe(&path, generation, limits, &cancellation)),
        },
        Job::Open {
            path,
            generation,
            limits,
        } => Completion::Opened {
            generation,
            result: Box::new(open(&path, generation, limits, &cancellation)),
        },
        Job::SaveAs {
            path,
            bytes,
            generation,
            revision,
        } => Completion::Created {
            generation,
            revision,
            result: Box::new(grafita_core::save::create(&path, &bytes)),
        },
        Job::Save {
            request,
            generation,
        } => Completion::Saved {
            generation,
            revision: request.revision(),
            result: Box::new(perform(&request, &cancellation)),
        },
    };
    let outcome = session.receive(completion);
    if let Some(next) = outcome.job {
        return pump(session, next);
    }
    outcome.event
}

fn open_session(path: &std::path::Path) -> (DocumentSession, Option<Event>) {
    let mut session = DocumentSession::new(Limits::default());
    let outcome = session.open(path);
    let job = outcome.job.expect("a probe job");
    let event = pump(&mut session, job);
    (session, event)
}

#[test]
fn a_text_file_opens_and_hands_its_projection_to_the_widget() {
    let root = scratch("session-open");
    let path = root.join("notas.txt");
    fs::write(&path, b"uno\r\ndos\r\n").expect("write");

    let (session, event) = open_session(&path);

    assert_eq!(
        event,
        Some(Event::PushText {
            text: "uno\ndos\n".to_owned(),
            caret: 0
        })
    );
    assert!(session.state().active);
    assert!(!session.state().dirty);
    assert!(!session.state().busy);
    assert_eq!(session.state().name, "notas.txt");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_binary_file_is_declined_by_content_and_opens_nothing() {
    let root = scratch("session-binary");
    let path = root.join("programa");
    fs::write(&path, b"\x7fELF\x02\x01\x01\x00\x00\x00").expect("write");

    let (session, event) = open_session(&path);

    assert!(matches!(
        event,
        Some(Event::Declined {
            reason: DeclineReason::NotText,
            ..
        })
    ));
    assert!(!session.state().active);
    assert!(!session.state().busy);
    assert!(matches!(
        session.state().failure,
        Some(Failure::Open(OpenRefusal::NotText { .. }))
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn typing_saving_and_closing_walk_the_whole_state_machine() {
    let root = scratch("session-cycle");
    let path = root.join("borrador.txt");
    fs::write(&path, b"uno\r\ndos\r\n").expect("write");

    let (mut session, _) = open_session(&path);

    // The widget reports its whole text, as a keystroke makes it do.
    let _ = session.apply_display_text("uno\ndos EDITADO\n");
    assert!(session.state().dirty);
    assert!(session.state().can_undo);

    // Closing over unsaved work asks rather than closing.
    let outcome = session.request_close();
    assert!(outcome.event.is_none());
    assert!(session.state().close_prompt);
    assert!(session.state().active);

    // Answering "save" writes and then closes.
    let outcome = session.save_and_close();
    let event = pump(&mut session, outcome.job.expect("a save job"));

    assert_eq!(event, Some(Event::Closed));
    assert!(!session.state().active);
    assert!(!session.state().close_prompt);
    assert_eq!(
        fs::read(&path).expect("read back"),
        b"uno\r\ndos EDITADO\r\n",
        "the terminators the file came with are what it keeps"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn undo_pushes_the_document_back_to_the_widget_and_redo_returns() {
    let root = scratch("session-undo");
    let path = root.join("historial.txt");
    fs::write(&path, b"base\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.apply_display_text("base\nsegunda\n");

    // Undo puts the caret back where the edit began — offset 5, the start
    // of the second line — not at the top of the document.
    let outcome = session.undo();
    assert_eq!(
        outcome.event,
        Some(Event::PushText {
            text: "base\n".to_owned(),
            caret: 5
        })
    );
    assert!(!session.state().dirty);
    assert!(session.state().can_redo);

    let outcome = session.redo();
    assert!(matches!(outcome.event, Some(Event::PushText { .. })));
    assert!(session.state().dirty);

    // Handing the document its own projection back is not an edit.
    let text = session.display_text().to_owned();
    let outcome = session.apply_display_text(&text);
    assert!(outcome.event.is_none() && outcome.job.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_save_report_for_a_closed_document_is_dropped() {
    let root = scratch("session-stale");
    let path = root.join("cerrado.txt");
    fs::write(&path, b"contenido\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.apply_display_text("contenido editado\n");
    let outcome = session.save();
    let job = outcome.job.expect("a save job");

    // The user closes before the worker answers.
    let _ = session.discard_and_close();
    assert!(!session.state().active);

    let event = pump(&mut session, job);
    assert_eq!(event, None, "a closed document takes no reports");
    assert!(!session.state().active);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_answer_older_than_the_newest_question_is_dropped() {
    let root = scratch("session-generation");
    let first = root.join("primero.txt");
    let second = root.join("segundo.txt");
    fs::write(&first, b"primero\n").expect("write");
    fs::write(&second, b"segundo\n").expect("write");

    let mut session = DocumentSession::new(Limits::default());
    let stale = session.open(&first).job.expect("a probe job");
    let fresh = session.open(&second).job.expect("a probe job");

    // The stale answer arrives after the newer question was asked.
    assert_eq!(pump(&mut session, stale), None);
    assert!(!session.state().active);

    let _ = pump(&mut session, fresh);
    assert_eq!(session.state().name, "segundo.txt");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_search_selects_matches_in_order_and_wraps_both_ways() {
    let root = scratch("session-search");
    let path = root.join("buscar.txt");
    fs::write(&path, b"uno dos uno\ntres uno\n").expect("write");

    let (mut session, _) = open_session(&path);
    let outcome = session.set_search("uno", grafita_core::search::Query::default());

    assert_eq!(session.state().search_matches, 3);
    assert_eq!(session.state().search_index, Some(0));
    assert_eq!(outcome.event, Some(Event::Select { start: 0, end: 3 }));

    let outcome = session.find_next();
    assert_eq!(session.state().search_index, Some(1));
    assert_eq!(outcome.event, Some(Event::Select { start: 8, end: 11 }));

    // Past the last one it wraps back to the first.
    let _ = session.find_next();
    let outcome = session.find_next();
    assert_eq!(session.state().search_index, Some(0));
    assert_eq!(outcome.event, Some(Event::Select { start: 0, end: 3 }));

    // And backwards from the first wraps to the last.
    let outcome = session.find_previous();
    assert_eq!(session.state().search_index, Some(2));
    assert!(matches!(outcome.event, Some(Event::Select { .. })));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn editing_the_document_keeps_the_match_count_honest() {
    let root = scratch("session-search-stale");
    let path = root.join("cambia.txt");
    fs::write(&path, b"gato gato\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.set_search("gato", grafita_core::search::Query::default());
    assert_eq!(session.state().search_matches, 2);

    // A match list computed before an edit describes a document that no
    // longer exists, so it is recomputed rather than left to go stale.
    let _ = session.apply_display_text("gato\n");
    assert_eq!(session.state().search_matches, 1);

    let _ = session.apply_display_text("nada\n");
    assert_eq!(session.state().search_matches, 0);
    assert_eq!(session.state().search_index, None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn replace_all_through_the_session_is_one_undo_and_clears_the_selection() {
    let root = scratch("session-replace-all");
    let path = root.join("reemplazo.txt");
    fs::write(&path, b"a x a x a\r\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.set_search("x", grafita_core::search::Query::default());
    let outcome = session.replace_all("Y");

    assert_eq!(session.display_text(), "a Y a Y a\n");
    assert_eq!(session.state().search_matches, 0);
    assert!(matches!(outcome.event, Some(Event::PushText { .. })));

    // One undo, and the CRLF the file came with is still there.
    let _ = session.undo();
    assert_eq!(
        session.document().expect("document").text(),
        "a x a x a\r\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn replacing_one_match_at_a_time_walks_the_document_without_skipping() {
    let root = scratch("session-replace-one");
    let path = root.join("uno-a-uno.txt");
    fs::write(&path, b"n n n\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.set_search("n", grafita_core::search::Query::default());

    // Removing a match shifts the rest down, so keeping the index is what
    // moves on. Three replacements must consume exactly three matches.
    for expected in [2, 1, 0] {
        let _ = session.replace_current("m");
        assert_eq!(session.state().search_matches, expected);
    }
    assert_eq!(session.display_text(), "m m m\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn go_to_line_lands_on_the_line_start_and_clamps() {
    let root = scratch("session-goto");
    let path = root.join("lineas.txt");
    fs::write(&path, b"una\ndos\ntres\n").expect("write");

    let (mut session, _) = open_session(&path);

    assert_eq!(
        session.go_to_line(2).event,
        Some(Event::Select { start: 4, end: 4 })
    );
    // Past the end goes as far as the document allows.
    assert_eq!(
        session.go_to_line(9_999).event,
        Some(Event::Select { start: 13, end: 13 })
    );
    assert_eq!(session.line_count(), 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn generations_advance_with_every_question() {
    let mut clock = GenerationClock::default();
    assert_eq!(clock.current(), Generation::INITIAL);
    assert!(clock.issue().is_ok());
}

/// Activation asks whether a file is text without opening it: the question must
/// not disturb a document already open, and must answer for the file's bytes
/// rather than its name.
#[test]
fn classify_answers_without_opening_anything() {
    let root = scratch("classify");
    let text = root.join("cancion.mp3");
    let binary = root.join("documento.txt");
    // Both names lie about their content, which is the whole point.
    fs::write(&text, b"esto es texto\n").expect("write");
    fs::write(&binary, b"\x7fELF\x02\x01\x01\x00\x00\x00").expect("write");

    let mut session = DocumentSession::new(Limits::default());

    for (path, editable) in [(&text, true), (&binary, false)] {
        let outcome = session.classify(path);
        let event = pump(&mut session, outcome.job.expect("a probe job"));
        match event {
            Some(Event::Classified {
                path: answered,
                editable: verdict,
            }) => {
                assert_eq!(&answered, path);
                assert_eq!(verdict, editable, "{}", path.display());
            }
            other => panic!("unexpected event for {}: {other:?}", path.display()),
        }
        assert!(!session.state().active, "classifying must open nothing");
        assert!(!session.state().busy, "and must not look busy afterwards");
    }

    let _ = fs::remove_dir_all(root);
}

/// A classification asked for while a document is open must leave that document
/// exactly where it was.
#[test]
fn classifying_does_not_disturb_an_open_document() {
    let root = scratch("classify-open");
    let open_me = root.join("abierto.txt");
    let other = root.join("otro.txt");
    fs::write(&open_me, b"contenido\n").expect("write");
    fs::write(&other, b"otra cosa\n").expect("write");

    let (mut session, _) = open_session(&open_me);
    let _ = session.apply_display_text("contenido editado\n");
    assert!(session.state().dirty);

    let outcome = session.classify(&other);
    let _ = pump(&mut session, outcome.job.expect("a probe job"));

    assert!(session.state().active);
    assert!(session.state().dirty, "the edit survived the question");
    assert_eq!(session.state().name, "abierto.txt");
    assert_eq!(session.display_text(), "contenido editado\n");

    let _ = fs::remove_dir_all(root);
}

/// The language follows the document, and a document Grafita cannot colour is
/// still a document Grafita edits.
#[test]
fn the_open_document_reports_its_language_and_colours_its_lines() {
    let root = scratch("language");
    let code = root.join("modulo.rs");
    let unknown = root.join("notas.desconocido");
    fs::write(&code, b"let x = 42; // nota\n").expect("write");
    fs::write(&unknown, b"let x = 42; // nota\n").expect("write");

    let (session, _) = open_session(&code);
    assert_eq!(session.language(), grafita_core::Language::Rust);
    let (spans, _) = session.highlight_line(0, grafita_core::LineState::Normal);
    assert_eq!(spans.len(), 3, "keyword, number and comment");

    let (session, _) = open_session(&unknown);
    assert_eq!(session.language(), grafita_core::Language::Plain);
    let (spans, _) = session.highlight_line(0, grafita_core::LineState::Normal);
    assert!(spans.is_empty(), "plain text is coloured as nothing");
    assert!(session.state().active, "and is still perfectly editable");

    let _ = fs::remove_dir_all(root);
}

/// A brand-new document has nowhere to go until it is told, and saying so is an
/// event rather than a refusal: a document without a name is still a document.
#[test]
fn a_new_document_asks_where_it_goes_and_then_belongs_there() {
    let root = scratch("new-document");
    let destination = root.join("recien.txt");

    let mut session = DocumentSession::new(Limits::default());
    let outcome = session.new_document();

    assert_eq!(
        outcome.event,
        Some(Event::PushText {
            text: String::new(),
            caret: 0
        })
    );
    assert!(session.state().active, "a new document is open");
    assert!(!session.has_destination());

    let _ = session.apply_display_text("primera\nsegunda\n");
    assert!(session.state().dirty);

    // Saving without a file asks, and asking writes nothing.
    let outcome = session.save();
    assert_eq!(outcome.event, Some(Event::DestinationNeeded));
    assert!(outcome.job.is_none(), "nothing may be written yet");
    assert!(!destination.exists());

    // Told where it goes, it writes and binds itself there.
    let outcome = session.save_as(&destination);
    let _ = pump(&mut session, outcome.job.expect("a save-as job"));

    assert_eq!(
        fs::read(&destination).expect("read back"),
        b"primera\nsegunda\n"
    );
    assert!(session.has_destination());
    assert!(!session.state().dirty, "saving cleaned it");
    assert_eq!(session.state().name, "recien.txt");

    // And from here it is an ordinary saved document: a plain save works.
    let _ = session.apply_display_text("primera\nsegunda\ntercera\n");
    let outcome = session.save();
    assert!(outcome.job.is_some(), "it knows where it lives now");
    let _ = pump(&mut session, outcome.job.expect("a save job"));
    assert_eq!(
        fs::read(&destination).expect("read back"),
        b"primera\nsegunda\ntercera\n"
    );

    let _ = fs::remove_dir_all(root);
}

/// Saving over a file the user picked must not quietly widen how it is
/// protected.
#[test]
fn save_as_over_an_existing_file_keeps_its_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("save-as-existing");
    let destination = root.join("privado.txt");
    fs::write(&destination, b"antes\n").expect("write");
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).expect("chmod");

    let mut session = DocumentSession::new(Limits::default());
    let _ = session.new_document();
    let _ = session.apply_display_text("despues\n");
    let outcome = session.save_as(&destination);
    let _ = pump(&mut session, outcome.job.expect("a save-as job"));

    assert_eq!(fs::read(&destination).expect("read back"), b"despues\n");
    let mode = fs::metadata(&destination)
        .expect("stat")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "the file kept its own permissions");

    let _ = fs::remove_dir_all(root);
}

/// A destination that cannot be written leaves the document dirty and unbound,
/// with the refusal visible — never a document that thinks it was saved.
#[test]
fn a_refused_save_as_leaves_the_document_unbound() {
    let root = scratch("save-as-refused");
    let impossible = root.join("no-existe").join("archivo.txt");

    let mut session = DocumentSession::new(Limits::default());
    let _ = session.new_document();
    let _ = session.apply_display_text("contenido\n");

    let outcome = session.save_as(&impossible);
    let _ = pump(&mut session, outcome.job.expect("a save-as job"));

    assert!(!session.has_destination(), "it is still nameless");
    assert!(session.state().dirty, "and still unsaved");
    assert!(session.state().failure.is_some(), "and says so");

    let _ = fs::remove_dir_all(root);
}

/// The whole point of the editor: a keystroke that lands while a "save as" is
/// being written and synced is in the document and not in the file, so the
/// document must still be dirty when the answer arrives.
#[test]
fn editing_during_a_save_as_leaves_the_document_dirty() {
    let root = scratch("save-as-raced");
    let destination = root.join("carrera.txt");

    let mut session = DocumentSession::new(Limits::default());
    let _ = session.new_document();
    let _ = session.apply_display_text("primera\n");

    // The bytes leave here; the job is not answered yet.
    let job = session.save_as(&destination).job.expect("a save-as job");

    // The user keeps typing while the worker writes and syncs.
    let _ = session.apply_display_text("primera\nsegunda\n");

    let _ = pump(&mut session, job);

    assert_eq!(
        fs::read(&destination).expect("read back"),
        b"primera\n",
        "only the snapshotted bytes were written"
    );
    assert!(
        session.state().dirty,
        "the keystrokes typed during the write are not in the file"
    );
    assert!(session.has_destination(), "the file itself is adopted");

    let _ = fs::remove_dir_all(root);
}

/// And the close waiting on that write waits for the rest of the work rather
/// than taking it away with the tab.
#[test]
fn a_save_as_that_left_work_behind_does_not_close_the_document() {
    let root = scratch("save-as-raced-close");
    let destination = root.join("cierre.txt");

    let mut session = DocumentSession::new(Limits::default());
    let _ = session.new_document();
    let _ = session.apply_display_text("primera\n");
    let _ = session.request_close();

    let outcome = session.save_and_close();
    assert_eq!(outcome.event, Some(Event::DestinationNeeded));
    let job = session.save_as(&destination).job.expect("a save-as job");
    let _ = session.apply_display_text("primera\nsegunda\n");

    let event = pump(&mut session, job);

    assert_eq!(
        event, None,
        "closing would discard the unwritten keystrokes"
    );
    assert!(session.state().active);
    assert!(session.state().dirty);

    let _ = fs::remove_dir_all(root);
}

/// Cancelling the destination chooser must disarm the pending close, or an
/// ordinary save much later closes the document on its own.
#[test]
fn cancelling_the_destination_chooser_disarms_the_pending_close() {
    let root = scratch("save-as-cancelled");
    let path = root.join("mantener.txt");
    fs::write(&path, b"contenido\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.apply_display_text("contenido editado\n");
    let _ = session.request_close();
    // A document with a file saves straight away, so the armed state is reached
    // the way the host reaches it: the answer to the guarded question.
    let _ = session.save_and_close();
    let _ = session.cancel_save_as();

    // Much later, an ordinary save.
    let _ = session.apply_display_text("contenido editado otra vez\n");
    let job = session.save().job.expect("a save job");
    let event = pump(&mut session, job);

    assert_eq!(event, None, "an ordinary save must not close anything");
    assert!(session.state().active);
    assert!(!session.state().dirty);

    let _ = fs::remove_dir_all(root);
}

/// Two saves of the same document state are one write. Queueing the second
/// would snapshot the identity the first is about to replace, and its own
/// predecessor would come back as "another program changed this file".
#[test]
fn saving_twice_over_the_same_state_writes_once_and_raises_no_conflict() {
    let root = scratch("double-save");
    let path = root.join("doble.txt");
    fs::write(&path, b"antes\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.apply_display_text("despues\n");

    let first = session.save().job.expect("a save job");
    assert!(
        session.save().job.is_none(),
        "the same state is already with the worker"
    );

    let _ = pump(&mut session, first);
    assert!(!session.state().dirty);
    assert_eq!(session.state().conflict, None);
    assert!(session.state().failure.is_none());

    // And a clean document writes nothing at all.
    assert!(session.save().job.is_none());

    let _ = fs::remove_dir_all(root);
}

/// A classify answer is recognised by the question it belongs to, not by its
/// age: `classify` deliberately does not move the newest-question mark, so the
/// staleness filter must not be what decides its fate.
#[test]
fn a_classify_answer_survives_an_open_asked_for_after_it() {
    let root = scratch("classify-then-open");
    let asked = root.join("preguntado.txt");
    let opened = root.join("abierto.txt");
    fs::write(&asked, b"esto es texto\n").expect("write");
    fs::write(&opened, b"otra cosa\n").expect("write");

    let mut session = DocumentSession::new(Limits::default());
    let classify = session.classify(&asked).job.expect("a probe job");
    let open = session.open(&opened).job.expect("a probe job");

    let event = pump(&mut session, classify);
    assert_eq!(
        event,
        Some(Event::Classified {
            path: asked.canonicalize().expect("canonical"),
            editable: true
        }),
        "the question the host is waiting on must still be answered"
    );

    let _ = pump(&mut session, open);
    assert_eq!(session.state().name, "abierto.txt");

    let _ = fs::remove_dir_all(root);
}

/// Matches point into the buffer they were found in, so opening another
/// document must not leave them pointing into it.
#[test]
fn opening_another_document_clears_the_live_search() {
    let root = scratch("search-across-documents");
    let first = root.join("primero.txt");
    let second = root.join("segundo.txt");
    fs::write(&first, b"gato gato gato\n").expect("write");
    fs::write(&second, b"corto\n").expect("write");

    let (mut session, _) = open_session(&first);
    let _ = session.set_search("gato", grafita_core::search::Query::default());
    assert_eq!(session.state().search_matches, 3);

    let job = session.open(&second).job.expect("a probe job");
    let _ = pump(&mut session, job);

    assert_eq!(session.state().search_matches, 0);
    assert_eq!(session.state().search_index, None);

    // Replacing now must splice nothing: there is no selected occurrence.
    let _ = session.replace_current("perro");
    assert_eq!(session.display_text(), "corto\n");

    let _ = fs::remove_dir_all(root);
}

/// An action with more members than the undo bound is still one action: undoing
/// it must not stop half way through, in a state no user step produced.
#[test]
fn a_replace_all_larger_than_the_undo_bound_is_reversed_completely() {
    let root = scratch("replace-all-bound");
    let path = root.join("muchos.txt");
    let occurrences = 600;
    let original: String = (0..occurrences).map(|_| "x\n").collect();
    fs::write(&path, original.as_bytes()).expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.set_search("x", grafita_core::search::Query::default());
    let _ = session.replace_all("y");
    assert_eq!(
        session.display_text().matches('y').count(),
        occurrences,
        "every occurrence was replaced"
    );

    let _ = session.undo();
    assert_eq!(
        session.document().expect("document").text(),
        original,
        "one undo returns the document the user had"
    );

    let _ = fs::remove_dir_all(root);
}

/// After a replace-all nothing is selected, so "next" must find the first
/// occurrence rather than counting from an index that was never there.
#[test]
fn find_next_after_a_replace_all_selects_the_first_occurrence() {
    let root = scratch("find-after-replace-all");
    let path = root.join("siguiente.txt");
    fs::write(&path, b"a b a b a\n").expect("write");

    let (mut session, _) = open_session(&path);
    let _ = session.set_search("b", grafita_core::search::Query::default());
    // A replacement that still contains the pattern leaves occurrences behind
    // and no selection, which is the state this is about.
    let _ = session.replace_all("bb");
    assert_eq!(session.state().search_matches, 4);
    assert_eq!(session.state().search_index, None);

    let outcome = session.find_next();
    assert_eq!(
        session.state().search_index,
        Some(0),
        "the first occurrence must not be skipped"
    );
    assert_eq!(outcome.event, Some(Event::Select { start: 2, end: 3 }));

    // And from the first, backwards wraps to the last.
    let _ = session.find_previous();
    assert_eq!(session.state().search_index, Some(3));

    let _ = fs::remove_dir_all(root);
}

/// A "save as" onto a symlink writes the file the link names and leaves the
/// link in place, exactly as an ordinary save does.
#[test]
fn a_save_as_onto_a_symlink_writes_through_it() {
    let root = scratch("save-as-symlink");
    let real = root.join("real.txt");
    let link = root.join("enlace");
    fs::write(&real, b"antes\n").expect("write");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let mut session = DocumentSession::new(Limits::default());
    let _ = session.new_document();
    let _ = session.apply_display_text("despues\n");
    let job = session.save_as(&link).job.expect("a save-as job");
    let _ = pump(&mut session, job);

    assert!(
        fs::symlink_metadata(&link).expect("stat").is_symlink(),
        "the link is followed, never replaced"
    );
    assert_eq!(fs::read(&real).expect("read back"), b"despues\n");
    assert!(!session.state().dirty);

    let _ = fs::remove_dir_all(root);
}

/// A new document that nobody has typed into still has to be nameable.
///
/// The clean check exists so an unchanged *file* is not rewritten. A document
/// with no file has nothing to rewrite, and answering nothing to the save
/// shortcut left it with no way to acquire a name at all.
#[test]
fn an_untouched_new_document_can_still_be_given_a_destination() {
    let mut session = DocumentSession::new(Limits::default());
    let outcome = session.new_document();
    assert!(matches!(outcome.event, Some(Event::PushText { .. })));
    assert!(!session.state().dirty, "a new document starts clean");

    let outcome = session.save();
    assert!(
        matches!(outcome.event, Some(Event::DestinationNeeded)),
        "a document with no file asks where it goes, however clean it is"
    );
}
