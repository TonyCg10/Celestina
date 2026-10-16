//! Highlights: which nodes draw hover, pressed and focus feedback, and how.
#![forbid(unsafe_code)]

use celestina_paint::{Rect, Role};

use crate::tree::NodeId;

/// Which nodes draw hover, pressed and focus feedback; mirrored into the
/// tree by `Interaction` so the paint walk needs no other input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Highlight {
    pub hovered: Option<NodeId>,
    pub pressed: Option<NodeId>,
    pub focused: Option<NodeId>,
}

impl Highlight {
    /// Drops every entry `gone` says is no longer a live node.
    pub(crate) fn forget(&mut self, gone: impl Fn(NodeId) -> bool) {
        for field in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
            if field.is_some_and(&gone) {
                *field = None;
            }
        }
    }
}

/// The focus ring's stroke width.
pub(crate) const FOCUS_WIDTH: f32 = 2.0;
/// How far inside the node the ring's outer edge sits.
pub(crate) const FOCUS_INSET: f32 = 2.0;

/// The fill a box is drawn with: an interactive box lifts to
/// `ControlPressed` while pressed and `ControlHover` while hovered (the
/// pointer lift), whatever its own fill; any other box keeps its fill.
pub(crate) fn box_fill(
    fill: Option<Role>,
    interactive: bool,
    hovered: bool,
    pressed: bool,
) -> Option<Role> {
    if !interactive {
        fill
    } else if pressed {
        Some(Role::ControlPressed)
    } else if hovered {
        Some(Role::ControlHover)
    } else {
        fill
    }
}

/// The rect and radius the focus ring of a node at `rect` with corner
/// `radius` is stroked on: the stroke is centred on that rect, so its outer
/// edge is `FOCUS_INSET` inside the node.
pub(crate) fn focus_ring(rect: Rect, radius: f32) -> (Rect, f32) {
    let inset = FOCUS_INSET + FOCUS_WIDTH / 2.0;
    let ring = Rect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - 2.0 * inset).max(0.0),
        h: (rect.h - 2.0 * inset).max(0.0),
    };
    (ring, (radius - inset).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect {
        x: 10.0,
        y: 20.0,
        w: 100.0,
        h: 40.0,
    };

    #[test]
    fn interactive_boxes_lift_on_hover_and_press() {
        let fill = Some(Role::ControlFill);
        assert_eq!(box_fill(fill, false, true, true), fill, "not interactive");
        assert_eq!(box_fill(fill, true, false, false), fill);
        assert_eq!(box_fill(None, true, false, false), None);
        assert_eq!(box_fill(fill, true, true, false), Some(Role::ControlHover));
        assert_eq!(box_fill(None, true, true, false), Some(Role::ControlHover));
        assert_eq!(box_fill(fill, true, true, true), Some(Role::ControlPressed));
        assert_eq!(
            box_fill(fill, true, false, true),
            Some(Role::ControlPressed)
        );
    }

    #[test]
    fn the_focus_ring_is_two_px_two_px_inside() {
        let (ring, radius) = focus_ring(R, 10.0);
        // The stroke is centred on the ring rect: its outer edge is 2 px in.
        assert_eq!((ring.x, ring.y, ring.w, ring.h), (13.0, 23.0, 94.0, 34.0));
        assert_eq!(radius, 7.0);
        assert_eq!(FOCUS_WIDTH, 2.0);
        let tiny = Rect {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
        };
        let (ring, radius) = focus_ring(tiny, 0.0);
        assert_eq!((ring.w, ring.h, radius), (0.0, 0.0, 0.0));
    }

    #[test]
    fn pruning_drops_only_the_removed_ids() {
        let mut h = Highlight {
            hovered: Some(1),
            pressed: Some(2),
            focused: Some(3),
        };
        h.forget(|id| id == 2);
        assert_eq!(
            h,
            Highlight {
                hovered: Some(1),
                pressed: None,
                focused: Some(3)
            }
        );
    }
}
