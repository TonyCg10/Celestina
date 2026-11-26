//! The private femtovg backend: the only module that names femtovg types
//! (besides `celestina_wayland::egl::femtovg_renderer`, which builds the
//! renderer). A wgpu backend later replaces this module and nothing else.
//!
//! Coordinates are logical px. `begin` sizes the canvas in buffer px with a
//! femtovg dpi of 1 and applies `scale(dpr)`, so one logical px is `dpr`
//! buffer px and edge fringes are one buffer px wide (from the spike's
//! `src/paint/mod.rs`).
#![forbid(unsafe_code)]

use std::ffi::c_void;

use femtovg::imgref::Img;
use femtovg::renderer::OpenGl;
use femtovg::rgb::FromSlice;
use femtovg::{
    Canvas, CompositeOperation, DrawCommand, GlyphDrawCommands, ImageFilter, ImageFlags, ImageId,
    ImageSource, Paint, Path, PixelFormat, Quad, RenderTarget,
};

use crate::color::Color;
use celestina_wayland::egl::{femtovg_renderer, EglError};
use celestina_wayland::Rect;

/// A backend failure, the source of a `PaintError`; opaque so femtovg's
/// types stay out of the public API.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct GpuError(#[from] Fault);

#[derive(Debug, thiserror::Error)]
pub(crate) enum Fault {
    #[error(transparent)]
    Femtovg(femtovg::ErrorKind),
    #[error("{len} bytes are not {width}×{height} RGBA texels")]
    Texels { width: u32, height: u32, len: usize },
}

impl From<femtovg::ErrorKind> for GpuError {
    fn from(e: femtovg::ErrorKind) -> GpuError {
        GpuError(Fault::Femtovg(e))
    }
}

/// `rgba` as `width`×`height` texels, refusing an empty or mismatched size
/// (imgref panics on a zero stride).
fn texels(width: u32, height: u32, rgba: &[u8]) -> Result<Img<&[femtovg::rgb::RGBA8]>, GpuError> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(rgba.len()) {
        return Err(GpuError(Fault::Texels {
            width,
            height,
            len: rgba.len(),
        }));
    }
    Ok(Img::new(rgba.as_rgba(), width as usize, height as usize))
}

/// A texture of the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImageHandle(ImageId);

/// How a texture is sampled and what its texels hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageKind {
    /// A premultiplied tile repeated across a shape, sampled nearest.
    Tile,
    /// Premultiplied texels (rasterised icons).
    Premultiplied,
    /// Straight-alpha texels (ARGB pixmaps converted to RGBA).
    Straight,
}

impl ImageKind {
    fn flags(self) -> ImageFlags {
        match self {
            ImageKind::Tile => {
                ImageFlags::REPEAT_X
                    | ImageFlags::REPEAT_Y
                    | ImageFlags::NEAREST
                    | ImageFlags::PREMULTIPLIED
            }
            ImageKind::Premultiplied => ImageFlags::PREMULTIPLIED,
            ImageKind::Straight => ImageFlags::empty(),
        }
    }
}

/// A textured quad of the glyph atlas: position in logical px, texture
/// coordinates normalised to the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlyphQuad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub s0: f32,
    pub t0: f32,
    pub s1: f32,
    pub t1: f32,
}

pub(crate) fn fv_color(c: Color) -> femtovg::Color {
    femtovg::Color::rgbaf(
        f32::from(c.0) / 255.0,
        f32::from(c.1) / 255.0,
        f32::from(c.2) / 255.0,
        c.3,
    )
}

fn rounded(rect: Rect, radius: f32) -> Path {
    let mut path = Path::new();
    path.rounded_rect(rect.x, rect.y, rect.w, rect.h, radius.max(0.0));
    path
}

/// The femtovg canvas over the shared GL context.
pub(crate) struct Backend {
    canvas: Canvas<OpenGl>,
    /// The offscreen images of the progressive-blur row, kept while the
    /// pixel size holds.
    row: Option<RowLayers>,
    /// The pixel size an image could not be made for; not retried.
    row_failed: Option<(u32, u32)>,
    /// The cached spotlight image.
    spot: Option<SpotImage>,
    /// Images freed after the next flush: commands recorded this frame may
    /// still read them.
    retired: Vec<ImageId>,
}

/// The offscreen images of a blurred row at one pixel size.
#[derive(Clone, Copy)]
pub(crate) struct RowLayers {
    w: u32,
    h: u32,
    src: ImageId,
    blur: [ImageId; 3],
    mask: [ImageId; 4],
    acc: ImageId,
}

impl RowLayers {
    pub(crate) fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    fn ids(self) -> impl Iterator<Item = ImageId> {
        std::iter::once(self.src)
            .chain(self.blur)
            .chain(self.mask)
            .chain(std::iter::once(self.acc))
    }
}

/// The cached spotlight: one premultiplied image and what it was made for.
#[derive(Clone, Copy)]
struct SpotImage {
    key: SpotKey,
    image: ImageId,
}

/// What a spotlight image depends on: the pill's width and the ratio.
pub(crate) type SpotKey = (u32, u32);

/// One filled, blurred polygon of the spotlight, in the image's logical px.
pub(crate) struct SpotShape {
    pub corners: [(f32, f32); 4],
    /// The vertical gradient: the y it starts and ends at, and white alphas
    /// at offsets 0..=1 along it.
    pub gradient: (f32, f32),
    pub stops: Vec<(f32, f32)>,
    pub sigma: f32,
}

/// Why the backend could not be created.
#[derive(Debug)]
pub(crate) enum BackendInit {
    Renderer(EglError),
    Canvas(GpuError),
}

impl Backend {
    /// Builds the canvas on the current GL context.
    pub(crate) fn new(
        proc_address: &dyn Fn(&str) -> *const c_void,
    ) -> Result<Backend, BackendInit> {
        let renderer = femtovg_renderer(proc_address).map_err(BackendInit::Renderer)?;
        let canvas = Canvas::new(renderer).map_err(|e| BackendInit::Canvas(e.into()))?;
        Ok(Backend {
            canvas,
            row: None,
            row_failed: None,
            spot: None,
            retired: Vec::new(),
        })
    }

    /// Starts a frame of `w`×`h` buffer px at `dpr`, cleared to transparent.
    pub(crate) fn begin(&mut self, w: u32, h: u32, dpr: f32) {
        self.canvas.reset();
        self.canvas.set_size(w, h, 1.0);
        self.canvas.scale(dpr, dpr);
        self.canvas.set_global_alpha(1.0);
        self.canvas
            .clear_rect(0, 0, w, h, femtovg::Color::rgbaf(0.0, 0.0, 0.0, 0.0));
    }

    /// Executes the recorded commands on the current framebuffer.
    pub(crate) fn flush(&mut self) {
        self.canvas.flush();
        for id in self.retired.drain(..) {
            self.canvas.delete_image(id);
        }
    }

    /// An empty premultiplied render-target image.
    fn target_image(&mut self, w: u32, h: u32) -> Option<ImageId> {
        self.canvas
            .create_image_empty(
                w as usize,
                h as usize,
                PixelFormat::Rgba8,
                ImageFlags::FLIP_Y | ImageFlags::PREMULTIPLIED,
            )
            .ok()
    }

    /// `n` target images of one size, or none (and nothing leaked).
    fn target_images(&mut self, n: usize, w: u32, h: u32) -> Option<Vec<ImageId>> {
        let mut made = Vec::with_capacity(n);
        for _ in 0..n {
            match self.target_image(w, h) {
                Some(id) => made.push(id),
                None => {
                    self.retired.extend(made);
                    return None;
                }
            }
        }
        Some(made)
    }

    /// The offscreen images for a `w`×`h` px row, kept while the size holds;
    /// a new size frees and remakes them. `None` when an image cannot be
    /// made (the size is then not tried again).
    pub(crate) fn row_layers(&mut self, w: u32, h: u32) -> Option<RowLayers> {
        if let Some(l) = self.row {
            if (l.w, l.h) == (w, h) {
                return Some(l);
            }
            self.retired.extend(l.ids());
            self.row = None;
        }
        if self.row_failed == Some((w, h)) {
            return None;
        }
        let Some(ids) = self.target_images(9, w, h) else {
            self.row_failed = Some((w, h));
            return None;
        };
        let l = RowLayers {
            w,
            h,
            src: ids[0],
            blur: [ids[1], ids[2], ids[3]],
            mask: [ids[4], ids[5], ids[6], ids[7]],
            acc: ids[8],
        };
        self.row = Some(l);
        Some(l)
    }

    /// Redirects drawing into `image` (`w`×`h` px), cleared: logical
    /// (`ox`, `oy`) is its top-left. Ends with `end_offscreen`.
    fn begin_offscreen(
        &mut self,
        image: ImageId,
        (w, h): (u32, u32),
        origin: (f32, f32),
        dpr: f32,
    ) {
        self.canvas.set_render_target(RenderTarget::Image(image));
        self.canvas.save();
        self.canvas.reset_transform();
        self.canvas.reset_scissor();
        self.canvas.set_global_alpha(1.0);
        self.canvas
            .clear_rect(0, 0, w, h, femtovg::Color::rgbaf(0.0, 0.0, 0.0, 0.0));
        self.canvas.scale(dpr, dpr);
        self.canvas.translate(-origin.0, -origin.1);
    }

    fn end_offscreen(&mut self) {
        self.canvas.restore();
        self.canvas.set_render_target(RenderTarget::Screen);
    }

    /// Starts drawing the row into the source image.
    pub(crate) fn begin_row(&mut self, l: &RowLayers, origin: (f32, f32), dpr: f32) {
        self.begin_offscreen(l.src, (l.w, l.h), origin, dpr);
    }

    pub(crate) fn end_row(&mut self) {
        self.end_offscreen();
    }

    /// Blurs the source into three levels, masks the four levels (sharp,
    /// then the blurs) by `stops` across the window and sums them into the
    /// accumulator. `window_x`/`window_w` are px of the row image; each
    /// level's stops are `(offset 0..=1 across the window, alpha)`.
    pub(crate) fn compose_row(
        &mut self,
        l: &RowLayers,
        sigmas_px: [f32; 3],
        window: (f32, f32),
        stops: &[Vec<(f32, f32)>; 4],
    ) {
        for (blur, sigma) in l.blur.into_iter().zip(sigmas_px) {
            self.canvas
                .filter_image(blur, ImageFilter::GaussianBlur { sigma }, l.src);
        }
        let (w, h) = (l.w as f32, l.h as f32);
        let mut full = Path::new();
        full.rect(0.0, 0.0, w, h);
        let clear = femtovg::Color::rgbaf(0.0, 0.0, 0.0, 0.0);
        let levels = [l.src, l.blur[0], l.blur[1], l.blur[2]];
        for ((level, mask), level_stops) in levels.into_iter().zip(l.mask).zip(stops) {
            self.canvas.set_render_target(RenderTarget::Image(mask));
            self.canvas.save();
            self.canvas.reset_transform();
            self.canvas.reset_scissor();
            self.canvas.set_global_alpha(1.0);
            self.canvas.clear_rect(0, 0, l.w, l.h, clear);
            self.canvas.fill_path(
                &full,
                &Paint::image(level, 0.0, 0.0, w, h, 0.0, 1.0).with_anti_alias(false),
            );
            self.canvas
                .global_composite_operation(CompositeOperation::DestinationIn);
            let colors = level_stops
                .iter()
                .map(|&(offset, alpha)| (offset, femtovg::Color::rgbaf(1.0, 1.0, 1.0, alpha)));
            self.canvas.fill_path(
                &full,
                &Paint::linear_gradient_stops(window.0, 0.0, window.0 + window.1, 0.0, colors)
                    .with_anti_alias(false),
            );
            self.canvas.restore();
        }
        self.canvas.set_render_target(RenderTarget::Image(l.acc));
        self.canvas.save();
        self.canvas.reset_transform();
        self.canvas.reset_scissor();
        self.canvas.set_global_alpha(1.0);
        self.canvas.clear_rect(0, 0, l.w, l.h, clear);
        self.canvas
            .global_composite_operation(CompositeOperation::Lighter);
        for mask in l.mask {
            self.canvas.fill_path(
                &full,
                &Paint::image(mask, 0.0, 0.0, w, h, 0.0, 1.0).with_anti_alias(false),
            );
        }
        self.canvas.restore();
        self.canvas.set_render_target(RenderTarget::Screen);
    }

    /// Draws the summed row over `rect` (logical px) as it is.
    pub(crate) fn draw_row(&mut self, l: &RowLayers, rect: Rect) {
        self.blit(l.acc, rect);
    }

    /// Draws `image` stretched over `rect` with no fringe, under the
    /// current alpha, clip and transform.
    fn blit(&mut self, image: ImageId, rect: Rect) {
        let mut path = Path::new();
        path.rect(rect.x, rect.y, rect.w, rect.h);
        self.canvas.fill_path(
            &path,
            &Paint::image(image, rect.x, rect.y, rect.w, rect.h, 0.0, 1.0).with_anti_alias(false),
        );
    }

    /// The cached spotlight image for `key`, built by `shapes` into an
    /// image of `size` px at `dpr` on first use or when the key changes.
    pub(crate) fn spot_image(
        &mut self,
        key: SpotKey,
        size: (u32, u32),
        dpr: f32,
        shapes: &[SpotShape; 2],
    ) -> Option<ImageId> {
        if let Some(spot) = self.spot {
            if spot.key == key {
                return Some(spot.image);
            }
            self.retired.push(spot.image);
            self.spot = None;
        }
        let ids = self.target_images(4, size.0, size.1)?;
        let (scratch, out) = (ids[0], ids[3]);
        let clear = femtovg::Color::rgbaf(0.0, 0.0, 0.0, 0.0);
        // Each shape: filled into the scratch image, blurred into its own.
        let targets = [ids[1], ids[2]];
        for (shape, target) in shapes.iter().zip(targets) {
            self.begin_offscreen(scratch, size, (0.0, 0.0), dpr);
            let mut path = Path::new();
            let [first, rest @ ..] = shape.corners;
            path.move_to(first.0, first.1);
            for c in rest {
                path.line_to(c.0, c.1);
            }
            path.close();
            let colors = shape
                .stops
                .iter()
                .map(|&(offset, alpha)| (offset, femtovg::Color::rgbaf(1.0, 1.0, 1.0, alpha)));
            self.canvas.fill_path(
                &path,
                &Paint::linear_gradient_stops(0.0, shape.gradient.0, 0.0, shape.gradient.1, colors),
            );
            self.end_offscreen();
            self.canvas.filter_image(
                target,
                ImageFilter::GaussianBlur {
                    sigma: shape.sigma * dpr,
                },
                scratch,
            );
        }
        // The two blurred shapes, added into the cached image.
        self.canvas.set_render_target(RenderTarget::Image(out));
        self.canvas.save();
        self.canvas.reset_transform();
        self.canvas.reset_scissor();
        self.canvas.set_global_alpha(1.0);
        self.canvas.clear_rect(0, 0, size.0, size.1, clear);
        self.canvas
            .global_composite_operation(CompositeOperation::Lighter);
        let mut full = Path::new();
        full.rect(0.0, 0.0, size.0 as f32, size.1 as f32);
        for image in targets {
            self.canvas.fill_path(
                &full,
                &Paint::image(image, 0.0, 0.0, size.0 as f32, size.1 as f32, 0.0, 1.0)
                    .with_anti_alias(false),
            );
        }
        self.canvas.restore();
        self.canvas.set_render_target(RenderTarget::Screen);
        self.retired.extend([scratch, ids[1], ids[2]]);
        self.spot = Some(SpotImage { key, image: out });
        Some(out)
    }

    /// Draws a spotlight image over `rect` (logical px) at `alpha`.
    pub(crate) fn draw_spot(&mut self, image: ImageId, rect: Rect, alpha: f32) {
        let mut path = Path::new();
        path.rect(rect.x, rect.y, rect.w, rect.h);
        self.canvas.fill_path(
            &path,
            &Paint::image(image, rect.x, rect.y, rect.w, rect.h, 0.0, alpha).with_anti_alias(false),
        );
    }

    pub(crate) fn save(&mut self) {
        self.canvas.save();
    }

    pub(crate) fn restore(&mut self) {
        self.canvas.restore();
    }

    pub(crate) fn set_global_alpha(&mut self, alpha: f32) {
        self.canvas.set_global_alpha(alpha);
    }

    pub(crate) fn translate(&mut self, dx: f32, dy: f32) {
        self.canvas.translate(dx, dy);
    }

    pub(crate) fn scale_about(&mut self, cx: f32, cy: f32, s: f32) {
        self.canvas.translate(cx, cy);
        self.canvas.scale(s, s);
        self.canvas.translate(-cx, -cy);
    }

    pub(crate) fn clip_rounded(&mut self, rect: Rect, radius: f32) {
        self.canvas
            .intersect_rounded_scissor(rect.x, rect.y, rect.w, rect.h, radius.max(0.0));
    }

    pub(crate) fn fill_rounded(&mut self, rect: Rect, radius: f32, color: Color) {
        self.canvas
            .fill_path(&rounded(rect, radius), &Paint::color(fv_color(color)));
    }

    /// A rounded outline of `width` centred on the rect edge.
    pub(crate) fn stroke_rounded(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        let paint = Paint::color(fv_color(color)).with_line_width(width);
        self.canvas.stroke_path(&rounded(rect, radius), &paint);
    }

    /// Fills a rounded rect with `image` mapped onto `placement` (repeated
    /// beyond it for a tile), multiplied by `tint`.
    pub(crate) fn fill_image(
        &mut self,
        rect: Rect,
        radius: f32,
        image: ImageHandle,
        placement: Rect,
        tint: Color,
    ) {
        let paint = Paint::image_tint(
            image.0,
            placement.x,
            placement.y,
            placement.w,
            placement.h,
            0.0,
            fv_color(tint),
        );
        self.canvas.fill_path(&rounded(rect, radius), &paint);
    }

    /// Draws `image` stretched exactly over `rect` with no tint and no
    /// antialiased fringe, so its edge texels are not blended with the
    /// clear colour (the wallpaper).
    pub(crate) fn blit_image(&mut self, rect: Rect, image: ImageHandle) {
        let paint =
            Paint::image(image.0, rect.x, rect.y, rect.w, rect.h, 0.0, 1.0).with_anti_alias(false);
        let mut path = Path::new();
        path.rect(rect.x, rect.y, rect.w, rect.h);
        self.canvas.fill_path(&path, &paint);
    }

    /// Uploads `width`×`height` RGBA texels as a new texture.
    pub(crate) fn create_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        kind: ImageKind,
    ) -> Result<ImageHandle, GpuError> {
        let img = texels(width, height, rgba)?;
        self.canvas
            .create_image(ImageSource::Rgba(img), kind.flags())
            .map(ImageHandle)
            .map_err(GpuError::from)
    }

    /// An empty `size`×`size` RGBA page for the glyph atlas.
    pub(crate) fn create_atlas(&mut self, size: u32) -> Result<ImageHandle, GpuError> {
        self.canvas
            .create_image_empty(
                size as usize,
                size as usize,
                PixelFormat::Rgba8,
                ImageFlags::empty(),
            )
            .map(ImageHandle)
            .map_err(GpuError::from)
    }

    /// Replaces a `width`×`height` region of a texture at (`x`, `y`).
    pub(crate) fn update_image(
        &mut self,
        image: ImageHandle,
        (x, y): (u32, u32),
        (width, height): (u32, u32),
        rgba: &[u8],
    ) -> Result<(), GpuError> {
        let img = texels(width, height, rgba)?;
        self.canvas
            .update_image(image.0, ImageSource::Rgba(img), x as usize, y as usize)
            .map_err(GpuError::from)
    }

    pub(crate) fn delete_image(&mut self, image: ImageHandle) {
        self.canvas.delete_image(image.0);
    }

    /// Draws atlas quads under the current alpha: coverage glyphs through
    /// femtovg's glyph path in `color`; colour glyphs (emoji) as image fills
    /// of the page, because the glyph path's colour texture discards the
    /// paint alpha (and with it `opacity`), while an image fill honours it.
    pub(crate) fn draw_glyphs(
        &mut self,
        atlas: ImageHandle,
        coverage: Vec<GlyphQuad>,
        colored: Vec<GlyphQuad>,
        color: Color,
    ) {
        if !coverage.is_empty() {
            let commands = GlyphDrawCommands {
                alpha_glyphs: vec![DrawCommand {
                    image_id: atlas.0,
                    quads: coverage
                        .into_iter()
                        .map(|q| Quad {
                            x0: q.x0,
                            y0: q.y0,
                            s0: q.s0,
                            t0: q.t0,
                            x1: q.x1,
                            y1: q.y1,
                            s1: q.s1,
                            t1: q.t1,
                        })
                        .collect(),
                }],
                color_glyphs: Vec::new(),
            };
            self.canvas
                .draw_glyph_commands(commands, &Paint::color(fv_color(color)));
        }
        let white = Color(0xff, 0xff, 0xff, 1.0);
        for quad in colored {
            let Some(page) = page_placement(&quad) else {
                continue;
            };
            let cell = Rect {
                x: quad.x0,
                y: quad.y0,
                w: quad.x1 - quad.x0,
                h: quad.y1 - quad.y0,
            };
            self.fill_image(cell, 0.0, atlas, page, white);
        }
    }
}

/// Where the whole atlas page lies, in the quad's coordinates, so that an
/// image fill of the quad's rect samples exactly its cell.
fn page_placement(q: &GlyphQuad) -> Option<Rect> {
    let (ds, dt) = (q.s1 - q.s0, q.t1 - q.t0);
    if ds <= 0.0 || dt <= 0.0 {
        return None;
    }
    let w = (q.x1 - q.x0) / ds;
    let h = (q.y1 - q.y0) / dt;
    Some(Rect {
        x: q.x0 - q.s0 * w,
        y: q.y0 - q.t0 * h,
        w,
        h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Role, Theme};

    #[test]
    fn color_converts_to_femtovg_keeping_alpha() {
        let theme = Theme::dark();
        let c = fv_color(theme.color(Role::Divider));
        assert!((c.a - 22.0 / 255.0).abs() < 1e-6);
        let c = fv_color(theme.color(Role::Card));
        assert_eq!(c.a, 1.0);
        assert!((c.r - 11.0 / 255.0).abs() < 1e-6);
        assert!((c.b - 16.0 / 255.0).abs() < 1e-6);
        let c = fv_color(Color(255, 255, 255, 0.5));
        assert_eq!((c.r, c.g, c.b, c.a), (1.0, 1.0, 1.0, 0.5));
    }

    #[test]
    fn texels_refuse_empty_and_mismatched_sizes() {
        assert!(texels(2, 1, &[0; 8]).is_ok());
        assert!(matches!(
            texels(0, 1, &[]),
            Err(GpuError(Fault::Texels { .. }))
        ));
        assert!(matches!(
            texels(2, 1, &[0; 7]),
            Err(GpuError(Fault::Texels { .. }))
        ));
        assert!(matches!(
            texels(u32::MAX, u32::MAX, &[]),
            Err(GpuError(Fault::Texels { .. }))
        ));
    }

    #[test]
    fn a_colour_glyph_fill_maps_the_page_onto_its_cell() {
        // A 20×20 texel cell at (512, 256) of the 1024 page, drawn at
        // (10, 30) logical px at dpr 2 (10×10 logical).
        let quad = GlyphQuad {
            x0: 10.0,
            y0: 30.0,
            x1: 20.0,
            y1: 40.0,
            s0: 512.0 / 1024.0,
            t0: 256.0 / 1024.0,
            s1: 532.0 / 1024.0,
            t1: 276.0 / 1024.0,
        };
        let page = page_placement(&quad).expect("a non-empty cell");
        assert_eq!((page.w, page.h), (512.0, 512.0), "1024 texels at dpr 2");
        assert_eq!((page.x, page.y), (10.0 - 256.0, 30.0 - 128.0));
        let empty = GlyphQuad {
            s1: quad.s0,
            ..quad
        };
        assert!(page_placement(&empty).is_none());
    }
}
