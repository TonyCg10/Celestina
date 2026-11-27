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
//! The piece a capsule sheet is open on stays in its slot (CEL-D-014): no
//! lift, no hidden node, no crossfade. [`OpenPiece`] holds the clocks of its
//! look, and each frame [`BarTree::sync_open`] hands the painted tree the
//! grey pill behind the piece (a plate of its capsule, under its pieces) and
//! the opacity of its content, with no pointer lift while it is open. The
//! capsule's extent does not change when a menu opens, switches or closes.
//!
//! Only the tray's changes move a row: a hidden tray app, or the chevron
//! entering or leaving, slides the pieces beside it on the open spring while
//! the capsule's glass, regions and anchor glide from the extent it had to
//! the one it has ([`BarTree::capsule_extent`]) on the same spring, so the
//! glass and the pieces in it stay one shape.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Role, Theme};
use celestina_shell::settings::BarLayout;
use celestina_ui::{
    view, Easing, Held, Highlight, Interaction, Key, Motion, Plate, Tree, TreeError, UiEvent, View,
};
use celestina_wayland::InputEvent;

use super::{
    open_piece::OpenPiece,
    state::{CAPSULE_H, TOP_GAP},
    view::centred,
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
    /// The pill and the content opacities of the piece a menu is open on.
    open: OpenPiece,
    /// The keys the painted tree holds a look for, as the last frame synced
    /// them: the ones to clear when their clocks end.
    held: Vec<Key>,
    /// The corner radius of the open pill, from the theme.
    pill_radius: f32,
    /// The hidden tray apps or the chevron changed since the last update:
    /// the slides it starts run on the open spring.
    lift_changed: bool,
    /// The open spring, from the theme: how long and on which curve.
    spring: (Duration, Easing),
    /// The gap the row keeps between pieces, from the theme.
    gap: f32,
    /// Each capsule's glide from the extent it was painted at to its rect,
    /// while its pieces slide.
    glides: [Option<Glide>; 4],
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

/// The open spring of `theme`: `motion.slow` with the overshoot easing.
fn spring_of(theme: &Theme) -> (Duration, Easing) {
    (
        Motion::Slow.duration(&theme.motion),
        Easing::OutBack(theme.overshoot),
    )
}

/// The pill of piece `key` in `tree` at `now`, if `views` hold the piece: its
/// node's painted rect, `PILL_H` high when it has no height.
fn pill_in(tree: &Tree, views: &ZoneViews, key: Key, now: Instant) -> Option<Rect> {
    views.owner(key)?;
    let rect = tree.painted_rect(tree.node_by_key(key)?, now);
    Some(if rect.h > 0.0 {
        rect
    } else {
        Rect { h: PILL_H, ..rect }
    })
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
            open: OpenPiece::new(theme, reduced),
            held: Vec::new(),
            pill_radius: theme.radius_md,
            lift_changed: false,
            spring: spring_of(theme),
            gap: theme.spacing.sm,
            glides: [None; 4],
        }
    }

    /// Opens the look of `piece` from `now` (`None` lets the open one go):
    /// the pill lights behind it, or slides or fades to where it is, and its
    /// content goes to the muted opacity. It changes no layout and marks no
    /// view stale. A piece the bar does not hold is no open piece.
    pub fn set_open(&mut self, piece: Option<Key>, now: Instant) {
        let open = piece.and_then(|key| self.zone_of(key).map(|zone| (key, zone)));
        let was = self.open.open();
        let (tree, views) = (&self.tree, &self.views);
        self.open
            .set(open, now, &|key| pill_in(tree, views, key, now));
        // The piece the pointer is on gives its pill up at once at the
        // restore moment: its hover fill takes over, so two fills never
        // stack while the pill fades.
        if let (Some(old), None) = (was, open) {
            let on = self.tree.node_by_key(old).is_some_and(|id| {
                let h = self.tree.highlight();
                h.hovered == Some(id) || h.pressed == Some(id)
            });
            if on {
                self.open.cut_pills();
            }
        }
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
        // Only the hidden tray apps changed (the person hid or showed one):
        // the icons beside the one that leaves or comes slide on the open
        // spring and the capsule glides. Any other change is a new
        // placement, which jumps.
        self.lift_changed |= layout.start == self.layout.start
            && layout.centre == self.layout.centre
            && layout.end == self.layout.end;
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
        self.pill_radius = theme.radius_md;
        self.open.set_theme(theme);
        self.stale = true;
    }

    pub fn set_reduced_motion(&mut self, on: bool) {
        if on != self.reduced {
            self.reduced = on;
            self.tree.set_reduced_motion(on);
            self.open.set_reduced(on);
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
        let mut lift = std::mem::take(&mut self.lift_changed);
        let had_chevron = self.views.owner(pieces::tray::chevron_key()).is_some();
        if self.stale {
            let cx = BarCx {
                tray_hidden: &self.layout.tray_hidden,
                ..*base
            };
            self.views = self.zones.build(&self.layout, &cx);
            // The tray's chevron enters or leaves the bar (an app was hidden,
            // shown, registered or gone): it takes its slot on the open
            // spring.
            lift |= had_chevron != self.views.owner(pieces::tray::chevron_key()).is_some();
            self.measure_views(now, lay);
            self.stale = false;
            // A piece that vanished takes its pill and opacity with it.
            let views = &self.views;
            self.open.settle(now, &|key| views.owner(key).is_some());
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
            // moves them and a press's recoil runs on. A tray change moves
            // the placement too, but there the capsules still jump while the
            // pieces inside slide on the open spring from where they were
            // drawn, and the capsules' glass follows them. One that comes
            // while a glide still runs is carried the same way: the capsules
            // jump and glide again from where they are drawn, so their glass
            // and regions stay on their pieces.
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

    /// Paints the bar, every capsule, with the open piece's pill and
    /// opacity as the clocks have them at `now`.
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        self.sync_extents(now);
        self.sync_open(now);
        self.tree.paint(painter, now);
    }

    /// Hands the painted tree the open piece's look at `now`: a plate for
    /// each zone with a pill (and none for the others), and the held state
    /// of the open piece (quiet, at its content opacity) and of the pieces
    /// still returning to full opacity (not quiet), cleared once they are
    /// done.
    fn sync_open(&mut self, now: Instant) {
        let views = &self.views;
        self.open.settle(now, &|key| views.owner(key).is_some());
        let (tree, views) = (&self.tree, &self.views);
        let pills = self.open.pills(now, &|key| pill_in(tree, views, key, now));
        for (zone, key) in zone_keys().into_iter().enumerate() {
            let plate = pills.iter().find(|p| p.zone == zone).map(|p| Plate {
                rect: p.rect,
                radius: self.pill_radius,
                role: Role::ControlHover,
                alpha: p.alpha,
            });
            self.tree.set_plate(key, plate);
        }
        let open = self.open.open();
        let mut keys: Vec<Key> = std::mem::take(&mut self.held);
        if let Some(key) = open.filter(|k| !keys.contains(k)) {
            keys.push(key);
        }
        for key in keys {
            let held = self.open.content(key, now).map(|inside| Held {
                quiet: open == Some(key),
                inside,
            });
            self.tree.set_held(key, held);
            if held.is_some() {
                self.held.push(key);
            }
        }
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
    /// rest, and while its pieces slide (a tray app hidden or shown) its
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

    /// The id of the piece that drew node `key`.
    #[must_use]
    pub fn piece_id_of(&self, key: Key) -> Option<&str> {
        self.views.owner(key)
    }

    /// The zone whose capsule holds node `key`.
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

    /// The pill of piece `key` at `now`: its node's painted rect, a pill as
    /// high as `PILL_H` when it has none. What the open pill lies on and what
    /// a window grows from and closes towards. `None` once the bar does not
    /// hold the piece.
    #[must_use]
    pub fn pill_of(&self, key: Key, now: Instant) -> Option<Rect> {
        pill_in(&self.tree, &self.views, key, now)
    }

    /// The piece a menu is open on in the bar (tests).
    #[cfg(test)]
    #[must_use]
    pub fn open_piece(&self) -> Option<Key> {
        self.open.open()
    }

    /// The pills the bar paints at `now` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn pill(&self, now: Instant) -> Vec<super::open_piece::PillLook> {
        let (tree, views) = (&self.tree, &self.views);
        self.open.pills(now, &|key| pill_in(tree, views, key, now))
    }

    /// The opacity of `key`'s content at `now`: 1 when it is not held
    /// (tests).
    #[cfg(test)]
    #[must_use]
    pub fn content_opacity(&self, key: Key, now: Instant) -> f32 {
        self.open.content(key, now).unwrap_or(1.0)
    }

    /// The plate and the held state the painted tree has for zone `zone`
    /// and for node `key` as the last sync left them (tests).
    #[cfg(test)]
    #[must_use]
    pub fn synced_look(
        &mut self,
        zone: usize,
        key: Key,
        now: Instant,
    ) -> (Option<Plate>, Option<Held>) {
        self.sync_extents(now);
        self.sync_open(now);
        (
            zone_keys().get(zone).and_then(|&z| self.tree.plate_of(z)),
            self.tree.held_of(key),
        )
    }

    /// The fill and the wash the painted tree draws for node `key` now
    /// (tests).
    #[cfg(test)]
    #[must_use]
    pub fn box_roles(&self, key: Key) -> (Option<Role>, Option<Role>) {
        self.tree
            .node_by_key(key)
            .map_or((None, None), |id| self.tree.box_roles(id))
    }

    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now)
            || self.glides.iter().flatten().any(|g| self.gliding(g, now))
            || self.open.is_animating(now)
    }

    /// Whether glide `g` still runs at `now`.
    fn gliding(&self, g: &Glide, now: Instant) -> bool {
        now.saturating_duration_since(g.start) < self.spring.0
    }

    /// The rotation of the bar's node `key` at `now` (tests).
    #[cfg(test)]
    #[must_use]
    pub fn rotation_of(&self, key: Key, now: Instant) -> Option<f32> {
        Some(self.tree.rotation(self.tree.node_by_key(key)?, now))
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
    use crate::island::pages::{tray::TrayRow, NameOwner};

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
            tray_open: false,
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

    fn tray_rows(n: usize) -> Vec<crate::island::pages::tray::TrayRow> {
        (0..n)
            .map(|i| crate::island::pages::tray::TrayRow {
                id: crate::tray::ItemId {
                    bus_name: format!(":1.{i}"),
                    path: "/StatusNotifierItem".to_owned(),
                },
                sni_id: format!("app{i}"),
                title: format!("app {i}"),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn hiding_a_tray_app_slides_the_icons_on_the_open_spring_and_settles() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let cx = BarCx {
                items: &rows,
                ..*base
            };
            let t0 = Instant::now();
            let mut bar = BarTree::new(&only(&[], &["tray"]), cx.theme, false);
            bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
            let first = crate::island::pages::tray::row_key(0);
            let x_of = |bar: &BarTree| bar.node_rect(first).map_or(f32::NAN, |r| r.x);
            let old = x_of(&bar);
            assert!(!bar.is_animating(t0), "a bar at rest draws no frames");

            // The last app is hidden: the capsule narrows and the icons
            // beside it slide to their new places.
            let mut hidden = only(&[], &["tray"]);
            hidden.tray_hidden = vec!["app2".to_owned()];
            assert!(bar.set_layout(&hidden));
            bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
            let new = x_of(&bar);
            assert!((new - old).abs() > 1.0, "the icon moves: {old} to {new}");
            let id = bar.tree.node_by_key(first);
            let painted =
                |ms: u64| id.map_or(f32::NAN, |id| bar.tree.painted_rect(id, after(t0, ms)).x);
            assert!((painted(0) - old).abs() < 0.01, "it starts where it was");
            let k = spring_at(cx.theme, 0.5);
            assert!((painted(175) - (old + (new - old) * k)).abs() < 0.01);
            assert!((painted(350) - new).abs() < 0.01);
            assert!(bar.is_animating(after(t0, 349)));
            assert!(!bar.is_animating(after(t0, 350)), "settled: no more frames");

            // Showing it again slides back the same way.
            assert!(bar.set_layout(&only(&[], &["tray"])));
            bar.update(&cx, 1920.0, 300.0, after(t0, 400), &mut lay());
            assert!((x_of(&bar) - old).abs() < 0.01);
            assert!(bar.is_animating(after(t0, 500)));
            assert!(!bar.is_animating(after(t0, 750)));
        });
    }

    #[test]
    fn a_zone_change_still_jumps_when_the_hidden_list_changes_with_it() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let cx = BarCx {
                items: &rows,
                ..*base
            };
            let t0 = Instant::now();
            let mut bar = BarTree::new(&only(&[], &["tray"]), cx.theme, false);
            bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
            let mut moved = only(&["tray"], &[]);
            moved.tray_hidden = vec!["app2".to_owned()];
            assert!(bar.set_layout(&moved));
            bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
            let first = crate::island::pages::tray::row_key(0);
            let id = bar.tree.node_by_key(first);
            let landed = bar.node_rect(first).map_or(f32::NAN, |r| r.x);
            assert!(
                id.is_some_and(|id| (bar.tree.painted_rect(id, t0).x - landed).abs() < 0.01),
                "a new placement jumps: nothing slides"
            );
            assert!(bar.glides.iter().all(Option::is_none), "no capsule glides");
        });
    }

    /// A bar of `layout` with the registered apps `rows`, laid out at rest.
    fn tray_bar(base: &BarCx<'_>, layout: &BarLayout, rows: &[TrayRow], t0: Instant) -> BarTree {
        let cx = BarCx {
            items: rows,
            ..*base
        };
        let mut bar = BarTree::new(layout, cx.theme, false);
        bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
        bar
    }

    /// Hides the apps `ids` at `t0`: the chevron comes in, the icons slide on
    /// the open spring and the capsule glides.
    fn hide_apps(bar: &mut BarTree, base: &BarCx<'_>, rows: &[TrayRow], ids: &[&str], t0: Instant) {
        let cx = BarCx {
            items: rows,
            ..*base
        };
        let mut layout = bar.layout().clone();
        layout.tray_hidden = ids.iter().map(|s| (*s).to_owned()).collect();
        assert!(bar.set_layout(&layout));
        bar.update(&cx, 1920.0, 300.0, t0, &mut lay());
    }

    #[test]
    fn the_row_slides_with_the_open_spring() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let t0 = Instant::now();
            let mut bar = tray_bar(base, &only(&[], &["tray"]), &rows, t0);
            let first = crate::island::pages::tray::row_key(0);
            let old = bar.node_rect(first).map_or(0.0, |r| r.x);
            hide_apps(&mut bar, base, &rows, &["app2"], t0);
            let new = bar.node_rect(first).map_or(0.0, |r| r.x);
            assert!((new - old).abs() > 1.0, "the icon moves: {old} to {new}");
            let id = bar.tree.node_by_key(first);
            let painted =
                |ms: u64| id.map_or(f32::NAN, |id| bar.tree.painted_rect(id, after(t0, ms)).x);
            // It starts where it was and ends where it goes; half way is the
            // open spring's, not the normal step's.
            assert!((painted(0) - old).abs() < 0.01);
            let k = spring_at(base.theme, 0.5);
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
        with_cx(|base| {
            let rows = tray_rows(3);
            let t0 = Instant::now();
            let mut bar = tray_bar(base, &only(&[], &["tray"]), &rows, t0);
            let first = crate::island::pages::tray::row_key(0);
            let whole = bar.zone_rect(3).unwrap_or(ZERO);
            let old = bar.node_rect(first).map_or(0.0, |r| r.x);
            hide_apps(&mut bar, base, &rows, &["app1", "app2"], t0);
            let rest = bar.zone_rect(3);
            let end = rest.unwrap_or(ZERO);
            let new = bar.node_rect(first).map_or(0.0, |r| r.x);
            for ms in [0, 60, 175, 300] {
                let now = after(t0, ms);
                // Both edges and the icon go on the open spring: the glass
                // and the icon in it stay one shape.
                let k = spring_at(base.theme, ms as f32 / 350.0);
                let (left, right) = (
                    whole.x + (end.x - whole.x) * k,
                    whole.x + whole.w + (end.x + end.w - whole.x - whole.w) * k,
                );
                let extent = bar.capsule_extent(3, now);
                assert!(
                    extent.is_some_and(
                        |e| (e.x - left).abs() < 0.01 && (e.x + e.w - right).abs() < 0.01
                    ),
                    "+{ms} ms: {extent:?} vs {left}..{right}"
                );
                let icon = bar
                    .tree
                    .node_by_key(first)
                    .map(|id| bar.tree.painted_rect(id, now).x);
                assert!(
                    icon.is_some_and(|x| (x - (old + (new - old) * k)).abs() < 0.01),
                    "+{ms} ms: {icon:?}"
                );
                // The glass and the regions are that extent.
                let shapes = bar.shapes(now, 20.0);
                assert_eq!(shapes.last().map(|s| s.0), extent);
                assert!(bar.contains(left + 1.0, 28.0, now));
            }
            // It starts as wide as the whole capsule and ends as the placed
            // one: no gap, no jump.
            assert_eq!(bar.capsule_extent(3, t0).map(|r| r.w), Some(whole.w));
            let done = after(t0, 400);
            assert_eq!(bar.capsule_extent(3, done), rest);
            assert_eq!(bar.shapes(done, 20.0).last().map(|s| s.0), rest);
            assert!(
                !bar.is_sliding(3, done) && !bar.is_animating(done),
                "no frames at rest"
            );
            assert!(bar.is_sliding(3, after(t0, 100)));
        });
    }

    #[test]
    fn a_capsule_moved_during_a_glide_keeps_its_glass_and_regions_on_it() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let cx = BarCx {
                items: &rows,
                ..*base
            };
            let t0 = Instant::now();
            let mut bar = tray_bar(base, &only(&["tray"], &["session"]), &rows, t0);
            hide_apps(&mut bar, base, &rows, &["app1", "app2"], t0);
            // The output narrows while the tray's glide runs.
            bar.update(&cx, 1900.0, 300.0, after(t0, 100), &mut lay());
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
    fn a_placement_change_during_a_tray_slide_keeps_the_zones_glide() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let cx = BarCx {
                items: &rows,
                ..*base
            };
            let t0 = Instant::now();
            let mut bar = tray_bar(base, &only(&["tray"], &[]), &rows, t0);
            hide_apps(&mut bar, base, &rows, &["app1", "app2"], t0);
            let mid = after(t0, 100);
            assert!(bar.is_sliding(0, mid));
            // Another capsule's width moves the placement, not the tray.
            bar.update(&cx, 1900.0, 300.0, mid, &mut lay());
            assert!(bar.is_sliding(0, mid), "the glide goes on");
            let extent = bar.capsule_extent(0, mid);
            assert_ne!(extent, bar.zone_rect(0), "the capsule has not jumped");
        });
    }

    #[test]
    fn the_pill_is_the_pieces_painted_rect() {
        with_cx(|base| {
            let rows = tray_rows(3);
            let t0 = Instant::now();
            let mut bar = tray_bar(base, &only(&[], &["notifications", "tray"]), &rows, t0);
            let bell = Key::named("bell");
            let first = crate::island::pages::tray::row_key(0);
            // At rest it is the piece's rect.
            for key in [bell, first] {
                assert_eq!(bar.pill_of(key, t0), bar.node_rect(key), "{key:?}");
            }
            // While a neighbour slides it is the rect the piece is painted at.
            hide_apps(&mut bar, base, &rows, &["app2"], t0);
            let mut moved = false;
            for ms in [0, 60, 175, 400] {
                let now = after(t0, ms);
                let painted = bar
                    .tree
                    .node_by_key(first)
                    .map(|id| bar.tree.painted_rect(id, now));
                assert_eq!(bar.pill_of(first, now), painted, "+{ms} ms");
                moved |= bar.pill_of(first, now) != bar.node_rect(first);
            }
            assert!(moved, "premise: the icon was sliding");
            assert_eq!(bar.pill_of(first, after(t0, 400)), bar.node_rect(first));
            // A piece the bar does not hold has none.
            assert!(bar.pill_of(Key::named("nothing"), t0).is_none());
        });
    }
}
