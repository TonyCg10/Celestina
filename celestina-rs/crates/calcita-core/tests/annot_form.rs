// language-contract: product-copy
//! The form: its fields read with their widgets and what they inherit, the
//! copy that stamps each widget's appearance into its page (QtPdf draws no
//! widget), and filling text fields and checkboxes as an incremental update
//! that reads back, the original bytes a prefix.

use std::path::{Path, PathBuf};

use calcita_core::annot::edit::{Base, Edit, EditSet};
use calcita_core::annot::form::{
    check_fill, stamped_view, view, Align, FieldId, FieldValue, Fill, FormField, ReadOnly,
};
use calcita_core::annot::write::{apply, hidden_copy};
use calcita_core::annot::{read_fields, AnnotError, FieldKind, Rect, Rgb};
use lopdf::encryption::{EncryptionState, EncryptionVersion, Permissions};
use lopdf::{dictionary, Document, Object};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../calcita/tests/fixtures")
        .join(name)
}

fn form() -> PathBuf {
    fixture("form.pdf")
}

/// A scratch directory that removes itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("calcita-core-form-{}-{label}", std::process::id()));
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

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("readable")
}

fn assert_prefix(original: &[u8], result: &[u8]) {
    assert!(result.len() > original.len(), "the update adds bytes");
    assert!(
        result[..original.len()] == original[..],
        "the original bytes are a prefix of the result"
    );
}

/// The id of the field `text` names in `form.pdf` (its objects keep their
/// numbers in every update); an id no field has for a name not there.
fn named(text: &str) -> FieldId {
    Base::read(&form())
        .expect("readable")
        .fields
        .iter()
        .find(|field| field.name.0 == text)
        .map_or(FieldId::Walk(u32::MAX), |field| field.id.clone())
}

/// `form.pdf` changed by `change`, saved in `scratch`.
fn modified(scratch: &Scratch, name: &str, change: impl FnOnce(&mut Document)) -> PathBuf {
    let mut document = Document::load(form()).expect("the fixture");
    change(&mut document);
    let path = scratch.path(name);
    document.save(&path).expect("saved");
    path
}

/// Adds `field` to the form's `/Fields` and each of `widgets` to page 1's
/// `/Annots`.
fn add_field(document: &mut Document, field: (u32, u16), widgets: &[(u32, u16)]) {
    document
        .get_dictionary_mut((3, 0))
        .and_then(|form| form.get_mut(b"Fields"))
        .and_then(Object::as_array_mut)
        .expect("the form's fields")
        .push(Object::Reference(field));
    let annots = document
        .get_dictionary_mut((4, 0))
        .and_then(|page| page.get_mut(b"Annots"))
        .and_then(Object::as_array_mut)
        .expect("page 1's annotations");
    annots.extend(widgets.iter().map(|widget| Object::Reference(*widget)));
}

/// A text widget on page 1 at `rect`, with `extra` entries.
fn text_widget(rect: [i64; 4], extra: lopdf::Dictionary) -> lopdf::Dictionary {
    let mut widget = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "F" => 4,
        "P" => Object::Reference((4, 0)),
        "Rect" => rect.iter().map(|value| Object::Integer(*value)).collect::<Vec<_>>(),
    };
    widget.extend(&extra);
    widget
}

fn field<'a>(fields: &'a [FormField], name: &str) -> &'a FormField {
    fields
        .iter()
        .find(|field| field.name.0 == name)
        .unwrap_or_else(|| panic!("a field {name}"))
}

/// The text of a stream's content, inflated when it must be.
fn content(document: &Document, id: (u32, u16)) -> String {
    let stream = document
        .get_object(id)
        .and_then(Object::as_stream)
        .expect("a stream");
    let plain = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    String::from_utf8_lossy(&plain).into_owned()
}

/// The `/AP /N` stream of the object `id`, and its content.
fn normal_appearance(document: &Document, id: (u32, u16)) -> (lopdf::Dictionary, String) {
    let normal = document
        .get_dictionary(id)
        .and_then(|widget| widget.get(b"AP"))
        .and_then(Object::as_dict)
        .and_then(|ap| ap.get(b"N"))
        .and_then(Object::as_reference)
        .expect("an /AP /N stream");
    let dictionary = document
        .get_object(normal)
        .and_then(Object::as_stream)
        .expect("a stream")
        .dict
        .clone();
    (dictionary, content(document, normal))
}

fn name_of(document: &Document, id: (u32, u16), key: &[u8]) -> String {
    let value = document
        .get_dictionary(id)
        .and_then(|dictionary| dictionary.get(key))
        .and_then(Object::as_name)
        .expect("a name");
    String::from_utf8_lossy(value).into_owned()
}

fn string_of(document: &Document, id: (u32, u16), key: &[u8]) -> Vec<u8> {
    document
        .get_dictionary(id)
        .and_then(|dictionary| dictionary.get(key))
        .and_then(Object::as_str)
        .expect("a string")
        .to_vec()
}

/// What the page's last content stream draws, and its resources' forms by
/// name, once stamped.
fn stamp_of(document: &Document, page: u32) -> (String, Vec<(String, u32)>) {
    let id = *document.get_pages().get(&page).expect("the page");
    let page = document.get_dictionary(id).expect("a page");
    let contents = page
        .get(b"Contents")
        .and_then(Object::as_array)
        .expect("the stamped page's contents are an array");
    let last = contents
        .last()
        .and_then(|last| last.as_reference().ok())
        .expect("a stream");
    let first = contents
        .first()
        .and_then(|first| first.as_reference().ok())
        .expect("a stream");
    assert_eq!(
        content(document, first),
        "q\n",
        "the page's own content is wrapped"
    );
    let resources = page
        .get(b"Resources")
        .and_then(Object::as_dict)
        .expect("the page's own resources");
    let forms = resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .expect("forms")
        .iter()
        .map(|(name, form)| {
            (
                String::from_utf8_lossy(name).into_owned(),
                form.as_reference().expect("a form").0,
            )
        })
        .collect();
    (content(document, last), forms)
}

#[test]
fn the_fields_are_read_with_their_widgets_and_what_they_inherit() {
    let base = Base::read(&form()).expect("readable");
    let names: Vec<&str> = base
        .fields
        .iter()
        .map(|field| field.name.0.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "name",
            "agree",
            "address.city",
            "address.zip",
            "copy",
            "choice",
            "color",
            "sign"
        ]
    );

    let name = field(&base.fields, "name");
    assert_eq!(name.kind, FieldKind::Text);
    assert_eq!(name.label.as_deref(), Some("Full name"));
    assert_eq!(name.current(), Some(FieldValue::Text(String::new())));
    let Fill::Text(format) = &name.fill else {
        panic!("a text field: {:?}", name.fill);
    };
    assert_eq!(format.size, Some(12.0));
    assert_eq!(format.align, Align::Left);
    assert_eq!(name.widgets.len(), 1);
    assert_eq!(name.widgets[0].rect, Rect::new(72.0, 650.0, 372.0, 680.0));
    assert_eq!(name.widgets[0].page, Some(0));
    assert_eq!(
        name.widgets[0].border,
        Some(Rgb {
            r: 128,
            g: 128,
            b: 128
        })
    );

    let agree = field(&base.fields, "agree");
    assert_eq!(
        agree.fill,
        Fill::Checkbox {
            on: "Si".to_owned()
        },
        "the on-state its appearance names, not «Yes»"
    );
    assert_eq!(agree.current(), Some(FieldValue::Checked(false)));

    let city = field(&base.fields, "address.city");
    let Fill::Text(format) = &city.fill else {
        panic!("the type inherited from «address»: {:?}", city.fill);
    };
    assert_eq!(format.size, None, "/DA size 0 inherited: fitted");
    assert_eq!(format.color, Rgb { r: 0, g: 0, b: 153 });
    assert_eq!(format.align, Align::Center, "/Q 1 inherited");

    let zip = field(&base.fields, "address.zip");
    let Fill::Text(format) = &zip.fill else {
        panic!("a text field: {:?}", zip.fill);
    };
    assert!(format.comb && !format.multiline);
    assert_eq!(format.max_len, Some(5));
    assert_eq!(format.align, Align::Left, "its own /Q 0 wins");

    let copy = field(&base.fields, "copy");
    assert_eq!(copy.value.as_deref(), Some("Twice"));
    let pages: Vec<Option<u32>> = copy.widgets.iter().map(|widget| widget.page).collect();
    assert_eq!(pages, [Some(0), Some(1)], "one field on two pages");
    assert!(matches!(&copy.fill, Fill::Text(format) if format.multiline));

    assert_eq!(
        field(&base.fields, "choice").fill,
        Fill::No(ReadOnly::Radio)
    );
    assert_eq!(field(&base.fields, "choice").value.as_deref(), Some("B"));
    assert_eq!(
        field(&base.fields, "color").fill,
        Fill::No(ReadOnly::Choice)
    );
    assert_eq!(
        field(&base.fields, "sign").fill,
        Fill::No(ReadOnly::Signature)
    );
}

#[test]
fn the_reader_keeps_giving_each_field_at_its_first_widget() {
    let fields = read_fields(&form()).expect("readable");
    assert_eq!(fields.len(), 8);
    assert_eq!(fields[4].name, "copy");
    assert_eq!(fields[4].rect, Rect::new(400.0, 640.0, 550.0, 690.0));
    assert_eq!(fields[4].page, Some(0));
    assert_eq!(fields[1].value.as_deref(), Some("Off"));
}

#[test]
fn a_field_without_its_own_da_takes_the_forms() {
    let scratch = Scratch::new("form-da");
    let mut document = Document::load(form()).expect("the fixture");
    document
        .get_dictionary_mut((8, 0))
        .expect("«name»")
        .remove(b"DA");
    let path = scratch.path("bare.pdf");
    document.save(&path).expect("saved");
    let base = Base::read(&path).expect("readable");
    let Fill::Text(format) = &field(&base.fields, "name").fill else {
        panic!("a text field");
    };
    assert_eq!(format.size, None, "the AcroForm's /DA, size 0");
    assert_eq!(format.color, Rgb::BLACK);
}

#[test]
fn the_view_stamps_every_shown_widget_into_its_page() {
    let original = bytes(&form());
    let copy = stamped_view(&form()).expect("a copy");
    assert_prefix(&original, &copy);
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let (drawn, forms) = stamp_of(&document, 1);
    assert_eq!(
        drawn.matches(" Do Q").count(),
        9,
        "nine widgets on page 1: {drawn}"
    );
    assert!(
        drawn.starts_with("Q\n"),
        "the page's own state closed first"
    );
    // In /Annots order; a checkbox and a radio button by their /AS.
    let by_name = |label: &str| {
        forms
            .iter()
            .find(|(name, _)| name == label)
            .map(|(_, form)| *form)
    };
    assert_eq!(by_name("CalcitaWidget1"), Some(9), "«name»");
    assert_eq!(by_name("CalcitaWidget2"), Some(13), "«agree» off");
    assert_eq!(by_name("CalcitaWidget6"), Some(28), "radio A off");
    assert_eq!(by_name("CalcitaWidget7"), Some(27), "radio B on");
    // A box at the origin is moved onto the widget; one in page space stays.
    assert!(
        drawn.contains("q 1 0 0 1 72 650 cm /CalcitaWidget1 Do Q"),
        "{drawn}"
    );
    assert!(
        drawn.contains("q 1 0 0 1 0 0 cm /CalcitaWidget5 Do Q"),
        "{drawn}"
    );
    let font = document
        .get_dictionary(*document.get_pages().get(&1).expect("page 1"))
        .and_then(|page| page.get(b"Resources"))
        .and_then(Object::as_dict)
        .and_then(|resources| resources.get(b"Font"))
        .and_then(Object::as_dict)
        .is_ok_and(|fonts| fonts.has(b"F1"));
    assert!(font, "the page keeps its own resources");
    let (drawn, _) = stamp_of(&document, 2);
    assert_eq!(drawn.matches(" Do Q").count(), 1, "«copy» on page 2");
}

#[test]
fn hidden_widgets_are_not_stamped_and_inherited_resources_are_kept() {
    let scratch = Scratch::new("form-hidden");
    let mut document = Document::load(form()).expect("the fixture");
    document
        .get_dictionary_mut((8, 0))
        .expect("«name»")
        .set("F", 6);
    let resources = document
        .get_dictionary_mut((4, 0))
        .expect("page 1")
        .remove(b"Resources")
        .expect("its resources");
    document
        .get_dictionary_mut((2, 0))
        .expect("the page tree")
        .set("Resources", resources);
    let path = scratch.path("hidden.pdf");
    document.save(&path).expect("saved");
    let copy = stamped_view(&path).expect("a copy");
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let (drawn, forms) = stamp_of(&document, 1);
    assert_eq!(drawn.matches(" Do Q").count(), 8, "the hidden one left out");
    assert!(!forms.iter().any(|(_, form)| *form == 9));
    let page = document
        .get_dictionary(*document.get_pages().get(&1).expect("page 1"))
        .expect("a page");
    let has_font = page
        .get(b"Resources")
        .and_then(Object::as_dict)
        .and_then(|resources| resources.get(b"Font"))
        .and_then(Object::as_dict)
        .is_ok_and(|fonts| fonts.has(b"F1"));
    assert!(has_font, "the inherited font is written into the page");
}

#[test]
fn a_document_without_a_widget_is_its_own_view() {
    assert_eq!(
        stamped_view(&fixture("annotated.pdf")).expect("a copy"),
        bytes(&fixture("annotated.pdf"))
    );
    let viewed = view(&fixture("annotated.pdf")).expect("readable");
    assert!(viewed.stamped.is_none());
    assert!(viewed.fields.is_empty());
    assert_eq!(viewed.annotations.len(), 4);
}

#[test]
fn the_view_reads_everything_the_window_shows_at_once() {
    let viewed = view(&form()).expect("readable");
    assert_eq!(viewed.fields.len(), 8);
    assert_eq!(viewed.boxes.len(), 2);
    assert_eq!(
        viewed.annotations.len(),
        10,
        "every widget is an annotation"
    );
    let copy = viewed.stamped.expect("a stamped copy");
    assert_eq!(copy, stamped_view(&form()).expect("a copy"));
}

#[test]
fn the_copy_shown_while_editing_stamps_only_the_fields_calcita_does_not_fill() {
    let copy = hidden_copy(&form(), &[]).expect("a copy");
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let (drawn, forms) = stamp_of(&document, 1);
    let mut stamped: Vec<u32> = forms.iter().map(|(_, form)| *form).collect();
    stamped.sort_unstable();
    assert_eq!(
        stamped,
        [27, 28, 30, 32],
        "the radio buttons, the list, the signature"
    );
    assert_eq!(drawn.matches(" Do Q").count(), 4);
    let page_2 = document
        .get_dictionary(*document.get_pages().get(&2).expect("page 2"))
        .expect("a page");
    assert!(
        page_2
            .get(b"Contents")
            .and_then(Object::as_reference)
            .is_ok(),
        "«copy» is a control while editing: page 2 is left as it is"
    );
}

#[test]
fn filling_writes_values_appearances_and_states_as_an_update() {
    let scratch = Scratch::new("form-fill");
    let base = Base::read(&form()).expect("readable");
    let mut edits = EditSet::new();
    let fills = [
        ("name", FieldValue::Text("Calcita".to_owned())),
        ("agree", FieldValue::Checked(true)),
        ("address.city", FieldValue::Text("Madrid".to_owned())),
        ("address.zip", FieldValue::Text("28001".to_owned())),
        ("copy", FieldValue::Text("uno\ndos".to_owned())),
    ];
    for (name, value) in fills {
        edits
            .push(&base, Edit::Fill(named(name), value))
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
    }
    let out = scratch.path("filled.pdf");
    apply(&form(), &edits, &out).expect("written");
    assert_prefix(&bytes(&form()), &bytes(&out));

    let reread = Base::read(&out).expect("rereads");
    assert_eq!(
        field(&reread.fields, "name").current(),
        Some(FieldValue::Text("Calcita".to_owned()))
    );
    assert_eq!(
        field(&reread.fields, "agree").current(),
        Some(FieldValue::Checked(true))
    );
    assert_eq!(
        field(&reread.fields, "address.city").value.as_deref(),
        Some("Madrid")
    );
    assert_eq!(
        field(&reread.fields, "copy").value.as_deref(),
        Some("uno\ndos")
    );
    assert_eq!(
        field(&reread.fields, "choice").value.as_deref(),
        Some("B"),
        "what was not filled stays"
    );

    let document = Document::load(&out).expect("lopdf reads the result");
    assert_eq!(string_of(&document, (8, 0), b"V"), b"Calcita");
    let (dictionary, drawn) = normal_appearance(&document, (8, 0));
    let bbox: Vec<f32> = dictionary
        .get(b"BBox")
        .and_then(Object::as_array)
        .expect("a box")
        .iter()
        .map(|number| number.as_float().expect("a number"))
        .collect();
    assert_eq!(bbox, [72.0, 650.0, 372.0, 680.0], "the widget's rectangle");
    assert!(drawn.contains("/Tx BMC"), "{drawn}");
    assert!(drawn.contains("/Helv 12 Tf 0 0 0 rg"), "{drawn}");
    assert!(
        drawn.contains("<43616C63697461> Tj"),
        "«Calcita» in WinAnsi: {drawn}"
    );
    assert!(
        drawn.contains("0.502 0.502 0.502 RG 1 w"),
        "the /MK border: {drawn}"
    );

    assert_eq!(name_of(&document, (11, 0), b"V"), "Si");
    assert_eq!(name_of(&document, (11, 0), b"AS"), "Si");

    let (_, city) = normal_appearance(&document, (15, 0));
    assert!(
        city.contains("/Helv 12 Tf 0 0 0.6 rg"),
        "auto size, inherited colour: {city}"
    );
    // «Madrid» is 3056 thousandths wide: 36.672 points at 12, centred in 300.
    assert!(city.contains("1 0 0 1 203.664 "), "centred: {city}");

    let (_, zip) = normal_appearance(&document, (17, 0));
    assert_eq!(zip.matches(" Tm ").count(), 5, "one cell each: {zip}");
    // Five cells of 20 points; «2» (556 thousandths) centred in the first.
    assert!(zip.starts_with("0.502 0.502 0.502 RG"), "{zip}");
    assert!(zip.contains("1 0 0 1 "), "{zip}");

    for widget in [(20, 0), (21, 0)] {
        let (_, copy) = normal_appearance(&document, widget);
        assert_eq!(
            copy.matches(" Tm ").count(),
            2,
            "two lines on each widget: {copy}"
        );
        assert!(copy.contains("/Helv 10 Tf"), "{copy}");
    }
}

#[test]
fn unchecking_and_a_second_fill_append_again() {
    let scratch = Scratch::new("form-again");
    let base = Base::read(&form()).expect("readable");
    let mut edits = EditSet::new();
    edits
        .push(&base, Edit::Fill(named("agree"), FieldValue::Checked(true)))
        .expect("checked");
    let first = scratch.path("first.pdf");
    apply(&form(), &edits, &first).expect("written");

    let base = Base::read(&first).expect("rereads");
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(named("agree"), FieldValue::Checked(false)),
        )
        .expect("unchecked");
    edits
        .push(
            &base,
            Edit::Fill(named("name"), FieldValue::Text("Ada".to_owned())),
        )
        .expect("filled");
    let second = scratch.path("second.pdf");
    apply(&first, &edits, &second).expect("written");
    assert_prefix(&bytes(&first), &bytes(&second));
    let document = Document::load(&second).expect("lopdf reads the result");
    assert_eq!(name_of(&document, (11, 0), b"V"), "Off");
    assert_eq!(name_of(&document, (11, 0), b"AS"), "Off");
    assert_eq!(string_of(&document, (8, 0), b"V"), b"Ada");
}

#[test]
fn what_a_field_cannot_hold_is_refused() {
    let base = Base::read(&form()).expect("readable");
    let mut edits = EditSet::new();
    let refused = |edits: &mut EditSet, name: &str, value: FieldValue| {
        edits
            .push(&base, Edit::Fill(named(name), value))
            .expect_err("refused")
    };
    let too_long = refused(
        &mut edits,
        "address.zip",
        FieldValue::Text("280011".to_owned()),
    );
    assert!(matches!(too_long, AnnotError::TooLong(5)), "{too_long:?}");
    assert_eq!(
        too_long.message_es(),
        "Este campo admite como máximo 5 caracteres."
    );
    let single_line = field(&base.fields, "name");
    let line_break = check_fill(single_line, &FieldValue::Text("dos\nlíneas".to_owned()))
        .expect_err("a line break in a single-line field");
    assert!(
        matches!(line_break, AnnotError::NotFillable),
        "{line_break:?}"
    );
    assert_eq!(
        line_break.message_es(),
        "Ese campo no se puede rellenar así."
    );
    assert!(matches!(
        refused(&mut edits, "name", FieldValue::Text("\u{2192}".to_owned())),
        AnnotError::Unencodable('\u{2192}')
    ));
    assert!(matches!(
        refused(&mut edits, "name", FieldValue::Checked(true)),
        AnnotError::NotFillable
    ));
    assert!(matches!(
        refused(&mut edits, "agree", FieldValue::Text("Si".to_owned())),
        AnnotError::NotFillable
    ));
    assert!(matches!(
        refused(&mut edits, "nowhere", FieldValue::Text(String::new())),
        AnnotError::NotFillable
    ));
    let cases = [
        (
            "choice",
            ReadOnly::Radio,
            "Calcita aún no rellena botones de opción: se muestran como están.",
        ),
        (
            "color",
            ReadOnly::Choice,
            "Calcita aún no rellena listas de opciones: se muestran como están.",
        ),
        (
            "sign",
            ReadOnly::Signature,
            "Las firmas no se pueden rellenar en Calcita.",
        ),
    ];
    for (name, why, message) in cases {
        let error = refused(&mut edits, name, FieldValue::Text("x".to_owned()));
        assert!(
            matches!(error, AnnotError::ReadOnlyField(found) if found == why),
            "{name}: {error:?}"
        );
        assert_eq!(error.message_es(), message);
    }
    assert!(edits.is_empty(), "nothing refused is kept");
    assert!(edits
        .push(
            &base,
            Edit::Fill(named("address.zip"), FieldValue::Text("28001".to_owned()))
        )
        .is_ok());
}

#[test]
fn fills_undo_redo_and_cost_nothing_when_filled_back() {
    let base = Base::read(&form()).expect("readable");
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(named("name"), FieldValue::Text("Ada".to_owned())),
        )
        .expect("filled");
    edits
        .push(&base, Edit::Fill(named("agree"), FieldValue::Checked(true)))
        .expect("checked");
    let shown = edits.fields(&base);
    assert_eq!(field(&shown, "name").value.as_deref(), Some("Ada"));
    assert_eq!(field(&shown, "agree").value.as_deref(), Some("Si"));
    assert_eq!(edits.resolve(&base).filled.len(), 2);
    assert!(edits.undo());
    assert_eq!(
        field(&edits.fields(&base), "agree").current(),
        Some(FieldValue::Checked(false))
    );
    assert!(edits.redo());
    assert_eq!(
        field(&edits.fields(&base), "agree").current(),
        Some(FieldValue::Checked(true))
    );
    edits
        .push(
            &base,
            Edit::Fill(named("name"), FieldValue::Text(String::new())),
        )
        .expect("emptied");
    edits
        .push(
            &base,
            Edit::Fill(named("agree"), FieldValue::Checked(false)),
        )
        .expect("unchecked");
    assert!(
        edits.resolve(&base).is_empty(),
        "both back as the file has them"
    );
    assert!(
        edits.apply(&base) == base.annotations,
        "a fill changes no annotation"
    );
}

fn encrypted(path: &Path) {
    let mut document = Document::load(form()).expect("the fixture");
    document.trailer.set(
        "ID",
        Object::Array(vec![
            Object::string_literal("calcita-form-id"),
            Object::string_literal("calcita-form-id"),
        ]),
    );
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
fn an_encrypted_or_damaged_form_is_never_filled() {
    let scratch = Scratch::new("form-refused");
    let locked = scratch.path("locked.pdf");
    encrypted(&locked);
    assert!(matches!(Base::read(&locked), Err(AnnotError::Encrypted)));
    assert!(matches!(stamped_view(&locked), Err(AnnotError::Encrypted)));
    assert!(matches!(view(&locked), Err(AnnotError::Encrypted)));

    let text = String::from_utf8_lossy(&bytes(&form())).into_owned();
    let start = text.rfind("startxref\n").expect("a startxref") + "startxref\n".len();
    let offset: String = text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let broken = text.replacen("\nxref\n", "\nxxxx\n", 1).replacen(
        &format!("startxref\n{offset}"),
        "startxref\n12",
        1,
    );
    let damaged = scratch.path("damaged.pdf");
    std::fs::write(&damaged, broken).expect("written");
    assert!(matches!(Base::read(&damaged), Err(AnnotError::Damaged)));
    assert!(matches!(stamped_view(&damaged), Err(AnnotError::Damaged)));
    let viewed = view(&damaged).expect("still read for showing");
    assert_eq!(viewed.fields.len(), 8);
    assert!(viewed.stamped.is_none(), "no update can point back at it");
}

#[test]
fn fields_are_found_by_id_whatever_their_names() {
    let scratch = Scratch::new("form-ids");
    let path = modified(&scratch, "twins.pdf", |document| {
        // Two root fields both named «dup», and one without a name.
        for (y, label) in [(250, Some("dup")), (210, Some("dup")), (170, None)] {
            let mut extra =
                dictionary! { "FT" => "Tx", "DA" => Object::string_literal("/Helv 10 Tf 0 g") };
            if let Some(label) = label {
                extra.set("T", Object::string_literal(label));
            }
            let id = document.add_object(text_widget([300, y, 500, y + 30], extra));
            add_field(document, id, &[id]);
        }
    });
    let base = Base::read(&path).expect("readable");
    let twins: Vec<&FormField> = base
        .fields
        .iter()
        .filter(|field| field.name.0 == "dup")
        .collect();
    assert_eq!(twins.len(), 2, "both kept");
    assert_ne!(twins[0].id, twins[1].id);
    let unnamed = field(&base.fields, "");
    assert!(
        matches!(unnamed.fill, Fill::Text(_)),
        "a field without a name is filled too"
    );
    let mut edits = EditSet::new();
    for (id, text) in [
        (&twins[0].id, "uno"),
        (&twins[1].id, "dos"),
        (&unnamed.id, "tres"),
    ] {
        edits
            .push(
                &base,
                Edit::Fill(id.clone(), FieldValue::Text(text.to_owned())),
            )
            .expect("filled");
    }
    let out = scratch.path("filled.pdf");
    apply(&path, &edits, &out).expect("written");
    let reread = Base::read(&out).expect("rereads");
    let values: Vec<Option<&str>> = reread
        .fields
        .iter()
        .filter(|field| field.name.0 == "dup" || field.name.0.is_empty())
        .map(|field| field.value.as_deref())
        .collect();
    assert_eq!(values, [Some("uno"), Some("dos"), Some("tres")]);
}

#[test]
fn a_kid_without_a_name_but_with_kids_is_a_field_node() {
    let scratch = Scratch::new("form-node");
    let path = modified(&scratch, "node.pdf", |document| {
        let group = document.new_object_id();
        let node = document.new_object_id();
        let widget = document.add_object(text_widget(
            [300, 470, 500, 500],
            dictionary! { "Parent" => Object::Reference(node) },
        ));
        document.objects.insert(
            node,
            Object::Dictionary(dictionary! {
                "FT" => "Tx",
                "Parent" => Object::Reference(group),
                "Kids" => vec![Object::Reference(widget)],
            }),
        );
        document.objects.insert(
            group,
            Object::Dictionary(dictionary! {
                "T" => Object::string_literal("group"),
                "Kids" => vec![Object::Reference(node)],
            }),
        );
        add_field(document, group, &[widget]);
    });
    let base = Base::read(&path).expect("readable");
    let group = field(&base.fields, "group");
    assert_eq!(group.widgets.len(), 1);
    assert_eq!(group.widgets[0].rect, Rect::new(300.0, 470.0, 500.0, 500.0));
    assert!(matches!(group.fill, Fill::Text(_)), "{:?}", group.fill);
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(group.id.clone(), FieldValue::Text("Grupo".to_owned())),
        )
        .expect("filled");
    let out = scratch.path("filled.pdf");
    apply(&path, &edits, &out).expect("written");
    let reread = Base::read(&out).expect("rereads");
    assert_eq!(
        field(&reread.fields, "group").value.as_deref(),
        Some("Grupo")
    );
}

#[test]
fn hidden_and_no_view_widgets_are_not_shown_but_invisible_ones_are() {
    let scratch = Scratch::new("form-shown");
    let path = modified(&scratch, "hidden.pdf", |document| {
        for (id, flags) in [((8, 0), 6), ((11, 0), 32 + 4), ((15, 0), 1)] {
            document
                .get_dictionary_mut(id)
                .expect("a widget")
                .set("F", flags);
        }
    });
    let base = Base::read(&path).expect("readable");
    for name in ["name", "agree"] {
        assert!(!field(&base.fields, name).widgets[0].shown, "{name}");
    }
    assert!(
        field(&base.fields, "address.city").widgets[0].shown,
        "invisible is for unknown types only"
    );
    assert!(field(&base.fields, "address.zip").widgets[0].shown);
    let copy = stamped_view(&path).expect("a copy");
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let (drawn, forms) = stamp_of(&document, 1);
    assert_eq!(
        drawn.matches(" Do Q").count(),
        7,
        "nine less the hidden and NoView ones"
    );
    assert!(
        forms.iter().any(|(_, form)| *form == 16),
        "the invisible one is stamped"
    );
}

#[test]
fn a_line_break_in_a_single_line_field_becomes_a_space_once_edited() {
    let scratch = Scratch::new("form-fold");
    let path = modified(&scratch, "folded.pdf", |document| {
        document
            .get_dictionary_mut((8, 0))
            .expect("«name»")
            .set("V", Object::string_literal("one\ntwo"));
    });
    let base = Base::read(&path).expect("readable");
    let name = field(&base.fields, "name").id.clone();
    assert_eq!(
        field(&base.fields, "name").value.as_deref(),
        Some("one\ntwo")
    );

    // Untouched (its own value given back), nothing is rewritten.
    let mut untouched = EditSet::new();
    untouched
        .push(
            &base,
            Edit::Fill(name.clone(), FieldValue::Text("one\ntwo".to_owned())),
        )
        .expect("its own value");
    assert!(
        untouched.resolve(&base).is_empty(),
        "the value read is never rewritten"
    );

    // Edited where it stands, with its line break and a tab: folded.
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(name, FieldValue::Text("one\ntwo!\tok".to_owned())),
        )
        .expect("folded, then taken");
    assert_eq!(
        field(&edits.fields(&base), "name").value.as_deref(),
        Some("one two! ok")
    );
    let out = scratch.path("filled.pdf");
    apply(&path, &edits, &out).expect("written");
    let reread = Base::read(&out).expect("rereads");
    assert_eq!(
        field(&reread.fields, "name").value.as_deref(),
        Some("one two! ok")
    );
}

#[test]
fn a_line_break_another_program_wrote_reads_and_saves_back() {
    let scratch = Scratch::new("form-lines");
    let path = modified(&scratch, "lines.pdf", |document| {
        document
            .get_dictionary_mut((19, 0))
            .expect("«copy»")
            .set("V", Object::string_literal("line1\rline2\tend"));
        document
            .get_dictionary_mut((8, 0))
            .expect("«name»")
            .set("V", Object::string_literal("one\r\ntwo"));
    });
    let base = Base::read(&path).expect("readable");
    let copy = field(&base.fields, "copy");
    assert_eq!(
        copy.value.as_deref(),
        Some("line1\nline2\tend"),
        "a CR reads as a line feed, a tab as itself"
    );
    assert_eq!(
        field(&base.fields, "name").value.as_deref(),
        Some("one\ntwo")
    );
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(
                copy.id.clone(),
                FieldValue::Text("line1\nline2 more".to_owned()),
            ),
        )
        .expect("filled");
    let out = scratch.path("filled.pdf");
    apply(&path, &edits, &out).expect("written");
    let reread = Base::read(&out).expect("rereads");
    assert_eq!(
        field(&reread.fields, "copy").value.as_deref(),
        Some("line1\nline2 more")
    );
}

#[test]
fn a_text_box_another_program_broke_with_a_cr_reads_with_its_line() {
    let scratch = Scratch::new("form-freetext");
    let mut document = Document::load(fixture("annotated.pdf")).expect("the fixture");
    document
        .get_dictionary_mut((6, 0))
        .expect("the FreeText")
        .set("Contents", Object::string_literal("Hello\rCalcita"));
    let path = scratch.path("annotated.pdf");
    document.save(&path).expect("saved");
    let annotations = calcita_core::annot::read_annotations(&path).expect("readable");
    assert!(
        matches!(&annotations[0].kind, calcita_core::annot::Kind::Text { contents, .. } if contents == "Hello\nCalcita"),
        "{:?}",
        annotations[0].kind
    );
}

#[test]
fn each_widget_lays_its_text_out_with_its_own_da_and_q() {
    let scratch = Scratch::new("form-own-da");
    let path = modified(&scratch, "own.pdf", |document| {
        let widget = document
            .get_dictionary_mut((21, 0))
            .expect("«copy» on page 2");
        widget.set("DA", Object::string_literal("/Helv 14 Tf 1 0 0 rg"));
        widget.set("Q", 2);
    });
    let base = Base::read(&path).expect("readable");
    let copy = field(&base.fields, "copy");
    let Fill::Text(format) = &copy.fill else {
        panic!("a text field");
    };
    let first = format.of(&copy.widgets[0]);
    let second = format.of(&copy.widgets[1]);
    assert_eq!(
        (first.size, first.color, first.align),
        (Some(10.0), Rgb::BLACK, Align::Left)
    );
    assert_eq!(
        (second.size, second.color, second.align),
        (Some(14.0), Rgb { r: 255, g: 0, b: 0 }, Align::Right)
    );
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Fill(copy.id.clone(), FieldValue::Text("Hola".to_owned())),
        )
        .expect("filled");
    let out = scratch.path("filled.pdf");
    apply(&path, &edits, &out).expect("written");
    let document = Document::load(&out).expect("lopdf reads the result");
    let (_, page_1) = normal_appearance(&document, (20, 0));
    let (_, page_2) = normal_appearance(&document, (21, 0));
    assert!(page_1.contains("/Helv 10 Tf 0 0 0 rg"), "{page_1}");
    assert!(page_2.contains("/Helv 14 Tf 1 0 0 rg"), "{page_2}");
}

#[test]
fn a_turned_appearance_is_carried_by_its_matrix_onto_the_widget() {
    let scratch = Scratch::new("form-matrix");
    let path = modified(&scratch, "turned.pdf", |document| {
        document
            .get_object_mut((9, 0))
            .and_then(Object::as_stream_mut)
            .expect("«name»'s appearance")
            .dict
            .set(
                "Matrix",
                vec![
                    0.into(),
                    1.into(),
                    (-1).into(),
                    0.into(),
                    0.into(),
                    0.into(),
                ],
            );
    });
    let copy = stamped_view(&path).expect("a copy");
    let document = Document::load_mem(&copy).expect("lopdf reads the copy");
    let (drawn, _) = stamp_of(&document, 1);
    // The box [0 0 300 30] turned is [-30 0 0 300]: scaled 10 across and
    // 0.1 up onto [72 650 372 680].
    assert!(
        drawn.contains("q 10 0 0 0.1 372 650 cm /CalcitaWidget1 Do Q"),
        "{drawn}"
    );
}
