//! The wallpaper duty (CEL-2-E): one still image per output, painted on a
//! layer-shell surface in the `Background` layer.
//!
//! [`decode`] turns a hostile image file into an opaque RGBA buffer of
//! exactly the output's device size, bounded in file size, dimensions, area
//! and predicted decoder memory (its module docs give the peak per format);
//! it runs on the wallpaper worker, never on the loop's thread.
//! [`surface::Wallpapers`] is the app's side: the surface per output, the
//! requests to the worker, the texture upload and the 1:1 draw.

pub mod decode;
pub mod surface;

/// The layer-shell namespace of the wallpaper surface; the owner check
/// recognises its own surface by it.
pub const NAMESPACE: &str = "celestina-wallpaper";
