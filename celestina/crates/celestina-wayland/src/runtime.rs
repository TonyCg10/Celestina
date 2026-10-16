//! The runtime: the calloop loop, the `Shell` and `PopupShell` contracts,
//! the controls a shell drives surfaces and the runtime with, outputs,
//! layer surfaces and `run()`. The sctk handler impls live in `handlers`,
//! the popup side in `popup`.
#![forbid(unsafe_code)]

use std::{
    ffi::c_void,
    rc::Rc,
    time::{Duration, Instant},
};

use calloop::{
    channel::{self, Event as ChannelEvent},
    signals::{Signal, Signals},
    timer::{TimeoutAction, Timer},
    EventLoop, LoopHandle, LoopSignal,
};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::CompositorState,
    error::GlobalError as SctkGlobalError,
    output::OutputState,
    registry::RegistryState,
    seat::{pointer::cursor_shape::CursorShapeManager, pointer::PointerData, SeatState},
    shell::{
        wlr_layer::{self, Anchor, LayerShell},
        WaylandSurface,
    },
};
use wayland_client::{
    globals::{registry_queue_init, BindError, GlobalError, GlobalList},
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    ConnectError, Connection, DispatchError, Proxy, QueueHandle,
};
use wayland_protocols::wp::{
    cursor_shape::v1::client::wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
    fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    viewporter::client::wp_viewporter::WpViewporter,
};

use crate::{
    blur::Blur,
    data_device::Clipboard,
    egl::{Egl, EglError},
    geometry::Rect,
    input::{CursorShape, InputEvent, Modifiers, SeatRequest, SeatRequests},
    metrics::{self, Metrics},
    popup::{
        CloseReason, GrabSerial, Placement, PopupAction, PopupEntry, PopupId, PopupQueue, WmBase,
    },
    surface::{NoEvents, OutputRef, Protocols, Role, Surface},
    text_input::TextInput,
};

/// What a run of the shell is told from the command line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Stop the loop after this long (`--exit-after`, for captures).
    pub exit_after: Option<Duration>,
    /// Hidden dev flag `--no-blur`: blur regions are never sent, to
    /// attribute frame cost to the blur.
    pub no_blur: bool,
    /// Count frame times, late frames and RSS, and return the summary.
    pub metrics: bool,
}

/// An output, by the global name of its `wl_output`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

/// What the product needs to know about an output.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    pub id: OutputId,
    /// The connector name (`DP-1`), or `wl_output-<id>` when the compositor
    /// gives none.
    pub name: String,
    /// Logical size: `xdg_output`'s when offered, else the current mode
    /// divided by the integer scale.
    pub logical_size: (u32, u32),
    /// The output's integer scale; the real (fractional) scale of a surface
    /// arrives per surface through `wp_fractional_scale_v1`.
    pub scale: f64,
    /// Refresh rate of the current mode in mHz, 0 when unknown.
    pub refresh_mhz: i32,
}

/// Hotplug, mode and scale changes, delivered to `Shell::output`.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    Added(OutputInfo),
    Changed(OutputInfo),
    Removed(OutputId),
}

/// The layer-shell layers the shell uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Top,
    Overlay,
}

/// The layer-shell keyboard policy (§5.6): `None` at rest, `OnDemand` with
/// a page open, `Exclusive` for the launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardInteractivity {
    None,
    OnDemand,
    Exclusive,
}

/// Where a layer surface sits on its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchoring {
    /// Anchored to the top edge, at the spec's size.
    Top,
    /// Centred on the output, at the spec's size.
    Centred,
    /// Anchored to all four edges: the compositor sizes it to the output
    /// and tells the size in the configure; the spec's size is not sent.
    Fill,
}

impl Anchoring {
    /// The layer-shell anchor and the size to request for `size`: a filling
    /// surface asks for 0×0, which the protocol defines as "as large as the
    /// anchors allow".
    pub(crate) fn request(self, size: (u32, u32)) -> (Anchor, (u32, u32)) {
        match self {
            Anchoring::Top => (Anchor::TOP, size),
            Anchoring::Centred => (Anchor::empty(), size),
            Anchoring::Fill => (Anchor::all(), (0, 0)),
        }
    }
}

/// How a layer surface is created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerSpec {
    pub output: OutputId,
    pub namespace: &'static str,
    pub layer: Layer,
    pub anchor: Anchoring,
    /// Logical size; a [`Anchoring::Fill`] surface learns its size from the
    /// configure instead.
    pub size: (u32, u32),
    pub exclusive_zone: i32,
    pub keyboard: KeyboardInteractivity,
}

/// The product side of the runtime: one implementation owns every surface.
pub trait Shell: 'static {
    /// What workers publish through the [`Sender`].
    type Message: Send + 'static;
    /// An output appeared, changed or went away (its surfaces and popups
    /// are already destroyed when `Removed` arrives).
    fn output(&mut self, ev: OutputEvent, ctl: &mut RuntimeCtl<'_>);
    /// Called once per paced frame of a surface that requested a redraw.
    fn frame(&mut self, cx: &mut FrameCx<'_>);
    /// Pointer and keyboard input for a layer surface.
    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>);
    /// A message a worker sent through the [`Sender`].
    fn message(&mut self, msg: Self::Message, ctl: &mut RuntimeCtl<'_>);
}

/// An `xdg_popup` child of a surface, with its own EGL window, fractional
/// scale, viewport and blur object, drawn with the same painter pipeline
/// (the GL context is shared; make-current per surface).
pub trait PopupShell: 'static {
    /// Called once per paced frame of the popup's surface.
    fn frame(&mut self, cx: &mut FrameCx<'_>);
    /// Pointer events on the popup's surface, and every key while the popup
    /// is open. `ctl` controls the popup's own surface.
    fn input(&mut self, ev: InputEvent, ctl: &mut SurfaceCtl<'_>) -> PopupAction;
    /// The compositor sent `popup_done` (e.g. a click outside); the popup is
    /// destroyed right after this returns.
    fn dismissed(&mut self);
}

/// A layer surface or a popup surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u32);

/// One frame of one surface.
pub struct FrameCx<'a> {
    pub surface: SurfaceId,
    /// Name of the output the surface is on.
    pub output: &'a str,
    /// Logical size.
    pub size: (f32, f32),
    /// Device pixel ratio (1.5 on DP-1).
    pub scale: f64,
    pub now: Instant,
    /// `eglGetProcAddress`, for loaders that build their own GL table.
    pub proc_address: &'a dyn Fn(&str) -> *const c_void,
    /// The compositor offers `ext_background_effect_v1`; without it the
    /// painter uses the opaque fallback.
    pub blur_available: bool,
    pub ctl: SurfaceCtl<'a>,
    /// The loop's stop, for [`FrameCx::stop`].
    signal: &'a LoopSignal,
}

impl FrameCx<'_> {
    /// Stops the loop once this frame is done, as `--exit-after` does: `run`
    /// returns after the shutdown. For a failure a frame cannot recover
    /// from (the painter cannot be created).
    pub fn stop(&self) {
        self.signal.stop();
    }
}

/// Borrows one surface. Everything it records is applied by the runtime
/// right after the shell call returns, or with the next frame for regions.
pub struct SurfaceCtl<'a> {
    surface: &'a mut Surface,
    popups: &'a mut PopupQueue,
    cursor: &'a mut Option<CursorShape>,
    seat: &'a mut SeatRequests,
    signal: &'a LoopSignal,
}

impl SurfaceCtl<'_> {
    /// The surface this control belongs to.
    pub fn id(&self) -> SurfaceId {
        self.surface.id
    }

    /// Name of the output this surface was created on (a popup reports its
    /// parent's).
    pub fn output_name(&self) -> &str {
        &self.surface.output().name
    }

    /// Schedules the next frame. Outside a frame the runtime commits a bare
    /// frame request; inside a frame the swap carries it.
    pub fn request_redraw(&mut self) {
        self.surface.request_redraw();
    }

    /// `request_redraw` after `d`, coalesced: one timer per surface, kept
    /// at the earliest deadline asked for.
    pub fn redraw_after(&mut self, d: Duration) {
        self.surface.redraw_timer_mut().arm(Instant::now() + d);
    }

    /// Blur region in logical px; empty = no blur. Sent with the next
    /// frame's commit.
    pub fn set_blur_region(&mut self, rects: &[Rect]) {
        self.surface.pending_regions_mut().set_blur(rects);
    }

    /// Input region in logical px; empty = pass-through. Sent with the next
    /// frame's commit.
    pub fn set_input_region(&mut self, rects: &[Rect]) {
        self.surface.pending_regions_mut().set_input(rects);
    }

    /// Layer surfaces only; takes effect on the next commit.
    pub fn set_keyboard_interactivity(&mut self, k: KeyboardInteractivity) {
        self.surface.set_keyboard_interactivity(k);
    }

    /// Layer surfaces only; takes effect on the next commit.
    pub fn set_exclusive_zone(&mut self, zone: i32) {
        self.surface.set_exclusive_zone(zone);
    }

    /// The cursor to show while the pointer is over this surface; a no-op
    /// on a compositor without `wp_cursor_shape_v1`.
    pub fn set_cursor(&mut self, shape: CursorShape) {
        *self.cursor = Some(shape);
    }

    /// IME text input for this surface while it has the keyboard focus:
    /// on while a text field of it is focused, off otherwise (the default).
    /// The input method's text then arrives as `InputEvent::Preedit` and
    /// `InputEvent::CommitText`. A no-op without `zwp_text_input_v3`.
    pub fn set_text_input_enabled(&mut self, enabled: bool) {
        self.seat
            .push((self.surface.id, SeatRequest::TextInput(enabled)));
    }

    /// The caret's rectangle in logical px of this surface, where the input
    /// method places its candidates; sent when it changes while text input
    /// is enabled.
    pub fn set_text_input_rect(&mut self, rect: Rect) {
        self.seat
            .push((self.surface.id, SeatRequest::CursorRect(rect)));
    }

    /// Reads the clipboard's text off the loop and delivers it to this
    /// surface as `InputEvent::Paste`. Nothing arrives when the clipboard
    /// holds no UTF-8 text, the text is not valid UTF-8, its owner does not
    /// answer in time, a read is already in flight, or the compositor has
    /// no `wl_data_device_manager`.
    pub fn request_paste(&mut self) {
        self.seat.push((self.surface.id, SeatRequest::Paste));
    }

    /// Opens an `xdg_popup` of this surface at `placement` (logical px of
    /// this surface), grabbing with the serial of the most recent pointer
    /// or key press. Applied right after the current shell call returns;
    /// one popup per parent (an open one is closed first); a popup may
    /// open its own (nested menus). However it ends, the parent gets
    /// `InputEvent::PopupClosed` with the reason.
    pub fn open_popup(
        &mut self,
        placement: Placement,
        size: (f32, f32),
        shell: Box<dyn PopupShell>,
    ) -> PopupId {
        self.popups.open(self.surface.id, placement, size, shell)
    }

    /// Closes popup `id` (no-op if it is already gone).
    pub fn close_popup(&mut self, id: PopupId) {
        self.popups.close(id);
    }

    /// Popups only: asks the compositor for `size` (logical px) at the
    /// placement this popup was opened with. Applied right after the
    /// current shell call returns; the new size comes with a later frame's
    /// `size`, and until then the popup keeps the old one. A no-op on a
    /// layer surface, and on a shell older than `xdg_wm_base` version 3.
    pub fn resize_popup(&mut self, size: (f32, f32)) {
        if self.surface.is_popup() {
            self.popups.resize(self.surface.id, size);
        }
    }

    /// Stops the loop once the current shell call returns; `run` returns
    /// after the shutdown. For an input that ends the shell (a menu's quit
    /// row), where no `RuntimeCtl` is at hand.
    pub fn stop(&mut self) {
        self.signal.stop();
    }
}

/// What the runtime does for the product outside a surface: surfaces come
/// and go, the loop stops.
pub struct RuntimeCtl<'a> {
    ops: &'a mut dyn RuntimeOps,
}

impl RuntimeCtl<'_> {
    /// Creates a layer surface on `spec.output`; its first frame follows
    /// the compositor's configure.
    pub fn create_layer_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        self.ops.create_layer_surface(spec)
    }

    /// Destroys a layer surface and its popups (no `PopupClosed` follows).
    pub fn destroy_surface(&mut self, id: SurfaceId) {
        self.ops.destroy_surface(id);
    }

    /// The control of a live surface (layer or popup).
    pub fn surface(&mut self, id: SurfaceId) -> Option<SurfaceCtl<'_>> {
        self.ops.surface_ctl(id)
    }

    /// The surface of a live popup, so a shell can reach a popup it opened
    /// in an earlier call (to ask for its redraw when what it shows
    /// changed); `None` for a popup that is gone or not yet applied.
    pub fn popup_surface(&self, id: PopupId) -> Option<SurfaceId> {
        self.ops.popup_surface(id)
    }

    /// Ids of the live layer surfaces, in creation order.
    pub fn surfaces(&self) -> Vec<SurfaceId> {
        self.ops.surfaces()
    }

    /// Stops the loop; `run` returns after the shutdown.
    pub fn stop(&mut self) {
        self.ops.stop();
    }
}

/// The runtime behind a `RuntimeCtl`, without the shell type.
trait RuntimeOps {
    fn create_layer_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError>;
    fn destroy_surface(&mut self, id: SurfaceId);
    fn surface_ctl(&mut self, id: SurfaceId) -> Option<SurfaceCtl<'_>>;
    fn popup_surface(&self, id: PopupId) -> Option<SurfaceId>;
    fn surfaces(&self) -> Vec<SurfaceId>;
    fn stop(&mut self);
}

/// Publishes messages into the loop from any thread; workers clone it.
pub struct Sender<M>(channel::Sender<M>);

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> std::fmt::Debug for Sender<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sender")
    }
}

impl<M> Sender<M> {
    /// Queues `msg` for `Shell::message`; fails once the loop is gone.
    pub fn send(&self, msg: M) -> Result<(), RuntimeGone> {
        self.0.send(msg).map_err(|_| RuntimeGone)
    }
}

/// The loop stopped receiving messages: the sender's worker should exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the runtime stopped receiving messages")]
pub struct RuntimeGone;

/// What `run` reports at exit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunReport {
    /// Frames drawn over the run, every surface counted.
    pub frames: u64,
    /// The metrics summary when `Config.metrics` was set.
    pub metrics: Option<String>,
}

/// What stops the runtime from starting or serving a request.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("cannot {what}")]
    Loop {
        what: &'static str,
        #[source]
        source: calloop::Error,
    },
    #[error("cannot connect to the Wayland display")]
    Connect(#[source] ConnectError),
    #[error("cannot read the Wayland registry")]
    Registry(#[source] GlobalError),
    #[error("the compositor lacks the required global {interface}")]
    MissingGlobal {
        interface: &'static str,
        #[source]
        source: BindError,
    },
    #[error("the Wayland round trip failed")]
    Dispatch(#[source] DispatchError),
    #[error(transparent)]
    Egl(#[from] EglError),
    #[error("output {0:?} is unknown")]
    UnknownOutput(OutputId),
    #[error("cannot create a wl_region")]
    Region(#[source] SctkGlobalError),
    #[error("the compositor has no xdg_wm_base: popups are unavailable")]
    NoWmBase,
    #[error("popup parent {0:?} is not a live surface")]
    NoParent(SurfaceId),
    #[error("cannot create the popup")]
    Popup(#[source] SctkGlobalError),
}

fn insert_error<T>(what: &'static str) -> impl FnOnce(calloop::InsertError<T>) -> RuntimeError {
    move |err| RuntimeError::Loop {
        what,
        source: err.error,
    }
}

/// The runtime with its shell: the loop's state type.
pub struct Runtime<S: Shell> {
    pub(crate) core: Core<S>,
    pub(crate) shell: S,
}

/// Everything of the runtime but the shell, so a shell call can borrow the
/// shell and the core disjointly.
pub(crate) struct Core<S: Shell> {
    pub(crate) config: Config,
    conn: Connection,
    pub(crate) qh: QueueHandle<Runtime<S>>,
    handle: LoopHandle<'static, Runtime<S>>,
    pub(crate) signal: LoopSignal,
    pub(crate) registry_state: RegistryState,
    pub(crate) output_state: OutputState,
    pub(crate) seat_state: SeatState,
    pub(crate) compositor: CompositorState,
    layer_shell: LayerShell,
    pub(crate) wm_base: Option<WmBase>,
    pub(crate) viewporter: WpViewporter,
    pub(crate) fractional_scale: WpFractionalScaleManagerV1,
    pub(crate) blur: Blur,
    pub(crate) egl: Egl,
    cursor_shape: Option<CursorShapeManager>,
    pub(crate) text_input: TextInput,
    pub(crate) clipboard: Clipboard,
    /// Text-input and paste requests of the running shell call.
    seat_requests: SeatRequests,
    shape_device: Option<WpCursorShapeDeviceV1>,
    pending_cursor: Option<CursorShape>,
    current_cursor: Option<CursorShape>,
    pub(crate) surfaces: Vec<Surface>,
    pub(crate) popups: Vec<PopupEntry>,
    pub(crate) popup_queue: PopupQueue,
    /// The token of the last `xdg_popup.reposition` sent; each one is new.
    pub(crate) reposition_tokens: u32,
    /// Input events produced while a shell call was running (popup
    /// closures), delivered by the next flush.
    pub(crate) pending_events: Vec<(SurfaceId, InputEvent)>,
    pub(crate) seat: Option<wl_seat::WlSeat>,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    /// The modifiers of the last `update_modifiers`.
    pub(crate) modifiers: Modifiers,
    /// The last pointer and key press serials: the popup grab serial is
    /// the newer of the two.
    pub(crate) grab_serial: GrabSerial,
    /// Layer surface or popup surface.
    pub(crate) keyboard_focus: Option<SurfaceId>,
    pub(crate) next_surface_id: u32,
    frames_drawn: u64,
    pub(crate) metrics: Option<Metrics>,
}

/// Builds the runtime, delivers the outputs, lets `setup` start the
/// product's workers with a [`Sender`] and register extra calloop sources,
/// then runs until a signal, `exit_after` or `RuntimeCtl::stop`.
pub fn run<S: Shell>(
    cfg: Config,
    shell: S,
    setup: impl FnOnce(Sender<S::Message>, &LoopHandle<'static, Runtime<S>>),
) -> Result<RunReport, RuntimeError> {
    let mut event_loop: EventLoop<'static, Runtime<S>> =
        EventLoop::try_new().map_err(|source| RuntimeError::Loop {
            what: "create the event loop",
            source,
        })?;
    let handle = event_loop.handle();

    let conn = Connection::connect_to_env().map_err(RuntimeError::Connect)?;
    let (globals, mut queue) =
        registry_queue_init::<Runtime<S>>(&conn).map_err(RuntimeError::Registry)?;
    let qh = queue.handle();
    let core = Core::new(
        cfg,
        conn.clone(),
        &globals,
        &qh,
        handle.clone(),
        event_loop.get_signal(),
    )?;
    let mut rt = Runtime { core, shell };

    let (sender, receiver) = channel::channel::<S::Message>();
    handle
        .insert_source(receiver, |event, _, rt| {
            if let ChannelEvent::Msg(msg) = event {
                rt.message(msg);
            }
        })
        .map_err(insert_error("insert the message channel"))?;

    // Two round trips: the first binds the outputs, the second delivers
    // their names and geometry; `Shell::output(Added)` fires for each.
    queue.roundtrip(&mut rt).map_err(RuntimeError::Dispatch)?;
    queue.roundtrip(&mut rt).map_err(RuntimeError::Dispatch)?;

    WaylandSource::new(conn, queue)
        .insert(handle.clone())
        .map_err(insert_error("insert the Wayland source"))?;

    let signals =
        Signals::new(&[Signal::SIGINT, Signal::SIGTERM]).map_err(|source| RuntimeError::Loop {
            what: "create the signal source",
            source,
        })?;
    handle
        .insert_source(signals, |event, _, rt| {
            tracing::info!(signal = ?event.signal(), "signal received, exiting");
            rt.core.signal.stop();
        })
        .map_err(insert_error("insert the signal source"))?;

    if let Some(after) = rt.core.config.exit_after {
        handle
            .insert_source(Timer::from_duration(after), |_, _, rt| {
                tracing::info!("--exit-after elapsed, exiting");
                rt.core.signal.stop();
                TimeoutAction::Drop
            })
            .map_err(insert_error("insert the exit timer"))?;
    }

    if rt.core.metrics.is_some() {
        handle
            .insert_source(Timer::from_duration(metrics::RSS_EVERY), |_, _, rt| {
                if let Some(metrics) = &mut rt.core.metrics {
                    metrics.sample_rss();
                }
                TimeoutAction::ToDuration(metrics::RSS_EVERY)
            })
            .map_err(insert_error("insert the RSS timer"))?;
    }

    setup(Sender(sender), &handle);

    event_loop
        .run(None, &mut rt, |_| {})
        .map_err(|source| RuntimeError::Loop {
            what: "run the event loop",
            source,
        })?;
    Ok(rt.shutdown())
}

impl<S: Shell> Core<S> {
    fn new(
        config: Config,
        conn: Connection,
        globals: &GlobalList,
        qh: &QueueHandle<Runtime<S>>,
        handle: LoopHandle<'static, Runtime<S>>,
        signal: LoopSignal,
    ) -> Result<Self, RuntimeError> {
        let missing = |interface: &'static str| {
            move |source: BindError| RuntimeError::MissingGlobal { interface, source }
        };
        let compositor = CompositorState::bind(globals, qh).map_err(missing("wl_compositor"))?;
        let layer_shell = LayerShell::bind(globals, qh).map_err(missing("zwlr_layer_shell_v1"))?;
        // Both are required: buffers are sized at the real (fractional) DPR
        // and scaled back by the viewport; there is no integer-scale path.
        let viewporter = globals
            .bind::<WpViewporter, _, _>(qh, 1..=1, NoEvents)
            .map_err(missing("wp_viewporter"))?;
        let fractional_scale = globals
            .bind::<WpFractionalScaleManagerV1, _, _>(qh, 1..=1, NoEvents)
            .map_err(missing("wp_fractional_scale_manager_v1"))?;
        let blur = Blur::bind(globals, qh);
        let wm_base = WmBase::bind(globals, qh)
            .inspect_err(|err| tracing::warn!(%err, "no xdg_wm_base: popups disabled"))
            .ok();
        let cursor_shape = CursorShapeManager::bind(globals, qh)
            .inspect_err(|err| tracing::info!(%err, "no wp_cursor_shape_v1: set_cursor is a no-op"))
            .ok();
        let text_input = TextInput::bind(globals, qh);
        let (paste_tx, paste_rx) = channel::channel();
        handle
            .insert_source(paste_rx, |event, _, rt: &mut Runtime<S>| {
                if let ChannelEvent::Msg(done) = event {
                    rt.paste_done(done);
                }
            })
            .map_err(insert_error("insert the paste channel"))?;
        let clipboard = Clipboard::bind(globals, qh, paste_tx);
        let egl = Egl::new(&conn)?;
        let metrics = config.metrics.then(Metrics::default);
        Ok(Self {
            config,
            conn,
            qh: qh.clone(),
            handle,
            signal,
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            seat_state: SeatState::new(globals, qh),
            compositor,
            layer_shell,
            wm_base,
            viewporter,
            fractional_scale,
            blur,
            egl,
            cursor_shape,
            text_input,
            clipboard,
            seat_requests: SeatRequests::new(),
            shape_device: None,
            pending_cursor: None,
            current_cursor: None,
            surfaces: Vec::new(),
            popups: Vec::new(),
            popup_queue: PopupQueue::default(),
            reposition_tokens: 0,
            pending_events: Vec::new(),
            seat: None,
            keyboard: None,
            pointer: None,
            modifiers: Modifiers::default(),
            grab_serial: GrabSerial::default(),
            keyboard_focus: None,
            next_surface_id: 1,
            frames_drawn: 0,
            metrics,
        })
    }

    pub(crate) fn surface_index(&self, id: SurfaceId) -> Option<usize> {
        self.surfaces.iter().position(|s| s.id == id)
    }

    pub(crate) fn surface_index_by_wl(&self, wl_surface: &wl_surface::WlSurface) -> Option<usize> {
        self.surfaces
            .iter()
            .position(|s| s.wl_surface() == wl_surface)
    }

    pub(crate) fn target_by_wl(&self, wl_surface: &wl_surface::WlSurface) -> Option<Target> {
        self.surface_index_by_wl(wl_surface)
            .map(Target::Layer)
            .or_else(|| self.popup_index_by_wl(wl_surface).map(Target::Popup))
    }

    pub(crate) fn surface_at(&mut self, target: Target) -> &mut Surface {
        match target {
            Target::Layer(idx) => &mut self.surfaces[idx],
            Target::Popup(idx) => &mut self.popups[idx].surface,
        }
    }

    /// The target of a surface id, layer or popup.
    pub(crate) fn target_of(&self, id: SurfaceId) -> Option<Target> {
        self.surface_index(id).map(Target::Layer).or_else(|| {
            self.popups
                .iter()
                .position(|p| p.surface.id == id)
                .map(Target::Popup)
        })
    }

    /// A layer surface or a popup surface by id.
    pub(crate) fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces
            .iter_mut()
            .chain(self.popups.iter_mut().map(|p| &mut p.surface))
            .find(|s| s.id == id)
    }

    pub(crate) fn next_surface_id(&mut self) -> SurfaceId {
        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;
        id
    }

    pub(crate) fn protocols(&self) -> Protocols<'_> {
        Protocols {
            compositor: &self.compositor,
            viewporter: &self.viewporter,
            fractional_scale: &self.fractional_scale,
        }
    }

    /// The product's view of an output, from sctk's info.
    pub(crate) fn output_info(&self, output: &wl_output::WlOutput) -> Option<OutputInfo> {
        let info = self.output_state.info(output)?;
        let id = OutputId(info.id);
        let current = info.modes.iter().find(|m| m.current);
        let scale = f64::from(info.scale_factor.max(1));
        let logical_size = match info.logical_size {
            Some((w, h)) => (w.max(0) as u32, h.max(0) as u32),
            None => current.map_or((0, 0), |m| {
                let side = |px: i32| (f64::from(px.max(0)) / scale).round() as u32;
                (side(m.dimensions.0), side(m.dimensions.1))
            }),
        };
        Some(OutputInfo {
            id,
            name: info.name.unwrap_or_else(|| format!("wl_output-{}", id.0)),
            logical_size,
            scale,
            refresh_mhz: current.map_or(0, |m| m.refresh_rate),
        })
    }

    fn wl_output(&self, id: OutputId) -> Option<wl_output::WlOutput> {
        self.output_state
            .outputs()
            .find(|o| self.output_state.info(o).is_some_and(|i| i.id == id.0))
    }

    /// Refresh period of the output's current mode.
    fn refresh_period(&self, output: &wl_output::WlOutput) -> Option<Duration> {
        let info = self.output_state.info(output)?;
        let mode = info.modes.iter().find(|m| m.current)?;
        metrics::period_from_mhz(mode.refresh_rate)
    }

    /// The pure table of what lives on which output.
    pub(crate) fn targets(&self) -> Targets {
        let mut targets = Targets::default();
        for surface in &self.surfaces {
            targets.layer(surface.id, surface.output().id);
        }
        for popup in &self.popups {
            targets.popup(popup.id, popup.surface.id, popup.parent);
        }
        targets
    }

    /// Removes a layer surface and its loop timer; popups were closed first.
    pub(crate) fn destroy_layer_surface(&mut self, id: SurfaceId) {
        let Some(idx) = self.surface_index(id) else {
            return;
        };
        if self.keyboard_focus == Some(id) {
            self.keyboard_focus = None;
        }
        let surface = self.surfaces.remove(idx);
        self.destroy_surface_object(surface);
    }

    /// Destroys a surface (layer or popup) after removing its redraw timer.
    pub(crate) fn destroy_surface_object(&mut self, mut surface: Surface) {
        self.text_input.forget(surface.id);
        if let Some(token) = surface.redraw_timer_mut().take_token() {
            self.handle.remove(token);
        }
        surface.destroy(&self.egl);
    }

    /// Registers the loop timer of every surface whose redraw deadline
    /// moved, replacing the stale one.
    fn arm_timers(&mut self) {
        let Core {
            surfaces,
            popups,
            handle,
            ..
        } = self;
        let all = surfaces
            .iter_mut()
            .chain(popups.iter_mut().map(|p| &mut p.surface));
        for surface in all {
            let id = surface.id;
            let timer = surface.redraw_timer_mut();
            let Some(deadline) = timer.take_rearm() else {
                continue;
            };
            let inserted = handle.insert_source(
                Timer::from_deadline(deadline),
                move |_, _, rt: &mut Runtime<S>| {
                    if let Some(surface) = rt.core.surface_mut(id) {
                        surface.redraw_timer_mut().fired();
                        surface.request_redraw();
                    }
                    rt.flush();
                    TimeoutAction::Drop
                },
            );
            match inserted {
                Ok(token) => {
                    if let Some(stale) = timer.replace_token(token) {
                        handle.remove(stale);
                    }
                }
                Err(err) => tracing::error!(%err, surface = id.0, "cannot insert the redraw timer"),
            }
        }
    }

    /// Sends the bare frame-request commits `request_redraw` made due.
    fn flush_commits(&mut self) {
        let Core {
            surfaces,
            popups,
            qh,
            ..
        } = self;
        for surface in surfaces
            .iter_mut()
            .chain(popups.iter_mut().map(|p| &mut p.surface))
        {
            surface.flush_commit(qh);
        }
    }

    /// Applies the text-input and paste requests the shell calls recorded;
    /// turning text input off may clear a composition (queued as events).
    fn apply_seat_requests(&mut self) {
        for (id, request) in std::mem::take(&mut self.seat_requests) {
            match request {
                SeatRequest::Paste => self.clipboard.request(id, &self.conn),
                other => self.text_input.request(id, other),
            }
        }
        let events = self.text_input.sync();
        self.pending_events.extend(events);
    }

    /// Sends the cursor shape a shell asked for, through the shape device
    /// of the pointer, with its latest enter serial.
    fn apply_cursor(&mut self) {
        let Some(shape) = self.pending_cursor.take() else {
            return;
        };
        if self.current_cursor == Some(shape) {
            return;
        }
        let (Some(device), Some(pointer)) = (&self.shape_device, &self.pointer) else {
            return;
        };
        let serial = pointer
            .data::<PointerData<()>>()
            .and_then(PointerData::latest_enter_serial);
        let Some(serial) = serial else {
            return;
        };
        device.set_shape(serial, wp_shape(shape));
        self.current_cursor = Some(shape);
    }

    /// The pointer entered a surface: the compositor forgot our cursor.
    pub(crate) fn pointer_entered(&mut self) {
        self.current_cursor = None;
    }

    pub(crate) fn pointer_added(&mut self, pointer: wl_pointer::WlPointer) {
        if let Some(manager) = &self.cursor_shape {
            self.shape_device = Some(manager.get_shape_device(&pointer, &self.qh));
        }
        self.pointer = Some(pointer);
    }

    pub(crate) fn pointer_removed(&mut self) {
        if let Some(device) = self.shape_device.take() {
            device.destroy();
        }
        if let Some(pointer) = self.pointer.take() {
            pointer.release();
        }
        self.current_cursor = None;
    }
}

fn wp_shape(shape: CursorShape) -> wp_cursor_shape_device_v1::Shape {
    match shape {
        CursorShape::Default => wp_cursor_shape_device_v1::Shape::Default,
        CursorShape::Pointer => wp_cursor_shape_device_v1::Shape::Pointer,
        CursorShape::Text => wp_cursor_shape_device_v1::Shape::Text,
    }
}

impl<S: Shell> RuntimeOps for Core<S> {
    fn create_layer_surface(&mut self, spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        let output = self
            .wl_output(spec.output)
            .ok_or(RuntimeError::UnknownOutput(spec.output))?;
        let info = self
            .output_info(&output)
            .ok_or(RuntimeError::UnknownOutput(spec.output))?;
        let wl_surface = self.compositor.create_surface(&self.qh);
        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            wl_surface,
            match spec.layer {
                Layer::Top => wlr_layer::Layer::Top,
                Layer::Overlay => wlr_layer::Layer::Overlay,
            },
            Some(spec.namespace),
            Some(&output),
        );
        let (anchor, size) = spec.anchor.request(spec.size);
        layer.set_anchor(anchor);
        layer.set_size(size.0, size.1);
        layer.set_exclusive_zone(spec.exclusive_zone);
        layer.set_margin(0, 0, 0, 0);

        let blur = self.blur.surface(layer.wl_surface(), &self.qh);
        let id = self.next_surface_id();
        let output_ref = OutputRef {
            id: spec.output,
            name: Rc::from(info.name.as_str()),
        };
        let mut surface = Surface::new(
            id,
            Role::Layer(layer),
            size,
            output_ref,
            self.protocols(),
            blur,
            &self.qh,
        )
        .map_err(RuntimeError::Region)?;
        surface.set_keyboard_interactivity(spec.keyboard);
        let period = self.refresh_period(&output);
        if let Some(metrics) = &mut self.metrics {
            metrics.register(id, info.name.clone(), period);
        }
        // Initial commit without a buffer: the compositor answers with the
        // first configure.
        surface.wl_surface().commit();
        tracing::info!(
            surface = id.0,
            output = %info.name,
            namespace = spec.namespace,
            layer = ?spec.layer,
            size = ?spec.size,
            "layer surface created"
        );
        self.surfaces.push(surface);
        Ok(id)
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.close_popups_of(id);
        self.destroy_layer_surface(id);
    }

    fn surface_ctl(&mut self, id: SurfaceId) -> Option<SurfaceCtl<'_>> {
        let Core {
            surfaces,
            popups,
            popup_queue,
            pending_cursor,
            seat_requests,
            signal,
            ..
        } = self;
        let surface = surfaces
            .iter_mut()
            .chain(popups.iter_mut().map(|p| &mut p.surface))
            .find(|s| s.id == id)?;
        Some(SurfaceCtl {
            surface,
            popups: popup_queue,
            cursor: pending_cursor,
            seat: seat_requests,
            signal,
        })
    }

    fn popup_surface(&self, id: PopupId) -> Option<SurfaceId> {
        self.popups
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.surface.id)
    }

    fn surfaces(&self) -> Vec<SurfaceId> {
        self.surfaces.iter().map(|s| s.id).collect()
    }

    fn stop(&mut self) {
        self.signal.stop();
    }
}

/// Who draws a target's frame.
enum Sink<'a> {
    Shell(&'a mut dyn FnMut(&mut FrameCx<'_>)),
    Popup(&'a mut dyn PopupShell),
}

/// A surface the runtime draws: a layer surface or a popup, by index.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Target {
    Layer(usize),
    Popup(usize),
}

impl<S: Shell> Runtime<S> {
    /// Runs one frame of the target's shell and swaps (= commits), with the
    /// regions set since the last frame.
    pub(crate) fn draw(&mut self, target: Target) {
        let Runtime { core, shell } = self;
        let Core {
            surfaces,
            popups,
            egl,
            blur,
            compositor,
            qh,
            frames_drawn,
            popup_queue,
            pending_cursor,
            seat_requests,
            metrics,
            config,
            signal,
            ..
        } = core;
        let signal: &LoopSignal = signal;
        let mut shell_frame = |cx: &mut FrameCx<'_>| shell.frame(cx);
        let (surface, sink) = match target {
            Target::Layer(idx) => (&mut surfaces[idx], Sink::Shell(&mut shell_frame)),
            Target::Popup(idx) => {
                let PopupEntry { surface, shell, .. } = &mut popups[idx];
                (surface, Sink::Popup(shell.as_mut()))
            }
        };
        let Some(window) = surface.egl_window() else {
            return;
        };
        let geometry = surface.geometry();
        if let Err(err) = egl.make_current(window, geometry.buffer()) {
            tracing::error!(%err, "cannot make the EGL context current");
            return;
        }

        surface.pacing_mut().begin_frame();
        let started = Instant::now();
        let output = Rc::clone(&surface.output().name);
        {
            let proc_address = |name: &str| egl.proc_address(name);
            let mut cx = FrameCx {
                surface: surface.id,
                output: &output,
                size: (geometry.logical.0 as f32, geometry.logical.1 as f32),
                scale: geometry.scale,
                now: Instant::now(),
                proc_address: &proc_address,
                blur_available: blur.available(),
                signal,
                ctl: SurfaceCtl {
                    surface,
                    popups: popup_queue,
                    cursor: pending_cursor,
                    seat: seat_requests,
                    signal,
                },
            };
            match sink {
                Sink::Shell(frame) => frame(&mut cx),
                Sink::Popup(shell) => shell.frame(&mut cx),
            }
        }

        // Regions travel with this frame's commit, never with a bare one.
        surface.apply_pending_regions(compositor, config.no_blur);
        let next_callback = surface.pacing_mut().end_frame();
        if next_callback {
            surface.request_frame_callback(qh);
        }
        match surface.egl_window() {
            Some(window) => {
                if let Err(err) = egl.swap_buffers(window) {
                    tracing::error!(%err, "eglSwapBuffers failed");
                }
            }
            None => tracing::error!(
                surface = surface.id.0,
                "EGL window vanished during the frame"
            ),
        }
        // A frame that asks for another redraw is part of an animation.
        if let Some(metrics) = metrics {
            metrics.on_frame(surface.id, started, started.elapsed(), next_callback);
        }
        *frames_drawn += 1;
        tracing::debug!(
            frame = *frames_drawn,
            surface = surface.id.0,
            next_callback,
            "frame drawn"
        );
        if let Target::Popup(idx) = target {
            self.core.popups[idx].frames += 1;
        }
        self.flush();
    }

    /// Routes an input event to the layer shell or the popup shell that
    /// owns surface `id`; events for a surface that is gone are dropped.
    fn deliver(&mut self, id: SurfaceId, ev: InputEvent) {
        match self.core.target_of(id) {
            Some(Target::Layer(idx)) => {
                let Runtime { core, shell } = self;
                let mut ctl = SurfaceCtl {
                    surface: &mut core.surfaces[idx],
                    popups: &mut core.popup_queue,
                    cursor: &mut core.pending_cursor,
                    seat: &mut core.seat_requests,
                    signal: &core.signal,
                };
                shell.input(ev, &mut ctl);
            }
            Some(Target::Popup(idx)) => self.deliver_to_popup(idx, ev),
            None => tracing::debug!(surface = id.0, ?ev, "input for a surface that is gone"),
        }
    }

    fn deliver_to_popup(&mut self, idx: usize, ev: InputEvent) {
        let core = &mut self.core;
        let PopupEntry { surface, shell, .. } = &mut core.popups[idx];
        let mut ctl = SurfaceCtl {
            surface,
            popups: &mut core.popup_queue,
            cursor: &mut core.pending_cursor,
            seat: &mut core.seat_requests,
            signal: &core.signal,
        };
        if shell.input(ev, &mut ctl) == PopupAction::Close {
            core.close_popup_at(idx, CloseReason::Shell);
        }
    }

    /// Delivers an input event to the shell of surface `id` and applies
    /// what the call recorded.
    pub(crate) fn dispatch_input(&mut self, id: SurfaceId, ev: InputEvent) {
        self.deliver(id, ev);
        self.flush();
    }

    pub(crate) fn dispatch_popup_input(&mut self, idx: usize, ev: InputEvent) {
        self.deliver_to_popup(idx, ev);
        self.flush();
    }

    /// Delivers a worker's message.
    pub(crate) fn message(&mut self, msg: S::Message) {
        let Runtime { core, shell } = self;
        shell.message(msg, &mut RuntimeCtl { ops: core });
        self.flush();
    }

    pub(crate) fn output_event(&mut self, ev: OutputEvent) {
        let Runtime { core, shell } = self;
        shell.output(ev, &mut RuntimeCtl { ops: core });
        self.flush();
    }

    /// An output went away: its popups, then its layer surfaces, then the
    /// product is told.
    pub(crate) fn output_destroyed(&mut self, id: OutputId) {
        for step in self.core.targets().remove_output(id) {
            match step {
                Destroy::Popup(popup) => {
                    if let Some(idx) = self.core.popups.iter().position(|p| p.id == popup) {
                        self.core.close_popup_at(idx, CloseReason::ParentGone);
                    }
                }
                Destroy::Surface(surface) => self.core.destroy_layer_surface(surface),
            }
        }
        self.output_event(OutputEvent::Removed(id));
    }

    /// Applies everything the shell calls recorded: popup requests, text
    /// input and paste requests, the input events they produced (until none
    /// remain), timers, the cursor and the bare frame-request commits.
    pub(crate) fn flush(&mut self) {
        loop {
            self.core.flush_popup_ops();
            self.core.apply_seat_requests();
            let events = std::mem::take(&mut self.core.pending_events);
            if events.is_empty() {
                break;
            }
            for (id, ev) in events {
                self.deliver(id, ev);
            }
        }
        self.core.arm_timers();
        self.core.apply_cursor();
        self.core.flush_commits();
    }

    fn shutdown(mut self) -> RunReport {
        // Popups before their parents: a parent must outlive its popups.
        while !self.core.popups.is_empty() {
            let last = self.core.popups.len() - 1;
            self.core.close_popup_at(last, CloseReason::Exit);
        }
        while let Some(surface) = self.core.surfaces.pop() {
            self.core.destroy_surface_object(surface);
        }
        if let Some(wm_base) = self.core.wm_base.take() {
            wm_base.destroy();
        }
        self.core.pointer_removed();
        self.core.clipboard.shutdown();
        self.core.text_input.destroy();
        self.core.fractional_scale.destroy();
        self.core.viewporter.destroy();
        self.core.egl.shutdown();
        if let Err(err) = self.core.conn.flush() {
            tracing::warn!(%err, "final flush failed");
        }
        tracing::info!(frames = self.core.frames_drawn, "clean exit");
        RunReport {
            frames: self.core.frames_drawn,
            metrics: self.core.metrics.as_mut().map(Metrics::summary),
        }
    }
}

/// What the runtime holds on which output, as a pure table: the destroy
/// order of an output's removal is computed here and executed by the
/// runtime.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Targets {
    layers: Vec<(SurfaceId, OutputId)>,
    popups: Vec<PopupRow>,
}

#[derive(Debug, PartialEq, Eq)]
struct PopupRow {
    id: PopupId,
    surface: SurfaceId,
    parent: SurfaceId,
}

/// One step of a removal plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destroy {
    Popup(PopupId),
    Surface(SurfaceId),
}

/// What a popup is parented on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Parent {
    /// A layer surface: `zwlr_layer_surface_v1.get_popup`.
    Layer,
    /// A live popup: the child's `xdg_popup` parent is that popup's
    /// `xdg_surface`, and its grab nests in the parent's.
    Popup(PopupId),
}

impl Targets {
    /// What a popup opened on `surface` is parented on; a surface that is
    /// not live (a closed or unknown popup) is refused.
    pub(crate) fn parent_of(&self, surface: SurfaceId) -> Result<Parent, RuntimeError> {
        if self.layers.iter().any(|(s, _)| *s == surface) {
            return Ok(Parent::Layer);
        }
        self.popups
            .iter()
            .find(|row| row.surface == surface)
            .map(|row| Parent::Popup(row.id))
            .ok_or(RuntimeError::NoParent(surface))
    }

    /// The order popup `id` closes in: every descendant first, deepest
    /// first, each with `CloseReason::Parent`, then `id` itself with
    /// `reason`. Empty for an unknown popup.
    pub(crate) fn close_popup(
        &self,
        id: PopupId,
        reason: CloseReason,
    ) -> Vec<(PopupId, CloseReason)> {
        let Some(own) = self.popups.iter().find(|row| row.id == id) else {
            return Vec::new();
        };
        let mut chain = vec![(id, reason)];
        for descendant in self.descendants(own.surface) {
            chain.push((descendant, CloseReason::Parent));
        }
        chain.reverse();
        chain
    }

    /// The popup that gets the keys while `focus` has the keyboard: the
    /// newest popup of the chain rooted on `focus` (the one holding the
    /// grab), which is `focus` itself when it is a leaf popup; `None` when
    /// nothing is open on it.
    pub(crate) fn key_target(&self, focus: SurfaceId) -> Option<PopupId> {
        self.descendants(focus).last().copied().or_else(|| {
            self.popups
                .iter()
                .find(|row| row.surface == focus)
                .map(|row| row.id)
        })
    }

    /// Every popup rooted on `surface`, in creation order (parents before
    /// children).
    fn descendants(&self, surface: SurfaceId) -> Vec<PopupId> {
        let mut rooted = vec![surface];
        let mut found = Vec::new();
        // A child is created after its parent, so one pass in creation
        // order finds every descendant.
        for row in &self.popups {
            if rooted.contains(&row.parent) {
                rooted.push(row.surface);
                found.push(row.id);
            }
        }
        found
    }

    pub(crate) fn layer(&mut self, surface: SurfaceId, output: OutputId) {
        self.layers.push((surface, output));
    }

    /// A popup `id` drawn on `surface`, child of `parent` (a layer surface
    /// or another popup's surface). Call in creation order.
    pub(crate) fn popup(&mut self, id: PopupId, surface: SurfaceId, parent: SurfaceId) {
        self.popups.push(PopupRow {
            id,
            surface,
            parent,
        });
    }

    /// The destroy order when `output` goes away: every popup rooted on one
    /// of its layer surfaces, children before parents, then those layer
    /// surfaces in creation order. Nothing of another output appears.
    pub(crate) fn remove_output(&self, output: OutputId) -> Vec<Destroy> {
        let layers: Vec<SurfaceId> = self
            .layers
            .iter()
            .filter(|(_, o)| *o == output)
            .map(|(s, _)| *s)
            .collect();
        let mut popups: Vec<PopupId> = layers
            .iter()
            .flat_map(|layer| self.descendants(*layer))
            .collect();
        popups.reverse();
        popups
            .into_iter()
            .map(Destroy::Popup)
            .chain(layers.into_iter().map(Destroy::Surface))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filling_surface_anchors_every_edge_and_asks_for_no_size() {
        assert_eq!(Anchoring::Top.request((760, 40)), (Anchor::TOP, (760, 40)));
        assert_eq!(
            Anchoring::Centred.request((560, 400)),
            (Anchor::empty(), (560, 400))
        );
        let (anchor, size) = Anchoring::Fill.request((560, 400));
        assert_eq!(
            anchor,
            Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT
        );
        assert_eq!(size, (0, 0), "the compositor sizes it to the output");
    }

    #[test]
    fn removing_an_output_destroys_its_popups_before_its_surfaces_and_nothing_else() {
        let (a, b) = (OutputId(1), OutputId(2));
        let mut targets = Targets::default();
        targets.layer(SurfaceId(1), a);
        targets.layer(SurfaceId(2), a);
        targets.layer(SurfaceId(3), b);
        // A menu on surface 1, a submenu nested in it, and a menu on output B.
        targets.popup(PopupId(1), SurfaceId(10), SurfaceId(1));
        targets.popup(PopupId(2), SurfaceId(11), SurfaceId(10));
        targets.popup(PopupId(3), SurfaceId(12), SurfaceId(3));

        assert_eq!(
            targets.remove_output(a),
            vec![
                Destroy::Popup(PopupId(2)),
                Destroy::Popup(PopupId(1)),
                Destroy::Surface(SurfaceId(1)),
                Destroy::Surface(SurfaceId(2)),
            ]
        );
        assert_eq!(
            targets.remove_output(b),
            vec![Destroy::Popup(PopupId(3)), Destroy::Surface(SurfaceId(3))]
        );
        assert_eq!(targets.remove_output(OutputId(9)), Vec::new());
    }

    /// A menu on the Island (popup 1), its submenu (2), a sub-submenu (3)
    /// and an unrelated menu on another surface (4).
    fn nested() -> Targets {
        let mut targets = Targets::default();
        targets.layer(SurfaceId(1), OutputId(1));
        targets.layer(SurfaceId(2), OutputId(1));
        targets.popup(PopupId(1), SurfaceId(10), SurfaceId(1));
        targets.popup(PopupId(2), SurfaceId(11), SurfaceId(10));
        targets.popup(PopupId(3), SurfaceId(12), SurfaceId(11));
        targets.popup(PopupId(4), SurfaceId(13), SurfaceId(2));
        targets
    }

    #[test]
    fn nested_popups_close_children_first() {
        let targets = nested();
        // Dismissing the menu: the deepest child goes first, each child
        // because its parent closes, the menu itself for the given reason.
        assert_eq!(
            targets.close_popup(PopupId(1), CloseReason::Dismissed),
            vec![
                (PopupId(3), CloseReason::Parent),
                (PopupId(2), CloseReason::Parent),
                (PopupId(1), CloseReason::Dismissed),
            ]
        );
        // Closing the submenu leaves the menu alone.
        assert_eq!(
            targets.close_popup(PopupId(2), CloseReason::Shell),
            vec![
                (PopupId(3), CloseReason::Parent),
                (PopupId(2), CloseReason::Shell),
            ]
        );
        // A leaf closes alone; an unknown popup closes nothing.
        assert_eq!(
            targets.close_popup(PopupId(3), CloseReason::Dismissed),
            vec![(PopupId(3), CloseReason::Dismissed)]
        );
        assert_eq!(targets.close_popup(PopupId(9), CloseReason::Parent), vec![]);
    }

    #[test]
    fn removing_an_output_destroys_a_popup_chain_deepest_first() {
        assert_eq!(
            nested().remove_output(OutputId(1)),
            vec![
                Destroy::Popup(PopupId(4)),
                Destroy::Popup(PopupId(3)),
                Destroy::Popup(PopupId(2)),
                Destroy::Popup(PopupId(1)),
                Destroy::Surface(SurfaceId(1)),
                Destroy::Surface(SurfaceId(2)),
            ]
        );
    }

    #[test]
    fn a_popup_parents_on_a_layer_surface_or_a_live_popup_only() {
        let targets = nested();
        assert_eq!(targets.parent_of(SurfaceId(1)).ok(), Some(Parent::Layer));
        assert_eq!(
            targets.parent_of(SurfaceId(11)).ok(),
            Some(Parent::Popup(PopupId(2)))
        );
        // A closed or never-opened popup's surface: a typed refusal.
        assert!(matches!(
            targets.parent_of(SurfaceId(99)),
            Err(RuntimeError::NoParent(SurfaceId(99)))
        ));
        let mut closed = targets;
        closed.popups.retain(|row| row.id != PopupId(3));
        assert!(matches!(
            closed.parent_of(SurfaceId(12)),
            Err(RuntimeError::NoParent(SurfaceId(12)))
        ));
    }

    #[test]
    fn keys_go_to_the_newest_popup_of_the_focused_chain() {
        let targets = nested();
        // The keyboard on the Island: the sub-submenu holds the grab.
        assert_eq!(targets.key_target(SurfaceId(1)), Some(PopupId(3)));
        // On the menu or the submenu: still the deepest one.
        assert_eq!(targets.key_target(SurfaceId(10)), Some(PopupId(3)));
        assert_eq!(targets.key_target(SurfaceId(11)), Some(PopupId(3)));
        // On the leaf: itself. On the other surface: its own menu.
        assert_eq!(targets.key_target(SurfaceId(12)), Some(PopupId(3)));
        assert_eq!(targets.key_target(SurfaceId(2)), Some(PopupId(4)));
        // A surface without popups: the keys are its own.
        let mut bare = Targets::default();
        bare.layer(SurfaceId(5), OutputId(1));
        assert_eq!(bare.key_target(SurfaceId(5)), None);
    }

    #[test]
    fn senders_cross_threads() {
        fn assert_send_and_clone<T: Send + Clone + 'static>() {}
        assert_send_and_clone::<Sender<String>>();
        assert_eq!(
            RuntimeGone.to_string(),
            "the runtime stopped receiving messages"
        );
    }

    #[test]
    fn config_defaults_to_a_plain_run() {
        let cfg = Config::default();
        assert_eq!(cfg.exit_after, None);
        assert!(!cfg.no_blur);
        assert!(!cfg.metrics);
    }
}
