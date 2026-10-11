//! The conversion between PDF user space and the displayed page's points,
//! checked against the values the CAL-2 spike measured on QtPdf's renders
//! (a highlight at `[70 752 180 786]` on an A4 page, six variants).

use calcita_core::annot::coords::{to_pdf, to_pdf_rect, to_view, to_view_rect, PageBox, Rotation};
use calcita_core::annot::{Point, Rect};

const A4: (f64, f64) = (595.0, 842.0);

fn page(x0: f64, y0: f64, rotate: Rotation) -> PageBox {
    PageBox {
        x0,
        y0,
        w: A4.0,
        h: A4.1,
        rotate,
    }
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// The spike's highlight, in PDF user space of an unshifted page.
fn highlight() -> Rect {
    Rect::new(70.0, 752.0, 180.0, 786.0)
}

fn shifted(rect: Rect, dx: f64, dy: f64) -> Rect {
    Rect::new(rect.x0 + dx, rect.y0 + dy, rect.x1 + dx, rect.y1 + dy)
}

#[test]
fn an_upright_page_flips_the_vertical_axis() {
    let upright = page(0.0, 0.0, Rotation::R0);
    assert_eq!(to_view(upright, point(70.0, 786.0)), point(70.0, 56.0));
    assert_eq!(
        to_view_rect(upright, highlight()),
        Rect::new(70.0, 56.0, 180.0, 90.0)
    );
}

#[test]
fn each_rotation_lands_where_qtpdf_drew_the_highlight() {
    // The view boxes QtPdf's renders showed the yellow in, at one pixel per
    // point, top-left origin.
    let cases = [
        (Rotation::R0, Rect::new(70.0, 56.0, 180.0, 90.0)),
        (Rotation::R90, Rect::new(752.0, 70.0, 786.0, 180.0)),
        (Rotation::R180, Rect::new(415.0, 752.0, 525.0, 786.0)),
        (Rotation::R270, Rect::new(56.0, 415.0, 90.0, 525.0)),
    ];
    for (rotate, expected) in cases {
        assert_eq!(
            to_view_rect(page(0.0, 0.0, rotate), highlight()),
            expected,
            "{rotate:?}"
        );
    }
}

#[test]
fn an_offset_box_moves_the_origin_and_nothing_else() {
    for rotate in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
        let plain = to_view_rect(page(0.0, 0.0, rotate), highlight());
        let offset = to_view_rect(page(50.0, 100.0, rotate), shifted(highlight(), 50.0, 100.0));
        assert_eq!(offset, plain, "{rotate:?}");
    }
}

#[test]
fn the_spike_measured_the_offset_box_exactly() {
    // The spike shifted the box by (50, 100) and left the highlight where it
    // was in user space; these are the yellow pixels' bounds it measured.
    let cases = [
        (Rotation::R0, Rect::new(20.0, 156.0, 130.0, 190.0)),
        (Rotation::R90, Rect::new(652.0, 20.0, 686.0, 130.0)),
        (Rotation::R180, Rect::new(465.0, 652.0, 575.0, 686.0)),
        (Rotation::R270, Rect::new(156.0, 465.0, 190.0, 575.0)),
    ];
    for (rotate, expected) in cases {
        assert_eq!(
            to_view_rect(page(50.0, 100.0, rotate), highlight()),
            expected,
            "{rotate:?}"
        );
    }
}

#[test]
fn the_displayed_size_swaps_on_a_quarter_turn() {
    assert_eq!(page(0.0, 0.0, Rotation::R0).view_size(), A4);
    assert_eq!(page(0.0, 0.0, Rotation::R180).view_size(), A4);
    assert_eq!(page(0.0, 0.0, Rotation::R90).view_size(), (A4.1, A4.0));
    assert_eq!(page(0.0, 0.0, Rotation::R270).view_size(), (A4.1, A4.0));
}

#[test]
fn view_and_pdf_are_inverse() {
    let corners = [
        point(0.0, 0.0),
        point(595.0, 842.0),
        point(70.0, 752.0),
        point(180.5, 786.25),
        point(12.0, 830.0),
    ];
    for rotate in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
        let paper = page(50.0, 100.0, rotate);
        for corner in corners {
            let user = point(corner.x + 50.0, corner.y + 100.0);
            assert_eq!(to_pdf(paper, to_view(paper, user)), user, "{rotate:?}");
            assert_eq!(to_view(paper, to_pdf(paper, corner)), corner, "{rotate:?}");
        }
        let rect = shifted(highlight(), 50.0, 100.0);
        assert_eq!(to_pdf_rect(paper, to_view_rect(paper, rect)), rect);
    }
}

#[test]
fn a_rotation_counts_whole_quarter_turns_as_pdfium_does() {
    assert_eq!(Rotation::from_degrees(0), Rotation::R0);
    assert_eq!(Rotation::from_degrees(90), Rotation::R90);
    assert_eq!(Rotation::from_degrees(-90), Rotation::R270);
    assert_eq!(Rotation::from_degrees(-180), Rotation::R180);
    assert_eq!(Rotation::from_degrees(-270), Rotation::R90);
    assert_eq!(Rotation::from_degrees(450), Rotation::R90);
    assert_eq!(Rotation::from_degrees(540), Rotation::R180);
    // Not a multiple of 90: PDFium truncates the quarter turns, so 100 is
    // one turn, 45 none and -100 one turn back.
    assert_eq!(Rotation::from_degrees(100), Rotation::R90);
    assert_eq!(Rotation::from_degrees(179), Rotation::R90);
    assert_eq!(Rotation::from_degrees(45), Rotation::R0);
    assert_eq!(Rotation::from_degrees(-45), Rotation::R0);
    assert_eq!(Rotation::from_degrees(-100), Rotation::R270);
}

#[test]
fn a_real_rotation_is_truncated_to_whole_degrees() {
    assert_eq!(Rotation::from_number(90.0), Rotation::R90);
    assert_eq!(Rotation::from_number(100.0), Rotation::R90);
    assert_eq!(Rotation::from_number(269.9), Rotation::R180);
    assert_eq!(Rotation::from_number(-90.0), Rotation::R270);
    assert_eq!(Rotation::from_number(f64::NAN), Rotation::R0);
}

#[test]
fn a_rectangle_is_normalized_whatever_corners_it_is_given() {
    assert_eq!(
        Rect::new(180.0, 786.0, 70.0, 752.0),
        Rect::new(70.0, 752.0, 180.0, 786.0)
    );
    let rect = Rect::new(10.0, 20.0, 30.0, 60.0);
    assert_eq!((rect.width(), rect.height()), (20.0, 40.0));
}
