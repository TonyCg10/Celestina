//! The wallpaper duty (CEL-2-E): one still image per output, painted on a
//! layer-shell surface in the `Background` layer.
//!
//! [`decode`] turns a hostile image file into an opaque RGBA buffer of
//! exactly the output's device size, bounded in file size, dimensions, area
//! and predicted decoder memory (its module docs give the peak per format);
//! it runs on the wallpaper worker, never on the loop's thread.
//! [`surface::Wallpapers`] is the app's side: the surface per output, the
//! requests to the worker, the texture upload and the 1:1 draw.
//! [`thumbnail`] is the small blurred copy of each picture the session lock
//! shows behind its prompt (CEL-2-H-b), derived on the same worker.

pub mod decode;
pub mod surface;
pub mod thumbnail;

/// The layer-shell namespace of the wallpaper surface; the owner check
/// recognises its own surface by it.
pub const NAMESPACE: &str = "celestina-wallpaper";
