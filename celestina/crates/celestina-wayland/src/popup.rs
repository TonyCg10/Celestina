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
    Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::shell::client::{
    xdg_positioner::{Anchor as XdgAnchor, ConstraintAdjustment, Gravity as XdgGravity},
    xdg_wm_base::XdgWmBase,
};

pub use crate::ids::{CloseReason, PopupId};
use crate::{
    geometry::{PixelRect, Rect},
    input::InputEvent,
    runtime::{Core, Parent, PopupShell, Runtime, RuntimeError, Shell, SurfaceId, Target},
    surface::{Role, Surface},
};

/// What a popup shell answers to an input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupAction {
    Keep,
    Close,
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
    /// The edge mirrored left-right (what `FlipX` does to it).
    #[must_use]
    pub fn flip_x(self) -> Self {
        match self {
            AnchorEdge::Left => AnchorEdge::Right,
            AnchorEdge::Right => AnchorEdge::Left,
            AnchorEdge::TopLeft => AnchorEdge::TopRight,
            AnchorEdge::TopRight => AnchorEdge::TopLeft,
            AnchorEdge::BottomLeft => AnchorEdge::BottomRight,
            AnchorEdge::BottomRight => AnchorEdge::BottomLeft,
            same @ (AnchorEdge::None | AnchorEdge::Top | AnchorEdge::Bottom) => same,
        }
    }

    /// The edge mirrored top-bottom (what `FlipY` does to it).
    #[must_use]
    pub fn flip_y(self) -> Self {
        match self {
            AnchorEdge::Top => AnchorEdge::Bottom,
            AnchorEdge::Bottom => AnchorEdge::Top,
            AnchorEdge::TopLeft => AnchorEdge::BottomLeft,
            AnchorEdge::BottomLeft => AnchorEdge::TopLeft,
            AnchorEdge::TopRight => AnchorEdge::BottomRight,
            AnchorEdge::BottomRight => AnchorEdge::TopRight,
            same @ (AnchorEdge::None | AnchorEdge::Left | AnchorEdge::Right) => same,
        }
    }

    /// The anchor point on `rect`: a corner, the middle of an edge, or
    /// the centre for `None`.
    fn point(self, rect: PixelRect) -> (i32, i32) {
        let x = match self {
            AnchorEdge::Left | AnchorEdge::TopLeft | AnchorEdge::BottomLeft => rect.x,
            AnchorEdge::Right | AnchorEdge::TopRight | AnchorEdge::BottomRight => rect.x + rect.w,
            AnchorEdge::None | AnchorEdge::Top | AnchorEdge::Bottom => rect.x + rect.w / 2,
        };
        let y = match self {
            AnchorEdge::Top | AnchorEdge::TopLeft | AnchorEdge::TopRight => rect.y,
            AnchorEdge::Bottom | AnchorEdge::BottomLeft | AnchorEdge::BottomRight => {
                rect.y + rect.h
            }
            AnchorEdge::None | AnchorEdge::Left | AnchorEdge::Right => rect.y + rect.h / 2,
        };
        (x, y)
    }

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
    /// The direction mirrored left-right (what `FlipX` does to it).
    #[must_use]
    pub fn flip_x(self) -> Self {
        match self {
            Gravity::Left => Gravity::Right,
            Gravity::Right => Gravity::Left,
            Gravity::TopLeft => Gravity::TopRight,
            Gravity::TopRight => Gravity::TopLeft,
            Gravity::BottomLeft => Gravity::BottomRight,
            Gravity::BottomRight => Gravity::BottomLeft,
            same @ (Gravity::None | Gravity::Top | Gravity::Bottom) => same,
        }
    }

    /// The direction mirrored top-bottom (what `FlipY` does to it).
    #[must_use]
    pub fn flip_y(self) -> Self {
        match self {
            Gravity::Top => Gravity::Bottom,
            Gravity::Bottom => Gravity::Top,
            Gravity::TopLeft => Gravity::BottomLeft,
            Gravity::BottomLeft => Gravity::TopLeft,
            Gravity::TopRight => Gravity::BottomRight,
            Gravity::BottomRight => Gravity::TopRight,
            same @ (Gravity::None | Gravity::Left | Gravity::Right) => same,
        }
    }

    /// The top-left of a popup of `size` growing from `point` in this
    /// direction; a `None` component centres that axis on the point.
    fn origin(self, point: (i32, i32), size: (i32, i32)) -> (i32, i32) {
        let x = match self {
            Gravity::Left | Gravity::TopLeft | Gravity::BottomLeft => point.0 - size.0,
            Gravity::Right | Gravity::TopRight | Gravity::BottomRight => point.0,
            Gravity::None | Gravity::Top | Gravity::Bottom => point.0 - size.0 / 2,
        };
        let y = match self {
            Gravity::Top | Gravity::TopLeft | Gravity::TopRight => point.1 - size.1,
            Gravity::Bottom | Gravity::BottomLeft | Gravity::BottomRight => point.1,
            Gravity::None | Gravity::Left | Gravity::Right => point.1 - size.1 / 2,
        };
        (x, y)
    }

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

impl Placement {
    /// The placement `FlipX` turns this one into: anchor edge and gravity
    /// mirrored left-right, the offset kept (smithay's positioner, which
    /// niri runs, keeps it).
    #[must_use]
    pub fn flip_x(self) -> Self {
        Self {
            anchor_edge: self.anchor_edge.flip_x(),
            gravity: self.gravity.flip_x(),
            ..self
        }
    }

    /// The placement `FlipY` turns this one into.
    #[must_use]
    pub fn flip_y(self) -> Self {
        Self {
            anchor_edge: self.anchor_edge.flip_y(),
            gravity: self.gravity.flip_y(),
            ..self
        }
    }
}

/// Every popup may be slid and flipped on both axes to stay on the output.
pub fn constraint_adjustment() -> ConstraintAdjustment {
    ConstraintAdjustment::SlideX
        | ConstraintAdjustment::SlideY
        | ConstraintAdjustment::FlipX
        | ConstraintAdjustment::FlipY
}

/// The rect a compositor gives the popup when nothing constrains it, in the
/// parent surface's logical px: the anchor point of `geometry.anchor` for
/// the placement's edge, the popup grown from it by the gravity, plus the
/// offset (`xdg_positioner` semantics).
pub fn unconstrained(placement: &Placement, geometry: PositionerGeometry) -> PixelRect {
    let point = placement.anchor_edge.point(geometry.anchor);
    let (x, y) = placement.gravity.origin(point, geometry.size);
    PixelRect {
        x: x + placement.offset.0,
        y: y + placement.offset.1,
        w: geometry.size.0,
        h: geometry.size.1,
    }
}

/// Where the popup lands inside `bounds` (the output, in the parent
/// surface's logical px) under [`constraint_adjustment`]: per axis, the
/// unconstrained rect if it fits, else the flipped placement if that fits,
/// else the unconstrained rect slid into the bounds (aligned with the
/// near edge when it is larger than them). This models the protocol's
/// order (flip, then slide) so a product can predict a placement and tests
/// can check one; the compositor's `configure` is the authority.
pub fn resolve(
    placement: &Placement,
    geometry: PositionerGeometry,
    bounds: PixelRect,
) -> PixelRect {
    let plain = unconstrained(placement, geometry);
    let fits_x = |r: PixelRect| r.x >= bounds.x && r.x + r.w <= bounds.x + bounds.w;
    let fits_y = |r: PixelRect| r.y >= bounds.y && r.y + r.h <= bounds.y + bounds.h;
    let x = if fits_x(plain) {
        plain.x
    } else {
        let flipped = unconstrained(&placement.flip_x(), geometry);
        if fits_x(flipped) {
            flipped.x
        } else {
            slide(plain.x, plain.w, bounds.x, bounds.w)
        }
    };
    let y = if fits_y(plain) {
        plain.y
    } else {
        let flipped = unconstrained(&placement.flip_y(), geometry);
        if fits_y(flipped) {
            flipped.y
        } else {
            slide(plain.y, plain.h, bounds.y, bounds.h)
        }
    };
    PixelRect { x, y, ..plain }
}

/// Slides a span of `len` at `pos` into `[start, start + room)` by the
/// shortest move; one that cannot fit keeps its start edge inside.
fn slide(pos: i32, len: i32, start: i32, room: i32) -> i32 {
    let far = start + room - len;
    if pos + len > start + room {
        far.max(start)
    } else if pos < start {
        start
    } else {
        pos
    }
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

/// One input serial and its place in the order the presses arrived in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub serial: u32,
    /// Counts up from 1 with every press the seat delivers.
    pub order: u64,
}

/// The serials of the last pointer press and the last key press. A popup
/// grabs with the newer of the two by arrival, never by serial value
/// (serials wrap): a menu opened from the keyboard grabs with the key's
/// serial, not a stale click's, and a click after a key press wins again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GrabSerial {
    pointer: Option<Arrival>,
    key: Option<Arrival>,
    arrivals: u64,
}

impl GrabSerial {
    /// A pointer button press arrived with `serial`.
    pub fn pointer_press(&mut self, serial: u32) {
        self.pointer = Some(self.arrive(serial));
    }

    /// A key press arrived with `serial`.
    pub fn key_press(&mut self, serial: u32) {
        self.key = Some(self.arrive(serial));
    }

    fn arrive(&mut self, serial: u32) -> Arrival {
        self.arrivals = self.arrivals.saturating_add(1);
        Arrival {
            serial,
            order: self.arrivals,
        }
    }

    /// The serial a popup opened now grabs with; `None` before any press.
    #[must_use]
    pub fn current(&self) -> Option<u32> {
        GrabSerial::latest(self.pointer, self.key)
    }

    /// The serial of whichever press arrived last.
    #[must_use]
    pub fn latest(pointer: Option<Arrival>, key: Option<Arrival>) -> Option<u32> {
        match (pointer, key) {
            (Some(p), Some(k)) => Some(if k.order > p.order {
                k.serial
            } else {
                p.serial
            }),
            (Some(p), None) => Some(p.serial),
            (None, Some(k)) => Some(k.serial),
            (None, None) => None,
        }
    }
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
    /// The popup whose surface is `surface` wants `size` at its placement.
    Resize {
        surface: SurfaceId,
        size: (f32, f32),
    },
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

    pub fn resize(&mut self, surface: SurfaceId, size: (f32, f32)) {
        self.ops.push_back(PopupOp::Resize { surface, size });
    }
}

/// A live popup: its surface (role `Popup`), its shell, the surface it
/// belongs to (a layer surface, or another popup's surface when nested)
/// and the placement it was opened at, which a resize repositions with.
pub struct PopupEntry {
    pub id: PopupId,
    pub parent: SurfaceId,
    pub surface: Surface,
    pub shell: Box<dyn PopupShell>,
    pub frames: u64,
    pub placement: Placement,
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

    /// The popup that gets a key while `focus` has the keyboard: the newest
    /// popup of the chain rooted on the focused surface (the one holding
    /// the grab), whether the focus sits on the layer surface or on a popup
    /// of the chain.
    pub(crate) fn popup_for_focus(&self, focus: SurfaceId) -> Option<usize> {
        let id = self.targets().key_target(focus)?;
        self.popups.iter().position(|p| p.id == id)
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
                PopupOp::Resize { surface, size } => self.resize_popup(surface, size),
            }
        }
    }

    /// Asks the compositor to reposition the popup whose surface is
    /// `surface` at its placement with `size` (`xdg_popup.reposition`, from
    /// version 3 of the shell). The new size arrives with the configure
    /// that follows; until then the popup keeps the old one. A popup that
    /// is gone, a shell older than that, or a positioner the compositor
    /// refuses leave the popup as it is.
    fn resize_popup(&mut self, surface: SurfaceId, size: (f32, f32)) {
        let Some(idx) = self.popups.iter().position(|p| p.surface.id == surface) else {
            return;
        };
        let (id, placement) = (self.popups[idx].id, self.popups[idx].placement);
        let Some(wm) = self.wm_base.as_ref() else {
            return;
        };
        let geometry = positioner_geometry(placement.anchor, size);
        let positioner = match positioner(wm, &placement, geometry) {
            Ok(positioner) => positioner,
            Err(err) => {
                tracing::warn!(popup = id.0, %err, "popup: cannot reposition");
                return;
            }
        };
        self.reposition_tokens = self.reposition_tokens.wrapping_add(1);
        let token = self.reposition_tokens;
        let Some(popup) = self.popups[idx].surface.popup() else {
            return;
        };
        if popup.xdg_popup().version() < 3 {
            tracing::warn!(
                popup = id.0,
                version = popup.xdg_popup().version(),
                "popup: the shell is too old to reposition; the popup keeps its size"
            );
            return;
        }
        popup.reposition(&positioner, token);
        tracing::info!(
            popup = id.0,
            size = ?geometry.size,
            token,
            "popup: repositioned, waiting for configure"
        );
    }

    /// Creates the popup in protocol order: positioner, `xdg_surface` +
    /// `xdg_popup` (its parent the parent popup's `xdg_surface` when
    /// nested, else set through `layer_surface.get_popup`; both before the
    /// first commit), grab, scale and blur objects, then the initial commit
    /// without a buffer. The EGL window waits for the first `configure`. A
    /// parent that is not live (a popup already closed when its request is
    /// applied) is refused with `RuntimeError::NoParent`.
    fn open_popup(
        &mut self,
        id: PopupId,
        parent: SurfaceId,
        placement: Placement,
        size: (f32, f32),
        shell: Box<dyn PopupShell>,
    ) -> Result<(), RuntimeError> {
        let parent_kind = self.targets().parent_of(parent)?;
        let wm = self.wm_base.as_ref().ok_or(RuntimeError::NoWmBase)?;
        let geometry = positioner_geometry(placement.anchor, size);
        let positioner = positioner(wm, &placement, geometry).map_err(RuntimeError::Popup)?;
        let wl_surface = self.compositor.create_surface(&self.qh);
        let (popup, output) = match parent_kind {
            Parent::Layer => {
                let parent_surface = self
                    .surface_index(parent)
                    .map(|idx| &self.surfaces[idx])
                    .ok_or(RuntimeError::NoParent(parent))?;
                let layer = parent_surface
                    .layer()
                    .ok_or(RuntimeError::NoParent(parent))?;
                let popup = Popup::from_surface(None, &positioner, &self.qh, wl_surface, wm)
                    .map_err(RuntimeError::Popup)?;
                layer.get_popup(popup.xdg_popup());
                (popup, parent_surface.output().clone())
            }
            Parent::Popup(parent_id) => {
                let parent_surface = self
                    .popups
                    .iter()
                    .find(|p| p.id == parent_id)
                    .map(|p| &p.surface)
                    .ok_or(RuntimeError::NoParent(parent))?;
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

        // A nested popup grabs too (a grabbing parent demands it, or the
        // compositor answers `invalid_grab`), with the current serial: a
        // submenu opened by hover reuses the press that opened its parent.
        let grab_serial = match (&self.seat, self.grab_serial.current()) {
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
            placement,
        });
        Ok(())
    }

    /// Closes popup `idx` for `reason`: its descendants first, deepest
    /// first, each with `CloseReason::Parent` (`Targets::close_popup`), so
    /// no `xdg_popup` outlives its parent. The parent of each closed popup
    /// gets `PopupClosed` unless it is going too: a parent popup closed by
    /// the same plan, a layer surface being destroyed (`ParentGone`) or
    /// the runtime exiting (`Exit`).
    pub(crate) fn close_popup_at(&mut self, idx: usize, reason: CloseReason) {
        let plan = self.targets().close_popup(self.popups[idx].id, reason);
        let doomed: Vec<SurfaceId> = plan
            .iter()
            .filter_map(|(id, _)| self.popups.iter().find(|p| p.id == *id))
            .map(|p| p.surface.id)
            .collect();
        for (id, reason) in plan {
            let Some(idx) = self.popups.iter().position(|p| p.id == id) else {
                continue;
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
            let parent = entry.parent;
            self.destroy_surface_object(entry.surface);
            let parent_stays = !matches!(reason, CloseReason::Exit | CloseReason::ParentGone)
                && !doomed.contains(&parent);
            if parent_stays {
                self.pending_events
                    .push((parent, InputEvent::PopupClosed { id, reason }));
            }
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

    fn origin(rect: PixelRect) -> (i32, i32) {
        (rect.x, rect.y)
    }

    /// A submenu hangs from the top-right corner of its row, growing down
    /// and right.
    fn submenu(row: Rect) -> Placement {
        Placement {
            anchor: row,
            anchor_edge: AnchorEdge::TopRight,
            gravity: Gravity::BottomRight,
            offset: (0, 0),
        }
    }

    #[test]
    fn submenu_opens_right_of_its_row_and_flips_left_at_the_output_edge() {
        // A row of the menu at x 0..220, y 80..120; the menu's surface sits
        // 500 px from the output's right edge.
        let p = submenu(r(0.0, 80.0, 220.0, 40.0));
        let g = positioner_geometry(p.anchor, (200.0, 136.0));
        let wide = px(-700, -100, 1200, 700);
        assert_eq!(unconstrained(&p, g), px(220, 80, 200, 136));
        assert_eq!(resolve(&p, g, wide), px(220, 80, 200, 136));
        // 320 px from the right edge (the submenu needs 420): it flips to
        // the row's left.
        let narrow = px(-700, -100, 1020, 700);
        assert_eq!(resolve(&p, g, narrow), px(-200, 80, 200, 136));
    }

    #[test]
    fn submenu_flips_up_at_the_bottom_and_slides_when_neither_way_fits() {
        // The row at y 600..640 of a 700 px tall output: 60 px below it.
        let p = submenu(r(0.0, 600.0, 220.0, 40.0));
        let g = positioner_geometry(p.anchor, (200.0, 136.0));
        let bounds = px(-100, 0, 1000, 700);
        // Flipped on Y: the popup's bottom on the row's bottom.
        assert_eq!(origin(resolve(&p, g, bounds)), (220, 504));
        // A popup taller than the room above and below slides up instead.
        let tall = positioner_geometry(p.anchor, (200.0, 650.0));
        assert_eq!(origin(resolve(&p, tall, bounds)), (220, 50));
        // One taller than the output aligns with its top edge.
        let huge = positioner_geometry(p.anchor, (200.0, 900.0));
        assert_eq!(origin(resolve(&p, huge, bounds)), (220, 0));
    }

    #[test]
    fn flipping_swaps_the_horizontal_or_vertical_side_only() {
        assert_eq!(AnchorEdge::TopRight.flip_x(), AnchorEdge::TopLeft);
        assert_eq!(AnchorEdge::TopRight.flip_y(), AnchorEdge::BottomRight);
        assert_eq!(AnchorEdge::Top.flip_x(), AnchorEdge::Top);
        assert_eq!(AnchorEdge::None.flip_y(), AnchorEdge::None);
        assert_eq!(Gravity::BottomRight.flip_x(), Gravity::BottomLeft);
        assert_eq!(Gravity::BottomRight.flip_y(), Gravity::TopRight);
        assert_eq!(Gravity::Left.flip_y(), Gravity::Left);
    }

    #[test]
    fn centred_edges_and_gravities_centre_the_popup() {
        let p = Placement {
            anchor: r(100.0, 100.0, 50.0, 20.0),
            anchor_edge: AnchorEdge::Bottom,
            gravity: Gravity::Bottom,
            offset: (0, 4),
        };
        let g = positioner_geometry(p.anchor, (30.0, 10.0));
        assert_eq!(unconstrained(&p, g), px(110, 124, 30, 10));
        let p = Placement {
            anchor_edge: AnchorEdge::None,
            gravity: Gravity::None,
            offset: (0, 0),
            ..p
        };
        assert_eq!(unconstrained(&p, g), px(110, 105, 30, 10));
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
        assert_eq!(origin(unconstrained(&p, g)), (480, 66));
    }

    #[test]
    fn menu_grabs_with_the_last_input_serial_pointer_or_key() {
        let pointer = |serial, order| Some(Arrival { serial, order });
        let key = pointer;
        // Nothing pressed yet: no grab.
        assert_eq!(GrabSerial::latest(None, None), None);
        assert_eq!(GrabSerial::latest(pointer(40, 1), None), Some(40));
        assert_eq!(GrabSerial::latest(None, key(41, 1)), Some(41));
        // The newer arrival wins, whatever the serials' values (they wrap).
        assert_eq!(GrabSerial::latest(pointer(900, 1), key(5, 2)), Some(5));
        assert_eq!(GrabSerial::latest(pointer(5, 3), key(900, 2)), Some(5));

        // The recorder numbers the arrivals itself.
        let mut grab = GrabSerial::default();
        assert_eq!(grab.current(), None);
        grab.pointer_press(u32::MAX);
        assert_eq!(grab.current(), Some(u32::MAX));
        // A key press after the click (Enter on the overflow button): the key's serial.
        grab.key_press(3);
        assert_eq!(grab.current(), Some(3));
        // Then a click again: the click's.
        grab.pointer_press(2);
        assert_eq!(grab.current(), Some(2));
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
        queue.resize(crate::runtime::SurfaceId(9), (10.0, 20.0));
        assert!(a.0 < b.0);
        let kinds: Vec<&str> = queue
            .ops
            .iter()
            .map(|op| match op {
                PopupOp::Open { .. } => "open",
                PopupOp::Close(_) => "close",
                PopupOp::Resize { .. } => "resize",
            })
            .collect();
        assert_eq!(kinds, ["open", "open", "close", "resize"]);
    }
}
