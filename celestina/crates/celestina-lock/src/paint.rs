//! The CPU painter's primitives (design §4.6.1, H-15): one embedded font, text
//! through `cosmic-text`, shapes through `tiny-skia`, pixels into `wl_shm`.
//!
//! The lock draws a scrim, two rounded capsules, a row of bullets and a few
//! lines of text, so nothing here needs a GPU. And because this is the one
//! process that can end the session lock, nothing here reads anything from the
//! person's account either:
//!
//! * the single face, Inter, is compiled into the binary and loaded into an
//!   **empty** `fontdb::Database` handed to
//!   `FontSystem::new_with_locale_and_db`; the system's fonts, `fonts.conf`
//!   and fontconfig are never touched (TM-7b, T-F1);
//! * there is no icon lookup and no image decoder; every glyph the screen
//!   needs (bullets, digits, the Spanish letters) is in Inter;
//! * there is no locale detection: the locale is a literal.
#![forbid(unsafe_code)]

use cosmic_text::{
    Align, Attrs, Buffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

use celestina_tokens::Rgba;

/// The one face of the lock: Inter (OFL, `celestina-style/fonts/LICENSE.txt`),
/// compiled in. The path is outside the package; `celestina-style/fonts` is a
/// registered production input of the project.
static INTER: &[u8] = include_bytes!("../../../../celestina-style/fonts/InterVariable.ttf");

/// The locale handed to the shaper. A literal, never the environment's.
const LOCALE: &str = "en-US";

/// The quarter-circle constant of a cubic Bezier.
const KAPPA: f32 = 0.552_284_8;

/// Why the painter cannot start. The lock then keeps its covers and the field
/// still works blind (F16).
#[derive(Debug, thiserror::Error)]
pub enum PaintError {
    /// The embedded font data holds no usable face.
    #[error("the embedded font holds no face")]
    NoFace,
}

/// A token colour as the rasteriser's.
#[must_use]
pub fn color(token: Rgba) -> Color {
    // `a` is in 0..=1; the cast saturates and the value is rounded first.
    let alpha = (token.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_rgba8(token.r, token.g, token.b, alpha)
}

/// How a line of text is set.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    /// Font size in pixels.
    pub size: f32,
    pub ink: Rgba,
    pub align: Align,
}

/// Where a block of text went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextExtent {
    pub width: f32,
    pub height: f32,
    pub lines: usize,
}

/// The shaper, its glyph cache and the family name of the embedded face.
pub struct Fonts {
    system: FontSystem,
    cache: SwashCache,
    family: String,
}

impl Fonts {
    /// Loads the embedded face into an empty database.
    ///
    /// # Errors
    ///
    /// [`PaintError::NoFace`] when the embedded data holds no face, which a
    /// test of this crate rules out.
    pub fn embedded() -> Result<Self, PaintError> {
        let mut db = fontdb::Database::new();
        db.load_font_data(INTER.to_vec());
        let family = db
            .faces()
            .find_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .ok_or(PaintError::NoFace)?;
        Ok(Self {
            system: FontSystem::new_with_locale_and_db(LOCALE.to_owned(), db),
            cache: SwashCache::new(),
            family,
        })
    }

    /// The font database the shaper uses (T-F1 counts its faces).
    #[must_use]
    pub fn database(&self) -> &fontdb::Database {
        self.system.db()
    }

    /// The family name of the embedded face.
    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    fn buffer(&mut self, text: &str, size: f32, width: Option<f32>, align: Align) -> Buffer {
        let metrics = Metrics::new(size, (size * 1.3).round());
        let mut buffer = Buffer::new(&mut self.system, metrics);
        buffer.set_size(width, None);
        let attrs = Attrs::new().family(Family::Name(&self.family));
        buffer.set_text(text, &attrs, Shaping::Advanced, Some(align));
        buffer.shape_until_scroll(&mut self.system, false);
        buffer
    }

    /// The size `text` takes when wrapped at `width` (one line when `None`).
    pub fn measure(&mut self, text: &str, size: f32, width: Option<f32>) -> TextExtent {
        let buffer = self.buffer(text, size, width, Align::Left);
        let mut extent = TextExtent {
            width: 0.0,
            height: 0.0,
            lines: 0,
        };
        for run in buffer.layout_runs() {
            extent.width = extent.width.max(run.line_w);
            extent.height = run.line_top + run.line_height;
            extent.lines += 1;
        }
        extent
    }

    /// Draws `text` into `pixmap` with its top-left at `(x, y)`, wrapped and
    /// aligned inside `width`. Source-over; the pixmap is premultiplied.
    pub fn draw(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        style: TextStyle,
        at: (f32, f32),
        width: f32,
    ) -> TextExtent {
        let mut buffer = self.buffer(text, style.size, Some(width), style.align);
        let mut extent = TextExtent {
            width: 0.0,
            height: 0.0,
            lines: 0,
        };
        for run in buffer.layout_runs() {
            extent.width = extent.width.max(run.line_w);
            extent.height = run.line_top + run.line_height;
            extent.lines += 1;
        }
        let ink = TextColor::rgba(style.ink.r, style.ink.g, style.ink.b, 255);
        let alpha_scale = style.ink.a.clamp(0.0, 1.0);
        let (ox, oy) = (at.0.round() as i32, at.1.round() as i32);
        let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
        let data = pixmap.data_mut();
        buffer.draw(
            &mut self.system,
            &mut self.cache,
            ink,
            |x, y, rect_w, rect_h, covered| {
                let alpha = (f32::from(covered.a()) * alpha_scale).round() as u32;
                if alpha == 0 {
                    return;
                }
                let (x0, y0) = (ox + x, oy + y);
                let x1 = x0.saturating_add(i32::try_from(rect_w).unwrap_or(i32::MAX));
                let y1 = y0.saturating_add(i32::try_from(rect_h).unwrap_or(i32::MAX));
                for py in y0.max(0)..y1.min(h) {
                    for px in x0.max(0)..x1.min(w) {
                        let at = ((py * w + px) * 4) as usize;
                        if let Some(dst) = data.get_mut(at..at + 4) {
                            blend(dst, [covered.r(), covered.g(), covered.b()], alpha);
                        }
                    }
                }
            },
        );
        extent
    }
}

/// Source-over of one straight-colour pixel at `alpha` (0..=255) onto a
/// premultiplied RGBA8 pixel.
fn blend(dst: &mut [u8], rgb: [u8; 3], alpha: u32) {
    let keep = 255 - alpha;
    for (d, s) in dst.iter_mut().zip(rgb) {
        // premultiplied: src*a + dst*(1-a), rounded.
        *d = ((u32::from(s) * alpha + u32::from(*d) * keep + 127) / 255) as u8;
    }
    if let Some(a) = dst.get_mut(3) {
        *a = (alpha + (u32::from(*a) * keep + 127) / 255).min(255) as u8;
    }
}

/// A rounded rectangle; `radius` is clamped to half the shorter side, so a
/// large radius makes a capsule.
#[must_use]
pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Option<Path> {
    let r = radius.clamp(0.0, w.min(h) / 2.0);
    let k = r * KAPPA;
    let (x1, y1) = (x + w, y + h);
    let mut b = PathBuilder::new();
    b.move_to(x + r, y);
    b.line_to(x1 - r, y);
    b.cubic_to(x1 - r + k, y, x1, y + r - k, x1, y + r);
    b.line_to(x1, y1 - r);
    b.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    b.line_to(x + r, y1);
    b.cubic_to(x + r - k, y1, x, y1 - r + k, x, y1 - r);
    b.line_to(x, y + r);
    b.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    b.close();
    b.finish()
}

/// Fills `path` with a token colour.
pub fn fill(pixmap: &mut Pixmap, path: &Path, token: Rgba) {
    let mut paint = Paint::default();
    paint.set_color(color(token));
    paint.anti_alias = true;
    pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

/// Strokes `path` with a token colour.
pub fn stroke(pixmap: &mut Pixmap, path: &Path, token: Rgba, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color(token));
    paint.anti_alias = true;
    let stroke = Stroke {
        width,
        ..Stroke::default()
    };
    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

/// A filled disc.
pub fn disc(pixmap: &mut Pixmap, cx: f32, cy: f32, radius: f32, token: Rgba) {
    if let Some(path) = PathBuilder::from_circle(cx, cy, radius) {
        fill(pixmap, &path, token);
    }
}

/// Copies `src` (premultiplied RGBA8, opaque) onto `dst` (`XRGB8888`, the
/// bytes of a pixel being blue, green, red, unused) with its top-left at
/// `(x, y)` of `dst`. Rows or pixels outside `dst` are left out.
pub fn copy_to_xrgb(src: &Pixmap, dst: &mut [u8], stride: usize, size: (u32, u32), at: (u32, u32)) {
    let (width, height) = (src.width() as usize, src.height() as usize);
    let (dst_w, dst_h) = (size.0 as usize, size.1 as usize);
    let (x0, y0) = (at.0 as usize, at.1 as usize);
    let rows = height.min(dst_h.saturating_sub(y0));
    let cols = width.min(dst_w.saturating_sub(x0));
    let from = src.data();
    for row in 0..rows {
        let src_row = &from[row * width * 4..row * width * 4 + cols * 4];
        let start = (y0 + row) * stride + x0 * 4;
        let Some(dst_row) = dst.get_mut(start..start + cols * 4) else {
            return;
        };
        for (d, s) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 0xff;
        }
    }
}

/// Sets a rectangle of an `XRGB8888` buffer to black.
pub fn clear_xrgb(dst: &mut [u8], stride: usize, size: (u32, u32), rect: (u32, u32, u32, u32)) {
    let (x, y, w, h) = rect;
    let cols = (w.min(size.0.saturating_sub(x))) as usize;
    let rows = (h.min(size.1.saturating_sub(y))) as usize;
    for row in 0..rows {
        let start = (y as usize + row) * stride + x as usize * 4;
        if let Some(line) = dst.get_mut(start..start + cols * 4) {
            line.fill(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_tokens::DARK;

    #[test]
    fn t_f1_the_font_database_holds_exactly_the_embedded_faces_and_no_system_font_is_loaded() {
        let fonts = Fonts::embedded().expect("the embedded font loads");
        let db = fonts.database();
        // One variable face, nothing from the system: a machine with hundreds
        // of fonts installed still has exactly this one.
        assert_eq!(db.len(), 1, "the database holds the embedded face only");
        let face = db.faces().next().expect("one face");
        assert_eq!(fonts.family(), face.families[0].0);
        assert_eq!(fonts.family(), "Inter Variable");
        assert!(
            matches!(face.source, fontdb::Source::Binary(_)),
            "the face comes from the binary, not from a file"
        );
    }

    #[test]
    fn text_is_drawn_from_the_embedded_face_with_the_spanish_letters() {
        let mut fonts = Fonts::embedded().expect("font");
        let mut pixmap = Pixmap::new(320, 40).expect("pixmap");
        let style = TextStyle {
            size: 20.0,
            ink: DARK.text,
            align: Align::Left,
        };
        let text = format!(
            "{} · {} ••••",
            crate::copy::LOCK_FIELD,
            crate::copy::LOCK_CAPS
        );
        let text = text.as_str();
        let extent = fonts.draw(&mut pixmap, text, style, (4.0, 4.0), 312.0);
        assert!(extent.width > 40.0 && extent.lines >= 1, "{extent:?}");
        assert!(
            pixmap.data().chunks_exact(4).any(|px| px[3] > 0),
            "something was drawn"
        );
        // The same text shaped from an empty database would come out with no
        // glyphs at all; here every character resolved to a glyph of Inter.
        let again = fonts.measure(text, 20.0, None);
        assert!(again.width > 0.0);
    }

    #[test]
    fn text_wraps_inside_its_width() {
        let mut fonts = Fonts::embedded().expect("font");
        let long = crate::copy::LOCK_UNSUPPORTED;
        let wide = fonts.measure(long, 17.0, None);
        let narrow = fonts.measure(long, 17.0, Some(300.0));
        assert_eq!(wide.lines, 1);
        assert!(
            narrow.lines >= 2 && narrow.width <= 300.0 + 1.0,
            "{narrow:?}"
        );
    }

    #[test]
    fn a_rounded_rect_with_a_large_radius_is_a_capsule() {
        let mut pixmap = Pixmap::new(100, 40).expect("pixmap");
        let path = rounded_rect(0.0, 0.0, 100.0, 40.0, 9999.0).expect("path");
        fill(&mut pixmap, &path, DARK.elevated);
        let alpha = |x: usize, y: usize| pixmap.data()[(y * 100 + x) * 4 + 3];
        assert_eq!(alpha(50, 20), 255, "the middle is filled");
        assert_eq!(alpha(0, 0), 0, "the corner is cut");
        assert_eq!(alpha(99, 39), 0, "the opposite corner too");
        assert!(alpha(1, 20) > 0, "the left end is round, not cut");
    }

    #[test]
    fn xrgb_copy_swaps_to_blue_green_red_and_stays_inside_the_buffer() {
        let mut src = Pixmap::new(2, 2).expect("pixmap");
        src.fill(Color::from_rgba8(10, 20, 30, 255));
        let mut dst = vec![7u8; 4 * 4 * 3];
        copy_to_xrgb(&src, &mut dst, 16, (4, 3), (3, 2));
        // Only column 3 of row 2 fits (columns 3 and 4; row 2 and 3, of 4x3).
        let at = 2 * 16 + 3 * 4;
        assert_eq!(&dst[at..at + 4], &[30, 20, 10, 255]);
        assert_eq!(dst.iter().filter(|b| **b == 7).count(), dst.len() - 4);
    }

    #[test]
    fn clearing_a_rectangle_blacks_it_and_clips_to_the_buffer() {
        let mut dst = vec![9u8; 8 * 4 * 4];
        clear_xrgb(&mut dst, 32, (8, 4), (6, 3, 10, 10));
        let zeros = dst.iter().filter(|b| **b == 0).count();
        assert_eq!(zeros, 2 * 4, "two pixels of the last row");
    }

    #[test]
    fn blending_is_source_over_on_a_premultiplied_pixel() {
        let mut px = [0, 0, 0, 255];
        blend(&mut px, [255, 255, 255], 128);
        assert_eq!(px, [128, 128, 128, 255]);
        let mut clear = [0, 0, 0, 0];
        blend(&mut clear, [200, 100, 0], 255);
        assert_eq!(clear, [200, 100, 0, 255]);
    }
}
