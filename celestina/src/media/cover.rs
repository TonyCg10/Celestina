//! A player's cover art, from its `mpris:artUrl` to a square RGBA buffer
//! (CEL-2-D plan, decision 4).
//!
//! The URL and the file it names are hostile input: any process on the
//! session bus can claim an MPRIS name and point it anywhere. Every bound is
//! checked before the work it bounds, in this order:
//!
//! 1. the URL: only `file://` plus an absolute path, the one rule of
//!    [`celestina_shell::media::cover_url_path`] (no fetcher: `http(s)` and
//!    `data:` are refused, as are relative names, hosts, NUL and `..`);
//! 2. the open: the wallpaper decoder's own guard (`stat`, then an
//!    `O_NONBLOCK | O_NOCTTY` open and `fstat`), so a FIFO named `x.png` is
//!    refused without blocking and only a non-empty regular file is opened;
//! 3. the size: at most [`celestina_core::image::MAX_ARTWORK_BYTES`]
//!    (8 MiB);
//! 4. the signature: PNG, JPEG or WebP by
//!    [`celestina_core::image::is_supported_image_header`];
//! 5. the header's dimensions, read on that same descriptor without
//!    decoding: at most [`MAX_COVER_SIDE`] a side and [`MAX_COVER_PIXELS`]
//!    in all, far below the wallpaper's limits, since a cover is drawn at
//!    [`COVER_SIZE`];
//! 6. the decode: the one bounded decoder,
//!    [`crate::wallpaper::decode::decode`], in [`Mode::Fill`] at
//!    [`COVER_SIZE`] square, so a non-square cover is cropped to its centre
//!    and every extension, memory and lossless-WebP rule is that decoder's.
//!
//! Steps 2 to 5 run on one descriptor, which is closed before the decoder
//! opens the path a second time and repeats its own checks. The cover
//! bounds above therefore hold only for the file seen the first time: a
//! player that swaps the file between the two opens gets the second one
//! decoded under the wallpaper decoder's limits instead (its file-size,
//! side, area and memory caps), not the cover's. That is the residual: one
//! decode bounded like a wallpaper, per allowed decode (the cover source
//! allows one per player per two seconds, and only for the shown player).
//! Only the decoded buffer leaves this module: the source bytes are never
//! retained.
//!
//! [`CoverCache`] remembers the outcome of the last few `(bus name, URL)`
//! pairs so a repeated `PropertiesChanged` does not decode the same file
//! again. It does no IO itself: the caller (the media worker, never the
//! event loop) passes the loader in.

use std::collections::VecDeque;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use celestina_core::image::{is_artwork_size, is_supported_image_header, IMAGE_HEADER_BYTES};
use celestina_shell::media::{clamp_text, cover_url_path};
use celestina_shell::wallpaper::Mode;

use crate::wallpaper::decode::{decode, open_regular, DecodeError};

/// The side of every decoded cover, in pixels.
pub const COVER_SIZE: u32 = 128;
/// The widest or tallest cover header accepted before decoding.
pub const MAX_COVER_SIDE: u32 = 4_096;
/// The most pixels a cover header may announce before decoding (16 MP).
pub const MAX_COVER_PIXELS: u64 = 16_000_000;
/// The most `(bus name, URL)` outcomes a [`CoverCache`] remembers.
pub const MAX_CACHED_COVERS: usize = 8;

/// A decoded cover: opaque RGBA8, row-major, [`COVER_SIZE`] square.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes; every alpha byte is 255.
    pub rgba: Vec<u8>,
}

/// Why a cover could not be loaded. Every case means "no cover".
#[derive(Debug, thiserror::Error)]
pub enum CoverError {
    /// The URL does not name a local absolute file (`url` is cut to
    /// [`celestina_shell::media::MAX_TEXT_BYTES`]).
    #[error("cover art is not a local file: {url:?}")]
    NotLocal { url: String },
    /// The path is missing, not a non-empty regular file, or cannot be
    /// opened (the source names the path).
    #[error("cannot open the cover")]
    Unreadable {
        #[source]
        source: DecodeError,
    },
    /// The file is larger than
    /// [`celestina_core::image::MAX_ARTWORK_BYTES`].
    #[error("cover of {bytes} bytes is over the artwork cap: {path:?}")]
    TooLarge { path: PathBuf, bytes: u64 },
    /// The file's first bytes could not be read.
    #[error("cannot read the cover {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The file does not start with a PNG, JPEG or WebP signature.
    #[error("cover is not a PNG, JPEG or WebP image: {path:?}")]
    NotAnImage { path: PathBuf },
    /// The header's dimensions could not be read.
    #[error("cannot read the dimensions of the cover {path:?}")]
    Header {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    /// The header announces more than [`MAX_COVER_SIDE`] a side or
    /// [`MAX_COVER_PIXELS`] in all.
    #[error("cover of {width}x{height} pixels is over the cover limits: {path:?}")]
    Oversized {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    /// The bounded decoder refused or failed on the file (the source
    /// names the path).
    #[error("cannot decode the cover")]
    Decode {
        #[source]
        source: DecodeError,
    },
}

impl CoverError {
    /// The error and its sources as one line for the log, every control
    /// character escaped: the path is the player's choice, and a newline or
    /// a terminal escape in it must not forge or recolour log lines. The
    /// decoder's own messages print the path verbatim, so the whole chain
    /// is escaped here rather than each message.
    #[must_use]
    pub fn describe(&self) -> String {
        escape_controls(&crate::error_chain(self))
    }
}

/// `text` with every control character written as its escape.
fn escape_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

/// Loads the cover `art_url` names into a [`COVER_SIZE`] square buffer.
/// Blocking IO: call it on the media worker, never on the event loop.
///
/// # Errors
///
/// A [`CoverError`] naming the first bound or check that failed; none of
/// them blocks on a FIFO or decodes past an out-of-bounds header.
pub fn load(art_url: &str) -> Result<Cover, CoverError> {
    let path = cover_url_path(art_url).ok_or_else(|| CoverError::NotLocal {
        url: clamp_text(art_url),
    })?;
    precheck(&path)?;
    let decoded = decode(&path, (COVER_SIZE, COVER_SIZE), Mode::Fill)
        .map_err(|source| CoverError::Decode { source })?;
    Ok(Cover {
        width: decoded.width,
        height: decoded.height,
        rgba: decoded.rgba,
    })
}

/// The open, size, signature and dimension checks, on one descriptor that
/// is closed before the decoder opens the file itself.
fn precheck(path: &Path) -> Result<(), CoverError> {
    let (file, fingerprint) =
        open_regular(path).map_err(|source| CoverError::Unreadable { source })?;
    if !is_artwork_size(i64::try_from(fingerprint.bytes).unwrap_or(i64::MAX)) {
        return Err(CoverError::TooLarge {
            path: path.to_owned(),
            bytes: fingerprint.bytes,
        });
    }
    let mut head = Vec::with_capacity(IMAGE_HEADER_BYTES);
    (&file)
        .take(IMAGE_HEADER_BYTES as u64)
        .read_to_end(&mut head)
        .map_err(|source| CoverError::Read {
            path: path.to_owned(),
            source,
        })?;
    if !is_supported_image_header(&head) {
        return Err(CoverError::NotAnImage {
            path: path.to_owned(),
        });
    }
    let header = |source| CoverError::Header {
        path: path.to_owned(),
        source,
    };
    (&file)
        .seek(SeekFrom::Start(0))
        .map_err(|source| CoverError::Read {
            path: path.to_owned(),
            source,
        })?;
    let (width, height) = image::ImageReader::new(BufReader::new(&file))
        .with_guessed_format()
        .map_err(|source| CoverError::Read {
            path: path.to_owned(),
            source,
        })?
        .into_dimensions()
        .map_err(header)?;
    if !is_cover_sized(width, height) {
        return Err(CoverError::Oversized {
            path: path.to_owned(),
            width,
            height,
        });
    }
    Ok(())
}

/// Whether a header of `width` by `height` is within the cover limits.
fn is_cover_sized(width: u32, height: u32) -> bool {
    width <= MAX_COVER_SIDE
        && height <= MAX_COVER_SIDE
        && u64::from(width) * u64::from(height) <= MAX_COVER_PIXELS
}

/// Which cover a player names: its bus name, cut to
/// [`celestina_shell::media::MAX_TEXT_BYTES`], and its whole `mpris:artUrl`.
/// The url is never cut: a url cut short names another file. Its bound is
/// the worker's ([`super::mpris::MAX_ART_URL_BYTES`]), which drops a longer
/// one whole before it reaches a key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CoverKey {
    pub bus_name: String,
    pub art_url: String,
}

impl CoverKey {
    #[must_use]
    pub fn new(bus_name: &str, art_url: &str) -> Self {
        Self {
            bus_name: clamp_text(bus_name),
            art_url: art_url.to_owned(),
        }
    }
}

/// The outcome of the last [`MAX_CACHED_COVERS`] keys, least recently used
/// first out. A failure is remembered as "no cover" so a broken file is not
/// retried on every `PropertiesChanged`; a new URL is a new key.
#[derive(Debug, Default)]
pub struct CoverCache {
    /// Most recently used last.
    entries: VecDeque<(CoverKey, Option<Arc<Cover>>)>,
}

impl CoverCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The remembered outcome of `key`, or `load(&key.art_url)` when there
    /// is none: `Ok(None)` is a remembered failure, `Err` a fresh one (now
    /// remembered too, returned once so the caller can log it).
    ///
    /// # Errors
    ///
    /// The [`CoverError`] `load` returned, the first time only.
    pub fn get_or_load<F>(
        &mut self,
        key: &CoverKey,
        load: F,
    ) -> Result<Option<Arc<Cover>>, CoverError>
    where
        F: FnOnce(&str) -> Result<Cover, CoverError>,
    {
        if let Some(outcome) = self.cached(key) {
            return Ok(outcome);
        }
        let loaded = load(&key.art_url).map(Arc::new);
        self.remember(key.clone(), loaded.as_ref().ok().cloned());
        loaded.map(Some)
    }

    /// The remembered outcome of `key`, without loading anything: `None`
    /// when it was never loaded (or was evicted).
    pub fn cached(&mut self, key: &CoverKey) -> Option<Option<Arc<Cover>>> {
        let index = self.entries.iter().position(|(known, _)| known == key)?;
        let entry = self.entries.remove(index)?;
        let outcome = entry.1.clone();
        self.entries.push_back(entry);
        Some(outcome)
    }

    /// Drops every outcome of `bus_name`: the player left the bus.
    pub fn forget_bus(&mut self, bus_name: &str) {
        self.entries.retain(|(key, _)| key.bus_name != bus_name);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn remember(&mut self, key: CoverKey, outcome: Option<Arc<Cover>>) {
        while self.entries.len() >= MAX_CACHED_COVERS {
            self.entries.pop_front();
        }
        self.entries.push_back((key, outcome));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn cover(shade: u8) -> Cover {
        Cover {
            width: 1,
            height: 1,
            rgba: vec![shade, shade, shade, 255],
        }
    }

    fn key(bus: &str, n: usize) -> CoverKey {
        CoverKey::new(bus, &format!("file:///covers/{n}.png"))
    }

    #[test]
    fn a_repeated_key_is_not_loaded_again() {
        let mut cache = CoverCache::new();
        let calls = Cell::new(0);
        let loader = |_: &str| {
            calls.set(calls.get() + 1);
            Ok(cover(7))
        };
        let first = cache.get_or_load(&key("a", 1), loader).expect("load");
        let again = cache.get_or_load(&key("a", 1), loader).expect("hit");
        assert_eq!(calls.get(), 1);
        assert_eq!(first, again);
        assert_eq!(again.map(|c| c.rgba[0]), Some(7));
    }

    #[test]
    fn the_loader_gets_the_url_of_the_key() {
        let mut cache = CoverCache::new();
        let seen = RefCell::new(String::new());
        let _ = cache.get_or_load(&key("a", 3), |url| {
            *seen.borrow_mut() = url.to_owned();
            Ok(cover(1))
        });
        assert_eq!(*seen.borrow(), "file:///covers/3.png");
    }

    #[test]
    fn a_failure_is_reported_once_and_then_remembered_as_no_cover() {
        let mut cache = CoverCache::new();
        let calls = Cell::new(0);
        let loader = |url: &str| {
            calls.set(calls.get() + 1);
            Err(CoverError::NotLocal {
                url: url.to_owned(),
            })
        };
        assert!(cache.get_or_load(&key("a", 1), loader).is_err());
        assert!(matches!(cache.get_or_load(&key("a", 1), loader), Ok(None)));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn the_same_url_from_two_players_is_two_keys() {
        let mut cache = CoverCache::new();
        let calls = Cell::new(0);
        let loader = |_: &str| {
            calls.set(calls.get() + 1);
            Ok(cover(1))
        };
        let _ = cache.get_or_load(&key("a", 1), loader);
        let _ = cache.get_or_load(&key("b", 1), loader);
        assert_eq!(calls.get(), 2);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn at_most_eight_keys_are_kept_and_the_least_recent_goes_first() {
        let mut cache = CoverCache::new();
        let calls = Cell::new(0);
        let loader = |_: &str| {
            calls.set(calls.get() + 1);
            Ok(cover(1))
        };
        for n in 0..MAX_CACHED_COVERS {
            let _ = cache.get_or_load(&key("a", n), loader);
        }
        // Touch 0 so 1 is now the least recent.
        let _ = cache.get_or_load(&key("a", 0), loader);
        let _ = cache.get_or_load(&key("a", 100), loader);
        assert_eq!(cache.len(), MAX_CACHED_COVERS);
        assert_eq!(calls.get(), MAX_CACHED_COVERS + 1);

        let _ = cache.get_or_load(&key("a", 0), loader);
        assert_eq!(calls.get(), MAX_CACHED_COVERS + 1, "0 was kept");
        let _ = cache.get_or_load(&key("a", 1), loader);
        assert_eq!(calls.get(), MAX_CACHED_COVERS + 2, "1 was evicted");
        assert_eq!(cache.len(), MAX_CACHED_COVERS);
    }

    #[test]
    fn a_player_leaving_drops_its_covers_only() {
        let mut cache = CoverCache::new();
        let loader = |_: &str| Ok(cover(1));
        let _ = cache.get_or_load(&key("a", 1), loader);
        let _ = cache.get_or_load(&key("a", 2), loader);
        let _ = cache.get_or_load(&key("b", 1), loader);
        cache.forget_bus("a");
        assert_eq!(cache.len(), 1);
        cache.forget_bus("b");
        assert!(cache.is_empty());
    }

    #[test]
    fn a_key_bounds_its_bus_name_and_keeps_its_url_whole() {
        let long = "\u{e9}".repeat(1_000);
        let url = format!("file:///{}", "a".repeat(1_000));
        let key = CoverKey::new(&long, &url);
        assert!(key.bus_name.len() <= celestina_shell::media::MAX_TEXT_BYTES);
        assert!(key.bus_name.chars().all(|c| c == '\u{e9}'));
        assert_eq!(key.art_url, url, "a url cut short names another file");
    }

    #[test]
    fn two_urls_sharing_their_first_256_bytes_are_two_entries() {
        let stem = format!("file:///{}", "d/".repeat(200));
        assert!(stem.len() > celestina_shell::media::MAX_TEXT_BYTES);
        let (one, two) = (format!("{stem}one.png"), format!("{stem}two.png"));
        let mut cache = CoverCache::new();
        let seen = RefCell::new(Vec::new());
        let loader = |url: &str| {
            seen.borrow_mut().push(url.to_owned());
            Ok(cover(u8::try_from(seen.borrow().len()).unwrap_or(0)))
        };
        let first = cache
            .get_or_load(&CoverKey::new("a", &one), loader)
            .expect("one");
        let second = cache
            .get_or_load(&CoverKey::new("a", &two), loader)
            .expect("two");
        assert_eq!(*seen.borrow(), [one, two], "each loads its own url");
        assert_eq!(cache.len(), 2);
        assert_ne!(first, second);
    }

    #[test]
    fn cached_answers_only_what_was_loaded() {
        let mut cache = CoverCache::new();
        assert!(cache.cached(&key("a", 1)).is_none());
        let _ = cache.get_or_load(&key("a", 1), |_| Ok(cover(3)));
        assert_eq!(
            cache.cached(&key("a", 1)).map(|c| c.map(|c| c.rgba[0])),
            Some(Some(3))
        );
        assert!(cache.cached(&key("a", 2)).is_none());
    }

    #[test]
    fn a_description_escapes_control_characters_in_the_path() {
        let err = load("file:///tmp/no\nsuch\u{1b}[31m.png").expect_err("missing");
        let text = err.describe();
        assert!(!text.chars().any(char::is_control), "{text:?}");
        assert!(text.contains("\\n") && text.contains("\\u{1b}"), "{text:?}");
    }

    #[test]
    fn a_remote_url_is_refused_before_any_io() {
        assert!(matches!(
            load("https://example.com/a.png"),
            Err(CoverError::NotLocal { .. })
        ));
    }
}
