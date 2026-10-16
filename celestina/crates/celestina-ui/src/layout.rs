//! Layout: taffy over the node tree, text measured through the text system
//! (or any measure, for tests), absolute rects, and the offset animation of
//! nodes a reconciliation moved.
#![forbid(unsafe_code)]

use celestina_paint::{Rect, TextStyle, TextSystem};
use taffy::{AvailableSpace, Size};

use crate::{
    curve::{Motion, ONE_UI},
    tree::{NodeId, Tree},
    view::ViewKind,
};

impl Tree {
    /// Lays every live root out inside `available`, measuring text with
    /// `text` at `dpr`. A no-op while nothing changed since the last
    /// layout; a new `dpr` re-measures every text node.
    pub fn layout(&mut self, available: (f32, f32), text: &mut TextSystem, dpr: f32) {
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        if dpr != self.dpr {
            self.dpr = dpr;
            self.remeasure_text();
        }
        self.layout_measured(available, |s, style, max_width| {
            text.measure(s, style, max_width, dpr)
        });
    }

    /// [`Tree::layout`] with any text measure: `(text, style, max_width)`
    /// to logical `(width, height)`. Then slides every node whose rect the
    /// last `apply` moved from where it was to where it is, on the One UI
    /// curve over `Normal`, starting at that `apply`'s time.
    pub fn layout_measured(
        &mut self,
        available: (f32, f32),
        mut measure: impl FnMut(&str, &TextStyle, Option<f32>) -> (f32, f32),
    ) {
        if !self.dirty && self.laid_out == Some(available) {
            self.forget_moves();
            return;
        }
        let space = Size {
            width: AvailableSpace::Definite(available.0),
            height: AvailableSpace::Definite(available.1),
        };
        let live_roots: Vec<NodeId> = self
            .roots
            .iter()
            .copied()
            .filter(|&r| self.slot(r).is_some_and(|s| !s.leaving))
            .collect();
        for root in live_roots {
            let Some(root_taffy) = self.slot(root).map(|s| s.taffy) else {
                continue;
            };
            let Tree { slots, taffy, .. } = self;
            let computed = taffy.compute_layout_with_measure(
                root_taffy,
                space,
                |inputs, _, context, style| {
                    taffy::compute_leaf_layout(
                        inputs,
                        style,
                        |_, _| 0.0,
                        |known, _| {
                            let slot = context
                                .and_then(|id| slots.get(*id))
                                .and_then(Option::as_ref);
                            let (w, h) = match slot.map(|s| &s.kind) {
                                Some(ViewKind::Text {
                                    text,
                                    style,
                                    max_width,
                                }) => measure(text, style, max_width.filter(|w| w.is_finite())),
                                Some(ViewKind::Icon { size, .. }) => (*size, *size),
                                _ => (0.0, 0.0),
                            };
                            Size {
                                width: known.width.unwrap_or(w),
                                height: known.height.unwrap_or(h),
                            }
                        },
                    )
                },
            );
            self.checked("compute the layout", computed);
        }
        self.resolve_rects();
        self.start_slides();
        self.dirty = false;
        self.laid_out = Some(available);
    }

    /// Turns taffy's parent-relative layouts into absolute rects. Leaving
    /// subtrees are out of layout and keep the rects they had. Hidden
    /// subtrees keep theirs too but count as not laid out (taffy puts them
    /// at their parent's origin with no size), so revealing one places it
    /// where it belongs instead of sliding it from that corner.
    fn resolve_rects(&mut self) {
        let mut hidden = Vec::new();
        let mut stack: Vec<(NodeId, f32, f32)> =
            self.roots.iter().map(|&r| (r, 0.0, 0.0)).collect();
        while let Some((id, ox, oy)) = stack.pop() {
            let Some(slot) = self.slots.get_mut(id).and_then(Option::as_mut) else {
                continue;
            };
            if slot.leaving {
                continue;
            }
            if slot.hidden() {
                hidden.push(id);
                continue;
            }
            let Ok(layout) = self.taffy.layout(slot.taffy) else {
                continue;
            };
            slot.rect = Rect {
                x: ox + layout.location.x,
                y: oy + layout.location.y,
                w: layout.size.width,
                h: layout.size.height,
            };
            slot.laid_out = true;
            let (x, y) = (slot.rect.x, slot.rect.y);
            stack.extend(slot.children.iter().map(|&c| (c, x, y)));
        }
        while let Some(id) = hidden.pop() {
            if let Some(slot) = self.slot_mut(id) {
                slot.laid_out = false;
                slot.moved_from = None;
                hidden.extend(slot.children.iter().copied());
            }
        }
    }

    /// Starts the slide of every node the last `apply` moved: its offset
    /// jumps by how far it moved relative to its parent (whose own slide
    /// already carries it), from wherever it is drawn now, and eases back
    /// to 0. Reduced motion makes it a step.
    fn start_slides(&mut self) {
        let at = self.applied_at;
        let reduced = self.spatial();
        let normal = self.duration(Motion::Normal);
        let mut stack: Vec<(NodeId, (f32, f32))> =
            self.roots.iter().map(|&r| (r, (0.0, 0.0))).collect();
        while let Some((id, parent_moved)) = stack.pop() {
            let Some(slot) = self.slots.get_mut(id).and_then(Option::as_mut) else {
                continue;
            };
            if slot.leaving || slot.hidden() {
                // Out of layout: its frozen rects did not move.
                continue;
            }
            let moved = slot.moved_from.take().map_or(parent_moved, |old| {
                (old.x - slot.rect.x, old.y - slot.rect.y)
            });
            let own = (moved.0 - parent_moved.0, moved.1 - parent_moved.1);
            for (axis, d) in [(&mut slot.offset_x, own.0), (&mut slot.offset_y, own.1)] {
                if d != 0.0 && d.is_finite() {
                    let from = axis.value(at) + d;
                    axis.set(from);
                    axis.animate_to(0.0, normal, ONE_UI, at, reduced);
                }
            }
            stack.extend(slot.children.iter().map(|&c| (c, moved)));
        }
        self.forget_moves();
    }

    fn forget_moves(&mut self) {
        for slot in self.slots.iter_mut().flatten() {
            slot.moved_from = None;
        }
    }

    /// Marks every text node for measuring and shaping again.
    fn remeasure_text(&mut self) {
        let mut stale = Vec::new();
        for slot in self.slots.iter_mut().flatten() {
            if matches!(slot.kind, ViewKind::Text { .. }) {
                slot.run = None;
                stale.push(slot.taffy);
            }
        }
        for node in stale {
            let r = self.taffy.mark_dirty(node);
            self.checked("mark text for layout", r);
        }
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, time::Instant};

    use celestina_paint::TextStyle;

    use celestina_paint::Theme;

    use super::*;
    use crate::{
        key::Key,
        testing::{body, card, card_with_rows, fake_measure, id, row_key, row_view, AVAILABLE},
        view,
    };

    fn assert_rect(got: Rect, want: (f32, f32, f32, f32)) {
        assert_eq!((got.x, got.y, got.w, got.h), want);
    }

    #[test]
    fn column_of_rows_in_a_padded_card() {
        let (tree, card, rows) = card_with_rows();
        assert_rect(tree.rect(card), (0.0, 0.0, 300.0, 160.0));
        assert_rect(tree.rect(rows[0]), (20.0, 20.0, 260.0, 40.0));
        assert_rect(tree.rect(rows[1]), (20.0, 60.0, 260.0, 40.0));
        assert_rect(tree.rect(rows[2]), (20.0, 100.0, 260.0, 40.0));
    }

    #[test]
    fn nested_rects_are_absolute() {
        let mut tree = Tree::new(false, &Theme::dark());
        let rows = (0..3).map(|i| {
            if i == 1 {
                row_view(i).child(view::col(Key::named("inner")).size(10.0, 10.0))
            } else {
                row_view(i)
            }
        });
        tree.apply(card(rows), Instant::now());
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_rect(
            tree.rect(id(&tree, Key::named("inner"))),
            (20.0, 60.0, 10.0, 10.0),
        );
    }

    #[test]
    fn text_is_measured_and_relaid_out_on_a_text_change() {
        let t0 = Instant::now();
        let label = |s: &str| {
            view::row(Key::named("row")).child(view::text(Key::named("label"), s, body()))
        };
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(label("Isla"), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let (row, text) = (id(&tree, Key::named("row")), id(&tree, Key::named("label")));
        assert_rect(tree.rect(text), (0.0, 0.0, 28.0, 16.0));
        assert_rect(tree.rect(row), (0.0, 0.0, 28.0, 16.0));

        tree.apply(label("Espacios"), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(tree.node_by_key(Key::named("label")), Some(text));
        assert_rect(tree.rect(text), (0.0, 0.0, 56.0, 16.0));

        tree.apply(label(""), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_rect(tree.rect(text), (0.0, 0.0, 0.0, 16.0));
    }

    #[test]
    fn the_measure_gets_the_text_s_max_width() {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(
            view::text(Key::named("t"), "abc", body()).max_width(90.0),
            Instant::now(),
        );
        let seen = Cell::new(None);
        tree.layout_measured(AVAILABLE, |s: &str, st: &TextStyle, max: Option<f32>| {
            seen.set(max);
            fake_measure(s, st, max)
        });
        assert_eq!(seen.get(), Some(90.0));
    }

    #[test]
    fn icons_take_their_size() {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(
            view::row(Key::named("r")).child(view::icon(Key::named("i"), "bell", 16.0)),
            Instant::now(),
        );
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_rect(
            tree.rect(id(&tree, Key::named("i"))),
            (0.0, 0.0, 16.0, 16.0),
        );
    }

    #[test]
    fn layout_is_cached_until_dirty() {
        let t0 = Instant::now();
        let calls = Cell::new(0);
        let counting = |s: &str, st: &TextStyle, m: Option<f32>| {
            calls.set(calls.get() + 1);
            fake_measure(s, st, m)
        };
        let with_text = |s: &str| {
            card((0..3).map(|i| {
                if i == 0 {
                    row_view(i).child(view::text(Key::named("x"), s, body()))
                } else {
                    row_view(i)
                }
            }))
        };
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(with_text("x"), t0);
        tree.layout_measured(AVAILABLE, counting);
        let after_first = calls.get();
        assert!(after_first > 0);
        tree.layout_measured(AVAILABLE, counting);
        assert_eq!(calls.get(), after_first, "a clean tree must not re-measure");
        tree.apply(with_text("xy"), t0);
        tree.layout_measured(AVAILABLE, counting);
        assert!(calls.get() > after_first);
    }

    #[test]
    fn a_new_available_size_relayouts_a_clean_tree() {
        let mut tree = Tree::new(false, &Theme::dark());
        let mut root = view::col(Key::named("root"));
        root.style.size.width = taffy::prelude::percent(1.0);
        tree.apply(root, Instant::now());
        let root = id(&tree, Key::named("root"));
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(tree.rect(root).w, 760.0);
        tree.layout_measured((400.0, 400.0), fake_measure);
        assert_eq!(tree.rect(root).w, 400.0);
    }

    #[test]
    fn a_style_change_relayouts() {
        let (mut tree, card_id, rows) = card_with_rows();
        let t0 = Instant::now();
        tree.apply(
            card((0..3).map(|i| {
                if i == 2 {
                    row_view(i).height(80.0)
                } else {
                    row_view(i)
                }
            })),
            t0,
        );
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_rect(tree.rect(rows[2]), (20.0, 100.0, 260.0, 80.0));
        assert_rect(tree.rect(card_id), (0.0, 0.0, 300.0, 200.0));
    }

    #[test]
    fn a_removed_child_leaves_the_layout_at_once() {
        let (mut tree, card_id, rows) = card_with_rows();
        let t0 = Instant::now();
        tree.apply(card([row_view(0), row_view(2)]), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        // The leaving row keeps its last rect while it fades.
        assert_rect(tree.rect(rows[1]), (20.0, 60.0, 260.0, 40.0));
        assert_rect(tree.rect(rows[2]), (20.0, 60.0, 260.0, 40.0));
        assert_rect(tree.rect(card_id), (0.0, 0.0, 300.0, 120.0));
    }

    #[test]
    fn a_child_moving_with_its_parent_does_not_slide_twice() {
        let t0 = Instant::now();
        let with_inner = |order: [u64; 3]| {
            card(order.map(|i| {
                if i == 1 {
                    row_view(i).child(view::col(Key::named("inner")).size(10.0, 10.0))
                } else {
                    row_view(i)
                }
            }))
        };
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(with_inner([0, 1, 2]), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        tree.apply(with_inner([0, 2, 1]), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let row = id(&tree, row_key(1));
        let inner = id(&tree, Key::named("inner"));
        assert_eq!(tree.offset(row, t0), (0.0, -40.0));
        assert_eq!(tree.offset(inner, t0), (0.0, 0.0));
    }

    #[test]
    fn a_resize_without_apply_does_not_slide() {
        let mut tree = Tree::new(false, &Theme::dark());
        let t0 = Instant::now();
        tree.apply(
            view::col(Key::named("root")).size(100.0, 100.0).child(
                view::col(Key::named("a"))
                    .absolute(10.0, 10.0)
                    .size(5.0, 5.0),
            ),
            t0,
        );
        tree.layout_measured(AVAILABLE, fake_measure);
        tree.layout_measured((300.0, 300.0), fake_measure);
        assert_eq!(tree.offset(id(&tree, Key::named("a")), t0), (0.0, 0.0));
    }

    #[test]
    fn revealing_or_hiding_a_subtree_does_not_slide() {
        let t0 = Instant::now();
        // The last row, so hiding it moves no sibling.
        let with_last = |hidden: bool| {
            card((0..3).map(|i| {
                if i == 2 {
                    row_view(i)
                        .child(view::col(Key::named("inner")).size(10.0, 10.0))
                        .hidden(hidden)
                } else {
                    row_view(i)
                }
            }))
        };
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(with_last(false), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        tree.apply(with_last(true), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert!(!tree.is_animating(t0), "hiding animates");
        tree.apply(with_last(false), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        let (row, inner) = (id(&tree, row_key(2)), id(&tree, Key::named("inner")));
        assert_rect(tree.rect(row), (20.0, 100.0, 260.0, 40.0));
        assert_eq!(tree.offset(row, t0), (0.0, 0.0));
        assert_eq!(tree.offset(inner, t0), (0.0, 0.0));
        assert!(!tree.is_animating(t0), "revealing slides");

        // Hidden from the start, then revealed.
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(with_last(true), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        tree.apply(with_last(false), t0);
        tree.layout_measured(AVAILABLE, fake_measure);
        assert_eq!(tree.offset(id(&tree, row_key(2)), t0), (0.0, 0.0));
        assert!(!tree.is_animating(t0));
    }
}
