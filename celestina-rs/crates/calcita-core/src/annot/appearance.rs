//! The appearance streams Calcita writes for its three kinds, and the text
//! layout a text box's appearance and its height share.
//!
//! An appearance (`/AP /N`) is what every viewer draws; without one each
//! program improvises. Calcita writes it for every annotation it adds or
//! changes, as a form XObject whose `/BBox` is the annotation's `/Rect`
//! in page space with the identity `/Matrix`, so the box maps onto the
//! rectangle unchanged:
//!
//! - a text box: Helvetica, one of the standard fonts every PDF viewer has,
//!   so nothing is embedded, in `WinAnsiEncoding` (which covers Spanish);
//!   the lines are broken with Helvetica's own widths and drawn upright on
//!   the page as it is shown, whatever its `/Rotate`;
//! - a highlight: each quadrilateral filled in its colour under the
//!   multiply blend mode, so the text under it stays dark;
//! - a drawing: each stroke as a round-capped, round-joined line.

use super::coords::{to_pdf, to_pdf_rect, to_view_rect, PageBox};
use super::model::{Point, Quad, Rect, Rgb};
use super::read::AnnotError;

/// The space between a text box's edge and its text, in points.
pub const TEXT_PADDING: f64 = 2.0;

/// The distance between two baselines, in font sizes.
pub const LINE_HEIGHT: f64 = 1.2;

/// How far the first baseline sits below the padding, in font sizes: the
/// line's share above its baseline (Helvetica's ascender is 0.718, the rest
/// of the line height split over and under it).
const FIRST_BASELINE: f64 = 0.95;

/// The name of the font in a text box's resources and in its `/DA`.
pub const FONT_RESOURCE: &str = "Helv";

/// The name of the multiply graphics state in a highlight's resources.
pub const MULTIPLY_RESOURCE: &str = "Multiply";

/// Helvetica's advance widths, in thousandths of the font size, for each
/// `WinAnsiEncoding` code (Adobe's Helvetica AFM, the standard-14 metrics
/// every viewer lays the font out with); 0 for a code WinAnsi leaves
/// undefined.
const HELVETICA_WIDTHS: [u16; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
    0, 556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 222, 222,
    333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667, 278, 333, 556, 556, 556, 556,
    260, 556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 556, 537, 278, 333,
    333, 365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667,
    278, 278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667,
    611, 556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278, 556, 556,
    556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];

/// What an appearance stream needs besides its content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resources {
    /// Nothing: a drawing's strokes.
    None,
    /// Helvetica under [`FONT_RESOURCE`]: a text box.
    Helvetica,
    /// The multiply blend mode under [`MULTIPLY_RESOURCE`]: a highlight.
    Multiply,
}

/// One appearance stream: its box in page space and its content.
#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    pub bbox: Rect,
    pub content: Vec<u8>,
    pub resources: Resources,
}

/// The `WinAnsiEncoding` code of `character`, if it has one.
fn win_ansi_code(character: char) -> Option<u8> {
    let code = u32::from(character);
    if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
        return u8::try_from(code).ok();
    }
    let code = match character {
        '\u{20AC}' => 0x80,
        '\u{201A}' => 0x82,
        '\u{0192}' => 0x83,
        '\u{201E}' => 0x84,
        '\u{2026}' => 0x85,
        '\u{2020}' => 0x86,
        '\u{2021}' => 0x87,
        '\u{02C6}' => 0x88,
        '\u{2030}' => 0x89,
        '\u{0160}' => 0x8A,
        '\u{2039}' => 0x8B,
        '\u{0152}' => 0x8C,
        '\u{017D}' => 0x8E,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201C}' => 0x93,
        '\u{201D}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{02DC}' => 0x98,
        '\u{2122}' => 0x99,
        '\u{0161}' => 0x9A,
        '\u{203A}' => 0x9B,
        '\u{0153}' => 0x9C,
        '\u{017E}' => 0x9E,
        '\u{0178}' => 0x9F,
        _ => return None,
    };
    Some(code)
}

/// `text` in `WinAnsiEncoding`, a line break kept as `\n`: a carriage
/// return before a line feed is dropped, alone it breaks the line, and a
/// tab is a space.
///
/// # Errors
///
/// [`AnnotError::Unencodable`] with the first character Helvetica in
/// WinAnsi cannot show.
pub fn win_ansi(text: &str) -> Result<Vec<u8>, AnnotError> {
    let mut encoded = Vec::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' => encoded.push(b'\n'),
            '\r' => {
                if characters.peek() != Some(&'\n') {
                    encoded.push(b'\n');
                }
            }
            '\t' => encoded.push(b' '),
            other => encoded.push(win_ansi_code(other).ok_or(AnnotError::Unencodable(other))?),
        }
    }
    Ok(encoded)
}

/// The advance of `bytes` in thousandths of the font size.
fn advance(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .map(|byte| u32::from(HELVETICA_WIDTHS[usize::from(*byte)]))
        .sum()
}

/// One paragraph broken into lines no wider than `room` (thousandths of
/// the font size): at the last space that fits, else inside the word; the
/// spaces a break falls on are dropped.
fn break_paragraph(paragraph: &[u8], room: u32, lines: &mut Vec<Vec<u8>>) {
    let mut line: Vec<u8> = Vec::new();
    for &byte in paragraph {
        line.push(byte);
        // A line of one character stays, however wide: it cannot break.
        while advance(&line) > room && line.len() > 1 {
            let words_before =
                |space: usize| line[..space].iter().any(|candidate| *candidate != b' ');
            let cut = match line.iter().rposition(|candidate| *candidate == b' ') {
                Some(space) if words_before(space) => space,
                _ => line.len() - 1,
            };
            let mut carried = line.split_off(cut);
            while line.last() == Some(&b' ') {
                line.pop();
            }
            let leading = carried.iter().take_while(|byte| **byte == b' ').count();
            carried.drain(..leading);
            lines.push(std::mem::replace(&mut line, carried));
        }
    }
    lines.push(line);
}

/// `text` laid out in a box `width` points wide at `size` points: its
/// lines in `WinAnsiEncoding`, at least one.
///
/// # Errors
///
/// As [`win_ansi`].
pub fn text_lines(text: &str, width: f64, size: f64) -> Result<Vec<Vec<u8>>, AnnotError> {
    let encoded = win_ansi(text)?;
    let room = ((width - TEXT_PADDING * 2.0).max(0.0) * 1000.0 / size.max(f64::MIN_POSITIVE))
        .floor()
        .clamp(0.0, f64::from(u32::MAX));
    // Clamped to the range of u32 first, so the cast cannot wrap.
    let room = room as u32;
    let mut lines = Vec::new();
    for paragraph in encoded.split(|byte| *byte == b'\n') {
        break_paragraph(paragraph, room, &mut lines);
    }
    Ok(lines)
}

/// The height a text box `width` points wide needs for `text` at `size`.
///
/// # Errors
///
/// As [`win_ansi`].
pub fn text_height(text: &str, width: f64, size: f64) -> Result<f64, AnnotError> {
    let lines = text_lines(text, width, size)?.len().max(1);
    // A text box has far fewer lines than f64 counts exactly.
    #[allow(clippy::cast_precision_loss)]
    let lines = lines as f64;
    Ok(TEXT_PADDING * 2.0 + lines * LINE_HEIGHT * size)
}

/// The narrowest a text box may be: its padding and one em.
#[must_use]
pub fn min_text_width(size: f64) -> f64 {
    TEXT_PADDING * 2.0 + size
}

/// `rect` grown, as the page is shown, until `text` fits it at `size`:
/// at least [`min_text_width`] wide, and its bottom edge lowered to the
/// height the lines need. The top-left corner stays.
///
/// # Errors
///
/// As [`win_ansi`].
pub fn fit_text_rect(page: PageBox, rect: Rect, text: &str, size: f64) -> Result<Rect, AnnotError> {
    let mut view = to_view_rect(page, rect);
    view.x1 = view.x1.max(view.x0 + min_text_width(size));
    let needed = text_height(text, view.width(), size)?;
    view.y1 = view.y1.max(view.y0 + needed);
    Ok(to_pdf_rect(page, view))
}

/// A number as a content stream writes it: at most three decimals, no
/// exponent, no trailing zeros, no negative zero.
#[must_use]
pub fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let fixed = format!("{value:.3}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    match trimmed {
        "-0" | "" => "0".to_owned(),
        other => other.to_owned(),
    }
}

/// A colour's three components for `rg` or `RG`.
fn components(color: Rgb) -> String {
    let unit = |channel: u8| number(f64::from(channel) / 255.0);
    format!("{} {} {}", unit(color.r), unit(color.g), unit(color.b))
}

/// `bytes` as a PDF hexadecimal string.
fn hex_string(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2 + 2);
    text.push('<');
    for byte in bytes {
        text.push_str(&format!("{byte:02X}"));
    }
    text.push('>');
    text
}

/// A text box's appearance: `text` at `size` points in `color`, laid out
/// upright in `rect` as `page` is shown and clipped to it.
///
/// # Errors
///
/// As [`win_ansi`].
pub fn free_text_ap(
    page: PageBox,
    rect: Rect,
    text: &str,
    size: f64,
    color: Rgb,
) -> Result<Appearance, AnnotError> {
    let view = to_view_rect(page, rect);
    let (width, height) = (view.width(), view.height());
    let lines = text_lines(text, width, size)?;
    // The box's own space: origin at its bottom-left as shown, x to the
    // right and y up as the reader sees them; the matrix takes it to page
    // space through the one conversion.
    let origin = to_pdf(
        page,
        Point {
            x: view.x0,
            y: view.y1,
        },
    );
    let across = to_pdf(
        page,
        Point {
            x: view.x0 + 1.0,
            y: view.y1,
        },
    );
    let up = to_pdf(
        page,
        Point {
            x: view.x0,
            y: view.y1 - 1.0,
        },
    );
    let mut content = format!(
        "q {} {} {} {} {} {} cm 0 0 {} {} re W n BT /{FONT_RESOURCE} {} Tf {} rg {} {} Td",
        number(across.x - origin.x),
        number(across.y - origin.y),
        number(up.x - origin.x),
        number(up.y - origin.y),
        number(origin.x),
        number(origin.y),
        number(width),
        number(height),
        number(size),
        components(color),
        number(TEXT_PADDING),
        number(height - TEXT_PADDING - FIRST_BASELINE * size),
    );
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            content.push_str(&format!(" 0 {} Td", number(-LINE_HEIGHT * size)));
        }
        content.push_str(&format!(" {} Tj", hex_string(line)));
    }
    content.push_str(" ET Q");
    Ok(Appearance {
        bbox: rect,
        content: content.into_bytes(),
        resources: Resources::Helvetica,
    })
}

/// A highlight's appearance: each of `quads` filled in `color` under the
/// multiply blend mode, in the box `rect`.
#[must_use]
pub fn highlight_ap(rect: Rect, quads: &[Quad], color: Rgb) -> Appearance {
    let mut content = format!("q /{MULTIPLY_RESOURCE} gs {} rg", components(color));
    for quad in quads {
        // `/QuadPoints` order: upper left, upper right, lower left, lower
        // right; the outline goes round them.
        let [a, b, c, d] = quad.points;
        for (point, operator) in [(a, "m"), (b, "l"), (d, "l"), (c, "l")] {
            content.push_str(&format!(
                " {} {} {operator}",
                number(point.x),
                number(point.y)
            ));
        }
        content.push_str(" h f");
    }
    content.push_str(" Q");
    Appearance {
        bbox: rect,
        content: content.into_bytes(),
        resources: Resources::Multiply,
    }
}

/// A drawing's appearance: each of `strokes` as a line `width` points wide
/// in `color`, round at its ends and joins (a single point is a dot), in
/// the box `rect`.
#[must_use]
pub fn ink_ap(rect: Rect, strokes: &[Vec<Point>], width: f64, color: Rgb) -> Appearance {
    let mut content = format!("q {} RG {} w 1 J 1 j", components(color), number(width));
    for stroke in strokes {
        let Some(first) = stroke.first() else {
            continue;
        };
        content.push_str(&format!(" {} {} m", number(first.x), number(first.y)));
        let rest = if stroke.len() == 1 {
            &stroke[..]
        } else {
            &stroke[1..]
        };
        for point in rest {
            content.push_str(&format!(" {} {} l", number(point.x), number(point.y)));
        }
        content.push_str(" S");
    }
    content.push_str(" Q");
    Appearance {
        bbox: rect,
        content: content.into_bytes(),
        resources: Resources::None,
    }
}

/// The box a drawing's strokes need: their bounds grown by half the line
/// width and one point more, so round caps are not clipped. `None` without
/// a point.
#[must_use]
pub fn ink_rect(strokes: &[Vec<Point>], width: f64) -> Option<Rect> {
    let margin = width / 2.0 + 1.0;
    bounds(strokes.iter().flatten()).map(|bounds| Rect {
        x0: bounds.x0 - margin,
        y0: bounds.y0 - margin,
        x1: bounds.x1 + margin,
        y1: bounds.y1 + margin,
    })
}

/// The box a highlight's quadrilaterals cover. `None` without one.
#[must_use]
pub fn highlight_rect(quads: &[Quad]) -> Option<Rect> {
    bounds(quads.iter().flat_map(|quad| quad.points.iter()))
}

fn bounds<'a>(points: impl Iterator<Item = &'a Point>) -> Option<Rect> {
    points.fold(None, |found: Option<Rect>, point| {
        Some(match found {
            None => Rect::new(point.x, point.y, point.x, point.y),
            Some(rect) => Rect {
                x0: rect.x0.min(point.x),
                y0: rect.y0.min(point.y),
                x1: rect.x1.max(point.x),
                y1: rect.y1.max(point.y),
            },
        })
    })
}
