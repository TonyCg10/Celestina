// language-contract: product-copy
//! The edit set: what may change, undo and redo, and what the writer is
//! asked to do.

use std::path::{Path, PathBuf};

use calcita_core::annot::coords::{to_view_rect, PageBox, Rotation};
use calcita_core::annot::edit::{self, Base, Edit, EditSet};
use calcita_core::annot::{AnnotError, AnnotId, Annotation, Kind, Point, Rect, Rgb};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../calcita/tests/fixtures")
        .join(name)
}

fn annotated() -> Base {
    Base::read(&fixture("annotated.pdf")).expect("readable")
}

fn object(number: u32) -> AnnotId {
    AnnotId::Object {
        number,
        generation: 0,
    }
}

/// The annotated fixture's FreeText, Highlight, Ink and Link.
fn ids(base: &Base) -> [AnnotId; 4] {
    [0, 1, 2, 3].map(|index| base.annotations[index].id.clone())
}

const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };

fn a4() -> PageBox {
    PageBox {
        x0: 0.0,
        y0: 0.0,
        w: 595.0,
        h: 842.0,
        rotate: Rotation::R0,
    }
}

#[test]
fn the_base_reads_the_model_and_the_boxes_at_once() {
    let base = annotated();
    assert_eq!(base.annotations.len(), 4);
    assert_eq!(base.boxes, vec![a4()]);
    let [text, highlight, ink, link] = ids(&base);
    assert_eq!(base.editable_ids(), vec![text, highlight, ink]);
    assert_eq!(link, object(12));
}

#[test]
fn every_kind_of_any_author_moves_recolours_and_goes() {
    let base = annotated();
    let [text, highlight, ink, _] = ids(&base);
    let mut edits = EditSet::new();
    edits
        .push(
            &base,
            Edit::Move(highlight.clone(), Rect::new(80.0, 652.0, 300.0, 686.0)),
        )
        .expect("a highlight moves");
    edits
        .push(&base, Edit::Recolor(ink.clone(), BLUE))
        .expect("a drawing recolours");
    edits
        .push(&base, Edit::SetText(text.clone(), "Hola".to_owned()))
        .expect("a plain text box takes new text");
    let shown = edits.apply(&base);
    let moved = shown
        .iter()
        .find(|a| a.id == highlight)
        .expect("still there");
    let Kind::Highlight { quads } = &moved.kind else {
        panic!("{moved:?}");
    };
    // The fixture's quad (70, 752)–(290, 786) carried with its rectangle.
    assert_eq!(quads[0].points[0], Point { x: 80.0, y: 686.0 });
    assert_eq!(quads[0].points[3], Point { x: 300.0, y: 652.0 });
    assert_eq!(
        shown.iter().find(|a| a.id == ink).expect("there").color,
        BLUE
    );
    let retexted = shown.iter().find(|a| a.id == text).expect("there");
    assert!(matches!(&retexted.kind, Kind::Text { contents, .. } if contents == "Hola"));

    edits
        .push(&base, Edit::Delete(ink.clone()))
        .expect("a drawing goes");
    assert!(edits.apply(&base).iter().all(|a| a.id != ink));
}

#[test]
fn a_link_or_a_missing_annotation_is_not_editable() {
    let base = annotated();
    let [_, _, _, link] = ids(&base);
    let mut edits = EditSet::new();
    for edit in [
        Edit::Delete(link.clone()),
        Edit::Recolor(link.clone(), BLUE),
        Edit::Move(link, Rect::new(0.0, 0.0, 10.0, 10.0)),
        Edit::Delete(object(999)),
        Edit::SetText(object(999), "x".to_owned()),
    ] {
        let error = edits.push(&base, edit).expect_err("refused");
        assert!(matches!(error, AnnotError::NotEditable), "{error:?}");
        assert_eq!(error.message_es(), "Esa anotación no se puede cambiar así.");
    }
    assert!(edits.is_empty(), "nothing refused is kept");
}

#[test]
fn text_outside_win_ansi_is_refused_and_not_kept() {
    let base = annotated();
    let [text, highlight, _, _] = ids(&base);
    let mut edits = EditSet::new();
    let error = edits
        .push(&base, Edit::SetText(text, "Привет".to_owned()))
        .expect_err("Cyrillic is not in WinAnsi");
    assert!(matches!(error, AnnotError::Unencodable('П')), "{error:?}");
    let added = edit::text_box(a4(), 0, Rect::new(10.0, 10.0, 200.0, 20.0), "→", 12.0, BLUE);
    assert!(matches!(
        edits.push(&base, Edit::Add(added)),
        Err(AnnotError::Unencodable('→'))
    ));
    assert!(matches!(
        edits.push(&base, Edit::SetText(highlight, "x".to_owned())),
        Err(AnnotError::NotEditable)
    ));
    assert!(edits.is_empty());
}

#[test]
fn rich_text_moves_and_goes_but_keeps_its_text_and_colour() {
    let rich = Annotation {
        id: object(40),
        page: 0,
        kind: Kind::Text {
            contents: "rich".to_owned(),
            size_pt: 12.0,
            rich: true,
        },
        rect: Rect::new(100.0, 100.0, 200.0, 130.0),
        color: Rgb::BLACK,
        author: Some("Another program".to_owned()),
    };
    let base = Base {
        annotations: vec![rich],
        boxes: vec![a4()],
        fields: Vec::new(),
    };
    let mut edits = EditSet::new();
    assert!(matches!(
        edits.push(&base, Edit::SetText(object(40), "plain".to_owned())),
        Err(AnnotError::NotEditable)
    ));
    assert!(matches!(
        edits.push(&base, Edit::Recolor(object(40), BLUE)),
        Err(AnnotError::NotEditable)
    ));
    edits
        .push(
            &base,
            Edit::Move(object(40), Rect::new(110.0, 100.0, 210.0, 130.0)),
        )
        .expect("it moves");
    edits
        .push(&base, Edit::Delete(object(40)))
        .expect("it goes");
}

#[test]
fn undo_and_redo_walk_the_edits_and_a_new_edit_forgets_the_undone() {
    let base = annotated();
    let [text, highlight, ink, _] = ids(&base);
    let mut edits = EditSet::new();
    assert!(!edits.can_undo() && !edits.can_redo());
    edits.push(&base, Edit::Delete(ink.clone())).expect("ok");
    edits
        .push(&base, Edit::Recolor(highlight.clone(), BLUE))
        .expect("ok");
    assert_eq!(edits.edits().len(), 2);

    assert!(edits.undo());
    let shown = edits.apply(&base);
    assert_eq!(
        shown
            .iter()
            .find(|a| a.id == highlight)
            .expect("there")
            .color,
        Rgb::YELLOW,
        "the colour is back"
    );
    assert!(edits.can_redo());
    assert!(edits.redo());
    assert_eq!(
        edits
            .apply(&base)
            .iter()
            .find(|a| a.id == highlight)
            .expect("there")
            .color,
        BLUE
    );
    assert!(edits.undo() && edits.undo());
    assert!(!edits.undo(), "nothing more to undo");
    assert_eq!(edits.apply(&base), base.annotations, "back to the file");
    assert!(edits.is_empty() && edits.can_redo());

    edits.push(&base, Edit::Delete(text)).expect("ok");
    assert!(!edits.can_redo(), "a new edit forgets what was undone");
    assert!(!edits.redo());
}

#[test]
fn an_edit_on_what_was_undone_is_refused() {
    let base = Base {
        annotations: Vec::new(),
        boxes: vec![a4()],
        fields: Vec::new(),
    };
    let added = edit::text_box(
        a4(),
        0,
        Rect::new(10.0, 10.0, 200.0, 20.0),
        "hola",
        12.0,
        BLUE,
    );
    let id = added.id.clone();
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(added)).expect("added");
    assert!(edits.undo());
    assert!(matches!(
        edits.push(&base, Edit::Recolor(id, BLUE)),
        Err(AnnotError::NotEditable)
    ));
}

#[test]
fn the_writer_is_asked_only_for_what_changed() {
    let base = annotated();
    let [text, highlight, ink, _] = ids(&base);
    let added = edit::text_box(
        a4(),
        0,
        Rect::new(10.0, 10.0, 200.0, 20.0),
        "nuevo",
        12.0,
        BLUE,
    );
    let gone = edit::text_box(
        a4(),
        0,
        Rect::new(10.0, 50.0, 200.0, 60.0),
        "fugaz",
        12.0,
        BLUE,
    );
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(added.clone())).expect("ok");
    edits.push(&base, Edit::Add(gone.clone())).expect("ok");
    edits.push(&base, Edit::Delete(gone.id)).expect("ok");
    edits
        .push(&base, Edit::Recolor(highlight.clone(), BLUE))
        .expect("ok");
    edits
        .push(&base, Edit::Recolor(highlight.clone(), Rgb::YELLOW))
        .expect("and back");
    edits.push(&base, Edit::Delete(ink.clone())).expect("ok");
    edits
        .push(&base, Edit::SetText(text.clone(), "otro".to_owned()))
        .expect("ok");

    let resolved = edits.resolve(&base);
    assert_eq!(resolved.added.len(), 1);
    assert_eq!(resolved.added[0].id, added.id);
    assert_eq!(resolved.changed.len(), 1, "the highlight is back as it was");
    assert_eq!(resolved.changed[0].id, text);
    assert_eq!(resolved.deleted, vec![ink]);
    assert!(EditSet::new().resolve(&base).is_empty());
}

#[test]
fn a_new_text_box_grows_to_its_lines_and_keeps_its_corner() {
    let base = Base {
        annotations: Vec::new(),
        boxes: vec![a4()],
        fields: Vec::new(),
    };
    let view = Rect::new(72.0, 72.0, 272.0, 80.0);
    let added = edit::text_box(a4(), 0, view, "uno\ndos", 14.0, BLUE);
    let mut edits = EditSet::new();
    edits.push(&base, Edit::Add(added)).expect("ok");
    let shown = edits.apply(&base);
    let fitted = to_view_rect(a4(), shown[0].rect);
    assert_eq!((fitted.x0, fitted.y0, fitted.x1), (72.0, 72.0, 272.0));
    assert!((fitted.height() - (2.0 * 2.0 + 2.0 * 1.2 * 14.0)).abs() < 1e-9);
}

#[test]
fn new_ids_are_names_that_never_repeat() {
    let first = edit::new_id();
    let second = edit::new_id();
    assert_ne!(first, second);
    assert!(matches!(&first, AnnotId::Name(name) if name.starts_with("calcita-")));
}

#[test]
fn a_highlight_takes_one_quad_per_selection_box_upright_as_shown() {
    let turned = PageBox {
        rotate: Rotation::R90,
        ..a4()
    };
    let boxes = [
        Rect::new(100.0, 50.0, 300.0, 64.0),
        Rect::new(100.0, 70.0, 200.0, 84.0),
        Rect::new(5.0, 5.0, 5.0, 9.0),
    ];
    let made = edit::highlight(turned, 2, &boxes, Rgb::YELLOW).expect("boxes");
    assert_eq!(made.page, 2);
    let Kind::Highlight { quads } = &made.kind else {
        panic!("{made:?}");
    };
    assert_eq!(quads.len(), 2, "an empty box is dropped");
    // The upper-left corner as shown: (100, 50) on a page turned a quarter
    // is x = Y, y = X in user space.
    assert_eq!(quads[0].points[0], Point { x: 50.0, y: 100.0 });
    assert_eq!(quads[0].points[3], Point { x: 64.0, y: 300.0 });
    assert_eq!(made.rect, Rect::new(50.0, 100.0, 84.0, 300.0));
    assert!(edit::highlight(turned, 0, &[], Rgb::YELLOW).is_none());
}

#[test]
fn a_stroke_drops_points_too_close_and_is_bounded() {
    let points = [
        Point { x: 10.0, y: 10.0 },
        Point { x: 10.05, y: 10.0 },
        Point { x: 20.0, y: 10.0 },
        Point {
            x: f64::NAN,
            y: 3.0,
        },
    ];
    let made = edit::ink(a4(), 0, &points, 3.0, BLUE).expect("points");
    let Kind::Ink { strokes, width } = &made.kind else {
        panic!("{made:?}");
    };
    assert_eq!(*width, 3.0);
    assert_eq!(
        strokes[0],
        vec![Point { x: 10.0, y: 832.0 }, Point { x: 20.0, y: 832.0 }]
    );
    let long: Vec<Point> = (0..20_000)
        .map(|step| Point {
            x: f64::from(step),
            y: 0.0,
        })
        .collect();
    let made = edit::ink(a4(), 0, &long, 1.0, BLUE).expect("points");
    let Kind::Ink { strokes, .. } = &made.kind else {
        panic!("{made:?}");
    };
    assert_eq!(strokes[0].len(), edit::MAX_STROKE_POINTS);
    assert!(edit::ink(a4(), 0, &[], 1.0, BLUE).is_none());
}

#[test]
fn another_programs_text_box_moves_to_the_rectangle_given() {
    let base = annotated();
    let [text, _, _, _] = ids(&base);
    let mut edits = EditSet::new();
    // The fixture's box (220 x 60) carried to the page's corner: kept where
    // it is put, not re-fitted.
    let target = Rect::new(10.0, 10.0, 230.0, 70.0);
    edits
        .push(&base, Edit::Move(text.clone(), target))
        .expect("moved");
    assert_eq!(edits.apply(&base)[0].rect, target);
    assert!(!edit::lays_out_again(
        Some(&base.annotations[0]),
        &edits.apply(&base)[0]
    ));
    // Once its text changes, Calcita lays it out, and it grows to its lines.
    edits
        .push(
            &base,
            Edit::SetText(text, "otro texto que ocupa varias lineas, ".repeat(6)),
        )
        .expect("retexted");
    let shown = &edits.apply(&base)[0];
    assert!(edit::lays_out_again(Some(&base.annotations[0]), shown));
    assert!(shown.rect.height() > target.height());
}

#[test]
fn an_untouched_foreign_box_only_moves_at_its_own_size() {
    let base = annotated();
    let [text, _, _, _] = ids(&base);
    let original = base.annotations[0].rect;
    let mut edits = EditSet::new();
    let wider = Rect::new(original.x0, original.y0, original.x1 + 40.0, original.y1);
    assert!(
        matches!(
            edits.push(&base, Edit::Move(text.clone(), wider)),
            Err(AnnotError::NotEditable)
        ),
        "a size change would stretch its own appearance"
    );
    // A translation a hair off its size keeps its size exactly.
    let moved = Rect::new(
        original.x0 + 5.0,
        original.y0,
        original.x1 + 5.004,
        original.y1,
    );
    edits.push(&base, Edit::Move(text, moved)).expect("moved");
    let shown = edits.apply(&base)[0].rect;
    assert_eq!(
        (shown.width(), shown.height()),
        (original.width(), original.height())
    );
    assert!(!edit::lays_out_again(
        Some(&base.annotations[0]),
        &edits.apply(&base)[0]
    ));
}
