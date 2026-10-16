//! The `Painter`: the one drawing API the product and the UI tree use, over
//! the private backend. One `Painter` per process; the runtime makes the GL
//! context current for a surface before `begin`. From the spike's
//! `src/paint/mod.rs`, plus the theme, the glyph atlas, icons and pixmaps.
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::PathBuf;

use celestina_wayland::egl::EglError;
use celestina_wayland::Rect;

use crate::atlas::{Atlas, GlyphKey, PAGE};
use crate::backend::{Backend, BackendInit, GlyphQuad, GpuError, ImageHandle, ImageKind};
use crate::color::{unit, Color};
use crate::haze::{grain_texels, GrainError, HazePlan, GRAIN_SIZE};
use crate::icon::{
    default_icon_dirs, device_size, rasterise_file, semantic_path, theme_path, IconCache, IconKey,
    PixmapId,
};
use crate::text::{TextRun, TextSystem};
use crate::theme::Theme;

/// The longest side of an uploaded pixmap, in px.
pub(crate) const MAX_PIXMAP_SIDE: u32 = 2048;

/// Transparent texels around each glyph in the atlas, so linear sampling
/// under a transform never bleeds a neighbour in.
const GLYPH_PAD: u32 = 1;

/// Rasterised blank glyphs remembered at most, so spaces are not
/// re-rasterised every frame.
const BLANK_LIMIT: usize = 1024;

const WHITE: Color = Color(0xff, 0xff, 0xff, 1.0);

/// Why the painter could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum PaintError {
    #[error("cannot create the GL renderer")]
    Renderer(#[source] EglError),
    #[error("cannot {what}")]
    Gpu {
        what: &'static str,
        #[source]
        source: GpuError,
    },
    #[error(transparent)]
    Grain(#[from] GrainError),
    #[error("a {width}×{height} ARGB pixmap cannot be {len} bytes (sides 1..={MAX_PIXMAP_SIDE})")]
    Pixmap { width: u32, height: u32, len: usize },
}

fn gpu(what: &'static str) -> impl FnOnce(GpuError) -> PaintError {
    move |source| PaintError::Gpu { what, source }
}

/// Converts ARGB32 texels in network byte order (StatusNotifierItem and
/// notification pixmaps) to straight RGBA.
pub(crate) fn argb_to_rgba(argb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, PaintError> {
    let refuse = || PaintError::Pixmap {
        width,
        height,
        len: argb.len(),
    };
    let sides = 1..=MAX_PIXMAP_SIDE;
    if !sides.contains(&width) || !sides.contains(&height) {
        return Err(refuse());
    }
    if argb.len() != width as usize * height as usize * 4 {
        return Err(refuse());
    }
    Ok(argb
        .chunks_exact(4)
        .flat_map(|p| [p[1], p[2], p[3], p[0]])
        .collect())
}

/// Tracks the global alpha so `opacity` can multiply and nest.
#[derive(Debug)]
struct AlphaStack {
    current: f32,
    saved: Vec<f32>,
}

impl AlphaStack {
    fn new() -> Self {
        Self {
            current: 1.0,
            saved: Vec::new(),
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn push(&mut self) {
        self.saved.push(self.current);
    }

    fn pop(&mut self) {
        if let Some(a) = self.saved.pop() {
            self.current = a;
        }
    }

    fn multiply(&mut self, alpha: f32) -> f32 {
        self.current = unit(self.current * unit(alpha));
        self.current
    }
}

/// Where a glyph sits in the atlas page relative to its pen position.
#[derive(Clone, Copy, Debug)]
struct GlyphMeta {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    color: bool,
}

/// The glyph atlas's occupancy: the glyphs it holds and the texels they
/// take out of its one page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasUsage {
    pub glyphs: usize,
    pub texels: u64,
    pub page_texels: u64,
}

impl AtlasUsage {
    /// The occupied share of the page, 0..=1.
    pub fn fraction(&self) -> f64 {
        if self.page_texels == 0 {
            0.0
        } else {
            self.texels as f64 / self.page_texels as f64
        }
    }
}

/// The one painter of the process.
pub struct Painter {
    backend: Backend,
    theme: Theme,
    text: TextSystem,
    atlas: Atlas<GlyphKey, GlyphMeta>,
    atlas_page: ImageHandle,
    blanks: HashSet<GlyphKey>,
    atlas_full_warned: bool,
    icons: IconCache<ImageHandle>,
    icon_dirs: Vec<PathBuf>,
    icon_theme: Option<String>,
    pixmaps: HashMap<PixmapId, ImageHandle>,
    next_pixmap: u32,
    grain: ImageHandle,
    alpha: AlphaStack,
    dpr: f32,
    blur_available: bool,
}

impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painter")
            .field("dpr", &self.dpr)
            .field("blur_available", &self.blur_available)
            .field("glyphs", &self.atlas.len())
            .field("pixmaps", &self.pixmaps.len())
            .finish_non_exhaustive()
    }
}

impl Painter {
    /// Builds the painter on the current GL context; `proc_address` is
    /// `FrameCx::proc_address`.
    pub fn new(
        proc_address: &dyn Fn(&str) -> *const c_void,
        theme: Theme,
        fonts: TextSystem,
    ) -> Result<Painter, PaintError> {
        let mut backend = Backend::new(proc_address).map_err(|e| match e {
            BackendInit::Renderer(e) => PaintError::Renderer(e),
            BackendInit::Canvas(source) => PaintError::Gpu {
                what: "create the canvas",
                source,
            },
        })?;
        let grain = grain_texels()?;
        let grain = backend
            .create_image(grain.width, grain.height, &grain.rgba, ImageKind::Tile)
            .map_err(gpu("upload the Haze grain"))?;
        let atlas_page = backend
            .create_atlas(PAGE)
            .map_err(gpu("create the glyph atlas"))?;
        Ok(Painter {
            backend,
            theme,
            text: fonts,
            atlas: Atlas::new(),
            atlas_page,
            blanks: HashSet::new(),
            atlas_full_warned: false,
            icons: IconCache::new(),
            icon_dirs: default_icon_dirs(),
            icon_theme: freedesktop_icons::default_theme_gtk(),
            pixmaps: HashMap::new(),
            next_pixmap: 0,
            grain,
            alpha: AlphaStack::new(),
            dpr: 1.0,
            blur_available: false,
        })
    }

    /// Starts a frame of `logical` px at `dpr`, cleared to transparent;
    /// `blur_available` says whether the compositor blurs behind the surface.
    pub fn begin(&mut self, logical: (f32, f32), dpr: f32, blur_available: bool) {
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        let buffer = |side: f32| {
            let px = (side * dpr).round();
            if px.is_finite() && px >= 1.0 {
                px as u32
            } else {
                1
            }
        };
        self.dpr = dpr;
        self.blur_available = blur_available;
        self.alpha.reset();
        self.atlas.begin_frame();
        self.backend
            .begin(buffer(logical.0), buffer(logical.1), dpr);
    }

    /// Flushes the recorded commands to GL.
    pub fn end(&mut self) {
        let leaked = self.alpha.saved.len();
        if leaked > 0 {
            tracing::warn!(depth = leaked, "unbalanced push_state/pop_state");
            // `begin`'s reset clears only the current state, not femtovg's
            // save stack: unwind the leaked levels so they cannot pile up
            // across frames.
            for _ in 0..leaked {
                self.backend.restore();
            }
            self.alpha.reset();
        }
        self.backend.flush();
    }

    /// How full the one glyph atlas page is (design §9: the atlas is
    /// bounded and measured).
    pub fn atlas_usage(&self) -> AtlasUsage {
        AtlasUsage {
            glyphs: self.atlas.len(),
            texels: self.atlas.occupied_texels(),
            page_texels: u64::from(PAGE) * u64::from(PAGE),
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn text(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    pub fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Color) {
        self.backend.fill_rounded(rect, radius, color);
    }

    /// A rounded outline of `width` centred on the rect edge.
    pub fn stroke_rounded(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        self.backend.stroke_rounded(rect, radius, width, color);
    }

    /// The Haze material over the rounded rect: with the compositor blur,
    /// the grain then the glass tint over it; without it, the opaque
    /// fallback. Nothing else: no outline, no edge, no shadow.
    pub fn haze(&mut self, rect: Rect, radius: f32) {
        let plan = HazePlan::for_(&self.theme, self.blur_available);
        if let Some(alpha) = plan.grain {
            // The tile is anchored at the surface origin, so the grain
            // stays still while the shape moves; one texel per logical px.
            let tile = GRAIN_SIZE as f32;
            let placement = Rect {
                x: 0.0,
                y: 0.0,
                w: tile,
                h: tile,
            };
            self.backend
                .fill_image(rect, radius, self.grain, placement, WHITE.with_alpha(alpha));
        }
        self.backend.fill_rounded(rect, radius, plan.fill);
    }

    /// Draws `run` with its top-left corner at (`x`, `y`) in `color`.
    pub fn glyphs(&mut self, run: &TextRun, x: f32, y: f32, color: Color) {
        let dpr = run.dpr;
        let (ox, oy) = ((x * dpr).round(), (y * dpr).round());
        let mut coverage = Vec::new();
        let mut colored = Vec::new();
        for glyph in run.glyphs() {
            let key = GlyphKey::from_cache_key(&glyph.cache_key);
            let Some((slot, meta)) = self.glyph_slot(key, glyph.cache_key) else {
                continue;
            };
            let x0 = (ox + (glyph.x + meta.left) as f32) / dpr;
            let y0 = (oy + (glyph.y - meta.top) as f32) / dpr;
            let page = PAGE as f32;
            let (sx, sy) = (slot.x + GLYPH_PAD, slot.y + GLYPH_PAD);
            let quad = GlyphQuad {
                x0,
                y0,
                x1: x0 + meta.width as f32 / dpr,
                y1: y0 + meta.height as f32 / dpr,
                s0: sx as f32 / page,
                t0: sy as f32 / page,
                s1: (sx + meta.width) as f32 / page,
                t1: (sy + meta.height) as f32 / page,
            };
            if meta.color {
                colored.push(quad);
            } else {
                coverage.push(quad);
            }
        }
        if !coverage.is_empty() || !colored.is_empty() {
            self.backend
                .draw_glyphs(self.atlas_page, coverage, colored, color);
        }
    }

    /// The atlas slot of a glyph, rasterising and uploading it on a miss.
    fn glyph_slot(
        &mut self,
        key: GlyphKey,
        cache_key: cosmic_text::CacheKey,
    ) -> Option<(crate::atlas::Slot, GlyphMeta)> {
        if let Some((slot, meta)) = self.atlas.get(&key) {
            return Some((slot, *meta));
        }
        if self.blanks.contains(&key) {
            return None;
        }
        let Some(bitmap) = self.text.rasterise(cache_key) else {
            if self.blanks.len() >= BLANK_LIMIT {
                self.blanks.clear();
            }
            self.blanks.insert(key);
            return None;
        };
        let meta = GlyphMeta {
            left: bitmap.left,
            top: bitmap.top,
            width: bitmap.width,
            height: bitmap.height,
            color: bitmap.color,
        };
        let Some(slot) = self.atlas.insert(
            key,
            bitmap.width + 2 * GLYPH_PAD,
            bitmap.height + 2 * GLYPH_PAD,
            meta,
        ) else {
            if !self.atlas_full_warned {
                self.atlas_full_warned = true;
                tracing::warn!(
                    glyphs = self.atlas.len(),
                    "the glyph atlas is full this frame; glyphs are skipped"
                );
            }
            return None;
        };
        // The padding is cleared with the glyph so an evicted neighbour's
        // texels never show at the edge.
        let (pw, ph) = (slot.w, slot.h);
        let mut padded = vec![0u8; pw as usize * ph as usize * 4];
        let row = bitmap.width as usize * 4;
        for (y, src) in bitmap.rgba.chunks_exact(row).enumerate() {
            let at = ((y + GLYPH_PAD as usize) * pw as usize + GLYPH_PAD as usize) * 4;
            if let Some(dst) = padded.get_mut(at..at + row) {
                dst.copy_from_slice(src);
            }
        }
        if let Err(err) =
            self.backend
                .update_image(self.atlas_page, (slot.x, slot.y), (pw, ph), &padded)
        {
            // Roll the insert back: a live entry would draw stale texels.
            self.atlas.remove(&key);
            tracing::warn!(error = %err, "cannot upload a glyph to the atlas");
            return None;
        }
        Some((slot, meta))
    }

    /// Draws an icon fitted and centred in `rect`. Semantic (Lucide) icons
    /// are white and take `tint`; theme icons and pixmaps keep their own
    /// colours unless tinted.
    pub fn icon(&mut self, key: &IconKey, rect: Rect, tint: Option<Color>) {
        let tint = tint.unwrap_or(WHITE);
        if let IconKey::Pixmap(id) = key {
            self.draw_pixmap(*id, rect, tint);
            return;
        }
        let size = device_size(rect, self.dpr);
        let cached = match self.icons.get(key, size) {
            Some(cached) => cached.texture,
            None => {
                let texture = self.load_icon(key, size);
                if let Some(evicted) = self.icons.insert(key.clone(), size, texture) {
                    self.backend.delete_image(evicted);
                }
                texture
            }
        };
        let Some((image, w, h)) = cached else {
            return;
        };
        let (lw, lh) = (w as f32 / self.dpr, h as f32 / self.dpr);
        let placed = Rect {
            x: rect.x + (rect.w - lw) / 2.0,
            y: rect.y + (rect.h - lh) / 2.0,
            w: lw,
            h: lh,
        };
        self.backend.fill_image(placed, 0.0, image, placed, tint);
    }

    /// Resolves, rasterises and uploads a file icon; `None` is cached so a
    /// missing icon is warned about once.
    fn load_icon(&mut self, key: &IconKey, size: u32) -> Option<(ImageHandle, u32, u32)> {
        let path = match key {
            IconKey::Semantic(name) => semantic_path(&self.icon_dirs, name),
            IconKey::Theme(name) => theme_path(self.icon_theme.as_deref(), name, size),
            IconKey::Pixmap(_) => None,
        };
        let Some(path) = path else {
            tracing::warn!(icon = ?key, "no such icon");
            return None;
        };
        let image = match rasterise_file(&path, size) {
            Ok(image) => image,
            Err(err) => {
                tracing::warn!(icon = ?key, error = %err, "cannot rasterise the icon");
                return None;
            }
        };
        match self.backend.create_image(
            image.width,
            image.height,
            &image.rgba,
            ImageKind::Premultiplied,
        ) {
            Ok(handle) => Some((handle, image.width, image.height)),
            Err(err) => {
                tracing::warn!(icon = ?key, error = %err, "cannot upload the icon");
                None
            }
        }
    }

    /// Draws an uploaded pixmap stretched over `rect` at `alpha`.
    pub fn image(&mut self, id: PixmapId, rect: Rect, alpha: f32) {
        self.draw_pixmap(id, rect, WHITE.with_alpha(alpha));
    }

    fn draw_pixmap(&mut self, id: PixmapId, rect: Rect, tint: Color) {
        match self.pixmaps.get(&id) {
            Some(&image) => self.backend.fill_image(rect, 0.0, image, rect, tint),
            None => tracing::debug!(?id, "drawing a released pixmap"),
        }
    }

    /// Uploads an ARGB32 pixmap (network byte order, as StatusNotifierItem
    /// and notification images carry it).
    pub fn upload_pixmap(
        &mut self,
        argb: &[u8],
        width: u32,
        height: u32,
    ) -> Result<PixmapId, PaintError> {
        let rgba = argb_to_rgba(argb, width, height)?;
        let image = self
            .backend
            .create_image(width, height, &rgba, ImageKind::Straight)
            .map_err(gpu("upload a pixmap"))?;
        self.next_pixmap = self.next_pixmap.wrapping_add(1);
        let id = PixmapId(self.next_pixmap);
        if let Some(stale) = self.pixmaps.insert(id, image) {
            self.backend.delete_image(stale);
        }
        Ok(id)
    }

    /// Frees an uploaded pixmap's texture; drawing it afterwards draws
    /// nothing.
    pub fn release_pixmap(&mut self, id: PixmapId) {
        if let Some(image) = self.pixmaps.remove(&id) {
            self.backend.delete_image(image);
        }
    }

    /// `push_state` plus a rounded-rect clip.
    pub fn push_clip(&mut self, rect: Rect, radius: f32) {
        self.push_state();
        self.backend.clip_rounded(rect, radius);
    }

    pub fn pop_clip(&mut self) {
        self.pop_state();
    }

    /// Saves clip, transform and opacity until the matching `pop_state`.
    pub fn push_state(&mut self) {
        self.backend.save();
        self.alpha.push();
    }

    pub fn pop_state(&mut self) {
        self.backend.restore();
        self.alpha.pop();
    }

    /// Multiplies the alpha of subsequent draws until the enclosing
    /// `pop_state` / `pop_clip` (or the next `begin`).
    pub fn opacity(&mut self, multiply: f32) {
        let alpha = self.alpha.multiply(multiply);
        self.backend.set_global_alpha(alpha);
    }

    /// Scales subsequent draws by `s` about (`cx`, `cy`), for the press
    /// recoil; scoped by the enclosing `push_state` / `pop_state`.
    pub fn scale_about(&mut self, cx: f32, cy: f32, s: f32) {
        self.backend.scale_about(cx, cy, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_multiplies_and_is_scoped() {
        let mut a = AlphaStack::new();
        assert_eq!(a.multiply(0.5), 0.5);
        assert_eq!(a.multiply(0.5), 0.25);
        a.push();
        assert_eq!(a.multiply(0.5), 0.125);
        a.pop();
        assert_eq!(a.current, 0.25);
        a.push();
        a.reset();
        assert_eq!(a.current, 1.0);
        assert!(a.saved.is_empty());
    }

    #[test]
    fn opacity_is_clamped_to_the_unit_range() {
        let mut a = AlphaStack::new();
        assert_eq!(a.multiply(2.0), 1.0);
        assert_eq!(a.multiply(f32::NAN), 0.0);
    }

    #[test]
    fn argb_pixmaps_become_straight_rgba() {
        // Two pixels in network byte order: opaque red, half-transparent blue.
        let argb = [255, 255, 0, 0, 128, 0, 0, 255];
        assert_eq!(
            argb_to_rgba(&argb, 2, 1).ok(),
            Some(vec![255, 0, 0, 255, 0, 0, 255, 128])
        );
    }

    #[test]
    fn argb_pixmaps_of_the_wrong_length_or_size_are_refused() {
        assert!(matches!(
            argb_to_rgba(&[0; 7], 2, 1),
            Err(PaintError::Pixmap { .. })
        ));
        assert!(matches!(
            argb_to_rgba(&[], 0, 0),
            Err(PaintError::Pixmap { .. })
        ));
        let huge = MAX_PIXMAP_SIDE + 1;
        assert!(matches!(
            argb_to_rgba(&[], huge, 1),
            Err(PaintError::Pixmap { .. })
        ));
    }
}
