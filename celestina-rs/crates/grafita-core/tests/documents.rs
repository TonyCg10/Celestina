//! End-to-end checks against real files: what opens, what refuses, and what a
//! save leaves on disk.

use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use celestina_core::{CancellationToken, Generation, GenerationClock};
use grafita_core::document::{SaveApplication, SaveIntent};
use grafita_core::open::{open, open_with, probe, Limits, OpenRefusal};
use grafita_core::save::{perform, Durability, SaveRefusal};
use grafita_core::{Document, Encoding, MultiByte, Position, SingleByte, Span};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(1);

fn scratch(label: &str) -> PathBuf {
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "grafita-documents-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn live() -> CancellationToken {
    CancellationToken::new()
}

fn first_generation() -> Generation {
    let mut clock = GenerationClock::default();
    clock.issue().expect("a first generation")
}

fn document_at(path: &Path) -> Document {
    let opened = open(path, first_generation(), Limits::default(), &live())
        .unwrap_or_else(|refusal| panic!("'{}' must open: {refusal}", path.display()));
    Document::from_opened(opened)
}

fn save_now(document: &mut Document) -> Durability {
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("an opened document has a target and encodable text");
    };
    let report = perform(&request, &live())
        .unwrap_or_else(|refusal| panic!("the save must succeed: {refusal}"));
    assert_eq!(document.apply_save(&report), SaveApplication::Clean);
    report.durability
}

fn entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .expect("read the scratch directory")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn every_textual_shape_opens_through_the_same_api() {
    let root = scratch("shapes");
    let cases: [(&str, &[u8], bool); 8] = [
        ("notas.txt", "una nota con tilde: acción\n".as_bytes(), true),
        (
            "main.rs",
            b"fn main() {\n    println!(\"hola\");\n}\n",
            true,
        ),
        ("config.json", b"{\n  \"puerto\": 8080\n}\n", true),
        ("ajustes.kdl", b"ventana ancho=800 {\n  borde 2\n}\n", true),
        (".gitconfig", b"[user]\n\tname = Toni\n", true),
        (
            "LICENCIA",
            b"Sin extension y perfectamente editable.\n",
            true,
        ),
        ("notas.bin", b"\x7fELF\x02\x01\x01\x00\x00\x00", false),
        ("imagen.txt", b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR", false),
    ];

    for (name, bytes, editable) in cases {
        let path = root.join(name);
        fs::write(&path, bytes).expect("write the fixture");

        let outcome = probe(&path, first_generation(), Limits::default(), &live())
            .unwrap_or_else(|refusal| panic!("{name} must probe: {refusal}"));
        assert_eq!(outcome.classification.is_editable(), editable, "{name}");
        assert!(outcome.complete, "{name}");

        let opened = open(&path, first_generation(), Limits::default(), &live());
        match (editable, opened) {
            (true, Ok(file)) => {
                assert_eq!(file.encoding, Encoding::Utf8, "{name}");
                assert_eq!(
                    file.text.as_bytes(),
                    bytes,
                    "{name} must decode to its own bytes"
                );
            }
            (false, Err(OpenRefusal::NotText { .. })) => {}
            (_, other) => panic!("{name}: unexpected outcome {other:?}"),
        }
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_untouched_open_and_save_is_byte_identical() {
    let root = scratch("identical");
    let cases: [(&str, Vec<u8>); 5] = [
        ("mixto", b"uno\r\ndos\rtres\ncuatro".to_vec()),
        ("sin-salto", b"una sola linea".to_vec()),
        ("vacio", Vec::new()),
        (
            "marca-utf8",
            Encoding::Utf8Bom
                .encode("con marca\r\nsegunda\r\n")
                .expect("a Unicode encoding carries every character"),
        ),
        (
            "ancho-le",
            Encoding::Utf16Le
                .encode("ancho\nsegunda\n")
                .expect("a Unicode encoding carries every character"),
        ),
    ];

    for (name, bytes) in cases {
        let path = root.join(name);
        fs::write(&path, &bytes).expect("write the fixture");

        let mut document = document_at(&path);
        assert!(!document.is_dirty(), "{name}");
        assert_eq!(
            document.to_bytes(),
            Ok(bytes.clone()),
            "{name} must re-encode exactly"
        );

        save_now(&mut document);
        assert_eq!(fs::read(&path).expect("read back"), bytes, "{name}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn marked_encodings_survive_an_edit() {
    let root = scratch("marked");
    let cases = [
        (Encoding::Utf8Bom, "primera\n"),
        (Encoding::Utf16Le, "primera\n"),
        (Encoding::Utf16Be, "primera\n"),
    ];

    for (encoding, text) in cases {
        let path = root.join(encoding.label().replace(' ', "-"));
        let bytes = encoding
            .encode(text)
            .expect("a Unicode encoding carries every character");
        fs::write(&path, bytes).expect("write the fixture");

        let mut document = document_at(&path);
        assert_eq!(document.encoding(), encoding);

        let end = document.buffer().end_position();
        document.insert(end, "añadida 🜲\n").expect("insert");
        save_now(&mut document);

        let written = fs::read(&path).expect("read back");
        assert!(
            written.starts_with(encoding.byte_order_mark()),
            "{encoding:?} must keep its mark"
        );
        assert_eq!(
            encoding.decode(&written),
            Ok("primera\nañadida 🜲\n".to_owned()),
            "{encoding:?}"
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bytes_that_cannot_be_mapped_back_are_never_offered_as_editable() {
    let root = scratch("unsupported");
    let raw = b"texto y luego \xff\xfe bytes crudos\n";
    let path = root.join("crudo");
    fs::write(&path, raw).expect("write the fixture");

    let outcome = probe(&path, first_generation(), Limits::default(), &live()).expect("probe");
    assert!(!outcome.classification.is_editable());

    let refusal = open(&path, first_generation(), Limits::default(), &live())
        .expect_err("must refuse to edit");
    assert!(matches!(refusal, OpenRefusal::UnsupportedEncoding { .. }));

    assert_eq!(fs::read(&path).expect("read back"), raw);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_file_past_the_ceiling_is_refused_before_it_is_read() {
    let root = scratch("ceiling");
    let path = root.join("grande.txt");
    fs::write(&path, b"mas largo que el limite").expect("write the fixture");

    let limits = Limits {
        max_bytes: 8,
        ..Limits::default()
    };
    let refusal = open(&path, first_generation(), limits, &live()).expect_err("must refuse");

    assert_eq!(refusal, OpenRefusal::TooLarge { size: 23, limit: 8 });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn editing_undo_redo_and_the_savepoint_move_together() {
    let root = scratch("history");
    let path = root.join("borrador.txt");
    fs::write(&path, b"uno\r\ndos\rtres\n").expect("write the fixture");

    let mut document = document_at(&path);
    assert!(!document.is_dirty());
    assert!(!document.can_undo());

    // Selection replacement across a CR boundary, then an insertion whose own
    // newline must adopt the document's dominant terminator.
    document
        .replace(
            Span::ordered(Position::new(1, 0), Position::new(2, 4)),
            "DOS",
            Position::new(1, 0),
        )
        .expect("replace the selection");
    assert_eq!(document.text(), "uno\r\nDOS\n");
    assert!(document.is_dirty());

    let end = document.buffer().end_position();
    document.insert(end, "cuatro\ncinco").expect("insert");
    assert_eq!(document.text(), "uno\r\nDOS\ncuatro\ncinco");

    save_now(&mut document);
    assert!(!document.is_dirty());
    assert_eq!(
        fs::read(&path).expect("read back"),
        b"uno\r\nDOS\ncuatro\ncinco"
    );

    document.undo().expect("undo").expect("a change to undo");
    assert_eq!(document.text(), "uno\r\nDOS\n");
    assert!(document.is_dirty());

    document.undo().expect("undo").expect("a second change");
    assert_eq!(document.text(), "uno\r\ndos\rtres\n");
    assert!(document.is_dirty());
    assert!(document.undo().expect("undo").is_none());

    document.redo().expect("redo").expect("a change to redo");
    document.redo().expect("redo").expect("a second change");
    assert_eq!(document.text(), "uno\r\nDOS\ncuatro\ncinco");
    assert!(
        !document.is_dirty(),
        "redoing back to the savepoint is clean"
    );

    let _ = fs::remove_dir_all(root);
}

/// The whole reason a text widget is not allowed to own the document: it works
/// in line feeds, and a round trip through one must not rewrite a CRLF file.
///
/// This drives the exact path a Qt `TextArea` drives — read the projection,
/// edit that string the way a widget would, hand the whole thing back — and
/// then checks the bytes on disk.
#[test]
fn editing_through_the_line_feed_projection_keeps_the_original_terminators() {
    let root = scratch("projection");
    let path = root.join("windows.txt");
    fs::write(&path, b"primera\r\nsegunda\r\ntercera\r\n").expect("write the fixture");

    let mut document = document_at(&path);
    assert_eq!(document.display_text(), "primera\nsegunda\ntercera\n");

    // A widget types " EDITADA" at the end of the second line.
    let typed = document
        .display_text()
        .replace("segunda", "segunda EDITADA");
    document
        .apply_display_text(&typed)
        .expect("a valid edit")
        .expect("the text changed");

    // And then presses Return at the very end, which the widget reports as a
    // bare line feed in its own text.
    let with_break = format!("{}cuarta\n", document.display_text());
    document
        .apply_display_text(&with_break)
        .expect("a valid edit")
        .expect("the text changed");

    save_now(&mut document);

    assert_eq!(
        fs::read(&path).expect("read back"),
        b"primera\r\nsegunda EDITADA\r\ntercera\r\ncuarta\r\n",
        "untouched lines keep their terminators and the typed one adopts CRLF"
    );

    // Handing the document its own projection back is not an edit.
    let projection = document.display_text().to_owned();
    assert!(document
        .apply_display_text(&projection)
        .expect("valid")
        .is_none());
    assert!(!document.is_dirty());

    // Undo walks back through the same edits and reports where the caret goes.
    document.undo().expect("undo").expect("a change");
    document.undo().expect("undo").expect("a second change");
    assert_eq!(document.display_text(), "primera\nsegunda\ntercera\n");
    assert!(document.is_dirty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_invalid_position_changes_neither_buffer_nor_revision() {
    let root = scratch("invalid-position");
    let path = root.join("corto.txt");
    fs::write(&path, "café\n".as_bytes()).expect("write the fixture");

    let mut document = document_at(&path);
    let revision = document.revision();

    document
        .insert(Position::new(0, 4), "!")
        .expect_err("splitting a character must refuse");
    document
        .insert(Position::new(9, 0), "!")
        .expect_err("a missing line must refuse");

    assert_eq!(document.text(), "café\n");
    assert_eq!(document.revision(), revision);
    assert!(!document.is_dirty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn saving_through_a_symlink_writes_the_target_and_keeps_the_link() {
    let root = scratch("symlink");
    let file = root.join("real.txt");
    let link = root.join("enlace");
    fs::write(&file, b"original\n").expect("write the fixture");
    std::os::unix::fs::symlink(&file, &link).expect("symlink");

    let mut document = document_at(&link);
    let end = document.buffer().end_position();
    document.insert(end, "editado\n").expect("insert");
    save_now(&mut document);

    assert!(fs::symlink_metadata(&link).expect("stat").is_symlink());
    assert_eq!(fs::read(&file).expect("read back"), b"original\neditado\n");
    assert_eq!(
        entries(&root),
        vec!["enlace".to_owned(), "real.txt".to_owned()]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_save_reproduces_the_original_permissions() {
    let root = scratch("permissions");
    let path = root.join("privado.txt");
    fs::write(&path, b"secreto\n").expect("write the fixture");
    fs::set_permissions(&path, Permissions::from_mode(0o600)).expect("mode");

    let mut document = document_at(&path);
    let end = document.buffer().end_position();
    document.insert(end, "mas\n").expect("insert");
    save_now(&mut document);

    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o7777;
    assert_eq!(mode, 0o600);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_file_changed_underneath_refuses_and_keeps_the_other_version() {
    let root = scratch("changed");
    let path = root.join("compartido.txt");
    fs::write(&path, b"version del editor\n").expect("write the fixture");

    let mut document = document_at(&path);
    document
        .insert(document.buffer().end_position(), "linea del editor\n")
        .expect("insert");

    fs::write(&path, b"otro proceso escribio esto\n").expect("external write");

    let refusal = perform(
        &document
            .save_request()
            .ready()
            .expect("an opened document has a target"),
        &live(),
    )
    .expect_err("must refuse");
    assert!(matches!(refusal, SaveRefusal::ChangedUnderneath { .. }));
    assert_eq!(
        fs::read(&path).expect("read back"),
        b"otro proceso escribio esto\n",
        "the other process's bytes must survive"
    );
    assert_eq!(entries(&root), vec!["compartido.txt".to_owned()]);

    assert_eq!(
        document.apply_save_refusal(&refusal),
        Some(&grafita_core::Conflict::ChangedUnderneath)
    );
    assert!(document.is_dirty(), "the edit must still be in hand");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_repointed_symlink_refuses_instead_of_writing_the_new_target() {
    let root = scratch("retarget");
    let first = root.join("primero.txt");
    let second = root.join("segundo.txt");
    let link = root.join("enlace");
    fs::write(&first, b"primero\n").expect("write");
    fs::write(&second, b"segundo\n").expect("write");
    std::os::unix::fs::symlink(&first, &link).expect("symlink");

    let mut document = document_at(&link);
    document
        .insert(document.buffer().end_position(), "editado\n")
        .expect("insert");

    fs::remove_file(&link).expect("remove the link");
    std::os::unix::fs::symlink(&second, &link).expect("repoint the link");

    let refusal = perform(
        &document
            .save_request()
            .ready()
            .expect("an opened document has a target"),
        &live(),
    )
    .expect_err("must refuse");
    assert!(matches!(refusal, SaveRefusal::Retargeted { .. }));
    assert_eq!(fs::read(&first).expect("read").as_slice(), b"primero\n");
    assert_eq!(fs::read(&second).expect("read").as_slice(), b"segundo\n");

    assert!(matches!(
        document.apply_save_refusal(&refusal),
        Some(&grafita_core::Conflict::Retargeted { .. })
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_deleted_target_refuses_rather_than_recreating_the_file() {
    let root = scratch("missing");
    let path = root.join("efimero.txt");
    fs::write(&path, b"aqui estaba\n").expect("write the fixture");

    let mut document = document_at(&path);
    document
        .insert(document.buffer().end_position(), "editado\n")
        .expect("insert");
    fs::remove_file(&path).expect("delete it externally");

    let refusal = perform(
        &document
            .save_request()
            .ready()
            .expect("an opened document has a target"),
        &live(),
    )
    .expect_err("must refuse");
    assert!(matches!(refusal, SaveRefusal::TargetMissing { .. }));
    assert!(entries(&root).is_empty(), "no file and no temporary");

    assert_eq!(
        document.apply_save_refusal(&refusal),
        Some(&grafita_core::Conflict::Missing)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_interrupted_save_leaves_the_original_and_no_temporary() {
    let root = scratch("interrupted");
    let path = root.join("interrumpido.txt");
    fs::write(&path, b"contenido original\n").expect("write the fixture");

    let mut document = document_at(&path);
    document
        .insert(document.buffer().end_position(), "nunca publicado\n")
        .expect("insert");

    // Cancellation lands after the temporary is written and its metadata
    // reproduced, which is the last moment before the rename publishes it.
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let refusal = perform(
        &document.save_request().ready().expect("a target"),
        &cancellation,
    )
    .expect_err("must refuse");

    assert_eq!(refusal, SaveRefusal::Cancelled);
    assert_eq!(fs::read(&path).expect("read back"), b"contenido original\n");
    assert_eq!(entries(&root), vec!["interrumpido.txt".to_owned()]);
    assert_eq!(document.apply_save_refusal(&refusal), None);
    assert!(document.is_dirty());

    // The same document still saves once nothing interrupts it.
    save_now(&mut document);
    assert_eq!(
        fs::read(&path).expect("read back"),
        b"contenido original\nnunca publicado\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_save_that_cannot_create_its_temporary_leaves_the_original_intact() {
    let root = scratch("readonly-parent");
    let path = root.join("bloqueado.txt");
    fs::write(&path, b"contenido original\n").expect("write the fixture");

    let mut document = document_at(&path);
    document
        .insert(document.buffer().end_position(), "no cabe\n")
        .expect("insert");

    fs::set_permissions(&root, Permissions::from_mode(0o500)).expect("seal the directory");
    let refusal = perform(
        &document
            .save_request()
            .ready()
            .expect("an opened document has a target"),
        &live(),
    )
    .expect_err("must refuse");
    fs::set_permissions(&root, Permissions::from_mode(0o700)).expect("unseal the directory");

    assert!(matches!(refusal, SaveRefusal::Io { .. }), "{refusal:?}");
    assert_eq!(fs::read(&path).expect("read back"), b"contenido original\n");
    assert_eq!(entries(&root), vec!["bloqueado.txt".to_owned()]);
    assert!(document.is_dirty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_save_report_older_than_the_document_does_not_clear_the_newer_edit() {
    let root = scratch("stale-save");
    let path = root.join("rapido.txt");
    fs::write(&path, b"base\n").expect("write the fixture");

    let mut document = document_at(&path);
    document
        .insert(document.buffer().end_position(), "primera\n")
        .expect("insert");
    let request = document.save_request().ready().expect("a target");

    // The user keeps typing while the worker is still writing.
    document
        .insert(document.buffer().end_position(), "segunda\n")
        .expect("insert");

    let report = perform(&request, &live()).expect("the worker's write succeeds");
    assert_eq!(document.apply_save(&report), SaveApplication::StillDirty);
    assert!(document.is_dirty(), "the newer keystroke is still unsaved");
    assert_eq!(fs::read(&path).expect("read back"), b"base\nprimera\n");

    // Adopting the written identity is what lets the next save proceed instead
    // of mistaking this document's own write for someone else's.
    save_now(&mut document);
    assert_eq!(
        fs::read(&path).expect("read back"),
        b"base\nprimera\nsegunda\n"
    );

    let _ = fs::remove_dir_all(root);
}

// ── Find and replace ─────────────────────────────────────────────────────────

/// Replace-all is dozens of splices and one thing the user did, so one undo
/// must reverse all of it — and reverse it to the exact original bytes,
/// terminators included.
#[test]
fn replace_all_is_a_single_undo_step_that_restores_the_original_bytes() {
    let root = scratch("replace-all-undo");
    let path = root.join("mezclado.txt");
    // Deliberately mixed terminators: the file must come back exactly as it
    // went in, not normalised to whatever the last edit used.
    let original = b"uno gato dos\r\ngato tres\rcuatro gato\n";
    fs::write(&path, original).expect("write");
    let mut document = document_at(&path);

    let replaced = document
        .replace_all("gato", "perro", grafita_core::Query::default())
        .expect("replace all");

    assert_eq!(replaced, 3);
    assert_eq!(
        document.text(),
        "uno perro dos\r\nperro tres\rcuatro perro\n"
    );
    assert!(document.is_dirty());

    // One undo, not three.
    document.undo().expect("undo").expect("an undone action");
    assert_eq!(
        document.text().as_bytes(),
        original,
        "a single undo must restore the original bytes exactly"
    );
    assert!(!document.can_undo(), "the action left no leftovers behind");
    assert!(!document.is_dirty(), "back at the savepoint");

    // And one redo brings the whole action back.
    document.redo().expect("redo").expect("a redone action");
    assert_eq!(
        document.text(),
        "uno perro dos\r\nperro tres\rcuatro perro\n"
    );

    let _ = fs::remove_dir_all(root);
}

/// A replacement is an ordinary edit, so it must survive the round trip to disk
/// with the file's own terminators intact.
#[test]
fn a_replacement_saves_without_rewriting_terminators() {
    let root = scratch("replace-save");
    let path = root.join("crlf.txt");
    fs::write(&path, b"alfa\r\nbeta\r\ngamma\r\n").expect("write");
    let mut document = document_at(&path);

    document
        .replace_all("beta", "BETA", grafita_core::Query::default())
        .expect("replace all");
    save_now(&mut document);

    assert_eq!(
        fs::read(&path).expect("read back"),
        b"alfa\r\nBETA\r\ngamma\r\n"
    );

    let _ = fs::remove_dir_all(root);
}

/// Replacing with something longer or shorter must not shift the matches that
/// have not been applied yet — the reason the splices run backwards.
#[test]
fn replacements_of_a_different_length_do_not_disturb_each_other() {
    let root = scratch("replace-lengths");
    let path = root.join("longitudes.txt");
    fs::write(&path, b"x a x a x\n").expect("write");
    let mut document = document_at(&path);

    document
        .replace_all("x", "LARGO", grafita_core::Query::default())
        .expect("replace all");
    assert_eq!(document.text(), "LARGO a LARGO a LARGO\n");

    document
        .replace_all("LARGO", "y", grafita_core::Query::default())
        .expect("replace all");
    assert_eq!(document.text(), "y a y a y\n");

    let _ = fs::remove_dir_all(root);
}

/// Nothing found is not an edit: it must not dirty the document or leave an
/// empty action on the undo stack.
#[test]
fn replacing_a_pattern_that_is_absent_changes_nothing() {
    let root = scratch("replace-absent");
    let path = root.join("intacto.txt");
    fs::write(&path, b"sin coincidencias\n").expect("write");
    let mut document = document_at(&path);

    let replaced = document
        .replace_all("ausente", "x", grafita_core::Query::default())
        .expect("replace all");

    assert_eq!(replaced, 0);
    assert!(!document.is_dirty());
    assert!(!document.can_undo());

    let _ = fs::remove_dir_all(root);
}

/// Replacing every character of the document, and replacing a match that
/// contains multi-byte characters, both have to land on real boundaries.
#[test]
fn replacement_handles_multibyte_text_and_a_whole_document_match() {
    let root = scratch("replace-multibyte");
    let path = root.join("acentos.txt");
    fs::write(&path, "año añejo\n".as_bytes()).expect("write");
    let mut document = document_at(&path);

    document
        .replace_all("añ", "AÑ", grafita_core::Query::default())
        .expect("replace all");
    assert_eq!(document.text(), "AÑo AÑejo\n");

    document.undo().expect("undo").expect("an undone action");
    assert_eq!(document.text(), "año añejo\n");

    let _ = fs::remove_dir_all(root);
}

/// Go-to-line counts from 1 and clamps rather than refusing, and the
/// indentation report comes from the file rather than from a default.
#[test]
fn go_to_line_and_indentation_read_the_real_document() {
    let root = scratch("navigation");
    let path = root.join("codigo.rs");
    fs::write(&path, b"fn main() {\n    let a = 1;\n    let b = 2;\n}\n").expect("write");
    let document = document_at(&path);

    assert_eq!(document.position_at_line(1), Position::new(0, 0));
    assert_eq!(document.position_at_line(3), Position::new(2, 0));
    // Past the end clamps to the last line instead of refusing to move.
    assert_eq!(
        document.position_at_line(9_999),
        Position::new(document.line_count() - 1, 0)
    );
    // Line 0 does not exist; 1 is the first.
    assert_eq!(document.position_at_line(0), Position::new(0, 0));

    assert_eq!(
        document.indentation(),
        grafita_core::Indentation::Spaces { width: 4 }
    );

    let _ = fs::remove_dir_all(root);
}

/// A tab-indented file must say so, or the editor would insert spaces into it.
#[test]
fn a_tab_indented_file_reports_tabs() {
    let root = scratch("indent-tabs");
    let path = root.join("Makefile");
    fs::write(&path, b"all:\n\tcargo build\n\tcargo test\n").expect("write");
    let document = document_at(&path);

    assert_eq!(document.indentation(), grafita_core::Indentation::Tabs);
    assert_eq!(document.indentation().unit(4), "\t");

    let _ = fs::remove_dir_all(root);
}

/// A file nothing in its bytes can identify still opens once the author names
/// its encoding, and saving it untouched reproduces it exactly.
#[test]
fn a_named_encoding_opens_what_the_bytes_cannot_prove() {
    let root = scratch("named");
    let latin = Encoding::SingleByte(SingleByte::Iso8859_1);
    let bytes = latin
        .encode("façade\nnaïve\n")
        .expect("latin-1 carries these");
    let path = root.join("note");
    fs::write(&path, &bytes).expect("write the fixture");

    // Left to itself the file is refused: 0xE7 is not UTF-8 and nothing says
    // which single-byte encoding it is.
    assert!(matches!(
        open(&path, first_generation(), Limits::default(), &live()),
        Err(OpenRefusal::UnsupportedEncoding { .. })
    ));

    let opened = open_with(&path, latin, first_generation(), Limits::default(), &live())
        .expect("the named encoding reads it");
    assert_eq!(opened.encoding, latin);
    assert_eq!(opened.text, "façade\nnaïve\n");

    let mut document = Document::from_opened(opened);
    assert!(!document.is_dirty());
    assert_eq!(document.to_bytes(), Ok(bytes.clone()));
    save_now(&mut document);
    assert_eq!(fs::read(&path).expect("read back"), bytes);

    let _ = fs::remove_dir_all(root);
}

/// Naming an encoding is a choice, not an override: one that reads the file but
/// would not write it back unchanged is refused before it can be edited.
#[test]
fn a_named_encoding_that_would_not_write_the_file_back_is_refused() {
    let root = scratch("named-refused");

    // ISO-8859-7 assigns no character to 0xAE, so it cannot even read this.
    let path = root.join("greek");
    fs::write(&path, b"alfa \xAE beta\n").expect("write the fixture");
    assert!(matches!(
        open_with(
            &path,
            Encoding::SingleByte(SingleByte::Iso8859_7),
            first_generation(),
            Limits::default(),
            &live()
        ),
        Err(OpenRefusal::UnsupportedEncoding { .. })
    ));

    // An odd number of bytes cannot be UTF-16 whatever the author names.
    let odd = root.join("odd");
    fs::write(&odd, b"abc").expect("write the fixture");
    assert!(matches!(
        open_with(
            &odd,
            Encoding::Utf16LeBare,
            first_generation(),
            Limits::default(),
            &live()
        ),
        Err(OpenRefusal::UnsupportedEncoding { .. })
    ));

    let _ = fs::remove_dir_all(root);
}

/// Unmarked UTF-16 is the case the probe cannot answer at all: its NUL bytes
/// make it binary, and only the author can say which byte order it is.
#[test]
fn unmarked_wide_text_opens_only_when_it_is_named() {
    let root = scratch("wide");
    for (label, encoding) in [
        ("le", Encoding::Utf16LeBare),
        ("be", Encoding::Utf16BeBare),
        ("32le", Encoding::Utf32LeBare),
        ("32be", Encoding::Utf32BeBare),
    ] {
        let bytes = encoding
            .encode("wide text\n")
            .expect("Unicode carries this");
        let path = root.join(label);
        fs::write(&path, &bytes).expect("write the fixture");

        assert!(
            matches!(
                open(&path, first_generation(), Limits::default(), &live()),
                Err(OpenRefusal::NotText { .. })
            ),
            "{label} must look like binary until it is named"
        );

        let opened = open_with(
            &path,
            encoding,
            first_generation(),
            Limits::default(),
            &live(),
        )
        .unwrap_or_else(|refusal| panic!("{label}: {refusal}"));
        assert_eq!(opened.text, "wide text\n", "{label}");
        assert_eq!(
            Document::from_opened(opened).to_bytes(),
            Ok(bytes),
            "{label} must re-encode exactly"
        );
    }

    let _ = fs::remove_dir_all(root);
}

/// A multi-byte file is the case a table alone cannot make safe, so opening it
/// is the check: decoded, re-encoded and compared with the bytes on disk.
#[test]
fn a_multi_byte_file_opens_named_and_saves_back_identically() {
    let root = scratch("multibyte");
    let shift = Encoding::MultiByte(MultiByte::ShiftJis);
    let bytes = shift
        .encode("私 wa\ntwo\n")
        .expect("Shift-JIS carries these");
    let path = root.join("nota");
    fs::write(&path, &bytes).expect("write the fixture");

    // Those bytes are not UTF-8, and nothing in them says which encoding they
    // are, so the file is refused until the author names one.
    assert!(matches!(
        open(&path, first_generation(), Limits::default(), &live()),
        Err(OpenRefusal::UnsupportedEncoding { .. })
    ));

    let opened = open_with(&path, shift, first_generation(), Limits::default(), &live())
        .expect("the named encoding reads it");
    assert_eq!(opened.text, "私 wa\ntwo\n");

    let mut document = Document::from_opened(opened);
    assert_eq!(document.to_bytes(), Ok(bytes.clone()));
    save_now(&mut document);
    assert_eq!(fs::read(&path).expect("read back"), bytes);

    // The limit of what naming an encoding can promise. These same bytes are
    // also valid GBK, where they spell a different character, and GBK writes
    // them back unchanged — so the byte check cannot catch it and does not
    // pretend to. What the contract guarantees is that no byte is lost, not
    // that the author picked the language the file was written in.
    let gbk = Encoding::MultiByte(MultiByte::Gbk);
    let other = open_with(&path, gbk, first_generation(), Limits::default(), &live())
        .expect("these bytes are valid GBK as well");
    assert_ne!(other.text, "私 wa\ntwo\n");
    assert_eq!(Document::from_opened(other).to_bytes(), Ok(bytes.clone()));

    let _ = fs::remove_dir_all(root);
}

// ─── The spliced projection ──────────────────────────────────────────────────
// The document keeps its display projection by splicing the edited lines
// rather than rebuilding the whole text per keystroke. The full rebuild is
// the oracle: after every kind of mutation — edit, undo, redo, reload — the
// two must be byte-identical, because `apply_display_text` recognising an
// echo depends on it.

#[test]
fn the_spliced_projection_matches_a_full_rebuild_through_edits_and_history() {
    let root = scratch("spliced-projection");
    let path = root.join("mixto.txt");
    fs::write(&path, b"uno\r\nd\xc3\xb3s\rtres\n").expect("write");
    let mut document = document_at(&path);

    let checks = |document: &Document| {
        assert_eq!(
            document.display_text(),
            grafita_core::display::project(document.buffer()),
            "the spliced projection diverged from the rebuilt one"
        );
    };
    checks(&document);

    // A multi-line replacement in the middle, an insert at the end, and a
    // deletion back across a terminator boundary.
    document
        .replace(
            Span::ordered(Position::new(0, 3), Position::new(1, 1)),
            " y\nmedio ",
            Position::new(0, 3),
        )
        .expect("edit");
    checks(&document);
    let end = document.buffer().end_position();
    document
        .replace(Span::empty(end), "\ncola", end)
        .expect("append");
    checks(&document);
    document
        .replace(
            Span::ordered(Position::new(0, 0), Position::new(1, 0)),
            "",
            Position::new(0, 0),
        )
        .expect("delete");
    checks(&document);

    while document.can_undo() {
        document.undo().expect("undo");
        checks(&document);
    }
    while document.can_redo() {
        document.redo().expect("redo");
        checks(&document);
    }
}
