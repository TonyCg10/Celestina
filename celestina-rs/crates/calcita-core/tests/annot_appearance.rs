// language-contract: product-copy
//! The text layout and the appearance streams Calcita writes.

use calcita_core::annot::appearance::{
    fit_text_rect, free_text_ap, highlight_ap, ink_ap, ink_rect, number, text_height, text_lines,
    win_ansi, Resources, LINE_HEIGHT, TEXT_PADDING,
};
use calcita_core::annot::coords::{to_view_rect, PageBox, Rotation};
use calcita_core::annot::{AnnotError, Point, Quad, Rect, Rgb};

fn a4(rotate: Rotation) -> PageBox {
    PageBox {
        x0: 0.0,
        y0: 0.0,
        w: 595.0,
        h: 842.0,
        rotate,
    }
}

#[test]
fn spanish_and_typographic_text_encodes_in_win_ansi() {
    assert_eq!(
        win_ansi("Año ñandú ¿qué? ¡sí!").expect("encodable"),
        b"A\xF1o \xF1and\xFA \xBFqu\xE9? \xA1s\xED!".to_vec()
    );
    assert_eq!(win_ansi("5 €").expect("encodable"), b"5 \x80".to_vec());
    assert_eq!(
        win_ansi("«cita» — “otra” …").expect("encodable"),
        b"\xABcita\xBB \x97 \x93otra\x94 \x85".to_vec()
    );
    assert_eq!(
        win_ansi("a\r\nb\rc\td").expect("encodable"),
        b"a\nb\nc d".to_vec()
    );
}

#[test]
fn a_character_outside_win_ansi_is_refused_by_name() {
    let error = win_ansi("hola 日本").expect_err("CJK is not in WinAnsi");
    assert!(matches!(error, AnnotError::Unencodable('日')), "{error:?}");
    assert_eq!(
        error.message_es(),
        "El carácter «日» no se puede escribir en un cuadro de texto."
    );
    assert!(matches!(
        win_ansi("\u{1F600}"),
        Err(AnnotError::Unencodable('\u{1F600}'))
    ));
}

#[test]
fn lines_break_at_spaces_with_helvetica_widths() {
    // "Hello" is 2278 thousandths of an em in Helvetica, a space 278: at
    // 10 pt "Hello Hello" is 48.34 pt, so 50 pt of text room keeps it on
    // one line and 40 pt breaks it at the space.
    let one = text_lines("Hello Hello", 50.0 + TEXT_PADDING * 2.0, 10.0).expect("laid out");
    assert_eq!(one, vec![b"Hello Hello".to_vec()]);
    let two = text_lines("Hello Hello", 40.0 + TEXT_PADDING * 2.0, 10.0).expect("laid out");
    assert_eq!(two, vec![b"Hello".to_vec(), b"Hello".to_vec()]);
}

#[test]
fn a_word_wider_than_the_box_breaks_inside_and_paragraphs_stay() {
    let lines = text_lines("WWWWWWWW\n\nab", 30.0 + TEXT_PADDING * 2.0, 10.0).expect("laid out");
    // W is 944 thousandths: three fit in 30 pt at 10 pt.
    assert_eq!(
        lines,
        vec![
            b"WWW".to_vec(),
            b"WWW".to_vec(),
            b"WW".to_vec(),
            Vec::new(),
            b"ab".to_vec()
        ]
    );
}

#[test]
fn a_box_is_as_tall_as_its_lines() {
    let height = text_height("uno\ndos\ntres", 200.0, 10.0).expect("laid out");
    assert!((height - (TEXT_PADDING * 2.0 + 3.0 * LINE_HEIGHT * 10.0)).abs() < 1e-9);
    assert!(
        (text_height("", 200.0, 10.0).expect("laid out") - (TEXT_PADDING * 2.0 + 12.0)).abs()
            < 1e-9
    );
}

#[test]
fn fitting_lowers_the_bottom_edge_as_the_page_is_shown() {
    let upright = a4(Rotation::R0);
    let fitted = fit_text_rect(upright, Rect::new(100.0, 700.0, 300.0, 710.0), "a\nb", 10.0)
        .expect("laid out");
    assert_eq!((fitted.x0, fitted.x1, fitted.y1), (100.0, 300.0, 710.0));
    assert!((fitted.y0 - (710.0 - (TEXT_PADDING * 2.0 + 24.0))).abs() < 1e-9);

    // On a page turned a quarter, "down" as shown is +x in user space.
    let turned = a4(Rotation::R90);
    let view = Rect::new(100.0, 100.0, 300.0, 110.0);
    let fitted = fit_text_rect(turned, to_pdf_view(turned, view), "a\nb", 10.0).expect("laid out");
    let shown = to_view_rect(turned, fitted);
    assert!((shown.x0 - 100.0).abs() < 1e-9 && (shown.y0 - 100.0).abs() < 1e-9);
    assert!((shown.height() - (TEXT_PADDING * 2.0 + 24.0)).abs() < 1e-9);
    assert!((shown.width() - 200.0).abs() < 1e-9);
}

fn to_pdf_view(page: PageBox, view: Rect) -> Rect {
    calcita_core::annot::coords::to_pdf_rect(page, view)
}

#[test]
fn numbers_are_written_plainly() {
    assert_eq!(number(1.0), "1");
    assert_eq!(number(0.5), "0.5");
    assert_eq!(number(-0.0001), "0");
    assert_eq!(number(12.345_6), "12.346");
    assert_eq!(number(1e-12), "0");
    assert_eq!(number(f64::NAN), "0");
}

#[test]
fn a_text_appearance_draws_each_line_in_helvetica_upright() {
    let rect = Rect::new(100.0, 700.0, 300.0, 740.0);
    let appearance = free_text_ap(
        a4(Rotation::R0),
        rect,
        "Hola\nseñor",
        12.0,
        Rgb { r: 255, g: 0, b: 0 },
    )
    .expect("laid out");
    assert_eq!(appearance.bbox, rect);
    assert_eq!(appearance.resources, Resources::Helvetica);
    let content = String::from_utf8(appearance.content).expect("ASCII");
    assert!(
        content.starts_with("q 1 0 0 1 100 700 cm 0 0 200 40 re W n BT /Helv 12 Tf 1 0 0 rg"),
        "{content}"
    );
    assert!(
        content.contains("<486F6C61> Tj 0 -14.4 Td <7365F16F72> Tj ET Q"),
        "{content}"
    );

    // Turned a quarter, the text turns back so it reads upright.
    let turned = free_text_ap(a4(Rotation::R90), rect, "x", 12.0, Rgb::BLACK).expect("laid out");
    let content = String::from_utf8(turned.content).expect("ASCII");
    assert!(content.starts_with("q 0 1 -1 0 "), "{content}");
}

#[test]
fn a_highlight_multiplies_each_quad() {
    let quad = Quad {
        points: [
            Point { x: 10.0, y: 20.0 },
            Point { x: 50.0, y: 20.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: 50.0, y: 10.0 },
        ],
    };
    let appearance = highlight_ap(Rect::new(10.0, 10.0, 50.0, 20.0), &[quad], Rgb::YELLOW);
    assert_eq!(appearance.resources, Resources::Multiply);
    assert_eq!(
        String::from_utf8(appearance.content).expect("ASCII"),
        "q /Multiply gs 1 1 0 rg 10 20 m 50 20 l 50 10 l 10 10 l h f Q"
    );
}

#[test]
fn a_drawing_strokes_round_lines_and_dots() {
    let strokes = vec![
        vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }],
        vec![Point { x: 5.0, y: 5.0 }],
    ];
    let rect = ink_rect(&strokes, 2.0).expect("points");
    assert_eq!(rect, Rect::new(-1.0, 0.0, 7.0, 7.0));
    let appearance = ink_ap(rect, &strokes, 2.0, Rgb { r: 0, g: 0, b: 255 });
    assert_eq!(appearance.resources, Resources::None);
    assert_eq!(
        String::from_utf8(appearance.content).expect("ASCII"),
        "q 0 0 1 RG 2 w 1 J 1 j 1 2 m 3 4 l S 5 5 m 5 5 l S Q"
    );
    assert_eq!(ink_rect(&[], 2.0), None);
}
