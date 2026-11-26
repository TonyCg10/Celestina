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
    highlight::{box_fill, box_wash, focus_ring, FOCUS_WIDTH},
    slider::{self, SliderPaint},
    tree::{NodeId, Scope, Slot, Tree, Visit},
    view::ViewKind,
    Key,
};

/// The alpha the accent is painted at behind selected text: a wash that
/// leaves the text readable in both schemes.
const SELECTION_ALPHA: f32 = 0.35;
/// A box outline's width, logical px.
const STROKE_WIDTH: f32 = 1.0;
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
        self.paint_scoped(painter, now, Scope::All, &[]);
    }

    /// [`Tree::paint`] with the subtrees of the nodes keyed in `hidden` painted
    /// by nobody (pieces that fly elsewhere): the rest of the walk is
    /// unchanged and the node keeps its place in the layout.
    pub fn paint_hiding(&mut self, painter: &mut Painter, now: Instant, hidden: &[Key]) {
        self.paint_scoped(painter, now, Scope::All, hidden);
    }

    /// [`Tree::paint_hiding`] without the node `skip` and its subtree (a
    /// sheet draws that one itself): the rest of the walk is unchanged.
    pub fn paint_except_hiding(
        &mut self,
        painter: &mut Painter,
        now: Instant,
        skip: NodeId,
        hidden: &[Key],
    ) {
        self.paint_scoped(painter, now, Scope::Except(skip), hidden);
    }

    /// [`Tree::paint_hiding`] for the subtrees of `parent`'s children only:
    /// not the node's own box, haze or ring. The slides of `parent` and its
    /// ancestors still shift them; their recoil scale and opacity do not
    /// apply.
    pub fn paint_children_hiding(
        &mut self,
        painter: &mut Painter,
        now: Instant,
        parent: NodeId,
        hidden: &[Key],
    ) {
        self.paint_scoped(painter, now, Scope::ChildrenOf(parent), hidden);
    }

    /// Paints what is inside the node `id` alone (its children's subtrees,
    /// not its own box), even while `id` is hidden (out of the layout, at
    /// the rects it was last laid out at): a piece that fades out of or into
    /// its slot is drawn so, by whoever owns its fade. The slides of its
    /// ancestors still shift it.
    pub fn paint_inside(&mut self, painter: &mut Painter, now: Instant, id: NodeId) {
        self.paint_scoped(painter, now, Scope::Inside(id), &[]);
    }

    /// The node `id`'s rect as it is painted at `now`: its layout rect moved
    /// by its slide and its ancestors', or the rect [`Tree::set_extent`] gave
    /// it. An empty rect for a stale id.
    #[must_use]
    pub fn painted_rect(&self, id: NodeId, now: Instant) -> Rect {
        let Some(slot) = self.slot(id) else {
            return Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            };
        };
        self.extent_of(slot.key)
            .unwrap_or_else(|| translated(slot.rect, self.slide_of(id, now)))
    }

    /// The rect set for the box `key`, if one is.
    fn extent_of(&self, key: Key) -> Option<Rect> {
        self.extents
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, r)| *r)
    }

    /// The children of `id` that are not leaving, in order, each with its
    /// key, the rect it is painted at `now` and whether it is hidden (out of
    /// the layout: the rect is then where it was, not where it is drawn).
    #[must_use]
    pub fn painted_children(&self, id: NodeId, now: Instant) -> Vec<(Key, Rect, bool)> {
        let Some(slot) = self.slot(id) else {
            return Vec::new();
        };
        let base = self.slide_of(id, now);
        slot.children
            .iter()
            .filter_map(|&c| self.slot(c).filter(|s| !s.leaving).map(|s| (c, s)))
            .map(|(c, s)| {
                let own = self.offset(c, now);
                let shift = (base.0 + own.0, base.1 + own.1);
                (s.key, translated(s.rect, shift), s.hidden())
            })
            .collect()
    }

    fn paint_scoped(&mut self, painter: &mut Painter, now: Instant, scope: Scope, hidden: &[Key]) {
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
        let base = match scope {
            Scope::ChildrenOf(parent) => self.slide_of(parent, now),
            Scope::Inside(root) => self.slide_of(root, now),
            _ => (0.0, 0.0),
        };
        let mut shifts: Vec<(f32, f32)> = vec![base];
        for v in self.without(self.walk(scope), hidden) {
            match v {
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
            }
        }
    }

    /// `visits` without the subtrees of the nodes keyed in `hidden`.
    fn without(&self, visits: Vec<Visit>, hidden: &[Key]) -> Vec<Visit> {
        if hidden.is_empty() {
            return visits;
        }
        let mut inside: Option<NodeId> = None;
        let mut out = Vec::with_capacity(visits.len());
        for v in visits {
            match v {
                Visit::Enter(id) => {
                    if inside.is_none() && self.slot(id).is_some_and(|s| hidden.contains(&s.key)) {
                        inside = Some(id);
                    }
                    if inside.is_none() {
                        out.push(v);
                    }
                }
                Visit::Leave(id) => {
                    if inside == Some(id) {
                        inside = None;
                    } else if inside.is_none() {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// The accumulated slide of `id` and its ancestors.
    fn slide_of(&self, id: NodeId, now: Instant) -> (f32, f32) {
        let mut sum = (0.0, 0.0);
        let mut at = Some(id);
        while let Some(n) = at {
            let own = self.offset(n, now);
            sum = (sum.0 + own.0, sum.1 + own.1);
            at = self.slot(n).and_then(|s| s.parent);
        }
        sum
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
        let rect = self
            .extent_of(slot.key)
            .unwrap_or_else(|| translated(slot.rect, shift));
        let scale = slot.scale.value(now);
        if scale != 1.0 {
            painter.scale_about(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0, scale);
        }
        let opacity = slot.opacity.value(now);
        if opacity < 1.0 {
            painter.opacity(opacity.max(0.0));
        }
        match &slot.kind {
            ViewKind::Box {
                fill,
                stroke,
                haze,
                radius,
            } => {
                if *haze {
                    painter.haze(rect, *radius);
                }
                let h = self.highlight;
                let (hovered, pressed) = (h.hovered == Some(id), h.pressed == Some(id));
                let shown = box_fill(*fill, slot.interactive, hovered, pressed);
                if let Some(role) = shown {
                    painter.fill_rounded(rect, *radius, theme.color(role));
                }
                if let Some(role) = box_wash(*fill, slot.interactive, hovered, pressed) {
                    painter.fill_rounded(rect, *radius, theme.color(role));
                }
                if let Some(role) = stroke {
                    let (edge, radius) = stroke_edge(rect, *radius);
                    painter.stroke_rounded(edge, radius, STROKE_WIDTH, theme.color(*role));
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
            ViewKind::Slider(s) => {
                let h = self.highlight;
                let p = SliderPaint {
                    slider: *s,
                    rect,
                    hovered: h.hovered == Some(id),
                    pressed: h.pressed == Some(id),
                };
                slider::paint(painter, theme, &self.slider_metrics, &p);
            }
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
                    // A slider's ring is a pill around the whole control.
                    ViewKind::Slider(_) => rect.h / 2.0,
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

/// The rect and radius a box's outline is stroked on: the stroke is centred
/// on that rect, so the rect is inset by half the width and the outline lies
/// wholly inside the box.
fn stroke_edge(rect: Rect, radius: f32) -> (Rect, f32) {
    let inset = STROKE_WIDTH / 2.0;
    let edge = Rect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - STROKE_WIDTH).max(0.0),
        h: (rect.h - STROKE_WIDTH).max(0.0),
    };
    (edge, (radius - inset).max(0.0))
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
            stroke: None,
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
    fn the_outline_lies_inside_the_box() {
        let (edge, radius) = stroke_edge(R, 10.0);
        assert_eq!((edge.x, edge.y, edge.w, edge.h), (1.5, 2.5, 2.0, 3.0));
        assert_eq!(radius, 9.5);
        let (_, radius) = stroke_edge(R, 0.0);
        assert_eq!(radius, 0.0);
    }

    /// The capsule of `layout.rs`'s tests: three 30 px pieces, 10 px
    /// padding, 8 px gap, the one `hide` hidden, hugging as `cap`.
    fn capsule(hide: Option<u64>) -> crate::view::View {
        let shown = if hide.is_some() { 2.0 } else { 3.0 };
        crate::view::row(Key::named("cap"))
            .absolute(100.0, 8.0)
            .width(20.0 + 30.0 * shown + 8.0 * (shown - 1.0))
            .height(40.0)
            .padding_xy(10.0, 0.0)
            .gap(8.0)
            .children((0..3).map(|i| {
                crate::view::row(crate::testing::row_key(i))
                    .size(30.0, 40.0)
                    .hidden(hide == Some(i))
            }))
    }

    fn lifted_capsule() -> (Tree, NodeId, Instant) {
        use crate::testing::{fake_measure, id, AVAILABLE};
        let t0 = Instant::now();
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(capsule(None), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        tree.slide_with(
            std::time::Duration::from_millis(350),
            crate::Easing::OutBack(1.15),
            &[Key::named("cap")],
        );
        tree.apply(capsule(Some(1)), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let cap = id(&tree, Key::named("cap"));
        (tree, cap, t0)
    }

    #[test]
    fn a_box_with_an_extent_is_painted_and_reported_there() {
        let (mut tree, cap, t0) = lifted_capsule();
        // At its layout rect: placed at 100, 88 wide.
        let rect = tree.painted_rect(cap, t0);
        assert_eq!((rect.x, rect.w, rect.y, rect.h), (100.0, 88.0, 8.0, 40.0));
        let wider = Rect {
            x: 100.0,
            y: 8.0,
            w: 126.0,
            h: 40.0,
        };
        tree.set_extent(Key::named("cap"), Some(wider));
        assert_eq!(tree.painted_rect(cap, t0), wider);
        // Only the box keyed so, and until it is put back.
        let kid = crate::testing::id(&tree, crate::testing::row_key(0));
        assert_ne!(tree.painted_rect(kid, t0), wider);
        tree.set_extent(Key::named("cap"), None);
        assert_eq!(tree.painted_rect(cap, t0), rect);
        assert_eq!(tree.painted_rect(usize::MAX, t0).w, 0.0);
    }

    #[test]
    fn painted_children_name_hidden_ones_and_skip_leaving_ones() {
        let (tree, cap, t0) = lifted_capsule();
        let kids = tree.painted_children(cap, t0);
        assert_eq!(kids.len(), 3);
        assert_eq!(
            kids.iter().map(|k| k.2).collect::<Vec<_>>(),
            [false, true, false]
        );
        assert_eq!(kids[0].0, crate::testing::row_key(0));
        assert_eq!(
            tree.painted_children(usize::MAX, t0),
            Vec::<(Key, Rect, bool)>::new()
        );
    }

    #[test]
    fn an_inside_walk_enters_what_is_in_a_hidden_node_but_not_the_node() {
        let (tree, cap, _) = lifted_capsule();
        let hidden = crate::testing::id(&tree, crate::testing::row_key(1));
        // The whole walk leaves the hidden node out; the inside walk paints
        // its children and not its own box.
        assert!(!tree.walk(Scope::All).contains(&Visit::Enter(hidden)));
        let inside = tree.walk(Scope::Inside(hidden));
        assert!(!inside.contains(&Visit::Enter(hidden)));
        assert!(!inside.contains(&Visit::Enter(cap)));
        // A stale id yields nothing.
        assert!(tree.walk(Scope::Inside(usize::MAX)).is_empty());
    }

    #[test]
    fn a_hidden_key_takes_its_subtree_out_of_the_walk_only() {
        let (tree, card, rows) = crate::testing::card_with_rows();
        let all = tree.walk(Scope::ChildrenOf(card));
        let kept = tree.without(all.clone(), &[crate::testing::row_key(1)]);
        assert_eq!(kept.len(), all.len() - 2);
        assert!(!kept.contains(&Visit::Enter(rows[1])));
        assert!(!kept.contains(&Visit::Leave(rows[1])));
        assert!(kept.contains(&Visit::Enter(rows[0])) && kept.contains(&Visit::Leave(rows[2])));
        assert_eq!(tree.without(all.clone(), &[]), all);
        assert_eq!(tree.without(all.clone(), &[Key::named("nope")]), all);
        // Several keys leave several subtrees out.
        let two = tree.without(
            all.clone(),
            &[crate::testing::row_key(0), crate::testing::row_key(2)],
        );
        assert_eq!(two.len(), all.len() - 4);
        assert!(two.contains(&Visit::Enter(rows[1])));
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
