//! `ext_background_effect_v1` binding and the region helpers shared by the
//! blur and input regions: a cache so a region is only re-sent when its
//! pixels changed, and the `wl_region` builder.
#![forbid(unsafe_code)]

use smithay_client_toolkit::{
    background_effect::BackgroundEffectState,
    compositor::{CompositorState, Region},
    error::GlobalError,
    globals::GlobalData,
};
use wayland_client::{globals::GlobalList, protocol::wl_surface::WlSurface, Dispatch, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};

use crate::geometry::{to_pixel_rects, PixelRect, Rect};

/// Remembers the last region sent to the compositor.
#[derive(Debug, Default)]
pub struct RegionCache {
    last: Option<Vec<PixelRect>>,
}

impl RegionCache {
    /// Returns the pixel rectangles to send when they differ from the last
    /// ones sent (the very first call always counts as a change).
    pub fn update(&mut self, rects: &[Rect]) -> Option<&[PixelRect]> {
        let pixels = to_pixel_rects(rects);
        if self.last.as_ref() == Some(&pixels) {
            return None;
        }
        Some(self.last.insert(pixels).as_slice())
    }
}

/// Builds a `wl_region` covering `rects`. An empty slice yields an empty
/// region (which, as an input region, means pass-through).
pub fn build_region(
    compositor: &CompositorState,
    rects: &[PixelRect],
) -> Result<Region, GlobalError> {
    let region = Region::new(compositor)?;
    for r in rects {
        region.add(r.x, r.y, r.w, r.h);
    }
    Ok(region)
}

/// The `ext_background_effect_manager_v1` global, if the compositor has it.
pub struct Blur {
    state: BackgroundEffectState,
    available: bool,
}

impl Blur {
    /// Binds the global; logs a warning and disables blur if it is absent.
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self
    where
        D: Dispatch<ExtBackgroundEffectManagerV1, GlobalData> + 'static,
    {
        let state = BackgroundEffectState::new(globals, qh);
        let available = state.ext_background_effect_manager_v1().is_ok();
        if available {
            tracing::info!("ext_background_effect_v1 bound");
        } else {
            tracing::warn!("compositor has no ext_background_effect_v1: continuing without blur");
        }
        Self { state, available }
    }

    /// Whether the compositor offers the blur protocol at all: the painter
    /// uses the opaque fallback when it does not.
    pub fn available(&self) -> bool {
        self.available
    }

    pub fn state_mut(&mut self) -> &mut BackgroundEffectState {
        &mut self.state
    }

    /// The blur object for `surface`, or `None` when the global is absent.
    pub fn surface<D>(&self, surface: &WlSurface, qh: &QueueHandle<D>) -> Option<BlurSurface>
    where
        D: Dispatch<ExtBackgroundEffectSurfaceV1, GlobalData> + 'static,
    {
        self.state
            .get_background_effect(surface, qh)
            .ok()
            .map(BlurSurface)
    }
}

/// The per-surface `ext_background_effect_surface_v1`, destroyed on drop.
pub struct BlurSurface(ExtBackgroundEffectSurfaceV1);

impl BlurSurface {
    /// `None` disables blur; the request takes effect on the next commit.
    pub fn set_blur_region(&self, region: Option<&Region>) {
        self.0.set_blur_region(region.map(Region::wl_region));
    }
}

impl Drop for BlurSurface {
    fn drop(&mut self) {
        self.0.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, Rect};

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn cache_reports_first_value_even_when_empty() {
        let mut cache = RegionCache::default();
        assert_eq!(cache.update(&[]), Some(&[][..]));
        assert_eq!(cache.update(&[]), None);
    }

    #[test]
    fn cache_only_reports_changes() {
        let mut cache = RegionCache::default();
        let a = [r(240.0, 0.0, 280.0, 40.0)];
        let expected = [PixelRect {
            x: 240,
            y: 0,
            w: 280,
            h: 40,
        }];
        assert_eq!(cache.update(&a), Some(&expected[..]));
        assert_eq!(cache.update(&a), None);
        // A sub-pixel move that rounds to the same pixels is not a change.
        assert_eq!(cache.update(&[r(240.2, 0.0, 279.5, 40.0)]), None);
        let b = [r(240.0, 0.0, 280.0, 60.0)];
        assert_eq!(
            cache.update(&b),
            Some(
                &[PixelRect {
                    x: 240,
                    y: 0,
                    w: 280,
                    h: 60
                }][..]
            )
        );
        assert_eq!(cache.update(&[]), Some(&[][..]));
    }
}
