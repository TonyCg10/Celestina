//! How much of a captured frame a preview shows, and at what size.
//!
//! The overview panel draws a workspace's picture as its preview. The picture
//! arrives as one whole-output `wl_shm` frame; this module cuts a rectangle
//! out of it (the whole frame, for a workspace preview) and shrinks the cut
//! to a thumbnail small enough to keep one per output. It is plain arithmetic
//! on byte slices: no Wayland, no GL, no allocation sized by anything the
//! compositor said that was not checked first.
//!
//! The frame is hostile input. Its dimensions, stride and length come from
//! another process, so every one is validated before a byte is read:
//! dimensions above [`MAX_FRAME_SIDE`], a stride too small for a row, a stride
//! times height that overflows or outruns the data, and any pixel format other
//! than the two 32-bit ones are refused with a typed [`ThumbError`]. A refusal
//! is the caller's cue to draw the tile without a picture.
//!
//! Pixels leave as opaque RGBA8 with the longer side at most
//! [`PREVIEW_MAX_SIDE`]; each output pixel is the mean of the source pixels it
//! covers (a box filter), so a frame shrunk to a sixth is smooth rather than
//! aliased. Nothing here logs, stores or keeps pixels past the call, and the
//! `Debug` forms of [`Frame`] and [`Thumb`] print sizes, never pixels.

use std::fmt;

/// The longest side a preview may have, in pixels. A preview is at most
/// 640 × 640 × 4 bytes = 1600 KiB (about 1.6 MiB, a square output), 900 KiB
/// for a 16:9 one.
pub const PREVIEW_MAX_SIDE: u32 = 640;
/// The widest or tallest frame accepted, in pixels. A larger announced size is
/// refused before anything is read.
pub const MAX_FRAME_SIDE: u32 = 8192;

/// An axis-aligned rectangle in pixels. The origin may be negative (a window
/// partly off its output); the size is never negative by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl Rect {
    /// A rectangle from its origin and size.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// The part of `rect` that lies inside `output`, in the same coordinates, or
/// `None` when they do not overlap or either has no area.
///
/// Arithmetic is done in `i64`, so no pair of rectangles can overflow it.
pub fn clip(rect: Rect, output: Rect) -> Option<Rect> {
    let left = i64::from(rect.x).max(i64::from(output.x));
    let top = i64::from(rect.y).max(i64::from(output.y));
    let right = (i64::from(rect.x) + i64::from(rect.width))
        .min(i64::from(output.x) + i64::from(output.width));
    let bottom = (i64::from(rect.y) + i64::from(rect.height))
        .min(i64::from(output.y) + i64::from(output.height));
    if right <= left || bottom <= top {
        return None;
    }
    Some(Rect {
        x: i32::try_from(left).ok()?,
        y: i32::try_from(top).ok()?,
        width: u32::try_from(right - left).ok()?,
        height: u32::try_from(bottom - top).ok()?,
    })
}

/// The 32-bit pixel formats a frame may have. Both are little-endian words, so
/// the bytes of a pixel are blue, green, red, then alpha (or padding).
///
/// Both are read as opaque. An output's picture is opaque by nature, and
/// Wayland's `ARGB8888` is premultiplied: passing its alpha on beside colour
/// that is already multiplied by it would be wrong as straight RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// `WL_SHM_FORMAT_ARGB8888`: premultiplied alpha, which is dropped.
    Argb8888,
    /// `WL_SHM_FORMAT_XRGB8888`: the fourth byte is padding; the pixel is opaque.
    Xrgb8888,
}

impl PixelFormat {
    /// The format of a `wl_shm` format code, or [`ThumbError::UnsupportedFormat`]
    /// for any other.
    ///
    /// # Errors
    /// Every code but 0 (Argb8888) and 1 (Xrgb8888).
    pub fn from_wl_code(code: u32) -> Result<Self, ThumbError> {
        match code {
            0 => Ok(Self::Argb8888),
            1 => Ok(Self::Xrgb8888),
            other => Err(ThumbError::UnsupportedFormat(other)),
        }
    }
}

/// One captured frame, borrowed: the compositor's announced geometry and the
/// bytes. Nothing is trusted until [`crop_scale`] validates it.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes from the start of one row to the start of the next.
    pub stride: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// The pixel bytes, at least `stride * height` of them.
    pub data: &'a [u8],
}

impl fmt::Debug for Frame<'_> {
    /// Geometry and length only: a frame's pixels never reach a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("format", &self.format)
            .field("bytes", &self.data.len())
            .finish()
    }
}

/// A finished thumbnail: tightly packed RGBA8 rows.
#[derive(Clone, PartialEq, Eq)]
pub struct Thumb {
    /// Width in pixels, at least 1.
    pub width: u32,
    /// Height in pixels, at least 1.
    pub height: u32,
    /// `width * height * 4` bytes, red, green, blue, alpha (always 255).
    pub rgba: Vec<u8>,
}

impl fmt::Debug for Thumb {
    /// Size and length only: a preview's pixels never reach a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Thumb")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.rgba.len())
            .finish()
    }
}

/// Why a frame produced no thumbnail. Every variant means "draw the tile
/// without a picture".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbError {
    /// The pixel format is not Argb8888 or Xrgb8888 (the `wl_shm` code).
    UnsupportedFormat(u32),
    /// A dimension is zero or above [`MAX_FRAME_SIDE`].
    BadDimensions {
        /// Announced width.
        width: u32,
        /// Announced height.
        height: u32,
    },
    /// The stride cannot hold one row of the announced width.
    BadStride {
        /// Announced stride.
        stride: u32,
        /// Announced width.
        width: u32,
    },
    /// The data is shorter than `stride * height`.
    ShortData {
        /// Bytes required.
        needed: u64,
        /// Bytes present.
        got: usize,
    },
    /// The crop does not overlap the frame, or `max_side` is zero.
    EmptyCrop,
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(code) => write!(f, "unsupported pixel format {code}"),
            Self::BadDimensions { width, height } => {
                write!(
                    f,
                    "frame size {width}x{height} is outside 1..={MAX_FRAME_SIDE}"
                )
            }
            Self::BadStride { stride, width } => {
                write!(f, "stride {stride} cannot hold a row of {width} pixels")
            }
            Self::ShortData { needed, got } => {
                write!(f, "frame data has {got} bytes, {needed} needed")
            }
            Self::EmptyCrop => f.write_str("crop is empty or outside the frame"),
        }
    }
}

impl std::error::Error for ThumbError {}

/// The thumbnail of `rect` (in frame pixels) cut out of `frame`, with its
/// longer side at most `max_side` (itself capped at [`PREVIEW_MAX_SIDE`]).
/// A crop already small enough keeps its size; a larger one is shrunk by one
/// factor on both axes, never below one pixel on either. Every pixel leaves
/// opaque (alpha 255), whatever the format (see [`PixelFormat`]).
///
/// `rect` is clipped to the frame first, so a rectangle partly off the output
/// yields the visible part.
///
/// # Errors
/// [`ThumbError`] for an unsupported-size, bad-stride or short frame, and
/// [`ThumbError::EmptyCrop`] when nothing of `rect` is inside the frame or
/// `max_side` is zero.
pub fn crop_scale(frame: &Frame<'_>, rect: Rect, max_side: u32) -> Result<Thumb, ThumbError> {
    validate(frame)?;
    let max_side = max_side.min(PREVIEW_MAX_SIDE);
    if max_side == 0 {
        return Err(ThumbError::EmptyCrop);
    }
    let bounds = Rect::new(0, 0, frame.width, frame.height);
    let crop = clip(rect, bounds).ok_or(ThumbError::EmptyCrop)?;
    // `clip` against the frame bounds makes these non-negative.
    let cx = u64::from(crop.x.unsigned_abs());
    let cy = u64::from(crop.y.unsigned_abs());
    let (cw, ch) = (u64::from(crop.width), u64::from(crop.height));

    let longer = cw.max(ch);
    let max = u64::from(max_side);
    let (ow, oh) = if longer <= max {
        (cw, ch)
    } else {
        (
            ((cw * max + longer / 2) / longer).max(1),
            ((ch * max + longer / 2) / longer).max(1),
        )
    };

    let stride = u64::from(frame.stride);
    let mut rgba =
        Vec::with_capacity(usize::try_from(ow * oh * 4).map_err(|_| ThumbError::EmptyCrop)?);
    for dy in 0..oh {
        let (y0, y1) = span(dy, oh, ch);
        for dx in 0..ow {
            let (x0, x1) = span(dx, ow, cw);
            let mut sum = [0u64; 3];
            for sy in y0..y1 {
                let row = (cy + sy) * stride;
                for sx in x0..x1 {
                    let at =
                        usize::try_from(row + (cx + sx) * 4).map_err(|_| ThumbError::EmptyCrop)?;
                    let px = frame.data.get(at..at + 4).ok_or(ThumbError::ShortData {
                        needed: row + (cx + sx) * 4 + 4,
                        got: frame.data.len(),
                    })?;
                    for (acc, byte) in sum.iter_mut().zip(px) {
                        *acc += u64::from(*byte);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let mean = |v: u64| u8::try_from((v + n / 2) / n).unwrap_or(u8::MAX);
            // Bytes are B, G, R, A: emit R, G, B and an opaque alpha.
            rgba.extend_from_slice(&[mean(sum[2]), mean(sum[1]), mean(sum[0]), 255]);
        }
    }
    Ok(Thumb {
        width: u32::try_from(ow).map_err(|_| ThumbError::EmptyCrop)?,
        height: u32::try_from(oh).map_err(|_| ThumbError::EmptyCrop)?,
        rgba,
    })
}

/// The source pixels `[start, end)` that output pixel `i` of `out` covers on an
/// axis of `src` pixels: never empty, never past `src`.
fn span(i: u64, out: u64, src: u64) -> (u64, u64) {
    let start = (i * src / out).min(src - 1);
    let end = ((i + 1) * src).div_ceil(out).clamp(start + 1, src);
    (start, end)
}

/// Refuse a frame whose announced geometry the data cannot back.
fn validate(frame: &Frame<'_>) -> Result<(), ThumbError> {
    let (w, h) = (frame.width, frame.height);
    if w == 0 || h == 0 || w > MAX_FRAME_SIDE || h > MAX_FRAME_SIDE {
        return Err(ThumbError::BadDimensions {
            width: w,
            height: h,
        });
    }
    if u64::from(frame.stride) < u64::from(w) * 4 {
        return Err(ThumbError::BadStride {
            stride: frame.stride,
            width: w,
        });
    }
    let needed = u64::from(frame.stride) * u64::from(h);
    if u64::try_from(frame.data.len()).map_or(true, |got| got < needed) {
        return Err(ThumbError::ShortData {
            needed,
            got: frame.data.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of one colour per pixel from `f(x, y) -> [b, g, r, a]`.
    fn build(w: u32, h: u32, pad: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> (Vec<u8>, u32) {
        let stride = w * 4 + pad;
        let mut data = vec![0xEE; (stride * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let at = (y * stride + x * 4) as usize;
                data[at..at + 4].copy_from_slice(&f(x, y));
            }
        }
        (data, stride)
    }

    fn frame(w: u32, h: u32, stride: u32, format: PixelFormat, data: &[u8]) -> Frame<'_> {
        Frame {
            width: w,
            height: h,
            stride,
            format,
            data,
        }
    }

    #[test]
    fn clip_inside_is_unchanged() {
        let out = Rect::new(0, 0, 100, 100);
        let r = Rect::new(10, 20, 30, 40);
        assert_eq!(clip(r, out), Some(r));
    }

    #[test]
    fn clip_cuts_each_edge() {
        let out = Rect::new(0, 0, 100, 100);
        assert_eq!(
            clip(Rect::new(-10, -10, 30, 30), out),
            Some(Rect::new(0, 0, 20, 20))
        );
        assert_eq!(
            clip(Rect::new(90, 95, 30, 30), out),
            Some(Rect::new(90, 95, 10, 5))
        );
    }

    #[test]
    fn clip_uses_output_origin() {
        let out = Rect::new(1920, 0, 1920, 1080);
        assert_eq!(
            clip(Rect::new(1900, 10, 100, 100), out),
            Some(Rect::new(1920, 10, 80, 100))
        );
    }

    #[test]
    fn clip_touching_or_disjoint_or_empty_is_none() {
        let out = Rect::new(0, 0, 100, 100);
        assert_eq!(clip(Rect::new(100, 0, 10, 10), out), None);
        assert_eq!(clip(Rect::new(-10, 0, 10, 10), out), None);
        assert_eq!(clip(Rect::new(500, 500, 10, 10), out), None);
        assert_eq!(clip(Rect::new(10, 10, 0, 10), out), None);
        assert_eq!(clip(Rect::new(10, 10, 10, 10), Rect::new(0, 0, 0, 0)), None);
    }

    #[test]
    fn clip_extremes_do_not_overflow() {
        let huge = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        let out = Rect::new(i32::MAX - 5, i32::MAX - 5, u32::MAX, u32::MAX);
        let got = clip(huge, out);
        assert!(got.is_none() || got.is_some_and(|r| r.width > 0 && r.height > 0));
        let edge = Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX);
        assert_eq!(clip(edge, edge), Some(edge));
        assert_eq!(
            clip(Rect::new(0, 0, u32::MAX, u32::MAX), Rect::new(0, 0, 10, 10)),
            Some(Rect::new(0, 0, 10, 10))
        );
    }

    #[test]
    fn small_crop_keeps_size_and_swaps_channels() {
        // Pixel bytes B, G, R, A.
        let (data, stride) = build(4, 4, 0, |_, _| [10, 20, 30, 128]);
        let f = frame(4, 4, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 4, 4), 320).unwrap();
        assert_eq!((t.width, t.height), (4, 4));
        assert_eq!(t.rgba.len(), 64);
        assert_eq!(&t.rgba[..4], &[30, 20, 10, 255]);
    }

    #[test]
    fn xrgb_is_opaque_whatever_the_padding_byte() {
        let (data, stride) = build(2, 2, 0, |_, _| [1, 2, 3, 0]);
        let f = frame(2, 2, stride, PixelFormat::Xrgb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 2, 2), 320).unwrap();
        assert!(t.rgba.chunks(4).all(|p| p == [3, 2, 1, 255]));
    }

    #[test]
    fn premultiplied_argb_is_made_opaque_never_passed_on_as_straight_alpha() {
        // Half-transparent premultiplied red: colour already scaled by alpha.
        let (data, stride) = build(2, 2, 0, |x, _| {
            if x == 0 {
                [0, 0, 128, 128]
            } else {
                [0, 0, 0, 0]
            }
        });
        let f = frame(2, 2, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 2, 2), 320).unwrap();
        assert!(t.rgba.chunks(4).all(|p| p[3] == 255));
        assert_eq!(&t.rgba[..4], &[128, 0, 0, 255]);
        assert_eq!(&t.rgba[4..8], &[0, 0, 0, 255]);
        let shrunk = crop_scale(&f, Rect::new(0, 0, 2, 2), 1).unwrap();
        assert_eq!(shrunk.rgba, vec![64, 0, 0, 255]);
    }

    #[test]
    fn debug_forms_print_sizes_never_pixels() {
        let thumb = Thumb {
            width: 2,
            height: 1,
            rgba: vec![201, 202, 203, 204, 205, 206, 207, 208],
        };
        let text = format!("{thumb:?}");
        assert_eq!(text, "Thumb { width: 2, height: 1, bytes: 8 }");
        assert!(!text.contains("201") && !text.contains("rgba"));
        let data = [211u8, 212, 213, 214];
        let f = frame(1, 1, 4, PixelFormat::Xrgb8888, &data);
        let text = format!("{f:?}");
        assert!(text.contains("bytes: 4"), "{text}");
        assert!(!text.contains("211") && !text.contains("data"), "{text}");
    }

    #[test]
    fn crop_selects_the_region() {
        let (data, stride) = build(10, 10, 0, |x, y| [x as u8, y as u8, 0, 255]);
        let f = frame(10, 10, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(3, 5, 2, 2), 320).unwrap();
        assert_eq!((t.width, t.height), (2, 2));
        // First pixel is (3,5): b=3, g=5 -> rgba [0, 5, 3, 255].
        assert_eq!(&t.rgba[..4], &[0, 5, 3, 255]);
        assert_eq!(&t.rgba[12..16], &[0, 6, 4, 255]);
    }

    #[test]
    fn stride_padding_is_skipped() {
        let (data, stride) = build(3, 3, 8, |x, y| [(x + y * 3) as u8, 0, 0, 255]);
        assert_eq!(stride, 20);
        let f = frame(3, 3, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 3, 3), 320).unwrap();
        let reds: Vec<u8> = t.rgba.chunks(4).map(|p| p[2]).collect();
        assert_eq!(reds, (0..9).collect::<Vec<u8>>());
    }

    #[test]
    fn downscale_averages_blocks() {
        // 4x4 of left half 0, right half 200 -> 2x2 keeps the split.
        let (data, stride) = build(4, 4, 0, |x, _| {
            let v = if x < 2 { 0 } else { 200 };
            [v, v, v, 255]
        });
        let f = frame(4, 4, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 4, 4), 2).unwrap();
        assert_eq!((t.width, t.height), (2, 2));
        assert_eq!(&t.rgba[..4], &[0, 0, 0, 255]);
        assert_eq!(&t.rgba[4..8], &[200, 200, 200, 255]);
    }

    #[test]
    fn downscale_mean_rounds_to_nearest() {
        let (data, stride) = build(2, 1, 0, |x, _| {
            let v = if x == 0 { 0 } else { 1 };
            [v, v, v, 255]
        });
        let f = frame(2, 1, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 2, 1), 1).unwrap();
        assert_eq!((t.width, t.height), (1, 1));
        // (0 + 1) / 2 = 0.5 rounds up.
        assert_eq!(t.rgba[0], 1);
    }

    #[test]
    fn longer_side_is_capped_and_aspect_kept() {
        let (data, stride) = build(1000, 500, 0, |_, _| [9, 9, 9, 255]);
        let f = frame(1000, 500, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 1000, 500), 320).unwrap();
        assert_eq!((t.width, t.height), (320, 160));
        assert!(t.rgba.chunks(4).all(|p| p == [9, 9, 9, 255]));
        let tall = crop_scale(&f, Rect::new(0, 0, 3, 500), 320).unwrap();
        assert_eq!((tall.width, tall.height), (2, 320));
    }

    #[test]
    fn extreme_aspect_never_reaches_zero() {
        let (data, stride) = build(1000, 2, 0, |_, _| [5, 5, 5, 255]);
        let f = frame(1000, 2, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 1000, 2), 10).unwrap();
        assert_eq!((t.width, t.height), (10, 1));
        let one = crop_scale(&f, Rect::new(0, 0, 1000, 1), 1).unwrap();
        assert_eq!((one.width, one.height), (1, 1));
    }

    #[test]
    fn odd_sizes_cover_every_source_pixel_once_per_axis() {
        for src in 1..60u64 {
            for out in 1..=src {
                let mut covered = 0;
                let mut last_end: u64 = 0;
                for i in 0..out {
                    let (s, e) = span(i, out, src);
                    assert!(s < e && e <= src, "src {src} out {out} i {i}");
                    assert!(s >= last_end.saturating_sub(1));
                    covered += e - s;
                    last_end = e;
                }
                assert_eq!(last_end, src);
                assert!(covered >= src);
            }
        }
    }

    #[test]
    fn one_by_one_frame() {
        let data = [7u8, 8, 9, 10];
        let f = frame(1, 1, 4, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 1, 1), 320).unwrap();
        assert_eq!(
            t,
            Thumb {
                width: 1,
                height: 1,
                rgba: vec![9, 8, 7, 255]
            }
        );
    }

    #[test]
    fn partly_outside_rect_yields_the_visible_part() {
        let (data, stride) = build(4, 4, 0, |_, _| [1, 1, 1, 255]);
        let f = frame(4, 4, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(-2, -2, 4, 4), 320).unwrap();
        assert_eq!((t.width, t.height), (2, 2));
    }

    #[test]
    fn outside_rect_or_zero_max_is_empty_crop() {
        let (data, stride) = build(4, 4, 0, |_, _| [1, 1, 1, 255]);
        let f = frame(4, 4, stride, PixelFormat::Argb8888, &data);
        assert_eq!(
            crop_scale(&f, Rect::new(10, 10, 4, 4), 320),
            Err(ThumbError::EmptyCrop)
        );
        assert_eq!(
            crop_scale(&f, Rect::new(0, 0, 4, 4), 0),
            Err(ThumbError::EmptyCrop)
        );
    }

    #[test]
    fn max_side_above_the_cap_is_capped() {
        let (data, stride) = build(800, 10, 0, |_, _| [1, 1, 1, 255]);
        let f = frame(800, 10, stride, PixelFormat::Argb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 800, 10), u32::MAX).unwrap();
        assert_eq!(t.width, PREVIEW_MAX_SIDE);
    }

    #[test]
    fn a_whole_output_frame_becomes_a_preview_at_the_cap() {
        // A 1920x1080 output shrinks by three to 640x360.
        let (data, stride) = build(1920, 1080, 0, |x, _| {
            let v = if x < 960 { 0 } else { 240 };
            [v, v, v, 0]
        });
        let f = frame(1920, 1080, stride, PixelFormat::Xrgb8888, &data);
        let t = crop_scale(&f, Rect::new(0, 0, 1920, 1080), PREVIEW_MAX_SIDE).unwrap();
        assert_eq!((t.width, t.height), (640, 360));
        assert_eq!(&t.rgba[..4], &[0, 0, 0, 255]);
        assert_eq!(&t.rgba[639 * 4..640 * 4], &[240, 240, 240, 255]);
    }

    #[test]
    fn hostile_dimensions_are_refused_before_reading() {
        let data = [0u8; 16];
        let r = Rect::new(0, 0, 1, 1);
        for (w, h) in [(0, 1), (1, 0), (8193, 1), (1, 8193), (u32::MAX, u32::MAX)] {
            let f = frame(w, h, u32::MAX, PixelFormat::Argb8888, &data);
            assert_eq!(
                crop_scale(&f, r, 320),
                Err(ThumbError::BadDimensions {
                    width: w,
                    height: h
                })
            );
        }
    }

    #[test]
    fn max_frame_size_passes_dimension_check_but_needs_data() {
        let data = [0u8; 16];
        let f = frame(8192, 8192, 8192 * 4, PixelFormat::Argb8888, &data);
        let err = crop_scale(&f, Rect::new(0, 0, 1, 1), 320).unwrap_err();
        assert_eq!(
            err,
            ThumbError::ShortData {
                needed: 8192 * 4 * 8192,
                got: 16
            }
        );
    }

    #[test]
    fn short_stride_is_refused() {
        let data = [0u8; 64];
        let f = frame(4, 4, 15, PixelFormat::Argb8888, &data);
        assert_eq!(
            crop_scale(&f, Rect::new(0, 0, 4, 4), 320),
            Err(ThumbError::BadStride {
                stride: 15,
                width: 4
            })
        );
    }

    #[test]
    fn short_data_is_refused() {
        let data = [0u8; 63];
        let f = frame(4, 4, 16, PixelFormat::Argb8888, &data);
        assert_eq!(
            crop_scale(&f, Rect::new(0, 0, 4, 4), 320),
            Err(ThumbError::ShortData {
                needed: 64,
                got: 63
            })
        );
    }

    #[test]
    fn huge_stride_times_height_does_not_overflow() {
        let data = [0u8; 16];
        let f = frame(1, 8192, u32::MAX, PixelFormat::Argb8888, &data);
        assert!(matches!(
            crop_scale(&f, Rect::new(0, 0, 1, 1), 320),
            Err(ThumbError::ShortData { .. })
        ));
    }

    #[test]
    fn format_codes() {
        assert_eq!(PixelFormat::from_wl_code(0), Ok(PixelFormat::Argb8888));
        assert_eq!(PixelFormat::from_wl_code(1), Ok(PixelFormat::Xrgb8888));
        assert_eq!(
            PixelFormat::from_wl_code(0x3432_4258),
            Err(ThumbError::UnsupportedFormat(0x3432_4258))
        );
    }

    #[test]
    fn errors_display_and_are_errors() {
        let e: Box<dyn std::error::Error> = Box::new(ThumbError::EmptyCrop);
        assert!(!e.to_string().is_empty());
        assert!(ThumbError::UnsupportedFormat(5).to_string().contains('5'));
    }
}
