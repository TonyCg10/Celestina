//! The lock profile's runtime: the sctk handler impls that turn protocol
//! events into [`Flow`] calls and the product's [`LockEvent`]s.
#![forbid(unsafe_code)]

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers as SctkModifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::{
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::{self, WpFractionalScaleV1},
    },
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use super::{
    canvas::Canvas,
    capability::UnlockCapability,
    cover,
    flow::{Effects, Emit, Flow},
    LockCtl, LockError, LockEvent, LockShell,
};
use crate::{
    input::{key_input, pointer_input, InputEvent, Modifiers},
    output::{OutputId, OutputInfo},
};

/// Everything of the runtime but the shell and the flow, so a shell call can
/// borrow them disjointly.
pub(crate) struct Env<S: LockShell> {
    pub(crate) conn: Connection,
    pub(crate) signal: calloop::LoopSignal,
    pub(crate) registry_state: RegistryState,
    pub(crate) output_state: OutputState,
    pub(crate) seat_state: SeatState,
    pub(crate) compositor: CompositorState,
    pub(crate) shm: Shm,
    /// The frames of the lock surfaces (shared memory, viewports, scale).
    pub(crate) canvas: Canvas<S>,
    pub(crate) session_lock_state: SessionLockState,
    pub(crate) session_lock: Option<SessionLock>,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) modifiers: Modifiers,
    /// The output whose lock surface holds the keyboard focus.
    pub(crate) keyboard_focus: Option<OutputId>,
}

/// The lock runtime with its shell: the loop's state type.
pub struct LockRuntime<S: LockShell> {
    env: Env<S>,
    flow: Flow<SessionLockSurface>,
    shell: S,
}

/// The protocol work of one handler call: borrows what [`Flow`] needs to act.
struct Live<'a, S: LockShell> {
    env: &'a mut Env<S>,
    shell: &'a mut S,
    qh: &'a QueueHandle<LockRuntime<S>>,
}

impl<S: LockShell> Effects for Live<'_, S> {
    type Handle = SessionLockSurface;

    fn create_surface(&mut self, output: OutputId) -> Option<SessionLockSurface> {
        let lock = self.env.session_lock.as_ref()?;
        let wl_output = self.env.output_state.outputs().find(|candidate| {
            self.env
                .output_state
                .info(candidate)
                .is_some_and(|info| info.id == output.0)
        })?;
        let surface = self.env.compositor.create_surface(self.qh);
        let handle = lock.create_lock_surface(surface, &wl_output, self.qh);
        self.env.canvas.add(output, handle.wl_surface().clone());
        Some(handle)
    }

    fn commit_cover(&mut self, surface: &SessionLockSurface, size: (u32, u32)) {
        // A frame of the product already showing at this very size stays; any
        // other configure gets the black cover back (never unlocked content).
        if self.env.canvas.configured(surface.wl_surface(), size) {
            return;
        }
        if let Err(err) = cover::commit(&self.env.shm, self.qh, surface.wl_surface(), size) {
            // The surface keeps no buffer; niri draws its lock colour there
            // and the session stays locked.
            tracing::error!(%err, "cannot commit the cover");
        }
    }

    fn emit(&mut self, event: Emit) {
        let event = match event {
            Emit::Locked => match &self.env.session_lock {
                Some(lock) => {
                    LockEvent::Locked(UnlockCapability::issue(lock.clone(), self.env.conn.clone()))
                }
                None => {
                    tracing::error!("locked without a session lock object");
                    return;
                }
            },
            Emit::Finished => {
                self.env.canvas.clear();
                LockEvent::Finished
            }
            Emit::SurfaceConfigured { output, size } => {
                LockEvent::SurfaceConfigured { output, size }
            }
            Emit::OutputAdded(info) => LockEvent::OutputAdded(info),
            Emit::OutputRemoved(id) => {
                self.env.canvas.remove(id);
                LockEvent::OutputRemoved(id)
            }
        };
        deliver_event(self.env, self.shell, event);
    }
}

fn deliver_event<S: LockShell>(env: &mut Env<S>, shell: &mut S, event: LockEvent) {
    let mut ctl = LockCtl::new(&env.signal, env.keyboard.is_some(), &mut env.canvas);
    shell.event(event, &mut ctl);
}

impl<S: LockShell> LockRuntime<S> {
    pub(crate) fn new(env: Env<S>, shell: S) -> Self {
        Self {
            env,
            flow: Flow::new(),
            shell,
        }
    }

    /// A worker's message, handed to the shell.
    pub(crate) fn message(&mut self, msg: S::Message) {
        let mut ctl = LockCtl::new(
            &self.env.signal,
            self.env.keyboard.is_some(),
            &mut self.env.canvas,
        );
        self.shell.message(msg, &mut ctl);
    }

    /// Sends the lock request and gives every known output its surface.
    pub(crate) fn request_lock(&mut self, qh: &QueueHandle<Self>) -> Result<(), LockError> {
        let lock = self
            .env
            .session_lock_state
            .lock(qh)
            .map_err(LockError::Request)?;
        self.env.session_lock = Some(lock);
        let Self { env, flow, shell } = self;
        flow.requested(&mut Live { env, shell, qh });
        Ok(())
    }

    fn live<'a>(
        &'a mut self,
        qh: &'a QueueHandle<Self>,
    ) -> (&'a mut Flow<SessionLockSurface>, Live<'a, S>) {
        let Self { env, flow, shell } = self;
        (flow, Live { env, shell, qh })
    }

    /// Delivers an input event to the shell for `output`.
    fn input(&mut self, output: OutputId, ev: InputEvent) {
        let mut ctl = LockCtl::new(
            &self.env.signal,
            self.env.keyboard.is_some(),
            &mut self.env.canvas,
        );
        self.shell.input(output, ev, &mut ctl);
    }

    fn output_of_surface(&self, surface: &wl_surface::WlSurface) -> Option<OutputId> {
        self.flow
            .output_where(|handle| handle.wl_surface() == surface)
    }
}

impl<S: LockShell> SessionLockHandler for LockRuntime<S> {
    fn locked(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: SessionLock) {
        tracing::info!("the compositor confirmed the lock");
        let (flow, mut fx) = self.live(qh);
        flow.locked(&mut fx);
    }

    fn finished(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: SessionLock) {
        tracing::warn!("the compositor ended the lock request");
        let (flow, mut fx) = self.live(qh);
        flow.finished(&mut fx);
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        serial: u32,
    ) {
        let Some(output) = self.output_of_surface(surface.wl_surface()) else {
            // A refused client still gets one for a surface the compositor
            // ignores, and a destroyed output's can arrive late.
            tracing::debug!(serial, "configure for a lock surface that is not live");
            return;
        };
        tracing::debug!(output = output.0, size = ?configure.new_size, serial, "lock surface configured");
        let (flow, mut fx) = self.live(qh);
        flow.configured(&mut fx, output, configure.new_size);
    }
}

impl<S: LockShell> CompositorHandler for LockRuntime<S> {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl<S: LockShell> OutputHandler for LockRuntime<S> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.env.output_state
    }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let Some(info) = OutputInfo::read(&self.env.output_state, &output) else {
            return;
        };
        tracing::info!(output = %info.name, id = info.id.0, "output added");
        let (flow, mut fx) = self.live(qh);
        flow.output_added(&mut fx, info);
    }

    fn update_output(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // A description that arrives again: an output whose first surface
        // could not be created gets another chance; a known one is untouched.
        let Some(info) = OutputInfo::read(&self.env.output_state, &output) else {
            return;
        };
        let (flow, mut fx) = self.live(qh);
        flow.output_added(&mut fx, info);
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let Some(info) = OutputInfo::read(&self.env.output_state, &output) else {
            return;
        };
        tracing::info!(output = %info.name, "output removed");
        if self.env.keyboard_focus == Some(info.id) {
            self.env.keyboard_focus = None;
        }
        let (flow, mut fx) = self.live(qh);
        flow.output_removed(&mut fx, info.id);
    }
}

impl<S: LockShell> SeatHandler for LockRuntime<S> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.env.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard if self.env.keyboard.is_none() => {
                match self.env.seat_state.get_keyboard(qh, &seat, None) {
                    Ok(keyboard) => {
                        self.env.keyboard = Some(keyboard);
                        deliver_event(
                            &mut self.env,
                            &mut self.shell,
                            LockEvent::Keyboard { present: true },
                        );
                    }
                    Err(err) => tracing::warn!(%err, "cannot create the keyboard"),
                }
            }
            Capability::Pointer if self.env.pointer.is_none() => {
                match self.env.seat_state.get_pointer(qh, &seat) {
                    Ok(pointer) => self.env.pointer = Some(pointer),
                    Err(err) => tracing::warn!(%err, "cannot create the pointer"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => {
                if let Some(keyboard) = self.env.keyboard.take() {
                    keyboard.release();
                    self.env.keyboard_focus = None;
                    deliver_event(
                        &mut self.env,
                        &mut self.shell,
                        LockEvent::Keyboard { present: false },
                    );
                }
            }
            Capability::Pointer => {
                if let Some(pointer) = self.env.pointer.take() {
                    pointer.release();
                }
            }
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<S: LockShell> KeyboardHandler for LockRuntime<S> {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        let Some(output) = self.output_of_surface(surface) else {
            return;
        };
        self.env.keyboard_focus = Some(output);
        tracing::debug!(output = output.0, "keyboard entered");
        self.input(output, InputEvent::FocusEnter);
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        let Some(output) = self.output_of_surface(surface) else {
            return;
        };
        if self.env.keyboard_focus == Some(output) {
            self.env.keyboard_focus = None;
        }
        self.input(output, InputEvent::FocusLost);
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, true, serial);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, true, serial);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.key_event(event, false, serial);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: SctkModifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.env.modifiers = Modifiers::from(&modifiers);
        // The state is news by itself: a Caps Lock latched before the
        // keyboard entered must show before the first key (finding M2).
        if let Some(output) = self.env.keyboard_focus {
            self.input(output, InputEvent::Modifiers(self.env.modifiers));
        }
    }
}

impl<S: LockShell> LockRuntime<S> {
    /// A key goes to the output holding the keyboard focus. Nothing here
    /// formats or logs the event: what was typed is a password.
    fn key_event(&mut self, event: KeyEvent, pressed: bool, serial: u32) {
        let Some(output) = self.env.keyboard_focus else {
            return;
        };
        self.input(
            output,
            key_input(event, self.env.modifiers, pressed, serial),
        );
    }
}

impl<S: LockShell> PointerHandler for LockRuntime<S> {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let Some(output) = self.output_of_surface(&event.surface) else {
                continue;
            };
            // Only what a lock needs: where the pointer is (the prompt
            // follows it before any keyboard focus) and that it moved.
            if !matches!(
                event.kind,
                PointerEventKind::Enter { .. }
                    | PointerEventKind::Motion { .. }
                    | PointerEventKind::Leave { .. }
                    | PointerEventKind::Press { .. }
            ) {
                continue;
            }
            if let Some(ev) = pointer_input(&event.kind, event.position) {
                self.input(output, ev);
            }
        }
    }
}

impl<S: LockShell> ShmHandler for LockRuntime<S> {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.env.shm
    }
}

impl<S: LockShell> ProvidesRegistryState for LockRuntime<S> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.env.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_registry!(@<S: LockShell> LockRuntime<S>);
smithay_client_toolkit::delegate_dispatch2!(@<S: LockShell> LockRuntime<S>);
wayland_client::delegate_noop!(@<S: LockShell> LockRuntime<S>: ignore wl_buffer::WlBuffer);

/// User data of a frame buffer: which one it is, so its `release` finds it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BufferTag {
    pub(crate) output: OutputId,
    /// Which allocation of the output's pool, so the release of a buffer that
    /// was replaced is ignored.
    pub(crate) generation: u64,
    pub(crate) index: usize,
}

impl<S: LockShell> Dispatch2<wl_buffer::WlBuffer, LockRuntime<S>> for BufferTag {
    fn event(
        &self,
        rt: &mut LockRuntime<S>,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &Connection,
        _: &QueueHandle<LockRuntime<S>>,
    ) {
        if matches!(event, wl_buffer::Event::Release) && rt.env.canvas.released(*self) {
            deliver_event(&mut rt.env, &mut rt.shell, LockEvent::Redraw);
        }
    }
}

/// User data of a surface's fractional-scale object.
pub(crate) struct ScaleTag(pub(crate) OutputId);

impl<S: LockShell> Dispatch2<WpFractionalScaleV1, LockRuntime<S>> for ScaleTag {
    fn event(
        &self,
        rt: &mut LockRuntime<S>,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &Connection,
        _: &QueueHandle<LockRuntime<S>>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            tracing::debug!(output = self.0 .0, preferred = scale, "preferred scale");
            if rt.env.canvas.preferred(self.0, scale) {
                deliver_event(&mut rt.env, &mut rt.shell, LockEvent::Redraw);
            }
        }
    }
}

/// User data of the objects that declare no events: one that sends any is a
/// compositor bug, logged and ignored.
pub(crate) struct NoEvents;

macro_rules! no_events {
    ($($iface:ty),* $(,)?) => {
        $(
            impl<S: LockShell> Dispatch2<$iface, LockRuntime<S>> for NoEvents {
                fn event(
                    &self,
                    _: &mut LockRuntime<S>,
                    _: &$iface,
                    event: <$iface as Proxy>::Event,
                    _: &Connection,
                    _: &QueueHandle<LockRuntime<S>>,
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

no_events!(WpViewporter, WpViewport, WpFractionalScaleManagerV1);
