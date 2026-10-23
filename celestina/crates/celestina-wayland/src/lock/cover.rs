//! The cover: a black, opaque `wl_shm` buffer of exactly the configured size,
//! committed on a lock surface before anything else.
//!
//! The buffer is `XRGB8888` over a fresh memory file. A fresh memory file is
//! zero, `X` is ignored and `RGB` zero is black, so nothing is written, no
//! GPU, font or decoder is involved, and the cost is the pool's creation
//! (design §4.6). The single-pixel-buffer path of the first design is gone:
//! niri serves that global only in its own tests (spike Q4, 2026-10-22).
//!
//! The buffer is logical-size at scale 1; niri maps it without a protocol
//! error at any output scale, the surface's preferred scale being advice.
#![forbid(unsafe_code)]

use smithay_client_toolkit::shm::{raw::RawPool, CreatePoolError, Shm};
use wayland_client::{
    protocol::{wl_shm, wl_surface::WlSurface},
    QueueHandle,
};

/// Bytes per pixel of `XRGB8888`.
const BYTES_PER_PIXEL: u64 = 4;

/// What a cover of a given size needs from `wl_shm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CoverGeometry {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) stride: i32,
    pub(crate) len: usize,
}

impl CoverGeometry {
    /// The geometry of a cover of `size`; `None` for a zero side or one whose
    /// byte count `wl_shm` (signed 32-bit offsets and sizes) cannot express.
    pub(crate) fn of(size: (u32, u32)) -> Option<Self> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return None;
        }
        let stride = u64::from(w).checked_mul(BYTES_PER_PIXEL)?;
        let len = stride.checked_mul(u64::from(h))?;
        Some(Self {
            width: i32::try_from(w).ok()?,
            height: i32::try_from(h).ok()?,
            stride: i32::try_from(stride).ok()?,
            len: usize::try_from(i32::try_from(len).ok()?).ok()?,
        })
    }
}

/// Why a cover could not be committed. The surface keeps no buffer then, and
/// niri draws its lock colour there: the session stays locked.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CoverError {
    #[error("a {0}x{1} cover cannot be expressed in wl_shm")]
    Size(u32, u32),
    #[error("cannot create the cover's shared-memory pool")]
    Pool(#[source] CreatePoolError),
}

/// Attaches a zeroed `XRGB8888` buffer of `size` to `surface` and commits it.
pub(crate) fn commit<D>(
    shm: &Shm,
    qh: &QueueHandle<D>,
    surface: &WlSurface,
    size: (u32, u32),
) -> Result<(), CoverError>
where
    D: wayland_client::Dispatch<wayland_client::protocol::wl_buffer::WlBuffer, ()> + 'static,
{
    let geometry = CoverGeometry::of(size).ok_or(CoverError::Size(size.0, size.1))?;
    let mut pool = RawPool::new(geometry.len, shm).map_err(CoverError::Pool)?;
    let buffer = pool.create_buffer(
        0,
        geometry.width,
        geometry.height,
        geometry.stride,
        wl_shm::Format::Xrgb8888,
        (),
        qh,
    );
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, geometry.width, geometry.height);
    surface.commit();
    // The compositor keeps what it needs of a committed shm buffer; the
    // pool's memory is released when `pool` drops at the end of the call.
    buffer.destroy();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_geometry_of_a_4k_cover_is_xrgb8888_without_padding() {
        let g = CoverGeometry::of((3840, 2160)).expect("a 4K cover fits wl_shm");
        assert_eq!(g.stride, 3840 * 4);
        assert_eq!(g.len, 3840 * 2160 * 4);
        assert_eq!((g.width, g.height), (3840, 2160));
    }

    #[test]
    fn a_zero_side_has_no_cover() {
        assert_eq!(CoverGeometry::of((0, 1080)), None);
        assert_eq!(CoverGeometry::of((1920, 0)), None);
    }

    #[test]
    fn a_cover_wl_shm_cannot_express_has_no_geometry() {
        // 2^31 bytes do not fit the protocol's signed 32-bit pool size.
        assert_eq!(CoverGeometry::of((32_768, 16_384)), None);
        assert_eq!(CoverGeometry::of((u32::MAX, u32::MAX)), None);
        // 2^31 - 4 bytes do.
        assert!(CoverGeometry::of((1, 536_870_911)).is_some());
        assert_eq!(CoverGeometry::of((1, 536_870_912)), None);
    }
}
