//! Text: a cosmic-text `FontSystem` with Inter Variable plus the system's
//! fonts for fallback, laid out once per change into a `TextRun` whose glyphs
//! the painter draws from the atlas. Layout runs at whole device-pixel font
//! sizes, so a run is sharp at a fractional scale and its glyph keys are
//! exact; measurement uses the same `Buffer`, so what the layout measures is
//! what is painted.
#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;

use cosmic_text::{
    fontdb, Attrs, Buffer, CacheKey, Family, FeatureTag, FontFeatures, FontSystem, Metrics,
    Shaping, SwashCache, SwashContent, Wrap,
};

use crate::atlas::size_px;
#[cfg(test)]
use crate::atlas::GlyphKey;
use crate::theme::Role;

/// The face the style ships (`celestina-style/fonts`).
const INTER_FILE: &str = "InterVariable.ttf";

/// Line height as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.25;

/// The two weights of the style: 400 for body, 600 for titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Semibold,
}

impl Weight {
    fn wght(self) -> cosmic_text::Weight {
        match self {
            Weight::Regular => cosmic_text::Weight::NORMAL,
            Weight::Semibold => cosmic_text::Weight::SEMIBOLD,
        }
    }
}

/// How a run of text is set: size in logical px, weight, tabular figures
/// (`tnum`) and the role it is painted in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub weight: Weight,
    pub tabular: bool,
    pub role: Role,
}

/// The fonts could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("{INTER_FILE} is in none of {searched:?}")]
    InterMissing { searched: Vec<PathBuf> },
    #[error("cannot load {path:?}")]
    Load {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path:?} holds no font face")]
    NoFace { path: PathBuf },
}

/// One glyph of a run: its rasterisation key and its pen position in device
/// pixels from the run's top-left corner (y is the baseline).
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunGlyph {
    pub cache_key: CacheKey,
    pub x: i32,
    pub y: i32,
}

/// A laid-out run of text, painted with [`crate::Painter::glyphs`].
#[derive(Debug)]
pub struct TextRun {
    text: String,
    /// The shaped and laid-out buffer the size was measured from.
    buffer: Buffer,
    size: (f32, f32),
    pub(crate) dpr: f32,
}

impl TextRun {
    /// Width and height in logical px.
    pub fn size(&self) -> (f32, f32) {
        self.size
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The run's glyphs, positioned from the layout (no reshaping).
    pub(crate) fn glyphs(&self) -> impl Iterator<Item = RunGlyph> + '_ {
        self.buffer.layout_runs().flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                let physical = glyph.physical((0.0, run.line_y), 1.0);
                RunGlyph {
                    cache_key: physical.cache_key,
                    x: physical.x,
                    y: physical.y,
                }
            })
        })
    }
}

/// A glyph rasterised for the atlas: RGBA texels (coverage in every channel
/// for an outline glyph, colour for an emoji) and its placement from the pen
/// position, in device pixels.
#[derive(Clone, Debug)]
pub(crate) struct GlyphBitmap {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub color: bool,
    pub rgba: Vec<u8>,
}

/// Fonts, shaping and rasterisation.
pub struct TextSystem {
    fonts: FontSystem,
    swash: SwashCache,
    /// Inter Variable's family name as the face declares it.
    pub(crate) family: String,
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem")
            .field("family", &self.family)
            .finish_non_exhaustive()
    }
}

/// The cosmic-text attributes of a style over `family`.
pub(crate) fn attrs_for<'a>(style: &TextStyle, family: &'a str) -> Attrs<'a> {
    let mut features = FontFeatures::new();
    if style.tabular {
        features.enable(FeatureTag::new(b"tnum"));
    }
    Attrs::new()
        .family(Family::Name(family))
        .weight(style.weight.wght())
        .font_features(features)
}

/// Where Inter Variable is looked for: `CELESTINA_FONTS`, the checkout's
/// `celestina-style/fonts`, then `$prefix/share/celestina/fonts` beside the
/// installed binary.
pub fn default_font_dirs() -> Vec<PathBuf> {
    font_dirs_from(
        std::env::var_os("CELESTINA_FONTS"),
        std::env::current_exe().ok(),
    )
}

pub(crate) fn font_dirs_from(over: Option<OsString>, exe: Option<PathBuf>) -> Vec<PathBuf> {
    crate::style_asset_dirs(over, exe, "fonts")
}

impl TextSystem {
    /// Loads Inter Variable from the first of `font_dirs` that holds it, and
    /// the system's fonts for fallback.
    pub fn new(font_dirs: &[PathBuf]) -> Result<TextSystem, TextError> {
        let path = font_dirs
            .iter()
            .map(|dir| dir.join(INTER_FILE))
            .find(|path| path.is_file())
            .ok_or_else(|| TextError::InterMissing {
                searched: font_dirs.to_vec(),
            })?;
        let mut fonts = FontSystem::new();
        let system: HashSet<fontdb::ID> = fonts.db().faces().map(|face| face.id).collect();
        fonts
            .db_mut()
            .load_font_file(&path)
            .map_err(|source| TextError::Load {
                path: path.clone(),
                source,
            })?;
        let family = fonts
            .db()
            .faces()
            .filter(|face| !system.contains(&face.id))
            .find_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .ok_or(TextError::NoFace { path })?;
        tracing::debug!(%family, faces = system.len(), "fonts loaded");
        Ok(TextSystem {
            fonts,
            swash: SwashCache::new(),
            family,
        })
    }

    /// Shapes and lays out `text` at `dpr`, wrapping at `max_width` logical
    /// px when given.
    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        dpr: f32,
    ) -> TextRun {
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        let font_px = size_px(style.size, dpr) as f32;
        let line_px = (font_px * LINE_HEIGHT).round().max(1.0);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(font_px, line_px));
        let width_px = max_width
            .filter(|w| w.is_finite())
            .map(|w| (w * dpr).max(1.0));
        buffer.set_wrap(if width_px.is_some() {
            Wrap::WordOrGlyph
        } else {
            Wrap::None
        });
        buffer.set_size(width_px, None);
        let attrs = attrs_for(style, &self.family);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);

        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        TextRun {
            text: text.to_owned(),
            buffer,
            size: (width.ceil() / dpr, height.max(line_px) / dpr),
            dpr,
        }
    }

    /// The logical size [`TextSystem::layout`] gives the same arguments.
    pub fn measure(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        dpr: f32,
    ) -> (f32, f32) {
        self.layout(text, style, max_width, dpr).size()
    }

    /// Rasterises one glyph; `None` for a glyph with no ink (a space) or one
    /// the font cannot render.
    pub(crate) fn rasterise(&mut self, key: CacheKey) -> Option<GlyphBitmap> {
        let image = self.swash.get_image_uncached(&mut self.fonts, key)?;
        let (width, height) = (image.placement.width, image.placement.height);
        let texels = (width as usize).checked_mul(height as usize)?;
        if texels == 0 {
            return None;
        }
        let (color, rgba) = match image.content {
            SwashContent::Mask if image.data.len() == texels => (
                false,
                image.data.iter().flat_map(|&a| [a, a, a, a]).collect(),
            ),
            SwashContent::Color if image.data.len() == texels.checked_mul(4)? => (true, image.data),
            _ => return None,
        };
        Some(GlyphBitmap {
            left: image.placement.left,
            top: image.placement.top,
            width,
            height,
            color,
            rgba,
        })
    }

    #[cfg(test)]
    fn family_of(&self, id: fontdb::ID) -> Option<String> {
        let face = self.fonts.db().face(id)?;
        face.families.first().map(|(name, _)| name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Family, FeatureTag};
    use std::sync::{Mutex, OnceLock};

    fn style(weight: Weight, tabular: bool) -> TextStyle {
        TextStyle {
            size: 14.0,
            weight,
            tabular,
            role: Role::TextHi,
        }
    }

    /// One `TextSystem` for the module: loading the system's fonts is slow.
    fn system() -> std::sync::MutexGuard<'static, TextSystem> {
        static SYSTEM: OnceLock<Mutex<TextSystem>> = OnceLock::new();
        let lock = SYSTEM.get_or_init(|| {
            let dirs = font_dirs_from(None, None);
            Mutex::new(TextSystem::new(&dirs).expect("Inter Variable in the checkout"))
        });
        lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn tabular_style_sets_tnum_and_semibold_sets_wght_600() {
        let attrs = attrs_for(&style(Weight::Semibold, true), "Inter Variable");
        assert_eq!(attrs.weight, cosmic_text::Weight(600));
        assert_eq!(attrs.family, Family::Name("Inter Variable"));
        let tnum = FeatureTag::new(b"tnum");
        assert!(attrs
            .font_features
            .features
            .iter()
            .any(|f| f.tag == tnum && f.value == 1));

        let attrs = attrs_for(&style(Weight::Regular, false), "Inter Variable");
        assert_eq!(attrs.weight, cosmic_text::Weight(400));
        assert!(attrs.font_features.features.iter().all(|f| f.tag != tnum));
    }

    #[test]
    fn font_dirs_put_the_override_first_then_the_checkout_then_the_prefix() {
        let dirs = font_dirs_from(
            Some("/opt/fonts".into()),
            Some("/usr/local/bin/celestina".into()),
        );
        assert_eq!(dirs[0], PathBuf::from("/opt/fonts"));
        assert!(dirs[1].ends_with("celestina-style/fonts"), "{:?}", dirs[1]);
        assert_eq!(
            dirs[2],
            PathBuf::from("/usr/local/bin/../share/celestina/fonts")
        );
        assert_eq!(font_dirs_from(None, None).len(), 1);
    }

    #[test]
    fn a_missing_inter_is_a_typed_error() {
        let err = TextSystem::new(&[PathBuf::from("/nonexistent/celestina-fonts")]);
        assert!(
            matches!(err, Err(TextError::InterMissing { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn layout_measures_what_it_paints() {
        let mut text = system();
        let s = style(Weight::Regular, false);
        let run = text.layout("Espacio de trabajo 3", &s, None, 1.5);
        assert_eq!(run.text(), "Espacio de trabajo 3");
        assert_eq!(
            text.measure("Espacio de trabajo 3", &s, None, 1.5),
            run.size()
        );
        let (w, h) = run.size();
        assert!(w > 50.0 && h >= 14.0, "{w}×{h}");
        assert!(run.glyphs().next().is_some());
        let (longer, _) = text.measure("Espacio de trabajo 3 y 4", &s, None, 1.5);
        assert!(longer > w);
        let (narrow, tall) = text.measure("Espacio de trabajo 3", &s, Some(60.0), 1.5);
        assert!(narrow <= 60.0 + 0.5 && tall > h, "{narrow}×{tall} wraps");
    }

    #[test]
    fn logical_size_does_not_depend_on_the_device_ratio() {
        let mut text = system();
        let s = style(Weight::Regular, false);
        let (w1, h1) = text.measure("Notificaciones", &s, None, 1.0);
        let (w2, h2) = text.measure("Notificaciones", &s, None, 2.0);
        assert!(
            (w1 - w2).abs() < 2.0 && (h1 - h2).abs() < 1.0,
            "{w1}×{h1} vs {w2}×{h2}"
        );
    }

    #[test]
    fn tabular_digits_share_one_advance() {
        let mut text = system();
        let tab = style(Weight::Regular, true);
        let prop = style(Weight::Regular, false);
        let (ones, _) = text.measure("1111", &tab, None, 1.0);
        let (eights, _) = text.measure("8888", &tab, None, 1.0);
        assert!((ones - eights).abs() < 0.01, "tnum: {ones} vs {eights}");
        let (ones, _) = text.measure("1111", &prop, None, 1.0);
        let (eights, _) = text.measure("8888", &prop, None, 1.0);
        assert!(eights - ones > 1.0, "proportional: {ones} vs {eights}");
    }

    #[test]
    fn glyphs_are_laid_out_in_inter_and_rasterise_to_a_mask() {
        let mut text = system();
        let run = text.layout("A", &style(Weight::Semibold, false), None, 1.5);
        let glyph = run.glyphs().next().expect("one glyph");
        let family = text.family_of(glyph.cache_key.font_id);
        assert_eq!(family.as_deref(), Some(text.family.as_str()));
        assert_eq!(GlyphKey::from_cache_key(&glyph.cache_key).size_px, 21);
        let bitmap = text.rasterise(glyph.cache_key).expect("an outline glyph");
        assert!(bitmap.width > 0 && bitmap.height > 0);
        assert!(!bitmap.color);
        assert_eq!(
            bitmap.rgba.len(),
            (bitmap.width * bitmap.height * 4) as usize
        );
        assert!(bitmap
            .rgba
            .chunks_exact(4)
            .any(|p| p == [255, 255, 255, 255]));
    }
}
