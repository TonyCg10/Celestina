//! The bar's zone capsules on the Island's wide surface (parity spec §6):
//! their tree, its pointer state and the pieces their input goes to. What
//! the zones hold and where they sit is [`crate::bar`]'s; this is where the
//! capsules are measured, placed, painted and hit.
//!
//! The zone views are measured in a tree of their own that is never painted
//! (a capsule is as wide as its pieces), then placed by
//! [`crate::bar::layout::place`] around the Island's current width and
//! applied, each at its rect and clipped to it, to the tree that is painted.
//! A new placement jumps: a capsule never slides, so what the regions cover
//! is what is painted at every frame, and a centre capsule keeps its gap to
//! a growing Island. A rebuild that moves no capsule lets the pieces inside
//! move as the tree moves them, and a press's recoil run on.
//!
//! The one piece a capsule sheet is open on is lifted out of its capsule
//! ([`BarTree::set_lifted`]): its node is hidden, so its slot closes and the
//! pieces beside it slide together on the open spring while the capsule's
//! glass, regions and anchor glide from the extent it had to the one it
//! has ([`BarTree::capsule_extent`]) on the same spring, so the glass and the
//! pieces in it stay one shape; landing it reopens the slot the same way.
//!
//! The piece does not vanish with its slot (CEL-D-010): while it leaves it
//! fades out where it was, blurred and sinking, and when it lands it fades
//! back in at its place, each on a clock of [`Fades`] that the paint reads.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Theme};
use celestina_shell::settings::BarLayout;
use celestina_ui::{
    view, Easing, Highlight, Interaction, Key, Motion, Tree, TreeError, UiEvent, View, BTN_LEFT,
};
use celestina_wayland::InputEvent;

use super::{
    piece_fade::{self, Fades},
    state::{CAPSULE_H, TOP_GAP},
    view::{centred, CAPSULE_PADDING},
    ScrollRemainder, BTN_MIDDLE, BTN_RIGHT,
};
use crate::{
    bar::{
        layout::{place, Placement, ZoneWidths},
        silhouette::Shape,
        view::{zone_key, ZoneViews, Zones},
    },
    pieces::{self, Axis, BarCx, Gesture, PieceAction, PieceInput},
    tray::scroll_steps,
};

/// The bar's band of the surface: the capsules and the gap above them.
const BAR_H: f32 = TOP_GAP + CAPSULE_H;
/// Room enough for any zone: the measuring tree's capsules take their
/// content's width, never the room's.
const MEASURE_ROOM: (f32, f32) = (65_536.0, 65_536.0);

/// The four zone keys, left to right.
fn zone_keys() -> [Key; 4] {
    [zone_key(0), zone_key(1), zone_key(2), zone_key(3)]
}

/// The key of the painted tree's root.
fn root_key() -> Key {
    Key::named("bar-root")
}

/// The key of the measuring tree's root.
fn measure_key() -> Key {
    Key::named("bar-measure")
}

/// Every key the bar draws itself, around the pieces' views. The bar finds
/// its capsules by key, so no piece may draw one of these (the pieces' key
/// test holds the table to it).
#[cfg(test)]
pub(crate) fn own_keys() -> Vec<Key> {
    let mut keys = vec![root_key(), measure_key()];
    keys.extend(zone_keys());
    keys
}

/// What a pointer event on the bar asked of its pieces.
#[derive(Debug, Default, PartialEq)]
pub struct BarInput {
    /// Each gesture on a piece's node, with what the piece asked for
    /// (`PieceAction::None` included: a click that asked for nothing is
    /// still a click).
    pub asked: Vec<(Key, Gesture, PieceAction)>,
    /// Whether the event landed on a piece's node rather than on a
    /// capsule's empty stretch.
    pub on_piece: bool,
}

/// One output's bar: the layout in force, its zones' views, their measured
/// widths, where they sit, and the painted tree with its pointer state.
#[derive(Debug)]
pub struct BarTree {
    layout: BarLayout,
    zones: Zones,
    views: ZoneViews,
    /// The zones' content widths (the Island's is filled in per placement).
    widths: ZoneWidths,
    placement: Option<Placement>,
    tree: Tree,
    measure: Tree,
    input: Interaction,
    reduced: bool,
    /// The views must be built again before the next placement.
    stale: bool,
    /// The wheel's rest, per node (the struct's `row` is the node).
    scroll: ScrollRemainder,
    /// The piece out of its capsule: its node is hidden and its slot closed.
    lifted: Option<Key>,
    /// The lifted piece changed since the last update: the slides it starts
    /// run on the open spring.
    lift_changed: bool,
    /// The open spring, from the theme: how long and on which curve.
    spring: (Duration, Easing),
    /// The gap the row keeps between pieces, from the theme.
    gap: f32,
    /// Each capsule's glide from the extent it was painted at to its rect,
    /// while its pieces slide.
    glides: [Option<Glide>; 4],
    /// The pieces fading out of the bar or back into it.
    fades: Fades,
    /// The width each leaving piece had in its row when it left: what its
    /// share of a room shared with a neighbour that also fades is made of.
    fade_widths: Vec<(Key, f32)>,
}

/// A capsule on its way from `from` to the rect it was placed at.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: Rect,
    start: Instant,
}

/// Whether two capsule rects are one to the eye (0.01 px).
fn same_extent(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.w - b.w).abs() < 0.01
}

/// The painted tree: its hit-test follows the slides of the pieces, so a press
/// lands on what is drawn while the row closes over a slot.
fn bar_tree(reduced: bool, theme: &Theme) -> Tree {
    let mut tree = Tree::new(reduced, theme);
    tree.set_hit_painted(true);
    tree
}

/// The height of a piece's pill when the bar has no rect for it.
const PILL_H: f32 = 28.0;

/// The grid a fading piece's blur layer snaps its edges to, logical px.
const LAYER_GRID: f32 = 32.0;

/// The open spring of `theme`: `motion.slow` with the overshoot easing.
fn spring_of(theme: &Theme) -> (Duration, Easing) {
    (
        Motion::Slow.duration(&theme.motion),
        Easing::OutBack(theme.overshoot),
    )
}

impl BarTree {
    /// A bar with `layout`, drawn on `theme`, under `reduced` motion.
    #[must_use]
    pub fn new(layout: &BarLayout, theme: &Theme, reduced: bool) -> BarTree {
        BarTree {
            layout: layout.clone(),
            zones: Zones::new(),
            views: ZoneViews::default(),
            widths: ZoneWidths::default(),
            placement: None,
            tree: bar_tree(reduced, theme),
            measure: Tree::new(true, theme),
            input: Interaction::new(),
            reduced,
            stale: true,
            scroll: ScrollRemainder::default(),
            lifted: None,
            lift_changed: false,
            spring: spring_of(theme),
            gap: theme.spacing.sm,
            glides: [None; 4],
            fades: Fades::default(),
            fade_widths: Vec::new(),
        }
    }

    /// Lifts `piece` out of its capsule (`None` lands the one lifted): its
    /// slot closes, or reopens, and the pieces beside it slide on the open
    /// spring at the next update, which is where the slides start (`_now` is
    /// when it was asked). A piece the bar does not hold is lifted from
    /// nothing, and lands again with the sheet. The piece lifted fades out
    /// from `now` and the one landed fades back in, each from the opacity it
    /// has when it is already fading.
    pub fn set_lifted(&mut self, piece: Option<Key>, now: Instant) {
        if piece != self.lifted {
            if let Some(landed) = self.lifted {
                self.fades.start(landed, true, now);
            }
            if let Some(leaving) = piece {
                self.fades.start(leaving, false, now);
                let width = self
                    .tree
                    .node_by_key(leaving)
                    .map_or(0.0, |id| self.tree.rect(id).w);
                self.fade_widths.retain(|(k, _)| *k != leaving);
                self.fade_widths.push((leaving, width));
            }
            self.lifted = piece;
            self.lift_changed = true;
            self.stale = true;
        }
    }

    /// The piece out of its capsule, if one is.
    #[must_use]
    pub fn lifted(&self) -> Option<Key> {
        self.lifted
    }

    /// The layout in force.
    #[must_use]
    pub fn layout(&self) -> &BarLayout {
        &self.layout
    }

    /// Puts `layout` in force; returns whether it changed. A new layout is a
    /// new generation: its unknown ids are reported again.
    pub fn set_layout(&mut self, layout: &BarLayout) -> bool {
        if *layout == self.layout {
            return false;
        }
        self.layout = layout.clone();
        self.zones.new_generation();
        self.stale = true;
        true
    }

    /// Something the pieces read changed: the views are built again.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// Whether the views are built again at the next update (tests).
    #[cfg(test)]
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    pub fn set_theme(&mut self, theme: &Theme) {
        self.tree.set_theme(theme);
        self.measure.set_theme(theme);
        self.spring = spring_of(theme);
        self.gap = theme.spacing.sm;
        self.stale = true;
    }

    pub fn set_reduced_motion(&mut self, on: bool) {
        if on != self.reduced {
            self.reduced = on;
            self.tree.set_reduced_motion(on);
        }
    }

    /// `base` with the hidden tray ids of the layout in force: the context
    /// the pieces are built and asked with.
    fn cx<'a>(&'a self, base: &BarCx<'a>) -> BarCx<'a> {
        BarCx {
            tray_hidden: &self.layout.tray_hidden,
            ..*base
        }
    }

    /// Builds the views again if something changed, measures them, places
    /// them around an Island `island_w` wide on a surface `surface_w` wide,
    /// and lays the painted tree out. `lay` lays a tree out in a room (the
    /// painter's text system in production, a fixed measure in tests).
    pub fn update(
        &mut self,
        base: &BarCx<'_>,
        surface_w: f32,
        island_w: f32,
        now: Instant,
        lay: &mut dyn FnMut(&mut Tree, (f32, f32)),
    ) {
        let rebuilt = self.stale;
        let lift = std::mem::take(&mut self.lift_changed);
        if self.stale {
            let cx = BarCx {
                tray_hidden: &self.layout.tray_hidden,
                ..*base
            };
            self.views = self.zones.build(&self.layout, &cx);
            if let Some(key) = self.lifted {
                self.views.hide(key);
            }
            self.measure_views(now, lay);
            self.stale = false;
            // A piece that vanished takes its fade with it.
            let views = &self.views;
            self.fades.settle(now, |key| views.owner(key).is_some());
        }
        let placement = place(
            base.theme,
            surface_w,
            ZoneWidths {
                island: island_w,
                ..self.widths
            },
        );
        let moved = self.placement != Some(placement);
        if rebuilt || moved {
            // A new placement jumps, so nothing slides out from under its
            // region; with the same one, the pieces inside move as the tree
            // moves them and a press's recoil runs on. A piece lifted or
            // landed moves the placement too, but there the capsules still
            // jump while the pieces inside slide on the open spring from
            // where they were drawn, and the capsules' glass follows them.
            // One that comes while a lift's glide still runs is carried as a
            // lift: the capsules jump and glide again from where they are
            // drawn, so their glass and regions stay on their pieces.
            let gliding = (0..4).any(|zone| self.is_sliding(zone, now));
            let carry = lift || (moved && gliding);
            let jump = moved && !carry;
            if jump {
                self.tree.set_reduced_motion(true);
                self.glides = [None; 4];
            }
            let before: [Option<Rect>; 4] =
                std::array::from_fn(|zone| self.capsule_extent(zone, now));
            if carry {
                let (duration, easing) = self.spring;
                self.tree.slide_with(duration, easing, &zone_keys());
            }
            self.tree.apply(self.placed(&placement, surface_w), now);
            lay(&mut self.tree, (surface_w, BAR_H));
            if jump {
                self.tree.set_reduced_motion(self.reduced);
            }
            if carry && !self.reduced {
                // The capsules glide on the pieces' spring, from where they
                // were drawn to where they are now.
                for (zone, from) in before.into_iter().enumerate() {
                    self.glides[zone] = from
                        .zip(self.zone_rect(zone))
                        .filter(|(from, to)| !same_extent(*from, *to))
                        .map(|(from, _)| Glide { from, start: now });
                }
            }
            self.placement = Some(placement);
        } else {
            // A no-op unless the device ratio changed.
            lay(&mut self.tree, (surface_w, BAR_H));
        }
    }

    /// The content width of every zone that has a capsule.
    fn measure_views(&mut self, now: Instant, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        let views = [
            &self.views.start,
            &self.views.centre_left,
            &self.views.centre_right,
            &self.views.end,
        ];
        let probe = centred(view::col(measure_key()))
            .children(views.iter().filter_map(|v| v.as_ref().cloned()));
        self.measure.apply(probe, now);
        lay(&mut self.measure, MEASURE_ROOM);
        let width = |view: &Option<View>, key: Key| {
            if view.is_none() {
                return 0.0;
            }
            self.measure
                .node_by_key(key)
                .map_or(0.0, |id| self.measure.rect(id).w)
        };
        let [start, centre_left, centre_right, end] = zone_keys();
        self.widths = ZoneWidths {
            start: width(&self.views.start, start),
            centre_left: width(&self.views.centre_left, centre_left),
            island: 0.0,
            centre_right: width(&self.views.centre_right, centre_right),
            end: width(&self.views.end, end),
        };
    }

    /// The bar's view: every zone that has a capsule and a rect, at that
    /// rect and clipped to it.
    fn placed(&self, placement: &Placement, surface_w: f32) -> View {
        let at = |view: &Option<View>, rect: Option<Rect>| match (view, rect) {
            (Some(v), Some(r)) => Some(v.clone().absolute(r.x, r.y).width(r.w).clip()),
            _ => None,
        };
        view::row(root_key()).size(surface_w, BAR_H).children(
            [
                at(&self.views.start, placement.start),
                at(&self.views.centre_left, placement.centre_left),
                at(&self.views.centre_right, placement.centre_right),
                at(&self.views.end, placement.end),
            ]
            .into_iter()
            .flatten(),
        )
    }

    /// Where the capsules sit since the last update; `None` before it.
    #[must_use]
    pub fn placement(&self) -> Option<Placement> {
        self.placement
    }

    /// The first layout fault of either tree since the last call.
    pub fn take_fault(&mut self) -> Option<TreeError> {
        self.tree.take_fault().or_else(|| self.measure.take_fault())
    }

    /// Paints the bar, every capsule. A piece that is fading (out of its
    /// capsule or back into it) is left out of the walk and painted after
    /// it, at its opacity, sunk and blurred as far as it has left.
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        self.sync_extents(now);
        let views = &self.views;
        self.fades.settle(now, |key| views.owner(key).is_some());
        let fading = self.fades.active(now);
        let apart: Vec<Key> = fading.iter().map(|(key, _)| *key).collect();
        self.tree.paint_hiding(painter, now, &apart);
        for &(key, opacity) in &fading {
            self.paint_fading(painter, now, key, opacity, &fading);
        }
    }

    /// The room piece `key` has in its capsule at `now`, and how far its
    /// content must move to sit in it: the gap its steady neighbours leave as
    /// they slide (less the row gap each keeps), clipped to the capsule as
    /// painted. Pieces side by side that both fade (a switch between
    /// neighbours) share that gap in proportion to their width and opacity,
    /// in row order, so the one that leaves gives way to the one that comes.
    /// The piece shows through its share only, as through the slot that
    /// closes and reopens around it.
    fn live_slot(&self, key: Key, now: Instant, fading: &[(Key, f32)]) -> Option<(Rect, f32)> {
        let id = self.tree.node_by_key(key)?;
        let parent = self.tree.parent_of(id)?;
        let kids = self.tree.painted_children(parent, now);
        let i = kids.iter().position(|k| k.0 == key)?;
        let rect = kids[i].1;
        let capsule = self
            .zone_of(key)
            .and_then(|zone| self.capsule_extent(zone, now))
            .unwrap_or_else(|| self.tree.rect(parent));
        let opacity = |k: Key| fading.iter().find(|f| f.0 == k).map(|f| f.1);
        let steady = |k: &&(Key, Rect, bool)| !k.2 && opacity(k.0).is_none();
        let p = kids[..i].iter().rposition(|k| steady(&k));
        let n = kids[i + 1..]
            .iter()
            .position(|k| steady(&k))
            .map(|n| n + i + 1);
        let left = p.map_or(capsule.x + CAPSULE_PADDING, |p| {
            kids[p].1.x + kids[p].1.w + self.gap
        });
        let right = n.map_or(capsule.x + capsule.w - CAPSULE_PADDING, |n| {
            kids[n].1.x - self.gap
        });
        let room = (right - left).max(0.0);
        // The fading pieces between the steady neighbours, in row order, each
        // weighted by its width and how much of it shows.
        let width = |k: &(Key, Rect, bool)| {
            if k.1.w > 0.0 {
                k.1.w
            } else {
                self.fade_widths
                    .iter()
                    .find(|w| w.0 == k.0)
                    .map_or(0.0, |w| w.1)
            }
        };
        let run: Vec<(Key, f32, f32)> = kids[p.map_or(0, |p| p + 1)..n.unwrap_or(kids.len())]
            .iter()
            .filter_map(|k| opacity(k.0).map(|o| (k.0, width(k), width(k) * o)))
            .collect();
        let total: f32 = run.iter().map(|r| r.2).sum();
        let mut x = left;
        let mut share = (left, if run.len() <= 1 { room } else { 0.0 });
        if run.len() > 1 && total > 0.0 {
            for r in &run {
                let w = room * r.2 / total;
                if r.0 == key {
                    share = (x, w);
                    break;
                }
                x += w;
            }
        }
        let own_w = run.iter().find(|r| r.0 == key).map_or(rect.w, |r| r.1);
        let shown = share.1.clamp(0.0, own_w.max(0.0));
        let centre = share.0 + share.1 / 2.0;
        let clip = Rect {
            x: centre - shown / 2.0,
            y: capsule.y,
            w: shown,
            h: capsule.h,
        };
        // Never outside the capsule.
        let x0 = clip.x.max(capsule.x);
        let x1 = (clip.x + clip.w).min(capsule.x + capsule.w);
        let clip = Rect {
            x: x0,
            w: (x1 - x0).max(0.0),
            ..clip
        };
        Some((clip, centre - (rect.x + rect.w / 2.0)))
    }

    /// The inside of piece `key` at `opacity`: in its slot and cut to it,
    /// moved down, drawn through the blur and faded as a whole, without its
    /// own plate. Under reduced motion it only fades.
    fn paint_fading(
        &mut self,
        painter: &mut Painter,
        now: Instant,
        key: Key,
        opacity: f32,
        fading: &[(Key, f32)],
    ) {
        let Some(id) = self.tree.node_by_key(key) else {
            return;
        };
        if opacity <= 0.0 {
            return;
        }
        let Some((clip, shift)) = self.live_slot(key, now, fading) else {
            return;
        };
        if clip.w <= 0.0 || clip.h <= 0.0 {
            return;
        }
        let (blur, drop) = piece_fade::look(opacity, self.reduced);
        // The layer holds the piece where the fade puts it and the blur's
        // reach, its edges on a 32 px grid so that its size, and so its
        // images, change only when it crosses a grid line.
        let own = self.tree.painted_rect(id, now);
        let reach = 3.0 * piece_fade::BLUR_PX;
        let grid = |v: f32, up: bool| {
            let g = (v / LAYER_GRID).floor() * LAYER_GRID;
            if up && g < v {
                g + LAYER_GRID
            } else {
                g
            }
        };
        let (x0, x1) = (
            grid(own.x + shift - reach, false),
            grid(own.x + shift + own.w + reach, true),
        );
        let layer = Rect {
            x: x0,
            y: clip.y - reach,
            w: x1 - x0,
            h: clip.h + 2.0 * reach,
        };
        let tree = &mut self.tree;
        // The slot cuts the blurred piece, not the piece before its blur, so
        // nothing of it shows past the slot or the capsule.
        painter.push_state();
        painter.opacity(opacity);
        painter.push_clip(clip, 0.0);
        painter.blurred(layer, blur, &mut |p| {
            p.push_state();
            p.translate(shift, drop);
            tree.paint_inside(p, now, id);
            p.pop_state();
        });
        painter.pop_clip();
        painter.pop_state();
    }

    /// Hands the tree the extents the capsules are painted at `now`.
    fn sync_extents(&mut self, now: Instant) {
        for (zone, key) in zone_keys().into_iter().enumerate() {
            let extent = self
                .capsule_extent(zone, now)
                .filter(|e| self.zone_rect(zone) != Some(*e));
            self.tree.set_extent(key, extent);
        }
    }

    fn zone_node(&self, zone: usize) -> Option<celestina_ui::NodeId> {
        self.tree.node_by_key(*zone_keys().get(zone)?)
    }

    /// The rect of zone `zone`'s capsule, when it has one.
    #[must_use]
    pub fn zone_rect(&self, zone: usize) -> Option<Rect> {
        self.zone_node(zone).map(|id| self.tree.rect(id))
    }

    /// The capsule of zone `zone` as it is painted at `now`: its rect at
    /// rest, and while its pieces slide (a piece lifted out or landed) its
    /// edges glide from the extent it had to its rect on the pieces' spring.
    /// What its glass, regions and sheet follow.
    #[must_use]
    pub fn capsule_extent(&self, zone: usize, now: Instant) -> Option<Rect> {
        let rest = self.zone_rect(zone)?;
        let Some(g) = self.glides.get(zone).copied().flatten() else {
            return Some(rest);
        };
        if !self.gliding(&g, now) {
            return Some(rest);
        }
        let (duration, easing) = self.spring;
        let t = now.saturating_duration_since(g.start).as_secs_f32() / duration.as_secs_f32();
        let k = easing.eval(t.clamp(0.0, 1.0));
        let edge = |from: f32, to: f32| from + (to - from) * k;
        let left = edge(g.from.x, rest.x);
        let right = edge(g.from.x + g.from.w, rest.x + rest.w);
        Some(Rect {
            x: left,
            w: (right - left).max(0.0),
            ..rest
        })
    }

    /// Whether the capsule of zone `zone` is on its glide at `now`, so that
    /// its painted extent is not its rect.
    #[must_use]
    pub fn is_sliding(&self, zone: usize, now: Instant) -> bool {
        self.glides
            .get(zone)
            .copied()
            .flatten()
            .is_some_and(|g| self.gliding(&g, now))
    }

    /// The id of the piece that drew node `key`, lifted or not.
    #[must_use]
    pub fn piece_id_of(&self, key: Key) -> Option<&str> {
        self.views.owner(key)
    }

    /// The zone whose capsule holds node `key`, lifted or not.
    #[must_use]
    pub fn zone_of(&self, key: Key) -> Option<usize> {
        self.views.owner(key)?;
        let zones = zone_keys();
        let mut at = self.tree.node_by_key(key)?;
        loop {
            let found = zones.iter().position(|&z| z == self.tree.key_of(at));
            if found.is_some() {
                return found;
            }
            at = self.tree.parent_of(at)?;
        }
    }

    /// Where the slot of piece `key` is at `now`, as a rect with no width:
    /// the centre of the piece while it is in its row, and, once lifted, of
    /// the gap its neighbours close over it (they slide, so it moves with
    /// them; with no neighbour, the capsule's end or centre). `None` once
    /// the bar does not hold the piece.
    #[must_use]
    pub fn slot_rect(&self, key: Key, now: Instant) -> Option<Rect> {
        self.views.owner(key)?;
        let id = self.tree.node_by_key(key)?;
        let parent = self.tree.parent_of(id)?;
        let kids = self.tree.painted_children(parent, now);
        let i = kids.iter().position(|k| k.0 == key)?;
        let (_, rect, hidden) = kids[i];
        let x = if hidden {
            let prev = kids[..i].iter().rev().find(|k| !k.2);
            let next = kids[i + 1..].iter().find(|k| !k.2);
            let host = self
                .zone_of(key)
                .and_then(|zone| self.capsule_extent(zone, now))
                .unwrap_or_else(|| self.tree.painted_rect(parent, now));
            match (prev, next) {
                (Some(p), Some(n)) => (p.1.x + p.1.w + n.1.x) / 2.0,
                // At an end of the row, the middle of what the piece took
                // beside its neighbour (its own rect is where it was), so
                // the slot starts where the piece was.
                (Some(p), None) => (p.1.x + p.1.w + rect.x + rect.w) / 2.0,
                (None, Some(n)) => (rect.x + n.1.x) / 2.0,
                (None, None) => host.x + host.w / 2.0,
            }
        } else {
            rect.x + rect.w / 2.0
        };
        Some(Rect {
            x,
            y: rect.y,
            w: 0.0,
            h: rect.h,
        })
    }

    /// The pill of piece `key` at `now`: what a window grows from and closes
    /// towards. While the piece is in its row at rest it is the piece's rect
    /// (its radius is half its height). Its slot, centred where
    /// [`BarTree::slot_rect`] says, is a pill as high as the piece and as wide
    /// as the gap its neighbours leave, never narrower than it is high: a
    /// square while the slot is shut, the piece's rect once it has reopened.
    /// `None` once the bar does not hold the piece.
    #[must_use]
    pub fn pill_of(&self, key: Key, now: Instant) -> Option<Rect> {
        let slot = self.slot_rect(key, now)?;
        let id = self.tree.node_by_key(key)?;
        let parent = self.tree.parent_of(id)?;
        let kids = self.tree.painted_children(parent, now);
        let i = kids.iter().position(|k| k.0 == key)?;
        let (_, rect, hidden) = kids[i];
        let h = if rect.h > 0.0 { rect.h } else { PILL_H };
        let own = if rect.w > 0.0 { rect.w } else { h };
        // The room between the nearest neighbours, less the gap each keeps;
        // a piece at an end of its capsule has the capsule's inner edge
        // there instead.
        let capsule = self
            .zone_of(key)
            .and_then(|zone| self.capsule_extent(zone, now))
            .unwrap_or_else(|| self.tree.rect(parent));
        let room = {
            let prev = kids[..i].iter().rev().find(|k| !k.2);
            let next = kids[i + 1..].iter().find(|k| !k.2);
            let left = prev.map_or(capsule.x + CAPSULE_PADDING, |p| p.1.x + p.1.w + self.gap);
            let right = next.map_or(capsule.x + capsule.w - CAPSULE_PADDING, |n| {
                n.1.x - self.gap
            });
            right - left
        };
        let w = if hidden { h } else { room.clamp(h, own.max(h)) };
        Some(Rect {
            x: slot.x - w / 2.0,
            y: rect.y,
            w,
            h,
        })
    }

    /// The slot clip and shift `piece` is painted with while it fades, and
    /// the painted rects of the pieces beside it in its row (tests).
    #[cfg(test)]
    #[must_use]
    pub fn fade_slot(&self, piece: Key, now: Instant) -> Option<(Rect, f32, Vec<Rect>)> {
        self.fades.value(piece, now)?;
        let fading = self.fades.active(now);
        let (clip, shift) = self.live_slot(piece, now, &fading)?;
        let id = self.tree.node_by_key(piece)?;
        let parent = self.tree.parent_of(id)?;
        let beside = self
            .tree
            .painted_children(parent, now)
            .into_iter()
            .filter(|k| k.0 != piece && !k.2)
            .map(|k| k.1)
            .collect();
        Some((clip, shift, beside))
    }

    /// The opacity, blur and drop `piece` is painted with while it fades
    /// (tests).
    #[cfg(test)]
    #[must_use]
    pub fn fade_of(&self, piece: Key, now: Instant) -> Option<(f32, f32, f32)> {
        let k = self.fades.value(piece, now)?;
        let (blur, drop) = piece_fade::look(k, self.reduced);
        Some((k, blur, drop))
    }

    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now)
            || self.glides.iter().flatten().any(|g| self.gliding(g, now))
            || self.fades.is_animating(now)
    }

    /// Whether glide `g` still runs at `now`.
    fn gliding(&self, g: &Glide, now: Instant) -> bool {
        now.saturating_duration_since(g.start) < self.spring.0
    }

    /// The key of the node the pointer is over (tests).
    #[cfg(test)]
    #[must_use]
    pub fn hovered_key(&self) -> Option<Key> {
        self.tree.highlight().hovered.map(|id| self.tree.key_of(id))
    }

    #[must_use]
    pub fn highlight(&self) -> Highlight {
        self.tree.highlight()
    }

    /// The capsules as they are painted at `now` (fading ones included), with
    /// the haze's corner `radius`: what the blur and input regions cover.
    #[must_use]
    pub fn shapes(&self, now: Instant, radius: f32) -> Vec<Shape> {
        (0..zone_keys().len())
            .filter(|&zone| {
                self.zone_node(zone)
                    .is_some_and(|id| self.tree.effective_opacity(id, now) > 0.0)
            })
            .filter_map(|zone| self.capsule_extent(zone, now))
            .filter(|r| r.w > 0.0 && r.h > 0.0)
            .map(|r| (r, radius))
            .collect()
    }

    /// Whether (`x`, `y`) is on one of the capsules, as painted at `now`.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32, now: Instant) -> bool {
        (0..zone_keys().len()).any(|zone| {
            self.capsule_extent(zone, now)
                .is_some_and(|r| r.contains(x, y))
        })
    }

    /// The rect of the node `key` of a piece the bar still holds: `None`
    /// once the piece is off the bar, though its node may still be fading
    /// out of the tree.
    #[must_use]
    pub fn piece_rect(&self, key: Key) -> Option<Rect> {
        self.views.owner(key)?;
        self.node_rect(key)
    }

    /// The rect of the bar's node `key`, for a popup anchored to it.
    #[must_use]
    pub fn node_rect(&self, key: Key) -> Option<Rect> {
        let id = self.tree.node_by_key(key)?;
        Some(self.tree.rect(id))
    }

    /// The pointer left the bar: its hover and press end.
    pub fn leave(&mut self, now: Instant) {
        self.input
            .pointer(&mut self.tree, &InputEvent::PointerLeave, now);
    }

    /// A pointer event on the bar: hover and press through the tree's
    /// interaction, then what each gesture on a piece's node asks of the
    /// piece. A left click is the primary gesture; the middle and right
    /// buttons act on release, as the Tray page's rows do; the wheel goes
    /// in whole steps per node, the rest kept for the next turn.
    pub fn pointer(&mut self, ev: &InputEvent, base: &BarCx<'_>, now: Instant) -> BarInput {
        let events = self.input.pointer(&mut self.tree, ev, now);
        let hovered = self.tree.highlight().hovered.map(|id| self.tree.key_of(id));
        let mut gestures: Vec<(Key, Gesture)> = Vec::new();
        if let InputEvent::PointerRelease { button, .. } = *ev {
            let other = match button {
                BTN_MIDDLE => Some(Gesture::Middle),
                BTN_RIGHT => Some(Gesture::Context),
                _ => None,
            };
            if let (Some(key), Some(gesture)) = (hovered, other) {
                gestures.push((key, gesture));
            }
        }
        for event in events {
            match event {
                UiEvent::Click(key) => gestures.push((key, Gesture::Primary)),
                UiEvent::Scroll(key, horizontal, vertical) => {
                    gestures.extend(self.steps(key, horizontal, vertical));
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Activate(_)
                | UiEvent::Escape
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        let cx = self.cx(base);
        let asked = gestures
            .into_iter()
            .map(|(key, gesture)| (key, gesture, self.ask(key, gesture, &cx)))
            .collect();
        BarInput {
            asked,
            on_piece: hovered.is_some(),
        }
    }

    /// What piece `key` is asked by an event that did not land on the bar
    /// but is its own (the open sheet's subtitle answers as the piece): a
    /// release of the left, middle or right button is the primary, middle
    /// or context gesture; a wheel turn goes in whole steps, as over the
    /// piece. Each gesture goes through the same `activate` as the bar's.
    pub fn ask_for(
        &mut self,
        key: Key,
        ev: &InputEvent,
        base: &BarCx<'_>,
    ) -> Vec<(Key, Gesture, PieceAction)> {
        let gestures: Vec<Gesture> = match *ev {
            InputEvent::PointerRelease { button, .. } => match button {
                BTN_LEFT => vec![Gesture::Primary],
                BTN_MIDDLE => vec![Gesture::Middle],
                BTN_RIGHT => vec![Gesture::Context],
                _ => Vec::new(),
            },
            InputEvent::Axis {
                horizontal,
                vertical,
                ..
            } => self
                .steps(key, horizontal, vertical)
                .into_iter()
                .map(|(_, gesture)| gesture)
                .collect(),
            _ => Vec::new(),
        };
        let cx = self.cx(base);
        gestures
            .into_iter()
            .map(|gesture| (key, gesture, self.ask(key, gesture, &cx)))
            .collect()
    }

    /// Whether a wheel step over piece `key` asks it for something (the
    /// level pieces step their level; most pieces have no wheel action).
    #[must_use]
    pub fn takes_wheel(&self, key: Key, base: &BarCx<'_>) -> bool {
        let cx = self.cx(base);
        [1, -1].into_iter().any(|steps| {
            self.ask(key, Gesture::Scroll(Axis::Vertical, steps), &cx) != PieceAction::None
        })
    }

    /// A wheel turn of (`horizontal`, `vertical`) logical px over `key`, in
    /// whole steps per axis with the Wayland sign; the rest waits for the
    /// next turn on the same node.
    fn steps(&mut self, key: Key, horizontal: f32, vertical: f32) -> Vec<(Key, Gesture)> {
        if self.scroll.row != Some(key) {
            self.scroll = ScrollRemainder {
                row: Some(key),
                ..ScrollRemainder::default()
            };
        }
        let px = |v: f32| if v.is_finite() { v.round() as i32 } else { 0 };
        let down = scroll_steps(px(vertical), &mut self.scroll.vertical);
        let right = scroll_steps(px(horizontal), &mut self.scroll.horizontal);
        [(Axis::Vertical, down), (Axis::Horizontal, right)]
            .into_iter()
            .filter(|&(_, steps)| steps != 0)
            .map(|(axis, steps)| (key, Gesture::Scroll(axis, steps)))
            .collect()
    }

    /// What `gesture` on `key` asks of the one piece that drew that node
    /// (a capsule's own node has none, and asks for nothing).
    fn ask(&self, key: Key, gesture: Gesture, cx: &BarCx<'_>) -> PieceAction {
        let input = PieceInput { key, gesture, cx };
        self.views
            .owner(key)
            .and_then(pieces::piece)
            .map_or(PieceAction::None, |piece| piece.activate(&input))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use celestina_paint::TextStyle;
    use celestina_ui::Easing;

    use super::*;
    use crate::island::pages::NameOwner;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn lay() -> impl FnMut(&mut Tree, (f32, f32)) {
        |tree: &mut Tree, room| tree.layout_measured(room, fake_measure)
    }

    fn with_cx<R>(f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels {
                brightness: Some(70),
                ..crate::pieces::Levels::default()
            },
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
            media: &crate::island::pages::media::NO_MEDIA,
        })
    }

    fn only(start: &[&str], end: &[&str]) -> BarLayout {
        let ids = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect();
        BarLayout {
            start: ids(start),
            centre: Vec::new(),
            end: ids(end),
            tray_hidden: Vec::new(),
        }
    }

    #[test]
    fn the_capsules_are_painted_where_the_layout_places_them() {
        with_cx(|cx| {
            let t0 = Instant::now();
            let mut bar = BarTree::new(&only(&["notifications"], &[]), cx.theme, false);
            bar.update(cx, 1920.0, 140.0, t0, &mut lay());
            let placed = bar.placement.and_then(|p| p.start);
            let painted = bar.shapes(t0, cx.theme.radius_lg);
            assert_eq!(painted.len(), 1);
            assert_eq!(Some(painted[0].0), placed);
            assert!(placed
                .is_some_and(|r| r.w > 0.0 && r.x == crate::bar::layout::edge_margin(cx.theme)));
        });
    }

    #[test]
    fn a_new_placement_jumps_instead_of_sliding() {
        with_cx(|cx| {
            let t0 = Instant::now();
            let mut bar = BarTree::new(&only(&[], &["notifications"]), cx.theme, false);
            bar.update(cx, 1920.0, 140.0, t0, &mut lay());
            let before = bar.shapes(t0, 0.0);
            // A narrower surface moves the end capsule left.
            bar.update(cx, 1280.0, 140.0, t0, &mut lay());
            let after = bar.shapes(t0, 0.0);
            assert_eq!(before.len(), 1);
            assert_eq!(after.len(), 1);
            assert!(after[0].0.x < before[0].0.x);
            assert_eq!(Some(after[0].0), bar.placement.and_then(|p| p.end));
            assert!(!bar.is_animating(t0), "no slide in flight");
        });
    }

    #[test]
    fn the_wheel_goes_in_whole_steps_per_node_with_the_wayland_sign() {
        let mut bar = BarTree::new(&only(&[], &[]), &Theme::dark(), false);
        let (a, b) = (Key::named("a"), Key::named("b"));
        assert_eq!(
            bar.steps(a, 0.0, 30.0),
            [(a, Gesture::Scroll(Axis::Vertical, 2))]
        );
        assert!(bar.steps(a, 0.0, -10.0).is_empty());
        assert_eq!(
            bar.steps(a, -16.0, -10.0),
            [
                (a, Gesture::Scroll(Axis::Vertical, -1)),
                (a, Gesture::Scroll(Axis::Horizontal, -1))
            ]
        );
        // Another node starts from nothing; nonsense is no turn.
        assert!(bar.steps(b, 0.0, 14.0).is_empty());
        assert!(bar.steps(b, f32::NAN, f32::INFINITY).is_empty());
    }

    /// `layout`'s bar, built and laid out once on a 1920 px surface.
    fn built(layout: &BarLayout, cx: &BarCx<'_>) -> BarTree {
        let mut bar = BarTree::new(layout, cx.theme, false);
        bar.update(cx, 1920.0, 300.0, Instant::now(), &mut lay());
        bar
    }

    #[test]
    fn a_click_asks_the_piece_that_owns_the_node() {
        with_cx(|cx| {
            let bar = built(&BarLayout::default(), cx);
            assert_eq!(bar.views.owner(Key::named("bell")), Some("notifications"));
            assert_eq!(
                bar.ask(Key::named("bell"), Gesture::Primary, &bar.cx(cx)),
                PieceAction::OpenPage(pieces::PageId::NOTIFICATIONS)
            );
            // A node no piece drew, and a capsule's own, ask for nothing.
            for key in [Key::named("nothing"), zone_key(0)] {
                assert_eq!(
                    bar.ask(key, Gesture::Primary, &bar.cx(cx)),
                    PieceAction::None
                );
            }
            // A piece the layout does not place is not asked.
            let bare = built(&only(&["tray"], &[]), cx);
            assert_eq!(bare.views.owner(Key::named("bell")), None);
            assert_eq!(
                bare.ask(Key::named("bell"), Gesture::Primary, &bare.cx(cx)),
                PieceAction::None
            );
            // A piece placed twice and the Island's mark: the first place
            // owns the node.
            let ids = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect();
            let twice = BarLayout {
                start: ids(&["ghost"]),
                centre: ids(&["notifications", "island"]),
                end: ids(&["notifications"]),
                tray_hidden: Vec::new(),
            };
            let bar = built(&twice, cx);
            assert_eq!(bar.views.owner(Key::named("bell")), Some("notifications"));
            assert!(bar.views.end.is_none(), "placed once, in the centre");
        });
    }

    /// The start zone of three pieces (the bell, the brightness with a
    /// reading, the session), laid out at rest.
    fn trio(cx: &BarCx<'_>, reduced: bool) -> (BarTree, [Key; 3], Instant) {
        let t0 = Instant::now();
        let mut bar = BarTree::new(
            &only(&["notifications", "brightness", "session"], &[]),
            cx.theme,
            reduced,
        );
        bar.update(cx, 1920.0, 300.0, t0, &mut lay());
        let keys: Vec<Key> = bar
            .views
            .start
            .as_ref()
            .map(|v| v.children.iter().map(|c| c.key).collect())
            .unwrap_or_default();
        assert_eq!(keys.len(), 3, "the three pieces draw something");
        (bar, [keys[0], keys[1], keys[2]], t0)
    }

    /// Lifts `key` at `t0` and lays the bar out again.
    fn lift(bar: &mut BarTree, cx: &BarCx<'_>, key: Option<Key>, t0: Instant) {
        bar.set_lifted(key, t0);
        bar.update(cx, 1920.0, 300.0, t0, &mut lay());
    }

    const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    fn spring_at(theme: &Theme, t: f32) -> f32 {
        Easing::OutBack(theme.overshoot).eval(t)
    }

    fn after(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn a_lifted_piece_leaves_no_gap() {
        with_cx(|cx| {
            let (mut bar, [a, b, c], t0) = trio(cx, false);
            let gap = cx.theme.spacing.sm;
            let whole = bar.zone_rect(0).map_or(0.0, |r| r.w);
            let width = |bar: &BarTree, k| bar.node_rect(k).map_or(0.0, |r| r.w);
            let middle = width(&bar, b);
            lift(&mut bar, cx, Some(b), t0);
            let narrow = bar.zone_rect(0).map_or(0.0, |r| r.w);
            assert_eq!(narrow, whole - middle - gap);
            // The neighbours abut with the zone's gap; the middle is out of
            // the layout.
            let (ra, rc) = (bar.node_rect(a), bar.node_rect(c));
            assert!(ra.zip(rc).is_some_and(|(ra, rc)| rc.x == ra.x + ra.w + gap));
            assert_eq!(
                bar.placement().and_then(|p| p.start).map(|r| r.w),
                Some(narrow)
            );
            assert_eq!(bar.lifted(), Some(b));
            assert_eq!(
                bar.zone_of(b),
                Some(0),
                "a lifted piece is still in its zone"
            );
        });
    }

    #[test]
    fn the_row_slides_with_the_open_spring() {
        with_cx(|cx| {
            let (mut bar, [_, b, c], t0) = trio(cx, false);
            let old = bar.node_rect(c).map_or(0.0, |r| r.x);
            lift(&mut bar, cx, Some(b), t0);
            let new = bar.node_rect(c).map_or(0.0, |r| r.x);
            assert!(new < old, "the piece after the lifted one moves left");
            let id = bar.tree.node_by_key(c);
            let painted =
                |ms: u64| id.map_or(f32::NAN, |id| bar.tree.painted_rect(id, after(t0, ms)).x);
            // It starts where it was and ends where it goes; half way is the
            // open spring's, not the normal step's.
            assert!((painted(0) - old).abs() < 0.01);
            let k = spring_at(cx.theme, 0.5);
            assert!((painted(175) - (old + (new - old) * k)).abs() < 0.01);
            assert!((painted(350) - new).abs() < 0.01);
            let normal = celestina_ui::ONE_UI.eval(0.5);
            assert!((k - normal).abs() > 0.01, "the two curves differ at half");
            assert!(bar.is_animating(after(t0, 349)));
            assert!(!bar.is_animating(after(t0, 350)));
        });
    }

    #[test]
    fn the_capsule_extent_follows_its_sliding_pieces() {
        with_cx(|cx| {
            let (mut bar, [a, b, c], t0) = trio(cx, false);
            let whole = bar.zone_rect(0);
            lift(&mut bar, cx, Some(b), t0);
            let rest = bar.zone_rect(0);
            let pad = CAPSULE_PADDING;
            for ms in [0, 60, 175, 300] {
                let now = after(t0, ms);
                let shown = |k: Key| {
                    bar.tree
                        .node_by_key(k)
                        .map(|id| bar.tree.painted_rect(id, now))
                        .unwrap_or(ZERO)
                };
                let (ra, rc) = (shown(a), shown(c));
                let extent = bar.capsule_extent(0, now);
                let want = Rect {
                    x: ra.x - pad,
                    w: (rc.x + rc.w + pad) - (ra.x - pad),
                    ..whole.unwrap_or(ZERO)
                };
                assert!(
                    extent.is_some_and(|e| (e.x - want.x).abs() < 0.01 && (e.w - want.w).abs() < 0.01),
                    "+{ms} ms: {extent:?} vs {want:?}"
                );
                // The glass and the regions are that extent.
                let shapes = bar.shapes(now, 20.0);
                assert_eq!(shapes.first().map(|s| s.0), extent);
                assert!(bar.contains(want.x + 1.0, 28.0, now));
            }
            // It starts as wide as the whole capsule and ends as the placed
            // one: no gap, no jump.
            assert_eq!(bar.capsule_extent(0, t0).map(|r| r.w), whole.map(|r| r.w));
            let end = after(t0, 400);
            assert_eq!(bar.capsule_extent(0, end), rest);
            assert_eq!(bar.shapes(end, 20.0).first().map(|s| s.0), rest);
            assert!(
                !bar.is_sliding(0, end) && !bar.is_animating(end),
                "no frames at rest"
            );
            assert!(bar.is_sliding(0, after(t0, 100)));
        });
    }

    #[test]
    fn a_capsule_moved_during_a_glide_keeps_its_glass_and_regions_on_it() {
        with_cx(|cx| {
            let t0 = Instant::now();
            let mut bar = BarTree::new(
                &only(&["notifications", "brightness"], &["session"]),
                cx.theme,
                false,
            );
            bar.update(cx, 1920.0, 300.0, t0, &mut lay());
            let lifted = bar
                .views
                .start
                .as_ref()
                .and_then(|v| v.children.get(1).map(|c| c.key));
            lift(&mut bar, cx, lifted, t0);
            // The output narrows while the lift's glide runs.
            bar.update(cx, 1900.0, 300.0, after(t0, 100), &mut lay());
            for ms in [150, 250] {
                let now = after(t0, ms);
                bar.sync_extents(now);
                let painted = bar.zone_node(3).map(|id| bar.tree.painted_rect(id, now));
                let extent = bar.capsule_extent(3, now);
                let shape = bar.shapes(now, 20.0).last().map(|s| s.0);
                assert!(
                    painted
                        .zip(extent)
                        .is_some_and(|(p, e)| (p.x - e.x).abs() < 0.01 && (p.w - e.w).abs() < 0.01),
                    "+{ms} ms: glass {painted:?} vs extent {extent:?}"
                );
                assert_eq!(shape, extent, "+{ms} ms: regions");
            }
            let end = after(t0, 800);
            assert_eq!(bar.capsule_extent(3, end), bar.zone_rect(3));
            assert!(!bar.is_animating(end), "no frames at rest");
        });
    }

    #[test]
    fn landing_the_piece_reopens_its_slot() {
        with_cx(|cx| {
            let (mut bar, [_, b, c], t0) = trio(cx, false);
            let whole = bar.zone_rect(0);
            let at_rest = bar.node_rect(c).map_or(0.0, |r| r.x);
            lift(&mut bar, cx, Some(b), t0);
            let t1 = after(t0, 400);
            let narrow = bar.node_rect(c).map_or(0.0, |r| r.x);
            lift(&mut bar, cx, None, t1);
            assert_eq!(bar.zone_rect(0), whole, "the width comes back");
            assert_eq!(bar.lifted(), None);
            let id = bar.tree.node_by_key(c);
            let painted =
                |ms: u64| id.map_or(f32::NAN, |id| bar.tree.painted_rect(id, after(t1, ms)).x);
            assert!((painted(0) - narrow).abs() < 0.01);
            let k = spring_at(cx.theme, 0.5);
            assert!((painted(175) - (narrow + (at_rest - narrow) * k)).abs() < 0.01);
            assert!((painted(350) - at_rest).abs() < 0.01);
            assert!(!bar.is_animating(after(t1, 350)));
        });
    }

    #[test]
    fn the_slot_keeps_its_centre_while_lifted() {
        with_cx(|cx| {
            let (mut bar, [a, b, c], t0) = trio(cx, false);
            let piece = bar.node_rect(b);
            let before = bar.slot_rect(b, t0);
            assert_eq!(
                before.map(|s| (s.x, s.w)),
                piece.map(|p| (p.x + p.w / 2.0, 0.0)),
                "in its row, the slot is the piece's centre"
            );
            lift(&mut bar, cx, Some(b), t0);
            // Right away the neighbours are still where they were: the gap
            // between them is the old one, centred on the piece's.
            let (ra, rc) = (bar.node_rect(a), bar.node_rect(c));
            let gap = cx.theme.spacing.sm;
            let slot = bar.slot_rect(b, after(t0, 400));
            let want = ra.zip(rc).map(|(ra, rc)| (ra.x + ra.w + rc.x) / 2.0);
            assert_eq!(slot.map(|s| s.x), want);
            assert_eq!(slot.map(|s| (s.w, s.h)), Some((0.0, 40.0)));
            assert!(ra
                .zip(rc)
                .is_some_and(|(ra, rc)| rc.x - (ra.x + ra.w) == gap));
            // It moves with the pieces that slide, so the sheet over it does.
            let mid = bar.slot_rect(b, after(t0, 100));
            assert!(mid.is_some_and(|m| before.is_some_and(|b0| m.x != b0.x)));
            // Off the bar, a piece has none.
            assert!(bar.slot_rect(Key::named("nothing"), t0).is_none());
        });
    }

    #[test]
    fn reduced_motion_jumps_the_slot() {
        with_cx(|cx| {
            let (mut bar, [a, b, c], t0) = trio(cx, true);
            lift(&mut bar, cx, Some(b), t0);
            assert!(!bar.tree.is_animating(t0), "no slide");
            assert!(!bar.is_sliding(0, t0));
            // The piece still fades, with no blur or drop.
            assert!(bar.is_animating(t0));
            assert_eq!(bar.fade_of(b, t0), Some((1.0, 0.0, 0.0)));
            assert_eq!(bar.capsule_extent(0, t0), bar.zone_rect(0));
            for k in [a, c] {
                let id = bar.tree.node_by_key(k);
                assert_eq!(
                    id.map(|id| bar.tree.painted_rect(id, t0)),
                    bar.node_rect(k),
                    "drawn where it is"
                );
            }
        });
    }

    #[test]
    fn the_only_piece_of_a_capsule_can_be_lifted() {
        with_cx(|cx| {
            let t0 = Instant::now();
            let mut bar = BarTree::new(&only(&["notifications"], &[]), cx.theme, false);
            bar.update(cx, 1920.0, 300.0, t0, &mut lay());
            let bell = Key::named("bell");
            lift(&mut bar, cx, Some(bell), t0);
            let pad = CAPSULE_PADDING;
            let zone = bar.zone_rect(0);
            assert_eq!(
                zone.map(|z| z.w),
                Some(2.0 * pad),
                "an empty row of padding"
            );
            assert!(bar.placement().is_some_and(|p| p.start.is_some()));
            let slot = bar.slot_rect(bell, t0);
            assert!(slot.is_some_and(|s| s.w == 0.0 && s.x.is_finite()));
            let end = after(t0, 400);
            assert!(bar.capsule_extent(0, end).is_some_and(|e| e.w == 2.0 * pad));
            assert_eq!(bar.shapes(end, 20.0).len(), 1);
        });
    }

    #[test]
    fn a_lifted_piece_that_leaves_the_bar_has_no_slot() {
        with_cx(|cx| {
            let (mut bar, [_, b, _], t0) = trio(cx, false);
            lift(&mut bar, cx, Some(b), t0);
            assert!(bar.slot_rect(b, t0).is_some());
            // The layout drops the piece: its node and slot are gone.
            assert!(bar.set_layout(&only(&["notifications", "clock"], &[])));
            bar.update(cx, 1920.0, 300.0, after(t0, 10), &mut lay());
            assert!(bar.slot_rect(b, after(t0, 10)).is_none());
            assert!(bar.zone_of(b).is_none());
        });
    }

    #[test]
    fn switching_lands_the_old_piece_and_lifts_the_new_one_on_one_spring() {
        with_cx(|cx| {
            let (mut bar, [a, b, c], t0) = trio(cx, false);
            let gap = cx.theme.spacing.sm;
            let whole = bar.zone_rect(0).map_or(0.0, |r| r.w);
            lift(&mut bar, cx, Some(a), t0);
            let t1 = after(t0, 400);
            let width = |bar: &BarTree, k| bar.node_rect(k).map_or(0.0, |r| r.w);
            // Now the last piece goes and the first comes back.
            let b_before = bar.node_rect(b).map_or(0.0, |r| r.x);
            lift(&mut bar, cx, Some(c), t1);
            let narrow = bar.zone_rect(0).map_or(0.0, |r| r.w);
            assert_eq!(narrow, whole - width(&bar, c) - gap);
            let id = bar.tree.node_by_key(b);
            let b_after = bar.node_rect(b).map_or(0.0, |r| r.x);
            assert!(b_after > b_before, "the landed piece reopens room before b");
            let painted =
                |ms: u64| id.map_or(f32::NAN, |id| bar.tree.painted_rect(id, after(t1, ms)).x);
            let k = spring_at(cx.theme, 0.5);
            assert!((painted(0) - b_before).abs() < 0.01);
            assert!((painted(175) - (b_before + (b_after - b_before) * k)).abs() < 0.01);
            assert!((painted(350) - b_after).abs() < 0.01);
            // The old piece is in its row again (not hidden), the new one is
            // out of it.
            let kids = bar
                .tree
                .node_by_key(zone_key(0))
                .map(|z| bar.tree.painted_children(z, t1))
                .unwrap_or_default();
            let hidden: Vec<bool> = kids.iter().map(|k| k.2).collect();
            assert_eq!(hidden, [false, false, true]);
        });
    }

    #[test]
    fn a_placement_change_during_a_lift_keeps_the_lifted_zones_glide() {
        with_cx(|cx| {
            let (mut bar, [_, b, _], t0) = trio(cx, false);
            lift(&mut bar, cx, Some(b), t0);
            let mid = after(t0, 100);
            assert!(bar.is_sliding(0, mid));
            // Another capsule's width moves the placement, not the lift.
            bar.update(cx, 1900.0, 300.0, mid, &mut lay());
            assert!(bar.is_sliding(0, mid), "the glide goes on");
            let extent = bar.capsule_extent(0, mid);
            assert_ne!(extent, bar.zone_rect(0), "the capsule has not jumped");
        });
    }

    #[test]
    fn the_slot_of_the_last_piece_starts_where_the_piece_was() {
        with_cx(|cx| {
            let (mut bar, [_, _, c], t0) = trio(cx, false);
            let rect = bar.node_rect(c).unwrap_or(ZERO);
            let centre = rect.x + rect.w / 2.0;
            lift(&mut bar, cx, Some(c), t0);
            let slot = bar.slot_rect(c, t0).map_or(f32::MAX, |s| s.x);
            assert!(
                (slot - centre).abs() <= cx.theme.spacing.sm,
                "slot {slot} centre {centre}"
            );
        });
    }
    #[test]
    fn the_pill_is_the_piece_at_rest_a_square_while_lifted_and_grows_back() {
        with_cx(|cx| {
            let (mut bar, [_, b, _], t0) = trio(cx, false);
            let piece = bar.node_rect(b).unwrap_or(ZERO);
            // In its row at rest: the piece's own rect.
            assert_eq!(bar.pill_of(b, t0), Some(piece));
            // Lifted, its slot is shut: a square as high as the piece,
            // centred on the slot.
            lift(&mut bar, cx, Some(b), t0);
            let shut = after(t0, 400);
            let slot = bar.slot_rect(b, shut).map_or(f32::NAN, |s| s.x);
            let pill = bar.pill_of(b, shut).unwrap_or(ZERO);
            assert_eq!((pill.w, pill.h, pill.y), (piece.h, piece.h, piece.y));
            assert!((pill.x + pill.w / 2.0 - slot).abs() < 0.01);
            // Landed, the slot reopens on the spring: the pill widens from
            // the square to the piece, and ends as the piece's rect.
            let t1 = after(t0, 500);
            lift(&mut bar, cx, None, t1);
            let width = |ms: u64| bar.pill_of(b, after(t1, ms)).map_or(f32::NAN, |p| p.w);
            assert!(width(0) <= piece.h + 0.01, "starts shut: {}", width(0));
            assert!(width(175) > width(0), "grows");
            assert!(width(175) <= piece.w + 0.01);
            assert!((width(400) - piece.w).abs() < 0.01, "ends as the piece");
            assert_eq!(bar.pill_of(b, after(t1, 400)), Some(piece));
            // A piece the bar does not hold has none.
            assert!(bar.pill_of(Key::named("nothing"), t0).is_none());
        });
    }

    #[test]
    fn a_piece_at_the_end_of_its_capsule_reopens_inside_the_capsules_edge() {
        with_cx(|cx| {
            for order in [
                ["brightness", "notifications"],
                ["notifications", "brightness"],
            ] {
                let t0 = Instant::now();
                let mut bar = BarTree::new(&only(&order, &[]), cx.theme, false);
                bar.update(cx, 1920.0, 300.0, t0, &mut lay());
                let keys: Vec<Key> = bar
                    .views
                    .start
                    .as_ref()
                    .map(|v| v.children.iter().map(|c| c.key).collect())
                    .unwrap_or_default();
                assert_eq!(keys.len(), 2, "both pieces draw something");
                let end = if order[0] == "brightness" {
                    keys[0]
                } else {
                    keys[1]
                };
                let piece = bar.node_rect(end).unwrap_or(ZERO);
                assert!(piece.w > piece.h, "a wide piece: {piece:?}");
                lift(&mut bar, cx, Some(end), t0);
                let shut = after(t0, 400);
                lift(&mut bar, cx, None, shut);
                // Its slot reopens from a square: the pill is not the whole
                // piece over the neighbour still sliding out.
                let pill = bar.pill_of(end, shut).unwrap_or(ZERO);
                assert!(
                    pill.w <= piece.h + 0.01,
                    "{order:?}: a square, not the whole piece: {} vs {}",
                    pill.w,
                    piece.w
                );
                let later = bar.pill_of(end, after(shut, 400)).unwrap_or(ZERO);
                assert!((later.w - piece.w).abs() < 0.01, "{order:?}: {later:?}");
            }
        });
    }
}
