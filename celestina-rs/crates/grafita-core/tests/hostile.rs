//! What the importer does with a file written to hurt it.
//!
//! Every document here is hostile on purpose: nested past any real file, cut
//! short, pointing outside itself, or compressed so that a few kilobytes
//! inflate into gigabytes. Grafita opens these on a worker, and Siderita opens
//! them in-process when `Space` previews a file, both built with
//! `panic = "abort"`. So the only acceptable answer is a typed refusal that
//! arrives quickly: a panic, a stack overflow or an allocation the size of
//! the bomb ends the whole application.
//!
//! The first cases are the 2026-09-26 audit's own probes (GRA-1, GRA-2,
//! GRA-3), turned into fixtures.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use celestina_core::{CancellationToken, Generation, GenerationClock};
use flate2::write::DeflateEncoder;
use flate2::Compression;
use grafita_core::document::SaveIntent;
use grafita_core::open::{open, Limits, OpenRefusal, OpenedFile};
use grafita_core::save::perform;
use grafita_core::Document;

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(1);

/// How long any one hostile document may take to be refused. Generous for a
/// debug build on a loaded machine; a regression is minutes, or an abort.
const PROMPT: Duration = Duration::from_secs(30);

const MIB: usize = 1024 * 1024;

fn first_generation() -> Generation {
    let mut clock = GenerationClock::default();
    clock.issue().expect("a first generation")
}

/// Writes `bytes` to a file of their own and opens it the way both hosts do.
fn open_bytes(label: &str, bytes: &[u8], limits: Limits) -> Result<OpenedFile, OpenRefusal> {
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "grafita-hostile-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch directory");
    let path: PathBuf = root.join("document");
    fs::write(&path, bytes).expect("write the fixture");

    let started = Instant::now();
    let outcome = open(&path, first_generation(), limits, &CancellationToken::new());
    let elapsed = started.elapsed();
    let _ = fs::remove_dir_all(root);
    assert!(
        elapsed < PROMPT,
        "'{label}' took {elapsed:?}; a hostile file must be refused promptly"
    );
    outcome
}

fn small_limits(max_bytes: usize) -> Limits {
    Limits {
        max_bytes: max_bytes as u64,
        ..Limits::default()
    }
}

fn assert_not_importable(label: &str, outcome: Result<OpenedFile, OpenRefusal>) {
    match outcome {
        Err(OpenRefusal::NotImportable { .. }) => {}
        other => panic!("'{label}' must be refused as not importable, got {other:?}"),
    }
}

fn assert_too_large(label: &str, outcome: Result<OpenedFile, OpenRefusal>) {
    match outcome {
        Err(OpenRefusal::TooLarge { .. }) => {}
        other => panic!("'{label}' must be refused as too large, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Bombs: raw deflate data that expands a thousandfold, built without ever
// holding the expanded bytes.
// ---------------------------------------------------------------------------

/// A raw deflate stream that inflates to `mebibytes` MiB of `a`.
///
/// One mebibyte is compressed once with a sync flush, which ends it on a byte
/// boundary with no final block; the same bytes repeated are therefore a valid
/// stream that inflates to one more mebibyte each time. A last empty block
/// (fixed Huffman, `BFINAL`, end-of-block) closes it.
fn deflate_bomb(mebibytes: usize) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&vec![b'a'; MIB]).expect("compress");
    encoder.flush().expect("sync flush");
    let chunk = encoder.get_ref().clone();
    let mut out = Vec::with_capacity(chunk.len() * mebibytes + 2);
    for _ in 0..mebibytes {
        out.extend_from_slice(&chunk);
    }
    out.extend_from_slice(&[0x03, 0x00]);
    out
}

/// The same bomb in a gzip wrapper. The trailer is never reached.
fn gzip_bomb(mebibytes: usize) -> Vec<u8> {
    let mut out = vec![0x1F, 0x8B, 0x08, 0, 0, 0, 0, 0, 0, 0x03];
    out.extend_from_slice(&deflate_bomb(mebibytes));
    out.extend_from_slice(&[0; 8]);
    out
}

/// The same bomb in a zlib wrapper, which is what PDF's `FlateDecode` reads.
fn zlib_bomb(mebibytes: usize) -> Vec<u8> {
    let mut out = vec![0x78, 0xDA];
    out.extend_from_slice(&deflate_bomb(mebibytes));
    out.extend_from_slice(&[0; 4]);
    out
}

fn zlib(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(bytes).expect("compress");
    encoder.finish().expect("compress")
}

#[test]
fn the_bomb_builder_really_is_a_bomb() {
    // The fixtures below are only meaningful if the bytes expand as claimed.
    let mut out = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::DeflateDecoder::new(deflate_bomb(3).as_slice()),
        &mut out,
    )
    .expect("a valid deflate stream");
    assert_eq!(out.len(), 3 * MIB);
    assert!(out.iter().all(|byte| *byte == b'a'));
    assert!(deflate_bomb(1024).len() < 2 * MIB, "1 GiB from under 2 MiB");
}

// ---------------------------------------------------------------------------
// ZIP containers written by hand, so a header can say what a writer would not.
// ---------------------------------------------------------------------------

struct Member<'a> {
    name: &'a str,
    method: u16,
    data: Vec<u8>,
    crc: u32,
    uncompressed_size: u32,
}

fn crc_of(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}

fn zip_of(members: &[Member<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for member in members {
        let offset = u32::try_from(out.len()).expect("a small fixture");
        let name = member.name.as_bytes();
        let compressed = u32::try_from(member.data.len()).expect("a small fixture");
        let name_length = u16::try_from(name.len()).expect("a short name");

        out.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&member.method.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&member.crc.to_le_bytes());
        out.extend_from_slice(&compressed.to_le_bytes());
        out.extend_from_slice(&member.uncompressed_size.to_le_bytes());
        out.extend_from_slice(&name_length.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&member.data);

        directory.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        directory.extend_from_slice(&20u16.to_le_bytes());
        directory.extend_from_slice(&20u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&member.method.to_le_bytes());
        directory.extend_from_slice(&[0; 4]);
        directory.extend_from_slice(&member.crc.to_le_bytes());
        directory.extend_from_slice(&compressed.to_le_bytes());
        directory.extend_from_slice(&member.uncompressed_size.to_le_bytes());
        directory.extend_from_slice(&name_length.to_le_bytes());
        directory.extend_from_slice(&[0; 12]);
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name);
    }
    let directory_offset = u32::try_from(out.len()).expect("a small fixture");
    let directory_size = u32::try_from(directory.len()).expect("a small fixture");
    let count = u16::try_from(members.len()).expect("a few members");
    out.extend_from_slice(&directory);
    out.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn deflated<'a>(name: &'a str, content: &[u8]) -> Member<'a> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(content).expect("compress");
    Member {
        name,
        method: 8,
        data: encoder.finish().expect("compress"),
        crc: crc_of(content),
        uncompressed_size: u32::try_from(content.len()).expect("a small member"),
    }
}

const WORD_DOCUMENT: &[u8] = concat!(
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:body><w:p><w:r><w:t>Report</w:t></w:r></w:p></w:body></w:document>"#
)
.as_bytes();

#[test]
fn the_hand_written_zip_is_a_real_docx() {
    // The control for every ZIP case below: honest headers open.
    let bytes = zip_of(&[deflated("word/document.xml", WORD_DOCUMENT)]);
    let opened = open_bytes("honest-docx", &bytes, Limits::default()).expect("an honest docx");
    assert_eq!(opened.text, "Report");
}

#[test]
fn a_zip_member_that_inflates_past_the_ceiling_is_too_large() {
    // GRA-2: a member that honestly declares one gibibyte.
    let bytes = zip_of(&[Member {
        name: "word/document.xml",
        method: 8,
        data: deflate_bomb(1024),
        crc: 0,
        uncompressed_size: u32::MAX,
    }]);
    assert_too_large(
        "zip-bomb",
        open_bytes("zip-bomb", &bytes, Limits::default()),
    );
}

#[test]
fn a_zip_member_that_hides_a_bomb_behind_a_small_size_is_refused() {
    // GRA-2: the header says 100 bytes; the data says a gibibyte. Neither the
    // header nor the data is believed past the smaller of the two.
    let bytes = zip_of(&[Member {
        name: "word/document.xml",
        method: 8,
        data: deflate_bomb(1024),
        crc: 0,
        uncompressed_size: 100,
    }]);
    assert_not_importable(
        "zip-small-header-bomb",
        open_bytes("zip-small-header-bomb", &bytes, Limits::default()),
    );
}

#[test]
fn a_zip_header_that_overstates_its_member_is_refused() {
    // GRA-2: a correct member whose header claims four gibibytes. The claim
    // used to size an allocation before a byte was inflated.
    let mut member = deflated("word/document.xml", WORD_DOCUMENT);
    member.uncompressed_size = u32::MAX;
    let bytes = zip_of(&[member]);
    assert_not_importable(
        "zip-lying-header",
        open_bytes("zip-lying-header", &bytes, Limits::default()),
    );
}

#[test]
fn members_that_each_fit_but_together_do_not_are_too_large() {
    // GRA-2: the ceiling is for the document, not for each part of it.
    let chapter = |word: &str| {
        let mut xhtml = String::from("<html><body>");
        while xhtml.len() < 400 * 1024 {
            xhtml.push_str("<p>");
            xhtml.push_str(word);
            xhtml.push_str("</p>");
        }
        xhtml.push_str("</body></html>");
        xhtml.into_bytes()
    };
    let (one, two, three) = (chapter("one"), chapter("two"), chapter("three"));
    let package = concat!(
        r#"<package><manifest>"#,
        r#"<item id="a" href="one.xhtml"/><item id="b" href="two.xhtml"/>"#,
        r#"<item id="c" href="three.xhtml"/></manifest>"#,
        r#"<spine><itemref idref="a"/><itemref idref="b"/><itemref idref="c"/></spine>"#,
        r#"</package>"#
    );
    let container =
        r#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#;
    let bytes = zip_of(&[
        deflated("META-INF/container.xml", container.as_bytes()),
        deflated("content.opf", package.as_bytes()),
        deflated("one.xhtml", &one),
        deflated("two.xhtml", &two),
        deflated("three.xhtml", &three),
    ]);

    // Each chapter alone is under the ceiling, and the book opens under a
    // ceiling that holds all three.
    open_bytes("epub-fits", &bytes, small_limits(2 * MIB)).expect("the book fits in 2 MiB");
    assert_too_large(
        "epub-total",
        open_bytes("epub-total", &bytes, small_limits(MIB)),
    );
}

// ---------------------------------------------------------------------------
// gzip
// ---------------------------------------------------------------------------

#[test]
fn a_gzip_bomb_of_one_gibibyte_is_too_large() {
    // GRA-2, the audit's own probe: a 1 MiB `.gz` became a 1 GiB document.
    let bytes = gzip_bomb(1024);
    assert!(bytes.len() < 2 * MIB);
    assert_too_large(
        "gzip-bomb",
        open_bytes("gzip-bomb", &bytes, Limits::default()),
    );
}

#[test]
fn a_gzip_file_just_over_a_small_ceiling_is_too_large() {
    // An honest file this time, with its checksum: the ceiling alone decides.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&vec![b'a'; 2 * MIB]).expect("compress");
    let bytes = encoder.finish().expect("compress");
    assert_too_large(
        "gzip-over",
        open_bytes("gzip-over", &bytes, small_limits(MIB)),
    );
    let fits = open_bytes("gzip-fits", &bytes, small_limits(3 * MIB)).expect("2 MiB of text");
    assert_eq!(fits.text.len(), 2 * MIB);
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

/// A classic PDF: each body becomes object `index + 1`, located by a table.
fn classic_pdf(bodies: &[Vec<u8>]) -> Vec<u8> {
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

fn stream_object(dictionary: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dictionary} >>\nstream\n").into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

const FONT: &str = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>";

/// A one-page document whose content stream is `content`, under `dictionary`.
fn one_page(dictionary: &str, content: &[u8]) -> Vec<u8> {
    classic_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_vec(),
        stream_object(dictionary, content),
        FONT.as_bytes().to_vec(),
    ])
}

#[test]
fn the_pdf_builder_opens() {
    // The control for every PDF case below.
    let content = b"BT /F1 12 Tf (Hello) Tj ET";
    let bytes = one_page(&format!("/Length {}", content.len()), content);
    let opened = open_bytes("pdf-control", &bytes, Limits::default()).expect("a plain pdf");
    assert_eq!(opened.text, "Hello");

    let packed = zlib(content);
    let bytes = one_page(
        &format!("/Length {} /Filter /FlateDecode", packed.len()),
        &packed,
    );
    let opened = open_bytes("pdf-flate-control", &bytes, Limits::default()).expect("flate");
    assert_eq!(opened.text, "Hello");
}

#[test]
fn twenty_thousand_nested_arrays_are_refused_not_a_stack_overflow() {
    // GRA-1, the audit's probe: `1 0 obj` followed by 20 000 `[` overflowed
    // the worker's stack, which aborts; it cannot be caught.
    let mut deep = vec![b'['; 20_000];
    deep.extend(vec![b']'; 20_000]);
    let bytes = classic_pdf(&[deep]);
    assert_not_importable(
        "deep-array",
        open_bytes("deep-array", &bytes, Limits::default()),
    );

    let mut unclosed = b"%PDF-1.4\n1 0 obj ".to_vec();
    unclosed.extend(vec![b'['; 200_000]);
    assert_not_importable(
        "deep-unclosed",
        open_bytes("deep-unclosed", &unclosed, Limits::default()),
    );
}

#[test]
fn nested_dictionaries_in_the_trailer_are_refused() {
    // The same recursion through `<<`, reached while the structure is read.
    let mut bytes = b"%PDF-1.4\nxref\n0 0\ntrailer\n".to_vec();
    for _ in 0..20_000 {
        bytes.extend_from_slice(b"<< /A ");
    }
    bytes.extend_from_slice(b"\nstartxref\n9\n%%EOF\n");
    assert_not_importable(
        "deep-dict",
        open_bytes("deep-dict", &bytes, Limits::default()),
    );
}

#[test]
fn a_cross_reference_entry_that_is_not_text_is_refused() {
    // GRA-1: 18 bytes of 0xFF where an entry belongs panicked slicing "".
    // The same bytes marked as an entry in use (`n`) must not be read as an
    // offset either.
    for kind in [0xFF, b'n'] {
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let xref = bytes.len();
        bytes.extend_from_slice(b"xref\n0 1\n");
        bytes.extend(vec![0xFF; 17]);
        bytes.push(kind);
        bytes.extend_from_slice(
            format!("\ntrailer\n<< /Root 0 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        assert_not_importable("xref-ff", open_bytes("xref-ff", &bytes, Limits::default()));
    }
}

#[test]
fn a_cross_reference_past_the_end_of_the_file_is_refused() {
    // GRA-1: `startxref 999999` in a 32-byte file panicked at the slice.
    let bytes = b"%PDF-1.4\nstartxref 999999\n%%EOF\n";
    assert_not_importable("past-eof", open_bytes("past-eof", bytes, Limits::default()));
}

#[test]
fn a_stream_length_that_wraps_the_address_space_is_refused_or_ignored() {
    // GRA-1: `start + length` wrapped, and the slice ran backwards.
    let content = b"BT /F1 12 Tf (Hello) Tj ET";
    let bytes = one_page("/Length 18446744073709551615", content);
    match open_bytes("wrapping-length", &bytes, Limits::default()) {
        // The `endstream` keyword still bounds the data, which is a fine
        // answer; so is a refusal. A panic is not.
        Ok(opened) => assert_eq!(opened.text, "Hello"),
        Err(OpenRefusal::NotImportable { .. }) => {}
        Err(other) => panic!("unexpected refusal {other:?}"),
    }
}

/// A PDF 1.5 file: `packed` bodies live in object stream `objstm`, the rest at
/// top level, and a cross-reference stream says where everything is.
fn modern_pdf(top: &[(u32, Vec<u8>)], packed: &[(u32, Vec<u8>)], objstm: u32, n: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut locations: Vec<(u32, u8, u32, u32)> = Vec::new();
    for (number, body) in top {
        locations.push((*number, 1, u32::try_from(out.len()).expect("small"), 0));
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    let mut header = String::new();
    let mut bodies = Vec::new();
    for (index, (number, body)) in packed.iter().enumerate() {
        header.push_str(&format!("{number} {} ", bodies.len()));
        bodies.extend_from_slice(body);
        bodies.push(b'\n');
        locations.push((*number, 2, objstm, u32::try_from(index).expect("small")));
    }
    let mut content = header.clone().into_bytes();
    content.extend_from_slice(&bodies);
    let dictionary = format!(
        "/Type /ObjStm /N {n} /First {} /Length {}",
        header.len(),
        content.len()
    );
    locations.push((objstm, 1, u32::try_from(out.len()).expect("small"), 0));
    out.extend_from_slice(format!("{objstm} 0 obj\n").as_bytes());
    out.extend_from_slice(&stream_object(&dictionary, &content));
    out.extend_from_slice(b"\nendobj\n");

    let xref_number = locations.iter().map(|entry| entry.0).max().unwrap_or(0) + 1;
    let xref_offset = u32::try_from(out.len()).expect("small");
    locations.push((xref_number, 1, xref_offset, 0));
    locations.sort_unstable();
    let size = xref_number + 1;
    let mut rows = Vec::new();
    for number in 0..size {
        match locations.iter().find(|entry| entry.0 == number) {
            Some((_, kind, field, index)) => {
                rows.push(*kind);
                rows.extend_from_slice(&field.to_be_bytes());
                rows.extend_from_slice(&u16::try_from(*index).expect("small").to_be_bytes());
            }
            None => rows.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0]),
        }
    }
    let dictionary = format!(
        "/Type /XRef /Size {size} /W [1 4 2] /Root 1 0 R /Length {}",
        rows.len()
    );
    out.extend_from_slice(format!("{xref_number} 0 obj\n").as_bytes());
    out.extend_from_slice(&stream_object(&dictionary, &rows));
    out.extend_from_slice(format!("\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes());
    out
}

fn modern_one_page(n: &str) -> Vec<u8> {
    let content = b"BT /F1 12 Tf (Hello) Tj ET";
    modern_pdf(
        &[(4, stream_object(&format!("/Length {}", content.len()), content))],
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
                    .to_vec(),
            ),
            (5, FONT.as_bytes().to_vec()),
        ],
        6,
        n,
    )
}

#[test]
fn an_object_stream_document_opens_and_takes_a_correction() {
    // The control for the object-stream cases: PDF 1.5's own layout, read
    // through the remembered object stream and written back as an update.
    let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "grafita-hostile-objstm-save-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch directory");
    let path = root.join("document.pdf");
    let original = modern_one_page("4");
    fs::write(&path, &original).expect("write the fixture");

    let opened = open(
        &path,
        first_generation(),
        Limits::default(),
        &CancellationToken::new(),
    )
    .expect("pdf 1.5");
    assert_eq!(opened.text, "Hello");

    let mut document = Document::from_opened(opened);
    let _ = document.apply_display_text("Howdy");
    let SaveIntent::Ready(request) = document.save_request() else {
        panic!("a pdf with a file has a write");
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
    .expect("the corrected document opens again");
    assert_eq!(reopened.text, "Howdy");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_object_stream_that_claims_a_huge_count_is_refused() {
    // GRA-3: `/N` sized a vector directly; a count past `usize::MAX`
    // saturated to it and the allocation aborted.
    for n in ["4294967295", "99999999999999999999"] {
        let bytes = modern_one_page(n);
        match open_bytes("objstm-huge-n", &bytes, Limits::default()) {
            Err(OpenRefusal::NotImportable { .. }) => {}
            other => panic!("/N {n} must be refused, got {other:?}"),
        }
    }
}

#[test]
fn cross_reference_field_widths_the_file_cannot_hold_are_refused() {
    // GRA-1: `/W` comes from the file; a width wider than a row, or the sum
    // of three of them, overflowed or read past the row.
    for widths in [
        "[99999999999999999999 1 1]",
        "[18446744073709551615 1 1]",
        "[0 0 0]",
        "[1 9 2]",
    ] {
        let rows = [1u8, 0, 0, 0, 9, 0, 0];
        let mut bytes = b"%PDF-1.5\n".to_vec();
        let offset = bytes.len();
        bytes.extend_from_slice(b"1 0 obj\n");
        bytes.extend_from_slice(&stream_object(
            &format!(
                "/Type /XRef /Size 4000000000 /W {widths} /Root 1 0 R /Length {}",
                rows.len()
            ),
            &rows,
        ));
        bytes.extend_from_slice(format!("\nendobj\nstartxref\n{offset}\n%%EOF\n").as_bytes());
        match open_bytes("xref-widths", &bytes, Limits::default()) {
            Err(OpenRefusal::NotImportable { .. }) => {}
            other => panic!("/W {widths} must be refused, got {other:?}"),
        }
    }
}

#[test]
fn a_predictor_with_absurd_columns_is_refused_or_ignored() {
    // `/Columns` sized a vector straight from the file.
    let rows = [2u8, 1, 0, 0, 0, 9, 0, 0];
    let mut bytes = b"%PDF-1.5\n".to_vec();
    let offset = bytes.len();
    bytes.extend_from_slice(b"1 0 obj\n");
    bytes.extend_from_slice(&stream_object(
        &format!(
            "/Type /XRef /Size 1 /W [1 4 2] /Root 1 0 R /DecodeParms << /Predictor 12 /Columns 99999999999999999999 >> /Length {}",
            rows.len()
        ),
        &rows,
    ));
    bytes.extend_from_slice(format!("\nendobj\nstartxref\n{offset}\n%%EOF\n").as_bytes());
    assert_not_importable(
        "predictor",
        open_bytes("predictor", &bytes, Limits::default()),
    );
}

#[test]
fn a_flate_bomb_in_a_page_is_too_large() {
    // GRA-2: `FlateDecode` inflated without a ceiling.
    let bomb = zlib_bomb(1024);
    let bytes = one_page(
        &format!("/Length {} /Filter /FlateDecode", bomb.len()),
        &bomb,
    );
    assert_too_large(
        "pdf-bomb",
        open_bytes("pdf-bomb", &bytes, Limits::default()),
    );
}

#[test]
fn a_filter_chain_counts_against_one_ceiling() {
    // GRA-2: each `FlateDecode` in a `/Filter` array multiplied the last. The
    // first stage inflates a few kilobytes into 8 MiB of stored deflate
    // blocks, and the second turns those into 8 MiB of text: either stage
    // alone fits a 12 MiB ceiling, and the chain as a whole does not.
    let mut stored = flate2::write::ZlibEncoder::new(Vec::new(), Compression::none());
    stored.write_all(&vec![b'a'; 8 * MIB]).expect("store");
    let middle = stored.finish().expect("store");
    let outer = zlib(&middle);
    let bytes = one_page(
        &format!(
            "/Length {} /Filter [/FlateDecode /FlateDecode]",
            outer.len()
        ),
        &outer,
    );
    assert_too_large(
        "pdf-chain",
        open_bytes("pdf-chain", &bytes, small_limits(12 * MIB)),
    );
}

#[test]
fn a_cross_reference_stream_bomb_is_too_large() {
    // The cross-reference stream is inflated before anything else is known.
    let bomb = zlib_bomb(1024);
    let mut bytes = b"%PDF-1.5\n".to_vec();
    let offset = bytes.len();
    bytes.extend_from_slice(b"1 0 obj\n");
    bytes.extend_from_slice(&stream_object(
        &format!(
            "/Type /XRef /Size 1 /W [1 4 2] /Root 1 0 R /Filter /FlateDecode /Length {}",
            bomb.len()
        ),
        &bomb,
    ));
    bytes.extend_from_slice(format!("\nendobj\nstartxref\n{offset}\n%%EOF\n").as_bytes());
    assert_too_large(
        "xref-bomb",
        open_bytes("xref-bomb", &bytes, Limits::default()),
    );
}

#[test]
fn a_page_tree_that_revisits_its_nodes_is_refused() {
    // Forty nodes whose kids are the next node twice: 2^40 leaves from a few
    // hundred bytes. A tree that names a node twice is not a tree.
    let mut bodies = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    for level in 0..40u32 {
        let next = level + 3;
        bodies.push(format!("<< /Type /Pages /Kids [{next} 0 R {next} 0 R] >>").into_bytes());
    }
    bodies.push(b"<< /Type /Page >>".to_vec());
    let bytes = classic_pdf(&bodies);
    assert_not_importable(
        "page-dag",
        open_bytes("page-dag", &bytes, Limits::default()),
    );
}

#[test]
fn many_pages_drawing_one_large_stream_run_out_of_budget() {
    // Decoding is bounded per stream; drawing the same decoded megabytes on
    // thousands of pages is bounded by the document's work budget.
    let mut content = b"BT /F1 12 Tf ".to_vec();
    while content.len() < 256 * 1024 {
        content.extend_from_slice(b"0 0 Td ");
    }
    content.extend_from_slice(b"(Hello) Tj ET");
    let packed = zlib(&content);
    let pages = 4096u32;
    let mut bodies = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        stream_object(
            &format!("/Length {} /Filter /FlateDecode", packed.len()),
            &packed,
        ),
        FONT.as_bytes().to_vec(),
    ];
    let mut kids = String::new();
    for page in 0..pages {
        kids.push_str(&format!("{} 0 R ", page + 5));
        bodies.push(
            b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 4 0 R >> >> /Contents 3 0 R >>"
                .to_vec(),
        );
    }
    bodies[1] = format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>").into_bytes();
    let bytes = classic_pdf(&bodies);
    assert_too_large(
        "shared-stream",
        open_bytes("shared-stream", &bytes, small_limits(4 * MIB)),
    );
}

#[test]
fn a_document_packed_in_one_object_stream_opens_in_linear_time() {
    // GRA-3: every in-stream lookup re-inflated and re-lexed the whole object
    // stream, so a document of n pages cost n² object reads.
    let pages = 3000u32;
    let content = b"BT /F1 12 Tf (Hello) Tj ET";
    let mut packed = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (5, FONT.as_bytes().to_vec()),
    ];
    let mut kids = String::new();
    for page in 0..pages {
        let number = page + 10;
        kids.push_str(&format!("{number} 0 R "));
        packed.push((
            number,
            b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
                .to_vec(),
        ));
    }
    packed.insert(
        1,
        (
            2,
            format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>").into_bytes(),
        ),
    );
    let count = packed.len().to_string();
    let bytes = modern_pdf(
        &[(
            4,
            stream_object(&format!("/Length {}", content.len()), content),
        )],
        &packed,
        6,
        &count,
    );

    let started = Instant::now();
    let opened = open_bytes("objstm-many", &bytes, Limits::default()).expect("pdf 1.5");
    let elapsed = started.elapsed();
    assert_eq!(opened.text.matches("Hello").count(), pages as usize);
    assert!(
        elapsed < Duration::from_secs(5),
        "{pages} pages in one object stream took {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// The text layer: fonts, maps and page trees that cost far more to read than
// the bytes they take. These are the 2026-09-26 review's probes of this unit.
// ---------------------------------------------------------------------------

fn flate_stream(data: &[u8]) -> Vec<u8> {
    let packed = zlib(data);
    stream_object(
        &format!("/Length {} /Filter /FlateDecode", packed.len()),
        &packed,
    )
}

/// One page drawn by `content` with a two-byte font whose `ToUnicode` map is
/// `map`.
fn page_with_map(content: &[u8], map: &[u8]) -> Vec<u8> {
    classic_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
        flate_stream(content),
        b"<< /Type /Font /Subtype /Type0 /ToUnicode 6 0 R >>".to_vec(),
        flate_stream(map),
    ])
}

#[test]
fn a_map_of_four_byte_ranges_stores_only_codes_the_font_can_draw() {
    // Each `bfrange` line of four-byte codes used to store 65 536 distinct
    // entries; a 2 KB file ran out of memory and aborted.
    let mut map = b"beginbfrange\n".to_vec();
    for line in 0..4096u32 {
        let low = line << 16;
        map.extend_from_slice(format!("<{low:08X}><{:08X}><0041>\n", low | 0xFFFF).as_bytes());
    }
    map.extend_from_slice(b"endbfrange\n");
    let bytes = page_with_map(b"BT /F1 1 Tf <0000> Tj ET", &map);
    assert!(bytes.len() < 64 * 1024);
    let opened = open_bytes("map-wide-codes", &bytes, Limits::default()).expect("one page");
    assert_eq!(opened.text, "A");
}

#[test]
fn a_map_that_rewrites_the_same_range_forever_is_refused() {
    // The same 65 536 codes restated thousands of times: bounded in memory,
    // but minutes of work for one font.
    let mut map = b"beginbfrange\n".to_vec();
    for _ in 0..20_000 {
        map.extend_from_slice(b"<0000><FFFF><0041>\n");
    }
    map.extend_from_slice(b"endbfrange\n");
    let bytes = page_with_map(b"BT /F1 1 Tf <0000> Tj ET", &map);
    assert_not_importable(
        "map-rewrites",
        open_bytes("map-rewrites", &bytes, Limits::default()),
    );
}

#[test]
fn a_map_range_at_the_top_of_the_code_space_does_not_overflow() {
    // `low + 0xFFFF` and `low + offset` overflowed `u32` on file data.
    let bytes = page_with_map(
        b"BT /F1 1 Tf <0000> Tj ET",
        b"beginbfrange\n<FFFFFFFF><FFFFFFFF><0041>\n<FFFFFFFF><FFFFFFFF>[<0041> <0042>]\nendbfrange\n",
    );
    match open_bytes("map-top", &bytes, Limits::default()) {
        Ok(_) | Err(OpenRefusal::NotImportable { .. }) => {}
        Err(other) => panic!("unexpected refusal {other:?}"),
    }
}

#[test]
fn selecting_one_font_many_times_does_not_copy_its_map() {
    // Every `Tf` used to clone the font's whole map into the extraction; a
    // few hundred bytes selecting a full-range font 100 000 times aborted.
    let map = b"beginbfrange\n<0000><FFFF><0041>\nendbfrange\n";
    let mut content = b"BT ".to_vec();
    for _ in 0..100_000 {
        content.extend_from_slice(b"/F1 1 Tf ");
    }
    content.extend_from_slice(b"<0000> Tj ET");
    let bytes = page_with_map(&content, map);
    assert!(bytes.len() < 4 * 1024);
    let opened = open_bytes("tf-repeat", &bytes, Limits::default()).expect("one page");
    assert_eq!(opened.text, "A");
}

/// `pages` inline pages under one node whose resources name a font
/// dictionary of `fonts` entries, all the same font. `shared` puts that
/// dictionary in an object of its own; otherwise it is written inline.
fn pages_naming_fonts(pages: usize, fonts: usize, shared: bool) -> Vec<u8> {
    let mut dictionary = String::from("<< ");
    for index in 0..fonts {
        dictionary.push_str(&format!("/F{index} 4 0 R "));
    }
    dictionary.push_str(">>");
    let mut kids = String::new();
    for _ in 0..pages {
        kids.push_str("<</Type/Page>>");
    }
    let resources = if shared {
        "<< /Font 3 0 R >>".to_owned()
    } else {
        format!("<< /Font {dictionary} >>")
    };
    classic_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Resources {resources} /Kids [{kids}] >>").into_bytes(),
        dictionary.into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /Encoding /WinAnsiEncoding >>".to_vec(),
    ])
}

#[test]
fn pages_sharing_a_large_font_dictionary_read_it_once() {
    // 9 000 pages inheriting a 9 000-entry font dictionary re-read it, and
    // every font in it, once per page: 215 KB ran for more than five minutes.
    let bytes = pages_naming_fonts(9_000, 9_000, true);
    assert!(bytes.len() < 256 * 1024);
    // No page draws anything, so the answer is "no text", promptly.
    assert_not_importable(
        "pages-fonts",
        open_bytes("pages-fonts", &bytes, Limits::default()),
    );

    // Written inline, the dictionary cannot be remembered by number, so
    // listing it per page runs out of the work budget instead.
    let bytes = pages_naming_fonts(9_000, 9_000, false);
    assert_too_large(
        "pages-inline-fonts",
        open_bytes("pages-inline-fonts", &bytes, small_limits(MIB)),
    );
}

#[test]
fn cross_reference_tables_nested_in_each_other_are_refused() {
    // Each section's entries run over the next section's header, so n
    // sections read about 1.5 n² entries between them: 600 million here,
    // from about a megabyte.
    let sections = 20_000usize;
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let base = bytes.len();
    for index in 0..sections {
        let header = format!("xref 0 {:010}\n", 3 * (sections - 1 - index));
        assert_eq!(header.len(), 18);
        bytes.extend_from_slice(header.as_bytes());
    }
    for index in (0..sections).rev() {
        let previous = if index + 1 < sections {
            base + 18 * (index + 1)
        } else {
            0
        };
        let mut trailer = format!("trailer<</Prev {previous:010}>>").into_bytes();
        while trailer.len() < 36 {
            trailer.push(b'%');
        }
        bytes.extend_from_slice(&trailer);
    }
    bytes.extend_from_slice(format!("\nstartxref\n{base}\n%%EOF\n").as_bytes());
    assert_not_importable(
        "xref-nest",
        open_bytes("xref-nest", &bytes, Limits::default()),
    );
}

#[test]
fn a_book_whose_spine_repeats_one_chapter_reads_it_once() {
    // A manifest of n items and a spine of 10n references were matched by
    // searching, n × 10n comparisons, and every reference read the member
    // again.
    let items = 5_000;
    let mut package = String::from("<package><manifest>");
    for index in 0..items {
        package.push_str(&format!("<item id=\"i{index}\" href=\"c.xhtml\"/>"));
    }
    package.push_str("</manifest><spine>");
    for _ in 0..10 * items {
        package.push_str(&format!("<itemref idref=\"i{}\"/>", items - 1));
    }
    package.push_str("</spine></package>");
    let container =
        r#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#;
    let bytes = zip_of(&[
        deflated("META-INF/container.xml", container.as_bytes()),
        deflated("content.opf", package.as_bytes()),
        deflated("c.xhtml", b"<html><body><p>x</p></body></html>"),
    ]);
    let opened = open_bytes("epub-spine", &bytes, Limits::default()).expect("one chapter");
    assert_eq!(opened.text, "x");
}

#[test]
fn a_long_document_with_shared_cjk_fonts_opens_under_the_preview_ceiling() {
    // Legitimate: 3 000 pages, two two-byte fonts with 2 000-entry maps
    // shared by every page. Charging each font once per page refused it as
    // too large under Siderita's 8 MiB preview ceiling.
    let mut map = b"2000 beginbfchar\n".to_vec();
    for code in 0..2000u32 {
        map.extend_from_slice(
            format!("<{:04X}><{:04X}>\n", code + 1, 0x4E00 + code * 7 % 20_000).as_bytes(),
        );
    }
    map.extend_from_slice(b"endbfchar\n");
    let mut bodies = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        b"<< /Type /Font /Subtype /Type0 /ToUnicode 5 0 R >>".to_vec(),
        b"<< /Type /Font /Subtype /Type0 /ToUnicode 6 0 R >>".to_vec(),
        flate_stream(&map),
        flate_stream(&map),
    ];
    let pages = 3_000;
    let mut kids = String::new();
    for _ in 0..pages {
        let content = bodies.len() + 1;
        bodies.push(flate_stream(
            b"BT /F1 12 Tf <00010002> Tj /F2 12 Tf <0003> Tj ET",
        ));
        let page = bodies.len() + 1;
        bodies.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content} 0 R >>"
            )
            .into_bytes(),
        );
        kids.push_str(&format!("{page} 0 R "));
    }
    bodies[1] = format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>").into_bytes();
    let bytes = classic_pdf(&bodies);
    assert!(bytes.len() < MIB);

    let opened = open_bytes("legit-cjk", &bytes, small_limits(8 * MIB)).expect("a real book");
    assert_eq!(opened.text.lines().count(), pages);
    assert!(opened.text.starts_with("\u{4E00}\u{4E07}\u{4E0E}"));
}

#[test]
fn a_child_field_listed_before_its_parent_keeps_its_full_name() {
    // `/Fields` names the child first and its parent second; the child is
    // still named by its path from the top.
    let stream = b"BT /F1 14 Tf 20 150 Td (Form) Tj ET";
    let bytes = classic_pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm 6 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            .to_vec(),
        stream_object(&format!("/Length {}", stream.len()), stream),
        FONT.as_bytes().to_vec(),
        b"<< /Fields [8 0 R 7 0 R] >>".to_vec(),
        b"<< /T (person) /Kids [8 0 R] >>".to_vec(),
        b"<< /FT /Tx /T (name) /V (Ann) /Parent 7 0 R >>".to_vec(),
    ]);
    let opened = open_bytes("field-order", &bytes, Limits::default()).expect("a form");
    assert!(
        opened.text.contains("\nperson.name: Ann"),
        "{}",
        opened.text
    );
    assert_eq!(opened.text.matches("Ann").count(), 1);
}

// ---------------------------------------------------------------------------
// Second review round: one large object reached many times, through aliased
// numbers or plain references, and cross-reference sections nested inside
// one another. Each must be refused, or read once, promptly.
// ---------------------------------------------------------------------------

/// A classic PDF of `objects` (number, body), where each `(alias, target)`
/// makes the cross-reference send `alias` to `target`'s bytes.
fn aliased_pdf(objects: &[(u32, Vec<u8>)], aliases: &[(u32, u32)]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = std::collections::BTreeMap::new();
    let mut sorted: Vec<&(u32, Vec<u8>)> = objects.iter().collect();
    sorted.sort_by_key(|(number, _)| *number);
    for (number, body) in sorted {
        offsets.insert(*number, out.len());
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    for (alias, target) in aliases {
        let offset = offsets[target];
        offsets.insert(*alias, offset);
    }
    let size = offsets.keys().max().copied().unwrap_or(0) + 1;
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for number in 1..size {
        match offsets.get(&number) {
            Some(offset) => out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes()),
            None => out.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

const ALIAS_BASE: u32 = 1000;

fn kids_from(first: u32, count: u32) -> String {
    (first..first + count)
        .map(|number| format!("{number} 0 R"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hello_stream() -> Vec<u8> {
    let content = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    stream_object(&format!("/Length {}", content.len()), content)
}

#[test]
fn many_numbers_for_one_large_page_are_refused_without_holding_it_each_time() {
    // 3 000 cross-reference entries at the offset of one page carrying
    // 100 000 numbers: each kid was read again and kept, 4.7 GB for 250 KB,
    // and an abort at 3 000 under a memory ceiling.
    let pages = 3_000;
    let junk = "0 ".repeat(100_000);
    let bytes = aliased_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (
                2,
                format!(
                    "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                    kids_from(ALIAS_BASE, pages)
                )
                .into_bytes(),
            ),
            (
                3,
                format!(
                    "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Junk [{junk}] >>"
                )
                .into_bytes(),
            ),
            (4, hello_stream()),
            (5, FONT.as_bytes().to_vec()),
        ],
        &(ALIAS_BASE..ALIAS_BASE + pages)
            .map(|alias| (alias, 3))
            .collect::<Vec<_>>(),
    );
    assert_not_importable(
        "page-aliases",
        open_bytes("page-aliases", &bytes, Limits::default()),
    );
}

#[test]
fn pages_that_all_name_one_large_contents_array_run_out_of_budget() {
    // Distinct pages whose `/Contents` is the same large array and never a
    // stream: the budget was only checked after a stream was decoded, so
    // 800 MB was lexed and nothing refused it.
    let pages = 2_000u32;
    let mut objects = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (
            2,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                kids_from(ALIAS_BASE, pages)
            )
            .into_bytes(),
        ),
        (6, format!("[{}]", "0 ".repeat(200_000)).into_bytes()),
    ];
    for page in 0..pages {
        objects.push((
            ALIAS_BASE + page,
            b"<< /Type /Page /Parent 2 0 R /Contents 6 0 R >>".to_vec(),
        ));
    }
    let bytes = aliased_pdf(&objects, &[]);
    assert_too_large(
        "contents-array",
        open_bytes("contents-array", &bytes, small_limits(MIB)),
    );
}

#[test]
fn a_form_whose_fields_all_name_one_large_object_reads_it_once() {
    // 10 000 `/Fields` entries at the offset of the `/AcroForm` itself: each
    // was read twice, and each read listed the whole form again.
    let fields = 10_000;
    let bytes = aliased_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R /AcroForm 7 0 R >>".to_vec()),
            (
                7,
                format!("<< /Fields [{}] >>", kids_from(ALIAS_BASE, fields)).into_bytes(),
            ),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
                    .to_vec(),
            ),
            (4, hello_stream()),
            (5, FONT.as_bytes().to_vec()),
        ],
        &(ALIAS_BASE..ALIAS_BASE + fields)
            .map(|alias| (alias, 7))
            .collect::<Vec<_>>(),
    );
    let opened = open_bytes("field-aliases", &bytes, Limits::default()).expect("one page");
    assert_eq!(opened.text, "Hello");
}

#[test]
fn distinct_fields_that_all_list_one_large_kid_read_it_once() {
    // Without aliases: every field lists the same large kid. It was read
    // again for each parent and the walk ran out of budget after seconds
    // (review round 3, R2); what a kid is, is now read once per object.
    let fields = 2_000u32;
    let mut objects = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R /AcroForm 7 0 R >>".to_vec()),
        (
            7,
            format!("<< /Fields [{}] >>", kids_from(ALIAS_BASE, fields)).into_bytes(),
        ),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
                .to_vec(),
        ),
        (4, hello_stream()),
        (5, FONT.as_bytes().to_vec()),
        (8, format!("<< /Junk [{}] >>", "0 ".repeat(200_000)).into_bytes()),
    ];
    for field in 0..fields {
        objects.push((ALIAS_BASE + field, b"<< /T (f) /Kids [8 0 R] >>".to_vec()));
    }
    let bytes = aliased_pdf(&objects, &[]);
    let opened = open_bytes("field-kids", &bytes, small_limits(MIB)).expect("a page and no field");
    assert_eq!(opened.text, "Hello");
}

#[test]
fn cross_reference_sections_nested_in_a_trailer_string_are_refused() {
    // Each empty section's trailer opens a string that holds every later
    // section, so section i lexes the rest of the file: 20 000 levels in
    // 820 KB lexed about 8 GB.
    let levels = 20_000usize;
    let head = b"%PDF-1.4\n";
    let base = head.len();
    let step = b"xref\ntrailer\n<< /Prev 0000000000 /S (".len();
    let mut bytes = head.to_vec();
    for level in 0..levels {
        if level + 1 < levels {
            bytes.extend_from_slice(
                format!(
                    "xref\ntrailer\n<< /Prev {:010} /S (",
                    base + (level + 1) * step
                )
                .as_bytes(),
            );
        } else {
            bytes.extend_from_slice(b"xref\ntrailer\n<< /Size 1 >>");
        }
    }
    for _ in 1..levels {
        bytes.extend_from_slice(b") >>");
    }
    bytes.extend_from_slice(format!("\nstartxref\n{base}\n%%EOF\n").as_bytes());
    assert!(bytes.len() < MIB);
    assert_too_large(
        "xref-strings",
        open_bytes("xref-strings", &bytes, Limits::default()),
    );
}

// ---------------------------------------------------------------------------
// Third review round: the same large object named many times from inside an
// object stream, whose header may repeat one offset under any number of
// entries.
// ---------------------------------------------------------------------------

/// A PDF 1.5 file whose object stream 10 has the raw `header` and `body`,
/// claims `n` objects, and whose cross-reference stream sends each
/// `(number, index)` of `packed` into it; `top` objects sit in the file.
fn objstm_raw_pdf(
    top: &[(u32, Vec<u8>)],
    header: &[u8],
    body: &[u8],
    n: usize,
    packed: &[(u32, u32)],
) -> Vec<u8> {
    let mut content = header.to_vec();
    content.extend_from_slice(body);
    let data = zlib(&content);
    let container = 10u32;
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut rows: std::collections::BTreeMap<u32, (u8, u32, u16)> =
        std::collections::BTreeMap::new();
    let mut objects: Vec<(u32, Vec<u8>)> = top.to_vec();
    objects.push((
        container,
        stream_object(
            &format!(
                "/Type /ObjStm /N {n} /First {} /Filter /FlateDecode /Length {}",
                header.len(),
                data.len()
            ),
            &data,
        ),
    ));
    objects.sort_by_key(|(number, _)| *number);
    for (number, body) in &objects {
        rows.insert(*number, (1, u32::try_from(out.len()).expect("small"), 0));
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    for (number, index) in packed {
        rows.insert(
            *number,
            (2, container, u16::try_from(*index).unwrap_or(u16::MAX)),
        );
    }
    let xref_number = rows.keys().max().copied().unwrap_or(0) + 1;
    let xref_offset = u32::try_from(out.len()).expect("small");
    rows.insert(xref_number, (1, xref_offset, 0));
    let mut table = Vec::new();
    for number in 0..=xref_number {
        let (kind, field, index) = rows.get(&number).copied().unwrap_or((0, 0, 0));
        table.push(kind);
        table.extend_from_slice(&field.to_be_bytes());
        table.extend_from_slice(&index.to_be_bytes());
    }
    let packed_table = zlib(&table);
    out.extend_from_slice(format!("{xref_number} 0 obj\n").as_bytes());
    out.extend_from_slice(&stream_object(
        &format!(
            "/Type /XRef /Size {} /W [1 4 2] /Root 1 0 R /Filter /FlateDecode /Length {}",
            xref_number + 1,
            packed_table.len()
        ),
        &packed_table,
    ));
    out.extend_from_slice(format!("\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes());
    out
}

fn one_page_top() -> Vec<(u32, Vec<u8>)> {
    vec![
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            b"<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
                .to_vec(),
        ),
        (4, hello_stream()),
        (5, FONT.as_bytes().to_vec()),
    ]
}

#[test]
fn an_object_stream_header_that_repeats_one_offset_keeps_one_copy() {
    // One large catalogue named by 1 000 and by 100 000 header entries at the
    // same offset: each entry kept its own copy, 4.7 GB from 888 bytes, and
    // an abort at 100 000.
    for entries in [1_000usize, 100_000] {
        let body = format!(
            "<< /Type /Catalog /Pages 2 0 R /Junk [{}] >>",
            "0 ".repeat(100_000)
        );
        let bytes = objstm_raw_pdf(
            &one_page_top(),
            "1 0 ".repeat(entries).as_bytes(),
            body.as_bytes(),
            entries,
            &[(1, 0)],
        );
        assert!(bytes.len() < 4 * 1024);
        let started = Instant::now();
        let opened = open_bytes("objstm-repeat", &bytes, Limits::default()).expect("one page");
        assert_eq!(opened.text, "Hello");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn pages_named_at_one_object_stream_offset_are_refused() {
    // 3 000 page numbers at distinct header entries that all name one
    // offset: the page tree's alias rule never saw them, and the page was
    // held 3 000 times.
    let pages = 3_000u32;
    let body = format!(
        "<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Junk [{}] >>",
        "0 ".repeat(100_000)
    );
    let top = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (
            2,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                kids_from(ALIAS_BASE, pages)
            )
            .into_bytes(),
        ),
        (4, hello_stream()),
        (5, FONT.as_bytes().to_vec()),
    ];
    let packed: Vec<(u32, u32)> = (0..pages)
        .map(|index| (ALIAS_BASE + index, index))
        .collect();
    let bytes = objstm_raw_pdf(
        &top,
        "1 0 ".repeat(pages as usize).as_bytes(),
        body.as_bytes(),
        pages as usize,
        &packed,
    );
    assert_not_importable(
        "objstm-pages",
        open_bytes("objstm-pages", &bytes, Limits::default()),
    );
}

#[test]
fn many_form_roots_are_not_all_held_at_once() {
    // Reading every root up front held them all (the review's 5 000 roots
    // went from 38 MB to 415 MB). They open, each read in its turn; this
    // checks the behaviour, the memory figure is in the evidence.
    let fields = 2_000u32;
    let mut objects = vec![
        (
            1,
            b"<< /Type /Catalog /Pages 2 0 R /AcroForm 7 0 R >>".to_vec(),
        ),
        (
            7,
            format!("<< /Fields [{}] >>", kids_from(ALIAS_BASE, fields)).into_bytes(),
        ),
    ];
    objects.extend(one_page_top());
    for field in 0..fields {
        objects.push((
            ALIAS_BASE + field,
            format!(
                "<< /T (f{field}) /FT /Tx /V (v) /Junk [{}] >>",
                "0 ".repeat(1_000)
            )
            .into_bytes(),
        ));
    }
    let bytes = aliased_pdf(&objects, &[]);
    let opened = open_bytes("form-roots", &bytes, Limits::default()).expect("a form");
    assert!(opened.text.starts_with("Hello"));
    assert_eq!(opened.text.matches(": v").count(), fields as usize);
}

// ---------------------------------------------------------------------------
// Fourth review round: a million objects in object streams, missing or tiny,
// named from `/Fields`.
// ---------------------------------------------------------------------------

/// A PDF 1.5 file of `top` objects at file offsets, plus `packed` rows
/// `(number, container, index)` that send objects into object streams, all
/// located by one compressed cross-reference stream.
fn xref_stream_pdf(top: &[(u32, Vec<u8>)], packed: &[(u32, u32, u16)]) -> Vec<u8> {
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut rows: std::collections::BTreeMap<u32, (u8, u32, u16)> =
        std::collections::BTreeMap::new();
    for (number, body) in top {
        rows.insert(*number, (1, u32::try_from(out.len()).expect("small"), 0));
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    for (number, container, index) in packed {
        rows.insert(*number, (2, *container, *index));
    }
    let xref_number = rows.keys().max().copied().unwrap_or(0) + 1;
    let xref_offset = u32::try_from(out.len()).expect("small");
    rows.insert(xref_number, (1, xref_offset, 0));
    let mut table = Vec::new();
    for number in 0..=xref_number {
        let (kind, field, index) = rows.get(&number).copied().unwrap_or((0, 0, 0));
        table.push(kind);
        table.extend_from_slice(&field.to_be_bytes());
        table.extend_from_slice(&index.to_be_bytes());
    }
    let table = zlib(&table);
    out.extend_from_slice(format!("{xref_number} 0 obj\n").as_bytes());
    out.extend_from_slice(&stream_object(
        &format!(
            "/Type /XRef /Size {} /W [1 4 2] /Root 1 0 R /Filter /FlateDecode /Length {}",
            xref_number + 1,
            table.len()
        ),
        &table,
    ));
    out.extend_from_slice(format!("\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes());
    out
}

fn form_top(fields: u32) -> Vec<(u32, Vec<u8>)> {
    let mut top = vec![
        (
            1,
            b"<< /Type /Catalog /Pages 2 0 R /AcroForm 7 0 R >>".to_vec(),
        ),
        (
            7,
            format!("<< /Fields [{}] >>", kids_from(100, fields)).into_bytes(),
        ),
    ];
    top.extend(one_page_top());
    top.sort_by_key(|(number, _)| *number);
    top
}

#[test]
fn fields_in_object_streams_the_file_does_not_have_are_refused_promptly() {
    // 100 000 fields, each in a distinct object stream that is not in the
    // file: every root added a remembered failure, and counting the streams
    // in progress walked them all, 41 s for 100 000 and minutes for a
    // million, with no charge and no cancellation.
    let fields = 100_000u32;
    let packed: Vec<(u32, u32, u16)> = (0..fields)
        .map(|field| (100 + field, 5_000_000 + field, 0))
        .collect();
    let bytes = xref_stream_pdf(&form_top(fields), &packed);
    assert_not_importable(
        "missing-containers",
        open_bytes("missing-containers", &bytes, Limits::default()),
    );
}

#[test]
fn many_small_object_streams_open_in_linear_time() {
    // 100 000 fields, each alone in a small object stream of its own: 88 s
    // before, for 14 MB.
    let fields = 100_000u32;
    let mut top = form_top(fields);
    let mut packed = Vec::new();
    for field in 0..fields {
        let container = 1_000_000 + field;
        let header = format!("{} 0 ", 100 + field);
        let content = format!("{header}<< /T (f{field}) /FT /Tx /V (v) >>\n");
        top.push((
            container,
            stream_object(
                &format!(
                    "/Type /ObjStm /N 1 /First {} /Length {}",
                    header.len(),
                    content.len()
                ),
                content.as_bytes(),
            ),
        ));
        packed.push((100 + field, container, 0));
    }
    let bytes = xref_stream_pdf(&top, &packed);
    let started = Instant::now();
    let opened = open_bytes("many-small", &bytes, Limits::default()).expect("a form");
    assert_eq!(opened.text.matches(": v").count(), fields as usize);
    assert!(started.elapsed() < PROMPT, "{:?}", started.elapsed());
}

// ---------------------------------------------------------------------------
// Fifth review round: an object stream that fails on its own is remembered as
// failed, not unpacked again by every lookup that reaches it.
// ---------------------------------------------------------------------------

#[test]
fn an_object_stream_that_fails_on_its_own_is_unpacked_once() {
    // Object 900 is packed in object stream 800, whose two Flate stages
    // unpack past the ceiling. Every page's content stream takes its
    // `/Length` from object 900, so each page asks for that stream again.
    let pages = 500u32;
    let packed_stream = zlib(&zlib_bomb(4));
    let mut top = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (
            2,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                (0..pages)
                    .map(|page| format!("{} 0 R", 10_000 + 2 * page))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .into_bytes(),
        ),
        (5, FONT.as_bytes().to_vec()),
        (
            800,
            stream_object(
                &format!(
                    "/Type /ObjStm /N 1 /First 6 /Filter [/FlateDecode /FlateDecode] /Length {}",
                    packed_stream.len()
                ),
                &packed_stream,
            ),
        ),
    ];
    let content = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    for page in 0..pages {
        let number = 10_000 + 2 * page;
        top.push((
            number,
            format!(
                "<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
                number + 1
            )
            .into_bytes(),
        ));
        top.push((number + 1, stream_object("/Length 900 0 R", content)));
    }
    let bytes = xref_stream_pdf(&top, &[(900, 800, 0)]);
    assert!(bytes.len() < 256 * 1024);

    // Under a 2 MiB ceiling the object stream fails once, and each page falls
    // back to its `endstream` keyword for the length.
    let started = Instant::now();
    let opened = open_bytes("failed-objstm", &bytes, small_limits(2 * MIB)).expect("500 pages");
    let elapsed = started.elapsed();
    assert_eq!(opened.text.matches("Hello").count(), pages as usize);
    // Unpacking it once costs a few hundred milliseconds in a debug build;
    // once per page (round 4) costs 500 times that.
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
}
