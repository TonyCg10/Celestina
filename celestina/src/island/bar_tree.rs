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

use std::time::Instant;

use celestina_paint::{Painter, Rect, Theme};
use celestina_shell::settings::BarLayout;
use celestina_ui::{view, Highlight, Interaction, Key, Tree, TreeError, UiEvent, View};
use celestina_wayland::InputEvent;

use super::{
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
            tree: Tree::new(reduced, theme),
            measure: Tree::new(true, theme),
            input: Interaction::new(),
            reduced,
            stale: true,
            scroll: ScrollRemainder::default(),
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
        if self.stale {
            let cx = BarCx {
                tray_hidden: &self.layout.tray_hidden,
                ..*base
            };
            self.views = self.zones.build(&self.layout, &cx);
            self.measure_views(now, lay);
            self.stale = false;
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
            // moves them and a press's recoil runs on.
            if moved {
                self.tree.set_reduced_motion(true);
            }
            self.tree.apply(self.placed(&placement, surface_w), now);
            lay(&mut self.tree, (surface_w, BAR_H));
            if moved {
                self.tree.set_reduced_motion(self.reduced);
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
        let probe = centred(view::col(Key::named("bar-measure")))
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
        view::row(Key::named("bar-root"))
            .size(surface_w, BAR_H)
            .children(
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

    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        self.tree.paint(painter, now);
    }

    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.tree.is_animating(now)
    }

    #[must_use]
    pub fn highlight(&self) -> Highlight {
        self.tree.highlight()
    }

    /// The capsules as they are painted at `now` (fading ones included),
    /// with the haze's corner `radius`: what the blur and input regions
    /// cover.
    #[must_use]
    pub fn shapes(&self, now: Instant, radius: f32) -> Vec<Shape> {
        let zones = zone_keys();
        self.tree
            .paint_order()
            .into_iter()
            .filter(|&id| {
                zones.contains(&self.tree.key_of(id)) && self.tree.effective_opacity(id, now) > 0.0
            })
            .map(|id| {
                let r = self.tree.rect(id);
                let (dx, dy) = self.tree.offset(id, now);
                (
                    Rect {
                        x: r.x + dx,
                        y: r.y + dy,
                        ..r
                    },
                    radius,
                )
            })
            .filter(|(r, _)| r.w > 0.0 && r.h > 0.0)
            .collect()
    }

    /// Whether (`x`, `y`) is on one of the capsules.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        zone_keys().into_iter().any(|key| {
            self.tree
                .node_by_key(key)
                .is_some_and(|id| self.tree.rect(id).contains(x, y))
        })
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
                | UiEvent::FieldChanged(_) => {}
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
    use celestina_paint::TextStyle;

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
}
