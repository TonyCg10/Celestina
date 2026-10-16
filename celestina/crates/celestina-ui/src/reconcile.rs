//! Keyed reconciliation: a new `View` matched against the live nodes.
//!
//! Children are matched by key, in order, through a key → node map per
//! parent. A matched node is updated in place: its kind and style change
//! only when they differ (a text or style change marks it for layout, an
//! unchanged one does not), its opacity is retargeted, never restarted, and
//! its hover, press, focus, recoil and slide carry on. Before matching, every
//! laid-out node records its rect so `layout` can slide the ones that moved.
//! An unmatched new view is inserted fading in (0 → its opacity, `Normal`,
//! linear) unless its parent is new too; an unmatched old node is marked
//! leaving and fades out (`Exit`, linear) out of layout, then is pruned.
//! The behaviour is tested in `tests/reconcile.rs` through the public API.
#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use crate::{
    anim::{Animated, Reduced},
    curve::{Motion, LINEAR},
    field::Field,
    key::Key,
    tree::{NodeId, Slot, Tree},
    view::{View, ViewKind},
};

const EMPTY: celestina_paint::Rect = celestina_paint::Rect {
    x: 0.0,
    y: 0.0,
    w: 0.0,
    h: 0.0,
};

impl Tree {
    /// Reconciles the tree with `root`, a view of the whole surface, at
    /// `now`; leaving nodes whose fade has settled are pruned first.
    pub fn apply(&mut self, root: View, now: Instant) {
        self.prune(now);
        self.applied_at = now;
        for slot in self.slots.iter_mut().flatten() {
            slot.moved_from = (slot.laid_out && !slot.leaving).then_some(slot.rect);
        }
        let live = self
            .roots
            .last()
            .copied()
            .filter(|&r| self.slot(r).is_some_and(|s| !s.leaving));
        match live {
            Some(r) if self.key_of(r) == root.key => self.update(r, root, now),
            Some(r) => {
                self.leave(r, now);
                if let Some(id) = self.insert(root, None, true, now) {
                    self.roots.push(id);
                }
            }
            None => {
                // The first view appears at once; a view replacing a
                // leaving one fades in over it.
                let fade = !self.roots.is_empty();
                if let Some(id) = self.insert(root, None, fade, now) {
                    self.roots.push(id);
                }
            }
        }
        // A node hidden, or under a hidden or leaving one, cannot keep the
        // focus: Enter and Space would activate something not drawn.
        if self.highlight.focused.is_some_and(|f| !self.is_shown(f)) {
            self.highlight.focused = None;
        }
    }

    /// Updates node `id` in place from `view` (same key) and reconciles
    /// its children.
    fn update(&mut self, id: NodeId, view: View, now: Instant) {
        let View {
            kind,
            style,
            interactive,
            focusable,
            clip,
            opacity,
            children,
            ..
        } = view;
        let normal = self.duration(Motion::Normal);
        let Some(slot) = self.slot_mut(id) else {
            return;
        };
        let taffy_id = slot.taffy;
        // A key that returns while leaving is revived where it is.
        slot.leaving = false;
        let mut relayout = false;
        if slot.kind != kind {
            relayout = measure_changed(&slot.kind, &kind);
            match (&slot.kind, &kind) {
                // The same field keeps its text, cursor and selection.
                (ViewKind::Field { id: a, .. }, ViewKind::Field { id: b, .. }) if a == b => {}
                (_, ViewKind::Field { .. }) => slot.field = Some(Field::new()),
                _ => slot.field = None,
            }
            if text_changed(&slot.kind, &kind) {
                slot.run = None;
            }
            slot.kind = kind;
        }
        let restyle = slot.style != style;
        if restyle {
            slot.style = style.clone();
        }
        slot.interactive = interactive;
        slot.focusable = focusable;
        slot.clip = clip;
        slot.opacity
            .animate_to(opacity, normal, LINEAR, now, Reduced::Opacity);
        if restyle {
            let r = self.taffy.set_style(taffy_id, style);
            self.checked("restyle a node", r);
            self.dirty = true;
        } else if relayout {
            let r = self.taffy.mark_dirty(taffy_id);
            self.checked("mark a node for layout", r);
            self.dirty = true;
        }
        if !focusable && self.highlight.focused == Some(id) {
            self.highlight.focused = None;
        }
        self.reconcile_children(id, children, now);
    }

    /// Matches `views` against `parent`'s children by key.
    fn reconcile_children(&mut self, parent: NodeId, views: Vec<View>, now: Instant) {
        let Some(slot) = self.slot_mut(parent) else {
            return;
        };
        let old = std::mem::take(&mut slot.children);
        let parent_taffy = slot.taffy;
        let mut by_key: HashMap<Key, VecDeque<NodeId>> = HashMap::new();
        for &c in &old {
            by_key.entry(self.key_of(c)).or_default().push_back(c);
        }
        let mut next = Vec::with_capacity(views.len().max(old.len()));
        for view in views {
            match by_key.get_mut(&view.key).and_then(VecDeque::pop_front) {
                Some(c) => {
                    self.update(c, view, now);
                    next.push(c);
                }
                None => {
                    if let Some(c) = self.insert(view, Some(parent), true, now) {
                        next.push(c);
                    }
                }
            }
        }
        let kept: HashSet<NodeId> = next.iter().copied().collect();
        for (i, &c) in old.iter().enumerate() {
            if !kept.contains(&c) {
                self.leave(c, now);
                next.insert(i.min(next.len()), c);
            }
        }
        let live: Vec<taffy::NodeId> = next
            .iter()
            .filter_map(|&c| self.slot(c).filter(|s| !s.leaving).map(|s| s.taffy))
            .collect();
        let current = self.taffy.children(parent_taffy);
        if self.checked("read a node's children", current).as_ref() != Some(&live) {
            let r = self.taffy.set_children(parent_taffy, &live);
            self.checked("set a node's children", r);
            self.dirty = true;
        }
        if let Some(slot) = self.slot_mut(parent) {
            slot.children = next;
        }
    }

    /// Adds `view` and its subtree under `parent` (a root when `None`);
    /// `fade` starts it at opacity 0. `None` when the layout engine
    /// refuses the node (recorded as the tree's fault).
    fn insert(
        &mut self,
        view: View,
        parent: Option<NodeId>,
        fade: bool,
        now: Instant,
    ) -> Option<NodeId> {
        let View {
            key,
            kind,
            style,
            interactive,
            focusable,
            clip,
            opacity: target,
            children,
        } = view;
        let field = matches!(kind, ViewKind::Field { .. }).then(Field::new);
        let id = self.next_id();
        let r = self.taffy.new_leaf_with_context(style.clone(), id);
        let taffy = self.checked("create a node", r)?;
        let mut opacity = Animated::new(if fade { 0.0 } else { target });
        if fade {
            let normal = self.duration(Motion::Normal);
            opacity.animate_to(target, normal, LINEAR, now, Reduced::Opacity);
        }
        let slot = Slot {
            key,
            kind,
            style,
            interactive,
            focusable,
            clip,
            opacity,
            scale: Animated::new(1.0),
            offset_x: Animated::new(0.0),
            offset_y: Animated::new(0.0),
            leaving: false,
            taffy,
            parent,
            children: Vec::new(),
            rect: EMPTY,
            laid_out: false,
            moved_from: None,
            run: None,
            field,
        };
        let id = self.alloc(slot);
        let mut kids = Vec::with_capacity(children.len());
        for child in children {
            if let Some(c) = self.insert(child, Some(id), false, now) {
                kids.push(c);
            }
        }
        let twins: Vec<taffy::NodeId> = kids
            .iter()
            .filter_map(|&c| self.slot(c).map(|s| s.taffy))
            .collect();
        if !twins.is_empty() {
            let r = self.taffy.set_children(taffy, &twins);
            self.checked("set a new node's children", r);
        }
        if let Some(slot) = self.slot_mut(id) {
            slot.children = kids;
        }
        self.dirty = true;
        Some(id)
    }

    /// Marks `id` leaving: it fades out and drops out of layout (its
    /// parent's taffy children are rebuilt by the caller), hit-test and
    /// focus; a focus inside it is dropped now.
    fn leave(&mut self, id: NodeId, now: Instant) {
        let exit = self.duration(Motion::Exit);
        if let Some(slot) = self.slot_mut(id) {
            slot.leaving = true;
            slot.opacity
                .animate_to(0.0, exit, LINEAR, now, Reduced::Opacity);
        }
        if self
            .highlight
            .focused
            .is_some_and(|f| self.is_within(f, id))
        {
            self.highlight.focused = None;
        }
    }
}

/// Whether going from `old` to `new` changes the node's measured size.
fn measure_changed(old: &ViewKind, new: &ViewKind) -> bool {
    match (old, new) {
        (ViewKind::Box { .. }, ViewKind::Box { .. }) => false,
        (ViewKind::Image { .. }, ViewKind::Image { .. }) => false,
        (ViewKind::Icon { size: a, .. }, ViewKind::Icon { size: b, .. }) => a != b,
        (
            ViewKind::Text {
                text: ta,
                style: sa,
                max_width: ma,
            },
            ViewKind::Text {
                text: tb,
                style: sb,
                max_width: mb,
            },
        ) => {
            ta != tb
                || ma != mb
                || sa.size != sb.size
                || sa.weight != sb.weight
                || sa.tabular != sb.tabular
        }
        (
            ViewKind::Field {
                placeholder: pa,
                style: sa,
                ..
            },
            ViewKind::Field {
                placeholder: pb,
                style: sb,
                ..
            },
        ) => pa != pb || sa.size != sb.size || sa.weight != sb.weight || sa.tabular != sb.tabular,
        _ => true,
    }
}

/// Whether the shaped run of a `Text` node (or the placeholder of a
/// `Field`) is stale (its colour role is
/// applied at draw time and does not count).
fn text_changed(old: &ViewKind, new: &ViewKind) -> bool {
    match (old, new) {
        (ViewKind::Text { .. }, ViewKind::Text { .. })
        | (ViewKind::Field { .. }, ViewKind::Field { .. }) => measure_changed(old, new),
        _ => true,
    }
}
