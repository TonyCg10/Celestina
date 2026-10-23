//! Frames on the lock surfaces: the product draws, the toolkit owns the
//! shared memory (design §4.6, §4.6.1, H-15).
//!
//! After `locked` the product paints with the CPU into `wl_shm`. This module
//! is everything protocol-shaped about that, so product code needs no
//! `unsafe`, no pool and no `wl_buffer`:
//!
//! * one pool per output with exactly two `XRGB8888` buffers, handed out
//!   alternately; a buffer the compositor has not released is never handed out
//!   (it would be drawn on while shown);
//! * the memory of a new pool is zero, so a fresh buffer is black and the
//!   product need not clear it ([`FrameBuf::fresh`]);
//! * at a fractional scale the buffer has the real pixel size and
//!   `wp_viewport` maps it to the logical size of the surface, which is the
//!   size the compositor configured, so the lock-surface size check holds;
//!   without the two globals the buffer is the logical size at scale 1 and the
//!   compositor scales it;
//! * a `configure` that repeats the size of a frame already shown re-commits
//!   that frame instead of blanking it with a cover.
//!
//! A frame that cannot be made (no pool, both buffers still in use) leaves the
//! last committed content, which is the cover or the previous frame; the lock
//! stays locked. A refused frame is remembered, and the release of a buffer
//! asks the product to draw again ([`super::LockEvent::Redraw`]).
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use smithay_client_toolkit::shm::{raw::RawPool, CreatePoolError, Shm};
use wayland_client::{
    protocol::{wl_buffer::WlBuffer, wl_shm, wl_surface::WlSurface},
    QueueHandle,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::WpFractionalScaleV1,
    },
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use super::{
    handlers::{BufferTag, NoEvents, ScaleTag},
    LockRuntime, LockShell,
};
use crate::{geometry::PixelRect, output::OutputId};

/// Bytes per pixel of `XRGB8888`.
const BYTES_PER_PIXEL: u64 = 4;
/// `wp_fractional_scale_v1.preferred_scale` is in 120ths.
const SCALE_UNIT: u64 = 120;
/// The largest pool `wl_shm` can express (signed 32-bit size).
const POOL_MAX: u64 = i32::MAX as u64;

/// One buffer to draw on, handed to the product's painter for the length of
/// the call.
pub struct FrameBuf<'a> {
    /// Which of the output's two buffers this is (`0` or `1`). The product
    /// tracks what it drew into each, because the buffer it gets next is the
    /// one it drew into two frames ago.
    pub index: usize,
    /// All zero (black) and never drawn on since its pool was made: the
    /// product forgets what it believed this buffer held.
    pub fresh: bool,
    /// Size in buffer pixels.
    pub width: u32,
    pub height: u32,
    /// Bytes per row.
    pub stride: usize,
    /// Buffer pixels per logical pixel (`1.0` without fractional scaling).
    pub scale: f64,
    /// `XRGB8888`, little endian: the bytes of a pixel are blue, green, red,
    /// unused. `stride * height` bytes.
    pub pixels: &'a mut [u8],
}

/// Why a frame was not shown.
#[derive(Debug, thiserror::Error)]
pub enum PresentError {
    /// The output has no lock surface (it went away, or the lock ended).
    #[error("the output has no lock surface")]
    UnknownOutput,
    /// The compositor has not configured the lock surface yet.
    #[error("the lock surface is not configured yet")]
    NotConfigured,
    /// Both buffers still belong to the compositor; the frame is drawn again
    /// when one is released.
    #[error("both buffers are still in use by the compositor")]
    Busy,
    /// A buffer of this size cannot be expressed in `wl_shm`.
    #[error("a {0}x{1} frame cannot be expressed in wl_shm")]
    Size(u32, u32),
    #[error("cannot create the frame's shared-memory pool")]
    Pool(#[source] CreatePoolError),
}

/// What [`super::LockCtl`] reaches the surfaces through; object safe so the
/// control handle does not carry the runtime's type parameter.
pub(crate) trait Surfaces {
    fn present(
        &mut self,
        output: OutputId,
        paint: &mut dyn FnMut(FrameBuf<'_>) -> Option<PixelRect>,
    ) -> Result<(), PresentError>;

    fn scale_of(&self, output: OutputId) -> f64;
}

/// The buffer pixel size for a logical size at a scale in 120ths: rounded,
/// never zero (`wp_fractional_scale_v1`'s rule).
#[must_use]
pub(crate) fn buffer_px(logical: (u32, u32), scale_120: Option<u32>) -> (u32, u32) {
    let Some(scale) = scale_120.filter(|s| *s > 0) else {
        return logical;
    };
    let side = |px: u32| {
        let scaled = (u64::from(px) * u64::from(scale) + SCALE_UNIT / 2) / SCALE_UNIT;
        u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
    };
    (side(logical.0), side(logical.1))
}

/// Where the two buffers of a pool sit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PoolPlan {
    pub(crate) stride: usize,
    /// Bytes of one buffer.
    pub(crate) frame: usize,
    /// Bytes of the pool: two frames.
    pub(crate) len: usize,
}

impl PoolPlan {
    /// The plan for two buffers of `size`; `None` for a zero side or a pool
    /// `wl_shm` cannot express.
    pub(crate) fn of(size: (u32, u32)) -> Option<Self> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return None;
        }
        let stride = u64::from(w).checked_mul(BYTES_PER_PIXEL)?;
        let frame = stride.checked_mul(u64::from(h))?;
        let len = frame.checked_mul(2)?;
        if len > POOL_MAX || stride > POOL_MAX {
            return None;
        }
        Some(Self {
            stride: usize::try_from(stride).ok()?,
            frame: usize::try_from(frame).ok()?,
            len: usize::try_from(len).ok()?,
        })
    }
}

/// The damage to send, clamped to the buffer; `None` when nothing is left.
#[must_use]
pub(crate) fn clamp_damage(rect: PixelRect, size: (u32, u32)) -> Option<PixelRect> {
    let w = i64::from(size.0);
    let h = i64::from(size.1);
    let x0 = i64::from(rect.x).clamp(0, w);
    let y0 = i64::from(rect.y).clamp(0, h);
    let x1 = (i64::from(rect.x) + i64::from(rect.w)).clamp(0, w);
    let y1 = (i64::from(rect.y) + i64::from(rect.h)).clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(PixelRect {
        x: i32::try_from(x0).ok()?,
        y: i32::try_from(y0).ok()?,
        w: i32::try_from(x1 - x0).ok()?,
        h: i32::try_from(y1 - y0).ok()?,
    })
}

/// Which of two buffers is free, and which was never drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ring {
    busy: [bool; 2],
    fresh: [bool; 2],
    next: usize,
}

impl Ring {
    pub(crate) fn new() -> Self {
        Self {
            busy: [false; 2],
            fresh: [true; 2],
            next: 0,
        }
    }

    /// The buffer to draw on: the one after the last committed, else the
    /// other; `None` when the compositor holds both.
    pub(crate) fn pick(&self) -> Option<usize> {
        (0..2).map(|k| (self.next + k) % 2).find(|i| !self.busy[*i])
    }

    pub(crate) fn is_fresh(&self, index: usize) -> bool {
        self.fresh[index]
    }

    /// The buffer was drawn on, whether or not it was committed.
    pub(crate) fn drawn(&mut self, index: usize) {
        self.fresh[index] = false;
    }

    /// The buffer was attached and committed: the compositor owns it.
    pub(crate) fn committed(&mut self, index: usize) {
        self.busy[index] = true;
        self.next = (index + 1) % 2;
    }

    pub(crate) fn released(&mut self, index: usize) {
        self.busy[index] = false;
    }
}

/// The pool and its two buffers for one size.
struct Buffers {
    generation: u64,
    size: (u32, u32),
    plan: PoolPlan,
    pool: RawPool,
    wl: [WlBuffer; 2],
    ring: Ring,
}

impl Drop for Buffers {
    fn drop(&mut self) {
        for buffer in &self.wl {
            buffer.destroy();
        }
    }
}

/// Everything the canvas keeps for one lock surface.
struct Slot {
    surface: WlSurface,
    viewport: Option<WpViewport>,
    fractional: Option<WpFractionalScaleV1>,
    /// `wp_fractional_scale_v1.preferred_scale`, once the compositor said.
    preferred: Option<u32>,
    /// The size the compositor configured.
    logical: Option<(u32, u32)>,
    /// A frame of the product is the surface's current content.
    shown: bool,
    /// A frame was refused for lack of a free buffer.
    wanted: bool,
    buffers: Option<Buffers>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        if let Some(fractional) = self.fractional.take() {
            fractional.destroy();
        }
        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }
    }
}

impl Slot {
    /// The scale of the frames: the preferred one, only with a viewport to map
    /// the buffer back to the logical size.
    fn scale_120(&self) -> Option<u32> {
        self.viewport.as_ref()?;
        self.preferred
    }
}

/// The frames of every lock surface.
pub(crate) struct Canvas<S: LockShell> {
    shm: Shm,
    qh: QueueHandle<LockRuntime<S>>,
    viewporter: Option<WpViewporter>,
    fractional: Option<WpFractionalScaleManagerV1>,
    slots: BTreeMap<OutputId, Slot>,
    generation: u64,
}

impl<S: LockShell> Canvas<S> {
    pub(crate) fn new(
        shm: Shm,
        qh: QueueHandle<LockRuntime<S>>,
        viewporter: Option<WpViewporter>,
        fractional: Option<WpFractionalScaleManagerV1>,
    ) -> Self {
        Self {
            shm,
            qh,
            viewporter,
            fractional,
            slots: BTreeMap::new(),
            generation: 0,
        }
    }

    /// A lock surface was created for `output`.
    pub(crate) fn add(&mut self, output: OutputId, surface: WlSurface) {
        let viewport = self
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&surface, &self.qh, NoEvents));
        // A preferred scale is only useful with a viewport to apply it.
        let fractional = viewport
            .as_ref()
            .and(self.fractional.as_ref())
            .map(|m| m.get_fractional_scale(&surface, &self.qh, ScaleTag(output)));
        self.slots.insert(
            output,
            Slot {
                surface,
                viewport,
                fractional,
                preferred: None,
                logical: None,
                shown: false,
                wanted: false,
                buffers: None,
            },
        );
    }

    pub(crate) fn remove(&mut self, output: OutputId) {
        self.slots.remove(&output);
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
    }

    /// The compositor configured `surface` at `size`. Returns `true` when a
    /// frame of the product is already showing at exactly this size and was
    /// re-committed as it is (so no cover must replace it); `false` when the
    /// caller must commit the cover.
    pub(crate) fn configured(&mut self, surface: &WlSurface, size: (u32, u32)) -> bool {
        let Some(slot) = self.slots.values_mut().find(|s| &s.surface == surface) else {
            return false;
        };
        let same = slot.logical == Some(size);
        slot.logical = Some(size);
        if same && slot.shown {
            slot.surface.commit();
            return true;
        }
        // The cover is the logical size at scale 1: no viewport mapping, and
        // the frames' pixel size no longer applies.
        slot.shown = false;
        slot.buffers = None;
        if let Some(viewport) = &slot.viewport {
            viewport.set_destination(-1, -1);
        }
        false
    }

    /// The compositor's preferred scale for `output` changed. Returns `true`
    /// when a frame is showing at the old scale and must be drawn again.
    pub(crate) fn preferred(&mut self, output: OutputId, scale_120: u32) -> bool {
        let Some(slot) = self.slots.get_mut(&output) else {
            return false;
        };
        let changed = slot.preferred != Some(scale_120);
        slot.preferred = Some(scale_120);
        changed && slot.shown
    }

    /// The compositor released a buffer. Returns `true` when a frame was
    /// refused meanwhile and the product should draw again.
    pub(crate) fn released(&mut self, tag: BufferTag) -> bool {
        let Some(slot) = self.slots.get_mut(&tag.output) else {
            return false;
        };
        let Some(buffers) = slot.buffers.as_mut() else {
            return false;
        };
        if buffers.generation != tag.generation {
            return false;
        }
        buffers.ring.released(tag.index);
        std::mem::take(&mut slot.wanted)
    }

    fn allocate(
        shm: &Shm,
        qh: &QueueHandle<LockRuntime<S>>,
        generation: u64,
        output: OutputId,
        size: (u32, u32),
    ) -> Result<Buffers, PresentError> {
        let plan = PoolPlan::of(size).ok_or(PresentError::Size(size.0, size.1))?;
        let mut pool = RawPool::new(plan.len, shm).map_err(PresentError::Pool)?;
        let (width, height, stride, frame) = (
            i32::try_from(size.0).map_err(|_| PresentError::Size(size.0, size.1))?,
            i32::try_from(size.1).map_err(|_| PresentError::Size(size.0, size.1))?,
            i32::try_from(plan.stride).map_err(|_| PresentError::Size(size.0, size.1))?,
            i32::try_from(plan.frame).map_err(|_| PresentError::Size(size.0, size.1))?,
        );
        let make = |pool: &mut RawPool, index: usize, offset: i32| {
            pool.create_buffer(
                offset,
                width,
                height,
                stride,
                wl_shm::Format::Xrgb8888,
                BufferTag {
                    output,
                    generation,
                    index,
                },
                qh,
            )
        };
        let first = make(&mut pool, 0, 0);
        let second = make(&mut pool, 1, frame);
        Ok(Buffers {
            generation,
            size,
            plan,
            pool,
            wl: [first, second],
            ring: Ring::new(),
        })
    }
}

impl<S: LockShell> Surfaces for Canvas<S> {
    fn present(
        &mut self,
        output: OutputId,
        paint: &mut dyn FnMut(FrameBuf<'_>) -> Option<PixelRect>,
    ) -> Result<(), PresentError> {
        let slot = self
            .slots
            .get_mut(&output)
            .ok_or(PresentError::UnknownOutput)?;
        let logical = slot.logical.ok_or(PresentError::NotConfigured)?;
        let scale_120 = slot.scale_120();
        let size = buffer_px(logical, scale_120);

        if slot.buffers.as_ref().is_none_or(|b| b.size != size) {
            self.generation += 1;
            // The old pair goes first: its pool is not needed any more.
            slot.buffers = None;
            slot.buffers = Some(Self::allocate(
                &self.shm,
                &self.qh,
                self.generation,
                output,
                size,
            )?);
        }
        let Some(buffers) = slot.buffers.as_mut() else {
            return Err(PresentError::UnknownOutput);
        };
        let Some(index) = buffers.ring.pick() else {
            slot.wanted = true;
            return Err(PresentError::Busy);
        };
        let fresh = buffers.ring.is_fresh(index);
        let start = index * buffers.plan.frame;
        let stride = buffers.plan.stride;
        let frame = buffers.plan.frame;
        let scale = if size == logical {
            1.0
        } else {
            f64::from(size.0) / f64::from(logical.0.max(1))
        };
        let damage = {
            let mmap = buffers.pool.mmap();
            let Some(pixels) = mmap.get_mut(start..start + frame) else {
                return Err(PresentError::Size(size.0, size.1));
            };
            paint(FrameBuf {
                index,
                fresh,
                width: size.0,
                height: size.1,
                stride,
                scale,
                pixels,
            })
        };
        buffers.ring.drawn(index);
        let Some(damage) = damage.and_then(|rect| clamp_damage(rect, size)) else {
            return Ok(());
        };

        if let Some(viewport) = &slot.viewport {
            // The surface size is the configured logical size whatever the
            // buffer's pixel size.
            viewport.set_destination(
                i32::try_from(logical.0).unwrap_or(i32::MAX),
                i32::try_from(logical.1).unwrap_or(i32::MAX),
            );
        }
        slot.surface.set_buffer_scale(1);
        slot.surface.attach(Some(&buffers.wl[index]), 0, 0);
        slot.surface
            .damage_buffer(damage.x, damage.y, damage.w, damage.h);
        slot.surface.commit();
        buffers.ring.committed(index);
        slot.shown = true;
        slot.wanted = false;
        Ok(())
    }

    fn scale_of(&self, output: OutputId) -> f64 {
        self.slots
            .get(&output)
            .and_then(|slot| {
                let logical = slot.logical?;
                let size = buffer_px(logical, slot.scale_120());
                Some(f64::from(size.0) / f64::from(logical.0.max(1)))
            })
            .unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_4k_output_at_scale_1_5_has_a_physical_buffer_and_a_logical_surface() {
        // 2560x1440 logical at 180/120 is the 3840x2160 panel.
        assert_eq!(buffer_px((2560, 1440), Some(180)), (3840, 2160));
        assert_eq!(buffer_px((1920, 1080), None), (1920, 1080));
        assert_eq!(buffer_px((1920, 1080), Some(0)), (1920, 1080));
    }

    #[test]
    fn the_buffer_size_rounds_half_up_and_is_never_zero() {
        // 1.25: 1001 * 150 / 120 = 1251.25 -> 1251; 3 * 60 / 120 = 1.5 -> 2.
        assert_eq!(buffer_px((1001, 3), Some(150)), (1251, 4));
        assert_eq!(buffer_px((1, 1), Some(1)), (1, 1));
    }

    #[test]
    fn the_pool_holds_two_xrgb8888_frames() {
        let plan = PoolPlan::of((3840, 2160)).expect("a 4K pool fits wl_shm");
        assert_eq!(plan.stride, 3840 * 4);
        assert_eq!(plan.frame, 3840 * 2160 * 4);
        assert_eq!(plan.len, 2 * plan.frame);
    }

    #[test]
    fn a_pool_wl_shm_cannot_express_has_no_plan() {
        assert_eq!(PoolPlan::of((0, 10)), None);
        assert_eq!(PoolPlan::of((10, 0)), None);
        assert_eq!(PoolPlan::of((u32::MAX, u32::MAX)), None);
        // Two frames of 2^30 bytes are 2^31: one byte too many.
        assert_eq!(PoolPlan::of((1, 268_435_456)), None);
        assert!(PoolPlan::of((1, 268_435_455)).is_some());
    }

    #[test]
    fn damage_is_clamped_to_the_buffer() {
        let size = (100, 50);
        let rect = |x, y, w, h| PixelRect { x, y, w, h };
        assert_eq!(
            clamp_damage(rect(10, 10, 20, 20), size),
            Some(rect(10, 10, 20, 20))
        );
        assert_eq!(
            clamp_damage(rect(-5, -5, 20, 20), size),
            Some(rect(0, 0, 15, 15))
        );
        assert_eq!(
            clamp_damage(rect(90, 40, 50, 50), size),
            Some(rect(90, 40, 10, 10))
        );
        assert_eq!(clamp_damage(rect(100, 0, 5, 5), size), None);
        assert_eq!(clamp_damage(rect(0, 0, 0, 5), size), None);
        assert_eq!(clamp_damage(rect(0, 0, -3, 5), size), None);
    }

    #[test]
    fn the_buffers_alternate_and_a_held_buffer_is_never_handed_out() {
        let mut ring = Ring::new();
        assert!(ring.is_fresh(0) && ring.is_fresh(1));
        assert_eq!(ring.pick(), Some(0));
        ring.drawn(0);
        ring.committed(0);
        assert!(!ring.is_fresh(0) && ring.is_fresh(1));
        assert_eq!(ring.pick(), Some(1));
        ring.drawn(1);
        ring.committed(1);
        // The compositor holds both: nothing to draw on.
        assert_eq!(ring.pick(), None);
        ring.released(0);
        assert_eq!(ring.pick(), Some(0));
        ring.committed(0);
        assert_eq!(ring.pick(), None);
        ring.released(1);
        assert_eq!(ring.pick(), Some(1));
    }

    #[test]
    fn a_buffer_released_at_once_is_reused_in_turn() {
        let mut ring = Ring::new();
        for expected in [0, 1, 0, 1] {
            let index = ring.pick().expect("a free buffer");
            assert_eq!(index, expected);
            ring.drawn(index);
            ring.committed(index);
            ring.released(index);
        }
    }
}
