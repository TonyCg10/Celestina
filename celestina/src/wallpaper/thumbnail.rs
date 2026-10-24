//! The wallpaper's blurred thumbnail for the session lock (CEL-2-H-b, design
//! §4.3 and §4.13).
//!
//! Each successful decode yields, on the wallpaper worker, a thumbnail of at
//! most [`MAX_SIDE`] pixels a side: an area-averaging downscale of the opaque
//! RGBA8 picture followed by a three-pass box blur. It is derived by
//! [`derive`] (bounded work, linear in the decoded picture, never on the
//! loop), carried to the loop inside the worker's `Decoded` event, and kept in
//! the [`ThumbnailStore`]: one entry per output name, at most [`MAX_BYTES`]
//! each, dropped when the output's picture, the duty or the output goes.
//!
//! The lock supervisor reads the store at every spawn
//! ([`ThumbnailStore::blocks`]) and copies the first
//! [`celestina_lock_proto::MAX_OUTPUTS`] entries by connector name into the
//! frozen bundle format; the store only hands out `Arc`s under its mutex, so
//! neither side waits on the copy.
//!
//! The blur and the scrim exist so the picture is never legible and the
//! lock's own text keeps its contrast; the thumbnail is the only picture
//! data that ever leaves the wallpaper state.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use celestina_lock_proto::{OutputBlock, MAX_OUTPUTS, SIDE_MAX};

use super::decode::Decoded;

/// The longest side of a thumbnail, in pixels: the bundle format's bound.
pub const MAX_SIDE: u32 = SIDE_MAX as u32;
/// The most bytes one thumbnail may hold: 1.6 MiB (a 640 × 640 RGBA8 picture
/// is 1 638 400 bytes, under it).
pub const MAX_BYTES: usize = 1_677_721;
/// The box blur's radius in thumbnail pixels: a window of `2 * radius + 1`.
pub const BLUR_RADIUS: usize = 6;
/// How many times the box blur runs (each pass is horizontal then vertical).
pub const BLUR_PASSES: usize = 3;

/// A blurred, downscaled, opaque RGBA8 picture of one output's wallpaper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
}

impl Thumbnail {
    /// The width in pixels, 1 to [`MAX_SIDE`].
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// The height in pixels, 1 to [`MAX_SIDE`].
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// `width * height * 4` bytes, row-major, every alpha byte 255.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// The thumbnail of a decoded picture, or `None` when the picture is empty,
/// its buffer does not match its size, or the result would be over the
/// bounds. Bounded: one pass over the source, then
/// `BLUR_PASSES * 2` passes over at most 640 × 640 pixels.
#[must_use]
pub fn derive(source: &Decoded) -> Option<Thumbnail> {
    let (sw, sh) = (
        usize::try_from(source.width).ok()?,
        usize::try_from(source.height).ok()?,
    );
    if sw == 0 || sh == 0 || sw.checked_mul(sh)?.checked_mul(4)? != source.rgba.len() {
        return None;
    }
    let (dw, dh) = fit(sw, sh, usize::try_from(MAX_SIDE).ok()?);
    let mut pixels = downscale(&source.rgba, (sw, sh), (dw, dh));
    box_blur(&mut pixels, (dw, dh), BLUR_RADIUS, BLUR_PASSES);
    if pixels.len() > MAX_BYTES {
        return None;
    }
    Some(Thumbnail {
        width: u16::try_from(dw).ok()?,
        height: u16::try_from(dh).ok()?,
        rgba: pixels,
    })
}

/// `(width, height)` scaled down to fit a square of `max` pixels, keeping the
/// aspect; a picture that already fits is not enlarged.
fn fit(width: usize, height: usize, max: usize) -> (usize, usize) {
    let longest = width.max(height);
    if longest <= max {
        return (width, height);
    }
    let scaled = |side: usize| (side * max + longest / 2) / longest;
    (scaled(width).clamp(1, max), scaled(height).clamp(1, max))
}

/// Area-averaging downscale of an opaque RGBA8 picture: every destination
/// pixel is the rounded mean of the source pixels its cell covers (a cell is
/// never empty).
fn downscale(source: &[u8], (sw, sh): (usize, usize), (dw, dh): (usize, usize)) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        let y0 = dy * sh / dh;
        let y1 = ((dy + 1) * sh / dh).max(y0 + 1).min(sh);
        for dx in 0..dw {
            let x0 = dx * sw / dw;
            let x1 = ((dx + 1) * sw / dw).max(x0 + 1).min(sw);
            let mut sum = [0u64; 4];
            for y in y0..y1 {
                let row = &source[(y * sw + x0) * 4..(y * sw + x1) * 4];
                for pixel in row.as_chunks::<4>().0.iter() {
                    for (total, &channel) in sum.iter_mut().zip(pixel) {
                        *total += u64::from(channel);
                    }
                }
            }
            let count = ((y1 - y0) * (x1 - x0)) as u64;
            let cell = &mut out[(dy * dw + dx) * 4..(dy * dw + dx) * 4 + 4];
            for (byte, total) in cell.iter_mut().zip(sum) {
                *byte = u8::try_from((total + count / 2) / count).unwrap_or(u8::MAX);
            }
            // The picture is opaque by contract; the bundle's block is too.
            cell[3] = u8::MAX;
        }
    }
    out
}

/// `passes` rounds of a box blur of `radius` over the picture, each round a
/// horizontal then a vertical window of `2 * radius + 1` pixels with the
/// edges extended by their nearest pixel. Integer arithmetic, rounded to
/// nearest; opaque stays opaque.
pub fn box_blur(pixels: &mut [u8], (width, height): (usize, usize), radius: usize, passes: usize) {
    if width == 0 || height == 0 || pixels.len() != width * height * 4 || radius == 0 {
        return;
    }
    let mut scratch = vec![0u8; pixels.len()];
    for _ in 0..passes {
        // Horizontal: lines are rows, neighbours 4 bytes apart.
        blur_lines(pixels, &mut scratch, (width, height, 4, width * 4), radius);
        // Vertical: lines are columns, neighbours one row apart.
        blur_lines(&scratch, pixels, (height, width, width * 4, 4), radius);
    }
}

/// Blurs `lines` lines of `len` pixels from `source` into `dest`: pixel `i`
/// of line `l` lives at byte `l * line_step + i * step`, four channels.
fn blur_lines(
    source: &[u8],
    dest: &mut [u8],
    (len, lines, step, line_step): (usize, usize, usize, usize),
    radius: usize,
) {
    let window = 2 * radius + 1;
    let at = |line: usize, index: usize| line * line_step + index * step;
    for line in 0..lines {
        for channel in 0..4 {
            let sample = |index: isize| {
                let clamped = index.clamp(0, len as isize - 1) as usize;
                u32::from(source[at(line, clamped) + channel])
            };
            let r = radius as isize;
            let mut sum: u32 = (-r..=r).map(sample).sum();
            for index in 0..len {
                dest[at(line, index) + channel] =
                    u8::try_from((sum + window as u32 / 2) / window as u32).unwrap_or(u8::MAX);
                let i = index as isize;
                sum = sum + sample(i + r + 1) - sample(i - r);
            }
        }
    }
}

/// The thumbnails the lock may be given, by output name. A clone is another
/// handle on the same store: the loop writes, the lock supervisor reads.
#[derive(Clone, Debug, Default)]
pub struct ThumbnailStore {
    inner: Arc<Mutex<BTreeMap<String, Arc<Thumbnail>>>>,
}

impl ThumbnailStore {
    fn map(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Arc<Thumbnail>>> {
        // A panic elsewhere while holding the lock leaves a map whose entries
        // are each whole; reading it is safe.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `thumbnail` for `output`, replacing the older one; refused (and
    /// the older one dropped) when it is over [`MAX_BYTES`].
    pub fn set(&self, output: &str, thumbnail: Thumbnail) {
        let mut map = self.map();
        if thumbnail.rgba.len() > MAX_BYTES {
            map.remove(output);
            return;
        }
        map.insert(output.to_owned(), Arc::new(thumbnail));
    }

    /// Drops `output`'s thumbnail.
    pub fn remove(&self, output: &str) {
        self.map().remove(output);
    }

    /// Drops every thumbnail.
    pub fn clear(&self) {
        self.map().clear();
    }

    /// How many thumbnails are kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map().len()
    }

    /// Whether no thumbnail is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
    }

    /// The bundle's blocks: the first [`MAX_OUTPUTS`] thumbnails by connector
    /// name (byte order), each copied into an [`OutputBlock`]. A name or a
    /// size the format refuses is skipped. The outputs without a thumbnail
    /// take no slot; the rest, beyond eight, get no block (F40).
    #[must_use]
    pub fn blocks(&self) -> Vec<OutputBlock> {
        let picked: Vec<(String, Arc<Thumbnail>)> = self
            .map()
            .iter()
            .map(|(name, thumbnail)| (name.clone(), Arc::clone(thumbnail)))
            .collect();
        picked
            .into_iter()
            .filter_map(|(name, thumbnail)| {
                OutputBlock::new(
                    name,
                    thumbnail.width,
                    thumbnail.height,
                    thumbnail.rgba.clone(),
                )
                .ok()
            })
            .take(MAX_OUTPUTS)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Decoded {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let [r, g, b] = pixel(x, y);
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        Decoded {
            rgba,
            width,
            height,
            source_px: (width, height),
        }
    }

    #[test]
    fn a_picture_that_fits_is_blurred_but_not_resized_and_stays_opaque() {
        let source = picture(40, 30, |x, y| [(x * 6) as u8, (y * 8) as u8, 9]);
        let thumb = derive(&source).expect("a thumbnail");
        assert_eq!((thumb.width(), thumb.height()), (40, 30));
        assert!(thumb.rgba().as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        assert_ne!(thumb.rgba(), source.rgba.as_slice(), "it is blurred");
    }

    #[test]
    fn a_mismatched_or_empty_picture_has_no_thumbnail() {
        let mut source = picture(4, 4, |_, _| [1, 2, 3]);
        source.rgba.pop();
        assert!(derive(&source).is_none());
        let empty = Decoded {
            rgba: Vec::new(),
            width: 0,
            height: 0,
            source_px: (0, 0),
        };
        assert!(derive(&empty).is_none());
    }

    #[test]
    fn a_uniform_picture_stays_that_colour_through_scale_and_blur() {
        let source = picture(1920, 1080, |_, _| [200, 100, 50]);
        let thumb = derive(&source).expect("a thumbnail");
        assert_eq!((thumb.width(), thumb.height()), (640, 360));
        assert!(thumb
            .rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [200, 100, 50, 255]));
    }

    #[test]
    fn the_store_sorts_by_name_skips_empty_slots_and_stops_at_8() {
        let store = ThumbnailStore::default();
        let thumb = derive(&picture(8, 8, |_, _| [5, 5, 5])).expect("a thumbnail");
        for name in [
            "HDMI-A-3", "DP-1", "eDP-1", "DP-2", "A", "B", "C", "D", "E", "F",
        ] {
            store.set(name, thumb.clone());
        }
        let names: Vec<String> = store.blocks().iter().map(|b| b.name().to_owned()).collect();
        assert_eq!(names, ["A", "B", "C", "D", "DP-1", "DP-2", "E", "F"]);
        store.remove("A");
        assert_eq!(store.blocks().len(), 8);
        store.clear();
        assert!(store.is_empty() && store.blocks().is_empty());
    }
}
