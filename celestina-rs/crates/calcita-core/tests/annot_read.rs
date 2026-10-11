// language-contract: product-copy
//! Reading the annotation model over Calcita's fixtures (`calcita/tests/
//! fixtures`, written by `calcita/scripts/make-fixture-pdf.py`) and over
//! documents made here with `lopdf`.

use std::path::{Path, PathBuf};

use calcita_core::annot::coords::{to_view_rect, Rotation};
use calcita_core::annot::{
    read_annotations, read_fields, read_page_boxes, AnnotError, AnnotId, FieldKind, Kind, Point,
    Rect, Rgb, MAX_FILE_BYTES,
};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{dictionary, Document, Object};

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
            std::env::temp_dir().join(format!("calcita-core-annot-{}-{label}", std::process::id()));
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

#[test]
fn the_annotated_fixture_reads_as_the_three_kinds_and_a_link() {
    let annotations = read_annotations(&fixture("annotated.pdf")).expect("readable");
    assert_eq!(annotations.len(), 4);
    assert!(annotations.iter().all(|annotation| annotation.page == 0));

    let text = &annotations[0];
    assert_eq!(
        text.kind,
        Kind::Text {
            contents: "Hello Calcita".to_owned(),
            size_pt: 16.0,
            rich: false
        }
    );
    assert_eq!(text.rect, Rect::new(300.0, 700.0, 520.0, 760.0));
    assert_eq!(text.color, Rgb { r: 0, g: 0, b: 153 }, "the /DA colour");
    assert_eq!(text.author.as_deref(), Some("Another program"));
    assert_eq!(
        text.id,
        AnnotId::Object {
            number: 6,
            generation: 0
        }
    );

    let highlight = &annotations[1];
    let Kind::Highlight { quads } = &highlight.kind else {
        panic!("a highlight: {highlight:?}");
    };
    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].points[0], Point { x: 70.0, y: 786.0 });
    assert_eq!(quads[0].points[3], Point { x: 290.0, y: 752.0 });
    assert_eq!(highlight.color, Rgb::YELLOW);

    let ink = &annotations[2];
    let Kind::Ink { strokes, width } = &ink.kind else {
        panic!("an ink drawing: {ink:?}");
    };
    assert_eq!(*width, 4.0, "the /BS width");
    assert_eq!(strokes.len(), 1);
    assert_eq!(strokes[0].len(), 5);
    assert_eq!(strokes[0][1], Point { x: 360.0, y: 620.0 });
    assert_eq!(ink.color, Rgb { r: 255, g: 0, b: 0 });

    let link = &annotations[3];
    assert_eq!(
        link.kind,
        Kind::Other {
            subtype: "Link".to_owned()
        }
    );
    assert_eq!(link.author, None);
}

#[test]
fn the_outline_fixtures_links_are_other() {
    let annotations = read_annotations(&fixture("outline.pdf")).expect("readable");
    assert_eq!(annotations.len(), 2);
    for annotation in &annotations {
        assert_eq!(annotation.page, 0);
        assert_eq!(
            annotation.kind,
            Kind::Other {
                subtype: "Link".to_owned()
            }
        );
    }
    assert!(read_annotations(&fixture("three-pages.pdf"))
        .expect("readable")
        .is_empty());
}

#[test]
fn the_rotated_fixture_has_four_turns_of_one_box() {
    let boxes = read_page_boxes(&fixture("rotated.pdf")).expect("readable");
    let turns: Vec<Rotation> = boxes.iter().map(|page| page.rotate).collect();
    assert_eq!(
        turns,
        [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270]
    );
    for page in &boxes {
        // The fourth page's /CropBox wins over its larger /MediaBox.
        assert_eq!(
            (page.x0, page.y0, page.w, page.h),
            (50.0, 100.0, 595.0, 842.0)
        );
    }

    // Each page's highlight sits 70 points in and 56 down from the top-left
    // corner of the unrotated box, so the view puts it at the corner each
    // turn carries that corner to.
    let annotations = read_annotations(&fixture("rotated.pdf")).expect("readable");
    let expected = [
        Rect::new(70.0, 56.0, 180.0, 90.0),
        Rect::new(752.0, 70.0, 786.0, 180.0),
        Rect::new(415.0, 752.0, 525.0, 786.0),
        Rect::new(56.0, 415.0, 90.0, 525.0),
    ];
    assert_eq!(annotations.len(), 4);
    for (index, annotation) in annotations.iter().enumerate() {
        assert_eq!(annotation.page as usize, index);
        assert_eq!(
            to_view_rect(boxes[index], annotation.rect),
            expected[index],
            "page {index}"
        );
    }
}

fn encrypted_copy(scratch: &Scratch, user_password: &str) -> PathBuf {
    let mut document = Document::load(fixture("annotated.pdf")).expect("the fixture");
    document.trailer.set(
        "ID",
        Object::Array(vec![
            Object::string_literal("calcita-fixture-id"),
            Object::string_literal("calcita-fixture-id"),
        ]),
    );
    let version = EncryptionVersion::V1 {
        document: &document,
        owner_password: "owner",
        user_password,
        permissions: Permissions::all(),
    };
    let state = EncryptionState::try_from(version).expect("an RC4 state");
    document.encrypt(&state).expect("encrypted");
    let path = scratch.path(&format!("encrypted-{}.pdf", user_password.len()));
    document.save(&path).expect("saved");
    path
}

#[test]
fn an_encrypted_document_is_refused() {
    let scratch = Scratch::new("encrypted");
    for password in ["secret", ""] {
        let path = encrypted_copy(&scratch, password);
        let error = read_annotations(&path).expect_err("refused");
        assert!(
            matches!(error, AnnotError::Encrypted),
            "password {password:?}: {error}"
        );
        assert!(matches!(read_fields(&path), Err(AnnotError::Encrypted)));
        assert_eq!(
            error.message_es(),
            "Este documento está cifrado y no se puede editar."
        );
    }
}

#[test]
fn a_file_that_is_not_a_pdf_or_is_missing_or_too_large_is_refused() {
    let scratch = Scratch::new("refusals");
    let garbage = scratch.path("garbage.pdf");
    std::fs::write(&garbage, b"%PDF-1.4\nthis is no document\n").expect("written");
    assert!(matches!(
        read_annotations(&garbage),
        Err(AnnotError::Unparseable(_))
    ));
    assert!(matches!(
        read_annotations(&scratch.path("absent.pdf")),
        Err(AnnotError::Io(_))
    ));
    // A sparse file: its length is past the bound without its bytes.
    let huge = scratch.path("huge.pdf");
    let file = std::fs::File::create(&huge).expect("created");
    file.set_len(MAX_FILE_BYTES + 1).expect("sparse length");
    assert!(matches!(
        read_annotations(&huge),
        Err(AnnotError::TooLarge { bytes }) if bytes == MAX_FILE_BYTES + 1
    ));
}

/// A one-page form: a text field «name» filled in, a checkbox «agree» on
/// `Yes`, and a radio group «choice» whose two buttons are kids.
fn form(scratch: &Scratch) -> PathBuf {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let page = document.new_object_id();
    let text = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Tx",
        "T" => Object::string_literal("name"), "V" => Object::string_literal("Ada"),
        "Rect" => vec![72.into(), 650.into(), 372.into(), 680.into()],
        "P" => page,
    });
    let check = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Btn",
        "T" => Object::string_literal("agree"), "V" => "Yes", "AS" => "Yes",
        "Rect" => vec![72.into(), 580.into(), 92.into(), 600.into()],
    });
    let radio = document.new_object_id();
    let first = document.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "Parent" => radio,
        "Rect" => vec![72.into(), 520.into(), 92.into(), 540.into()],
    });
    document.objects.insert(
        radio,
        Object::Dictionary(dictionary! {
            "FT" => "Btn", "Ff" => 1 << 15, "T" => Object::string_literal("choice"),
            "Kids" => vec![first.into()],
        }),
    );
    document.objects.insert(
        page,
        Object::Dictionary(dictionary! {
            "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Annots" => vec![text.into(), check.into(), first.into()],
        }),
    );
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
        }),
    );
    let form = document.add_object(dictionary! {
        "Fields" => vec![text.into(), check.into(), radio.into()],
    });
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages, "AcroForm" => form,
    });
    document.trailer.set("Root", catalog);
    let path = scratch.path("form.pdf");
    document.save(&path).expect("saved");
    path
}

#[test]
fn the_form_fields_are_read_with_their_values() {
    let scratch = Scratch::new("form");
    let fields = read_fields(&form(&scratch)).expect("readable");
    assert_eq!(fields.len(), 3);

    assert_eq!(fields[0].name, "name");
    assert_eq!(fields[0].kind, FieldKind::Text);
    assert_eq!(fields[0].value.as_deref(), Some("Ada"));
    assert_eq!(fields[0].rect, Rect::new(72.0, 650.0, 372.0, 680.0));
    assert_eq!(fields[0].page, Some(0));

    assert_eq!(fields[1].name, "agree");
    assert_eq!(fields[1].kind, FieldKind::Checkbox);
    assert_eq!(fields[1].value.as_deref(), Some("Yes"));

    assert_eq!(fields[2].name, "choice");
    assert_eq!(
        fields[2].kind,
        FieldKind::Other,
        "a radio group is not filled"
    );
    assert_eq!(fields[2].rect, Rect::new(72.0, 520.0, 92.0, 540.0));
    assert_eq!(fields[2].page, Some(0));

    let annotations = read_annotations(&scratch.path("form.pdf")).expect("readable");
    assert_eq!(annotations.len(), 3, "each widget is an annotation too");
    assert!(annotations.iter().all(|annotation| annotation.kind
        == Kind::Other {
            subtype: "Widget".to_owned()
        }));
    assert!(read_fields(&fixture("annotated.pdf"))
        .expect("readable")
        .is_empty());
}

fn text_string(bytes: &[u8]) -> Object {
    Object::String(bytes.to_vec(), lopdf::StringFormat::Literal)
}

fn inline_text(contents: &[u8], author: &[u8]) -> Object {
    Object::Dictionary(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText",
        "Rect" => vec![10.into(), 10.into(), 110.into(), 40.into()],
        "Contents" => text_string(contents), "T" => text_string(author),
    })
}

/// Five pages that stretch the reader: a real /Rotate and `null` and
/// dangling /Annots entries (page 0), a /Rotate of 100 (page 1), a /CropBox
/// larger than the /MediaBox (page 2), a negative /Rotate and malformed or
/// marked text strings (page 3), a /CropBox off the /MediaBox (page 4).
fn odd_document(scratch: &Scratch) -> PathBuf {
    let mut document = Document::with_version("1.7");
    let pages = document.new_object_id();
    let a4 = || vec![0.into(), 0.into(), 595.into(), 842.into()];
    let page = |document: &mut Document, extra: lopdf::Dictionary| {
        let mut dictionary = dictionary! {
            "Type" => "Page", "Parent" => pages, "MediaBox" => a4(),
        };
        dictionary.extend(&extra);
        document.add_object(dictionary)
    };
    let first = page(
        &mut document,
        dictionary! {
            "Rotate" => Object::Real(90.0),
            "Annots" => vec![
                Object::Null,
                inline_text(b"first", b"A"),
                Object::Reference((999, 0)),
                Object::Dictionary(dictionary! {
                    "Type" => "Annot", "Subtype" => "Highlight",
                    "Rect" => vec![10.into(), 10.into(), 20.into(), 20.into()],
                }),
            ],
        },
    );
    let second = page(&mut document, dictionary! { "Rotate" => 100 });
    let third = page(
        &mut document,
        dictionary! { "CropBox" => vec![0.into(), 0.into(), 700.into(), 900.into()] },
    );
    let fourth = page(
        &mut document,
        dictionary! {
            "Rotate" => -90,
            "Annots" => vec![
                inline_text(b"\xEF\xBB\xBFHola", b"\xEF\xBB\xBFAutora"),
                inline_text(b"\xFE\xFF\x00H\x00i\x00", b"\xFF\xFEH\x00i\x00"),
                inline_text(b"\xFE\xFF\xD8\x00\x00A", b"caf\xE9"),
            ],
        },
    );
    let fifth = page(
        &mut document,
        dictionary! { "CropBox" => vec![700.into(), 900.into(), 800.into(), 1000.into()] },
    );
    document.objects.insert(
        pages,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![first.into(), second.into(), third.into(), fourth.into(), fifth.into()],
            "Count" => 5,
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    document.trailer.set("Root", catalog);
    let path = scratch.path("odd.pdf");
    document.save(&path).expect("saved");
    path
}

#[test]
fn rotations_and_boxes_are_read_as_pdfium_reads_them() {
    let scratch = Scratch::new("boxes");
    let boxes = read_page_boxes(&odd_document(&scratch)).expect("readable");
    let turns: Vec<Rotation> = boxes.iter().map(|page| page.rotate).collect();
    assert_eq!(
        turns,
        [
            Rotation::R90,
            Rotation::R90,
            Rotation::R0,
            Rotation::R270,
            Rotation::R0
        ],
        "90.0, 100, none, -90, none"
    );
    for (index, page) in boxes.iter().enumerate() {
        // The larger /CropBox is clipped to the /MediaBox (page 2); one off
        // the /MediaBox leaves the /MediaBox (page 4).
        assert_eq!(
            (page.x0, page.y0, page.w, page.h),
            (0.0, 0.0, 595.0, 842.0),
            "page {index}"
        );
    }
}

#[test]
fn an_inline_annotation_is_found_at_its_true_place_in_annots() {
    let scratch = Scratch::new("positions");
    let annotations = read_annotations(&odd_document(&scratch)).expect("readable");
    let first_page: Vec<&AnnotId> = annotations
        .iter()
        .filter(|annotation| annotation.page == 0)
        .map(|annotation| &annotation.id)
        .collect();
    // `null` at 0 and a dangling reference at 2 still count.
    assert_eq!(
        first_page,
        [
            &AnnotId::Position { page: 0, index: 1 },
            &AnnotId::Position { page: 0, index: 3 }
        ]
    );
}

#[test]
fn text_strings_lose_their_mark_and_keep_what_decodes() {
    let scratch = Scratch::new("text");
    let annotations = read_annotations(&odd_document(&scratch)).expect("readable");
    let texts: Vec<(String, Option<String>)> = annotations
        .iter()
        .filter(|annotation| annotation.page == 3)
        .map(|annotation| match &annotation.kind {
            Kind::Text { contents, .. } => (contents.clone(), annotation.author.clone()),
            other => panic!("a text box: {other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        [
            ("Hola".to_owned(), Some("Autora".to_owned())),
            ("Hi\u{FFFD}".to_owned(), Some("Hi".to_owned())),
            ("\u{FFFD}A".to_owned(), Some("café".to_owned())),
        ],
        "a UTF-8 mark stripped; UTF-16 with an odd byte, a lone surrogate, little-endian; PDFDocEncoding"
    );
}

#[test]
fn a_document_saved_with_object_streams_reads_through_them() {
    let scratch = Scratch::new("objstm");
    let mut document = Document::load(fixture("annotated.pdf")).expect("the fixture");
    let path = scratch.path("modern.pdf");
    let mut file = std::fs::File::create(&path).expect("created");
    document
        .save_with_options(
            &mut file,
            lopdf::SaveOptions::builder()
                .use_object_streams(true)
                .use_xref_streams(true)
                .build(),
        )
        .expect("saved with object streams");
    drop(file);
    let bytes = std::fs::read(&path).expect("written");
    assert!(
        bytes.windows(7).any(|window| window == b"/ObjStm"),
        "the annotations live in an object stream"
    );
    let annotations = read_annotations(&path).expect("readable");
    let original = read_annotations(&fixture("annotated.pdf")).expect("readable");
    let kinds = |list: &[calcita_core::annot::Annotation]| -> Vec<Kind> {
        list.iter()
            .map(|annotation| annotation.kind.clone())
            .collect()
    };
    assert_eq!(kinds(&annotations), kinds(&original));
}
