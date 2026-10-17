//! Thumbnails of the image entries (CEL-2-G design §6).
//!
//! The worker makes one when it records a PNG, with the wallpaper unit's
//! bounded decoder ([`crate::wallpaper::decode::decode_png_thumbnail`]): the
//! header's dimensions and the decode's memory are checked before a pixel is
//! decoded, against a budget tighter than a wallpaper's (at most 16 MP and
//! 64 MiB decoded), so a hostile PNG costs bounded work. A PNG that does not
//! decode within it is not recorded at all. The thumbnail is at most [`THUMB_SIDE`] pixels a
//! side, a few kilobytes, and is what the surfaces draw; the entry's own
//! bytes never leave the worker.
//!
//! [`Thumbs`] remembers each image entry's thumbnail by where the entry's
//! bytes live, so publishing the rows costs no decode. The key is the
//! address of the entry's PNG buffer: an entry owns its buffer for as long as
//! it is in the history (the history moves entries, never their heap data),
//! and [`Thumbs::keep_only`] forgets the thumbnails of entries that are gone
//! after every change, before any address can be reused. An entry the cache
//! does not know simply has no thumbnail; nothing depends on it.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use celestina_shell::clipboard::{Content, History};

use crate::wallpaper::decode::{decode_png_thumbnail, DecodeError};

/// The longest side of a thumbnail, in pixels.
pub const THUMB_SIDE: u32 = 64;

/// An image entry's thumbnail: opaque straight RGBA, row-major, at most
/// [`THUMB_SIDE`] a side. Cloning shares the pixels.
#[derive(Clone, PartialEq, Eq)]
pub struct Thumb(Arc<Pixels>);

#[derive(PartialEq, Eq)]
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Thumb {
    /// A thumbnail of the PNG in `bytes`, made with the bounded decoder.
    ///
    /// # Errors
    ///
    /// The bound or check the bytes failed. The error holds no pixel.
    pub fn from_png(bytes: &[u8]) -> Result<Self, DecodeError> {
        let decoded = decode_png_thumbnail(bytes, THUMB_SIDE)?;
        Ok(Self(Arc::new(Pixels {
            width: decoded.width,
            height: decoded.height,
            rgba: decoded.rgba,
        })))
    }

    /// A thumbnail from pixels, when they are within the bounds: each side
    /// from 1 to [`THUMB_SIDE`] and exactly `width * height * 4` bytes.
    #[must_use]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let sides = 1..=THUMB_SIDE;
        let fits = sides.contains(&width) && sides.contains(&height);
        (fits && rgba.len() as u64 == u64::from(width) * u64::from(height) * 4).then(|| {
            Self(Arc::new(Pixels {
                width,
                height,
                rgba,
            }))
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.0.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.0.height
    }

    /// The straight RGBA pixels.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.0.rgba
    }

    /// The pixels as ARGB32 in network byte order, which is what the painter
    /// uploads.
    #[must_use]
    pub fn argb(&self) -> Vec<u8> {
        self.0
            .rgba
            .chunks_exact(4)
            .flat_map(|px| [px[3], px[0], px[1], px[2]])
            .collect()
    }

    /// A stable identity for this thumbnail's pixels while it lives.
    #[must_use]
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

impl fmt::Debug for Thumb {
    /// Dimensions only: never a pixel.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Thumb")
            .field("width", &self.0.width)
            .field("height", &self.0.height)
            .finish()
    }
}

/// The thumbnails of the image entries a history holds.
#[derive(Debug, Default)]
pub struct Thumbs {
    by_buffer: HashMap<usize, Thumb>,
}

/// Where the PNG bytes of `content` live, when it is an image.
fn buffer_of(content: &Content) -> Option<usize> {
    content.as_png().map(|bytes| bytes.as_ptr() as usize)
}

impl Thumbs {
    /// Remembers `thumb` for the image entry `content` (a no-op for text).
    pub fn insert(&mut self, content: &Content, thumb: Thumb) {
        if let Some(key) = buffer_of(content) {
            self.by_buffer.insert(key, thumb);
        }
    }

    /// The thumbnail of the image entry `content`, if one is remembered.
    #[must_use]
    pub fn get(&self, content: &Content) -> Option<Thumb> {
        buffer_of(content).and_then(|key| self.by_buffer.get(&key).cloned())
    }

    /// Forgets every thumbnail whose entry `history` no longer holds.
    pub fn keep_only(&mut self, history: &History) {
        let live: Vec<usize> = history
            .entries()
            .iter()
            .filter_map(|entry| buffer_of(entry.content()))
            .collect();
        self.by_buffer.retain(|key, _| live.contains(key));
    }

    /// How many thumbnails are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_buffer.len()
    }

    /// Whether none is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, red: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(w, h, image::Rgba([red, 0, 0, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encode a png");
        out.into_inner()
    }

    #[test]
    fn a_thumbnail_is_bounded_and_prints_dimensions_only() {
        let thumb = Thumb::from_png(&png(640, 480, 200)).expect("thumb");
        assert_eq!((thumb.width(), thumb.height()), (64, 48));
        assert_eq!(thumb.rgba().len(), 64 * 48 * 4);
        assert_eq!(&thumb.rgba()[..4], &[200, 0, 0, 255]);
        assert_eq!(&thumb.argb()[..4], &[255, 200, 0, 0]);
        let printed = format!("{thumb:?}");
        assert!(
            printed.contains("64") && printed.contains("48"),
            "{printed}"
        );
        assert!(!printed.contains("200"), "{printed}");
    }

    #[test]
    fn pixels_outside_the_bounds_are_not_a_thumbnail() {
        assert!(Thumb::new(0, 1, vec![]).is_none());
        assert!(Thumb::new(65, 1, vec![0; 65 * 4]).is_none());
        assert!(Thumb::new(2, 2, vec![0; 15]).is_none());
        assert!(Thumb::new(2, 2, vec![0; 16]).is_some());
    }

    #[test]
    fn bytes_that_are_not_a_decodable_png_make_no_thumbnail() {
        let mut signature_only = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        signature_only.push(7);
        assert!(Thumb::from_png(&signature_only).is_err());
        assert!(Thumb::from_png(&[]).is_err());
    }

    #[test]
    fn the_cache_follows_the_entries_it_was_made_for() {
        let mut history = History::new();
        let first = Content::png(png(4, 4, 1)).expect("png");
        let second = Content::png(png(4, 4, 2)).expect("png");
        let mut thumbs = Thumbs::default();
        for content in [&first, &second] {
            history.record(content.clone(), 10);
            let entry = history.entries()[0].content();
            thumbs.insert(
                entry,
                Thumb::from_png(entry.as_png().expect("png")).expect("thumb"),
            );
        }
        assert_eq!(thumbs.len(), 2);
        // Moving an entry to the front keeps its buffer and its thumbnail.
        let before = thumbs.get(history.entries()[1].content());
        assert!(before.is_some());
        history.select(1, 11);
        thumbs.keep_only(&history);
        assert_eq!(thumbs.len(), 2);
        assert_eq!(thumbs.get(history.entries()[0].content()), before);
        // Removing an entry forgets its thumbnail.
        assert!(history.remove(0));
        thumbs.keep_only(&history);
        assert_eq!(thumbs.len(), 1);
        history.clear();
        thumbs.keep_only(&history);
        assert!(thumbs.is_empty());
    }

    #[test]
    fn text_entries_have_no_thumbnail() {
        let text = Content::text("plain".to_owned()).expect("text");
        let mut thumbs = Thumbs::default();
        thumbs.insert(&text, Thumb::new(1, 1, vec![0; 4]).expect("thumb"));
        assert!(thumbs.is_empty());
        assert!(thumbs.get(&text).is_none());
    }
}
