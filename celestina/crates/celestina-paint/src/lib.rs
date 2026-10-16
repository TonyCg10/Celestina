//! The painter of the shell: the `Painter` contract over femtovg, the Haze
//! material, the cosmic-text glyph atlas and icons, as the design specifies
//! in §5.3 (`docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md`).
#![forbid(unsafe_code)]

mod atlas;
mod backend;
mod color;
mod haze;
mod icon;
mod painter;
mod text;
mod theme;

pub use backend::GpuError;
pub use celestina_wayland::Rect;
pub use color::{Color, ColorError};
pub use haze::GrainError;
pub use haze::{silhouette_slices, HazePlan};
pub use icon::{default_icon_dirs, IconKey, PixmapId};
pub use painter::{AtlasUsage, PaintError, Painter};
pub use text::{default_font_dirs, TextError, TextRun, TextStyle, TextSystem, Weight, LINE_HEIGHT};
pub use theme::{MotionMs, Role, Spacing, Theme, TypeScale};

/// The directories a style asset `kind` (`fonts`, `icons`) is looked for in,
/// in order: the `over` environment override, the checkout's
/// `celestina-style/<kind>`, then `$prefix/share/celestina/<kind>` beside an
/// installed binary at `$prefix/bin`.
pub(crate) fn style_asset_dirs(
    over: Option<std::ffi::OsString>,
    exe: Option<std::path::PathBuf>,
    kind: &str,
) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(over) = over.filter(|o| !o.is_empty()) {
        dirs.push(over.into());
    }
    dirs.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../celestina-style")
            .join(kind),
    );
    if let Some(bin) = exe.as_deref().and_then(std::path::Path::parent) {
        dirs.push(bin.join("../share/celestina").join(kind));
    }
    dirs
}

/// A decoded RGBA image, premultiplied, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
