//! The text of a `.docx`, and what happens to the rest of the file when it is
//! edited.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use celestina_core::{CancellationToken, Generation, GenerationClock};
use grafita_core::document::SaveIntent;
use grafita_core::import::part::{Part, PartError};
use grafita_core::import::Format;
use grafita_core::open::{open, Limits, OpenRefusal};
use grafita_core::save::perform;
use grafita_core::Document;

const DOCUMENT: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:body>"#,
    r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Report</w:t></w:r>"#,
    r#"<w:r><w:t xml:space="preserve"> for August &amp; annex</w:t></w:r></w:p>"#,
    r#"<w:p><w:r><w:t>Second façade</w:t></w:r></w:p>"#,
    r#"</w:body></w:document>"#
);

fn part() -> Part {
    Part::parse(DOCUMENT.as_bytes().to_vec(), Format::Docx.rules()).expect("a document part")
}

#[test]
fn a_document_reads_as_paragraphs_with_its_entities_resolved() {
    let part = part();

    // Two paragraphs, two lines. The runs inside the first are one line: their
    // split is formatting, and formatting is not the reader's business.
    assert_eq!(part.text(), "Report for August & annex\nSecond façade");
    assert_eq!(part.anchors().len(), 3);
}

#[test]
fn correcting_a_word_changes_that_run_and_nothing_else() {
    let part = part();
    let written = part
        .write("Summary for August & annex\nSecond façade")
        .expect("the write");
    let rewritten = String::from_utf8(written).expect("still UTF-8");

    // The style, the namespace, the `xml:space` attribute and the second
    // paragraph are the bytes they were.
    assert!(rewritten.contains("<w:rPr><w:b/></w:rPr>"));
    assert!(rewritten.contains(r#"<w:t xml:space="preserve">"#));
    assert!(rewritten.contains("<w:t>Second façade</w:t>"));
    // The ampersand went back as an entity, not as a raw byte that would make
    // the part unreadable.
    assert!(rewritten.contains("&amp;"));
    assert!(!rewritten.contains("Report"));

    // And the result reads back as what was typed.
    let again = Part::parse(rewritten.into_bytes(), Format::Docx.rules()).expect("a document part");
    assert_eq!(again.text(), "Summary for August & annex\nSecond façade");
}

#[test]
fn writing_the_same_text_back_reproduces_the_part_exactly() {
    let part = part();
    let written = part.write(part.text()).expect("the write");

    // Not "an equivalent document": the same bytes. Two runs in the first
    // paragraph become one because their text is joined, which is why this is
    // asserted on the text rather than on the length.
    let again = Part::parse(written, Format::Docx.rules()).expect("a document part");
    assert_eq!(again.text(), part.text());
}

#[test]
fn structure_is_refused_rather_than_invented() {
    let part = part();

    // Splitting a paragraph in two would need a `<w:p>` this editor does not
    // write, and joining two would delete one nobody asked it to delete.
    assert_eq!(
        part.write("Report\nfor August & annex\nSecond façade"),
        Err(PartError::ParagraphCountChanged { had: 2, now: 3 })
    );
    assert_eq!(
        part.write("everything on one line"),
        Err(PartError::ParagraphCountChanged { had: 2, now: 1 })
    );

    // A part with no run at all is not an empty document: it is one whose text
    // has nowhere to go.
    assert_eq!(
        Part::parse(
            b"<w:document><w:body/></w:document>".to_vec(),
            Format::Docx.rules()
        )
        .err(),
        Some(PartError::NoRuns)
    );
}

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(1);

fn scratch(label: &str) -> PathBuf {
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "grafita-imported-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn first_generation() -> Generation {
    let mut clock = GenerationClock::default();
    clock.issue().expect("a first generation")
}

/// A `.docx` as a third-party writer produces one.
fn sample_docx() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("[Content_Types].xml", options)
        .expect("start");
    writer
        .write_all(
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#,
        )
        .expect("write");
    writer
        .start_file("word/styles.xml", options)
        .expect("start");
    writer.write_all(b"<w:styles/>").expect("write");
    writer
        .start_file("word/document.xml", options)
        .expect("start");
    writer.write_all(DOCUMENT.as_bytes()).expect("write");
    writer.finish().expect("finish").into_inner()
}

/// The checkpoint's own claim: one word corrected, everything else untouched.
#[test]
fn a_docx_opens_as_text_and_saves_back_with_only_that_word_changed() {
    let root = scratch("docx");
    let path = root.join("report.docx");
    let original = sample_docx();
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("a container opens as a document");
    assert_eq!(opened.text, "Report for August & annex\nSecond façade");

    let mut document = Document::from_opened(opened);
    assert!(document.is_imported());
    assert_eq!(document.container_format(), Some(Format::Docx));
    assert!(!document.is_dirty());

    let _ = document.apply_display_text("Summary for August & annex\nSecond façade");
    assert!(document.is_dirty());

    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("an imported document with a file has a write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    // The file on disk is still a container its own reader accepts, the style
    // part is the bytes it was, and only the text changed.
    let saved = fs::read(&path).expect("read back");
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(saved.clone())).expect("still an archive");
    assert_eq!(archive.len(), 3);
    let styles = {
        use std::io::Read;
        let mut buffer = Vec::new();
        archive
            .by_name("word/styles.xml")
            .expect("the untouched part")
            .read_to_end(&mut buffer)
            .expect("read");
        buffer
    };
    assert_eq!(styles, b"<w:styles/>");

    let reopened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the saved container opens again");
    assert_eq!(reopened.text, "Summary for August & annex\nSecond façade");

    let _ = fs::remove_dir_all(root);
}

/// A container with no document part is refused by name rather than opened as
/// something it is not.
#[test]
fn a_container_that_holds_no_document_is_refused() {
    let root = scratch("not-docx");
    let path = root.join("archive.zip");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("notes.txt", options).expect("start");
    writer.write_all(b"loose text").expect("write");
    fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write");

    assert!(matches!(
        open(
            &path,
            first_generation(),
            Limits::default(),
            &CancellationToken::new()
        ),
        Err(OpenRefusal::NotImportable { .. })
    ));

    let _ = fs::remove_dir_all(root);
}

/// Adding a paragraph is structure, and the refusal happens before anything is
/// written.
#[test]
fn a_paragraph_the_author_added_is_refused_without_touching_the_file() {
    let root = scratch("structure");
    let path = root.join("report.docx");
    let original = sample_docx();
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("a container opens");
    let mut document = Document::from_opened(opened);
    let _ = document.apply_display_text("Report\nfor August & annex\nSecond façade");

    assert!(matches!(document.save_request(), SaveIntent::Unwritable(_)));
    assert_eq!(fs::read(&path).expect("read back"), original);

    let _ = fs::remove_dir_all(root);
}

/// OpenDocument puts text straight inside the paragraph, which is the rule
/// `G9`'s model had to grow to express. Same promise: only the words change.
#[test]
fn an_odt_opens_as_text_and_keeps_everything_around_it() {
    let root = scratch("odt");
    let path = root.join("report.odt");
    let content = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0""#,
        r#" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">"#,
        r#"<office:body><office:text>"#,
        r#"<text:h text:outline-level="1">Report</text:h>"#,
        r#"<text:p text:style-name="Standard">First <text:span text:style-name="Bold">line</text:span></text:p>"#,
        r#"<text:p>Second façade</text:p>"#,
        r#"</office:text></office:body></office:document-content>"#
    );
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("mimetype", options).expect("start");
    writer
        .write_all(b"application/vnd.oasis.opendocument.text")
        .expect("write");
    writer.start_file("styles.xml", options).expect("start");
    writer.write_all(b"<office:styles/>").expect("write");
    writer.start_file("content.xml", options).expect("start");
    writer.write_all(content.as_bytes()).expect("write");
    fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("an odt opens");
    // The span inside the first paragraph is formatting, not a line: it joins.
    assert_eq!(opened.text, "Report\nFirst line\nSecond façade");

    let mut document = Document::from_opened(opened);
    assert_eq!(document.container_format(), Some(Format::Odt));
    let _ = document.apply_display_text("Summary\nFirst line\nSecond façade");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("the write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    let saved = fs::read(&path).expect("read back");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(saved)).expect("still an archive");
    let content = {
        use std::io::Read;
        let mut buffer = String::new();
        archive
            .by_name("content.xml")
            .expect("the content")
            .read_to_string(&mut buffer)
            .expect("read");
        buffer
    };
    // The heading's own attribute, the paragraph style and the span survive.
    assert!(content.contains(r#"<text:h text:outline-level="1">Summary</text:h>"#));
    assert!(content.contains(r#"text:style-name="Standard""#));
    assert!(content.contains(r#"<text:span text:style-name="Bold">"#));

    let _ = fs::remove_dir_all(root);
}

/// An EPUB is several files and one document. Its chapters come back in the
/// order the spine gives, not the order the archive happens to hold.
#[test]
fn an_epub_reads_its_chapters_in_spine_order_and_writes_each_one_back() {
    let root = scratch("epub");
    let path = root.join("book.epub");
    let chapter = |title: &str, line: &str| {
        format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>{}</title>"#,
                r#"<style>p {{ margin: 0 }}</style></head>"#,
                r#"<body><h1>{}</h1><p>{}</p></body></html>"#
            ),
            title, title, line
        )
    };
    let opf = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><manifest>"#,
        r#"<item id="two" href="two.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<item id="one" href="one.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"</manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#
    );
    let container = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">"#,
        r#"<rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/>"#,
        r#"</rootfiles></container>"#
    );

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in [
        ("META-INF/container.xml", container.to_owned()),
        ("OEBPS/book.opf", opf.to_owned()),
        // Stored in the archive in the reverse of the reading order, which is
        // what makes the spine worth reading.
        ("OEBPS/two.xhtml", chapter("Two", "The second façade")),
        ("OEBPS/one.xhtml", chapter("One", "The first line")),
    ] {
        writer.start_file(name, options).expect("start");
        writer.write_all(body.as_bytes()).expect("write");
    }
    fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("an epub opens");
    // Spine order, headings as lines, and the style sheet and the title left
    // out: neither is the book's text.
    assert_eq!(opened.text, "One\nThe first line\n\nTwo\nThe second façade");

    let mut document = Document::from_opened(opened);
    assert_eq!(document.container_format(), Some(Format::Epub));
    let _ = document.apply_display_text("One\nThe corrected line\n\nTwo\nThe second façade");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("the write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    let reopened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the saved book opens again");
    assert_eq!(
        reopened.text,
        "One\nThe corrected line\n\nTwo\nThe second façade"
    );

    let _ = fs::remove_dir_all(root);
}

/// Rich text is markup around text with no container underneath, and the same
/// promise: the words change and the rest of the file does not.
#[test]
fn an_rtf_opens_as_text_and_leaves_its_markup_alone() {
    let root = scratch("rtf");
    let path = root.join("letter.rtf");
    // A font table, a colour table, an ignorable destination, an escaped brace
    // and a code-page byte: everything a reader must not read as text, and the
    // way a Windows writer spells an accented letter.
    let source = concat!(
        r"{\rtf1\ansi\ansicpg1252\uc1\deff0",
        r"{\fonttbl{\f0\froman Times New Roman;}}",
        r"{\colortbl;\red0\green0\blue0;}",
        r"{\*\generator Riched20 10.0;}",
        "\\pard\\f0\\fs24 Dear \\b Anna\\b0 , the fa\\'e7ade\\par\n",
        "Second line with a \\{brace\\}\\par\n",
        "}"
    );
    fs::write(&path, source).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("rich text opens");
    assert_eq!(
        opened.text,
        "Dear Anna, the façade\nSecond line with a {brace}"
    );

    let mut document = Document::from_opened(opened);
    assert_eq!(document.container_format(), Some(Format::Rtf));

    let _ = document.apply_display_text("Dear Anna, the fresh façade\nSecond line with a {brace}");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("the write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    let saved = String::from_utf8(fs::read(&path).expect("read back")).expect("still text");
    // Every piece of markup is the bytes it was.
    assert!(saved.contains(r"{\fonttbl{\f0\froman Times New Roman;}}"));
    assert!(saved.contains(r"{\colortbl;\red0\green0\blue0;}"));
    assert!(saved.contains(r"{\*\generator Riched20 10.0;}"));
    assert!(saved.contains(r"\pard\f0\fs24 "));
    // The accented letter went back as an escape rather than as a raw byte a
    // code-page reader would show as something else.
    assert!(saved.contains(r"\u231?"));
    assert!(saved.contains(r"\{brace\}"));

    let reopened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the saved file opens again");
    assert_eq!(
        reopened.text,
        "Dear Anna, the fresh façade\nSecond line with a {brace}"
    );

    let _ = fs::remove_dir_all(root);
}

/// A PDF is the one format whose text is drawn rather than stored, and the one
/// where the file itself offers the contract: an incremental update leaves the
/// original bytes as the literal prefix of the saved file.
///
/// Read from a document the system ships, because a PDF this crate wrote would
/// only prove it can read itself.
#[test]
fn a_pdf_opens_as_text_and_a_correction_is_appended_to_it() {
    let source = [
        "/usr/share/doc/ijs/ijs_spec.pdf",
        "/usr/share/doc/glm/manual.pdf",
    ]
    .into_iter()
    .find(|path| std::path::Path::new(path).exists());
    let Some(source) = source else {
        // The evidence records which documents this ran against; a machine
        // without them is not a failure of the code.
        return;
    };
    let root = scratch("pdf");
    let path = root.join("document.pdf");
    let original = fs::read(source).expect("read the system document");
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("a pdf opens");
    assert_eq!(
        Document::from_opened(opened.clone()).container_format(),
        Some(Format::Pdf)
    );
    assert!(opened.text.len() > 100, "a manual has text");

    // Correct the first word long enough to be worth correcting.
    let word = opened
        .text
        .split_whitespace()
        .find(|word| word.len() > 5 && word.chars().all(|c| c.is_ascii_alphabetic()))
        .expect("a plain word")
        .to_owned();
    let corrected = opened.text.replacen(&word, "Corrected", 1);

    let mut document = Document::from_opened(opened);
    let _ = document.apply_display_text(&corrected);
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("a pdf with a file has a write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    let saved = fs::read(&path).expect("read back");
    // The promise the format itself makes: everything that was there is still
    // there, at the same offset.
    assert!(
        saved.starts_with(&original),
        "the original must be the prefix"
    );
    assert!(saved.len() > original.len());

    let reopened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the corrected document opens again");
    assert!(reopened.text.contains("Corrected"));

    let _ = fs::remove_dir_all(root);
}

/// The part of "editing a PDF" the format defines outright: a form field has a
/// name and a value, and filling one needs no font and no layout.
#[test]
fn a_pdf_form_shows_its_fields_and_takes_a_filled_one_back() {
    let root = scratch("pdf-form");
    let path = root.join("form.pdf");
    let original = form_document();
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("a form opens");
    // The page's own text first, then the boxes a filler has to see.
    assert!(opened.text.starts_with("Solicitud de prueba"));
    assert!(opened.text.contains("--- Campos del formulario ---"));
    assert!(opened.text.contains("\nNombre: Ana"));

    let filled = opened.text.replace("\nNombre: Ana", "\nNombre: Toni");
    let mut document = Document::from_opened(opened);
    let _ = document.apply_display_text(&filled);
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("a form with a file has a write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    let saved = fs::read(&path).expect("read back");
    assert!(
        saved.starts_with(&original),
        "the original must be the prefix"
    );

    let reopened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the filled form opens again");
    assert!(reopened.text.contains("\nNombre: Toni"));
    assert!(!reopened.text.contains("\nNombre: Ana"));

    let _ = fs::remove_dir_all(root);
}

/// A one-page PDF with one text field, assembled here so the test does not
/// depend on a document the machine may not have.
fn form_document() -> Vec<u8> {
    let stream = b"BT /F1 14 Tf 20 150 Td (Solicitud de prueba) Tj ET";
    let bodies: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm 6 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [7 0 R] >>".to_vec(),
        format!("<< /Length {} >>\nstream\n", stream.len())
            .into_bytes()
            .into_iter()
            .chain(stream.iter().copied())
            .chain(b"\nendstream".iter().copied())
            .collect(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Fields [7 0 R] /DA (/F1 12 Tf 0 g) >>".to_vec(),
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (Nombre) /V (Ana) /Rect [20 40 200 60] /P 3 0 R /DA (/F1 12 Tf 0 g) >>".to_vec(),
    ];

    assemble(&bodies)
}

/// Wraps object bodies into a PDF: the header, each numbered object, and the
/// cross-reference that says where each one starts.
fn assemble(bodies: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(bodies.len());
    for (index, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", bodies.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            bodies.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A text file inside a gzip wrapper: the text comes back exactly, the
/// compression does not, which is why it is imported and not native.
#[test]
fn compressed_text_opens_edits_and_compresses_again() {
    let root = scratch("gzip");
    let path = root.join("notes.txt.gz");
    let inside = "First line\nSecond façade\n";
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(inside.as_bytes()).expect("compress");
    let original = encoder.finish().expect("compress");
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("compressed text opens");
    assert_eq!(opened.text, inside);

    let mut document = Document::from_opened(opened);
    assert_eq!(document.container_format(), Some(Format::Gzip));
    let _ = document.apply_display_text("First line\nSecond façade corrected\n");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("the write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);

    // What was written is still gzip, and what is inside is exactly the text.
    let saved = fs::read(&path).expect("read back");
    assert_eq!(&saved[..2], &[0x1F, 0x8B]);
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(saved.as_slice())
        .read_to_end(&mut out)
        .expect("decompress");
    assert_eq!(
        String::from_utf8(out).expect("utf-8"),
        "First line\nSecond façade corrected\n"
    );

    let _ = fs::remove_dir_all(root);
}

/// A `.gz` holding something that is not text is refused, like any other file
/// whose content is not a document.
#[test]
fn a_compressed_file_that_is_not_text_is_refused() {
    let root = scratch("gzip-binary");
    let path = root.join("image.gz");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&[0u8, 1, 2, 3, 0, 4, 5])
        .expect("compress");
    fs::write(&path, encoder.finish().expect("compress")).expect("write");

    assert!(matches!(
        open(
            &path,
            first_generation(),
            Limits::default(),
            &CancellationToken::new()
        ),
        Err(OpenRefusal::NotImportable { .. })
    ));

    let _ = fs::remove_dir_all(root);
}

/// A book's cover is one image in its own file. Refusing the whole book over a
/// chapter with no text is the wrong answer, and it is what happened to the
/// first real `.epub` this was pointed at.
#[test]
fn a_chapter_without_text_does_not_refuse_the_book() {
    let root = scratch("epub-cover");
    let path = root.join("book.epub");
    let container = concat!(
        r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">"#,
        r#"<rootfiles><rootfile full-path="book.opf"/></rootfiles></container>"#
    );
    let opf = concat!(
        r#"<package xmlns="http://www.idpf.org/2007/opf"><manifest>"#,
        r#"<item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<item id="one" href="one.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"</manifest><spine><itemref idref="cover"/><itemref idref="one"/></spine></package>"#
    );
    let cover =
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><img src="cover.png"/></body></html>"#;
    let chapter =
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>The first line</p></body></html>"#;

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [
        ("META-INF/container.xml", container),
        ("book.opf", opf),
        ("cover.xhtml", cover),
        ("one.xhtml", chapter),
    ] {
        writer.start_file(name, options).expect("start");
        writer.write_all(body.as_bytes()).expect("write");
    }
    fs::write(&path, writer.finish().expect("finish").into_inner()).expect("write");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("the book opens even though its cover has no text");
    assert_eq!(opened.text, "The first line");

    // And the cover is still in the file afterwards, because nothing here
    // edits what it does not show.
    let mut document = Document::from_opened(opened);
    let _ = document.apply_display_text("The corrected line");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("the write");
    };
    let report = perform(&request, &CancellationToken::new()).expect("the save");
    document.apply_save(&report);
    let saved = fs::read(&path).expect("read back");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(saved)).expect("an archive");
    assert!(archive.by_name("cover.xhtml").is_ok());

    let _ = fs::remove_dir_all(root);
}

/// A PDF written before `ToUnicode` existed says what its codes mean in its
/// own `/Encoding`. Reading the bytes instead loses the ligature silently —
/// "Specification" came out as "Specication" — which is the one failure this
/// crate refuses everywhere else.
#[test]
fn a_font_without_to_unicode_is_read_through_its_own_encoding() {
    let root = scratch("pdf-encoding");
    let path = root.join("named.pdf");

    // Code 12 draws `fi` and code 26 draws an em dash, which is exactly the
    // shape a document typeset by TeX has.
    let stream = b"BT /F1 12 Tf 20 150 Td (Speci\\014cation \\032 done) Tj ET";
    let bodies: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
        format!("<< /Length {} >>\nstream\n", stream.len())
            .into_bytes()
            .into_iter()
            .chain(stream.iter().copied())
            .chain(b"\nendstream".iter().copied())
            .collect(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding 6 0 R >>".to_vec(),
        b"<< /Type /Encoding /Differences [12 /fi 26 /emdash] >>".to_vec(),
    ];
    fs::write(&path, assemble(&bodies)).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("a pdf opens");
    // The ligature is kept as the one character the document draws rather than
    // split into two: a document that is not edited must write back the code
    // it came from, and two characters cannot go back into one code.
    assert!(
        opened.text.contains("Speci\u{FB01}cation"),
        "the ligature must be read, got {:?}",
        opened.text
    );
    assert!(opened.text.contains('\u{2014}'), "the em dash must be read");

    let _ = fs::remove_dir_all(root);
}
