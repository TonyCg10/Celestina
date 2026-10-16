//! The paint walk: the tree drawn through the `Painter` in paint order,
//! each node translated by its slide offset, scaled by its recoil about its
//! centre and faded by its opacity, scoped to its subtree.
//!
//! The painter has no translate, so the walk carries the accumulated slide
//! of the ancestors and adds it to every rect it draws; the recoil scale is
//! a painter transform pushed per node, so a child's slide happens inside
//! its parent's scale.
//!
//! A clipping box clips its subtree to its rounded rect. The painter keeps
//! exact rounded corners only for the first clip, or for a clip whose rect
//! contains the current one under the same transform; a nested clip that
//! does not contain its parent's falls back to a plain rectangle (Task 7's
//! `push_clip`). Hit-testing clips to the rectangles in every case.
#![forbid(unsafe_code)]

use std::time::Instant;

use celestina_paint::{Painter, Rect, Role, Theme};

use crate::{
    highlight::{box_fill, focus_ring, FOCUS_WIDTH},
    tree::{NodeId, Slot, Tree, Visit},
    view::ViewKind,
};

/// The alpha the accent is painted at behind selected text: a wash that
/// leaves the text readable in both schemes.
const SELECTION_ALPHA: f32 = 0.35;
/// The text caret's width, and the preedit underline's thickness, in
/// logical px.
const CARET_WIDTH: f32 = 1.5;
const UNDERLINE_WIDTH: f32 = 1.0;

impl Tree {
    /// Paints the tree in `paint_order` at `now`. Per node: its slide as a
    /// translate, its recoil scale about its centre and its opacity, scoped
    /// to its subtree; a clip after its own fill; the focus ring on top of
    /// its children (a text field shows its caret instead). Leaving nodes whose fade settled are pruned first.
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        self.prune(now);
        let dpr = self.dpr;
        for slot in self.slots.iter_mut().flatten() {
            match &slot.kind {
                ViewKind::Text {
                    text,
                    style,
                    max_width,
                } => {
                    if slot.run.is_none() {
                        slot.run = Some(painter.text().layout(text, style, *max_width, dpr));
                    }
                }
                ViewKind::Field {
                    placeholder, style, ..
                } => {
                    if slot.run.is_none() {
                        slot.run = Some(painter.text().layout(placeholder, style, None, dpr));
                    }
                    if let Some(field) = &mut slot.field {
                        field.shape(painter.text(), style, dpr);
                        field.reveal_caret(slot.rect.w);
                    }
                }
                _ => {}
            }
        }
        let theme = painter.theme().clone();
        let mut shifts: Vec<(f32, f32)> = vec![(0.0, 0.0)];
        self.visit(&mut |v| match v {
            Visit::Enter(id) => {
                let base = shifts.last().copied().unwrap_or((0.0, 0.0));
                let own = self.offset(id, now);
                let shift = (base.0 + own.0, base.1 + own.1);
                shifts.push(shift);
                self.paint_enter(id, painter, &theme, shift, now);
            }
            Visit::Leave(id) => {
                let shift = shifts.pop().unwrap_or((0.0, 0.0));
                self.paint_leave(id, painter, &theme, shift);
            }
        });
    }

    fn paint_enter(
        &self,
        id: NodeId,
        painter: &mut Painter,
        theme: &Theme,
        shift: (f32, f32),
        now: Instant,
    ) {
        painter.push_state();
        let Some(slot) = self.slot(id) else {
            return;
        };
        let rect = translated(slot.rect, shift);
        let scale = slot.scale.value(now);
        if scale != 1.0 {
            painter.scale_about(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0, scale);
        }
        let opacity = slot.opacity.value(now);
        if opacity < 1.0 {
            painter.opacity(opacity.max(0.0));
        }
        match &slot.kind {
            ViewKind::Box { fill, haze, radius } => {
                if *haze {
                    painter.haze(rect, *radius);
                }
                let h = self.highlight;
                let fill = box_fill(
                    *fill,
                    slot.interactive,
                    h.hovered == Some(id),
                    h.pressed == Some(id),
                );
                if let Some(role) = fill {
                    painter.fill_rounded(rect, *radius, theme.color(role));
                }
            }
            ViewKind::Text { style, .. } => {
                if let Some(run) = &slot.run {
                    let (x, y) = text_origin(rect, run.size());
                    painter.glyphs(run, x, y, theme.color(style.role));
                }
            }
            ViewKind::Field { .. } => {
                paint_field(
                    slot,
                    painter,
                    theme,
                    rect,
                    self.highlight.focused == Some(id),
                );
            }
            ViewKind::Icon { key, tint, .. } => {
                painter.icon(key, rect, tint.map(|r| theme.color(r)));
            }
            ViewKind::Image { id, alpha } => painter.image(*id, rect, *alpha),
        }
        if let Some((clip, radius)) = clip_of(&slot.kind, slot.clip, rect) {
            painter.push_clip(clip, radius);
        }
    }

    fn paint_leave(&self, id: NodeId, painter: &mut Painter, theme: &Theme, shift: (f32, f32)) {
        if let Some(slot) = self.slot(id) {
            let rect = translated(slot.rect, shift);
            if clip_of(&slot.kind, slot.clip, rect).is_some() {
                painter.pop_clip();
            }
            // A focused field shows its caret instead: it holds the focus
            // whenever its surface takes typing, not only after a key.
            let field = matches!(slot.kind, ViewKind::Field { .. });
            if self.highlight.focused == Some(id) && !field {
                let radius = match slot.kind {
                    ViewKind::Box { radius, .. } => radius,
                    _ => theme.radius_sm,
                };
                let (ring, radius) = focus_ring(rect, radius);
                painter.stroke_rounded(ring, radius, FOCUS_WIDTH, theme.color(Role::FocusRing));
            }
        }
        painter.pop_state();
    }
}

/// A field inside `rect`, clipped to it: the selection wash, the text (or
/// the placeholder in the faint role while it is empty), the underline of
/// the composition in progress, and the caret while the field has the focus.
fn paint_field(slot: &Slot, painter: &mut Painter, theme: &Theme, rect: Rect, focused: bool) {
    let (ViewKind::Field { style, .. }, Some(field)) = (&slot.kind, &slot.field) else {
        return;
    };
    painter.push_clip(rect, 0.0);
    if let Some(run) = field.run() {
        let (_, y) = text_origin(rect, run.size());
        let h = run.size().1;
        let x = rect.x - field.scroll_x();
        if let Some((lo, hi)) = field.selection_span().filter(|_| focused) {
            let wash = theme.color(Role::Accent).with_alpha(SELECTION_ALPHA);
            painter.fill_rounded(
                Rect {
                    x: x + lo,
                    y,
                    w: hi - lo,
                    h,
                },
                0.0,
                wash,
            );
        }
        painter.glyphs(run, x, y, theme.color(style.role));
        if let Some((lo, hi)) = field.preedit_span() {
            let line = Rect {
                x: x + lo,
                y: y + h - UNDERLINE_WIDTH,
                w: hi - lo,
                h: UNDERLINE_WIDTH,
            };
            painter.fill_rounded(line, 0.0, theme.color(style.role));
        }
        if focused {
            let caret = Rect {
                x: x + field.caret_x() - CARET_WIDTH / 2.0,
                y,
                w: CARET_WIDTH,
                h,
            };
            painter.fill_rounded(caret, 0.0, theme.color(style.role));
        }
    }
    if field.text().is_empty() && field.preedit().is_none() {
        if let Some(run) = &slot.run {
            let (x, y) = text_origin(rect, run.size());
            painter.glyphs(run, x, y, theme.color(Role::TextFaint));
        }
    }
    painter.pop_clip();
}

/// The clip pushed around a node's subtree: its own rect and radius, for a
/// `Box` with `clip` set. Pushed after the node's own haze and fill (which
/// it does not clip) and popped before its focus ring.
pub(crate) fn clip_of(kind: &ViewKind, clip: bool, rect: Rect) -> Option<(Rect, f32)> {
    match kind {
        ViewKind::Box { radius, .. } if clip => Some((rect, *radius)),
        _ => None,
    }
}

pub(crate) fn translated(rect: Rect, (dx, dy): (f32, f32)) -> Rect {
    Rect {
        x: rect.x + dx,
        y: rect.y + dy,
        ..rect
    }
}

/// Where a run of `size` is drawn in `rect`: at the left edge, centred
/// vertically, never above the top.
pub(crate) fn text_origin(rect: Rect, size: (f32, f32)) -> (f32, f32) {
    (rect.x, rect.y + ((rect.h - size.1) / 2.0).max(0.0))
}

#[cfg(test)]
mod tests {
    use celestina_paint::{IconKey, Role};

    use super::*;
    use crate::view::ViewKind;

    const R: Rect = Rect {
        x: 1.0,
        y: 2.0,
        w: 3.0,
        h: 4.0,
    };

    #[test]
    fn clip_applies_to_clipping_boxes_only() {
        let boxed = ViewKind::Box {
            fill: None,
            haze: false,
            radius: 20.0,
        };
        assert_eq!(clip_of(&boxed, false, R), None);
        assert_eq!(clip_of(&boxed, true, R), Some((R, 20.0)));
        let icon = ViewKind::Icon {
            key: IconKey::Semantic("bell"),
            tint: Some(Role::TextHi),
            size: 16.0,
        };
        assert_eq!(clip_of(&icon, true, R), None);
    }

    #[test]
    fn rects_are_moved_by_the_offset() {
        let moved = translated(R, (10.0, -2.0));
        assert_eq!((moved.x, moved.y, moved.w, moved.h), (11.0, 0.0, 3.0, 4.0));
    }

    #[test]
    fn text_sits_at_the_left_and_centred_vertically() {
        assert_eq!(text_origin(R, (2.0, 2.0)), (1.0, 3.0));
        // A run taller than its rect starts at the top.
        assert_eq!(text_origin(R, (2.0, 8.0)), (1.0, 2.0));
    }
}
