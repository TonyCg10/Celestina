//! xdg_popup plumbing: the `xdg_wm_base` global (bound without sctk's
//! `XdgShell`, whose decoration-manager binding would demand a
//! `WindowHandler`), the placement and positioner maths, the queue of popup
//! requests made through `SurfaceCtl`, and the runtime side of opening,
//! routing to and closing popups, nested ones included.
#![forbid(unsafe_code)]

use std::collections::VecDeque;

use smithay_client_toolkit::{
    error::GlobalError,
    globals::{GlobalData, ProvidesBoundGlobal},
    shell::xdg::{
        popup::{Popup, PopupConfigure},
        XdgPositioner,
    },
};
use wayland_client::{
    globals::{BindError, GlobalList},
    protocol::wl_surface::WlSurface,
    Dispatch, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{
    xdg_positioner::{Anchor as XdgAnchor, ConstraintAdjustment, Gravity as XdgGravity},
    xdg_wm_base::XdgWmBase,
};

use crate::{
    geometry::{PixelRect, Rect},
    input::InputEvent,
    runtime::{Core, PopupShell, Runtime, RuntimeError, Shell, SurfaceId, Target},
    surface::{Role, Surface},
};

/// A popup, by the id `SurfaceCtl::open_popup` returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PopupId(pub u32);

/// What a popup shell answers to an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupAction {
    Keep,
    Close,
}

/// Why a popup is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The popup's shell returned `PopupAction::Close`.
    Shell,
    /// The parent shell called `close_popup`.
    Parent,
    /// The compositor sent `popup_done`.
    Dismissed,
    /// The parent opened another popup.
    Replaced,
    /// The parent surface went away.
    ParentGone,
    /// The popup could not be created or mapped.
    Failed,
    /// The runtime is shutting down.
    Exit,
}

/// The bound `xdg_wm_base`. sctk answers its pings (`Dispatch2` on
/// `GlobalData`).
pub struct WmBase(XdgWmBase);

impl WmBase {
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Result<Self, BindError>
    where
        D: Dispatch<XdgWmBase, GlobalData> + 'static,
    {
        Ok(Self(globals.bind(qh, 1..=6, GlobalData)?))
    }

    /// Call after every popup is gone.
    pub fn destroy(self) {
        self.0.destroy();
    }
}

impl ProvidesBoundGlobal<XdgWmBase, 5> for WmBase {
    fn bound_global(&self) -> Result<XdgWmBase, GlobalError> {
        Ok(self.0.clone())
    }
}

impl ProvidesBoundGlobal<XdgWmBase, 6> for WmBase {
    fn bound_global(&self) -> Result<XdgWmBase, GlobalError> {
        Ok(self.0.clone())
    }
}

/// The edge or corner of the anchor rect the popup hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

impl AnchorEdge {
    pub fn to_xdg(self) -> XdgAnchor {
        match self {
            AnchorEdge::None => XdgAnchor::None,
            AnchorEdge::Top => XdgAnchor::Top,
            AnchorEdge::Bottom => XdgAnchor::Bottom,
            AnchorEdge::Left => XdgAnchor::Left,
            AnchorEdge::Right => XdgAnchor::Right,
            AnchorEdge::TopLeft => XdgAnchor::TopLeft,
            AnchorEdge::BottomLeft => XdgAnchor::BottomLeft,
            AnchorEdge::TopRight => XdgAnchor::TopRight,
            AnchorEdge::BottomRight => XdgAnchor::BottomRight,
        }
    }
}

/// The direction the popup grows in from the anchor point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gravity {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

impl Gravity {
    pub fn to_xdg(self) -> XdgGravity {
        match self {
            Gravity::None => XdgGravity::None,
            Gravity::Top => XdgGravity::Top,
            Gravity::Bottom => XdgGravity::Bottom,
            Gravity::Left => XdgGravity::Left,
            Gravity::Right => XdgGravity::Right,
            Gravity::TopLeft => XdgGravity::TopLeft,
            Gravity::BottomLeft => XdgGravity::BottomLeft,
            Gravity::TopRight => XdgGravity::TopRight,
            Gravity::BottomRight => XdgGravity::BottomRight,
        }
    }
}

/// Where a popup goes, computed by the product: the anchor rect in logical
/// px of the parent surface, the edge of it to hang from, the growth
/// direction and an offset in logical px. The compositor may slide and flip
/// it to keep it on the output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub anchor: Rect,
    pub anchor_edge: AnchorEdge,
    pub gravity: Gravity,
    pub offset: (i32, i32),
}

/// Every popup may be slid and flipped on both axes to stay on the output.
pub fn constraint_adjustment() -> ConstraintAdjustment {
    ConstraintAdjustment::SlideX
        | ConstraintAdjustment::SlideY
        | ConstraintAdjustment::FlipX
        | ConstraintAdjustment::FlipY
}

/// The positioner for `placement` at `geometry`.
pub fn positioner(
    wm: &WmBase,
    placement: &Placement,
    geometry: PositionerGeometry,
) -> Result<XdgPositioner, GlobalError> {
    let positioner = XdgPositioner::new(wm)?;
    let a = geometry.anchor;
    positioner.set_size(geometry.size.0, geometry.size.1);
    positioner.set_anchor_rect(a.x, a.y, a.w, a.h);
    positioner.set_anchor(placement.anchor_edge.to_xdg());
    positioner.set_gravity(placement.gravity.to_xdg());
    positioner.set_offset(placement.offset.0, placement.offset.1);
    positioner.set_constraint_adjustment(constraint_adjustment());
    Ok(positioner)
}

/// Popup requests made through `SurfaceCtl`; the runtime applies them right
/// after the shell call that made them returns.
#[derive(Default)]
pub struct PopupQueue {
    next_id: u32,
    ops: VecDeque<PopupOp>,
}

enum PopupOp {
    Open {
        id: PopupId,
        parent: SurfaceId,
        placement: Placement,
        size: (f32, f32),
        shell: Box<dyn PopupShell>,
    },
    Close(PopupId),
}

impl PopupQueue {
    pub fn open(
        &mut self,
        parent: SurfaceId,
        placement: Placement,
        size: (f32, f32),
        shell: Box<dyn PopupShell>,
    ) -> PopupId {
        self.next_id += 1;
        let id = PopupId(self.next_id);
        self.ops.push_back(PopupOp::Open {
            id,
            parent,
            placement,
            size,
            shell,
        });
        id
    }

    pub fn close(&mut self, id: PopupId) {
        self.ops.push_back(PopupOp::Close(id));
    }
}

/// A live popup: its surface (role `Popup`), its shell and the surface it
/// belongs to (a layer surface, or another popup's surface when nested).
pub struct PopupEntry {
    pub id: PopupId,
    pub parent: SurfaceId,
    pub surface: Surface,
    pub shell: Box<dyn PopupShell>,
    pub frames: u64,
}

/// Integer positioner geometry: the anchor rect in parent-surface pixels
/// (logical px; the layer surface's window geometry is the whole surface)
/// and the popup size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionerGeometry {
    pub anchor: PixelRect,
    pub size: (i32, i32),
}

/// The anchor rect rounds outwards (floor of the origin, ceil of the far
/// edges) so it always covers the anchor; a negative extent becomes 0 (the
/// protocol rejects negative anchor sizes). The size rounds up, never below
/// 1 × 1 (the protocol rejects zero sizes).
pub fn positioner_geometry(anchor: Rect, size: (f32, f32)) -> PositionerGeometry {
    let x0 = anchor.x.floor();
    let y0 = anchor.y.floor();
    let x1 = (anchor.x + anchor.w.max(0.0)).ceil();
    let y1 = (anchor.y + anchor.h.max(0.0)).ceil();
    let side = |v: f32| (v.ceil() as i32).max(1);
    PositionerGeometry {
        anchor: PixelRect {
            x: x0 as i32,
            y: y0 as i32,
            w: (x1 - x0) as i32,
            h: (y1 - y0) as i32,
        },
        size: (side(size.0), side(size.1)),
    }
}

impl<S: Shell> Core<S> {
    pub(crate) fn popup_index_by_wl(&self, wl_surface: &WlSurface) -> Option<usize> {
        self.popups
            .iter()
            .position(|p| p.surface.wl_surface() == wl_surface)
    }

    /// The popup that gets a key while `focus` has the keyboard: the focused
    /// popup itself, else the newest popup rooted on the focused surface
    /// (the one holding the grab).
    pub(crate) fn popup_for_focus(&self, focus: SurfaceId) -> Option<usize> {
        if let Some(idx) = self.popups.iter().position(|p| p.surface.id == focus) {
            return Some(idx);
        }
        let mut rooted: Vec<SurfaceId> = vec![focus];
        let mut newest = None;
        for (idx, popup) in self.popups.iter().enumerate() {
            if rooted.contains(&popup.parent) {
                rooted.push(popup.surface.id);
                newest = Some(idx);
            }
        }
        newest
    }

    pub(crate) fn has_popup(&self, parent: SurfaceId) -> bool {
        self.popups.iter().any(|p| p.parent == parent)
    }

    /// Applies the queued popup requests in order. Failures and closures
    /// become pending `PopupClosed` events for the parent.
    pub(crate) fn flush_popup_ops(&mut self) {
        while let Some(op) = self.popup_queue.ops.pop_front() {
            match op {
                PopupOp::Open {
                    id,
                    parent,
                    placement,
                    size,
                    shell,
                } => {
                    if let Some(idx) = self.popups.iter().position(|p| p.parent == parent) {
                        self.close_popup_at(idx, CloseReason::Replaced);
                    }
                    if let Err(err) = self.open_popup(id, parent, placement, size, shell) {
                        tracing::error!(popup = id.0, %err, "popup: cannot open");
                        self.pending_events.push((
                            parent,
                            InputEvent::PopupClosed {
                                id,
                                reason: CloseReason::Failed,
                            },
                        ));
                    }
                }
                PopupOp::Close(id) => {
                    if let Some(idx) = self.popups.iter().position(|p| p.id == id) {
                        self.close_popup_at(idx, CloseReason::Parent);
                    }
                }
            }
        }
    }

    /// Creates the popup in protocol order: positioner, `xdg_surface` +
    /// `xdg_popup` (parented to the popup's `xdg_surface` when nested, else
    /// through `layer_surface.get_popup`), grab, scale and blur objects,
    /// then the initial commit without a buffer. The EGL window waits for
    /// the first `configure`.
    fn open_popup(
        &mut self,
        id: PopupId,
        parent: SurfaceId,
        placement: Placement,
        size: (f32, f32),
        shell: Box<dyn PopupShell>,
    ) -> Result<(), RuntimeError> {
        let parent_target = self
            .target_of(parent)
            .ok_or(RuntimeError::NoParent(parent))?;
        let wm = self.wm_base.as_ref().ok_or(RuntimeError::NoWmBase)?;
        let geometry = positioner_geometry(placement.anchor, size);
        let positioner = positioner(wm, &placement, geometry).map_err(RuntimeError::Popup)?;
        let wl_surface = self.compositor.create_surface(&self.qh);
        let (popup, output) = match parent_target {
            Target::Layer(idx) => {
                let parent_surface = &self.surfaces[idx];
                let layer = parent_surface
                    .layer()
                    .ok_or(RuntimeError::NoParent(parent))?;
                let popup = Popup::from_surface(None, &positioner, &self.qh, wl_surface, wm)
                    .map_err(RuntimeError::Popup)?;
                layer.get_popup(popup.xdg_popup());
                (popup, parent_surface.output().clone())
            }
            Target::Popup(idx) => {
                let parent_surface = &self.popups[idx].surface;
                let parent_popup = parent_surface
                    .popup()
                    .ok_or(RuntimeError::NoParent(parent))?;
                let popup = Popup::from_surface(
                    Some(parent_popup.xdg_surface()),
                    &positioner,
                    &self.qh,
                    wl_surface,
                    wm,
                )
                .map_err(RuntimeError::Popup)?;
                (popup, parent_surface.output().clone())
            }
        };

        let grab_serial = match (&self.seat, self.last_input_serial) {
            (Some(seat), Some(serial)) => {
                popup.xdg_popup().grab(seat, serial);
                Some(serial)
            }
            (Some(_), None) => {
                tracing::warn!("popup: no pointer or key press yet, opening without a grab");
                None
            }
            (None, _) => {
                tracing::warn!("popup: no seat, opening without a grab");
                None
            }
        };

        let blur = self.blur.surface(popup.wl_surface(), &self.qh);
        let blur_bound = blur.is_some();
        let surface_id = self.next_surface_id();
        let surface = Surface::new(
            surface_id,
            Role::Popup(popup),
            (geometry.size.0 as u32, geometry.size.1 as u32),
            output,
            self.protocols(),
            blur,
            &self.qh,
        )
        .map_err(RuntimeError::Region)?;
        if let Some(metrics) = &mut self.metrics {
            let period = metrics.period(parent);
            let label = format!("{} popup", surface.output().name);
            metrics.register(surface_id, label, period);
        }
        surface.wl_surface().commit();
        tracing::info!(
            popup = id.0,
            surface = surface_id.0,
            parent = parent.0,
            anchor = ?geometry.anchor,
            size = ?geometry.size,
            ?grab_serial,
            blur_bound,
            "popup: created, waiting for configure"
        );
        self.popups.push(PopupEntry {
            id,
            parent,
            surface,
            shell,
            frames: 0,
        });
        Ok(())
    }

    /// Destroys popup `idx` (its own nested popups first, then the EGL
    /// window, the protocol objects, the role and its `wl_surface`) and
    /// queues `PopupClosed` for the parent unless the parent is going too.
    pub(crate) fn close_popup_at(&mut self, idx: usize, reason: CloseReason) {
        let own_surface = self.popups[idx].surface.id;
        self.close_popups_of(own_surface);
        // Closing the children may have shifted the index.
        let Some(idx) = self.popups.iter().position(|p| p.surface.id == own_surface) else {
            return;
        };
        let entry = self.popups.remove(idx);
        if self.keyboard_focus == Some(entry.surface.id) {
            self.keyboard_focus = None;
        }
        tracing::info!(
            popup = entry.id.0,
            ?reason,
            frames = entry.frames,
            "popup: closed"
        );
        let (id, parent) = (entry.id, entry.parent);
        self.destroy_surface_object(entry.surface);
        if !matches!(reason, CloseReason::Exit | CloseReason::ParentGone) {
            self.pending_events
                .push((parent, InputEvent::PopupClosed { id, reason }));
        }
    }

    /// Closes every popup of surface `parent` (it is going away).
    pub(crate) fn close_popups_of(&mut self, parent: SurfaceId) {
        while let Some(idx) = self.popups.iter().position(|p| p.parent == parent) {
            self.close_popup_at(idx, CloseReason::ParentGone);
        }
    }
}

impl<S: Shell> Runtime<S> {
    /// `xdg_surface.configure` (already acked by sctk): the first one
    /// creates the EGL window and draws.
    pub(crate) fn popup_configure(&mut self, popup: &Popup, config: PopupConfigure) {
        let Some(idx) = self.core.popup_index_by_wl(popup.wl_surface()) else {
            return;
        };
        tracing::info!(
            popup = self.core.popups[idx].id.0,
            position = ?config.position,
            size = ?(config.width, config.height),
            kind = ?config.kind,
            "popup: configured"
        );
        let size = (config.width.max(0) as u32, config.height.max(0) as u32);
        match self.core.popups[idx]
            .surface
            .configure(size, &self.core.egl)
        {
            Ok(true) => self.draw(Target::Popup(idx)),
            Ok(false) => self.flush(),
            Err(err) => {
                tracing::error!(%err, "popup: cannot create the EGL window");
                self.core.close_popup_at(idx, CloseReason::Failed);
                self.flush();
            }
        }
    }

    /// `xdg_popup.popup_done`.
    pub(crate) fn popup_done(&mut self, popup: &Popup) {
        let Some(idx) = self.core.popup_index_by_wl(popup.wl_surface()) else {
            return;
        };
        self.core.popups[idx].shell.dismissed();
        self.core.close_popup_at(idx, CloseReason::Dismissed);
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, Rect};

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn px(x: i32, y: i32, w: i32, h: i32) -> PixelRect {
        PixelRect { x, y, w, h }
    }

    /// Where an unconstrained compositor puts the popup's top-left for
    /// the anchor/gravity pairs the menu uses (xdg_positioner semantics).
    fn unconstrained_origin(anchor: PixelRect, p: &Placement) -> (i32, i32) {
        let point = match p.anchor_edge {
            AnchorEdge::BottomLeft => (anchor.x, anchor.y + anchor.h),
            other => panic!("anchor edge {other:?} not modelled"),
        };
        let origin = match p.gravity {
            Gravity::BottomRight => point,
            other => panic!("gravity {other:?} not modelled"),
        };
        (origin.0 + p.offset.0, origin.1 + p.offset.1)
    }

    #[test]
    fn menu_hangs_from_the_buttons_left_edge_with_a_small_gap() {
        // The overflow button at x 480..536, y 30..58: the product anchors
        // the menu at its bottom-left corner, growing down and right, 8 px
        // below.
        let p = Placement {
            anchor: r(480.0, 30.0, 56.0, 28.0),
            anchor_edge: AnchorEdge::BottomLeft,
            gravity: Gravity::BottomRight,
            offset: (0, 8),
        };
        assert_eq!(p.anchor_edge.to_xdg(), XdgAnchor::BottomLeft);
        assert_eq!(p.gravity.to_xdg(), XdgGravity::BottomRight);
        assert_eq!(
            constraint_adjustment(),
            ConstraintAdjustment::SlideX
                | ConstraintAdjustment::SlideY
                | ConstraintAdjustment::FlipX
                | ConstraintAdjustment::FlipY
        );
        let g = positioner_geometry(p.anchor, (200.0, 136.0));
        assert_eq!(unconstrained_origin(g.anchor, &p), (480, 66));
    }

    #[test]
    fn integer_anchor_and_size_pass_through() {
        let g = positioner_geometry(r(480.0, 30.0, 56.0, 28.0), (200.0, 136.0));
        assert_eq!(g.anchor, px(480, 30, 56, 28));
        assert_eq!(g.size, (200, 136));
    }

    #[test]
    fn fractional_anchor_rounds_outwards() {
        // x 480.4 → 480, right 536.6 → 537; y 29.5 → 29, bottom 57.75 → 58.
        let g = positioner_geometry(r(480.4, 29.5, 56.2, 28.25), (200.0, 136.0));
        assert_eq!(g.anchor, px(480, 29, 57, 29));
    }

    #[test]
    fn negative_origin_rounds_outwards_too() {
        let g = positioner_geometry(r(-0.5, -1.5, 2.0, 1.0), (10.0, 10.0));
        // x -0.5 → -1, right 1.5 → 2; y -1.5 → -2, bottom -0.5 → 0.
        assert_eq!(g.anchor, px(-1, -2, 3, 2));
    }

    #[test]
    fn negative_anchor_extent_collapses_to_zero() {
        let g = positioner_geometry(r(10.0, 10.0, -4.0, -0.5), (10.0, 10.0));
        assert_eq!(g.anchor, px(10, 10, 0, 0));
    }

    #[test]
    fn size_rounds_up_and_is_never_zero() {
        assert_eq!(
            positioner_geometry(r(0.0, 0.0, 1.0, 1.0), (199.2, 135.5)).size,
            (200, 136)
        );
        assert_eq!(
            positioner_geometry(r(0.0, 0.0, 1.0, 1.0), (0.0, -3.0)).size,
            (1, 1)
        );
    }

    #[test]
    fn every_edge_and_gravity_maps_to_its_xdg_value() {
        assert_eq!(AnchorEdge::None.to_xdg(), XdgAnchor::None);
        assert_eq!(AnchorEdge::Top.to_xdg(), XdgAnchor::Top);
        assert_eq!(AnchorEdge::Bottom.to_xdg(), XdgAnchor::Bottom);
        assert_eq!(AnchorEdge::Left.to_xdg(), XdgAnchor::Left);
        assert_eq!(AnchorEdge::Right.to_xdg(), XdgAnchor::Right);
        assert_eq!(AnchorEdge::TopLeft.to_xdg(), XdgAnchor::TopLeft);
        assert_eq!(AnchorEdge::TopRight.to_xdg(), XdgAnchor::TopRight);
        assert_eq!(AnchorEdge::BottomRight.to_xdg(), XdgAnchor::BottomRight);
        assert_eq!(Gravity::None.to_xdg(), XdgGravity::None);
        assert_eq!(Gravity::TopLeft.to_xdg(), XdgGravity::TopLeft);
        assert_eq!(Gravity::Bottom.to_xdg(), XdgGravity::Bottom);
    }

    #[test]
    fn queue_hands_out_increasing_ids_and_keeps_order() {
        struct Silent;
        impl crate::runtime::PopupShell for Silent {
            fn frame(&mut self, _: &mut crate::runtime::FrameCx<'_>) {}
            fn input(
                &mut self,
                _: crate::input::InputEvent,
                _: &mut crate::runtime::SurfaceCtl<'_>,
            ) -> PopupAction {
                PopupAction::Keep
            }
            fn dismissed(&mut self) {}
        }
        let mut queue = PopupQueue::default();
        let parent = crate::runtime::SurfaceId(7);
        let placement = Placement {
            anchor: r(0.0, 0.0, 1.0, 1.0),
            anchor_edge: AnchorEdge::BottomLeft,
            gravity: Gravity::BottomRight,
            offset: (0, 0),
        };
        let a = queue.open(parent, placement, (10.0, 10.0), Box::new(Silent));
        let b = queue.open(parent, placement, (10.0, 10.0), Box::new(Silent));
        queue.close(a);
        assert!(a.0 < b.0);
        let kinds: Vec<&str> = queue
            .ops
            .iter()
            .map(|op| match op {
                PopupOp::Open { .. } => "open",
                PopupOp::Close(_) => "close",
            })
            .collect();
        assert_eq!(kinds, ["open", "open", "close"]);
    }
}
