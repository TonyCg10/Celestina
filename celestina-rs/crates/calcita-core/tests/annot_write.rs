// language-contract: product-copy
//! Writing edits as incremental updates: the original bytes stay a prefix,
//! the file reads back as the edits say, a second save appends again, and
//! the copy shown while editing hides exactly the editable annotations.

use std::path::{Path, PathBuf};

use calcita_core::annot::coords::{to_view_rect, PageBox, Rotation};
use calcita_core::annot::edit::{self, Base, Edit, EditSet};
use calcita_core::annot::write::{apply, apply_to_bytes, hidden_copy};
use calcita_core::annot::{
    read_annotations, AnnotError, AnnotId, Annotation, Kind, Point, Rect, Rgb,
};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{dictionary, Document, Object, SaveOptions};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../calcita/tests/fixtures")
        .join(name)
}

/// A scratch directory that removes itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("calcita-core-write-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("readable")
}

/// `out` begins with every byte of `original` and adds more.
fn assert_prefix(original: &Path, out: &Path) {
    let (original, out) = (bytes(original), bytes(out));
    assert!(out.len() > original.len(), "the update adds bytes");
    assert!(
        out[..original.len()] == original[..],
        "the original bytes are a prefix of the result"
    );
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

fn same_rect(a: Rect, b: Rect) -> bool {
    close(a.x0, b.x0) && close(a.y0, b.y0) && close(a.x1, b.x1) && close(a.y1, b.y1)
}

fn page_box(path: &Path, page: usize) -> PageBox {
    Base::read(path).expect("readable").boxes[page]
}

/// The one annotation `lopdf` finds at `id` in `path`, as a dictionary.
fn dictionary_of(path: &Path, id: &AnnotId) -> lopdf::Dictionary {
    let AnnotId::Object { number, generation } = id else {
        panic!("an indirect annotation: {id:?}");
    };
    let document = Document::load(path).expect("lopdf reads the result");
    document
        .get_dictionary((*number, *generation))
        .expect("the annotation")
        .clone()
}

/// The appearance stream an annotation dictionary names.
fn appearance_of(path: &Path, dictionary: &lopdf::Dictionary) -> lopdf::Stream {
    let document = Document::load(path).expect("lopdf reads the result");
    let normal = dictionary
        .get(b"AP")
        .and_then(Object::as_dict)
        .and_then(|ap| ap.get(b"N"))
        .and_then(Object::as_reference)
        .expect("an /AP /N stream");
    document
        .get_object(normal)
        .and_then(Object::as_stream)
        .expect("a stream")
        .clone()
}

fn only_added(path: &Path, before: usize) -> Annotation {
    let annotations = read_annotations(path).expect("rereads");
    assert_eq!(annotations.len(), before + 1);
    annotations.last().expect("one more").clone()
}

#[test]
fn a_text_box_is_appended_and_reads_back_as_written() {
    let scratch = Scratch::new("text");
    let original = fixture("three-pages.pdf");
    let out = scratch.path("edited.pdf");
    let page = page_box(&original, 1);
    let base = Base::read(&original).expect("readable");
    let added = edit::text_box(
        page,
        1,
        Rect::new(72.0, 100.0, 272.0, 110.0),
        "¿Qué tal, señora? Cuesta 5 €.",
        14.0,
        BLUE,
    );
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(added)).expect("added");
    let expected = edits.apply(&base)[0].clone();
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = only_added(&out, 0);
    assert_eq!(reread.page, 1);
    assert_eq!(
        reread.kind,
        Kind::Text {
            contents: "¿Qué tal, señora? Cuesta 5 €.".to_owned(),
            size_pt: 14.0,
            rich: false
        }
    );
    assert_eq!(reread.color, BLUE, "from /DA");
    assert!(same_rect(reread.rect, expected.rect), "{reread:?}");
    assert!(
        matches!(reread.id, AnnotId::Object { .. }),
        "an indirect object"
    );

    let dictionary = dictionary_of(&out, &reread.id);
    let AnnotId::Name(name) = &expected.id else {
        panic!("a new annotation has a name");
    };
    assert_eq!(
        dictionary.get(b"NM").and_then(Object::as_str).expect("/NM"),
        name.as_bytes()
    );
    assert_eq!(
        dictionary.get(b"DA").and_then(Object::as_str).expect("/DA"),
        b"/Helv 14 Tf 0 0 1 rg"
    );
    assert_eq!(
        dictionary.get(b"F").and_then(Object::as_i64).expect("/F"),
        4
    );
    let appearance = appearance_of(&out, &dictionary);
    let content = String::from_utf8(appearance.content.clone()).expect("ASCII");
    assert!(content.contains("/Helv 14 Tf 0 0 1 rg"), "{content}");
    let document = Document::load(&out).expect("lopdf reads the result");
    let font = appearance
        .dict
        .get(b"Resources")
        .and_then(Object::as_dict)
        .and_then(|resources| resources.get(b"Font"))
        .and_then(Object::as_dict)
        .and_then(|fonts| fonts.get(b"Helv"))
        .and_then(Object::as_reference)
        .and_then(|font| document.get_dictionary(font))
        .expect("Helvetica in the resources");
    assert_eq!(
        font.get(b"BaseFont")
            .and_then(Object::as_name)
            .expect("name"),
        b"Helvetica"
    );
    assert_eq!(
        font.get(b"Encoding")
            .and_then(Object::as_name)
            .expect("name"),
        b"WinAnsiEncoding"
    );
}

#[test]
fn a_highlight_is_appended_with_its_quads_and_a_multiply_appearance() {
    let scratch = Scratch::new("highlight");
    let original = fixture("three-pages.pdf");
    let out = scratch.path("edited.pdf");
    let base = Base::read(&original).expect("readable");
    let page = base.boxes[0];
    let made = edit::highlight(
        page,
        0,
        &[
            Rect::new(72.0, 72.0, 300.0, 86.0),
            Rect::new(72.0, 90.0, 200.0, 104.0),
        ],
        Rgb {
            r: 243,
            g: 207,
            b: 142,
        },
    )
    .expect("boxes");
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(made.clone())).expect("added");
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = only_added(&out, 0);
    let (Kind::Highlight { quads }, Kind::Highlight { quads: written }) =
        (&reread.kind, &made.kind)
    else {
        panic!("{reread:?}");
    };
    assert_eq!(quads.len(), 2);
    for (read, made) in quads.iter().zip(written) {
        for (a, b) in read.points.iter().zip(made.points.iter()) {
            assert!(close(a.x, b.x) && close(a.y, b.y), "{a:?} {b:?}");
        }
    }
    assert_eq!(
        reread.color,
        Rgb {
            r: 243,
            g: 207,
            b: 142
        }
    );
    let appearance = appearance_of(&out, &dictionary_of(&out, &reread.id));
    let blend = appearance
        .dict
        .get(b"Resources")
        .and_then(Object::as_dict)
        .and_then(|resources| resources.get(b"ExtGState"))
        .and_then(Object::as_dict)
        .and_then(|states| states.get(b"Multiply"))
        .and_then(Object::as_dict)
        .and_then(|state| state.get(b"BM"))
        .and_then(Object::as_name)
        .expect("a blend mode");
    assert_eq!(blend, b"Multiply");
}

#[test]
fn a_drawing_is_appended_with_its_stroke_and_width() {
    let scratch = Scratch::new("ink");
    let original = fixture("three-pages.pdf");
    let out = scratch.path("edited.pdf");
    let base = Base::read(&original).expect("readable");
    let points = [
        Point { x: 100.0, y: 100.0 },
        Point { x: 150.0, y: 140.0 },
        Point { x: 200.0, y: 100.0 },
    ];
    let made =
        edit::ink(base.boxes[2], 2, &points, 3.0, Rgb { r: 255, g: 0, b: 0 }).expect("points");
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(made.clone())).expect("added");
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = only_added(&out, 0);
    assert_eq!(reread.page, 2);
    let (Kind::Ink { strokes, width }, Kind::Ink { strokes: drawn, .. }) =
        (&reread.kind, &made.kind)
    else {
        panic!("{reread:?}");
    };
    assert_eq!(*width, 3.0);
    assert_eq!(strokes.len(), 1);
    assert_eq!(strokes[0].len(), 3);
    for (a, b) in strokes[0].iter().zip(&drawn[0]) {
        assert!(close(a.x, b.x) && close(a.y, b.y));
    }
    assert!(same_rect(reread.rect, made.rect));
}

#[test]
fn a_second_save_appends_again_and_keeps_the_first() {
    let scratch = Scratch::new("twice");
    let original = fixture("three-pages.pdf");
    let first = scratch.path("first.pdf");
    let second = scratch.path("second.pdf");
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    let page = base.boxes[0];
    edits
        .push(
            &base,
            Edit::Add(edit::text_box(
                page,
                0,
                Rect::new(72.0, 72.0, 272.0, 80.0),
                "uno",
                12.0,
                BLUE,
            )),
        )
        .expect("added");
    apply(&original, &edits, &first).expect("written");

    let base = Base::read(&first).expect("rereads");
    let written = base.annotations[0].id.clone();
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::SetText(written.clone(), "dos".to_owned()))
        .expect("retexted");
    edits
        .push(
            &base,
            Edit::Add(edit::text_box(
                page,
                0,
                Rect::new(72.0, 200.0, 272.0, 210.0),
                "tres",
                12.0,
                BLUE,
            )),
        )
        .expect("added");
    apply(&first, &edits, &second).expect("written again");

    assert_prefix(&original, &second);
    assert_prefix(&first, &second);
    let annotations = read_annotations(&second).expect("rereads");
    let texts: Vec<&str> = annotations
        .iter()
        .map(|annotation| match &annotation.kind {
            Kind::Text { contents, .. } => contents.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(texts, vec!["dos", "tres"]);
    assert_eq!(annotations[0].id, written, "rewritten in place");
    // The update's trailer points back at the first one's.
    let tail = String::from_utf8_lossy(&bytes(&second)[bytes(&first).len()..]).into_owned();
    assert!(tail.contains("/Prev"), "{tail}");
}

#[test]
fn another_programs_annotations_move_recolour_retext_and_go() {
    let scratch = Scratch::new("others");
    let original = fixture("annotated.pdf");
    let out = scratch.path("edited.pdf");
    let base = Base::read(&original).expect("readable");
    let [text, highlight, ink, link] = [0, 1, 2, 3].map(|index| base.annotations[index].id.clone());
    let mut edits = EditSet::new();
    // The text box only carried 20 points down: its own appearance stays.
    edits
        .push(
            &base,
            Edit::Move(text.clone(), Rect::new(300.0, 680.0, 520.0, 740.0)),
        )
        .expect("moved");
    edits
        .push(
            &base,
            Edit::Move(highlight.clone(), Rect::new(80.0, 742.0, 300.0, 776.0)),
        )
        .expect("moved");
    edits
        .push(&base, Edit::Recolor(highlight.clone(), BLUE))
        .expect("recoloured");
    edits
        .push(&base, Edit::Delete(ink.clone()))
        .expect("deleted");
    let expected = edits.apply(&base);
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = read_annotations(&out).expect("rereads");
    assert_eq!(reread.len(), 3, "the drawing is gone, the link stays");
    assert!(reread.iter().any(|annotation| annotation.id == link));
    assert!(reread.iter().all(|annotation| annotation.id != ink));
    for (read, wanted) in reread.iter().zip(expected.iter()) {
        assert_eq!(read.id, wanted.id);
        assert!(same_rect(read.rect, wanted.rect), "{read:?} {wanted:?}");
        assert_eq!(read.color, wanted.color);
    }
    let before = dictionary_of(&original, &text);
    let after = dictionary_of(&out, &text);
    assert_eq!(
        before.get(b"AP").ok(),
        after.get(b"AP").ok(),
        "the moved text keeps its look"
    );
    let before = dictionary_of(&original, &highlight);
    let after = dictionary_of(&out, &highlight);
    assert_ne!(
        before.get(b"AP").ok(),
        after.get(b"AP").ok(),
        "a recoloured highlight is redrawn"
    );

    let base = Base::read(&out).expect("rereads");
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::SetText(text.clone(), "Reescrito".to_owned()))
        .expect("plain text is rewritten");
    let again = scratch.path("again.pdf");
    apply(&out, &edits, &again).expect("written");
    let text_now = read_annotations(&again).expect("rereads").remove(0);
    assert!(matches!(&text_now.kind, Kind::Text { contents, .. } if contents == "Reescrito"));
    assert_eq!(
        text_now.color,
        Rgb { r: 0, g: 0, b: 153 },
        "the colour stays"
    );
}

/// A one-page document whose `/Annots` holds inline dictionaries: a FreeText
/// with `/NM`, a highlight without, then a link.
fn inline_document(path: &Path) {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let page = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Annots" => vec![
            Object::Dictionary(dictionary! {
                "Type" => "Annot", "Subtype" => "FreeText",
                "Rect" => vec![100.into(), 700.into(), 300.into(), 730.into()],
                "Contents" => Object::string_literal("inline"),
                "DA" => Object::string_literal("/Helv 12 Tf 0 g"),
                "NM" => Object::string_literal("inline-text"),
            }),
            Object::Dictionary(dictionary! {
                "Type" => "Annot", "Subtype" => "Highlight",
                "Rect" => vec![100.into(), 600.into(), 300.into(), 620.into()],
                "QuadPoints" => vec![100.into(), 620.into(), 300.into(), 620.into(), 100.into(), 600.into(), 300.into(), 600.into()],
            }),
            Object::Dictionary(dictionary! {
                "Type" => "Annot", "Subtype" => "Link",
                "Rect" => vec![100.into(), 500.into(), 300.into(), 520.into()],
            }),
        ],
    });
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => Object::Reference(pages),
    });
    document.trailer.set("Root", Object::Reference(catalog));
    document.save(path).expect("saved");
}

#[test]
fn inline_annotations_are_rewritten_and_removed_in_place() {
    let scratch = Scratch::new("inline");
    let original = scratch.path("inline.pdf");
    inline_document(&original);
    let base = Base::read(&original).expect("readable");
    assert_eq!(
        base.annotations[0].id,
        AnnotId::Name("inline-text".to_owned())
    );
    assert_eq!(
        base.annotations[1].id,
        AnnotId::Position { page: 0, index: 1 }
    );
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::SetText(
                AnnotId::Name("inline-text".to_owned()),
                "cambiado".to_owned(),
            ),
        )
        .expect("retexted");
    edits
        .push(&base, Edit::Delete(AnnotId::Position { page: 0, index: 1 }))
        .expect("deleted");
    let out = scratch.path("edited.pdf");
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = read_annotations(&out).expect("rereads");
    assert_eq!(reread.len(), 2);
    assert_eq!(reread[0].id, AnnotId::Name("inline-text".to_owned()));
    assert!(matches!(&reread[0].kind, Kind::Text { contents, .. } if contents == "cambiado"));
    assert!(matches!(&reread[1].kind, Kind::Other { subtype } if subtype == "Link"));
}

#[test]
fn a_page_inside_an_object_stream_takes_an_update() {
    let scratch = Scratch::new("objstm");
    let original = scratch.path("modern.pdf");
    let mut document = Document::load(fixture("annotated.pdf")).expect("the fixture");
    let mut file = std::fs::File::create(&original).expect("created");
    document
        .save_with_options(
            &mut file,
            SaveOptions::builder()
                .use_object_streams(true)
                .use_xref_streams(true)
                .build(),
        )
        .expect("saved");
    drop(file);
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    let added = edit::text_box(
        base.boxes[0],
        0,
        Rect::new(72.0, 72.0, 272.0, 80.0),
        "nuevo",
        12.0,
        BLUE,
    );
    edits.push(&base, Edit::Add(added)).expect("added");
    edits
        .push(&base, Edit::Delete(base.annotations[2].id.clone()))
        .expect("deleted");
    let out = scratch.path("edited.pdf");
    apply(&original, &edits, &out).expect("written");

    assert_prefix(&original, &out);
    let reread = read_annotations(&out).expect("rereads");
    assert_eq!(reread.len(), 4, "four, one gone, one new");
    assert!(matches!(&reread[3].kind, Kind::Text { contents, .. } if contents == "nuevo"));
}

#[test]
fn a_text_box_on_a_turned_page_reads_back_where_it_was_placed() {
    let scratch = Scratch::new("turned");
    let original = fixture("rotated.pdf");
    let base = Base::read(&original).expect("readable");
    let page = base.boxes[1];
    assert_eq!(page.rotate, Rotation::R90);
    let view = Rect::new(60.0, 40.0, 260.0, 50.0);
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Add(edit::text_box(page, 1, view, "girado", 12.0, BLUE)),
        )
        .expect("added");
    let out = scratch.path("edited.pdf");
    apply(&original, &edits, &out).expect("written");
    let reread = read_annotations(&out).expect("rereads");
    let added = reread
        .iter()
        .find(|a| matches!(&a.kind, Kind::Text { contents, .. } if contents == "girado"))
        .expect("there");
    let shown = to_view_rect(page, added.rect);
    assert!(
        close(shown.x0, 60.0) && close(shown.y0, 40.0) && close(shown.x1, 260.0),
        "{shown:?}"
    );
}

#[test]
fn nothing_but_text_win_ansi_can_show_is_written() {
    let base = Base::read(&fixture("three-pages.pdf")).expect("readable");
    let mut edits = EditSet::new();
    let added = edit::text_box(
        base.boxes[0],
        0,
        Rect::new(72.0, 72.0, 272.0, 80.0),
        "ok",
        12.0,
        BLUE,
    );
    let id = added.id.clone();
    edits.push(&base, Edit::Add(added)).expect("added");
    assert!(matches!(
        edits.push(&base, Edit::SetText(id, "ok ✓".to_owned())),
        Err(AnnotError::Unencodable('✓'))
    ));
    let written = apply_to_bytes(&fixture("three-pages.pdf"), &edits).expect("written");
    assert!(!written.is_empty());
}

fn encrypted(path: &Path) {
    let mut document = Document::load(fixture("three-pages.pdf")).expect("the fixture");
    document.trailer.set(
        "ID",
        Object::Array(vec![
            Object::string_literal("calcita-fixture-id"),
            Object::string_literal("calcita-fixture-id"),
        ]),
    );
    // An empty user password: lopdf opens it, and it is still refused.
    let version = EncryptionVersion::V1 {
        document: &document,
        owner_password: "owner",
        user_password: "",
        permissions: Permissions::all(),
    };
    let state = EncryptionState::try_from(version).expect("an RC4 state");
    document.encrypt(&state).expect("encrypted");
    document.save(path).expect("saved");
}

#[test]
fn an_encrypted_document_is_never_written() {
    let scratch = Scratch::new("encrypted");
    let original = scratch.path("locked.pdf");
    encrypted(&original);
    let out = scratch.path("out.pdf");
    let error = apply(&original, &EditSet::new(), &out).expect_err("refused");
    assert!(matches!(error, AnnotError::Encrypted), "{error:?}");
    assert!(!out.exists(), "nothing is written");
    assert!(matches!(
        hidden_copy(&original, &[]),
        Err(AnnotError::Encrypted)
    ));
}

#[test]
fn an_existing_file_is_never_replaced() {
    let scratch = Scratch::new("exists");
    let out = scratch.path("taken.pdf");
    std::fs::write(&out, b"mine").expect("written");
    let error = apply(&fixture("three-pages.pdf"), &EditSet::new(), &out).expect_err("refused");
    assert!(matches!(error, AnnotError::Write(_)), "{error:?}");
    assert_eq!(bytes(&out), b"mine");
}

#[test]
fn the_copy_shown_while_editing_hides_only_the_editable_annotations() {
    let original = fixture("annotated.pdf");
    let base = Base::read(&original).expect("readable");
    let copy = hidden_copy(&original, &base.editable_ids()).expect("a copy");
    let source = bytes(&original);
    assert!(copy.len() > source.len() && copy[..source.len()] == source[..]);
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let flags: Vec<i64> = [6, 8, 10, 12]
        .map(|number| {
            document
                .get_dictionary((number, 0))
                .ok()
                .and_then(|annotation| annotation.get(b"F").ok())
                .and_then(|flags| flags.as_i64().ok())
                .unwrap_or(0)
        })
        .to_vec();
    assert_eq!(
        flags.iter().map(|flags| flags & 2).collect::<Vec<_>>(),
        vec![2, 2, 2, 0]
    );
    assert_eq!(
        hidden_copy(&fixture("three-pages.pdf"), &[]).expect("a copy"),
        bytes(&fixture("three-pages.pdf")),
        "nothing to hide, nothing appended"
    );
}

/// A stream object holding `content` as an appearance in `bbox`.
fn appearance_stream(content: &[u8]) -> lopdf::Stream {
    lopdf::Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 100.into(), 30.into()],
        },
        content.to_vec(),
    )
}

fn utf16(text: &str) -> Object {
    let mut bytes = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

/// A one-page document as another program might write it: a rich-text
/// FreeText (6), a tight FreeText whose text is outside WinAnsi (8), a
/// highlight whose corners run counter-clockwise (9), a drawing (10) with a
/// reply (11, its pop-up 12) and a reply to the reply (13), and an
/// unrelated note (14).
fn others_document(path: &Path) {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id(); // 1
    let page = document.new_object_id(); // 2
    let font = document.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    ); // 3
    let rich_ap = document.add_object(appearance_stream(b"BT /F 12 Tf 2 10 Td (rich) Tj ET")); // 4
    let tight_ap = document.add_object(appearance_stream(b"BT /F 12 Tf 2 3 Td (arrows) Tj ET")); // 5
    let rich = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText",
        "Rect" => vec![100.into(), 700.into(), 200.into(), 730.into()],
        "Contents" => Object::string_literal("rich"),
        "RC" => Object::string_literal("<body><p style=\"color:#0000ff\">rich</p></body>"),
        "DA" => Object::string_literal("/Helv 12 Tf 0 0 1 rg"),
        "AP" => dictionary! { "N" => Object::Reference(rich_ap) },
    }); // 6
    let tight_ap_2 = document.add_object(appearance_stream(b"q Q")); // 7
    let tight = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText",
        "Rect" => vec![100.into(), 600.into(), 160.into(), 615.into()],
        "Contents" => utf16("\u{2192} \u{2264} \u{3b1}\u{3b2}"),
        "DA" => Object::string_literal("/Helv 12 Tf 0 g"),
        "AP" => dictionary! { "N" => Object::Reference(tight_ap) },
    }); // 8
    let _ = tight_ap_2;
    let ccw = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Highlight",
        "Rect" => vec![70.into(), 500.into(), 290.into(), 534.into()],
        // Lower left, lower right, upper right, upper left.
        "QuadPoints" => vec![70.into(), 500.into(), 290.into(), 500.into(), 290.into(), 534.into(), 70.into(), 534.into()],
    }); // 9
    let ink = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Ink",
        "Rect" => vec![300.into(), 300.into(), 400.into(), 400.into()],
        "InkList" => vec![Object::Array(vec![310.into(), 310.into(), 390.into(), 390.into()])],
    }); // 10
    let reply_id = document.new_object_id(); // 11
    let popup = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Popup",
        "Rect" => vec![400.into(), 400.into(), 500.into(), 450.into()],
        "Parent" => Object::Reference(reply_id),
    }); // 12
    document.objects.insert(
        reply_id,
        Object::Dictionary(dictionary! {
            "Type" => "Annot", "Subtype" => "Text",
            "Rect" => vec![400.into(), 380.into(), 420.into(), 400.into()],
            "IRT" => Object::Reference(ink),
            "Popup" => Object::Reference(popup),
            "Contents" => Object::string_literal("a reply"),
        }),
    );
    let second = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Text",
        "Rect" => vec![420.into(), 380.into(), 440.into(), 400.into()],
        "IRT" => Object::Reference(reply_id),
        "Contents" => Object::string_literal("a reply to the reply"),
    }); // 13
    let note = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Text",
        "Rect" => vec![50.into(), 50.into(), 70.into(), 70.into()],
        "Contents" => Object::string_literal("unrelated"),
    }); // 14
    document.objects.insert(
        page,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages),
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F" => Object::Reference(font) } },
            "Annots" => vec![rich, tight, ccw, ink, reply_id, popup, second, note]
                .into_iter().map(Object::Reference).collect::<Vec<_>>(),
        }),
    );
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => Object::Reference(pages),
    });
    document.trailer.set("Root", Object::Reference(catalog));
    document.save(path).expect("saved");
}

fn object_id(number: u32) -> AnnotId {
    AnnotId::Object {
        number,
        generation: 0,
    }
}

fn stream_bytes(path: &Path, dictionary: &lopdf::Dictionary) -> Vec<u8> {
    appearance_of(path, dictionary).content
}

#[test]
fn a_moved_rich_text_box_keeps_its_text_style_and_appearance() {
    let scratch = Scratch::new("rich-move");
    let original = scratch.path("others.pdf");
    others_document(&original);
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Move(object_id(6), Rect::new(110.0, 680.0, 210.0, 710.0)),
        )
        .expect("a rich box moves");
    let out = scratch.path("moved.pdf");
    apply(&original, &edits, &out).expect("written");
    assert_prefix(&original, &out);
    let (before, after) = (
        dictionary_of(&original, &object_id(6)),
        dictionary_of(&out, &object_id(6)),
    );
    for key in [b"RC".as_slice(), b"DA", b"Contents", b"AP"] {
        assert_eq!(
            before.get(key).ok(),
            after.get(key).ok(),
            "{}",
            String::from_utf8_lossy(key)
        );
    }
    assert_eq!(
        stream_bytes(&original, &before),
        stream_bytes(&out, &after),
        "the same appearance"
    );
    let moved = read_annotations(&out).expect("rereads").remove(0);
    assert!(
        same_rect(moved.rect, Rect::new(110.0, 680.0, 210.0, 710.0)),
        "{moved:?}"
    );
}

#[test]
fn a_tight_box_moves_without_changing_size() {
    let scratch = Scratch::new("tight");
    let original = scratch.path("others.pdf");
    others_document(&original);
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    let target = Rect::new(150.0, 400.0, 210.0, 415.0);
    edits
        .push(&base, Edit::Move(object_id(8), target))
        .expect("moved");
    assert_eq!(edits.apply(&base)[1].rect, target, "not re-fitted");
    let out = scratch.path("moved.pdf");
    apply(&original, &edits, &out).expect("written");
    let moved = read_annotations(&out).expect("rereads").remove(1);
    assert!(same_rect(moved.rect, target), "{moved:?}");
}

#[test]
fn a_box_outside_win_ansi_moves_but_its_colour_is_refused_and_the_save_lands() {
    let scratch = Scratch::new("arrows");
    let original = scratch.path("others.pdf");
    others_document(&original);
    let base = Base::read(&original).expect("readable");
    let Kind::Text { contents, .. } = &base.annotations[1].kind else {
        panic!("a text box");
    };
    assert_eq!(contents, "\u{2192} \u{2264} \u{3b1}\u{3b2}");
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Move(object_id(8), Rect::new(110.0, 590.0, 170.0, 605.0)),
        )
        .expect("nothing is re-encoded on a move");
    let refused = edits
        .push(&base, Edit::Recolor(object_id(8), BLUE))
        .expect_err("it would be laid out again in WinAnsi");
    assert!(
        matches!(refused, AnnotError::Unencodable('\u{2192}')),
        "{refused:?}"
    );
    assert!(
        matches!(
            edits.push(&base, Edit::Recolor(object_id(6), BLUE)),
            Err(AnnotError::NotEditable)
        ),
        "rich text keeps its colour"
    );
    let out = scratch.path("moved.pdf");
    apply(&original, &edits, &out).expect("every accepted edit can be written");
    let (before, after) = (
        dictionary_of(&original, &object_id(8)),
        dictionary_of(&out, &object_id(8)),
    );
    assert_eq!(before.get(b"Contents").ok(), after.get(b"Contents").ok());
    assert_eq!(before.get(b"AP").ok(), after.get(b"AP").ok());
}

#[test]
fn counter_clockwise_quads_read_in_drawing_order() {
    let scratch = Scratch::new("ccw");
    let original = scratch.path("others.pdf");
    others_document(&original);
    let annotations = read_annotations(&original).expect("readable");
    let Kind::Highlight { quads } = &annotations[2].kind else {
        panic!("a highlight: {:?}", annotations[2]);
    };
    let corner = |x, y| Point { x, y };
    assert_eq!(
        quads[0].points,
        [
            corner(70.0, 500.0),
            corner(290.0, 500.0),
            corner(70.0, 534.0),
            corner(290.0, 534.0)
        ],
        "the outline 0, 1, 3, 2 goes round"
    );
    // Recoloured, it is redrawn as a box, not an hourglass.
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::Recolor(object_id(9), BLUE))
        .expect("recoloured");
    let out = scratch.path("recoloured.pdf");
    apply(&original, &edits, &out).expect("written");
    let content =
        String::from_utf8(stream_bytes(&out, &dictionary_of(&out, &object_id(9)))).expect("ASCII");
    assert!(
        content.contains("70 500 m 290 500 l 290 534 l 70 534 l h f"),
        "{content}"
    );
}

#[test]
fn deleting_an_annotation_takes_its_replies_and_their_pop_ups() {
    let scratch = Scratch::new("replies");
    let original = scratch.path("others.pdf");
    others_document(&original);
    let base = Base::read(&original).expect("readable");
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::Delete(object_id(10)))
        .expect("deleted");
    let out = scratch.path("deleted.pdf");
    apply(&original, &edits, &out).expect("written");
    let left: Vec<AnnotId> = read_annotations(&out)
        .expect("rereads")
        .into_iter()
        .map(|annotation| annotation.id)
        .collect();
    assert_eq!(
        left,
        vec![object_id(6), object_id(8), object_id(9), object_id(14)],
        "the drawing, its reply, the reply's pop-up and the reply to the reply are gone"
    );
}

/// `three-pages.pdf` with its cross-reference table broken (keyword and
/// offset), which `lopdf` rebuilds by scanning.
fn damaged(path: &Path) {
    let bytes = std::fs::read(fixture("three-pages.pdf")).expect("the fixture");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let broken =
        text.replacen("\nxref\n", "\nxxxx\n", 1)
            .replacen("startxref\n340", "startxref\n12", 1);
    assert_ne!(broken, text, "the table was broken");
    std::fs::write(path, broken).expect("written");
}

#[test]
fn a_document_whose_table_was_rebuilt_is_shown_but_never_edited() {
    let scratch = Scratch::new("damaged");
    let original = scratch.path("damaged.pdf");
    damaged(&original);
    assert!(
        read_annotations(&original).is_ok(),
        "still read for showing"
    );
    let error = Base::read(&original).expect_err("not for editing");
    assert!(matches!(error, AnnotError::Damaged), "{error:?}");
    assert_eq!(
        error.message_es(),
        "Este documento está dañado: se puede leer, pero no editar."
    );
    assert!(matches!(
        hidden_copy(&original, &[]),
        Err(AnnotError::Damaged)
    ));
    let out = scratch.path("out.pdf");
    assert!(matches!(
        apply(&original, &EditSet::new(), &out),
        Err(AnnotError::Damaged)
    ));
    assert!(!out.exists());
}

#[test]
fn a_foreign_box_recoloured_widened_and_recoloured_back_is_drawn_again_at_its_new_size() {
    let scratch = Scratch::new("resized");
    let original = fixture("annotated.pdf");
    let base = Base::read(&original).expect("readable");
    let text = base.annotations[0].id.clone();
    let before = base.annotations[0].clone();
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::Recolor(text.clone(), BLUE))
        .expect("recoloured");
    let wider = Rect::new(
        before.rect.x0,
        before.rect.y0,
        before.rect.x1 + 60.0,
        before.rect.y1,
    );
    edits
        .push(&base, Edit::Move(text.clone(), wider))
        .expect("laid out by Calcita now, so it may grow");
    edits
        .push(&base, Edit::Recolor(text.clone(), before.color))
        .expect("and back");
    let shown = edits.apply(&base)[0].clone();
    assert!(
        edit::lays_out_again(Some(&before), &shown),
        "its size is new"
    );
    let out = scratch.path("resized.pdf");
    apply(&original, &edits, &out).expect("written");
    let (old, new) = (dictionary_of(&original, &text), dictionary_of(&out, &text));
    assert_ne!(
        old.get(b"AP").ok(),
        new.get(b"AP").ok(),
        "a new appearance, not the old one stretched"
    );
    let appearance = appearance_of(&out, &new);
    let bbox: Vec<f32> = appearance
        .dict
        .get(b"BBox")
        .and_then(Object::as_array)
        .expect("a box")
        .iter()
        .map(|value| value.as_float().expect("a number"))
        .collect();
    assert!(
        (f64::from(bbox[2] - bbox[0]) - wider.width()).abs() < 0.01,
        "{bbox:?}"
    );
    let reread = read_annotations(&out).expect("rereads").remove(0);
    assert!((reread.rect.width() - wider.width()).abs() < 0.01);
    assert_eq!(reread.color, before.color);
}
