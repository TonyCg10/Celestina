//! Icons: Lucide SVGs of `celestina-style/icons` by semantic name, theme
//! icons through `freedesktop-icons` (SVG or PNG), and uploaded pixmaps.
//! Files are rasterised white-on-transparent (Lucide's `stroke="#fff"`) or
//! in their own colours at the device size, and tinted when drawn; the
//! painter caches the textures by (key, device size).
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};

use crate::RgbaImage;
use celestina_wayland::Rect;

/// The largest icon raster, in device px.
pub(crate) const MAX_ICON_PX: u32 = 512;

/// The largest icon file read, in bytes.
const MAX_ICON_FILE: u64 = 4 << 20;

/// Textures kept at once; the least recently drawn is dropped beyond this.
const ICON_CACHE_LIMIT: usize = 256;

/// A pixmap uploaded with [`crate::Painter::upload_pixmap`]. The number is
/// public so a product can build views and tests without a GL context; a
/// number the painter never handed out draws nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixmapId(pub u32);

/// What an icon is drawn from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconKey {
    /// `celestina-style/icons/<name>.svg`.
    Semantic(&'static str),
    /// A freedesktop icon theme name (`.desktop` entries, tray items).
    Theme(String),
    /// A pixmap already uploaded.
    Pixmap(PixmapId),
}

/// Why an icon file could not be rasterised.
#[derive(Debug, thiserror::Error)]
pub(crate) enum IconError {
    #[error("cannot read {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path:?} is larger than {MAX_ICON_FILE} bytes")]
    TooLarge { path: PathBuf },
    #[error("{path:?} is neither SVG nor PNG")]
    Unsupported { path: PathBuf },
    #[error("cannot parse {path:?}")]
    Svg {
        path: PathBuf,
        #[source]
        source: usvg::Error,
    },
    #[error("cannot decode {path:?}")]
    Png {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("{path:?} has an empty or oversized raster")]
    Raster { path: PathBuf },
}

/// Where semantic icons are looked for: `CELESTINA_ICONS`, the checkout's
/// `celestina-style/icons`, then `$prefix/share/celestina/icons`.
pub fn default_icon_dirs() -> Vec<PathBuf> {
    icon_dirs_from(
        std::env::var_os("CELESTINA_ICONS"),
        std::env::current_exe().ok(),
    )
}

pub(crate) fn icon_dirs_from(over: Option<OsString>, exe: Option<PathBuf>) -> Vec<PathBuf> {
    crate::style_asset_dirs(over, exe, "icons")
}

/// An icon name that stays inside the directory it is looked up in.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
}

/// The first `<dir>/<name>.svg` that exists.
pub(crate) fn semantic_path(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    dirs.iter()
        .map(|dir| dir.join(format!("{name}.svg")))
        .find(|path| path.is_file())
}

/// The theme's file for `name` at `size` device px.
pub(crate) fn theme_path(theme: Option<&str>, name: &str, size: u32) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    let size = u16::try_from(size).unwrap_or(u16::MAX);
    let lookup = freedesktop_icons::lookup(name).with_size(size);
    match theme {
        Some(theme) => lookup.with_theme(theme).find(),
        None => lookup.find(),
    }
}

/// The square device size an icon is rasterised at for `rect`: its longer
/// side, clamped to `1..=MAX_ICON_PX`.
pub(crate) fn device_size(rect: Rect, dpr: f32) -> u32 {
    let px = (rect.w.max(rect.h) * dpr).round();
    if px.is_finite() && px >= 1.0 {
        (px as u32).min(MAX_ICON_PX)
    } else {
        1
    }
}

/// Rasterises an SVG or PNG file to fit a `size`×`size` square, keeping its
/// aspect ratio, as premultiplied RGBA.
pub(crate) fn rasterise_file(path: &Path, size: u32) -> Result<RgbaImage, IconError> {
    let read = |source| IconError::Read {
        path: path.to_owned(),
        source,
    };
    let len = std::fs::metadata(path).map_err(read)?.len();
    if len > MAX_ICON_FILE {
        return Err(IconError::TooLarge {
            path: path.to_owned(),
        });
    }
    let bytes = std::fs::read(path).map_err(read)?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let raster = || IconError::Raster {
        path: path.to_owned(),
    };
    match extension.as_deref() {
        Some("svg") => {
            let tree =
                usvg::Tree::from_data(&bytes, &usvg::Options::default()).map_err(|source| {
                    IconError::Svg {
                        path: path.to_owned(),
                        source,
                    }
                })?;
            let (w, h) = (tree.size().width(), tree.size().height());
            fit(w, h, size, |pixmap, transform| {
                resvg::render(&tree, transform, &mut pixmap.as_mut());
            })
            .ok_or_else(raster)
        }
        Some("png") => {
            let png = tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| IconError::Png {
                path: path.to_owned(),
                source: Box::new(e),
            })?;
            let (w, h) = (png.width() as f32, png.height() as f32);
            let paint = tiny_skia::PixmapPaint {
                quality: tiny_skia::FilterQuality::Bicubic,
                ..tiny_skia::PixmapPaint::default()
            };
            fit(w, h, size, |pixmap, transform| {
                pixmap.draw_pixmap(0, 0, png.as_ref(), &paint, transform, None);
            })
            .ok_or_else(raster)
        }
        _ => Err(IconError::Unsupported {
            path: path.to_owned(),
        }),
    }
}

/// Renders a `w`×`h` source scaled to fit a `size`×`size` square, keeping
/// its aspect ratio, through `draw(pixmap, scale)`.
fn fit(
    w: f32,
    h: f32,
    size: u32,
    draw: impl FnOnce(&mut tiny_skia::Pixmap, tiny_skia::Transform),
) -> Option<RgbaImage> {
    let longer = w.max(h);
    if !(longer.is_finite() && longer > 0.0) {
        return None;
    }
    let scale = size.min(MAX_ICON_PX) as f32 / longer;
    let out_w = (w * scale).round().max(1.0) as u32;
    let out_h = (h * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(out_w, out_h)?;
    draw(&mut pixmap, tiny_skia::Transform::from_scale(scale, scale));
    Some(RgbaImage {
        width: out_w,
        height: out_h,
        rgba: pixmap.take(),
    })
}

/// What a cached icon is: an uploaded texture `H` and its size in device
/// px, or `None` for one that failed (warned about once, not per frame).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cached<H> {
    pub texture: Option<(H, u32, u32)>,
    tick: u64,
}

/// File icons by (key, device size), bounded by [`ICON_CACHE_LIMIT`].
#[derive(Debug)]
pub(crate) struct IconCache<H> {
    entries: HashMap<(IconKey, u32), Cached<H>>,
    tick: u64,
}

impl<H: Copy> IconCache<H> {
    pub(crate) fn new() -> IconCache<H> {
        IconCache {
            entries: HashMap::new(),
            tick: 0,
        }
    }

    /// The cached icon, marking it recently drawn.
    pub(crate) fn get(&mut self, key: &IconKey, size: u32) -> Option<Cached<H>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.entries.get_mut(&(key.clone(), size))?;
        entry.tick = tick;
        Some(*entry)
    }

    /// Caches an icon; returns the texture it evicted, which the caller
    /// deletes.
    pub(crate) fn insert(
        &mut self,
        key: IconKey,
        size: u32,
        texture: Option<(H, u32, u32)>,
    ) -> Option<H> {
        let mut evicted = None;
        if self.entries.len() >= ICON_CACHE_LIMIT {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, cached)| cached.tick)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest {
                evicted = self
                    .entries
                    .remove(&oldest)
                    .and_then(|cached| cached.texture)
                    .map(|(handle, _, _)| handle);
            }
        }
        self.tick += 1;
        self.entries.insert(
            (key, size),
            Cached {
                texture,
                tick: self.tick,
            },
        );
        evicted
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkout_icons() -> Vec<PathBuf> {
        icon_dirs_from(None, None)
    }

    #[test]
    fn a_semantic_icon_rasterises_white_at_the_device_size() {
        let path = semantic_path(&checkout_icons(), "bell").expect("bell.svg in the checkout");
        let image = rasterise_file(&path, 36).expect("bell rasterises");
        assert_eq!((image.width, image.height), (36, 36));
        assert_eq!(image.rgba.len(), 36 * 36 * 4);
        assert_eq!(&image.rgba[..4], &[0, 0, 0, 0], "the corner is transparent");
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .any(|p| p == [255, 255, 255, 255]),
            "white ink"
        );
    }

    #[test]
    fn a_png_is_scaled_to_fit_the_device_size() {
        let path = checkout_icons()[0].join("haze-noise.png");
        let image = rasterise_file(&path, 32).expect("the noise PNG decodes");
        assert_eq!((image.width, image.height), (32, 32));
    }

    #[test]
    fn unknown_semantic_names_resolve_to_nothing() {
        assert!(semantic_path(&checkout_icons(), "no-such-icon").is_none());
    }

    #[test]
    fn names_that_could_leave_the_icon_directory_are_refused() {
        for bad in [
            "",
            "../bell",
            "a/b",
            "/etc/passwd",
            "bell\0",
            &"x".repeat(256),
        ] {
            assert!(!valid_name(bad), "{bad:?}");
            assert!(semantic_path(&checkout_icons(), bad).is_none());
        }
        assert!(valid_name("org.celestina.Calcita"));
        assert!(valid_name("app-window"));
    }

    #[test]
    fn device_size_is_the_longer_side_clamped() {
        let r = |w, h| Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        };
        assert_eq!(device_size(r(24.0, 16.0), 1.5), 36);
        assert_eq!(device_size(r(0.0, 0.0), 2.0), 1);
        assert_eq!(device_size(r(5000.0, 10.0), 2.0), MAX_ICON_PX);
    }

    #[test]
    fn the_icon_cache_is_bounded_and_drops_the_least_recent() {
        let mut cache = IconCache::<u32>::new();
        let key = |i: u32| IconKey::Theme(format!("app-{i}"));
        for i in 0..ICON_CACHE_LIMIT as u32 {
            assert_eq!(cache.insert(key(i), 32, Some((i, 32, 32))), None);
        }
        assert!(cache.get(&key(0), 32).is_some(), "0 is now the most recent");
        let evicted = cache.insert(key(9999), 32, Some((9999, 32, 32)));
        assert_eq!(evicted, Some(1), "1 was the least recently drawn");
        assert_eq!(cache.len(), ICON_CACHE_LIMIT);
        assert!(cache.get(&key(1), 32).is_none());
        assert!(
            cache.get(&key(0), 48).is_none(),
            "the size is part of the key"
        );
    }
}
