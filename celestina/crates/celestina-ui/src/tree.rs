//! The node tree: a slab of nodes with their taffy twins, laid-out rects,
//! animated properties and the highlight mirror.
//!
//! Nodes live in a slab indexed by [`NodeId`]; an id stays valid for as
//! long as its key survives in the views given to `Tree::apply`
//! (`reconcile.rs`), and is reused only after the node it named is pruned
//! and forgotten by the highlight. Each node has a taffy twin for layout
//! (`layout.rs`); the paint walk is `paint.rs`.
#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use celestina_paint::{MotionMs, Rect, TextRun, Theme};
use taffy::{Display, TaffyTree};

use crate::{
    anim::{Animated, Reduced},
    curve::Motion,
    field::{Field, FieldId},
    highlight::Highlight,
    key::Key,
    slider::{Slider, SliderMetrics},
    view::ViewKind,
};

/// A node's index in the tree's slab.
pub type NodeId = usize;

/// The layout engine refused an operation on its own node ids: the tree's
/// bookkeeping and taffy's disagree. The tree records the first such fault
/// and keeps serving the last good layout.
#[derive(Debug, thiserror::Error)]
pub enum TreeError {
    #[error("the layout engine refused to {op}")]
    Layout {
        op: &'static str,
        #[source]
        source: taffy::TaffyError,
    },
}

const EMPTY: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 0.0,
    h: 0.0,
};

/// One node: what the view said, its animated state and its place.
pub(crate) struct Slot {
    pub key: Key,
    pub kind: ViewKind,
    pub style: taffy::Style,
    pub interactive: bool,
    pub focusable: bool,
    pub clip: bool,
    /// The view's opacity; inserted nodes fade in, leaving ones fade out.
    pub opacity: Animated,
    /// 1.0 = none; the press recoil animates it about the node's centre.
    pub scale: Animated,
    /// The slide of a node a reconciliation moved, from its old place to
    /// its new one, relative to its parent's own slide; painted as a
    /// translate of its subtree.
    pub offset_x: Animated,
    pub offset_y: Animated,
    /// Removed from the view: fading out, out of layout, hit-test and
    /// focus, pruned when the fade settles.
    pub leaving: bool,
    pub taffy: taffy::NodeId,
    pub parent: Option<NodeId>,
    /// In paint order, leaving children kept where they were.
    pub children: Vec<NodeId>,
    /// Absolute, in logical px, from the last layout.
    pub rect: Rect,
    /// Whether `rect` comes from a layout yet.
    pub laid_out: bool,
    /// The rect before the last `apply`, for the slide `layout` starts.
    pub moved_from: Option<Rect>,
    /// The shaped text of a `Text` node, or the placeholder of a `Field`,
    /// built by the paint walk.
    pub run: Option<TextRun>,
    /// The editor of a `Field` node; it outlives every update that keeps
    /// the node's key and field id.
    pub field: Option<Field>,
}

impl Slot {
    pub(crate) fn is_animating(&self, now: Instant) -> bool {
        self.opacity.is_animating(now)
            || self.scale.is_animating(now)
            || self.offset_x.is_animating(now)
            || self.offset_y.is_animating(now)
    }

    pub(crate) fn hidden(&self) -> bool {
        self.style.display == Display::None
    }
}

/// One step of a walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Visit {
    Enter(NodeId),
    Leave(NodeId),
}

/// The retained UI tree of one surface.
pub struct Tree {
    pub(crate) slots: Vec<Option<Slot>>,
    pub(crate) free: Vec<NodeId>,
    /// Leaving roots first, the live root last.
    pub(crate) roots: Vec<NodeId>,
    /// Every node carries its `NodeId` as context for the measure call.
    pub(crate) taffy: TaffyTree<NodeId>,
    pub(crate) dirty: bool,
    /// The available size of the cached layout.
    pub(crate) laid_out: Option<(f32, f32)>,
    /// The device pixel ratio text was measured and shaped at.
    pub(crate) dpr: f32,
    /// When the last `apply` ran: the slides `layout` starts begin then.
    pub(crate) applied_at: Instant,
    pub(crate) highlight: Highlight,
    pub(crate) reduced_motion: bool,
    /// The theme's duration ladder, read once per theme.
    ladder: MotionMs,
    /// The theme's press recoil scale.
    pub(crate) press_scale: f32,
    /// The theme's slider anatomy, shared by layout, hit-test and paint.
    pub(crate) slider_metrics: SliderMetrics,
    fault: Option<TreeError>,
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tree")
            .field("nodes", &self.slots.iter().flatten().count())
            .field("roots", &self.roots)
            .field("dirty", &self.dirty)
            .field("highlight", &self.highlight)
            .field("reduced_motion", &self.reduced_motion)
            .finish_non_exhaustive()
    }
}

impl Tree {
    /// An empty tree animating on `theme`'s motion ladder and press scale;
    /// `reduced_motion` makes spatial and scale motion a single step while
    /// opacity fades stay.
    pub fn new(reduced_motion: bool, theme: &Theme) -> Tree {
        Tree {
            slots: Vec::new(),
            free: Vec::new(),
            roots: Vec::new(),
            taffy: TaffyTree::new(),
            dirty: true,
            laid_out: None,
            dpr: 1.0,
            applied_at: Instant::now(),
            highlight: Highlight::default(),
            reduced_motion,
            ladder: theme.motion,
            press_scale: theme.press_scale,
            slider_metrics: SliderMetrics::of(theme),
            fault: None,
        }
    }

    /// Takes `theme`'s motion ladder and press scale for the animations
    /// started from now on (running ones finish as they were), and its
    /// slider anatomy, re-measuring every slider when it changed.
    pub fn set_theme(&mut self, theme: &Theme) {
        self.ladder = theme.motion;
        self.press_scale = theme.press_scale;
        let metrics = SliderMetrics::of(theme);
        if metrics != self.slider_metrics {
            self.slider_metrics = metrics;
            let sliders: Vec<taffy::NodeId> = self
                .slots
                .iter()
                .flatten()
                .filter(|s| matches!(s.kind, ViewKind::Slider(_)))
                .map(|s| s.taffy)
                .collect();
            for node in sliders {
                let r = self.taffy.mark_dirty(node);
                self.checked("mark a slider for layout", r);
            }
            self.dirty = true;
        }
    }

    /// `motion` on the theme's ladder.
    pub(crate) fn duration(&self, motion: Motion) -> Duration {
        motion.duration(&self.ladder)
    }

    /// Switches reduced motion; turning it on ends every running slide and
    /// recoil at its target.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
        if on {
            for slot in self.slots.iter_mut().flatten() {
                slot.scale.settle();
                slot.offset_x.settle();
                slot.offset_y.settle();
            }
        }
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// The policy for a spatial animation under the current switch.
    pub(crate) fn spatial(&self) -> Reduced {
        Reduced::spatial(self.reduced_motion)
    }

    /// Pre-order: parent first, children in order, so the last child is
    /// painted on top. Hidden (`Display::None`) subtrees are left out, so
    /// they are neither painted, hit nor focused; leaving nodes are in
    /// (they are painted while they fade).
    pub fn paint_order(&self) -> Vec<NodeId> {
        let mut order = Vec::new();
        self.visit(&mut |v| {
            if let Visit::Enter(id) = v {
                order.push(id);
            }
        });
        order
    }

    /// Walks the visible tree in paint order, entering and leaving each
    /// node.
    pub(crate) fn visit(&self, f: &mut impl FnMut(Visit)) {
        let mut stack: Vec<Visit> = self.roots.iter().rev().map(|&r| Visit::Enter(r)).collect();
        while let Some(v) = stack.pop() {
            if let Visit::Enter(id) = v {
                let Some(slot) = self.slot(id) else {
                    continue;
                };
                if slot.hidden() {
                    continue;
                }
                stack.push(Visit::Leave(id));
                stack.extend(slot.children.iter().rev().map(|&c| Visit::Enter(c)));
            }
            f(v);
        }
    }

    /// Absolute rect in logical px after `layout`; empty for a stale id.
    /// Hit areas are these rects: slides and recoils are visual only.
    pub fn rect(&self, id: NodeId) -> Rect {
        self.slot(id).map_or(EMPTY, |s| s.rect)
    }

    /// The node's key; [`Key::STALE`] for an id the tree no longer holds.
    pub fn key_of(&self, id: NodeId) -> Key {
        self.slot(id).map_or(Key::STALE, |s| s.key)
    }

    /// The first node in paint order (hidden ones included) with `key`
    /// that is not leaving, else the first leaving one. Keys are matched
    /// among siblings; a product that wants events to name one node keeps
    /// its keys unique across the view.
    pub fn node_by_key(&self, key: Key) -> Option<NodeId> {
        let mut leaving = None;
        let mut stack: Vec<NodeId> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let Some(slot) = self.slot(id) else {
                continue;
            };
            if slot.key == key {
                if !slot.leaving {
                    return Some(id);
                }
                leaving = leaving.or(Some(id));
            }
            stack.extend(slot.children.iter().rev());
        }
        leaving
    }

    /// The editor of the field the product named `id`: the first such node
    /// in paint order that is not leaving.
    pub fn field(&self, id: FieldId) -> Option<&Field> {
        let node = self.field_node(id)?;
        self.slot(node)?.field.as_ref()
    }

    /// [`Tree::field`], to set its text or edit it.
    pub fn field_mut(&mut self, id: FieldId) -> Option<&mut Field> {
        let node = self.field_node(id)?;
        self.slot_mut(node)?.field.as_mut()
    }

    /// What the slider keyed `key` shows (its level after any press, drag
    /// or key since the last view), or `None` when that node is not a
    /// slider.
    pub fn slider(&self, key: Key) -> Option<Slider> {
        match self.slot(self.node_by_key(key)?)?.kind {
            ViewKind::Slider(s) => Some(s),
            _ => None,
        }
    }

    fn field_node(&self, id: FieldId) -> Option<NodeId> {
        let mut stack: Vec<NodeId> = self.roots.iter().rev().copied().collect();
        while let Some(node) = stack.pop() {
            let Some(slot) = self.slot(node) else {
                continue;
            };
            if !slot.leaving && matches!(&slot.kind, ViewKind::Field { id: f, .. } if *f == id) {
                return Some(node);
            }
            stack.extend(slot.children.iter().rev());
        }
        None
    }

    /// Whether any opacity, scale or slide is in flight.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.slots.iter().flatten().any(|s| s.is_animating(now))
    }

    pub fn highlight(&self) -> Highlight {
        self.highlight
    }

    pub(crate) fn set_highlight(&mut self, h: Highlight) {
        self.highlight = h;
    }

    /// The node's opacity at `now` times every ancestor's: what it is drawn
    /// with; 0 for a stale id.
    pub fn effective_opacity(&self, id: NodeId, now: Instant) -> f32 {
        if self.slot(id).is_none() {
            return 0.0;
        }
        let mut opacity = 1.0;
        let mut current = Some(id);
        while let Some(slot) = current.and_then(|i| self.slot(i)) {
            opacity *= slot.opacity.value(now);
            current = slot.parent;
        }
        opacity
    }

    /// The node's own recoil scale at `now`; 1 for a stale id.
    pub fn scale(&self, id: NodeId, now: Instant) -> f32 {
        self.slot(id).map_or(1.0, |s| s.scale.value(now))
    }

    /// The node's own slide at `now`, relative to its parent's; (0, 0) for
    /// a stale id.
    pub fn offset(&self, id: NodeId, now: Instant) -> (f32, f32) {
        self.slot(id).map_or((0.0, 0.0), |s| {
            (s.offset_x.value(now), s.offset_y.value(now))
        })
    }

    /// The first layout fault since the last call, if any.
    pub fn take_fault(&mut self) -> Option<TreeError> {
        self.fault.take()
    }

    /// Keeps the first fault: later ones are usually its consequences.
    pub(crate) fn checked<T>(&mut self, op: &'static str, r: taffy::TaffyResult<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(source) => {
                self.fault.get_or_insert(TreeError::Layout { op, source });
                None
            }
        }
    }

    pub(crate) fn slot(&self, id: NodeId) -> Option<&Slot> {
        self.slots.get(id).and_then(Option::as_ref)
    }

    pub(crate) fn slot_mut(&mut self, id: NodeId) -> Option<&mut Slot> {
        self.slots.get_mut(id).and_then(Option::as_mut)
    }

    /// Stores `slot` in a free id (or a new one) and returns it.
    pub(crate) fn alloc(&mut self, slot: Slot) -> NodeId {
        match self.free.pop() {
            Some(id) if id < self.slots.len() => {
                self.slots[id] = Some(slot);
                id
            }
            _ => {
                self.slots.push(Some(slot));
                self.slots.len() - 1
            }
        }
    }

    /// The id the next `alloc` returns.
    pub(crate) fn next_id(&self) -> NodeId {
        match self.free.last() {
            Some(&id) if id < self.slots.len() => id,
            _ => self.slots.len(),
        }
    }

    /// Whether `id` is a live node drawn in the tree: neither it nor an
    /// ancestor is hidden or leaving.
    pub(crate) fn is_shown(&self, id: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(i) = current {
            let Some(slot) = self.slot(i) else {
                return false;
            };
            if slot.hidden() || slot.leaving {
                return false;
            }
            current = slot.parent;
        }
        true
    }

    /// Whether `id` is `ancestor` or below it.
    pub(crate) fn is_within(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(i) = current {
            if i == ancestor {
                return true;
            }
            current = self.slot(i).and_then(|s| s.parent);
        }
        false
    }

    /// Drops every leaving node whose fade has settled, with its subtree.
    pub(crate) fn prune(&mut self, now: Instant) {
        let settled: Vec<NodeId> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(id, s)| {
                s.as_ref()
                    .filter(|s| s.leaving && !s.opacity.is_animating(now))
                    .map(|_| id)
            })
            .collect();
        for id in settled {
            // An ancestor pruned first took this one with it.
            if self.slot(id).is_some() {
                self.remove_subtree(id);
            }
        }
    }

    /// Removes `id` and everything below it: from its parent (or the
    /// roots), from taffy and from the highlight, before any id is reused.
    fn remove_subtree(&mut self, id: NodeId) {
        match self.slot(id).and_then(|s| s.parent) {
            Some(parent) => {
                if let Some(p) = self.slot_mut(parent) {
                    p.children.retain(|&c| c != id);
                }
            }
            None => self.roots.retain(|&r| r != id),
        }
        let mut stack = vec![id];
        let mut removed = Vec::new();
        while let Some(n) = stack.pop() {
            let Some(slot) = self.slots.get_mut(n).and_then(Option::take) else {
                continue;
            };
            let r = self.taffy.remove(slot.taffy);
            self.checked("remove a node", r);
            stack.extend(slot.children);
            removed.push(n);
        }
        self.highlight.forget(|n| removed.contains(&n));
        self.free.extend(removed);
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use celestina_paint::Theme;

    use super::*;
    use crate::{
        curve::{Motion, LINEAR},
        testing::{card, card_with_rows, dur, fake_measure, id, row_key, row_view, AVAILABLE},
        view,
    };

    #[test]
    fn paint_order_is_preorder_last_child_on_top() {
        let mut tree = Tree::new(false, &Theme::dark());
        let rows = (0..3).map(|i| {
            let r = row_view(i);
            if i == 1 {
                r.child(view::col(Key::named("inner")).size(1.0, 1.0))
            } else {
                r
            }
        });
        tree.apply(card(rows), Instant::now());
        let card = id(&tree, Key::named("card"));
        let inner = id(&tree, Key::named("inner"));
        let rows = [0, 1, 2].map(|i| id(&tree, row_key(i)));
        assert_eq!(
            tree.paint_order(),
            vec![card, rows[0], rows[1], inner, rows[2]]
        );
    }

    #[test]
    fn hidden_subtrees_are_left_out_of_the_walk() {
        let t0 = Instant::now();
        let with_inner = |hidden: bool| {
            card((0..3).map(|i| {
                let r = row_view(i);
                if i == 1 {
                    r.child(view::col(Key::named("inner")).size(1.0, 1.0))
                        .hidden(hidden)
                } else {
                    r
                }
            }))
        };
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(with_inner(true), t0);
        let card = id(&tree, Key::named("card"));
        let inner = id(&tree, Key::named("inner"));
        let rows = [0, 1, 2].map(|i| id(&tree, row_key(i)));
        assert_eq!(tree.paint_order(), vec![card, rows[0], rows[2]]);
        tree.apply(with_inner(false), t0);
        assert_eq!(
            tree.paint_order(),
            vec![card, rows[0], rows[1], inner, rows[2]]
        );
    }

    #[test]
    fn keys_and_ids_map_both_ways() {
        let (tree, card, rows) = card_with_rows();
        assert_eq!(tree.key_of(card), Key::named("card"));
        assert_eq!(tree.key_of(rows[2]), row_key(2));
        assert_eq!(tree.node_by_key(row_key(1)), Some(rows[1]));
        assert_eq!(tree.node_by_key(Key::named("nothing")), None);
    }

    #[test]
    fn stale_ids_read_as_empty() {
        let (tree, _, _) = card_with_rows();
        let stale: NodeId = 999;
        let r = tree.rect(stale);
        assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(tree.key_of(stale), Key::STALE);
        assert_eq!(tree.effective_opacity(stale, Instant::now()), 0.0);
    }

    #[test]
    fn effective_opacity_multiplies_the_ancestors() {
        let t0 = Instant::now();
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(
            card((0..3).map(|i| row_view(i).opacity(0.5))).opacity(0.5),
            t0,
        );
        let row = id(&tree, row_key(0));
        assert_eq!(tree.effective_opacity(row, t0), 0.25);
    }

    #[test]
    fn is_animating_follows_any_property() {
        let (mut tree, _, rows) = card_with_rows();
        let t0 = Instant::now();
        assert!(!tree.is_animating(t0));
        if let Some(slot) = tree.slot_mut(rows[2]) {
            slot.opacity
                .animate_to(0.0, dur(Motion::Fast), LINEAR, t0, Reduced::Opacity);
        }
        assert!(tree.is_animating(t0));
        assert!(!tree.is_animating(t0 + dur(Motion::Fast)));
        if let Some(slot) = tree.slot_mut(rows[1]) {
            slot.scale
                .animate_to(0.5, dur(Motion::Fast), LINEAR, t0, Reduced::Opacity);
        }
        assert!(tree.is_animating(t0 + Duration::from_millis(50)));
    }

    #[test]
    fn reduced_motion_settles_running_spatial_motion() {
        let (mut tree, _, rows) = card_with_rows();
        let t0 = Instant::now();
        if let Some(slot) = tree.slot_mut(rows[0]) {
            slot.scale
                .animate_to(0.96, dur(Motion::Slow), LINEAR, t0, Reduced::Opacity);
            slot.opacity
                .animate_to(0.0, dur(Motion::Slow), LINEAR, t0, Reduced::Opacity);
        }
        tree.set_reduced_motion(true);
        assert!(tree.reduced_motion());
        assert_eq!(tree.scale(rows[0], t0), 0.96);
        // Opacity keeps fading.
        assert!(tree.is_animating(t0));
    }

    #[test]
    fn the_tree_starts_empty() {
        let tree = Tree::new(false, &Theme::dark());
        assert!(tree.paint_order().is_empty());
        assert_eq!(tree.highlight(), Highlight::default());
        assert!(!tree.is_animating(Instant::now()));
        let mut tree = tree;
        tree.layout_measured(AVAILABLE, fake_measure);
        assert!(tree.take_fault().is_none());
    }
}
