//! Icons: Lucide SVGs of `celestina-style/icons` by semantic name, theme
//! icons through `freedesktop-icons` (SVG or PNG), and uploaded pixmaps.
//! Files are rasterised white-on-transparent (Lucide's `stroke="#fff"`) or
//! in their own colours at the device size, and tinted when drawn; the
//! painter caches the textures by (key, device size).
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use celestina_core::atomic_file::ReadError;
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
    /// An absolute icon file named by a `.desktop` entry; never built from a
    /// string a remote peer sent.
    File(PathBuf),
    /// A pixmap already uploaded.
    Pixmap(PixmapId),
}

impl IconKey {
    /// The key of a `.desktop` entry's `Icon=` value: an absolute path is a
    /// [`IconKey::File`], any other value a theme name; empty is none. Only
    /// for strings read from a `.desktop` file.
    #[must_use]
    pub fn desktop_icon(value: &str) -> Option<IconKey> {
        if value.is_empty() {
            None
        } else if Path::new(value).is_absolute() {
            Some(IconKey::File(PathBuf::from(value)))
        } else {
            Some(IconKey::Theme(value.to_owned()))
        }
    }
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
    #[error("{path:?} is not a regular file")]
    NotRegular { path: PathBuf },
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

/// Where an icon theme was chosen from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeSource {
    Gsettings,
    Gtk3Settings,
    Gtk4Settings,
    Adwaita,
}

/// The value of `gtk-icon-theme-name` in a GTK `settings.ini` text.
pub(crate) fn settings_icon_theme(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with(['#', ';']) {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() != "gtk-icon-theme-name" {
            return None;
        }
        let value = value.trim().trim_matches(['"', '\'']).trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Whether some base directory holds `<name>/index.theme`.
pub(crate) fn theme_installed(bases: &[PathBuf], name: &str) -> bool {
    valid_name(name)
        && bases
            .iter()
            .any(|base| base.join(name).join("index.theme").is_file())
}

/// The icon theme to look names up in: the GNOME setting, the GTK 3 then
/// GTK 4 settings file, then Adwaita, each only if installed.
///
/// `gsettings` answers the schema default (`Adwaita`) when the user never set
/// a theme, so it then outranks `settings.ini`, as GTK itself does on Wayland.
pub(crate) fn choose_theme(
    gsettings: Option<&str>,
    gtk3_ini: Option<&str>,
    gtk4_ini: Option<&str>,
    bases: &[PathBuf],
) -> Option<(String, ThemeSource)> {
    let named = [
        (gsettings.map(str::to_owned), ThemeSource::Gsettings),
        (
            gtk3_ini.and_then(settings_icon_theme),
            ThemeSource::Gtk3Settings,
        ),
        (
            gtk4_ini.and_then(settings_icon_theme),
            ThemeSource::Gtk4Settings,
        ),
        (Some("Adwaita".to_owned()), ThemeSource::Adwaita),
    ];
    named.into_iter().find_map(|(name, source)| {
        let name = name?;
        theme_installed(bases, &name).then_some((name, source))
    })
}

/// The icon base directories of the freedesktop spec, from the environment.
fn icon_base_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let var = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());
    let mut dirs = Vec::new();
    if let Some(home) = &home {
        dirs.push(home.join(".icons"));
    }
    match (var("XDG_DATA_HOME"), &home) {
        (Some(data), _) => dirs.push(PathBuf::from(data).join("icons")),
        (None, Some(home)) => dirs.push(home.join(".local/share/icons")),
        (None, None) => {}
    }
    let data_dirs = var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        std::env::split_paths(&data_dirs)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.join("icons")),
    );
    dirs
}

/// `gtk-3.0` or `gtk-4.0` `settings.ini` under the user's config directory.
fn read_gtk_settings(version: &str) -> Option<String> {
    let config = match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(config) => PathBuf::from(config),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    std::fs::read_to_string(config.join(version).join("settings.ini")).ok()
}

/// `gsettings get org.gnome.desktop.interface icon-theme`, if it answers.
fn gsettings_icon_theme() -> Option<String> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let name = text.trim().trim_matches('\'');
    (!name.is_empty()).then(|| name.to_owned())
}

/// The user's icon theme directory name, or `None` for hicolor alone. Runs
/// `gsettings`, so call it once.
pub(crate) fn default_theme() -> Option<String> {
    let gtk3 = read_gtk_settings("gtk-3.0");
    let gtk4 = read_gtk_settings("gtk-4.0");
    let chosen = choose_theme(
        gsettings_icon_theme().as_deref(),
        gtk3.as_deref(),
        gtk4.as_deref(),
        &icon_base_dirs(),
    );
    match chosen {
        Some((name, source)) => {
            tracing::info!(theme = %name, ?source, "icon theme");
            Some(name)
        }
        None => {
            tracing::info!("icon theme: none installed, hicolor only");
            None
        }
    }
}

/// `path` itself when it is absolute, a PNG or SVG by extension (any case)
/// and a regular file.
pub(crate) fn file_path(path: &Path) -> Option<PathBuf> {
    let icon_file = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("svg"));
    (path.is_absolute() && icon_file && path.is_file()).then(|| path.to_owned())
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

/// The largest icon file a peer may name by path, in bytes.
const MAX_PEER_ICON_FILE: u64 = 1 << 20;

/// The directories an icon a peer named by absolute path may live in:
/// `$XDG_DATA_DIRS` (default `/usr/local/share:/usr/share`) with `icons`
/// and `pixmaps`, and `$XDG_DATA_HOME` (default `~/.local/share`) with
/// `icons`.
pub(crate) fn xdg_icon_roots_from(
    data_dirs: Option<OsString>,
    data_home: Option<OsString>,
    home: Option<OsString>,
) -> Vec<PathBuf> {
    let dirs = data_dirs
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| OsString::from("/usr/local/share:/usr/share"));
    let mut roots = Vec::new();
    for dir in std::env::split_paths(&dirs).filter(|d| d.is_absolute()) {
        roots.push(dir.join("icons"));
        roots.push(dir.join("pixmaps"));
    }
    let data_home = data_home
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|h| h.join(".local/share"))
        });
    if let Some(dir) = data_home {
        roots.push(dir.join("icons"));
    }
    roots
}

/// [`xdg_icon_roots_from`] with the process environment.
pub(crate) fn xdg_icon_roots() -> Vec<PathBuf> {
    xdg_icon_roots_from(
        std::env::var_os("XDG_DATA_DIRS"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

/// The canonical path of `path` when it is absolute, a `.png` or `.svg`
/// (any case), canonicalizes to a regular file of at most 1 MiB, and that
/// file lies under one of `roots` (canonicalized the same way). `..`
/// segments and symlinks are resolved first, so neither leads out of a root.
/// Only the metadata is read: nothing is opened, so a FIFO or a device
/// refused here never blocks. The one way a path a remote peer sent becomes
/// something that may be drawn.
#[cfg(test)]
pub(crate) fn bounded_icon_file(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let roots: Vec<PathBuf> = roots.iter().filter_map(|r| r.canonicalize().ok()).collect();
    bounded_under(path, &roots)
}

/// [`bounded_icon_file`] with `roots` already canonical.
fn bounded_under(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let icon_file = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("svg"));
    if !path.is_absolute() || !icon_file {
        return None;
    }
    let real = path.canonicalize().ok()?;
    let meta = std::fs::metadata(&real).ok()?;
    if !meta.is_file() || meta.len() > MAX_PEER_ICON_FILE {
        return None;
    }
    roots
        .iter()
        .any(|root| real.starts_with(root))
        .then_some(real)
}

/// The XDG icon roots, canonical, computed once per process.
fn cached_roots() -> &'static [PathBuf] {
    static ROOTS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    ROOTS.get_or_init(|| {
        xdg_icon_roots()
            .iter()
            .filter_map(|r| r.canonicalize().ok())
            .collect()
    })
}

/// Whether `dir` is too broad to serve as a peer's icon root: fewer than two
/// components (`/`, `/home`), the home directory or an ancestor of it, or an
/// ancestor of an XDG icon root. A peer that named one would make every file
/// below it an icon it may draw.
#[must_use]
pub fn broad_icon_dir(dir: &Path) -> bool {
    broad_with(
        dir,
        std::env::var_os("HOME").map(PathBuf::from),
        cached_roots(),
    )
}

fn broad_with(dir: &Path, home: Option<PathBuf>, roots: &[PathBuf]) -> bool {
    let real = dir.canonicalize().unwrap_or_else(|_| dir.to_owned());
    broad_real(&real, home, roots)
}

/// [`broad_with`] for a path already canonical.
fn broad_real(real: &Path, home: Option<PathBuf>, roots: &[PathBuf]) -> bool {
    let parts = real
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count();
    if parts < 2 {
        return true;
    }
    let home = home.map(|h| h.canonicalize().unwrap_or(h));
    home.is_some_and(|h| h.starts_with(real))
        || roots.iter().any(|r| r != real && r.starts_with(real))
}

/// A peer's icon directory as the root it may serve icons from: canonical,
/// and checked as that same canonical value, so a symlink the peer swaps
/// after the check cannot widen the root (files are matched by prefix
/// against this path once they are canonical themselves). `None` when the
/// directory cannot be resolved or is too broad ([`broad_icon_dir`]).
fn peer_root(dir: &Path) -> Option<PathBuf> {
    let real = dir.canonicalize().ok()?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    (!broad_real(
        &real,
        home.map(|h| h.canonicalize().unwrap_or(h)),
        cached_roots(),
    ))
    .then_some(real)
}

/// A file a peer's `IconName` that is an absolute path points at, when
/// [`bounded_icon_file`] accepts it under the item's `theme_dir` or the XDG
/// icon roots.
#[must_use]
pub fn peer_icon_file(path: &Path, theme_dir: Option<&Path>) -> Option<PathBuf> {
    let mut roots = cached_roots().to_vec();
    roots.extend(theme_dir.and_then(peer_root));
    bounded_under(path, &roots)
}

/// The file for the relative icon `name` inside an item's own `dir`
/// (`IconThemePath`): `<dir>/<name>.svg|png`, then the freedesktop layout
/// (`<dir>/hicolor/<size>/apps/`, scalable first). The result is checked
/// like a peer's absolute path with `dir` as its only root, so a symlink in
/// the directory cannot point out of it.
#[must_use]
pub fn icon_in_dir(dir: &Path, name: &str, size: u32) -> Option<PathBuf> {
    if !valid_name(name) {
        return None;
    }
    let dir = peer_root(dir)?;
    let dir = dir.as_path();
    let mut folders = vec![
        dir.to_owned(),
        dir.join("hicolor/scalable/apps"),
        dir.join("icons/hicolor/scalable/apps"),
    ];
    for side in [size, 48, 32, 24, 22, 64, 128, 256] {
        for kind in ["apps", "status"] {
            folders.push(dir.join(format!("hicolor/{side}x{side}/{kind}")));
            folders.push(dir.join(format!("icons/hicolor/{side}x{side}/{kind}")));
        }
    }
    let roots = [dir.to_owned()];
    folders.iter().find_map(|folder| {
        ["svg", "png"]
            .iter()
            .find_map(|ext| bounded_under(&folder.join(format!("{name}.{ext}")), &roots))
    })
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
    // `read_bounded` owns the recipe: type check before the open, a
    // non-blocking open (a FIFO never waits for a writer), a regular-file
    // check on the handle and a read of at most one byte past the limit.
    let bytes = match celestina_core::atomic_file::read_bounded(path, MAX_ICON_FILE) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Err(IconError::Read {
                path: path.to_owned(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            })
        }
        Err(ReadError::NotRegular { .. }) => {
            return Err(IconError::NotRegular {
                path: path.to_owned(),
            })
        }
        Err(ReadError::TooLarge { .. }) => {
            return Err(IconError::TooLarge {
                path: path.to_owned(),
            })
        }
        Err(ReadError::Io { source, .. }) => {
            return Err(IconError::Read {
                path: path.to_owned(),
                source,
            })
        }
    };
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let raster = || IconError::Raster {
        path: path.to_owned(),
    };
    match extension.as_deref() {
        Some("svg") => {
            // Icon SVGs are drawn from themes and `.desktop` files: a file or
            // URL one names is not followed (the default string resolver
            // would read it with an unbounded `fs::read`, and block on a
            // FIFO). Embedded `data:` images still resolve.
            let options = usvg::Options {
                image_href_resolver: usvg::ImageHrefResolver {
                    resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
                    resolve_string: Box::new(|_, _| None),
                },
                ..usvg::Options::default()
            };
            let tree =
                usvg::Tree::from_data(&bytes, &options).map_err(|source| IconError::Svg {
                    path: path.to_owned(),
                    source,
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
                .as_chunks::<4>()
                .0
                .contains(&[255, 255, 255, 255]),
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

    /// A unique scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "celestina-paint-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Scratch(dir)
        }

        fn file(&self, rel: &str) -> PathBuf {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
            path
        }

        fn theme(&self, name: &str) {
            self.file(&format!("{name}/index.theme"));
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_theme_name_with_an_absolute_path_resolves_to_nothing() {
        let dir = Scratch::new("abs-theme");
        let path = dir.file("a/totk.png");
        assert!(theme_path(None, path.to_str().unwrap(), 32).is_none());
        assert!(theme_path(None, "a/b.png", 32).is_none());
    }

    #[test]
    fn desktop_icon_values_become_names_or_files() {
        assert_eq!(IconKey::desktop_icon(""), None);
        assert_eq!(
            IconKey::desktop_icon("phone"),
            Some(IconKey::Theme("phone".to_owned()))
        );
        assert_eq!(
            IconKey::desktop_icon("/opt/a.png"),
            Some(IconKey::File(PathBuf::from("/opt/a.png")))
        );
    }

    #[test]
    fn absolute_icon_name_under_a_root_is_used() {
        let dir = Scratch::new("bounded-in");
        let png = dir.file("root/app.png");
        let svg = dir.file("root/sub/APP.SVG");
        let roots = [dir.0.join("root")];
        let real_root = dir.0.join("root").canonicalize().unwrap();
        assert_eq!(
            bounded_icon_file(&png, &roots),
            Some(real_root.join("app.png"))
        );
        assert_eq!(
            bounded_icon_file(&svg, &roots),
            Some(real_root.join("sub/APP.SVG"))
        );
        // The peer-facing entry point accepts the item's own directory.
        assert_eq!(
            peer_icon_file(&png, Some(&dir.0.join("root"))),
            Some(real_root.join("app.png"))
        );
    }

    #[test]
    fn absolute_icon_name_outside_the_roots_is_refused() {
        let dir = Scratch::new("bounded-out");
        let inside = dir.file("root/app.png");
        let outside = dir.file("elsewhere/secret.png");
        let roots = [dir.0.join("root")];
        assert!(bounded_icon_file(&outside, &roots).is_none());
        // `..` that leaves the root.
        let dotdot = inside.parent().unwrap().join("../elsewhere/secret.png");
        assert!(bounded_icon_file(&dotdot, &roots).is_none());
        // A symlink in the root that points out of it.
        let link = dir.0.join("root/link.png");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(bounded_icon_file(&link, &roots).is_none());
        // A directory symlink out of the root.
        let dir_link = dir.0.join("root/dir");
        std::os::unix::fs::symlink(dir.0.join("elsewhere"), &dir_link).unwrap();
        assert!(bounded_icon_file(&dir_link.join("secret.png"), &roots).is_none());
        // Files that are not icons, relative names, missing files.
        assert!(bounded_icon_file(Path::new("/etc/shadow"), &roots).is_none());
        assert!(bounded_icon_file(Path::new("../../x.png"), &roots).is_none());
        assert!(bounded_icon_file(&dir.0.join("root/none.png"), &roots).is_none());
        let text = dir.file("root/notes.txt");
        assert!(bounded_icon_file(&text, &roots).is_none());
        // A directory named like an icon, and an oversized file.
        let folder = dir.0.join("root/d.png");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(bounded_icon_file(&folder, &roots).is_none());
        let big = dir.0.join("root/big.png");
        std::fs::write(&big, vec![0u8; (MAX_PEER_ICON_FILE + 1) as usize]).unwrap();
        assert!(bounded_icon_file(&big, &roots).is_none());
        // No roots, nothing is allowed.
        assert!(bounded_icon_file(&inside, &[]).is_none());
    }

    #[test]
    fn broad_dirs_are_refused_as_peer_roots() {
        let roots = [PathBuf::from("/usr/share/icons")];
        let home = Some(PathBuf::from("/home/u"));
        for broad in ["/", "/home", "/home/u", "/usr/share", "/usr"] {
            assert!(
                broad_with(Path::new(broad), home.clone(), &roots),
                "{broad}"
            );
        }
        for fine in [
            "/opt/app/icons",
            "/home/u/.local/share/app",
            "/usr/share/icons",
        ] {
            assert!(!broad_with(Path::new(fine), home.clone(), &roots), "{fine}");
        }
    }

    #[test]
    fn xdg_roots_follow_the_environment_with_the_spec_defaults() {
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            xdg_icon_roots_from(None, None, os("/home/u")),
            [
                "/usr/local/share/icons",
                "/usr/local/share/pixmaps",
                "/usr/share/icons",
                "/usr/share/pixmaps",
                "/home/u/.local/share/icons",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            xdg_icon_roots_from(os("/a:rel:/b"), os("/data"), os("/home/u")),
            [
                "/a/icons",
                "/a/pixmaps",
                "/b/icons",
                "/b/pixmaps",
                "/data/icons"
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn icon_from_an_item_dir_is_found_flat_or_in_the_theme_layout() {
        let dir = Scratch::new("item-dir");
        let flat = dir.file("app/flat.png");
        let themed = dir.file("app/hicolor/48x48/apps/nested.png");
        let svg = dir.file("app/hicolor/scalable/apps/vec.svg");
        let root = dir.0.join("app");
        assert_eq!(icon_in_dir(&root, "flat", 24), flat.canonicalize().ok());
        assert_eq!(icon_in_dir(&root, "nested", 24), themed.canonicalize().ok());
        assert_eq!(icon_in_dir(&root, "vec", 24), svg.canonicalize().ok());
        assert_eq!(icon_in_dir(&root, "absent", 24), None);
        assert_eq!(icon_in_dir(&root, "../flat", 24), None);
        // A symlink inside the directory that leaves it is not followed.
        let outside = dir.file("out/leak.png");
        std::os::unix::fs::symlink(&outside, root.join("leak.png")).unwrap();
        assert_eq!(icon_in_dir(&root, "leak", 24), None);
    }

    #[test]
    fn the_file_helper_takes_only_absolute_png_or_svg_regular_files() {
        let dir = Scratch::new("file-helper");
        for rel in ["a/totk.png", "a/up.PNG", "a/vec.svg"] {
            let path = dir.file(rel);
            assert_eq!(file_path(&path), Some(path.clone()), "{rel}");
        }
        assert!(file_path(&dir.0.join("missing.png")).is_none());
        for rel in ["a/x.xpm", "a/x.txt", "a/noext"] {
            assert!(file_path(&dir.file(rel)).is_none(), "{rel}");
        }
        let folder = dir.0.join("d.png");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(file_path(&folder).is_none());
        assert!(file_path(Path::new("a/b.png")).is_none());
    }

    #[test]
    fn rasterise_refuses_directories_and_oversized_files() {
        let dir = Scratch::new("raster-bounds");
        let folder = dir.0.join("d.png");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(matches!(
            rasterise_file(&folder, 16),
            Err(IconError::NotRegular { .. })
        ));
        let big = dir.0.join("big.png");
        std::fs::write(&big, vec![0u8; MAX_ICON_FILE as usize + 1]).unwrap();
        assert!(matches!(
            rasterise_file(&big, 16),
            Err(IconError::TooLarge { .. })
        ));
    }

    /// Runs `rasterise_file` on a thread and fails if it does not return in
    /// 5 s, so a regression that blocks cannot hang the suite. The blocked
    /// thread is leaked, which the process exit reaps.
    fn rasterise_within_timeout(path: &Path) -> Result<RgbaImage, IconError> {
        let (tx, rx) = std::sync::mpsc::channel();
        let path = path.to_owned();
        std::thread::spawn(move || {
            let _ = tx.send(rasterise_file(&path, 16));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("rasterise_file blocked for 5 s")
    }

    /// A FIFO at `dir/name`. `mkfifo` is a coreutils build requirement, so
    /// a missing one is a failure, not a skip.
    #[cfg(target_os = "linux")]
    fn make_fifo(dir: &Scratch, name: &str) -> PathBuf {
        let path = dir.0.join(name);
        let status = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo (coreutils) must be installed to run this test");
        assert!(status.success(), "mkfifo failed");
        path
    }

    #[test]
    fn rasterise_refuses_a_fifo_without_blocking() {
        let dir = Scratch::new("raster-fifo");
        let fifo = make_fifo(&dir, "pipe.png");
        assert!(matches!(
            rasterise_within_timeout(&fifo),
            Err(IconError::NotRegular { .. })
        ));
    }

    #[test]
    fn rasterise_reports_a_missing_file_as_not_found() {
        let dir = Scratch::new("raster-missing");
        match rasterise_file(&dir.0.join("gone.png"), 16) {
            Err(IconError::Read { source, .. }) => {
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("expected a NotFound read error, got {other:?}"),
        }
    }

    /// A 4x4 opaque red square: drawn by resvg without any raster decoder,
    /// which this build leaves out (`raster-images` is off, so PNG `<image>`
    /// elements parse but draw nothing).
    const RED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#f00"/></svg>"##;

    fn base64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
            for i in 0..=chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            }
            for _ in chunk.len()..3 {
                out.push('=');
            }
        }
        out
    }

    fn image_svg(href: &str) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><image href="{href}" width="8" height="8"/></svg>"#
        )
    }

    fn alpha(image: &RgbaImage) -> impl Iterator<Item = u8> + '_ {
        image.rgba.iter().skip(3).step_by(4).copied()
    }

    #[test]
    fn an_svg_image_href_to_a_file_is_not_followed() {
        let dir = Scratch::new("svg-href");
        // The default resolver would read this file and draw it.
        let red = dir.0.join("red.svg");
        std::fs::write(&red, RED_SVG).unwrap();
        let plain = dir.0.join("plain.svg");
        std::fs::write(&plain, image_svg(red.to_str().unwrap())).unwrap();
        let image = rasterise_within_timeout(&plain).expect("renders");
        assert!(
            alpha(&image).all(|a| a == 0),
            "the href is not followed, so nothing is drawn"
        );
        // The default resolver would also open a FIFO and wait for a writer.
        let fifo = make_fifo(&dir, "image.svg");
        let piped = dir.0.join("piped.svg");
        std::fs::write(&piped, image_svg(fifo.to_str().unwrap())).unwrap();
        assert!(rasterise_within_timeout(&piped).is_ok());
    }

    #[test]
    fn an_svg_with_data_images_still_resolves() {
        let dir = Scratch::new("svg-data");
        // A data: SVG is drawn, so the data resolver still works.
        let path = dir.0.join("data.svg");
        std::fs::write(
            &path,
            image_svg(&format!(
                "data:image/svg+xml;base64,{}",
                base64(RED_SVG.as_bytes())
            )),
        )
        .unwrap();
        let image = rasterise_within_timeout(&path).expect("renders");
        assert!(alpha(&image).any(|a| a > 0), "the data: image is drawn");
        // A data: PNG still parses (this build does not draw raster images).
        let png = dir.0.join("data-png.svg");
        std::fs::write(
            &png,
            image_svg(&format!(
                "data:image/png;base64,{}",
                base64(include_bytes!(
                    "../../../../celestina-style/icons/haze-noise.png"
                ))
            )),
        )
        .unwrap();
        assert!(rasterise_within_timeout(&png).is_ok());
    }

    #[test]
    fn settings_ini_parsing_trims_quotes_and_skips_comments() {
        let parse = settings_icon_theme;
        assert_eq!(
            parse("[Settings]\ngtk-icon-theme-name=Adwaita\n").as_deref(),
            Some("Adwaita")
        );
        assert_eq!(
            parse("gtk-icon-theme-name = Papirus Dark \n").as_deref(),
            Some("Papirus Dark")
        );
        assert_eq!(
            parse("gtk-icon-theme-name = \"Adwaita\"\n").as_deref(),
            Some("Adwaita")
        );
        assert_eq!(
            parse("gtk-icon-theme-name='Adwaita'").as_deref(),
            Some("Adwaita")
        );
        assert_eq!(
            parse("# gtk-icon-theme-name=Old\n; gtk-icon-theme-name=Older\n"),
            None
        );
        assert_eq!(
            parse("# gtk-icon-theme-name=Old\ngtk-icon-theme-name=New").as_deref(),
            Some("New")
        );
        assert_eq!(parse("gtk-theme-name=Adwaita\n"), None);
        assert_eq!(parse("gtk-icon-theme-name=\n"), None);
    }

    #[test]
    fn the_gsettings_theme_wins_when_it_is_installed() {
        let dir = Scratch::new("theme-gs");
        dir.theme("Papirus");
        dir.theme("Adwaita");
        let bases = [dir.0.clone()];
        let ini = "gtk-icon-theme-name=Adwaita";
        assert_eq!(
            choose_theme(Some("Papirus"), Some(ini), None, &bases),
            Some(("Papirus".to_owned(), ThemeSource::Gsettings))
        );
    }

    #[test]
    fn a_missing_gsettings_theme_falls_back_to_the_settings_files() {
        let dir = Scratch::new("theme-ini");
        dir.theme("Adwaita");
        dir.theme("Other");
        let bases = [dir.0.clone()];
        assert_eq!(
            choose_theme(
                Some("WhiteSur-dark"),
                Some("gtk-icon-theme-name = Adwaita"),
                Some("gtk-icon-theme-name = Other"),
                &bases
            ),
            Some(("Adwaita".to_owned(), ThemeSource::Gtk3Settings))
        );
        assert_eq!(
            choose_theme(
                None,
                Some("gtk-icon-theme-name = Gone"),
                Some("gtk-icon-theme-name = Other"),
                &bases
            ),
            Some(("Other".to_owned(), ThemeSource::Gtk4Settings))
        );
    }

    #[test]
    fn adwaita_is_the_last_resort_and_nothing_installed_means_hicolor() {
        let dir = Scratch::new("theme-last");
        let bases = [dir.0.clone()];
        assert_eq!(choose_theme(Some("Gone"), None, None, &bases), None);
        dir.theme("Adwaita");
        assert_eq!(
            choose_theme(Some("Gone"), None, None, &bases),
            Some(("Adwaita".to_owned(), ThemeSource::Adwaita))
        );
        assert!(!theme_installed(&bases, "../x"));
        assert!(!theme_installed(&bases, ""));
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
