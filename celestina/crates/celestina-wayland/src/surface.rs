//! One surface with a role (a layer surface or an xdg_popup): configure,
//! viewport, fractional scale, blur and input regions, EGL window, frame
//! pacing, the coalesced redraw timer and the regions pending for the next
//! frame. Both roles share all of it; only the role object differs.
#![forbid(unsafe_code)]

use std::{rc::Rc, time::Instant};

use calloop::RegistrationToken;
use smithay_client_toolkit::{
    compositor::{CompositorState, FrameCallbackData},
    dispatch2::Dispatch2,
    error::GlobalError,
    shell::{
        wlr_layer::{self, LayerSurface},
        xdg::popup::Popup,
        WaylandSurface,
    },
};
use wayland_client::{
    protocol::{wl_callback::WlCallback, wl_surface::WlSurface},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::{self, WpFractionalScaleV1},
    },
    text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use crate::{
    blur::{build_region, BlurSurface, RegionCache},
    egl::{Egl, EglError, EglWindow},
    geometry::Rect,
    runtime::{KeyboardInteractivity, OutputId, SurfaceId},
};

/// Buffer size for a logical size at a scale: round, never truncate, and
/// never zero.
pub fn buffer_size(logical: (u32, u32), scale: f64) -> (u32, u32) {
    let side = |px: u32| ((f64::from(px) * scale).round() as u32).max(1);
    (side(logical.0), side(logical.1))
}

/// `wp_fractional_scale_v1.preferred_scale` carries the scale in 120ths.
pub fn scale_from_preferred(preferred: u32) -> f64 {
    f64::from(preferred) / 120.0
}

/// Frame pacing state. Redraws are paced by `wl_surface.frame` callbacks
/// only; an idle surface requests no callback and draws no frame.
#[derive(Debug, Default)]
pub struct FramePacing {
    in_frame: bool,
    callback_pending: bool,
    redraw_requested: bool,
}

impl FramePacing {
    /// Records a redraw request. Returns `true` when the caller must request
    /// a frame callback and commit now (outside a frame, nothing in flight,
    /// surface mapped); otherwise the swap or the pending callback covers it.
    pub fn request_redraw(&mut self, mapped: bool) -> bool {
        self.redraw_requested = true;
        if !mapped || self.in_frame || self.callback_pending {
            return false;
        }
        self.callback_pending = true;
        true
    }

    pub fn begin_frame(&mut self) {
        self.in_frame = true;
        self.redraw_requested = false;
    }

    /// Ends the frame; returns whether a frame callback must be requested
    /// before the swap.
    pub fn end_frame(&mut self) -> bool {
        self.in_frame = false;
        if self.redraw_requested {
            self.callback_pending = true;
        }
        self.redraw_requested
    }

    /// The frame callback fired; returns whether to draw.
    pub fn frame_done(&mut self) -> bool {
        self.callback_pending = false;
        self.redraw_requested
    }
}

/// The one redraw timer of a surface: `redraw_after` calls coalesce into
/// the earliest deadline, and the runtime keeps a single calloop timer for
/// it, re-armed only when the deadline moves earlier.
#[derive(Debug, Default)]
pub struct RedrawTimer {
    deadline: Option<Instant>,
    /// The deadline the loop timer must be (re)registered for, until the
    /// runtime takes it.
    rearm: Option<Instant>,
    token: Option<RegistrationToken>,
}

impl RedrawTimer {
    /// Arms the timer for `deadline`, or keeps the earlier deadline already
    /// armed.
    pub fn arm(&mut self, deadline: Instant) {
        if self.deadline.is_some_and(|armed| armed <= deadline) {
            return;
        }
        self.deadline = Some(deadline);
        self.rearm = Some(deadline);
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// The deadline the runtime must register a loop timer for, once per
    /// change.
    pub fn take_rearm(&mut self) -> Option<Instant> {
        self.rearm.take()
    }

    /// The loop timer fired: nothing is armed any more.
    pub fn fired(&mut self) {
        self.deadline = None;
        self.rearm = None;
        self.token = None;
    }

    /// Stores the token of the registered loop timer, returning the stale
    /// one to remove.
    pub fn replace_token(&mut self, token: RegistrationToken) -> Option<RegistrationToken> {
        self.token.replace(token)
    }

    /// The registered loop timer, when the surface goes away.
    pub fn take_token(&mut self) -> Option<RegistrationToken> {
        self.token.take()
    }
}

/// Regions a shell set since the last frame; they reach the compositor
/// with that frame's commit, never with a bare one.
#[derive(Debug, Default)]
pub struct PendingRegions {
    blur: Option<Vec<Rect>>,
    input: Option<Vec<Rect>>,
}

impl PendingRegions {
    pub fn set_blur(&mut self, rects: &[Rect]) {
        self.blur = Some(rects.to_vec());
    }

    pub fn set_input(&mut self, rects: &[Rect]) {
        self.input = Some(rects.to_vec());
    }

    /// Both regions set since the last frame, each once; `None` when
    /// nothing was set.
    pub fn take_for_frame(&mut self) -> (Option<Vec<Rect>>, Option<Vec<Rect>>) {
        (self.blur.take(), self.input.take())
    }
}

/// Logical size and scale of a surface.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub logical: (u32, u32),
    pub scale: f64,
}

impl Geometry {
    pub fn buffer(&self) -> (u32, u32) {
        buffer_size(self.logical, self.scale)
    }
}

/// The role object of a surface. Dropping it destroys the role, then the
/// `wl_surface` (sctk orders both).
pub enum Role {
    Layer(LayerSurface),
    Popup(Popup),
}

impl Role {
    fn wl_surface(&self) -> &WlSurface {
        match self {
            Role::Layer(layer) => layer.wl_surface(),
            Role::Popup(popup) => popup.wl_surface(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Role::Layer(_) => "layer",
            Role::Popup(_) => "popup",
        }
    }
}

/// Which output a surface was created on (a popup inherits its parent's).
#[derive(Clone, Debug)]
pub struct OutputRef {
    pub id: OutputId,
    pub name: Rc<str>,
}

/// Protocol objects for one surface (layer surface or popup).
pub struct Surface {
    pub id: SurfaceId,
    role: Role,
    viewport: WpViewport,
    fractional: WpFractionalScaleV1,
    blur: Option<BlurSurface>,
    egl_window: Option<EglWindow>,
    geometry: Geometry,
    pacing: FramePacing,
    output: OutputRef,
    blur_cache: RegionCache,
    input_cache: RegionCache,
    pending: PendingRegions,
    redraw_timer: RedrawTimer,
    keyboard: KeyboardInteractivity,
    /// A bare commit carrying a frame request is due (outside a frame).
    commit_pending: bool,
}

/// User data of the per-surface fractional-scale object.
pub struct ScaleData(pub SurfaceId);

/// User data for objects that never send events.
pub struct NoEvents;

/// The globals a surface needs at creation.
#[derive(Clone, Copy)]
pub struct Protocols<'a> {
    pub compositor: &'a CompositorState,
    pub viewporter: &'a WpViewporter,
    pub fractional_scale: &'a WpFractionalScaleManagerV1,
}

/// The runtime side of `wp_fractional_scale_v1.preferred_scale`.
pub trait PreferredScaleSink {
    fn preferred_scale(&mut self, id: SurfaceId, preferred: u32);
}

impl Surface {
    /// Wraps a freshly created role object (initial commit sent by the
    /// caller after this returns). The input region starts empty so the
    /// transparent surface never swallows input before the first frame.
    pub fn new<D>(
        id: SurfaceId,
        role: Role,
        logical: (u32, u32),
        output: OutputRef,
        protocols: Protocols<'_>,
        blur: Option<BlurSurface>,
        qh: &QueueHandle<D>,
    ) -> Result<Self, GlobalError>
    where
        D: Dispatch<WpViewport, NoEvents> + Dispatch<WpFractionalScaleV1, ScaleData> + 'static,
    {
        let wl_surface = role.wl_surface();
        let viewport = protocols.viewporter.get_viewport(wl_surface, qh, NoEvents);
        let fractional =
            protocols
                .fractional_scale
                .get_fractional_scale(wl_surface, qh, ScaleData(id));
        let mut surface = Self {
            id,
            role,
            viewport,
            fractional,
            blur,
            egl_window: None,
            geometry: Geometry {
                logical,
                scale: 1.0,
            },
            pacing: FramePacing::default(),
            output,
            blur_cache: RegionCache::default(),
            input_cache: RegionCache::default(),
            pending: PendingRegions::default(),
            redraw_timer: RedrawTimer::default(),
            keyboard: KeyboardInteractivity::None,
            commit_pending: false,
        };
        surface.send_input_region(protocols.compositor, &[])?;
        Ok(surface)
    }

    pub fn wl_surface(&self) -> &WlSurface {
        self.role.wl_surface()
    }

    /// The layer surface, when that is the role.
    pub fn layer(&self) -> Option<&LayerSurface> {
        match &self.role {
            Role::Layer(layer) => Some(layer),
            Role::Popup(_) => None,
        }
    }

    /// The popup, when that is the role.
    pub fn popup(&self) -> Option<&Popup> {
        match &self.role {
            Role::Popup(popup) => Some(popup),
            Role::Layer(_) => None,
        }
    }

    pub fn is_popup(&self) -> bool {
        matches!(self.role, Role::Popup(_))
    }

    /// The output the surface lives on.
    pub fn output(&self) -> &OutputRef {
        &self.output
    }

    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    pub fn egl_window(&self) -> Option<&EglWindow> {
        self.egl_window.as_ref()
    }

    pub fn is_mapped(&self) -> bool {
        self.egl_window.is_some()
    }

    pub fn pacing_mut(&mut self) -> &mut FramePacing {
        &mut self.pacing
    }

    pub fn pending_regions_mut(&mut self) -> &mut PendingRegions {
        &mut self.pending
    }

    pub fn redraw_timer_mut(&mut self) -> &mut RedrawTimer {
        &mut self.redraw_timer
    }

    /// Applies a `configure`. Returns `true` on the first configure, when the
    /// caller must draw directly (the surface is not mapped yet, so no frame
    /// callback would arrive).
    pub fn configure(&mut self, new_size: (u32, u32), egl: &Egl) -> Result<bool, EglError> {
        if new_size.0 != 0 && new_size.1 != 0 {
            self.geometry.logical = new_size;
        }
        if self.egl_window.is_some() {
            self.apply_geometry();
            self.request_redraw();
            return Ok(false);
        }
        let buffer = self.geometry.buffer();
        let window = egl.create_window(self.wl_surface(), buffer)?;
        self.egl_window = Some(window);
        if self.wl_surface().version() >= 3 {
            self.wl_surface().set_buffer_scale(1);
        }
        self.apply_geometry();
        Ok(true)
    }

    /// `wp_fractional_scale_v1.preferred_scale`.
    pub fn set_preferred_scale(&mut self, preferred: u32) {
        self.set_scale(scale_from_preferred(preferred));
    }

    fn set_scale(&mut self, scale: f64) {
        if (scale - self.geometry.scale).abs() < f64::EPSILON {
            return;
        }
        self.geometry.scale = scale;
        if self.egl_window.is_some() {
            self.apply_geometry();
            self.request_redraw();
        }
    }

    fn apply_geometry(&mut self) {
        let logical = self.geometry.logical;
        let buffer = self.geometry.buffer();
        if let Some(window) = &self.egl_window {
            window.resize(buffer);
        }
        self.viewport
            .set_destination(logical.0 as i32, logical.1 as i32);
        tracing::debug!(
            surface = self.id.0,
            role = self.role.name(),
            scale = self.geometry.scale,
            logical = ?logical,
            buffer = ?buffer,
            "geometry applied"
        );
    }

    /// Schedules the next frame. Outside a frame a bare commit carrying the
    /// frame request is due, sent by the runtime's flush; inside a frame the
    /// swap carries it.
    pub fn request_redraw(&mut self) {
        if self.pacing.request_redraw(self.is_mapped()) {
            self.commit_pending = true;
        }
    }

    /// Sends the bare frame-request commit that `request_redraw` made due,
    /// if any.
    pub fn flush_commit<D>(&mut self, qh: &QueueHandle<D>)
    where
        D: Dispatch<WlCallback, FrameCallbackData> + 'static,
    {
        if !std::mem::take(&mut self.commit_pending) {
            return;
        }
        self.request_frame_callback(qh);
        self.wl_surface().commit();
    }

    pub fn request_frame_callback<D>(&self, qh: &QueueHandle<D>)
    where
        D: Dispatch<WlCallback, FrameCallbackData> + 'static,
    {
        let wl_surface = self.wl_surface();
        wl_surface.frame(qh, FrameCallbackData(wl_surface.clone()));
    }

    /// Sends the regions the shell set since the last frame, right before
    /// that frame's swap commits them. `no_blur` drops the blur region (the
    /// `--no-blur` attribution run).
    pub fn apply_pending_regions(&mut self, compositor: &CompositorState, no_blur: bool) {
        let (blur, input) = self.pending.take_for_frame();
        if let Some(rects) = blur.filter(|_| !no_blur) {
            if let Err(err) = self.send_blur_region(compositor, &rects) {
                tracing::error!(%err, surface = self.id.0, "cannot build the blur region");
            }
        }
        if let Some(rects) = input {
            if let Err(err) = self.send_input_region(compositor, &rects) {
                tracing::error!(%err, surface = self.id.0, "cannot build the input region");
            }
        }
    }

    /// Blur region in logical px; empty disables blur. Sent only on change.
    fn send_blur_region(
        &mut self,
        compositor: &CompositorState,
        rects: &[Rect],
    ) -> Result<(), GlobalError> {
        let Some(blur) = &self.blur else {
            return Ok(());
        };
        let Some(pixels) = self.blur_cache.update(rects) else {
            return Ok(());
        };
        if pixels.is_empty() {
            blur.set_blur_region(None);
        } else {
            let region = build_region(compositor, pixels)?;
            blur.set_blur_region(Some(&region));
        }
        tracing::trace!(surface = self.id.0, ?pixels, "blur region updated");
        Ok(())
    }

    /// Input region in logical px; empty means pass-through. Sent only on
    /// change.
    fn send_input_region(
        &mut self,
        compositor: &CompositorState,
        rects: &[Rect],
    ) -> Result<(), GlobalError> {
        let Some(pixels) = self.input_cache.update(rects) else {
            return Ok(());
        };
        let region = build_region(compositor, pixels)?;
        self.role
            .wl_surface()
            .set_input_region(Some(region.wl_region()));
        tracing::trace!(surface = self.id.0, ?pixels, "input region updated");
        Ok(())
    }

    /// Takes effect on the next commit. Layer surfaces only: a popup's
    /// keyboard follows its grab.
    pub fn set_keyboard_interactivity(&mut self, keyboard: KeyboardInteractivity) {
        let Role::Layer(layer) = &self.role else {
            return;
        };
        if self.keyboard == keyboard {
            return;
        }
        self.keyboard = keyboard;
        layer.set_keyboard_interactivity(match keyboard {
            KeyboardInteractivity::None => wlr_layer::KeyboardInteractivity::None,
            KeyboardInteractivity::OnDemand => wlr_layer::KeyboardInteractivity::OnDemand,
            KeyboardInteractivity::Exclusive => wlr_layer::KeyboardInteractivity::Exclusive,
        });
    }

    /// Takes effect on the next commit. Layer surfaces only.
    pub fn set_exclusive_zone(&mut self, zone: i32) {
        if let Role::Layer(layer) = &self.role {
            layer.set_exclusive_zone(zone);
        }
    }

    /// Destroys the EGL window and every protocol object, in order: EGL
    /// window, blur, fractional scale, viewport, then the role and the
    /// `wl_surface` (dropped with `self`). The caller removes the redraw
    /// timer from the loop first.
    pub fn destroy(mut self, egl: &Egl) {
        if let Some(window) = self.egl_window.take() {
            egl.destroy_window(window);
        }
        self.blur = None;
        self.fractional.destroy();
        self.viewport.destroy();
        tracing::debug!(
            surface = self.id.0,
            role = self.role.name(),
            "surface destroyed"
        );
    }
}

impl<D: PreferredScaleSink> Dispatch2<WpFractionalScaleV1, D> for ScaleData {
    fn event(
        &self,
        runtime: &mut D,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            tracing::debug!(surface = self.0 .0, preferred = scale, "preferred_scale");
            runtime.preferred_scale(self.0, scale);
        }
    }
}

/// Objects whose interface declares no events: an event is a compositor
/// bug, logged and ignored rather than a crash of the shell.
macro_rules! no_events {
    ($($iface:ty),* $(,)?) => {
        $(
            impl<D> Dispatch2<$iface, D> for NoEvents {
                fn event(
                    &self,
                    _: &mut D,
                    _: &$iface,
                    event: <$iface as Proxy>::Event,
                    _: &Connection,
                    _: &QueueHandle<D>,
                ) {
                    tracing::warn!(
                        interface = <$iface as Proxy>::interface().name,
                        ?event,
                        "event on an interface that declares none"
                    );
                }
            }
        )*
    };
}

no_events!(
    WpFractionalScaleManagerV1,
    WpViewporter,
    WpViewport,
    ZwpTextInputManagerV3,
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn buffer_size_at_integer_scales() {
        assert_eq!(buffer_size((760, 560), 1.0), (760, 560));
        assert_eq!(buffer_size((760, 560), 2.0), (1520, 1120));
    }

    #[test]
    fn buffer_size_at_fractional_scale() {
        assert_eq!(buffer_size((760, 560), 1.5), (1140, 840));
    }

    #[test]
    fn buffer_size_rounds_instead_of_truncating() {
        // 333 × 1.5 = 499.5 → 500, 101 × 1.5 = 151.5 → 152.
        assert_eq!(buffer_size((333, 101), 1.5), (500, 152));
        // 761 × 1.25 = 951.25 → 951.
        assert_eq!(buffer_size((761, 560), 1.25), (951, 700));
    }

    #[test]
    fn buffer_size_is_never_zero() {
        assert_eq!(buffer_size((0, 0), 1.5), (1, 1));
    }

    #[test]
    fn preferred_scale_is_in_120ths() {
        assert_eq!(scale_from_preferred(120), 1.0);
        assert_eq!(scale_from_preferred(180), 1.5);
        assert_eq!(scale_from_preferred(240), 2.0);
    }

    #[test]
    fn pacing_idle_surface_requests_no_callback() {
        let mut p = FramePacing::default();
        p.begin_frame();
        assert!(!p.end_frame(), "no request_redraw → no frame callback");
        // A frame callback that was never requested cannot arrive, but be safe.
        assert!(!p.frame_done());
    }

    #[test]
    fn pacing_request_inside_frame_defers_to_callback() {
        let mut p = FramePacing::default();
        p.begin_frame();
        assert!(
            !p.request_redraw(true),
            "inside a frame the swap carries the request"
        );
        assert!(p.end_frame());
        assert!(
            !p.request_redraw(true),
            "callback pending → nothing to send now"
        );
        assert!(p.frame_done(), "callback arrived → draw");
    }

    #[test]
    fn pacing_request_outside_frame_sends_a_callback_once() {
        let mut p = FramePacing::default();
        assert!(p.request_redraw(true));
        assert!(!p.request_redraw(true));
        assert!(p.frame_done());
        p.begin_frame();
        assert!(!p.end_frame());
    }

    #[test]
    fn pacing_request_before_map_only_flags() {
        let mut p = FramePacing::default();
        assert!(!p.request_redraw(false));
        assert!(p.redraw_requested);
    }

    #[test]
    fn redraw_after_rearms_one_timer_per_surface() {
        let now = Instant::now();
        let mut timer = RedrawTimer::default();
        assert_eq!(timer.take_rearm(), None, "nothing armed yet");

        timer.arm(now + Duration::from_millis(100));
        assert_eq!(timer.deadline(), Some(now + Duration::from_millis(100)));
        // A later deadline never postpones the armed one.
        timer.arm(now + Duration::from_millis(500));
        assert_eq!(timer.deadline(), Some(now + Duration::from_millis(100)));
        // The runtime registers one calloop timer for it, once.
        assert_eq!(timer.take_rearm(), Some(now + Duration::from_millis(100)));
        assert_eq!(timer.take_rearm(), None);

        // An earlier deadline replaces it: the loop timer is re-armed.
        timer.arm(now + Duration::from_millis(40));
        assert_eq!(timer.deadline(), Some(now + Duration::from_millis(40)));
        assert_eq!(timer.take_rearm(), Some(now + Duration::from_millis(40)));

        // Firing clears everything; the next arm starts over.
        timer.fired();
        assert_eq!(timer.deadline(), None);
        assert_eq!(timer.take_rearm(), None);
        timer.arm(now + Duration::from_millis(900));
        assert_eq!(timer.take_rearm(), Some(now + Duration::from_millis(900)));
    }

    #[test]
    fn regions_apply_only_with_the_next_frame() {
        let mut pending = PendingRegions::default();
        assert_eq!(
            pending.take_for_frame(),
            (None, None),
            "nothing before a set"
        );

        let blur = [Rect::whole((100.0, 40.0))];
        let input = [Rect {
            x: 10.0,
            y: 0.0,
            w: 80.0,
            h: 40.0,
        }];
        pending.set_blur(&blur);
        pending.set_input(&input);
        // The last value before the frame wins.
        pending.set_blur(&[blur[0], input[0]]);
        assert_eq!(
            pending.take_for_frame(),
            (Some(vec![blur[0], input[0]]), Some(input.to_vec()))
        );
        assert_eq!(pending.take_for_frame(), (None, None), "taken once");

        pending.set_input(&[]);
        assert_eq!(
            pending.take_for_frame(),
            (None, Some(Vec::new())),
            "an empty region is a value, not an absence"
        );
    }
}
