//! The catcher (CEL-D-003): one transparent layer surface per output, mapped
//! while the Island shows a page or a capsule sheet. It covers the output
//! below the Island's own surface, so a press anywhere else closes what is
//! shown (the same way a press outside the sheet does) and never reaches the
//! window under it. It paints nothing, takes no keyboard and asks for no
//! blur: the Island's surface holds the keyboard (`Exclusive`) while
//! anything is shown, which gives Escape, Tab and Enter from the first frame.

use celestina_wayland::{
    Anchoring, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputId, Rect, SurfaceId,
};

use crate::app::SURFACE_SIZE;

/// The catcher's layer-shell namespace.
pub const NAMESPACE: &str = "celestina-catcher";

/// The spec of the catcher on `output`: the whole output, over every
/// exclusive zone, with no keyboard.
#[must_use]
pub fn spec(output: OutputId) -> LayerSpec {
    LayerSpec {
        output,
        namespace: NAMESPACE,
        layer: Layer::Top,
        anchor: Anchoring::Fill,
        size: (0, 0),
        exclusive_zone: -1,
        keyboard: KeyboardInteractivity::None,
    }
}

/// The input region of a catcher of `size`: everything under the Island's
/// own surface (`band` tall), nothing of the band itself.
#[must_use]
pub fn input_region(size: (f32, f32), band: f32) -> Vec<Rect> {
    if size.0 <= 0.0 || size.1 <= band {
        return Vec::new();
    }
    vec![Rect {
        x: 0.0,
        y: band,
        w: size.0,
        h: size.1 - band,
    }]
}

/// [`input_region`] for the Island's surface height.
#[must_use]
pub fn region_for(size: (f32, f32)) -> Vec<Rect> {
    input_region(size, SURFACE_SIZE.1 as f32)
}

/// Whether `ev` on the catcher closes what is shown: a pointer press.
#[must_use]
pub fn closes(ev: &InputEvent) -> bool {
    matches!(ev, InputEvent::PointerPress { .. })
}

/// One live catcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Catcher {
    /// The Island's surface it serves.
    island: SurfaceId,
    output: OutputId,
    surface: SurfaceId,
}

/// The catchers by Island.
#[derive(Debug, Default)]
pub struct Catchers {
    live: Vec<Catcher>,
}

impl Catchers {
    /// Whether the Island on `island` has a catcher.
    #[must_use]
    pub fn has(&self, island: SurfaceId) -> bool {
        self.live.iter().any(|c| c.island == island)
    }

    /// Whether `surface` is a catcher.
    #[must_use]
    pub fn owns(&self, surface: SurfaceId) -> bool {
        self.live.iter().any(|c| c.surface == surface)
    }

    /// The Island surface the catcher `surface` serves.
    #[must_use]
    pub fn island_of(&self, surface: SurfaceId) -> Option<SurfaceId> {
        self.live
            .iter()
            .find(|c| c.surface == surface)
            .map(|c| c.island)
    }

    /// Records a catcher mapped for `island`.
    pub fn add(&mut self, island: SurfaceId, output: OutputId, surface: SurfaceId) {
        self.live.push(Catcher {
            island,
            output,
            surface,
        });
    }

    /// Forgets the catcher of `island`; its surface, to destroy.
    pub fn take(&mut self, island: SurfaceId) -> Option<SurfaceId> {
        let at = self.live.iter().position(|c| c.island == island)?;
        Some(self.live.remove(at).surface)
    }

    /// Forgets the catchers of `output` (their surfaces went with it).
    pub fn remove_output(&mut self, output: OutputId) {
        self.live.retain(|c| c.output != output);
    }

    /// How many catchers live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Whether none lives.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_region_is_everything_under_the_islands_band() {
        let region = input_region((2560.0, 1440.0), 560.0);
        assert_eq!(
            region,
            vec![Rect {
                x: 0.0,
                y: 560.0,
                w: 2560.0,
                h: 880.0
            }]
        );
        assert!(!region[0].contains(100.0, 559.0), "the band is the panel's");
        assert!(region[0].contains(100.0, 560.0));
        assert!(region[0].contains(2559.0, 1439.0));
    }

    #[test]
    fn an_output_no_taller_than_the_band_has_no_region() {
        assert!(input_region((1920.0, 560.0), 560.0).is_empty());
        assert!(input_region((0.0, 1080.0), 560.0).is_empty());
    }

    #[test]
    fn the_spec_covers_the_output_without_the_keyboard() {
        let spec = spec(OutputId(3));
        assert_eq!(spec.layer, Layer::Top);
        assert_eq!(spec.anchor, Anchoring::Fill);
        assert_eq!(spec.exclusive_zone, -1);
        assert_eq!(spec.keyboard, KeyboardInteractivity::None);
        assert_eq!(spec.output, OutputId(3));
    }

    #[test]
    fn only_a_pointer_press_closes() {
        let press = InputEvent::PointerPress {
            x: 10.0,
            y: 900.0,
            button: 0x110,
            serial: 1,
        };
        assert!(closes(&press));
        assert!(!closes(&InputEvent::PointerMotion { x: 1.0, y: 1.0 }));
        assert!(!closes(&InputEvent::PointerLeave));
    }

    #[test]
    fn the_registry_follows_each_island() {
        let mut catchers = Catchers::default();
        assert!(catchers.is_empty());
        catchers.add(SurfaceId(10), OutputId(1), SurfaceId(100));
        catchers.add(SurfaceId(20), OutputId(2), SurfaceId(200));
        assert!(catchers.has(SurfaceId(10)));
        assert!(catchers.owns(SurfaceId(200)));
        assert_eq!(catchers.island_of(SurfaceId(200)), Some(SurfaceId(20)));
        assert_eq!(catchers.take(SurfaceId(10)), Some(SurfaceId(100)));
        assert!(!catchers.has(SurfaceId(10)));
        catchers.remove_output(OutputId(2));
        assert_eq!(catchers.len(), 0);
    }
}
